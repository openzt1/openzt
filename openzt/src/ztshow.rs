//! Stage 2 (`ZTShow`/`ZTShowInfo` raw-access call sites) of the vanilla `ZTShowScriptMgr` reimplementation
//! - see `openzt/plans/ztshowscriptmgr-implementation-plan.md`. Stage 1 (`ztshowscriptmgr.rs`) made
//! `ZTShowScriptMgr`/`ZTShowScript`/`ZTShowScriptItem` an independent Rust store; this module ports the
//! real vanilla functions that used to dereference those classes' raw memory directly (and would
//! otherwise crash against Stage 1's synthetic handles/sentinels) onto that store's id-keyed accessors.
//!
//! `ZTShow`/`ZTShowInfo` themselves stay real, un-virtualized vanilla memory - only the
//! embedded show-script data moved into Rust. (`ZTShowMgr` has since started its own reimplementation -
//! see `ztshowmgr.rs`; this module reads only its threshold fields off the live global.) Field offsets
//! below are confirmed directly from
//! `.asm`-level reads (see each function's doc comment), not the (less reliable) decompiled `.c` alone,
//! except where noted as semantics-unconfirmed-but-byte-faithful.
//!
//! Implemented so far: `ZTShowInfo::checkUnitType`, `ZTShow::doTrickEvent`, `ZTShow::doCurrentItem`,
//! `ZTShow::validateItem`, `ZTShow::start` (plus `ZTShow::stop`'s 1-arg overload, an independently-broken
//! consumer found while porting `start` - see that function's own doc comment),
//! `ZTShowInfo::checkPendingScripts`. Only `ZTShowInfo::addScript` remains - see the plan doc's "Open
//! items" for its current scoping blocker (a real tree-insert-with-rebalancing path, not just field reads).

use openzt_detour::generated::{
    bfworldmgr::{GET_TYPE, GET_UNIT},
    ztgamemgr::GET_DATE,
    ztshow::{
        CALCULATE_PERCENT_ADJUSTMENT, CHECK_SCRIPT, CLEAR_SHOW_SCRIPT_STATES, DO_CURRENT_ITEM, DO_KEEPER_EVENT, DO_TRICK_EVENT,
        GATHER_UNITS, GET_SHOW_SCRIPT_STATE, REINIT, RESOLVE_NEXT_SCHEDULED_SCRIPT_ID, START, STOP_0, VALIDATE, VALIDATE_ITEM,
    },
    ztshowinfo::{
        ADD_SCRIPT, ADD_SHOW, CHECK_PENDING_SCRIPTS, CHECK_UNIT, CHECK_UNIT_TYPE, GET_NUM_UNITS, GET_SHOW_UNIT_LIST, IS_STARTED,
        RECALCULATE_SCHEDULE, REMOVE_SHOW, SEND_EVENT,
    },
    standalone::OPERATOR_NEW,
    ztshowmgr::GET_SHOW_INFO,
    zthabitat::PLAY_SHOW_START_SOUND,
};
use openzt_detour_macro::detour_mod;
use tracing::error;
use windows::Win32::Foundation::FILETIME;

use crate::{
    globals::globals,
    util::{get_from_memory, save_to_memory},
    ztmegatilemgr::entity_type_matches,
    ztshowinfo::remove_unit,
};

/// `DAT_006386b0`'s RVA - the same vtable-slot-`0x1c` "isKindOf"-style type-check argument used by
/// `ztmegatilemgr::entity_type_matches`'s own callers, here gating `doCurrentItem`/`validateItem`'s
/// trick-eligible-unit check. RVA = `0x006386b0 - 0x400000`.
pub(crate) const RVA_SHOW_TRICK_TYPE_CHECK: u32 = 0x0023_86b0;

/// Raw virtual dispatch through a `BFEntity`-ish object's own vtable at `slot_offset`, taking two `u16`
/// args and returning `i32` - the shape both `ZTUnit`'s `+0x210` (`doCurrentItem`) and `+0x218`
/// (`validateItem`) slots share. No named symbol exists for either slot; semantics unconfirmed beyond
/// "trick availability/eligibility check", raw calling convention confirmed via `.asm` push-order reads.
unsafe fn call_unit_vtable_u16_u16(unit_ptr: u32, slot_offset: u32, arg1: u16, arg2: u16) -> i32 {
    let vtable = get_from_memory::<u32>(unit_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u16, u16) -> i32>(target) };
    f(unit_ptr, arg1, arg2)
}

/// `ZTShowInfo::sendEvent`'s vtable-slot-0 target (`ztshowinfo::SEND_EVENT`, confirmed against
/// `private/docs/vtables/ZTShowInfo.md`'s slot `+0x0` = `0x0059f013`, exactly `SEND_EVENT`'s own address).
/// `doTrickEvent` dispatches through `ZTShow`'s `+0x10` `ZTShowInfo*` back-pointer's vtable slot 0 rather
/// than calling `SEND_EVENT` by name, but they're the same function, so this calls it directly.
///
/// `.hooked()`, not `.original()`, since Stage 6 (`ztshowinfo.rs`) now detours this address - see
/// `CLAUDE.md`'s "every stage that ports a call-through-only method must also switch its own
/// `.original()` call sites to `.hooked()`" rule.
unsafe fn send_event(show_info: u32, event_id: u16, unused: u32, category: u8, value: u32, value2: u16, flag: u16) {
    unsafe { SEND_EVENT.hooked()(show_info as *const u32, event_id, unused, category, value, value2, flag) };
}

/// Reimplementation of `ZTShowInfo::checkUnitType`, per `ZTShowInfo_checkUnitType.c`/`.asm`. `this` is
/// `ZTShowInfo*`; `+0x8` is its own assigned script id (u16).
pub fn check_unit_type(this: u32, unit_type: u32) -> u32 {
    let script_id = get_from_memory::<u16>(this + 0x8);
    match crate::ztshowscriptmgr::script_type_by_id(script_id) {
        Some(script_type) if script_type != unit_type => unit_type & 0xffff_ff00,
        Some(_) => (unit_type & 0xffff_ff00) | 1,
        None => 1,
    }
}

/// Pure `_Tree::find` descent over `ZTShow::getShowScriptState`'s script-state map, per
/// `AI_cls_0x404fd6_find.asm`: standard MSVC lower-bound-then-equality-check shape, node layout
/// `+0x8` left, `+0xc` right, `+0x10` key (a full **32-bit** compare, confirmed via `.asm` - not the
/// 16-bit width a unit id might suggest), `+0x14` value. `header` is the map's own header/nil node
/// (self-referential when empty, matching every other `_Tree` header this codebase has already
/// ported); this returns the header itself on a miss, exactly like vanilla's own descent, leaving the
/// final equality check to the caller.
fn find_script_state_node(header: u32, key: u32) -> u32 {
    let mut candidate = header;
    let mut node = get_from_memory::<u32>(header + 0x4);
    while node != 0 {
        if get_from_memory::<u32>(node + 0x10) < key {
            node = get_from_memory::<u32>(node + 0xc);
        } else {
            candidate = node;
            node = get_from_memory::<u32>(node + 0x8);
        }
    }
    candidate
}

/// Reimplementation of `ZTShow::getShowScriptState` (`ztshow::GET_SHOW_SCRIPT_STATE`, `0x0059eb99`,
/// now detoured - see the module's own detour list): a plain, read-only lookup in the
/// `std::map<u32, ZTShowScriptState*>` whose header lives at `ztshow+0x34` (`show_info+0x38`, per
/// `ztshowmgr.rs`'s `is_doing_show`/`is_show_script_done` callers). Reads vanilla's still-vanilla-owned,
/// vanilla-written, vanilla-freed tree directly in place - no ownership claim, no allocator interaction,
/// same "narrow vanilla-memory carve-out" this file's other tree readers already rely on. Its own logic
/// was already ported/unit-tested internally (any caller reaching this address already ran through this
/// exact function via a direct Rust call) - detouring the real address just extends the same behavior to
/// real, un-decompiled-by-us vanilla callers that invoke it through the vtable/address directly instead.
pub fn get_show_script_state(ztshow: u32, key: u32) -> u32 {
    let header = get_from_memory::<u32>(ztshow + 0x34);
    let candidate = find_script_state_node(header, key);
    if candidate != header && get_from_memory::<u32>(candidate + 0x10) <= key {
        get_from_memory::<u32>(candidate + 0x14)
    } else {
        0
    }
}

/// Reimplementation of `ZTShow::doCurrentItem`, per `ZTShow_doCurrentItem.c`/`.asm`. `this` is `ZTShow*`;
/// `+0x4` its assigned script id (u16), `+0x6` a secondary u16 field passed through to the unit's own
/// trick-dispatch call (semantics unconfirmed - same raw field `ZTShow::start` also propagates onto units,
/// per that function's own decompile).
pub fn do_current_item(this: u32, unit_id: u32) -> i32 {
    let state = get_show_script_state(this, unit_id);
    if state == 0 {
        return 5;
    }
    if get_from_memory::<u8>(state + 0xe) != 0 {
        return 0;
    }
    if get_from_memory::<u8>(state + 0x12) != 0 {
        return 0;
    }

    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_ptr = unsafe { GET_UNIT.original()(world, unit_id as i32) };
    if unit_ptr == 0 {
        return -1;
    }
    if !unsafe { entity_type_matches(unit_ptr, RVA_SHOW_TRICK_TYPE_CHECK) } {
        return -1;
    }

    let script_id = get_from_memory::<u16>(this + 0x4);
    let item_count = crate::ztshowscriptmgr::script_item_count_by_id(script_id);
    let trick_index = get_from_memory::<u16>(state + 0xc);
    if trick_index as usize >= item_count {
        return -1;
    }
    if trick_index == 0xffff {
        return 0;
    }
    let Some(item) = crate::ztshowscriptmgr::item_snapshot_by_id(script_id, trick_index) else {
        return -1;
    };

    let secondary = get_from_memory::<u16>(this + 0x6);
    let result = unsafe { call_unit_vtable_u16_u16(unit_ptr, 0x210, item.id, secondary) };
    if result == 0 {
        save_to_memory(state + 0x12, 1u8);
    }
    result
}

