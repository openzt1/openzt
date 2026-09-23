//! `ZTShowScriptState` reimplementation - the per-`(unit, show)` progress record for a running zoo
//! show.
//!
//! One instance per `(unit, show)` pair, tracking a unit's progress through a show script: the current
//! trick index plus a handful of status flags. Instances live as the values of the
//! `std::map<u32, ZTShowScriptState*>` keyed by unit id whose header sits at `ZTShowState`+`0x1c` (the
//! same memory as `ZTShow`'s own `+0x34` field, since `ZTShowState` is embedded at `ZTShow+0x18` - see
//! `ztshowstate.rs`, which owns that tree's whole lifecycle: allocation, insert, free). This module owns
//! the per-value surface - field init, save/load, trick assignment, item-count lookup - plus the
//! create/find-or-insert entry point itself ([`create_show_script_state`], detoured over
//! `generated.rs`'s mislabeled `ztshowscriptstate::CONSTRUCTOR`).
//!
//! ## Field layout (`this` = `ZTShowScriptState*`, `0x14` bytes)
//! Confirmed by `init`/`load`/`save` and the create/find-or-insert constructor agreeing exactly:
//! `+0x0` vtable ptr (always `0x0063502c` - RVA `0x23502c`, a one-slot table whose single entry is
//! `init`; never touched by this module's methods), `+0x4` `script_id: u16` (copied from the owning
//! `ZTShow`'s own `+0x4` at construction), `+0x6` padding (never read by any decompiled function),
//! `+0x8` `key: u32` (the unit id - also the tree's own map key; persisted redundantly in
//! `save`/`load`, matching vanilla), `+0xc` `trick_index: u16` (`0xffff` = "no item assigned"
//! sentinel), then six bool flags at `+0xe`-`+0x13`: `+0xe` gates `ztshow.rs`'s `do_current_item`
//! early-return, `+0xf` is `skip_scoring` (read by `do_trick_event`), `+0x11` and `+0x13` are read by
//! real, un-ported `ZTShow::run()`, `+0x12` is `trick_done` (set by `do_current_item` after a
//! completed trick call). Every flag except `+0x13` is cleared by each successful `setNextItem`;
//! `init` resets all six.
//!
//! `ZTShowScriptState` values are plain `operator_new(0x14)` allocations with the real vtable stamped
//! on. Real `ZTShowState::load` (via `ztshowstate.rs`) still constructs them itself, so that detour
//! intercepts reads/writes over already-constructed vanilla instances; the create/find-or-insert entry
//! point (`generated.rs`'s `ztshowscriptstate::CONSTRUCTOR`, `0x005a4075` - really
//! `ZTShow::createShowScriptState`, a combined find-or-insert-and-construct entry point taking a
//! `ZTShow*`, not `ZTShowScriptState*`) is ported here as [`create_show_script_state`], which does the
//! constructing for real callers through its own detour.

use std::mem;

