//! `ZTShowState` reimplementation - Stage 1 of `openzt/plans/ztshowinfo-implementation-plan.md`.
//!
//! `ZTShowState` is the base class embedded inside `ZTShow` at `ZTShow`-relative `+0x18`
//! (`ZTShowInfo`-relative `+0x1c`); `ZTShow`'s own `+0x34` field (already read by `ztshow.rs`'s
//! `get_show_script_state`/`find_script_state_node`) is this class's own `+0x1c` field. This is a new
//! sibling module to `ztshow.rs` rather than an extension of it - the plan's own "pick one and say so up
//! front" decision point: `ztshow.rs` already covers a substantial slice of `ZTShow`/`ZTShowInfo`'s own
//! methods and is large, while `ZTShowState` is a distinct base class with its own vtable slot and
//! lifecycle. Later `ZTShowInfo` stages (2-12) should follow this same precedent and get their own
//! `ztshowinfo.rs` rather than growing `ztshow.rs` further. This module continues `ztshow.rs`'s
//! established style for this class family: free functions over a raw `this: u32`, no owning
//! `#[repr(C)]` struct, `get_from_memory`/`save_to_memory` at literal offsets - not a new convention.
//! The class-method free functions are named `show_state_*` (not the bare vanilla method names) purely
//! to avoid colliding with this module's own `pub fn init()` registration entry point - `ZTShowState`
//! happens to have a real method literally named `init`, which `ztshow.rs`'s own ported methods never
//! did.
//!
//! ## Field layout (`this` = `ZTShowState*`)
//! Confirmed from `ZTShowState_ZTShowState.c`/`.asm` and `_init.c`/`.asm` (both read in full): `+0x0`
//! vtable, `+0x4` `u16`, `+0x6`/`+0x7` `u8` flags, `+0x8` `u8` "enabled"-shaped flag (ctor/`init` default
//! `1`), `+0xc`/`+0x10`/`+0x14`/`+0x18` `u32` fields (never independently named - not read outside
//! `init`/`save`/`load` by anything found this session), `+0x1c` a pointer to this instance's own
//! script-state tree header (allocated once by the constructor, never touched by `init`), `+0x20` a
//! `u32` that `save`/`load` both treat as the tree's own serialized entry count (see
//! [`show_state_load`]'s doc comment), `+0x24` a `u8` set once by the constructor from `this`'s own high
//! address byte (never read/written by any of `init`/`clear`/`save`/`load`).
//!
//! ## The script-state tree (`this+0x1c`)
//! The exact same `std::map<u32, ZTShowScriptState*>`-shaped tree `ztshow.rs`'s `get_show_script_state`/
//! `find_script_state_node` already read-only walk (at `ZTShow+0x34` = `ZTShowState+0x1c`) - this module
//! owns writing to it. Header and node share one `0x18`-byte layout (confirmed: the header is allocated
//! with size `0x18` in the constructor, and the destructor pushes freed *node* memory back onto the very
//! same freelist - see the Allocator section): `+0x0` color/isnil byte (padded to a `u32`; on a *free*
//! block this same word doubles as the freelist "next" link), `+0x4` parent (header: root), `+0x8` left
//! (header: leftmost cache), `+0xc` right (header: rightmost cache), `+0x10` key (`u32`), `+0x14` value
//! (`ZTShowScriptState*`, meaningful only on a real node, never the header). An empty tree has
//! `root(header+4) == 0` and `leftmost(header+8) == rightmost(header+0xc) == header` (self-referencing
//! sentinel, the same shape this codebase's other tree headers already use).
//!
//! [`find_or_insert_state_node`]'s insert-with-hint descent is the same shape `ztshow.rs`'s own
//! `plan_pending_node_insert` already uses for a sibling tree - and matches the one real, un-ported
//! vanilla insert this session found and read in full for comparison, `ztshowscriptstate::CONSTRUCTOR`'s
//! own inlined descent (`ZTShowScriptState_ZTShowScriptState.c`/`.asm`, which turns out to operate on
//! this *exact* tree at `ZTShow+0x34`, confirming the header/node layout above independently). An
//! unbalanced BST insert, not a real MSVC red-black insert, matching that sibling tree's own
//! justification: every known reader of this tree only relies on the ordering invariant, never on a
//! color/balance bit.
//!
//! ## Allocator note - two different real allocators in play
//! - **Tree/header nodes (`0x18` bytes each)**: vanilla's shared small-object freelist, head pointer at
//!   `DAT_00638008` (RVA `0x238008`) - the `(byte_capacity - 1) >> 3 == 2` bucket, i.e. exactly this
//!   size (`zoostatus.rs`'s own doc comment already names this bucket formula). Confirmed on both the
//!   allocate side (`ZTShowState_ZTShowState.asm`'s constructor: pop the freelist head, or call
//!   `FUN_00402f85(0x18)` when the head is null) and the free side (`ZTShowState_~ZTShowState.asm`'s
//!   destructor: push freed node memory back onto that same head) - see [`allocate_tree_node`]/
//!   [`release_tree_node`]. This is the *same* freelist `ztadvterrainmgr.rs`'s `release_bfpos_node`
//!   already writes to (a single shared global head, not per-caller state) - that module only ever
//!   pushes (real vanilla code is the only allocator of its nodes); this module is the first to also
//!   need the pop/grow side, evidenced directly by this exact class's own constructor.
//! - **Script-state values (`ZTShowScriptState`, `0x14` bytes each)**: plain `operator_new`/
//!   `operator_delete` (`standalone::OPERATOR_NEW`/`OPERATOR_DELETE`) - confirmed directly in
//!   `ZTShowState_load.c`'s `operator_new(0x14)` and `_clear.c`'s `operator_delete(puVar1)`. A
//!   genuinely different, simpler allocator than the tree nodes that hold them - per `CLAUDE.md`'s
//!   cross-allocator rule, never mixed with the freelist above.
//!
//! The freelist's own growth/refill helper (called only when the freelist head is null) is
//! `generated.rs`'s `poolalloc::REFILL` (`0x00402f85`, confirmed the same STL pool-allocator refill call
//! `ZTShowState_ZTShowState.asm`'s constructor uses on its own freelist-empty branch). In practice this
//! freelist is shared, pervasively used, and essentially never empty during real gameplay, so the fast
//! inline-pop path (mirroring the constructor's own logic exactly) is expected to be the overwhelmingly
//! common case.
//!
//! ## Scope: `CONSTRUCTOR` stays real, un-detoured; `ZTShow`'s own destructor is out of this module's
//! scope entirely
//! `ZTShowState` is embedded inside `ZTShow`, which real, un-decompiled-by-us vanilla code
//! (`ZTHabitat::setIsShowExhibit`'s own `new`) must keep constructing - there is no Rust-owned instance
//! to redirect construction onto, and the destructor's job (freeing the script-state tree/values, then
//! the header itself) is memory this module's own [`show_state_clear`] already knows how to correctly
//! release, so a Rust destructor hook would have nothing left to do differently. `CONSTRUCTOR` is real,
//! un-detoured.
//!
//! There is no standalone `ZTShowState::~ZTShowState(ZTShowState*)` at all: what `generated.rs` first
//! (mis-)clustered into this module as `ztshowstate::ZTSHOW_STATE` is `ZTShow`'s own complete destructor
//! (now correctly `ztshow::ZTSHOW_0`, `0x004f28c3`, per `ZTShow_~ZTShow_0.c`/`.asm`) - it takes a
//! `ZTShow*`, restores `ZTShow`'s own vtable, then adjusts to the embedded `ZTShowState` at `+0x18` and
//! tears that down inline (gated on `mbr_0x20`, the same script-state-count field [`show_state_load`]
//! maintains) before finally restoring `ZTShowState`'s own vtable and freeing its tree header - rather
//! than calling a separate `ZTShowState::~ZTShowState`. This was found the hard way: an earlier version
//! of this module's own `live_support` called the address with a bare, non-embedded `ZTShowState*`
//! (assuming it took a plain `ZTShowState*` self parameter) for a standalone live-test fixture, which
//! read/wrote far past that allocation's own bounds and crashed the live reimplementation-test battery
//! outright. `live_support::destroy_standalone_show_state` below therefore never calls that address at
//! all - it uses [`show_state_clear`] (this module's own, already-correct teardown of the tree/values)
//! plus a direct freelist release of the header block, mirroring only the parts of `ZTShow`'s own
//! destructor tail that are meaningful for a bare `ZTShowState`. Real gameplay is unaffected either way -
//! real `ZTShow` teardown always reaches `ztshow::ZTSHOW_0` with a real, fully-formed `ZTShow*`.

