//! `ZTGuide`: the tour-guide staff entity, a `ZTStaff` subclass (`ZTUnit` -> `BFUnit` -> `BFEntity`).
//!
//! Vanilla-layout-compatible (style 1): guides are heap entities that un-ported `ZTGuest` code (`followGuide`, the
//! `releaseGuests` callers) and the goal classes (`ZTGoalGuideDuties`, `ZTGoalGuideVA`) read by raw offset, so the
//! object stays vanilla memory and this module reads and writes it in place. A guide is `0x2a4` bytes: the `ZTStaff`
//! prefix (`0x000`-`0x293`) followed by the fields below. Nothing here owns an allocation. The follower list's
//! nodes (`0x294`) come from vanilla's 12-byte small-block freelist, shared with `ZTGuest`; code that touches them
//! goes through that freelist and never through Rust's allocator.
//!
//! This module holds `fRoutine`, `fVARoutine`, `getTerrainCost`, `fencesOK`, `isSpeaking`, `doConditionals`, the
//! follower-list methods (`addGuest`, `removeGuest`, `releaseGuests`, `clearGoal`, `removeFromMap`, `initStatusVars`) and
//! `save`/`load`. The constructor and destructor stay vanilla: the constructor allocates the list sentinel from the
//! 12-byte freelist, and the destructor frees it through helpers `generated.rs` does not expose.
//! The `"Doing VA Duties"` debug string `fVARoutine` builds and frees has no observable effect and is not
//! reproduced.

use openzt_detour::generated::{
    bfcoggoal::SET_ID as COG_GOAL_SET_ID,
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfmap::GET_NEIGHBOR_0 as BFMAP_GET_NEIGHBOR,
    bftile::IS_IN_ZOO as BFTILE_IS_IN_ZOO,
    bfunit::{IDLE as BFUNIT_IDLE, IS_MAX_COST_FOR_ALL, IS_NON_PATH_STEEP, IS_SAME_TILE},
    msvc_std::RAND,
    ztguide::{
        ADD_GUEST, CLEAR_GOAL, DO_CONDITIONALS, FENCES_OK, F_ROUTINE, F_VAROUTINE, GET_HAB_FROM_VA, GET_NEEDIEST_VA, GET_RANDOM_ASSIGNED_HABITAT, GET_TERRAIN_COST,
        INIT_STATUS_VARS, IS_SPEAKING, LOAD, RELEASE_GUESTS, REMOVE_FROM_MAP, REMOVE_GUEST, SAVE, TILE_FILTER, TRIGGER_NEARBY_GUESTS, UPDATE_GOALS,
    },
    ztguest::FOLLOW_GUIDE,
    ztstaff::{
        DO_CONDITIONALS as ZTSTAFF_DO_CONDITIONALS, INIT_STATUS_VARS as ZTSTAFF_INIT_STATUS_VARS, IS_HABITAT_ASSIGNED as ZTSTAFF_IS_HABITAT_ASSIGNED,
        LOAD as ZTSTAFF_LOAD, REMOVE_FROM_MAP as ZTSTAFF_REMOVE_FROM_MAP, SAVE as ZTSTAFF_SAVE,
    },
    ztunit::{CLEAR_GOAL as ZTUNIT_CLEAR_GOAL, UPDATE_GOALS as ZTUNIT_UPDATE_GOALS},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    zthabitat::{next_game_random, viewing_area::has_oa, ZTHabitat, MAX_PATH_COST_RVA},
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, read_field, unit_offsets, write_field},
    ztguest::guest_offsets,
    ztmegatilemgr::entity_type_matches,
    vanilla_list,
};

/// `ZTGuideType::createEntity` allocates this many bytes.
pub const GUIDE_SIZE: u32 = 0x2a4;

/// Vtable address of `ZTGuide` (`0x0062f714`) as an RVA from the `zoo.exe` base.
pub const GUIDE_VTABLE_RVA: u32 = 0x0022_f714;

/// Class tag (RVA) the entity type's `isCastClass` takes to test for a guide type (`CAST_ZTGuide`).
pub const GUIDE_TYPE_TAG_RVA: u32 = 0x0023_8770;

/// Sentinel of the `list<uint>` of followers' entity ids. Nodes are `{next +0x0, previous +0x4, guest id +0x8}`.
pub const FOLLOWERS_OFFSET: u32 = 0x294;
/// Nothing but `initStatusVars` (zero) and `load` writes it; `getRandomAssignedHabitat` and `getNeediestVA` return
/// null while it is non-zero. Its role is unknown.
pub const FIELD_0X298_OFFSET: u32 = 0x298;
/// Crowd-check countdown. `doConditionals` counts it down and, when it was zero, reloads it from the type's
/// `cCrowdCheck` minus one.
pub const CROWD_CHECK_COUNTDOWN_OFFSET: u32 = 0x29c;
/// Set to `1` by `ZTGoalGuideVA::checkDone`, which also reads it (zero means guests not yet informed at this viewing
/// area); cleared by `removeFromMap`, `clearGoal`, `initStatusVars` and `updateGoals`.
pub const VIEWING_AREA_INFORMED_OFFSET: u32 = 0x2a0;

/// A `ZTUnit` byte `doConditionals` and `ZTGoalGuideVA::checkDone` test before informing guests on guide duties.
const INFORM_ARMED_OFFSET: u32 = 0x20c;

/// `ZTGuideType` fields.
mod type_offsets {
    /// `cCrowdCheck`: the period of the crowd-check countdown (`0` disables the reload).
    pub const CROWD_CHECK: u32 = 0x1fc;
    /// `cCrowdRadius`, `cFollowChance`, `cMaxGroupSize`.
    pub const CROWD_RADIUS: u32 = 0x200;
    pub const FOLLOW_CHANCE: u32 = 0x204;
    pub const MAX_GROUP_SIZE: u32 = 0x208;
    /// The type's name string (`{begin, end}`); `triggerNearbyGuests` compares it with `"f"` and `"m"`.
    pub const NAME_BEGIN: u32 = 0x98;
    pub const NAME_END: u32 = 0x9c;
    /// `initStatusVars` links the unit's `+0x68` to this block, present when `+0x84` is non-zero.
    pub const BLOCK: u32 = 0x80;
    pub const BLOCK_GATE: u32 = 0x84;
}

