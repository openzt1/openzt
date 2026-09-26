use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, mut_from_memory, ref_from_memory},
};

#[allow(clippy::module_inception)]
pub mod ztthoughtmgr;
pub use ztthoughtmgr::*;

/// Registers this module's live detours: the UI-consumer detours, the raw-accessor observability
/// detours, the mutator detours (`addThought`/`removeThoughtsBy{Thinker,Object,Habitat}`), the
/// save/load-family detours (`save`/`load`/`populateThoughts`), and the destructor detour.
pub fn init() {
    thought_ui_detours::init();
    thought_accessor_detours::init();
    thought_mutator_detours::init();
    thought_save_detours::init();
    thought_dtor_detour::init();
}

/// Detours `ZTThoughtMgr`'s three read-only accessors - `getThoughtsBy{Thinker,Object,Habitat}` - purely
/// for observability, not behavior. An exhaustive search of the decompiled call-graph corpus in
/// `private/resources/decompiles` found exactly three callers of these three addresses -
/// `_fillListBox_0`/`_fillListBox_1`/`_refillThoughtsList` - all three fully replaced by
/// [`thought_ui_detours`], which calls the `Vec`-returning `get_thoughts_by_*` methods directly - so
/// nothing known ever reaches these detours at all.
///
/// Reimplementing these accessors properly would mean synthesizing a vanilla-shaped output
/// `std::list<ZTThought>` into the caller's out-param, which the caller tears down afterward with
/// vanilla's own *inlined* freelist push - a cross-allocator heap corruption if we built the list with
/// `Box` - and there's no confirmed Windows address for the generic small-object allocator that would
/// let us build a genuinely vanilla-freeable list. With zero known callers, that work isn't justified:
/// each detour just logs (so a real hit gets noticed) and falls through to the real vanilla body via the
/// `<NAME>_DETOUR.call(...)` trampoline (`.original()` would recurse into the detour itself in release),
/// which only reads `this`'s own `sentinel_ptr` - still a genuine, permanently self-referencing (i.e.
/// permanently empty) sentinel node. Worst case, an undiscovered caller sees an always-empty result - a
/// cosmetic gap, never a crash.
///
/// **Flagged for deletion once confident.** This module exists purely to catch a caller the Windows
/// decompile-corpus grep above might have missed (it wasn't cross-checked against the macOS corpus, and a
/// grep can't see a computed/indirect call). If the `error!` in each detour below never fires across
/// enough real play/testing to trust that absence, delete this module (and its `init()` call) entirely -
/// three plain, un-detoured addresses are strictly simpler than three detours that only log and call
/// through, for the same runtime behavior either way.
mod thought_accessor_detours {
    use openzt_detour::generated::ztthoughtmgr::{GET_THOUGHTS_BY_HABITAT, GET_THOUGHTS_BY_OBJECT, GET_THOUGHTS_BY_THINKER};
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    #[detour_mod]
    mod detours {
        use super::*;

        /// `max_count` is declared `*const i32` in `generated.rs`, but per
        /// `ZTThoughtMgr_getThoughtsByObject.c` it's actually passed by value (a Ghidra type-inference
        /// artifact on this parameter, unlike `getThoughtsByThinker`'s correctly-inferred `i32`) - logged
        /// via a raw cast back to `i32`, never dereferenced.
        #[detour(GET_THOUGHTS_BY_OBJECT)]
        unsafe extern "thiscall" fn get_thoughts_by_object(this: *const u32, out: *const i32, object_ptr: *const i32, max_count: *const i32) -> *const i32 {
            error!(
                "GET_THOUGHTS_BY_OBJECT invoked directly (this={this:p}, object_ptr={object_ptr:p}, max_count={}) - no known caller; falling \
                 through to vanilla, which reads the permanently-empty sentinel and returns zero matches",
                max_count as i32
            );
            unsafe { GET_THOUGHTS_BY_OBJECT_DETOUR.call(this, out, object_ptr, max_count) }
        }

        /// See `get_thoughts_by_object`'s doc comment re: `max_count`'s pointer typing.
        #[detour(GET_THOUGHTS_BY_HABITAT)]
        unsafe extern "thiscall" fn get_thoughts_by_habitat(this: *const u32, out: *const i32, habitat_ptr: *const i32, max_count: *const i32) -> *const i32 {
            error!(
                "GET_THOUGHTS_BY_HABITAT invoked directly (this={this:p}, habitat_ptr={habitat_ptr:p}, max_count={}) - no known caller; falling \
                 through to vanilla, which reads the permanently-empty sentinel and returns zero matches",
                max_count as i32
            );
            unsafe { GET_THOUGHTS_BY_HABITAT_DETOUR.call(this, out, habitat_ptr, max_count) }
        }

