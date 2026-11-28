//! `ZTGuest`'s goal selection and per-tick entry points: `updateGoals`, `update`, `listen`, `fGuestThought`
//! and `initAfterLoad`. Reads and writes guest memory in place through [`ZTGuest`].
//!
//! `updateGoals` is a priority ladder over the guest's current goal (`+0x170`): each rung either replaces the goal
//! through the virtual `+0x1d8` setter and stops, or falls through to the next. Every exit but two runs
//! `ZTUnit::updateGoals` last; the two that tail into `FUN_004290c9` run it through that helper. Debug strings
//! vanilla builds and discards are not built here.

use openzt_detour::{
    generated::{
        bfcoggoal::{SET as BFCOGGOAL_SET, SET_ID as BFCOGGOAL_SET_ID},
        bfentity::{GET_TILE as BFENTITY_GET_TILE, SET_BS, UPDATE as BFENTITY_UPDATE},
        msvc_std_basic_string::BASIC_STRING_RANGE,
        ztguest::{F_GUEST_THOUGHT, GET_NUM_NONSEEN_SITES, INIT_AFTER_LOAD, LISTEN, TRIGGER_GRANDSTAND_ESCAPE_1, UPDATE, UPDATE_GOALS},
        ztthoughtmgr::ADD_THOUGHT,
        ztworldmgr::FIND_NEAREST_UNSEEN_STARTING_SHOW,
    },
};
use std::ffi::c_void;

use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{
    guest_offsets, lifecycle_offsets, ZTGuest, GUEST_TYPE_TAG_RVA,
};
use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, position_in_tile_xy, position_in_tile_z},
    zthabitat::next_game_random,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, call_slot_3},
};

/// `CAST_ZTAnimal` (`0x00638690`) as an RVA.
const ANIMAL_TYPE_TAG_RVA: u32 = 0x0023_8690;

/// `DAT_00639360`, whose address `BFCogGoal::set`/`setID` take as their trailing pointer.
const COG_GOAL_ARG_RVA: u32 = 0x0023_9360;
/// `DAT_006394b0`, a dword copied into `+0x230` when a goal is taken over.
const GOAL_DEFAULT_RVA: u32 = 0x0023_94b0;
/// `DAT_006393ec`, a dword copied into `+0x230` when the guest heads for a show.
const SHOW_DEFAULT_RVA: u32 = 0x0023_93ec;
/// `DAT_0063afb8`, the need name `getNeedIndex` resolves for a show.
const SHOW_NEED_NAME_RVA: u32 = 0x0023_afb8;

/// Goal ids `updateGoals` tests and sets.
mod goal {
    pub const HOME_SEARCH: u32 = 0x64;
    pub const SHOW: u32 = 0x66;
    pub const SEAT: u32 = 0x67;
    pub const EXHIBIT_A: u32 = 0x68;
    pub const EXHIBIT_B: u32 = 0x69;
    pub const EXHIBIT_C: u32 = 0x6a;
    pub const SOUVENIR: u32 = 0x6b;
    pub const SOUVENIR_ITEM: u32 = 0x6c;
    pub const SOUVENIR_ITEM_B: u32 = 0x6d;
    pub const LEAVE: u32 = 0x71;
    pub const GRANDSTAND: u32 = 0x75;
    pub const CHASE: u32 = 0x79;
    pub const SCARED: u32 = 0x7f;
    pub const SCARED_B: u32 = 0x84;
}