/// Reimplementation of `ZTShow::doTrickEvent`, per `ZTShow_doTrickEvent.c`/`.asm`. `this` is `ZTShow*`;
/// `+0x4` script id, `+0x10` owning `ZTShowInfo*`, `+0x28`/`+0x2c`/`+0x30` trick-count/satisfaction/
/// satisfaction-mirror accumulators (all confirmed directly from `.asm`, not just the decompiled `.c`).
/// `state_ptr` is a real, vanilla-owned `ZTShowScriptState*` (never freed/allocated by this module - see
/// the plan's "narrow vanilla-memory-compatible carve-out" decision); `+0xc` trick index, `+0xf`
/// skip-scoring flag.
///
/// `FUN_005a698a` (called at the very end of the real function, in the tail shared by every path that
/// doesn't `return` early) is a confirmed no-op (`{ return; }`) - omitted here rather than ported.
pub fn do_trick_event(this: u32, state_ptr: u32) {
    if state_ptr == 0 {
        return;
    }
    let script_id = get_from_memory::<u16>(this + 0x4);
    let trick_index = get_from_memory::<u16>(state_ptr + 0xc);
    let Some(item) = crate::ztshowscriptmgr::item_snapshot_by_id(script_id, trick_index) else {
        return;
    };
    if item.item_type == 3 {
        return;
    }

    let satisfaction_sum = get_from_memory::<i32>(this + 0x2c);
    save_to_memory(this + 0x2c, satisfaction_sum.wrapping_add(item.satisfaction as i32));

    let show_info = get_from_memory::<u32>(this + 0x10);
    let skip_scoring = get_from_memory::<u8>(state_ptr + 0xf) != 0;

    if skip_scoring {
        unsafe { send_event(show_info, 0x272a, 0, 0x57, 0, 0, 1) };
    } else {
        let count = get_from_memory::<i32>(this + 0x28);
        save_to_memory(this + 0x28, count.wrapping_add(1));
        let mirror = item.satisfaction_mirror as i32;
        let mirror_sum = get_from_memory::<i32>(this + 0x30);
        save_to_memory(this + 0x30, mirror_sum.wrapping_add(mirror));

        let mgr_ptr = globals().ztshowmgr_ptr();
        if !mgr_ptr.is_null() {
            let mgr = unsafe { &*mgr_ptr };
            if mirror <= mgr.threshold_a as i32 {
                unsafe {
                    send_event(show_info, 0x272a, 0, 0x57, mirror as u32, (mgr.threshold_a as i32 - mirror) as u16, 1);
                    DO_KEEPER_EVENT.original()(this as *const u32, 0x271f, state_ptr as *const u32);
                }
                return;
            }
            if mirror < mgr.threshold_c as i32 {
                unsafe {
                    if mgr.threshold_b as i32 <= mirror {
                        send_event(show_info, 0x272c, 0, 0x57, mirror as u32, (mirror - mgr.threshold_b as i32) as u16, 1);
                    } else {
                        send_event(show_info, 0x272b, 0, 0x57, mirror as u32, 0, 1);
                    }
                    send_event(show_info, 0x271f, 0, 0x4b, 0, trick_index, 1);
                }
                return;
            }
            unsafe {
                send_event(show_info, 0x272d, 0, 0x57, mirror as u32, (mirror - mgr.threshold_c as i32) as u16, 1);
                DO_KEEPER_EVENT.original()(this as *const u32, 0x271f, state_ptr as *const u32);
            }
            return;
        }
        // GLOBAL_ZTShowMgr == null: vanilla's threshold block is skipped entirely (matches its own
        // `if (GLOBAL_ZTShowMgr != 0) { ... }` guard with no else) and falls through to the shared tail.
    }
    unsafe { DO_KEEPER_EVENT.original()(this as *const u32, 0x271f, state_ptr as *const u32) };
}

/// Reimplementation of `ZTShow::validateItem`, per `ZTShow_validateItem.c`/`.asm`. `this` is `ZTShow*`;
/// `+0x4` script id (positional item index, matching [`crate::ztshowscriptmgr::item_snapshot_by_id`]'s own
/// indexing), `+0x8` unit-type id (passed to `getShowUnitList`), `+0x10` owning `ZTShowInfo*`.
///
/// The real function's first-node lookup reads `ZTShowInfo::getShowUnitList`'s return value as the
/// *address holding* the list's own sentinel pointer, then dereferences the sentinel once more to reach
/// the first real node (same double-indirection pattern as `ztmegatilemgr::recalculate_characteristics`'s
/// tile guest-list walk) - and does so **without an empty-list check**, so an empty unit list would read
/// garbage from the sentinel node itself. Ported faithfully rather than "fixed": `validateItem` is only
/// ever called once a show is already running with units assigned, so the list is expected non-empty in
/// practice.
///
/// `FUN_005d923d` (the real fallback when the unit lookup/type-check fails) is a confirmed no-op - its
/// return value feeds directly into vanilla's own return via a decompiler `extraout_EAX` (register value
/// left over from the preceding failed call, not a value `FUN_005d923d` itself produces); this returns
/// `0` for that path, matching the `unit_ptr == 0` case exactly and the type-check-`false` case in the
/// overwhelmingly likely (bool-return-zero-extended) case - see this function's own inline comment.
///
/// The final `call_unit_vtable_u16_u16(unit_ptr, 0x218, ...)` dispatch (real, untouched `ZTAnimal::
/// validateTrickType`, `0x005a6f96`) used to crash live (`mov ecx,[ebx+0x58]` with `ebx==1`,
/// `openzt/plans/ztshowscriptmgr-open-items.md` item 12): its own real body calls `ZTUnit::getShowItem` ->
/// `ZTUnitType::getShowItem` -> `ZTShowScript::getItemByTrickID`, directly dereferencing the returned
/// `ZTShowScriptItem*` - a third raw-dereferencing consumer of Stage 1 data, same hazard class as
/// `validate`/`stop_with_id`, just one level further down the real call graph than either. Fixed at the
/// source (`ztshowscriptmgr::get_item_by_trick_id`) rather than here, since the raw dereference happens
/// inside real vanilla code this module doesn't otherwise touch - see that function's doc comment.
pub fn validate_item(this: u32, index: u16) -> i32 {
    if index == 0xffff {
        return 0;
    }
    let show_info = get_from_memory::<u32>(this + 0x10);
    let unit_type_id = get_from_memory::<u32>(this + 0x8);
    // `.hooked()`: `GET_SHOW_UNIT_LIST` is now detoured by `ztshowinfo.rs` (Stage 7) - see `CLAUDE.md`'s
    // "every stage that ports a call-through-only method must also switch its own `.original()` call
    // sites to `.hooked()`" rule.
    let list_ptr = unsafe { GET_SHOW_UNIT_LIST.hooked()(show_info as *const u32, unit_type_id) } as u32;
    let sentinel = get_from_memory::<u32>(list_ptr);
    let first_node = get_from_memory::<u32>(sentinel);
    let unit_numeric_id = get_from_memory::<u32>(first_node + 0x8);

    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_ptr = unsafe { GET_UNIT.original()(world, unit_numeric_id as i32) };
    if unit_ptr == 0 || !unsafe { entity_type_matches(unit_ptr, RVA_SHOW_TRICK_TYPE_CHECK) } {
        // FUN_005d923d() - no-op, see doc comment.
        return 0;
    }

    let script_id = get_from_memory::<u16>(this + 0x4);
    let Some(item) = crate::ztshowscriptmgr::item_snapshot_by_id(script_id, index) else {
        return -1;
    };
    let arg2 = get_from_memory::<u16>(show_info + 0x70);
    unsafe { call_unit_vtable_u16_u16(unit_ptr, 0x218, item.id, arg2) }
}

/// Same mechanism as [`entity_type_matches`], for an *already-resolved* type pointer (e.g. `BFWorldMgr::
/// getType`'s return) rather than a `BFEntity*` needing its own `+0x128` indirection first - `stop_with_id`/
/// `start` both call `getType` directly and check its result's own vtable slot `0x1c`. `pub(crate)` since
/// `ztshowinfo.rs`'s `get_scheduled_show_keeper_type` needs the exact same mechanism (per its own doc
/// comment).
pub(crate) unsafe fn type_check(type_ptr: u32, type_check_arg_rva: u32) -> bool {
    let vtable = get_from_memory::<u32>(type_ptr);
    let check_fn = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
    let arg = crate::globals::get_module_base("zoo.exe") as u32 + type_check_arg_rva;
    check_fn(type_ptr, arg)
}

/// `DAT_00638690`'s RVA - the same "is this an animal-ish type" check `ztthoughtmgr::
/// resolve_object_own_habitat_ptr` already uses, reused here for `stop_with_id`/`start`'s own type check
/// (distinct from [`RVA_SHOW_TRICK_TYPE_CHECK`] - a different sentinel, confirmed via each's own `.asm`).
/// `pub(crate)` - see [`type_check`]'s own doc comment.
pub(crate) const RVA_ANIMAL_TYPE_CHECK: u32 = 0x0023_8690;

/// Raw no-arg virtual dispatch through an object's own vtable at `slot_offset`, returning `bool` - the
/// shape both the habitat's `+0x20` slot (`start`'s owning-habitat check) and a unit's `+0x22c` slot
/// (`start`'s per-unit show-state-needed check) share. No named symbol for either; raw calling convention
/// confirmed via `.asm` push-order reads (no pushed args beyond `this`/`ECX`). `pub(crate)`:
/// `zthabitatmgr.rs`'s own `teardown_sound` reuses this for a real `SNDSound`'s `+0x50` predicate slot,
/// the identical implicit-`this`-via-thiscall shape.
pub(crate) unsafe fn call_entity_vtable_noargs(entity_ptr: u32, slot_offset: u32) -> bool {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> bool>(target) };
    f(entity_ptr)
}

/// Same shape as [`call_entity_vtable_noargs`] but for a no-arg vtable slot returning a `u32` value
/// rather than `bool` - `DAT_0063e450`'s (the show-editor's currently-selected `ZTUnitType*`) own
/// vtable slot `0x20`, used by `showpanel_fillTrickLists`/`_copyListToScript` (`ztshowui.rs`) to resolve
/// the selected species' numeric unit-type id. `pub(crate)` since `ztshowui.rs` needs it too.
pub(crate) unsafe fn call_entity_vtable_u32_noargs(entity_ptr: u32, slot_offset: u32) -> u32 {
    let vtable = get_from_memory::<u32>(entity_ptr);
    let target = get_from_memory::<u32>(vtable + slot_offset);
    let f = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target) };
    f(entity_ptr)
}

