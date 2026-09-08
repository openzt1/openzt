//! Stage 4 (UI consumers) of the vanilla `ZTShowScriptMgr` reimplementation - see
//! `openzt/plans/ztshowscriptmgr-implementation-plan.md`. Stages 1-3 made `ZTShowScriptMgr`/
//! `ZTShowScript`/`ZTShowScriptItem` an independent Rust store and ported every `ZTShow`/`ZTShowInfo`
//! call site that used to raw-dereference that data. This module ports the last two: the show-editor
//! UI panel's `showpanel_fillTrickLists` (populates the "available"/"assigned" trick list boxes) and
//! `_copyListToScript` (the "apply UI selection back to the script" reconciliation handler it calls).
//!
//! Both ported functions also call through to `ZTUI::showpanel::recalcShowStats` (`FUN_00474826` in the
//! Windows decompiles - unnamed there, identified via the macOS decompile and hand-added to
//! `generated.rs` as `ztui_showpanel::RECALC_SHOW_STATS`; see that entry's own comment and
//! [`recalc_show_stats`]'s doc comment), which computes and displays the "Happiness Bonus" number/smiley
//! next to the show-editor's "Tricks in Show" header. An earlier version of this module treated that call
//! as safe to drop entirely (deemed "purely cosmetic-looking"); it is not - since this module's Rust
//! reimplementations replace the *entire* body of the vanilla functions that used to call it, dropping the
//! call meant that display stopped updating altogether (observed live as a stuck `0`).
//!
//! `showpanel_updateAvailableTrickList` (the plan's 3rd originally-flagged UI consumer) needed **no**
//! port and is **not** detoured here: confirmed via a full re-read of its decompile
//! (`private/resources/decompiles/showpanel_updateAvailableTrickList.c`) that every raw dereference it
//! performs is against real, un-touched vanilla memory - `ZTShowMgr::getShowScriptItems`'s own local
//! temporary list (itself a plain deep-copy of `ZTUnitType`'s own real `+0x1ac` trick list, per Stage
//! 3's `getTrickList` finding), `ZTShowInfo::validateTrick`, and UI widget state
//! (`UIListBoxItem::disable`/`field_0x14`). It never touches this crate's independent store.
//!
//! ## Field-offset ground truth
//!
//! All offsets below were cross-checked against `.asm` (not just `.c`) per CLAUDE.md's requirement.
//! Three real, non-obvious findings from that cross-check shape this whole module:
//!
//! 1. **`showpanel_fillTrickLists.c`'s `piVarN[k]` indices are relative to the trick-list *node*, not
//!    the item.** The node is `{next: u32 @+0x0, item: ZTShowScriptItem @+0x8}`, so `piVar7[0x1b]`
//!    (node+0x6c) is `item+0x64` = `normalIcon`, not what a naive item-relative reading would suggest.
//!    Every node-relative index below has already been converted to the item-relative offset it
//!    actually names (confirmed self-consistently: the same four fields - `normalIcon`/`grayedIcon`/
//!    `normalHelpID`/`grayedHelpID` - turn up again as item-relative offsets in `_copyListToScript`'s
//!    `.asm`, this time read directly off an item pointer with no node-vs-item ambiguity at all).
//! 2. **`_copyListToScript.asm`'s "`UIText::setText`" calls are a symbol-misattribution artifact, not
//!    real UI calls.** Each one copy-constructs one of `ZTShowScriptItem`'s six embedded 12-byte SSO
//!    strings (`name`/`anim`/`keeperPreTrick`/`keeperPostTrick`/`normalIcon`/`grayedIcon`) onto the
//!    stack as part of scalarizing a found item into `ZTShowScript::addItem`'s by-value argument -
//!    confirmed by the exact 12-byte-aligned offsets matching the established item field table exactly.
//!    Net effect: the whole "find + scalarize + addItem" sequence in `_copyListToScript` is a
//!    byte-for-byte 124-byte copy of a real item, letting this module read the found item directly as a
//!    [`crate::ztshowscriptmgr::ZTShowScriptItemRaw`] and hand it straight to
//!    [`crate::ztshowscriptmgr::add_item`] instead of hand-mapping 18 fields.
//! 3. **`AI_cls_0x404fd6::meth_0x5a997f`** (called by both `fillTrickLists` and `_copyListToScript` to
//!    resolve a unit type's pending-scripts map entry) returns a pointer whose `+4`/`+6` fields line up
//!    exactly with [`crate::ztshow::find_or_insert_pending_script_node`]'s own node's `+0x1c`
//!    (`current`)/`+0x1e` (`pending`) fields once the base offset (`node+0x18`) is accounted for -
//!    i.e. it is vanilla's own `map::operator[]`-style find-or-insert on the *exact same tree* Stage 2
//!    already reimplemented. Rather than resolve `meth_0x5a997f`'s own address and calling convention
//!    (never found/confirmed - see the plan's Stage 0.2 "hard constraint" list, which never enumerated
//!    it precisely because it wasn't originally recognized as the same tree operation), this module
//!    reuses that existing, live-tested Stage 2 helper directly. This is the "reuse/adapt existing code"
//!    option the handover notes called out as acceptable in place of a real find-only port.
//!
//! ## Deliberately not ported
//!
//! - **`_copyListToScript`'s listbox-not-found teardown branch** (`if (pUStack_c == 0) { if (pZVar11 !=
//!   0) { ZTShowScript::~ZTShowScript(pZVar11); FUN_00402629(pZVar11); } }`) - reached only if the
//!   "assigned tricks" UI element (`0x2b6b`) doesn't exist, which cannot happen while the show-editor
//!   panel that owns this function is even open (the panel's own `init` creates it). Replicating a real
//!   destructor call against a pointer that might be one of this store's synthetic handles would risk
//!   exactly the cross-allocator/invalid-dereference hazard CLAUDE.md warns about, for a branch with no
//!   real-world reachability. This module just returns `0` for that case instead (see
//!   [`copy_list_to_script`]'s own doc comment).
//! - **`ZTShowInfo::validateTrick`'s second half** (a `ZTWorldMgr::getBuildingList` walk gating a
//!   trick's building requirement) is real, un-detoured vanilla code already safe to call through
//!   wherever it's handed a *real* `ZTShowScriptItem*` (both halves of `fillTrickLists`' UI-populate
//!   loop, and `updateAvailableTrickList`, all do exactly this). `fillTrickLists`' second half is the
//!   one exception: vanilla calls it against `ZTShowScript::getItem`'s return value - i.e. an item that
//!   lives in *this crate's* store, which has no real, byte-compatible memory representation to hand a
//!   real vanilla function. This module substitutes the corresponding real item from the unit type's own
//!   `+0x1ac` trick list (found by matching `id`) instead - see [`fill_trick_lists`]'s own doc comment
//!   for why this is a sound substitution (the two copies share the same field values in the common,
//!   non-corrupted case, which is exactly the case this check exists to confirm).
//! - **`addTrick`** (the "Add" button handler moving a selection from "available" to "assigned"; real
//!   name confirmed via macOS's `_addTrick.c` - no Windows `generated.rs` entry exists for it, so it
//!   can't be detoured yet) is left real/un-ported. It only ever reads real vanilla memory this crate
//!   doesn't own (the selected unit type's own trick list via `ZTUnitType::getTrick`,
//!   `ZTShowInfo::validateTrick`, the "assigned" listbox) - **except** one real global vector this
//!   module must keep in sync as a side effect of [`fill_trick_lists`]; see
//!   [`AVAILABLE_TRICK_IDS_BEGIN_RVA`]'s own doc comment for the mechanism and the live crash on "Add"
//!   this fixed.