        #[detour(GET_THOUGHTS_BY_THINKER)]
        unsafe extern "thiscall" fn get_thoughts_by_thinker(this: *const u32, out: *const i32, thinker_ptr: *const i32, max_count: i32) -> *const i32 {
            error!(
                "GET_THOUGHTS_BY_THINKER invoked directly (this={this:p}, thinker_ptr={thinker_ptr:p}, max_count={max_count}) - no known caller; \
                 falling through to vanilla, which reads the permanently-empty sentinel and returns zero matches"
            );
            unsafe { GET_THOUGHTS_BY_THINKER_DETOUR.call(this, out, thinker_ptr, max_count) }
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztthoughtmgr raw-accessor observability detours: {e:?}");
        }
    }
}

/// Detours the three UI functions that are `getThoughtsBy*`'s only consumers of the vanilla `std::list`
/// it builds - `_fillListBox` (both instantiations) and `_refillThoughtsList`. Each calls the
/// `Vec`-returning accessors above directly and drives
/// `BFUIMgr::getElement`/`UIListBox::clear`/`addString`/`restoreState` (all real vanilla functions,
/// called via `.original()`, never detoured themselves) in a loop instead. This is what makes the
/// `Vec` return type viable: with these three detoured, nothing vanilla-side ever constructs or walks a
/// `getThoughtsBy*` result as a real intrusive list.
mod thought_ui_detours {
    use openzt_detour::generated::{
        bfuimgr::GET_ELEMENT_0,
        standalone::{FILL_LIST_BOX_0, FILL_LIST_BOX_1, REFILL_THOUGHTS_LIST},
        uilistbox::{ADD_STRING_0, CLEAR, RESTORE_STATE},
    };
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::*;
    use crate::{
        encoding_utils::encode_to_ansi,
        util::{save_to_memory, ZTBufferString},
    };

    /// `GLOBAL_BFUIMgr`'s own fixed address.
    fn global_bfuimgr() -> *const u32 {
        (get_module_base("zoo.exe") as u32 + 0x0023_8de0) as *const u32
    }

    /// The `BFUIMgr::getElement` ids the three detoured functions each look up: the two `_fillListBox`
    /// instantiations are byte-identical twins parameterized only by this id and which `getThoughtsBy*`
    /// they call.
    const OBJECT_THOUGHTS_LIST_ELEMENT_ID: i32 = 0xc35;
    const THINKER_THOUGHTS_LIST_ELEMENT_ID: i32 = 0xd8d;
    const HABITAT_THOUGHTS_LIST_ELEMENT_ID: i32 = 0x10ea;

    /// Per-call-site match caps: both `_fillListBox` instantiations request at most 5,
    /// `_refillThoughtsList` requests at most `0x14` (20).
    const OBJECT_OR_THINKER_THOUGHTS_MAX_COUNT: usize = 5;
    const HABITAT_THOUGHTS_MAX_COUNT: usize = 20;

    /// The "habitat info" UI window's own currently-displayed habitat, a plain `*ZTHabitat` global
    /// entirely outside `ZTThoughtMgr`'s own state (set by `habitatinfo_setHabitat`, cleared by
    /// `habitatinfo_remove{Habitat,AllHabitats}` - neither reimplemented here). `_refillThoughtsList`
    /// reads it directly rather than taking it as a parameter.
    const CURRENT_HABITAT_INFO_HABITAT_PTR_ADDR: u32 = 0x0023_915c;

