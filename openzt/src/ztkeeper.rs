//! `ZTKeeper`: the zookeeper entity, a thin subclass of `ZTStaff` (`ZTUnit` -> `BFUnit` -> `BFEntity`).
//!
//! Vanilla-layout-compatible (style 1): keepers are heap entities that un-ported `ZTStaff`/`ZTUnit`/`BFEntity`
//! code, `ZTHabitat`, the goal classes and `ZTAIMgr` all read by raw offset, so the object stays vanilla memory
//! and this module reads and writes it in place. A keeper is `0x2ac` bytes: the `ZTStaff` prefix (`0x000`-`0x287`)
//! followed by the fields below. Nothing here owns an allocation. Entities are created through
//! `ZTKeeperType::createEntity` and freed through the scalar-deleting destructor, both vanilla allocator paths.
//!
//! The own-field names come from the macOS decompiles' readers (`ZTGoalHabitatDuties::needFood`/`needHeal`/
//! `needClean`, `ZTGoalKeeperHabitat::decide`/`getNeediestHabitat`); macOS offsets are these minus `0x30`.

use std::{ffi::c_void, mem::transmute};

use openzt_detour::generated::{
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    bfcoggoal::{SET as COG_GOAL_SET, SET_ID as COG_GOAL_SET_ID},
    bfunit::{IDLE as BFUNIT_IDLE, IS_BLOCKED, IS_MAX_COST_FOR_ALL, IS_NON_PATH_STEEP, IS_SAME_TILE},
    poolalloc::DEALLOCATE as POOL_DEALLOCATE,
    standalone::{DEALLOCATE, OPERATOR_DELETE, WRITE_BYTES_TO_FILE},
    ztkeeper::{
        CHECK_ASSIGNED_HABITATS, CLEANS_UP, CONSTRUCTOR, DESTRUCTOR_0, DESTRUCTOR_1, DO_CONDITIONALS, DROP, F_ESCAPED_ANIMAL_ROUTINE,
        F_HABITAT_ROUTINE, F_ROUTINE, GET_EVENTS, GET_EVENTS_0X43F40D, GET_RANDOM_ASSIGNED_HABITAT, GET_TERRAIN_COST, IN_TARGET_HABITAT,
        INIT_AFTER_LOAD, INIT_STATUS_VARS, LISTEN, LOAD, NOTIFY_HABITAT_NOT_COMING, REMOVE_FROM_MAP, RESET_AI, SAVE, SET_SPEED, SET_TARGET_TILE,
        UPDATE_GOALS,
    },
    ztfence::CLOSE_GATE as ZTFENCE_CLOSE_GATE,
    ztshow::ABORT_SHOW as ZTSHOW_ABORT_SHOW,
    ztstaff::{
        CONSTRUCTOR as ZTSTAFF_CONSTRUCTOR, DESTRUCTOR_0 as ZTSTAFF_DESTRUCTOR_0, DO_CONDITIONALS as ZTSTAFF_DO_CONDITIONALS, DROP as ZTSTAFF_DROP,
        INIT_AFTER_LOAD as ZTSTAFF_INIT_AFTER_LOAD, INIT_STATUS_VARS as ZTSTAFF_INIT_STATUS_VARS, LOAD as ZTSTAFF_LOAD,
        REMOVE_ASSIGNED_HABITAT as ZTSTAFF_REMOVE_ASSIGNED_HABITAT, REMOVE_FROM_MAP as ZTSTAFF_REMOVE_FROM_MAP,
        SAVE as ZTSTAFF_SAVE,
        SET_TARGET_TILE as ZTSTAFF_SET_TARGET_TILE,
    },
    ztunit::{RESET_AI as ZTUNIT_RESET_AI, UPDATE_GOALS as ZTUNIT_UPDATE_GOALS},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    bfentity::set_bs,
    bfscriptmgr::RawString,
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    zthabitat::{next_game_random, ZTHabitat, MAX_PATH_COST_RVA},
    ztmegatilemgr::{entity_type_matches, RVA_SCENERY_TYPE_CHECK_ARG},
    vanilla_string::VanillaString,
    ztshow::{remove_unit as show_info_remove_unit, script::mgr::{get_item as show_script_get_item, get_script as show_script_get}},
    ztshowmgr::ZTShowMgr,
};

/// `ZTKeeperType::createEntity` allocates this many bytes.
pub const KEEPER_SIZE: u32 = 0x2ac;

/// Vtable address of `ZTKeeper` (`0x0062f3e4`) as an RVA from the `zoo.exe` base.
pub const KEEPER_VTABLE_RVA: u32 = 0x0022_f3e4;

/// Class tag (RVA) the entity type's `isCastClass` takes to test for a keeper type.
pub const KEEPER_TYPE_TAG_RVA: u32 = 0x0023_8760;

/// Food amount to leave: `ZTHabitat::getFoodToLeave`'s result, stored by `ZTGoalHabitatDuties::needFood`.
pub const FOOD_TO_LEAVE_OFFSET: u32 = 0x294;
/// Number of sick animals in the habitat being serviced, stored by `ZTGoalHabitatDuties::needHeal`.
pub const SICK_ANIMAL_COUNT_OFFSET: u32 = 0x298;
/// Number of dirt tiles in the habitat being serviced, stored by `ZTGoalHabitatDuties::needClean`.
pub const DIRT_TILE_COUNT_OFFSET: u32 = 0x29c;
/// Habitat auto-selection gate. `getRandomAssignedHabitat` and `ZTGoalKeeperHabitat::getNeediestHabitat` only
/// choose a habitat while it is `0`. Nothing but `initStatusVars` and `load` writes it.
pub const HABITAT_SELECT_DISABLED_OFFSET: u32 = 0x2a0;
/// Food type to leave, stored by `ZTGoalHabitatDuties::needFood`.
pub const FOOD_TYPE_TO_LEAVE_OFFSET: u32 = 0x2a4;
/// Set to `1` around the `BFUnit::getPath` retry in `ZTGoalKeeperHabitat::decide` and `0` afterwards. `getTerrainCost`
/// reads it to lower the cost of walking toward a habitat gate.
pub const PATHING_TO_HABITAT_GATE_OFFSET: u32 = 0x2a8;

/// The five own dwords, in address order. `initStatusVars` zeroes all of them.
pub const OWN_DWORD_OFFSETS: [u32; 5] =
    [FOOD_TO_LEAVE_OFFSET, SICK_ANIMAL_COUNT_OFFSET, DIRT_TILE_COUNT_OFFSET, HABITAT_SELECT_DISABLED_OFFSET, FOOD_TYPE_TO_LEAVE_OFFSET];