use std::ffi::CString;

use openzt_detour::generated::{
    bfapp::LOAD_STRING,
    bfuimgr::GET_ELEMENT_0,
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    uilistbox::{ADD_STRING_2, CLEAR, GET_ITEM},
    uilistboxitem::DISABLE,
    ztshowinfo::{CREATE_DEFAULT_SCRIPT, VALIDATE_TRICK},
    ztshowmgr::GET_SHOW_INFO,
    ztshowscript::CONSTRUCTOR as ZTSHOW_SCRIPT_CONSTRUCTOR,
    ztui_showpanel::{FILL_TRICK_LISTS, RECALC_SHOW_STATS},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    string_registry::GLOBAL_BFAPP,
    util::{get_from_memory, save_to_memory},
    ztshow::{call_entity_vtable_u32_noargs, find_or_insert_pending_script_node},
    ztshowscriptmgr::ZTShowScriptItemRaw,
};

const AVAILABLE_TRICKS_LIST_ELEMENT_ID: i32 = 0x2b67;
const ASSIGNED_TRICKS_LIST_ELEMENT_ID: i32 = 0x2b6b;

/// The sentinel "trick" id (`ZTShowScriptItem::id == 0x2c23`) both `fillTrickLists` and
/// `_copyListToScript` special-case (never shown/matched as a normal trick, but appended to a script
/// separately - almost certainly the "return to keeper" trick, given `DAT_0063e4ac`'s own confirmed
/// config key name below).
const SENTINEL_TRICK_ID: u16 = 0x2c23;

/// `DAT_0063e44c`'s RVA - the show-editor's currently-selected `ZTHabitat*` (`ZTUI::showpanel::
/// setExhibit`'s own parameter, confirmed via `showpanel_setExhibit.c`/`.asm`). RVA = VA - `0x400000`.
const SELECTED_HABITAT_PTR_RVA: u32 = 0x0023_e44c;

/// `DAT_0063e450`'s RVA - the show-editor's currently-selected `ZTUnitType*` (`ZTUI::showpanel::
/// setSpecies`'s own parameter, confirmed via `showpanel_setSpecies.c`/`.asm`).
const SELECTED_UNIT_TYPE_PTR_RVA: u32 = 0x0023_e450;

/// `DAT_0063e4a8`'s RVA - a real, persistent flag `ZTUI::showpanel::setSpecies` also reads/writes
/// (clears it after calling `copyListToScript()` on a species change, confirmed via
/// `showpanel_setSpecies.c`) - must stay real memory, not a local variable, so that real vanilla
/// `setSpecies` continues to observe it correctly.
const MISMATCH_FLAG_RVA: u32 = 0x0023_e4a8;

/// `DAT_0063e4ac`'s RVA - `Behavior/returnToKeeperThreshold` from `shows.cfg`, loaded once by
/// `ZTUI::showpanel::init` (confirmed via `showpanel_init.c`'s own `BFConfigFile::getInt` call using
/// that exact key) - the per-segment trick-complexity budget before a "return to keeper" trick
/// ([`SENTINEL_TRICK_ID`]) gets auto-inserted. Read-only from this module's perspective.
const COMPLEXITY_BUDGET_RVA: u32 = 0x0023_e4ac;

