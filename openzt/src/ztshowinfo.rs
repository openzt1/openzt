//! `ZTShowInfo` reimplementation - a sibling module to `ztshow.rs`/`ztshowstate.rs` (per the decision
//! recorded in `ztshowstate.rs`'s module doc comment that later `ZTShowInfo` work gets its own
//! `ztshowinfo.rs`), not an extension of either existing module. Covers the seven status predicates
//! (`isReady`, `isStarted`, `isStopped`, `hasKeeper`, `needsKeeper`, `getScheduledShowKeeperType`,
//! `getScheduledShowScript`), the two attendance/receipts accumulators (`incrementAttendance`,
//! `incrementReceipts`), the schedule/frequency pair (`setShowFrequency`/`recalculateSchedule`), and
//! (Stage 5) the show registration lifecycle group: `addShow`/`removeShow` (the registered-unit-types
//! array at `this+0x50..0x58`) and `createDefaultScript` (builds a fresh script from a unit type's real
//! trick list). `getShowSpeciesList`/`validateTrick` are also Stage 5's scope but deliberately left
//! un-detoured - see [`scheduled_species_ids`]'s and `ztshowui.rs`'s `validate_trick`'s own doc comments
//! for why.
//!
//! Continues the established free-function style for this class family: plain functions over a raw
//! `this: u32` (`ZTShowInfo*`), no owning `#[repr(C)]` struct, `get_from_memory`/`save_to_memory` at
//! literal offsets. None of these methods is a vtable slot (see the plan's own vtable slot table).
//!
//! ## `getScheduledShowScript` is not a pure field read
//! Despite the plan's own summary calling this group "seven pure field-read methods, no mutation",
//! `getScheduledShowScript` (`ZTShowInfo_getScheduledShowScript.c`/`.asm`, both read in full) actually
//! walks - and can insert into - the pending-scripts tree at `this+0x44`, the exact same tree `ztshow.rs`'s
//! [`crate::ztshow::find_or_insert_pending_script_node`]/`check_pending_scripts`/`add_script` already own.
//! Real vanilla reads the show's currently-scheduled unit-type id off a schedule array
//! (`this+0x50`..`this+0x54`, indexed by the current slot at `this+0xa4` - the same fields
//! `setShowFrequency`/`recalculateSchedule` below maintain), then looks that id up in the pending-scripts
//! tree; on a **miss** it calls the tree's own real insert helper (`AI_cls_0x404fd6::meth_0x5abe74`,
//! confirmed by `ztshow.rs`'s own doc comment on `find_or_insert_pending_script_node` to be that exact
//! tree's insert routine, and the thing that increments `this+0x48`'s save-format node count on every
//! genuine insert) via two short-lived temporary list/vector objects that are constructed and immediately
//! destructed again inside this one call (never touching `this` itself beyond the tree insert) - then
//! returns the (possibly freshly-inserted, all-zero) node's `+0x1c` field (`current` script id, `u16`).
//! This port reuses [`crate::ztshow::find_or_insert_pending_script_node`] directly instead of
//! reverse-engineering `meth_0x5abe74`/its STL glue (`BFTile::cls_0x40143b`,
//! `__vector_pod<>::__vector_pod<>?`) from raw `.asm` - the same risk/benefit tradeoff `ztshow.rs`'s own
//! doc comment already made for this identical tree, and it already increments the node count via that
//! same shared helper, so no separate handling is needed here. Only the low 16 bits of the real return
//! value are ever meaningful to any known caller (`isReady`/`isStarted` below each only read a specific
//! byte/word derived from it), so this returns `u16` rather than faithfully reproducing the decompile's
//! `CONCAT22`-with-undefined-upper-bits shape.
//!
//! **Side effect ported faithfully, not dropped**: [`is_ready`]/[`is_started`]/[`is_stopped`] (which calls
//! both) each call [`get_scheduled_show_script`] for its own return value, in vanilla's own call order -
//! so the pending-scripts-tree insert-on-first-schedule side effect fires exactly when real vanilla's
//! would, including `isStopped`'s double call (once via each of `isReady`/`isStarted` - a redundant-looking
//! but faithful call shape, confirmed via `.asm`).
//!
//! ## `getScheduledShowKeeperType`'s `0x2550` sentinel and animal-type check
//! `ZTShowInfo_getScheduledShowKeeperType.c`/`.asm`: resolves the same schedule-array unit-type id
//! `getScheduledShowScript` reads (`this+0x50`/`this+0x54`/`this+0xa4`, independently re-read here - real
//! vanilla does not call through to `getScheduledShowScript`, confirmed via `.asm`), looks up its
//! `BFEntityType*` via real `BFWorldMgr::getType` (`bfworldmgr::GET_TYPE`, `.original()` - never detoured
//! by this module), then runs the exact same "is this an animal-ish type" vtable-slot-`0x1c` check
//! `ztshow::stop_with_id`/`start` already use ([`crate::ztshow::type_check`]/
//! [`crate::ztshow::RVA_ANIMAL_TYPE_CHECK`], both bumped to `pub(crate)` for this module's reuse rather
//! than duplicated). `0x2550` (a magic constant that recurs elsewhere in this class family, e.g.
//! `ztshow::stop_with_id`) is returned only when the type check passes, `0` otherwise (including when no
//! unit type is currently scheduled, or `getType` returns null).
//!
//! ## `hasKeeper`/`needsKeeper` call through to real, still-un-ported `getNumUnits`
//! Both call the real `ztshowinfo::GET_NUM_UNITS` via `.original()` (Stage 7's own scope, not this one) -
//! `ztshow.rs` already establishes this exact call-through pattern for the same address.
//!
//! ## `incrementAttendance`/`incrementReceipts`
//! Both accumulators (`ZTShowInfo_incrementAttendance.asm`/`.c`, `ZTShowInfo_incrementReceipts.asm`/`.c`;
//! the real guest-admission caller `ZTBuilding_addUser` invokes them per entering guest as
//! `incrementReceipts(admission_price)` then `incrementAttendance(1)`) look the show's unit-type id up in
//! the same pending-scripts tree `getScheduledShowScript` uses - keyed by `this+0xc` directly (the same
//! key every other tree consumer uses, e.g. real `getMonthsOperating`), not via the schedule array - then
//! bump two per-node counters plus two `ZTShowInfo`-level totals each: integer pairs for attendance
//! (`node+0x34`/`node+0x3c`, `this+0x7c`/`this+0x84`), float pairs for receipts (`node+0x28`/`node+0x30`,
//! `this+0x94`/`this+0x9c`). Real vanilla re-runs the identical tree lookup once per counter - collapsed
//! into one [`crate::ztshow::find_or_insert_pending_script_node`] call here, same justification as
//! `ztshow.rs`'s `add_script` (the tree's structure cannot change between the redundant lookups). The
//! lookup is find-**or-insert**, and the insert side effect is ported faithfully through the shared
//! helper: a `this+0xc` id with no node yet gets one (zeroed, save-format node count at `this+0x48`
//! incremented), exactly as the real body's own `AI_cls_0x404fd6::meth_0x5abe74` insert does. Receipts'
//! real body accumulates through x87 `FLD`/`FADD`/`FSTP` on 32-bit floats - a Rust `f32` add is the same
//! single-precision round-to-nearest operation, so the results are bit-identical.
//!
//! ## `setShowFrequency`/`recalculateSchedule`
//! The scheduling pair (`ZTShowInfo_setShowFrequency.c`/`.asm`, `ZTShowInfo_recalculateSchedule.c`/
//! `.asm`, all four read in full). Field map, all `ZTShowInfo`-relative: `+0x68` holds the configured
//! frequency (`i32`, `-1` = the "never" sentinel), `+0x6c` the derived next-eligible time, and
//! `+0x50`/`+0x54`/`+0xa4` the schedule array and current slot already described above.
//! [`recalculate_schedule`] advances the slot cursor `(slot + 1) % element_count` - only when its
//! boolean argument says so: real vanilla's callers split cleanly along that flag (`setShowFrequency`
//! and `ZTShow::stop_with_id`'s tail pass false; `ZTShow::update`'s finished-show path and the 0-arg
//! `ZTShow::stop` pass true, per the callers' own decompiles) - skipping the advance entirely when the
//! schedule array is empty - and then recomputes `+0x6c` as `GLOBAL_ZTAIMgr`'s global AI-time cursor
//! (field `+0xec` on the singleton, deref of the global first - the same field the goal/keeper AI
//! compares stored deadlines against, and `ZTShowInfo_init.c` seeds `+0x6c` from) plus the frequency,
//! or pins it to `-1` for the "never" sentinel. The advance's raw asm is `INC` + `CDQ`/`IDIV`: no
//! overflow trap on the increment and a truncating signed remainder, exactly
//! [`i32::wrapping_add`] followed by Rust's `%` on `i32`. [`set_show_frequency`] is a no-op when the
//! frequency is unchanged - deliberately observable, even the `+0x6c` refresh is skipped - and on a
//! transition into `-1` while the show is **not** started calls real, still-un-ported `ZTShow::
//! abortShow` on the `ZTShow` embedded at `this+0x4` (`generated::ztshow::ABORT_SHOW` via
//! `.original()` - never detoured by this module; its body sends event `0x2717` then dispatches the
//! embedded show's *own* vtable `stop` slot, so it needs a genuinely-constructed `ZTShow`, not this
//! module's zeroed standalone fixtures). The sibling `isStarted` call on that same branch goes through
//! this module's own [`is_started`] - the same code the detoured `IS_STARTED` address runs, matching
//! how vanilla's internal call lands on this module's detour once installed. `set_show_frequency`'s
//! non-sentinel path and `recalculate_schedule`'s `+0x6c` recompute both read the live
//! `GLOBAL_ZTAIMgr`, so only the sentinel paths are host-unit-testable (see the `#[cfg(test)]`
//! module); the rest is covered by `ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE`.
//!
//! ## Deliberately no fabricated-buffer unit test for `hasKeeper`/`needsKeeper`/`getScheduledShowKeeperType`
//! Unlike `isReady`/`isStarted`/`isStopped`/`getScheduledShowScript` (whose only real-memory dependency,
//! the pending-scripts tree, can be safely modeled with plain Rust-allocated fixture memory for the
//! tree-hit case, or skipped entirely for the empty-schedule case), these three call through real vanilla
//! functions (`GET_TYPE.original()`/`GET_NUM_UNITS.original()`) and read a live `GLOBAL_ZTWorldMgr` -
//! meaningless outside an injected, running game process. Matches this codebase's established convention
//! (`ztshow.rs`'s own `#[cfg(test)]` module never unit-tests a function that itself calls `.original()`) -
//! these three get live coverage only, in `reimplementation_tests`, and specifically in `live_zoo_tests`
//! (needs a real, populated `GLOBAL_ZTWorldMgr`) rather than `always_late_tests` (where the tree-only four
//! live).
//!
//! ## Stage 6 - the event system (4 of `ZTShowInfo`'s 9 vtable slots)
//! `sendEvent` (`+0x0`), `getEvents` (`+0x8`), `listen` (`+0x10`), `cleanupEvents` (`+0x14`) - all four
//! read in full from `ZTShowInfo_sendEvent.c`/`.asm`, `_getEvents.c`/`.asm`, `_listen.c`/`.asm`,
//! `_cleanupEvents.c`/`.asm`.
//!
//! **The plan's own `+0xc` "unresolved slot" note was stale, not a real gap.** `getEvents`'s own body
//! forwards to `this`'s own vtable slot `+0xc` - the plan looked for that address only inside
//! `generated.rs`'s `ztshowinfo` module and didn't find it there, but it exists under `bfunit` as
//! `GET_EVENTS_3` (`0x0043f40d`, confirmed identical address): `ZTShowInfo` doesn't override `+0xc`, it
//! inherits `BFUnit::getEvents` unchanged. [`get_events`] below reproduces the real **virtual** dispatch
//! (reads `this`'s own vtable pointer, calls slot `+0xc` through it) rather than hardcoding a call to
//! `bfunit::GET_EVENTS_3`'s address directly - so it keeps working correctly even though the callee is
//! never itself detoured, the same "dispatch through the vtable, don't assume the target" precedent
//! `ztshowmgr.rs`'s own `ZTShowMgr::update` doc comment already establishes for its `+0x20` call.
//!
//! **`generated.rs`'s `ztshowinfo::GET_EVENTS` entry has the wrong signature** - `fn(*const u32)`, one
//! param, no others. Its own `.asm` (`RET 0x4`) proves it actually takes one stack argument beyond `this`
//! (confirmed independently against `bfunit::GET_EVENTS_3`'s own `.asm`, `RET 0xc` - three stack args
//! beyond its own unused `this`, the same value forwarded on unchanged). Per `CLAUDE.md`'s "never hand-edit
//! `generated.rs`, surface it instead" rule, [`GET_EVENTS_FIXED`] is a local, corrected `FunctionDef` for
//! the same address (`0x0059e8c7`) via the sanctioned `FunctionDef::new` stopgap - **flagging this for a
//! future `generated.rs` regeneration pass**, not a substitute for one.
//!
//! **`getEvents`'s real argument, per `.asm`**: `(this, arg, this->field_0x70 as u16, 0x53)` - `arg` is
//! whatever the caller passed in (`listen` passes `&this->field_0x5c`, the event vector's own address);
//! `field_0x70` is this instance's own AI-event target id; `0x53` is a fixed "ZTShowInfo" class tag that
//! recurs identically in `sendEvent`'s own forwarded args below. None of these three downstream hops
//! (`ZTShowInfo::getEvents` -> `BFUnit::getEvents` -> `GLOBAL_ZTAIMgr`'s own embedded sub-object's vtable)
//! is reimplemented here - each is a pure pointer-chase forward with no allocation and no vanilla-owned
//! memory write, so faithfully reproducing the **dispatch mechanics** (which slot, which args, in which
//! order) is sufficient; the AI manager's own internals stay untouched, out of scope, matching this class
//! family's established "port the orchestration, call through to a still-real sibling" precedent (see
//! `ztshowinfo-implementation-plan.md`'s Stage 9 note on `ZTShow::run`).
//!
//! **`sendEvent`'s forward, per `.c`** (`ZTShowInfo_sendEvent.c`; the `.asm`'s own arg-staging is just
//! spill/reload mechanics around the same eight values, not a different arg count - the outer function's
//! own `RET 0x18` independently confirms six stack args, matching `generated.rs`'s already-correct
//! `SEND_EVENT` signature): when `GLOBAL_ZTAIMgr` is non-null, dispatches through the **embedded
//! sub-object at `GLOBAL_ZTAIMgr+0x8`**'s own vtable slot `+0x4` (`this` for that call is
//! `GLOBAL_ZTAIMgr+0x8` itself, not `GLOBAL_ZTAIMgr`), forwarding `(event_id, unused, category,
//! this->field_0x70, 0x53, value, value2, flag)` - the caller's own six args with `field_0x70`/`0x53`
//! spliced in, same shape as `getEvents`'s own forward above.
//!
//! **`listen`, per `.asm`** (the `.c`'s own call to `getEvents` silently drops the pushed `&this->field_0x5c`
//! argument it's decompiled right next to - not trusted; see the module's own established "asm over c"
//! preference): calls [`get_events`] with `this+0x5c`'s own address, re-reads `this+0x5c`/`this+0x60` fresh
//! *after* that call (the callee may have repopulated the vector), then walks `0x1c`-byte event records
//! between them. A record matches when its own `+0xc` dword equals `this->field_0x70`; on a match, event id
//! `0x2714` (`+0x16` word) dispatches through the **embedded `ZTShow`'s own vtable `+0x14` slot with zero
//! explicit args** (confirmed via `.asm` - no `PUSH` between the vtable read and the `CALL`, unlike the
//! `.c`'s own fabricated `(puVar2)` argument, the same class of decompiler-invented-parameter error already
//! seen and corrected in `ztshowscriptmgr-implementation-plan.md`'s history); that slot resolves to the
//! real, deliberately-un-detoured function `stop_with_id`'s own doc comment already names
//! (`ztshow::STOP_1`/`0x005a85e9` - `generated.rs`'s naming, not `private/docs/vtables/ZTShow.md`'s
//! swapped "start"/"stop" labels for the same two addresses; that doc comment's own audit is reused
//! directly here rather than re-derived). Event id `0x2716` calls real, un-ported `ZTShow::abortShow`
//! (`ztshow::ABORT_SHOW`, `.original()` - already the established call-through this module's own
//! `set_show_frequency` uses for the same address).
//!
//! **`cleanupEvents` reduces to `this->field_0x60 = this->field_0x5c`** (vector `end = begin`, i.e. `size =
//! 0`, capacity/begin untouched) - **not** the elaborate division-and-compaction loop the raw `.asm` also
//! contains. Two independent signals agree the loop is dead code, never reached: (1) the macOS `.c`
//! decompile (a differently-inlined build of the exact same source) names the call plainly -
//! `std::vector<BFEvent>::_deleter::clear()`, i.e. a real `clear()`; the Windows `.c`'s own decompiler
//! *removes* the loop as three separate "unreachable block" warnings; (2) hand-tracing the Windows `.asm`'s
//! own division-by-stride setup shows it literally computing `esi = field_0x60 - field_0x60` (`MOV ESI,EDI;
//! SUB ESI,EDI`, both reads of the *same* just-loaded `field_0x60` value, never `field_0x5c`) - always
//! zero regardless of the vector's real contents, so the `JG` guarding the compaction loop can never take
//! it. [`cleanup_events`] below reproduces only the reachable `end = begin` result;
//! `ZTSHOWINFO_EVENT_SYSTEM_LIVE` exercises this live against real vanilla with a populated (non-empty,
//! Rust-owned scratch) event range specifically to empirically confirm the loop really is unreachable, not
//! just argued from static reading.
//!
//! **No fabricated-buffer unit tests for `send_event`/`get_events`/`listen`** - same reasoning as the
//! keeper-predicates section above: all three either dispatch through a real vtable pointer or read a live
//! `GLOBAL_ZTAIMgr`, meaningless outside the injected game process. Live coverage only
//! (`ZTSHOWINFO_EVENT_SYSTEM_LIVE`, `always_late_tests` - none of the three needs a populated
//! `GLOBAL_ZTWorldMgr`/live zoo, just a non-null `GLOBAL_ZTAIMgr`, which exists from process start).
//! [`cleanup_events`] is the one function in this group pure enough for a real fabricated-buffer unit test.

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bfworldmgr::GET_TYPE,
        standalone::{OPERATOR_DELETE, OPERATOR_NEW},
        ztshow::ABORT_SHOW,
        ztshowinfo::{
            ADD_SHOW, CLEANUP_EVENTS, CREATE_DEFAULT_SCRIPT, GET_NUM_UNITS, GET_SCHEDULED_SHOW_KEEPER_TYPE, GET_SCHEDULED_SHOW_SCRIPT,
            HAS_KEEPER, INCREMENT_ATTENDANCE, INCREMENT_RECEIPTS, IS_READY, IS_STARTED, IS_STOPPED, LISTEN, NEEDS_KEEPER,
            RECALCULATE_SCHEDULE, REMOVE_SHOW, SEND_EVENT, SET_SHOW_FREQUENCY,
        },
        ztshowscript::CONSTRUCTOR as ZTSHOW_SCRIPT_CONSTRUCTOR,
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, save_to_memory},
    ztshow::{collect_pending_script_nodes, find_or_insert_pending_script_node, type_check, RVA_ANIMAL_TYPE_CHECK},
    ztshowscriptmgr::{add_item, ZTShowScriptItemRaw},
    ztshowui::{find_trick_by_id, validate_trick, walk_trick_list},
};