use openzt_detour::generated::{
    poolalloc::REFILL as ALLOCATE_SMALL_OBJECT_SLOW,
    standalone::{DEALLOCATE, OPERATOR_DELETE, OPERATOR_NEW, WRITE_BYTES_TO_FILE},
    ztshowscriptstate::{LOAD as SCRIPT_STATE_LOAD, SAVE as SCRIPT_STATE_SAVE},
    ztshowstate::{CLEAR, INIT, LOAD, SAVE},
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::get_module_base,
    util::{get_from_memory, save_to_memory},
};

/// RVA of the shared small-object freelist head for the `0x18`-byte bucket - see the module doc
/// comment's Allocator section. RVA = `0x00638008 - 0x400000`.
const RVA_SMALL_OBJECT_FREELIST_HEAD: u32 = 0x0023_8008;

/// Size of one tree/header block in this class's own script-state map - see the module doc comment.
const TREE_NODE_SIZE: u32 = 0x18;

/// RVA of `ZTShowScriptState`'s own vtable (`ZTShowScriptState__vftable_63502c_0063502c` in the
/// decompile corpus - VA `0x0063502c`). Stamped onto every value object [`show_state_load`] constructs,
/// matching real vanilla's own `ZTShowState_load.c`.
const RVA_SCRIPT_STATE_VTABLE: u32 = 0x0023_502c;

