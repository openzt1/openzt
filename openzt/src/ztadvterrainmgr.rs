//! `ZTAdvTerrainMgr` - thin-shell reimplementation. Vanilla's actual terrain-rendering behavior
//! (`setAuxImage`/`setGroundImage`/`renderShape`/`renderPass`/`setupRender`/`start2D`/`startD3D`/
//! `loadTextures`) is almost entirely D3D internals OpenZT has no reason to replace - porting that math
//! would just recompute vanilla's own pixels for no behavioral gain, at real risk (device state, texture
//! atlases, vertex geometry). This file instead ports only the orchestration/decision logic of `start()`,
//! `setImage()`, and `update()`, delegating every D3D-facing internal to real vanilla via `.original()`.
//!
//! Allocator strategy: `start()`/`setImage()` never allocate. `update()`'s pending-tile queue
//! (`std::list<BFPos>`, sentinel pointer at `+0x1d8`) is populated only by other, un-reimplemented
//! vanilla code - this file only ever reads and pops front nodes, so every node it touches was allocated
//! by vanilla's own small-object freelist. Per CLAUDE.md's cross-allocator rule, popped nodes are never
//! freed through Rust's `Box`/allocator - they're spliced back onto vanilla's own freelist head
//! (`DAT_00638008`, confirmed directly in `update()`'s own disassembly) via [`release_bfpos_node`].
//!
//! `stop()` and the destructor are deliberately **not** detoured: `stop()` is straight-line vanilla-owned
//! cleanup with no decision logic, and this module never allocates a byte of Rust-owned memory (state/
//! queue-sentinel are read/written in place on vanilla's own object) - nothing for a Rust `stop` or dtor
//! hook to free, mirroring `ztmegatilemgr.rs`'s own documented reasoning for skipping its destructor
//! detour.

use std::{ffi::c_void, fmt, fmt::Display};

use openzt_detour::generated::{
    bfterrainimage,
    bfuimgr::{HIDE_BUSY_CURSOR, SHOW_BUSY_CURSOR},
    ztadvterrainmgr::{LOAD_TEXTURES_0, SETUP_RENDER, SET_AUX_IMAGE, SET_GROUND_IMAGE, SET_IMAGE, START, START2D, START_D3D, UPDATE},
};
use openzt_detour_macro::detour_mod;
use tracing::{error, info};
use windows::Win32::System::SystemInformation::GetTickCount;

use crate::{
    command_console::CommandError,
    globals::{get_module_base, globals},
    lua_fn,
    util::{get_from_memory, save_to_memory, ZTBufferString},
    ztworldmgr::IVec3,
};

const BFTERRAINTYPEINFO_SIZE: usize = 0x30;

/// Offset of the `std::list<BFPos>` sentinel-pointer field `update()`'s queue-drain loop reads -
/// confirmed directly from `ZTAdvTerrainMgr_update.asm` (`LEA ESI,[EBX+0x1d8]`). Windows-only; not a
/// struct field (the ~0x400 bytes between `bf_terrain_type_info_buffer_end` and here are opaque D3D/
/// texture state this thin-shell scope never reads or writes).
const QUEUE_SENTINEL_PTR_OFFSET: u32 = 0x1d8;

/// Vanilla's shared small-object freelist head `update()` recycles popped `BfPosNode`s onto - confirmed
/// directly and unambiguously in `update()`'s own disassembly (`MOV EDX,[DAT_00638008]; MOV [EAX],EDX;
/// MOV DAT_00638008,EAX`). RVA = `0x00638008 - 0x400000`.
const RVA_BFPOS_NODE_FREELIST_HEAD: u32 = 0x0023_8008;