/// Reads the show's currently-scheduled unit-type id off its schedule array (`this+0x50`..`this+0x54`, a
/// `u32` array, indexed by the current slot `this+0xa4`) - `0` if the array is empty. Confirmed
/// identically inlined in both `getScheduledShowScript`'s and `getScheduledShowKeeperType`'s own `.asm`
/// (independently - real vanilla does not call one from the other).
fn scheduled_unit_type_id(this: u32) -> u32 {
    let begin = get_from_memory::<u32>(this + 0x50);
    let end = get_from_memory::<u32>(this + 0x54);
    if begin == end {
        return 0;
    }
    let slot = get_from_memory::<u32>(this + 0xa4);
    get_from_memory::<u32>(begin + slot * 4)
}

/// Reimplementation of `ZTShowInfo::getScheduledShowScript` - see the module doc comment's own section
/// for why this is not a pure field read.
pub fn get_scheduled_show_script(this: u32) -> u16 {
    let unit_type_id = scheduled_unit_type_id(this);
    if unit_type_id == 0 {
        return 0;
    }
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    get_from_memory::<u16>(node + 0x1c)
}

/// Reimplementation of `ZTShowInfo::isReady`, per `ZTShowInfo_isReady.c`/`.asm`. `this+0x22` is a `u8`
/// flag; the real function's own call to `getScheduledShowScript` only ever feeds unused upper bytes of
/// its own decompiled return value, but is still called for its tree-insert side effect - see the module
/// doc comment.
pub fn is_ready(this: u32) -> bool {
    get_scheduled_show_script(this);
    get_from_memory::<u8>(this + 0x22) != 0
}

