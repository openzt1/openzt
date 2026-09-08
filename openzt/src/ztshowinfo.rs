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
//! ## `hasKeeper`/`needsKeeper` call this module's own [`get_num_units`] (Stage 7)
//! Both used to call through to real, un-ported `getNumUnits` via `.original()`; now that Stage 7 ports it
//! (see the module's own Stage 7 section below), they call [`get_num_units`] directly, a plain same-module
//! Rust call rather than detour indirection.
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
//! ## Deliberately no fabricated-buffer unit test for `hasKeeper`/`needsKeeper`/`getScheduledShowKeeperType`/`checkUnit`
//! Unlike `isReady`/`isStarted`/`isStopped`/`getScheduledShowScript`/[`get_num_units`]/[`get_show_unit_list`]
//! (whose only real-memory dependency, the pending-scripts tree, can be safely modeled with plain
//! Rust-allocated fixture memory for the tree-hit case, or skipped entirely for the empty-schedule case),
//! these four call through real vanilla functions (`GET_TYPE.original()`/`GET_UNIT.original()`) and read a
//! live `GLOBAL_ZTWorldMgr` - meaningless outside an injected, running game process (`hasKeeper`/
//! `needsKeeper` themselves no longer call `.original()` directly since Stage 7, but still transitively
//! depend on the live world through `getScheduledShowKeeperType`). Matches this codebase's established
//! convention (`ztshow.rs`'s own `#[cfg(test)]` module never unit-tests a function that itself calls
//! `.original()`) - these four get live coverage only, in `reimplementation_tests`, and specifically in
//! `live_zoo_tests` (needs a real, populated `GLOBAL_ZTWorldMgr`) rather than `always_late_tests` (where
//! the tree-only six - the four listed above plus [`get_num_units`]/[`get_show_unit_list`] - live).
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
//! **`generated.rs`'s `ztshowinfo::GET_EVENTS` entry originally had the wrong signature** - `fn(*const
//! u32)`, one param, no others, when its own `.asm` (`RET 0x4`) proved it actually took one stack argument
//! beyond `this`. Flagged per `CLAUDE.md`'s "never hand-edit `generated.rs`, surface it instead" rule via a
//! local, corrected `FunctionDef` stopgap at the time this stage was first ported; a later regeneration
//! pass has since fixed the real entry to `fn(*const u32, u32)` (address unchanged, `0x0059e8c7`), so this
//! module now imports it directly rather than keeping the stopgap. `bfunit::GET_EVENTS_3` (`0x0043f40d`) -
//! the sibling entry this module's own `get_events` never calls by name, since it dispatches virtually
//! instead (see above) - had the same class of bug (`fn(*const u32)` when its own `.asm`'s `RET 0xc` proved
//! three stack args); a further regeneration pass has since fixed that one too, to `fn(*const u32, u32,
//! u32, u32)`, matching this module's own independent derivation exactly.
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
//!
//! ## Stage 7 - unit-roster read-only queries
//! `getNumUnits`, `getShowUnitList`, `checkUnit` (`ZTShowInfo_getNumUnits.c`/`.asm`,
//! `_getShowUnitList.c`/`.asm`, `_checkUnit.c`/`.asm`, all read in full). **The plan's own "Known hazards"
//! section describes the unit roster as a structure distinct from the pending-scripts tree at `this+0x44`
//! - reading these three's own decompiles shows that's not so: it's the exact same tree.** Every node
//! already carries a `node+0x18` field ([`crate::ztshow::find_or_insert_pending_script_node`]'s own
//! `allocate_pending_script_node` allocates a real, self-referential sentinel there for every node it
//! creates - see that function's own doc comment on why a null `+0x18` would crash the first real vanilla
//! caller) holding an intrusive circular list of that unit type's assigned unit ids
//! (`{next:+0x0, prev:+0x4, payload:+0x8}`, confirmed via real `removeUnit`'s own unlink code - out of
//! scope until Stage 8) - `getNumUnits`/`getShowUnitList` only ever read that list; they never touch the
//! pending-scripts value fields (`+0x1c`/`+0x1e`/`+0x28`.../`+0x3c`) at all.
//!
//! [`get_num_units`]/[`get_show_unit_list`] both reuse
//! [`crate::ztshow::find_or_insert_pending_script_node`] for the same find-**or-insert** reason
//! [`get_scheduled_show_script`]/[`increment_attendance`] already do - real vanilla's own descent is the
//! identical BST walk, falling through to the tree's real insert helper only on a genuine miss. **The
//! insert-on-miss side effect is preserved faithfully, not dropped**, per the plan's own explicit warning
//! for this exact pair: a caller that queries an as-yet-unseen `unit_type_id` gets a freshly-inserted,
//! empty-list node back (count `0`, or a valid-but-empty list pointer), exactly like real vanilla, rather
//! than some sentinel "not found" value.
//!
//! `checkUnit` is unrelated to the tree - it resolves `unit_id` through real `BFWorldMgr::getUnit`
//! (`bfworldmgr::GET_UNIT`, `.original()` - never detoured by this module), gates on the exact same
//! trick-eligible-type check `ztshow.rs`'s `validate_item`/`do_current_item` already use (`DAT_006386b0`,
//! [`crate::ztshow::RVA_SHOW_TRICK_TYPE_CHECK`]/[`crate::ztmegatilemgr::entity_type_matches`] - confirmed
//! identical constant via each's own `.asm`), then forwards the eligible unit's own numeric unit-type id
//! (its type's own vtable slot `+0x20`, no args) into [`crate::ztshow::check_unit_type`] and returns that
//! call's raw `u32` result unchanged - **not** just a boolean low byte: real vanilla's own `checkUnitType`
//! can return `unit_type_id & 0xffffff00` (preserving upper byte bits) on a script-type mismatch, and
//! `checkUnit`'s own real callers (`ZTShow::validate`'s per-unit loop, already ported) compare the raw
//! result against `0`, not just its low bit.