/// Offsets inherited from `ZTUnit`/`BFEntity` that keeper methods read or write. Shared with the other staff
/// classes, so they live here only until a `ZTStaff` module exists.
pub mod unit_offsets {
    /// `BFEntity` id, the key `ZTShowInfo::removeUnit` and `getEvents` use.
    pub const ENTITY_ID: u32 = 0x124;
    /// The entity's `ZTKeeperType` (or other entity type) pointer.
    pub const ENTITY_TYPE: u32 = 0x128;
    /// Start of the embedded `BFCogGoal`.
    pub const COG_GOAL: u32 = 0x15c;
    /// `BFCogGoal` validity flag cleared by `updateGoals`' no-habitat tail.
    pub const COG_INVALID: u32 = 0x20e;
    /// The embedded `BFCogGoal`'s state word (`COG_GOAL + 0x18`); `0x13` marks a unit tied to a show.
    pub const COG_GOAL_STATE: u32 = 0x174;
    /// Movement speed written by `setSpeed`.
    pub const SPEED: u32 = 0x1f8;
    /// The tile the unit is heading for.
    pub const TARGET_TILE: u32 = 0x234;
    /// The unit's goal id: `0x6e` while on habitat duties, `0x6f` while retrieving an escaped animal.
    pub const GOAL_ID: u32 = 0x170;
    /// A byte of the embedded `BFCogGoal` (`COG_GOAL + 0x4c`); the routines only run their goal while it is clear.
    pub const COG_GOAL_FLAG: u32 = 0x1a8;
    /// The byte every `fRoutine` copies into its `done` flag: set while the unit is not moving.
    pub const STOPPED: u32 = 0xf9;
    /// `ZTStaff`'s assigned-habitat list: a pointer to the sentinel node of a doubly linked list whose nodes hold
    /// the next link at `+0x0` and the habitat pointer at `+0x8`.
    pub const ASSIGNED_HABITATS: u32 = 0x260;
    /// A byte `updateGoals` consumes: set when something wants a predator check on the next goal update.
    pub const PREDATOR_CHECK_PENDING: u32 = 0x24c;
    /// The predator `updateGoals` records when it switches to the predator goal.
    pub const PREDATOR: u32 = 0x22c;
    /// The animal id the escaped-animal goal is set against (a `ZTStaff` dword).
    pub const ESCAPED_ANIMAL: u32 = 0x290;
    /// The fence gate the unit is queued at; `notifyHabitatNotComing` closes it.
    pub const GATE: u32 = 0x248;
    /// The id of the `ZTShowInfo` the unit performs in (`u16`).
    pub const SHOW_INFO_ID: u32 = 0x254;
    /// A byte `removeFromMap` tests before cancelling the unit's habitat visit.
    pub const MBR_0X142: u32 = 0x142;
    /// `initStatusVars` stores the unit's own address here.
    pub const SELF_POINTER: u32 = 0x64;
    /// `initStatusVars` stores `entity_type + 0x80` here when the type's `+0x84` is non-zero, otherwise null.
    pub const TYPE_BLOCK_POINTER: u32 = 0x68;
}

/// `ZTHabitat` fields the keeper methods read; the habitat code owns their meaning.
mod habitat_offsets {
    /// `unknown_flag_0x2c`: set on a "world" habitat that keepers do not service.
    pub const IS_WORLD: u32 = 0x2c;
    /// `characteristics_dirty`.
    pub const CHARACTERISTICS_DIRTY: u32 = 0x2d;
    /// `num_keepers`.
    pub const NUM_KEEPERS: u32 = 0xf0;
    /// `scheduled_service_counter`: keepers announced as coming. `notifyHabitatNotComing` decrements it.
    pub const SCHEDULED_SERVICE_COUNTER: u32 = 0xf4;
}

/// `BFTile` fields `getTerrainCost` reads.
mod tile_offsets {
    /// The entity standing on the tile.
    pub const ENTITY: u32 = 0x4;
    /// Byte tested against `0x8`.
    pub const FLAGS_83: u32 = 0x83;
    /// Byte tested against `0x20`.
    pub const FLAGS_85: u32 = 0x85;
}

/// `ZTSceneryType`'s per-item amount; a positive value makes the tile cost like a steep one.
const SCENERY_AMOUNT_OFFSET: u32 = 0x154;
/// `BFEntityType` fields `initStatusVars` links: the block at `+0x80`, present when `+0x84` is non-zero.
const TYPE_BLOCK_OFFSET: u32 = 0x80;
const TYPE_BLOCK_GATE_OFFSET: u32 = 0x84;

/// `ZTKeeperType` fields.
mod keeper_type_offsets {
    /// Start and end of the vector of entity-type ids this keeper type cleans up after.
    pub const CLEANS_UP_BEGIN: u32 = 0x20c;
    pub const CLEANS_UP_END: u32 = 0x210;
}

/// Vtable slots (byte offsets) keeper methods dispatch through, so a subclass override is honoured.
mod slots {
    /// `ZTKeeper::getEvents` (the three-argument one), on the keeper and on `ZTAIMgr`'s event listener.
    pub const GET_EVENTS: u32 = 0xc;
    /// `ZTKeeper::getEvents` (the one-argument form that fills a vector of this keeper's events).
    pub const GET_EVENTS_LIST: u32 = 0x8;
    /// `ZTUnit::stopEverything`.
    pub const STOP_EVERYTHING: u32 = 0x198;
    /// `ZTUnit::findPredator`.
    pub const FIND_PREDATOR: u32 = 0x2fc;
    /// `ZTStaff::foundEscapedAnimal`.
    pub const FOUND_ESCAPED_ANIMAL: u32 = 0x32c;
    /// `ZTKeeper::setTargetTile`.
    pub const SET_TARGET_TILE: u32 = 0x2d8;
    /// `ZTHabitat::isTank` (habitat vtable).
    pub const HABITAT_IS_TANK: u32 = 0x20;
    /// `ZTStaff::goalSet`.
    pub const GOAL_SET: u32 = 0x1d8;
    /// `ZTStaff::getHabitat`.
    pub const GET_HABITAT: u32 = 0x24c;
    /// The entity type's id getter, called by `setSpeed` and `cleansUp` (entity-type vtable).
    pub const TYPE_GET_ID: u32 = 0x20;
    /// `ZTStaff::getHabitatFromTile`.
    pub const GET_HABITAT_FROM_TILE: u32 = 0x248;
}

/// `BFEvent` record size in the vector `getEvents` fills.
const EVENT_SIZE: u32 = 0x1c;
/// Fields of a `BFEvent` that `listen` reads: the show id it is addressed to (`u32`), the script item index (`u16`)
/// and the event kind (`u16`).
mod event_offsets {
    pub const SHOW_ID: u32 = 0x8;
    pub const ITEM_INDEX: u32 = 0x14;
    pub const KIND: u32 = 0x16;
}
/// Show-script event kinds `listen` reacts to.
const EVENT_SHOW_ITEM: u16 = 0x2715;
const EVENT_SHOW_SOUND_A: u16 = 0x271d;
const EVENT_SHOW_SOUND_B: u16 = 0x271f;
/// `ZTShowInfo`'s script id (`u16`).
const SHOW_INFO_SCRIPT_ID_OFFSET: u32 = 0x8;
/// `ZTShowScriptItem` strings `listen` plays: the keeper's pre-trick and post-trick behavior-set names, each a
/// `{begin, end}` pair at the offset.
const ITEM_SOUND_A_OFFSET: u32 = 0x28;
const ITEM_SOUND_B_OFFSET: u32 = 0x34;
/// `setBS`'s start mode: leave the mixer untouched.
const SET_BS_CLEAR_MODE: i32 = 2;

/// Save versions that change what `load` reads.
mod save_version {
    /// From here on the habitat-select dword is saved.
    pub const SELECT_GATE: u32 = 0x10;
    /// Version `0x10` alone saved a 25-byte block that `load` reads and discards.
    pub const LEGACY_BLOCK: u32 = 0x10;
    /// Above this version the other four dwords and the `+0x2a8` byte are saved.
    pub const OWN_FIELDS_AFTER: u32 = 0x22;
    /// Versions in this range also carry a dword that lands at `+0x290`.
    pub const EXTRA_DWORD: std::ops::RangeInclusive<u32> = 0x34..=0x4a;
}

/// Bytes version `0x10` saved between the habitat-select dword and the `ZTStaff` data: six bytes, four dwords, two
/// bytes and one more byte. `load` reads and discards them.
const LEGACY_BLOCK_READS: [u32; 13] = [1, 1, 1, 1, 1, 1, 4, 4, 4, 4, 1, 1, 1];
/// Where the extra dword of versions `0x34..=0x4a` lands.
const EXTRA_DWORD_OFFSET: u32 = 0x290;