mod fields {
    pub const GOAL: u32 = 0x170;
    pub const ENTITY_ID: u32 = 0x124;
    pub const ENTITY_TYPE: u32 = 0x128;
    /// `BFCogGoal` embedded in the unit; its byte at `+0x4c` (`0x1a8`) is a flag the ladder tests.
    pub const COG_GOAL: u32 = 0x15c;
    pub const COG_FLAG: u32 = 0x1a8;
    /// Head of the `list<uint>` of followers/targets; empty when the head points to itself.
    pub const LIST_HEAD: u32 = 0x154;
    pub const FLAG_0X20E: u32 = 0x20e;
    pub const GOAL_TARGET: u32 = 0x22c;
    pub const GOAL_DEFAULT: u32 = 0x230;
    pub const FLAG_0X24C: u32 = 0x24c;
    pub const FLAG_0X25C: u32 = 0x25c;
    pub const NEED_INDEX: u32 = 0x24e;
    pub const WORD_0X334: u32 = 0x334;
    pub const SCARED_FLAG: u32 = 0x336;
    pub const LEAVING: u32 = 0x339;
    pub const FORCE_CHECK: u32 = 0x33e;
    pub const NEEDS_FLAG: u32 = 0x33f;
    /// Type field: percentage chance the guest chases after being startled.
    pub const TYPE_CHASE_PERCENT: u32 = 0x2b4;
    /// Show entity fields.
    pub const SHOW_X: u32 = 0x114;
    pub const SHOW_Y: u32 = 0x118;
    pub const SHOW_Z: u32 = 0x11c;
    /// Puking/screaming source bits and the two animation-vector fields `update` reads.
    pub const SOURCE_BITS: u32 = 0x340;
    pub const HIDDEN_A: u32 = 0x142;
    pub const HIDDEN_B: u32 = 0x143;
    pub const MARK_BEGIN: u32 = 0xfc;
    pub const MARK_END: u32 = 0x100;
    /// Entity type's name pointer.
    pub const TYPE_NAME: u32 = 0x98;
}

mod slots {
    pub const CLEAR_GOAL: u32 = 0x174;
    pub const GOAL_SET: u32 = 0x1d8;
    pub const STOP_EVERYTHING: u32 = 0x198;
    /// Predicate whose true answer ends `updateGoals` through `FUN_004290c9`.
    pub const RELEASE_PREDICATE: u32 = 0x184;
    /// Returns the unit's current target object; that object's `+0x20` answers whether it is dangerous.
    pub const TARGET_OBJECT: u32 = 0x24c;
    pub const TARGET_DANGEROUS: u32 = 0x20;
    /// Returns a goal id (`0` for none); the argument is a flag.
    pub const FIND_GOAL: u32 = 0x2fc;
    pub const SET_TARGET_TILE: u32 = 0x2e4;
    /// `BFCogGoal` vtable: `valid` and `hasTile`.
    pub const COG_VALID: u32 = 0x0;
    pub const COG_HAS_TILE: u32 = 0x8;
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// `ZTWorldMgr::findNearestUnseenStartingShow` (`0x00440f0c`): `stdcall(guest)`; the entry's `ECX` setup is incidental.
fn find_nearest_unseen_starting_show(guest: u32) -> u32 {
    unsafe { FIND_NEAREST_UNSEEN_STARTING_SHOW.hooked()(guest as *const std::ffi::c_void) }
}

impl ZTGuest {
    fn goal(self) -> u32 {
        self.read(fields::GOAL)
    }

    fn goal_is_exhibit(self) -> bool {
        matches!(self.goal(), goal::EXHIBIT_A | goal::EXHIBIT_B | goal::EXHIBIT_C)
    }

    fn cog_goal(self) -> u32 {
        self.0 + fields::COG_GOAL
    }

    fn first_item(self) -> u32 {
        let begin: u32 = self.read(guest_offsets::ITEMS);
        if begin == 0 {
            return 0;
        }
        get_from_memory(begin)
    }

    fn clear_goal_slot(self) {
        call_slot_0(self.0, slots::CLEAR_GOAL);
    }

    fn set_goal_virtual(self, id: u32, kind: u32) {
        call_slot_2(self.0, slots::GOAL_SET, id, kind);
    }

    fn finish_goals(self) {
        crate::ztunit::ZTUnit(self.0).update_goals();
    }

    /// `FUN_004290c9`: the tail two exits share.
    fn release_tail(self) {
        self.set_goal_virtual(1, 1);
        self.write(fields::FLAG_0X20E, 0u8);
        self.finish_goals();
    }

