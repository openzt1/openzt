//! Manager lifecycle and bookkeeping: `update`, `save`, `load`, `create`, construction and destruction, `checkPath`, `callDone`, `removeEntities`, `getPath`.

use super::*;

pub(super) fn read_global_pointer(rva: u32) -> u32 {
    get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + rva)
}

/// `BFAIMgr::update(dt)`: advances the tick accumulator, flushes the path cache on its interval, removes
/// flagged entities, ticks every entity and pumps the event manager. A zero divisor is a divide fault in
/// vanilla; here it skips the tick instead.
pub unsafe fn bfaimgr_update(this: u32, dt: u32) {
    let divisor = get_from_memory::<u32>(this + OFFSET_AI_TICK_DIVISOR);
    if divisor == 0 {
        return;
    }
    let base = get_module_base("zoo.exe") as u32;
    let total = dt.wrapping_add(get_from_memory::<u32>(this + OFFSET_AI_TICK_REMAINDER));
    save_to_memory(this + OFFSET_AI_TICK_REMAINDER, total % divisor);
    let ticks = get_from_memory::<u32>(this + OFFSET_AI_TICK_COUNT).wrapping_add(total / divisor);
    save_to_memory(this + OFFSET_AI_TICK_COUNT, ticks);
    if ticks >= get_from_memory::<u32>(base + RVA_PATH_CACHE_NEXT_FLUSH) {
        crate::vanilla_rbtree::rb_tree_clear(this + OFFSET_PATH_CACHE, PATH_CACHE_NODE_SIZE);
        let interval = get_from_memory::<u32>(base + RVA_PATH_CACHE_FLUSH_INTERVAL);
        save_to_memory(base + RVA_PATH_CACHE_NEXT_FLUSH, get_from_memory::<u32>(this + OFFSET_AI_TICK_COUNT).wrapping_add(interval));
    }
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    unsafe { call_entity_virtual(this, AI_VTABLE_REMOVE_ENTITIES) };
    let count = (get_from_memory::<u32>(world + WORLD_ENTITY_END).wrapping_sub(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN)) as i32) >> 2;
    for index in 0..count.max(0) as u32 {
        let entity = get_from_memory::<u32>(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN) + index * 4);
        if entity != 0 {
            unsafe { call_virtual_args(entity, ENTITY_VTABLE_UPDATE, [dt]) };
        }
    }
    unsafe { BFEVENTMGR_UPDATE.original()((this + OFFSET_EVENT_MGR) as *const u32) };
}

/// `ZTAIMgr::update(dt)`: pumps the maintenance task pool and the show manager, then the base update.
pub unsafe fn ztaimgr_update(this: u32, dt: u32) {
    let base = get_module_base("zoo.exe") as u32;
    unsafe { ZTMAINTTASKPOOL_LISTEN.original()((base + RVA_MAINT_TASK_POOL) as *const i32) };
    let show_mgr = read_global_pointer(RVA_GLOBAL_SHOW_MGR);
    unsafe { call_entity_virtual(show_mgr, SHOW_MGR_VTABLE_UPDATE) };
    unsafe { bfaimgr_update(this, dt) };
}

/// `BFAIMgr::save(file)`: writes the tick remainder and counter, then the event manager. Every write is
/// attempted; the result is true only if all succeed.
pub unsafe fn bfaimgr_save(this: u32, file: *const i8) -> bool {
    let write = |offset: u32| unsafe { WRITE_BYTES_TO_FILE.hooked()((this + offset) as *const u32, 4, 1, file) == 1 };
    let first = write(OFFSET_AI_TICK_REMAINDER);
    let second = write(OFFSET_AI_TICK_COUNT);
    let events = unsafe { BFEVENTMGR_SAVE.hooked()((this + OFFSET_EVENT_MGR) as *const u32, file) };
    events && first && second
}