/// Reimplementation of `ZTShowInfo::isStarted`, per `ZTShowInfo_isStarted.c`/`.asm`. `this+0x23` is a
/// `u8` "currently running" flag; `this+0x8` is the currently-running script id (`u16`), compared against
/// [`get_scheduled_show_script`]'s own return value.
pub fn is_started(this: u32) -> bool {
    let scheduled = get_scheduled_show_script(this);
    if get_from_memory::<u8>(this + 0x23) == 0 {
        return false;
    }
    get_from_memory::<u16>(this + 0x8) == scheduled
}

/// Reimplementation of `ZTShowInfo::isStopped`, per `ZTShowInfo_isStopped.c`/`.asm`: `!isReady() &&
/// !isStarted()`, in that call order (matches vanilla's own short-circuit shape - `isStarted` is only
/// called when `isReady` returned false).
pub fn is_stopped(this: u32) -> bool {
    !is_ready(this) && !is_started(this)
}

/// Reimplementation of `ZTShowInfo::getScheduledShowKeeperType` - see the module doc comment's own
/// section.
pub fn get_scheduled_show_keeper_type(this: u32) -> u32 {
    let unit_type_id = scheduled_unit_type_id(this);
    let world = globals().ztworldmgr_ptr() as *const u32;
    let type_ptr = unsafe { GET_TYPE.original()(world, unit_type_id as i32) } as u32;
    if type_ptr != 0 && unsafe { type_check(type_ptr, RVA_ANIMAL_TYPE_CHECK) } {
        0x2550
    } else {
        0
    }
}

/// Reimplementation of `ZTShowInfo::hasKeeper`, per `ZTShowInfo_hasKeeper.c`/`.asm`.
pub fn has_keeper(this: u32) -> bool {
    let keeper_type = get_scheduled_show_keeper_type(this);
    let count = unsafe { GET_NUM_UNITS.original()(this as *const u32, keeper_type) };
    count > 0
}

/// Reimplementation of `ZTShowInfo::needsKeeper`, per `ZTShowInfo_needsKeeper.c`/`.asm`.
pub fn needs_keeper(this: u32, unit_type_id: u32) -> bool {
    let keeper_type = get_scheduled_show_keeper_type(this);
    if unit_type_id != keeper_type {
        return false;
    }
    let count = unsafe { GET_NUM_UNITS.original()(this as *const u32, unit_type_id) };
    if count >= 1 {
        return false;
    }
    is_ready(this)
}

/// Reimplementation of `ZTShowInfo::incrementAttendance`, per `ZTShowInfo_incrementAttendance.asm`/`.c`.
/// The real guest-admission caller (`ZTBuilding_addUser`) passes `1` per entering guest. Bumps both
/// per-node attendance counters and both `ZTShowInfo`-level totals - see the module doc comment's own
/// accumulator section for the field map and the collapsed-lookup/insert-side-effect notes. Real vanilla
/// does raw dword adds, so the `i32` arithmetic wraps deliberately (`wrapping_add` - never a debug-build
/// overflow panic).
pub fn increment_attendance(this: u32, amount: i32) {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, get_from_memory::<u32>(this + 0xc));
    save_to_memory(node + 0x34, get_from_memory::<i32>(node + 0x34).wrapping_add(amount));
    save_to_memory(node + 0x3c, get_from_memory::<i32>(node + 0x3c).wrapping_add(amount));
    save_to_memory(this + 0x7c, get_from_memory::<i32>(this + 0x7c).wrapping_add(amount));
    save_to_memory(this + 0x84, get_from_memory::<i32>(this + 0x84).wrapping_add(amount));
}