    fn current_habitat_info_habitat_ptr() -> u32 {
        get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + CURRENT_HABITAT_INFO_HABITAT_PTR_ADDR)
    }

    /// The fixed styling args every `addString` call site in this file passes: four zeroed `*const i32`
    /// slots, a zero `u8` flag, a `1`-valued `*const i32` slot (a raw sentinel value, not a real
    /// pointer), the display color, and a final null `*const i32`.
    const THOUGHT_LIST_ITEM_COLOR: u32 = 0x00ff_00ff;

    /// Builds a temporary `ZTBufferString`-shaped buffer for `text` and hands it to the real
    /// `UIListBox::addString`. Unlike vanilla, which heap-allocates the buffer via its own
    /// small-object allocator and frees it once `addString` returns (`addString` copies the string
    /// into its own permanently-owned storage before returning), this just uses a plain Rust `Vec` for
    /// the temporary buffer and lets it drop normally.
    #[allow(clippy::manual_dangling_ptr)] // literal sentinel value `1`, not a real pointer - `ptr::dangling` would substitute a different bit pattern (alignment-sized, not `1`)
    fn add_thought_to_list_box(list_box: *const u32, text: &str) {
        let mut encoded = encode_to_ansi(text);
        let len = encoded.len() as u32;
        encoded.push(0);
        let start = encoded.as_ptr() as u32;
        let text_buffer = ZTBufferString::from_raw_parts(start, start + len, start + encoded.len() as u32);
        unsafe {
            ADD_STRING_0.original()(
                list_box,
                &text_buffer as *const ZTBufferString as *const u32,
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                std::ptr::null(),
                0,
                1 as *const i32,
                THOUGHT_LIST_ITEM_COLOR,
                std::ptr::null(),
            );
        }
    }

    #[detour_mod]
    mod detours {
        use super::*;

        /// The `_fillListBox` instantiation at `0x00467a33`, using
        /// [`OBJECT_THOUGHTS_LIST_ELEMENT_ID`]/[`get_thoughts_by_object`].
        #[detour(FILL_LIST_BOX_0)]
        unsafe extern "cdecl" fn fill_object_thoughts_list_box(object_ptr: *const i32) {
            let element = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), OBJECT_THOUGHTS_LIST_ELEMENT_ID) };
            if element.is_null() {
                return;
            }
            unsafe { CLEAR.original()(element) };
            for thought in globals().ztthoughtmgr().get_thoughts_by_object(object_ptr as u32, OBJECT_OR_THINKER_THOUGHTS_MAX_COUNT) {
                add_thought_to_list_box(element, &thought.get_string());
            }
        }

        /// The `_fillListBox` instantiation at `0x0046a040`, using
        /// [`THINKER_THOUGHTS_LIST_ELEMENT_ID`]/[`get_thoughts_by_thinker`].
        #[detour(FILL_LIST_BOX_1)]
        unsafe extern "cdecl" fn fill_thinker_thoughts_list_box(thinker_ptr: *const i32) {
            let element = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), THINKER_THOUGHTS_LIST_ELEMENT_ID) };
            if element.is_null() {
                return;
            }
            unsafe { CLEAR.original()(element) };
            for thought in globals().ztthoughtmgr().get_thoughts_by_thinker(thinker_ptr as u32, OBJECT_OR_THINKER_THOUGHTS_MAX_COUNT) {
                add_thought_to_list_box(element, &thought.get_string());
            }
        }

        /// `preserve_scroll`: when set, the current scroll position is snapshotted into the list box's
        /// own save-slot fields before the list is cleared and repopulated, so that the real
        /// `UIListBox::restoreState` call afterward has something to restore.
        #[detour(REFILL_THOUGHTS_LIST)]
        unsafe extern "cdecl" fn refill_thoughts_list(preserve_scroll: i8) {
            let element = unsafe { GET_ELEMENT_0.original()(global_bfuimgr(), HABITAT_THOUGHTS_LIST_ELEMENT_ID) };
            if element.is_null() {
                return;
            }

            let habitat_ptr = current_habitat_info_habitat_ptr();
            if habitat_ptr == 0 {
                unsafe { CLEAR.original()(element) };
                return;
            }

            if preserve_scroll != 0 {
                let element_addr = element as u32;
                let scroll_offset = get_from_memory::<u32>(element_addr + 0x36c);
                let scroll_extent = get_from_memory::<u32>(element_addr + 0x37c);
                save_to_memory::<u8>(element_addr + 0x398, 1);
                save_to_memory::<u32>(element_addr + 0x39c, scroll_offset);
                save_to_memory::<u32>(element_addr + 0x3a0, scroll_extent);
            }

            unsafe { CLEAR.original()(element) };
            for thought in globals().ztthoughtmgr().get_thoughts_by_habitat(habitat_ptr, HABITAT_THOUGHTS_MAX_COUNT) {
                add_thought_to_list_box(element, &thought.get_string());
            }
            if preserve_scroll != 0 {
                unsafe { RESTORE_STATE.original()(element) };
            }
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztthoughtmgr UI-consumer detours: {e:?}");
        }
    }
}