/// Reimplementation of `ZTShow::stop`'s 1-arg overload (`ztshow::STOP_0`, per `ZTShow_stop_0.c`/`.asm`) -
/// reassigns the show's script id, discovered as a **second, independent raw-dereferencing consumer of
/// `ZTShowScriptMgr::getScript`'s return value** not on the plan's original "must reimplement" list (found
/// while porting `start`, which calls this directly - see that function's own doc comment). `+0x4` current
/// script id, `+0x8` script type, `+0xc` unconfirmed flag field (`0x2550` magic constant, faithfully
/// ported), `+0x10` owning `ZTShowInfo*`.
///
/// `ZTShow::stop`'s **0-arg** overload (`ztshow::STOP_1`, `0x005a85e9`) is a distinct, separate real
/// function - not a wrapper around this one - and is deliberately left un-detoured. Audited safe via
/// `private/resources/decompiles/ZTShow_stop_1.c`: its body only ever touches habitat/UI-toast state (the
/// owning habitat's gate, a `BFApp::buildString`/`BFUIMgr::displayMessage` "show stopped" toast - the same
/// class of small-object-freelist buffer teardown `start`'s own doc comment already declined to risk
/// reimplementing) before calling `sendShowEndedEvent`/`reinit`/`ZTShowInfo::checkPendingScripts` - the two
/// show-script-adjacent calls in that tail (`sendShowEndedEvent` -> [`calculate_percent_adjustment`],
/// `checkPendingScripts` -> [`check_pending_scripts`]) already route through this module's own
/// reimplementations, so this function never itself raw-dereferences Stage 1 data.
pub fn stop_with_id(this: u32, new_script_id: u16) {
    let current_id = get_from_memory::<u16>(this + 0x4);
    if current_id != new_script_id {
        if current_id != 0 {
            unsafe { REINIT.original()(this as *const u32) };
        }
        if let Some(script_type) = crate::ztshowscriptmgr::script_type_by_id(new_script_id) {
            save_to_memory(this + 0x4, new_script_id);
            save_to_memory(this + 0x8, script_type);
            let world = globals().ztworldmgr_ptr() as *const u32;
            let unit_type_ptr = unsafe { GET_TYPE.original()(world, script_type as i32) } as u32;
            if unit_type_ptr != 0 && unsafe { type_check(unit_type_ptr, RVA_ANIMAL_TYPE_CHECK) } {
                save_to_memory(this + 0xc, 0x2550u32);
            }
        }
        let show_info = get_from_memory::<u32>(this + 0x10);
        if show_info != 0 {
            unsafe { RECALCULATE_SCHEDULE.hooked()(show_info as *const u32, 0) };
        }
    }
}

/// Inlined `checkOwningHabitat`. Returns **true when the show must be BLOCKED from starting** - i.e. it
/// mirrors vanilla's `checkOwningHabitat` returning its nonzero "blocked" code (`10`), not a "safe to
/// proceed" flag: the owning habitat exists, passes its own `+0x20` vtable check (real tank exhibit), and
/// has zero water level (`zthabitatmgr::ZTHabitat::water_level`, `+0x188`) - i.e. a real show tank that is
/// currently empty. Every other case (no owning habitat, vtable check fails, or the tank has water) means
/// vanilla returns `0` and lets the show proceed, so this returns `false` there. Confirmed against the
/// macOS `ZTShow::checkOwningHabitat`/`ZTShow::start` decompiles: `checkOwningHabitat() == 0` is vanilla's
/// own "proceed" branch, so callers here must treat a `true` return as "stop", not "go" - see `start`'s
/// call site. None of this is a separate named function on Windows, confirmed via `.asm`-level inlining
/// directly into `start`'s own body. Factored out as its own function (rather than left inline) so it can
/// be exercised directly against real `GLOBAL_ZTHabitatMgr`-owned habitat data in a live
/// reimplementation-test without also needing `start`'s own earlier preconditions
/// (`RESOLVE_NEXT_SCHEDULED_SCRIPT_ID` returning a real scheduled script, `GET_NUM_UNITS >= 1`)
/// independently satisfied first - see `reimplementation_tests/mod.rs`'s `ZTSHOW_CHECK_OWNING_HABITAT_LIVE`
/// test. `show_info` is a `ZTShowInfo*`.
pub(crate) fn check_owning_habitat(show_info: u32) -> bool {
    let habitat = get_from_memory::<u32>(show_info + 0xa0);
    habitat != 0 && unsafe { call_entity_vtable_noargs(habitat, 0x20) } && get_from_memory::<u32>(habitat + 0x188) == 0
}

/// Reimplementation of `ZTShow::validate`, per `ZTShow_validate.c`. Vanilla's own body calls
/// `ZTShowScriptMgr::getScript(id)` and then directly dereferences the returned `ZTShowScript*`'s raw
/// `+0x10` field to walk its item list - exactly the "will misbehave if it ever dereferences a handle
/// this module returns" hazard `ztshowscriptmgr.rs`'s own module doc comment warns about: `getScript` is
/// Stage-1-detoured to return a non-dereferenceable synthetic handle (`SYNTHETIC_SCRIPT_HANDLE_BASE |
/// id`), never a real pointer. `validate` was missed from the plan's "must reimplement" list even though
/// `start` calls it directly - confirmed live: real gameplay reaches this path on essentially the first
/// scheduled show attempting to start after a save loads, corrupting `ecx` with the synthetic handle and
/// crashing on `mov edx, [ecx+0x10]` inside vanilla's own list-length loop. Ported onto the Stage 1
/// store's safe id-keyed accessors (`get_script`/`size`/`get_item`/`remove_item`) and the already-ported
/// [`validate_item`] instead - `ZTShowInfo::getShowUnitList`/`checkUnit` stay real vanilla calls, since
/// `ZTShowInfo` itself is untouched by Stage 1. `this` is `ZTShow*`; `+0x4` script id, `+0x8` unit-type
/// id, `+0x10` owning `ZTShowInfo*`. `check_units` is vanilla's own `param_1`.
///
/// The item-list loop's own decision logic (which position to re-check/remove) is factored out into
/// [`run_validate_loop`], pure control flow with no memory access, so it can be unit-tested directly with
/// synthetic closures - see that function's own doc comment for why it always returns `0` (the real
/// `iVar6`/`result` accumulator this was ported from provably always ends up `0` by the time the loop
/// exits, a non-obvious reduction from vanilla's own decompile).
pub fn validate(this: u32, check_units: bool) -> i32 {
    if check_units {
        let show_info = get_from_memory::<u32>(this + 0x10);
        let unit_type_id = get_from_memory::<u32>(this + 0x8);
        // `.hooked()`: both `GET_SHOW_UNIT_LIST` and `CHECK_UNIT` are now detoured by `ztshowinfo.rs`
        // (Stage 7) - see `validate_item`'s own comment above for the rule this follows.
        let list_ptr = unsafe { GET_SHOW_UNIT_LIST.hooked()(show_info as *const u32, unit_type_id) } as u32;
        let sentinel = get_from_memory::<u32>(list_ptr);

        let mut unit_count = 0;
        let mut node = get_from_memory::<u32>(sentinel);
        while node != sentinel {
            unit_count += 1;
            node = get_from_memory::<u32>(node);
        }
        if unit_count == 0 {
            return 1;
        }

        node = get_from_memory::<u32>(sentinel);
        while node != sentinel {
            let unit_id = get_from_memory::<u32>(node + 0x8);
            if !unsafe { CHECK_UNIT.hooked()(show_info as *const u32, unit_id) } {
                return 4;
            }
            node = get_from_memory::<u32>(node);
        }
    }

    let script_id = get_from_memory::<u16>(this + 0x4);
    let handle = crate::ztshowscriptmgr::get_script(script_id);
    if handle != 0 {
        let count = crate::ztshowscriptmgr::size(handle);
        let result = run_validate_loop(
            count,
            |index| validate_item(this, index),
            |index| {
                crate::ztshowscriptmgr::remove_item(handle, index);
            },
        );
        if crate::ztshowscriptmgr::size(handle) != 0 {
            return result;
        }
    }
    6
}

/// Pure control-flow half of [`validate`]'s item-validation loop: for each position `0..item_count`,
/// calls `is_valid(index)` (matching [`validate_item`]'s own return convention - `0` means valid); a
/// nonzero result removes that position via `remove` and re-visits the same position (the next item
/// shifts down into it), same as vanilla's own `index -= 1` immediately before the loop's `index += 1`.
///
/// Always returns `0`: every vanilla write to the real `iVar6`/`result` accumulator this loop was
/// originally ported from happens only in the branch that immediately resets it back to `0` again (see
/// [`validate`]'s own doc comment on this) - by the time the loop exits, the last value written is always
/// `0`, whether from the reset itself or from a final valid item's own `0` result. Kept as an explicit
/// function call (not inlined as a bare `0` in [`validate`]) to keep this reduction visibly tied to the
/// reasoning above rather than silently hardcoded.
///
/// Doesn't reproduce vanilla's own `ZTShowScript::getItem(handle, index) != 0` existence check before each
/// `is_valid` call: the loop's own invariant (`index < count`, where `count` only ever decreases in
/// lockstep with an actual removal) guarantees an item always exists at `index` against Stage 1's own
/// internally-consistent store, so that check can never actually be false here - dropped as dead weight
/// rather than ported literally.
fn run_validate_loop(item_count: i32, mut is_valid: impl FnMut(u16) -> i32, mut remove: impl FnMut(u16)) -> i32 {
    let mut count = item_count;
    let mut index: i32 = 0;
    while index < count {
        if is_valid(index as u16) != 0 {
            remove(index as u16);
            count -= 1;
            index -= 1;
        }
        index += 1;
    }
    0
}

/// Reimplementation of `ZTShow::checkScript`, per `ZTShow_checkScript.c`/`.asm`. A fourth real,
/// un-reimplemented raw-dereferencing consumer of `ZTShowScriptMgr::getScript`'s return value, found by
/// auditing the plan's "safe to leave untouched" consumer list (`openzt/plans/ztshowscriptmgr-open-items.md`
/// item 1) - same hazard class as `validate`/`stop_with_id`/`ZTAnimal::validateTrickType`. Reachable from
/// `ZTShow::run`/`update`/`aboutToStart` (all real, un-detoured, hit on every simulation tick of an active
/// show once a script is assigned), so this was gameplay-reachable, not just theoretical.
///
/// The decompiled `.c`'s `_param_1`/`CONCAT22` local is a Ghidra artifact of the u16 stack parameter having
/// an uninitialized-per-the-decompiler upper half; the `.asm` shows the real logic plainly: `id = param_1 if
/// param_1 != 0, else this->+0x4` (the show's own currently-assigned script id, same field every other
/// function in this module reads at that offset). Returns whether that script exists and has at least one
/// item.
pub fn check_script(this: u32, param_1: u16) -> bool {
    let id = if param_1 != 0 { param_1 } else { get_from_memory::<u16>(this + 0x4) };
    id != 0 && crate::ztshowscriptmgr::script_item_count_by_id(id) > 0
}