/// `BFTile` fields.
mod tile_offsets {
    /// The four edge slots, one fence entity pointer each, at `+0x14 + (direction / 2) * 4`.
    pub const EDGE_SLOTS: u32 = 0x14;
    /// Byte tested against `0x8`.
    pub const FLAGS_83: u32 = 0x83;
    /// The tile's grid position.
    pub const X: u32 = 0x34;
    pub const Y: u32 = 0x38;
    /// Size in bytes of a tile in the map's tile array.
    pub const SIZE: u32 = 0x8c;
    /// First word of a tile: pointer to the sentinel of its entity list (nodes `{next, previous, entity +0x8}`).
    pub const ENTITY_LIST: u32 = 0;
}

/// `BFMap` fields, relative to `ZTWorldMgr + 0x8`.
mod map_offsets {
    pub const X_SIZE: u32 = 0x2c;
    pub const Y_SIZE: u32 = 0x30;
    pub const TILE_ARRAY: u32 = 0x38;
}

/// `ZTFence` fields.
mod fence_offsets {
    /// Set on a fence that lets units through (an open gate).
    pub const PASSABLE: u32 = 0x164;
    /// The entity type's name pointer lives at `+0xa4` of the type; a fence type starting with `'g'` is a gate.
    pub const TYPE_NAME_POINTER: u32 = 0xa4;
}

/// The unit's own string at `+0x54`/`+0x58` (`{begin, end}`; the current behavior name) `isSpeaking` compares with `"bSpeak"`.
mod unit_string {
    pub const BEGIN: u32 = 0x54;
    pub const END: u32 = 0x58;
}

const SPEAK_STRING: &[u8] = b"bSpeak";

/// `ZTWorldMgr` fields: the entity pointer vector's begin and end.
mod world_offsets {
    pub const ENTITY_BEGIN: u32 = 0x80;
    pub const ENTITY_END: u32 = 0x84;
}

/// Vtable slot of `ZTUnit::clearGoal`, dispatched on a guest.
const SLOT_CLEAR_GOAL: u32 = 0x174;
/// Vtable slot of `getRandomAssignedHabitat`, dispatched on the guide.
const SLOT_GET_RANDOM_ASSIGNED_HABITAT: u32 = 0x328;

/// Save versions that change what `load` reads.
mod save_version {
    /// From here on `+0x298` is saved.
    pub const FIELD_0X298: u32 = 0x10;
    /// Version `0x10` alone saved two more bytes that `load` reads and discards, then went straight to `ZTStaff::load`.
    pub const LEGACY_BYTES: u32 = 0x10;
    /// From here on the countdown is saved.
    pub const COUNTDOWN: u32 = 0x36;
    /// From here on the follower list is saved; below it, versions `0x36..0x41` carry one discarded dword instead.
    pub const FOLLOWERS: u32 = 0x41;
    /// From here on the `+0x2a0` byte is saved.
    pub const INFORMED_BYTE: u32 = 0x42;
}

/// Goal ids.
const GOAL_GUIDE_DUTIES: u32 = 0x76;
const GOAL_VA_DUTIES: u32 = 0x77;
const GOAL_PREDATOR: u32 = 0x75;
/// Goal set when the zoo has no habitats.
const GOAL_NO_HABITAT: u32 = 1;

/// Vtable slots dispatched on the guide.
const SLOT_GOAL_SET: u32 = 0x1d8;
const SLOT_SET_TARGET_TILE: u32 = 0x2d8;
const SLOT_FIND_PREDATOR: u32 = 0x2fc;
/// Vtable slot dispatched on a tile's entities by `tileFilter`'s open-slot test.
const SLOT_TILE_ENTITY_BLOCKS: u32 = 0xb4;

/// `BFCogGoal::setID`'s trailing argument (`DAT_00639360`) as an RVA.
const COG_GOAL_ARG_RVA: u32 = 0x0023_9360;
/// `ZTHabitatMgr`'s habitat pointer array: begin and end.
const HABITAT_ARRAY_BEGIN_OFFSET: u32 = 0x1c;
const HABITAT_ARRAY_END_OFFSET: u32 = 0x20;

/// Globals, as RVAs from the `zoo.exe` base.
mod globals_rva {
    /// `GLOBAL_ZTWorldMgr`; the `BFMap` is embedded at `+0x8`.
    pub const WORLD_MGR: u32 = 0x0023_8040;
    /// `CAST_ZTGuest`.
    pub const GUEST_TAG: u32 = 0x0023_8700;
    /// `GLOBAL_ZTHabitatMgr`.
    pub const HABITAT_MGR: u32 = 0x0023_805c;
    /// `CAST_ZTFence`.
    pub const FENCE_TAG: u32 = 0x0023_8660;
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// Whether `GLOBAL_ZTHabitatMgr` is non-null, read live as vanilla does.
fn habitat_mgr_present() -> bool {
    get_from_memory::<u32>(base() + globals_rva::HABITAT_MGR) != 0
}

/// A guide in vanilla memory. All reads and writes are volatile through the raw address.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTGuide(pub u32);

impl ZTGuide {
    fn goal_id(self) -> u32 {
        get_from_memory(self.0 + unit_offsets::GOAL_ID)
    }

    pub fn crowd_check_countdown(self) -> i32 {
        get_from_memory(self.0 + CROWD_CHECK_COUNTDOWN_OFFSET)
    }

    pub fn set_crowd_check_countdown(self, value: i32) {
        save_to_memory(self.0 + CROWD_CHECK_COUNTDOWN_OFFSET, value);
    }

    /// The guide's `ZTGuideType` pointer, or `0` when the entity type is missing or not a guide type.
    fn guide_type(self) -> u32 {
        if unsafe { entity_type_matches(self.0, GUIDE_TYPE_TAG_RVA) } {
            get_from_memory(self.0 + unit_offsets::ENTITY_TYPE)
        } else {
            0
        }
    }

    fn followers(self) -> u32 {
        get_from_memory(self.0 + FOLLOWERS_OFFSET)
    }

    /// Frees every follower node onto the freelist and leaves the list empty and self-linked.
    fn empty_followers(self) {
        vanilla_list::clear(self.followers());
    }

