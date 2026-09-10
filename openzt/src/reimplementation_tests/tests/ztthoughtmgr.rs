//! Compares real vanilla `ZTThoughtMgr`/`ZTThought` against their Rust reimplementation
//! (production file `openzt/src/ztthoughtmgr.rs`): add/remove/get thoughts, save/load (the early
//! format and the `version >= 0x1e` modern branch), addThought's animal-override behavior,
//! populateThoughts, `ZTThought::getString`'s `%s` substitution, and the real-zoo save round-trip.

use openzt_detour::generated::ztthought as gen_ztthought;
use openzt_detour::generated::ztthoughtmgr as gen_ztthoughtmgr;
use proptest::prelude::*;
use std::io::Write;
use tracing::{error, info};

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::util::{get_from_memory, save_to_memory, ZTBufferString, ZTString};
use crate::zthabitatmgr::ZTHabitat;
use crate::ztthoughtmgr::{live_support as thought_live_support, ZTThought, ZTThoughtMgr};
use crate::ztworldmgr::{BFEntity, IVec3, ZTAnimal};
use crate::bfentitytype::ZTAnimalType;

// ============================================================================================
// Live comparison battery for ZTThoughtMgr/ZTThought, registered in `battery.rs`. The
// add/remove/get/save/load tests are self-contained (no `GLOBAL_ZTWorldMgr`/string-table dependency)
// and run from `early_tests()`; `ZTTHOUGHTMGR_LOAD_MODERN`, `ZTTHOUGHTMGR_ADD_THOUGHT_ANIMAL_OVERRIDE`,
// `ZTTHOUGHTMGR_POPULATE_THOUGHTS` and `ZTTHOUGHT_GET_STRING` need `GLOBAL_ZTWorldMgr`/language DLLs
// respectively, so they run from `always_late_tests()`.
// ============================================================================================

/// Field tuple used to compare two `ZTThought`s structurally via their existing public getters -
/// `ZTThought` derives neither `PartialEq` nor a public constructor, so this is simpler than adding
/// either just for these tests.
fn thought_fields(t: &ZTThought) -> (u32, u32, u32, i32, i32, u32, u32, u32) {
    (t.string_id(), t.thinker_id(), t.object_id(), t.tile_x(), t.tile_y(), t.thinker_ptr(), t.object_ptr(), t.habitat_ptr())
}

