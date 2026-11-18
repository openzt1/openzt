//! `BFUnit::getMoveAnimSpeed` (`0x4a9e00`) and `getMoveAnimSymbol` (`0x4a9bb9`): the movement speed and
//! animation symbol of the unit's current movement, chosen by what the deepest active behavior set is calling.
//!
//! Both walk the child chain from the embedded `BFBehaviorSet` (`+0x14`) to the last set, ask `ZTAIMgr`
//! (through `GLOBAL_ZTAIMgr`'s vtable) whether that set's parsed call is a slow (`+0x28`), medium (`+0x2c`) or
//! fast (`+0x30`) move function, and call the matching entity vtable slot, the first match winning and no match
//! using the slow slot. The speed uses `+0x140`/`+0x144`/`+0x148` and returns the low 16 bits; the symbol uses
//! `+0x120`/`+0x124`/`+0x128` and returns the whole register.
//!
//! Vanilla first copies the call (`0x4a9917`, then `0x4a9a05`) into a local `BFFunctionCall`, passes that to
//! the predicates and destroys it afterwards. The predicates only read the call, so the port passes the set's
//! own call object and builds no copy; the only difference is the pool traffic of the copy. Vanilla's branch
//! for an embedded set that is null, which would use an empty call, cannot occur and is omitted.
//!
//! `stop` (`0x43e642`), `setMoving` (`0x43e5a5`) and the goal clear at `0x43e6c0` are the unit's movement-state
//! writers. The goal clear is `BFUnit::clearGoal` in the macOS binary (same sequence: `BFGoal::clear` on the
//! embedded cog goal, empty the target list, clear two flag bytes); `generated.rs` and Ghidra still call it
//! `getOnlyUnderwater`. Each empties the target list through `ztaimgr::TargetList`, which returns the nodes to
//! vanilla's pool exactly as vanilla's inlined freelist pushes do.

use openzt_detour::generated::bfunit::{GET_MOVE_ANIM_SPEED, GET_MOVE_ANIM_SYMBOL, GET_ONLY_UNDERWATER, IDLE, SET_MOVING, STOP};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    util::{get_from_memory, save_to_memory},
    ztaimgr::{TargetList, VanillaPool},
};

/// Unit flag bytes: `+0xd4` cleared by `stop(true, ..)`, `+0xd6` set by every `stop`, `+0x20c` moving, `+0x20d`
/// "finished at once" (suppresses a non-forced move start), `+0x20e` cog goal invalid, `+0x210` cleared with it.
const UNIT_CLEAR_ON_STOP: u32 = 0xd4;
const UNIT_STOP_REQUESTED: u32 = 0xd6;
const UNIT_MOVING: u32 = 0x20c;
const UNIT_FINISHED: u32 = 0x20d;
const UNIT_COG_INVALID: u32 = 0x20e;
const UNIT_FLAG_210: u32 = 0x210;
/// The embedded `BFCogGoal` and the `BFGoal::clear` (`0x0041332f`, `thiscall`, no arguments) vanilla runs on it.
const UNIT_COG_GOAL: u32 = 0x15c;
const BFGOAL_CLEAR: u32 = 0x0041_332f;

/// `GLOBAL_ZTAIMgr`, a pointer to the real manager.
const GLOBAL_ZTAIMGR: u32 = 0x0063_8098;
const EMBEDDED_SET: u32 = 0x14;
const SET_CHILD: u32 = 0x4c;
const SET_CALL: u32 = 0xc;
const AIMGR_VTABLE_IS_SLOW_MOVE: u32 = 0x28;
const AIMGR_VTABLE_IS_MEDIUM_MOVE: u32 = 0x2c;
const AIMGR_VTABLE_IS_FAST_MOVE: u32 = 0x30;
const ENTITY_VTABLE_SLOW_SPEED: u32 = 0x140;
const ENTITY_VTABLE_MEDIUM_SPEED: u32 = 0x144;
const ENTITY_VTABLE_FAST_SPEED: u32 = 0x148;

unsafe fn predicate(manager: u32, slot: u32, call: u32) -> bool {
    let vtable = get_from_memory::<u32>(manager);
    let function = get_from_memory::<u32>(vtable + slot);
    unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32, u32) -> u8>(function)(manager, call) != 0 }
}