/// `BFWorldMgr` habitat-pointer vector begin/end, and the `BFWorldMgr` vtable slot run first when loading.
pub(super) const WORLD_HABITAT_BEGIN: u32 = 0x98;
pub(super) const WORLD_HABITAT_END: u32 = 0x9c;
pub(super) const WORLD_VTABLE_PRE_LOAD: u32 = 0x4;
/// Habitat vtable slots: a check returning a success byte, then a no-argument finish step.
pub(super) const HABITAT_VTABLE_CHECK: u32 = 0x10;
pub(super) const HABITAT_VTABLE_FINISH: u32 = 0x58;
/// Entity vtable slots run for each entity on load: `(0, 0)` reset, no-argument refresh, `(1, 0, 1)` unit fixup.
pub(super) const ENTITY_VTABLE_RESET: u32 = 0x110;
pub(super) const ENTITY_VTABLE_LOAD_REFRESH: u32 = 0x10c;
pub(super) const ENTITY_VTABLE_UNIT_FIXUP: u32 = 0x198;
/// Entity fields touched on a full load: a byte vector's begin/end (`+0xdc`/`+0xe0`) and a flag byte (`+0xe8`).
pub(super) const OFFSET_ENTITY_VECTOR_BEGIN: u32 = 0xdc;
pub(super) const OFFSET_ENTITY_VECTOR_END: u32 = 0xe0;
pub(super) const OFFSET_ENTITY_FLAG: u32 = 0xe8;

/// `BFAIMgr::loadData(full)`: post-load pass over the world. When `full`, runs the world managers pre-load
/// step first. Every habitat is checked (result ANDed over the low bytes) and finished, then every entity is
/// reset and, on a full load, has its byte vector truncated, its flag set, is refreshed, and gets the unit
/// fixup if its type is a `ZTUnitType`. The `this` pointer is unused.
pub unsafe fn bfaimgr_load_data(full: bool) -> bool {
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    let mut ok = true;
    if full {
        unsafe { call_virtual_args(world, WORLD_VTABLE_PRE_LOAD, [0]) };
    }
    let mut cursor = get_from_memory::<u32>(world + WORLD_HABITAT_BEGIN);
    while cursor != get_from_memory::<u32>(world + WORLD_HABITAT_END) {
        let habitat = get_from_memory::<u32>(cursor);
        if habitat != 0 {
            ok &= unsafe { call_virtual_args(habitat, HABITAT_VTABLE_CHECK, []) } & 0xff != 0;
        }
        cursor += 4;
    }
    cursor = get_from_memory::<u32>(world + WORLD_HABITAT_BEGIN);
    while cursor != get_from_memory::<u32>(world + WORLD_HABITAT_END) {
        unsafe { call_virtual_args(get_from_memory::<u32>(cursor), HABITAT_VTABLE_FINISH, []) };
        cursor += 4;
    }
    cursor = get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN);
    while cursor != get_from_memory::<u32>(world + WORLD_ENTITY_END) {
        let entity = get_from_memory::<u32>(cursor);
        if entity != 0 {
            unsafe { call_virtual_args(entity, ENTITY_VTABLE_RESET, [0, 0]) };
            if full {
                let begin = get_from_memory::<u32>(entity + OFFSET_ENTITY_VECTOR_BEGIN);
                if begin != get_from_memory::<u32>(entity + OFFSET_ENTITY_VECTOR_END) {
                    save_to_memory(begin, 0u8);
                    save_to_memory(entity + OFFSET_ENTITY_VECTOR_END, begin);
                }
                save_to_memory(entity + OFFSET_ENTITY_FLAG, 1u8);
                unsafe { call_virtual_args(entity, ENTITY_VTABLE_LOAD_REFRESH, []) };
                let entity_type = get_from_memory::<u32>(entity + OFFSET_ENTITY_TYPE);
                let tag = get_module_base("zoo.exe") as u32 + RVA_TAG_ZTUNITTYPE;
                if unsafe { call_virtual_args(entity_type, TYPE_VTABLE_IS_CAST_CLASS, [tag]) } & 0xff != 0 {
                    unsafe { call_virtual_args(entity, ENTITY_VTABLE_UNIT_FIXUP, [1, 0, 1]) };
                }
            }
        }
        cursor += 4;
    }
    ok
}

/// Allocation size of a `ZTAIMgr`, its vtable and `BFEventListenerInterface` table (RVAs), and the vtable slot
/// of `initAIParams`.
pub(super) const ZTAIMGR_SIZE: u32 = 0x150;
pub(super) const RVA_ZTAIMGR_VTABLE: u32 = 0x0023_5324;
pub(super) const RVA_ZTAIMGR_LISTENER_TABLE: u32 = 0x0023_2288;
pub(super) const AI_VTABLE_INIT_AI_PARAMS: u32 = 0x38;

