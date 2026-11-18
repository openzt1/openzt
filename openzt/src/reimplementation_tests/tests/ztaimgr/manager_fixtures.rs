//! Fixtures for the manager lifecycle tests: a fake world, show manager and AI manager.

use super::*;

// ---------------------------------------------------------------------------------------------------------------
// Manager lifecycle: `BFAIMgr::update`/`save`/`loadData`, `ZTAIMgr::update`/`create`.
//
// The manager methods read `GLOBAL_ZTWorldMgr`/`GLOBAL_ZTShowMgr` straight from their pointer cells, so the
// tests point those cells at Rust-owned fake managers for the duration of one call (restored afterwards) and
// run the real vanilla body and the port over the same fixtures. Every virtual the bodies call is a fake that
// appends `[slot, id-of-this, args..]` to a shared log, so the comparison sees which virtual ran, in what order,
// on which object, with which arguments.
// ---------------------------------------------------------------------------------------------------------------

pub(super) const RVA_GLOBAL_WORLD_MGR: u32 = 0x0023_8040;
pub(super) const RVA_GLOBAL_SHOW_MGR: u32 = 0x0023_9008;
pub(super) const RVA_PATH_CACHE_NEXT_FLUSH: u32 = 0x0023_94b4;
pub(super) const RVA_PATH_CACHE_FLUSH_INTERVAL: u32 = 0x0023_9028;
pub(super) const PATH_CACHE_NODE_SIZE: u32 = 0x18;

pub(super) const ID_WORLD: u32 = 0x100;
pub(super) const ID_HABITAT: u32 = 0x200;
pub(super) const ID_ENTITY: u32 = 0x300;
pub(super) const ID_MGR: u32 = 0x400;
pub(super) const ID_SHOW: u32 = 0x500;

pub(super) static CALL_LOG: Mutex<Vec<[u32; 5]>> = Mutex::new(Vec::new());
/// Address of the fake world whose entity vector `fake_mgr_remove_entities` shrinks by one, or `0`.
pub(super) static SHRINK_WORLD: AtomicU32 = AtomicU32::new(0);

pub(super) fn log_call(slot: u32, this: u32, a: u32, b: u32, c: u32) {
    let id = get_from_memory::<u32>(this + 4);
    CALL_LOG.lock().unwrap().push([slot, id, a, b, c]);
}

pub(super) fn take_log() -> Vec<[u32; 5]> {
    std::mem::take(&mut *CALL_LOG.lock().unwrap())
}

pub(super) extern "thiscall" fn fake_world_pre_load(this: u32, a: u32) {
    log_call(0x04, this, a, 0, 0);
}
pub(super) extern "thiscall" fn fake_habitat_check(this: u32) -> u32 {
    log_call(0x10, this, 0, 0, 0);
    get_from_memory::<u32>(this + 8)
}
pub(super) extern "thiscall" fn fake_habitat_finish(this: u32) {
    log_call(0x58, this, 0, 0, 0);
}
pub(super) extern "thiscall" fn fake_entity_update(this: u32, dt: u32) {
    log_call(0x28, this, dt, 0, 0);
}
pub(super) extern "thiscall" fn fake_entity_reset(this: u32, a: u32, b: u32) {
    log_call(0x110, this, a, b, 0);
}
pub(super) extern "thiscall" fn fake_entity_load_refresh(this: u32) {
    log_call(0x10c, this, 0, 0, 0);
}
pub(super) extern "thiscall" fn fake_entity_fixup(this: u32, a: u32, b: u32, c: u32) {
    log_call(0x198, this, a, b, c);
}
pub(super) extern "thiscall" fn fake_mgr_remove_entities(this: u32) {
    log_call(0x40, this, 0, 0, 0);
    let world = SHRINK_WORLD.load(Ordering::Relaxed);
    if world != 0 {
        save_to_memory(world + 0x84, get_from_memory::<u32>(world + 0x84) - 4);
    }
}
pub(super) extern "thiscall" fn fake_entity_is_removed(this: u32) -> u32 {
    log_call(0xac, this, 0, 0, 0);
    get_from_memory::<u32>(this + 8)
}
pub(super) extern "thiscall" fn fake_entity_is_removed_undo(this: u32) -> u32 {
    log_call(0xb0, this, 0, 0, 0);
    get_from_memory::<u32>(this + 12)
}
pub(super) extern "thiscall" fn fake_entity_set_is_removed(this: u32, a: u32, b: u32) {
    log_call(0xa8, this, a, b, 0);
}
pub(super) extern "thiscall" fn fake_world_remove_entity(this: u32, entity: u32, flag: u32) {
    log_call(0x1c, this, get_from_memory::<u32>(entity + 4), flag, 0);
}

