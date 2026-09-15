//! Stage 1 (core data model) of the vanilla `ZTShowScriptMgr`/`ZTShowScript`/`ZTShowScriptItem`
//! reimplementation - see `openzt/plans/ztshowscriptmgr-implementation-plan.md` for the full staged
//! plan and the research this is based on.
//!
//! This is an **independent Rust store**, not vanilla-layout-compatible (an explicit, deliberate user
//! choice - see the plan's "Decision record"): `ZTShowScriptMgr` embeds directly into `ZTShowMgr` at
//! `+0x34` and owns an id-keyed collection of `ZTShowScript`s, each owning its own list of
//! `ZTShowScriptItem`s. None of that vanilla memory is read or written by this module - the real
//! (un-detoured) constructors still run and produce real, briefly-live heap allocations, but this
//! module treats their addresses purely as **opaque lookup keys**, never dereferencing them. This
//! sidesteps the cross-allocator hazard class entirely for everything except [`add_item`], documented
//! there.
//!
//! **Identity problem this module has to solve:** unlike `ztawardmgr.rs` (one static singleton) or
//! `ztthoughtmgr.rs` (one key per manager instance), `ZTShowScript`s are plural and dynamically
//! constructed, and two different call patterns can reach the *same* logical script by two different
//! raw addresses: (a) the code that constructed a script keeps using *that* address directly for
//! `save`/`addItem`/etc., while (b) other code (e.g. `ZTShow::doCurrentItem`, per the plan's confirmed
//! call site) looks a script up fresh via `getScript(id)` and chains further calls onto *that* return
//! value. [`get_script`] therefore mints a deterministic, collision-free synthetic handle
//! (`SYNTHETIC_SCRIPT_HANDLE_BASE | id`, never a real pointer - never dereference it, matching
//! `ztawardmgr.rs`'s `GET_AWARD` sentinel-return precedent) and registers it as an additional alias for
//! the same script id, so every other detour in this module resolves either identity to the same data.
//!
//! **Known Stage 1 limitation:** the plan's own "must reimplement" list (`ZTShow::start`,
//! `ZTShowInfo::checkUnitType`, `doCurrentItem`, `doTrickEvent`, `validateItem`, `addScript`,
//! `checkPendingScripts`) still raw-reads `ZTShowScript`/`ZTShowScriptItem` fields directly and is
//! Stage 2 scope, not touched here. Until Stage 2 lands, those functions remain real vanilla code that
//! will misbehave if they ever dereference a handle this module returns - matching the plan's own
//! verification note that no stage before Stage 2+ should be considered gameplay-safe, only
//! unit/live-test-safe. `ztshowscriptitem::*`'s own methods and `ztshowscript::{INIT, CONSTRUCTOR,
//! ZTSHOW_SCRIPT}` are intentionally left un-detoured: every Stage-1-detoured `ZTShowScript` method is
//! reimplemented fully in Rust and never calls through to the real `ZTShowScriptItem` FFI, and nothing
//! else in Stage 1's scope calls those directly (config-file item construction is Stage 3).

use std::{
    cell::{Cell, RefCell},
    collections::{BTreeMap, HashMap},
    sync::{LazyLock, Mutex},
};

use openzt_detour::generated::standalone::{DEALLOCATE, WRITE_BYTES_TO_FILE};

use crate::{
    encoding_utils::{decode_game_text, encode_to_ansi},
    util::{ZTBufferString, ZTString},
};

/// Deterministic, collision-free synthetic handle range for [`get_script`]'s return value - see the
/// module doc comment's "Identity problem" section. High bit set and well outside real heap address
/// ranges; **never a real pointer, never dereference it**.
const SYNTHETIC_SCRIPT_HANDLE_BASE: u32 = 0x7300_0000;

fn synthetic_script_handle(id: u16) -> u32 {
    SYNTHETIC_SCRIPT_HANDLE_BASE | id as u32
}

/// Owned Rust equivalent of vanilla's `ZTShowScriptItem` (real size `0x7c`/124 bytes, see the plan's
/// "Data model reference" table). Field order/defaults below match `ZTShowScriptItem::ZTShowScriptItem`/
/// `::init` exactly (`private/resources/decompiles/ZTShowScriptItem_{ZTShowScriptItem,init}.c`).
#[derive(Debug, Clone, PartialEq)]
pub struct ShowScriptItem {
    pub(crate) default_available: bool,
    pub(crate) visible: bool,
    pub(crate) id: u16,
    pub(crate) item_type: u32,
    /// Vanilla's `+0xc` sentinel dword - semantics unconfirmed, kept only for save/load byte fidelity.
    pub(crate) sentinel: u32,
    pub(crate) name: String,
    pub(crate) anim: String,
    pub(crate) keeper_pre_trick: String,
    pub(crate) keeper_post_trick: String,
    pub(crate) building: u32,
    pub(crate) complexity: u32,
    pub(crate) return_to_keeper: bool,
    pub(crate) satisfaction: u32,
    pub(crate) satisfaction_delta: u32,
    /// Vanilla's `+0x54` field - written identically to `satisfaction` by every real caller seen so
    /// far; semantics/independence from `satisfaction` unconfirmed, kept for byte fidelity.
    pub(crate) satisfaction_mirror: u32,
    pub(crate) minimum_depth: u32,
    pub(crate) normal_help_id: u32,
    pub(crate) grayed_help_id: u32,
    pub(crate) normal_icon: String,
    pub(crate) grayed_icon: String,
}

impl Default for ShowScriptItem {
    /// Matches `ZTShowScriptItem::ZTShowScriptItem`/`::init`'s real defaults exactly (confirmed by both
    /// independently agreeing): `visible`, `complexity`, `satisfaction`, `satisfaction_delta`,
    /// `satisfaction_mirror` and `minimum_depth` default to `1`/`true`, not `0`/`false`.
    fn default() -> Self {
        ShowScriptItem {
            default_available: false,
            visible: true,
            id: 0,
            item_type: 0,
            sentinel: 0xffff_ffff,
            name: String::new(),
            anim: String::new(),
            keeper_pre_trick: String::new(),
            keeper_post_trick: String::new(),
            building: 0,
            complexity: 1,
            return_to_keeper: false,
            satisfaction: 1,
            satisfaction_delta: 1,
            satisfaction_mirror: 1,
            minimum_depth: 1,
            normal_help_id: 0,
            grayed_help_id: 0,
            normal_icon: String::new(),
            grayed_icon: String::new(),
        }
    }
}

/// Raw, `#[repr(C)]`, byte-exact mirror of vanilla's `ZTShowScriptItem` (124 bytes) - **only** used as
/// the scalarized by-value argument shape `ZTShowScript::addItem` actually takes (confirmed via
/// `ZTShowScript_addItem.asm`'s `RET 0x7c`: the item is blitted directly onto the stack, not passed by
/// pointer - see [`add_item`]'s doc comment). Never constructed by this module - only read, as a
/// `&ZTShowScriptItemRaw` reinterpretation of the raw bytes the real ABI delivers.
#[repr(C)]
pub(crate) struct ZTShowScriptItemRaw {
    _vtable: u32,
    default_available: u8,
    visible: u8,
    id: u16,
    item_type: u32,
    sentinel: u32,
    name: ZTBufferString,
    anim: ZTBufferString,
    keeper_pre_trick: ZTBufferString,
    keeper_post_trick: ZTBufferString,
    building: u32,
    complexity: u32,
    return_to_keeper: u8,
    _pad: [u8; 3],
    satisfaction: u32,
    satisfaction_delta: u32,
    satisfaction_mirror: u32,
    minimum_depth: u32,
    normal_help_id: u32,
    grayed_help_id: u32,
    normal_icon: ZTBufferString,
    grayed_icon: ZTBufferString,
}