    /// `ZTGuide::triggerNearbyGuests` (`0x0058d349`): when the guide has fewer followers than its type's `cMaxGroupSize`,
    /// scans the tiles within `cCrowdRadius` of the guide (columns outer, rows inner, clipped to the map). For each
    /// guest entity on them the follow chance (`cFollowChance`, plus 15 when the type's name is `"f"` and the guest's
    /// `+0x320` is `0`, or plus 10 when it is `"m"` and `+0x320` is `1`; the bonus accumulates across guests, as in
    /// vanilla) is rolled against one shared-RNG draw modulo 100; a guest rolled in, or one not yet following anyone
    /// (`+0x337` clear) that already follows this guide (`+0x2e4`), is handed to `ZTGuest::followGuide` while the group
    /// still has room. Does nothing when the entity type is not a guide type, where vanilla would fault.
    pub fn trigger_nearby_guests(self) {
        let guide_type = self.guide_type();
        if guide_type == 0 {
            return;
        }
        let tile = unsafe { BFENTITY_GET_TILE.original()(self.0 as *const u32) } as u32;
        let radius: i32 = get_from_memory(guide_type + type_offsets::CROWD_RADIUS);
        let mut chance: i32 = get_from_memory(guide_type + type_offsets::FOLLOW_CHANCE);
        let max_group = || get_from_memory::<i32>(guide_type + type_offsets::MAX_GROUP_SIZE);
        if self.follower_count() >= max_group() {
            return;
        }
        let own_id: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_ID);
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let map = world.wrapping_add(8);
        let (tile_x, tile_y): (i32, i32) = (get_from_memory(tile + tile_offsets::X), get_from_memory(tile + tile_offsets::Y));
        let name_begin: u32 = get_from_memory(guide_type + type_offsets::NAME_BEGIN);
        let name_end: u32 = get_from_memory(guide_type + type_offsets::NAME_END);
        let name = (name_end.wrapping_sub(name_begin) == 1).then(|| get_from_memory::<u8>(name_begin));
        let (radius, tile_x, tile_y) = (radius as i64, tile_x as i64, tile_y as i64);
        for x in (tile_x - radius)..=(tile_x + radius) {
            for y in (tile_y - radius)..=(tile_y + radius) {
                let (x_size, y_size): (i32, i32) = (get_from_memory(map + map_offsets::X_SIZE), get_from_memory(map + map_offsets::Y_SIZE));
                if x < 0 || y < 0 || x >= x_size as i64 || y >= y_size as i64 {
                    continue;
                }
                let index = (x_size as u32).wrapping_mul(y as u32).wrapping_add(x as u32);
                let tile_address = get_from_memory::<u32>(map + map_offsets::TILE_ARRAY).wrapping_add(index.wrapping_mul(tile_offsets::SIZE));
                let mut node: u32 = get_from_memory(get_from_memory::<u32>(tile_address + tile_offsets::ENTITY_LIST));
                while node != get_from_memory::<u32>(tile_address + tile_offsets::ENTITY_LIST) {
                    let guest: u32 = get_from_memory(node + 8);
                    if guest != 0 && unsafe { entity_type_matches(guest, globals_rva::GUEST_TAG) } {
                        let state: u32 = get_from_memory(guest + guest_offsets::STATE);
                        match name {
                            Some(b'f') if state == 0 => chance = chance.wrapping_add(15),
                            Some(b'm') if state == 1 => chance = chance.wrapping_add(10),
                            _ => {}
                        }
                        let rolled_in = ((next_game_random() % 100) as i32) < chance;
                        let already_ours = get_from_memory::<u8>(guest + guest_offsets::FOLLOWING_FLAG) == 0
                            && get_from_memory::<u32>(guest + guest_offsets::GUIDE_ID) == own_id;
                        if (rolled_in || already_ours) && self.follower_count() < max_group() {
                            // `.hooked()`: what vanilla's direct call runs (`ZTGuest` is not ported, so this is the real body).
                            unsafe { FOLLOW_GUIDE.hooked()(guest as *const u32, self.0 as *const std::ffi::c_void) };
                        }
                    }
                    node = get_from_memory(node);
                }
            }
        }
    }

    /// `ZTGuide::addGuest` (`0x0058dad9`): appends `guest_id` to the follower list unless it is already there, through
    /// vanilla's `list<uint>::insert` at the end position so the node comes from the 12-byte freelist.
    pub fn add_guest(self, guest_id: i32) {
        let sentinel = self.followers();
        if vanilla_list::nodes(sentinel).any(|node| vanilla_list::value(node) == guest_id as u32) {
            return;
        }
        vanilla_list::push_back(sentinel, guest_id as u32);
    }

    /// Number of followers.
    pub fn follower_count(self) -> i32 {
        let sentinel = self.followers();
        let mut node: u32 = get_from_memory(sentinel);
        let mut count = 0;
        while node != sentinel {
            count += 1;
            node = get_from_memory(node);
        }
        count
    }

    /// `ZTGuide::removeGuest` (`0x0058aab4`): unlinks and frees every follower node whose id is `guest_id`.
    pub fn remove_guest(self, guest_id: i32) {
        vanilla_list::remove_matching(self.followers(), guest_id as u32);
    }