/// The goal id `resetAI` hands `goalSet`: the keeper's habitat-duties goal.
const GOAL_HABITAT_DUTIES: u32 = 0x6e;
/// Goal ids `updateGoals` switches between.
const GOAL_PREDATOR: u32 = 0x75;
const GOAL_TANK: u32 = 0x7f;
const GOAL_NO_HABITAT: u32 = 0x01;
const GOAL_NO_HABITAT_WITH_HABITATS: u32 = 0x65;
/// The global `BFCogGoal::set`/`setID` take as their trailing string argument (`DAT_00639360`), as an RVA.
const COG_GOAL_ARG_RVA: u32 = 0x0023_9360;
/// The `ZTHabitatMgr` global (`GLOBAL_ZTHabitatMgr`) as an RVA, and the start and end of its habitat array.
const HABITAT_MGR_GLOBAL_RVA: u32 = 0x0023_805c;
const HABITAT_ARRAY_START_OFFSET: u32 = 0x1c;
const HABITAT_ARRAY_END_OFFSET: u32 = 0x20;
/// The goal id of retrieving an escaped animal.
const GOAL_ESCAPED_ANIMAL: u32 = 0x6f;
/// `ZTCheat::renameEntity` sets this byte; while it is set, one keeper type runs at double speed.
const SPEED_CHEAT_FLAG_RVA: u32 = 0x0023_c2e4;
/// The entity-type id whose keepers double their speed under the cheat.
const SPEED_CHEAT_TYPE_ID: u32 = 0x254f;
/// Cog-goal state of a unit tied to a show.
const GOAL_STATE_SHOW: u32 = 0x13;
/// Event kind the keeper's `getEvents` asks for.
const EVENT_KIND_KEEPER: u32 = 0x4b;
/// Offset of the `BFEventListenerInterface` inside `ZTAIMgr`.
const AIMGR_LISTENER_OFFSET: u32 = 0x8;

/// Writes `size` bytes at `address` through vanilla's file-write primitive. `.hooked()` so a test build's I/O
/// redirect sees the write.
fn write_field(address: u32, size: u32, file: *const u32) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(address as *const u32, size, 1, file as *const i8) == 1 }
}

/// Reads `size` bytes into `address` through vanilla's file-read primitive (the misnamed `deallocate`).
fn read_field(address: u32, size: u32, file: *const u32) -> bool {
    unsafe { DEALLOCATE.hooked()(address as *const u32, size, 1, file as *const u8) == 1 }
}

/// The bytes of the `{begin, end}` string at `item + offset`; empty when `item` is null.
fn item_string(item: u32, offset: u32) -> Vec<u8> {
    if item == 0 {
        return Vec::new();
    }
    let begin: u32 = get_from_memory(item + offset);
    let end: u32 = get_from_memory(item + offset + 4);
    if begin == 0 || end < begin {
        return Vec::new();
    }
    (begin..end).map(get_from_memory::<u8>).collect()
}

fn vtable_slot(this: u32, offset: u32) -> u32 {
    get_from_memory(get_from_memory::<u32>(this) + offset)
}

fn call_slot_0(this: u32, offset: u32) -> u32 {
    let function = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(vtable_slot(this, offset)) };
    function(this)
}

fn call_slot_1(this: u32, offset: u32, arg: u32) -> u32 {
    let function = unsafe { transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(vtable_slot(this, offset)) };
    function(this, arg)
}

fn call_slot_2(this: u32, offset: u32, arg1: u32, arg2: u32) {
    let function = unsafe { transmute::<u32, extern "thiscall" fn(u32, u32, u32)>(vtable_slot(this, offset)) };
    function(this, arg1, arg2);
}

fn call_slot_3(this: u32, offset: u32, arg1: u32, arg2: u32, arg3: u32) {
    let function = unsafe { transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32)>(vtable_slot(this, offset)) };
    function(this, arg1, arg2, arg3);
}

/// A keeper in vanilla memory. All reads and writes are volatile through the raw address, so a keeper that vanilla
/// is rewriting under a live reference stays sound.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ZTKeeper(pub u32);

#[allow(dead_code)]
impl ZTKeeper {
    pub fn food_to_leave(self) -> i32 {
        get_from_memory(self.0 + FOOD_TO_LEAVE_OFFSET)
    }

    pub fn set_food_to_leave(self, value: i32) {
        save_to_memory(self.0 + FOOD_TO_LEAVE_OFFSET, value);
    }

    pub fn sick_animal_count(self) -> i32 {
        get_from_memory(self.0 + SICK_ANIMAL_COUNT_OFFSET)
    }

    pub fn set_sick_animal_count(self, value: i32) {
        save_to_memory(self.0 + SICK_ANIMAL_COUNT_OFFSET, value);
    }

    pub fn dirt_tile_count(self) -> i32 {
        get_from_memory(self.0 + DIRT_TILE_COUNT_OFFSET)
    }

    pub fn set_dirt_tile_count(self, value: i32) {
        save_to_memory(self.0 + DIRT_TILE_COUNT_OFFSET, value);
    }

    pub fn habitat_select_disabled(self) -> u32 {
        get_from_memory(self.0 + HABITAT_SELECT_DISABLED_OFFSET)
    }

    pub fn set_habitat_select_disabled(self, value: u32) {
        save_to_memory(self.0 + HABITAT_SELECT_DISABLED_OFFSET, value);
    }

    pub fn food_type_to_leave(self) -> i32 {
        get_from_memory(self.0 + FOOD_TYPE_TO_LEAVE_OFFSET)
    }

    pub fn set_food_type_to_leave(self, value: i32) {
        save_to_memory(self.0 + FOOD_TYPE_TO_LEAVE_OFFSET, value);
    }

    pub fn pathing_to_habitat_gate(self) -> u8 {
        get_from_memory(self.0 + PATHING_TO_HABITAT_GATE_OFFSET)
    }

    pub fn set_pathing_to_habitat_gate(self, value: u8) {
        save_to_memory(self.0 + PATHING_TO_HABITAT_GATE_OFFSET, value);
    }

