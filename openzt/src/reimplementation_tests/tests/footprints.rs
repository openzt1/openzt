//! GET_FOOTPRINT-family comparisons for `BFEntity`/`ZTUnit`/`ZTAnimal` (production file
//! `openzt/src/ztworldmgr.rs`): synthetic entity/type fixtures, real vanilla `.original()` calls
//! vs. the reimplemented `get_footprint` - early-battery tests, no live zoo needed.

use openzt_detour::FunctionDef;
use openzt_detour::generated::bfentity::GET_FOOTPRINT as BFENTITY_GET_FOOTPRINT;
use openzt_detour::generated::ztanimal::GET_FOOTPRINT as ZTANIMAL_GET_FOOTPRINT;
use openzt_detour::generated::ztunit::GET_FOOTPRINT as ZTUNIT_GET_FOOTPRINT;
use proptest::prelude::*;
use std::io::Write;
use tracing::{error, info};

use crate::bfentitytype::{BFEntityType, ZTAnimalType, ZTUnitType};
use crate::reimplementation_tests::NoopFailurePersistence;
use crate::reimplementation_tests::harness::write_success_line;
use crate::ztworldmgr::{BFEntity, IVec3, ZTAnimal, ZTUnit};

/// Calls the real GET_FOOTPRINT function at `entity_ptr` and reads the `IVec3` it writes back.
fn call_original_get_footprint(
    original: FunctionDef<unsafe extern "thiscall" fn(*const u32, *const u32, bool) -> *const u32>,
    entity_ptr: *const u32,
    use_map_footprint: bool,
) -> IVec3 {
    let mut result = IVec3::default();
    unsafe {
        (original.original())(entity_ptr, &raw mut result as *const u32, use_map_footprint);
    }
    result
}

pub(crate) fn run_bfentity_get_footprint_tests(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "BFENTITY_GET_FOOTPRINT";
    let mut fail_flag = false;

    match runner.run(
        &(-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32, -8i32..8i32, proptest::bool::ANY),
        |(fx, fy, fz, rotation, use_map_footprint)| {
            let mut entity_type: BFEntityType = unsafe { std::mem::zeroed() };
            entity_type.footprintx = fx;
            entity_type.footprinty = fy;
            entity_type.footprintz = fz;

            let entity = BFEntity::new_for_test(&raw const entity_type as u32, rotation, 0);

            let reimplemented_result = entity.get_footprint(use_map_footprint);
            let real_result = call_original_get_footprint(BFENTITY_GET_FOOTPRINT, &raw const entity as *const u32, use_map_footprint);

            assert_eq!(
                (real_result.x, real_result.y, real_result.z),
                (reimplemented_result.x, reimplemented_result.y, reimplemented_result.z),
                "BFEntity::get_footprint mismatch: fx={}, fy={}, fz={}, rotation={}, use_map_footprint={}, real={:?}, reimplemented={:?}",
                fx, fy, fz, rotation, use_map_footprint, real_result, reimplemented_result
            );
            Ok(())
        },
    ) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let proptest::test_runner::TestError::Fail(r, (fx, fy, fz, rotation, use_map_footprint)) = e {
                let failure_line = format!(
                    "{}: fx={}, fy={}, fz={}, rotation={}, use_map_footprint={}, reason={}\n",
                    test_name, fx, fy, fz, rotation, use_map_footprint, r
                );
                if let Some(log_file) = failure_log
                    && let Err(write_err) = log_file.write_all(failure_line.as_bytes())
                {
                    error!("Failed to write to failure log: {}", write_err);
                }
                fail_flag = true;
            }
        }
    }

    fail_flag
}