//! ## Stage 8 - unit-roster mutation
//! `ZTShowInfo_addUnit.c`/`.asm`, `_addUnitToList.c`/`.asm`, `_removeUnit.c`/`.asm`, `_gatherUnits.c`/`.asm`
//! all read in full. Narrower scope than the plan's own Stage 8 originally described: only
//! [`remove_unit`] (erase-only) is ported; `addUnit`/`addUnitToList` and `ZTShowInfo`'s own `gatherUnits`
//! stay real and un-detoured, per the plan's own explicit permission to narrow this stage rather than ship
//! an under-verified allocator interaction. Both reasons below matured directly from reading their own
//! `.asm`, not from re-deriving the plan's own hedge in the abstract.
//!
//! **`addUnit`/`addUnitToList` deferred**: unlike every other insert this class family already ports
//! (the pending-scripts tree, via [`crate::ztshow::find_or_insert_pending_script_node`]'s own
//! reduced-to-an-unbalanced-BST substitute for `AI_cls_0x404fd6::meth_0x5abe74`), inserting a *unit* into a
//! node's own circular list (`node+0x18`) goes through a different, shared helper - `FUN_0040146c`,
//! confirmed via cross-reference (`private/resources/decompiles/*.c`) to be called from dozens of unrelated
//! classes across this whole corpus (`UILayout`, `ZTGuest`, `ZTWorldMgr`, `ZTShowScript`, `BFTile`, ...),
//! all funnelling into the *same* shared small-object freelist this module's own [`free_unit_array_buffer`]
//! already uses (`DAT_00638004` - confirmed to be the same base by [`remove_unit`]'s own read of its
//! erase-side counterpart, see that function's doc comment). No call site of `FUN_0040146c` has a
//! `generated.rs` entry of its own, and its own `.asm` (read at several call sites, e.g.
//! `BFTile_addUnit.asm`, `UICallbackMgr_addGlobalTrigger.asm`) marshals its arguments through several
//! layers of stack-temporary copies whose real calling convention this session could not pin down with
//! confidence - reproducing a subtly-wrong version of a helper this many unrelated systems share would risk
//! corrupting freelist state well outside this class's own scope, a materially worse failure mode than
//! leaving two methods un-ported. Calling the real address directly (the `ALLOCATE_UNIT_ARRAY`-style
//! workaround [`add_show`] already uses for a different *not-shared*, single-purpose helper) was considered
//! and rejected for the same reason: an ABI mistake here has blast radius far beyond `ZTShowInfo`.
//!
//! **`ZTShowInfo`'s own `gatherUnits` (`0x005a437d`) deferred**: the plan's own "Open items" flagged this
//! address as ambiguous against `ztshow::GATHER_UNITS` (`0x005a4519`, already called through by
//! [`crate::ztshow::do_current_item`]/friends). Reading both resolves the ambiguity - they're genuinely
//! different, both-real functions, and `ZTShow_gatherUnits.c` confirms the *chain*:
//! `ZTShow::gatherUnits` (`0x005a4519`, real, called via `.original()`) forwards straight into
//! `ZTShowInfo::gatherUnits` (`0x005a437d`, this class's own, still real/un-ported) as
//! `ZTShowInfo::gatherUnits(this->mbr_0x10, this->mbr_0x8)` - i.e. it *is* reachable, not dead weight.
//! But its own body is a materially harder problem than [`remove_unit`]'s erase: it drains a node's *entire*
//! unit list in one call (walking it twice - once to check an eligibility predicate per unit via real
//! `BFWorldMgr::getEntity` + a vtable-`0x1c` call, once to unlink-and-free every entry), via yet another
//! unconfirmed shared helper (`FUN_00404f0c`) neither this session nor any already-ported code in this
//! corpus has previously pinned down. Since the existing `ztshow::GATHER_UNITS.original()` call-through
//! already reaches this real function correctly, un-hooked, nothing in this crate needs
//! `ZTShowInfo::gatherUnits` itself ported for current behavior to keep working - matching this module's own
//! `getShowSpeciesList` precedent (Stage 5's section above) for deferring a real-but-uncalled-by-Rust method
//! rather than guessing at an unconfirmed allocator interaction.
//!
//! **`removeUnit`'s own real third parameter is a pointer-as-integer typing wart, not a genuine pointer -
//! and the one existing `.original()` call site this stage converts had a real bug because of it.**
//! `generated.rs` types it `*const i32`, but `ZTShowInfo_removeUnit.asm` never dereferences it anywhere -
//! it only compares the raw incoming stack dword directly against each list entry's own `+0x8` payload
//! field (a plain `u32` unit id, confirmed by [`crate::ztshow::stop_with_id`]'s own already-live-tested
//! `get_from_memory::<u32>(node + 0x8)` read of that same field). Real vanilla's own two confirmed callers
//! (`ZTHabitat_removeShowUnit.c`, `ZTKeeper_removeFromMap.c`) both pass the unit's own `+0x124` field
//! *value* directly, never its address - matching `addUnitToList`'s own insertion of that same value. The
//! one existing `.original()` call site this stage converts ([`crate::ztshow::stop_with_id`]'s
//! ineligible-unit branch) passed `&unit_id` - the address of a *local* stack variable - instead: since real
//! vanilla's comparison never dereferences that argument, no real list entry's payload (a small integer)
//! could ever have equalled a stack address, so that branch was a silent no-op that left ineligible units in
//! the roster instead of removing them. Fixed as part of converting that call site to call [`remove_unit`]
//! directly.
//!
//! [`remove_unit`] preserves real vanilla's own find-or-insert side effect on the type-id node (via
//! [`crate::ztshow::find_or_insert_pending_script_node`], same as every other reader/mutator of this tree)
//! and its own unconditional `ZTUI::showpanel::forceUpdate()` call (real, un-ported, on both the found and
//! not-found paths, matching the asm's own single shared exit label) - only the actual unlink-and-free
//! differs from a byte-for-byte trace, reproducing the doubly-linked unlink and the freelist push
//! ([`free_unit_array_buffer`], reused as-is: a `0xc`-byte capacity resolves to the exact same bucket
//! `removeUnit`'s own asm writes to, `DAT_00638004`) rather than the real body's own three-times-redundant
//! `AI_cls_0x404fd6::find`/`meth_0x5abe74` re-walks (the same redundant-lookup collapsing this module's own
//! accumulators/`checkUnit` already establish).
//!
//! **No fabricated-buffer unit test for `remove_unit`** - every path (found, not-found, and even the
//! find-or-insert side effect it starts with) ends in a real, un-detoured `ZTUI::showpanel::forceUpdate()`
//! call, meaningless outside the injected game process, matching this module's own established convention
//! for any function that calls `.original()` unconditionally. Live coverage only
//! (`ZTSHOWINFO_REMOVE_UNIT_LIVE`).
//!
//! ## Stage 9 - `enterNewMonth` / `update`
//! `ZTShowInfo_enterNewMonth.c`/`.asm`, `_update.c`/`.asm` (all four read in full). Both close with the
//! identical tail: sample a real "engagement" value keyed by `this->field_0x70` (the AI-event target id
//! [`send_event`]/[`get_events`]/[`listen`] already use) via [`FUN_0059E8F0`], store it at `this+0x88`,
//! and fold it into a running total at `this+0x90` - see [`apply_engagement_sample`]'s own doc comment for
//! the shared tail and [`FUN_0059E8F0`]'s own doc comment for why its real calling convention (not its
//! internal logic) is all that's reproduced here.
//!
//! **`enterNewMonth`'s own job**: roll every "current month" attendance/receipts accumulator - both
//! instance-level (`this+0x7c`/`this+0x94`) and per-node (pending-scripts tree, `node+0x34`/`node+0x28`) -
//! into its own "last month" slot, resetting the current-month accumulator to `0`; the "all-time" slots
//! ([`increment_attendance`]/[`increment_receipts`]'s own `+0x3c`/`+0x84`/`+0x30`/`+0x9c`) are untouched, and
//! neither instance total nor a real allocator call is involved. [`roll_monthly_totals`] is the pure half of
//! this (host-testable); [`enter_new_month`] itself only adds the live [`FUN_0059E8F0`] tail. The node loop
//! walks the pending-scripts tree via [`collect_pending_script_nodes`] rather than real vanilla's own
//! in-order successor algorithm (`.asm`'s own `_Tree::_Inc`-style walk, starting from the tree header's
//! `+0x8` leftmost cache) - the exact same "value-mutation-only, so an equivalent recursive collect is
//! safe" substitution [`collect_pending_script_nodes`]'s own doc comment already justifies for
//! `checkPendingScripts`, reused here rather than re-derived, since this loop also only ever mutates each
//! node's own value fields, never its structural pointers.
//!
//! **`update`'s own job** (vtable slot `+0x20`, `private/docs/vtables/ZTShowInfo.md`): three real
//! call-throughs in vanilla's own order - [`listen`] (this class's own, `+0x10`), the embedded `ZTShow`'s
//! own `update` (`ZTShow`'s own vtable slot `+0xc`, `0x0059e773` per `private/docs/vtables/ZTShow.md` -
//! real, un-ported; the plan's own "likely `ZTShow::run`" guess named the wrong method, the vtable doc's
//! own confirmed-slot table settles it), then [`cleanup_events`] (`+0x14`) - followed by `this+0x90 -=
//! this+0x88` (undoing the *previous* tick's own sample before folding in a fresh one, i.e. a decaying
//! running total rather than `enterNewMonth`'s own accumulate-then-archive), then the same
//! [`FUN_0059E8F0`]/[`apply_engagement_sample`] tail. The embedded `ZTShow::update` call-through is left
//! real and un-ported, matching this class family's established "port the orchestration, call through to a
//! still-real sibling" precedent (`ztshowinfo-implementation-plan.md`'s own Stage 9 note).
//!
//! **Live coverage split, not one shared test**: `enterNewMonth` never touches either object's own vtable
//! pointer, so `ZTSHOWINFO_ENTER_NEW_MONTH_LIVE` runs a full real-vs-rust field comparison on two ordinary
//! standalone fixtures. `update` unconditionally dispatches through the embedded `ZTShow`'s own `+0xc`
//! slot, and real vanilla's own callee there was never proven safe against anything but a genuinely
//! constructed `ZTShow` (the same hazard [`set_show_frequency`]'s own doc comment already flags for the
//! sibling `abortShow` call-through) - `ZTSHOWINFO_UPDATE_LIVE` stubs just that one slot to a harmless
//! no-op on both poles (confirmed via `.asm` to be the only slot `update` itself reads off the embedded
//! object) rather than either risking a crash or skipping the comparison outright, matching the plan's own
//! "accepting the same real-callee-call-through caveats `ZTAdvTerrainMgr`'s own tests document" allowance.
//!
//! ## Stage 10 - `setShowInfoID` reconciliation
//! [`set_show_info_id`] moves here from `ztshowmgr.rs`'s own `ZTShowMgr::set_show_info_id` (a private,
//! `register_show`-only helper introduced there before this module existed) and its real address
//! (`SET_SHOW_INFO_ID`, `0x005ab8c3`) is now detoured too, so any other real vanilla caller of
//! `setShowInfoID` - not just `ZTShowMgr::register_show`'s own internal call - gets the ported behavior.
//! `register_show` now calls [`set_show_info_id`] directly rather than keeping its own private copy. See
//! [`set_show_info_id`]'s own doc comment (carried over from the original `ZTShowMgr::set_show_info_id`
//! derivation) for the field map and the unreachable-through-any-current-caller return-0 path.
//!
//! ## Stage 11 - `save`/`load` (`updateFromLoad` deferred - see below)
//! Read `ztshowinfo-pending-scripts-tree-plan.md` and `ztshow-save-corruption-investigation.md` in full
//! before touching this section again - both documents are the live-verified ground truth for
//! `this+0x44`/`+0x48` (the pending-scripts tree header pointer and its *separate* cached node count,
//! respectively) this stage builds on rather than re-derives. [`show_info_save`]/[`show_info_load`] are
//! derived from `ZTShowInfo_save.c`/`.asm` and `_load.c`/`.asm` (both read in full - only `.c` exists for
//! neither, contrary to the plan's own stale note); the pending-scripts tree's own insert-with-node-count
//! side effect is untouched, reused as-is via [`find_or_insert_pending_script_node`], per the plan's own
//! instruction for this stage.
//!
//! **Field/write order** (`save`, unconditional, no version parameter - confirmed real vanilla `save` never
//! branches on a version at all, only `load` does): `+0x68` (frequency, 4B), `+0xa4` (schedule slot, 4B),
//! the registered-unit-types array's element count then every element (`+0x50..+0x54`, reusing
//! [`add_show`]'s own field layout), the pending-scripts tree's cached node count (`+0x48`, 4B) then every
//! node in ascending-key order (via [`collect_pending_script_nodes`], reused directly) - per node:
//! `+0x10`(key,4B)/`+0x1c`(2B)/`+0x1e`(2B)/`+0x24`(4B)/`+0x28`(4B)/`+0x2c`(4B)/`+0x30`(4B)/`+0x34`(4B)/
//! `+0x38`(4B)/`+0x3c`(4B)/`+0x40..+0x48`(FILETIME, 8B) - the exact order `ZTShowInfo_save.c` lines 61-120
//! write in; node `+0x20` is read into a scratch local but never actually written to file anywhere in the
//! real body (dead code, matching `+0x20`'s own established "unconfirmed flag, zeroed" status elsewhere in
//! this module) - then the `BFEvent` array's element count and each element's own real, un-ported
//! `BFEvent::save` (`.original()` - `BFEvent` itself is not reimplemented anywhere in this codebase), then
//! eleven more scalars in file order (`+0x6c`,`+0x70`,`+0x88`,`+0x8c`,`+0x90`,`+0x94`,`+0x98`,`+0x9c`,
//! `+0x7c`,`+0x80`,`+0x84`), then real, un-ported `ZTShow::save` on the embedded `ZTShow` at `this+0x4`
//! (`.original()` - this transparently picks up Stage 1's own `ZTShowState::save` port, since real vanilla's
//! own internal call to that address executes whatever is currently patched there, detour or not, the same
//! reasoning `CLAUDE.md`'s detouring section documents generally). Matches vanilla's own accumulate-and-
//! don't-early-return contract for every section except the `BFEvent` loop, which vanilla's own `.c`
//! explicitly early-returns from on the first failed element (reproduced here, the only early return in
//! [`show_info_save`]).
//!
//! **`load`'s version gates** (`ZTShowInfo_load.c`, confirmed against the `.asm`'s own literal compares -
//! `0x63`/`0x61`/`0x69`/`0x6a`, i.e. exactly what the `.c` renders as `99`/`0x60`/`0x68`/`0x6a`, just mixing
//! decimal/hex per Ghidra's own magnitude-based default): a save whose `version < 99` skips the per-node
//! `+0x1e`/`+0x24..+0x3c` reads entirely (using zeroed defaults, plus a live `ZTGameMgr::getDate` call for
//! the FILETIME pair, matching real vanilla's own pre-99 fallback) and, before the node loop even starts,
//! consumes (not persists into any field - a real vanilla "skip legacy data" quirk, reproduced faithfully
//! rather than "fixed") one more `u32` count of discarded `(u32,u32)` pairs; `version > 0x60` gates the
//! whole `BFEvent` array section plus `+0x6c`/`+0x70`; `version > 0x68` gates `+0x88`/`+0x8c`/`+0x90`;
//! `version < 0x6a` calls real, opaque [`SET_DEFAULT_SATISFACTION_FIELDS`] instead of reading
//! `+0x94`/`+0x98`/`+0x9c`/`+0x7c`/`+0x80`/`+0x84` from file. Matches `save`'s own single early-return
//! location (inside the `BFEvent` loop) and nowhere else.
//!
//! **`version < 99`'s own `+0x1e` ("pending") gap, left as real vanilla leaves it**: real vanilla's own
//! per-node scratch buffer only has its `+0x1c` ("current") bytes explicitly read from file in this branch -
//! `+0x1e` is never written at all before being copied into the node, so real vanilla's own behavior here is
//! whatever uninitialized stack bytes happen to be present, not a meaningful value. This port does not
//! attempt to reproduce that non-determinism (nothing could); [`PendingScriptRecord`]'s own version-gated
//! constructor leaves `pending` at [`find_or_insert_pending_script_node`]'s already-zeroed
//! [`allocate_pending_script_node`] default instead, matching this module's established "zero harmlessly
//! where real vanilla's own value is genuinely undefined" convention (see this file's own `+0x0`/`+0x14`/
//! `+0x20`/`+0x24` field notes elsewhere).
//!
//! **`BFEvent` array growth substitution**: real vanilla's own incremental during-read growth
//! (`FUN_0046b931`/`FUN_004f2629`, both opaque - no decompile or independently-resolved address for either
//! anywhere in this repo) is not reproduced. Since the final element count is already known from the file's
//! own leading count field (unlike vanilla's own per-element incremental design), [`show_info_load`] instead
//! allocates one buffer sized to the exact final count up front (via the same [`ALLOCATE_UNIT_ARRAY`]
//! cdecl helper `save`... i.e. `addShow`'s own growth helper, confirmed identical via `ZTShowInfo_load.asm`'s
//! own `FUN_0040107f` call inside the `BFEvent` growth branch too) and placement-constructs + loads each
//! element directly into its final slot (`bfevent::CONSTRUCTOR` then `bfevent::LOAD`, both `.original()` -
//! `BFEvent` is not reimplemented). This produces the same final array content and `begin`/`end`/`cap_end`
//! state as vanilla's own algorithm without needing either unconfirmed helper - the same "equivalent end
//! state via a simpler algorithm" substitution this class family already uses for the pending-scripts tree's
//! own unbalanced-BST insert.
//!
//! **`updateFromLoad` is deliberately deferred, left real and un-detoured.** Its own body
//! (`ZTShowInfo_updateFromLoad.c`, read in full) copy-constructs the embedded `ZTShow` sub-object at
//! `this+0x4` via `cls_0x485448::cls_0x485448(&this->field_0x4, &param_1->field_0x4)` - an opaque helper
//! with no decompile, no independently-resolved address, and no other call site anywhere in this corpus to
//! cross-check against. `ZTShow` embeds `ZTShowState` (Stage 1) at its own `+0x18`, whose `+0x1c` field is a
//! *heap pointer* to that instance's own script-state tree header - if `cls_0x485448` performs anything
//! resembling a naive memberwise copy over that offset range (impossible to rule out without its own
//! decompile), `this` would end up aliasing `param_1`'s tree header pointer directly. `param_1` is always the
//! `ZTHabitatMgr::load` stack temporary that gets destructed immediately after `updateFromLoad` returns (see
//! `ztshowinfo-pending-scripts-tree-plan.md`'s own lifecycle table) - an aliased pointer would leave `this`
//! holding a dangling reference into memory that destructor may free, the same class of cross-allocator/
//! dangling-pointer hazard `CLAUDE.md`'s own section on this warns about, and genuinely not something to
//! guess at. This is the same "narrow the stage's own scope rather than ship an unverified aliasing/allocator
//! interaction" call Stage 8 already made for `addUnit`/`addUnitToList` - surfaced here rather than resolved,
//! per `CLAUDE.md`'s "surface it to the user" instruction for a `generated.rs`-adjacent gap this session
//! can't close confidently. [`show_info_save`]/[`show_info_load`] do not depend on `updateFromLoad` in any
//! way (they operate on a single object, never two), so this gap does not block either of them.
//!
//! ## Stage 12 (closing stage) - constructors / destructor
//! `CONSTRUCTOR_0`/`CONSTRUCTOR_1` (`ZTShowInfo_ZTShowInfo_0.c`/`_1.c`), `ZTSHOW_INFO_0`/`ZTSHOW_INFO_1`
//! (the vtable-slot-`+0x18` deleting destructor and its plain body, `ZTShowInfo_~ZTShowInfo_0.c`/`.asm`,
//! `_~ZTShowInfo_1.c`, all read in full) - confirmed and left real, un-detoured, exactly the plan's own
//! predicted outcome for this closing stage.
//!
//! **Nothing here needs a Rust-owned reimplementation.** Real vanilla code (`ZTHabitat::
//! setIsShowExhibit` and friends) must keep constructing new `ZTShowInfo` instances via these same real
//! addresses - this class's whole reimplementation style (see this module's own decision record in the
//! plan) never introduces an owning Rust struct for `ZTShowInfo`, so there is no Rust-side construction
//! path to redirect either overload onto. `CONSTRUCTOR_0` (the copy constructor - `param_1` is a source
//! `ZTShowInfo*`, not an `int`, despite `generated.rs`'s own `i32` typing; the same pointer-as-integer
//! wart already documented for `remove_unit`'s third parameter) deep-copies the `BFEvent` array via
//! per-element `BFEvent::cls_0x63556c` and the registered-unit-types array via `FUN_0040107f`
//! (allocate)/`FUN_00401118` (copy), and reconstructs the pending-scripts tree wrapper at `+0x44` via
//! `AI_cls_0x404fd6::cls_0x404fd6` over the source's own `+0x38`/`+0x44` state - three more opaque,
//! allocator-touching helpers with no independently-resolved address or decompile anywhere in this
//! corpus, the same class of hazard Stage 8 already deferred `addUnit`/`addUnitToList` for and Stage 11
//! already deferred `updateFromLoad` for. `CONSTRUCTOR_1` (the default ctor) is comparatively simple -
//! zeroes most fields, calls real `setShowFrequency` with a fixed `0x5a` (90), and constructs the same
//! `AI_cls_0x404fd6`-wrapped pending-scripts tree header in place at `+0x44` - but that last construction
//! is exactly the one real allocation this module cannot safely reproduce with `Box`/Rust's own
//! allocator (the destructor below frees it through real vanilla's own small-object machinery), so
//! porting even the "simple" overload would leave a cross-allocator hazard at the one field every other
//! method in this module already depends on being real. Both overloads restore `this->vftptr_0x0` to
//! `ZTShowInfo`'s own vtable (`0x006353cc`) as their very last write - the same "vtable temporarily
//! borrowed during base-class construction, restored on exit" shape already familiar from this class
//! family's other constructors.
//!
//! **The destructor pair needed the same "does this address take the class's own pointer" check
//! `CLAUDE.md`'s own warning calls out - here it checks out clean, unlike `ztshowstate`'s same-shaped
//! slot.** `ZTSHOW_INFO_1` (`ZTShowInfo_~ZTShowInfo_1.c`, the vtable `+0x18` "deleting destructor") calls
//! plain `~ZTShowInfo(this)` (`ZTSHOW_INFO_0`) with **no `this`-adjustment** and restores *this class's
//! own* vtable pointer (`ZTShowInfo__vtable_006353cc`, not some enclosing class's), then conditionally
//! `operator_delete(this)` when its flag byte's low bit is set - the genuine "usual MSVC deleting
//! destructor flag-byte shape" the plan's own vtable table already predicted, confirmed rather than
//! assumed. The plain destructor's own body (`ZTShowInfo_~ZTShowInfo_0.c`/`.asm`, both read - the `.asm`
//! shows a rotated/shared free block the `.c` flattens into three separate `if`s, same result) frees, in
//! order: the `BFEvent` array (`+0x5c`, via `FUN_00401b16` sized by element count) if non-null, the
//! registered-unit-types array (`+0x50`, via the shared small-object freelist `DAT_00638000` bucketed by
//! byte size, or `operator_delete` past `0x80` bytes) if non-null, and the pending-scripts tree: a
//! non-empty tree (`+0x48` node count != 0 **and** the header's own root != 0) tail-calls opaque
//! `FUN_005aade2` (no decompile, no independently-resolved address anywhere in this corpus - the same
//! class of gap `CONSTRUCTOR_0`'s own tree reconstruction above already surfaces) to recursively free
//! every node; otherwise (empty tree) it inlines a trivial self-unlink and unconditionally frees the
//! header block itself (`+0x44`, `0x48` bytes, via `FUN_00401b16`) if non-null. None of this is
//! reproducible without pinning `FUN_00401b16`/`FUN_005aade2`'s real calling conventions - the same
//! "reproducing a subtly-wrong version of a helper this many unrelated systems share would risk
//! corrupting freelist state well outside this class's own scope" call Stage 8's own doc comment already
//! made for `FUN_0040146c`, reused verbatim here for these two.
//!
//! **Live coverage**: `ZTSHOWINFO_STANDALONE_ROUNDTRIP` runs both constructors and the real destructor
//! end-to-end - entirely real vanilla address calls operating on real vanilla-allocated memory throughout
//! (`OPERATOR_NEW.original()` for the outer buffers, `CONSTRUCTOR_1`/`CONSTRUCTOR_0.original()` to
//! construct, `ZTSHOW_INFO_1.original()` with the flag byte clear to destruct without a nested
//! `operator_delete`, then this module's own `OPERATOR_DELETE.original()` for the outer buffers it
//! allocated itself) - so unlike most of this class family's other standalone fixtures, this test needs
//! no leak-only exception: nothing here ever crosses into Rust's own allocator. Confirms this module's
//! own documented understanding of the default constructor's field defaults (`+0x68` == `0x5a`, `+0x6c`
//! derived from the live `GLOBAL_ZTAIMgr`, the array trio fields zeroed, a non-null pending-scripts tree
//! header) and that the copy constructor produces an independent tree/array allocation rather than
//! aliasing the source's own.

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bfevent::{CONSTRUCTOR as BFEVENT_CONSTRUCTOR, LOAD as BFEVENT_LOAD, SAVE as BFEVENT_SAVE},
        bfworldmgr::{GET_TYPE, GET_UNIT},
        standalone::{DEALLOCATE, OPERATOR_DELETE, OPERATOR_NEW, WRITE_BYTES_TO_FILE},
        ztgamemgr::GET_DATE,
        ztshow::{ABORT_SHOW, LOAD as ZTSHOW_LOAD, SAVE as ZTSHOW_SAVE},
        ztshowinfo::{
            ADD_SHOW, CHECK_UNIT, CLEANUP_EVENTS, CREATE_DEFAULT_SCRIPT, ENTER_NEW_MONTH, GET_EVENTS, GET_NUM_UNITS,
            GET_SCHEDULED_SHOW_KEEPER_TYPE, GET_SCHEDULED_SHOW_SCRIPT, GET_SHOW_UNIT_LIST, HAS_KEEPER, INCREMENT_ATTENDANCE,
            INCREMENT_RECEIPTS, IS_READY, IS_STARTED, IS_STOPPED, LISTEN, LOAD, NEEDS_KEEPER, RECALCULATE_SCHEDULE, REMOVE_SHOW,
            REMOVE_UNIT, SAVE, SEND_EVENT, SET_SHOW_FREQUENCY, SET_SHOW_INFO_ID, UPDATE,
        },
        ztshowscript::CONSTRUCTOR as ZTSHOW_SCRIPT_CONSTRUCTOR,
        ztui_showpanel::FORCE_UPDATE,
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;
use tracing::error;
use windows::Win32::Foundation::FILETIME;