    /// `ZTKeeper::setSpeed` (`0x0041678b`): stores `speed`, doubled for the cheat-boosted keeper type.
    pub fn set_speed(self, speed: u32) {
        let cheat: u8 = get_from_memory(get_module_base("zoo.exe") as u32 + SPEED_CHEAT_FLAG_RVA);
        let mut value = speed;
        if cheat != 0 {
            let entity_type: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_TYPE);
            let type_id = call_slot_0(entity_type, slots::TYPE_GET_ID);
            if type_id == SPEED_CHEAT_TYPE_ID {
                value = speed.wrapping_mul(2);
            }
        }
        save_to_memory(self.0 + unit_offsets::SPEED, value);
    }

    /// `ZTKeeper::setTargetTile` (`0x004167db`): a changed target tile first cancels the habitat visit announced
    /// for the old one, then the `ZTStaff` behaviour runs.
    pub fn set_target_tile(self, tile: u32, flag: bool) {
        let current: u32 = get_from_memory(self.0 + unit_offsets::TARGET_TILE);
        if tile != current {
            self.notify_habitat_not_coming(0);
        }
        unsafe { ZTSTAFF_SET_TARGET_TILE.original()(self.0 as *const u32, tile as *const u32, flag) };
    }

    /// `ZTKeeper::resetAI` (`0x005ae772`): the `ZTUnit` reset, then back to habitat duties.
    pub fn reset_ai(self) {
        unsafe { ZTUNIT_RESET_AI.original()(self.0 as *const u32) };
        call_slot_2(self.0, slots::GOAL_SET, GOAL_HABITAT_DUTIES, 0);
    }

    /// `ZTKeeper::initAfterLoad` (`0x00595386`): a show keeper re-registers with its habitat's show, then the
    /// `ZTStaff` behaviour runs. `param` reaches `ZTStaff` as the full dword vanilla pushes. Vanilla tail-returns
    /// `ZTStaff::initAfterLoad`'s result in `AL` and the load path checks it, so this returns it too.
    pub fn init_after_load(self, param: u32) -> bool {
        let habitat = call_slot_0(self.0, slots::GET_HABITAT);
        if habitat != 0 && get_from_memory::<u32>(self.0 + unit_offsets::COG_GOAL_STATE) == GOAL_STATE_SHOW {
            unsafe { ref_from_memory::<ZTHabitat>(habitat) }.add_show_unit(self.0);
        }
        let staff_init_after_load = unsafe { transmute::<u32, unsafe extern "thiscall" fn(*const u32, u32) -> u32>(ZTSTAFF_INIT_AFTER_LOAD.address) };
        low_byte_bool(unsafe { staff_init_after_load(self.0 as *const u32, param) })
    }

    /// `ZTKeeper::getEvents` (vtable `+0x8`, `0x00423cee`): this keeper's events, through the three-argument
    /// `getEvents` slot.
    pub fn get_events(self, out: u32) {
        let id: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_ID);
        call_slot_3(self.0, slots::GET_EVENTS, out, id, EVENT_KIND_KEEPER);
    }

    /// `ZTKeeper::getEvents` (vtable `+0xc`, `0x0043f40d`): forwards to `ZTAIMgr`'s event listener when it exists.
    pub fn get_events_from_aimgr(arg1: u32, arg2: u32, arg3: u32) {
        let aimgr = globals().ztaimgr_ptr() as u32;
        if aimgr == 0 {
            return;
        }
        let listener = aimgr + AIMGR_LISTENER_OFFSET;
        call_slot_3(listener, slots::GET_EVENTS, arg1, arg2, arg3);
    }

    /// `ZTKeeper::inTargetHabitat` (`0x0049f6b7`): whether the habitat of the target tile is the habitat of the tile
    /// the keeper stands on.
    pub fn in_target_habitat(self) -> bool {
        let target_tile: u32 = get_from_memory(self.0 + unit_offsets::TARGET_TILE);
        let target_habitat = call_slot_1(self.0, slots::GET_HABITAT_FROM_TILE, target_tile);
        let current_tile = unsafe { BFENTITY_GET_TILE.original()(self.0 as *const u32) } as u32;
        target_habitat == call_slot_1(self.0, slots::GET_HABITAT_FROM_TILE, current_tile)
    }

    /// The habitats this keeper is assigned to, in list order.
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

    /// `ZTKeeper::checkAssignedHabitats` (`0x00423d95`): drops every assigned habitat that now blocks this keeper's
    /// service (`blockService == 1`), restarting the walk from the head after each removal.
    pub fn check_assigned_habitats(self) {
        'restart: loop {
            let sentinel: u32 = get_from_memory(self.0 + unit_offsets::ASSIGNED_HABITATS);
            let mut node: u32 = get_from_memory(sentinel);
            if node == sentinel {
                return;
            }
            loop {
                let habitat: u32 = get_from_memory(node + 8);
                if unsafe { ref_from_memory::<ZTHabitat>(habitat) }.block_service(self.0, false, true) == 1 {
                    unsafe { ZTSTAFF_REMOVE_ASSIGNED_HABITAT.original()(self.0 as *const u32, habitat as *const u32) };
                    continue 'restart;
                }
                node = get_from_memory(node);
                if node == get_from_memory::<u32>(self.0 + unit_offsets::ASSIGNED_HABITATS) {
                    return;
                }
            }
        }
    }

    /// `ZTKeeper::doConditionals` (`0x00423d84`): re-validates the assigned habitats, then the `ZTStaff` conditionals.
    pub fn do_conditionals(self) {
        self.check_assigned_habitats();
        unsafe { ZTSTAFF_DO_CONDITIONALS.original()(self.0 as *const u32) };
    }

    /// `ZTKeeper::cleansUp` (`0x0042404e`): whether `entity`'s type is one this keeper's type cleans up after.
    /// False for a null entity, a null type on either side, or a keeper that is not a keeper type.
    pub fn cleans_up(self, entity: u32) -> bool {
        if entity == 0 {
            return false;
        }
        let keeper_type = if unsafe { entity_type_matches(self.0, KEEPER_TYPE_TAG_RVA) } { get_from_memory::<u32>(self.0 + unit_offsets::ENTITY_TYPE) } else { 0 };
        let entity_type: u32 = get_from_memory(entity + unit_offsets::ENTITY_TYPE);
        if entity_type == 0 || keeper_type == 0 {
            return false;
        }
        let type_id = call_slot_0(entity_type, slots::TYPE_GET_ID);
        let begin: u32 = get_from_memory(keeper_type + keeper_type_offsets::CLEANS_UP_BEGIN);
        let end: u32 = get_from_memory(keeper_type + keeper_type_offsets::CLEANS_UP_END);
        (begin..end).step_by(4).any(|address| get_from_memory::<u32>(address) == type_id)
    }

    /// `ZTKeeper::drop` (`0x004e76bb`): a keeper dropped in a servicing habitat goes back to habitat duties unless
    /// another keeper already serves it, then the `ZTStaff` behaviour runs.
    pub fn drop_keeper(self) {
        let habitat = call_slot_0(self.0, slots::GET_HABITAT);
        if habitat != 0 && get_from_memory::<u8>(habitat + habitat_offsets::IS_WORLD) == 0 {
            let serviced = unsafe { ref_from_memory::<ZTHabitat>(habitat) }.is_being_serviced();
            let another_keeper = serviced && {
                if get_from_memory::<u8>(habitat + habitat_offsets::CHARACTERISTICS_DIRTY) != 0 {
                    unsafe { ref_from_memory::<ZTHabitat>(habitat) }.recalculate_characteristics();
                }
                get_from_memory::<i32>(habitat + habitat_offsets::NUM_KEEPERS) >= 2
            };
            if !another_keeper {
                call_slot_2(self.0, slots::GOAL_SET, GOAL_HABITAT_DUTIES, 0);
            }
        }
        unsafe { ZTSTAFF_DROP.original()(self.0 as *const u32) };
    }

    /// `ZTKeeper::getRandomAssignedHabitat` (`0x0049db62`): a random habitat from the assigned list, or - with no
    /// assignments - from every habitat that does not block this keeper. Null when the habitat manager is absent,
    /// the keeper's `+0x2a0` gate is set, or nothing qualifies. The game RNG advances only when there is a choice.
    pub fn get_random_assigned_habitat(self) -> u32 {
        if globals().zthabitatmgr_ptr().is_null() || self.habitat_select_disabled() != 0 {
            return 0;
        }
        let mut candidates = self.assigned_habitats();
        if candidates.is_empty() {
            let exhibits = globals().zthabitatmgr().exhibit_array();
            let all: Vec<u32> = (0..exhibits.len()).map(|index| exhibits.get_ptr(index)).collect();
            candidates = all.into_iter().filter(|&habitat| unsafe { ref_from_memory::<ZTHabitat>(habitat) }.block_service(self.0, false, true) == 0).collect();
        }
        if candidates.is_empty() {
            return 0;
        }
        candidates[next_game_random() as usize % candidates.len()]
    }

    /// `ZTKeeper::fRoutine` (`0x004a86ba`): the keeper's per-tick behaviour, run by `ZTAIMgr`'s `fKeeperRoutine`. A
    /// non-keeper type returns `false`. On habitat duties in a real habitat, or retrieving an escaped animal in a
    /// world habitat, it runs that routine; every other case reports the stopped flag as `done` and idles. Returns
    /// `true` for every keeper.
    pub fn f_routine(self, done: *const bool) -> bool {
        if !unsafe { entity_type_matches(self.0, KEEPER_TYPE_TAG_RVA) } {
            return false;
        }
        let goal: u32 = get_from_memory(self.0 + unit_offsets::GOAL_ID);
        let habitat = call_slot_0(self.0, slots::GET_HABITAT);
        let goal_flag_clear = || get_from_memory::<u8>(self.0 + unit_offsets::COG_GOAL_FLAG) == 0;
        if habitat != 0 {
            if get_from_memory::<u8>(habitat + habitat_offsets::IS_WORLD) == 0 {
                if goal == GOAL_HABITAT_DUTIES && goal_flag_clear() {
                    self.f_habitat_routine(done);
                    return true;
                }
            } else if goal == GOAL_ESCAPED_ANIMAL && goal_flag_clear() {
                self.f_escaped_animal_routine(done);
                return true;
            }
        }
        self.report_stopped(done);
        unsafe { BFUNIT_IDLE.original()(self.0 as *const u32) };
        true
    }

    fn report_stopped(self, done: *const bool) {
        save_to_memory(done as u32, get_from_memory::<u8>(self.0 + unit_offsets::STOPPED));
    }

    /// `ZTKeeper::fHabitatRoutine` (`0x0049ee0f`): reports the stopped flag as `done`. Vanilla also builds and frees
    /// a throwaway `"Doing Habitat Duties"` string when on habitat duties, which leaves the allocator as it was; the
    /// port skips it, and the habitat lookup that only gates it.
    pub fn f_habitat_routine(self, done: *const bool) {
        self.report_stopped(done);
    }

    /// `ZTKeeper::fEscapedAnimalRoutine` (`0x0050de9f`): reports the stopped flag as `done`. Vanilla also builds and
    /// frees a throwaway `"Retrieving Escaped Animal"` string, which the port skips.
    pub fn f_escaped_animal_routine(self, done: *const bool) {
        self.report_stopped(done);
    }

    /// `ZTKeeper::notifyHabitatNotComing` (`0x004a879f`): the keeper will not reach its target tile's habitat after
    /// all, so the habitat's announced-keeper counter drops (clamped at 0). When the low byte of `keep_gate` is
    /// clear it also closes the gate the keeper was queued at and forgets it. Vanilla builds a `"Notifying habitat
    /// ... : not coming"` string into locals that nothing reads and frees it again, which leaves the allocator as it
    /// was; the port skips it.
    pub fn notify_habitat_not_coming(self, keep_gate: u32) {
        let target_tile: u32 = get_from_memory(self.0 + unit_offsets::TARGET_TILE);
        let habitat = call_slot_1(self.0, slots::GET_HABITAT_FROM_TILE, target_tile);
        if habitat != 0 && get_from_memory::<u8>(habitat + habitat_offsets::IS_WORLD) == 0 {
            let counter: i32 = get_from_memory(habitat + habitat_offsets::SCHEDULED_SERVICE_COUNTER);
            if counter != 0 {
                save_to_memory(habitat + habitat_offsets::SCHEDULED_SERVICE_COUNTER, counter.wrapping_sub(1).max(0));
            }
        }
        if keep_gate & 0xff == 0 {
            let gate: u32 = get_from_memory(self.0 + unit_offsets::GATE);
            if gate != 0 {
                unsafe { ZTFENCE_CLOSE_GATE.original()(gate as *const c_void) };
                save_to_memory(self.0 + unit_offsets::GATE, 0u32);
            }
        }
    }

    /// `ZTKeeper::removeFromMap` (`0x00458759`): a keeper that is not just being moved leaves its show, then a
    /// keeper not yet marked at `+0x142` cancels its habitat visit, then the `ZTStaff` removal runs.
    pub fn remove_from_map(self, param: bool) {
        let show_info = ZTShowMgr::get_show_info(get_from_memory::<u16>(self.0 + unit_offsets::SHOW_INFO_ID));
        if show_info != 0 && !param {
            let id: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_ID);
            let entity_type: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_TYPE);
            let type_id = call_slot_0(entity_type, slots::TYPE_GET_ID);
            show_info_remove_unit(show_info, type_id, id);
            unsafe { ZTSHOW_ABORT_SHOW.original()((show_info + 0x4) as *const u32) };
        }
        if get_from_memory::<u8>(self.0 + unit_offsets::MBR_0X142) == 0 {
            self.notify_habitat_not_coming(param as u32);
        }
        unsafe { ZTSTAFF_REMOVE_FROM_MAP.original()(self.0 as *const u32, param) };
    }

    /// `ZTKeeper::initStatusVars` (`0x00457b70`): the `ZTStaff` reset, the five own dwords cleared, then the unit
    /// links itself at `+0x64` and to its type's `+0x80` block at `+0x68` (null when the type's `+0x84` is clear).
    /// Returns `1`, vanilla's `AL`.
    pub fn init_status_vars(self) -> u32 {
        unsafe { ZTSTAFF_INIT_STATUS_VARS.original()(self.0 as *const u32) };
        for offset in OWN_DWORD_OFFSETS {
            save_to_memory(self.0 + offset, 0u32);
        }
        save_to_memory(self.0 + unit_offsets::SELF_POINTER, self.0);
        let entity_type: u32 = get_from_memory(self.0 + unit_offsets::ENTITY_TYPE);
        let block = if get_from_memory::<u32>(entity_type + TYPE_BLOCK_GATE_OFFSET) == 0 { 0 } else { entity_type + TYPE_BLOCK_OFFSET };
        save_to_memory(self.0 + unit_offsets::TYPE_BLOCK_POINTER, block);
        1
    }

    /// `ZTKeeper::getTerrainCost` (`0x00423b28`): the pathfinder's cost of stepping onto `tile`. The unit's own tile
    /// costs `1`; a tile that is maximally costly for everyone, or blocked (unless a scenery item with a positive
    /// amount stands there), costs the path-cost sentinel. A steep or scenery tile costs the sentinel minus 4, or `1`
    /// while the keeper is pathing to a habitat gate. Ordinary open ground costs `2`, or `1` while pathing to a gate.
    pub fn get_terrain_cost(self, tile: u32) -> i32 {
        let max_cost = || get_from_memory::<i32>(get_module_base("zoo.exe") as u32 + MAX_PATH_COST_RVA);
        let this = self.0 as *const u32;
        let tile_ptr = tile as *const u32;
        if unsafe { IS_SAME_TILE.original()(this, tile_ptr) } != 0 {
            return 1;
        }
        if low_byte_bool(unsafe { IS_MAX_COST_FOR_ALL.original()(this, tile_ptr) }) {
            return max_cost();
        }
        let occupant: u32 = get_from_memory(tile + tile_offsets::ENTITY);
        let scenery_blocks = occupant != 0
            && unsafe { entity_type_matches(occupant, RVA_SCENERY_TYPE_CHECK_ARG) }
            && get_from_memory::<i32>(occupant + SCENERY_AMOUNT_OFFSET) > 0;
        if get_from_memory::<u8>(tile + tile_offsets::FLAGS_85) & 0x20 == 0
            && low_byte_bool(unsafe { IS_BLOCKED.original()(this, tile as *const c_void) })
            && !scenery_blocks
        {
            return max_cost();
        }
        let gate_pathing = self.pathing_to_habitat_gate() != 0;
        if low_byte_bool(unsafe { IS_NON_PATH_STEEP.original()(tile as i32) }) || scenery_blocks {
            return if gate_pathing { 1 } else { max_cost() - 4 };
        }
        if get_from_memory::<u8>(tile + tile_offsets::FLAGS_83) & 0x8 == 0 && get_from_memory::<u8>(tile + tile_offsets::FLAGS_85) & 0x20 == 0 {
            return if gate_pathing { 1 } else { 2 };
        }
        1
    }

    /// Hands over to `ZTUnit::updateGoals`, which every `updateGoals` path ends with. `.hooked()` runs what vanilla's
    /// direct call would; nothing detours that address, so it is the real body, and a test can observe the hand-over.
    fn finish_update_goals(self) {
        unsafe { ZTUNIT_UPDATE_GOALS.hooked()(self.0 as *const u32) };
    }

    /// `ZTKeeper::updateGoals` (`0x00423d04`), laid out from the `.asm` and its outlined blocks:
    /// 1. A pending predator check (`+0x24c`) on a goal other than the predator goal looks for a predator; one found
    ///    switches to goal `0x75` and is recorded at `+0x22c`; the check flag clears and the update ends.
    /// 2. Otherwise, off the escaped-animal goal, a found escaped animal stops everything and sets goal `0x6f` on
    ///    the animal at `+0x290`.
    /// 3. An invalid cog goal, or a set goal flag in a world habitat, goes to goal `0x7f` for a tank habitat; for any
    ///    other it goes to the no-habitat goal (`1`, clearing the cog goal's validity byte) when the zoo has no
    ///    habitats or goal `0x65` when it has some, and either way retargets tile 0.
    /// 4. A valid goal with a clear flag ends. Otherwise a keeper standing in the habitat of its target tile goes
    ///    back to habitat duties (`0x6e`) unless another keeper already services that habitat.
    ///
    /// Every path then runs `ZTUnit::updateGoals`. Vanilla builds and frees a throwaway debug string in step 3, which
    /// the port skips. Step 3 dereferences the habitat unchecked, so a null one crashes vanilla; the port treats it as
    /// "not a tank".
    pub fn update_goals(self) {
        let habitat = call_slot_0(self.0, slots::GET_HABITAT);
        let cog_goal = self.0 + unit_offsets::COG_GOAL;
        let cog_arg = (get_module_base("zoo.exe") as u32 + COG_GOAL_ARG_RVA) as *const i32;
        let goal: u32 = get_from_memory(self.0 + unit_offsets::GOAL_ID);

        if get_from_memory::<u8>(self.0 + unit_offsets::PREDATOR_CHECK_PENDING) != 0 && goal != GOAL_PREDATOR {
            let predator = call_slot_1(self.0, slots::FIND_PREDATOR, 0);
            if predator != 0 {
                call_slot_2(self.0, slots::GOAL_SET, GOAL_PREDATOR, 3);
                unsafe { COG_GOAL_SET_ID.original()(cog_goal as *const u32, predator, cog_arg) };
                save_to_memory(self.0 + unit_offsets::PREDATOR, predator);
            }
            save_to_memory(self.0 + unit_offsets::PREDATOR_CHECK_PENDING, 0u8);
            return self.finish_update_goals();
        }

        if goal != GOAL_ESCAPED_ANIMAL && call_slot_0(self.0, slots::FOUND_ESCAPED_ANIMAL) & 0xff != 0 {
            call_slot_3(self.0, slots::STOP_EVERYTHING, 1, 0, 1);
            let animal: u32 = get_from_memory(self.0 + unit_offsets::ESCAPED_ANIMAL);
            unsafe { COG_GOAL_SET.original()(cog_goal as *const u32, GOAL_ESCAPED_ANIMAL as i32, 3, animal, cog_arg, 0) };
            return self.finish_update_goals();
        }

        // The habitat manager is read from its global on every call, as vanilla does.
        let habitat_count = || {
            let manager: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + HABITAT_MGR_GLOBAL_RVA);
            if manager == 0 {
                return 0;
            }
            let (start, end): (u32, u32) = (get_from_memory(manager + HABITAT_ARRAY_START_OFFSET), get_from_memory(manager + HABITAT_ARRAY_END_OFFSET));
            (end.wrapping_sub(start) as i32) >> 2
        };
        let cog_valid = || call_slot_0(cog_goal, 0) & 0xff != 0;
        let goal_flag = || get_from_memory::<u8>(self.0 + unit_offsets::COG_GOAL_FLAG) != 0;
        let in_world_habitat = habitat != 0 && get_from_memory::<u8>(habitat + habitat_offsets::IS_WORLD) != 0;

        if !cog_valid() || (goal_flag() && in_world_habitat) {
            if habitat != 0 && call_slot_0(habitat, slots::HABITAT_IS_TANK) & 0xff != 0 {
                call_slot_2(self.0, slots::GOAL_SET, GOAL_TANK, 0);
            } else {
                if habitat_count() <= 0 {
                    call_slot_2(self.0, slots::GOAL_SET, GOAL_NO_HABITAT, 1);
                    save_to_memory(self.0 + unit_offsets::COG_INVALID, 0u8);
                } else {
                    call_slot_2(self.0, slots::GOAL_SET, GOAL_NO_HABITAT_WITH_HABITATS, 4);
                }
                call_slot_2(self.0, slots::SET_TARGET_TILE, 0, 1);
            }
            return self.finish_update_goals();
        }

        if cog_valid() && !goal_flag() {
            return self.finish_update_goals();
        }
        if habitat == 0 || in_world_habitat || !self.in_target_habitat() {
            return self.finish_update_goals();
        }
        let habitat_ref = unsafe { ref_from_memory::<ZTHabitat>(habitat) };
        if habitat_ref.is_being_serviced() && habitat_ref.get_num_keepers() >= 2 {
            return self.finish_update_goals();
        }
        call_slot_2(self.0, slots::GOAL_SET, GOAL_HABITAT_DUTIES, 0);
        self.finish_update_goals();
    }

    /// `ZTKeeper::listen` (`0x00423c03`): a keeper tied to a show (cog-goal state `0x13`) reacts to its show's script
    /// events by switching behavior sets. The last `0x271d` event names a script item whose pre-trick string is
    /// sound A; the last `0x271f` event names one whose post-trick string is sound B (`0x2715` also sets that index
    /// without arming B). A non-empty sound B wins over sound A; the chosen string becomes the keeper's behavior set
    /// through `setBS` (call flag 0, forced, mixer untouched). The event vector `getEvents` filled is vanilla memory
    /// and is released to the pool as vanilla does.
    pub fn listen(self) {
        let mut events = [0u32; 3];
        call_slot_1(self.0, slots::GET_EVENTS_LIST, events.as_mut_ptr() as u32);
        let (begin, end, capacity) = unsafe { (std::ptr::read_volatile(&events[0]), std::ptr::read_volatile(&events[1]), std::ptr::read_volatile(&events[2])) };

        let show_id = get_from_memory::<u16>(self.0 + unit_offsets::SHOW_INFO_ID);
        let show_state: u32 = get_from_memory(self.0 + unit_offsets::COG_GOAL_STATE);
        let (mut index_a, mut index_b) = (0xffff_u16, 0xffff_u16);
        let (mut sound_a_armed, mut sound_b_armed) = (false, false);
        if begin != end {
            for record in (begin..end).step_by(EVENT_SIZE as usize) {
                if get_from_memory::<u32>(record + event_offsets::SHOW_ID) != show_id as u32 || show_state != GOAL_STATE_SHOW {
                    continue;
                }
                let index: u16 = get_from_memory(record + event_offsets::ITEM_INDEX);
                match get_from_memory::<u16>(record + event_offsets::KIND) {
                    EVENT_SHOW_ITEM => index_b = index,
                    EVENT_SHOW_SOUND_A => {
                        sound_a_armed = true;
                        index_a = index;
                    }
                    EVENT_SHOW_SOUND_B => {
                        sound_b_armed = true;
                        index_b = index;
                    }
                    _ => {}
                }
            }
        }

        let (mut sound_a, mut sound_b) = (Vec::<u8>::new(), Vec::<u8>::new());
        if sound_a_armed || sound_b_armed {
            let show_info = ZTShowMgr::get_show_info(show_id);
            if show_info != 0 {
                let script = show_script_get(get_from_memory::<u16>(show_info + SHOW_INFO_SCRIPT_ID_OFFSET));
                if script != 0 {
                    if sound_a_armed {
                        sound_a = item_string(show_script_get_item(script, index_a), ITEM_SOUND_A_OFFSET);
                    }
                    if sound_b_armed {
                        sound_b = item_string(show_script_get_item(script, index_b), ITEM_SOUND_B_OFFSET);
                    }
                }
                if sound_b_armed && !sound_b.is_empty() {
                    self.play_behavior_set(&sound_b);
                } else if sound_a_armed && !sound_a.is_empty() {
                    self.play_behavior_set(&sound_a);
                }
            }
        }

        if begin != 0 {
            let bytes = (capacity.wrapping_sub(begin) / EVENT_SIZE) * EVENT_SIZE;
            unsafe { POOL_DEALLOCATE.original()(begin as *const u32, bytes) };
        }
    }

    /// Hands `name` to `BFEntity::setBS` as a vanilla string, which takes it by value and frees it.
    fn play_behavior_set(self, name: &[u8]) {
        let string = VanillaString::from_bytes(name);
        let base = string.as_ptr() as u32;
        let raw = RawString {
            begin: get_from_memory::<u32>(base) as *const u8,
            end: get_from_memory::<u32>(base + 4) as *const u8,
            capacity: get_from_memory::<u32>(base + 8) as *const u8,
        };
        std::mem::forget(string);
        unsafe { set_bs(self.0, raw, 0, 1, SET_BS_CLEAR_MODE) };
    }

    /// `ZTKeeper::save` (`0x004796d9`): the habitat-select dword, the other four dwords, the `+0x2a8` byte, then the
    /// `ZTStaff` data. Every write and the `ZTStaff` save run even after a failure; the result is their conjunction.
    pub fn save(self, file: *const u32) -> bool {
        let mut ok = true;
        for offset in [HABITAT_SELECT_DISABLED_OFFSET, FOOD_TO_LEAVE_OFFSET, SICK_ANIMAL_COUNT_OFFSET, DIRT_TILE_COUNT_OFFSET, FOOD_TYPE_TO_LEAVE_OFFSET] {
            ok &= write_field(self.0 + offset, 4, file);
        }
        ok &= write_field(self.0 + PATHING_TO_HABITAT_GATE_OFFSET, 1, file);
        let staff_ok = unsafe { ZTSTAFF_SAVE.original()(self.0 as *const u32, file) };
        staff_ok && ok
    }

    /// `ZTKeeper::load` (`0x00455444`): reads what `save` wrote, gated by the save `version`, then the `ZTStaff`
    /// data. Every read runs; `ZTStaff::load` is skipped only if one failed. Version `0x10` reads and discards a
    /// 25-byte legacy block after the habitat-select dword and nothing else; versions in `0x34..=0x4a` also read a
    /// dword into `+0x290`.
    pub fn load(self, file: *const u32, version: u32) -> bool {
        let mut ok = true;
        if version >= save_version::SELECT_GATE {
            ok = read_field(self.0 + HABITAT_SELECT_DISABLED_OFFSET, 4, file);
        }
        if version == save_version::LEGACY_BLOCK {
            let mut scratch = [0u8; 4];
            for size in LEGACY_BLOCK_READS {
                ok &= read_field(scratch.as_mut_ptr() as u32, size, file);
            }
        } else {
            if version > save_version::OWN_FIELDS_AFTER {
                for offset in [FOOD_TO_LEAVE_OFFSET, SICK_ANIMAL_COUNT_OFFSET, DIRT_TILE_COUNT_OFFSET, FOOD_TYPE_TO_LEAVE_OFFSET] {
                    ok &= read_field(self.0 + offset, 4, file);
                }
                ok &= read_field(self.0 + PATHING_TO_HABITAT_GATE_OFFSET, 1, file);
            }
            if save_version::EXTRA_DWORD.contains(&version) {
                ok &= read_field(self.0 + EXTRA_DWORD_OFFSET, 4, file);
            }
        }
        if !ok {
            return false;
        }
        unsafe { ZTSTAFF_LOAD.original()(self.0 as *const u32, file, version) }
    }

    /// `ZTKeeper::ZTKeeper` (`0x00457b2e`): the `ZTStaff` constructor, a clear `+0x2a8` byte and the keeper vtable.
    pub fn construct(self, keeper_type: u32) {
        unsafe { ZTSTAFF_CONSTRUCTOR.original()(self.0 as *const u32, keeper_type) };
        self.set_pathing_to_habitat_gate(0);
        save_to_memory(self.0, get_module_base("zoo.exe") as u32 + KEEPER_VTABLE_RVA);
    }

    /// `ZTKeeper::~ZTKeeper` (`0x004f9da1`, non-deleting): restores the keeper vtable, then the `ZTStaff`
    /// destructor. Returns what that destructor returns.
    pub fn destruct(self) -> *const u32 {
        save_to_memory(self.0, get_module_base("zoo.exe") as u32 + KEEPER_VTABLE_RVA);
        unsafe { ZTSTAFF_DESTRUCTOR_0.original()(self.0 as *const u32) }
    }
}