const _: () = assert!(std::mem::size_of::<ZTShowScriptItemRaw>() == 0x7c);

impl ZTShowScriptItemRaw {
    /// Reads this raw item's fields into an owned [`ShowScriptItem`] - copies the string bytes out of
    /// wherever the caller's temporary pointed (via `ZTBufferString::copy_to_string`), never taking
    /// ownership of that memory. See [`add_item`] for what happens to the original buffers afterward.
    fn to_owned_item(&self) -> ShowScriptItem {
        ShowScriptItem {
            default_available: self.default_available != 0,
            visible: self.visible != 0,
            id: self.id,
            item_type: self.item_type,
            sentinel: self.sentinel,
            name: self.name.copy_to_string(),
            anim: self.anim.copy_to_string(),
            keeper_pre_trick: self.keeper_pre_trick.copy_to_string(),
            keeper_post_trick: self.keeper_post_trick.copy_to_string(),
            building: self.building,
            complexity: self.complexity,
            return_to_keeper: self.return_to_keeper != 0,
            satisfaction: self.satisfaction,
            satisfaction_delta: self.satisfaction_delta,
            satisfaction_mirror: self.satisfaction_mirror,
            minimum_depth: self.minimum_depth,
            normal_help_id: self.normal_help_id,
            grayed_help_id: self.grayed_help_id,
            normal_icon: self.normal_icon.copy_to_string(),
            grayed_icon: self.grayed_icon.copy_to_string(),
        }
    }

    /// Reverse of [`Self::to_owned_item`] - builds a real, byte-exact raw item from the Rust store's own
    /// copy, for [`get_item_by_trick_id`]'s real-vanilla-consumer case (see its doc comment). String
    /// fields are left zeroed (`ZTBufferString::from_raw_parts(0, 0, 0)`, a safe empty-string
    /// representation as long as nothing calls `copy_to_string`/`get_cstr` on it) and `_vtable` is left
    /// `0` - neither is read by the one confirmed real consumer this exists for.
    fn from_owned(item: &ShowScriptItem) -> Self {
        let empty = ZTBufferString::from_raw_parts(0, 0, 0);
        ZTShowScriptItemRaw {
            _vtable: 0,
            default_available: item.default_available as u8,
            visible: item.visible as u8,
            id: item.id,
            item_type: item.item_type,
            sentinel: item.sentinel,
            name: empty.clone(),
            anim: empty.clone(),
            keeper_pre_trick: empty.clone(),
            keeper_post_trick: empty.clone(),
            building: item.building,
            complexity: item.complexity,
            return_to_keeper: item.return_to_keeper as u8,
            _pad: [0; 3],
            satisfaction: item.satisfaction,
            satisfaction_delta: item.satisfaction_delta,
            satisfaction_mirror: item.satisfaction_mirror,
            minimum_depth: item.minimum_depth,
            normal_help_id: item.normal_help_id,
            grayed_help_id: item.grayed_help_id,
            normal_icon: empty.clone(),
            grayed_icon: empty,
        }
    }
}

/// One registered `ZTShowScript`'s data - a script's own header fields plus its owned item list.
/// Matches `ZTShowScript::ZTShowScript`/`::init`'s real defaults (`private/resources/decompiles/
/// ZTShowScript_{ZTShowScript,init}.c`): `sentinel = 0xffffffff`, `script_type` from the constructor
/// argument, item list starts empty.
#[derive(Debug, Clone)]
struct ShowScriptData {
    sentinel: u32,
    script_type: u32,
    items: Vec<ShowScriptItem>,
}

#[derive(Default)]
struct ShowScriptMgrState {
    /// Canonical storage, keyed by script id (the manager's own rb-tree key).
    scripts: BTreeMap<u16, ShowScriptData>,
    /// Any known "this"-identity for a script (the real, un-detoured constructor's dead-weight address,
    /// or a [`synthetic_script_handle`] minted by [`get_script`]) resolves to its script id - see the
    /// module doc comment's "Identity problem" section.
    aliases: HashMap<u32, u16>,
    /// Vanilla's `DAT_0063e484` `makeID()` counter, owned directly by Rust (independent store - see the
    /// plan's "Data model reference" section on why this doesn't need vanilla's real memory address).
    next_id_counter: u16,
}

static STATE: LazyLock<Mutex<ShowScriptMgrState>> = LazyLock::new(|| Mutex::new(ShowScriptMgrState::default()));

/// Serializes plain `#[cfg(test)]` unit tests (in this file and in `ztshow.rs`) that touch the shared
/// process-global [`STATE`], avoiding cross-test interference under `cargo test`'s default parallel
/// execution. Distinct from [`live_support::reset_state`] (only compiled under `reimplementation-tests`,
/// used by the live harness against the real, running process) - plain unit tests need their own path.
#[cfg(test)]
pub(crate) static SHOW_SCRIPT_STORE_TEST_LOCK: Mutex<()> = Mutex::new(());

/// Resets [`STATE`] to empty - test-only, shared by this file's and `ztshow.rs`'s `#[cfg(test)]` tests.
#[cfg(test)]
pub(crate) fn reset_store_for_test() {
    let mut state = STATE.lock().unwrap();
    state.scripts.clear();
    state.aliases.clear();
    state.next_id_counter = 0;
}

/// Builds a matching-type-only [`ZTShowScriptItemRaw`] for plain `#[cfg(test)]` unit tests elsewhere in
/// the crate (e.g. `ztshow.rs`'s `check_script` test) - shares the module's own `#[cfg(test)]` `raw_item`
/// helper's field defaults. Distinct from `live_support::raw_item_matching_type`, which is gated behind
/// the `reimplementation-tests` feature (not enabled for a plain `cargo test`/`./openzt.bat test` run).
#[cfg(test)]
pub(crate) fn test_item(item_type: u32, trick_id: u16) -> ZTShowScriptItemRaw {
    let empty = ZTBufferString::from_raw_parts(0, 0, 0);
    ZTShowScriptItemRaw {
        _vtable: 0,
        default_available: 0,
        visible: 1,
        id: trick_id,
        item_type,
        sentinel: 0xffff_ffff,
        name: empty.clone(),
        anim: empty.clone(),
        keeper_pre_trick: empty.clone(),
        keeper_post_trick: empty.clone(),
        building: 0,
        complexity: 1,
        return_to_keeper: 0,
        _pad: [0; 3],
        satisfaction: 1,
        satisfaction_delta: 1,
        satisfaction_mirror: 1,
        minimum_depth: 1,
        normal_help_id: 0,
        grayed_help_id: 0,
        normal_icon: empty.clone(),
        grayed_icon: empty,
    }
}