/// `ZTUnit::getFootprint`'s `use_map_footprint=true` branch virtual-dispatches through
/// `entity_type`'s vtable (not `this`'s own), so the fixture's `ZTUnitType` needs a real
/// vtable pointer - see `ZTUnitType::new_for_test` / `ztunit-ztanimal-footprint-crash-investigation.md`.
pub(crate) fn run_ztunit_get_footprint_tests(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTUNIT_GET_FOOTPRINT";
    let mut fail_flag = false;

    match runner.run(
        &(
            -1000i32..1000i32,
            -1000i32..1000i32,
            -1000i32..1000i32,
            -1000i32..1000i32,
            -8i32..8i32,
            proptest::bool::ANY,
        ),
        |(fx, fy, fz, map_footprint, rotation, use_map_footprint)| {
            let entity_type = ZTUnitType::new_for_test(IVec3::new(fx, fy, fz), map_footprint);
            let entity = ZTUnit::new_for_test(&raw const entity_type as u32, rotation, 0);

            let reimplemented_result = entity.get_footprint(use_map_footprint);
            let real_result = call_original_get_footprint(ZTUNIT_GET_FOOTPRINT, &raw const entity as *const u32, use_map_footprint);

            assert_eq!(
                (real_result.x, real_result.y, real_result.z),
                (reimplemented_result.x, reimplemented_result.y, reimplemented_result.z),
                "ZTUnit::get_footprint mismatch: fx={}, fy={}, fz={}, map_footprint={}, rotation={}, use_map_footprint={}, real={:?}, reimplemented={:?}",
                fx, fy, fz, map_footprint, rotation, use_map_footprint, real_result, reimplemented_result
            );
            Ok(())
        },
    ) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let proptest::test_runner::TestError::Fail(r, (fx, fy, fz, map_footprint, rotation, use_map_footprint)) = e {
                let failure_line = format!(
                    "{}: fx={}, fy={}, fz={}, map_footprint={}, rotation={}, use_map_footprint={}, reason={}\n",
                    test_name, fx, fy, fz, map_footprint, rotation, use_map_footprint, r
                );
                if let Some(log_file) = failure_log
                    && let Err(write_err) = log_file.write_all(failure_line.as_bytes())
                {
                    error!("Failed to write to failure log: {}", write_err);
                }
                fail_flag = true;
            }
        }
    }

    fail_flag
}

/// `ZTAnimal::getFootprint`'s `is_egg`/`is_boxed` branches virtual-dispatch through
/// `entity_type`'s vtable unconditionally (regardless of `use_map_footprint`) - same fixture
/// requirement as `ZTUnit` above.
pub(crate) fn run_ztanimal_get_footprint_tests(failure_log: &mut Option<std::fs::File>) -> bool {
    let runner_config = ProptestConfig {
        failure_persistence: Some(Box::new(NoopFailurePersistence)),
        ..ProptestConfig::default()
    };
    let mut runner = proptest::test_runner::TestRunner::new(runner_config);
    let test_name = "ZTANIMAL_GET_FOOTPRINT";
    let mut fail_flag = false;

    match runner.run(
        &(
            (-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32),
            -1000i32..1000i32,
            (-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32),
            (-1000i32..1000i32, -1000i32..1000i32, -1000i32..1000i32),
            -8i32..8i32,
            proptest::bool::ANY,
            proptest::bool::ANY,
            proptest::bool::ANY,
        ),
        |((fx, fy, fz), map_footprint, box_footprint, egg_footprint, rotation, use_map_footprint, is_egg, is_boxed)| {
            let entity_type = ZTAnimalType::new_for_test(
                IVec3::new(fx, fy, fz),
                map_footprint,
                IVec3::new(box_footprint.0, box_footprint.1, box_footprint.2),
                IVec3::new(egg_footprint.0, egg_footprint.1, egg_footprint.2),
            );
            let entity = ZTAnimal::new_for_test(&raw const entity_type as u32, rotation, 0, is_egg, is_boxed);

            let reimplemented_result = entity.get_footprint(use_map_footprint);
            let real_result = call_original_get_footprint(ZTANIMAL_GET_FOOTPRINT, &raw const entity as *const u32, use_map_footprint);

            assert_eq!(
                (real_result.x, real_result.y, real_result.z),
                (reimplemented_result.x, reimplemented_result.y, reimplemented_result.z),
                "ZTAnimal::get_footprint mismatch: is_egg={}, is_boxed={}, rotation={}, use_map_footprint={}, real={:?}, reimplemented={:?}",
                is_egg, is_boxed, rotation, use_map_footprint, real_result, reimplemented_result
            );
            Ok(())
        },
    ) {
        Ok(_) => {
            info!("Proptest passed for {}", test_name);
            write_success_line(failure_log, test_name);
        }
        Err(e) => {
            error!("Proptest failed: {:?}", e);
            if let proptest::test_runner::TestError::Fail(r, ((fx, fy, fz), map_footprint, box_footprint, egg_footprint, rotation, use_map_footprint, is_egg, is_boxed)) = e {
                let failure_line = format!(
                    "{}: fx={}, fy={}, fz={}, map_footprint={}, box_footprint={:?}, egg_footprint={:?}, rotation={}, use_map_footprint={}, is_egg={}, is_boxed={}, reason={}\n",
                    test_name, fx, fy, fz, map_footprint, box_footprint, egg_footprint, rotation, use_map_footprint, is_egg, is_boxed, r
                );
                if let Some(log_file) = failure_log
                    && let Err(write_err) = log_file.write_all(failure_line.as_bytes())
                {
                    error!("Failed to write to failure log: {}", write_err);
                }
                fail_flag = true;
            }
        }
    }

    fail_flag
}
