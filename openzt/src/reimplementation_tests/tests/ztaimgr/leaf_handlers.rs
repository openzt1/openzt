//! Leaf manager methods and the building/trick/routine handler wrappers.

use super::*;

/// `ZTAIMGR_DETOURS_ENABLED`: asserts every `ZTAIMgr` detour reports enabled, so a silently failed
/// `init_detours()` cannot leave the comparisons below running vanilla against vanilla.
pub(crate) fn run_ztaimgr_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

/// `ZTAIMGR_GET_RANDOM_XY`: guest/staff/other entities against a fake dword block.
pub(crate) fn run_ztaimgr_get_random_xy_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_GET_RANDOM_XY";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    for kind in ALL_KINDS {
        let entity = fixture.entity(kind);
        let (rust_x, real_x) = unsafe { (GET_RANDOM_X.hooked()(fixture.this(), entity), live_support::real_get_random_x(fixture.this(), entity)) };
        let (rust_y, real_y) = unsafe { (GET_RANDOM_Y.hooked()(fixture.this(), entity), live_support::real_get_random_y(fixture.this(), entity)) };
        if rust_x != real_x {
            failures.push(format!("{kind:?}: getRandomX rust {rust_x:#x} != real {real_x:#x}"));
        }
        if rust_y != real_y {
            failures.push(format!("{kind:?}: getRandomY rust {rust_y:#x} != real {real_y:#x}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_MOVE_FUNCTIONS`: every move-function name, plus `fMove` with each `walk`/`trot`/`run` variant
/// as the third parameter (always at least three parameters: vanilla reads that slot unchecked).
pub(crate) fn run_ztaimgr_move_functions_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_MOVE_FUNCTIONS";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    let names = [
        "fMoveSlow", "fMoveMedium", "fMoveFast", "fWalk", "fTrot", "fRun", "fMoveReverse", "fMoveSlowX", "fmove", "", "fPlay",
    ];
    let variants = ["walk", "trot", "run", "gallop", "", "Walk"];
    let mut calls = Vec::new();
    for name in names {
        calls.push(Call::new(name, &[]));
    }
    for variant in variants {
        calls.push(Call::new("fMove", &["a", "b", variant]));
        calls.push(Call::new("fMove", &["walk", "trot", variant, "run"]));
    }
    for call in &calls {
        let this = fixture.this();
        let pairs = unsafe {
            [
                ("slow", IS_SLOW_MOVE_FUNCTION.hooked()(this, call.ptr()), live_support::real_is_slow_move_function(this, call.ptr())),
                ("medium", IS_MEDIUM_MOVE_FUNCTION.hooked()(this, call.ptr()), live_support::real_is_medium_move_function(this, call.ptr())),
                ("fast", IS_FAST_MOVE_FUNCTION.hooked()(this, call.ptr()), live_support::real_is_fast_move_function(this, call.ptr())),
            ]
        };
        for (which, rust, real) in pairs {
            if rust != real {
                failures.push(format!("is{which}MoveFunction call {:#x}: rust {rust} != real {real}", call.ptr() as u32));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

pub(super) type PoleOutcome<R> = (R, bool, (u32, u32));

/// Runs `pole` on `unit` with a fresh `done` flag, returning `(result, done, (virtual log, last this))`.
pub(super) fn run_pole<R>(unit: *const u32, pole: impl FnOnce(*const u32, *const bool) -> R) -> PoleOutcome<R> {
    reset_virtual_log();
    let done = Box::new(false);
    let result = pole(unit, &*done as *const bool);
    (result, *done, virtual_log())
}

/// `ZTAIMGR_BUILDING_AND_TRICK_HANDLERS`: `fEndTrick` over every entity kind and a null unit, and
/// `fEnterBuilding`/`fReturnToBuilding` over every unit kind, against the real bodies. The real
/// `fEnterBuilding`/`fReturnToBuilding` fault on a null or non-unit target, so there the test asserts only that
/// the port defers without acting. The routine handlers are compared only for targets that are not exactly
/// their class (the real `fRoutine` bodies are not safe to run on fake entities).
pub(crate) fn run_ztaimgr_building_and_trick_handlers_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BUILDING_AND_TRICK_HANDLERS";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    let null = std::ptr::null::<u32>();

    let mut targets: Vec<(String, Option<Kind>, *const u32)> = ALL_KINDS.iter().map(|k| (format!("{k:?}"), Some(*k), fixture.entity(*k))).collect();
    targets.push(("null".to_string(), None, null));

    for (label, kind, unit) in &targets {
        let rust = run_pole(*unit, |u, d| unsafe { F_END_TRICK.hooked()(u, d) });
        let real = run_pole(*unit, live_support::real_f_end_trick);
        if rust != real {
            failures.push(format!("fEndTrick {label}: rust {rust:?} != real {real:?}"));
        }

        if kind.is_some_and(|k| k != Kind::Scenery) {
            let rust = run_pole(*unit, |u, d| {
                unsafe { F_ENTER_BUILDING.hooked()(u, d) };
                true
            });
            let real = run_pole(*unit, |u, d| {
                live_support::real_f_enter_building(u, d);
                true
            });
            if rust != real {
                failures.push(format!("fEnterBuilding {label}: rust {rust:?} != real {real:?}"));
            }
            let rust = run_pole(*unit, |u, d| {
                unsafe { F_RETURN_TO_BUILDING.hooked()(u, d) };
                true
            });
            let real = run_pole(*unit, |u, d| {
                live_support::real_f_return_to_building(u, d);
                true
            });
            if rust != real {
                failures.push(format!("fReturnToBuilding {label}: rust {rust:?} != real {real:?}"));
            }
        } else {
            let enter = run_pole(*unit, |u, d| unsafe { live_support::f_enter_building(u as u32, d) });
            let ret = run_pole(*unit, |u, d| unsafe { live_support::f_return_to_building(u as u32, d) });
            for (name, outcome) in [("fEnterBuilding", enter), ("fReturnToBuilding", ret)] {
                if outcome != (false, false, (0, 0)) {
                    failures.push(format!("{name} {label}: expected the port to defer untouched, got {outcome:?}"));
                }
            }
        }

        // The fake types never report `ZTAnimal`/`ZTGuest`-for-castclass, so these take vanilla's
        // non-animal paths, which are safe to run for real.
        if kind.is_some_and(|k| k != Kind::Scenery) {
            let rust = run_pole(*unit, |u, d| {
                unsafe { F_EXIT_BUILDING.hooked()(u as *const i32, d as *const u8) };
                true
            });
            let real = run_pole(*unit, |u, d| {
                live_support::real_f_exit_building(u as *const i32, d as *const u8);
                true
            });
            if rust != real {
                failures.push(format!("fExitBuilding {label}: rust {rust:?} != real {real:?}"));
            }
        } else {
            let outcome = run_pole(*unit, |u, d| unsafe { live_support::f_exit_building(u as u32, d) });
            if outcome != (false, false, (0, 0)) {
                failures.push(format!("fExitBuilding {label}: expected the port to defer untouched, got {outcome:?}"));
            }
        }
        let rust = run_pole(*unit, |u, d| unsafe { F_KILL_PREY.hooked()(u as *const u8, d as *const u8) });
        let real = run_pole(*unit, |u, d| live_support::real_f_kill_prey(u as *const u8, d as *const u8));
        if (rust.0 & 0xff, rust.1, rust.2) != (real.0 & 0xff, real.1, real.2) {
            failures.push(format!("fKillPrey {label}: rust {rust:?} != real {real:?}"));
        }
        let rust = run_pole(*unit, |u, d| unsafe { F_CAUGHT_PREY.hooked()(u as *const i32, d as *const u8) });
        let real = run_pole(*unit, |u, d| live_support::real_f_caught_prey(u as *const i32, d as *const u8));
        if (rust.0 & 0xff, rust.1, rust.2) != (real.0 & 0xff, real.1, real.2) {
            failures.push(format!("fCaughtPrey {label}: rust {rust:?} != real {real:?}"));
        }
        let rust = run_pole(*unit, |u, d| unsafe { F_DIE.hooked()(u as *const i32, d as *const u8) });
        let real = run_pole(*unit, |u, d| live_support::real_f_die(u as *const i32, d as *const u8));
        if (rust.0 & 0xff, rust.1, rust.2) != (real.0 & 0xff, real.1, real.2) {
            failures.push(format!("fDie {label}: rust {rust:?} != real {real:?}"));
        }
        let rust = run_pole(*unit, |u, d| unsafe { F_FACE_TOWARD_FOOD.hooked()(u as *const u8, d as *const u8) });
        let real = run_pole(*unit, |u, d| live_support::real_f_face_toward_food(u as *const u8, d as *const u8));
        if (rust.0 & 0xff, rust.1, rust.2) != (real.0 & 0xff, real.1, real.2) {
            failures.push(format!("fFaceTowardFood {label}: rust {rust:?} != real {real:?}"));
        }
        // Only a non-unit, non-staff/guest target avoids the real `faceTowardTarget` helpers.
        if matches!(kind, None | Some(Kind::Scenery)) {
            let rust = run_pole(*unit, |u, d| unsafe { F_FACE_TOWARD_TARGET.hooked()(u as *const std::ffi::c_void, d as *const u8) });
            let real = run_pole(*unit, |u, d| live_support::real_f_face_toward_target(u as *const std::ffi::c_void, d as *const u8));
            if (rust.0 & 0xff, rust.1, rust.2) != (real.0 & 0xff, real.1, real.2) {
                failures.push(format!("fFaceTowardTarget {label}: rust {rust:?} != real {real:?}"));
            }
        }
        let predator = run_pole(*unit, |u, d| unsafe { live_support::f_show_predator(u as u32, d) });
        let hatch = run_pole(*unit, |u, d| unsafe { live_support::f_hatch(u as u32, d) });
        if predator != (None, false, (0, 0)) || hatch != (false, false, (0, 0)) {
            failures.push(format!("{label}: show-predator/hatch should defer untouched, got {predator:?} / {hatch:?}"));
        }

        let is_guest = *kind == Some(Kind::Guest);
        if !is_guest {
            let rust = run_pole(*unit, |u, d| unsafe { F_GUEST_ROUTINE.hooked()(u, d) });
            let real = run_pole(*unit, live_support::real_f_guest_routine);
            if rust != real {
                failures.push(format!("fGuestRoutine {label}: rust {rust:?} != real {real:?}"));
            }
        }
        let rust = run_pole(*unit, |u, d| unsafe { F_MAINT_ROUTINE.hooked()(u, d) });
        let real = run_pole(*unit, live_support::real_f_maint_routine);
        if rust != real {
            failures.push(format!("fMaintRoutine {label}: rust {rust:?} != real {real:?}"));
        }
        let rust = run_pole(*unit, |u, d| unsafe { F_GUIDE_ROUTINE.hooked()(u, d) });
        let real = run_pole(*unit, live_support::real_f_guide_routine);
        if rust != real {
            failures.push(format!("fGuideRoutine {label}: rust {rust:?} != real {real:?}"));
        }
        let rust = run_pole(*unit, |u, d| unsafe { F_KEEPER_ROUTINE.hooked()(u, d) });
        let real = run_pole(*unit, live_support::real_f_keeper_routine);
        if rust != real {
            failures.push(format!("fKeeperRoutine {label}: rust {rust:?} != real {real:?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_WRAPPER_RETURNS`: each `ZTAIMgr::f*F` wrapper returns its handler's `AL` in vanilla (a tail call),
/// and `ZTAIMgr::executeCall` passes that on. Compares the port's wrapper return against the real inner
/// handler's return over every unit kind and a null unit, for the handlers whose real body is safe on the fake
/// entities (the building, hatch, show/hide-predator and dust-ball handlers are excluded: their real bodies
/// fault on a non-unit, or their inner is typed `void`). `fSprayF` forwards to a `return 1` stub.
pub(crate) fn run_ztaimgr_f_wrapper_returns_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztaimgr::{
        F_CAUGHT_PREY_F, F_DIE_F, F_END_TRICK_F, F_FACE_TOWARD_FOOD_F, F_FACE_TOWARD_TARGET_F, F_GUEST_ROUTINE_F, F_GUIDE_ROUTINE_F, F_KEEPER_ROUTINE_F, F_KILL_PREY_F, F_MAINT_ROUTINE_F, F_SPRAY_F,
    };
    let test_name = "ZTAIMGR_F_WRAPPER_RETURNS";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    let mut targets: Vec<(String, Option<Kind>, *const u32)> = ALL_KINDS.iter().map(|k| (format!("{k:?}"), Some(*k), fixture.entity(*k))).collect();
    targets.push(("null".to_string(), None, std::ptr::null()));

    for (label, kind, unit) in &targets {
        let mut check = |name: &str, wrapper: bool, real: u8| {
            if wrapper as u8 != real {
                failures.push(format!("{name} {label}: wrapper returned {wrapper}, real inner {real:#x}"));
            }
        };
        let call = Call::new("f", &[]);
        let c = call.ptr() as i32;
        let u = *unit as u32;

        check("fEndTrickF", unsafe { F_END_TRICK_F.hooked()(c, *unit as *const i32) }, run_pole(*unit, live_support::real_f_end_trick).0 as u8);
        check("fKeeperRoutineF", unsafe { F_KEEPER_ROUTINE_F.hooked()(c as u32, *unit as *const i32) }, run_pole(*unit, live_support::real_f_keeper_routine).0 as u8);
        if *kind != Some(Kind::Guest) {
            check("fGuestRoutineF", unsafe { F_GUEST_ROUTINE_F.hooked()(c as u32, u) }, run_pole(*unit, live_support::real_f_guest_routine).0 as u8);
        }
        check("fMaintRoutineF", unsafe { F_MAINT_ROUTINE_F.hooked()(c as u32, u) }, run_pole(*unit, live_support::real_f_maint_routine).0 as u8);
        check("fGuideRoutineF", unsafe { F_GUIDE_ROUTINE_F.hooked()(c as u32, u) }, run_pole(*unit, live_support::real_f_guide_routine).0 as u8);
        check(
            "fKillPreyF",
            unsafe { F_KILL_PREY_F.hooked()(c, *unit as *const u8) },
            run_pole(*unit, |u, d| live_support::real_f_kill_prey(u as *const u8, d as *const u8)).0 as u8,
        );
        check(
            "fCaughtPreyF",
            unsafe { F_CAUGHT_PREY_F.hooked()(c, *unit as *const i32) },
            run_pole(*unit, |u, d| live_support::real_f_caught_prey(u as *const i32, d as *const u8)).0 as u8,
        );
        check("fDieF", unsafe { F_DIE_F.hooked()(c, *unit as *const i32) }, run_pole(*unit, |u, d| live_support::real_f_die(u as *const i32, d as *const u8)).0 as u8);
        check(
            "fFaceTowardFoodF",
            unsafe { F_FACE_TOWARD_FOOD_F.hooked()(c, *unit as *const u8) },
            run_pole(*unit, |u, d| live_support::real_f_face_toward_food(u as *const u8, d as *const u8)).0 as u8,
        );
        if matches!(kind, None | Some(Kind::Scenery)) {
            check(
                "fFaceTowardTargetF",
                unsafe { F_FACE_TOWARD_TARGET_F.hooked()(c, *unit as *const std::ffi::c_void) },
                run_pole(*unit, |u, d| live_support::real_f_face_toward_target(u as *const std::ffi::c_void, d as *const u8)).0 as u8,
            );
        }
        check("fSprayF", unsafe { F_SPRAY_F.hooked()(c, *unit) }, 1);
    }
    finish_test(test_name, failures, failure_log)
}