/// Detours `ZTThoughtMgr`'s four mutating entry points - `addThought` and
/// `removeThoughtsBy{Thinker,Object,Habitat}` - onto the `impl ZTThoughtMgr` methods of the same name.
/// Any vanilla code path that inserts or removes a node must go through `insert_front`/`remove_where`'s
/// `Box`-based allocator, never vanilla's own freelist one, so these four are detoured at their real
/// addresses (unlike the read-only `getThoughtsBy*` accessors, which are safe against either
/// allocator and left un-detoured).
mod thought_mutator_detours {
    use openzt_detour::generated::ztthoughtmgr::{
        ADD_THOUGHT, REMOVE_THOUGHTS_BY_HABITAT, REMOVE_THOUGHTS_BY_OBJECT, REMOVE_THOUGHTS_BY_THINKER,
    };
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::*;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(ADD_THOUGHT)]
        unsafe extern "thiscall" fn add_thought(this: *const u32, string_id: u32, thinker_ptr: *const u32, object_ptr: *const u32, habitat_ptr: *const u32) {
            unsafe { mut_from_memory::<ZTThoughtMgr>(this) }.add_thought(string_id, thinker_ptr as u32, object_ptr as u32, habitat_ptr as u32);
        }

        #[detour(REMOVE_THOUGHTS_BY_THINKER)]
        unsafe extern "thiscall" fn remove_thoughts_by_thinker(this: *const u32, thinker_ptr: *const u32) {
            unsafe { mut_from_memory::<ZTThoughtMgr>(this) }.remove_thoughts_by_thinker(thinker_ptr as u32);
        }

        #[detour(REMOVE_THOUGHTS_BY_OBJECT)]
        unsafe extern "thiscall" fn remove_thoughts_by_object(this: *const u32, object_ptr: *const u32) {
            unsafe { mut_from_memory::<ZTThoughtMgr>(this) }.remove_thoughts_by_object(object_ptr as u32);
        }

        #[detour(REMOVE_THOUGHTS_BY_HABITAT)]
        unsafe extern "thiscall" fn remove_thoughts_by_habitat(this: *const u32, habitat_ptr: *const i32, force: i8) {
            unsafe { mut_from_memory::<ZTThoughtMgr>(this) }.remove_thoughts_by_habitat(habitat_ptr as u32, force != 0);
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztthoughtmgr mutator detours: {e:?}");
        }
    }
}

/// Detours `ZTThoughtMgr::save`/`load`/`populateThoughts` onto the `impl ZTThoughtMgr` methods of the
/// same name. Like the mutator detours, these must be real detours rather than plain Rust helpers:
/// `ZTWorldMgr::load` (out of scope here, left as vanilla) calls `ZTThoughtMgr::load`/`populateThoughts`
/// itself depending on save version.
mod thought_save_detours {
    use openzt_detour::generated::ztthoughtmgr::{LOAD, POPULATE_THOUGHTS, SAVE};
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::*;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(SAVE)]
        unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
            unsafe { ref_from_memory::<ZTThoughtMgr>(this) }.save(file)
        }

        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
            unsafe { mut_from_memory::<ZTThoughtMgr>(this) }.load(file, version)
        }

        #[detour(POPULATE_THOUGHTS)]
        unsafe extern "thiscall" fn populate_thoughts(this: *const u32) {
            unsafe { mut_from_memory::<ZTThoughtMgr>(this) }.populate_thoughts();
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztthoughtmgr save/load detours: {e:?}");
        }
    }
}

/// Detours `ZTThoughtMgr`'s vtable destructor slot - the scalar deleting destructor at `0x0057d852`
/// (`ztthoughtmgr::DESTRUCTOR_1` in `generated.rs`) - onto [`ZTThoughtMgr::clear`]. Vanilla's own version of this
/// function calls the real destructor body, then conditionally calls `operator delete` on `this` if the
/// caller-supplied flag byte's low bit is set. Since `ZTThoughtMgr` is a process-lifetime singleton and
/// no address for the real vanilla `operator delete` this class would use is known or needed, this
/// reimplementation only ever frees the list's own `Box`-allocated nodes and never the flag-gated
/// `this` itself. `ztthoughtmgr::DESTRUCTOR_0` (`0x0057d815`, the real destructor's own address, only ever reached
/// indirectly through this wrapper) is intentionally left un-detoured: nothing else in vanilla calls it
/// directly.
mod thought_dtor_detour {
    use openzt_detour::generated::ztthoughtmgr::DESTRUCTOR_1 as ZTTHOUGHTMGR_DESTRUCTOR;
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    use super::*;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(ZTTHOUGHTMGR_DESTRUCTOR)]
        unsafe extern "thiscall" fn ztthoughtmgr_dtor(this: *const u32, _flags: u8) -> *const u32 {
            unsafe { mut_from_memory::<ZTThoughtMgr>(this) }.clear();
            this
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztthoughtmgr destructor detour: {e:?}");
        }
    }
}