#[detour_mod]
mod keeper_detours {
    use super::*;

    #[detour(SET_SPEED)]
    unsafe extern "thiscall" fn set_speed(this: *const u32, speed: u32) {
        ZTKeeper(this as u32).set_speed(speed);
    }

    #[detour(SET_TARGET_TILE)]
    unsafe extern "thiscall" fn set_target_tile(this: *const u32, tile: *const u32, flag: bool) {
        ZTKeeper(this as u32).set_target_tile(tile as u32, flag);
    }

    #[detour(RESET_AI)]
    unsafe extern "thiscall" fn reset_ai(this: *const u32) {
        ZTKeeper(this as u32).reset_ai();
    }

    #[detour(GET_EVENTS)]
    unsafe extern "thiscall" fn get_events(this: *const c_void, out: u32) {
        ZTKeeper(this as u32).get_events(out);
    }

    #[detour(GET_EVENTS_0X43F40D)]
    unsafe extern "thiscall" fn get_events_from_aimgr(_this: *const c_void, arg1: u32, arg2: u32, arg3: u32) {
        ZTKeeper::get_events_from_aimgr(arg1, arg2, arg3);
    }

    #[detour(IN_TARGET_HABITAT)]
    unsafe extern "thiscall" fn in_target_habitat(this: *const c_void) -> bool {
        ZTKeeper(this as u32).in_target_habitat()
    }