    /// `ZTGuide::releaseGuests` (`0x0058b362`): every guest in the world that follows this guide is, with a
    /// `percent` in 100 chance (one shared-RNG draw per such guest), sent its `clearGoal` (vtable `+0x174`), has its
    /// `+0x337` byte cleared and is removed from the follower list. The world's entity vector end is re-read every
    /// step, as vanilla does.
    pub fn release_guests(self, percent: i32) {
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let end = || get_from_memory::<u32>(world + world_offsets::ENTITY_END);
        let mut slot: u32 = get_from_memory(world + world_offsets::ENTITY_BEGIN);
        if slot == end() {
            return;
        }
        let own_id: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_ID);
        loop {
            let guest: u32 = get_from_memory(slot);
            if guest != 0
                && unsafe { entity_type_matches(guest, globals_rva::GUEST_TAG) }
                && get_from_memory::<u32>(guest + guest_offsets::GUIDE_ID) == own_id
                && ((next_game_random() % 100) as i32) < percent
            {
                call_slot_0(guest, SLOT_CLEAR_GOAL);
                save_to_memory(guest + guest_offsets::FOLLOWING_FLAG, 0u8);
                self.remove_guest(get_from_memory::<i32>(guest + unit_offsets::ENTITY_ID));
            }
            slot = slot.wrapping_add(4);
            if slot == end() {
                break;
            }
        }
    }

    /// `ZTGuide::clearGoal` (`0x0061b9a8`, vtable `+0x174`): `ZTUnit::clearGoal`, then the follower list is emptied
    /// and `+0x2a0` cleared.
    pub fn clear_goal(self) {
        unsafe { ZTUNIT_CLEAR_GOAL.original()(self.0 as *const u32) };
        self.empty_followers();
        save_to_memory(self.0 + VIEWING_AREA_INFORMED_OFFSET, 0u8);
    }

    /// `ZTGuide::removeFromMap` (`0x0058952b`): a guide removed for good (`param` false) first releases its guests
    /// unconditionally (`percent` 100); `+0x2a0` is cleared, then the `ZTStaff` removal.
    pub fn remove_from_map(self, param: bool) {
        if !param {
            self.release_guests(100);
        }
        save_to_memory(self.0 + VIEWING_AREA_INFORMED_OFFSET, 0u8);
        unsafe { ZTSTAFF_REMOVE_FROM_MAP.original()(self.0 as *const u32, param) };
    }

    /// `ZTGuide::initStatusVars` (`0x0059b2cf`): the `ZTStaff` reset; then, for a guide type, `+0x298` is zeroed, the
    /// countdown is seeded (`-1`, or one shared-RNG draw modulo the type's `cCrowdCheck`), the follower list is
    /// emptied, `+0x2a0` is cleared, `+0x64` links to the unit itself and `+0x68` to the type's `+0x80` block (null
    /// when the type's `+0x84` is clear). Returns `true` for a guide type, `false` for another type, and the
    /// `ZTStaff` result when the unit has no type.
    pub fn init_status_vars(self) -> bool {
        let staff_result = unsafe { ZTSTAFF_INIT_STATUS_VARS.original()(self.0 as *const u32) };
        let entity_type: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_TYPE);
        if entity_type == 0 {
            return low_byte_bool(staff_result);
        }
        if self.guide_type() == 0 {
            return false;
        }
        save_to_memory(self.0 + FIELD_0X298_OFFSET, 0u32);
        let period: i32 = get_from_memory(entity_type + type_offsets::CROWD_CHECK);
        let countdown = if period == 0 { -1 } else { (next_game_random() as i32).wrapping_rem(period) };
        self.set_crowd_check_countdown(countdown);
        self.empty_followers();
        save_to_memory(self.0 + VIEWING_AREA_INFORMED_OFFSET, 0u8);
        save_to_memory(self.0 + unit_offsets::SELF_POINTER, self.0);
        let block = if get_from_memory::<u32>(entity_type + type_offsets::BLOCK_GATE) == 0 { 0 } else { entity_type + type_offsets::BLOCK };
        save_to_memory(self.0 + unit_offsets::TYPE_BLOCK_POINTER, block);
        true
    }

    /// `ZTGuide::save` (`0x0047987f`): `+0x298`, the countdown, the follower count, one id per follower, the `+0x2a0`
    /// byte, then the `ZTStaff` data. Every write and the `ZTStaff` save run even after a failure; the result is
    /// their conjunction.
    pub fn save(self, file: *const u32) -> bool {
        let mut ok = write_field(self.0 + FIELD_0X298_OFFSET, 4, file);
        ok &= write_field(self.0 + CROWD_CHECK_COUNTDOWN_OFFSET, 4, file);
        let count = self.follower_count();
        ok &= write_field(&count as *const i32 as u32, 4, file);
        let mut node: u32 = get_from_memory(self.followers());
        for _ in 0..count {
            let id: u32 = get_from_memory(node + 8);
            node = get_from_memory(node);
            ok &= write_field(&id as *const u32 as u32, 4, file);
        }
        ok &= write_field(self.0 + VIEWING_AREA_INFORMED_OFFSET, 1, file);
        let staff_ok = unsafe { ZTSTAFF_SAVE.original()(self.0 as *const u32, file) };
        staff_ok && ok
    }

    /// `ZTGuide::load` (`0x0059b3a1`), from the `.asm`: `+0x298` from version `0x10`; version `0x10` then reads two
    /// discarded bytes and goes straight to `ZTStaff::load`. From `0x36` the countdown; below `0x41` one discarded
    /// dword; from `0x41` the old follower list is emptied, the count read (a failure so far returns `false` here),
    /// then each id read and appended through vanilla's `list<uint>::insert` (a failed read still appends the previous
    /// value); from `0x42` the `+0x2a0` byte. Any failed read returns `false` without running `ZTStaff::load`.
    pub fn load(self, file: *const u32, version: u32) -> bool {
        let mut ok = true;
        if version >= save_version::FIELD_0X298 {
            ok = read_field(self.0 + FIELD_0X298_OFFSET, 4, file);
            if version == save_version::LEGACY_BYTES {
                let mut discarded: u8 = 0;
                ok &= read_field(&mut discarded as *mut u8 as u32, 1, file);
                ok &= read_field(&mut discarded as *mut u8 as u32, 1, file);
                return self.finish_load(ok, file, version);
            }
        }
        if version >= save_version::COUNTDOWN {
            ok &= read_field(self.0 + CROWD_CHECK_COUNTDOWN_OFFSET, 4, file);
            if version < save_version::FOLLOWERS {
                let mut discarded: u32 = 0;
                ok &= read_field(&mut discarded as *mut u32 as u32, 4, file);
            } else {
                self.empty_followers();
                let mut count: i32 = 0;
                ok &= read_field(&mut count as *mut i32 as u32, 4, file);
                if !ok {
                    return false;
                }
                let mut id: u32 = 0;
                for _ in 0..count.max(0) {
                    ok &= read_field(&mut id as *mut u32 as u32, 4, file);
                    vanilla_list::push_back(self.followers(), id);
                }
                if version >= save_version::INFORMED_BYTE {
                    ok &= read_field(self.0 + VIEWING_AREA_INFORMED_OFFSET, 1, file);
                }
            }
        }
        self.finish_load(ok, file, version)
    }

    fn finish_load(self, ok: bool, file: *const u32, version: u32) -> bool {
        if !ok {
            return false;
        }
        unsafe { ZTSTAFF_LOAD.original()(self.0 as *const u32, file, version) }
    }

    /// The habitats the guide is assigned to, in list order (`ZTStaff`'s list at `+0x260`).
    fn assigned_habitats(self) -> Vec<u32> {
        let sentinel: u32 = get_from_memory(self.0 + unit_offsets::ASSIGNED_HABITATS);
        let mut habitats = Vec::new();
        let mut node: u32 = get_from_memory(sentinel);
        while node != sentinel {
            habitats.push(get_from_memory(node + 8));
            node = get_from_memory(node);
        }
        habitats
    }

    /// `ZTGuide::getHabFromVA` (`0x0061b68a`): a habitat whose viewing area on `tile` can be observed. Starts at one of
    /// the tile's eight viewing-area slots chosen by the CRT `rand()` (modulo 8) and takes the first slot whose viewing
    /// area `hasOA` and whose habitat the guide is assigned to (any habitat when the guide has no assignments). `0`
    /// when there is no habitat manager or tile, or no slot qualifies. A tile outside the manager's grid reads as an
    /// empty slot where vanilla would read out of bounds.
    pub fn get_hab_from_va(self, tile: u32) -> u32 {
        if !habitat_mgr_present() || tile == 0 {
            return 0;
        }
        // `.hooked()`: what vanilla's direct call runs, so a test can script the draw.
        let start = (unsafe { RAND.hooked()() } as i32).wrapping_rem(8);
        let row = globals().zthabitatmgr().get_habitat_cell_addr(get_from_memory(tile + tile_offsets::X), get_from_memory(tile + tile_offsets::Y));
        for step in 0..8i32 {
            let slot = step.wrapping_add(start).wrapping_rem(8);
            let viewing_area: u32 = match row {
                Some(row) => get_from_memory(row.wrapping_add(4).wrapping_add((slot as u32).wrapping_mul(4))),
                None => 0,
            };
            if viewing_area == 0 || !has_oa(viewing_area) {
                continue;
            }
            let habitat: u32 = get_from_memory(viewing_area);
            if self.assigned_habitats().is_empty() || low_byte_bool(unsafe { ZTSTAFF_IS_HABITAT_ASSIGNED.original()(self.0 as *const u32, habitat as *const u32) }) {
                return habitat;
            }
        }
        0
    }

    /// `ZTGuide::getRandomAssignedHabitat` (`0x0058af73`, vtable `+0x328`): a random habitat from the assigned list, or,
    /// with no assignments, from the habitats with animals of their own (`ZTHabitatMgr::getNonemptyNonWorldHabitats`'s
    /// set). `0` when there is no habitat manager, `+0x298` is non-zero, or nothing qualifies. One shared-RNG draw, only
    /// when there is a choice. Vanilla builds the candidates in a pool-allocated temporary vector; here it is a `Vec`.
    pub fn get_random_assigned_habitat(self) -> u32 {
        if !habitat_mgr_present() || get_from_memory::<u32>(self.0 + FIELD_0X298_OFFSET) != 0 {
            return 0;
        }
        let mut candidates = self.assigned_habitats();
        if candidates.is_empty() {
            let exhibits = globals().zthabitatmgr().exhibit_array();
            candidates = (0..exhibits.len())
                .map(|index| exhibits.get_ptr(index))
                .filter(|&habitat| unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_num_animals(false) != 0)
                .collect();
        }
        if candidates.is_empty() {
            return 0;
        }
        candidates[next_game_random() as usize % candidates.len()]
    }

    /// `ZTGuide::getNeediestVA` (`0x0058b0a3`): picks a habitat through the virtual `getRandomAssignedHabitat` (vtable
    /// `+0x328`), stores it at `*out`, then returns a viewing area of it: one shared-RNG draw, whose low bit selects a
    /// random viewing area (clear) or the one nearest the guide (set). `0` with no habitat manager (`*out` untouched),
    /// with `+0x298` non-zero (`*out` untouched), or with no habitat (`*out` set to `0`).
    pub fn get_neediest_va(self, out: u32) -> u32 {
        if !habitat_mgr_present() || get_from_memory::<u32>(self.0 + FIELD_0X298_OFFSET) != 0 {
            return 0;
        }
        let habitat = call_slot_0(self.0, SLOT_GET_RANDOM_ASSIGNED_HABITAT);
        save_to_memory(out, habitat);
        if habitat == 0 {
            return 0;
        }
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat) };
        if next_game_random() & 1 == 0 {
            habitat.get_random_viewing_area(true)
        } else {
            let tile = unsafe { BFENTITY_GET_TILE.original()(self.0 as *const u32) } as u32;
            habitat.get_nearest_viewing_area(tile, true)
        }
    }

    /// `ZTGuide::fRoutine` (`0x0058a929`): on the viewing-area-duties goal with the cog-goal flag clear, hands over to
    /// `fVARoutine`; otherwise reports whether the unit is stopped and lets `BFUnit::idle` run. Always `true`.
    pub fn f_routine(this: u32, done: *const bool) -> bool {
        let guide = ZTGuide(this);
        if guide.goal_id() == GOAL_VA_DUTIES && get_from_memory::<u8>(this + unit_offsets::COG_GOAL_FLAG) == 0 {
            Self::f_va_routine(this, done);
            return true;
        }
        let stopped: u8 = get_from_memory(this + unit_offsets::STOPPED);
        save_to_memory(done as u32, stopped);
        unsafe { BFUNIT_IDLE.original()(this as *const u32) };
        true
    }

    /// `ZTGuide::fVARoutine` (`0x0058abff`): reports whether the unit is stopped. Always `true`.
    pub fn f_va_routine(this: u32, done: *const bool) -> bool {
        let stopped: u8 = get_from_memory(this + unit_offsets::STOPPED);
        save_to_memory(done as u32, stopped);
        true
    }

    /// Whether any of a tile's four entity slots (`+0x4`..`+0x10`) is empty or holds an entity whose vtable `+0xb4`
    /// answers true (`BFTile::meth_0x43f578`, `0x0043f578`).
    fn tile_slot_open(tile: u32) -> bool {
        (1..=4u32).any(|slot| {
            let entity: u32 = get_from_memory(tile + slot * 4);
            entity == 0 || call_slot_0(entity, SLOT_TILE_ENTITY_BLOCKS) & 0xff != 0
        })
    }

    /// `ZTGuide::tileFilter` (`0x0061b639`): whether `tile` is in the zoo and either carries the `0x8` flag (byte
    /// `+0x83`) or, with `flag` set, the guide's own tile lacks it, and has an open entity slot.
    pub fn tile_filter(self, tile: u32, flag: bool) -> bool {
        let own_tile = unsafe { BFENTITY_GET_TILE.original()(self.0 as *const u32) } as u32;
        if tile == 0 || !low_byte_bool(unsafe { BFTILE_IS_IN_ZOO.original()(tile as *const u32, 0) }) {
            return false;
        }
        let passes = (flag && get_from_memory::<u8>(own_tile + tile_offsets::FLAGS_83) & 0x8 == 0)
            || get_from_memory::<u8>(tile + tile_offsets::FLAGS_83) & 0x8 != 0;
        passes && Self::tile_slot_open(tile)
    }

    /// `ZTGuide::getTerrainCost` (`0x0058cfd6`): the pathfinder's cost of stepping onto `tile`. The unit's own tile
    /// costs `1`; a tile that is maximally costly for everyone costs the path-cost sentinel; a steep one costs the
    /// sentinel minus 4; a tile flagged `0x8` in byte `+0x83` costs `1`. Ordinary ground costs the sentinel, or the
    /// sentinel minus 3 when the guide's own tile lacks that flag.
    pub fn get_terrain_cost(self, tile: u32) -> i32 {
        let max_cost: i32 = get_from_memory(base() + MAX_PATH_COST_RVA);
        let this = self.0 as *const u32;
        let tile_ptr = tile as *const u32;
        if unsafe { IS_SAME_TILE.original()(this, tile_ptr) } != 0 {
            return 1;
        }
        if low_byte_bool(unsafe { IS_MAX_COST_FOR_ALL.original()(this, tile_ptr) }) {
            return max_cost;
        }
        if low_byte_bool(unsafe { IS_NON_PATH_STEEP.original()(tile as i32) }) {
            return max_cost - 4;
        }
        if get_from_memory::<u8>(tile + tile_offsets::FLAGS_83) & 0x8 != 0 {
            return 1;
        }
        let own_tile = unsafe { BFENTITY_GET_TILE.original()(this) } as u32;
        if get_from_memory::<u8>(own_tile + tile_offsets::FLAGS_83) & 0x8 == 0 {
            max_cost - 3
        } else {
            max_cost
        }
    }

    /// `ZTGuide::fencesOK` (`0x0058cf4e`): whether the step from `tile` in `direction` crosses no closed fence. Clears
    /// the byte at `out` first. A missing tile or neighbour is `false`; direction `-1` (no edge) is `true`. The fence on
    /// the tile's edge and the fence on the neighbour's far edge (`(direction - 4) & 7`) must each be absent, open
    /// (`+0x164` set) or a gate (type name starting with `'g'`).
    pub fn fences_ok(self, tile: u32, direction: i32, out: u32) -> bool {
        save_to_memory(out, 0u8);
        if tile == 0 {
            return false;
        }
        if direction == -1 {
            return true;
        }
        let world: u32 = get_from_memory(base() + globals_rva::WORLD_MGR);
        let map = world.wrapping_add(8);
        let neighbour = unsafe { BFMAP_GET_NEIGHBOR.original()(map as *const u32, tile as *const u32, direction as u32) };
        if neighbour == 0 {
            return false;
        }
        let near_fence = edge_fence(tile, direction);
        let far_fence = edge_fence(neighbour, direction.wrapping_sub(4) & 7);
        [near_fence, far_fence].into_iter().all(|fence| fence == 0 || fence_lets_through(fence))
    }

    /// `ZTGuide::isSpeaking` (`0x0061b964`): on the viewing-area-duties goal, or when the unit's own string at `+0x54`
    /// is `"bSpeak"`.
    pub fn is_speaking(self) -> bool {
        if self.goal_id() == GOAL_VA_DUTIES {
            return true;
        }
        let begin: u32 = get_from_memory(self.0 + unit_string::BEGIN);
        let end: u32 = get_from_memory(self.0 + unit_string::END);
        if end.wrapping_sub(begin) as usize != SPEAK_STRING.len() {
            return false;
        }
        unsafe { std::slice::from_raw_parts(begin as *const u8, SPEAK_STRING.len()) == SPEAK_STRING }
    }

    /// `ZTGuide::doConditionals` (`0x0058ceb9`): counts the crowd-check countdown down. When it was zero it reloads
    /// from the type's `cCrowdCheck` minus one (left as counted when the entity type is not a guide type, where vanilla
    /// would fault) and, on the viewing-area-duties goal, or on guide duties with the `+0x20c` byte set, informs
    /// nearby guests. Always ends in `ZTStaff::doConditionals`.
    pub fn do_conditionals(self) {
        let guide_type = self.guide_type();
        let previous = self.crowd_check_countdown();
        self.set_crowd_check_countdown(previous.wrapping_sub(1));
        if previous == 0 {
            if guide_type != 0 {
                let period: i32 = get_from_memory(guide_type + type_offsets::CROWD_CHECK);
                self.set_crowd_check_countdown(period.wrapping_sub(1));
            }
            let inform = match self.goal_id() {
                GOAL_GUIDE_DUTIES => get_from_memory::<u8>(self.0 + INFORM_ARMED_OFFSET) != 0,
                GOAL_VA_DUTIES => true,
                _ => false,
            };
            if inform {
                self.trigger_nearby_guests();
            }
        }
        unsafe { ZTSTAFF_DO_CONDITIONALS.hooked()(self.0 as *const u32) };
    }

    /// Number of habitats the habitat manager holds, `0` without a manager. Read live, as vanilla does.
    fn habitat_count() -> i32 {
        let manager: u32 = get_from_memory(base() + globals_rva::HABITAT_MGR);
        if manager == 0 {
            return 0;
        }
        let begin: u32 = get_from_memory(manager + HABITAT_ARRAY_BEGIN_OFFSET);
        let end: u32 = get_from_memory(manager + HABITAT_ARRAY_END_OFFSET);
        (end.wrapping_sub(begin) as i32) >> 2
    }

    /// Hands over to `ZTUnit::updateGoals`, which every `updateGoals` path ends with. Nothing detours that address.
    fn finish_update_goals(self) {
        unsafe { ZTUNIT_UPDATE_GOALS.hooked()(self.0 as *const u32) };
    }

    /// `ZTGuide::updateGoals` (`0x0058ce7e`):
    /// 1. With no predator check pending, or on the predator goal, a valid cog goal just runs `ZTUnit::updateGoals`.
    ///    An invalid one clears `+0x2a0`, goes to the no-habitat goal (`1`, clearing the `+0x20e` byte) when the zoo has
    ///    no habitats or to guide duties (`0x76`) when it has some, retargets tile 0 and runs `ZTUnit::updateGoals`.
    /// 2. Otherwise a predator found through `findPredator` releases every guest, clears `+0x2a0`, switches to the
    ///    predator goal (`0x75`) on that predator and records it at `+0x22c`. The pending flag clears and
    ///    `ZTUnit::updateGoals` runs.
    ///
    /// Vanilla builds and drops a debug string on the predator path; it has no effect and is not reproduced.
    pub fn update_goals(self) {
        let cog_goal = self.0 + unit_offsets::COG_GOAL;
        let pending = get_from_memory::<u8>(self.0 + unit_offsets::PREDATOR_CHECK_PENDING) != 0;

        if !pending || self.goal_id() == GOAL_PREDATOR {
            if call_slot_0(cog_goal, 0) & 0xff != 0 {
                return self.finish_update_goals();
            }
            save_to_memory(self.0 + VIEWING_AREA_INFORMED_OFFSET, 0u8);
            if Self::habitat_count() <= 0 {
                call_slot_2(self.0, SLOT_GOAL_SET, GOAL_NO_HABITAT, 1);
                save_to_memory(self.0 + unit_offsets::COG_INVALID, 0u8);
            } else {
                call_slot_2(self.0, SLOT_GOAL_SET, GOAL_GUIDE_DUTIES, 4);
            }
            call_slot_2(self.0, SLOT_SET_TARGET_TILE, 0, 1);
            return self.finish_update_goals();
        }

        let predator = call_slot_1(self.0, SLOT_FIND_PREDATOR, 0);
        if predator != 0 {
            self.release_guests(100);
            save_to_memory(self.0 + VIEWING_AREA_INFORMED_OFFSET, 0u8);
            call_slot_2(self.0, SLOT_GOAL_SET, GOAL_PREDATOR, 3);
            let cog_arg = (base() + COG_GOAL_ARG_RVA) as *const i32;
            unsafe { COG_GOAL_SET_ID.original()(cog_goal as *const u32, predator, cog_arg) };
            save_to_memory(self.0 + unit_offsets::PREDATOR, predator);
        }
        save_to_memory(self.0 + unit_offsets::PREDATOR_CHECK_PENDING, 0u8);
        self.finish_update_goals();
    }
}