/// Live-comparison test support for `reimplementation_tests`. Production `ZTThoughtMgr` methods never
/// touch `sentinel_ptr`'s raw memory (the persistent list lives in [`THOUGHT_STORES`]), so the
/// live-comparison suite needs explicit helpers to drive and read a genuine vanilla-shaped `ThoughtNode`
/// chain: the registry-based helpers production code also uses
/// (`build_standalone_mgr`/`destroy_standalone_mgr`), plus `*_raw_chain*` helpers that operate directly
/// on `sentinel_ptr`'s intrusive chain, for seeding/reading instances a test drives through a real,
/// undetoured `.original()` call.
///
/// Nothing here ever frees vanilla-allocated memory through `Box`, or `Box`-allocated memory through
/// vanilla's own freelist - see each function's own doc comment for which allocator it assumes.
#[cfg(feature = "reimplementation-tests")]
pub mod live_support {
    use super::*;
    use super::super::thought::ZTThought;
    use crate::globals::get_module_base;

    /// A persistent-list node: 8 bytes of intrusive links followed by the `ZTThought` payload at `+0x8`.
    /// The sentinel node vanilla allocates at startup shares this same link layout (its `data` is never
    /// read). Test-only: no production code walks this layout.
    #[repr(C)]
    pub struct ThoughtNode {
        pub next: *mut ThoughtNode,
        pub prev: *mut ThoughtNode,
        pub data: ZTThought,
    }

    /// Builds a `ZTThought` with every field directly settable - unlike `ZTThought::new`, which
    /// dereferences `thinker_ptr`/`object_ptr`/`habitat_arg` when non-null to resolve
    /// `thinker_id`/`object_id`/the habitat-flag gate. Several live comparisons need fields set
    /// directly without running that resolution logic.
    ///
    /// `vtable` is set to the same real `ZTThought` vtable address `ZTThought::new` itself uses, not
    /// `0`: `ZTThoughtMgr::save` dispatches through each node's own `data.vtable` slot 0 rather than
    /// calling `ZTThought::save` directly, so every node reachable from a real vanilla call needs a
    /// genuinely valid vtable, not just correct data fields.
    #[allow(clippy::too_many_arguments)]
    pub fn new_thought(
        string_id: u32,
        thinker_id: u32,
        object_id: u32,
        tile_x: i32,
        tile_y: i32,
        thinker_ptr: u32,
        object_ptr: u32,
        habitat_ptr: u32,
    ) -> ZTThought {
        let vtable = get_module_base("zoo.exe") as u32 + 0x0023_5400;
        ZTThought { vtable, string_id, thinker_id, object_id, tile_x, tile_y, thinker_ptr, object_ptr, habitat_ptr }
    }

    /// Builds a standalone `ZTThoughtMgr` with a freshly heap-allocated, self-referencing sentinel node,
    /// never spliced into the real singleton. Heap-allocates the `ZTThoughtMgr` itself too (returned as
    /// a raw pointer, for passing to real vanilla `.original()()` calls). The sentinel is a genuine,
    /// self-referencing `ThoughtNode` (not just an opaque placeholder): any real, undetoured vanilla call
    /// against this instance (`ADD_THOUGHT.original()`, `GET_THOUGHTS_BY_THINKER.original()`, ...) reads
    /// `sentinel_ptr` as a real intrusive-list pointer and needs it to be one. Reimplemented-side methods
    /// never dereference it, only using its value as a [`THOUGHT_STORES`] key, so the exact same
    /// standalone instance works for both sides.
    pub fn build_standalone_mgr(max_thoughts: u32) -> *mut ZTThoughtMgr {
        let sentinel = Box::into_raw(Box::new(ThoughtNode {
            next: std::ptr::null_mut(),
            prev: std::ptr::null_mut(),
            data: new_thought(0, 0, 0, -1, -1, 0, 0, 0),
        }));
        unsafe {
            (*sentinel).next = sentinel;
            (*sentinel).prev = sentinel;
        }
        Box::into_raw(Box::new(ZTThoughtMgr { vtable: 0, flag: 0, _pad: [0; 3], sentinel_ptr: sentinel as u32, max_thoughts }))
    }