#[derive(Debug, Copy, Clone)]
#[repr(C)]
pub struct ZTAdvTerrainMgr_raw {
    vtable: u32,
    /// `0x4` - a single **byte**, not a dword - confirmed from `BFTerrainMgr_BFTerrainMgr.asm`'s
    /// `MOV byte ptr [ESI+0x4], AL` (zeroed at construction). Same shape as `ztmegatilemgr.rs`'s own
    /// `flag: u8 @ 0x4` (also zeroed by its ctor) - two independently reimplemented `BFMgr` subclasses
    /// putting a lone byte flag right after the vtable pointer is strong evidence this is a
    /// `BFMgr`-inherited field, not `ZTAdvTerrainMgr`-specific. No decompiled call site in the corpus
    /// reads it back; not behaviorally relevant to this thin-shell scope.
    flag: u8,
    _pad0: [u8; 3],
    /// `0x8` - a debug/dev-mode toggle, flipped by a debug hotkey in `ZTMapView::handleChar`'s `case 7`
    /// (a cheat-key dispatch branch, sibling to `addCash`/`increaseDonations` in the same handler),
    /// which mirrors the new value into the `DAT_00638064` global and a UI bool at
    /// `cls_0x6313e4.field_0x44c`. `BFTerrainMgr`'s constructor zeroes it while computing that same
    /// global from it, so field and mirror stay in sync from construction onward. What it visually
    /// toggles isn't confirmed (no string/comment names it, and the corpus doesn't include whatever
    /// reads `field_0x44c`). Not read/written anywhere in this thin-shell scope.
    debug_toggle: u32,
    /// `0xc` - written `2` by `start()`; read by `setImage()` (`state > 1`) and
    /// `update()`'s time-budget switch. Confirmed identical on both platforms. Independently corroborated
    /// as the terrain-quality options-menu setting by `_checkTerrainOptions.c`: menu item ids
    /// `0x636`-`0x639` map to `state` values `0`-`3` (selected from a UI dropdown at element `0x635`).
    state: u32,
    bf_terrain_type_info_array_start: u32, // TODO: Use ZTArray
    bf_terrain_type_info_array_end: u32,
    bf_terrain_type_info_buffer_end: u32,
    // Total size is 0x1dc. Only the front ~0x1c bytes plus one far-away field (update()'s queue sentinel
    // pointer at +0x1d8, accessed via raw offset arithmetic - see QUEUE_SENTINEL_PTR_OFFSET - not
    // modeled as a struct field) are load-bearing here.
}

/// One node of the vanilla `std::list<BFPos>` `update()` drains - `{next, prev, x, y}`, confirmed from
/// the unlink sequence in `ZTAdvTerrainMgr_update.asm`. Heap-allocated and recycled through the shared
/// small-object freelist at `DAT_00638008` - never allocated by this file, always vanilla-owned, read/
/// unlinked/recycled only, per CLAUDE.md's cross-allocator rule. The asm also reads a dead value at
/// `front_node+0x10` into a stack temp that's never used again, so only the first 0x10 bytes are modeled.
#[derive(Debug, Copy, Clone)]
#[repr(C)]
struct BfPosNode {
    next: u32,
    prev: u32,
    x: i32,
    y: i32,
}

const _: () = assert!(std::mem::size_of::<BfPosNode>() == 0x10);

struct ZTAdvTerrainMgr {
    bf_terrain_type_info_array: Vec<BFTerrainTypeInfo>,
}

impl From<ZTAdvTerrainMgr_raw> for ZTAdvTerrainMgr {
    fn from(raw: ZTAdvTerrainMgr_raw) -> Self {
        info!(
            "Reading terrain types from {:#x} to {:#x}",
            raw.bf_terrain_type_info_array_start, raw.bf_terrain_type_info_array_end
        );
        let mut bf_terrain_type_info_array = Vec::new();
        let mut current_bf_terrain_type_info_address = raw.bf_terrain_type_info_array_start;
        while current_bf_terrain_type_info_address < raw.bf_terrain_type_info_array_end {
            bf_terrain_type_info_array.push(read_bfterraintypeinfo_from_memory(current_bf_terrain_type_info_address));
            current_bf_terrain_type_info_address += BFTERRAINTYPEINFO_SIZE as u32;
        }
        ZTAdvTerrainMgr { bf_terrain_type_info_array }
    }
}

impl ZTAdvTerrainMgr_raw {
    fn base_addr(&self) -> u32 {
        self as *const _ as u32
    }

    pub fn state(&self) -> i32 {
        self.state as i32
    }

    pub fn set_state(&mut self, value: i32) {
        self.state = value as u32;
    }

    fn queue_sentinel_ptr(&self) -> u32 {
        get_from_memory(self.base_addr() + QUEUE_SENTINEL_PTR_OFFSET)
    }