    /// `ZTBuilding::meth_0x437392` (`0x00437392`), run on a guest: for the show, seat or exhibit goals the cog-goal
    /// flag decides, and the second exhibit goal always answers true.
    fn building_goal_active(self, entry_goal: u32) -> bool {
        let in_set = matches!(self.goal(), goal::SHOW | goal::SEAT) || self.goal_is_exhibit();
        if in_set && self.read::<u8>(fields::COG_FLAG) != 0 {
            return true;
        }
        entry_goal == goal::EXHIBIT_B
    }

    fn type_is_guest(entity_type: u32) -> bool {
        if entity_type == 0 {
            return false;
        }
        let vtable: u32 = get_from_memory(entity_type);
        let is_cast_class = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
        is_cast_class(entity_type, base() + GUEST_TYPE_TAG_RVA)
    }

    /// Takes over goal `GRANDSTAND` with the id the unit's finder returned (`BFCogGoal::setID`).
    fn take_over_grandstand(self, id: u32) {
        self.set_goal_virtual(goal::GRANDSTAND, 3);
        let default: u32 = get_from_memory(base() + GOAL_DEFAULT_RVA);
        self.write(fields::GOAL_DEFAULT, default);
        unsafe { BFCOGGOAL_SET_ID.hooked()(self.cog_goal() as *const u32, id, (base() + COG_GOAL_ARG_RVA) as *const i32) };
    }

    /// `ZTGuest::updateGoals` (`0x0043a6e4`).
    pub fn update_goals(self) {
        let entry_goal = self.goal();
        let entity_type: u32 = self.read(fields::ENTITY_TYPE);
        let guest_type = if Self::type_is_guest(entity_type) { entity_type } else { 0 };
        let cog = self.cog_goal();

        // Something dangerous is targeted: stop, panic, and flee.
        let target = call_slot_0(self.0, slots::TARGET_OBJECT);
        if target != 0
            && low_byte_bool(call_slot_0(target, slots::TARGET_DANGEROUS))
            && entry_goal != goal::SCARED
            && entry_goal != goal::SCARED_B
            && (entry_goal != goal::GRANDSTAND || crate::ztunit::ZTUnit(self.0).in_friendly_chase())
        {
            self.write(fields::FLAG_0X24C, 0u8);
            call_slot_3(self.0, slots::STOP_EVERYTHING, 1, 1, 1);
            unsafe { F_GUEST_THOUGHT.hooked()(self.0 as *const u32, 0x2948, std::ptr::null(), std::ptr::null()) };
            self.set_goal_virtual(goal::SCARED, 0);
            return self.finish_goals();
        }

        if self.leave_ok() {
            self.set_goal_virtual(goal::LEAVE, 1);
            return self.finish_goals();
        }

        if self.read::<u8>(fields::FLAG_0X24C) != 0 && entry_goal != goal::GRANDSTAND && (!self.goal_is_exhibit() || self.in_grandstand()) {
            let found = call_slot_1(self.0, slots::FIND_GOAL, 0);
            if found == 0 {
                self.write(fields::FLAG_0X24C, 0u8);
                return self.finish_goals();
            }
            if self.in_grandstand() {
                unsafe { TRIGGER_GRANDSTAND_ESCAPE_1.hooked()(self.0 as *const u32, found as i32) };
                return self.finish_goals();
            }
            self.clear_goal_slot();
            self.take_over_grandstand(found);
            self.write(fields::GOAL_TARGET, found);
            self.write(fields::LEAVING, 0u8);
            return self.finish_goals();
        }

        if self.read::<u8>(fields::FLAG_0X25C) != 0 && entry_goal != goal::GRANDSTAND {
            if self.read::<u32>(fields::GOAL_TARGET) == 0 {
                let found = call_slot_1(self.0, slots::FIND_GOAL, 0);
                if found != 0 {
                    self.write(fields::GOAL_TARGET, found);
                }
            }
            self.clear_goal_slot();
            let target_id: u32 = self.read(fields::GOAL_TARGET);
            self.take_over_grandstand(target_id);
            return self.finish_goals();
        }

        if self.read::<u8>(fields::SCARED_FLAG) != 0
            && !matches!(entry_goal, goal::CHASE | goal::SCARED | goal::LEAVE)
            && !self.goal_is_exhibit()
        {
            let percent: i32 = if guest_type == 0 { 0 } else { get_from_memory(guest_type + fields::TYPE_CHASE_PERCENT) };
            let roll = (next_game_random() % 100) as i32;
            if percent > roll {
                let id: u32 = self.read(fields::ENTITY_ID);
                unsafe { BFCOGGOAL_SET.hooked()(cog as *const u32, goal::CHASE as i32, 4, id, (base() + COG_GOAL_ARG_RVA) as *const i32, 0) };
                return self.finish_goals();
            }
        }

        if self.read::<u8>(fields::NEEDS_FLAG) != 0 && !matches!(entry_goal, goal::SOUVENIR_ITEM | goal::SOUVENIR_ITEM_B) {
            self.clear_goal_and_souvenir();
            return self.finish_goals();
        }

        if self.building_goal_active(entry_goal) {
            if entry_goal != goal::EXHIBIT_B || self.read::<u8>(fields::COG_FLAG) == 0 {
                return self.finish_goals();
            }
            if self.first_item() == 0 {
                self.write(fields::FORCE_CHECK, 1u8);
            } else {
                self.set_goal_virtual(goal::SOUVENIR, 4);
            }
            return self.finish_goals();
        }

        if !self.keep_current_goal(entry_goal) && self.take_idle_goal(entry_goal) {
            return;
        }
        self.finish_goals();
    }