/// The fence entity on `tile`'s edge `direction`, or `0` when the slot is empty or holds something that is not a
/// `ZTFence`.
fn edge_fence(tile: u32, direction: i32) -> u32 {
    let slot = tile.wrapping_add(tile_offsets::EDGE_SLOTS).wrapping_add(((direction / 2) as u32).wrapping_mul(4));
    let fence: u32 = get_from_memory(slot);
    if fence != 0 && unsafe { entity_type_matches(fence, globals_rva::FENCE_TAG) } {
        fence
    } else {
        0
    }
}

/// A fence a guide may cross: open, or a gate (its type's name starts with `'g'`).
fn fence_lets_through(fence: u32) -> bool {
    if get_from_memory::<u8>(fence + fence_offsets::PASSABLE) != 0 {
        return true;
    }
    let entity_type: u32 = get_from_memory(fence + unit_offsets::ENTITY_TYPE);
    let name: u32 = get_from_memory(entity_type + fence_offsets::TYPE_NAME_POINTER);
    name != 0 && get_from_memory::<u8>(name) == b'g'
}

#[detour_mod]
mod guide_detours {
    use super::*;

    #[detour(F_ROUTINE)]
    unsafe extern "thiscall" fn f_routine(this: *const u32, done: *const bool) -> bool {
        ZTGuide::f_routine(this as u32, done)
    }