/// Reimplementation of `ZTShowInfo::incrementReceipts`, per `ZTShowInfo_incrementReceipts.asm`/`.c`.
/// Same tree lookup and insert-on-miss side effect as [`increment_attendance`], over the receipts field
/// pairs (`node+0x28`/`node+0x30`, `this+0x94`/`this+0x9c`) - see the module doc comment.
pub fn increment_receipts(this: u32, amount: f32) {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, get_from_memory::<u32>(this + 0xc));
    save_to_memory(node + 0x28, get_from_memory::<f32>(node + 0x28) + amount);
    save_to_memory(node + 0x30, get_from_memory::<f32>(node + 0x30) + amount);
    save_to_memory(this + 0x94, get_from_memory::<f32>(this + 0x94) + amount);
    save_to_memory(this + 0x9c, get_from_memory::<f32>(this + 0x9c) + amount);
}

/// Reimplementation of `ZTShowInfo::recalculateSchedule`, per `ZTShowInfo_recalculateSchedule.c`/`.asm` -
/// see the module doc comment's own schedule/frequency section for the field map and faithfulness notes.
/// `advance_slot` is real vanilla's `char` argument: `false` refreshes only `this+0x6c`, `true` first
/// advances the schedule-array cursor at `this+0xa4` (skipped when the array is empty).
pub fn recalculate_schedule(this: u32, advance_slot: bool) {
    if advance_slot {
        let count = (get_from_memory::<i32>(this + 0x54) - get_from_memory::<i32>(this + 0x50)) >> 2;
        if count > 0 {
            let slot = get_from_memory::<i32>(this + 0xa4).wrapping_add(1);
            save_to_memory(this + 0xa4, slot % count);
        }
    }
    let frequency = get_from_memory::<i32>(this + 0x68);
    if frequency == -1 {
        save_to_memory(this + 0x6c, -1i32);
    } else {
        let ai_mgr = globals().ztaimgr_ptr() as u32;
        save_to_memory(this + 0x6c, get_from_memory::<i32>(ai_mgr + 0xec).wrapping_add(frequency));
    }
}

/// Reimplementation of `ZTShowInfo::setShowFrequency`, per `ZTShowInfo_setShowFrequency.c`/`.asm` -
/// see the module doc comment's own schedule/frequency section for the unchanged-frequency
/// short-circuit, the abort call-through's own preconditions, and the reproduced post-recalculate
/// reread of `this+0x68`.
pub fn set_show_frequency(this: u32, frequency: i32) {
    if get_from_memory::<i32>(this + 0x68) == frequency {
        return;
    }
    save_to_memory(this + 0x68, frequency);
    recalculate_schedule(this, false);
    if get_from_memory::<i32>(this + 0x68) == -1 && !is_started(this) {
        unsafe { ABORT_SHOW.original()((this + 0x4) as *const u32) };
    }
}

/// Growth allocator for the show's registered-unit-types array (`this+0x50`/`+0x54`/`+0x58`) - a plain
/// cdecl helper (`ZTShowInfo_addShow.asm`'s own `PUSH bytes; CALL FUN_0040107f; ADD ESP,0x4`), has no
/// `generated.rs` entry. Declared locally per CLAUDE.md's sanctioned workaround (`FunctionDef::new`),
/// matching `zoostatus.rs`'s/`ztshowstate.rs`'s own precedent for a confirmed-but-unclaimed address.
const ALLOCATE_UNIT_ARRAY: FunctionDef<unsafe extern "cdecl" fn(u32) -> *const c_void> = FunctionDef::new(0x0040_107f);

/// Base of vanilla's shared small-object freelist bucket array, bucketed by `(byte_capacity - 1) >> 3` -
/// confirmed directly and completely in `ZTShowInfo_addShow.asm`'s own old-buffer-free tail (`DEC EAX;
/// SHR EAX,3; MOV EDX,[EAX*4+0x638000]; MOV [ECX],EDX; MOV [EAX*4+0x638000],ECX`) - a plain per-bucket
/// singly-linked freelist push, the same protocol `ztadvterrainmgr.rs`'s `release_bfpos_node`/
/// `ztshowstate.rs`'s `release_tree_node` already use for their own single-bucket freelists (unlike
/// those, this is a whole array of bucket heads, but the push mechanics per bucket are identical, and
/// read directly off *this exact function's own* disassembly rather than inferred from a different call
/// site - unlike `zoostatus.rs`'s own explicitly-unconfirmed hedge on this same freelist family). RVA =
/// `0x00638000 - 0x400000`.
const RVA_UNIT_ARRAY_FREELIST_BUCKETS: u32 = 0x0023_8000;

/// Frees a former `this+0x50` buffer of `byte_capacity` bytes back to wherever real vanilla's own
/// `addShow` would - see [`RVA_UNIT_ARRAY_FREELIST_BUCKETS`]'s doc comment. No-op for a null buffer.
fn free_unit_array_buffer(buf: u32, byte_capacity: u32) {
    if buf == 0 {
        return;
    }
    if byte_capacity > 0x80 {
        unsafe { OPERATOR_DELETE.original()(buf) };
        return;
    }
    let bucket_head_addr = get_module_base("zoo.exe") as u32 + RVA_UNIT_ARRAY_FREELIST_BUCKETS + (((byte_capacity - 1) >> 3) * 4);
    let old_head = get_from_memory::<u32>(bucket_head_addr);
    save_to_memory(buf, old_head);
    save_to_memory(bucket_head_addr, buf);
}

/// Reimplementation of `ZTShowInfo::addShow`, per `ZTShowInfo_addShow.c`/`.asm` (both read in full).
/// Appends `unit_type_id` to the show's own registered-unit-types array (`this+0x50..this+0x58`, a plain
/// `u32` array with `begin`/`end`/`cap_end` pointers - **not** the tree-of-lists unit roster at
/// `this+0x44`, an unrelated structure) unless it's already present or `unit_type_id == 0`.
///
/// On capacity growth: `new_count = max(count, 1) * 2` when `count != 0`, else `1` (pinned exactly via
/// the asm's own aliasing trick around `local_4`/`param_1` - a standard "double capacity, minimum 1"
/// vector growth), allocated via the real vanilla cdecl helper [`ALLOCATE_UNIT_ARRAY`] (never Rust's own
/// allocator - other real, un-ported vanilla code reads this exact buffer), old elements copied across,
/// `unit_type_id` appended, then the old buffer is freed via [`free_unit_array_buffer`]. The asm's own
/// generic "insert in the middle, shift the tail" second copy loop is provably dead code for this call
/// path specifically (its own trip count is `old_end - old_insertion_point`, and `addShow` always inserts
/// at `old_end` itself, i.e. always zero) - not reproduced.
pub fn add_show(this: u32, unit_type_id: u32) {
    if unit_type_id == 0 {
        return;
    }

    let begin = get_from_memory::<u32>(this + 0x50);
    let end = get_from_memory::<u32>(this + 0x54);
    let mut cursor = begin;
    while cursor != end {
        if get_from_memory::<u32>(cursor) == unit_type_id {
            return;
        }
        cursor += 4;
    }

    let cap_end = get_from_memory::<u32>(this + 0x58);
    if end != cap_end {
        if end != 0 {
            save_to_memory(end, unit_type_id);
        }
        save_to_memory(this + 0x54, end + 4);
        return;
    }

    let count = (end - begin) >> 2;
    let new_count = if count == 0 { 1 } else { count * 2 };
    let new_buf = unsafe { ALLOCATE_UNIT_ARRAY.original()(new_count * 4) as u32 };

    let mut src = begin;
    let mut dst = new_buf;
    for _ in 0..count {
        if dst != 0 {
            save_to_memory(dst, get_from_memory::<u32>(src));
            dst += 4;
        }
        src += 4;
    }
    if dst != 0 {
        save_to_memory(dst, unit_type_id);
        dst += 4;
    }

    free_unit_array_buffer(begin, cap_end - begin);

    save_to_memory(this + 0x50, new_buf);
    save_to_memory(this + 0x54, dst);
    save_to_memory(this + 0x58, new_buf + new_count * 4);
}