/// `ZTAIMgr::create()`: allocates the `0x150`-byte manager through vanilla `operator new` (vanilla frees it
/// with the matching delete), runs the `BFAIMgr` constructor, installs the `ZTAIMgr` vtable and listener
/// table, and runs `initAIParams` through the vtable. Returns the manager, or `0` if allocation failed.
pub unsafe fn ztaimgr_create() -> u32 {
    let this = unsafe { OPERATOR_NEW.original()(ZTAIMGR_SIZE) } as u32;
    if this == 0 {
        return 0;
    }
    unsafe { BFAIMGR_CONSTRUCTOR.hooked()(this as *const u32) };
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(this, base + RVA_ZTAIMGR_VTABLE);
    save_to_memory(this + 8, base + RVA_ZTAIMGR_LISTENER_TABLE);
    unsafe { call_virtual_args(this, AI_VTABLE_INIT_AI_PARAMS, []) };
    this
}

/// `BFAIMgr`'s vtable and listener table (RVAs), and the `BFMgr` vtable its destructor restores.
pub(super) const RVA_BFAIMGR_VTABLE: u32 = 0x0023_536c;
pub(super) const RVA_BFAIMGR_LISTENER_TABLE: u32 = 0x0023_2270;
pub(super) const RVA_BFMGR_VTABLE: u32 = 0x0023_55c0;
/// Manager fields the constructor and destructor touch: the `GXLLEAnimSet` pointer, the embedded `BFEventMgr`
/// (`+0xa0`; second event vector `+0xac`, listener tree `+0xc0`), and the path cache (`+0x12c`: head, size at
/// `+0x130`, one flag byte at `+0x134`).
pub(super) const OFFSET_ANIM_SET: u32 = 0xd0;
pub(super) const OFFSET_EVENT_VECTOR_B: u32 = 0xac;
pub(super) const OFFSET_LISTENER_TREE: u32 = 0xc0;
pub(super) const OFFSET_PATH_CACHE_SIZE: u32 = 0x130;
pub(super) const OFFSET_PATH_CACHE_FLAG: u32 = 0x134;
/// Pool freelist bucket for sizes `9..=0x18` (`DAT_00638008`): tree heads of `0x14`/`0x18` bytes.
pub(super) const RVA_POOL_FREELIST_BUCKET_2: u32 = 0x0023_8008;
/// `GXLLEAnimSet` allocation size, and its animation elements (`+0x10` begin, `+0x14` end, `+0x18` capacity end).
pub(super) const ANIM_SET_SIZE: u32 = 0x28;
pub(super) const ANIM_ELEMENT_SIZE: u32 = 0x1c;
pub(super) const PATH_CACHE_HEAD_SIZE: u32 = 0x18;

/// `BFAIMgr::BFAIMgr()` (`0x00525e90`): constructs the embedded `BFPathFinder` and `BFEventMgr`, builds the empty
/// path-cache tree (head from `PoolAlloc`, size `0`), installs the `BFAIMgr` vtable and listener table (the
/// `BFMgr` ones vanilla writes first are overwritten before anything reads them), allocates a zeroed
/// `GXLLEAnimSet` (`0x28` bytes, `operator new`) into `+0xd0`, and runs `BFAIMgr::init`. A failed allocation
/// stores a null set. `+0x134` is a stale stack byte in vanilla; it is zeroed here.
pub unsafe fn bfaimgr_construct(this: u32) -> u32 {
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(this + 0x4, 0u8);
    unsafe {
        BFPATHFINDER_CONSTRUCTOR.original()((this + OFFSET_PATHFINDER) as *const u32, 0, 2);
        BFEVENTMGR_CONSTRUCTOR.original()((this + OFFSET_EVENT_MGR) as *const u32);
    }
    let head = unsafe { POOLALLOC_ALLOCATE.original()(PATH_CACHE_HEAD_SIZE) } as u32;
    save_to_memory(this + OFFSET_PATH_CACHE, head);
    save_to_memory(this + OFFSET_PATH_CACHE_SIZE, 0u32);
    save_to_memory(this + OFFSET_PATH_CACHE_FLAG, 0u8);
    save_to_memory(head, 0u8);
    save_to_memory(head + 4, 0u32);
    save_to_memory(head + 8, head);
    save_to_memory(head + 0xc, head);
    save_to_memory(this, base + RVA_BFAIMGR_VTABLE);
    save_to_memory(this + 8, base + RVA_BFAIMGR_LISTENER_TABLE);
    let anim_set = unsafe { OPERATOR_NEW.original()(ANIM_SET_SIZE) } as u32;
    if anim_set != 0 {
        for offset in (0x10..=0x20).step_by(4) {
            save_to_memory(anim_set + offset, 0u32);
        }
        save_to_memory(anim_set + 0x24, 0u8);
        save_to_memory(anim_set + 0x25, 0u8);
    }
    save_to_memory(this + OFFSET_ANIM_SET, anim_set);
    unsafe { bfaimgr_init(this) };
    this
}