    /// Rung for a guest with the needs flag set: with nothing carried the goal is cleared; with an item it is also set
    /// to the souvenir-item goal. Written as two branches to match vanilla's order of virtual calls.
    fn clear_goal_and_souvenir(self) {
        let carrying = self.first_item() != 0;
        self.clear_goal_slot();
        if carrying {
            self.set_goal_virtual(goal::SOUVENIR_ITEM, 1);
        }
    }

    /// Whether the cog-goal state keeps the current goal as is (the label vanilla jumps to when the goal is valid and
    /// not `HOME_SEARCH`), in which case only `ZTUnit::updateGoals` runs.
    fn keep_current_goal(self, entry_goal: u32) -> bool {
        let cog = self.cog_goal();
        let cog_valid = || low_byte_bool(call_slot_0(cog, slots::COG_VALID));
        let check_valid = || cog_valid() && entry_goal != goal::HOME_SEARCH;

        let record = self.position_record();
        let grid: i32 = self.read(lifecycle_offsets::BUILDING_GRID);
        let on_satisfier = crate::ztunit::ZTUnit(self.0).is_need_or_fun_satisfier(grid as u32, record.as_ptr() as *const i32);
        if !on_satisfier {
            return check_valid();
        }
        if self.read::<u8>(fields::COG_FLAG) != 0 {
            return check_valid();
        }
        if !low_byte_bool(call_slot_0(cog, slots::COG_HAS_TILE)) && !self.goal_is_exhibit() {
            return false;
        }
        if entry_goal == goal::SHOW {
            let head: u32 = self.read(fields::LIST_HEAD);
            let first: u32 = get_from_memory(head);
            if first == head {
                return false;
            }
        }
        check_valid()
    }