/// `DAT_0063ba58`/`DAT_0063ba5c`/`DAT_0063ba60`'s RVAs - the real vanilla `_Myfirst`/`_Mylast`/`_Myend`
/// triple of a `std::vector<u32>` (4-byte, zero-extended-u16 trick-id slots) that real, still-un-ported
/// vanilla `addTrick` (the show-editor's "Add" button handler moving a selection from "available" to
/// "assigned" - confirmed via macOS's `_addTrick.c`; no Windows `generated.rs` entry exists for it yet,
/// its body sits in the gap `zoo_functions.rs` never attributes between `GET_SHOW_SCRIPT_ITEMS` and
/// `CLEAR_ALL`) indexes with the "available tricks" listbox's own currently-selected index to recover
/// which trick id was picked (`_DAT_102dc200[2 + selected_index]` on macOS; `.asm`'s
/// `MOV %ECX, dword ptr [EAX + 8]` shape on Windows) before looking it up via `ZTUnitType::getTrick`
/// and validating/appending it to the "assigned" list. Real vanilla `fillTrickLists` populates this
/// vector as a side effect while building the "available tricks" listbox
/// (`showpanel_fillTrickLists.c`/`.asm`: resets the size cursor to `DAT_0063ba58` up front, then pushes
/// each non-sentinel item's id, valid or not, in the exact order it's added to the listbox) - since
/// [`fill_trick_lists`] replaces that entire body, it must reproduce this side effect itself
/// ([`rewrite_available_trick_ids`]) or `addTrick` reads stale/out-of-bounds data, which is the
/// confirmed cause of a live crash on "Add" (fault EIP `~0x475a32`, inside this un-attributed real
/// function). `showpanel::fillExhibitInfo` (also real, un-ported) independently resets just the size
/// cursor (`DAT_0063ba5c = DAT_0063ba58`) when the panel is cleared - compatible with this module owning
/// the buffer's allocation, since that reset never touches `DAT_0063ba58`/`DAT_0063ba60`.
const AVAILABLE_TRICK_IDS_BEGIN_RVA: u32 = 0x0023_ba58;
const AVAILABLE_TRICK_IDS_END_RVA: u32 = 0x0023_ba5c;
const AVAILABLE_TRICK_IDS_CAP_RVA: u32 = 0x0023_ba60;

/// Rewrites the real vanilla vector at [`AVAILABLE_TRICK_IDS_BEGIN_RVA`] to hold exactly `ids`, in
/// order - the [`fill_trick_lists`] side effect real vanilla `addTrick` depends on (see that constant's
/// own doc comment). Always frees the previous buffer first (real `operator delete`, a documented no-op
/// on a null/never-allocated pointer - matches this global's zero-initialized start state) and, for a
/// non-empty `ids`, allocates a fresh buffer sized exactly to `ids.len()` (real `operator new`) rather
/// than replicating vanilla's own incremental-growth reallocation: nothing else ever appends to this
/// buffer (`addTrick` only reads it, `fillExhibitInfo` only resets the size cursor), so this function can
/// safely own the buffer's entire lifecycle on every call instead. Leaves the vector empty (all three
/// fields `0`, matching a default-constructed vector) if `ids` is empty or allocation fails.
fn rewrite_available_trick_ids(ids: &[u16]) {
    let old_buf = get_from_memory::<u32>(dat(AVAILABLE_TRICK_IDS_BEGIN_RVA));
    if old_buf != 0 {
        unsafe { OPERATOR_DELETE.original()(old_buf) };
    }
    save_to_memory(dat(AVAILABLE_TRICK_IDS_BEGIN_RVA), 0u32);
    save_to_memory(dat(AVAILABLE_TRICK_IDS_END_RVA), 0u32);
    save_to_memory(dat(AVAILABLE_TRICK_IDS_CAP_RVA), 0u32);
    if ids.is_empty() {
        return;
    }
    let new_buf = unsafe { OPERATOR_NEW.original()(ids.len() as u32 * 4) } as u32;
    if new_buf == 0 {
        return;
    }
    for (index, &id) in ids.iter().enumerate() {
        save_to_memory(new_buf + index as u32 * 4, id as u32);
    }
    let end = new_buf + ids.len() as u32 * 4;
    save_to_memory(dat(AVAILABLE_TRICK_IDS_BEGIN_RVA), new_buf);
    save_to_memory(dat(AVAILABLE_TRICK_IDS_END_RVA), end);
    save_to_memory(dat(AVAILABLE_TRICK_IDS_CAP_RVA), end);
}

/// `GLOBAL_BFUIMgr`'s own fixed address (not a pointer slot) - same constant already established in
/// `ztawardmgr.rs`/`ztresearch.rs`/`ztthoughtmgr.rs`, duplicated locally per those modules' own
/// precedent (each of those keeps its own copy rather than sharing one).
fn global_bfuimgr() -> *const u32 {
    (get_module_base("zoo.exe") as u32 + 0x0023_8de0) as *const u32
}

fn dat(rva: u32) -> u32 {
    get_module_base("zoo.exe") as u32 + rva
}

/// Loads a display string via `BFApp::loadString` (`bfapp::LOAD_STRING`). Deliberately calls
/// `.hooked()` rather than `.original()` here even though `LOAD_STRING` **is** itself detoured by
/// this crate's own `string_registry.rs` (OpenZT string-registry overrides + language-DLL
/// fallback): `hooked()` is whatever is currently installed at the address - our detour, when
/// string_registry has hooked it - so this gets the *same* registry-aware behavior any other real
/// caller of `BFApp::loadString` would see, which is exactly what trick display names should have
/// too. Returns the raw, still-encoded bytes vanilla's own loader produces (trailing NUL and any
/// unused buffer tail included) since the only consumers ([`add_available_trick`]/[`add_assigned_trick`])
/// just forward the buffer pointer on to `UIListBox::addString`, matching vanilla's own
/// `aBStack_200` usage exactly.
fn load_display_string(string_id: u16) -> [u8; 512] {
    let mut buffer = [0u8; 512];
    unsafe { LOAD_STRING.hooked()(GLOBAL_BFAPP as *const u32, string_id as u32 as *const u32, buffer.as_mut_ptr()) };
    buffer
}