use openzt_detour::generated::{
    standalone::{DEALLOCATE, OPERATOR_NEW, WRITE_BYTES_TO_FILE},
    // `CONSTRUCTOR` is really `ZTShow::createShowScriptState` (`0x005a4075`, a find-or-insert-and-construct
    // entry point taking a `ZTShow*`, not `ZTShowScriptState*`; belongs under `ztshow` at the next regen).
    ztshowscriptstate::{
        CONSTRUCTOR as CREATE_SHOW_SCRIPT_STATE, GET_NUM_ITEMS, INIT, LOAD, SAVE, SET_NEXT_ITEM_0,
        SET_NEXT_ITEM_1,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::get_module_base,
    util::{get_from_memory, mut_from_memory, ref_from_memory, save_to_memory},
    ztshowstate::{link_new_state_node, plan_state_node_insert, StateNodeInsertPlan},
};

/// Mirror of the vanilla `ZTShowScriptState` layout (see the module doc comment). The auto-generated
/// `openzt-detour::structs` version is mis-typed - single `u8`s over the multi-byte `script_id`/`key`/
/// `trick_index` fields - and unused; this file owns the real one.
#[repr(C)]
pub struct ZTShowScriptState {
    vtable: u32,      // 0x0 - always 0x0063502c (RVA 0x23502c); never read/written by this module
    script_id: u16,   // 0x4 - the owning ZTShow's own +0x4 script id
    pad_0x6: u16,     // 0x6 - never read by any decompiled function
    key: u32,         // 0x8 - the unit id; also the tree's own map key
    trick_index: u16, // 0xc - 0xffff = "no item assigned"
    flag_a: u8,       // 0xe - gates do_current_item's early-return
    skip_scoring: u8, // 0xf - read by do_trick_event
    flag_c: u8,       // 0x10
    flag_d: u8,       // 0x11 - read by real ZTShow::run()
    trick_done: u8,   // 0x12 - set by do_current_item after a completed trick call
    flag_f: u8,       // 0x13 - read by real ZTShow::run(); not touched by setNextItem
}

const _: () = assert!(mem::size_of::<ZTShowScriptState>() == 0x14);

/// RVA of `ZTShowScriptState`'s own vtable (`ZTShowScriptState__vftable_63502c_0063502c` in the
/// decompile corpus - VA `0x0063502c`). Stamped onto every value object - [`create_show_script_state`]'s
/// own, and [`crate::ztshowstate::show_state_load`]'s - matching real vanilla's own
/// `ZTShowState_load.c`.
pub(crate) const RVA_SCRIPT_STATE_VTABLE: u32 = 0x0023_502c;

/// Writes `value`'s raw bytes through real vanilla `WriteBytesToFile` (`standalone::WRITE_BYTES_TO_FILE`,
/// the `fwrite`-shaped primitive every `*::save` in this codebase goes through - `true` on success).
/// `.hooked()`, not `.original()`: a `reimplementation-tests` build's `io_redirect` module detours this
/// exact address to redirect the write into an in-memory capture buffer when a capture window is
/// active, and `.hooked()` is this codebase's established way to reach whatever real address currently
/// holds (see `CLAUDE.md`'s Reimplementation Pattern section) - `.original()` would bypass that
/// redirect entirely in a debug build's trampoline-routed table.
fn write_bytes_to_file<T>(value: &T, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(value as *const T as *const u32, mem::size_of::<T>() as u32, 1, file) == 1 }
}

/// Reads `value`'s raw bytes through real vanilla `deallocate` (`standalone::DEALLOCATE`, the
/// `fread`-shaped primitive despite its misleading decompiler-given name - see
/// [`write_bytes_to_file`]'s doc comment for the full `.hooked()`/`io_redirect` reasoning, which
/// applies identically here).
fn read_bytes<T>(value: &mut T, file: *const u32) -> bool {
    unsafe { DEALLOCATE.hooked()(value as *mut T as *const u32, mem::size_of::<T>() as u32, 1, file as *const u8) == 1 }
}

impl ZTShowScriptState {
    /// Reimplementation of `ZTShowScriptState::init` (`0x0061c938`, the class's one vtable slot), per
    /// `ZTShowScriptState_init.c`/`.asm`. Resets `trick_index` to the `0xffff` "no item assigned"
    /// sentinel and zeroes everything else except the vtable pointer and the never-read `+0x6` padding
    /// byte pair - matching vanilla's own field list exactly (note `+0x13` is reset here but **not** by
    /// `set_next_item`; only `init` touches it).
    pub fn init(&mut self) -> u32 {
        self.trick_index = 0xffff;
        self.script_id = 0;
        self.key = 0;
        self.flag_a = 0;
        self.skip_scoring = 0;
        self.flag_c = 0;
        self.flag_d = 0;
        self.trick_done = 0;
        self.flag_f = 0;
        1
    }

    /// Reimplementation of `ZTShowScriptState::load` (`0x0046e226`), per
    /// `ZTShowScriptState_load.c`/`.asm`. Version gate matches `save`/`load`'s shared threshold:
    /// `version <= 0x60` reads nothing at all and returns `true` (vanilla's own `if (0x60 < param_2)`
    /// guard around the entire body). The nine field reads then run unconditionally in offset order and
    /// are ANDed - vanilla reads all nine even after an earlier one failed, so there is deliberately no
    /// early return.
    pub fn load(&mut self, file: *const u32, version: u32) -> bool {
        if version <= 0x60 {
            return true;
        }

        let mut ok = read_bytes(&mut self.script_id, file);
        ok &= read_bytes(&mut self.key, file);
        ok &= read_bytes(&mut self.trick_index, file);
        ok &= read_bytes(&mut self.flag_a, file);
        ok &= read_bytes(&mut self.skip_scoring, file);
        ok &= read_bytes(&mut self.flag_c, file);
        ok &= read_bytes(&mut self.flag_d, file);
        ok &= read_bytes(&mut self.trick_done, file);
        ok &= read_bytes(&mut self.flag_f, file);
        ok
    }