/// Reimplementation of the mac-decompiled `makeID()`: `sVar1 = *counter; *counter = sVar1+1; return
/// (u16)(sVar1+1) % 0xffff;` - a wrapping counter, modulo `0xffff` (**not** `0x10000` - implemented
/// exactly, not "fixed").
fn next_script_id(counter: &mut u16) -> u16 {
    *counter = counter.wrapping_add(1);
    *counter % 0xffff
}

/// Reimplementation of `ZTShowScriptMgr::registerScript`. Real vanilla always calls this immediately
/// after construction with a freshly-constructed script (id always `0`, per
/// `ZTShowScript::ZTShowScript`'s own default) - unlike vanilla's real find-before-insert dance (which
/// only matters for the degenerate case of registering an already-registered id), this always assigns a
/// fresh id, which is what every real call site's actual usage pattern reduces to. Returns the assigned
/// id, or `None` for a null `ctor_ptr` (matching vanilla's own null check).
pub fn register_script(ctor_ptr: u32, script_type: u32) -> Option<u16> {
    if ctor_ptr == 0 {
        return None;
    }
    let mut state = STATE.lock().unwrap();
    let id = next_script_id(&mut state.next_id_counter);
    state.scripts.insert(id, ShowScriptData { sentinel: 0xffff_ffff, script_type, items: Vec::new() });
    state.aliases.insert(ctor_ptr, id);
    Some(id)
}

/// Reimplementation of `ZTShowScriptMgr::getScript`. Returns a [`synthetic_script_handle`] for `id`
/// (registered as an alias so every other detour in this module can resolve it), or `0` if `id` isn't
/// registered (matching vanilla's own `return 0` miss case).
pub fn get_script(id: u16) -> u32 {
    let mut state = STATE.lock().unwrap();
    if !state.scripts.contains_key(&id) {
        return 0;
    }
    let handle = synthetic_script_handle(id);
    state.aliases.insert(handle, id);
    handle
}

fn resolve(state: &ShowScriptMgrState, this_ptr: u32) -> Option<u16> {
    state.aliases.get(&this_ptr).copied()
}

/// Reimplementation of `ZTShowScriptMgr::unregisterScript`: removes the id→script mapping (matching
/// vanilla, which only erases the manager's tree node here, not the script object itself - real
/// destruction is a separate, Stage 2-scope call path). Returns `true` if `this_ptr` resolved to a
/// registered script.
pub fn unregister_script(this_ptr: u32) -> bool {
    let mut state = STATE.lock().unwrap();
    let Some(id) = resolve(&state, this_ptr) else { return false };
    state.scripts.remove(&id);
    state.aliases.retain(|_, v| *v != id);
    true
}

/// Reimplementation of `ZTShowScriptMgr::clearAllScripts`: drops every registered script and alias.
pub fn clear_all_scripts() -> bool {
    let mut state = STATE.lock().unwrap();
    state.scripts.clear();
    state.aliases.clear();
    true
}

/// Reimplementation of `ZTShowScript::size`.
pub fn size(this_ptr: u32) -> i32 {
    let state = STATE.lock().unwrap();
    resolve(&state, this_ptr).and_then(|id| state.scripts.get(&id)).map(|s| s.items.len() as i32).unwrap_or(0)
}

/// Reimplementation of `ZTShowScript::clearAll`: empties the item list, leaving the script's own header
/// fields untouched (matching vanilla).
pub fn clear_all(this_ptr: u32) {
    let mut state = STATE.lock().unwrap();
    if let Some(id) = resolve(&state, this_ptr)
        && let Some(script) = state.scripts.get_mut(&id)
    {
        script.items.clear();
    }
}

/// Fixed-size, per-thread ring buffer of leaked [`ZTShowScriptItemRaw`] slots, reused round-robin instead
/// of leaking a fresh allocation on every call - see [`get_item`]/[`get_item_by_trick_id`]'s own doc
/// comments for why a real, dereferenceable pointer is required at all (a non-dereferenceable sentinel
/// isn't safe here). Bounds the leak at [`ITEM_BUFFER_POOL_SIZE`] allocations per thread instead of
/// unbounded growth over a long play session, without needing an airtight proof that no real consumer
/// ever holds a returned pointer across more than [`ITEM_BUFFER_POOL_SIZE`] subsequent calls on the same
/// thread - a handful of slots tolerates limited reentrancy safely even if such a case is later found.
/// [`get_item`] and [`get_item_by_trick_id`] each get their own pool (see their `thread_local!` decls
/// below): their real consumer call graphs are separate, and sharing one pool between them would need a
/// stronger reentrancy proof than currently exists.
const ITEM_BUFFER_POOL_SIZE: usize = 4;

struct ItemBufferPool {
    slots: RefCell<Vec<*mut ZTShowScriptItemRaw>>,
    cursor: Cell<usize>,
}

impl ItemBufferPool {
    const fn new() -> Self {
        ItemBufferPool { slots: RefCell::new(Vec::new()), cursor: Cell::new(0) }
    }

    /// Writes `item` into the next slot (allocating all [`ITEM_BUFFER_POOL_SIZE`] slots up front on first
    /// use), advances the round-robin cursor, and returns the slot's address as a `u32` for FFI return.
    fn write(&self, item: &ShowScriptItem) -> u32 {
        let mut slots = self.slots.borrow_mut();
        if slots.is_empty() {
            slots.extend((0..ITEM_BUFFER_POOL_SIZE).map(|_| Box::leak(Box::new(ZTShowScriptItemRaw::from_owned(item))) as *mut ZTShowScriptItemRaw));
        }
        let i = self.cursor.get();
        self.cursor.set((i + 1) % ITEM_BUFFER_POOL_SIZE);
        unsafe { *slots[i] = ZTShowScriptItemRaw::from_owned(item) };
        slots[i] as u32
    }
}

thread_local! {
    static GET_ITEM_POOL: ItemBufferPool = const { ItemBufferPool::new() };
    static GET_ITEM_BY_TRICK_ID_POOL: ItemBufferPool = const { ItemBufferPool::new() };
}

/// Reimplementation of `ZTShowScript::getItem` (positional, **not** id-keyed - confirmed via
/// `ZTShowScript_getItem.c`: it walks `param_1` nodes from the list head).
///
/// Previously returned a non-dereferenceable found/not-found sentinel (matching `ztawardmgr.rs`'s
/// `GET_AWARD` precedent), on the assumption nothing in Stage 1's own scope consumed the return value.
/// That assumption was wrong: `BFUnit::listen` (`BFUnit_listen_0.c`, real, un-reimplemented, reached on
/// real keeper/trick UI events `0x271d`/`0x271f`) calls `ZTShowScript::getItem` directly and, on a
/// nonzero result, dereferences the returned pointer's `+0x28`/`+0x34` string fields via
/// `BFConfigFile::getString` - the exact same "real vanilla code raw-dereferences a Stage 1 lookup's
/// return value" hazard class as [`get_item_by_trick_id`] (see that function's own doc comment for the
/// two earlier, independently-discovered instances). Fixed the same way: builds a real
/// [`ZTShowScriptItemRaw`] instead of a `0`/`1` sentinel, written into [`GET_ITEM_POOL`]'s next slot
/// (bounded, not a per-call leak - see [`ItemBufferPool`]'s own doc comment). Every existing detoured
/// caller of this function already only checked the result for zero/nonzero, so returning a real pointer
/// instead is backward-compatible for them.
pub fn get_item(this_ptr: u32, index: u16) -> u32 {
    let state = STATE.lock().unwrap();
    let Some(item) = resolve(&state, this_ptr).and_then(|id| state.scripts.get(&id)).and_then(|s| s.items.get(index as usize)) else {
        return 0;
    };
    GET_ITEM_POOL.with(|pool| pool.write(item))
}

