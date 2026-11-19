//! `fPlaySetTank`, `initAIParams`, `BFAIMgr::load` and the listener overrides.

use super::*;

/// `ZTAIMGR_F_PLAY_SET_TANK`: calls with a parameter count other than two must behave exactly like vanilla (return
/// `0`, leave the call alone) for every unit kind. With two parameters the port picks the last parameter when
/// the unit's selector is `4` and the first otherwise, clears the call's result flag and passes the unit; a
/// non-unit target defers. The play itself is a recording fake, since the real one needs live animation state.
pub(crate) fn run_ztaimgr_f_play_set_tank_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::ztaimgr::f_play_set_tank;
    let test_name = "ZTAIMGR_F_PLAY_SET_TANK";
    let mut failures: Vec<String> = Vec::new();
    let fixture = Fixture::new();
    type VanillaTank = unsafe extern "stdcall" fn(i32, *const std::ffi::c_void) -> u32;
    let real: VanillaTank = unsafe { std::mem::transmute::<usize, VanillaTank>(0x005b_2be5) };

    for count in [0usize, 1, 3, 4] {
        let params: Vec<&str> = vec!["tankA"; count];
        for kind in ALL_KINDS {
            let unit = if kind == Kind::Scenery { 0 } else { fixture.entity(kind) as u32 };
            let (real_call, port_call) = (Call::new("fPlaySetTank", &params), Call::new("fPlaySetTank", &params));
            real_call.raw_mut_flag(0xaa);
            port_call.raw_mut_flag(0xaa);
            let real_result = unsafe { real(real_call.ptr() as i32, unit as *const std::ffi::c_void) } & 0xff;
            let port_result = unsafe { f_play_set_tank(port_call.ptr() as u32, unit, must_not_play) };
            if port_result != Some(real_result) || real_result != 0 || real_call.flag() != 0xaa || port_call.flag() != 0xaa {
                failures.push(format!("{count} params, {kind:?}: real {real_result} flag {:#x}, port {port_result:?} flag {:#x}", real_call.flag(), port_call.flag()));
            }
        }
    }

    unsafe fn must_not_play(_: u32, _: u32) -> u32 {
        panic!("play_set must not run for a parameter count other than two")
    }

    // Two parameters: selector 4 picks the last parameter, anything else the first.
    static PLAYED: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    unsafe fn record(name: u32, unit: u32) -> u32 {
        PLAYED.lock().unwrap().push((name, unit));
        0x77
    }
    let mut unit_block = Box::new([0u32; 0x100]);
    for selector in [4u32, 3, 0, 0xdead_beef] {
        unit_block[0x258 / 4] = selector;
        let unit = unit_block.as_ptr() as u32;
        let call = Call::new("fPlaySetTank", &["first", "second"]);
        call.raw_mut_flag(0xaa);
        PLAYED.lock().unwrap().clear();
        let result = unsafe { f_play_set_tank(call.ptr() as u32, unit, record) };
        let (begin, end) = (call.raw[3], call.raw[4]);
        let expected_name = if selector == 4 { end - 12 } else { begin };
        if result != Some(0x77) || *PLAYED.lock().unwrap() != vec![(expected_name, unit)] || call.flag() != 0 {
            failures.push(format!("selector {selector:#x}: result {result:?}, played {:x?}, flag {:#x}, expected name {expected_name:#x}", PLAYED.lock().unwrap(), call.flag()));
        }
    }
    let call = Call::new("fPlaySetTank", &["first", "second"]);
    call.raw_mut_flag(0xaa);
    if unsafe { f_play_set_tank(call.ptr() as u32, 0, record) }.is_some() || call.flag() != 0 {
        failures.push("two parameters with a non-unit target should defer after clearing the flag".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_INIT_AI_PARAMS` (live zoo, so the configuration is loaded): the real and ported `initAIParams` over
/// the same fake manager block must leave every dword identical.
pub(crate) fn run_ztaimgr_init_ai_params_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztaimgr::INIT_AIPARAMS;
    let test_name = "ZTAIMGR_INIT_AI_PARAMS";
    let mut failures: Vec<String> = Vec::new();
    let mut blocks = [Box::new([0u32; 0x60]), Box::new([0u32; 0x60])];
    for block in blocks.iter_mut() {
        for (index, word) in block.iter_mut().enumerate() {
            *word = 0x7000_0000 + index as u32;
        }
    }
    let real_ret = live_support::real_ztaimgr_init_ai_params(blocks[0].as_ptr());
    let port_ret = unsafe { INIT_AIPARAMS.hooked()(blocks[1].as_ptr()) };
    if real_ret & 0xff != 0 || port_ret & 0xff != 0 {
        failures.push(format!("return low byte real {:#x} port {:#x}, expected 0", real_ret & 0xff, port_ret & 0xff));
    }
    for index in 0..0x60 {
        if blocks[0][index] != blocks[1][index] {
            failures.push(format!("+{:#x}: real {:#x} != port {:#x}", index * 4, blocks[0][index], blocks[1][index]));
        }
    }
    for offset in [0x138usize, 0x13c, 0x140, 0x144, 0x148, 0x14c] {
        if blocks[0][offset / 4] == 0x7000_0000 + (offset / 4) as u32 {
            failures.push(format!("+{offset:#x}: not written by the real initAIParams"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_LOAD`: save a manager with events, then load those bytes with the real and the ported `load`
/// into fresh managers (empty event vectors, so vanilla allocates the loaded events) for every version around the
/// two gates, and for truncated streams. Result, tick fields, loaded events and bytes consumed must agree.
pub(crate) fn run_ztaimgr_bfaimgr_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::io_redirect;
    use openzt_detour::generated::bfaimgr::LOAD as BFAIMGR_LOAD;
    let test_name = "ZTAIMGR_BFAIMGR_LOAD";
    let mut failures: Vec<String> = Vec::new();
    let dummy_file: u32 = 0;
    let file = &dummy_file as *const u32 as *const i8;

    let mut source = FakeAiMgr::new(2, 1);
    source.set_ticks(1000, 0x1111_2222, 0x3333_4444);
    io_redirect::begin_capture();
    let saved_ok = live_support::real_bfaimgr_save(source.ptr(), file);
    let saved = io_redirect::end_capture();
    if !saved_ok || saved.len() < 8 {
        return finish_test(test_name, vec![format!("fixture check: save returned {saved_ok} with {} bytes", saved.len())], failure_log);
    }

    let all_versions = vec![0x10u32, 0x22, 0x23, 0x30, 0x60, 0x61, 0x62, 0x100];
    let gate_versions = vec![0x22u32, 0x23, 0x61, 0x100];
    let streams: Vec<(&str, Vec<u8>, Vec<u32>)> = vec![
        ("full stream", saved.clone(), all_versions),
        ("empty stream", Vec::new(), gate_versions.clone()),
        ("only the remainder", saved[..4].to_vec(), gate_versions.clone()),
        ("both ticks, no events", saved[..8].to_vec(), gate_versions.clone()),
        ("truncated events", saved[..saved.len() - 9].to_vec(), gate_versions),
    ];
    for (label, bytes, versions) in &streams {
        for &version in versions {
            let mut outcomes = Vec::new();
            for port in [false, true] {
                let mut target = FakeAiMgr::new(0, 0);
                target.use_real_event_mgr();
                target.set_ticks(777, 0xaaaa_aaaa, 0xbbbb_bbbb);
                io_redirect::begin_replay(bytes.clone());
                let result = if port {
                    unsafe { BFAIMGR_LOAD.hooked()(target.ptr(), file as *const u8, version) }
                } else {
                    live_support::real_bfaimgr_load(target.ptr(), file as *const u8, version)
                };
                let consumed = io_redirect::replay_position();
                io_redirect::end_replay();
                outcomes.push((result & 0xff, consumed, [target.words[0xe8 / 4], target.words[0xec / 4]], target.event_bytes()));
            }
            let what = format!("{label}, version {version:#x}");
            if outcomes[0] != outcomes[1] {
                failures.push(format!("{what}: real {:x?} != port {:x?}", outcomes[0], outcomes[1]));
            }
            if label.ends_with("full stream") {
                let expected_ticks = if version >= 0x23 { [0x1111_2222, 0x3333_4444] } else { [0xaaaa_aaaa, 0xbbbb_bbbb] };
                if outcomes[0].2 != expected_ticks || outcomes[0].0 != 1 {
                    failures.push(format!("{what}: fixture check: result {} ticks {:x?}", outcomes[0].0, outcomes[0].2));
                }
                if version >= 0x61 && outcomes[0].3[0].is_empty() && outcomes[0].3[1].is_empty() {
                    failures.push(format!("{what}: fixture check: no events were loaded"));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_SEND_EVENT`: posts several events through the real and the ported listener override into
/// separate managers with empty event vectors (vanilla allocates and grows them) and compares the stored event
/// records byte for byte.
pub(crate) fn run_ztaimgr_bfaimgr_send_event_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::bfaimgr::SEND_EVENT as BFAIMGR_SEND_EVENT;
    let test_name = "ZTAIMGR_BFAIMGR_SEND_EVENT";
    let mut failures: Vec<String> = Vec::new();
    let events: [(u16, u32, u8, u32, u8, u32, u16, u16); 4] = [
        (0x2714, 0x1234_5678, 0x58, 0xdead_beef, 0x01, 0x0bad_f00d, 0x1111, 0x0002),
        (0x2716, 0, 0, 0, 0, 0, 0, 0),
        (0xffff, 0xffff_ffff, 0xff, 0xffff_ffff, 0xff, 0xffff_ffff, 0xffff, 0xffff),
        (0x0001, 7, 3, 9, 4, 11, 5, 6),
    ];
    let mut results = Vec::new();
    for port in [false, true] {
        let mut mgr = FakeAiMgr::new(0, 0);
        mgr.use_real_event_mgr();
        let subobject = mgr.ptr() as u32 + 8;
        for (kind, target, flag_b, value_a, flag_a, value_b, extra, count) in events {
            if port {
                unsafe { BFAIMGR_SEND_EVENT.hooked()(subobject as *const u32, kind, target, flag_b, value_a, flag_a, value_b, extra, count) };
            } else {
                live_support::real_bfaimgr_send_event(subobject as *const u32, kind, target, flag_b, value_a, flag_a, value_b, extra, count);
            }
        }
        results.push(mgr.event_bytes());
    }
    if results[0] != results[1] {
        failures.push(format!("stored events differ: real {:x?} != port {:x?}", results[0], results[1]));
    }
    if results[0][0].len() != events.len() * 0x1c {
        failures.push(format!("fixture check: {} event bytes stored, expected {}", results[0][0].len(), events.len() * 0x1c));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_GET_EVENTS`: the listener override collects the matching events of the event manager's active
/// vector into a caller-supplied vector. Events with a matching or zero target, the requested kind byte and a
/// count below two are copied; the real and ported overrides must produce identical output vectors.
pub(crate) fn run_ztaimgr_bfaimgr_get_events_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::bfaimgr::GET_EVENTS as BFAIMGR_GET_EVENTS;
    let test_name = "ZTAIMGR_BFAIMGR_GET_EVENTS";
    let mut failures: Vec<String> = Vec::new();
    let mgr = FakeAiMgr::new(0, 6);
    // (target, kind byte, count) per event in the vector `+0xbc` points at.
    let events: [(u32, u8, u16); 6] = [(5, 1, 0), (0, 1, 1), (7, 1, 0), (5, 2, 0), (5, 1, 2), (5, 1, 5)];
    let begin = mgr.words[0xac / 4];
    for (index, (target, kind, count)) in events.iter().enumerate() {
        let record = begin + index as u32 * 0x1c;
        save_to_memory(record + 0xc, *target);
        save_to_memory(record + 0x18, *count);
        save_to_memory(record + 0x1b, *kind);
    }
    let subobject = mgr.ptr() as u32 + 8;
    for (query_target, query_kind, expected_count) in [(5u32, 1i8, 2usize), (7, 1, 2), (0, 1, 1), (5, 2, 1), (9, 9, 0)] {
        let mut outputs: Vec<Box<[u32; 3]>> = Vec::new();
        for port in [false, true] {
            let output = Box::new([0u32; 3]);
            if port {
                unsafe { BFAIMGR_GET_EVENTS.hooked()(subobject as *const u32, output.as_ptr() as *const std::ffi::c_void, query_target, query_kind) };
            } else {
                live_support::real_bfaimgr_get_events(subobject as *const u32, output.as_ptr() as *const std::ffi::c_void, query_target, query_kind);
            }
            outputs.push(output);
        }
        let bytes = |output: &[u32; 3]| (output[0]..output[1]).map(get_from_memory::<u8>).collect::<Vec<u8>>();
        let (real, port) = (bytes(&outputs[0]), bytes(&outputs[1]));
        let label = format!("target {query_target}, kind {query_kind}");
        if real != port {
            failures.push(format!("{label}: real {real:x?} != port {port:x?}"));
        }
        if real.len() != expected_count * 0x1c {
            failures.push(format!("{label}: fixture check: {} events collected, expected {expected_count}", real.len() / 0x1c));
        }
    }
    finish_test(test_name, failures, failure_log)
}