    /// Reimplementation of `ZTShowScriptState::save` (`0x0061c95c`), per
    /// `ZTShowScriptState_save.c`/`.asm` - mirror of [`Self::load`]: the same nine field writes in
    /// offset order, all executed and ANDed (no early return). Returns `0`/`1` in the low byte, like
    /// every other `*::save` in this codebase - vanilla's own `CONCAT31` return construction leaves
    /// the upper 3 bytes as unrelated register garbage (the same wart `zoostatus.rs`'s `save` already
    /// declines to reproduce), so only the low byte is a defined contract.
    pub fn save(&self, file: *const i8) -> u32 {
        let mut ok = write_bytes_to_file(&self.script_id, file);
        ok &= write_bytes_to_file(&self.key, file);
        ok &= write_bytes_to_file(&self.trick_index, file);
        ok &= write_bytes_to_file(&self.flag_a, file);
        ok &= write_bytes_to_file(&self.skip_scoring, file);
        ok &= write_bytes_to_file(&self.flag_c, file);
        ok &= write_bytes_to_file(&self.flag_d, file);
        ok &= write_bytes_to_file(&self.trick_done, file);
        ok &= write_bytes_to_file(&self.flag_f, file);
        ok as u32
    }

    /// Reimplementation of `ZTShowScriptState::setNextItem(u16)` (`0x005a6778`), per
    /// `ZTShowScriptState_setNextItem_0.c`/`.asm`:
    /// - no script items (`get_num_items() < 1`): return `6`, touch nothing;
    /// - explicit `index != 0xffff` in range: store `index + 1` (vanilla's own "assigned item,
    ///   one-based" encoding, wrapping at `u16`) into `trick_index`, clear the five `+0xe`-`+0x12`
    ///   flags, return `0`;
    /// - explicit out-of-range index: return `7`, touch nothing;
    /// - `index == 0xffff` ("assign first item"): store `0`, clear the same five flags, return `0`.
    ///
    /// `+0x13` is untouched on every path - only `init` resets it.
    pub fn set_next_item(&mut self, index: u16) -> u32 {
        let n = self.get_num_items();
        if n < 1 {
            return 6;
        }
        if index != 0xffff {
            if (index as i32) < n {
                self.trick_index = index.wrapping_add(1);
                self.clear_trick_flags();
                return 0;
            }
            return 7;
        }
        self.trick_index = 0;
        self.clear_trick_flags();
        0
    }

    /// Reimplementation of `ZTShowScriptState::setNextItem()` (`0x005a67d0`), per
    /// `ZTShowScriptState_setNextItem_1.c`/`.asm` - a one-line forward to [`Self::set_next_item`] with
    /// the current `trick_index`.
    pub fn set_next_item_current(&mut self) {
        let current = self.trick_index;
        self.set_next_item(current);
    }

    /// Clears the five flags every successful `setNextItem` resets (`+0xe`-`+0x12`) - `setNextItem`'s
    /// own flag-block, factored out of [`Self::set_next_item`]'s two clearing paths.
    fn clear_trick_flags(&mut self) {
        self.flag_a = 0;
        self.skip_scoring = 0;
        self.flag_c = 0;
        self.flag_d = 0;
        self.trick_done = 0;
    }

    /// Reimplementation of `ZTShowScriptState::getNumItems` (`0x005a218f`), per
    /// `ZTShowScriptState_getNumItems.c`/`.asm` - the item count of this state's own assigned script.
    /// Called from real, un-ported `ZTShow::run` (twice) and both `setNextItem` overloads.
    ///
    /// The assigned script id is `+0x4` (confirmed via `.asm`: `word ptr [this+0x4]`) - **not**
    /// `ZTShow`'s `+0x4` field despite the coincidental offset, a different struct entirely.
    pub fn get_num_items(&self) -> i32 {
        crate::ztshowscriptmgr::script_item_count_by_id(self.script_id) as i32
    }
}

