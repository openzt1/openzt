//! `ZTUnit` show and trick methods (group C of `plans/ztunit-scoping-plan.md`): whether the unit performs in a show, the
//! show-script events it reacts to, and the trick items it can be asked to do.
//!
//! A unit records the trick it performs (`+0x252`, the trick type) and the show it performs in (`+0x254`, a `ZTShowInfo`
//! id). Show items come from the unit type's show script (`ZTUnitType::getShowItem`); an item's `+0xc` is its kind
//! (`0` behaviour, `1` toy, `3` food) and `+0x10` a `{begin, end}` string holding the behaviour-set name. Every
//! collaborator (`ZTShowMgr`, `ZTShowInfo`, `ZTShow`, `BFEntity::setBS`) stays vanilla and is called through its
//! `generated.rs` entry.

use openzt_detour::generated::{
    zthabitat::GET_GATE_TILE_IN,
    ztshow::ABORT_SHOW as ZTSHOW_ABORT_SHOW,
    ztshowinfo::HAS_KEEPER,
    ztshowmgr::{GET_SCRIPT_ID, GET_SHOW_INFO, IS_DOING_SHOW as MGR_IS_DOING_SHOW, IS_SHOW_SCRIPT_DONE as MGR_IS_SHOW_SCRIPT_DONE},
    ztunit::{
        ABORT_SHOW, ABORT_TRICK, DO_BEHAVIOR_TRICK, DO_FOOD_TRICK, DO_SHOW_ITEM, DO_TOY_TRICK, END_TRICK, GET_SHOW_ITEM, GET_SKIP_TRICK_CHANCE, GET_SKIP_TRICK_HAPPINESS, HANDLE_SHOW_EVENTS,
        HAS_SHOW_KEEPER, IS_DOING_SHOW, IS_SHOW_SCRIPT_DONE, IS_SHOW_UNIT, VALIDATE_TOY_TRICK, VALIDATE_TRICK_TYPE,
    },
    ztunittype::GET_SHOW_ITEM as ZTUNITTYPE_GET_SHOW_ITEM,
    ztworldmgr::GET_NEED_INDEX,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{base, call_slot_2_returning, goal_slots, type_slots, unit_offsets, ZTUnit, BUILDING_NEED_NAME_RVA};
use crate::{
    pool_vec::PoolVec,
    util::{get_from_memory, low_byte_bool, save_to_memory},
    vanilla_string::{c_string_bytes, object_bytes},
    ztkeeper::{call_slot_0, call_slot_1, vtable_slot},
};

/// `GLOBAL_ZTShowMgr` (`0x00639008`), as an RVA.
const SHOW_MGR_RVA: u32 = 0x0023_9008;

/// `BFEntity` slot `getEvents(out vector<BFEvent>)`.
const SLOT_GET_EVENTS: u32 = 0x8;
/// `BFEvent` records: `0x1c` bytes, the target entity id at `+0xc` and the event kind (`u16`) at `+0x16`.
const EVENT_SIZE: u32 = 0x1c;
const EVENT_ENTITY: u32 = 0xc;
const EVENT_KIND: u32 = 0x16;
/// Event kinds the unit reacts to.
const EVENT_SHOW_STARTED: u32 = 0x271a;
const EVENT_SHOW_ITEM_DONE: u32 = 0x271b;
const EVENT_TRICK_ENDED: u32 = 0x271e;
const EVENT_TRICK_ABORTED: u32 = 0x2722;
const EVENT_SHOW_STOPPED: u32 = 0x2723;
const EVENT_SHOW_FINISHED: u32 = 0x2725;
/// The `BFEvent` kinds `endTrick`/`abortTrick` send the unit itself (`sendEvent(kind, id, 0x55, 0, 0, 1)`).
const SEND_EVENT_TAG: u32 = 0x55;
/// `ZTShowScriptState` bytes the events set.
const STATE_ITEM_DONE: u32 = 0xe;
const STATE_ABORTED: u32 = 0xf;
const STATE_FINISHED: u32 = 0x11;

/// `ZTUnitType` fields: the list of show items (a `std::list` sentinel pointer), and the happiness and chance a skipped
/// trick costs.
const TYPE_SHOW_LIST: u32 = 0x1ac;
const TYPE_SKIP_HAPPINESS: u32 = 0x180;
const TYPE_SKIP_CHANCE: u32 = 0x184;
/// `ZTShowItem` fields.
const ITEM_KIND: u32 = 0xc;
const ITEM_NAME: u32 = 0x10;
const ITEM_TOY_ID: u32 = 0x40;
const ITEM_MIN_LEVEL: u32 = 0x58;
const KIND_BEHAVIOR: u32 = 0;
const KIND_TOY: u32 = 1;
const KIND_FOOD: u32 = 3;
/// `ZTHabitat` fields: the owner whose `+0x70` is the show id, the "world habitat" byte and the level compared with an
/// item's minimum.
const HABITAT_OWNER: u32 = 0x4;
const OWNER_SHOW_ID: u32 = 0x70;
const HABITAT_WORLD_FLAG: u32 = 0x2c;
const HABITAT_LEVEL: u32 = 0x188;
/// `ZTShowInfo` field: the habitat the show is held in.
const SHOW_INFO_HABITAT: u32 = 0xa0;
/// The farthest a toy may be from the unit (squared tile distance, `DAT_0xb504`).
const TOY_RANGE: i32 = 0xb504;

/// Answers of the trick routines.
const TRICK_OK: u32 = 0;
const TRICK_NOT_POSSIBLE: u32 = 3;
const TRICK_NOT_READY: u32 = 8;
const TRICK_NO_KEEPER: u32 = 9;

/// Vtable slots the show methods dispatch through, so a subclass override is honoured.
mod slots {
    pub const SEND_EVENT: u32 = 0x0;
    pub const DO_TOY_TRICK: u32 = 0x204;
    pub const DO_FOOD_TRICK: u32 = 0x208;
    pub const DO_BEHAVIOR_TRICK: u32 = 0x20c;
    pub const VALIDATE_FOOD_TRICK: u32 = 0x220;
    pub const VALIDATE_TOY_TRICK: u32 = 0x21c;
    pub const VALIDATE_BEHAVIOR_TRICK: u32 = 0x224;
    pub const IS_SHOW_UNIT: u32 = 0x228;
    pub const CHECK_DO_TRICK: u32 = 0x234;
    /// `ZTHabitat::isTank`, on a habitat.
    pub const HABITAT_IS_TANK: u32 = super::type_slots::HABITAT_IS_TANK;
}

fn show_mgr() -> u32 {
    get_from_memory(base() + SHOW_MGR_RVA)
}

/// The `ZTShowInfo` of show `id`, or `0` (also when there is no show manager).
fn show_info(id: u16) -> u32 {
    let manager = show_mgr();
    if manager == 0 {
        return 0;
    }
    unsafe { GET_SHOW_INFO.hooked()(manager as *const u32, id) as u32 }
}

fn slot_3(this: u32, offset: u32, first: u32, second: u32, third: u32) -> u32 {
    let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32) -> u32>(vtable_slot(this, offset)) };
    function(this, first, second, third)
}