    /// The idle rungs: heading for a seat-less goal, a show, or the home search. Returns `true` when the method has
    /// already run its tail.
    fn take_idle_goal(self, entry_goal: u32) -> bool {
        if self.read::<u8>(fields::NEEDS_FLAG) != 0 || matches!(entry_goal, goal::SEAT | goal::SCARED_B | goal::SCARED | goal::LEAVE) {
            return false;
        }
        if crate::ztunit::ZTUnit(self.0).update_building_tiles(false) {
            return false;
        }

        if self.first_item() != 0 && !self.goal_is_exhibit() {
            self.clear_goal_slot();
            self.set_goal_virtual(goal::SOUVENIR, 4);
            self.finish_goals();
            return true;
        }

        if self.read::<u8>(fields::FORCE_CHECK) != 0 {
            if next_game_random().is_multiple_of(0xf) {
                self.write(fields::FORCE_CHECK, 0u8);
                self.write(fields::FLAG_0X20E, 1u8);
                self.finish_goals();
                return true;
            }
            return false;
        }

        if entry_goal == goal::HOME_SEARCH {
            if low_byte_bool(call_slot_0(self.0, slots::RELEASE_PREDICATE)) {
                self.release_tail();
                return true;
            }
            return false;
        }

        self.clear_goal_slot();
        let show = find_nearest_unseen_starting_show(self.0);
        if show != 0 {
            self.head_for_show(show);
            self.finish_goals();
            return true;
        }
        if unsafe { GET_NUM_NONSEEN_SITES.hooked()(self.0 as *const u32) } > 0 {
            self.set_goal_virtual(goal::HOME_SEARCH, 4);
            self.finish_goals();
            return true;
        }
        self.write(fields::WORD_0X334, 0u16);
        if self.check_leave_zoo() {
            return false;
        }
        if low_byte_bool(call_slot_0(self.0, slots::RELEASE_PREDICATE)) {
            self.release_tail();
            return true;
        }
        false
    }

    /// Points the guest at `show`'s tile and sets the show goal.
    fn head_for_show(self, show: u32) {
        let need = Self::need_index(SHOW_NEED_NAME_RVA);
        self.write(fields::NEED_INDEX, need);
        let tile = unsafe { BFENTITY_GET_TILE.hooked()(show as *const u32) };
        call_slot_1(self.0, slots::SET_TARGET_TILE, tile as u32);
        let x: i32 = get_from_memory(show + fields::SHOW_X);
        let y: i32 = get_from_memory(show + fields::SHOW_Y);
        let z: i32 = get_from_memory(show + fields::SHOW_Z);
        self.write(lifecycle_offsets::POSITION_RECORD, position_in_tile_xy(x));
        self.write(lifecycle_offsets::POSITION_RECORD + 4, position_in_tile_xy(y));
        self.write(lifecycle_offsets::POSITION_RECORD + 8, position_in_tile_z(z));
        self.set_goal_virtual(goal::SHOW, 4);
        let default: u32 = get_from_memory(base() + SHOW_DEFAULT_RVA);
        self.write(fields::GOAL_DEFAULT, default);
        crate::ztunit::ZTUnit(self.0).update_building_tiles(false);
    }

    /// `ZTGuest::fGuestThought` (`0x004231c1`): a thought about an animal that is flagged (`+0x395`) is replaced by
    /// thought `0x280a`.
    pub fn guest_thought(self, thought: u32, subject: u32, habitat: u32) {
        #[cfg(feature = "reimplementation-tests")]
        {
            if overrides::THOUGHT.lock().ok().and_then(|hook| *hook).is_some_and(|hook| hook(self.0, thought, subject, habitat)) {
                return;
            }
        }
        let mut id = thought;
        if subject != 0 {
            let subject_type: u32 = get_from_memory(subject + fields::ENTITY_TYPE);
            let vtable: u32 = get_from_memory(subject_type);
            let is_cast_class = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
            if is_cast_class(subject_type, base() + ANIMAL_TYPE_TAG_RVA) && get_from_memory::<u8>(subject + 0x395) != 0 {
                id = 0x280a;
            }
        }
        unsafe { ADD_THOUGHT.hooked()(globals().ztthoughtmgr_ptr() as *const u32, id, self.0 as *const u32, subject as *const u32, habitat as *const u32) };
    }

    /// `ZTGuest::initAfterLoad` (`0x00595219`): for versions `0x48`-`0x4a` a guest with a non-empty list at `+0x154`
    /// clears its goal; then `ZTUnit::initAfterLoad`.
    pub fn init_after_load(self, version: u32) -> bool {
        if (0x48..0x4b).contains(&version) {
            let head: u32 = self.read(fields::LIST_HEAD);
            let first: u32 = get_from_memory(head);
            if first != head {
                self.clear_goal_slot();
            }
        }
        crate::ztunit::ZTUnit(self.0).init_after_load(version)
    }

