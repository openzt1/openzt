//! `executeCall` dispatch chains and `convertConstantParam`.

use super::*;

/// `ZTAIMGR_EXECUTE_CALL`: the real dispatch chain against the port for the names whose real handler is safe
/// on a null or scenery unit (the cast unit is then null for every ZT handler), plus names that fall through
/// the whole `BFAIMgr` chain. Compares the returned `AL`, the `done` byte, the call's own flag byte and
/// entity-virtual calls.
pub(crate) fn run_ztaimgr_execute_call_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_EXECUTE_CALL";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    let names = [
        "fSpray", "fKeeperRoutine", "fGuestRoutine", "fGuideRoutine", "fMaintRoutine", "fFaceTowardFood", "fFaceTowardTarget", "fKillPrey", "fCaughtPrey", "fDie", "fEndTrick",
        "fKeepControl", "fNoSuchFunction", "", "fdie", "fDieX", "fDi",
    ];
    for name in names {
        for (label, unit) in [("null", std::ptr::null::<u32>()), ("scenery", fixture.entity(Kind::Scenery))] {
            let mut outcomes = Vec::new();
            for real in [false, true] {
                let call = Call::new(name, &[]);
                reset_virtual_log();
                let done = Box::new(0xabu8);
                let (call_ptr, unit_ptr, done_ptr) = (call.ptr() as *const i8, unit as *const i32, &*done as *const u8);
                let returned = if real {
                    live_support::real_execute_call(fixture.this(), call_ptr, unit_ptr, done_ptr)
                } else {
                    unsafe { EXECUTE_CALL.hooked()(fixture.this(), call_ptr, unit_ptr, done_ptr) }
                };
                outcomes.push((returned, *done, call.raw[6] & 0xff, virtual_log()));
            }
            if outcomes[0] != outcomes[1] {
                failures.push(format!("{name:?} {label}: rust {:?} != real {:?}", outcomes[0], outcomes[1]));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BASE_EXECUTE_CALL`: the real `BFAIMgr::executeCall` against the port, for calls whose real handler
/// is safe on a fake unit: `fKeepControl` (inline, depends only on the cast unit), a parameterless `fPlay` (the
/// handler returns before touching the unit), and unknown/near-miss names. Compares the returned byte, the
/// `done` byte and the call's flag byte.
pub(crate) fn run_ztaimgr_base_execute_call_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BASE_EXECUTE_CALL";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    let names = ["fKeepControl", "fPlay", "fNoSuchFunction", "", "fKeepControlX", "fkeepcontrol", "fPla"];
    let mut units: Vec<(String, *const u32)> = ALL_KINDS.iter().map(|k| (format!("{k:?}"), fixture.entity(*k))).collect();
    units.push(("null".to_string(), std::ptr::null()));
    for name in names {
        for (label, unit) in &units {
            let mut outcomes = Vec::new();
            for real in [false, true] {
                let call = Call::new(name, &[]);
                let done = Box::new(0xabu8);
                let (call_ptr, unit_ptr, done_ptr) = (call.ptr() as *const i8, *unit as *const i32, &*done as *const u8);
                let result = if real {
                    live_support::real_base_execute_call(fixture.this(), call_ptr, unit_ptr, done_ptr)
                } else {
                    unsafe { openzt_detour::generated::bfaimgr::EXECUTE_CALL.hooked()(fixture.this(), call_ptr, unit_ptr, done_ptr) }
                };
                outcomes.push((result & 0xff, *done, call.raw[6] & 0xff));
            }
            if outcomes[0] != outcomes[1] {
                failures.push(format!("{name:?} {label}: rust {:?} != real {:?}", outcomes[0], outcomes[1]));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_CONVERT_CONSTANT_PARAM`: the eight named constants for every entity kind that vanilla can
/// survive (a named walk/run constant on a non-unit type dereferences null in vanilla), plus names that
/// fall through to the numeric parse.
pub(crate) fn run_ztaimgr_convert_constant_param_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_CONVERT_CONSTANT_PARAM";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    let walk_run = [
        "cWalkRandomlyTime1", "cWalkRandomlyTime2", "cWalkRandomlyTime3", "cRunRandomlyTime1", "cRunRandomlyTime2", "cRunRandomlyTime3",
    ];
    let numeric = ["0", "7", "-3", "250", "12abc", "cWalkRandomlyTime4", "cInformGuestTime2", ""];

    for kind in ALL_KINDS {
        let entity = fixture.entity(kind);
        let mut names: Vec<&str> = numeric.to_vec();
        names.push("cInformGuestTime");
        if kind != Kind::Scenery {
            names.extend(walk_run);
        }
        for name in names {
            let text = CallString::new(name);
            let name_ptr = text.raw.as_ptr() as *const std::ffi::c_void;
            let rust = unsafe { CONVERT_CONSTANT_PARAM.hooked()(fixture.this(), name_ptr, entity) };
            let real = live_support::real_convert_constant_param(fixture.this(), name_ptr, entity);
            if rust != real {
                failures.push(format!("{kind:?} {name:?}: rust {rust:#x} != real {real:#x}"));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