/// Reimplementation of `ZTShowInfo::removeShow`, per `ZTShowInfo_removeShow.c`/`.asm` (both read in
/// full). Erases the first `unit_type_id` match from the same array [`add_show`] appends to (no-op if
/// absent), shifting later elements down and shrinking `end` by one slot - no allocation on this path.
/// Also resets the schedule-slot cursor (`this+0xa4`) to `0` if it now falls at or past the shrunk
/// element count, matching real vanilla's own post-removal bounds check exactly (runs unconditionally,
/// whether or not a match was actually found).
pub fn remove_show(this: u32, unit_type_id: u32) {
    let begin = get_from_memory::<u32>(this + 0x50);
    let mut end = get_from_memory::<u32>(this + 0x54);
    let mut cursor = begin;
    while cursor != end {
        if get_from_memory::<u32>(cursor) == unit_type_id {
            let mut dst = cursor;
            let mut src = cursor + 4;
            while src != end {
                save_to_memory(dst, get_from_memory::<u32>(src));
                dst += 4;
                src += 4;
            }
            end -= 4;
            save_to_memory(this + 0x54, end);
            break;
        }
        cursor += 4;
    }

    let count = ((end as i32) - (begin as i32)) >> 2;
    if get_from_memory::<i32>(this + 0xa4) >= count {
        save_to_memory(this + 0xa4, 0i32);
    }
}

/// Show-script complexity budget (`Behavior/returnToKeeperThreshold`, `DAT_0063e4ac`) - the same
/// constant `ztshowui.rs`'s `copy_list_to_script` already reads, duplicated locally per this class
/// family's own established per-module-constant convention (see `ztshowui.rs`'s `global_bfuimgr` doc
/// comment).
const COMPLEXITY_BUDGET_RVA: u32 = 0x0023_e4ac;

/// The "sentinel" trick (real trick id `0x2c23`) auto-inserted whenever a script's running complexity
/// total reaches [`COMPLEXITY_BUDGET_RVA`] - same constant `ztshowui.rs`'s `SENTINEL_TRICK_ID` already
/// names, duplicated locally per this module's own constant convention.
const SENTINEL_TRICK_ID: u16 = 0x2c23;

fn dat(rva: u32) -> u32 {
    get_module_base("zoo.exe") as u32 + rva
}

/// Reimplementation of `ZTShowInfo::createDefaultScript`, per `ZTShowInfo_createDefaultScript.asm`
/// (Windows - the macOS `.c` decompile was also read for corroboration, but the two platforms disagree on
/// the constructed script's own byte size, `0x14` here vs `0x1c` there, so the Windows asm is
/// authoritative for this Windows-only project). Builds a fresh `ZTShowScript` (real vanilla
/// allocation/construction - the same `ztshowscript::CONSTRUCTOR` call `ztshowui.rs`'s
/// `copy_list_to_script` already uses, which itself calls through to Stage 1's real `REGISTER_SCRIPT`
/// detour, so the new script is registered into this crate's own store the instant it's constructed) for
/// `unit_type_id`, gated on `unit_type_id` resolving to a real `ZTAnimalType`-castable `BFEntityType*`
/// (real vanilla `BFWorldMgr::getType` + [`type_check`]/[`RVA_ANIMAL_TYPE_CHECK`], the same gate
/// [`get_scheduled_show_keeper_type`] already uses). Populates it from the unit type's own real trick
/// list ([`walk_trick_list`], a byte-identical deep copy of what `ZTUnitType::getTrickList` would
/// produce, per `ztshowui.rs`'s own doc comment - reading the master list directly sidesteps
/// reproducing that temporary-list machinery), keeping only tricks [`validate_trick`] accepts,
/// auto-inserting [`SENTINEL_TRICK_ID`] whenever the running complexity total (`item+0x44`) reaches
/// [`COMPLEXITY_BUDGET_RVA`] - the exact same accumulate/insert-sentinel/reset shape as `ztshowui.rs`'s
/// `copy_list_to_script`, confirmed against this function's own asm tail (`ADD EBP,[item+0x44]; CMP
/// EBP,DAT_0063e4ac; JGE insert_sentinel`). Returns `0` (matching real vanilla's own null return) if the
/// type gate or either allocation fails.
pub fn create_default_script(this: u32, unit_type_id: u32) -> u32 {
    let world = globals().ztworldmgr_ptr() as *const u32;
    let type_ptr = unsafe { GET_TYPE.original()(world, unit_type_id as i32) } as u32;
    if type_ptr == 0 || !unsafe { type_check(type_ptr, RVA_ANIMAL_TYPE_CHECK) } {
        return 0;
    }

    let alloc = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
    if alloc == 0 {
        return 0;
    }
    let script_ptr = unsafe { ZTSHOW_SCRIPT_CONSTRUCTOR.original()(alloc as *const u32, unit_type_id, true) } as u32;
    if script_ptr == 0 {
        return 0;
    }

    let budget = get_from_memory::<i32>(dat(COMPLEXITY_BUDGET_RVA)) as i64;
    let mut complexity_accum: i64 = 0;
    for item_ptr in walk_trick_list(type_ptr) {
        if !validate_trick(this, item_ptr) {
            continue;
        }
        let raw = unsafe { &*(item_ptr as *const ZTShowScriptItemRaw) };
        add_item(script_ptr, raw);

        complexity_accum += get_from_memory::<u32>(item_ptr + 0x44) as i64;
        if complexity_accum >= budget {
            if let Some(sentinel_ptr) = find_trick_by_id(type_ptr, SENTINEL_TRICK_ID) {
                let sentinel_raw = unsafe { &*(sentinel_ptr as *const ZTShowScriptItemRaw) };
                add_item(script_ptr, sentinel_raw);
            }
            complexity_accum = 0;
        }
    }

    script_ptr
}

/// Corrected `FunctionDef` for `ZTShowInfo::getEvents` (`0x0059e8c7`) - `generated.rs`'s own
/// `ztshowinfo::GET_EVENTS` entry declares zero stack arguments, but its `.asm` (`RET 0x4`) proves it
/// takes one - see the module doc comment's own Stage 6 section. Declared locally per `CLAUDE.md`'s
/// sanctioned `FunctionDef::new` stopgap, never hand-editing `generated.rs` itself.
pub(crate) const GET_EVENTS_FIXED: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32)> = FunctionDef::new(0x0059_e8c7);

/// Reimplementation of `ZTShowInfo::getEvents` - see the module doc comment's own Stage 6 section for the
/// `+0xc` slot resolution and the real forwarded-argument order. `arg` is the caller-supplied address
/// (`listen` passes `this+0x5c`, the event vector's own address) forwarded unchanged.
pub fn get_events(this: u32, arg: u32) {
    let field_0x70 = get_from_memory::<u16>(this + 0x70) as u32;
    let vtable = get_from_memory::<u32>(this);
    let slot = get_from_memory::<u32>(vtable + 0xc);
    let f: unsafe extern "thiscall" fn(u32, u32, u32, u32) = unsafe { std::mem::transmute(slot) };
    unsafe { f(this, arg, field_0x70, 0x53) };
}

/// Reimplementation of `ZTShowInfo::sendEvent`, per `ZTShowInfo_sendEvent.c`/`.asm` - see the module doc
/// comment's own Stage 6 section for the forwarded-argument order and the `GLOBAL_ZTAIMgr+0x8` embedded
/// sub-object dispatch target.
pub fn send_event(this: u32, event_id: u16, unused: u32, category: u8, value: u32, value2: u16, flag: u16) {
    let ai_mgr = globals().ztaimgr_ptr() as u32;
    if ai_mgr == 0 {
        return;
    }
    let target = ai_mgr + 0x8;
    let field_0x70 = get_from_memory::<u16>(this + 0x70);
    let vtable = get_from_memory::<u32>(target);
    let slot = get_from_memory::<u32>(vtable + 0x4);
    let f: unsafe extern "thiscall" fn(u32, u16, u32, u8, u16, u32, u32, u16, u16) = unsafe { std::mem::transmute(slot) };
    unsafe { f(target, event_id, unused, category, field_0x70, 0x53, value, value2, flag) };
}

/// Reimplementation of `ZTShowInfo::listen`, per `ZTShowInfo_listen.asm` (the `.c`'s own fabricated
/// `getEvents(puVar2)` call argument is not trusted - see the module doc comment's own Stage 6 section).
/// Walks the `0x1c`-byte-stride event array `get_events` (re-)populates, dispatching two specific event
/// ids for records whose own `+0xc` target id matches `this->field_0x70`.
pub fn listen(this: u32) {
    get_events(this, this + 0x5c);

    let target_id = get_from_memory::<u16>(this + 0x70) as u32;
    let mut cursor = get_from_memory::<u32>(this + 0x5c);
    if cursor == get_from_memory::<u32>(this + 0x60) {
        return;
    }
    loop {
        if get_from_memory::<u32>(cursor + 0xc) == target_id {
            match get_from_memory::<u16>(cursor + 0x16) {
                0x2714 => {
                    let ztshow = this + 0x4;
                    let vtable = get_from_memory::<u32>(ztshow);
                    let slot = get_from_memory::<u32>(vtable + 0x14);
                    let f: unsafe extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(slot) };
                    unsafe { f(ztshow) };
                }
                0x2716 => {
                    unsafe { ABORT_SHOW.original()((this + 0x4) as *const u32) };
                }
                _ => {}
            }
        }
        cursor += 0x1c;
        if cursor == get_from_memory::<u32>(this + 0x60) {
            break;
        }
    }
}