/// Walks the real, un-touched vanilla `ZTUnitType` trick list looking for a node whose item `id`
/// (item+0x6) matches `target_id`. Returns the item's own address (`node+8`, real, vanilla-owned 124-byte
/// `ZTShowScriptItem` memory - safe to read, never to free).
///
/// **`unit_type_ptr + 0x1ac` is a pointer *to* the list's own separately-allocated dummy head node, not
/// an embedded sentinel node itself** - confirmed against `_copyListToScript.asm`'s own identical
/// find-by-id walk (its very first label, `.0`, pushing the sentinel trick id `0x2c23` and reading
/// `DAT_0063e450`/the selected unit type - i.e. this *is* `find_trick_by_id`'s real vanilla body):
/// `MOV EDX,[ECX+0x1ac]` loads the dummy-head pointer once (`EDX`, fixed for the whole walk), then
/// `MOV ECX,[EDX]` dereferences *that* to reach the first real node - termination compares against `EDX`
/// (the dummy head's address), never against `unit_type_ptr+0x1ac` itself. Reading only one level (as an
/// earlier version of this function did, comparing traversal nodes directly against `unit_type_ptr+0x1ac`)
/// walks real nodes forever without ever matching that address - confirmed live via
/// `ZTSHOWUI_FILL_TRICK_LISTS_LIVE`'s bounded-iteration diagnostic, which caught the one-level version
/// cycling through the same ~11 real node addresses for 1,000,000+ iterations without terminating.
fn find_trick_by_id(unit_type_ptr: u32, target_id: u16) -> Option<u32> {
    let dummy_head = get_from_memory::<u32>(unit_type_ptr + 0x1ac);
    let mut node = get_from_memory::<u32>(dummy_head);
    while node != dummy_head {
        let item_ptr = node + 8;
        if get_from_memory::<u16>(item_ptr + 0x6) == target_id {
            return Some(item_ptr);
        }
        node = get_from_memory::<u32>(node);
    }
    None
}

/// Every node in `unit_type_ptr`'s own real trick list, as item pointers (`node+8`) - see
/// [`find_trick_by_id`] for the walk/offset justification (including the dummy-head double-indirection).
/// Used by [`fill_trick_lists`]'s first half to populate the "available tricks" listbox in one pass.
fn walk_trick_list(unit_type_ptr: u32) -> Vec<u32> {
    let dummy_head = get_from_memory::<u32>(unit_type_ptr + 0x1ac);
    let mut items = Vec::new();
    let mut node = get_from_memory::<u32>(dummy_head);
    while node != dummy_head {
        items.push(node + 8);
        node = get_from_memory::<u32>(node);
    }
    items
}

/// Resolves the show-editor's currently-selected `(habitat_ptr, unit_type_ptr)` pair, `None` if either
/// selection is empty (matching every real caller's own `DAT_0063e44c`/`DAT_0063e450` null checks).
fn selected_habitat_and_unit_type() -> Option<(u32, u32)> {
    let habitat_ptr = get_from_memory::<u32>(dat(SELECTED_HABITAT_PTR_RVA));
    let unit_type_ptr = get_from_memory::<u32>(dat(SELECTED_UNIT_TYPE_PTR_RVA));
    if habitat_ptr == 0 || unit_type_ptr == 0 {
        None
    } else {
        Some((habitat_ptr, unit_type_ptr))
    }
}

/// Resolves `habitat_ptr`'s owning `ZTShowInfo*` via the real `ZTShowMgr::getShowInfo`, matching every
/// real caller's inlined `*(ushort*)(*(int*)(habitat+4)+0x70)` id lookup (`habitat+4` is `ZTHabitat`'s
/// own real `zt_show_info_ptr` field, already named and exposed by `zthabitatmgr.rs` - read raw here
/// rather than through that accessor purely because this module only needs the one dependent field, not
/// a full `ZTHabitat` wrapper).
///
/// Calls `.hooked()` rather than `.original()` because `GET_SHOW_INFO` **is** itself detoured by
/// `ztshowmgr.rs` (stage 4's read cutover onto the Rust registered-shows store): `hooked()` is
/// whatever is currently installed at the address, so this gets the *same* answer any other real
/// caller of `ZTShowMgr::getShowInfo` sees - the [`load_display_string`] precedent. A release build's
/// raw-cast `.original()` would be an accidental re-entry here while debug silently routed to
/// vanilla's tree instead.
fn show_info_for_habitat(habitat_ptr: u32) -> u32 {
    let zt_show_info_ptr = get_from_memory::<u32>(habitat_ptr + 0x4);
    let show_info_id = if zt_show_info_ptr == 0 { 0u16 } else { get_from_memory::<u16>(zt_show_info_ptr + 0x70) };
    unsafe { GET_SHOW_INFO.hooked()(globals().ztshowmgr_ptr() as *const u32, show_info_id) }
}