/// World whose entity vector `fake_mgr_remove_entity` erases the removed entity from, or `0`.
pub(super) static ERASE_WORLD: AtomicU32 = AtomicU32::new(0);
pub(super) static FAKE_PATH_RESULT: AtomicU32 = AtomicU32::new(0);

pub(super) extern "thiscall" fn fake_mgr_remove_entity(this: u32, entity: u32) {
    log_call(0x44, this, get_from_memory::<u32>(entity + 4), 0, 0);
    let world = ERASE_WORLD.load(Ordering::Relaxed);
    if world == 0 {
        return;
    }
    let end = get_from_memory::<u32>(world + 0x84);
    let mut cursor = get_from_memory::<u32>(world + 0x80);
    while cursor < end {
        if get_from_memory::<u32>(cursor) == entity {
            let mut shift = cursor;
            while shift + 4 < end {
                save_to_memory(shift, get_from_memory::<u32>(shift + 4));
                shift += 4;
            }
            save_to_memory(world + 0x84, end - 4);
            return;
        }
        cursor += 4;
    }
}

pub(super) extern "thiscall" fn fake_mgr_get_path(this: u32, from: u32, to: u32, unit: u32, flag: u32) -> u32 {
    log_call(0x18 | ((flag & 0xff) << 16), this, from, to, unit);
    FAKE_PATH_RESULT.load(Ordering::Relaxed)
}

pub(super) extern "thiscall" fn fake_show_update(this: u32) {
    log_call(0x04 + 0x200, this, 0, 0, 0);
}

pub(super) fn fn_word(function: usize) -> u32 {
    function as u32
}

/// `(kind, byte-vector length)` per entity slot; `None` is a null entry.
pub(super) type EntitySpec = Vec<Option<(Kind, u32)>>;

pub(super) struct FakeWorld {
    pub(super) words: Box<[u32; 0x60]>,
    pub(super) _world_vtable: Box<[u32; 0x10]>,
    pub(super) _habitat_vtable: Box<[u32; 0x40]>,
    pub(super) _entity_vtable: Box<[u32; 0x100]>,
    pub(super) _type_vtable: Box<FakeVtable>,
    pub(super) _types: Vec<Box<FakeType>>,
    pub(super) _habitats: Vec<Box<[u32; 4]>>,
    pub(super) habitat_ptrs: Box<[u32]>,
    pub(super) entities: Vec<Option<Box<[u32; 0x80]>>>,
    pub(super) entity_bufs: Vec<Box<[u8; 8]>>,
    pub(super) entity_ptrs: Box<[u32]>,
    pub(super) initial_entity_ptrs: Vec<u32>,
    pub(super) spec: EntitySpec,
}