    #[detour(CHECK_ASSIGNED_HABITATS)]
    unsafe extern "thiscall" fn check_assigned_habitats(this: *const u32) {
        ZTKeeper(this as u32).check_assigned_habitats();
    }

    #[detour(DO_CONDITIONALS)]
    unsafe extern "thiscall" fn do_conditionals(this: *const u32) {
        ZTKeeper(this as u32).do_conditionals();
    }

    #[detour(CLEANS_UP)]
    unsafe extern "thiscall" fn cleans_up(this: *const u32, entity: *const u32) -> bool {
        ZTKeeper(this as u32).cleans_up(entity as u32)
    }

    #[detour(INIT_AFTER_LOAD)]
    unsafe extern "thiscall" fn init_after_load(this: *const u32, param: u32) -> bool {
        ZTKeeper(this as u32).init_after_load(param)
    }

    #[detour(DROP)]
    unsafe extern "thiscall" fn drop_keeper(this: *const u32) {
        ZTKeeper(this as u32).drop_keeper();
    }

    #[detour(GET_RANDOM_ASSIGNED_HABITAT)]
    unsafe extern "thiscall" fn get_random_assigned_habitat(this: *const u32) -> *const u32 {
        ZTKeeper(this as u32).get_random_assigned_habitat() as *const u32
    }