use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, save_to_memory},
    ztmegatilemgr::entity_type_matches,
    ztshow::{
        call_entity_vtable_u32_noargs, check_unit_type, collect_pending_script_nodes, find_or_insert_pending_script_node, type_check,
        RVA_ANIMAL_TYPE_CHECK, RVA_SHOW_TRICK_TYPE_CHECK,
    },
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

/// Reimplementation of `ZTShowInfo::hasKeeper`, per `ZTShowInfo_hasKeeper.c`/`.asm`. Calls this module's
/// own [`get_num_units`] directly (Stage 7) rather than through the detour - see the module doc comment's
/// own updated note.
pub fn has_keeper(this: u32) -> bool {
    let keeper_type = get_scheduled_show_keeper_type(this);
    get_num_units(this, keeper_type) > 0
}

/// Reimplementation of `ZTShowInfo::needsKeeper`, per `ZTShowInfo_needsKeeper.c`/`.asm`. Calls this
/// module's own [`get_num_units`] directly (Stage 7) rather than through the detour - see the module doc
/// comment's own updated note.
pub fn needs_keeper(this: u32, unit_type_id: u32) -> bool {
    let keeper_type = get_scheduled_show_keeper_type(this);
    if unit_type_id != keeper_type {
        return false;
    }
    if get_num_units(this, unit_type_id) >= 1 {
        return false;
    }
    is_ready(this)
}