    /// Unlinks the queue's front node from between the sentinel and its current next element, returning
    /// the front node's address. Only ever called on the front element (never mid-list), so the general
    /// prev/next unlink specializes to sentinel<->next relinking - confirmed equivalent to the asm's
    /// general-case unlink by inspection. Pure pointer surgery, no allocator involvement either side -
    /// safe against `sentinel`'s node being vanilla-owned regardless of confidence in any other field.
    fn unlink_front(sentinel: u32) -> u32 {
        let front = get_from_memory::<u32>(sentinel);
        let front_next = get_from_memory::<u32>(front);
        save_to_memory::<u32>(sentinel, front_next);
        save_to_memory::<u32>(front_next + 0x4, sentinel);
        front
    }

    /// Unlinks the queue's front node and recycles it via [`release_bfpos_node`] onto vanilla's own
    /// freelist - never through Rust's allocator, since the node was never allocated by Rust (see the
    /// module doc comment).
    fn pop_front_and_release(&mut self) {
        let sentinel = self.queue_sentinel_ptr();
        let front = Self::unlink_front(sentinel);
        unsafe { release_bfpos_node(front) };
    }

    /// Pure state-based time-budget/busy-cursor logic behind [`update`](Self::update), isolated for unit
    /// testing without touching real memory. Confirmed from `ZTAdvTerrainMgr_update.asm`'s jump table.
    /// Returns `(deadline, show_busy_cursor, early_return)` - callers must check `early_return` before
    /// using the other two.
    pub(crate) fn compute_update_state(state: i32, now: u32, delta_ticks: u32) -> (u32, bool, bool) {
        match state {
            4 => (now, false, true),
            0 => (u32::MAX, true, false),
            1 | 3 => (now.wrapping_add(delta_ticks >> 2), false, false),
            _ => (now.wrapping_add(delta_ticks), false, false),
        }
    }

    /// Straight-line 4-call bring-up pipeline with short-circuit-on-failure, per
    /// `ZTAdvTerrainMgr_start.c`: `start2D()` -> `startD3D()` -> `loadTextures()` -> `setupRender()`,
    /// `true` only if all four succeed. `this->state = 2` is a plain field write with no other side
    /// effects (the mac build calls `BFTerrainMgr::setPerfBias(this, 2)` for the same effect).
    pub fn start(&mut self) -> bool {
        self.set_state(2);
        let this = self.base_addr() as *const u32;
        unsafe {
            if START2D.original()(this) == 0 {
                return false;
            }
            if START_D3D.original()(this) == 0 {
                return false;
            }
            if LOAD_TEXTURES_0.original()(this) == 0 {
                return false;
            }
            SETUP_RENDER.original()(this) != 0
        }
    }

    /// 3-call dispatcher, per `ZTAdvTerrainMgr_setImage.c` (both platforms agree): `setAuxImage`, then
    /// `setGroundImage` (with `ground` set from `state > 1`), then `BFTerrainImage::computeImageSize`,
    /// returning `setGroundImage`'s result.
    pub fn set_image(&mut self, image: *const u32, map: *const u32, tile: *const u32) -> i8 {
        let this = self.base_addr() as *const u32;
        unsafe {
            SET_AUX_IMAGE.original()(this, image, map as *const i8, tile as *const c_void);
            let ground_flag = (self.state() > 1) as i32;
            let result = SET_GROUND_IMAGE.original()(this, image, map, tile, ground_flag);
            bfterrainimage::COMPUTE_IMAGE_SIZE.original()(image as *const c_void);
            result
        }
    }

    /// Time-budgeted drain of the pending-tile queue at `+0x1d8`, per `ZTAdvTerrainMgr_update.c`/`.asm`.
    /// Per iteration: bounds-checks `(x, y)` against the live `ZTWorldMgr`'s map dimensions (via
    /// `get_tile_from_pos`, which already returns `None` for out-of-range/negative coords) and, if in
    /// range, calls `setGroundImage(this, tile+0x50, world_mgr+0x8, tile, 0)` - confirmed directly from
    /// the asm's `CALL ZTAdvTerrainMgr::setGroundImage` argument setup - then pops and recycles the node.
    pub fn update(&mut self, delta_ticks: u32) {
        let now = unsafe { GetTickCount() };
        let (deadline, show_busy_cursor, early_return) = Self::compute_update_state(self.state(), now, delta_ticks);
        if early_return {
            return;
        }
        if show_busy_cursor {
            unsafe { SHOW_BUSY_CURSOR.original()(global_bfuimgr()) };
        }

        let world = globals().ztworldmgr();
        let world_addr = globals().ztworldmgr_ptr() as u32;
        let this = self.base_addr() as *const u32;
        let mut elapsed = now;

        loop {
            let sentinel = self.queue_sentinel_ptr();
            let front = get_from_memory::<u32>(sentinel);
            if front == sentinel || elapsed >= deadline {
                break;
            }

            let x = get_from_memory::<i32>(front + 0x8);
            let y = get_from_memory::<i32>(front + 0xc);
            if let Some(tile) = world.get_tile_from_pos(IVec3::new(x, y, 0)) {
                let tile_addr = world.get_ptr_from_bftile(&tile);
                unsafe {
                    SET_GROUND_IMAGE.original()(this, (tile_addr + 0x50) as *const u32, (world_addr + 0x8) as *const u32, tile_addr as *const u32, 0);
                }
            }
            self.pop_front_and_release();
            elapsed = unsafe { GetTickCount() };
        }

        if show_busy_cursor {
            unsafe { HIDE_BUSY_CURSOR.original()(global_bfuimgr()) };
        }
    }
}