/// Reimplementation of `ZTShow::createShowScriptState` (`generated.rs`'s
/// `ztshowscriptstate::CONSTRUCTOR`, `0x005a4075` - really this function, not a `ZTShowScriptState`
/// constructor; belongs under `ztshow` at the next regen), per
/// `ZTShowScriptState_ZTShowScriptState.c`/`.asm` cross-checked against the macOS
/// `ZTShow_createShowScriptState.c`: thiscall `(ZTShow* show, u32 key) -> u32`, `RET 0x4` at all three
/// return sites. Finds `key` in the script-state tree at `ZTShow+0x34`; an existing entry returns
/// `0xffffffff` with nothing touched. Otherwise a fresh `0x14`-byte value is allocated *first*
/// (vanilla's own order - a null `operator_new` returns `5` with nothing inserted, stranding no
/// valueless tree node), zeroed (vanilla leaves `+0x6`/`+0x7` uninitialized; nothing reads them), then
/// stamped: `+0x0` the vtable, `+0x4` the `u16` script id copied from `show`'s own `+0x4`, `+0x8` the
/// key, `+0xc` the `0xffff` "no item assigned" sentinel - the six `+0xe`-`+0x13` flags are zero from the
/// zeroing. The node is then linked and the value pointer written to `node+0x14`; returns `0`.
pub fn create_show_script_state(show: u32, key: u32) -> u32 {
    let header = get_from_memory::<u32>(show + 0x34);
    let plan = plan_state_node_insert(header, key);
    if matches!(plan, StateNodeInsertPlan::Found(_)) {
        return 0xffff_ffff;
    }

    let value = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
    if value == 0 {
        return 5;
    }
    unsafe { std::ptr::write_bytes(value as *mut u8, 0, 0x14) };
    save_to_memory(value, get_module_base("zoo.exe") as u32 + RVA_SCRIPT_STATE_VTABLE);
    save_to_memory(value + 0x4, get_from_memory::<u16>(show + 0x4));
    save_to_memory(value + 0x8, key);
    save_to_memory(value + 0xc, 0xffffu16);

    let node = link_new_state_node(header, key, plan);
    save_to_memory(node + 0x14, value);
    0
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(INIT)]
    unsafe extern "thiscall" fn init_detour(this: *const u32) -> u32 {
        unsafe { mut_from_memory::<ZTShowScriptState>(this) }.init()
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load_detour(this: *const u32, file: *const u32, version: u32) -> bool {
        unsafe { mut_from_memory::<ZTShowScriptState>(this) }.load(file, version)
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTShowScriptState>(this) }.save(file as *const i8) != 0
    }

    #[detour(SET_NEXT_ITEM_0)]
    unsafe extern "thiscall" fn set_next_item_detour(this: *const u32, index: u16) -> u32 {
        unsafe { mut_from_memory::<ZTShowScriptState>(this) }.set_next_item(index)
    }

    #[detour(SET_NEXT_ITEM_1)]
    unsafe extern "thiscall" fn set_next_item_current_detour(this: *const u32) {
        unsafe { mut_from_memory::<ZTShowScriptState>(this) }.set_next_item_current();
    }

    #[detour(GET_NUM_ITEMS)]
    unsafe extern "thiscall" fn get_num_items_detour(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTShowScriptState>(this) }.get_num_items()
    }

    #[detour(CREATE_SHOW_SCRIPT_STATE)]
    unsafe extern "thiscall" fn create_show_script_state_detour(this: *const u32, key: u32) -> u32 {
        create_show_script_state(this as u32, key)
    }

    /// Trampolines to the real vanilla bodies for the battery's "real vanilla" pole - live inside the
    /// detour module because the generated `*_DETOUR` statics are module-private. `.original()` on a
    /// hooked address is a raw cast in release and would re-enter this port's own detour there (debug
    /// `.original()` routes through openzt-detour's hook registry, but the vanilla pole must hold in
    /// every profile - `ztgamemgr_menumusichandler`'s `test_real` precedent).
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn create_show_script_state(show: u32, key: u32) -> u32 {
            unsafe { super::CREATE_SHOW_SCRIPT_STATE_DETOUR.call(show as *const u32, key) }
        }

        pub(crate) fn init(this: u32) -> u32 {
            unsafe { super::INIT_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn load(this: u32, file: *const u32, version: u32) -> bool {
            unsafe { super::LOAD_DETOUR.call(this as *const u32, file, version) }
        }

        pub(crate) fn save(this: u32, file: *const u32) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }

        pub(crate) fn set_next_item(this: u32, index: u16) -> u32 {
            unsafe { super::SET_NEXT_ITEM_0_DETOUR.call(this as *const u32, index) }
        }

        pub(crate) fn set_next_item_current(this: u32) {
            unsafe { super::SET_NEXT_ITEM_1_DETOUR.call(this as *const u32) }
        }
    }
}