impl FakeWorld {
    pub(super) fn new(habitat_results: &[u32], spec: EntitySpec) -> Self {
        let mut type_vtable = Box::new(FakeVtable { slots: [0; 0x20] });
        type_vtable.slots[0x18 / 4] = fn_word(fake_is_class as *const () as usize);
        type_vtable.slots[0x1c / 4] = fn_word(fake_is_cast_class as *const () as usize);
        let type_vtable_ptr = type_vtable.slots.as_ptr() as u32;
        let mut types = Vec::new();
        for kind in ALL_KINDS {
            let mut ty = Box::new(FakeType { words: [0; 0x100] });
            ty.words[0] = type_vtable_ptr;
            ty.words[1] = kind as u32;
            types.push(ty);
        }

        let mut world_vtable = Box::new([0u32; 0x10]);
        world_vtable[1] = fn_word(fake_world_pre_load as *const () as usize);
        world_vtable[0x1c / 4] = fn_word(fake_world_remove_entity as *const () as usize);
        let mut habitat_vtable = Box::new([0u32; 0x40]);
        habitat_vtable[0x10 / 4] = fn_word(fake_habitat_check as *const () as usize);
        habitat_vtable[0x58 / 4] = fn_word(fake_habitat_finish as *const () as usize);
        let mut entity_vtable = Box::new([0u32; 0x100]);
        entity_vtable[0x28 / 4] = fn_word(fake_entity_update as *const () as usize);
        entity_vtable[0xac / 4] = fn_word(fake_entity_is_removed as *const () as usize);
        entity_vtable[0xb0 / 4] = fn_word(fake_entity_is_removed_undo as *const () as usize);
        entity_vtable[0xa8 / 4] = fn_word(fake_entity_set_is_removed as *const () as usize);
        entity_vtable[0x110 / 4] = fn_word(fake_entity_reset as *const () as usize);
        entity_vtable[0x10c / 4] = fn_word(fake_entity_load_refresh as *const () as usize);
        entity_vtable[0x198 / 4] = fn_word(fake_entity_fixup as *const () as usize);

        let mut habitats = Vec::new();
        for (index, result) in habitat_results.iter().enumerate() {
            let mut habitat = Box::new([0u32; 4]);
            habitat[0] = habitat_vtable.as_ptr() as u32;
            habitat[1] = ID_HABITAT + index as u32;
            habitat[2] = *result;
            habitats.push(habitat);
        }
        let habitat_ptrs: Box<[u32]> = habitats.iter().map(|h| h.as_ptr() as u32).collect();

        let mut entities = Vec::new();
        let mut entity_bufs = Vec::new();
        for (index, slot) in spec.iter().enumerate() {
            entity_bufs.push(Box::new([0x7fu8; 8]));
            match slot {
                Some((kind, _)) => {
                    let mut entity = Box::new([0u32; 0x80]);
                    entity[0] = entity_vtable.as_ptr() as u32;
                    entity[1] = ID_ENTITY + index as u32;
                    entity[0x128 / 4] = types[*kind as usize].words.as_ptr() as u32;
                    entities.push(Some(entity));
                }
                None => entities.push(None),
            }
        }
        let entity_ptrs: Box<[u32]> = entities.iter().map(|e| e.as_ref().map_or(0, |b| b.as_ptr() as u32)).collect();

        let mut words = Box::new([0u32; 0x60]);
        words[0] = world_vtable.as_ptr() as u32;
        words[1] = ID_WORLD;
        let mut world = Self {
            words,
            _world_vtable: world_vtable,
            _habitat_vtable: habitat_vtable,
            _entity_vtable: entity_vtable,
            _type_vtable: type_vtable,
            _types: types,
            _habitats: habitats,
            habitat_ptrs,
            entities,
            entity_bufs,
            initial_entity_ptrs: entity_ptrs.to_vec(),
            entity_ptrs,
            spec,
        };
        world.reset();
        world
    }

    pub(super) fn ptr(&self) -> u32 {
        self.words.as_ptr() as u32
    }

    /// Restores every mutable field to the initial state, so the second pole starts where the first did.
    pub(super) fn reset(&mut self) {
        self.entity_ptrs.copy_from_slice(&self.initial_entity_ptrs);
        self.words[0x80 / 4] = self.entity_ptrs.as_ptr() as u32;
        self.words[0x84 / 4] = self.entity_ptrs.as_ptr() as u32 + 4 * self.entity_ptrs.len() as u32;
        self.words[0x98 / 4] = self.habitat_ptrs.as_ptr() as u32;
        self.words[0x9c / 4] = self.habitat_ptrs.as_ptr() as u32 + 4 * self.habitat_ptrs.len() as u32;
        for (index, slot) in self.spec.iter().enumerate() {
            let Some((_, length)) = slot else { continue };
            let buf = self.entity_bufs[index].as_ptr() as u32;
            *self.entity_bufs[index] = [0x7f; 8];
            let entity = self.entities[index].as_mut().unwrap();
            entity[0xdc / 4] = buf;
            entity[0xe0 / 4] = buf + length;
            entity[0xe8 / 4] = 0;
        }
    }

    /// Sets what entity `index` reports for `isRemoved` and `isRemovedUndo` (raw return values).
    pub(super) fn set_removed(&mut self, index: usize, is_removed: u32, undo: u32) {
        let entity = self.entities[index].as_mut().unwrap();
        entity[2] = is_removed;
        entity[3] = undo;
    }