/// ZTTHOUGHTMGR_ADD_THOUGHT: compares the real `ZTThoughtMgr::addThought`'s effect on list
/// order/length against the reimplemented `add_thought`, across a generated sequence of calls and a
/// small `max_thoughts` cap (so cap-trimming is actually exercised). Restricted to
/// `thinker_ptr = object_ptr = habitat_ptr = 0` for every call - `ZTThought::new` dereferences all
/// three when non-null, and there's no live entity/habitat this standalone test could safely point
/// them at. This still fully exercises `addThought`'s own cap-trim/insertion-order logic, the part
/// this test actually targets.
pub(crate) fn run_thoughtmgr_add_thought_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_ADD_THOUGHT";
    let mut fail_flag = false;

    match runner.run(&(1u32..5, prop::collection::vec(any::<u32>(), 0..8)), |(max_thoughts, string_ids)| {
        let real_ptr = thought_live_support::build_standalone_mgr(max_thoughts);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(max_thoughts);

        for &string_id in &string_ids {
            unsafe {
                gen_ztthoughtmgr::ADD_THOUGHT.original()(real_ptr as *const u32, string_id, std::ptr::null(), std::ptr::null(), std::ptr::null());
            }
            unsafe { &mut *reimpl_ptr }.add_thought(string_id, 0, 0, 0);
        }

        let real_thoughts = thought_live_support::read_raw_chain(unsafe { &*real_ptr });
        let real_fields: Vec<_> = real_thoughts.iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();
        let real_len = real_thoughts.len();
        let reimpl_len = unsafe { &*reimpl_ptr }.len();

        thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(real_len, reimpl_len, "length mismatch for max_thoughts={}, string_ids={:?}", max_thoughts, string_ids);
        prop_assert_eq!(real_fields, reimpl_fields, "content mismatch for max_thoughts={}, string_ids={:?}", max_thoughts, string_ids);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// Seeds `mgr`'s reimplemented-side store with one `ZTThought` per `specs` entry (`(string_id,
/// thinker_ptr, object_ptr, habitat_ptr)`), front-to-back, via `insert_front` - for the side of a
/// comparison driven through a direct reimplemented-method call. `thinker_id`/`object_id`/`tile_x`/
/// `tile_y` are left at ctor defaults since no consumer of this helper reads them.
fn seed_thoughts(mgr: &mut ZTThoughtMgr, specs: &[(u32, u32, u32, u32)]) {
    for &(string_id, thinker_ptr, object_ptr, habitat_ptr) in specs {
        mgr.insert_front(thought_live_support::new_thought(string_id, 0, 0, -1, -1, thinker_ptr, object_ptr, habitat_ptr));
    }
}

/// Seeds `mgr`'s raw `sentinel_ptr` chain (not its reimplemented-side store) with one `ZTThought` per
/// `specs` entry, front-to-back, via `seed_raw_chain` - for the "real" side of a comparison driven
/// through a genuine, undetoured `.original()` call, which reads `sentinel_ptr` directly and knows
/// nothing about the reimplemented-side store.
fn seed_thoughts_raw(mgr: &ZTThoughtMgr, specs: &[(u32, u32, u32, u32)]) {
    for &(string_id, thinker_ptr, object_ptr, habitat_ptr) in specs {
        thought_live_support::seed_raw_chain(mgr, thought_live_support::new_thought(string_id, 0, 0, -1, -1, thinker_ptr, object_ptr, habitat_ptr));
    }
}

/// Seeds `mgr` on *both* representations at once (raw chain and reimplemented-side store) with
/// identical content - for tests that drive a *single* instance through both a real `.original()`
/// call (reads the raw chain) and a direct reimplemented-method call (reads the store), e.g. the
/// `getThoughtsBy*` comparisons below.
fn seed_thoughts_both(mgr: &mut ZTThoughtMgr, specs: &[(u32, u32, u32, u32)]) {
    seed_thoughts_raw(mgr, specs);
    seed_thoughts(mgr, specs);
}

fn thought_spec_strategy() -> impl Strategy<Value = (u32, u32, u32, u32)> {
    (any::<u32>(), 0u32..5, 0u32..5, 0u32..5)
}

/// ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_THINKER: compares the real `ZTThoughtMgr::removeThoughtsByThinker`
/// against the reimplemented `remove_thoughts_by_thinker`, on two identically-seeded standalone
/// managers. `thinker_ptr`/`object_ptr`/`habitat_ptr` are generated over a small `0..5` range so
/// `target` collides with a seeded value often enough to exercise real removals, not just the no-op
/// case.
pub(crate) fn run_thoughtmgr_remove_thoughts_by_thinker_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_THINKER";
    let mut fail_flag = false;

    match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5), |(specs, target)| {
        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
        seed_thoughts_raw(unsafe { &*real_ptr }, &specs);
        seed_thoughts(unsafe { &mut *reimpl_ptr }, &specs);

        unsafe {
            gen_ztthoughtmgr::REMOVE_THOUGHTS_BY_THINKER.original()(real_ptr as *const u32, target as *const u32);
        }
        unsafe { &mut *reimpl_ptr }.remove_thoughts_by_thinker(target);

        let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

        thought_live_support::free_raw_chain_mgr(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}", specs, target);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_OBJECT: same shape as
/// `run_thoughtmgr_remove_thoughts_by_thinker_test`, comparing `removeThoughtsByObject`/
/// `remove_thoughts_by_object` instead.
pub(crate) fn run_thoughtmgr_remove_thoughts_by_object_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_OBJECT";
    let mut fail_flag = false;

    match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5), |(specs, target)| {
        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
        seed_thoughts_raw(unsafe { &*real_ptr }, &specs);
        seed_thoughts(unsafe { &mut *reimpl_ptr }, &specs);

        unsafe {
            gen_ztthoughtmgr::REMOVE_THOUGHTS_BY_OBJECT.original()(real_ptr as *const u32, target as *const u32);
        }
        unsafe { &mut *reimpl_ptr }.remove_thoughts_by_object(target);

        let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

        thought_live_support::free_raw_chain_mgr(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}", specs, target);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_HABITAT: same shape again, additionally generating `force` -
/// `removeThoughtsByHabitat` has a third outcome `removeThoughtsBy{Thinker,Object}` don't: a
/// matching thought with a live `object_ptr` survives with its `habitat_ptr` link cleared instead of
/// being removed outright, unless `force` is set.
pub(crate) fn run_thoughtmgr_remove_thoughts_by_habitat_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_REMOVE_THOUGHTS_BY_HABITAT";
    let mut fail_flag = false;

    match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, any::<bool>()), |(specs, target, force)| {
        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
        seed_thoughts_raw(unsafe { &*real_ptr }, &specs);
        seed_thoughts(unsafe { &mut *reimpl_ptr }, &specs);

        unsafe {
            gen_ztthoughtmgr::REMOVE_THOUGHTS_BY_HABITAT.original()(real_ptr as *const u32, target as *const i32, force as i8);
        }
        unsafe { &mut *reimpl_ptr }.remove_thoughts_by_habitat(target, force);

        let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

        thought_live_support::free_raw_chain_mgr(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, force={}", specs, target, force);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_GET_THOUGHTS_BY_THINKER: compares the real, undetoured `getThoughtsByThinker`'s
/// output - a real vanilla temporary list, walked read-only via
/// `thought_live_support::read_raw_chain_from_sentinel` - against the reimplemented
/// `get_thoughts_by_thinker`, on a single standalone manager seeded on both representations via
/// `seed_thoughts_both` (the real call reads its raw `sentinel_ptr` chain; the reimplemented call
/// reads its `THOUGHT_STORES` entry - a single instance can drive both since they're independent
/// storage, no need for two separate instances).
pub(crate) fn run_thoughtmgr_get_thoughts_by_thinker_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_GET_THOUGHTS_BY_THINKER";
    let mut fail_flag = false;

    match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, 1i32..5), |(specs, target, max_count)| {
        let mgr_ptr = thought_live_support::build_standalone_mgr(1000);
        seed_thoughts_both(unsafe { &mut *mgr_ptr }, &specs);
        let mgr = unsafe { &*mgr_ptr };

        let mut real_sentinel: u32 = 0;
        unsafe {
            gen_ztthoughtmgr::GET_THOUGHTS_BY_THINKER.original()(
                mgr_ptr as *const u32,
                &raw mut real_sentinel as *const i32,
                target as *const i32,
                max_count,
            );
        }
        let real_fields: Vec<_> = thought_live_support::read_raw_chain_from_sentinel(real_sentinel).iter().map(thought_fields).collect();

        let reimpl_thoughts = mgr.get_thoughts_by_thinker(target, max_count as usize);
        let reimpl_fields: Vec<_> = reimpl_thoughts.iter().map(thought_fields).collect();

        thought_live_support::destroy_standalone_mgr_both(mgr_ptr);

        prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, max_count={}", specs, target, max_count);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_GET_THOUGHTS_BY_OBJECT: same shape as
/// `run_thoughtmgr_get_thoughts_by_thinker_test`, for `getThoughtsByObject`/`get_thoughts_by_object`.
/// `max_count` is passed as `max_count as *const i32` - `GET_THOUGHTS_BY_OBJECT`'s `*const i32`
/// signature is a Ghidra type-inference artifact; the real calling convention passes the count by
/// value for all three `getThoughtsBy*` functions.
pub(crate) fn run_thoughtmgr_get_thoughts_by_object_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_GET_THOUGHTS_BY_OBJECT";
    let mut fail_flag = false;

    match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, 1i32..5), |(specs, target, max_count)| {
        let mgr_ptr = thought_live_support::build_standalone_mgr(1000);
        seed_thoughts_both(unsafe { &mut *mgr_ptr }, &specs);
        let mgr = unsafe { &*mgr_ptr };

        let mut real_sentinel: u32 = 0;
        unsafe {
            gen_ztthoughtmgr::GET_THOUGHTS_BY_OBJECT.original()(
                mgr_ptr as *const u32,
                &raw mut real_sentinel as *const i32,
                target as *const i32,
                max_count as *const i32,
            );
        }
        let real_fields: Vec<_> = thought_live_support::read_raw_chain_from_sentinel(real_sentinel).iter().map(thought_fields).collect();

        let reimpl_thoughts = mgr.get_thoughts_by_object(target, max_count as usize);
        let reimpl_fields: Vec<_> = reimpl_thoughts.iter().map(thought_fields).collect();

        thought_live_support::destroy_standalone_mgr_both(mgr_ptr);

        prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, max_count={}", specs, target, max_count);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_GET_THOUGHTS_BY_HABITAT: same shape as
/// `run_thoughtmgr_get_thoughts_by_object_test`, including `max_count`'s own by-value passing - for
/// `getThoughtsByHabitat`/`get_thoughts_by_habitat`.
pub(crate) fn run_thoughtmgr_get_thoughts_by_habitat_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_GET_THOUGHTS_BY_HABITAT";
    let mut fail_flag = false;

    match runner.run(&(prop::collection::vec(thought_spec_strategy(), 0..8), 0u32..5, 1i32..20), |(specs, target, max_count)| {
        let mgr_ptr = thought_live_support::build_standalone_mgr(1000);
        seed_thoughts_both(unsafe { &mut *mgr_ptr }, &specs);
        let mgr = unsafe { &*mgr_ptr };

        let mut real_sentinel: u32 = 0;
        unsafe {
            gen_ztthoughtmgr::GET_THOUGHTS_BY_HABITAT.original()(
                mgr_ptr as *const u32,
                &raw mut real_sentinel as *const i32,
                target as *const i32,
                max_count as *const i32,
            );
        }
        let real_fields: Vec<_> = thought_live_support::read_raw_chain_from_sentinel(real_sentinel).iter().map(thought_fields).collect();

        let reimpl_thoughts = mgr.get_thoughts_by_habitat(target, max_count as usize);
        let reimpl_fields: Vec<_> = reimpl_thoughts.iter().map(thought_fields).collect();

        thought_live_support::destroy_standalone_mgr_both(mgr_ptr);

        prop_assert_eq!(real_fields, reimpl_fields, "mismatch for specs={:?}, target={}, max_count={}", specs, target, max_count);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_SAVE: compares the real `ZTThoughtMgr::save`'s captured output (via `io_redirect`)
/// against the reimplemented `save`, on two identically-seeded standalone managers.
pub(crate) fn run_thoughtmgr_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_SAVE";
    let mut fail_flag = false;

    let record_strategy = (any::<u32>(), any::<u32>(), any::<u32>(), -2i32..4, -2i32..4);
    match runner.run(&prop::collection::vec(record_strategy, 0..6), |records| {
        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
        for &(string_id, thinker_id, object_id, tile_x, tile_y) in &records {
            thought_live_support::seed_raw_chain(
                unsafe { &*real_ptr },
                thought_live_support::new_thought(string_id, thinker_id, object_id, tile_x, tile_y, 0, 0, 0),
            );
            unsafe { &mut *reimpl_ptr }.insert_front(thought_live_support::new_thought(string_id, thinker_id, object_id, tile_x, tile_y, 0, 0, 0));
        }

        let dummy_file: u32 = 0;
        io_redirect::begin_capture();
        unsafe { gen_ztthoughtmgr::SAVE.original()(real_ptr as *const u32, &dummy_file as *const u32) };
        let real_bytes = io_redirect::end_capture();

        io_redirect::begin_capture();
        let _ = unsafe { &*reimpl_ptr }.save(&dummy_file as *const u32);
        let reimpl_bytes = io_redirect::end_capture();

        thought_live_support::free_raw_chain_mgr(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(real_bytes, reimpl_bytes, "save byte mismatch for records={:?}", records);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_LOAD: compares the real `ZTThoughtMgr::load`'s effect on list content/order (and its
/// own return value) against the reimplemented `load`, for a generated stream of legacy-format
/// `(string_id, object_id, thinker_id)` records and `version < 0x1e` - the pre-`0x1e` legacy branch.
/// Restricted to this range: `version >= 0x1e` triggers `ZTThought::load`'s own
/// `thinker_id`/`object_id` -> pointer resolution via `ZTWorldMgr::resolve_entity_by_id`, which
/// needs `GLOBAL_ZTWorldMgr` initialized - not true yet at this early injection point.
/// `object_id`/`thinker_id` are generated over a small `0..3` range to land on both sides of
/// `ZTThoughtMgr::load`'s survival gate (a legacy-format record only splices into the list if both
/// ids end up `0`), not just the trivially-true `id == 0` case `any::<u32>()` would mostly hit.
pub(crate) fn run_thoughtmgr_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_LOAD";
    let mut fail_flag = false;

    let record_strategy = (any::<u32>(), 0u32..3, 0u32..3);
    match runner.run(&(prop::collection::vec(record_strategy, 0..6), 0u32..0x1e), |(records, version)| {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for &(string_id, object_id, thinker_id) in &records {
            bytes.extend_from_slice(&string_id.to_le_bytes());
            bytes.extend_from_slice(&object_id.to_le_bytes());
            bytes.extend_from_slice(&thinker_id.to_le_bytes());
        }

        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
        let file_buffer = [0u32; 4];

        io_redirect::begin_replay(bytes.clone());
        let real_ret = unsafe { gen_ztthoughtmgr::LOAD.original()(real_ptr as *const u32, file_buffer.as_ptr(), version) };
        io_redirect::end_replay();

        io_redirect::begin_replay(bytes);
        let reimpl_ret = unsafe { &mut *reimpl_ptr }.load(file_buffer.as_ptr(), version);
        io_redirect::end_replay();

        let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

        thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(real_ret, reimpl_ret, "load() return mismatch for records={:?}, version={}", records, version);
        prop_assert_eq!(real_fields, reimpl_fields, "loaded content mismatch for records={:?}, version={}", records, version);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_LOAD_MODERN: compares the real `ZTThoughtMgr::load`'s effect against the
/// reimplemented `load` for the `version >= 0x1e` branch - the branch `ZTTHOUGHTMGR_LOAD` above
/// never exercises, since it drives inline `ZTWorldMgr::resolve_entity_by_id`/
/// `ZTHabitatMgr::get_habitat_ptr` resolution that needs both globals initialized. Runs from
/// `always_late_tests()`, like `ZTTHOUGHTMGR_POPULATE_THOUGHTS`/`ZTTHOUGHT_GET_STRING` below.
///
/// `thinker_id`/`object_id` stay in the same small `0..5` range `ZTTHOUGHTMGR_POPULATE_THOUGHTS`
/// already uses (real entity ids are never this low in a fresh test process, so
/// `resolve_entity_by_id` deterministically returns null on both sides). Every record's tile is
/// fixed at the `(-1, -1)` "no tile" sentinel, not generated: `ZTHabitatMgr::get_habitat_ptr`
/// performs no bounds-checking against its own `other_array_start`/`other_array_end` fields, and at
/// this early injection point (before any zoo is loaded) that array is too small/empty for even
/// single-digit tile coordinates to stay in-bounds, so tile-based exercise of that path is deferred
/// rather than attempted here.
///
/// `truncate_at`, when `Some`, cuts the serialized byte stream short - folds in short-read coverage
/// `ZTTHOUGHTMGR_LOAD` doesn't have. `io_redirect::deallocate` just returns failure once the replay
/// buffer runs out, never touching out-of-bounds memory.
pub(crate) fn run_thoughtmgr_load_modern_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTHOUGHTMGR_LOAD_MODERN";

    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let mut fail_flag = false;

    let record_strategy = (any::<u32>(), 0u32..5, 0u32..5);
    let strategy = prop::collection::vec(record_strategy, 0..6).prop_flat_map(|records| {
        let total_len = 4 + records.len() * 20;
        (Just(records), 0x1eu32..0x40, prop::option::of(0usize..total_len))
    });

    match runner.run(&strategy, |(records, version, truncate_at)| {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&(records.len() as u32).to_le_bytes());
        for &(string_id, thinker_id, object_id) in &records {
            bytes.extend_from_slice(&string_id.to_le_bytes());
            bytes.extend_from_slice(&thinker_id.to_le_bytes());
            bytes.extend_from_slice(&object_id.to_le_bytes());
            bytes.extend_from_slice(&(-1i32 as u32).to_le_bytes());
            bytes.extend_from_slice(&(-1i32 as u32).to_le_bytes());
        }
        if let Some(cut) = truncate_at {
            bytes.truncate(cut);
        }

        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
        let file_buffer = [0u32; 4];

        io_redirect::begin_replay(bytes.clone());
        let real_ret = unsafe { gen_ztthoughtmgr::LOAD.original()(real_ptr as *const u32, file_buffer.as_ptr(), version) };
        io_redirect::end_replay();

        io_redirect::begin_replay(bytes);
        let reimpl_ret = unsafe { &mut *reimpl_ptr }.load(file_buffer.as_ptr(), version);
        io_redirect::end_replay();

        let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

        thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(
            real_ret,
            reimpl_ret,
            "load() return mismatch for records={:?}, version={}, truncate_at={:?}",
            records,
            version,
            truncate_at
        );
        prop_assert_eq!(
            real_fields,
            reimpl_fields,
            "loaded content mismatch for records={:?}, version={}, truncate_at={:?}",
            records,
            version,
            truncate_at
        );
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_ADD_THOUGHT_ANIMAL_OVERRIDE: exercises the animal-subtype override branch inside
/// `add_thought` (`resolve_object_own_habitat_ptr`'s two vtable calls) - `ZTAnimalType::isCastClass`
/// at `0x004020cd` and `ZTAnimal::getHabitat` at `0x00410685` (via `calcHabitat` ->
/// `ZTUnit::getHabitat`).
///
/// Both the "real" (`ADD_THOUGHT.original()`) and "reimplemented" (`add_thought`) sides dispatch
/// through the exact same real vanilla function pointers here - `resolve_object_own_habitat_ptr` is a
/// call-through wrapper around vanilla's own vtable slots, not reimplemented logic. So this test
/// isn't validating a separate vanilla habitat-resolution algorithm; it's validating that our
/// sequencing (vtable offsets, `this`/argument marshalling, override-vs-fallback logic) matches
/// vanilla's own `addThought.asm` exactly.
///
/// The fixture: a `ZTAnimal` (zeroed via `ZTAnimal::new_for_test`; entity type built via
/// `ZTAnimalType::new_for_test` so its vtable is the real `0x00630268` - slot `0x1c` resolves to
/// `isCastClass`) with its own vtable field overwritten to the real `ZTAnimal` vtable `0x0062ff54`
/// via a raw memory write, so slot `0x24c` resolves to `getHabitat`. `isCastClass` always returns
/// true when called on a genuine `ZTAnimalType`-vtabled object, so this fixture reliably exercises
/// the override-taken path, not the fallback.
///
/// `getHabitat`'s own chain reads `BFEntity::getTile()` (a real vanilla, non-virtual call) and a
/// show-info flag at a `BFUnit` offset; the fixture's zeroed base leaves that flag `0`, skipping the
/// `ZTShowMgr` branch, and `pos = (0, 0, 0)` is expected to miss real vanilla's own tile lookup at
/// this injection point (no zoo loaded yet).
pub(crate) fn run_thoughtmgr_add_thought_animal_override_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_ADD_THOUGHT_ANIMAL_OVERRIDE";
    let mut fail_flag = false;

    match runner.run(&(any::<u32>(), 0u32..5), |(string_id, fallback_habitat_ptr)| {
        let entity_type = ZTAnimalType::new_for_test(IVec3::default(), 0, IVec3::default(), IVec3::default());
        let animal = ZTAnimal::new_for_test(&entity_type as *const ZTAnimalType as u32, 0, 0, false, false);
        let animal_addr = &animal as *const ZTAnimal as u32;
        save_to_memory::<u32>(animal_addr, 0x0062_ff54u32);

        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);

        unsafe {
            gen_ztthoughtmgr::ADD_THOUGHT.original()(
                real_ptr as *const u32,
                string_id,
                std::ptr::null(),
                animal_addr as *const u32,
                fallback_habitat_ptr as *const u32,
            );
        }
        unsafe { &mut *reimpl_ptr }.add_thought(string_id, 0, animal_addr, fallback_habitat_ptr);

        let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

        thought_live_support::destroy_standalone_mgr_leaking_nodes(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(
            real_fields,
            reimpl_fields,
            "content mismatch for string_id={}, fallback_habitat_ptr={:#x}",
            string_id,
            fallback_habitat_ptr
        );
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_POPULATE_THOUGHTS: compares the real `ZTThoughtMgr::populateThoughts`'s effect on
/// every thought's resolved `thinker_ptr`/`object_ptr`/`habitat_ptr`/`tile_x`/`tile_y` against the
/// reimplemented `populate_thoughts`, on two identically-seeded standalone managers. Needs
/// `GLOBAL_ZTWorldMgr` initialized (`ZTThought::populate` calls `ZTWorldMgr::resolve_entity_by_id`
/// unconditionally), so this runs from `always_late_tests()`, not `early_tests()`.
/// `thinker_id`/`object_id` are generated over a small `0..5` range: real
/// entities essentially never have ids this low, so `resolve_entity_by_id` returns null on both
/// sides for the overwhelming majority of cases - a safe, deterministic "no match" - while still
/// leaving room for a genuine match.
pub(crate) fn run_thoughtmgr_populate_thoughts_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHTMGR_POPULATE_THOUGHTS";
    let mut fail_flag = false;

    let record_strategy = (any::<u32>(), 0u32..5, 0u32..5);
    match runner.run(&prop::collection::vec(record_strategy, 0..6), |records| {
        let real_ptr = thought_live_support::build_standalone_mgr(1000);
        let reimpl_ptr = thought_live_support::build_standalone_mgr(1000);
        for &(string_id, thinker_id, object_id) in &records {
            thought_live_support::seed_raw_chain(
                unsafe { &*real_ptr },
                thought_live_support::new_thought(string_id, thinker_id, object_id, -1, -1, 0, 0, 0),
            );
            unsafe { &mut *reimpl_ptr }.insert_front(thought_live_support::new_thought(string_id, thinker_id, object_id, -1, -1, 0, 0, 0));
        }

        unsafe { gen_ztthoughtmgr::POPULATE_THOUGHTS.original()(real_ptr as *const u32) };
        unsafe { &mut *reimpl_ptr }.populate_thoughts();

        let real_fields: Vec<_> = thought_live_support::read_raw_chain(unsafe { &*real_ptr }).iter().map(thought_fields).collect();
        let reimpl_fields: Vec<_> = unsafe { &*reimpl_ptr }.iter().map(|t| thought_fields(&t)).collect();

        thought_live_support::free_raw_chain_mgr(real_ptr);
        thought_live_support::destroy_standalone_mgr(reimpl_ptr);

        prop_assert_eq!(real_fields, reimpl_fields, "mismatch for records={:?}", records);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// Writes `name` into `entity`'s `name` field (`+0x108`, a `ZTBufferString`) by raw offset write.
/// `entity` must not move after this call - its address is baked into the write. Returns the
/// backing byte buffer, which the caller must keep alive at least as long as `entity` is read
/// through.
fn set_bfentity_name(entity: &BFEntity, name: &str) -> Vec<u8> {
    let mut encoded = name.as_bytes().to_vec();
    let len = encoded.len() as u32;
    encoded.push(0);
    let start = encoded.as_ptr() as u32;
    let entity_addr = entity as *const BFEntity as u32;
    save_to_memory::<u32>(entity_addr + 0x108, start);
    save_to_memory::<u32>(entity_addr + 0x10c, start + len);
    save_to_memory::<u32>(entity_addr + 0x110, start + encoded.len() as u32);
    encoded
}

/// Writes `name` into `habitat`'s `exhibit_name` field (`+0x154`, a 3-pointer `ZTBufferString`) by raw
/// offset write - same technique as `set_bfentity_name`. All three pointers must be written: a zeroed
/// `buffer_end_ptr` (`+0x15c`, always less than any real `start`) makes `ZTBufferString::copy_to_string`'s
/// `char_address < buffer_end` loop read zero bytes regardless of `name`'s actual content.
fn set_habitat_exhibit_name(habitat: &ZTHabitat, name: &str) -> Vec<u8> {
    let mut encoded = name.as_bytes().to_vec();
    let len = encoded.len() as u32;
    encoded.push(0);
    let start = encoded.as_ptr() as u32;
    let habitat_addr = habitat as *const ZTHabitat as u32;
    save_to_memory::<u32>(habitat_addr + 0x154, start);
    save_to_memory::<u32>(habitat_addr + 0x158, start + len);
    save_to_memory::<u32>(habitat_addr + 0x15c, start + encoded.len() as u32);
    encoded
}

/// Which of `get_string`'s three substitution branches a `ZTTHOUGHT_GET_STRING` case exercises
/// (priority: object, then habitat, then no substitution).
#[derive(Debug, Clone)]
enum GetStringSubstitution {
    None,
    Object(String),
    Habitat(String),
}

fn get_string_substitution_strategy() -> impl Strategy<Value = GetStringSubstitution> {
    prop_oneof![
        Just(GetStringSubstitution::None),
        "[a-zA-Z0-9 ]{0,16}".prop_map(GetStringSubstitution::Object),
        "[a-zA-Z0-9 ]{0,16}".prop_map(GetStringSubstitution::Habitat),
    ]
}

/// True if `template` is shaped the way every real, decompile-confirmed `ZTThought` message is: either
/// no `%` conversion at all, or exactly one `%s`. Real vanilla always calls `wsprintfA(dest, template,
/// name_ptr)` with exactly one argument when a substitution is attempted; this reimplementation's naive
/// `replacen("%s", name, 1)` only agrees with that for templates shaped this way - confirmed against
/// every real thought-message string id `ZTGuest::fGuestThought`'s call sites use (see
/// `run_thought_get_string_test`'s own doc comment).
fn is_single_percent_s_or_none(template: &str) -> bool {
    match template.matches('%').count() {
        0 => true,
        1 => template.contains("%s"),
        _ => false,
    }
}

/// ZTTHOUGHT_GET_STRING: compares the real `ZTThought::getString`'s output against the reimplemented
/// `get_string`, across all three substitution branches: no substitution (`object_ptr = habitat_ptr
/// = 0`), object-name substitution (a fixture `BFEntity` with its `name` field set via
/// `set_bfentity_name`), and habitat exhibit-name substitution (a fixture `ZTHabitat` with
/// `exhibit_name` set via `set_habitat_exhibit_name`). `get_string` only ever reads these two fields
/// directly (no vtable dispatch), so a zeroed fixture with just the name field populated is a safe,
/// complete stand-in for a real live `BFEntity`/`ZTHabitat`. Runs from
/// `always_late_tests()`: language DLLs, which
/// `load_string_by_id`/`BFApp::loadString` both depend on, aren't loaded yet at the early injection
/// point.
///
/// `string_id` is fuzzed unconstrained across `any::<u32>()`, which can land on a real, loadable
/// string that has nothing to do with `ZTThought` (e.g. a research-progress `"Months to complete:
/// %d"` or a marketing `"Adopt %d %r(s)."` string). Real vanilla's `getString` always calls
/// `wsprintfA` with exactly one variadic argument no matter what the template asks for, so it only
/// agrees with this reimplementation's `replacen("%s", name, 1)` when the template has zero `%`
/// conversions or exactly one `%s`. Every real `ZTThought` message is proven to be shaped that way:
/// the closed set of literal string-id constants baked into `zoo.exe` at `ZTGuest::fGuestThought`'s
/// call sites (`0x2758, 0x2759, 0x27fe, 0x2803, 0x2806, 0x2807, 0x280a, 0x282d, 0x2946, 0x2948,
/// 0x2972, 0x2974, 0x2975`) resolve, per the official string-table dumps, to templates that are
/// either plain text or exactly one `%s` - never `%d`, never a repeated `%s`. Below, resolved
/// templates outside that shape are discarded via `prop_assume!` rather than filtering `string_id`
/// itself, so the fuzzer/shrinker still exercises the full `u32` space and every id that does resolve
/// to a real `%s`-or-none template. This filtering is skipped for `GetStringSubstitution::None`:
/// when both `object_ptr` and `habitat_ptr` are null, real vanilla skips `wsprintfA` entirely and
/// returns the template untouched, and the reimplementation does the same, so both sides already
/// agree unconditionally there for any template shape.
///
/// Known accepted limitation: a few `fGuestThought` call sites (`ZTBuilding_addUser.c`'s `iVar10`,
/// `ZTGuest_consumeItem.c`'s `param_1[8]`, `ZTGuest_listen{,_maybe}.c`'s `uVar7`/`uVar4`) pass a
/// runtime-computed string id rather than a literal, so they aren't covered by the enumeration above.
/// `param_1[8]` in particular looks `.cfg`-driven, so a mod with a custom item config could in
/// principle point it at a non-`%s`-shaped string and hit a real (if narrow, mod-specific) divergence
/// this fix doesn't address.
pub(crate) fn run_thought_get_string_test(failure_log: &mut Option<std::fs::File>) -> bool {
    debug_assert!(!is_single_percent_s_or_none("Adopt %d %r(s)."));
    debug_assert!(!is_single_percent_s_or_none("Months to complete: %d"));

    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTTHOUGHT_GET_STRING";
    let mut fail_flag = false;

    match runner.run(&(any::<u32>(), get_string_substitution_strategy()), |(string_id, case)| {
        if !matches!(case, GetStringSubstitution::None)
            && let Some(template) = &crate::string_registry::load_string_by_id(string_id)
        {
            prop_assume!(is_single_percent_s_or_none(template));
        }

        let entity_storage = BFEntity::new_for_test(0, 0, 0);
        let habitat_storage: ZTHabitat = unsafe { std::mem::zeroed() };
        let _name_buf: Option<Vec<u8>>;

        let (object_ptr, habitat_ptr) = match &case {
            GetStringSubstitution::None => {
                _name_buf = None;
                (0u32, 0u32)
            }
            GetStringSubstitution::Object(name) => {
                _name_buf = Some(set_bfentity_name(&entity_storage, name));
                (&entity_storage as *const BFEntity as u32, 0u32)
            }
            GetStringSubstitution::Habitat(name) => {
                _name_buf = Some(set_habitat_exhibit_name(&habitat_storage, name));
                (0u32, &habitat_storage as *const ZTHabitat as u32)
            }
        };

        let thought = thought_live_support::new_thought(string_id, 0, 0, -1, -1, 0, object_ptr, habitat_ptr);

        let mut buffer = [0u32; 3];
        unsafe {
            gen_ztthought::GET_STRING.original()(&thought as *const ZTThought as *const u32, buffer.as_mut_ptr() as *const u32);
        }
        let real_text = get_from_memory::<ZTBufferString>(buffer.as_ptr() as u32).copy_to_string();
        let reimpl_text = thought.get_string();

        prop_assert_eq!(real_text, reimpl_text, "get_string mismatch for string_id={}, case={:?}", string_id, case);
        Ok(())
    }) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: {:?}\n", test_name, e).as_bytes());
            }
            fail_flag = true;
        }
    }

    fail_flag
}

/// ZTTHOUGHTMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE: real-zoo SAVE-only coverage for
/// `openzt/plans/real-zoo-save-load-roundtrip-tests-plan.md`'s `ZTThoughtMgr` item. The real zoo's
/// own thought list lives in real vanilla memory reachable through the live singleton's own
/// `sentinel_ptr` chain (this test build never installs `ztthoughtmgr`'s own detours, so real
/// vanilla `ZTThoughtMgr::load` populated that chain directly, never the Rust-side
/// `THOUGHT_STORES`) - read read-only via `thought_live_support::read_raw_chain`. Captures real
/// vanilla `save()`'s own output for the real singleton (`.original()`, undetoured here), then parses
/// those bytes independently and asserts every record matches the chain snapshot. Deliberately
/// SAVE-only, not a full round-trip - `ZTThoughtMgr::load`'s `version >= 0x1e` pointer-resolution
/// step can legitimately drop a record whose referenced object/thinker/habitat no longer resolves,
/// which isn't a bug (see the plan's own caution).
pub(crate) fn run_ztthoughtmgr_real_zoo_save_roundtrip_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTTHOUGHTMGR_REAL_ZOO_SAVE_ROUNDTRIP_LIVE";
    let mut fail_flag = false;

    /// One `ZTThoughtMgr::save` wire record: `(string_id, thinker_id, object_id, tile_x, tile_y)`.
    type SavedThoughtRecord = (u32, u32, u32, i32, i32);

    let mgr = globals().ztthoughtmgr();
    let expected: Vec<SavedThoughtRecord> = thought_live_support::read_raw_chain(mgr)
        .iter()
        .map(|t| (t.string_id(), t.thinker_id(), t.object_id(), t.tile_x(), t.tile_y()))
        .collect();
    if expected.is_empty() {
        info!("{}: no real thoughts active in the loaded zoo - nothing to round-trip, skipping", test_name);
        write_success_line(failure_log, &format!("{} (skipped: no real thoughts)", test_name));
        return false;
    }

    let real_ptr = globals().ztthoughtmgr_ptr() as *const u32;
    let dummy_file: u32 = 0;
    io_redirect::begin_capture();
    let save_ok = unsafe { gen_ztthoughtmgr::SAVE.original()(real_ptr, &dummy_file as *const u32) };
    let captured_bytes = io_redirect::end_capture();

    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("CHECKPOINT {} thoughts={} bytes={}\n", test_name, expected.len(), captured_bytes.len()).as_bytes());
    }

    if !save_ok {
        error!("{}: real vanilla save() returned failure", test_name);
        fail_flag = true;
    }

    fn parse(bytes: &[u8]) -> Option<Vec<SavedThoughtRecord>> {
        if bytes.len() < 4 {
            return None;
        }
        let count = u32::from_le_bytes(bytes[0..4].try_into().unwrap()) as usize;
        let mut offset = 4;
        let mut records = Vec::with_capacity(count);
        for _ in 0..count {
            if offset + 20 > bytes.len() {
                return None;
            }
            let read_u32 = |o: usize| u32::from_le_bytes(bytes[o..o + 4].try_into().unwrap());
            let string_id = read_u32(offset);
            let thinker_id = read_u32(offset + 4);
            let object_id = read_u32(offset + 8);
            let tile_x = read_u32(offset + 12) as i32;
            let tile_y = read_u32(offset + 16) as i32;
            records.push((string_id, thinker_id, object_id, tile_x, tile_y));
            offset += 20;
        }
        Some(records)
    }

    match parse(&captured_bytes) {
        Some(parsed) if parsed == expected => {}
        other => {
            error!("{}: parsed real save bytes don't match the real chain snapshot", test_name);
            if let Some(log_file) = failure_log {
                let _ = log_file.write_all(format!("Test Failed {}: expected={:?}\nparsed={:?}\n", test_name, expected, other).as_bytes());
            }
            fail_flag = true;
        }
    }

    if !fail_flag {
        info!("{}: {} real thought(s) round-tripped byte-identically through save()", test_name, expected.len());
        write_success_line(failure_log, test_name);
    } else if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("Test Failed {}\n", test_name).as_bytes());
    }
    fail_flag
}