/// A building's position inside its tile: x and y modulo `64`, z modulo `16` (a negative z is brought into `1..=16`).
fn position_in_tile(building: u32) -> [i32; 3] {
    let (x, y, z): (i32, i32, i32) = (get_from_memory(building + unit_offsets::POS_X), get_from_memory(building + unit_offsets::POS_Y), get_from_memory(building + unit_offsets::POS_Z));
    crate::util::position_in_tile(x, y, z)
}

impl ZTUnit {
    /// `ZTUnit::isShowUnit` (`0x00410f40`): the unit's type has at least one show item.
    pub fn is_show_unit(self) -> bool {
        let unit_type = self.unit_type();
        if unit_type == 0 {
            return false;
        }
        let sentinel: u32 = get_from_memory(unit_type + TYPE_SHOW_LIST);
        sentinel != 0 && get_from_memory::<u32>(sentinel) != sentinel
    }

    /// `ZTUnit::abortShow` (`0x0042fa42`): aborts the show the unit performs in.
    pub fn abort_show(self) {
        let info = show_info(self.read(unit_offsets::SHOW_INFO_ID));
        if info != 0 {
            unsafe { ZTSHOW_ABORT_SHOW.hooked()((info + 4) as *const u32) };
        }
    }

    /// `ZTUnit::isDoingShow` (`0x00437402`): the unit is a show unit and is flagged as performing, or its show is running.
    pub fn is_doing_show(self) -> bool {
        if !low_byte_bool(call_slot_0(self.0, slots::IS_SHOW_UNIT)) {
            return false;
        }
        if self.read::<u8>(unit_offsets::SHOW_FLAG_B) != 0 {
            return true;
        }
        let show: u16 = self.read(unit_offsets::SHOW_INFO_ID);
        let manager = show_mgr();
        show != 0 && manager != 0 && unsafe { MGR_IS_DOING_SHOW.hooked()(manager as *const u32, self.read(unit_offsets::ENTITY_ID), show) }
    }