    /// Ids of the entities still in the world's vector, in order (`0` for a null entry).
    pub(super) fn live_ids(&self) -> Vec<u32> {
        let (begin, end) = (self.words[0x80 / 4], self.words[0x84 / 4]);
        (begin..end).step_by(4).map(|slot| get_from_memory::<u32>(slot)).map(|entity| if entity == 0 { 0 } else { get_from_memory::<u32>(entity + 4) }).collect()
    }

    /// `[end - begin, flag byte, first byte]` per entity (`u32::MAX` triple for a null entry).
    pub(super) fn snapshot(&self) -> Vec<[u32; 3]> {
        self.entities
            .iter()
            .map(|slot| match slot {
                Some(entity) => {
                    let (begin, end) = (entity[0xdc / 4], entity[0xe0 / 4]);
                    [end.wrapping_sub(begin), entity[0xe8 / 4] & 0xff, get_from_memory::<u8>(begin) as u32]
                }
                None => [u32::MAX; 3],
            })
            .collect()
    }
}

/// Points the world/show pointer cells at fakes for the duration of `f` (`show == 0` leaves the show cell).
pub(super) fn with_globals<R>(world: u32, show: u32, f: impl FnOnce() -> R) -> R {
    let base = get_module_base("zoo.exe") as u32;
    let (world_cell, show_cell) = (base + RVA_GLOBAL_WORLD_MGR, base + RVA_GLOBAL_SHOW_MGR);
    let (old_world, old_show) = (get_from_memory::<u32>(world_cell), get_from_memory::<u32>(show_cell));
    save_to_memory(world_cell, world);
    if show != 0 {
        save_to_memory(show_cell, show);
    }
    let result = f();
    save_to_memory(world_cell, old_world);
    save_to_memory(show_cell, old_show);
    result
}

pub(super) struct FakeShow {
    pub(super) words: Box<[u32; 4]>,
    pub(super) _vtable: Box<[u32; 4]>,
}

impl FakeShow {
    pub(super) fn new() -> Self {
        let mut vtable = Box::new([0u32; 4]);
        vtable[1] = fn_word(fake_show_update as *const () as usize);
        let mut words = Box::new([0u32; 4]);
        words[0] = vtable.as_ptr() as u32;
        words[1] = ID_SHOW;
        Self { words, _vtable: vtable }
    }

    pub(super) fn ptr(&self) -> u32 {
        self.words.as_ptr() as u32
    }
}

/// A fake `BFAIMgr`/`ZTAIMgr`: tick fields, the path-cache tree container at `+0x12c` over a vanilla pool
/// header, and an embedded `BFEventMgr` whose two event vectors hold `events_a`/`events_b` real `BFEvent`s.
pub(super) struct FakeAiMgr {
    pub(super) words: Box<[u32; 0x60]>,
    pub(super) _vtable: Box<[u32; 0x40]>,
    pub(super) cache_head: u32,
    pub(super) _event_storage: Vec<Box<[u32]>>,
}

impl FakeAiMgr {
    pub(super) fn new(events_a: u32, events_b: u32) -> Self {
        let mut vtable = Box::new([0u32; 0x40]);
        vtable[0x40 / 4] = fn_word(fake_mgr_remove_entities as *const () as usize);
        vtable[0x44 / 4] = fn_word(fake_mgr_remove_entity as *const () as usize);
        vtable[0x18 / 4] = fn_word(fake_mgr_get_path as *const () as usize);
        let head = unsafe { POOL_ALLOCATE.original()(PATH_CACHE_NODE_SIZE) } as u32;
        save_to_memory(head, 0u32);
        save_to_memory(head + 4, 0u32);
        save_to_memory(head + 8, head);
        save_to_memory(head + 0xc, head);

        let mut words = Box::new([0u32; 0x60]);
        words[0] = vtable.as_ptr() as u32;
        words[1] = ID_MGR;
        words[0x12c / 4] = head;
        let base = words.as_ptr() as u32;
        let mut storage: Vec<Box<[u32]>> = Vec::new();
        for (vector_word, count, seed) in [(0xa0 / 4, events_a, 0x10u32), (0xac / 4, events_b, 0x90u32)] {
            let buffer: Box<[u32]> = vec![0u32; 7 * count.max(1) as usize].into_boxed_slice();
            let begin = buffer.as_ptr() as u32;
            for event in 0..count {
                let at = begin + event * 0x1c;
                bfevent_live_support::real_constructor(at as *const u32);
                for field in 2..7u32 {
                    save_to_memory(at + field * 4, seed + event * 0x101 + field * 0x1111);
                }
            }
            words[vector_word] = begin;
            words[vector_word + 1] = begin + count * 0x1c;
            words[vector_word + 2] = begin + count.max(1) * 0x1c;
            storage.push(buffer);
        }
        words[0xb8 / 4] = base + 0xa0;
        words[0xbc / 4] = base + 0xac;
        Self { words, _vtable: vtable, cache_head: head, _event_storage: storage }
    }