    #[detour(F_ROUTINE)]
    unsafe extern "thiscall" fn f_routine(this: *const u32, done: *const bool) -> bool {
        ZTKeeper(this as u32).f_routine(done)
    }

    #[detour(F_HABITAT_ROUTINE)]
    unsafe extern "thiscall" fn f_habitat_routine(this: *const c_void, done: *const u8) -> u32 {
        ZTKeeper(this as u32).f_habitat_routine(done as *const bool);
        1
    }

    #[detour(F_ESCAPED_ANIMAL_ROUTINE)]
    unsafe extern "thiscall" fn f_escaped_animal_routine(this: *const c_void, done: *const u8) -> u32 {
        ZTKeeper(this as u32).f_escaped_animal_routine(done as *const bool);
        1
    }

    #[detour(NOTIFY_HABITAT_NOT_COMING)]
    unsafe extern "thiscall" fn notify_habitat_not_coming(this: *const u32, keep_gate: u32) {
        ZTKeeper(this as u32).notify_habitat_not_coming(keep_gate);
    }

    #[detour(REMOVE_FROM_MAP)]
    unsafe extern "thiscall" fn remove_from_map(this: *const u32, param: bool) {
        ZTKeeper(this as u32).remove_from_map(param);
    }

    #[detour(INIT_STATUS_VARS)]
    unsafe extern "thiscall" fn init_status_vars(this: *const u32) -> u32 {
        ZTKeeper(this as u32).init_status_vars()
    }