/// Reimplementation of `ZTShowScript::getItemByTrickID` - confirmed via `ZTShowScript_getItemByTrickID.c`
/// to match against each item's own `id` field (struct offset `0x6`) directly, i.e. "trick ID" is the
/// same field this module calls `id`, not a separate one.
///
/// Unlike [`get_item`], this **cannot** use a non-dereferenceable found/not-found sentinel: real, untouched
/// vanilla code (`ZTUnitType::getShowItem` -> `ZTUnit::getShowItem`, reached from `ZTAnimal::
/// validateTrickType` - `ZTUnit`'s own vtable slot `0x218`, itself called from `ztshow::validate_item`'s
/// vtable dispatch) directly dereferences the returned `ZTShowScriptItem*`'s `+0x58`/`+0xc`/`+0x40` fields.
/// Confirmed live: the previous found/not-found sentinel (`1`) was misread as a pointer, crashing on
/// `mov ecx,[ebx+0x58]` with `ebx==1` (`openzt/plans/ztshowscriptmgr-open-items.md` item 12) - a third,
/// independently-discovered raw-dereferencing consumer of Stage 1 data, same hazard class as `validate`/
/// `stop_with_id`. Builds a real [`ZTShowScriptItemRaw`] (the same byte-exact struct [`add_item`]
/// already validates against vanilla's real by-value ABI) populated from the Rust store's own item copy -
/// see [`ZTShowScriptItemRaw::from_owned`] for what's left unpopulated and why that's safe here. Written
/// into [`GET_ITEM_BY_TRICK_ID_POOL`]'s next slot rather than freed: mirrors a real vanilla lookup (never
/// an ownership transfer - the real function doesn't free its result either), bounded to
/// [`ITEM_BUFFER_POOL_SIZE`] live slots per thread rather than the cross-allocator free hazard CLAUDE.md
/// warns about, or an unbounded per-call leak.
pub fn get_item_by_trick_id(this_ptr: u32, trick_id: u16) -> u32 {
    let state = STATE.lock().unwrap();
    let Some(item) = resolve(&state, this_ptr).and_then(|id| state.scripts.get(&id)).and_then(|s| s.items.iter().find(|it| it.id == trick_id))
    else {
        return 0;
    };
    GET_ITEM_BY_TRICK_ID_POOL.with(|pool| pool.write(item))
}

/// Reimplementation of `ZTShowScript::removeItem` (positional, matching [`get_item`]).
pub fn remove_item(this_ptr: u32, index: u16) -> bool {
    let mut state = STATE.lock().unwrap();
    let Some(id) = resolve(&state, this_ptr) else { return false };
    let Some(script) = state.scripts.get_mut(&id) else { return false };
    if (index as usize) < script.items.len() {
        script.items.remove(index as usize);
        true
    } else {
        false
    }
}

/// Snapshot of an item's event-relevant fields, by *positional* index (matching `ZTShowScript::getItem`'s
/// own indexing, same as [`get_item`]) - added for Stage 2 (`ztshow.rs`)'s `doTrickEvent`/`doCurrentItem`/
/// `validateItem` ports, which (unlike Stage 1's own detoured callers) need the item's real field values,
/// not just a found/not-found sentinel.
#[derive(Debug, Clone, Copy)]
pub struct ItemSnapshot {
    pub id: u16,
    pub item_type: u32,
    pub satisfaction: u32,
    pub satisfaction_mirror: u32,
}

/// Id-keyed (not handle-keyed) positional item lookup - see [`ItemSnapshot`]. Bypasses the
/// handle/alias table entirely since Stage 2 callers already have the id directly (from `ZTShow`'s own
/// `+0x4` field), avoiding unnecessary alias-table churn on a per-tick call path.
pub fn item_snapshot_by_id(id: u16, index: u16) -> Option<ItemSnapshot> {
    let state = STATE.lock().unwrap();
    let item = state.scripts.get(&id)?.items.get(index as usize)?;
    Some(ItemSnapshot { id: item.id, item_type: item.item_type, satisfaction: item.satisfaction, satisfaction_mirror: item.satisfaction_mirror })
}

/// Id-keyed `ZTShowScript::size()` equivalent - see [`item_snapshot_by_id`] on why Stage 2 uses id-keyed
/// accessors directly instead of going through [`get_script`]'s handle/alias indirection.
pub fn script_item_count_by_id(id: u16) -> usize {
    STATE.lock().unwrap().scripts.get(&id).map(|s| s.items.len()).unwrap_or(0)
}

/// Id-keyed script `type` field read (vanilla `ZTShowScript+0xc`) - see [`item_snapshot_by_id`].
pub fn script_type_by_id(id: u16) -> Option<u32> {
    STATE.lock().unwrap().scripts.get(&id).map(|s| s.script_type)
}

/// Id-keyed existence check - see [`item_snapshot_by_id`]. Used by Stage 2 callers that only need
/// `getScript(id) != 0`'s truth value, not a dereferenceable handle.
pub fn script_exists_by_id(id: u16) -> bool {
    STATE.lock().unwrap().scripts.contains_key(&id)
}

/// Full, id-keyed, positional item clone - unlike [`item_snapshot_by_id`] (which only exposes the four
/// fields Stage 2's per-tick `ZTShow`/`ZTShowInfo` ports need), this exposes every field, for Stage 4's
/// `fillTrickLists` port (`ztshowui.rs`), which needs to reproduce vanilla's real
/// `*(ZTShowScriptItem*)(ZTShowScript::getItem(...))` raw field reads (`visible`, `id`, `normalIcon`,
/// `grayedIcon`, `normalHelpID`) for the "assigned tricks" listbox. Predates [`get_item`] returning a real
/// leaked pointer (added later to fix a raw-dereferencing real-vanilla consumer, see that function's own
/// doc comment) - kept as the id-keyed accessor Stage 4 already relies on rather than switched over, since
/// it avoids the handle/alias table indirection [`get_item`] still goes through.
pub(crate) fn item_full_by_id(id: u16, index: u16) -> Option<ShowScriptItem> {
    let state = STATE.lock().unwrap();
    state.scripts.get(&id)?.items.get(index as usize).cloned()
}