/// Reimplementation of `ZTShowInfo::getNumUnits` (Stage 7) - see the module doc comment's own Stage 7
/// section for why this is the same tree [`get_scheduled_show_script`]/[`increment_attendance`] already
/// own, and why the insert-on-miss side effect is preserved.
pub fn get_num_units(this: u32, unit_type_id: u32) -> i32 {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    let sentinel = get_from_memory::<u32>(node + 0x18);
    let mut cursor = get_from_memory::<u32>(sentinel);
    let mut count = 0i32;
    while cursor != sentinel {
        count += 1;
        cursor = get_from_memory::<u32>(cursor);
    }
    count
}

/// Reimplementation of `ZTShowInfo::getShowUnitList` (Stage 7) - see the module doc comment's own Stage 7
/// section. Same find-or-insert lookup as [`get_num_units`]; returns the *address of* the node's own
/// `+0x18` list field (not its content), matching real vanilla's own `return node + 0x18` and the
/// address-of-field convention `ztshow.rs`'s `validate_item`/`start` already assume when calling this
/// address via `.original()`.
pub fn get_show_unit_list(this: u32, unit_type_id: u32) -> u32 {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    node + 0x18
}

/// Reimplementation of `ZTShowInfo::checkUnit` (Stage 7) - see the module doc comment's own Stage 7
/// section for the trick-eligible-type gate and why the full raw `u32` result is returned, not just a
/// boolean.
pub fn check_unit(this: u32, unit_id: u32) -> u32 {
    if unit_id == 0 {
        return 0;
    }
    let world = globals().ztworldmgr_ptr() as *const u32;
    let unit_ptr = unsafe { GET_UNIT.original()(world, unit_id as i32) };
    if unit_ptr == 0 {
        return 0;
    }
    if !unsafe { entity_type_matches(unit_ptr, RVA_SHOW_TRICK_TYPE_CHECK) } {
        return 0;
    }
    let entity_type_ptr = get_from_memory::<u32>(unit_ptr + 0x128);
    let unit_type_id = unsafe { call_entity_vtable_u32_noargs(entity_type_ptr, 0x20) };
    check_unit_type(this, unit_type_id)
}