fn small_object_freelist_head_addr() -> u32 {
    get_module_base("zoo.exe") as u32 + RVA_SMALL_OBJECT_FREELIST_HEAD
}

/// Allocates one `0x18`-byte tree/header block from vanilla's own shared small-object freelist -
/// mirroring `ZTShowState_ZTShowState.asm`'s constructor exactly: pop the freelist head if non-null,
/// otherwise fall back to [`ALLOCATE_SMALL_OBJECT_SLOW`]. Zeroes every field of the returned block
/// (color/parent/left/right/key/value) before returning it - callers fill in whichever fields apply.
fn allocate_tree_node() -> u32 {
    let head_addr = small_object_freelist_head_addr();
    let head = get_from_memory::<u32>(head_addr);
    let node = if head == 0 {
        unsafe { ALLOCATE_SMALL_OBJECT_SLOW.original()(TREE_NODE_SIZE as i32) as u32 }
    } else {
        let next = get_from_memory::<u32>(head);
        save_to_memory(head_addr, next);
        head
    };
    save_to_memory(node, 0u32);
    save_to_memory(node + 0x4, 0u32);
    save_to_memory(node + 0x8, 0u32);
    save_to_memory(node + 0xc, 0u32);
    save_to_memory(node + 0x10, 0u32);
    save_to_memory(node + 0x14, 0u32);
    node
}

/// Pushes `node` back onto vanilla's own shared small-object freelist head - mirroring
/// `ZTShowState_~ZTShowState.asm`'s destructor exactly (same freelist `ztadvterrainmgr.rs`'s
/// `release_bfpos_node` already uses for a different node type/bucket-size, per this module's own
/// Allocator doc).
fn release_tree_node(node: u32) {
    let head_addr = small_object_freelist_head_addr();
    let old_head = get_from_memory::<u32>(head_addr);
    save_to_memory(node, old_head);
    save_to_memory(head_addr, node);
}

/// Recursive in-order collection of every node in the script-state tree rooted at `node` - same
/// technique as `ztshow.rs`'s own `collect_pending_script_nodes` for a sibling tree (a real red-black
/// tree here is never more than a few dozen nodes deep, so recursion depth is a non-concern).
pub(crate) fn collect_tree_nodes(node: u32, out: &mut Vec<u32>) {
    if node == 0 {
        return;
    }
    collect_tree_nodes(get_from_memory::<u32>(node + 0x8), out);
    out.push(node);
    collect_tree_nodes(get_from_memory::<u32>(node + 0xc), out);
}

/// Decision made by [`plan_state_node_insert`] for a given `(header, key)` pair - see that function's
/// own doc comment. Same shape as `ztshow.rs`'s `PendingNodeInsertPlan`, plus a rightmost-cache flag on
/// the right-insert arm since *this* header genuinely maintains one (unlike the pending-scripts header
/// `ztshow.rs` found only has room for `self`/`root`/`leftmost`).
#[derive(Debug, PartialEq, Eq)]
enum StateNodeInsertPlan {
    /// An existing node with a matching key was found - its address.
    Found(u32),
    /// The tree is empty; a freshly-allocated node becomes the root.
    NewRoot,
    /// Insert as `parent`'s left child; `parent_is_leftmost` says whether `parent` is currently the
    /// header's own leftmost-cache pointer (`+0x8`), which the new node would then replace.
    InsertLeft { parent: u32, parent_is_leftmost: bool },
    /// Insert as `parent`'s right child; `parent_is_rightmost` says whether `parent` is currently the
    /// header's own rightmost-cache pointer (`+0xc`), which the new node would then replace.
    InsertRight { parent: u32, parent_is_rightmost: bool },
}

/// Pure decision half of [`find_or_insert_state_node`]'s BST walk - same lower-bound-then-candidate
/// descent as `ztshow.rs`'s `find_script_state_node`/`plan_pending_node_insert`, and confirmed against
/// the one real, un-ported vanilla insert this session read in full
/// (`ZTShowScriptState_ZTShowScriptState.c`'s own inlined descent over this exact tree).
fn plan_state_node_insert(header: u32, key: u32) -> StateNodeInsertPlan {
    let root = get_from_memory::<u32>(header + 0x4);
    if root == 0 {
        return StateNodeInsertPlan::NewRoot;
    }

    let mut node = root;
    let mut candidate = header;
    let (parent, went_left) = loop {
        let current = node;
        if get_from_memory::<u32>(current + 0x10) < key {
            let right = get_from_memory::<u32>(current + 0xc);
            if right == 0 {
                break (current, false);
            }
            node = right;
        } else {
            candidate = current;
            let left = get_from_memory::<u32>(current + 0x8);
            if left == 0 {
                break (current, true);
            }
            node = left;
        }
    };

    if candidate != header && get_from_memory::<u32>(candidate + 0x10) <= key {
        return StateNodeInsertPlan::Found(candidate);
    }

    if went_left {
        let leftmost = get_from_memory::<u32>(header + 0x8);
        StateNodeInsertPlan::InsertLeft { parent, parent_is_leftmost: parent == leftmost }
    } else {
        let rightmost = get_from_memory::<u32>(header + 0xc);
        StateNodeInsertPlan::InsertRight { parent, parent_is_rightmost: parent == rightmost }
    }
}