    /// Seeds `thought` into `mgr.sentinel_ptr`'s intrusive chain via direct raw-pointer manipulation,
    /// using `Box` to allocate the node. Use only to prep fixtures for calls that will run through real
    /// vanilla code that *reads* (but does not free/allocate) nodes, or before tearing down with
    /// [`free_raw_chain_mgr`].
    pub fn seed_raw_chain(mgr: &ZTThoughtMgr, thought: ZTThought) {
        let sentinel = mgr.sentinel_ptr as *mut ThoughtNode;
        let old_front = unsafe { (*sentinel).next };
        let node = Box::into_raw(Box::new(ThoughtNode { next: old_front, prev: sentinel, data: thought }));
        unsafe {
            (*old_front).prev = node;
            (*sentinel).next = node;
        }
    }

    /// Reads the raw intrusive chain reachable from `mgr.sentinel_ptr` front-to-back, returning a copy
    /// of each node's `data`. Used to read results after running an operation through vanilla's real
    /// `ADD_THOUGHT.original()`.
    pub fn read_raw_chain(mgr: &ZTThoughtMgr) -> Vec<ZTThought> {
        let sentinel = mgr.sentinel_ptr as *mut ThoughtNode;
        let mut result = Vec::new();
        let mut curr = unsafe { (*sentinel).next };
        while curr != sentinel && !curr.is_null() {
            result.push(unsafe { (*curr).data });
            curr = unsafe { (*curr).next };
        }
        result
    }

    /// Frees every non-sentinel node in `mgr.sentinel_ptr`'s intrusive chain via `Box::from_raw`.
    ///
    /// # Safety
    /// Every node in the chain MUST have been allocated by Rust (`Box`), not by vanilla code.
    fn free_raw_chain_nodes(mgr: &ZTThoughtMgr) {
        let sentinel = mgr.sentinel_ptr as *mut ThoughtNode;
        if sentinel.is_null() {
            return;
        }
        let mut curr = unsafe { (*sentinel).next };
        while curr != sentinel && !curr.is_null() {
            let next = unsafe { (*curr).next };
            unsafe { drop(Box::from_raw(curr)) };
            curr = next;
        }
        unsafe {
            (*sentinel).next = sentinel;
            (*sentinel).prev = sentinel;
        }
    }

    /// Frees `ptr` and every node in its intrusive chain via `Box::from_raw`. Use only when every node
    /// was allocated by [`seed_raw_chain`] (or left empty by [`build_standalone_mgr`]).
    pub fn free_raw_chain_mgr(ptr: *mut ZTThoughtMgr) {
        if ptr.is_null() {
            return;
        }
        free_raw_chain_nodes(unsafe { &*ptr });
        let sentinel = unsafe { (*ptr).sentinel_ptr } as *mut ThoughtNode;
        if !sentinel.is_null() {
            unsafe { drop(Box::from_raw(sentinel)) };
        }
        unsafe { drop(Box::from_raw(ptr)) };
    }

    /// Tears down a standalone manager where BOTH sides (reimplemented and vanilla raw chain) need
    /// cleanup. The raw-chain side must have been populated only by [`seed_raw_chain`], never vanilla.
    pub fn destroy_standalone_mgr_both(ptr: *mut ZTThoughtMgr) {
        if ptr.is_null() {
            return;
        }
        unsafe { (*ptr).clear() };
        free_raw_chain_mgr(ptr);
    }

    /// Tears down a standalone manager whose persistent list lives in [`THOUGHT_STORES`]. Its intrusive
    /// chain must be empty (the sentinel node alone is freed).
    pub fn destroy_standalone_mgr(ptr: *mut ZTThoughtMgr) {
        if ptr.is_null() {
            return;
        }
        unsafe { (*ptr).clear() };
        let sentinel = unsafe { (*ptr).sentinel_ptr } as *mut ThoughtNode;
        if !sentinel.is_null() {
            unsafe { drop(Box::from_raw(sentinel)) };
        }
        unsafe { drop(Box::from_raw(ptr)) };
    }