/// Id-keyed [`unregister_script`] equivalent - see [`item_snapshot_by_id`]. Used by Stage 2's
/// `ZTShowInfo::checkPendingScripts`/`addScript` ports to replace their real vanilla bodies' direct
/// `ZTShowScript::~ZTShowScript()` calls (unsafe against our synthetic handles - see the module doc
/// comment) with a plain store removal.
pub fn unregister_script_by_id(id: u16) -> bool {
    let mut state = STATE.lock().unwrap();
    if state.scripts.remove(&id).is_some() {
        state.aliases.retain(|_, v| *v != id);
        true
    } else {
        false
    }
}

/// Reimplementation of `ZTShowScript::addItem`. Real vanilla only inserts when `item.type ==
/// this->type` (`ZTShowScript_addItem.c:12`) - checked and reproduced here - and, in **both** the
/// matching and non-matching case, destroys the passed-in item's six string members before returning
/// (the shared `.179` cleanup label in `ZTShowScript_addItem.asm`, reached either way).
///
/// **Known leak, deliberate:** those six strings' backing buffers were allocated by the real (still
/// un-reimplemented) caller via vanilla's own allocator before this call. Freeing them correctly would
/// mean calling vanilla's own `std::basic_string::~basic_string()` - no confirmed address exists for it
/// anywhere in this repo, and guessing one risks exactly the cross-allocator heap corruption class
/// CLAUDE.md warns about, which is strictly worse than a small, bounded leak on a call path that isn't a
/// hot loop (show-editor apply / default-script creation, both Stage 3/4 scope). This mirrors CLAUDE.md's
/// "leak-only teardown" precedent (`ztthoughtmgr.rs`'s `destroy_standalone_mgr_leaking_nodes`), applied
/// to a by-value argument's embedded buffers instead of a linked list.
pub fn add_item(this_ptr: u32, item: &ZTShowScriptItemRaw) -> u32 {
    let mut state = STATE.lock().unwrap();
    if let Some(id) = resolve(&state, this_ptr)
        && let Some(script) = state.scripts.get_mut(&id)
        && item.item_type == script.script_type
    {
        script.items.push(item.to_owned_item());
    }
    0
}

// ---------------------------------------------------------------------------------------------
// Save/load wire format - byte-for-byte matches vanilla's real save-game format (confirmed via
// `ZTShowScriptMgr_{save,load}.c`, `ZTShowScript-old_save.c`, `ZTShowScript_load.c`,
// `ZTShowScriptItem_{save,load_0}.c`), so existing save files stay loadable.
// ---------------------------------------------------------------------------------------------

const STRING_LENGTH_CAP: u32 = 0x1000;

/// Sanity cap on a stream-read item count for one script - not a real vanilla limit (vanilla trusts the
/// stream unconditionally, per `ZTShowScript_load.c`'s own unchecked loop), just a guard against
/// [`read_script`] driving `Vec::with_capacity` off a garbage length read from a corrupted save file. A
/// real script's item list is a handful of tricks; this is generous headroom, not a tight bound.
const MAX_SCRIPT_ITEM_COUNT: u32 = 0x1000;

/// Sanity cap on a stream-read script count for the whole manager - same guard as
/// [`MAX_SCRIPT_ITEM_COUNT`] for [`load_mgr`]'s own count read. Script ids are `u16`, so the store can
/// never legitimately hold more than `0x10000` distinct scripts.
const MAX_SCRIPT_COUNT: u32 = 0x1_0000;

fn write_string(buf: &mut Vec<u8>, s: &str) {
    let encoded = encode_to_ansi(s);
    let len = (encoded.len() as u32).min(STRING_LENGTH_CAP - 1);
    buf.extend_from_slice(&len.to_le_bytes());
    buf.extend_from_slice(&encoded[..len as usize]);
}

fn encode_item(item: &ShowScriptItem) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.push(item.default_available as u8);
    buf.push(item.visible as u8);
    buf.extend_from_slice(&item.id.to_le_bytes());
    buf.extend_from_slice(&item.item_type.to_le_bytes());
    buf.extend_from_slice(&item.sentinel.to_le_bytes());
    write_string(&mut buf, &item.name);
    write_string(&mut buf, &item.anim);
    write_string(&mut buf, &item.keeper_pre_trick);
    write_string(&mut buf, &item.keeper_post_trick);
    buf.extend_from_slice(&item.building.to_le_bytes());
    buf.extend_from_slice(&item.complexity.to_le_bytes());
    buf.push(item.return_to_keeper as u8);
    buf.extend_from_slice(&item.satisfaction.to_le_bytes());
    buf.extend_from_slice(&item.satisfaction_delta.to_le_bytes());
    buf.extend_from_slice(&item.satisfaction_mirror.to_le_bytes());
    buf.extend_from_slice(&item.minimum_depth.to_le_bytes());
    buf.extend_from_slice(&item.normal_help_id.to_le_bytes());
    buf.extend_from_slice(&item.grayed_help_id.to_le_bytes());
    write_string(&mut buf, &item.normal_icon);
    write_string(&mut buf, &item.grayed_icon);
    buf
}

fn encode_script(id: u16, script: &ShowScriptData) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&id.to_le_bytes());
    buf.extend_from_slice(&script.sentinel.to_le_bytes());
    buf.extend_from_slice(&script.script_type.to_le_bytes());
    buf.extend_from_slice(&(script.items.len() as u32).to_le_bytes());
    for item in &script.items {
        buf.extend_from_slice(&encode_item(item));
    }
    buf
}

/// Pure encode of the whole manager - one buffer, written via a single `WriteBytesToFile` call
/// (matching `ztresearch.rs`'s `research_save_reimplementation` precedent).
fn encode_mgr(state: &ShowScriptMgrState) -> Vec<u8> {
    let mut buf = Vec::new();
    buf.extend_from_slice(&(state.scripts.len() as u32).to_le_bytes());
    // BTreeMap iterates ascending by id, matching the rb-tree's real in-order walk.
    for (&id, script) in &state.scripts {
        buf.extend_from_slice(&encode_script(id, script));
    }
    buf.extend_from_slice(&state.next_id_counter.to_le_bytes());
    buf
}

pub fn save_mgr(file: *const u32) -> bool {
    let state = STATE.lock().unwrap();
    let bytes = encode_mgr(&state);
    (unsafe { WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, bytes.len() as u32, 1, file as *const i8) }) == 1
}

/// Reimplementation of the "old" `ZTShowScript::save` (real name; `ztshowscript_old` per
/// `generated.rs`), called per-script by [`save_mgr`]'s real vanilla counterpart. `this_ptr` must
/// already resolve via [`resolve`] - `false` otherwise.
pub fn save_script(this_ptr: u32, file: *const u32) -> bool {
    let state = STATE.lock().unwrap();
    let Some(id) = resolve(&state, this_ptr) else { return false };
    let Some(script) = state.scripts.get(&id) else { return false };
    let bytes = encode_script(id, script);
    (unsafe { WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, bytes.len() as u32, 1, file as *const i8) }) == 1
}

fn read_bytes(file: *const u32, buf: &mut [u8]) -> bool {
    unsafe { DEALLOCATE.hooked()(buf.as_mut_ptr() as *const u32, buf.len() as u32, 1, file as *const u8) == 1 }
}

fn read_u8(file: *const u32) -> Option<u8> {
    let mut b = [0u8; 1];
    read_bytes(file, &mut b).then_some(b[0])
}