/// Reimplementation of `ZTShow::calculatePercentAdjustment`, per `ZTShow_calculatePercentAdjustment.asm`
/// (the decompiled `.c`'s `(param_1->cls_0x6355b8).mbr_0x10` is a Ghidra misattribution of a global-vs-member
/// access - the `.asm` shows it's plainly `this+0x28`, the same trick-count accumulator [`do_trick_event`]
/// already reads/writes at that offset). A fifth real, un-reimplemented raw-dereferencing consumer found by
/// the same open-items audit as [`check_script`] - reachable via `calculateRestBonus`/
/// `calculateSatisfactionPercent`, both called from `ZTShow::sendShowEndedEvent` on every show start/stop
/// cycle.
///
/// `this` is `ZTShow*`; `+0x4` script id, `+0x28` trick-count accumulator. Unlike [`do_trick_event`], the
/// real function's `GLOBAL_ZTShowMgr` read here has **no null check** in the `.asm` (confirmed - no
/// `TEST`/`JZ` on it before the `+0x20`/`+0x24` dereferences) - ported faithfully rather than defensively:
/// a null `GLOBAL_ZTShowMgr` here would crash real vanilla too, so this doesn't add a guard vanilla itself
/// doesn't have.
pub fn calculate_percent_adjustment(this: u32) -> i32 {
    let script_id = get_from_memory::<u16>(this + 0x4);
    let item_count = crate::ztshowscriptmgr::script_item_count_by_id(script_id) as i32;
    let threshold = get_from_memory::<i32>(this + 0x28);
    if item_count > 0 && threshold <= item_count {
        let mgr_ptr = globals().ztshowmgr_ptr() as u32;
        let mgr_lower = get_from_memory::<i32>(mgr_ptr + 0x20);
        let mgr_upper = get_from_memory::<i32>(mgr_ptr + 0x24);
        if threshold > mgr_upper {
            return threshold - mgr_upper;
        }
        if threshold < mgr_lower {
            return mgr_lower - threshold;
        }
    }
    0
}

/// Reimplementation of `ZTShow::start` - real body is `ztshow::START` (`0x005a3db4`, a thin wrapper) tail
/// -calling into `ztshow::RESOLVE_NEXT_SCHEDULED_SCRIPT_ID`/`standalone::INIT_SHOW_SCRIPT_STATE`
/// (`0x005a3de4`). Ported from `INIT_SHOW_SCRIPT_STATE`'s decompiled source (supplied directly, not a
/// local decompile file), cross-checked against the macOS `ZTShow::start` decompile's matching call
/// sequence. The per-unit create/find-or-insert call below goes through
/// [`crate::ztshowscriptstate::create_show_script_state`]; the real function behind it
/// (`generated.rs`'s `ztshowscriptstate::CONSTRUCTOR`, `0x005a4075`, really
/// `ZTShow::createShowScriptState`) is thiscall `(ZTShow*, u32) -> u32`, `RET 0x4` at all three return
/// sites per `ZTShowScriptState_ZTShowScriptState.c` - it takes no `show_id` argument (it reads the u16
/// for the new state's own `+0x4` from `this`'s own `+0x4` internally); the caller separately writes
/// `show_id` (read from `this+0x6`) into each unit's own `+0x254` field afterward
/// (`_initShowScriptState.c` line 85), which the loop below does.
///
/// Two pieces of the real function's success-path tail are deliberately **not** ported:
/// - `(this->cls_0x6355b8+0xc)`/`ZTShow+0x24`'s write from `GLOBAL_ZTAIMgr->field_0xec` - `GLOBAL_ZTAIMgr`
///   has no known RVA anywhere in this repo (only its vtable address), and this field isn't read by
///   anything else in this module's scope - a real but low-stakes fidelity gap, not a correctness risk.
/// - The "show has started" UI toast (`BFApp::buildString`/`BFUIMgr::displayMessage`) - its real body
///   includes a small-object-freelist buffer teardown (`FUN_00402629`/`DAT_00638000`) whose exact
///   semantics aren't independently confirmed; guessing wrong risks the cross-allocator heap corruption
///   class CLAUDE.md warns about, for a purely cosmetic message. Skipped rather than risked.
pub fn start(this: u32) {
    let script_id = unsafe { RESOLVE_NEXT_SCHEDULED_SCRIPT_ID.original()(this as *const u32) as u16 };
    let Some(script_type) = crate::ztshowscriptmgr::script_type_by_id(script_id) else { return };
    if script_type == 0 {
        return;
    }

    let show_info = get_from_memory::<u32>(this + 0x10);
    // `.hooked()`: `GET_NUM_UNITS` is now detoured by `ztshowinfo.rs` (Stage 7) - see `validate_item`'s
    // own comment above for the rule this follows.
    let unit_count = unsafe { GET_NUM_UNITS.hooked()(show_info as *const u32, script_type) };
    if unit_count < 1 {
        return;
    }

    if check_owning_habitat(show_info) {
        return;
    }

    // Inlined `setShowUnitTypeID` + the same animal-type flag check `stop_with_id` does independently
    // (matches vanilla's own real redundancy - both `start` and `stop` compute it separately).
    save_to_memory(this + 0x8, script_type);
    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_type_ptr = unsafe { GET_TYPE.original()(world, script_type as i32) } as u32;
    if unit_type_ptr != 0 && unsafe { type_check(unit_type_ptr, RVA_ANIMAL_TYPE_CHECK) } {
        save_to_memory(this + 0xc, 0x2550u32);
    }

    // "setShowScriptID" on Windows isn't a separate function either - it's achieved by calling `stop`
    // with the new id, which reassigns `+0x4` as a side effect of its own reinit-and-reassign logic.
    stop_with_id(this, script_id);

    if validate(this, true) != 0 {
        return;
    }
    unsafe { CLEAR_SHOW_SCRIPT_STATES.original()(this as *const u32) };

    let unit_type_for_list = get_from_memory::<u32>(this + 0x8);
    // `.hooked()`: see the other `GET_SHOW_UNIT_LIST` call site's own comment above.
    let list_ptr = unsafe { GET_SHOW_UNIT_LIST.hooked()(show_info as *const u32, unit_type_for_list) } as u32;
    let sentinel = get_from_memory::<u32>(list_ptr);
    let mut node = get_from_memory::<u32>(sentinel);
    while node != sentinel {
        let next_node = get_from_memory::<u32>(node);
        let unit_id = get_from_memory::<u32>(node + 0x8);
        let unit_ptr = globals().ztworldmgr().resolve_entity_by_id(unit_id) as u32;
        let eligible = unit_ptr != 0 && unsafe { entity_type_matches(unit_ptr, RVA_SHOW_TRICK_TYPE_CHECK) };
        if !eligible {
            // Direct Rust call, not `.hooked()`/`.original()`: `ZTShowInfo::removeUnit` is now a real port
            // (`ztshowinfo.rs` Stage 8) - see [`crate::ztshowinfo::remove_unit`]'s own doc comment for why
            // this replaces a genuine bug this call site used to have (passing `&unit_id`, a *local's*
            // address, where real vanilla's own un-dereferencing third-parameter comparison needed
            // `unit_id` itself).
            remove_unit(show_info, unit_type_for_list, unit_id);
        } else {
            let assigned_show_id = get_from_memory::<u16>(unit_ptr + 0x254);
            // `.hooked()`, not `.original()`: `ZTShowMgr::getShowInfo` is detoured onto the Rust
            // registered-shows store since `ztshowmgr.rs`'s stage 4, and this call site wants exactly
            // what any other real caller of the address now gets (the `load_display_string` precedent
            // in `ztshowui.rs`). A release build's raw-cast `.original()` would be an accidental
            // re-entry here while debug silently routed to vanilla's tree instead.
            let owning_show_info = unsafe { GET_SHOW_INFO.hooked()(globals().ztshowmgr_ptr() as *const u32, assigned_show_id) };
            // `.hooked()`, not `.original()`: `IS_STARTED` is now detoured by `ztshowinfo.rs` (Stage 2 of
            // `ztshowinfo-implementation-plan.md`) - see that plan's own "every stage that ports a
            // call-through-only method must switch its `.original()` call sites to `.hooked()`" rule.
            let needs_state = (!owning_show_info.is_null() && unsafe { IS_STARTED.hooked()(owning_show_info) } == 0)
                || !unsafe { call_entity_vtable_noargs(unit_ptr, 0x22c) };
            if needs_state {
                let show_id = get_from_memory::<u16>(this + 0x6);
                // Direct Rust call, not `.original()`/`.hooked()`: the real address is detoured onto the
                // Rust port (`ztshowscriptstate.rs` Stage 3), and this call site wants the port's own
                // behavior for every caller.
                let result = crate::ztshowscriptstate::create_show_script_state(this, unit_id);
                if result != 0 {
                    return;
                }
                save_to_memory(unit_ptr + 0x254, show_id);
            }
        }
        node = next_node;
    }

    let gather_result = unsafe { GATHER_UNITS.original()(this as *const u32) };
    if !gather_result {
        save_to_memory(this + 0x1e, 0u8);
        save_to_memory(this + 0x1f, 1u8);
        save_to_memory(this + 0x20, 0u8);
        // `+0x24` (GLOBAL_ZTAIMgr->field_0xec) intentionally skipped - see this function's doc comment.
        let show_info = get_from_memory::<u32>(this + 0x10);
        unsafe { send_event(show_info, 0x2713, 0, 0x57, 0, 0, 1) };
        let habitat = get_from_memory::<u32>(show_info + 0xa0);
        unsafe { PLAY_SHOW_START_SOUND.original()(habitat as *const u32) };
        // "Show has started" UI toast intentionally skipped - see this function's doc comment.
    }
}