/// Reimplementation of `ZTShowInfo::removeUnit` (Stage 8, erase-only - see the module doc comment's own
/// Stage 8 section for why the insert side of this class's own unit-roster mutation is deferred, and for
/// the real third-parameter typing wart/bug this function's own conversion fixes). Unlinks `unit_id` from
/// `unit_type_id`'s own circular unit list (`node+0x18`, the same list [`get_num_units`]/
/// [`get_show_unit_list`] already read), freeing the removed `0xc`-byte entry back to the shared freelist
/// [`free_unit_array_buffer`] already uses, then always calls real vanilla's own `ZTUI::showpanel::
/// forceUpdate()` - a no-op (beyond the find-or-insert side effect and the `forceUpdate` call) when
/// `unit_id` isn't actually present.
pub fn remove_unit(this: u32, unit_type_id: u32, unit_id: u32) {
    let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
    let sentinel = get_from_memory::<u32>(node + 0x18);
    let mut cursor = get_from_memory::<u32>(sentinel);
    while cursor != sentinel {
        if get_from_memory::<u32>(cursor + 0x8) == unit_id {
            let prev = get_from_memory::<u32>(cursor + 0x4);
            let next = get_from_memory::<u32>(cursor);
            save_to_memory(prev, next);
            save_to_memory(next + 0x4, prev);
            free_unit_array_buffer(cursor, 0xc);
            break;
        }
        cursor = get_from_memory::<u32>(cursor);
    }
    unsafe { FORCE_UPDATE.original()() };
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

/// Raw FPU helper `FUN_0059e8f0`, no `generated.rs` entry - see the module doc comment's own Stage 9
/// section and CLAUDE.md's sanctioned local-`FunctionDef` workaround (this module's own
/// [`ALLOCATE_UNIT_ARRAY`] precedent). Confirmed thiscall on `GLOBAL_ZTWorldMgr` with a single `u16` stack
/// argument, returning a value via `ST0` - Ghidra's own `float10 * __return_storage_ptr__(short)` signature
/// (both real call sites' own `.meta`) is its usual false-positive for an FPU-return function, not a
/// genuine hidden-pointer parameter: both real callers push exactly one stack value (no second push for a
/// return slot) and read the result straight off `ST0` via `FST`/`FADD`/`FSTP` immediately after the call.
/// Both real callers (`update`'s/`enterNewMonth`'s own `.asm`) only ever fill the low 16 bits of the pushed
/// register before the call (`MOV AX,...`/`MOV DX,...`, from `this->field_0x70`) and leave the upper half
/// as leftover garbage from an unrelated earlier load (a pointer's own upper bits, in both cases) - the same
/// "callee only reads the meaningful low bits" shape CLAUDE.md's `ZooStatus::fChance` precedent documents,
/// so a clean `u16` argument here reproduces real vanilla's own behavior exactly. Real vanilla's own body
/// past this calling convention was not investigated - only how to call it correctly, matching this class
/// family's established "port the orchestration, call through to a still-real sibling" precedent.
const FUN_0059E8F0: FunctionDef<unsafe extern "thiscall" fn(*const u32, u16) -> f32> = FunctionDef::new(0x0059_e8f0);

/// The float-decay tail shared verbatim by [`update`]/[`enter_new_month`]'s own real bodies - see the
/// module doc comment's own Stage 9 section. Stores `sample` (both real callers' own [`FUN_0059E8F0`]
/// result) into `this+0x88` and folds it into the running total at `this+0x90`. Pure and host-testable with
/// a fabricated `sample`; only obtaining that sample from real vanilla needs the live game process.
fn apply_engagement_sample(this: u32, sample: f32) {
    save_to_memory(this + 0x88, sample);
    save_to_memory(this + 0x90, sample + get_from_memory::<f32>(this + 0x90));
}

/// Pure field-rollover half of [`enter_new_month`] - see the module doc comment's own Stage 9 section for
/// the field map and the "value-mutation-only, so [`collect_pending_script_nodes`] is a safe substitute for
/// vanilla's own successor walk" justification. Shifts every "current month" attendance/receipts
/// accumulator (per-node `+0x34`/`+0x28`, instance-level `+0x7c`/`+0x94`) into its own "last month" slot
/// (`+0x38`/`+0x2c`, `+0x80`/`+0x98`) and resets the current-month accumulator to zero; the "all-time" slots
/// [`increment_attendance`]/[`increment_receipts`] also maintain are untouched. Pure and host-testable;
/// only [`enter_new_month`]'s own tail (the live [`FUN_0059E8F0`] call) needs the game process.
fn roll_monthly_totals(this: u32) {
    save_to_memory(this + 0x98, get_from_memory::<f32>(this + 0x94));
    save_to_memory(this + 0x80, get_from_memory::<i32>(this + 0x7c));
    save_to_memory(this + 0x7c, 0i32);
    save_to_memory(this + 0x94, 0f32);

    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);
    for node in nodes {
        let attendance = get_from_memory::<i32>(node + 0x34);
        save_to_memory(node + 0x34, 0i32);
        save_to_memory(node + 0x38, attendance);
        let receipts = get_from_memory::<f32>(node + 0x28);
        save_to_memory(node + 0x2c, receipts);
        save_to_memory(node + 0x28, 0f32);
    }
}

/// Reimplementation of `ZTShowInfo::enterNewMonth` - see the module doc comment's own Stage 9 section.
pub fn enter_new_month(this: u32) {
    roll_monthly_totals(this);
    save_to_memory(this + 0x8c, get_from_memory::<f32>(this + 0x88));

    let world = globals().ztworldmgr_ptr() as *const u32;
    let target_id = get_from_memory::<u16>(this + 0x70);
    let sample = unsafe { FUN_0059E8F0.original()(world, target_id) };
    apply_engagement_sample(this, sample);
}

/// Reimplementation of `ZTShowInfo::update` (vtable slot `+0x20`) - see the module doc comment's own Stage
/// 9 section for the call order and the embedded `ZTShow::update` call-through's own real vtable slot.
pub fn update(this: u32) {
    listen(this);

    let ztshow = this + 0x4;
    let vtable = get_from_memory::<u32>(ztshow);
    let slot = get_from_memory::<u32>(vtable + 0xc);
    let f: unsafe extern "thiscall" fn(u32) = unsafe { std::mem::transmute(slot) };
    unsafe { f(ztshow) };

    cleanup_events(this);

    save_to_memory(this + 0x90, get_from_memory::<f32>(this + 0x90) - get_from_memory::<f32>(this + 0x88));
    let world = globals().ztworldmgr_ptr() as *const u32;
    let target_id = get_from_memory::<u16>(this + 0x70);
    let sample = unsafe { FUN_0059E8F0.original()(world, target_id) };
    apply_engagement_sample(this, sample);
}

/// Reimplementation of `ZTShowInfo::setShowInfoID` (`0x005ab8c3`, per `ZTShowInfo_setShowInfoID.asm`;
/// macOS symbolizes the same address split across `ZTShowInfo_setShowInfoID.c` + `ZTShow_setShowInfoID.c`).
/// See the module doc comment's own Stage 10 section for why this moved here from `ztshowmgr.rs`'s
/// original `ZTShowMgr::set_show_info_id`. Not a trivial `field_0x70` store: after `this->field_0x70 =
/// id`, it re-points the embedded `ZTShow`'s (`this+0x4`) `+0x10` back-pointer at `this` unless it already
/// points at an object whose own `field_0x70` equals the new id, and always refreshes the `ZTShow`'s `+0x6`
/// u16 id copy. All four writes are reproduced here, in the real order, so a plain `field_0x70` store could
/// not leave the embedded mirror fields stale.
///
/// Not reproduced: the real body's return-0 path ("outer show's `field_0x70` != the new id"; macOS's
/// `ZTShow::setShowInfoID` returns 0 there explicitly) - the guard compares the field this port's first
/// write just set, so it is unreachable through this function's own body and the real body always exits
/// `AL=1`; and the `.asm`'s null-`this` guard (`TEST %ECX,%ECX; JZ`), which every known caller
/// (`ZTShowMgr::register_show`'s own null check; the detour below, which real vanilla itself never invokes
/// with a null `this`) already excludes. Reading traps honored: the Windows `.c` renders the embedded-
/// `ZTShow` half with confusing flattened offsets - trust the `.asm`'s `ADD %ECX, 0x4` (the offsets here),
/// not the decompile's `this->field_0x14`/`this->field_0xa` renderings.
pub fn set_show_info_id(this: u32, id: u16) -> bool {
    save_to_memory(this + 0x70, id);
    let ztshow = this + 0x4;
    let back_pointer: u32 = get_from_memory(ztshow + 0x10);
    if back_pointer == 0 || get_from_memory::<u16>(back_pointer + 0x70) != id {
        save_to_memory(ztshow + 0x10, this);
    }
    save_to_memory(ztshow + 0x6, id);
    true
}

/// `ZTShowInfo::meth_0x48a1d7` (real address embedded in the decompile's own symbolic name, per this
/// codebase's established convention for an opaque helper with no `generated.rs` entry) - sets some default
/// satisfaction-related fields for a pre-`0x6a`-version load. No decompile exists for its own body; called
/// through untouched, matching this class family's "port the orchestration, call through to a still-real
/// sibling" precedent for a real, un-ported callee whose own internals aren't this stage's concern.
const SET_DEFAULT_SATISFACTION_FIELDS: FunctionDef<unsafe extern "thiscall" fn(*const u32)> = FunctionDef::new(0x0048_a1d7);

fn write_field(addr: u32, size: u32, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(addr as *const u32, size, 1, file) == 1 }
}

fn read_field(addr: u32, size: u32, file: *const u32) -> bool {
    unsafe { DEALLOCATE.hooked()(addr as *const u32, size, 1, file as *const u8) == 1 }
}