/// **Wiring note**: this module's detours are installed from two different places depending on how the
/// DLL was entered - the real game's normal boot path (`lib.rs`'s `LOAD_LANG_DLLS_DETOUR` hook, gated on
/// the `experimental` feature) calls this `init()` directly, but `openzt-test-dll` (the
/// `reimplementation-tests` binary the live battery runs under) never reaches that hook at all - its own
/// `DllMain` calls `reimplementation_tests::init()` instead, which has its own explicit list of which
/// modules' detours to install for the battery. **Both call sites must list this module** - missing it
/// from the `reimplementation_tests::init()` list means every address this module detours silently
/// falls back to real vanilla (or, for `GET_NUM_ITEMS`, goes completely un-hooked after its detour
/// moved here from `ztshow.rs`), with no error logged anywhere.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshowscriptstate detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;
    use openzt_detour::generated::standalone::OPERATOR_DELETE;

    /// `(name, is_enabled)` per detour - see `detours::status()`'s own doc comment.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }

    /// Vanilla-pole trampolines - see `detours::test_real`'s own doc comment for why these route
    /// through `_DETOUR.call` rather than `.original()`.
    pub(crate) fn real_init(this: u32) -> u32 {
        super::detours::test_real::init(this)
    }

    pub(crate) fn real_load(this: u32, file: *const u32, version: u32) -> bool {
        super::detours::test_real::load(this, file, version)
    }

    pub(crate) fn real_save(this: u32, file: *const u32) -> bool {
        super::detours::test_real::save(this, file)
    }

    pub(crate) fn real_set_next_item(this: u32, index: u16) -> u32 {
        super::detours::test_real::set_next_item(this, index)
    }

    pub(crate) fn real_set_next_item_current(this: u32) {
        super::detours::test_real::set_next_item_current(this)
    }

    /// Trampoline to the real vanilla `ZTShow::createShowScriptState` body for the battery's "real
    /// vanilla" pole - see `detours::test_real`'s own doc comment.
    pub(crate) fn real_create_show_script_state(show: u32, key: u32) -> u32 {
        super::detours::test_real::create_show_script_state(show, key)
    }

    /// Allocates a standalone `ZTShowScriptState` fixture the same way [`super::create_show_script_state`]
    /// and `ztshowstate::show_state_load` build their own values: `operator_new(0x14)`, zeroed, the real
    /// vtable stamped at `+0x0`. No vanilla constructor call is needed - vanilla constructs these values
    /// inline in its own find-or-insert entry point, field-by-field, exactly like this.
    pub(crate) fn build_standalone_script_state() -> u32 {
        let buf = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
        unsafe { std::ptr::write_bytes(buf as *mut u8, 0, 0x14) };
        save_to_memory(buf, get_module_base("zoo.exe") as u32 + RVA_SCRIPT_STATE_VTABLE);
        buf
    }

    /// Tears down a fixture built by [`build_standalone_script_state`]. No cross-allocator surface:
    /// fixtures are this module's own `OPERATOR_NEW` allocations, and neither the real vanilla bodies
    /// (`init`/`save`/`load`/`setNextItem` are pure field operations) nor this port ever frees `this`.
    pub(crate) fn destroy_standalone_script_state(buf: u32) {
        unsafe { OPERATOR_DELETE.original()(buf) };
    }

    /// Builds a standalone `(show, header)` pair for exercising [`super::create_show_script_state`] /
    /// [`real_create_show_script_state`] end to end: `show` is a synthetic `ZTShow*` stand-in
    /// populated only at the two offsets the real function actually reads off it (`+0x4` script id,
    /// `+0x34` the script-state tree header pointer - confirmed via
    /// `ZTShowScriptState_ZTShowScriptState.c`; nothing else is touched, no vtable dispatch happens on
    /// `show` itself, so an otherwise-zeroed stand-in is safe to hand to the real vanilla body too).
    /// `header` is a fresh, properly-sentineled *empty* tree header (`0x18` bytes: `root` `0`,
    /// `leftmost`/`rightmost` self-referencing - matching `ztshowstate.rs`'s own confirmed empty-tree
    /// shape; real vanilla's own insert-with-hint STL routine reads that cache on every insert, so an
    /// incorrectly-shaped header risks a bad dereference there, not just a wrong Rust-side answer).
    /// Both blocks are real-allocator memory (`OPERATOR_NEW`), matching this module's other
    /// standalone-fixture helpers; deliberately never freed - a one-shot test-process fixture, same
    /// leak-only precedent as [`build_standalone_script_state`] and `ztshow.rs`'s
    /// `build_standalone_show_info`.
    pub(crate) fn build_standalone_show_and_header(script_id: u16) -> (u32, u32) {
        let show = unsafe { OPERATOR_NEW.original()(0x38) } as u32;
        unsafe { std::ptr::write_bytes(show as *mut u8, 0, 0x38) };
        save_to_memory(show + 0x4, script_id);

        let header = unsafe { OPERATOR_NEW.original()(0x18) } as u32;
        unsafe { std::ptr::write_bytes(header as *mut u8, 0, 0x18) };
        save_to_memory(header + 0x8, header);
        save_to_memory(header + 0xc, header);

        save_to_memory(show + 0x34, header);
        (show, header)
    }
}