/// Recursive in-order collection of every node in the pending-scripts `std::map<unitTypeID, {current:u16
/// @+0x1c, pending:u16 @+0x1e}>` embedded in `ZTShowInfo+0x44` - real, untouched vanilla memory (never part
/// of Stage 1's independent store), node shape `left+0x8/right+0xc/key+0x10` confirmed consistent across
/// this function's, `addScript`'s, and `start`'s own decompiled descent code, child pointers null (`0`)
/// terminated (confirmed via `addScript`'s own `while (fVar4 != 0.0)` descent guard). Same technique as
/// `ztawardmgr::live_support::walk_tree` (already live-tested there) rather than reconstructing
/// `check_pending_scripts`'s real successor-iterator algorithm, whose decompiled form has an edge-case
/// check that couldn't be independently verified against a standard `_Tree::_Inc` reference - since this
/// function only mutates each node's *value* fields (`+0x1c`/`+0x1e`), never the tree's own structural
/// pointers, collecting every node upfront and processing them after is equivalent and sidesteps that risk
/// entirely. A real red-black tree of unit types is never more than a few dozen nodes deep, so recursion
/// depth is a non-concern (same reasoning `ztawardmgr.rs`'s own `walk_tree` already relies on).
pub(crate) fn collect_pending_script_nodes(node: u32, out: &mut Vec<u32>) {
    if node == 0 {
        return;
    }
    collect_pending_script_nodes(get_from_memory::<u32>(node + 0x8), out);
    out.push(node);
    collect_pending_script_nodes(get_from_memory::<u32>(node + 0xc), out);
}

/// Live node count for `show_info`'s pending-scripts tree (`+0x44`), via the same trusted, always-compiled
/// walk [`check_pending_scripts`] itself runs every tick - unlike `live_support::collect_pending_script_nodes`
/// (only compiled under `reimplementation-tests`), this is available to any build, specifically so
/// `zthabitatmgr.rs`'s `save_load_diag` module can compare "what does the live tree actually contain right
/// after load" against "what does the file say on reload" for `ztshow-save-corruption-investigation.md`.
pub(crate) fn pending_script_node_count(show_info: u32) -> usize {
    let header = get_from_memory::<u32>(show_info + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);
    nodes.len()
}

/// Reimplementation of `ZTShowInfo::checkPendingScripts`, per `ZTShowInfo_checkPendingScripts.c`/`.asm`.
/// For every node whose `pending` field (`+0x1e`) isn't `0xffff` (no pending change): if `pending != current`
/// (`+0x1c`), reassigns `current = pending`, drops the *old* current script from Stage 1's store (replacing
/// the real body's direct `ZTShowScript::~ZTShowScript()` call - unsafe against Stage 1's synthetic handles,
/// see the module doc comment), then calls the real vanilla `addShow`/`removeShow` depending on whether the
/// new current script exists and has any items. Either way, `pending` is reset to `0xffff`.
pub fn check_pending_scripts(show_info: u32) {
    let header = get_from_memory::<u32>(show_info + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);

    for node in nodes {
        let pending_id = get_from_memory::<u16>(node + 0x1e);
        if pending_id == 0xffff {
            continue;
        }
        let unit_type_id = get_from_memory::<u32>(node + 0x10);
        let current_id = get_from_memory::<u16>(node + 0x1c);
        if current_id != pending_id {
            save_to_memory(node + 0x1c, pending_id);
            if crate::ztshowscriptmgr::script_exists_by_id(current_id) {
                crate::ztshowscriptmgr::unregister_script_by_id(current_id);
            }
            let has_items =
                crate::ztshowscriptmgr::script_exists_by_id(pending_id) && crate::ztshowscriptmgr::script_item_count_by_id(pending_id) > 0;
            unsafe {
                if has_items {
                    ADD_SHOW.hooked()(show_info as *const u32, unit_type_id);
                } else {
                    REMOVE_SHOW.hooked()(show_info as *const u32, unit_type_id);
                }
            }
        }
        save_to_memory(node + 0x1e, 0xffffu16);
    }
}

/// Node size for the pending-scripts map (`ZTShowInfo+0x44`), confirmed byte-exact via `ZTShowInfo::save`'s
/// own per-node serialization (`ZTShowInfo_save.c` lines 109-120): tree-bookkeeping prefix `0x10` bytes
/// (`+0x0` unknown/color, `+0x4` parent, `+0x8` left, `+0xc` right) + value fields `+0x10` key through
/// `+0x44` (last field, 4 bytes) = `0x48` total. Every field `save`/`load` round-trip is listed in
/// [`allocate_pending_script_node`]'s own comment; nothing beyond `+0x44` is ever read or written by
/// `save`/`load` or any other consumer found this session, which is the strongest available evidence this
/// is the complete struct (a save/load round-trip losing a real field would be an observable data-loss bug).
const PENDING_SCRIPT_NODE_SIZE: u32 = 0x48;

/// Allocates and zero-initializes a new pending-scripts map node via the real vanilla allocator
/// (`standalone::OPERATOR_NEW`) - never `Box`, since this node lives in memory `getShowUnitList`/
/// `getNumUnits`/`addUnitToList`/`removeUnit`/`incrementAttendance`/`incrementReceipts`/`enterNewMonth`
/// (all real, un-reimplemented vanilla code, per the plan's "safe to leave untouched" list) read and write
/// directly, and mixing allocators on either side of that boundary is exactly the heap-corruption class
/// CLAUDE.md warns about.
///
/// Field-by-field defaults (all zeroed except where noted), confirmed via `ZTShowInfo_save.c`'s complete
/// per-node write list plus each named accessor's own read/write (`+0x28`/`+0x2c`/`+0x30` receipts,
/// `+0x34`/`+0x38`/`+0x3c` attendance, all `incrementAttendance`/`incrementReceipts`/`enterNewMonth`;
/// `+0x40`/`+0x44` creation date, `addScript` itself, stamped by the caller only on a genuine first
/// insertion, not here): `+0x0`/`+0x14` unknown/never read by anything found this session (zeroed, safe
/// regardless of real meaning), `+0x20` unconfirmed flag byte, `+0x24` unconfirmed `u32` (both zeroed,
/// matching every other never-independently-set field's natural zero-init default for a value-initialized
/// aggregate). `+0x18` (the unit list header slot) is the one field that can't just be zeroed - see below.
///
/// `+0x18`'s target: `getNumUnits`/`getShowUnitList`/`removeUnit` all double-dereference it
/// (`**(node+0x18)`) as an intrusive circular list's sentinel, node shape `{next:+0x0, prev:+0x4,
/// payload:+0x8}` (confirmed via `removeUnit`'s own unlink code). A null `+0x18` would crash the first real
/// vanilla caller that touches it - same class of hazard as `ztmegatilemgr::empty_category_map_sentinel`'s
/// own precedent - so this allocates a second, real, self-referential sentinel node (`next`/`prev` both
/// pointing at itself) and stores its address here, matching what a genuinely-empty intrusive list looks
/// like everywhere else in this codebase.
fn allocate_pending_script_node(unit_type_id: u32) -> u32 {
    let node = unsafe { OPERATOR_NEW.original()(PENDING_SCRIPT_NODE_SIZE) } as u32;
    unsafe { std::ptr::write_bytes(node as *mut u8, 0, PENDING_SCRIPT_NODE_SIZE as usize) };
    save_to_memory(node + 0x10, unit_type_id);

    let sentinel = unsafe { OPERATOR_NEW.original()(0xc) } as u32;
    save_to_memory(sentinel, sentinel);
    save_to_memory(sentinel + 4, sentinel);
    save_to_memory(sentinel + 8, 0u32);
    save_to_memory(node + 0x18, sentinel);

    node
}

/// Finds or inserts a node for `unit_type_id` in the pending-scripts map (`ZTShowInfo+0x44`) - a plain,
/// unbalanced BST insert rather than a real MSVC red-black insert. This is deliberate: every reader of this
/// tree found this session (this function's own descent, `checkPendingScripts`, `getShowUnitList`,
/// `getNumUnits`, `addUnitToList`, `removeUnit`) only ever relies on the BST key-ordering invariant
/// (`left < key <= right`) for correctness, never on a color/balance bit - none was found being read
/// anywhere. Replicating vanilla's real `AI_cls_0x404fd6::meth_0x5abe74` call would additionally require
/// reverse-engineering three more unknown STL-glue helpers' calling conventions from raw `.asm`
/// (`BFTile::cls_0x40143b`, `cls_0x484e11::cls_0x484e11`, `__vector_pod<>::__vector_pod<>?`) with real
/// heap-corruption risk if any marshalling detail were wrong - an unbalanced insert sidesteps all of that
/// while staying correct for every actual consumer.
///
/// Also maintains the header's own leftmost (`+0x8`) cache pointer (standard BST bookkeeping, unrelated to
/// red-black balancing) - `enterNewMonth` (real vanilla, un-reimplemented, not in this module's scope)
/// starts its own walk from `header+0x8`, so an insert that left it stale would make `enterNewMonth`
/// silently skip freshly-added unit types.
///
/// **Bug found and fixed while adding this function's first live test coverage**: an earlier version of
/// this function also maintained a "rightmost" cache at `header+0xc`, by analogy with the leftmost cache at
/// `header+0x8` - but the `AI_cls_0x404fd6` tree header embedded at `ZTShowInfo+0x44` is only `0xc` bytes
/// (`self`/`root`/`leftmost`, confirmed via `ZTShowInfo_ZTShowInfo.asm`: the constructor call
/// `AI_cls_0x404fd6::cls_0x404fd6(ESI+0x44, ...)` is immediately followed by `MOV [ESI+0x50], 0` -  i.e.
/// vanilla's own ctor treats `ZTShowInfo+0x50` as the *next*, separate field the instant the tree ctor
/// returns), so `header+0xc` is `ZTShowInfo+0x50` - the real, live `addShow`/`removeShow` dynamic array's
/// own `begin` pointer (`ZTShowInfo_addShow.c`/`_removeShow.c`), not part of the tree at all. No real
/// vanilla reader of this tree (`ZTShowInfo::addScript`, `checkPendingScripts`, `AI_cls_0x404fd6::find`)
/// was ever found reading a "rightmost" field either - only `header+0x8` (leftmost) is genuinely read
/// (`checkPendingScripts`'s own traversal start). Writing to `header+0xc` therefore corrupted a real,
/// separate `ZTShowInfo` field on every first-ever insert for a given `ZTShowInfo` (stomping its `addShow`
/// array's `begin` pointer from `0`/null to a tree-node address), which made the very next real
/// `addShow`/`removeShow` call walk that array using the tree node as a garbage `begin` pointer against a
/// still-null `end` (`ZTShowInfo+0x54`) - an unbounded scan that reads until it hits unmapped memory and
/// crashes. This is a genuine, previously-unexercised live gameplay-corruption bug (Stage 2's `addScript`/
/// `checkPendingScripts` had no live test until now, per the plan's own open item 11) - fixed by dropping
/// the "rightmost" cache concept entirely, not just working around it in the test.
///
/// Returns `(node_address, was_newly_inserted)` - callers that need to stamp creation-only data (the
/// `+0x40`/`+0x44` date fields) check the second value, matching vanilla's own `pair<iterator,bool>`
/// insert-return convention.
/// Decision made by [`plan_pending_node_insert`] for a given `(header, unit_type_id)` pair - see that
/// function's own doc comment.
#[derive(Debug, PartialEq, Eq)]
enum PendingNodeInsertPlan {
    /// An existing node with a matching key was found - its address.
    Found(u32),
    /// The tree is empty; a freshly-allocated node becomes the root.
    NewRoot,
    /// Insert as `parent`'s left child; `parent_is_leftmost` says whether `parent` is currently the
    /// header's own leftmost-cache pointer (`+0x8`), which the new node would then replace.
    InsertLeft { parent: u32, parent_is_leftmost: bool },
    /// Insert as `parent`'s right child (never affects the leftmost cache).
    InsertRight { parent: u32 },
}