/// Tears down an existing pending-scripts tree (`this+0x44`) before [`show_info_load`] rebuilds it from
/// file - real vanilla only takes this path when `this+0x48` (the cached node count) is non-zero, i.e.
/// essentially never in practice (`load`'s only real caller constructs a fresh `ZTShowInfo` immediately
/// beforehand - see `ztshowinfo-pending-scripts-tree-plan.md`'s own lifecycle table), but is reproduced for
/// completeness rather than assumed unreachable. Real vanilla's own teardown helper for this
/// (`FUN_005aad9a`) has no decompile; this instead reuses two already-independently-verified teardown
/// primitives from elsewhere in this class family for the two sub-allocations every node owns: each unit-list
/// entry via [`free_unit_array_buffer`] (the same freelist [`remove_unit`] already frees list entries
/// through) and the list's own sentinel plus the node itself via `OPERATOR_DELETE` (matching
/// [`allocate_pending_script_node`]'s own `OPERATOR_NEW` for both).
fn clear_pending_script_tree(this: u32) {
    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);

    for node in nodes {
        let sentinel = get_from_memory::<u32>(node + 0x18);
        if sentinel != 0 {
            let mut cursor = get_from_memory::<u32>(sentinel);
            while cursor != sentinel {
                let next = get_from_memory::<u32>(cursor);
                free_unit_array_buffer(cursor, 0xc);
                cursor = next;
            }
            unsafe { OPERATOR_DELETE.original()(sentinel) };
        }
        unsafe { OPERATOR_DELETE.original()(node) };
    }

    save_to_memory(header + 4, 0u32);
    save_to_memory(header + 8, header);
    save_to_memory(this + 0x48, 0u32);
}

/// Reimplementation of `ZTShowInfo::save`, per `ZTShowInfo_save.c`/`.asm` (both read in full) - see the
/// module doc comment's own Stage 11 section for the full field/write order and the single early-return
/// location (matching real vanilla's own).
pub fn show_info_save(this: u32, file: *const i8) -> bool {
    let mut ok = write_field(this + 0x68, 4, file);
    ok &= write_field(this + 0xa4, 4, file);

    let begin = get_from_memory::<u32>(this + 0x50);
    let end = get_from_memory::<u32>(this + 0x54);
    let show_count = (end - begin) >> 2;
    ok &= write_field(&show_count as *const u32 as u32, 4, file);
    let mut cursor = begin;
    while cursor != end {
        ok &= write_field(cursor, 4, file);
        cursor += 4;
    }

    let node_count = get_from_memory::<u32>(this + 0x48);
    ok &= write_field(&node_count as *const u32 as u32, 4, file);

    let header = get_from_memory::<u32>(this + 0x44);
    let root = get_from_memory::<u32>(header + 4);
    let mut nodes = Vec::new();
    collect_pending_script_nodes(root, &mut nodes);
    for node in nodes {
        ok &= write_field(node + 0x10, 4, file);
        ok &= write_field(node + 0x1c, 2, file);
        ok &= write_field(node + 0x1e, 2, file);
        ok &= write_field(node + 0x24, 4, file);
        ok &= write_field(node + 0x28, 4, file);
        ok &= write_field(node + 0x2c, 4, file);
        ok &= write_field(node + 0x30, 4, file);
        ok &= write_field(node + 0x34, 4, file);
        ok &= write_field(node + 0x38, 4, file);
        ok &= write_field(node + 0x3c, 4, file);
        ok &= write_field(node + 0x40, 8, file);
    }

    let ev_begin = get_from_memory::<u32>(this + 0x5c);
    let ev_end = get_from_memory::<u32>(this + 0x60);
    let ev_count = (ev_end - ev_begin) / 0x1c;
    ok &= write_field(&ev_count as *const u32 as u32, 4, file);
    let mut ev_cursor = ev_begin;
    while ev_cursor != ev_end {
        let saved = unsafe { BFEVENT_SAVE.original()(ev_cursor as *const u32, file) };
        ok &= saved & 0xff != 0;
        if !ok {
            return false;
        }
        ev_cursor += 0x1c;
    }

    ok &= write_field(this + 0x6c, 4, file);
    ok &= write_field(this + 0x70, 2, file);
    ok &= write_field(this + 0x88, 4, file);
    ok &= write_field(this + 0x8c, 4, file);
    ok &= write_field(this + 0x90, 4, file);
    ok &= write_field(this + 0x94, 4, file);
    ok &= write_field(this + 0x98, 4, file);
    ok &= write_field(this + 0x9c, 4, file);
    ok &= write_field(this + 0x7c, 4, file);
    ok &= write_field(this + 0x80, 4, file);
    ok &= write_field(this + 0x84, 4, file);

    let show_ok = unsafe { ZTSHOW_SAVE.original()((this + 0x4) as *const u32, file as *const u32) != 0 };
    ok && show_ok
}

/// The eleven non-key pending-scripts node fields [`show_info_load`] reads per record - see the module doc
/// comment's own Stage 11 section for the version-gated field/read order and the `version < 99`/`+0x1e`
/// note. Pulled into its own pure, host-testable constructor ([`PendingScriptRecord::defaulted`]) separate
/// from the file-I/O-heavy per-record read loop, matching this module's own established "no fabricated-
/// buffer test for anything that calls `.hooked()`/`.original()`" convention - only the *decision* of which
/// defaults apply for an old save is pure enough to unit test.
struct PendingScriptRecord {
    current: u16,
    pending: u16,
    field_0x24: u32,
    receipts_current: u32,
    field_0x2c: u32,
    receipts_total: u32,
    attendance_current: u32,
    field_0x38: u32,
    attendance_total: u32,
    filetime_low: u32,
    filetime_high: u32,
}

impl PendingScriptRecord {
    /// The `version < 99` defaults: everything zeroed except the FILETIME pair, which uses the live current
    /// game date (matching real vanilla's own `ZTGameMgr::getDate` fallback) - `pending` is left `0`, not
    /// reproduced from real vanilla's own genuinely-uninitialized stack read, see the module doc comment.
    fn defaulted(current: u16, filetime_low: u32, filetime_high: u32) -> Self {
        PendingScriptRecord {
            current,
            pending: 0,
            field_0x24: 0,
            receipts_current: 0,
            field_0x2c: 0,
            receipts_total: 0,
            attendance_current: 0,
            field_0x38: 0,
            attendance_total: 0,
            filetime_low,
            filetime_high,
        }
    }
}