/// Returns a tree head of the `0x14`/`0x18` size class to vanilla's pool freelist (null is skipped).
pub(super) fn pool_free_bucket_2(node: u32) {
    if node == 0 {
        return;
    }
    let freelist = get_module_base("zoo.exe") as u32 + RVA_POOL_FREELIST_BUCKET_2;
    save_to_memory(node, get_from_memory::<u32>(freelist));
    save_to_memory(freelist, node);
}

/// `BFAIMgr::~BFAIMgr()` (`0x0050374b`): drops back to the `BFAIMgr` vtable, releases and frees the
/// `GXLLEAnimSet` (`release`, the element destructor loop over what `release` left, the element buffer through
/// `PoolAlloc::deallocate_n_28`, then `operator delete`), clears the path cache and returns its head to the
/// freelist, runs the inlined `BFEventMgr` teardown (listener tree cleared and its head freed, both event
/// vectors), destroys the `BFPathFinder`, and ends on the `BFMgr` vtable.
pub unsafe fn bfaimgr_destruct(this: u32) -> u32 {
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(this, base + RVA_BFAIMGR_VTABLE);
    save_to_memory(this + 8, base + RVA_BFAIMGR_LISTENER_TABLE);
    let anim_set = get_from_memory::<u32>(this + OFFSET_ANIM_SET);
    if anim_set != 0 {
        unsafe { GXLLEANIMSET_RELEASE.original()(anim_set as *const c_void) };
        let begin = get_from_memory::<u32>(anim_set + 0x10);
        let end = get_from_memory::<u32>(anim_set + 0x14);
        let mut element = begin;
        while element != end {
            unsafe { GXLLEANIM_DESTRUCTOR.original()(element as *const u32) };
            element += ANIM_ELEMENT_SIZE;
        }
        let capacity = (get_from_memory::<u32>(anim_set + 0x18).wrapping_sub(begin) as i32) / ANIM_ELEMENT_SIZE as i32;
        unsafe { DEALLOCATE_N_28.original()(begin as *const u32, capacity) };
        unsafe { OPERATOR_DELETE.original()(anim_set) };
    }
    crate::vanilla_rbtree::rb_tree_clear(this + OFFSET_PATH_CACHE, PATH_CACHE_NODE_SIZE);
    pool_free_bucket_2(get_from_memory::<u32>(this + OFFSET_PATH_CACHE));
    unsafe { TREE20_CLEAR.original()((this + OFFSET_LISTENER_TREE) as *const u32) };
    pool_free_bucket_2(get_from_memory::<u32>(this + OFFSET_LISTENER_TREE));
    unsafe {
        VECTOR_T_28.original()((this + OFFSET_EVENT_VECTOR_B) as *const u32);
        VECTOR_T_28.original()((this + OFFSET_EVENT_MGR) as *const u32);
        BFPATHFINDER_DESTRUCTOR.original()((this + OFFSET_PATHFINDER) as *const u32);
    }
    save_to_memory(this, base + RVA_BFMGR_VTABLE);
    this
}