    #[detour(F_VAROUTINE)]
    unsafe extern "thiscall" fn f_va_routine(this: *const u32, done: *const u8) -> bool {
        ZTGuide::f_va_routine(this as u32, done as *const bool)
    }

    #[detour(GET_TERRAIN_COST)]
    unsafe extern "thiscall" fn get_terrain_cost(this: *const u32, tile: *const u32) -> i32 {
        ZTGuide(this as u32).get_terrain_cost(tile as u32)
    }

    #[detour(FENCES_OK)]
    unsafe extern "thiscall" fn fences_ok(this: *const u32, tile: *const u32, direction: u32, out: *const bool, _unused: *const bool) -> bool {
        ZTGuide(this as u32).fences_ok(tile as u32, direction as i32, out as u32)
    }

    #[detour(IS_SPEAKING)]
    unsafe extern "thiscall" fn is_speaking(this: *const u32) -> bool {
        ZTGuide(this as u32).is_speaking()
    }

    #[detour(TILE_FILTER)]
    unsafe extern "thiscall" fn tile_filter(this: *const u32, tile: *const u32, flag: bool) -> bool {
        ZTGuide(this as u32).tile_filter(tile as u32, flag)
    }

    #[detour(DO_CONDITIONALS)]
    unsafe extern "thiscall" fn do_conditionals(this: *const u32) {
        ZTGuide(this as u32).do_conditionals();
    }