/// Reimplementation of `ZTShowInfo::load`, per `ZTShowInfo_load.c`/`.asm` (both read in full) - see the
/// module doc comment's own Stage 11 section for the version-gate thresholds, the `BFEvent` array growth
/// substitution, and the pre-99 "discard legacy data" quirk.
pub fn show_info_load(this: u32, file: *const u32, version: u32) -> bool {
    let mut ok = read_field(this + 0x68, 4, file);
    ok &= read_field(this + 0xa4, 4, file);

    let mut show_count: u32 = 0;
    ok &= read_field(&mut show_count as *mut u32 as u32, 4, file);

    let old_begin = get_from_memory::<u32>(this + 0x50);
    if old_begin != 0 {
        let old_cap_end = get_from_memory::<u32>(this + 0x58);
        free_unit_array_buffer(old_begin, old_cap_end - old_begin);
    }
    save_to_memory(this + 0x50, 0u32);
    save_to_memory(this + 0x54, 0u32);
    save_to_memory(this + 0x58, 0u32);
    for _ in 0..show_count {
        let mut id: u32 = 0;
        ok &= read_field(&mut id as *mut u32 as u32, 4, file);
        add_show(this, id);
    }

    if version < 99 {
        let mut legacy_count: u32 = 0;
        ok &= read_field(&mut legacy_count as *mut u32 as u32, 4, file);
        for _ in 0..legacy_count {
            let mut discard_a: u32 = 0;
            let mut discard_b: u32 = 0;
            ok &= read_field(&mut discard_a as *mut u32 as u32, 4, file);
            ok &= read_field(&mut discard_b as *mut u32 as u32, 4, file);
        }
    }

    let mut node_count: u32 = 0;
    ok &= read_field(&mut node_count as *mut u32 as u32, 4, file);

    if get_from_memory::<u32>(this + 0x48) != 0 {
        clear_pending_script_tree(this);
    }

    for _ in 0..node_count {
        let mut unit_type_id: u32 = 0;
        ok &= read_field(&mut unit_type_id as *mut u32 as u32, 4, file);

        let mut current: u16 = 0;
        ok &= read_field(&mut current as *mut u16 as u32, 2, file);

        let record = if version < 99 {
            let mut date = FILETIME::default();
            unsafe { GET_DATE.original()(globals().ztgamemgr_ptr() as *const u32, &mut date as *const FILETIME) };
            PendingScriptRecord::defaulted(current, date.dwLowDateTime, date.dwHighDateTime)
        } else {
            let mut pending: u16 = 0;
            ok &= read_field(&mut pending as *mut u16 as u32, 2, file);
            let mut field_0x24: u32 = 0;
            ok &= read_field(&mut field_0x24 as *mut u32 as u32, 4, file);
            let mut receipts_current: u32 = 0;
            ok &= read_field(&mut receipts_current as *mut u32 as u32, 4, file);
            let mut field_0x2c: u32 = 0;
            ok &= read_field(&mut field_0x2c as *mut u32 as u32, 4, file);
            let mut receipts_total: u32 = 0;
            ok &= read_field(&mut receipts_total as *mut u32 as u32, 4, file);
            let mut attendance_current: u32 = 0;
            ok &= read_field(&mut attendance_current as *mut u32 as u32, 4, file);
            let mut field_0x38: u32 = 0;
            ok &= read_field(&mut field_0x38 as *mut u32 as u32, 4, file);
            let mut attendance_total: u32 = 0;
            ok &= read_field(&mut attendance_total as *mut u32 as u32, 4, file);
            let mut filetime_low: u32 = 0;
            ok &= read_field(&mut filetime_low as *mut u32 as u32, 4, file);
            let mut filetime_high: u32 = 0;
            ok &= read_field(&mut filetime_high as *mut u32 as u32, 4, file);
            PendingScriptRecord {
                current,
                pending,
                field_0x24,
                receipts_current,
                field_0x2c,
                receipts_total,
                attendance_current,
                field_0x38,
                attendance_total,
                filetime_low,
                filetime_high,
            }
        };

        let (node, _was_inserted) = find_or_insert_pending_script_node(this, unit_type_id);
        save_to_memory(node + 0x1c, record.current);
        save_to_memory(node + 0x1e, record.pending);
        save_to_memory(node + 0x24, record.field_0x24);
        save_to_memory(node + 0x28, record.receipts_current);
        save_to_memory(node + 0x2c, record.field_0x2c);
        save_to_memory(node + 0x30, record.receipts_total);
        save_to_memory(node + 0x34, record.attendance_current);
        save_to_memory(node + 0x38, record.field_0x38);
        save_to_memory(node + 0x3c, record.attendance_total);
        save_to_memory(node + 0x40, record.filetime_low);
        save_to_memory(node + 0x44, record.filetime_high);
    }

    if version > 0x60 {
        let ev_begin = get_from_memory::<u32>(this + 0x5c);
        save_to_memory(this + 0x60, ev_begin);

        let mut ev_count: u32 = 0;
        ok &= read_field(&mut ev_count as *mut u32 as u32, 4, file);

        if ev_count != 0 {
            let buf = unsafe { ALLOCATE_UNIT_ARRAY.original()(ev_count * 0x1c) as u32 };
            for i in 0..ev_count {
                let slot = buf + i * 0x1c;
                unsafe { BFEVENT_CONSTRUCTOR.original()(slot as *const u32) };
                let loaded = unsafe { BFEVENT_LOAD.original()(slot as *const u32, file, version) };
                ok &= loaded;
                if !ok {
                    return false;
                }
            }
            save_to_memory(this + 0x5c, buf);
            save_to_memory(this + 0x60, buf + ev_count * 0x1c);
            save_to_memory(this + 0x64, buf + ev_count * 0x1c);
        }

        ok &= read_field(this + 0x6c, 4, file);
        ok &= read_field(this + 0x70, 2, file);
    }

    if version > 0x68 {
        ok &= read_field(this + 0x88, 4, file);
        ok &= read_field(this + 0x8c, 4, file);
        ok &= read_field(this + 0x90, 4, file);
    }

    if version < 0x6a {
        unsafe { SET_DEFAULT_SATISFACTION_FIELDS.original()(this as *const u32) };
    } else {
        ok &= read_field(this + 0x94, 4, file);
        ok &= read_field(this + 0x98, 4, file);
        ok &= read_field(this + 0x9c, 4, file);
        ok &= read_field(this + 0x7c, 4, file);
        ok &= read_field(this + 0x80, 4, file);
        ok &= read_field(this + 0x84, 4, file);
    }

    let show_ok = unsafe { ZTSHOW_LOAD.original()((this + 0x4) as *const u32, file, version) };
    ok && (show_ok & 0xff != 0)
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

    #[detour(GET_EVENTS)]
    unsafe extern "thiscall" fn get_events_detour(this: *const u32, arg: u32) {
        get_events(this as u32, arg);
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

    #[detour(GET_NUM_UNITS)]
    unsafe extern "thiscall" fn get_num_units_detour(this: *const u32, unit_type_id: u32) -> i32 {
        get_num_units(this as u32, unit_type_id)
    }

    #[detour(GET_SHOW_UNIT_LIST)]
    unsafe extern "thiscall" fn get_show_unit_list_detour(this: *const u32, unit_type_id: u32) -> i32 {
        get_show_unit_list(this as u32, unit_type_id) as i32
    }

    #[detour(CHECK_UNIT)]
    unsafe extern "thiscall" fn check_unit_detour(this: *const u32, unit_id: u32) -> u32 {
        check_unit(this as u32, unit_id)
    }

    /// `unit_id_ptr as u32`, not `*unit_id_ptr` - see the module doc comment's own Stage 8 section on why
    /// `generated.rs`'s `*const i32` typing here is a pointer-as-integer wart, not a genuine pointer:
    /// real vanilla's own body never dereferences this argument.
    #[detour(REMOVE_UNIT)]
    unsafe extern "thiscall" fn remove_unit_detour(this: *const u32, unit_type_id: u32, unit_id_ptr: *const i32) {
        remove_unit(this as u32, unit_type_id, unit_id_ptr as u32);
    }

    #[detour(ENTER_NEW_MONTH)]
    unsafe extern "thiscall" fn enter_new_month_detour(this: *const u32) {
        enter_new_month(this as u32);
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update_detour(this: *const u32) {
        update(this as u32);
    }

    #[detour(SET_SHOW_INFO_ID)]
    unsafe extern "thiscall" fn set_show_info_id_detour(this: *const u32, id: u16) -> bool {
        set_show_info_id(this as u32, id)
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const i8) -> u32 {
        show_info_save(this as u32, file) as u32
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load_detour(this: *const u32, file: *const u32, version: u32) -> u8 {
        show_info_load(this as u32, file, version) as u8
    }

    /// `(name, is_enabled)` per detour - lets `reimplementation_tests`'s `ZTSHOWINFO_DETOURS_ENABLED`
    /// catch a silently-failed `init_detours()`, same rationale as `ztshowstate::detours::status`'s own
    /// doc comment.
    pub(crate) fn status() -> [(&'static str, bool); 27] {
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
            ("GET_EVENTS", GET_EVENTS_DETOUR.is_enabled()),
            ("SEND_EVENT", SEND_EVENT_DETOUR.is_enabled()),
            ("LISTEN", LISTEN_DETOUR.is_enabled()),
            ("CLEANUP_EVENTS", CLEANUP_EVENTS_DETOUR.is_enabled()),
            ("GET_NUM_UNITS", GET_NUM_UNITS_DETOUR.is_enabled()),
            ("GET_SHOW_UNIT_LIST", GET_SHOW_UNIT_LIST_DETOUR.is_enabled()),
            ("CHECK_UNIT", CHECK_UNIT_DETOUR.is_enabled()),
            ("REMOVE_UNIT", REMOVE_UNIT_DETOUR.is_enabled()),
            ("ENTER_NEW_MONTH", ENTER_NEW_MONTH_DETOUR.is_enabled()),
            ("UPDATE", UPDATE_DETOUR.is_enabled()),
            ("SET_SHOW_INFO_ID", SET_SHOW_INFO_ID_DETOUR.is_enabled()),
            ("SAVE", SAVE_DETOUR.is_enabled()),
            ("LOAD", LOAD_DETOUR.is_enabled()),
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
    /// See `detours::status()`'s own doc comment. `crate::ztshow::live_support::
    /// build_standalone_show_info` is still the right choice for any test that only needs a valid
    /// pending-scripts tree header (no real ctor call, one allocation instead of the several a real
    /// construction performs) - Stage 12's own real-ctor-backed builders below are only for the
    /// constructor/destructor round trip test itself, which needs the real thing.
    pub(crate) fn detour_status() -> [(&'static str, bool); 27] {
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

    use openzt_detour::generated::standalone::{OPERATOR_DELETE, OPERATOR_NEW};
    use openzt_detour::generated::ztshowinfo::{CONSTRUCTOR_0, CONSTRUCTOR_1, ZTSHOW_INFO_1};

    /// Real `ZTShowInfo` size (Windows) - see `ztshow::live_support::build_standalone_show_info`'s own
    /// doc comment for the `ZTHabitat_setIsShowExhibit.c` `new(0xa8)` evidence.
    const ZTSHOWINFO_SIZE: u32 = 0xa8;

    /// Allocates a fresh `0xa8`-byte buffer and runs the real vanilla default constructor
    /// (`CONSTRUCTOR_1`) over it, for `ZTSHOWINFO_STANDALONE_ROUNDTRIP`. Unlike
    /// `crate::ztshow::live_support::build_standalone_show_info`, this runs the genuine constructor
    /// rather than hand-rolling a stand-in - see the module doc comment's Stage 12 section for why that's
    /// safe here (`GLOBAL_ZTAIMgr` exists from process start, no live zoo needed).
    pub(crate) fn build_standalone_show_info_via_real_ctor() -> u32 {
        let buf = unsafe { OPERATOR_NEW.original()(ZTSHOWINFO_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(buf as *mut u8, 0, ZTSHOWINFO_SIZE as usize) };
        unsafe { CONSTRUCTOR_1.original()(buf as *const u32) };
        buf
    }

    /// Runs the real copy constructor (`CONSTRUCTOR_0`) with `source` (any real, fully-constructed
    /// `ZTShowInfo`, e.g. one built by [`build_standalone_show_info_via_real_ctor`]) into a fresh
    /// `0xa8`-byte buffer.
    pub(crate) fn build_standalone_show_info_copy_via_real_ctor(source: u32) -> u32 {
        let buf = unsafe { OPERATOR_NEW.original()(ZTSHOWINFO_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(buf as *mut u8, 0, ZTSHOWINFO_SIZE as usize) };
        unsafe { CONSTRUCTOR_0.original()(buf as *const u32, source as i32) };
        buf
    }

    /// Tears down a buffer built by either constructor helper above via the real destructor
    /// (`ZTSHOW_INFO_1`, vtable slot `+0x18`) with its flag byte clear (don't `operator_delete` `this`
    /// internally), then frees the outer buffer this module itself allocated via
    /// `OPERATOR_NEW.original()`. Confirmed safe to call directly against a bare `ZTShowInfo*` - unlike
    /// `ztshowstate`'s own same-shaped destructor slot, `ZTSHOW_INFO_1`'s own body performs no
    /// `this`-adjustment and restores *this class's own* vtable (see the module doc comment's Stage 12
    /// section). Every step here is a real vanilla allocator call over real vanilla-allocated memory, so
    /// this needs no leak-only exception, unlike most of this class family's other standalone fixtures.
    pub(crate) fn destroy_standalone_show_info_via_real_dtor(buf: u32) {
        unsafe { ZTSHOW_INFO_1.original()(buf as *const u32, 0) };
        unsafe { OPERATOR_DELETE.original()(buf) };
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

    /// Found-path only, same discipline as `increment_attendance_accumulates_into_node_counters_and_instance_totals`:
    /// a genuine miss runs through `find_or_insert_pending_script_node`'s real-allocator insert
    /// (`OPERATOR_NEW.original()`), meaningless outside the live process - covered live instead
    /// (`ZTSHOWINFO_UNIT_ROSTER_READ_LIVE`). The node's own `+0x18` list field is fabricated directly here
    /// rather than via `allocate_pending_script_node` (also real-allocator-backed), matching the same
    /// `{next:+0x0, prev:+0x4, payload:+0x8}` self-referential-sentinel shape it produces.
    #[test]
    fn get_num_units_counts_the_matching_nodes_own_unit_list() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;

        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x1c];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node); // root = node
        save_to_memory(node + 0x10, 42u32); // key

        let mut sentinel_buf = [0u8; 0xc];
        let sentinel = sentinel_buf.as_mut_ptr() as u32;
        save_to_memory(sentinel, sentinel);
        save_to_memory(sentinel + 4, sentinel);
        save_to_memory(node + 0x18, sentinel);

        save_to_memory(this + 0x44, header);
        assert_eq!(get_num_units(this, 42), 0, "an empty list must count as zero");

        let mut entry_a = [0u8; 0xc];
        let entry_a_addr = entry_a.as_mut_ptr() as u32;
        let mut entry_b = [0u8; 0xc];
        let entry_b_addr = entry_b.as_mut_ptr() as u32;
        // sentinel <-> entry_a <-> entry_b <-> sentinel (circular).
        save_to_memory(sentinel, entry_a_addr);
        save_to_memory(entry_a_addr, entry_b_addr);
        save_to_memory(entry_a_addr + 4, sentinel);
        save_to_memory(entry_b_addr, sentinel);
        save_to_memory(entry_b_addr + 4, entry_a_addr);
        save_to_memory(sentinel + 4, entry_b_addr);

        assert_eq!(get_num_units(this, 42), 2);
    }

    #[test]
    fn get_show_unit_list_returns_the_address_of_the_matching_nodes_own_list_field() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;

        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x1c];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node);
        save_to_memory(node + 0x10, 7u32);
        save_to_memory(this + 0x44, header);

        assert_eq!(get_show_unit_list(this, 7), node + 0x18, "must return &node.list, not its content");
    }

    #[test]
    fn apply_engagement_sample_stores_and_folds_into_the_running_total() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x90, 2.0f32);

        apply_engagement_sample(this, 7.5);

        assert_eq!(get_from_memory::<f32>(this + 0x88), 7.5);
        assert_eq!(get_from_memory::<f32>(this + 0x90), 9.5);
    }

    /// Found-path only, same discipline as `increment_attendance_accumulates_into_node_counters_and_instance_totals`:
    /// a genuine miss runs through `find_or_insert_pending_script_node`'s real-allocator insert, meaningless
    /// outside the live process - covered live instead (`ZTSHOWINFO_ENTER_NEW_MONTH_LIVE`).
    #[test]
    fn roll_monthly_totals_archives_current_month_and_resets_it_leaving_all_time_untouched() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;
        let mut node_buf = [0u8; 0x48];
        let node = node_buf.as_mut_ptr() as u32;
        save_to_memory(header + 0x4, node); // root = node
        save_to_memory(node + 0x10, 42u32); // key
        save_to_memory(node + 0x28, 3.5f32); // receipts current month
        save_to_memory(node + 0x30, 40.0f32); // receipts all-time
        save_to_memory(node + 0x34, 5i32); // attendance current month
        save_to_memory(node + 0x3c, 60i32); // attendance all-time

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0x7c, 100i32); // attendance current month
        save_to_memory(this + 0x84, 900i32); // attendance all-time
        save_to_memory(this + 0x94, 25.5f32); // receipts current month
        save_to_memory(this + 0x9c, 400.25f32); // receipts all-time

        roll_monthly_totals(this);

        assert_eq!(get_from_memory::<i32>(this + 0x80), 100, "attendance last-month must hold the old current-month value");
        assert_eq!(get_from_memory::<i32>(this + 0x7c), 0, "attendance current-month must reset to 0");
        assert_eq!(get_from_memory::<i32>(this + 0x84), 900, "attendance all-time must be untouched");
        assert_eq!(get_from_memory::<f32>(this + 0x98), 25.5, "receipts last-month must hold the old current-month value");
        assert_eq!(get_from_memory::<f32>(this + 0x94), 0.0, "receipts current-month must reset to 0");
        assert_eq!(get_from_memory::<f32>(this + 0x9c), 400.25, "receipts all-time must be untouched");

        assert_eq!(get_from_memory::<i32>(node + 0x38), 5, "node attendance last-month must hold the old current-month value");
        assert_eq!(get_from_memory::<i32>(node + 0x34), 0, "node attendance current-month must reset to 0");
        assert_eq!(get_from_memory::<i32>(node + 0x3c), 60, "node attendance all-time must be untouched");
        assert_eq!(get_from_memory::<f32>(node + 0x2c), 3.5, "node receipts last-month must hold the old current-month value");
        assert_eq!(get_from_memory::<f32>(node + 0x28), 0.0, "node receipts current-month must reset to 0");
        assert_eq!(get_from_memory::<f32>(node + 0x30), 40.0, "node receipts all-time must be untouched");
    }

    #[test]
    fn roll_monthly_totals_is_a_no_op_on_an_empty_tree() {
        let mut header_buf = [0u8; 0xc];
        let header = header_buf.as_mut_ptr() as u32;

        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        save_to_memory(this + 0x44, header);
        save_to_memory(this + 0x7c, 100i32);
        save_to_memory(this + 0x94, 25.5f32);

        roll_monthly_totals(this);

        assert_eq!(get_from_memory::<i32>(this + 0x80), 100);
        assert_eq!(get_from_memory::<i32>(this + 0x7c), 0);
        assert_eq!(get_from_memory::<f32>(this + 0x98), 25.5);
        assert_eq!(get_from_memory::<f32>(this + 0x94), 0.0);
    }

    #[test]
    fn set_show_info_id_writes_field_0x70_and_ztshow_mirror_fields() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;

        assert!(set_show_info_id(this, 0x42));

        assert_eq!(get_from_memory::<u16>(this + 0x70), 0x42, "this+0x70 must hold the new id");
        assert_eq!(get_from_memory::<u32>(this + 0x4 + 0x10), this, "a null back-pointer must be repointed at this");
        assert_eq!(get_from_memory::<u16>(this + 0x4 + 0x6), 0x42, "the embedded ZTShow's own id copy must always refresh");
    }

    #[test]
    fn set_show_info_id_repoints_a_back_pointer_whose_own_field_0x70_disagrees() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut other = fake_show_info();
        let other_addr = other.as_mut_ptr() as u32;
        save_to_memory(other_addr + 0x70, 0x99u16); // disagrees with the id being set below.
        save_to_memory(this + 0x4 + 0x10, other_addr);

        set_show_info_id(this, 0x42);

        assert_eq!(get_from_memory::<u32>(this + 0x4 + 0x10), this, "a stale back-pointer must be repointed at this");
    }

    #[test]
    fn set_show_info_id_leaves_a_back_pointer_whose_own_field_0x70_already_matches() {
        let mut buf = fake_show_info();
        let this = buf.as_mut_ptr() as u32;
        let mut other = fake_show_info();
        let other_addr = other.as_mut_ptr() as u32;
        save_to_memory(other_addr + 0x70, 0x42u16); // already matches the id being set below.
        save_to_memory(this + 0x4 + 0x10, other_addr);

        set_show_info_id(this, 0x42);

        assert_eq!(
            get_from_memory::<u32>(this + 0x4 + 0x10),
            other_addr,
            "a back-pointer whose target already agrees on the new id must be left alone"
        );
    }

    #[test]
    fn pending_script_record_defaulted_zeroes_everything_but_current_and_filetime() {
        let record = PendingScriptRecord::defaulted(0x7, 0x1111_2222, 0x3333_4444);

        assert_eq!(record.current, 0x7);
        assert_eq!(record.pending, 0, "not reproduced from real vanilla's own uninitialized read - see the module doc comment");
        assert_eq!(record.field_0x24, 0);
        assert_eq!(record.receipts_current, 0);
        assert_eq!(record.field_0x2c, 0);
        assert_eq!(record.receipts_total, 0);
        assert_eq!(record.attendance_current, 0);
        assert_eq!(record.field_0x38, 0);
        assert_eq!(record.attendance_total, 0);
        assert_eq!(record.filetime_low, 0x1111_2222);
        assert_eq!(record.filetime_high, 0x3333_4444);
    }
}