    pub(super) fn ptr(&self) -> *const u32 {
        self.words.as_ptr()
    }

    /// Replaces the embedded event manager with one built by the real `BFEventMgr` constructor: empty event vectors
    /// (so vanilla growth allocates fresh vanilla storage and never frees this fixture's Rust-owned buffers) and a
    /// valid empty listener set. What vanilla allocates is deliberately leaked.
    pub(super) fn use_real_event_mgr(&mut self) {
        unsafe { BFEVENTMGR_CONSTRUCTOR.original()((self.base() + 0xa0) as *const u32) };
    }

    /// Bytes of every event in both event vectors, `[vector A bytes, vector B bytes]`.
    pub(super) fn event_bytes(&self) -> [Vec<u8>; 2] {
        let read = |begin: u32, end: u32| (begin..end).map(get_from_memory::<u8>).collect::<Vec<u8>>();
        [read(self.words[0xa0 / 4], self.words[0xa4 / 4]), read(self.words[0xac / 4], self.words[0xb0 / 4])]
    }

    pub(super) fn base(&self) -> u32 {
        self.words.as_ptr() as u32
    }

    pub(super) fn container(&self) -> u32 {
        self.base() + 0x12c
    }

    pub(super) fn set_ticks(&mut self, divisor: u32, remainder: u32, ticks: u32) {
        self.words[0xe4 / 4] = divisor;
        self.words[0xe8 / 4] = remainder;
        self.words[0xec / 4] = ticks;
        let base = self.base();
        self.words[0xb8 / 4] = base + 0xa0;
        self.words[0xbc / 4] = base + 0xac;
    }

    /// Empties the path cache and refills it with `keys` nodes.
    pub(super) fn reset_cache(&mut self, keys: u32) {
        rb_tree_clear(self.container(), PATH_CACHE_NODE_SIZE);
        for key in 0..keys {
            rb_set_insert(self.container(), 0x1000 + key * 0x10);
        }
    }

    /// Tick fields, path-cache shape and the embedded event manager's words with in-object pointers made
    /// relative, so two separate fixtures (or two runs on one) compare equal.
    pub(super) fn snapshot(&self) -> Vec<u32> {
        let base = self.base();
        let head = self.words[0x12c / 4];
        let mut snap = vec![self.words[0xe4 / 4], self.words[0xe8 / 4], self.words[0xec / 4], self.words[0x130 / 4]];
        snap.push((get_from_memory::<u32>(head + 4) != 0) as u32);
        snap.push((get_from_memory::<u32>(head + 8) == head) as u32);
        snap.push((get_from_memory::<u32>(head + 0xc) == head) as u32);
        snap.push(walk_nodes(head).count() as u32);
        for index in 0xa0 / 4..=0xbc / 4 {
            let word = self.words[index];
            snap.push(if word >= base && word < base + 0x180 { word - base } else { word });
        }
        snap.push(get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_PATH_CACHE_NEXT_FLUSH));
        snap
    }
}

impl Drop for FakeAiMgr {
    fn drop(&mut self) {
        rb_tree_clear(self.container(), PATH_CACHE_NODE_SIZE);
        unsafe { POOL_DEALLOCATE.original()(self.cache_head as *const u32, PATH_CACHE_NODE_SIZE) };
    }
}

pub(super) fn standard_entities() -> EntitySpec {
    vec![Some((Kind::Guest, 3)), None, Some((Kind::Scenery, 0)), Some((Kind::Animal, 3)), Some((Kind::Staff, 1))]
}

pub(super) struct UpdateCase {
    pub(super) name: &'static str,
    pub(super) dt: u32,
    pub(super) remainder: u32,
    pub(super) ticks: u32,
    pub(super) divisor: u32,
    pub(super) next_flush: u32,
    pub(super) interval: u32,
    pub(super) keys: u32,
    pub(super) shrink: bool,
}