    /// The other guests standing on this guest's tile, in tile-list order, that `visit` is called with.
    fn for_each_tile_neighbour(self, tile: u32, mut visit: impl FnMut(u32)) {
        let head: u32 = get_from_memory(tile);
        let mut node: u32 = get_from_memory(head);
        while node != head {
            let other: u32 = get_from_memory(node + 8);
            if other != 0 && other != self.0 {
                let other_type: u32 = get_from_memory(other + fields::ENTITY_TYPE);
                if Self::type_is_guest(other_type) {
                    visit(other);
                }
            }
            node = get_from_memory(node);
        }
    }

    /// The first character of `guest`'s type name.
    fn type_initial(guest: u32) -> u8 {
        let entity_type: u32 = get_from_memory(guest + fields::ENTITY_TYPE);
        let name: u32 = get_from_memory(entity_type + fields::TYPE_NAME);
        get_from_memory(name)
    }

    /// Sets the first byte of the vector `[+0xfc, +0x100)` when it is non-empty (`0x0061922d` stores `1`,
    /// `0x00619215` stores `9`).
    fn mark_first_byte(guest: u32, value: u8) {
        let begin: u32 = get_from_memory(guest + fields::MARK_BEGIN);
        let end: u32 = get_from_memory(guest + fields::MARK_END);
        if (end.wrapping_sub(begin) as i32) > 0 {
            crate::util::save_to_memory(begin, value);
        }
    }

    /// `ZTGuest::update` (`0x0043a62c`): after `BFEntity::update`, a visible guest that is puking or screaming
    /// affects the guests on its tile.
    pub fn update(self, elapsed: u32) {
        unsafe { BFENTITY_UPDATE.hooked()(self.0 as *const u32, elapsed) };
        if self.read::<u8>(fields::HIDDEN_B) != 0 || self.read::<u8>(fields::HIDDEN_A) != 0 {
            return;
        }
        let tile = |this: ZTGuest| unsafe { BFENTITY_GET_TILE.hooked()(this.0 as *const u32) } as u32;
        let bits = || self.read::<u8>(fields::SOURCE_BITS);

        if bits() & 2 != 0 {
            let tile = tile(self);
            // The tile's flag bytes gate this one: neither the low two bits of `+0x83` nor bit `0x20` of `+0x85`.
            if tile != 0 && get_from_memory::<u8>(tile + 0x83) & 3 == 0 && get_from_memory::<u8>(tile + 0x85) & 0x20 == 0 {
                self.for_each_tile_neighbour(tile, |other| {
                    if !ZTGuest(other).has_building_use_goal() {
                        ZTGuest(other).start_puking();
                    }
                });
            }
        }
        if bits() & 4 != 0 {
            let tile = tile(self);
            if tile != 0 {
                self.for_each_tile_neighbour(tile, |other| {
                    if !ZTGuest(other).has_building_use_goal() && Self::type_initial(other) == b'w' {
                        ZTGuest(other).run_screaming();
                    }
                });
            }
        }
        if bits() & 8 != 0 {
            let tile = tile(self);
            if tile != 0 {
                self.for_each_tile_neighbour(tile, |other| {
                    if !ZTGuest(other).has_building_use_goal() && matches!(Self::type_initial(other), b'b' | b'g') {
                        ZTGuest(other).run_screaming();
                    }
                });
            }
        }
        if bits() & 0x20 != 0 {
            let tile = tile(self);
            if tile != 0 {
                self.for_each_tile_neighbour(tile, |other| Self::mark_first_byte(other, 1));
            }
        }
        if bits() & 0x10 != 0 {
            let tile = tile(self);
            if tile != 0 {
                self.for_each_tile_neighbour(tile, |other| Self::mark_first_byte(other, 9));
            }
        }
    }
}

/// `GLOBAL_ZTShowMgr` as an RVA (the global holds the manager pointer); `+0x14`, `+0x18` and `+0x1c` are the three
/// performance thresholds `listen` compares a show's time against.
const SHOW_MGR_RVA: u32 = 0x0023_9008;

/// Message ids `listen` reacts to.
mod message {
    pub const EXIT_SHOW: u16 = 0x2715;
    pub const EXIT_SHOW_BAD: u16 = 0x2717;
    pub const BAD_TRICK: u16 = 0x272a;
    pub const TRICK_SKIPPED: u16 = 0x272b;
    pub const GOOD_TRICK: u16 = 0x272c;
    pub const GREAT_TRICK: u16 = 0x272d;
}

/// Size of one pending-message record in the vector at `+0x4`/`+0x8`: `+0x10` a time or amount, `+0x14` a word,
/// `+0x16` the id and `+0x18` a state word (only states `0` and `1` are handled).
const MESSAGE_SIZE: u32 = 0x1c;

const SHOW_EXIT_NAMES: [&[u8]; 4] = [b"bExitShow", b"bExitShowBad", b"bExitShowGreat", b"bExitShowGood"];

impl ZTGuest {
    fn animation_is(self, name: &[u8]) -> bool {
        let begin: u32 = self.read(0x54);
        let end: u32 = self.read(0x58);
        end.wrapping_sub(begin) as usize == name.len() && unsafe { std::slice::from_raw_parts(begin as *const u8, name.len()) } == name
    }