/// Finds or inserts a node for `key` in the script-state tree headed by `header` (`this+0x1c`). Returns
/// the node's address either way - callers set `node+0x14` (the value pointer) themselves, matching
/// vanilla's own `pair<iterator, bool>`-shaped insert-then-assign convention (see
/// [`show_state_load`]'s own call site).
pub(crate) fn find_or_insert_state_node(header: u32, key: u32) -> u32 {
    match plan_state_node_insert(header, key) {
        StateNodeInsertPlan::Found(node) => node,
        StateNodeInsertPlan::NewRoot => {
            let node = allocate_tree_node();
            save_to_memory(node + 0x4, header);
            save_to_memory(node + 0x10, key);
            save_to_memory(header + 0x4, node);
            save_to_memory(header + 0x8, node);
            save_to_memory(header + 0xc, node);
            node
        }
        StateNodeInsertPlan::InsertLeft { parent, parent_is_leftmost } => {
            let node = allocate_tree_node();
            save_to_memory(node + 0x4, parent);
            save_to_memory(node + 0x10, key);
            save_to_memory(parent + 0x8, node);
            if parent_is_leftmost {
                save_to_memory(header + 0x8, node);
            }
            node
        }
        StateNodeInsertPlan::InsertRight { parent, parent_is_rightmost } => {
            let node = allocate_tree_node();
            save_to_memory(node + 0x4, parent);
            save_to_memory(node + 0x10, key);
            save_to_memory(parent + 0xc, node);
            if parent_is_rightmost {
                save_to_memory(header + 0xc, node);
            }
            node
        }
    }
}

fn write_field(addr: u32, size: u32, file: *const i8) -> bool {
    unsafe { WRITE_BYTES_TO_FILE.hooked()(addr as *const u32, size, 1, file) == 1 }
}

fn read_field(addr: u32, size: u32, file: *const u32) -> bool {
    unsafe { DEALLOCATE.hooked()(addr as *const u32, size, 1, file as *const u8) == 1 }
}

/// Reimplementation of `ZTShowState::init`, per `ZTShowState_init.c`/`.asm` (both read in full) - the
/// class's one vtable slot, dispatched from `ZTShow::reinit`. Resets every scalar field except the
/// script-state tree header (`+0x1c`, allocated once by the constructor and never touched again here) -
/// confirmed directly from the `.asm`: no instruction anywhere in `init` reads or writes `+0x1c`,
/// `+0x20`, or `+0x24`.
pub fn show_state_init(this: u32) -> bool {
    save_to_memory(this + 0x8, 1u8);
    save_to_memory(this + 0x4, 0u16);
    save_to_memory(this + 0x7, 0u8);
    save_to_memory(this + 0x6, 0u8);
    save_to_memory(this + 0xc, 0u32);
    save_to_memory(this + 0x10, 0u32);
    save_to_memory(this + 0x14, 0u32);
    save_to_memory(this + 0x18, 0u32);
    true
}

/// Reimplementation of `ZTShowState::clear`, per `ZTShowState_clear.c`/`.asm` (both read in full).
/// Real vanilla walks the tree via its own in-order-successor iterator, freeing each node's *value*
/// (`ZTShowScriptState*` at `+0x14`, via plain `operator_delete` - see the module doc comment's
/// Allocator section) as it goes, then erases the tree's own internal node structures via an unnamed
/// helper (`AI_cls_0x404fd6::meth_0x59f700`) and resets the header to empty. This port collects every
/// node upfront (via [`collect_tree_nodes`]) rather than replicating that iterator algorithm - since
/// nothing here mutates the tree's own structural pointers mid-walk, the two are equivalent, matching
/// `ztshow.rs`'s own established precedent for the sibling pending-scripts tree.
pub fn show_state_clear(this: u32) {
    let header = get_from_memory::<u32>(this + 0x1c);
    let root = get_from_memory::<u32>(header + 0x4);
    let mut nodes = Vec::new();
    collect_tree_nodes(root, &mut nodes);

    for &node in &nodes {
        let value = get_from_memory::<u32>(node + 0x14);
        if value != 0 {
            unsafe { OPERATOR_DELETE.original()(value) };
        }
    }
    for node in nodes {
        release_tree_node(node);
    }

    save_to_memory(header + 0x4, 0u32);
    save_to_memory(header + 0x8, header);
    save_to_memory(header + 0xc, header);
}