/// Adds one real trick-list item (`item_ptr`, real vanilla memory, see [`find_trick_by_id`]) to the
/// "available tricks" listbox, disabling it (and swapping its help id to the grayed variant) if
/// `valid` is `false` - the [`fill_trick_lists`] first-half body, factored out for clarity. `item_ptr`'s
/// `normalIcon`/`grayedIcon` string pointers are real, live vanilla memory - forwarded directly to
/// `UIListBox::addString` for the duration of this call, never copied/owned by this module.
///
/// (`clippy::manual_dangling_ptr` is a false positive below: `ADD_STRING_2`'s `p6` slot is a plain
/// integer `1`, not a pointer, per both real call sites' `.asm` - Ghidra's parameter typing is simply
/// imprecise, this isn't an actual dangling-pointer construction.)
#[allow(clippy::manual_dangling_ptr)]
fn add_available_trick(list_element: u32, item_ptr: u32, valid: bool) {
    let id = get_from_memory::<u16>(item_ptr + 0x6);
    let normal_icon = get_from_memory::<u32>(item_ptr + 0x64) as *const i32;
    let grayed_icon = get_from_memory::<u32>(item_ptr + 0x70) as *const i32;
    let normal_help_id = get_from_memory::<u32>(item_ptr + 0x5c);
    let buffer = load_display_string(id);
    let list_item = unsafe {
        ADD_STRING_2.original()(
            list_element as *const u32,
            buffer.as_ptr() as *const u32,
            normal_icon,
            std::ptr::null(),
            grayed_icon,
            0xffff_ffffu32 as *const i32,
            0,
            1 as *const i32,
            0x00ff_00ff,
            normal_help_id as *const i32,
        )
    };
    if list_item == 0 {
        return;
    }
    save_to_memory(list_item + 0x1c, id as u32);
    if !valid {
        unsafe { DISABLE.original()(list_item as *const u32, true) };
        let grayed_help_id = get_from_memory::<u32>(item_ptr + 0x60);
        save_to_memory(list_item + 0x14, grayed_help_id);
    }
}

/// Calls the real, un-detoured `ZTUI::showpanel::recalcShowStats` (`ztui_showpanel::RECALC_SHOW_STATS`,
/// hand-added to `generated.rs` - see that entry's own comment). Computes and displays the "Happiness
/// Bonus" number/smiley shown next to the show-editor's "Tricks in Show" header. This module's Rust
/// reimplementations of [`fill_trick_lists`]/[`copy_list_to_script`] replaced the *entire* body of the
/// vanilla functions that used to call this at specific points in their control flow - without explicitly
/// replicating those calls, the real vanilla body that computes that display never runs anymore, leaving
/// the happiness-bonus display stuck at whatever it last showed (observed live as a stuck `0`).
///
/// **Correction, not yet acted on beyond [`copy_list_to_script`]'s own call site: this function is NOT
/// safe to call with a real/synthetic `ZTShowScript` identity, despite an earlier version of this doc
/// comment claiming otherwise.** `showpanel_recalcShowStats.c`'s own `param_1 != 0` branch raw-walks
/// `param_1->mbr_0x10`'s embedded list directly (`for (puVar15 = *(u32**)param_1->mbr_0x10; puVar15 !=
/// param_1->mbr_0x10; puVar15 = *puVar15)`) to count items **before** ever reaching the safely
/// Stage-1-detoured `ZTShowScript::getItem` call later in the same branch - live-confirmed via
/// `crash-capture`: calling this with a synthetic handle (`ztshowscriptmgr::synthetic_script_handle`,
/// returned whenever the species already has a registered script) crashes exactly on that raw
/// dereference. Only `param_1 == 0` (the real, un-detoured "assigned tricks" listbox branch, which never
/// touches `param_1` at all) is actually safe - matching what [`copy_list_to_script`] now always passes.
/// A full reimplementation of this function (real fix, not attempted here - see that call site's own
/// comment for why) would let it safely take a real handle again.
fn recalc_show_stats(script_handle: u32) {
    unsafe { RECALC_SHOW_STATS.original()(script_handle as *const u32) };
}