/// Pure decision half of [`find_or_insert_pending_script_node`]'s BST walk - reproduces the exact same
/// comparisons and candidate-tracking as that function's own body (`ztshow.rs`'s own history has one real
/// bug already found in this exact walk, see [`find_or_insert_pending_script_node`]'s doc comment), just
/// returning a description of what to do instead of allocating/mutating anything. [`get_from_memory`] is a
/// plain, FFI-free `ptr::read`, so this works identically against real game memory or a plain
/// Rust-allocated test arena - see this module's `#[cfg(test)]` `pending_node_plan_tests` below.
fn plan_pending_node_insert(header: u32, unit_type_id: u32) -> PendingNodeInsertPlan {
    let root = get_from_memory::<u32>(header + 4);
    if root == 0 {
        return PendingNodeInsertPlan::NewRoot;
    }

    let mut node = root;
    let mut candidate = header;
    let (parent, went_left) = loop {
        let parent = node;
        if get_from_memory::<u32>(node + 0x10) < unit_type_id {
            let right = get_from_memory::<u32>(node + 0xc);
            if right == 0 {
                break (parent, false);
            }
            node = right;
        } else {
            candidate = node;
            let left = get_from_memory::<u32>(node + 0x8);
            if left == 0 {
                break (parent, true);
            }
            node = left;
        }
    };

    if candidate != header && get_from_memory::<u32>(candidate + 0x10) <= unit_type_id {
        return PendingNodeInsertPlan::Found(candidate);
    }

    if went_left {
        let leftmost = get_from_memory::<u32>(header + 8);
        PendingNodeInsertPlan::InsertLeft { parent, parent_is_leftmost: parent == leftmost }
    } else {
        PendingNodeInsertPlan::InsertRight { parent }
    }
}

/// `show_info + 0x48` is **not** part of the tree's own header (that lives behind the pointer at
/// `show_info + 0x44` - a separately-allocated `AI_cls_0x404fd6` control block; `header + 4`/`header + 8` are
/// its real root/leftmost fields, per [`plan_pending_node_insert`]'s own use of `header`). `show_info + 0x48`
/// is a **separate cached node count** `ZTShowInfo` maintains itself: real vanilla `ZTShowInfo::save`
/// (`ZTShowInfo_save.c` line 61-62) writes this field's raw value directly to disk immediately before the
/// tree-walk loop, and real vanilla `ZTShowInfo::load` reads that exact file position back as the tile-record
/// loop's iteration count (`ZTShowInfo_load.c` line 100's `local_e4`) - i.e. this field, not a tree walk, is
/// the save format's authoritative node count. Confirmed live (`ztshowinfo-pending-scripts-tree-plan.md`):
/// with a real tree holding 3 nodes (`pending_script_node_count` walk, matching file content), `show_info +
/// 0x48` read directly (no indirection through `header` at all) held `2` - stale, stuck at whatever `load`
/// originally set it to. Real vanilla's own insert helper (`AI_cls_0x404fd6::meth_0x5abe74`, not called here -
/// this function is an independent reimplementation) must increment it on every genuine insert; not
/// replicating that was the actual root cause of the save-corruption bug this plan document investigates.
fn increment_pending_script_node_count(show_info: u32) {
    let count = get_from_memory::<u32>(show_info + 0x48);
    save_to_memory(show_info + 0x48, count + 1);
}

pub(crate) fn find_or_insert_pending_script_node(show_info: u32, unit_type_id: u32) -> (u32, bool) {
    let header = get_from_memory::<u32>(show_info + 0x44);
    let result = match plan_pending_node_insert(header, unit_type_id) {
        PendingNodeInsertPlan::Found(node) => (node, false),
        PendingNodeInsertPlan::NewRoot => {
            let new_node = allocate_pending_script_node(unit_type_id);
            save_to_memory(new_node + 4, header);
            save_to_memory(header + 4, new_node);
            save_to_memory(header + 8, new_node);
            (new_node, true)
        }
        PendingNodeInsertPlan::InsertLeft { parent, parent_is_leftmost } => {
            let new_node = allocate_pending_script_node(unit_type_id);
            save_to_memory(new_node + 4, parent);
            save_to_memory(parent + 8, new_node);
            if parent_is_leftmost {
                save_to_memory(header + 8, new_node);
            }
            (new_node, true)
        }
        PendingNodeInsertPlan::InsertRight { parent } => {
            let new_node = allocate_pending_script_node(unit_type_id);
            save_to_memory(new_node + 4, parent);
            save_to_memory(parent + 0xc, new_node);
            (new_node, true)
        }
    };
    if result.1 {
        increment_pending_script_node_count(show_info);
    }
    result
}