fn read_u16(file: *const u32) -> Option<u16> {
    let mut b = [0u8; 2];
    read_bytes(file, &mut b).then(|| u16::from_le_bytes(b))
}

fn read_u32(file: *const u32) -> Option<u32> {
    let mut b = [0u8; 4];
    read_bytes(file, &mut b).then(|| u32::from_le_bytes(b))
}

fn read_string(file: *const u32) -> Option<String> {
    let len = read_u32(file)?;
    if len >= STRING_LENGTH_CAP {
        return None;
    }
    if len == 0 {
        return Some(String::new());
    }
    let mut buf = vec![0u8; len as usize];
    read_bytes(file, &mut buf).then(|| decode_game_text(&buf))
}

/// Reimplementation of `ZTShowScriptItem::load`'s stream-based overload (`generated.rs`'s
/// `ztshowscriptitem::LOAD_0`, 0x0046d2d7 - **not** `LOAD_1`/0x4b9690, which is the unrelated
/// `.cfg`-parsing overload used by Stage 3's config-loading paths). Version-gated exactly like the real
/// stream reader: base fields require `version > 0x58`, `normalHelpID`/`grayedHelpID`/icon strings
/// additionally require `version > 0x66`. Fields not covered by an older stream keep
/// [`ShowScriptItem::default`]'s values, matching what a freshly-constructed item already had before
/// vanilla's loader started overwriting fields in place.
fn read_item(file: *const u32, version: u32) -> Option<ShowScriptItem> {
    let mut item = ShowScriptItem::default();
    if version > 0x58 {
        item.default_available = read_u8(file)? != 0;
        item.visible = read_u8(file)? != 0;
        item.id = read_u16(file)?;
        item.item_type = read_u32(file)?;
        item.sentinel = read_u32(file)?;
        item.name = read_string(file)?;
        item.anim = read_string(file)?;
        item.keeper_pre_trick = read_string(file)?;
        item.keeper_post_trick = read_string(file)?;
        item.building = read_u32(file)?;
        item.complexity = read_u32(file)?;
        item.return_to_keeper = read_u8(file)? != 0;
        item.satisfaction = read_u32(file)?;
        item.satisfaction_delta = read_u32(file)?;
        item.satisfaction_mirror = read_u32(file)?;
        item.minimum_depth = read_u32(file)?;
    }
    if version > 0x66 {
        item.normal_help_id = read_u32(file)?;
        item.grayed_help_id = read_u32(file)?;
        item.normal_icon = read_string(file)?;
        item.grayed_icon = read_string(file)?;
    }
    Some(item)
}

/// Reimplementation of `ZTShowScript::load`: reads the script's own header, then `count` items via
/// [`read_item`]. No-op (returns the incoming `version` unchanged as success per vanilla's own
/// `if (0x58 < param_2)` gate) for an older stream.
fn read_script(file: *const u32, version: u32) -> Option<(u16, ShowScriptData)> {
    if version <= 0x58 {
        return Some((0, ShowScriptData { sentinel: 0xffff_ffff, script_type: 0, items: Vec::new() }));
    }
    let id = read_u16(file)?;
    let sentinel = read_u32(file)?;
    let script_type = read_u32(file)?;
    let count = read_u32(file)?;
    if count > MAX_SCRIPT_ITEM_COUNT {
        return None;
    }
    let mut items = Vec::with_capacity(count as usize);
    for _ in 0..count {
        items.push(read_item(file, version)?);
    }
    Some((id, ShowScriptData { sentinel, script_type, items }))
}

/// Reimplementation of `ZTShowScriptMgr::load`: clears the store, reads `count` scripts (each inserted
/// under the id its own stream data carries - **not** auto-assigned via [`register_script`], matching
/// vanilla constructing load-time scripts with `autoRegister = false`), then - only for `version > 0x60`
/// - restores the persisted `makeID` counter.
pub fn load_mgr(file: *const u32, version: u32) -> bool {
    let mut state = STATE.lock().unwrap();
    state.scripts.clear();
    state.aliases.clear();
    if version <= 0x58 {
        return true;
    }
    let Some(count) = read_u32(file) else {
        return false;
    };
    if count > MAX_SCRIPT_COUNT {
        return false;
    }
    for _ in 0..count {
        let Some((id, script)) = read_script(file, version) else {
            return false;
        };
        state.scripts.insert(id, script);
    }
    if version > 0x60 {
        let Some(counter) = read_u16(file) else {
            return false;
        };
        state.next_id_counter = counter;
    }
    true
}

pub fn init() {
    ztshowscriptmgr_detours::init();
    ztshowscript_detours::init();
}

mod ztshowscriptmgr_detours {
    use openzt_detour::generated::{
        ztshowscript::SAVE as SAVE_SCRIPT_OLD,
        ztshowscriptmgr::{CLEAR_ALL_SCRIPTS, GET_SCRIPT, LOAD, REGISTER_SCRIPT, SAVE, UNREGISTER_SCRIPT},
    };
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(REGISTER_SCRIPT)]
        unsafe extern "thiscall" fn register_script(_this: *const u32, script: *const u32) -> u32 {
            if script.is_null() {
                return 0;
            }
            // Real `ZTShowScript::ZTShowScript` always writes `type` at `+0xc` before an
            // auto-registering call reaches here - safe to read from the still-live, un-detoured
            // constructor's dead-weight allocation (never written back, only read once here).
            let script_type = crate::util::get_from_memory::<u32>(script as u32 + 0xc);
            match crate::ztshowscriptmgr::register_script(script as u32, script_type) {
                Some(id) => {
                    // Real `ZTShowScriptMgr::registerScript` stamps the assigned id into the
                    // script object's own `+0x4` field before returning (`ZTShowScriptMgr_
                    // registerScript.c:32`) - several real, un-reimplemented callers still read
                    // it raw afterward (e.g. `ZTShowInfo::addUnitToList`'s `*(ushort*)(script+4)`,
                    // feeding straight into `ZTShowInfo::addScript`'s `new_script_id` parameter),
                    // so this write-back has to happen even though our own independent store never
                    // re-reads it. Without it, every real caller sees the ctor's default `0` here,
                    // and `add_script` silently no-ops on `new_script_id == 0` - found while tracing
                    // Stage 3's `ZTShowInfo::createDefaultScript` consumer.
                    crate::util::save_to_memory::<u16>(script as u32 + 4, id);
                    1
                }
                None => 0,
            }
        }

        #[detour(SAVE)]
        unsafe extern "thiscall" fn save(_this: *const u32, file: *const i8) -> bool {
            crate::ztshowscriptmgr::save_mgr(file as *const u32)
        }

        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(_this: *const u32, file: *const u32, version: u32) -> bool {
            crate::ztshowscriptmgr::load_mgr(file, version)
        }

        #[detour(GET_SCRIPT)]
        unsafe extern "thiscall" fn get_script(_this: *const u32, id: u16) -> u32 {
            crate::ztshowscriptmgr::get_script(id)
        }

        #[detour(UNREGISTER_SCRIPT)]
        unsafe extern "thiscall" fn unregister_script(_this: *const u32, script: *const u32) -> u32 {
            crate::ztshowscriptmgr::unregister_script(script as u32) as u32
        }

        #[detour(CLEAR_ALL_SCRIPTS)]
        unsafe extern "thiscall" fn clear_all_scripts(_this: *const u32) -> u32 {
            crate::ztshowscriptmgr::clear_all_scripts() as u32
        }

        #[detour(SAVE_SCRIPT_OLD)]
        unsafe extern "thiscall" fn save_script_old(this: *const u32, file: *const i8) -> bool {
            crate::ztshowscriptmgr::save_script(this as u32, file as *const u32)
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztshowscriptmgr detours: {e:?}");
        }
    }
}