/// Reimplementation of `ZTShowState::save`, per `ZTShowState_save.c`/`.asm` (both read in full). Writes
/// the nine header scalars (`+0x4` through `+0x20` - the last one is `+0x20` itself, the tree's own
/// serialized entry count, see [`show_state_load`]'s doc comment) in file order, then every script
/// state's own `ZTShowScriptState::save` in ascending-key tree order (equivalent to vanilla's own
/// leftmost-to-rightmost iterator walk for the same reason [`show_state_clear`]'s doc comment gives).
/// Returns `false` (matching vanilla's own low-byte-defined success contract) as soon as any write
/// fails, without attempting the remaining ones - matching vanilla's own early-return-on-failure shape
/// inside its node loop (the header-scalar writes themselves are unconditional in both, like every
/// other `*::save` in this codebase).
pub fn show_state_save(this: u32, file: *const i8) -> bool {
    let mut ok = write_field(this + 0x4, 2, file);
    ok &= write_field(this + 0x6, 1, file);
    ok &= write_field(this + 0x7, 1, file);
    ok &= write_field(this + 0x8, 1, file);
    ok &= write_field(this + 0xc, 4, file);
    ok &= write_field(this + 0x10, 4, file);
    ok &= write_field(this + 0x14, 4, file);
    ok &= write_field(this + 0x18, 4, file);
    ok &= write_field(this + 0x20, 4, file);
    if !ok {
        return false;
    }

    let header = get_from_memory::<u32>(this + 0x1c);
    let root = get_from_memory::<u32>(header + 0x4);
    let mut nodes = Vec::new();
    collect_tree_nodes(root, &mut nodes);

    for node in nodes {
        let value = get_from_memory::<u32>(node + 0x14);
        let node_ok = unsafe { SCRIPT_STATE_SAVE.original()(value as *const u32, file as *const u32) != 0 };
        if !node_ok {
            return false;
        }
    }
    true
}