/// Vanilla `BFUIMgr` singleton address, for `SHOW_BUSY_CURSOR`/`HIDE_BUSY_CURSOR` - same RVA
/// (`0x0023_8de0`) `ztthoughtmgr.rs`/`ztresearch.rs`/`ztshowui.rs`/`ztawardmgr.rs` already use.
fn global_bfuimgr() -> *const u32 {
    (get_module_base("zoo.exe") as u32 + 0x0023_8de0) as *const u32
}

/// Splices `node_addr` onto vanilla's own `BfPosNode` freelist head (`DAT_00638008`) - never through
/// Rust's allocator, since the node was allocated by vanilla's own small-object freelist (see the module
/// doc comment).
unsafe fn release_bfpos_node(node_addr: u32) {
    let freelist_head_addr = get_module_base("zoo.exe") as u32 + RVA_BFPOS_NODE_FREELIST_HEAD;
    let old_head = get_from_memory::<u32>(freelist_head_addr);
    save_to_memory::<u32>(node_addr, old_head);
    save_to_memory::<u32>(freelist_head_addr, node_addr);
}

impl Display for BFTerrainTypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BFTerrainTypeInfo {{ vtable: {:#x} type_id: {} cost: {} blend: {} water: {} ?: {:#x} ?: {} ?: {} help_id: {} icon_string: {} }}",
            self.vtable, self.type_id, self.cost, self.blend, self.water, self.unknown_ptr, self.unknown_u32_6, self.unknown_u32_7, self.help_id, self.icon_string,
        )
    }
}

#[derive(Debug)]
#[repr(C)]
struct BFTerrainTypeInfo {
    vtable: u32,
    type_id: u32,
    cost: f32,
    blend: u32,
    water: u32,
    unknown_ptr: u32,
    unknown_u32_6: u32,
    unknown_u32_7: u32,
    help_id: u32,
    icon_string: ZTBufferString,
}


fn read_ztadvterrainmgr_from_memory() -> ZTAdvTerrainMgr {
    ZTAdvTerrainMgr::from(*globals().ztadvterrainmgr())
}

fn read_bfterraintypeinfo_from_memory(address: u32) -> BFTerrainTypeInfo {
    get_from_memory(address)
}

fn command_get_bfterraintypeinfo(_args: Vec<&str>) -> Result<String, CommandError> {
    let ztadvterrainmgr = read_ztadvterrainmgr_from_memory();
    info!("Found {} BFTerrainTypeInfo", ztadvterrainmgr.bf_terrain_type_info_array.len());
    let mut string_array = Vec::new();
    for bfterraintypeinfo in ztadvterrainmgr.bf_terrain_type_info_array {
        string_array.push(bfterraintypeinfo.to_string());
    }
    Ok(string_array.join("\n"))
}

#[detour_mod]
mod ztadvterrainmgr_detours {
    use super::*;
    use crate::util::mut_from_memory;

    #[detour(START)]
    unsafe extern "thiscall" fn start(this: *const u32) -> u32 {
        unsafe { mut_from_memory::<ZTAdvTerrainMgr_raw>(this) }.start() as u32
    }

    #[detour(SET_IMAGE)]
    unsafe extern "thiscall" fn set_image(this: *const u32, image: *const u32, map: *const u32, tile: *const u32) -> u32 {
        unsafe { mut_from_memory::<ZTAdvTerrainMgr_raw>(this) }.set_image(image, map, tile) as u32
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, delta_ticks: u32) {
        unsafe { mut_from_memory::<ZTAdvTerrainMgr_raw>(this) }.update(delta_ticks);
    }
}