    /// `BFEntity::setBS(BFBSCall(name, 1), force = 1, mode = 2)`: the call object is `name` plus the call flag.
    fn set_behaviour(self, name: &'static [u8]) {
        let mut string = [0u32; 3];
        let start = name.as_ptr();
        unsafe {
            BASIC_STRING_RANGE.hooked()(string.as_mut_ptr() as *const c_void, start as *const u32, start.add(name.len()) as i32);
            SET_BS.hooked()(self.0 as *const u32, string[0] as *const u32, string[1] as *const u32, string[2] as *const u32, 1, 1, 2);
        }
    }

    fn guest_thought_plain(self, id: u32) {
        unsafe { F_GUEST_THOUGHT.hooked()(self.0 as *const u32, id, std::ptr::null(), std::ptr::null()) };
    }

    /// `ZTGuest::listen` (`0x0043a698`): turns the entity's pending messages into thoughts, mood changes and
    /// behaviour-set switches. A first pass notes whether a show-exit message (`0x2715`) and a bad-exit message
    /// (`0x2717`) are both pending; the second pass handles each message with state `0` or `1`.
    pub fn listen(self) {
        call_slot_1(self.0, 0x8, self.0 + 4);
        let begin: u32 = self.read(0x4);
        let first_end: u32 = self.read(0x8);
        let (mut exit_bad, mut exit_show) = (false, false);
        let mut message = begin;
        while message != first_end {
            if get_from_memory::<u16>(message + 0x18) < 2 {
                let id: u16 = get_from_memory(message + 0x16);
                if id == message::EXIT_SHOW_BAD {
                    exit_bad = true;
                } else {
                    if id == message::EXIT_SHOW {
                        exit_show = true;
                    }
                    if !exit_bad {
                        message += MESSAGE_SIZE;
                        continue;
                    }
                }
                if exit_show {
                    break;
                }
            }
            message += MESSAGE_SIZE;
        }
        if begin == first_end {
            return;
        }
        let mut message = begin;
        loop {
            if get_from_memory::<u16>(message + 0x18) < 2 {
                self.handle_message(message, &mut exit_bad, exit_show);
            }
            message += MESSAGE_SIZE;
            if message == self.read::<u32>(0x8) {
                break;
            }
        }
    }