/// Reimplementation of `ZTShowInfo::addScript`, per `ZTShowInfo_addScript.c`/`.asm`. Assigns `new_script_id`
/// to `unit_type_id`'s pending-scripts map entry (creating it via [`find_or_insert_pending_script_node`] if
/// this is the first script ever assigned to that unit type), applying it immediately to `current` if the
/// show isn't currently started (dropping the *old* current script from Stage 1's store rather than calling
/// the real, unsafe-against-synthetic-handles `ZTShowScript::~ZTShowScript()` directly - same fix pattern as
/// `checkPendingScripts`), or leaving it queued in `pending` for `checkPendingScripts` to pick up later
/// otherwise. Vanilla's real body redundantly re-finds the node by the same key up to three times (a
/// natural consequence of repeated `map[key]`-style access in the original source, each independently
/// resolving to the same node) - collapsed here into one `find_or_insert` call reused throughout, since
/// nothing else mutates the tree's structure in between.
///
/// **Deliberately not ported**: the config-file-driven default-admission-cost block (`BFConfigFile`/
/// `s_shows.cfg`/`meth_0x46ec56`, gated behind an unrelated global flag) - `meth_0x46ec56` has no
/// decompile/address anywhere in this repo, and this only affects admission pricing, not show-script
/// correctness. Same class of deliberate skip as `start`'s own UI-toast/`GLOBAL_ZTAIMgr` gaps.
pub fn add_script(show_info: u32, unit_type_id: u32, new_script_id: u16) -> bool {
    if new_script_id == 0 || new_script_id == 0xffff || unit_type_id == 0 {
        return false;
    }

    let (node, was_inserted) = find_or_insert_pending_script_node(show_info, unit_type_id);
    if was_inserted {
        let mut date = FILETIME::default();
        unsafe { GET_DATE.original()(globals().ztgamemgr_ptr() as *const u32, &mut date as *const FILETIME) };
        let date_ticks = ((date.dwHighDateTime as u64) << 32) | date.dwLowDateTime as u64;
        save_to_memory(node + 0x40, date_ticks as i64);
    }

    save_to_memory(node + 0x1e, new_script_id);

    // `.hooked()`: see the other `IS_STARTED` call site's own comment above (`ztshowinfo.rs` Stage 2).
    let started = unsafe { IS_STARTED.hooked()(show_info as *const u32) } != 0;
    if !started {
        let old_current = get_from_memory::<u16>(node + 0x1c);
        save_to_memory(node + 0x1c, new_script_id);
        save_to_memory(node + 0x1e, 0xffffu16);
        if crate::ztshowscriptmgr::script_exists_by_id(old_current) {
            crate::ztshowscriptmgr::unregister_script_by_id(old_current);
        }
    }

    let current_id = get_from_memory::<u16>(node + 0x1c);
    let has_items =
        crate::ztshowscriptmgr::script_exists_by_id(current_id) && crate::ztshowscriptmgr::script_item_count_by_id(current_id) > 0;
    unsafe {
        if has_items {
            ADD_SHOW.hooked()(show_info as *const u32, unit_type_id);
        } else {
            REMOVE_SHOW.hooked()(show_info as *const u32, unit_type_id);
        }
    }
    true
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(CHECK_UNIT_TYPE)]
    unsafe extern "thiscall" fn check_unit_type_detour(this: *const u32, unit_type: u32) -> u32 {
        check_unit_type(this as u32, unit_type)
    }

    #[detour(GET_SHOW_SCRIPT_STATE)]
    unsafe extern "thiscall" fn get_show_script_state_detour(this: *const u32, key: u32) -> u32 {
        get_show_script_state(this as u32, key)
    }

    #[detour(DO_CURRENT_ITEM)]
    unsafe extern "thiscall" fn do_current_item_detour(this: *const u32, unit_id: u32) -> i32 {
        do_current_item(this as u32, unit_id)
    }

    #[detour(DO_TRICK_EVENT)]
    unsafe extern "thiscall" fn do_trick_event_detour(this: *const u32, state_ptr: *const u32) {
        do_trick_event(this as u32, state_ptr as u32);
    }

    #[detour(VALIDATE_ITEM)]
    unsafe extern "thiscall" fn validate_item_detour(this: *const u32, index: u16) -> u32 {
        validate_item(this as u32, index) as u32
    }

    #[detour(VALIDATE)]
    unsafe extern "thiscall" fn validate_detour(this: *const u32, check_units: bool) -> i32 {
        validate(this as u32, check_units)
    }

    #[detour(STOP_0)]
    unsafe extern "thiscall" fn stop_0_detour(this: *const u32, new_script_id: u32) {
        stop_with_id(this as u32, new_script_id as u16);
    }

    #[detour(START)]
    unsafe extern "thiscall" fn start_detour(this: *const u32) {
        start(this as u32);
    }

    #[detour(CHECK_PENDING_SCRIPTS)]
    unsafe extern "thiscall" fn check_pending_scripts_detour(this: *const u32) {
        check_pending_scripts(this as u32);
    }

    #[detour(ADD_SCRIPT)]
    unsafe extern "thiscall" fn add_script_detour(this: *const u32, unit_type_id: u32, new_script_id: u16) -> bool {
        add_script(this as u32, unit_type_id, new_script_id)
    }

    #[detour(CHECK_SCRIPT)]
    unsafe extern "thiscall" fn check_script_detour(this: *const u32, param_1: u16) -> u32 {
        check_script(this as u32, param_1) as u32
    }

    #[detour(CALCULATE_PERCENT_ADJUSTMENT)]
    unsafe extern "fastcall" fn calculate_percent_adjustment_detour(this: *const u32) -> i32 {
        calculate_percent_adjustment(this as u32)
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshow detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// Allocates and zero-initializes a standalone, `0xa8`-byte `ZTShowInfo` buffer via the real vanilla
    /// allocator (`standalone::OPERATOR_NEW`) - **without** running the real `ZTShowInfo::ZTShowInfo` ctor,
    /// which unconditionally dereferences an unconfirmed `GLOBAL_ZTAIMgr` field (see `start`'s own doc
    /// comment on why that field is skipped project-wide). Real size `0xa8` on Windows, confirmed via
    /// `ZTHabitat_setIsShowExhibit.c`'s own `new(0xa8)` (macOS uses `0xb0` - a real per-platform struct-size
    /// difference, not a bug; this project builds for Windows).
    ///
    /// The one piece of real-ctor state this buffer *does* need is the pending-scripts tree header at
    /// `+0x44`: `find_or_insert_pending_script_node`/`plan_pending_node_insert` dereference `header+4`
    /// (root) and `header+8` (leftmost), so `+0x44` must hold a valid, writable pointer, not null.
    ///
    /// **Does not point `+0x44` at `show_info` itself.** An earlier version of this function wrote
    /// `*(show_info+0x44) = show_info+0x44`, treating the header as embedded in place - modeled on
    /// `AI_cls_0x404fd6`'s self-referencing-sentinel pattern, but that pattern describes the header
    /// object's own internal layout, not where it lives. `ztshowinfo-pending-scripts-tree-plan.md`'s
    /// live investigation of a *real* `ZTShowInfo` disproves the embedded reading directly: `header`'s
    /// value (read from `this+0x44`) did **not** equal `this+0x44`, proving the header is a genuinely
    /// separate allocation for real objects. Aliasing it onto `show_info` here made `header+4` land on
    /// `show_info+0x48` - a real, distinct `ZTShowInfo` field (`increment_pending_script_node_count`'s
    /// save-format node count). The very first `find_or_insert_pending_script_node` insert would then
    /// increment that field by reading the tree's own justwritten root pointer as if it were a small
    /// integer count and writing back `root_pointer + 1`, corrupting the root into an unmapped address -
    /// live-reproduced via `crash-capture`: an access violation on `mov edx,[ecx+0x10]` with `ecx` a
    /// bogus, non-heap value once the corrupted "root" was dereferenced on the next lookup.
    ///
    /// Fixed by allocating the header as its own, separate `0xc`-byte block (`self`/`root`/`leftmost` -
    /// the three fields `find_or_insert_pending_script_node`'s Rust logic actually reads/writes; real
    /// vanilla's own `AI_cls_0x404fd6::find` is never called through this buffer, so no larger real-struct
    /// size is needed here) and storing *that* block's address at `show_info+0x44` - the same
    /// separate-allocation shape `reimplementation_tests/mod.rs`'s `ZTSHOWMGR_ENTER_NEW_MONTH` test already
    /// uses for its own `init_pending_script_header` helper. Every other field of `show_info` stays zeroed,
    /// matching what the real ctor's own explicit zero-writes to `+0x50..+0xa4` already produce (see
    /// `ZTShowInfo_ZTShowInfo.c`) - the fields this buffer's zero-init does *not* match the real ctor for
    /// are `+0x68` (real ctor sets `1`) and the `GLOBAL_ZTAIMgr`-derived `+0x6c`/`+0x70` fields, none of
    /// which `add_script`/`check_pending_scripts`/`check_owning_habitat` (this buffer's intended consumers)
    /// ever read.
    ///
    /// Deliberately never freed by a matching helper here: this is a one-shot smoke-test buffer (see
    /// `reimplementation_tests/mod.rs`'s `ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE` test), and real
    /// vanilla `ADD_SHOW`/`REMOVE_SHOW` may link further real-allocator-owned memory into its
    /// `+0x50`/`+0x54`/`+0x58` dynamic array by the time the test finishes - freeing only the outer buffer
    /// while leaving that memory dangling would be a partial/incorrect teardown, so per CLAUDE.md's
    /// leak-only-teardown precedent (`ztthoughtmgr.rs`'s `destroy_standalone_mgr_leaking_nodes`), this
    /// buffer, its separately-allocated header block, and anything real vanilla code links into it are just
    /// left allocated for the rest of the (short, one-shot) test process's lifetime instead.
    pub(crate) fn build_standalone_show_info() -> u32 {
        let show_info = unsafe { OPERATOR_NEW.original()(0xa8) } as u32;
        unsafe { std::ptr::write_bytes(show_info as *mut u8, 0, 0xa8) };

        let header = unsafe { OPERATOR_NEW.original()(0xc) } as u32;
        unsafe { std::ptr::write_bytes(header as *mut u8, 0, 0xc) };
        save_to_memory(header, header);
        save_to_memory(show_info + 0x44, header);

        show_info
    }

    /// Thin wrapper around the module's own private [`super::collect_pending_script_nodes`] (already
    /// visible here via `use super::*` - no visibility change needed on the original) - an in-order walk
    /// of `show_info`'s own pending-scripts tree (`+0x44`), for
    /// `ZTSHOWINFO_PENDING_SCRIPT_TREE_STRESS_LIVE`'s own final ascending-key-order assertion.
    pub(crate) fn collect_pending_script_nodes(show_info: u32) -> Vec<u32> {
        let header = get_from_memory::<u32>(show_info + 0x44);
        let root = get_from_memory::<u32>(header + 4);
        let mut nodes = Vec::new();
        super::collect_pending_script_nodes(root, &mut nodes);
        nodes
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Doesn't depend on `ztshowscriptmgr`'s shared process-global store (see that module's own
    /// `TEST_LOCK` precedent for why cross-test interference is a real concern there) - exercises just
    /// the "script id not found" path, which `check_unit_type` handles independently of any registration
    /// state.
    #[test]
    fn check_unit_type_returns_true_with_low_byte_set_when_script_not_found() {
        let mut fake_show_info = [0u8; 0x10];
        fake_show_info[8..10].copy_from_slice(&0xfffeu16.to_le_bytes());
        let this = fake_show_info.as_ptr() as u32;
        assert_eq!(check_unit_type(this, 42) & 1, 1);
    }

    /// Uses the shared `ztshowscriptmgr` store, so needs its `SHOW_SCRIPT_STORE_TEST_LOCK`/
    /// `reset_store_for_test` precedent to avoid cross-test interference under `cargo test`'s default
    /// parallel execution (see that module's own doc comments on both).
    #[test]
    fn check_script_falls_back_to_own_field_and_rejects_missing_id() {
        let _guard = crate::ztshowscriptmgr::SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        crate::ztshowscriptmgr::reset_store_for_test();
        const CTOR_PTR: u32 = 0x9000;
        let id = crate::ztshowscriptmgr::register_script(CTOR_PTR, 1).unwrap();
        crate::ztshowscriptmgr::add_item(CTOR_PTR, &crate::ztshowscriptmgr::test_item(1, 5));

        let mut fake_show = [0u8; 0x8];
        fake_show[4..6].copy_from_slice(&id.to_le_bytes());
        let this = fake_show.as_ptr() as u32;

        assert!(check_script(this, 0), "param_1==0 should fall back to this+0x4's own script id");
        assert!(!check_script(this, 0xffff), "a non-existent script id should return false");
    }

    /// Synthetic closures over a shared, shrinking `Vec<i32>` "item id" list, modeling
    /// [`validate`]'s own positional index semantics: `remove` actually removes the entry at that
    /// position, shifting later items down - the same shape [`crate::ztshowscriptmgr::remove_item`]'s real
    /// `Vec::remove` produces. Marks the items originally at positions 1 and 3 (ids 11, 13, both odd)
    /// invalid; every other id is even.
    #[test]
    fn run_validate_loop_removes_invalid_items_and_revisits_shifted_position() {
        let items = std::cell::RefCell::new(vec![10, 11, 12, 13, 14]);
        let removed = std::cell::RefCell::new(Vec::new());
        let count = items.borrow().len() as i32;

        let result = run_validate_loop(
            count,
            |index| if items.borrow()[index as usize] % 2 != 0 { 1 } else { 0 },
            |index| {
                let id = items.borrow_mut().remove(index as usize);
                removed.borrow_mut().push(id);
            },
        );

        assert_eq!(result, 0);
        assert_eq!(*removed.borrow(), vec![11, 13]);
        assert_eq!(*items.borrow(), vec![10, 12, 14]);
    }

    /// Fixed stand-in for one script-state map node, laid out at the same `+0x8`/`+0xc`/`+0x10`/`+0x14`
    /// (left/right/key/value) offsets [`find_script_state_node`]/[`get_show_script_state`] read.
    #[repr(C)]
    struct FakeStateNode {
        _unused: [u8; 8],
        left: u32,
        right: u32,
        key: u32,
        value: u32,
    }

    #[test]
    fn get_show_script_state_returns_zero_for_empty_tree() {
        let mut show = [0u8; 0x38];
        let header = [0u8; 0x18];
        let header_addr = header.as_ptr() as u32;
        save_to_memory(show.as_mut_ptr() as u32 + 0x34, header_addr);
        let this = show.as_ptr() as u32;
        assert_eq!(get_show_script_state(this, 0), 0);
        assert_eq!(get_show_script_state(this, 0xffff_ffff), 0);
    }

    #[test]
    fn get_show_script_state_finds_single_node_and_misses_other_keys() {
        let mut show = [0u8; 0x38];
        let mut header = [0u8; 0x18];
        let node = FakeStateNode { _unused: [0; 8], left: 0, right: 0, key: 7, value: 0xdead_beef };
        let node_addr = &node as *const FakeStateNode as u32;
        save_to_memory(header.as_mut_ptr() as u32 + 4, node_addr);
        let header_addr = header.as_ptr() as u32;
        save_to_memory(show.as_mut_ptr() as u32 + 0x34, header_addr);
        let this = show.as_ptr() as u32;

        assert_eq!(get_show_script_state(this, 7), 0xdead_beef);
        assert_eq!(get_show_script_state(this, 6), 0);
        assert_eq!(get_show_script_state(this, 8), 0);
    }
}

#[cfg(test)]
mod pending_node_plan_tests {
    use super::*;

    /// Fixed stand-in for one pending-scripts tree node, laid out at the same `+0x8`/`+0xc`/`+0x10`
    /// (left/right/key) offsets [`plan_pending_node_insert`] reads - real node fields beyond that (up to
    /// the real `0x48`-byte size) are irrelevant to the pure decision logic under test, so this is
    /// intentionally smaller.
    #[repr(C)]
    struct FakeNode {
        _unused: [u8; 8],
        left: u32,
        right: u32,
        key: u32,
    }

    /// Owns a header buffer plus every node handed to [`Self::push_node`], keeping them alive (and their
    /// addresses stable) for the duration of a test - `get_from_memory`/`save_to_memory` are plain
    /// `ptr::read`/`ptr::write`, so real heap addresses of these Rust allocations work exactly like real
    /// game memory addresses would.
    struct Arena {
        // 0x10 bytes, not the tree header's real 0xc: the vanilla successor-walk this module's own
        // `vanilla_inorder_walk` test replica transliterates reads one field past the header
        // (`header+0xc`, the real game's *separate*, unrelated `ZTShowInfo+0x50` field per
        // `find_or_insert_pending_script_node`'s own doc comment) as part of its climb-termination check -
        // a too-small buffer here reads out of bounds instead of the deterministic zero a real adjacent
        // field would (usually) provide.
        header: Box<[u8; 0x10]>,
        nodes: Vec<Box<FakeNode>>,
    }

    impl Arena {
        /// Matches real vanilla `AI_cls_0x404fd6`'s own constructor: an empty tree's `leftmost` cache is
        /// self-referential (`header+8 == &header`), not null - confirmed via `ZTShowInfo_save.c`'s own
        /// empty-tree check (`iVar3 != iVar2` where both start as the header's self-pointer). Getting this
        /// wrong crashes [`vanilla_inorder_walk`] on an empty tree (reads from address `0xc` instead of
        /// returning immediately) rather than silently mis-testing anything - caught by
        /// `vanilla_walk_empty_tree` itself the first time this was tried without the fixup.
        fn new() -> Self {
            let mut arena = Arena { header: Box::new([0u8; 0x10]), nodes: Vec::new() };
            let header = arena.header_addr();
            save_to_memory(header + 8, header);
            arena
        }

        fn header_addr(&self) -> u32 {
            self.header.as_ptr() as u32
        }

        fn push_node(&mut self, key: u32, left: u32, right: u32) -> u32 {
            let node = Box::new(FakeNode { _unused: [0; 8], left, right, key });
            let addr = node.as_ref() as *const FakeNode as u32;
            self.nodes.push(node);
            addr
        }

        fn set_root(&mut self, root: u32) {
            save_to_memory(self.header_addr() + 4, root);
        }

        fn set_leftmost(&mut self, leftmost: u32) {
            save_to_memory(self.header_addr() + 8, leftmost);
        }
    }

    #[test]
    fn empty_tree_plans_new_root() {
        let arena = Arena::new();
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 5), PendingNodeInsertPlan::NewRoot);
    }

    #[test]
    fn single_node_smaller_key_plans_insert_right() {
        let mut arena = Arena::new();
        let root = arena.push_node(5, 0, 0);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 10), PendingNodeInsertPlan::InsertRight { parent: root });
    }

    #[test]
    fn single_node_larger_key_plans_insert_left_as_new_leftmost() {
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(
            plan_pending_node_insert(arena.header_addr(), 5),
            PendingNodeInsertPlan::InsertLeft { parent: root, parent_is_leftmost: true }
        );
    }

    #[test]
    fn exact_key_match_is_found() {
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 10), PendingNodeInsertPlan::Found(root));
    }

    #[test]
    fn insert_left_when_parent_is_not_the_current_leftmost() {
        // root(10) with a right child (15); leftmost is root itself (no left child exists yet).
        // Searching for 12 descends right into 15, then left off 15 (15 has no left child) -> InsertLeft
        // with parent=15, which is NOT the header's leftmost (root, key 10).
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        let right_child = arena.push_node(15, 0, 0);
        save_to_memory(root + 0xc, right_child);
        arena.set_root(root);
        arena.set_leftmost(root);
        assert_eq!(
            plan_pending_node_insert(arena.header_addr(), 12),
            PendingNodeInsertPlan::InsertLeft { parent: right_child, parent_is_leftmost: false }
        );
    }

    #[test]
    fn insert_right_after_descending_left_then_right() {
        // root(10) with a left child (3), which is also the leftmost. Searching for 7 descends left into
        // 3 (candidate=root, since 10>=7), then right off 3 (3<7, no right child) -> InsertRight with
        // parent=3.
        let mut arena = Arena::new();
        let root = arena.push_node(10, 0, 0);
        let left_child = arena.push_node(3, 0, 0);
        save_to_memory(root + 8, left_child);
        arena.set_root(root);
        arena.set_leftmost(left_child);
        assert_eq!(plan_pending_node_insert(arena.header_addr(), 7), PendingNodeInsertPlan::InsertRight { parent: left_child });
    }

    /// Full mutation (not just the pure decision) for [`Arena`]-backed test nodes: mirrors
    /// [`super::find_or_insert_pending_script_node`]'s real memory writes exactly (parent pointer, child
    /// slot, leftmost cache), just allocating via [`Arena::push_node`] instead of real vanilla
    /// `OPERATOR_NEW`. Returns the node address (new or existing).
    fn insert(arena: &mut Arena, key: u32) -> u32 {
        let header = arena.header_addr();
        match plan_pending_node_insert(header, key) {
            PendingNodeInsertPlan::Found(node) => node,
            PendingNodeInsertPlan::NewRoot => {
                let node = arena.push_node(key, 0, 0);
                save_to_memory(node + 4, header);
                arena.set_root(node);
                arena.set_leftmost(node);
                node
            }
            PendingNodeInsertPlan::InsertLeft { parent, parent_is_leftmost } => {
                let node = arena.push_node(key, 0, 0);
                save_to_memory(node + 4, parent);
                save_to_memory(parent + 8, node);
                if parent_is_leftmost {
                    arena.set_leftmost(node);
                }
                node
            }
            PendingNodeInsertPlan::InsertRight { parent } => {
                let node = arena.push_node(key, 0, 0);
                save_to_memory(node + 4, parent);
                save_to_memory(parent + 0xc, node);
                node
            }
        }
    }

    /// Faithful, unmodified transliteration of real vanilla `ZTShowInfo::save`'s in-order tree walk
    /// (`ZTShowInfo_save.c` lines 63-136: the standard Dinkumware/MSVC `_Tree::_Inc` successor algorithm -
    /// climb via the parent pointer when there's no right child, otherwise descend to the right subtree's
    /// leftmost node), starting from the header's leftmost cache and stopping when the walk returns to the
    /// header itself. This exists to answer one question empirically rather than by further manual
    /// decompile arithmetic (already a source of at least one wrong turn on this same investigation - see
    /// `ztshow-save-corruption-investigation.md`): does this algorithm visit every node of a tree built via
    /// [`super::find_or_insert_pending_script_node`], for a variety of insertion orders? If it silently
    /// visits **fewer** nodes than were inserted, that's byte-for-byte the same shape as the live symptom
    /// this investigation is chasing (`ZTHabitatMgr`'s own reload consuming 76,598 real file-read calls
    /// instead of 211, immediately downstream of an under-sized node count read back from disk).
    fn vanilla_inorder_walk(header: u32) -> Vec<u32> {
        let mut visited = Vec::new();
        let mut node = get_from_memory::<u32>(header + 8); // leftmost
        if node == header {
            return visited;
        }
        loop {
            visited.push(node);
            let right = get_from_memory::<u32>(node + 0xc);
            if right != 0 {
                node = right;
                loop {
                    let left = get_from_memory::<u32>(node + 8);
                    if left == 0 {
                        break;
                    }
                    node = left;
                }
            } else {
                let mut parent = get_from_memory::<u32>(node + 4);
                if node == get_from_memory::<u32>(parent + 0xc) {
                    loop {
                        node = parent;
                        parent = get_from_memory::<u32>(node + 4);
                        if node != get_from_memory::<u32>(parent + 0xc) {
                            break;
                        }
                    }
                }
                if get_from_memory::<u32>(node + 0xc) != parent {
                    node = parent;
                }
            }
            if node == header {
                break;
            }
        }
        visited
    }

    fn keys_of(arena: &Arena, nodes: &[u32]) -> Vec<u32> {
        nodes.iter().map(|&n| get_from_memory::<u32>(n + 0x10)).collect()
    }

    #[test]
    fn vanilla_walk_visits_every_node_ascending_insert() {
        let mut arena = Arena::new();
        for key in [1, 2, 3, 4, 5, 6, 7, 8] {
            insert(&mut arena, key);
        }
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn vanilla_walk_visits_every_node_descending_insert() {
        let mut arena = Arena::new();
        for key in [8, 7, 6, 5, 4, 3, 2, 1] {
            insert(&mut arena, key);
        }
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![1, 2, 3, 4, 5, 6, 7, 8]);
    }

    #[test]
    fn vanilla_walk_visits_every_node_mixed_insert_order() {
        let mut arena = Arena::new();
        for key in [50, 25, 75, 10, 30, 60, 90, 5, 15, 27, 40, 55, 65, 80, 95] {
            insert(&mut arena, key);
        }
        let mut expected: Vec<u32> = vec![50, 25, 75, 10, 30, 60, 90, 5, 15, 27, 40, 55, 65, 80, 95];
        expected.sort();
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), expected);
    }

    #[test]
    fn vanilla_walk_after_new_node_becomes_new_overall_rightmost() {
        // Reproduces the live scenario this investigation is chasing: a tree already containing several
        // nodes (standing in for tricks loaded from a real file, built via real vanilla's own insert) gets
        // exactly one more node added afterwards (standing in for a single "Add" click during the live
        // session), with the new key larger than everything already present.
        let mut arena = Arena::new();
        for key in [10, 20, 30, 40] {
            insert(&mut arena, key);
        }
        insert(&mut arena, 999);
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![10, 20, 30, 40, 999]);
    }

    #[test]
    fn vanilla_walk_single_node_tree() {
        let mut arena = Arena::new();
        insert(&mut arena, 42);
        let visited = vanilla_inorder_walk(arena.header_addr());
        assert_eq!(keys_of(&arena, &visited), vec![42]);
    }

    #[test]
    fn vanilla_walk_empty_tree() {
        let arena = Arena::new();
        assert_eq!(vanilla_inorder_walk(arena.header_addr()), Vec::<u32>::new());
    }
}