/// Reimplementation of `ZTUI::showpanel::fillTrickLists`, per `showpanel_fillTrickLists.c`/`.asm`. Two
/// halves:
///
/// 1. **Available tricks** (`0x2b67`): walks the selected unit type's own real trick list
///    ([`walk_trick_list`]), validating each non-sentinel item against the real, un-detoured
///    `ZTShowInfo::validateTrick` (`FUN_0046e5bc`) - safe here since the pointer handed to it is always
///    real vanilla memory (never this crate's store). Never touches the independent store.
/// 2. **Assigned tricks** (`0x2b6b`): resolves the unit type's currently-effective script id (`pending`
///    if set, else `current`, via [`find_or_insert_pending_script_node`] - see the module doc comment's
///    point 3), falling back to `ZTShowInfo::createDefaultScript` (real, un-detoured, Stage-3-confirmed
///    safe to call through) if no non-empty script is assigned yet and the pending-node's own
///    `+0x20` flag says one should be auto-created. For each *visible* item in the resolved script
///    (read via [`crate::ztshowscriptmgr::item_full_by_id`], **not** the detoured `GET_ITEM`, which only
///    returns a sentinel - see that function's own doc comment), checks the item's `id` is still among
///    the unit type's own available tricks and passes `ZTShowInfo::validateTrick` - substituting the
///    real available-trick item found by matching `id` in place of the store's own (non-real-memory)
///    item for that call, see the module doc comment's final bullet for why this is sound. Any item that
///    fails either check sets the real, persistent `DAT_0063e4a8` mismatch flag; if set by the end,
///    calls [`copy_list_to_script`] to reconcile (matching vanilla's own tail call).
pub fn fill_trick_lists() {
    let Some((habitat_ptr, unit_type_ptr)) = selected_habitat_and_unit_type() else { return };
    let show_info = show_info_for_habitat(habitat_ptr);
    let unit_type_id = unsafe { call_entity_vtable_u32_noargs(unit_type_ptr, 0x20) };

    let all_tricks = walk_trick_list(unit_type_ptr);
    let mut available_ids: std::collections::HashSet<u16> = std::collections::HashSet::new();

    let available_list = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), AVAILABLE_TRICKS_LIST_ELEMENT_ID) } as u32;
    if available_list != 0 {
        unsafe { CLEAR.original()(available_list as *const u32) };
        let mut available_id_order: Vec<u16> = Vec::new();
        for &item_ptr in &all_tricks {
            let id = get_from_memory::<u16>(item_ptr + 0x6);
            if id == SENTINEL_TRICK_ID {
                continue;
            }
            let valid = validate_trick(show_info, item_ptr);
            add_available_trick(available_list, item_ptr, valid);
            available_ids.insert(id);
            available_id_order.push(id);
        }
        rewrite_available_trick_ids(&available_id_order);
    }

    let assigned_list = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), ASSIGNED_TRICKS_LIST_ELEMENT_ID) } as u32;
    if assigned_list == 0 {
        recalc_show_stats(0);
        return;
    }
    unsafe { CLEAR.original()(assigned_list as *const u32) };
    if available_ids.is_empty() {
        recalc_show_stats(0);
        return;
    }

    let (node, _) = find_or_insert_pending_script_node(show_info, unit_type_id);
    let pending = get_from_memory::<u16>(node + 0x1e);
    let current = get_from_memory::<u16>(node + 0x1c);
    let mut script_id = if pending != 0 && pending != 0xffff { pending } else { current };
    let mut script_exists = crate::ztshowscriptmgr::script_exists_by_id(script_id);
    let mut item_count = if script_exists { crate::ztshowscriptmgr::script_item_count_by_id(script_id) } else { 0 };

    if !script_exists || item_count == 0 {
        let flag = get_from_memory::<u8>(node + 0x20);
        if flag != 0 {
            let new_script_ptr = unsafe { CREATE_DEFAULT_SCRIPT.original()(show_info as *const u32, unit_type_id as i32) } as u32;
            if new_script_ptr == 0 {
                recalc_show_stats(0);
                return;
            }
            let new_id = get_from_memory::<u16>(new_script_ptr + 0x4);
            let size = crate::ztshowscriptmgr::script_item_count_by_id(new_id);
            if size > 0 {
                save_to_memory(node + 0x20, 0u8);
            }
            crate::ztshow::add_script(show_info, unit_type_id, new_id);
            script_id = new_id;
            script_exists = crate::ztshowscriptmgr::script_exists_by_id(new_id);
            item_count = size;
        }
    }

    if !script_exists {
        recalc_show_stats(0);
        return;
    }

    let mut mismatch = false;
    for index in 0..item_count as u16 {
        let Some(item) = crate::ztshowscriptmgr::item_full_by_id(script_id, index) else { continue };
        if !item.visible {
            continue;
        }
        let matched = available_ids.contains(&item.id)
            && find_trick_by_id(unit_type_ptr, item.id).is_some_and(|real_ptr| validate_trick(show_info, real_ptr));
        if matched {
            add_assigned_trick(assigned_list, &item);
        } else {
            mismatch = true;
        }
    }

    if mismatch {
        save_to_memory(dat(MISMATCH_FLAG_RVA), 1u8);
        copy_list_to_script();
    }
    recalc_show_stats(0);
}

/// Adds one *store-owned* item (from the currently-assigned script, not real memory - see
/// [`fill_trick_lists`]'s doc comment) to the "assigned tricks" listbox. Builds temporary
/// null-terminated buffers for the icon fields since, unlike [`add_available_trick`]'s real-memory
/// pointers, `item`'s icon strings only exist as owned Rust `String`s.
///
/// (`clippy::manual_dangling_ptr` is a false positive below - see [`add_available_trick`]'s own note.)
#[allow(clippy::manual_dangling_ptr)]
fn add_assigned_trick(list_element: u32, item: &crate::ztshowscriptmgr::ShowScriptItem) {
    let buffer = load_display_string(item.id);
    let normal_icon = CString::new(item.normal_icon.clone()).unwrap_or_default();
    let grayed_icon = CString::new(item.grayed_icon.clone()).unwrap_or_default();
    let list_item = unsafe {
        ADD_STRING_2.original()(
            list_element as *const u32,
            buffer.as_ptr() as *const u32,
            normal_icon.as_ptr() as *const i32,
            std::ptr::null(),
            grayed_icon.as_ptr() as *const i32,
            0xffff_ffffu32 as *const i32,
            0,
            1 as *const i32,
            0x00ff_00ff,
            item.normal_help_id as *const i32,
        )
    };
    if list_item != 0 {
        save_to_memory(list_item + 0x1c, item.id as u32);
    }
}

/// Calls the real, un-detoured `ZTShowInfo::validateTrick` (`ztshowinfo::VALIDATE_TRICK`, hand-added to
/// `generated.rs` - see that entry's own comment) against a *real* item pointer - safe per the module
/// doc comment's final bullet (never called with a store-owned item from this module). Return
/// convention matches every other caller in this codebase: non-zero low byte means valid.
fn validate_trick(show_info: u32, real_item_ptr: u32) -> bool {
    (unsafe { VALIDATE_TRICK.original()(show_info as *const u32, real_item_ptr as *const u32) } & 0xff) != 0
}