/// Entity vtable slots `(slow, medium, fast)` for `getMoveAnimSymbol`.
const SYMBOL_SLOTS: [u32; 3] = [0x120, 0x124, 0x128];
/// Entity vtable slots `(slow, medium, fast)` for `getMoveAnimSpeed`.
const SPEED_SLOTS: [u32; 3] = [ENTITY_VTABLE_SLOW_SPEED, ENTITY_VTABLE_MEDIUM_SPEED, ENTITY_VTABLE_FAST_SPEED];

/// Picks the slot for a call's move class. The predicates are tried slow, medium, fast and stop at the first
/// match (the later ones are not evaluated); no match uses the slow slot.
fn select_move_slot(slots: [u32; 3], is_slow: bool, is_medium: impl FnOnce() -> bool, is_fast: impl FnOnce() -> bool) -> u32 {
    if is_slow {
        slots[0]
    } else if is_medium() {
        slots[1]
    } else if is_fast() {
        slots[2]
    } else {
        slots[0]
    }
}

/// Calls the entity vtable slot for the move class of the deepest behavior set's call and returns its `EAX`.
/// `slots` is `(slow, medium, fast)`; no match uses the slow slot.
unsafe fn dispatch_by_move_class(unit: u32, slots: [u32; 3]) -> u32 {
    let mut set = unit + EMBEDDED_SET;
    loop {
        let child = get_from_memory::<u32>(set + SET_CHILD);
        if child == 0 {
            break;
        }
        set = child;
    }
    let call = set + SET_CALL;
    let manager = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
    let slot = unsafe {
        select_move_slot(
            slots,
            predicate(manager, AIMGR_VTABLE_IS_SLOW_MOVE, call),
            || predicate(manager, AIMGR_VTABLE_IS_MEDIUM_MOVE, call),
            || predicate(manager, AIMGR_VTABLE_IS_FAST_MOVE, call),
        )
    };
    let function = get_from_memory::<u32>(get_from_memory::<u32>(unit) + slot);
    unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32) -> u32>(function)(unit) }
}

pub(crate) unsafe fn get_move_anim_speed(unit: u32) -> u16 {
    unsafe { dispatch_by_move_class(unit, SPEED_SLOTS) as u16 }
}

pub(crate) unsafe fn get_move_anim_symbol(unit: u32) -> u32 {
    unsafe { dispatch_by_move_class(unit, SYMBOL_SLOTS) }
}

/// The byte `setMoving` stores at `+0x20c`. A forced call stores `moving` as given; otherwise the unit moves
/// only when asked to and not flagged finished.
fn moving_value(moving: u8, force: u8, finished: u8) -> u8 {
    if force != 0 {
        moving
    } else if moving != 0 && finished == 0 {
        1
    } else {
        0
    }
}

/// `BFUnit::setMoving(moving, force)`. Vanilla also builds and destroys a throwaway `"Moving"`/`"Stopped Moving"`
/// string (a pool allocation and its free, which leaves the freelist as it was); the port skips it.
pub(crate) unsafe fn set_moving(unit: u32, moving: u8, force: u8) {
    let value = moving_value(moving, force, get_from_memory::<u8>(unit + UNIT_FINISHED));
    save_to_memory(unit + UNIT_MOVING, value);
}

/// `BFUnit::stop(clear, idle)`: clears `+0xd4` when `clear` is set, flags `+0xd6`, empties the target list,
/// runs `setMoving(false, false)` and, when `idle` is set, vanilla's `BFUnit::idle`.
pub(crate) unsafe fn stop(unit: u32, clear: u8, idle: u8) {
    if clear != 0 {
        save_to_memory(unit + UNIT_CLEAR_ON_STOP, 0u8);
    }
    save_to_memory(unit + UNIT_STOP_REQUESTED, 1u8);
    unsafe { TargetList::of_unit(unit).clear(&mut VanillaPool) };
    unsafe { set_moving(unit, 0, 0) };
    if idle != 0 {
        unsafe { IDLE.original()(unit as *const u32) };
    }
}