mod ztshowscript_detours {
    use openzt_detour::generated::ztshowscript::{ADD_ITEM, CLEAR_ALL, GET_ITEM, GET_ITEM_BY_TRICK_ID, LOAD, REMOVE_ITEM, SIZE};
    use openzt_detour_macro::detour_mod;
    use tracing::error;

    #[detour_mod]
    mod detours {
        use super::*;

        #[detour(SIZE)]
        unsafe extern "thiscall" fn size(this: *const u32) -> i32 {
            crate::ztshowscriptmgr::size(this as u32)
        }

        #[detour(CLEAR_ALL)]
        unsafe extern "thiscall" fn clear_all(this: *const u32) {
            crate::ztshowscriptmgr::clear_all(this as u32);
        }

        #[detour(GET_ITEM)]
        unsafe extern "thiscall" fn get_item(this: *const u32, index: u16) -> u32 {
            crate::ztshowscriptmgr::get_item(this as u32, index)
        }

        #[detour(GET_ITEM_BY_TRICK_ID)]
        unsafe extern "thiscall" fn get_item_by_trick_id(this: *const u32, trick_id: u16) -> u32 {
            crate::ztshowscriptmgr::get_item_by_trick_id(this as u32, trick_id)
        }

        #[detour(REMOVE_ITEM)]
        unsafe extern "thiscall" fn remove_item(this: *const u32, index: u16) -> u32 {
            crate::ztshowscriptmgr::remove_item(this as u32, index) as u32
        }

        // `[u8; 0x7c]` isn't recognized as "FFI-safe" by rustc's conservative lint, but the x86 Windows
        // by-value struct/array-passing convention treats a byte array and a same-sized `#[repr(C)]`
        // struct identically (both blitted onto the stack) - confirmed real here via
        // `ZTShowScript_addItem.asm`'s `RET 0x7c`, see the hand-patched `ADD_ITEM` entry's comment in
        // `generated.rs`.
        #[allow(improper_ctypes_definitions)]
        #[detour(ADD_ITEM)]
        unsafe extern "thiscall" fn add_item(this: *const u32, item: [u8; 0x7c]) -> u32 {
            let item = unsafe { &*(item.as_ptr() as *const crate::ztshowscriptmgr::ZTShowScriptItemRaw) };
            crate::ztshowscriptmgr::add_item(this as u32, item)
        }

        #[detour(LOAD)]
        unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
            let Some((id, script)) = crate::ztshowscriptmgr::read_script(file, version) else { return false };
            let mut state = crate::ztshowscriptmgr::STATE.lock().unwrap();
            state.aliases.insert(this as u32, id);
            state.scripts.insert(id, script);
            // Same write-back as `REGISTER_SCRIPT` (see its doc comment) - real `ZTShowScript::load`
            // also stamps the stream's id into `this->mbr_0x4` (`ZTShowScript_load.c:51`'s first
            // `deallocate` call), so a loaded save's script needs it set too.
            crate::util::save_to_memory::<u16>(this as u32 + 4, id);
            true
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise ztshowscript detours: {e:?}");
        }
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    /// Resets this module's independent store to empty - test-only, mirroring `ztawardmgr.rs`'s
    /// `reset_reimplemented_store` precedent (there's no real vanilla memory to reset alongside it).
    pub(crate) fn reset_state() {
        clear_all_scripts();
        STATE.lock().unwrap().next_id_counter = 0;
    }

    pub(crate) fn registered_script_count() -> usize {
        STATE.lock().unwrap().scripts.len()
    }

    /// Every script id currently in the store, ascending - for a live test to snapshot which real
    /// scripts a real zoo load populated, before/after a round-trip through [`super::encode_mgr`]/
    /// [`super::load_mgr`] (which [`snapshot_encoded`] and the real `SAVE`/`LOAD` addresses both
    /// exercise).
    pub(crate) fn all_script_ids() -> Vec<u16> {
        STATE.lock().unwrap().scripts.keys().copied().collect()
    }

    /// Pure encode of the current store state, bypassing `WriteBytesToFile`/`io_redirect` entirely -
    /// lets a live test diff [`super::encode_mgr`]'s output against what [`super::load_mgr`] reads back
    /// from it, directly, without needing a real or captured file handle.
    pub(crate) fn snapshot_encoded() -> Vec<u8> {
        super::encode_mgr(&STATE.lock().unwrap())
    }

    /// Reads the `makeID()` counter directly - for `ZTSHOWSCRIPTMGR_LOAD_VERSION_GATES_LIVE`'s own
    /// assertions on whether [`super::load_mgr`]'s `version > 0x60` counter-restore gate fired.
    pub(crate) fn next_id_counter() -> u16 {
        STATE.lock().unwrap().next_id_counter
    }

    /// Builds a [`ZTShowScriptItemRaw`] with `item_type`/`id` set and every other field a plain, inert
    /// default (empty strings via `ZTBufferString::from_raw_parts(0, 0, 0)`, matching this module's own
    /// `#[cfg(test)]` `raw_item` helper) - for live reimplementation-tests elsewhere in the crate (e.g.
    /// `reimplementation_tests/mod.rs`'s `ZTSHOWINFO_ADD_SCRIPT_CHECK_PENDING_SCRIPTS_LIVE`) that need to
    /// hand [`add_item`] a matching-type item but can't construct a `ZTShowScriptItemRaw` directly - its
    /// fields are module-private, only reachable from within this module (or a descendant, like this
    /// `live_support` module) even though the struct itself is `pub(crate)`.
    pub(crate) fn raw_item_matching_type(item_type: u32, trick_id: u16) -> ZTShowScriptItemRaw {
        ZTShowScriptItemRaw {
            _vtable: 0,
            default_available: 0,
            visible: 1,
            id: trick_id,
            item_type,
            sentinel: 0xffff_ffff,
            name: ZTBufferString::from_raw_parts(0, 0, 0),
            anim: ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_pre_trick: ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_post_trick: ZTBufferString::from_raw_parts(0, 0, 0),
            building: 0,
            complexity: 1,
            return_to_keeper: 0,
            _pad: [0; 3],
            satisfaction: 1,
            satisfaction_delta: 1,
            satisfaction_mirror: 1,
            minimum_depth: 1,
            normal_help_id: 0,
            grayed_help_id: 0,
            normal_icon: ZTBufferString::from_raw_parts(0, 0, 0),
            grayed_icon: ZTBufferString::from_raw_parts(0, 0, 0),
        }
    }

