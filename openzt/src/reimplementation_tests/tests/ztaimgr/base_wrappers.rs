//! The `BFAIMgr` wrapper guards and forwarding, and `fPlaySetTerrainF`.

use super::*;

/// `BFAIMgr` forwarding wrappers whose real body is safe to run over fakes: the argument-count guards (which return before
/// touching the unit or manager), compared real vs port, plus `fPlayStopAtEndF`'s virtual call on a fake entity.
/// Paths that reach a real `fPlayF`/`fFlyF`/`fPlayTime`/`fPlaySound`/`fPlayWithSound`/`fPlaySet`/`setMoving`
/// need live animation state, so those are checked port-only in the forwarding test with recording stand-ins.
pub(crate) fn run_ztaimgr_base_wrapper_guards_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::ztaimgr::live_support::{
        real_f_face_direction_f, real_f_fly_reverse_f, real_f_move_entity_special_f, real_f_play_ping_pong_f, real_f_play_reverse_f, real_f_play_set_f, real_f_play_sound_f, real_f_play_stop_at_end_f, real_f_play_time_f,
    };
    use openzt_detour::generated::bfaimgr::{
        F_FACE_DIRECTION_F, F_FLY_REVERSE_F, F_MOVE_ENTITY_SPECIAL_F, F_PLAY_PING_PONG_F, F_PLAY_REVERSE_F, F_PLAY_SET_F, F_PLAY_SOUND_F, F_PLAY_STOP_AT_END_F, F_PLAY_TIME_F,
    };
    let test_name = "ZTAIMGR_BASE_WRAPPER_GUARDS";
    let mut failures: Vec<String> = Vec::new();
    let fixture = Fixture::new();
    let this = fixture.this();

    // fFaceDirectionF: true only for exactly one non-empty parameter; never touches the call.
    let face_cases: [Vec<&str>; 5] = [vec![], vec![""], vec!["north"], vec!["a", "b"], vec!["", ""]];
    for params in face_cases {
        let (real_call, port_call) = (Call::new("fFaceDirection", &params), Call::new("fFaceDirection", &params));
        real_call.raw_mut_flag(0xaa);
        port_call.raw_mut_flag(0xaa);
        let real = real_f_face_direction_f(real_call.ptr(), std::ptr::null());
        let port = unsafe { F_FACE_DIRECTION_F.hooked()(port_call.ptr(), std::ptr::null()) };
        let expected = params.len() == 1 && !params[0].is_empty();
        if real != port || real != expected || real_call.flag() != 0xaa || port_call.flag() != 0xaa {
            failures.push(format!("fFaceDirectionF {params:?}: real {real} port {port} expected {expected}, flags {:#x}/{:#x}", real_call.flag(), port_call.flag()));
        }
    }

    // fPlaySetF and fPlaySoundF: any parameter count other than one returns false and leaves the call alone.
    for count in [0usize, 2, 3] {
        let params = vec!["x"; count];
        let (real_call, port_call) = (Call::new("fPlaySet", &params), Call::new("fPlaySet", &params));
        real_call.raw_mut_flag(0xaa);
        port_call.raw_mut_flag(0xaa);
        let real = real_f_play_set_f(real_call.ptr(), std::ptr::null());
        let port = unsafe { F_PLAY_SET_F.hooked()(port_call.ptr(), std::ptr::null()) };
        if real || port || real_call.flag() != 0xaa || port_call.flag() != 0xaa {
            failures.push(format!("fPlaySetF {count} params: real {real} port {port}, flags {:#x}/{:#x}", real_call.flag(), port_call.flag()));
        }
        let (real_call, port_call) = (Call::new("fPlaySound", &params), Call::new("fPlaySound", &params));
        let real = real_f_play_sound_f(this, real_call.ptr(), std::ptr::null());
        let port = unsafe { F_PLAY_SOUND_F.hooked()(this, port_call.ptr(), std::ptr::null()) };
        if real || port {
            failures.push(format!("fPlaySoundF {count} params: real {real} port {port}"));
        }
    }

    // fMoveEntitySpecialF: any parameter count other than three returns 0 before touching the unit or call.
    for count in [0usize, 1, 2, 4] {
        let params = vec!["1"; count];
        let (real_call, port_call) = (Call::new("fMoveEntitySpecial", &params), Call::new("fMoveEntitySpecial", &params));
        real_call.raw_mut_flag(0xaa);
        port_call.raw_mut_flag(0xaa);
        let real = real_f_move_entity_special_f(this, real_call.ptr(), std::ptr::null()) & 0xff;
        let port = unsafe { F_MOVE_ENTITY_SPECIAL_F.hooked()(this, port_call.ptr(), std::ptr::null()) } & 0xff;
        if real != 0 || port != 0 || real_call.flag() != 0xaa || port_call.flag() != 0xaa {
            failures.push(format!("fMoveEntitySpecialF {count} params: real {real} port {port}, flags {:#x}/{:#x}", real_call.flag(), port_call.flag()));
        }
    }

    // fPlayTimeF: any parameter count other than two returns false before converting anything.
    for count in [0usize, 1, 3] {
        let params = vec!["1"; count];
        let (real_call, port_call) = (Call::new("fPlayTime", &params), Call::new("fPlayTime", &params));
        let real = real_f_play_time_f(this, real_call.ptr(), std::ptr::null());
        let port = unsafe { F_PLAY_TIME_F.hooked()(this, port_call.ptr(), std::ptr::null()) };
        if real || port {
            failures.push(format!("fPlayTimeF {count} params: real {real} port {port}"));
        }
    }

    // fPlayReverseF / fPlayPingPongF / fPlayStopAtEndF with no parameters: the real fPlayF returns false before
    // using the unit. The stop-at-end variant first calls entity vtable +0xdc(1), recorded on a fake entity.
    static STOP_AT_END_LOG: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    extern "thiscall" fn fake_set_stop_at_end(this: u32, flag: u32) {
        STOP_AT_END_LOG.lock().unwrap().push((this, flag & 0xff));
    }
    let mut entity_vtable = Box::new([0u32; 0x100]);
    entity_vtable[0xdc / 4] = fake_set_stop_at_end as *const () as usize as u32;
    let mut entity = Box::new([0u32; 0x60]);
    entity[0] = entity_vtable.as_ptr() as u32;
    let unit = entity.as_ptr();
    let empty = Call::new("fPlay", &[]);
    let pairs: [(&str, bool, bool); 3] = [
        ("fPlayReverseF", real_f_play_reverse_f(this, empty.ptr(), unit), unsafe { F_PLAY_REVERSE_F.hooked()(this, empty.ptr(), unit) }),
        ("fPlayPingPongF", real_f_play_ping_pong_f(this, empty.ptr(), unit), unsafe { F_PLAY_PING_PONG_F.hooked()(this, empty.ptr(), unit) }),
        ("fFlyReverseF (null unit)", real_f_fly_reverse_f(this, empty.ptr(), std::ptr::null()), unsafe { F_FLY_REVERSE_F.hooked()(this, empty.ptr(), std::ptr::null()) }),
    ];
    for (name, real, port) in pairs {
        if real || port {
            failures.push(format!("{name}: real {real} port {port}, expected both false"));
        }
    }
    STOP_AT_END_LOG.lock().unwrap().clear();
    let real = real_f_play_stop_at_end_f(this, empty.ptr(), unit);
    let real_log = std::mem::take(&mut *STOP_AT_END_LOG.lock().unwrap());
    let port = unsafe { F_PLAY_STOP_AT_END_F.hooked()(this, empty.ptr(), unit) };
    let port_log = std::mem::take(&mut *STOP_AT_END_LOG.lock().unwrap());
    if real || port || real_log != port_log || port_log != vec![(unit as u32, 1)] {
        failures.push(format!("fPlayStopAtEndF: real {real} {real_log:x?}, port {port} {port_log:x?}"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFAIMgr` forwarding wrappers' forwarding paths, port only (the real targets need live animation state): the exact
/// arguments passed on, the call-flag writes, the millisecond conversion and the null-unit deferrals.
pub(crate) fn run_ztaimgr_base_wrapper_forwarding_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::ztaimgr::{f_face_direction_f, f_halt_f, f_play_set_f, f_play_stop_at_end_prepare, f_play_time_f};
    let test_name = "ZTAIMGR_BASE_WRAPPER_FORWARDING";
    let mut failures: Vec<String> = Vec::new();

    // fPlaySetF: one parameter clears the flag and plays the first parameter's string.
    static PLAYED: Mutex<Vec<(u32, u32)>> = Mutex::new(Vec::new());
    unsafe fn record_play_set(name: u32, unit: u32) -> u32 {
        PLAYED.lock().unwrap().push((name, unit));
        0x177
    }
    PLAYED.lock().unwrap().clear();
    let call = Call::new("fPlaySet", &["walk"]);
    call.raw_mut_flag(0xaa);
    let result = unsafe { f_play_set_f(call.ptr() as u32, 0x1234, record_play_set) };
    if result != 0x177 || *PLAYED.lock().unwrap() != vec![(call.raw[3], 0x1234)] || call.flag() != 0 {
        failures.push(format!("fPlaySetF: result {result:#x}, played {:x?}, flag {:#x}", PLAYED.lock().unwrap(), call.flag()));
    }
    PLAYED.lock().unwrap().clear();
    let call = Call::new("fPlaySet", &["a", "b"]);
    call.raw_mut_flag(0xaa);
    if unsafe { f_play_set_f(call.ptr() as u32, 0x1234, record_play_set) } != 0 || !PLAYED.lock().unwrap().is_empty() || call.flag() != 0xaa {
        failures.push("fPlaySetF with two parameters should return 0 and leave the call alone".to_string());
    }

    // fPlayTimeF: converts the second parameter through the manager's vtable +0x34 and multiplies by 1000.
    static CONVERTED: Mutex<Vec<(u32, u32, u32)>> = Mutex::new(Vec::new());
    static PLAYED_TIME: Mutex<Vec<[u32; 6]>> = Mutex::new(Vec::new());
    extern "thiscall" fn fake_convert(this: u32, param: u32, unit: u32) -> u32 {
        CONVERTED.lock().unwrap().push((this, param, unit));
        7
    }
    unsafe fn record_play_time(this: u32, name: u32, millis: u32, unit: u32, done: u32, flag_b: u32) -> u32 {
        PLAYED_TIME.lock().unwrap().push([this, name, millis, unit, done, flag_b]);
        0x255
    }
    let mut vtable = Box::new([0u32; 0x20]);
    vtable[0x34 / 4] = fake_convert as *const () as usize as u32;
    let mut mgr = Box::new([0u32; 0x20]);
    mgr[0] = vtable.as_ptr() as u32;
    let this = mgr.as_ptr() as u32;
    let call = Call::new("fPlayTime", &["idle", "3"]);
    unsafe { *((call.ptr() as u32 + 0x19) as *mut u8) = 0x5a };
    let result = unsafe { f_play_time_f(this, call.ptr() as u32, 0x4321, record_play_time) };
    let expected_play = [this, call.raw[3], 7000, 0x4321, call.ptr() as u32 + 0x18, 0x5a];
    if result != 0x255 || *CONVERTED.lock().unwrap() != vec![(this, call.raw[3] + 12, 0x4321)] || *PLAYED_TIME.lock().unwrap() != vec![expected_play] {
        failures.push(format!("fPlayTimeF: result {result:#x}, converted {:x?}, played {:x?}, expected {expected_play:x?}", CONVERTED.lock().unwrap(), PLAYED_TIME.lock().unwrap()));
    }

    // Deferrals and the stub-only paths.
    if unsafe { f_play_stop_at_end_prepare(0) } {
        failures.push("fPlayStopAtEndF with a null unit should defer".to_string());
    }
    let call = Call::new("fHalt", &[]);
    call.raw_mut_flag(0xaa);
    if unsafe { f_halt_f(call.ptr() as u32, 0) }.is_some() || call.flag() != 0xaa {
        failures.push("fHaltF with a null unit should defer and leave the call alone".to_string());
    }
    if !unsafe { f_face_direction_f(Call::new("fFaceDirection", &["n"]).ptr() as u32) } {
        failures.push("fFaceDirectionF with one parameter should be true".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_SET_TERRAIN_F`: port only, over a fake unit block and recording stand-ins for the tile lookup
/// and `fPlaySet`. Checks the parameter choice by selector and tile flags, the string handed to `fPlaySet`, the
/// call's result flag and the returned low byte, and that other parameter counts and non-unit targets defer.
pub(crate) fn run_ztaimgr_f_play_set_terrain_f_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::f_play_set_terrain_f;
    let test_name = "ZTAIMGR_F_PLAY_SET_TERRAIN_F";
    let mut failures: Vec<String> = Vec::new();
    let fixture = Fixture::new();

    static PLAYED: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());
    static TILE_FLAGS: AtomicU32 = AtomicU32::new(0);
    unsafe fn tile_flags(_: u32) -> u8 {
        TILE_FLAGS.load(Ordering::Relaxed) as u8
    }
    unsafe fn record(name: u32, unit: u32) -> u32 {
        let (begin, end) = (unsafe { *(name as *const u32) }, unsafe { *((name + 4) as *const u32) });
        let bytes = unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) };
        PLAYED.lock().unwrap().push((String::from_utf8_lossy(bytes).into_owned(), unit));
        0x1234_5677
    }
    unsafe fn must_not_run(_: u32, _: u32) -> u32 {
        panic!("play_set must not run when the port defers")
    }

    let mut unit_block = Box::new([0u32; 0x100]);
    unit_block[0x128 / 4] = unsafe { *(fixture.entity(Kind::Guest).add(0x128 / 4)) };
    let unit = unit_block.as_ptr() as u32;
    let names = ["p0", "p1", "p2", "p3"];

    // (selector, tile flags, expected parameter)
    for (selector, flags, expected) in [(4u32, 0u32, 3usize), (4, 3, 3), (3, 0, 2), (3, 1, 2), (0, 0, 0), (0, 0xfc, 0), (0, 1, 1), (0, 2, 1), (7, 3, 1)] {
        unit_block[0x258 / 4] = selector;
        TILE_FLAGS.store(flags, Ordering::Relaxed);
        PLAYED.lock().unwrap().clear();
        let call = Call::new("fPlaySetTerrain", &names);
        call.raw_mut_flag(0xaa);
        let result = unsafe { f_play_set_terrain_f(call.ptr() as u32, unit, tile_flags, record) };
        let played = PLAYED.lock().unwrap().clone();
        if result != Some(true) || played != vec![(names[expected].to_string(), unit)] || call.flag() != 0 {
            failures.push(format!("selector {selector} flags {flags:#x}: result {result:?}, played {played:?}, flag {:#x}, expected {}", call.flag(), names[expected]));
        }
    }

    // `fPlaySet`'s result is a bool: only the low byte counts.
    unsafe fn record_low_byte_clear(_: u32, _: u32) -> u32 {
        0x1234_5600
    }
    unit_block[0x258 / 4] = 4;
    let call = Call::new("fPlaySetTerrain", &names);
    let result = unsafe { f_play_set_terrain_f(call.ptr() as u32, unit, tile_flags, record_low_byte_clear) };
    if result != Some(false) {
        failures.push(format!("low byte clear: result {result:?}"));
    }

    // An empty chosen parameter plays an empty name.
    unit_block[0x258 / 4] = 4;
    PLAYED.lock().unwrap().clear();
    let call = Call::new("fPlaySetTerrain", &["a", "b", "c", ""]);
    unsafe { f_play_set_terrain_f(call.ptr() as u32, unit, tile_flags, record) };
    if *PLAYED.lock().unwrap() != vec![(String::new(), unit)] {
        failures.push(format!("empty parameter: played {:?}", PLAYED.lock().unwrap()));
    }

    // Deferrals leave the call untouched.
    for count in [0usize, 1, 2, 3, 5] {
        let params = vec!["x"; count];
        let call = Call::new("fPlaySetTerrain", &params);
        call.raw_mut_flag(0xaa);
        if unsafe { f_play_set_terrain_f(call.ptr() as u32, unit, tile_flags, must_not_run) }.is_some() || call.flag() != 0xaa {
            failures.push(format!("{count} parameters should defer without touching the call"));
        }
    }
    for target in [0u32, fixture.entity(Kind::Scenery) as u32] {
        let call = Call::new("fPlaySetTerrain", &names);
        call.raw_mut_flag(0xaa);
        if unsafe { f_play_set_terrain_f(call.ptr() as u32, target, tile_flags, must_not_run) }.is_some() || call.flag() != 0xaa {
            failures.push(format!("target {target:#x} should defer without touching the call"));
        }
    }
    finish_test(test_name, failures, failure_log)
}