    /// Tears down a standalone manager where vanilla code touched the intrusive chain (e.g.
    /// `ADD_THOUGHT.original()` allocated nodes via vanilla's own allocator).
    ///
    /// Frees the `ZTThoughtMgr` struct and the sentinel node (both were allocated by Rust in
    /// [`build_standalone_mgr`]), but DELIBERATELY LEAKS any other nodes on the chain to avoid a
    /// cross-allocator crash.
    pub fn destroy_standalone_mgr_leaking_nodes(ptr: *mut ZTThoughtMgr) {
        if ptr.is_null() {
            return;
        }
        let sentinel = unsafe { (*ptr).sentinel_ptr } as *mut ThoughtNode;
        if !sentinel.is_null() {
            unsafe { drop(Box::from_raw(sentinel)) };
        }
        unsafe { drop(Box::from_raw(ptr)) };
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use super::super::thought::{substitute_thought_string, ZTThought};

    fn thought_fixture(string_id: u32, thinker_id: u32) -> ZTThought {
        ZTThought {
            vtable: 0,
            string_id,
            thinker_id,
            object_id: 0,
            tile_x: -1,
            tile_y: -1,
            thinker_ptr: 0,
            object_ptr: 0,
            habitat_ptr: 0,
        }
    }

    fn thought_fixture_with_ptrs(string_id: u32, thinker_ptr: u32, object_ptr: u32, habitat_ptr: u32) -> ZTThought {
        ZTThought {
            vtable: 0,
            string_id,
            thinker_id: 0,
            object_id: 0,
            tile_x: -1,
            tile_y: -1,
            thinker_ptr,
            object_ptr,
            habitat_ptr,
        }
    }

    fn build_test_mgr(max_thoughts: u32) -> ZTThoughtMgr {
        let sentinel_ptr = Box::into_raw(Box::new(0u8)) as u32;
        ZTThoughtMgr { vtable: 0, flag: 0, _pad: [0; 3], sentinel_ptr, max_thoughts }
    }

    #[test]
    fn new_mgr_is_empty() {
        let mgr = build_test_mgr(1000);
        assert_eq!(mgr.len(), 0);
        assert!(mgr.is_empty());
        assert!(mgr.iter().next().is_none());
    }

    #[test]
    fn insert_front_adds_most_recent_first() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture(1, 0));
        mgr.insert_front(thought_fixture(2, 0));
        mgr.insert_front(thought_fixture(3, 0));