/// Reimplementation of `_copyListToScript` (real name `copyListToScript`, the show-editor's "apply"
/// handler), per `_copyListToScript.c`/`.asm`. Resolves (or constructs, via the real, un-detoured
/// `ZTShowScript::ZTShowScript` ctor - which itself calls through to Stage 1's real `REGISTER_SCRIPT`
/// detour, see `ztshowscriptmgr.rs`'s own doc comment on why that's safe, confirmed live by
/// `reimplementation_tests`'s `ZTSHOWSCRIPT_CTOR_REGISTRATION_LIVE` test) the unit type's *pending*
/// script id, clears it (`crate::ztshowscriptmgr::clear_all`, not the detoured FFI path - this module
/// already has the resolved store id/handle in hand), then walks the "assigned tricks" UI listbox
/// (`0x2b6b`, via the real `UIListBox::getItem`, looping until it returns `0` - confirmed via the mac
/// `UIListBox::getItem` decompile to bounds-check and return `0` past the end, sidestepping the need to
/// replicate the listbox's own internal item-count bookkeeping). Each listbox item's stored trick id
/// (`item+0x1c`, set by [`fill_trick_lists`]/`updateAvailableTrickList`) is matched against the unit
/// type's own real trick list ([`find_trick_by_id`]) and the found item is copied byte-for-byte into the
/// script via [`crate::ztshowscriptmgr::add_item`] (see the module doc comment's point 2 for why a raw
/// byte copy is correct here). Accumulates each added item's `complexity` field; once the running total
/// reaches `DAT_0063e4ac` (`Behavior/returnToKeeperThreshold`), inserts the sentinel trick
/// ([`SENTINEL_TRICK_ID`]) and resets the accumulator, matching vanilla exactly.
///
/// Returns `1` if the resulting script ended up with at least one item, `0` otherwise (including every
/// early-exit case) - matching vanilla's own `CONCAT31(_, 0 < itemCount)` return convention with the
/// upper bytes always zero here rather than garbage.
pub fn copy_list_to_script() -> u32 {
    let Some((habitat_ptr, unit_type_ptr)) = selected_habitat_and_unit_type() else { return 0 };
    let show_info = show_info_for_habitat(habitat_ptr);
    let unit_type_id = unsafe { call_entity_vtable_u32_noargs(unit_type_ptr, 0x20) };

    let (node, _) = find_or_insert_pending_script_node(show_info, unit_type_id);
    let pending_id = get_from_memory::<u16>(node + 0x1e);

    let mut script_id = pending_id;
    let mut script_handle = crate::ztshowscriptmgr::get_script(script_id);
    if script_handle == 0 {
        let alloc = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
        if alloc == 0 {
            return 0;
        }
        let ctor_ptr = unsafe { ZTSHOW_SCRIPT_CONSTRUCTOR.original()(alloc as *const u32, unit_type_id, true) } as u32;
        if ctor_ptr == 0 {
            return 0;
        }
        script_handle = ctor_ptr;
        script_id = get_from_memory::<u16>(ctor_ptr + 0x4);
    }
    crate::ztshowscriptmgr::clear_all(script_handle);

    let listbox = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), ASSIGNED_TRICKS_LIST_ELEMENT_ID) } as u32;
    if listbox == 0 {
        // Real vanilla destroys a freshly-constructed (but never listbox-not-found in practice, see the
        // module doc comment) script here; deliberately not replicated - see the module doc comment.
        return 0;
    }

    let mut complexity_accum: i64 = 0;
    let mut index = 0i32;
    loop {
        let list_item = unsafe { GET_ITEM.original()(listbox as *const u32, index) };
        if list_item == 0 {
            break;
        }
        index += 1;
        let trick_id = get_from_memory::<u32>(list_item + 0x1c) as u16;
        let Some(item_ptr) = find_trick_by_id(unit_type_ptr, trick_id) else { continue };
        let raw = unsafe { &*(item_ptr as *const ZTShowScriptItemRaw) };
        crate::ztshowscriptmgr::add_item(script_handle, raw);

        let complexity = get_from_memory::<u32>(item_ptr + 0x44);
        complexity_accum += complexity as i64;
        let budget = get_from_memory::<i32>(dat(COMPLEXITY_BUDGET_RVA));
        if complexity_accum >= budget as i64 {
            if let Some(sentinel_ptr) = find_trick_by_id(unit_type_ptr, SENTINEL_TRICK_ID) {
                let sentinel_raw = unsafe { &*(sentinel_ptr as *const ZTShowScriptItemRaw) };
                crate::ztshowscriptmgr::add_item(script_handle, sentinel_raw);
            }
            complexity_accum = 0;
        }
    }

    // TEMPORARY, INACCURATE WORKAROUND - remove once `recalc_show_stats`/`RECALC_SHOW_STATS` is properly
    // reimplemented. Passing `script_handle` here crashes real vanilla `recalcShowStats` whenever it's a
    // synthetic handle (the species already had a registered script) - it raw-dereferences `+0x10`
    // directly, see `recalc_show_stats`'s own doc comment for the live-confirmed evidence. Passing `0`
    // instead routes through vanilla's other, real-vanilla-memory-only branch (recomputes stats from the
    // "assigned tricks" listbox we just populated above, rather than from the script object), which is
    // safe but NOT byte-identical: it won't reflect a sentinel trick this function just auto-inserted for
    // exceeding the complexity budget, since the listbox never contains that sentinel. Cosmetic only (the
    // displayed happiness-bonus number/smiley), not a correctness/safety issue - but it is a real,
    // known-inaccurate stand-in, not the real fix.
    recalc_show_stats(0);
    crate::ztshow::add_script(show_info, unit_type_id, script_id);
    (crate::ztshowscriptmgr::script_item_count_by_id(script_id) > 0) as u32
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(FILL_TRICK_LISTS)]
    unsafe extern "stdcall" fn fill_trick_lists_detour() {
        fill_trick_lists();
    }
}

