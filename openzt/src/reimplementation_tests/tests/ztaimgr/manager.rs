//! Manager lifecycle: `update`, `save`, `loadData`, `create`, construction, `checkPath`, `getPath`, `callDone`.

use super::*;

/// `ZTAIMGR_BFAIMGR_UPDATE`: tick carry and wraparound, the path-cache flush threshold (`>=`) and refill of the
/// next-flush global, `removeEntities` running before the entity count is taken, null entity entries, and the
/// event manager pump. The port skips a zero tick divisor instead of faulting; asserted separately.
pub(crate) fn run_ztaimgr_bfaimgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_UPDATE";
    let mut failures = compare_update(live_support::real_bfaimgr_update, |this, dt| unsafe { BFAIMGR_UPDATE.hooked()(this, dt) }, false);
    let mut world = FakeWorld::new(&[], standard_entities());
    let mut mgr = FakeAiMgr::new(0, 0);
    mgr.set_ticks(0, 123, 456);
    mgr.reset_cache(2);
    world.reset();
    take_log();
    let before = mgr.snapshot();
    with_globals(world.ptr(), 0, || unsafe { BFAIMGR_UPDATE.hooked()(mgr.ptr(), 50) });
    if mgr.snapshot() != before || !take_log().is_empty() {
        failures.push("zero divisor: the port should leave all state alone and run nothing".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_UPDATE` (live zoo, for the real `ZTMaintTaskPool`): the real `ZTAIMgr::update` pumps the real
/// maintenance pool and the fake show manager, then reaches `BFAIMgr::update` at its fixed address, which is
/// the port; so this checks the `ZTAIMgr` half (order and objects of the pumps) and re-checks the base half.
pub(crate) fn run_ztaimgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let failures = compare_update(live_support::real_ztaimgr_update, |this, dt| unsafe { ZTAIMGR_UPDATE.hooked()(this, dt) }, true);
    finish_test("ZTAIMGR_UPDATE", failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_SAVE`: serialized bytes and return value from the real and ported `save`, for empty and
/// populated event vectors.
pub(crate) fn run_ztaimgr_bfaimgr_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_SAVE";
    let mut failures: Vec<String> = Vec::new();
    let dummy_file: u32 = 0;
    let file = &dummy_file as *const u32 as *const i8;
    for (events_a, events_b) in [(0u32, 0u32), (2, 1), (1, 3), (0, 2)] {
        let mut mgr = FakeAiMgr::new(events_a, events_b);
        mgr.set_ticks(1000, 0x1111_2222, 0x3333_4444);
        io_redirect::begin_capture();
        let real_ret = live_support::real_bfaimgr_save(mgr.ptr(), file);
        let real_bytes = io_redirect::end_capture();
        io_redirect::begin_capture();
        let port_ret = unsafe { BFAIMGR_SAVE.hooked()(mgr.ptr(), file) };
        let port_bytes = io_redirect::end_capture();
        let label = format!("events {events_a}+{events_b}");
        // Two counts, the events, and the one trailing byte `BFEventMgr::save` writes.
        let expected_len = (8 + 4 + events_a * 0x14 + 4 + events_b * 0x14 + 1) as usize;
        if real_bytes.len() != expected_len {
            failures.push(format!("{label}: fixture check: real save wrote {} bytes, expected {expected_len}", real_bytes.len()));
        }
        if real_bytes != port_bytes {
            failures.push(format!("{label}: bytes differ (real {} bytes, port {} bytes)", real_bytes.len(), port_bytes.len()));
        }
        if real_ret != port_ret || !real_ret {
            failures.push(format!("{label}: return real {real_ret} port {port_ret}, expected both true"));
        }
        if real_bytes.len() >= 8 && (real_bytes[0..4] != 0x1111_2222u32.to_le_bytes() || real_bytes[4..8] != 0x3333_4444u32.to_le_bytes()) {
            failures.push(format!("{label}: remainder/tick words not first in the stream"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_LOAD_DATA`: partial and full loads over fake habitats (success bytes with garbage upper
/// bytes, which must not count) and fake entities (unit and non-unit types, empty and non-empty byte vectors,
/// null entries): return value, call order and arguments, and every entity's mutated fields.
pub(crate) fn run_ztaimgr_bfaimgr_load_data_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_LOAD_DATA";
    let mut failures: Vec<String> = Vec::new();
    let habitat_sets: [(&str, Vec<u32>); 5] = [
        ("none", vec![]),
        ("all ok", vec![1, 1, 1]),
        ("one failing low byte, garbage above", vec![1, 0x5500, 1]),
        ("garbage above a passing low byte", vec![0x1234_5601, 0xffff_ff01]),
        ("only failing", vec![0]),
    ];
    let entity_sets: [(&str, EntitySpec); 4] = [
        ("no entities", vec![]),
        ("standard", standard_entities()),
        ("scenery only", vec![Some((Kind::Scenery, 2)), None]),
        ("units", vec![Some((Kind::Guest, 0)), Some((Kind::Guide, 5)), Some((Kind::Staff, 1))]),
    ];
    for (habitat_name, habitats) in &habitat_sets {
        for (entity_name, entities) in &entity_sets {
            for full in [false, true] {
                let label = format!("habitats {habitat_name}, entities {entity_name}, full {full}");
                let mut world = FakeWorld::new(habitats, entities.clone());
                let mut outcomes = Vec::new();
                for port in [false, true] {
                    world.reset();
                    take_log();
                    let result = with_globals(world.ptr(), 0, || {
                        if port {
                            unsafe { LOAD_DATA.hooked()(std::ptr::null(), full) }
                        } else {
                            live_support::real_bfaimgr_load_data(std::ptr::null(), full)
                        }
                    });
                    outcomes.push((result, take_log(), world.snapshot()));
                }
                if outcomes[0].0 != outcomes[1].0 {
                    failures.push(format!("{label}: return real {} port {}", outcomes[0].0, outcomes[1].0));
                }
                if outcomes[0].1 != outcomes[1].1 {
                    failures.push(format!("{label}: call log real {:x?} != port {:x?}", outcomes[0].1, outcomes[1].1));
                }
                if outcomes[0].2 != outcomes[1].2 {
                    failures.push(format!("{label}: entity state real {:?} != port {:?}", outcomes[0].2, outcomes[1].2));
                }
                let finishes = outcomes[0].1.iter().filter(|entry| entry[0] == 0x58).count();
                if finishes != habitats.len() {
                    failures.push(format!("{label}: fixture check: {finishes} habitat finishes, expected {}", habitats.len()));
                }
                let fixups = outcomes[0].1.iter().filter(|entry| entry[0] == 0x198).count();
                let expected_fixups = if full { entities.iter().flatten().filter(|(kind, _)| *kind != Kind::Scenery).count() } else { 0 };
                if fixups != expected_fixups {
                    failures.push(format!("{label}: fixture check: {fixups} unit fixups, expected {expected_fixups}"));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_CREATE` (live zoo: `initAIParams` reads the loaded configuration): the real and ported `create`
/// each build a manager; both must install the same vtable and listener table, run `initAIParams` to the same
/// values, and agree on every other word (heap pointers compared only as non-null). Both managers are leaked:
/// freeing them would run the live destructor, which this test has no reason to exercise.
pub(crate) fn run_ztaimgr_create_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_CREATE";
    let mut failures: Vec<String> = Vec::new();
    let base = get_module_base("zoo.exe") as u32;
    let real = live_support::real_ztaimgr_create();
    let port = unsafe { CREATE.hooked()() };
    if real == 0 || port == 0 {
        failures.push(format!("create returned real {real:#x} port {port:#x}"));
        return finish_test(test_name, failures, failure_log);
    }
    for manager in [real, port] {
        if get_from_memory::<u32>(manager) != base + 0x0023_5324 {
            failures.push(format!("{manager:#x}: vtable word {:#x}, expected ZTAIMgr's", get_from_memory::<u32>(manager)));
        }
        if get_from_memory::<u32>(manager + 8) != base + 0x0023_2288 {
            failures.push(format!("{manager:#x}: listener table word {:#x}, expected ZTAIMgr's", get_from_memory::<u32>(manager + 8)));
        }
    }
    for offset in (0..0x150u32).step_by(4) {
        // `BFPathFinder`'s constructor, given a null map, derives the row count (`+0xc` of the pathfinder, `+0x1c`
        // here) and the tile count (`+0x20`) from `[pathfinder + 0x38]`, which it never initialises, so both
        // hold whatever `operator new` returned.
        if offset == 0x1c || offset == 0x20 {
            continue;
        }
        let (real_word, port_word) = (get_from_memory::<u32>(real + offset), get_from_memory::<u32>(port + offset));
        let both_heap_pointers = real_word >= 0x10000 && port_word >= 0x10000;
        if real_word != port_word && !both_heap_pointers {
            failures.push(format!("+{offset:#x}: real {real_word:#x} != port {port_word:#x}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_CONSTRUCT_DESTRUCT` (live zoo: `BFAIMgr::init` reads the loaded configuration): the real and
/// ported constructor each build a zeroed `0x150`-byte block (every word equal, heap pointers compared only as
/// non-null, `+0x134` skipped: vanilla leaves a stale stack byte there); the real and ported destructor then tear
/// one each down (every word equal, same final `BFMgr` vtable, empty path cache). Blocks are freed with `operator
/// delete`; the real constructor's and destructor's own allocations stay with vanilla's allocator throughout.
pub(crate) fn run_ztaimgr_bfaimgr_construct_destruct_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::{
        bfaimgr::{CONSTRUCTOR as BFAIMGR_CONSTRUCTOR, DESTRUCTOR_0 as BFAIMGR_DESTRUCTOR_0, DESTRUCTOR_1 as BFAIMGR_DESTRUCTOR_1},
        standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    };
    let test_name = "ZTAIMGR_BFAIMGR_CONSTRUCT_DESTRUCT";
    let mut failures: Vec<String> = Vec::new();
    let base = get_module_base("zoo.exe") as u32;
    const SIZE: u32 = 0x150;
    let allocate = || {
        let block = unsafe { OPERATOR_NEW.original()(SIZE) } as u32;
        for offset in (0..SIZE).step_by(4) {
            save_to_memory(block + offset, 0u32);
        }
        block
    };
    let (real, port) = (allocate(), allocate());
    if real == 0 || port == 0 {
        failures.push("operator new failed".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    unsafe {
        live_support::real_bfaimgr_construct(real as *const u32);
        BFAIMGR_CONSTRUCTOR.hooked()(port as *const u32);
    }
    let compare = |phase: &str, failures: &mut Vec<String>| {
        for offset in (0..SIZE).step_by(4) {
            if offset == 0x134 {
                continue;
            }
            let (real_word, port_word) = (get_from_memory::<u32>(real + offset), get_from_memory::<u32>(port + offset));
            let both_heap_pointers = real_word >= 0x10000 && port_word >= 0x10000 && !(real_word >= base && real_word < base + 0x300000);
            if real_word != port_word && !both_heap_pointers {
                failures.push(format!("{phase} +{offset:#x}: real {real_word:#x} != port {port_word:#x}"));
            }
        }
    };
    compare("construct", &mut failures);
    if get_from_memory::<u32>(real + 0xd0) == 0 || get_from_memory::<u32>(port + 0xd0) == 0 {
        failures.push("construct left a null GXLLEAnimSet".to_string());
    }
    unsafe {
        live_support::real_bfaimgr_destruct(real as *const u32);
        BFAIMGR_DESTRUCTOR_0.hooked()(port as *const u32);
    }
    compare("destruct", &mut failures);
    for manager in [real, port] {
        if get_from_memory::<u32>(manager) != base + 0x0023_55c0 {
            failures.push(format!("{manager:#x}: vtable word {:#x}, expected BFMgr's", get_from_memory::<u32>(manager)));
        }
        if get_from_memory::<u32>(manager + 0x130) != 0 {
            failures.push(format!("{manager:#x}: path cache size not zero after destruct"));
        }
    }
    // Deleting destructor: bit 0 clear leaves the block, bit 0 set frees it (nothing to read back afterwards).
    let deleting = allocate();
    unsafe {
        BFAIMGR_CONSTRUCTOR.hooked()(deleting as *const u32);
        let returned = BFAIMGR_DESTRUCTOR_1.hooked()(deleting as *const u32, 0);
        if returned as u32 != deleting {
            failures.push(format!("deleting destructor returned {:#x}, expected {deleting:#x}", returned as u32));
        }
        BFAIMGR_CONSTRUCTOR.hooked()(deleting as *const u32);
        BFAIMGR_DESTRUCTOR_1.hooked()(deleting as *const u32, 1);
        OPERATOR_DELETE.original()(real);
        OPERATOR_DELETE.original()(port);
    }
    finish_test(test_name, failures, failure_log)
}

pub(super) static FIND_PATH_LOG: Mutex<Vec<[u32; 9]>> = Mutex::new(Vec::new());
pub(super) static FIND_PATH_MGR: AtomicU32 = AtomicU32::new(0);

/// Fake `BFPathFinder::findPath`: records the finder-relative object, the two position pointers, the unit, the
/// trailing flag, and the pathfinder fields `checkPath` is supposed to have set up before this call.
pub(super) extern "thiscall" fn fake_find_path(this: u32, from: u32, to: u32, unit: u32, flag: u32) -> u32 {
    let mgr = FIND_PATH_MGR.load(Ordering::Relaxed);
    FIND_PATH_LOG.lock().unwrap().push([
        this - mgr,
        from,
        to,
        unit,
        flag,
        get_from_memory::<u32>(this + 0x3c),
        get_from_memory::<u32>(this + 0x8),
        get_from_memory::<u32>(this + 0xc),
        get_from_memory::<u32>(this + 0x10),
    ]);
    0
}

/// `ZTAIMGR_BFAIMGR_CHECK_PATH` (live zoo, manager leaked like `ZTAIMGR_CREATE`'s): a real manager (so the real `setNodeMax` has real buffers to
/// resize) whose pathfinder vtable is swapped for one with a fake `findPath`, and whose result list is filled
/// with 0, 1 or several pool nodes. Real and port `checkPath` must agree on the result byte, the arguments the
/// search is given, the pathfinder fields set up beforehand, and the restored node limit.
pub(crate) fn run_ztaimgr_bfaimgr_check_path_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_CHECK_PATH";
    let mut failures: Vec<String> = Vec::new();
    let mgr = live_support::real_ztaimgr_create();
    if mgr == 0 {
        return finish_test(test_name, vec!["could not build a real manager".to_string()], failure_log);
    }
    FIND_PATH_MGR.store(mgr, Ordering::Relaxed);
    let mut finder_vtable = Box::new([0u32; 4]);
    finder_vtable[0] = fn_word(fake_find_path as *const () as usize);
    let real_finder_vtable = get_from_memory::<u32>(mgr + 0x10);
    save_to_memory(mgr + 0x10, finder_vtable.as_ptr() as u32);

    let mut world = FakeWorld::new(&[], Vec::new());
    world.words[0x34 / 4] = 37;
    world.words[0x38 / 4] = 23;
    let from = Box::new([0u32; 0x20]);
    let to = Box::new([0u32; 0x20]);
    let list = get_from_memory::<u32>(mgr + 0x80);
    let (saved_first, saved_last) = (get_from_memory::<u32>(list), get_from_memory::<u32>(list + 4));

    for (nodes, limit, unit) in [(0u32, 250u32, 0x77u32), (1, 7, 0), (3, 250, 0x1234_5678), (0, 100, 5)] {
        // Replace the result list with `nodes` pool nodes, linked into the head's circular list.
        let mut node_ptrs = Vec::new();
        for _ in 0..nodes {
            node_ptrs.push(unsafe { POOL_ALLOCATE.original()(0x14) } as u32);
        }
        let (mut previous, mut next_of_previous) = (list, list + 0);
        for &node in &node_ptrs {
            save_to_memory(next_of_previous, node);
            save_to_memory(node + 4, previous);
            previous = node;
            next_of_previous = node;
        }
        save_to_memory(next_of_previous, list);
        save_to_memory(list + 4, previous);
        save_to_memory(mgr + 0x4c, limit);

        let mut outcomes = Vec::new();
        for port in [false, true] {
            FIND_PATH_LOG.lock().unwrap().clear();
            let result = with_globals(world.ptr(), 0, || {
                if port {
                    unsafe { BFAIMGR_CHECK_PATH.hooked()(mgr as *const u32, from.as_ptr(), to.as_ptr(), unit as *const u32) }
                } else {
                    live_support::real_bfaimgr_check_path(mgr as *const u32, from.as_ptr(), to.as_ptr(), unit as *const u32)
                }
            });
            outcomes.push((
                result & 0xff,
                std::mem::take(&mut *FIND_PATH_LOG.lock().unwrap()),
                [get_from_memory::<u32>(mgr + 0x18), get_from_memory::<u32>(mgr + 0x1c), get_from_memory::<u32>(mgr + 0x20), get_from_memory::<u32>(mgr + 0x4c)],
            ));
        }
        let label = format!("{nodes} nodes, limit {limit}");
        if outcomes[0].0 != outcomes[1].0 {
            failures.push(format!("{label}: result real {} port {}", outcomes[0].0, outcomes[1].0));
        }
        if outcomes[0].0 != u32::from(nodes > 0) {
            failures.push(format!("{label}: fixture check: real result {} for {nodes} nodes", outcomes[0].0));
        }
        if outcomes[0].1 != outcomes[1].1 {
            failures.push(format!("{label}: findPath calls real {:x?} != port {:x?}", outcomes[0].1, outcomes[1].1));
        }
        if outcomes[0].1.len() != 1 || outcomes[0].1[0][5] != 100 {
            failures.push(format!("{label}: fixture check: findPath calls {:x?}", outcomes[0].1));
        }
        if outcomes[0].2 != outcomes[1].2 {
            failures.push(format!("{label}: pathfinder state real {:x?} != port {:x?}", outcomes[0].2, outcomes[1].2));
        }
        if outcomes[0].2[3] != limit {
            failures.push(format!("{label}: fixture check: node limit {} after the call, expected {limit}", outcomes[0].2[3]));
        }
        for node in node_ptrs {
            unsafe { POOL_DEALLOCATE.original()(node as *const u32, 0x14) };
        }
    }

    save_to_memory(list, saved_first);
    save_to_memory(list + 4, saved_last);
    save_to_memory(mgr + 0x10, real_finder_vtable);
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_REMOVE_ENTITIES`: sweeps over a fake world whose entities report removed / undo-removed
/// through raw return values with garbage upper bytes. The fake manager's `removeEntity` erases the entity from
/// the list, so the index and count stepping is exercised, including consecutive removals and removal of
/// everything.
pub(crate) fn run_ztaimgr_bfaimgr_remove_entities_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_REMOVE_ENTITIES";
    let mut failures: Vec<String> = Vec::new();
    let mgr = FakeAiMgr::new(0, 0);
    // (name, per-entity (isRemoved, isRemovedUndo) raw returns, entity spec)
    let five = standard_entities();
    let cases: Vec<(&str, Vec<(u32, u32)>, EntitySpec)> = vec![
        ("nothing removed", vec![(0, 0); 5], five.clone()),
        ("first removed", vec![(1, 0), (0, 0), (0, 0), (0, 0), (0, 0)], five.clone()),
        ("two consecutive removed", vec![(0, 0), (0, 0), (1, 0), (1, 0), (0, 0)], five.clone()),
        ("last removed", vec![(0, 0), (0, 0), (0, 0), (0, 0), (1, 0)], five.clone()),
        ("everything removed", vec![(1, 0), (0, 0), (1, 0), (1, 0), (1, 0)], five.clone()),
        ("undo-removed is not destroyed", vec![(0, 0), (0, 0), (1, 1), (0, 0), (1, 0)], five.clone()),
        ("garbage above a zero low byte", vec![(0x1234_5600, 0), (0, 0), (0xffff_ff00, 0), (0, 0), (0x5500, 0)], five.clone()),
        ("garbage above a one low byte", vec![(0x1234_5601, 0xabcd_0000), (0, 0), (0xffff_ff01, 0xabcd_0001), (0, 0), (0, 0)], five.clone()),
        ("empty world", Vec::new(), Vec::new()),
    ];
    for (name, flags, spec) in cases {
        let mut world = FakeWorld::new(&[], spec.clone());
        let mut flag_iter = flags.iter();
        let mut expected_removed = 0usize;
        for (index, slot) in spec.iter().enumerate() {
            if slot.is_some() {
                let (removed, undo) = *flag_iter.next().unwrap_or(&(0, 0));
                if removed & 0xff != 0 && undo & 0xff == 0 {
                    expected_removed += 1;
                }
                world.set_removed(index, removed, undo);
            } else {
                let _ = flag_iter.next();
            }
        }
        let mut outcomes = Vec::new();
        for port in [false, true] {
            world.reset();
            take_log();
            ERASE_WORLD.store(world.ptr(), Ordering::Relaxed);
            with_globals(world.ptr(), 0, || {
                if port {
                    unsafe { BFAIMGR_REMOVE_ENTITIES.hooked()(mgr.ptr()) }
                } else {
                    live_support::real_bfaimgr_remove_entities(mgr.ptr())
                }
            });
            ERASE_WORLD.store(0, Ordering::Relaxed);
            outcomes.push((take_log(), world.live_ids()));
        }
        if outcomes[0].0 != outcomes[1].0 {
            failures.push(format!("{name}: call log real {:x?} != port {:x?}", outcomes[0].0, outcomes[1].0));
        }
        if outcomes[0].1 != outcomes[1].1 {
            failures.push(format!("{name}: remaining entities real {:?} != port {:?}", outcomes[0].1, outcomes[1].1));
        }
        if outcomes[0].1.len() != spec.len() - expected_removed {
            failures.push(format!("{name}: fixture check: {} entities left, expected {}", outcomes[0].1.len(), spec.len() - expected_removed));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_GET_PATH_TILES`: the tile overload forwards to vtable slot `+0x18` with both tiles offset to
/// their positions, the unit and the flag, and returns the callee's low byte.
pub(crate) fn run_ztaimgr_bfaimgr_get_path_tiles_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_GET_PATH_TILES";
    let mut failures: Vec<String> = Vec::new();
    let mgr = FakeAiMgr::new(0, 0);
    let from = Box::new([0u32; 0x20]);
    let to = Box::new([0u32; 0x20]);
    for result in [0u32, 1, 0x1234_5601, 0x5500, 0xffff_ff00] {
        for flag in [false, true] {
            for unit in [0u32, 0x77, 0xdead_beef] {
                FAKE_PATH_RESULT.store(result, Ordering::Relaxed);
                take_log();
                let real = live_support::real_bfaimgr_get_path_tiles(mgr.ptr(), from.as_ptr(), to.as_ptr(), unit as *const u32, flag);
                let real_log = take_log();
                let port = unsafe { BFAIMGR_GET_PATH_1.hooked()(mgr.ptr(), from.as_ptr(), to.as_ptr(), unit as *const u32, flag) };
                let port_log = take_log();
                let label = format!("result {result:#x}, flag {flag}, unit {unit:#x}");
                if real != port || real != (result & 0xff != 0) {
                    failures.push(format!("{label}: result real {real} port {port}, expected {}", result & 0xff != 0));
                }
                if real_log != port_log {
                    failures.push(format!("{label}: call log real {real_log:x?} != port {port_log:x?}"));
                }
                let expected = [0x18 | (u32::from(flag) << 16), ID_MGR, from.as_ptr() as u32 + 0x34, to.as_ptr() as u32 + 0x34, unit];
                if port_log != vec![expected] {
                    failures.push(format!("{label}: fixture check: log {port_log:x?}, expected {expected:x?}"));
                }
            }
        }
    }
    FAKE_PATH_RESULT.store(0, Ordering::Relaxed);
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_CALL_DONE`: every combination of the entity flag, the mixer flag and the cursor/end
/// comparison, over a zeroed mixer (so the real `GXMixer::getAnim` finds no animation), plus a null entity.
pub(crate) fn run_ztaimgr_bfaimgr_call_done_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_CALL_DONE";
    let mut failures: Vec<String> = Vec::new();
    let mgr = FakeAiMgr::new(0, 0);
    let mut entity = Box::new([0u32; 0x80]);
    for entity_flag in [0u32, 1, 0xff, 0x100] {
        for mixer_flag in [0u32, 1, 0x80] {
            for equal in [false, true] {
                entity[0xf8 / 4] = entity_flag;
                entity[0xd4 / 4] = mixer_flag;
                entity[0x9c / 4] = 0x5000;
                entity[0x8c / 4] = if equal { 0x5000 } else { 0x6000 };
                let ptr = entity.as_ptr();
                let real = live_support::real_bfaimgr_call_done(mgr.ptr(), ptr);
                let port = unsafe { BFAIMGR_CALL_DONE.hooked()(mgr.ptr(), ptr) };
                let expected = entity_flag & 0xff != 0 && mixer_flag & 0xff != 0;
                if real != port || real != expected {
                    failures.push(format!("entity flag {entity_flag:#x}, mixer flag {mixer_flag:#x}, equal {equal}: real {real} port {port}, expected {expected}"));
                }
            }
        }
    }
    let (real, port) = (live_support::real_bfaimgr_call_done(mgr.ptr(), std::ptr::null()), unsafe { BFAIMGR_CALL_DONE.hooked()(mgr.ptr(), std::ptr::null()) });
    if real || port {
        failures.push(format!("null entity: real {real} port {port}"));
    }
    finish_test(test_name, failures, failure_log)
}