        let ids: Vec<u32> = mgr.iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![3, 2, 1]);
        assert_eq!(mgr.len(), 3);
    }

    #[test]
    fn insert_front_trims_oldest_once_over_cap() {
        let mut mgr = build_test_mgr(2);
        mgr.insert_front(thought_fixture(1, 0));
        mgr.insert_front(thought_fixture(2, 0));
        mgr.insert_front(thought_fixture(3, 0));

        let ids: Vec<u32> = mgr.iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![3, 2]);
        assert_eq!(mgr.len(), 2);
    }

    #[test]
    fn insert_front_with_zero_cap_keeps_list_empty() {
        let mut mgr = build_test_mgr(0);
        mgr.insert_front(thought_fixture(1, 0));
        assert_eq!(mgr.len(), 0);
    }

    #[test]
    fn remove_where_frees_matching_nodes_only() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture(1, 10));
        mgr.insert_front(thought_fixture(2, 20));
        mgr.insert_front(thought_fixture(3, 10));

        mgr.remove_where(|t| t.thinker_id() == 10);

        let ids: Vec<u32> = mgr.iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![2]);
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn remove_where_matching_nothing_is_a_no_op() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture(1, 10));
        mgr.remove_where(|t| t.thinker_id() == 999);
        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn remove_where_can_empty_the_list() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture(1, 10));
        mgr.insert_front(thought_fixture(2, 10));
        mgr.remove_where(|t| t.thinker_id() == 10);
        assert!(mgr.is_empty());
    }

    #[test]
    fn get_thoughts_by_thinker_filters_and_caps() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 10, 0, 0));
        mgr.insert_front(thought_fixture_with_ptrs(2, 20, 0, 0));
        mgr.insert_front(thought_fixture_with_ptrs(3, 10, 0, 0));
        mgr.insert_front(thought_fixture_with_ptrs(4, 10, 0, 0));

        let ids: Vec<u32> = mgr.get_thoughts_by_thinker(10, 2).iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![3, 4]);
    }

    #[test]
    fn get_thoughts_by_object_filters_by_object_ptr_only() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 0, 99, 0));
        mgr.insert_front(thought_fixture_with_ptrs(2, 0, 100, 0));

        let ids: Vec<u32> = mgr.get_thoughts_by_object(99, 10).iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![1]);
    }

    #[test]
    fn get_thoughts_by_habitat_filters_by_habitat_ptr_only() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 0, 0, 55));
        mgr.insert_front(thought_fixture_with_ptrs(2, 0, 0, 55));
        mgr.insert_front(thought_fixture_with_ptrs(3, 0, 0, 56));

        let ids: Vec<u32> = mgr.get_thoughts_by_habitat(55, 10).iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![1, 2]);
    }

    #[test]
    fn get_thoughts_by_thinker_with_no_match_is_empty() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 10, 0, 0));
        assert!(mgr.get_thoughts_by_thinker(999, 10).is_empty());
    }

    #[test]
    fn get_thoughts_by_thinker_zero_max_count_returns_empty() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 10, 0, 0));
        assert!(mgr.get_thoughts_by_thinker(10, 0).is_empty());
    }

    #[test]
    fn substitution_uses_provided_name_when_present() {
        assert_eq!(substitute_thought_string(Some("Caught a %s!".to_string()), Some("Zebra")), "Caught a Zebra!");
    }

    #[test]
    fn substitution_falls_back_to_template_when_none_available() {
        assert_eq!(substitute_thought_string(Some("Bored".to_string()), None), "Bored");
    }

    #[test]
    fn substitution_missing_template_returns_empty_string() {
        assert_eq!(substitute_thought_string(None, Some("Zebra")), "");
    }

    #[test]
    fn substitution_empty_template_returned_as_is() {
        assert_eq!(substitute_thought_string(Some(String::new()), Some("Zebra")), "");
    }

    #[test]
    fn substitution_only_replaces_first_occurrence() {
        assert_eq!(substitute_thought_string(Some("%s and %s".to_string()), Some("X")), "X and %s");
    }

    #[test]
    fn remove_thoughts_by_thinker_removes_only_matching() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 10, 0, 0));
        mgr.insert_front(thought_fixture_with_ptrs(2, 20, 0, 0));
        mgr.insert_front(thought_fixture_with_ptrs(3, 10, 0, 0));

        mgr.remove_thoughts_by_thinker(10);

        let ids: Vec<u32> = mgr.iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![2]);
    }

    #[test]
    fn remove_thoughts_by_object_removes_only_matching() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 0, 99, 0));
        mgr.insert_front(thought_fixture_with_ptrs(2, 0, 100, 0));

        mgr.remove_thoughts_by_object(99);

        let ids: Vec<u32> = mgr.iter().map(|t| t.string_id()).collect();
        assert_eq!(ids, vec![2]);
    }

    #[test]
    fn remove_thoughts_by_habitat_force_true_always_removes() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 0, 99, 55));
        mgr.insert_front(thought_fixture_with_ptrs(2, 0, 0, 55));

        mgr.remove_thoughts_by_habitat(55, true);

        assert!(mgr.is_empty());
    }

    #[test]
    fn remove_thoughts_by_habitat_force_false_with_object_only_clears_habitat_link() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 0, 99, 55));

        mgr.remove_thoughts_by_habitat(55, false);

        assert_eq!(mgr.len(), 1);
        let survivor = mgr.iter().next().unwrap();
        assert_eq!(survivor.object_ptr(), 99);
        assert_eq!(survivor.habitat_ptr(), 0);
    }

    #[test]
    fn remove_thoughts_by_habitat_force_false_without_object_removes_node() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 0, 0, 55));

        mgr.remove_thoughts_by_habitat(55, false);

        assert!(mgr.is_empty());
    }

    #[test]
    fn remove_thoughts_by_habitat_ignores_non_matching() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture_with_ptrs(1, 0, 99, 55));

        mgr.remove_thoughts_by_habitat(56, true);

        assert_eq!(mgr.len(), 1);
    }

    #[test]
    fn clear_frees_every_node_and_leaves_list_empty() {
        let mut mgr = build_test_mgr(1000);
        mgr.insert_front(thought_fixture(1, 10));
        mgr.insert_front(thought_fixture(2, 20));
        mgr.insert_front(thought_fixture(3, 30));

        mgr.clear();

        assert!(mgr.is_empty());
        assert!(mgr.iter().next().is_none());
    }

    #[test]
    fn clear_on_empty_list_is_a_no_op() {
        let mut mgr = build_test_mgr(1000);
        mgr.clear();
        assert!(mgr.is_empty());
    }
}