    /// `ZTUnit::isShowScriptDone` (`0x0059fa79`): the unit's show script has run to its end.
    pub fn is_show_script_done(self) -> bool {
        if !low_byte_bool(call_slot_0(self.0, slots::IS_SHOW_UNIT)) {
            return false;
        }
        let show: u16 = self.read(unit_offsets::SHOW_INFO_ID);
        let manager = show_mgr();
        show != 0 && manager != 0 && low_byte_bool(u32::from(unsafe { MGR_IS_SHOW_SCRIPT_DONE.hooked()(manager as *const u32, self.read(unit_offsets::ENTITY_ID), show) }))
    }

    /// Sends the unit itself a `BFEvent` of `kind` through its virtual `sendEvent` (`sendEvent(kind, id, 0x55, 0, 0, 1)`).
    fn send_own_event(self, kind: u32) {
        type SendEvent = extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u32) -> u32;
        let function = unsafe { std::mem::transmute::<u32, SendEvent>(vtable_slot(self.0, slots::SEND_EVENT)) };
        function(self.0, kind, self.read(unit_offsets::ENTITY_ID), SEND_EVENT_TAG, 0, 0, 1);
    }

    /// `ZTUnit::endTrick` (`0x005a69a1`).
    pub fn end_trick(self) {
        self.send_own_event(EVENT_TRICK_ENDED);
    }

    /// `ZTUnit::abortTrick` (`0x0050c2ae`).
    pub fn abort_trick(self) {
        self.send_own_event(EVENT_TRICK_ABORTED);
    }

    /// `ZTUnit::hasShowKeeper` (`0x005a6f50`): the show of the unit's habitat has a keeper; a unit with no habitat, or
    /// whose show is unknown, counts as having one.
    pub fn has_show_keeper(self) -> bool {
        let habitat = call_slot_0(self.0, goal_slots::GET_HABITAT);
        if habitat == 0 {
            return true;
        }
        let owner: u32 = get_from_memory(habitat + HABITAT_OWNER);
        let show: u16 = if owner == 0 { 0 } else { get_from_memory(owner + OWNER_SHOW_ID) };
        let info = show_info(show);
        info == 0 || unsafe { HAS_KEEPER.hooked()(info as *const u32) }
    }

    /// `ZTUnit::getSkipTrickHappiness` (`0x005a7096`): the unit type's value, `0` for a unit without one (vanilla reads
    /// through null).
    pub fn get_skip_trick_happiness(self) -> i32 {
        let unit_type = self.unit_type();
        if unit_type == 0 {
            0
        } else {
            get_from_memory(unit_type + TYPE_SKIP_HAPPINESS)
        }
    }

    /// `ZTUnit::getSkipTrickChance` (`0x005a70f4`).
    pub fn get_skip_trick_chance(self) -> i32 {
        let unit_type = self.unit_type();
        if unit_type == 0 {
            0
        } else {
            get_from_memory(unit_type + TYPE_SKIP_CHANCE)
        }
    }

    /// `ZTUnit::getShowItem` (`0x005a2682`): the show item of trick `trick` in the script of show `show`, or `0`.
    pub fn get_show_item(self, trick: u16, show: u16) -> u32 {
        let manager = show_mgr();
        if manager == 0 {
            return 0;
        }
        let script = unsafe { GET_SCRIPT_ID.hooked()(manager as *const u32, show) };
        unsafe { ZTUNITTYPE_GET_SHOW_ITEM.hooked()(trick, u32::from(script)) }
    }

    /// `ZTUnit::handleShowEvents` (`0x005a2415`): applies the unit's pending show events to the script `state` and answers
    /// `0` (`-1` without a state). Vanilla's event vector is returned to its own allocator.
    pub fn handle_show_events(self, state: u32) -> u32 {
        if state == 0 {
            return u32::MAX;
        }
        let mut events = [0u32; 3];
        call_slot_1(self.0, SLOT_GET_EVENTS, events.as_mut_ptr() as u32);
        let id: u32 = self.read(unit_offsets::ENTITY_ID);
        let mut record = events[0];
        while record != 0 && record < events[1] {
            if get_from_memory::<u32>(record + EVENT_ENTITY) == id {
                match u32::from(get_from_memory::<u16>(record + EVENT_KIND)) {
                    EVENT_SHOW_STARTED => {
                        self.write(unit_offsets::SHOW_FLAG_B, 0u8);
                        self.write(unit_offsets::SHOW_FLAG_A, 1u8);
                    }
                    EVENT_SHOW_ITEM_DONE => {
                        call_slot_0(self.0, goal_slots::CLEAR_GOAL);
                        save_to_memory(state + STATE_ITEM_DONE, 1u8);
                        save_to_memory(state + STATE_FINISHED, 1u8);
                    }
                    EVENT_TRICK_ENDED => {
                        save_to_memory(state + STATE_ITEM_DONE, 1u8);
                        call_slot_0(self.0, goal_slots::CLEAR_GOAL);
                    }
                    EVENT_TRICK_ABORTED => {
                        save_to_memory(state + STATE_ITEM_DONE, 1u8);
                        save_to_memory(state + STATE_ABORTED, 1u8);
                    }
                    EVENT_SHOW_STOPPED => {
                        call_slot_0(self.0, goal_slots::CLEAR_GOAL);
                    }
                    EVENT_SHOW_FINISHED => {
                        save_to_memory(state + STATE_FINISHED, 1u8);
                    }
                    _ => {}
                }
            }
            record += EVENT_SIZE;
        }
        drop(PoolVec::adopt_sized(events, EVENT_SIZE));
        0
    }

    /// `ZTUnit::doShowItem` (`0x005a66c0`): performs show item `trick` of show `show` through the virtual routine for the
    /// item's kind. Anything but a food item needs the unit to be ready (`checkDoTrick`), else `8`. Vanilla dereferences a
    /// missing item; the port answers "not possible".
    pub fn do_show_item(self, trick: u16, show: u16) -> u32 {
        let item = self.get_show_item(trick, show);
        if item == 0 {
            return TRICK_NOT_POSSIBLE;
        }
        let kind: u32 = get_from_memory(item + ITEM_KIND);
        if kind != KIND_FOOD && !low_byte_bool(call_slot_0(self.0, slots::CHECK_DO_TRICK)) {
            return TRICK_NOT_READY;
        }
        match kind {
            KIND_BEHAVIOR => call_slot_2_returning(self.0, slots::DO_BEHAVIOR_TRICK, u32::from(trick), u32::from(show)),
            KIND_TOY => slot_3(self.0, slots::DO_TOY_TRICK, u32::from(trick), get_from_memory(item + ITEM_TOY_ID), u32::from(show)),
            KIND_FOOD => call_slot_2_returning(self.0, slots::DO_FOOD_TRICK, u32::from(trick), u32::from(show)),
            _ => TRICK_NOT_POSSIBLE,
        }
    }

    /// `ZTUnit::doBehaviorTrick` (`0x005a6b10`): queues the behaviour set `prefix + item name` and records the trick and
    /// show; `3` when there is no such item.
    pub fn do_behavior_trick(self, trick: u16, show: u16) -> u32 {
        let item = self.get_show_item(trick, show);
        if item == 0 {
            return TRICK_NOT_POSSIBLE;
        }
        let mut name = c_string_bytes(base() + super::buildings::USE_NAME_PREFIX_RVA);
        name.extend(object_bytes(item + ITEM_NAME));
        self.queue_behaviour_set(&name);
        self.write(unit_offsets::TRICK_TYPE, trick);
        self.write(unit_offsets::SHOW_INFO_ID, show);
        TRICK_OK
    }

    /// `ZTUnit::doFoodTrick` (`0x00612ed3`): units do not take food tricks.
    pub fn do_food_trick(self) -> u32 {
        TRICK_NOT_POSSIBLE
    }

    /// `ZTUnit::validateTrickType` (`0x005a6f96`): whether trick `trick` of show `show` can be done now. `9` without a show
    /// keeper, `3` without an item; otherwise, with the unit's habitat a tank whose level reaches the item's minimum, the
    /// answer of the virtual validator for the item's kind. The unit is marked as in a show for the duration.
    pub fn validate_trick_type(self, trick: u16, show: u16) -> u32 {
        if !self.has_show_keeper() {
            return TRICK_NO_KEEPER;
        }
        let item = self.get_show_item(trick, show);
        if item == 0 {
            return TRICK_NOT_POSSIBLE;
        }
        let previous_show: u16 = self.read(unit_offsets::SHOW_INFO_ID);
        self.write(unit_offsets::SHOW_INFO_ID, show);
        self.write(unit_offsets::IN_SHOW, 1u8);
        let mut answer = TRICK_NOT_POSSIBLE;
        let habitat = call_slot_0(self.0, goal_slots::GET_HABITAT);
        if habitat != 0 && low_byte_bool(call_slot_0(habitat, slots::HABITAT_IS_TANK)) && get_from_memory::<i32>(item + ITEM_MIN_LEVEL) <= get_from_memory::<i32>(habitat + HABITAT_LEVEL) {
            answer = match get_from_memory::<u32>(item + ITEM_KIND) {
                KIND_BEHAVIOR => call_slot_2_returning(self.0, slots::VALIDATE_BEHAVIOR_TRICK, u32::from(trick), u32::from(show)),
                KIND_TOY => slot_3(self.0, slots::VALIDATE_TOY_TRICK, u32::from(trick), get_from_memory(item + ITEM_TOY_ID), u32::from(show)),
                KIND_FOOD => call_slot_2_returning(self.0, slots::VALIDATE_FOOD_TRICK, u32::from(trick), u32::from(show)),
                _ => answer,
            };
        }
        self.write(unit_offsets::IN_SHOW, 0u8);
        self.write(unit_offsets::SHOW_INFO_ID, previous_show);
        answer
    }

    /// The tile `validateToyTrick`/`doToyTrick` search from: the gate of the show's habitat when it is not a world
    /// habitat, else the unit's own tile.
    fn toy_search_tile(self, show: u16) -> u32 {
        let info = show_info(show);
        if info != 0 {
            let habitat: u32 = get_from_memory(info + SHOW_INFO_HABITAT);
            if habitat != 0 && get_from_memory::<u8>(habitat + HABITAT_WORLD_FLAG) == 0 {
                let gate = unsafe { GET_GATE_TILE_IN.hooked()(habitat as *const u32) } as u32;
                if gate != 0 {
                    return gate;
                }
            }
        }
        self.tile()
    }

    /// `ZTUnit::validateToyTrick` (`0x005a7168`): `0` when a toy of type `toy` is within reach of the show's habitat, else `3`.
    pub fn validate_toy_trick(self, toy: u32, show: u16) -> u32 {
        let tile = self.toy_search_tile(show);
        let mut building: u32 = 0;
        if self.find_any_satisfier(tile, base() + BUILDING_NEED_NAME_RVA, &mut building as *mut u32 as u32, TOY_RANGE, toy as i32) {
            TRICK_OK
        } else {
            TRICK_NOT_POSSIBLE
        }
    }

    /// `ZTUnit::doToyTrick` (`0x005a7202`): finds a toy of type `toy` near the unit, heads for its tile and records the
    /// trick, the show and the toy's position inside its tile; `3` when none is in reach.
    pub fn do_toy_trick(self, trick: u16, toy: u32, show: u16) -> u32 {
        let mut building: u32 = 0;
        if !self.find_any_satisfier(self.tile(), base() + BUILDING_NEED_NAME_RVA, &mut building as *mut u32 as u32, TOY_RANGE, toy as i32) {
            return TRICK_NOT_POSSIBLE;
        }
        let need = unsafe { GET_NEED_INDEX.hooked()((base() + BUILDING_NEED_NAME_RVA) as *const i8) } as u8;
        self.write(unit_offsets::NEED_INDEX, need);
        let toy_tile = unsafe { super::BFENTITY_GET_TILE.hooked()(building as *const u32) } as u32;
        call_slot_1(self.0, goal_slots::SET_BUILDING_TILE, toy_tile);
        let [x, y, z] = position_in_tile(building);
        self.write(unit_offsets::BUILDING_OFFSET, x);
        self.write(unit_offsets::SHOW_INFO_ID, show);
        self.write(unit_offsets::BUILDING_OFFSET + 4, y);
        self.write(unit_offsets::BUILDING_OFFSET + 8, z);
        self.write(unit_offsets::TRICK_TYPE, trick);
        TRICK_OK
    }
}

