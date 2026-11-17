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

use openzt_detour::generated::bfunit::{GET_MOVE_ANIM_SPEED, GET_MOVE_ANIM_SYMBOL};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::util::get_from_memory;

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
        if predicate(manager, AIMGR_VTABLE_IS_SLOW_MOVE, call) {
            slots[0]
        } else if predicate(manager, AIMGR_VTABLE_IS_MEDIUM_MOVE, call) {
            slots[1]
        } else if predicate(manager, AIMGR_VTABLE_IS_FAST_MOVE, call) {
            slots[2]
        } else {
            slots[0]
        }
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

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn get_move_anim_symbol(this: u32) -> u32 {
            unsafe { super::GET_MOVE_ANIM_SYMBOL_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_move_anim_speed(this: u32) -> u16 {
            unsafe { super::GET_MOVE_ANIM_SPEED_DETOUR.call(this as *const u32) }
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