/// `BFAIMgr`'s deleting destructor (`0x00503857`, vtable slot 0): runs the destructor, then frees the object
/// with `operator delete` when bit 0 of `flags` is set. Returns `this`.
pub unsafe fn bfaimgr_deleting_destruct(this: u32, flags: u8) -> u32 {
    unsafe { bfaimgr_destruct(this) };
    if flags & 1 != 0 {
        unsafe { OPERATOR_DELETE.original()(this) };
    }
    this
}

/// `BFAIMgr`'s embedded `BFPathFinder` (`+0x10`): the map pointer, map row count and tile count it is pointed at
/// before a search, and its search-node limit (`+0x4c` of the manager is the limit `checkPath` restores).
pub(super) const OFFSET_PATHFINDER: u32 = 0x10;
pub(super) const PATHFINDER_MAP: u32 = 0x8;
pub(super) const PATHFINDER_MAP_ROWS: u32 = 0xc;
pub(super) const PATHFINDER_TILE_COUNT: u32 = 0x10;
pub(super) const OFFSET_PATH_NODE_LIMIT: u32 = 0x4c;
/// The pathfinder's result list head pointer (`+0x70` of the pathfinder, `+0x80` of the manager).
pub(super) const OFFSET_PATH_RESULT: u32 = 0x80;
/// Node limit `checkPath` searches with.
pub(super) const CHECK_PATH_NODE_LIMIT: u32 = 100;
/// `BFWorldMgr` map block (`+0x8`) and its column and row counts (`+0x2c`/`+0x30` into it).
pub(super) const WORLD_MAP: u32 = 0x8;
pub(super) const MAP_COLUMNS: u32 = 0x2c;
pub(super) const MAP_ROWS: u32 = 0x30;
/// Offset of the position inside a `BFTile`, and the `BFPathFinder` vtable slot `findPath`.
pub(super) const TILE_POSITION: u32 = 0x34;
pub(super) const PATHFINDER_VTABLE_FIND_PATH: u32 = 0x0;

/// `BFAIMgr::checkPath(from, to, unit)`: whether a path exists. Points the pathfinder at the world map, runs a
/// 100-node `findPath` (virtual) between the two tile positions, restores the node limit and reports whether the
/// pathfinder's result list is non-empty. Vanilla copies the result list into a temporary pool list before
/// counting it, then frees the copy; the copy has no other effect, so the port tests the source list directly.
/// The result is the low byte only (`AL`), like the vanilla function.
pub unsafe fn bfaimgr_check_path(this: u32, from: u32, to: u32, unit: u32) -> u32 {
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    let finder = this + OFFSET_PATHFINDER;
    let map = world.wrapping_add(WORLD_MAP);
    if map != 0 {
        save_to_memory(finder + PATHFINDER_MAP, map);
        let rows = get_from_memory::<u32>(map + MAP_ROWS);
        save_to_memory(finder + PATHFINDER_MAP_ROWS, rows);
        let columns = get_from_memory::<u32>(map + MAP_COLUMNS);
        save_to_memory(finder + PATHFINDER_TILE_COUNT, columns.wrapping_mul(rows));
    }
    let saved_limit = get_from_memory::<u32>(this + OFFSET_PATH_NODE_LIMIT);
    unsafe { SET_NODE_MAX.hooked()(finder as *const u32, CHECK_PATH_NODE_LIMIT) };
    unsafe { call_virtual_args(finder, PATHFINDER_VTABLE_FIND_PATH, [from + TILE_POSITION, to + TILE_POSITION, unit, 0]) };
    unsafe { SET_NODE_MAX.hooked()(finder as *const u32, saved_limit) };
    let list = get_from_memory::<u32>(this + OFFSET_PATH_RESULT);
    u32::from(get_from_memory::<u32>(list) != list)
}

/// `BFEntity` fields `callDone` reads: the flag byte at `+0xf8`, the embedded `GXMixer` at `+0x74`, and inside the
/// mixer the flag byte at `+0x60` and the two dwords at `+0x28`/`+0x18` that `callDone` compares.
pub(super) const OFFSET_ENTITY_CALL_DONE_FLAG: u32 = 0xf8;
pub(super) const OFFSET_ENTITY_MIXER: u32 = 0x74;
pub(super) const MIXER_FLAG: u32 = 0x60;
pub(super) const MIXER_CURSOR: u32 = 0x28;
pub(super) const MIXER_END: u32 = 0x18;