    #[detour(GET_TERRAIN_COST)]
    unsafe extern "thiscall" fn get_terrain_cost(this: *const u32, tile: *const u32) -> i32 {
        ZTKeeper(this as u32).get_terrain_cost(tile as u32)
    }

    #[detour(UPDATE_GOALS)]
    unsafe extern "thiscall" fn update_goals(this: *const u32) {
        ZTKeeper(this as u32).update_goals();
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn listen(this: *const c_void) {
        ZTKeeper(this as u32).listen();
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        ZTKeeper(this as u32).save(file)
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        ZTKeeper(this as u32).load(file, version)
    }

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor(this: *const u32, keeper_type: *const u32) -> *const u32 {
        ZTKeeper(this as u32).construct(keeper_type as u32);
        this
    }

    /// The scalar-deleting destructor (vtable `+0x18`): destroys, then frees when bit 0 of `flags` is set.
    #[detour(DESTRUCTOR_0)]
    unsafe extern "thiscall" fn destructor_deleting(this: *const u32, flags: u8) -> *const u32 {
        ZTKeeper(this as u32).destruct();
        if flags & 1 != 0 {
            unsafe { OPERATOR_DELETE.original()(this as u32) };
        }
        this
    }

    #[detour(DESTRUCTOR_1)]
    unsafe extern "thiscall" fn destructor(this: *const u32) -> *const u32 {
        ZTKeeper(this as u32).destruct()
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn set_speed(this: u32, speed: u32) {
            unsafe { super::SET_SPEED_DETOUR.call(this as *const u32, speed) }
        }

        pub(crate) fn set_target_tile(this: u32, tile: u32, flag: bool) {
            unsafe { super::SET_TARGET_TILE_DETOUR.call(this as *const u32, tile as *const u32, flag) }
        }

        pub(crate) fn reset_ai(this: u32) {
            unsafe { super::RESET_AI_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_events(this: u32, out: u32) {
            unsafe { super::GET_EVENTS_DETOUR.call(this as *const c_void, out) }
        }

        pub(crate) fn get_events_from_aimgr(arg1: u32, arg2: u32, arg3: u32) {
            unsafe { super::GET_EVENTS_0X43F40D_DETOUR.call(std::ptr::null::<c_void>(), arg1, arg2, arg3) }
        }

        pub(crate) fn in_target_habitat(this: u32) -> bool {
            unsafe { super::IN_TARGET_HABITAT_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn check_assigned_habitats(this: u32) {
            unsafe { super::CHECK_ASSIGNED_HABITATS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_conditionals(this: u32) {
            unsafe { super::DO_CONDITIONALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn init_after_load(this: u32, param: u32) -> bool {
            unsafe { super::INIT_AFTER_LOAD_DETOUR.call(this as *const u32, param) }
        }

        pub(crate) fn cleans_up(this: u32, entity: u32) -> bool {
            unsafe { super::CLEANS_UP_DETOUR.call(this as *const u32, entity as *const u32) }
        }

        pub(crate) fn drop_keeper(this: u32) {
            unsafe { super::DROP_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_random_assigned_habitat(this: u32) -> u32 {
            unsafe { super::GET_RANDOM_ASSIGNED_HABITAT_DETOUR.call(this as *const u32) as u32 }
        }

        pub(crate) fn f_routine(this: u32, done: *const bool) -> bool {
            unsafe { super::F_ROUTINE_DETOUR.call(this as *const u32, done) }
        }

        pub(crate) fn f_habitat_routine(this: u32, done: *const u8) -> u32 {
            unsafe { super::F_HABITAT_ROUTINE_DETOUR.call(this as *const c_void, done) }
        }

        pub(crate) fn f_escaped_animal_routine(this: u32, done: *const u8) -> u32 {
            unsafe { super::F_ESCAPED_ANIMAL_ROUTINE_DETOUR.call(this as *const c_void, done) }
        }

        pub(crate) fn notify_habitat_not_coming(this: u32, keep_gate: u32) {
            unsafe { super::NOTIFY_HABITAT_NOT_COMING_DETOUR.call(this as *const u32, keep_gate) }
        }

        pub(crate) fn remove_from_map(this: u32, param: bool) {
            unsafe { super::REMOVE_FROM_MAP_DETOUR.call(this as *const u32, param) }
        }

        pub(crate) fn init_status_vars(this: u32) -> u32 {
            unsafe { super::INIT_STATUS_VARS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_terrain_cost(this: u32, tile: u32) -> i32 {
            unsafe { super::GET_TERRAIN_COST_DETOUR.call(this as *const u32, tile as *const u32) }
        }

        pub(crate) fn update_goals(this: u32) {
            unsafe { super::UPDATE_GOALS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn listen(this: u32) {
            unsafe { super::LISTEN_DETOUR.call(this as *const c_void) }
        }

        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }

        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file, version) }
        }

        pub(crate) fn constructor(this: u32, keeper_type: u32) -> *const u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this as *const u32, keeper_type as *const u32) }
        }

        pub(crate) fn destructor_deleting(this: u32, flags: u8) -> *const u32 {
            unsafe { super::DESTRUCTOR_0_DETOUR.call(this as *const u32, flags) }
        }

        pub(crate) fn destructor(this: u32) -> *const u32 {
            unsafe { super::DESTRUCTOR_1_DETOUR.call(this as *const u32) }
        }
    }
}

/// Installs `ZTKeeper`'s detours. Called once at DLL load (via `lib.rs`) and before the `ZTKEEPER_*` tests in
/// `reimplementation_tests::init`.
pub fn init() {
    if let Err(e) = unsafe { keeper_detours::init_detours() } {
        error!("Failed to initialise ztkeeper detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::standalone::OPERATOR_NEW;

    use super::{keeper_detours::test_real, ZTKeeper, KEEPER_SIZE, KEEPER_TYPE_TAG_RVA};
    use crate::{
        globals::{get_module_base, globals},
        util::get_from_memory,
    };

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::keeper_detours::status()
    }

    /// The first `ZTKeeperType` the live world holds, if any.
    pub fn find_keeper_type() -> Option<u32> {
        let world = globals().ztworldmgr();
        let tag = get_module_base("zoo.exe") as u32 + KEEPER_TYPE_TAG_RVA;
        let mut slot = world.entity_type_array_start;
        while slot != world.entity_type_array_end {
            let ty: u32 = get_from_memory(slot);
            slot += 4;
            if ty == 0 {
                continue;
            }
            let vtable: u32 = get_from_memory(ty);
            let is_cast_class =
                unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
            if is_cast_class(ty, tag) {
                return Some(ty);
            }
        }
        None
    }

    /// Zeroed keeper-sized memory from vanilla's allocator, unconstructed.
    pub fn allocate_zeroed_keeper() -> ZTKeeper {
        let memory = unsafe { OPERATOR_NEW.original()(KEEPER_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(memory as *mut u8, 0, KEEPER_SIZE as usize) };
        ZTKeeper(memory)
    }

    /// A zeroed keeper built by the real vanilla constructor over `keeper_type`.
    pub fn build_standalone_keeper(keeper_type: u32) -> ZTKeeper {
        let keeper = allocate_zeroed_keeper();
        test_real::constructor(keeper.0, keeper_type);
        keeper
    }

    /// A zeroed keeper built by the Rust constructor over `keeper_type`.
    pub fn build_standalone_keeper_port(keeper_type: u32) -> ZTKeeper {
        let keeper = allocate_zeroed_keeper();
        keeper.construct(keeper_type);
        keeper
    }

    /// Tears a keeper down through the real scalar-deleting destructor.
    pub fn destroy_standalone_keeper(keeper: ZTKeeper) {
        test_real::destructor_deleting(keeper.0, 1);
    }

    pub(crate) use super::keeper_detours::test_real as real;
}