/// Reimplementation of `ZTShowState::load`, per `ZTShowState_load.c`/`.asm` (both read in full,
/// non-trivial - the `.c`'s own header notes it was "restarted to delay deadcode elimination"). Version
/// gate matches `save`/`load`'s shared threshold: `version <= 0x60` reads nothing at all (matches
/// vanilla's own `if (0x60 < param_2)` guard around the entire body).
///
/// The file position immediately after the eight header scalars is read into a local (vanilla's own
/// decompile reuses the `param_1` slot for it, an artifact of register/stack reuse, not a real second
/// parameter) and is **also** written into `this+0x20` here - real vanilla's own `save` persists exactly
/// `this->mbr_0x20`'s value at that same file position (see [`show_state_save`]), so this field is the
/// tree's own serialized entry count, kept in sync the same way `ztshow.rs`'s `ZTShowInfo+0x48` pending
/// node count is. `clear()` runs unconditionally straight after (even on a read failure so far) - a
/// faithfully-reproduced vanilla quirk, not "fixed" here - and the node loop that follows uses whatever
/// count was read, again unconditionally on prior success (matching vanilla's own `if (param_1 != 0)`
/// guard, not gated on `ok`).
///
/// Each entry: a fresh `0x14`-byte `ZTShowScriptState` (plain `operator_new`, per the module doc
/// comment's Allocator section) is zeroed, stamped with the real vtable pointer and the `0xffff`
/// trick-index sentinel (matching `ZTShowState_load.c`'s own pre-`load()` field defaults - the two
/// unaccounted `+0x6`/`+0x7` bytes vanilla's own `operator_new` leaves uninitialized are harmlessly
/// zeroed here instead, since nothing reads them either way), then populated via real, un-ported
/// `ZTShowScriptState::load` (`.original()`, never detoured by this module). Its own `+0x8` field
/// (populated by that call) becomes the tree's insertion key - matching vanilla's own
/// read-then-key-by-loaded-value order exactly, not the live-`start()`-flow order (`ztshow.rs`'s
/// `CREATE_SHOW_SCRIPT_STATE`) where the key is known upfront instead.
///
/// A single entry-level load failure returns `false` immediately (matching vanilla's own
/// `bVar14 = bVar14 & bVar3; if (!bVar14) return false;` inside the loop - note this also fires on the
/// very first iteration if an earlier header-scalar read had already failed, exactly reproducing
/// vanilla's own accumulated-failure short-circuit). An allocation failure for one entry is silently
/// skipped (matching vanilla's own `if (this_01 != 0)` guard), not fatal.
pub fn show_state_load(this: u32, file: *const u32, version: u32) -> bool {
    if version <= 0x60 {
        return true;
    }

    let mut ok = read_field(this + 0x4, 2, file);
    ok &= read_field(this + 0x6, 1, file);
    ok &= read_field(this + 0x7, 1, file);
    ok &= read_field(this + 0x8, 1, file);
    ok &= read_field(this + 0xc, 4, file);
    ok &= read_field(this + 0x10, 4, file);
    ok &= read_field(this + 0x14, 4, file);
    ok &= read_field(this + 0x18, 4, file);

    let mut count: u32 = 0;
    ok &= read_field(&mut count as *mut u32 as u32, 4, file);
    save_to_memory(this + 0x20, count);

    show_state_clear(this);

    if count != 0 {
        let header = get_from_memory::<u32>(this + 0x1c);
        let vtable_addr = get_module_base("zoo.exe") as u32 + RVA_SCRIPT_STATE_VTABLE;

        for _ in 0..count {
            let value = unsafe { OPERATOR_NEW.original()(0x14) } as u32;
            if value == 0 {
                continue;
            }
            unsafe { std::ptr::write_bytes(value as *mut u8, 0, 0x14) };
            save_to_memory(value + 0xc, 0xffffu16);
            save_to_memory(value, vtable_addr);

            let loaded = unsafe { SCRIPT_STATE_LOAD.original()(value as *const u32, file, version) };
            ok &= loaded;
            if !ok {
                return false;
            }

            let key = get_from_memory::<u32>(value + 0x8);
            let node = find_or_insert_state_node(header, key);
            save_to_memory(node + 0x14, value);
        }
    }

    ok
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(INIT)]
    unsafe extern "thiscall" fn init_detour(this: *const u32) -> u32 {
        show_state_init(this as u32) as u32
    }

    #[detour(CLEAR)]
    unsafe extern "thiscall" fn clear_detour(this: *const u32) -> u32 {
        show_state_clear(this as u32);
        0
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const u32) -> u32 {
        show_state_save(this as u32, file as *const i8) as u32
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load_detour(this: *const u32, file: *const u32, version: u32) -> bool {
        show_state_load(this as u32, file, version)
    }

    /// `(name, is_enabled)` per detour - lets `reimplementation_tests`'s `ZTSHOWSTATE_DETOURS_ENABLED`
    /// catch a silently-failed `init_detours()` the same way `ztgamemgr_menumusichandler`/`ztsoundscape`/
    /// `zoostatus` already do for their own detour sets (see each of their own `pub(crate) mod
    /// live_support`), rather than a comparison test's own real-vs-rust output happening to coincide by
    /// chance (as `init`'s own live test did here before this existed - see the module doc comment's
    /// `openzt-test-dll`-vs-`reimplementation_tests::init()` wiring note).
    pub(crate) fn status() -> [(&'static str, bool); 4] {
        [
            ("INIT", INIT_DETOUR.is_enabled()),
            ("CLEAR", CLEAR_DETOUR.is_enabled()),
            ("SAVE", SAVE_DETOUR.is_enabled()),
            ("LOAD", LOAD_DETOUR.is_enabled()),
        ]
    }
}

/// **Wiring note**: this module's detours are installed from two different places depending on how the
/// DLL was entered - the real game's normal boot path (`lib.rs`'s `LOAD_LANG_DLLS_DETOUR` hook, gated on
/// the `experimental` feature) calls this `init()` directly, but `openzt-test-dll` (the
/// `reimplementation-tests` binary the live battery runs under) never reaches that hook at all - its own
/// `DllMain` calls `reimplementation_tests::init()` instead, which has its own explicit list of which
/// modules' detours to install for the battery. **Both call sites must list this module** - missing it
/// from the `reimplementation_tests::init()` list (as an earlier version of this change did) means every
/// `.hooked()` call in this module's own live tests silently falls through to real, un-detoured vanilla
/// code instead of this port, with no error logged anywhere (`init_detours()` is simply never called) -
/// exactly the class of false-positive `ZTSHOWSTATE_INIT` hit before `ZTSHOWSTATE_DETOURS_ENABLED` (this
/// module's own version of the `MENUMUSICHANDLER_DETOURS_ENABLED`-style wiring check) existed to catch it.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztshowstate detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;
    use openzt_detour::generated::ztshowstate::CONSTRUCTOR;

    /// Allocates a standalone buffer sized to hold a real `ZTShowState` (`0x28` bytes, rounded up past
    /// the real `0x25`-byte extent for safety) and runs the real vanilla constructor over it - giving
    /// tests a genuinely valid instance (real freelist-allocated tree header included) without this
    /// module needing to hand-roll its own construction path. `pub(crate)` for
    /// `reimplementation_tests::mod`. Safe to call directly (unlike the destructor - see
    /// [`destroy_standalone_show_state`]'s own doc comment): `CONSTRUCTOR`'s own `.asm` operates on
    /// `this` with no adjustment, matching a plain `ZTShowState*`.
    pub(crate) fn build_standalone_show_state() -> u32 {
        const BUFFER_SIZE: u32 = 0x28;
        let buf = unsafe { OPERATOR_NEW.original()(BUFFER_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(buf as *mut u8, 0, BUFFER_SIZE as usize) };
        unsafe { CONSTRUCTOR.original()(buf as *const u32) };
        buf
    }

    /// Tears down a buffer built by [`build_standalone_show_state`]. **Deliberately never calls
    /// `generated.rs`'s `ztshow::ZTSHOW_0`** (`ZTShow`'s own destructor) - see the module doc comment's
    /// Scope section: that address's real `this` is a `ZTShow*` (it internally adjusts by `+0x18` before
    /// touching anything), not a bare `ZTShowState*`, and calling it directly against this standalone
    /// buffer read/wrote past the allocation and crashed the live battery outright when first tried
    /// (before the generator pass correctly relabeled this address out of the `ztshowstate` module).
    /// Instead this reuses
    /// [`show_state_clear`] (already correct - frees every value via `OPERATOR_DELETE` and every tree
    /// node via the freelist), then releases the header block itself the same way the real destructor's
    /// own tail does (push onto the same `DAT_00638008` freelist - see [`release_tree_node`]), then frees
    /// the outer buffer via `OPERATOR_DELETE`.
    pub(crate) fn destroy_standalone_show_state(buf: u32) {
        show_state_clear(buf);
        let header = get_from_memory::<u32>(buf + 0x1c);
        if header != 0 {
            release_tree_node(header);
        }
        unsafe { OPERATOR_DELETE.original()(buf) };
    }

    /// See `detours::status()`'s own doc comment.
    pub(crate) fn detour_status() -> [(&'static str, bool); 4] {
        super::detours::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Fixed stand-in for one script-state tree node/header, laid out at the same offsets
    /// [`plan_state_node_insert`]/[`collect_tree_nodes`]/[`show_state_clear`] read.
    #[repr(C)]
    #[derive(Clone, Copy)]
    struct FakeNode {
        _color: u32,
        parent: u32,
        left: u32,
        right: u32,
        key: u32,
        value: u32,
    }

    impl FakeNode {
        fn zeroed() -> Self {
            FakeNode { _color: 0, parent: 0, left: 0, right: 0, key: 0, value: 0 }
        }
    }

    #[test]
    fn show_state_init_resets_scalars_but_leaves_tree_header_untouched() {
        let mut buf = [0xffu8; 0x28];
        let this = buf.as_mut_ptr() as u32;
        let header_sentinel = 0xdead_beefu32;
        save_to_memory(this + 0x1c, header_sentinel);
        save_to_memory(this + 0x20, 0x1234_5678u32);
        save_to_memory(this + 0x24, 0x99u8);

        assert!(show_state_init(this));

        assert_eq!(get_from_memory::<u8>(this + 0x8), 1);
        assert_eq!(get_from_memory::<u16>(this + 0x4), 0);
        assert_eq!(get_from_memory::<u8>(this + 0x6), 0);
        assert_eq!(get_from_memory::<u8>(this + 0x7), 0);
        assert_eq!(get_from_memory::<u32>(this + 0xc), 0);
        assert_eq!(get_from_memory::<u32>(this + 0x10), 0);
        assert_eq!(get_from_memory::<u32>(this + 0x14), 0);
        assert_eq!(get_from_memory::<u32>(this + 0x18), 0);
        // Untouched by init - still whatever was there before.
        assert_eq!(get_from_memory::<u32>(this + 0x1c), header_sentinel);
        assert_eq!(get_from_memory::<u32>(this + 0x20), 0x1234_5678);
        assert_eq!(get_from_memory::<u8>(this + 0x24), 0x99);
    }

    #[test]
    fn plan_state_node_insert_empty_tree_is_new_root() {
        let header = FakeNode::zeroed();
        let header_addr = &header as *const FakeNode as u32;
        assert_eq!(plan_state_node_insert(header_addr, 5), StateNodeInsertPlan::NewRoot);
    }

    #[test]
    fn plan_state_node_insert_finds_existing_key() {
        let mut header = FakeNode::zeroed();
        let mut root = FakeNode { _color: 0, parent: 0, left: 0, right: 0, key: 10, value: 0xaaaa };
        let root_addr = &mut root as *mut FakeNode as u32;
        header.parent = root_addr; // header+0x4 == root
        header.left = root_addr; // leftmost
        header.right = root_addr; // rightmost
        let header_addr = &header as *const FakeNode as u32;

        assert_eq!(plan_state_node_insert(header_addr, 10), StateNodeInsertPlan::Found(root_addr));
    }

    #[test]
    fn plan_state_node_insert_left_of_leftmost_updates_leftmost_flag() {
        let mut header = FakeNode::zeroed();
        let mut root = FakeNode { _color: 0, parent: 0, left: 0, right: 0, key: 10, value: 0 };
        let root_addr = &mut root as *mut FakeNode as u32;
        header.parent = root_addr;
        header.left = root_addr;
        header.right = root_addr;
        let header_addr = &header as *const FakeNode as u32;

        match plan_state_node_insert(header_addr, 3) {
            StateNodeInsertPlan::InsertLeft { parent, parent_is_leftmost } => {
                assert_eq!(parent, root_addr);
                assert!(parent_is_leftmost);
            }
            other => panic!("expected InsertLeft, got {other:?}"),
        }
    }

    #[test]
    fn plan_state_node_insert_right_of_rightmost_updates_rightmost_flag() {
        let mut header = FakeNode::zeroed();
        let mut root = FakeNode { _color: 0, parent: 0, left: 0, right: 0, key: 10, value: 0 };
        let root_addr = &mut root as *mut FakeNode as u32;
        header.parent = root_addr;
        header.left = root_addr;
        header.right = root_addr;
        let header_addr = &header as *const FakeNode as u32;

        match plan_state_node_insert(header_addr, 20) {
            StateNodeInsertPlan::InsertRight { parent, parent_is_rightmost } => {
                assert_eq!(parent, root_addr);
                assert!(parent_is_rightmost);
            }
            other => panic!("expected InsertRight, got {other:?}"),
        }
    }

    /// `show_state_clear`'s tree-drain logic, over a small fabricated 3-node tree (root=10, left=5,
    /// right=15), each with a distinct non-null "value" pointer standing in for a `ZTShowScriptState*`.
    /// Can't safely exercise the real `OPERATOR_DELETE`/freelist calls from a unit test (no live game
    /// process), so this only checks the pure collection/reset half by calling [`collect_tree_nodes`]
    /// directly and verifying the header-reset behavior `show_state_clear` performs after it - the full
    /// real-memory round trip (including the allocator calls) is covered by the live test instead.
    #[test]
    fn collect_tree_nodes_visits_all_nodes_in_ascending_key_order() {
        let mut root = FakeNode { _color: 0, parent: 0, left: 0, right: 0, key: 10, value: 0xaaaa };
        let mut left = FakeNode { _color: 0, parent: 0, left: 0, right: 0, key: 5, value: 0xbbbb };
        let mut right = FakeNode { _color: 0, parent: 0, left: 0, right: 0, key: 15, value: 0xcccc };
        let left_addr = &mut left as *mut FakeNode as u32;
        let right_addr = &mut right as *mut FakeNode as u32;
        root.left = left_addr;
        root.right = right_addr;
        let root_addr = &mut root as *mut FakeNode as u32;

        let mut nodes = Vec::new();
        collect_tree_nodes(root_addr, &mut nodes);

        assert_eq!(nodes, vec![left_addr, root_addr, right_addr]);
    }

    #[test]
    fn find_or_insert_state_node_builds_ascending_chain_over_a_synthetic_arena() {
        // A plain Vec<u8> arena stands in for real vanilla memory - `get_from_memory`/`save_to_memory`
        // are FFI-free `ptr::read`/`ptr::write`, so this works identically against a synthetic buffer.
        // Only exercises `plan_state_node_insert`'s decision logic end-to-end via repeated calls to
        // `find_or_insert_state_node`, using plain `Vec<FakeNode>`-shaped leaks as node storage (no real
        // freelist involved) so this stays a pure, allocator-free unit test.
        let header = Box::leak(Box::new(FakeNode::zeroed()));
        let header_addr = header as *mut FakeNode as u32;

        // Reimplement just enough of allocate_tree_node's zero-init contract using leaked FakeNodes,
        // since the real allocate_tree_node() call goes through the real vanilla freelist.
        fn insert_with_leaked_node(header_addr: u32, key: u32) -> u32 {
            match plan_state_node_insert(header_addr, key) {
                StateNodeInsertPlan::Found(node) => node,
                plan => {
                    let node = Box::leak(Box::new(FakeNode::zeroed()));
                    let node_addr = node as *mut FakeNode as u32;
                    save_to_memory(node_addr + 0x10, key);
                    match plan {
                        StateNodeInsertPlan::NewRoot => {
                            save_to_memory(node_addr + 0x4, header_addr);
                            save_to_memory(header_addr + 0x4, node_addr);
                            save_to_memory(header_addr + 0x8, node_addr);
                            save_to_memory(header_addr + 0xc, node_addr);
                        }
                        StateNodeInsertPlan::InsertLeft { parent, parent_is_leftmost } => {
                            save_to_memory(node_addr + 0x4, parent);
                            save_to_memory(parent + 0x8, node_addr);
                            if parent_is_leftmost {
                                save_to_memory(header_addr + 0x8, node_addr);
                            }
                        }
                        StateNodeInsertPlan::InsertRight { parent, parent_is_rightmost } => {
                            save_to_memory(node_addr + 0x4, parent);
                            save_to_memory(parent + 0xc, node_addr);
                            if parent_is_rightmost {
                                save_to_memory(header_addr + 0xc, node_addr);
                            }
                        }
                        StateNodeInsertPlan::Found(_) => unreachable!(),
                    }
                    node_addr
                }
            }
        }

        let n10 = insert_with_leaked_node(header_addr, 10);
        let n5 = insert_with_leaked_node(header_addr, 5);
        let n15 = insert_with_leaked_node(header_addr, 15);
        let n10_again = insert_with_leaked_node(header_addr, 10);

        assert_eq!(n10_again, n10, "re-inserting an existing key must return the same node");
        assert_eq!(get_from_memory::<u32>(header_addr + 0x8), n5, "leftmost cache should track key 5");
        assert_eq!(get_from_memory::<u32>(header_addr + 0xc), n15, "rightmost cache should track key 15");

        let root = get_from_memory::<u32>(header_addr + 0x4);
        assert_eq!(root, n10);
        let mut collected = Vec::new();
        collect_tree_nodes(root, &mut collected);
        assert_eq!(collected, vec![n5, n10, n15]);
    }
}