/// `_copyListToScript`'s own `generated.rs` entry lives under `standalone` (a free function, per
/// CLAUDE.md's note on where un-classed UI functions get namespaced) rather than `ztui_showpanel`
/// alongside `FILL_TRICK_LISTS` - detoured in its own `#[detour_mod]` block since `#[detour_mod]`
/// generates one `init_detours()` per block and this crate's convention (see `ztshow.rs`'s multiple
/// detour submodules) is one block per logically-grouped import set rather than forcing every detour in
/// a file through a single mixed-module block.
mod copy_list_to_script_detour {
    use openzt_detour::generated::standalone::COPY_LIST_TO_SCRIPT;
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::copy_list_to_script;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(COPY_LIST_TO_SCRIPT)]
        unsafe extern "stdcall" fn copy_list_to_script_detour() -> u32 {
            copy_list_to_script()
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise copy_list_to_script detour: {e:?}");
        }
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshowui detours: {e:?}");
    }
    copy_list_to_script_detour::init();
}

/// Test-only helpers for live-exercising [`fill_trick_lists`]/[`copy_list_to_script`] without driving the
/// real UI click path - see plan open item 1 and `reimplementation_tests::ZTSHOWUI_FILL_TRICK_LISTS_LIVE`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// Writes the show-editor's own selection globals directly - the same globals real `ZTUI::showpanel::
    /// setExhibit`/`setSpecies` write in response to a real click, letting a live test drive
    /// [`super::fill_trick_lists`]/[`super::copy_list_to_script`] against a real `(habitat, unit type)`
    /// pair without needing the real show-editor panel open.
    pub(crate) fn set_selection(habitat_ptr: u32, unit_type_ptr: u32) {
        save_to_memory(dat(SELECTED_HABITAT_PTR_RVA), habitat_ptr);
        save_to_memory(dat(SELECTED_UNIT_TYPE_PTR_RVA), unit_type_ptr);
    }

    /// Real vanilla trick-list length for `unit_type_ptr` ([`walk_trick_list`]'s own count) - lets a live
    /// test confirm the real `+0x1ac` field-offset chain resolves to a non-empty list, independent of
    /// whether the show-editor's own UI listbox elements exist in a headless test process (see
    /// [`ui_elements_present`]).
    pub(crate) fn trick_list_len(unit_type_ptr: u32) -> usize {
        walk_trick_list(unit_type_ptr).len()
    }

    /// Whether the show-editor's own "available"/"assigned" trick listbox UI elements currently resolve
    /// (`BFUIMgr::getElement` against [`AVAILABLE_TRICKS_LIST_ELEMENT_ID`]/[`ASSIGNED_TRICKS_LIST_ELEMENT_ID`]).
    /// Expected `false` in a headless test process unless the real show-editor panel has actually been
    /// constructed (`ZTUI::showpanel::init`/`show`) - when `false`, [`super::fill_trick_lists`]/
    /// [`super::copy_list_to_script`] take their early-return branch before touching either listbox.
    pub(crate) fn ui_elements_present() -> bool {
        let available = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), AVAILABLE_TRICKS_LIST_ELEMENT_ID) };
        let assigned = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), ASSIGNED_TRICKS_LIST_ELEMENT_ID) };
        !available.is_null() && !assigned.is_null()
    }

    /// Number of non-[`SENTINEL_TRICK_ID`] items in `unit_type_ptr`'s own real trick list - the count
    /// [`super::fill_trick_lists`] should have written into the real vanilla vector at
    /// [`AVAILABLE_TRICK_IDS_BEGIN_RVA`] the last time it ran against this unit type (with the "available
    /// tricks" listbox present). Lets a live test confirm [`rewrite_available_trick_ids`] actually kept
    /// that vector in sync, independent of re-deriving the same count a second, differently-shaped way.
    pub(crate) fn non_sentinel_trick_count(unit_type_ptr: u32) -> usize {
        walk_trick_list(unit_type_ptr)
            .into_iter()
            .filter(|&item_ptr| get_from_memory::<u16>(item_ptr + 0x6) != SENTINEL_TRICK_ID)
            .count()
    }

    /// The real vanilla vector at [`AVAILABLE_TRICK_IDS_BEGIN_RVA`], read back as owned `u32`s (trick ids,
    /// zero-extended) - what real, still-un-ported vanilla `addTrick` would index into on an "Add" click.
    /// See that constant's own doc comment for why this vector exists and what keeps it in sync.
    pub(crate) fn available_trick_id_vector() -> Vec<u32> {
        let begin = get_from_memory::<u32>(dat(AVAILABLE_TRICK_IDS_BEGIN_RVA));
        let end = get_from_memory::<u32>(dat(AVAILABLE_TRICK_IDS_END_RVA));
        if begin == 0 || end <= begin {
            return Vec::new();
        }
        let count = (end - begin) / 4;
        (0..count).map(|index| get_from_memory::<u32>(begin + index * 4)).collect()
    }
}