/// `BFUnit::clearGoal` (`0x43e6c0`): clears the cog goal, empties the target list and clears the invalid-goal
/// flags.
pub(crate) unsafe fn clear_goal(unit: u32) {
    unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32)>(BFGOAL_CLEAR)(unit + UNIT_COG_GOAL) };
    unsafe { TargetList::of_unit(unit).clear(&mut VanillaPool) };
    save_to_memory(unit + UNIT_COG_INVALID, 0u8);
    save_to_memory(unit + UNIT_FLAG_210, 0u8);
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_MOVE_ANIM_SPEED)]
    unsafe extern "thiscall" fn get_move_anim_speed_detour(this: *const u32) -> u16 {
        unsafe { get_move_anim_speed(this as u32) }
    }

    #[detour(GET_MOVE_ANIM_SYMBOL)]
    unsafe extern "thiscall" fn get_move_anim_symbol_detour(this: *const u32) -> u32 {
        unsafe { get_move_anim_symbol(this as u32) }
    }

    #[detour(SET_MOVING)]
    unsafe extern "thiscall" fn set_moving_detour(this: *const u32, moving: i8, force: i8) {
        unsafe { set_moving(this as u32, moving as u8, force as u8) }
    }

    #[detour(STOP)]
    unsafe extern "thiscall" fn stop_detour(this: *const u32, clear: i8, idle: i8) {
        unsafe { stop(this as u32, clear as u8, idle as u8) }
    }

    #[detour(GET_ONLY_UNDERWATER)]
    unsafe extern "thiscall" fn clear_goal_detour(this: *const u32) {
        unsafe { clear_goal(this as u32) }
    }

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn get_move_anim_symbol(this: u32) -> u32 {
            unsafe { super::GET_MOVE_ANIM_SYMBOL_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_move_anim_speed(this: u32) -> u16 {
            unsafe { super::GET_MOVE_ANIM_SPEED_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn stop(this: u32, clear: u8, idle: u8) {
            unsafe { super::STOP_DETOUR.call(this as *const u32, clear as i8, idle as i8) }
        }
        pub(crate) fn set_moving(this: u32, moving: u8, force: u8) {
            unsafe { super::SET_MOVING_DETOUR.call(this as *const u32, moving as i8, force as i8) }
        }
        pub(crate) fn clear_goal(this: u32) {
            unsafe { super::GET_ONLY_UNDERWATER_DETOUR.call(this as *const u32) }
        }
    }
}

/// Installs this module's detours. Called from both `lib.rs`'s boot cascade and
/// `reimplementation_tests::init()`.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise bfunit detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    pub(crate) use super::detours::test_real as real;

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        detours::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::cell::Cell;

    const SLOTS: [u32; 3] = [10, 20, 30];

    #[test]
    fn set_moving_stores_the_request_unless_finished_and_unforced() {
        // Not forced: moves only when asked and not finished; the stored byte is normalised to 0 or 1.
        assert_eq!(moving_value(1, 0, 0), 1);
        assert_eq!(moving_value(7, 0, 0), 1);
        assert_eq!(moving_value(1, 0, 1), 0);
        assert_eq!(moving_value(0, 0, 0), 0);
        // Forced: the request is stored as given, whatever finished says.
        assert_eq!(moving_value(1, 1, 1), 1);
        assert_eq!(moving_value(0, 1, 0), 0);
        assert_eq!(moving_value(7, 1, 0), 7);
    }

    #[test]
    fn slot_follows_the_first_matching_move_class() {
        assert_eq!(select_move_slot(SLOTS, true, || true, || true), 10);
        assert_eq!(select_move_slot(SLOTS, false, || true, || true), 20);
        assert_eq!(select_move_slot(SLOTS, false, || false, || true), 30);
        assert_eq!(select_move_slot(SLOTS, false, || false, || false), 10);
    }

    #[test]
    fn later_predicates_are_not_evaluated_after_a_match() {
        let medium_calls = Cell::new(0);
        let fast_calls = Cell::new(0);
        select_move_slot(SLOTS, true, || { medium_calls.set(1); false }, || { fast_calls.set(1); false });
        assert_eq!((medium_calls.get(), fast_calls.get()), (0, 0));
        select_move_slot(SLOTS, false, || { medium_calls.set(1); true }, || { fast_calls.set(1); false });
        assert_eq!((medium_calls.get(), fast_calls.get()), (1, 0));
    }
}