    #[detour(UPDATE_GOALS)]
    unsafe extern "thiscall" fn update_goals(this: *const u32) {
        ZTGuide(this as u32).update_goals();
    }

    #[detour(ADD_GUEST)]
    unsafe extern "thiscall" fn add_guest(this: *const u32, guest_id: i32) {
        #[cfg(feature = "reimplementation-tests")]
        {
            let hook = crate::ztguest::live_support::viewing_overrides::ADD_GUEST.lock().ok().and_then(|hook| *hook);
            if hook.is_some_and(|hook| hook(this as u32, guest_id)) {
                return;
            }
        }
        ZTGuide(this as u32).add_guest(guest_id)
    }

    #[detour(REMOVE_GUEST)]
    unsafe extern "thiscall" fn remove_guest(this: *const u32, guest_id: i32) {
        ZTGuide(this as u32).remove_guest(guest_id);
    }

    #[detour(RELEASE_GUESTS)]
    unsafe extern "thiscall" fn release_guests(this: *const u32, percent: i32) {
        ZTGuide(this as u32).release_guests(percent);
    }

    #[detour(CLEAR_GOAL)]
    unsafe extern "thiscall" fn clear_goal(this: *const u32) {
        ZTGuide(this as u32).clear_goal();
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, param: bool) {
        ZTGuide(this as u32).remove_from_map(param);
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const u32) -> bool {
        ZTGuide(this as u32).init_status_vars()
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        ZTGuide(this as u32).save(file)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        ZTGuide(this as u32).load(file, version)
    }

    #[detour(TRIGGER_NEARBY_GUESTS)]
    unsafe extern "thiscall" fn trigger_nearby_guests(this: *const u32) {
        ZTGuide(this as u32).trigger_nearby_guests();
    }