pub(super) fn update_cases() -> Vec<UpdateCase> {
    let case = |name, dt, remainder, ticks, divisor, next_flush, interval, keys, shrink| UpdateCase { name, dt, remainder, ticks, divisor, next_flush, interval, keys, shrink };
    vec![
        case("no tick, no flush", 16, 0, 0, 1000, u32::MAX, 500, 4, false),
        case("carry into ticks, empty cache flush", 1500, 600, 7, 1000, 0, 77, 0, false),
        case("flush frees a populated cache", 1500, 600, 7, 1000, 0, 77, 5, false),
        case("ticks equal next flush", 1000, 0, 9, 1000, 10, 40, 3, false),
        case("one tick short of flush", 1000, 0, 8, 1000, 10, 40, 3, false),
        case("large flush tree", 2000, 1, 1, 1000, 1, 5, 40, false),
        case("wrapping accumulators", u32::MAX, 5, u32::MAX, 1, 2, 3, 2, false),
        case("zero dt", 0, 999, 3, 1000, 4, 10, 2, false),
        case("removeEntities shrinks the entity list", 100, 0, 0, 1000, u32::MAX, 9, 1, true),
    ]
}

pub(super) fn set_flush_globals(next_flush: u32, interval: u32) {
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(base + RVA_PATH_CACHE_NEXT_FLUSH, next_flush);
    save_to_memory(base + RVA_PATH_CACHE_FLUSH_INTERVAL, interval);
}

/// Shared driver for the two `update` tests: every case runs `real` then `port` over identical fixtures and
/// compares the call log, the manager snapshot and every entity's mutated fields.
pub(super) fn compare_update(real: fn(*const u32, u32), port: fn(*const u32, u32), with_show: bool) -> Vec<String> {
    let base = get_module_base("zoo.exe") as u32;
    let saved_next_flush = get_from_memory::<u32>(base + RVA_PATH_CACHE_NEXT_FLUSH);
    let saved_interval = get_from_memory::<u32>(base + RVA_PATH_CACHE_FLUSH_INTERVAL);
    let mut failures: Vec<String> = Vec::new();
    for case in update_cases() {
        let mut world = FakeWorld::new(&[], standard_entities());
        let mut mgr = FakeAiMgr::new(0, 0);
        let show = FakeShow::new();
        let show_ptr = if with_show { show.ptr() } else { 0 };
        let mut outcomes = Vec::new();
        for pole in [real, port] {
            mgr.set_ticks(case.divisor, case.remainder, case.ticks);
            mgr.reset_cache(case.keys);
            world.reset();
            set_flush_globals(case.next_flush, case.interval);
            SHRINK_WORLD.store(if case.shrink { world.ptr() } else { 0 }, Ordering::Relaxed);
            take_log();
            with_globals(world.ptr(), show_ptr, || pole(mgr.ptr(), case.dt));
            SHRINK_WORLD.store(0, Ordering::Relaxed);
            outcomes.push((take_log(), mgr.snapshot(), world.snapshot()));
        }
        if outcomes[0].0 != outcomes[1].0 {
            failures.push(format!("{}: call log real {:x?} != port {:x?}", case.name, outcomes[0].0, outcomes[1].0));
        }
        if outcomes[0].1 != outcomes[1].1 {
            failures.push(format!("{}: manager state real {:x?} != port {:x?}", case.name, outcomes[0].1, outcomes[1].1));
        }
        if outcomes[0].2 != outcomes[1].2 {
            failures.push(format!("{}: entity state differs", case.name));
        }
        let expected_updates = 4 - usize::from(case.shrink);
        let entity_updates = outcomes[0].0.iter().filter(|entry| entry[0] == 0x28).count();
        if entity_updates != expected_updates {
            failures.push(format!("{}: fixture check: {entity_updates} entity updates, expected {expected_updates}", case.name));
        }
        let removals = outcomes[0].0.iter().filter(|entry| entry[0] == 0x40).count();
        if removals != 1 {
            failures.push(format!("{}: fixture check: removeEntities ran {removals} times", case.name));
        }
    }
    set_flush_globals(saved_next_flush, saved_interval);
    failures
}