/// Reimplementation of `ZTShowInfo::cleanupEvents` - see the module doc comment's own Stage 6 section for
/// why the real function's own division-and-compaction loop is dead code, reducing to a plain `end =
/// begin` (vector size reset to `0`, capacity/begin untouched).
pub fn cleanup_events(this: u32) {
    let begin = get_from_memory::<u32>(this + 0x5c);
    save_to_memory(this + 0x60, begin);
}

/// Pure logic half of `ZTShowInfo::getShowSpeciesList` - the set of distinct unit-type ids from this
/// show's pending-scripts tree (`this+0x44`, [`collect_pending_script_nodes`]) that a caller would learn
/// about, per `ZTShowInfo_getShowSpeciesList.asm` (Windows - the tree walked is `this+0x44`, not the
/// macOS `.c`'s differently-offset `this+0x54`/`this+0x4c`, a real per-platform layout difference like
/// this class's own struct-size difference documented elsewhere in this module). Each node's key
/// (`node+0x10`) is included unless it's `0` (no unit type) or `0x2550` (the keeper-type sentinel this
/// same tree's nodes can also hold, per [`get_scheduled_show_keeper_type`]'s own doc comment) -
/// confirmed via the asm's own `CMP EDX,0x2550` gate.
///
/// **`GET_SHOW_SPECIES_LIST` itself is deliberately left un-detoured.** The real function's own job past
/// this filtering is inserting each surviving id into a *caller-owned* real `std::set<uint>` via the
/// private MSVC STL insert-with-hint primitive `tree::meth_0x4fbeee` - reconstructible with reasonable
/// confidence from two independent, cleanly-decompiled real call sites elsewhere in this corpus
/// (`BFMap::addShadow`, `ZTHabitat::addAmphibiousNeighbor`, both confirming a 4-explicit-arg
/// `(this, &out_iterator, 0, hint_node, &value)` shape), but this function's own asm additionally
/// requires reproducing its own internal BST search over the *output* set to compute that hint - a
/// second, separate tree walk this session did not invest in re-deriving with full confidence. Writing a
/// subtly-wrong argument into a real function that mutates caller-owned memory is a materially different
/// (and worse) risk than this module reading its own memory read-only. Since nothing ported by this plan
/// calls `getShowSpeciesList` (its only known real caller, `_addSpeciesFromHabitat`, is un-ported UI code
/// that keeps working exactly as before as long as this address stays un-hooked), this is deferred rather
/// than guessed at - matching Stage 8's own explicit narrowing-scope precedent for a similarly-uncertain
/// tree-mutation path.
pub(crate) fn scheduled_species_ids(this: u32) -> Vec<u32> {
    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);

    nodes
        .into_iter()
        .map(|node| get_from_memory::<u32>(node + 0x10))
        .filter(|&key| key != 0 && key != 0x2550)
        .collect()
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_SCHEDULED_SHOW_SCRIPT)]
    unsafe extern "thiscall" fn get_scheduled_show_script_detour(this: *const u32) -> u32 {
        get_scheduled_show_script(this as u32) as u32
    }

    #[detour(IS_READY)]
    unsafe extern "thiscall" fn is_ready_detour(this: *const u32) -> u32 {
        is_ready(this as u32) as u32
    }

    #[detour(IS_STARTED)]
    unsafe extern "thiscall" fn is_started_detour(this: *const u32) -> u32 {
        is_started(this as u32) as u32
    }

    #[detour(IS_STOPPED)]
    unsafe extern "thiscall" fn is_stopped_detour(this: *const u32) -> u32 {
        is_stopped(this as u32) as u32
    }

    #[detour(HAS_KEEPER)]
    unsafe extern "thiscall" fn has_keeper_detour(this: *const u32) -> u32 {
        has_keeper(this as u32) as u32
    }

    #[detour(NEEDS_KEEPER)]
    unsafe extern "thiscall" fn needs_keeper_detour(this: *const u32, unit_type_id: u32) -> u32 {
        needs_keeper(this as u32, unit_type_id) as u32
    }

    #[detour(GET_SCHEDULED_SHOW_KEEPER_TYPE)]
    unsafe extern "thiscall" fn get_scheduled_show_keeper_type_detour(this: *const u32) -> u32 {
        get_scheduled_show_keeper_type(this as u32)
    }

    #[detour(INCREMENT_ATTENDANCE)]
    unsafe extern "thiscall" fn increment_attendance_detour(this: *const u32, amount: i32) {
        increment_attendance(this as u32, amount);
    }

    #[detour(INCREMENT_RECEIPTS)]
    unsafe extern "thiscall" fn increment_receipts_detour(this: *const u32, amount: f32) {
        increment_receipts(this as u32, amount);
    }

    #[detour(SET_SHOW_FREQUENCY)]
    unsafe extern "thiscall" fn set_show_frequency_detour(this: *const u32, frequency: i32) {
        set_show_frequency(this as u32, frequency);
    }

    #[detour(RECALCULATE_SCHEDULE)]
    unsafe extern "thiscall" fn recalculate_schedule_detour(this: *const u32, advance_slot: i8) {
        recalculate_schedule(this as u32, advance_slot != 0);
    }

    #[detour(ADD_SHOW)]
    unsafe extern "thiscall" fn add_show_detour(this: *const u32, unit_type_id: u32) {
        add_show(this as u32, unit_type_id);
    }

    #[detour(REMOVE_SHOW)]
    unsafe extern "thiscall" fn remove_show_detour(this: *const u32, unit_type_id: u32) {
        remove_show(this as u32, unit_type_id);
    }

    #[detour(CREATE_DEFAULT_SCRIPT)]
    unsafe extern "thiscall" fn create_default_script_detour(this: *const u32, unit_type_id: i32) -> *const u32 {
        create_default_script(this as u32, unit_type_id as u32) as *const u32
    }

    #[detour(GET_EVENTS_FIXED)]
    unsafe extern "thiscall" fn get_events_detour(this: *const u32, arg: *const u32) {
        get_events(this as u32, arg as u32);
    }

    #[detour(SEND_EVENT)]
    unsafe extern "thiscall" fn send_event_detour(
        this: *const u32,
        event_id: u16,
        unused: u32,
        category: u8,
        value: u32,
        value2: u16,
        flag: u16,
    ) {
        send_event(this as u32, event_id, unused, category, value, value2, flag);
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn listen_detour(this: *const u32) {
        listen(this as u32);
    }

    #[detour(CLEANUP_EVENTS)]
    unsafe extern "thiscall" fn cleanup_events_detour(this: *const u32) {
        cleanup_events(this as u32);
    }

    /// `(name, is_enabled)` per detour - lets `reimplementation_tests`'s `ZTSHOWINFO_DETOURS_ENABLED`
    /// catch a silently-failed `init_detours()`, same rationale as `ztshowstate::detours::status`'s own
    /// doc comment.
    pub(crate) fn status() -> [(&'static str, bool); 18] {
        [
            ("GET_SCHEDULED_SHOW_SCRIPT", GET_SCHEDULED_SHOW_SCRIPT_DETOUR.is_enabled()),
            ("IS_READY", IS_READY_DETOUR.is_enabled()),
            ("IS_STARTED", IS_STARTED_DETOUR.is_enabled()),
            ("IS_STOPPED", IS_STOPPED_DETOUR.is_enabled()),
            ("HAS_KEEPER", HAS_KEEPER_DETOUR.is_enabled()),
            ("NEEDS_KEEPER", NEEDS_KEEPER_DETOUR.is_enabled()),
            ("GET_SCHEDULED_SHOW_KEEPER_TYPE", GET_SCHEDULED_SHOW_KEEPER_TYPE_DETOUR.is_enabled()),
            ("INCREMENT_ATTENDANCE", INCREMENT_ATTENDANCE_DETOUR.is_enabled()),
            ("INCREMENT_RECEIPTS", INCREMENT_RECEIPTS_DETOUR.is_enabled()),
            ("ADD_SHOW", ADD_SHOW_DETOUR.is_enabled()),
            ("REMOVE_SHOW", REMOVE_SHOW_DETOUR.is_enabled()),
            ("CREATE_DEFAULT_SCRIPT", CREATE_DEFAULT_SCRIPT_DETOUR.is_enabled()),
            ("SET_SHOW_FREQUENCY", SET_SHOW_FREQUENCY_DETOUR.is_enabled()),
            ("RECALCULATE_SCHEDULE", RECALCULATE_SCHEDULE_DETOUR.is_enabled()),
            ("GET_EVENTS", GET_EVENTS_FIXED_DETOUR.is_enabled()),
            ("SEND_EVENT", SEND_EVENT_DETOUR.is_enabled()),
            ("LISTEN", LISTEN_DETOUR.is_enabled()),
            ("CLEANUP_EVENTS", CLEANUP_EVENTS_DETOUR.is_enabled()),
        ]
    }
}

/// **Wiring note**: see `ztshowstate.rs`'s own `init()` doc comment - the same two-call-site rule applies
/// here (`lib.rs`'s boot cascade for the real game, `reimplementation_tests::init()` for the live
/// battery). Both must list this module.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshowinfo detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    /// See `detours::status()`'s own doc comment. Standalone `ZTShowInfo` construction reuses
    /// `crate::ztshow::live_support::build_standalone_show_info` rather than duplicating it - this
    /// module doesn't need its own instance builder.
    pub(crate) fn detour_status() -> [(&'static str, bool); 18] {
        super::detours::status()
    }

    /// `ZTShowInfo`'s own real vtable VA (`private/docs/vtables/ZTShowInfo.md` - `0x006353cc`, assigned to
    /// `this->vftptr_0x0` i.e. `this+0x0` by the real constructor). `ztshow::live_support::
    /// build_standalone_show_info`'s fixture zeroes this field (no prior Stage 1-5 port ever reads it) -
    /// Stage 6's [`super::get_events`]/[`super::listen`] are the first to need a genuinely valid vtable
    /// pointer here, so this installs one on an already-built standalone fixture before either is
    /// exercised live. Same RVA-from-VA data-address handling as `ztshowmgr.rs`'s own
    /// `ZTSHOWMGR_VTABLE_RVA`.
    const ZTSHOWINFO_VTABLE_RVA: u32 = 0x0023_53cc;

    pub(crate) fn install_vtable_pointer(show_info: u32) {
        crate::util::save_to_memory(show_info, crate::globals::get_module_base("zoo.exe") as u32 + ZTSHOWINFO_VTABLE_RVA);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::util::save_to_memory;

    /// `0xa8` matches real `ZTShowInfo`'s own size (`ztshow::live_support::build_standalone_show_info`'s
    /// own doc comment) - large enough to safely read/write every offset these functions touch, including
    /// `this+0xa4`.
    fn fake_show_info() -> [u8; 0xa8] {
        [0u8; 0xa8]
    }

    #[test]
    fn is_ready_reads_field_0x22_when_schedule_is_empty() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        assert!(!is_ready(this));
        save_to_memory(this + 0x22, 1u8);
        assert!(is_ready(this));
    }

    #[test]
    fn is_started_requires_flag_and_matching_current_script_id() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        // Schedule array is empty, so `get_scheduled_show_script` always returns 0 here.
        assert!(!is_started(this), "field_0x23==0 must be false regardless of this+0x8");

        save_to_memory(this + 0x23, 1u8);
        save_to_memory(this + 0x8, 5u16);
        assert!(!is_started(this), "this+0x8 (5) != scheduled (0) must be false");

        save_to_memory(this + 0x8, 0u16);
        assert!(is_started(this), "this+0x8 (0) == scheduled (0) must be true");
    }

    #[test]
    fn is_stopped_is_true_only_when_neither_ready_nor_started() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        assert!(is_stopped(this), "neither ready nor started -> stopped");

        save_to_memory(this + 0x22, 1u8);
        assert!(!is_stopped(this), "ready -> not stopped");

        save_to_memory(this + 0x22, 0u8);
        save_to_memory(this + 0x23, 1u8);
        save_to_memory(this + 0x8, 0u16); // matches scheduled==0 -> started
        assert!(!is_stopped(this), "started -> not stopped");
    }

    #[test]
    fn get_scheduled_show_script_returns_zero_when_schedule_is_empty() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        assert_eq!(get_scheduled_show_script(this), 0);
    }

    /// Models the pending-scripts tree's "found" path (no insert, so no real-allocator call) with a
    /// single fabricated header+node pair - safe plain-Rust-allocated fixture memory, same shape
    /// `ztshow.rs`'s own `pending_node_plan_tests` module uses for the identical tree layout. The BST
    /// walk itself is already covered there; this only checks this module's own wiring (schedule-array
    /// read -> tree lookup -> `+0x1c` field read).
    #[test]
    fn get_scheduled_show_script_reads_current_field_of_matching_tree_node() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x20];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node); // root = node
        save_to_memory(node + 0x10, 42u32); // key
        save_to_memory(node + 0x1c, 0x1234u16); // current script id

        let mut schedule_buf = [0u8; 4];
        let schedule = schedule_buf.as_mut_ptr() as u32;
        save_to_memory(schedule, 42u32); // scheduled unit-type id, matches the node's own key

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0x50, schedule);
        save_to_memory(this + 0x54, schedule + 4);
        save_to_memory(this + 0xa4, 0u32);

        assert_eq!(get_scheduled_show_script(this), 0x1234);
    }

    /// Found-path only, like the tree fixture above: the insert-on-miss branch runs through
    /// `find_or_insert_pending_script_node`'s real-allocator insert (`OPERATOR_NEW.original()`),
    /// meaningless outside the live process - covered live instead (`ZTSHOWINFO_ACCUMULATORS_LIVE`'s
    /// miss case, plus the tree-stress test that already exercises the shared helper's insert).
    #[test]
    fn increment_attendance_accumulates_into_node_counters_and_instance_totals() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x48];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node); // root = node
        save_to_memory(node + 0x10, 42u32); // key

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0xc, 42u32); // the show's unit-type id, matching the node's own key
        save_to_memory(node + 0x34, 100i32);
        save_to_memory(node + 0x3c, 200i32);
        save_to_memory(this + 0x7c, 300i32);
        save_to_memory(this + 0x84, 400i32);

        increment_attendance(this, 5);
        increment_attendance(this, 3);
        increment_attendance(this, -2);

        assert_eq!(get_from_memory::<i32>(node + 0x34), 106);
        assert_eq!(get_from_memory::<i32>(node + 0x3c), 206);
        assert_eq!(get_from_memory::<i32>(this + 0x7c), 306);
        assert_eq!(get_from_memory::<i32>(this + 0x84), 406);
        assert_eq!(get_from_memory::<u32>(this + 0x44), header, "found path must not restructure the tree");
        assert_eq!(get_from_memory::<u32>(this + 0x48), 0, "found path must not touch the save-format node count");
    }

    /// Real vanilla's raw dword adds cannot overflow-trap: a wrapped counter keeps accumulating from
    /// `i32::MIN`, matching `wrapping_add` (a debug-build panic here would be a port bug).
    #[test]
    fn increment_attendance_wraps_like_vanillas_raw_dword_add() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x48];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node);
        save_to_memory(node + 0x10, 7u32);

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0xc, 7u32);
        save_to_memory(this + 0x7c, i32::MAX);

        increment_attendance(this, 1);

        assert_eq!(get_from_memory::<i32>(this + 0x7c), i32::MIN);
    }

    #[test]
    fn increment_receipts_accumulates_float_fields() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x48];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node);
        save_to_memory(node + 0x10, 42u32);

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0xc, 42u32);
        save_to_memory(node + 0x28, 10.5f32);
        save_to_memory(node + 0x30, 20.25f32);
        save_to_memory(this + 0x94, 30.5f32);
        save_to_memory(this + 0x9c, 40.75f32);

        // 2.5 and 0.75 are exact binary fractions, so the accumulation is exact.
        increment_receipts(this, 2.5);
        increment_receipts(this, 0.75);

        assert_eq!(get_from_memory::<f32>(node + 0x28), 13.75);
        assert_eq!(get_from_memory::<f32>(node + 0x30), 23.5);
        assert_eq!(get_from_memory::<f32>(this + 0x94), 33.75);
        assert_eq!(get_from_memory::<f32>(this + 0x9c), 44.0);
        assert_eq!(get_from_memory::<u32>(this + 0x44), header, "found path must not restructure the tree");
    }

    /// Schedule/frequency tests below run only the `-1` "never" sentinel paths: the non-sentinel
    /// `this+0x6c` recompute reads the live `GLOBAL_ZTAIMgr`, meaningless outside the injected game -
    /// covered live by `ZTSHOWINFO_SCHEDULE_FREQUENCY_LIVE` instead (same split as the keeper
    /// predicates above).
    #[test]
    fn recalculate_schedule_advances_and_wraps_the_slot_cursor() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut schedule_buf = [0u32; 3];
        let schedule = schedule_buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, schedule);
        save_to_memory(this + 0x54, schedule + 12);
        save_to_memory(this + 0x68, -1i32);

        recalculate_schedule(this, true);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 1);
        recalculate_schedule(this, true);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 2);
        recalculate_schedule(this, true);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 0, "cursor must wrap modulo the 3-element schedule");

        recalculate_schedule(this, false);
        assert_eq!(get_from_memory::<i32>(this + 0xa4), 0, "advance_slot=false must not move the cursor");
        assert_eq!(get_from_memory::<i32>(this + 0x6c), -1, "sentinel frequency pins next-eligible to -1");
    }

    #[test]
    fn recalculate_schedule_skips_the_advance_when_the_schedule_is_empty() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x68, -1i32);

        recalculate_schedule(this, true);

        assert_eq!(get_from_memory::<i32>(this + 0xa4), 0, "empty schedule array must not advance the cursor");
        assert_eq!(get_from_memory::<i32>(this + 0x6c), -1);
    }

    /// Real vanilla's raw `INC`/`IDIV` cannot overflow-trap: a wrapped cursor keeps counting from
    /// `i32::MIN`, matching `wrapping_add` + `i32`'s truncating `%` (a debug-build panic here would be
    /// a port bug).
    #[test]
    fn recalculate_schedule_wraps_the_cursor_like_vanillas_inc_idiv() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut schedule_buf = [0u32; 3];
        let schedule = schedule_buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, schedule);
        save_to_memory(this + 0x54, schedule + 12);
        save_to_memory(this + 0xa4, i32::MAX);
        save_to_memory(this + 0x68, -1i32);

        recalculate_schedule(this, true);

        assert_eq!(get_from_memory::<i32>(this + 0xa4), i32::MIN % 3);
    }

    #[test]
    fn set_show_frequency_short_circuits_when_the_frequency_is_unchanged() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x68, 7i32);
        save_to_memory(this + 0x6c, 0x5au32);

        set_show_frequency(this, 7);

        assert_eq!(get_from_memory::<u32>(this + 0x6c), 0x5a, "unchanged frequency must skip even the +0x6c refresh");
    }

    /// `-1` while the show counts as started must stop at the sentinel refresh: real vanilla only
    /// aborts when `isStarted` is false. The zeroed fixture is trivially "started" here (empty schedule
    /// array -> scheduled script id 0, matching `this+0x8`'s zero, with `this+0x23` set) without ever
    /// touching the pending-scripts tree - which is also what keeps this host-testable, since the abort
    /// itself would call real vanilla's `abortShow`.
    #[test]
    fn set_show_frequency_to_never_while_started_stops_at_the_sentinel() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x23, 1u8);

        set_show_frequency(this, -1);

        assert_eq!(get_from_memory::<i32>(this + 0x68), -1);
        assert_eq!(get_from_memory::<i32>(this + 0x6c), -1);
    }

    /// Within-capacity path only (`end != cap_end`) - the growth path calls through real vanilla's own
    /// allocator ([`ALLOCATE_UNIT_ARRAY`]), meaningless outside the injected game - covered live instead
    /// (`ZTSHOWINFO_ADD_REMOVE_SHOW_LIVE`).
    #[test]
    fn add_show_appends_within_existing_capacity() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [0u32; 4];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr);
        save_to_memory(this + 0x58, array_addr + 16);

        add_show(this, 42);
        assert_eq!(get_from_memory::<u32>(array_addr), 42);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 4);

        add_show(this, 99);
        assert_eq!(get_from_memory::<u32>(array_addr + 4), 99);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 8);
    }

    #[test]
    fn add_show_skips_existing_duplicate_and_zero_id() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [7u32, 0, 0, 0];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr + 4);
        save_to_memory(this + 0x58, array_addr + 16);

        add_show(this, 7);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 4, "duplicate must not be appended");

        add_show(this, 0);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 4, "id 0 must be ignored");
    }

    #[test]
    fn remove_show_erases_match_and_shifts_later_elements() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [1u32, 2, 3, 4];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr + 16);
        save_to_memory(this + 0x58, array_addr + 16);
        save_to_memory(this + 0xa4, 0i32);

        remove_show(this, 2);

        assert_eq!(get_from_memory::<u32>(array_addr), 1);
        assert_eq!(get_from_memory::<u32>(array_addr + 4), 3);
        assert_eq!(get_from_memory::<u32>(array_addr + 8), 4);
        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 12);
    }

    #[test]
    fn remove_show_is_no_op_when_id_absent() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [1u32, 2];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr + 8);
        save_to_memory(this + 0x58, array_addr + 8);

        remove_show(this, 99);

        assert_eq!(get_from_memory::<u32>(this + 0x54), array_addr + 8);
        assert_eq!(get_from_memory::<u32>(array_addr), 1);
        assert_eq!(get_from_memory::<u32>(array_addr + 4), 2);
    }

    /// Matches `ZTShowInfo_removeShow.c`'s own unconditional post-removal bounds check: the slot cursor
    /// resets to `0` whenever it falls at or past the (possibly just-shrunk) element count.
    #[test]
    fn remove_show_resets_slot_cursor_when_it_falls_out_of_bounds() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut array = [1u32, 2];
        let array_addr = array.as_mut_ptr() as u32;
        save_to_memory(this + 0x50, array_addr);
        save_to_memory(this + 0x54, array_addr + 8);
        save_to_memory(this + 0x58, array_addr + 8);
        save_to_memory(this + 0xa4, 1i32);

        remove_show(this, 2);

        assert_eq!(get_from_memory::<i32>(this + 0xa4), 0);
    }

    /// Models the pending-scripts tree the same way `get_scheduled_show_script_reads_current_field_of_matching_tree_node`
    /// does - a small fabricated BST, no real-allocator interaction.
    #[test]
    fn scheduled_species_ids_filters_zero_and_sentinel_keys() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;

        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_a = [0u8; 0x20];
        let node_a_addr = node_a.as_mut_ptr() as u32;
        let mut node_b = [0u8; 0x20];
        let node_b_addr = node_b.as_mut_ptr() as u32;
        let mut node_zero = [0u8; 0x20];
        let node_zero_addr = node_zero.as_mut_ptr() as u32;
        let mut node_sentinel = [0u8; 0x20];
        let node_sentinel_addr = node_sentinel.as_mut_ptr() as u32;

        save_to_memory(header + 0x4, node_a_addr);
        save_to_memory(node_a_addr + 0x10, 10u32);
        save_to_memory(node_a_addr + 0x8, node_zero_addr);
        save_to_memory(node_a_addr + 0xc, node_b_addr);
        save_to_memory(node_zero_addr + 0x10, 0u32);
        save_to_memory(node_b_addr + 0x10, 20u32);
        save_to_memory(node_b_addr + 0x8, node_sentinel_addr);
        save_to_memory(node_sentinel_addr + 0x10, 0x2550u32);

        save_to_memory(this + 0x44, header);

        let mut ids = scheduled_species_ids(this);
        ids.sort();
        assert_eq!(ids, vec![10, 20]);
    }

    #[test]
    fn cleanup_events_resets_end_to_begin_without_touching_the_range_contents() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut records = [0xffu8; 0x1c * 2];
        let begin = records.as_mut_ptr() as u32;
        let end = begin + 0x1c * 2;
        save_to_memory(this + 0x5c, begin);
        save_to_memory(this + 0x60, end);

        cleanup_events(this);

        assert_eq!(get_from_memory::<u32>(this + 0x60), begin, "end must be reset to begin");
        assert_eq!(records, [0xffu8; 0x1c * 2], "the (dead) compaction loop must never touch the range's own bytes");
    }

    #[test]
    fn cleanup_events_is_a_no_op_on_an_already_empty_range() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x5c, 0x1000u32);
        save_to_memory(this + 0x60, 0x1000u32);

        cleanup_events(this);

        assert_eq!(get_from_memory::<u32>(this + 0x60), 0x1000);
    }
}