    #[detour(GET_HAB_FROM_VA)]
    unsafe extern "thiscall" fn get_hab_from_va(this: *const u32, tile: *const u32) -> i32 {
        ZTGuide(this as u32).get_hab_from_va(tile as u32) as i32
    }

    #[detour(GET_RANDOM_ASSIGNED_HABITAT)]
    unsafe extern "thiscall" fn get_random_assigned_habitat(this: *const u32) -> u32 {
        ZTGuide(this as u32).get_random_assigned_habitat()
    }

    #[detour(GET_NEEDIEST_VA)]
    unsafe extern "thiscall" fn get_neediest_va(this: *const u32, out: u32) -> i32 {
        ZTGuide(this as u32).get_neediest_va(out) as i32
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn f_routine(this: u32, done: *const bool) -> bool {
            unsafe { super::F_ROUTINE_DETOUR.call(this as *const u32, done) }
        }

        pub(crate) fn f_va_routine(this: u32, done: *const u8) -> bool {
            unsafe { super::F_VAROUTINE_DETOUR.call(this as *const u32, done) }
        }

        pub(crate) fn get_terrain_cost(this: u32, tile: u32) -> i32 {
            unsafe { super::GET_TERRAIN_COST_DETOUR.call(this as *const u32, tile as *const u32) }
        }

        pub(crate) fn fences_ok(this: u32, tile: u32, direction: u32, out: *const bool) -> bool {
            unsafe { super::FENCES_OK_DETOUR.call(this as *const u32, tile as *const u32, direction, out, std::ptr::null()) }
        }

        pub(crate) fn is_speaking(this: u32) -> bool {
            unsafe { super::IS_SPEAKING_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn tile_filter(this: u32, tile: u32, flag: bool) -> bool {
            unsafe { super::TILE_FILTER_DETOUR.call(this as *const u32, tile as *const u32, flag) }
        }

        pub(crate) fn do_conditionals(this: u32) {
            unsafe { super::DO_CONDITIONALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn update_goals(this: u32) {
            unsafe { super::UPDATE_GOALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn trigger_nearby_guests(this: u32) {
            unsafe { super::TRIGGER_NEARBY_GUESTS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_hab_from_va(this: u32, tile: u32) -> i32 {
            unsafe { super::GET_HAB_FROM_VA_DETOUR.call(this as *const u32, tile as *const u32) }
        }

        pub(crate) fn get_random_assigned_habitat(this: u32) -> u32 {
            unsafe { super::GET_RANDOM_ASSIGNED_HABITAT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_neediest_va(this: u32, out: u32) -> i32 {
            unsafe { super::GET_NEEDIEST_VA_DETOUR.call(this as *const u32, out) }
        }

        pub(crate) fn add_guest(this: u32, guest_id: i32) {
            unsafe { super::ADD_GUEST_DETOUR.call(this as *const u32, guest_id) }
        }

        pub(crate) fn remove_guest(this: u32, guest_id: i32) {
            unsafe { super::REMOVE_GUEST_DETOUR.call(this as *const u32, guest_id) }
        }

        pub(crate) fn release_guests(this: u32, percent: i32) {
            unsafe { super::RELEASE_GUESTS_DETOUR.call(this as *const u32, percent) }
        }

        pub(crate) fn clear_goal(this: u32) {
            unsafe { super::CLEAR_GOAL_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn remove_from_map(this: u32, param: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const u32, param) }
        }

        pub(crate) fn init_status_vars(this: u32) -> bool {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }

        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file, version) }
        }
    }
}

/// Installs `ZTGuide`'s detours. Called once at DLL load (via `lib.rs`) and before the `ZTGUIDE_*` tests in
/// `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { guide_detours::init_detours() } {
        error!("Failed to initialise ztguide detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::{standalone::OPERATOR_NEW, ztguide::{CONSTRUCTOR, DESTRUCTOR}};

    use super::{ZTGuide, GUIDE_SIZE, GUIDE_TYPE_TAG_RVA};
    use crate::{
        globals::{get_module_base, globals},
        util::get_from_memory,
    };

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::guide_detours::status()
    }

    /// The first `ZTGuideType` the live world holds, if any.
    pub fn find_guide_type() -> Option<u32> {
        let world = globals().ztworldmgr();
        let tag = get_module_base("zoo.exe") as u32 + GUIDE_TYPE_TAG_RVA;
        let mut slot = world.entity_type_array_start;
        while slot != world.entity_type_array_end {
            let ty: u32 = get_from_memory(slot);
            slot += 4;
            if ty == 0 {
                continue;
            }
            let vtable: u32 = get_from_memory(ty);
            let is_cast_class = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
            if is_cast_class(ty, tag) {
                return Some(ty);
            }
        }
        None
    }

    /// Guides built by [`build_standalone_guide`] and not yet destroyed.
    static LIVE_FIXTURES: std::sync::Mutex<Vec<u32>> = std::sync::Mutex::new(Vec::new());

    /// A zeroed guide built by the real vanilla constructor over `guide_type` (not detoured). It is torn down through
    /// the real scalar-deleting destructor by [`destroy_standalone_guide`], or by [`teardown_standalone_guides`] when the
    /// test finishes. A test that points a field of the guide at memory it owns must put the original value back first:
    /// the destructor frees whatever the fields hold.
    pub fn build_standalone_guide(guide_type: u32) -> ZTGuide {
        let memory = unsafe { OPERATOR_NEW.original()(GUIDE_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(memory as *mut u8, 0, GUIDE_SIZE as usize) };
        unsafe { CONSTRUCTOR.original()(memory as *const u32, guide_type as *const u32) };
        LIVE_FIXTURES.lock().unwrap().push(memory);
        ZTGuide(memory)
    }

    /// Tears a guide down through the real scalar-deleting destructor (not detoured).
    pub fn destroy_standalone_guide(guide: ZTGuide) {
        LIVE_FIXTURES.lock().unwrap().retain(|&fixture| fixture != guide.0);
        unsafe { DESTRUCTOR.original()(guide.0 as *const u32, 1) };
    }

    /// Destroys every guide still alive from [`build_standalone_guide`].
    pub fn teardown_standalone_guides() {
        let fixtures = std::mem::take(&mut *LIVE_FIXTURES.lock().unwrap());
        for fixture in fixtures {
            unsafe { DESTRUCTOR.original()(fixture as *const u32, 1) };
        }
    }

    pub(crate) use super::guide_detours::test_real as real;
}