    /// Same shape as [`raw_item_matching_type`], but with explicit `satisfaction`/`satisfaction_mirror`
    /// values instead of both hardcoded to `1` - for `ZTSHOW_GROUP3_TRICK_LIVE`'s `do_trick_event`
    /// threshold-branch coverage, which needs to place a real item's `satisfaction_mirror` on either side
    /// of `ZTShowMgr`'s own real, live `threshold_a`/`threshold_b`/`threshold_c` fields.
    pub(crate) fn raw_item_with_mirror(item_type: u32, trick_id: u16, satisfaction: u32, satisfaction_mirror: u32) -> ZTShowScriptItemRaw {
        let mut item = raw_item_matching_type(item_type, trick_id);
        item.satisfaction = satisfaction;
        item.satisfaction_mirror = satisfaction_mirror;
        item
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn next_script_id_wraps_at_0xffff_not_0x10000() {
        let mut counter = 0xfffeu16;
        assert_eq!(next_script_id(&mut counter), 0xffff % 0xffff);
        assert_eq!(counter, 0xffff);
        assert_eq!(next_script_id(&mut counter), 0);
        assert_eq!(counter, 0);
    }

    #[test]
    fn register_get_unregister_roundtrip() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        let id = register_script(0x1000, 42).unwrap();
        let handle = get_script(id);
        assert_ne!(handle, 0);
        assert_eq!(size(handle), 0);
        // Both the original ctor pointer and the synthetic handle resolve to the same script.
        assert_eq!(size(0x1000), 0);
        assert!(unregister_script(0x1000));
        assert_eq!(get_script(id), 0, "unregistered id should no longer resolve via getScript");
    }

    #[test]
    fn register_script_rejects_null() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        assert_eq!(register_script(0, 1), None);
    }

    #[test]
    fn add_item_only_inserts_matching_type() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        let id = register_script(0x2000, 7).unwrap();
        let matching = raw_item(7, 5);
        let mismatched = raw_item(9, 6);
        assert_eq!(add_item(0x2000, &matching), 0);
        assert_eq!(add_item(0x2000, &mismatched), 0);
        assert_eq!(size(0x2000), 1, "only the type-matching item should have been inserted");
        let item_ptr = get_item(0x2000, 0);
        assert_ne!(item_ptr, 0, "found item should return a real, non-null pointer, not a boolean sentinel");
        let raw = unsafe { &*(item_ptr as *const ZTShowScriptItemRaw) };
        assert_eq!(raw.id, 5);
        assert_eq!(raw.item_type, 7);
        assert_eq!(get_item(0x2000, 1), 0, "out-of-range index should return 0");
        let found_ptr = get_item_by_trick_id(0x2000, 5);
        assert_ne!(found_ptr, 0, "found item should return a real, non-null pointer, not a boolean sentinel");
        let raw = unsafe { &*(found_ptr as *const ZTShowScriptItemRaw) };
        assert_eq!(raw.id, 5);
        assert_eq!(raw.item_type, 7);
        assert_eq!(get_item_by_trick_id(0x2000, 6), 0, "not found should return 0, matching vanilla's real getItemByTrickID");
    }

    #[test]
    fn remove_and_clear_items() {
        let _guard = SHOW_SCRIPT_STORE_TEST_LOCK.lock().unwrap();
        reset_store_for_test();
        register_script(0x3000, 1).unwrap();
        add_item(0x3000, &raw_item(1, 1));
        add_item(0x3000, &raw_item(1, 2));
        assert_eq!(size(0x3000), 2);
        assert!(remove_item(0x3000, 0));
        assert_eq!(size(0x3000), 1);
        assert!(!remove_item(0x3000, 5), "out-of-range index should fail");
        clear_all(0x3000);
        assert_eq!(size(0x3000), 0);
    }

    #[test]
    fn encode_decode_item_roundtrip_matches_wire_format() {
        let mut item = ShowScriptItem::default();
        item.id = 7;
        item.item_type = 3;
        item.name = "trick".to_string();
        item.satisfaction = 99;
        let bytes = encode_item(&item);
        // Manual cursor-based re-decode using the same helpers `read_item` uses, via a byte-slice
        // "file" is not wired up (real reads go through DEALLOCATE/FFI) - instead assert the fixed
        // fields land at the expected byte offsets, pinning the wire format directly.
        assert_eq!(bytes[0], item.default_available as u8);
        assert_eq!(bytes[1], item.visible as u8);
        assert_eq!(u16::from_le_bytes([bytes[2], bytes[3]]), item.id);
        assert_eq!(u32::from_le_bytes([bytes[4], bytes[5], bytes[6], bytes[7]]), item.item_type);
        assert_eq!(u32::from_le_bytes([bytes[8], bytes[9], bytes[10], bytes[11]]), item.sentinel);
        let name_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        assert_eq!(name_len, item.name.len());
        assert_eq!(&bytes[16..16 + name_len], item.name.as_bytes());
    }

    #[test]
    fn encode_mgr_orders_scripts_ascending_by_id() {
        let mut state = ShowScriptMgrState::default();
        state.scripts.insert(5, ShowScriptData { sentinel: 0xffff_ffff, script_type: 1, items: Vec::new() });
        state.scripts.insert(2, ShowScriptData { sentinel: 0xffff_ffff, script_type: 1, items: Vec::new() });
        state.next_id_counter = 5;
        let bytes = encode_mgr(&state);
        assert_eq!(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]), 2, "script count");
        // First script header starts at byte 4; its id (u16) must be the smaller one (2).
        assert_eq!(u16::from_le_bytes([bytes[4], bytes[5]]), 2);
    }

    fn raw_item(item_type: u32, trick_id: u16) -> ZTShowScriptItemRaw {
        ZTShowScriptItemRaw {
            _vtable: 0,
            default_available: 0,
            visible: 1,
            id: trick_id,
            item_type,
            sentinel: 0xffff_ffff,
            name: ZTBufferString::from_raw_parts(0, 0, 0),
            anim: ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_pre_trick: ZTBufferString::from_raw_parts(0, 0, 0),
            keeper_post_trick: ZTBufferString::from_raw_parts(0, 0, 0),
            building: 0,
            complexity: 1,
            return_to_keeper: 0,
            _pad: [0; 3],
            satisfaction: 1,
            satisfaction_delta: 1,
            satisfaction_mirror: 1,
            minimum_depth: 1,
            normal_help_id: 0,
            grayed_help_id: 0,
            normal_icon: ZTBufferString::from_raw_parts(0, 0, 0),
            grayed_icon: ZTBufferString::from_raw_parts(0, 0, 0),
        }
    }

    #[test]
    fn encode_item_truncates_overlong_strings_at_string_length_cap() {
        let mut item = ShowScriptItem::default();
        item.name = "x".repeat(STRING_LENGTH_CAP as usize + 50);
        let bytes = encode_item(&item);
        let name_len = u32::from_le_bytes([bytes[12], bytes[13], bytes[14], bytes[15]]) as usize;
        assert_eq!(name_len, (STRING_LENGTH_CAP - 1) as usize, "encoded length prefix should be truncated to STRING_LENGTH_CAP - 1");
        assert_eq!(&bytes[16..16 + name_len], "x".repeat(name_len).as_bytes());
    }
}