#[detour_mod]
mod show_detours {
    use super::*;

    #[detour(IS_SHOW_UNIT)]
    unsafe extern "thiscall" fn is_show_unit(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).is_show_unit())
    }

    #[detour(ABORT_SHOW)]
    unsafe extern "thiscall" fn abort_show(this: *const u32) {
        ZTUnit(this as u32).abort_show();
    }

    #[detour(IS_DOING_SHOW)]
    unsafe extern "thiscall" fn is_doing_show(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).is_doing_show())
    }

    #[detour(ABORT_TRICK)]
    unsafe extern "thiscall" fn abort_trick(this: *const u32) {
        ZTUnit(this as u32).abort_trick();
    }

    #[detour(IS_SHOW_SCRIPT_DONE)]
    unsafe extern "thiscall" fn is_show_script_done(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).is_show_script_done())
    }

    #[detour(HANDLE_SHOW_EVENTS)]
    unsafe extern "thiscall" fn handle_show_events(this: *const u32, state: *const u32, _show: u16) -> u32 {
        ZTUnit(this as u32).handle_show_events(state as u32)
    }

    #[detour(GET_SHOW_ITEM)]
    unsafe extern "thiscall" fn get_show_item(this: *const u32, trick: u16, show: u16) -> *const std::ffi::c_void {
        ZTUnit(this as u32).get_show_item(trick, show) as *const std::ffi::c_void
    }

    #[detour(DO_SHOW_ITEM)]
    unsafe extern "thiscall" fn do_show_item(this: *const u32, trick: u16, show: u16) -> u32 {
        ZTUnit(this as u32).do_show_item(trick, show)
    }

    #[detour(END_TRICK)]
    unsafe extern "thiscall" fn end_trick(this: *const u32) {
        ZTUnit(this as u32).end_trick();
    }

    #[detour(DO_BEHAVIOR_TRICK)]
    unsafe extern "thiscall" fn do_behavior_trick(this: *const u32, trick: u16, show: u16) -> u32 {
        ZTUnit(this as u32).do_behavior_trick(trick, show)
    }

    #[detour(HAS_SHOW_KEEPER)]
    unsafe extern "thiscall" fn has_show_keeper(this: *const u32) -> u32 {
        u32::from(ZTUnit(this as u32).has_show_keeper())
    }

    #[detour(VALIDATE_TRICK_TYPE)]
    unsafe extern "thiscall" fn validate_trick_type(this: *const u32, trick: u16, show: u16) -> u32 {
        ZTUnit(this as u32).validate_trick_type(trick, show)
    }

    #[detour(GET_SKIP_TRICK_HAPPINESS)]
    unsafe extern "thiscall" fn get_skip_trick_happiness(this: *const u32) -> i32 {
        ZTUnit(this as u32).get_skip_trick_happiness()
    }

    #[detour(GET_SKIP_TRICK_CHANCE)]
    unsafe extern "thiscall" fn get_skip_trick_chance(this: *const u32) -> i32 {
        ZTUnit(this as u32).get_skip_trick_chance()
    }

    #[detour(VALIDATE_TOY_TRICK)]
    unsafe extern "thiscall" fn validate_toy_trick(this: *const u32, _trick: u16, toy: u32, show: u16) -> u32 {
        ZTUnit(this as u32).validate_toy_trick(toy, show)
    }

    #[detour(DO_TOY_TRICK)]
    unsafe extern "thiscall" fn do_toy_trick(this: *const u32, trick: u16, toy: u32, show: u16) -> u32 {
        ZTUnit(this as u32).do_toy_trick(trick, toy, show)
    }

    #[detour(DO_FOOD_TRICK)]
    unsafe extern "thiscall" fn do_food_trick(this: *const u32, _trick: u16, _show: u16) -> u32 {
        ZTUnit(this as u32).do_food_trick()
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn is_show_unit(this: u32) -> u32 {
            unsafe { super::IS_SHOW_UNIT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn abort_show(this: u32) {
            unsafe { super::ABORT_SHOW_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_doing_show(this: u32) -> u32 {
            unsafe { super::IS_DOING_SHOW_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn abort_trick(this: u32) {
            unsafe { super::ABORT_TRICK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn is_show_script_done(this: u32) -> u32 {
            unsafe { super::IS_SHOW_SCRIPT_DONE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn handle_show_events(this: u32, state: u32) -> u32 {
            unsafe { super::HANDLE_SHOW_EVENTS_DETOUR.call(this as *const u32, state as *const u32, 0) }
        }

        pub(crate) fn get_show_item(this: u32, trick: u16, show: u16) -> u32 {
            unsafe { super::GET_SHOW_ITEM_DETOUR.call(this as *const u32, trick, show) as u32 }
        }

        pub(crate) fn do_show_item(this: u32, trick: u16, show: u16) -> u32 {
            unsafe { super::DO_SHOW_ITEM_DETOUR.call(this as *const u32, trick, show) }
        }

        pub(crate) fn end_trick(this: u32) {
            unsafe { super::END_TRICK_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn do_behavior_trick(this: u32, trick: u16, show: u16) -> u32 {
            unsafe { super::DO_BEHAVIOR_TRICK_DETOUR.call(this as *const u32, trick, show) }
        }

        pub(crate) fn has_show_keeper(this: u32) -> u32 {
            unsafe { super::HAS_SHOW_KEEPER_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn validate_trick_type(this: u32, trick: u16, show: u16) -> u32 {
            unsafe { super::VALIDATE_TRICK_TYPE_DETOUR.call(this as *const u32, trick, show) }
        }

        pub(crate) fn get_skip_trick_happiness(this: u32) -> i32 {
            unsafe { super::GET_SKIP_TRICK_HAPPINESS_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn get_skip_trick_chance(this: u32) -> i32 {
            unsafe { super::GET_SKIP_TRICK_CHANCE_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn validate_toy_trick(this: u32, trick: u16, toy: u32, show: u16) -> u32 {
            unsafe { super::VALIDATE_TOY_TRICK_DETOUR.call(this as *const u32, trick, toy, show) }
        }

        pub(crate) fn do_toy_trick(this: u32, trick: u16, toy: u32, show: u16) -> u32 {
            unsafe { super::DO_TOY_TRICK_DETOUR.call(this as *const u32, trick, toy, show) }
        }

        pub(crate) fn do_food_trick(this: u32, trick: u16, show: u16) -> u32 {
            unsafe { super::DO_FOOD_TRICK_DETOUR.call(this as *const u32, trick, show) }
        }
    }
}

/// Installs the show and trick detours.
pub(super) fn init() {
    if let Err(e) = unsafe { show_detours::init_detours() } {
        error!("Failed to initialise ztunit show detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    show_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use show_detours::test_real;