/// `BFAIMgr::callDone(entity)`: true when the entity is flagged and its animation mixer has run out of
/// animations (the cursor equals its end, or `GXMixer::getAnim` finds none).
pub unsafe fn bfaimgr_call_done(entity: u32) -> bool {
    if entity == 0 || get_from_memory::<u8>(entity + OFFSET_ENTITY_CALL_DONE_FLAG) == 0 {
        return false;
    }
    let mixer = entity + OFFSET_ENTITY_MIXER;
    if get_from_memory::<u8>(mixer + MIXER_FLAG) == 0 {
        return false;
    }
    if get_from_memory::<u32>(mixer + MIXER_CURSOR) == get_from_memory::<u32>(mixer + MIXER_END) {
        return true;
    }
    (unsafe { crate::gxmixer::get_anim(mixer) }) == 0
}

/// Entity vtable slots `removeEntities` tests and clears: `isRemoved`, `isRemovedUndo` and `setIsRemoved(a, b)`.
pub(super) const ENTITY_VTABLE_IS_REMOVED: u32 = 0xac;
pub(super) const ENTITY_VTABLE_IS_REMOVED_UNDO: u32 = 0xb0;
pub(super) const ENTITY_VTABLE_SET_IS_REMOVED: u32 = 0xa8;
/// `BFAIMgr` vtable slot `removeEntity(entity)`.
pub(super) const AI_VTABLE_REMOVE_ENTITY: u32 = 0x44;

/// `BFAIMgr::removeEntities()`: sweeps the world's entity list. An entity flagged removed is handed to the
/// manager's `removeEntity` (which takes it out of the list, so the index and count step back), unless it is
/// flagged undo-removed, in which case the world removes it without destroying it and the flag is cleared.
/// The count is taken once; the list pointer is re-read for every entity.
pub unsafe fn bfaimgr_remove_entities(this: u32) {
    let world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
    let mut count = (get_from_memory::<u32>(world + WORLD_ENTITY_END).wrapping_sub(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN)) as i32) >> 2;
    let mut index: i32 = 0;
    if count <= 0 {
        return;
    }
    loop {
        let entity = get_from_memory::<u32>(get_from_memory::<u32>(world + WORLD_ENTITY_BEGIN).wrapping_add((index as u32).wrapping_mul(4)));
        if entity != 0 && unsafe { call_virtual_args(entity, ENTITY_VTABLE_IS_REMOVED, []) } & 0xff != 0 {
            if unsafe { call_virtual_args(entity, ENTITY_VTABLE_IS_REMOVED_UNDO, []) } & 0xff == 0 {
                unsafe { call_virtual_args(this, AI_VTABLE_REMOVE_ENTITY, [entity]) };
                index -= 1;
                count -= 1;
            } else {
                let current_world = read_global_pointer(RVA_GLOBAL_WORLD_MGR);
                unsafe { call_virtual_args(current_world, WORLD_MGR_VTABLE_REMOVE_ENTITY, [entity, 0]) };
                unsafe { call_virtual_args(entity, ENTITY_VTABLE_SET_IS_REMOVED, [0, 0]) };
            }
        }
        index += 1;
        if index >= count {
            break;
        }
    }
}

/// `BFAIMgr` vtable slot of the tile-pointer `getPath` overload's target, the position-based `getPath`.
pub(super) const AI_VTABLE_GET_PATH: u32 = 0x18;

/// `BFAIMgr::getPath(from, to, unit, flag)` for tiles: forwards to the position-based `getPath` through the
/// manager's vtable with both tiles replaced by their positions.
pub unsafe fn bfaimgr_get_path_tiles(this: u32, from: u32, to: u32, unit: u32, flag: bool) -> bool {
    low_byte_bool(unsafe { call_virtual_args(this, AI_VTABLE_GET_PATH, [from + TILE_POSITION, to + TILE_POSITION, unit, u32::from(flag)]) })
}

/// `ZTUnit` field `fPlaySetTank` compares against `4` to choose which parameter to play.
pub(super) const UNIT_TANK_SELECTOR_OFFSET: u32 = 0x258;
/// Byte size of one `BFFunctionCall` parameter.
pub(super) const CALL_PARAM_SIZE: u32 = 0xc;