/// Registers this module's live detours. Deliberately does **not** detour `stop()` or the destructor -
/// see the module doc comment.
pub fn init() {
    lua_fn!("list_bfterraintypeinfo", "Lists terrain type info", "list_bfterraintypeinfo()", || {
        match command_get_bfterraintypeinfo(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    if let Err(e) = unsafe { ztadvterrainmgr_detours::init_detours() } {
        error!("Failed to initialise ztadvterrainmgr detours: {e:?}");
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn update_state_state0_unlimited_budget_shows_cursor() {
        let (deadline, show_cursor, early_return) = ZTAdvTerrainMgr_raw::compute_update_state(0, 1000, 50);
        assert_eq!(deadline, u32::MAX);
        assert!(show_cursor);
        assert!(!early_return);
    }

    #[test]
    fn update_state_state1_quarter_budget() {
        let (deadline, show_cursor, early_return) = ZTAdvTerrainMgr_raw::compute_update_state(1, 1000, 400);
        assert_eq!(deadline, 1100);
        assert!(!show_cursor);
        assert!(!early_return);
    }

    #[test]
    fn update_state_state3_quarter_budget() {
        let (deadline, show_cursor, early_return) = ZTAdvTerrainMgr_raw::compute_update_state(3, 2000, 40);
        assert_eq!(deadline, 2010);
        assert!(!show_cursor);
        assert!(!early_return);
    }

    #[test]
    fn update_state_state4_is_noop() {
        let (_, show_cursor, early_return) = ZTAdvTerrainMgr_raw::compute_update_state(4, 1000, 500);
        assert!(early_return);
        assert!(!show_cursor);
    }

    #[test]
    fn update_state_state2_full_budget() {
        let (deadline, show_cursor, early_return) = ZTAdvTerrainMgr_raw::compute_update_state(2, 1000, 500);
        assert_eq!(deadline, 1500);
        assert!(!show_cursor);
        assert!(!early_return);
    }

    #[test]
    fn update_state_negative_falls_to_full_budget() {
        let (deadline, show_cursor, early_return) = ZTAdvTerrainMgr_raw::compute_update_state(-1, 1000, 500);
        assert_eq!(deadline, 1500);
        assert!(!show_cursor);
        assert!(!early_return);
    }

    #[test]
    fn update_state_five_and_above_falls_to_full_budget() {
        let (deadline, show_cursor, early_return) = ZTAdvTerrainMgr_raw::compute_update_state(5, 1000, 500);
        assert_eq!(deadline, 1500);
        assert!(!show_cursor);
        assert!(!early_return);
    }

    /// Hand-built 3-node circular list (sentinel -> a -> b -> sentinel) exercising
    /// `ZTAdvTerrainMgr_raw::unlink_front`'s pointer arithmetic in isolation - no live/vanilla memory or
    /// `get_module_base` involved, so this is safe to run without a loaded game (unlike
    /// `release_bfpos_node`, which is vanilla-freelist-dependent and only exercised live).
    #[test]
    fn unlink_front_pops_first_node_and_relinks_sentinel() {
        let sentinel: &'static mut [u32; 2] = Box::leak(Box::new([0u32; 2]));
        let node_a: &'static mut BfPosNode = Box::leak(Box::new(BfPosNode { next: 0, prev: 0, x: 3, y: 4 }));
        let node_b: &'static mut BfPosNode = Box::leak(Box::new(BfPosNode { next: 0, prev: 0, x: 7, y: 8 }));

        let sentinel_addr = sentinel.as_mut_ptr() as u32;
        let a_addr = node_a as *mut BfPosNode as u32;
        let b_addr = node_b as *mut BfPosNode as u32;

        // sentinel.next = a; a.prev = sentinel; a.next = b; b.prev = a; b.next = sentinel
        sentinel[0] = a_addr;
        node_a.prev = sentinel_addr;
        node_a.next = b_addr;
        node_b.prev = a_addr;
        node_b.next = sentinel_addr;

        let popped = ZTAdvTerrainMgr_raw::unlink_front(sentinel_addr);

        assert_eq!(popped, a_addr);
        assert_eq!(sentinel[0], b_addr, "sentinel.next should now point at b");
        assert_eq!(node_b.prev, sentinel_addr, "b.prev should now point at sentinel");
    }
}