    fn handle_message(self, message: u32, exit_bad: &mut bool, exit_show: bool) {
        let id: u16 = get_from_memory(message + 0x16);
        let time: u32 = get_from_memory(message + 0x10);
        let mut name: Option<&'static [u8]> = None;
        if self.animation_is(b"bExitShowBad") {
            *exit_bad = true;
        }
        let show = || get_from_memory::<u32>(base() + SHOW_MGR_RVA);
        let thresholds = |show: u32| (get_from_memory::<u32>(show + 0x14), get_from_memory::<u32>(show + 0x18), get_from_memory::<u32>(show + 0x1c));
        match id {
            message::EXIT_SHOW if !*exit_bad && show() != 0 => {
                let (low, mid, high) = thresholds(show());
                name = Some(if time <= low {
                    SHOW_EXIT_NAMES[1]
                } else if time >= high {
                    SHOW_EXIT_NAMES[2]
                } else if time < mid {
                    SHOW_EXIT_NAMES[0]
                } else {
                    SHOW_EXIT_NAMES[3]
                });
            }
            message::EXIT_SHOW_BAD => name = Some(SHOW_EXIT_NAMES[1]),
            message::BAD_TRICK if !*exit_bad && !exit_show => name = Some(b"bBadTrick"),
            message::GOOD_TRICK if !*exit_bad && !exit_show => name = Some(b"bGoodTrick"),
            message::GREAT_TRICK if !*exit_bad && !exit_show => name = Some(b"bGreatTrick"),
            _ => {}
        }
        if id == message::EXIT_SHOW {
            let thought = match show() {
                0 => 0x2830,
                show => {
                    let (low, mid, high) = thresholds(show);
                    if time <= low {
                        0x282f
                    } else if time >= high {
                        0x2832
                    } else if time >= mid {
                        0x2831
                    } else {
                        0x2830
                    }
                }
            };
            self.guest_thought_plain(thought);
            let amount = -(get_from_memory::<u16>(message + 0x14) as i32) as u32;
            let record = [0u32, 0, 0, 0, 0, amount];
            call_slot_1(self.0, 0x1fc, record.as_ptr() as u32);
            call_slot_0(self.0, 0x30);
        } else if (message::BAD_TRICK..=message::GREAT_TRICK).contains(&id) {
            if id != message::TRICK_SKIPPED {
                self.mood_change(time, time);
                call_slot_0(self.0, 0x30);
            }
            match id {
                message::BAD_TRICK => self.guest_thought_plain(0x2972),
                message::GOOD_TRICK => self.guest_thought_plain(0x2974),
                message::GREAT_TRICK => self.guest_thought_plain(0x2975),
                _ => {}
            }
        }
        if let Some(name) = name {
            self.set_behaviour(name);
        }
    }
}

/// Overrides the live tests install on the detours and ported methods.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod overrides {
    use std::sync::Mutex;

    /// Answers `true` once it has handled a `fGuestThought(this, id, subject, habitat)` call.
    pub type Thought = fn(this: u32, id: u32, subject: u32, habitat: u32) -> bool;

    pub static THOUGHT: Mutex<Option<Thought>> = Mutex::new(None);
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(UPDATE_GOALS)]
    unsafe extern "thiscall" fn update_goals(this: *const u32) {
        ZTGuest(this as u32).update_goals();
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn listen(this: *const u32) {
        ZTGuest(this as u32).listen();
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, elapsed: u32) {
        ZTGuest(this as u32).update(elapsed);
    }

    #[detour(F_GUEST_THOUGHT)]
    unsafe extern "thiscall" fn f_guest_thought(this: *const u32, thought: u32, subject: *const i32, habitat: *const i32) {
        ZTGuest(this as u32).guest_thought(thought, subject as u32, habitat as u32);
    }

    #[detour(INIT_AFTER_LOAD)]
    unsafe extern "thiscall" fn init_after_load(this: *const u32, version: u32) -> bool {
        ZTGuest(this as u32).init_after_load(version)
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn update_goals(this: u32) {
            unsafe { super::UPDATE_GOALS_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn listen(this: u32) {
            unsafe { super::LISTEN_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn update(this: u32, elapsed: u32) {
            unsafe { super::UPDATE_DETOUR.call(this as *const u32, elapsed) }
        }
        pub(crate) fn f_guest_thought(this: u32, thought: u32, subject: u32, habitat: u32) {
            unsafe { super::F_GUEST_THOUGHT_DETOUR.call(this as *const u32, thought, subject as *const i32, habitat as *const i32) }
        }
        pub(crate) fn init_after_load(this: u32, version: u32) -> bool {
            unsafe { super::INIT_AFTER_LOAD_DETOUR.call(this as *const u32, version) }
        }
    }
}

/// Registers the goal and entry-point detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest goal detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub(crate) use super::detours::test_real as real;

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }
}
