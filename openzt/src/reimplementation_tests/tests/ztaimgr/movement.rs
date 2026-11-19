//! The move wrappers and `fMove`.

use super::*;

/// `ZTAIMGR_F_MOVE_SPEED_F`: port only, over a recording `fMove` stand-in. Covers the gates of
/// `fMoveSlowF`/`fMoveMediumF`/`fMoveFastF` (null unit, parameter count, empty first parameter, non-`ZTUnitType`
/// type) and the per-speed name, speed and limit handed to `fMove`, with the converted seconds value.
pub(crate) fn run_ztaimgr_f_move_speed_f_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_move_speed_f, MoveSpeed, MoveSpeedOps};
    let test_name = "ZTAIMGR_F_MOVE_SPEED_F";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static CONVERTED: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    unsafe fn f_move(this: u32, first: u32, seconds: u32, unit: u32, done: u32, name: u32, speed: u32, empty: u32, limit: u32) -> bool {
        let entry = format!(
            "move({this:#x},{},{seconds},{unit:#x},{done:#x},{},{speed:#x},{},{limit})",
            unsafe { read_vanilla_string(first) },
            unsafe { read_vanilla_string(name) },
            unsafe { read_vanilla_string(empty) }
        );
        LOG.lock().unwrap().push(entry);
        true
    }
    extern "thiscall" fn fake_convert(_this: u32, param: u32, _unit: u32) -> u32 {
        CONVERTED.lock().unwrap().push(param);
        7
    }
    extern "thiscall" fn fake_limit(_this: u32) -> u32 {
        55
    }
    let ops = MoveSpeedOps { f_move };

    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x34 / 4] = fake_convert as *const () as usize as u32;
    let mgr = Box::new([mgr_vtable.as_ptr() as u32]);
    let this = mgr.as_ptr() as u32;

    let text = "walk anim";
    let mut bytes = text.as_bytes().to_vec();
    bytes.push(0);
    let begin = bytes.as_ptr() as u32;
    let name_string = Box::new([begin, begin + text.len() as u32, begin + text.len() as u32 + 1]);
    FAKE_NAME_PTR.store(name_string.as_ptr() as u32, Ordering::Relaxed);

    let mut unit = StringFixtureUnit::new(false, true);
    for step in 0..3usize {
        unit._vtable[0x120 / 4 + step] = fake_unit_name as *const () as usize as u32;
        unit._vtable[0x140 / 4 + step] = fake_limit as *const () as usize as u32;
        unit._ty[0x100 / 4 + step] = 0x100 + step as u32;
    }
    let call = Call::new("fWalk", &["anim", "3"]);
    let first = call.raw[3];

    for (speed, expected_speed) in [(MoveSpeed::Slow, 0x100u32), (MoveSpeed::Medium, 0x101), (MoveSpeed::Fast, 0x102)] {
        LOG.lock().unwrap().clear();
        CONVERTED.lock().unwrap().clear();
        let result = unsafe { f_move_speed_f(this, call.ptr() as u32, unit.ptr(), speed, &ops) };
        let expected = format!("move({this:#x},anim,7,{:#x},{:#x},walk anim,{expected_speed:#x},,55)", unit.ptr(), call.ptr() as u32 + 0x18);
        if !result || *LOG.lock().unwrap() != vec![expected.clone()] || *CONVERTED.lock().unwrap() != vec![first + 12] {
            failures.push(format!("speed {expected_speed:#x}: result {result}, log {:?}, converted {:x?}, expected {expected}", LOG.lock().unwrap(), CONVERTED.lock().unwrap()));
        }
    }

    // Gates: nothing reaches fMove.
    let one_param = Call::new("fWalk", &["anim"]);
    let three_params = Call::new("fWalk", &["anim", "3", "x"]);
    let empty_first = Call::new("fWalk", &["", "3"]);
    let non_unit = StringFixtureUnit::new(false, false);
    let cases: [(&str, u32, u32); 5] = [
        ("null unit", call.ptr() as u32, 0),
        ("one parameter", one_param.ptr() as u32, unit.ptr()),
        ("three parameters", three_params.ptr() as u32, unit.ptr()),
        ("empty first parameter", empty_first.ptr() as u32, unit.ptr()),
        ("non-unit type", call.ptr() as u32, non_unit.ptr()),
    ];
    for (label, c, u) in cases {
        LOG.lock().unwrap().clear();
        let result = unsafe { f_move_speed_f(this, c, u, MoveSpeed::Fast, &ops) };
        if result || !LOG.lock().unwrap().is_empty() {
            failures.push(format!("{label}: result {result}, log {:?}", LOG.lock().unwrap()));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_MOVE_F`: port only, over a recording `fMove` stand-in. Covers `fMoveF`'s and
/// `fMoveWithSoundF`'s gates (null unit, parameter-count windows, empty first parameter) and the converted
/// values, strings, reverse flag and default/explicit limit handed to `fMove`.
pub(crate) fn run_ztaimgr_f_move_f_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_move_f, f_move_with_sound_f, MoveOps};
    let test_name = "ZTAIMGR_F_MOVE_F";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    unsafe fn f_move(this: u32, first: u32, a2: u32, unit: u32, done: u32, name: u32, speed: u32, extra: u32, reverse: u32, limit: u32) -> bool {
        let entry = format!(
            "move({this:#x},{},{a2},{unit:#x},{done:#x},{},{speed},{},{reverse},{limit})",
            unsafe { read_vanilla_string(first) },
            unsafe { read_vanilla_string(name) },
            unsafe { read_vanilla_string(extra) }
        );
        LOG.lock().unwrap().push(entry);
        true
    }
    // Converts a parameter by reading it as a decimal number.
    extern "thiscall" fn fake_convert(_this: u32, param: u32, _unit: u32) -> u32 {
        unsafe { read_vanilla_string(param) }.parse().unwrap_or(9999)
    }
    let ops = MoveOps { f_move };
    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x34 / 4] = fake_convert as *const () as usize as u32;
    let mgr = Box::new([mgr_vtable.as_ptr() as u32]);
    let this = mgr.as_ptr() as u32;
    let unit = 0x5000_0000u32;

    let run_f = |call: &Call, unit: u32, reverse: bool| -> (bool, Vec<String>) {
        LOG.lock().unwrap().clear();
        let result = unsafe { f_move_f(this, call.ptr() as u32, unit, reverse, &ops) };
        (result, LOG.lock().unwrap().clone())
    };
    let run_s = |call: &Call, unit: u32| -> (bool, Vec<String>) {
        LOG.lock().unwrap().clear();
        let result = unsafe { f_move_with_sound_f(this, call.ptr() as u32, unit, &ops) };
        (result, LOG.lock().unwrap().clone())
    };

    let four = Call::new("fMove", &["a", "2", "walk", "5"]);
    let five = Call::new("fMove", &["a", "2", "walk", "5", "70"]);
    for (label, call, reverse, expected_limit) in [("four", &four, false, 100), ("five", &five, true, 70)] {
        let (result, log) = run_f(call, unit, reverse);
        let expected = format!("move({this:#x},a,2000,{unit:#x},{:#x},walk,5,,{},{expected_limit})", call.ptr() as u32 + 0x18, u32::from(reverse));
        if !result || log != vec![expected.clone()] {
            failures.push(format!("fMoveF {label}: result {result}, log {log:?}, expected {expected}"));
        }
    }
    let three = Call::new("fMove", &["a", "2", "walk"]);
    let six = Call::new("fMove", &["a", "2", "walk", "5", "70", "1"]);
    let empty_first = Call::new("fMove", &["", "2", "walk", "5"]);
    for (label, call, u) in [("null unit", &four, 0), ("three", &three, unit), ("six", &six, unit), ("empty first", &empty_first, unit)] {
        let (result, log) = run_f(call, u, false);
        if result || !log.is_empty() {
            failures.push(format!("fMoveF {label}: result {result}, log {log:?}"));
        }
    }

    let sound5 = Call::new("fMove", &["a", "2", "walk", "5", "purr"]);
    let sound6 = Call::new("fMove", &["a", "2", "walk", "5", "purr", "70"]);
    for (label, call, expected_limit) in [("five", &sound5, 100), ("six", &sound6, 70)] {
        let (result, log) = run_s(call, unit);
        let expected = format!("move({this:#x},a,2,{unit:#x},{:#x},walk,5,purr,0,{expected_limit})", call.ptr() as u32 + 0x18);
        if !result || log != vec![expected.clone()] {
            failures.push(format!("fMoveWithSoundF {label}: result {result}, log {log:?}, expected {expected}"));
        }
    }
    let sound7 = Call::new("fMove", &["a", "2", "walk", "5", "purr", "70", "1"]);
    let sound_empty = Call::new("fMove", &["", "2", "walk", "5", "purr"]);
    for (label, call, u) in [("null unit", &sound5, 0), ("four", &four, unit), ("seven", &sound7, unit), ("empty first", &sound_empty, unit)] {
        let (result, log) = run_s(call, u);
        if result || !log.is_empty() {
            failures.push(format!("fMoveWithSoundF {label}: result {result}, log {log:?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_MOVE`: port only, over a fake unit (own vtable, cog goal and target list) and recording
/// stand-ins for the vanilla helpers. Checks every path of the `fMove` state machine: the order of virtual
/// calls, the flag writes, the done flag, the arguments handed to the mixer helpers and the result.
pub(crate) fn run_ztaimgr_f_move_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_move, FMoveArgs, FMoveOps};
    let test_name = "ZTAIMGR_F_MOVE";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static HAS_TARGET: AtomicU32 = AtomicU32::new(0);
    static CAN_PLAY: AtomicU32 = AtomicU32::new(0);
    static VALID: AtomicU32 = AtomicU32::new(0);
    static REACHED: AtomicU32 = AtomicU32::new(0);
    static NO_SOUND: AtomicU32 = AtomicU32::new(0);
    static ANIM_FIRST: AtomicU32 = AtomicU32::new(0);
    static ANIM_SECOND: AtomicU32 = AtomicU32::new(0);
    static PAIR_RESULT: AtomicU32 = AtomicU32::new(0);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    extern "thiscall" fn v_has_target(_: u32) -> u32 {
        log("has_target".into());
        HAS_TARGET.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_can_play(_: u32) -> u32 {
        log("can_play".into());
        CAN_PLAY.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_prepare(_: u32, arg: u32) -> u32 {
        log(format!("prepare({arg:#x})"));
        0
    }
    extern "thiscall" fn v_find_anim(_: u32, name: u32) -> u32 {
        log(format!("find_anim({name:#x})"));
        ANIM_FIRST.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_find_anim_second(_: u32) -> u32 {
        log("find_anim_second".into());
        ANIM_SECOND.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_set_speed(_: u32, arg: u32) -> u32 {
        log(format!("speed({arg})"));
        0
    }
    extern "thiscall" fn v_set_limit(_: u32, arg: u32) -> u32 {
        log(format!("limit({arg})"));
        0
    }
    extern "thiscall" fn c_valid(_: u32) -> u32 {
        log("cog_valid".into());
        VALID.load(Ordering::Relaxed)
    }
    extern "thiscall" fn c_clear(_: u32) -> u32 {
        log("cog_clear".into());
        0
    }
    extern "thiscall" fn c_clear_sub(_: u32) -> u32 {
        log("cog_clear_sub".into());
        0
    }
    unsafe fn stop(_: u32, a: i8, b: i8) {
        log(format!("stop({a},{b})"));
    }
    unsafe fn set_moving(_: u32, a: i8, b: i8) {
        log(format!("set_moving({a},{b})"));
    }
    unsafe fn reached(_: u32) -> bool {
        log("reached".into());
        REACHED.load(Ordering::Relaxed) != 0
    }
    unsafe fn is_no_sound(_: u32) -> bool {
        NO_SOUND.load(Ordering::Relaxed) != 0
    }
    unsafe fn find_sound(_: u32, _: u32) -> u32 {
        log("find_sound".into());
        77
    }
    unsafe fn play_single(_: u32, anim: u32, flag: u8, sound: u32) {
        log(format!("single({anim:#x},{flag},{sound})"));
    }
    unsafe fn play_pair(_: u32, _: u32, first: u32, second: u32, flag: u8, sound: u32, seconds: i32) -> bool {
        log(format!("pair({first:#x},{second:#x},{flag},{sound},{seconds})"));
        PAIR_RESULT.load(Ordering::Relaxed) != 0
    }
    let ops = FMoveOps { stop, set_moving, reached_target: reached, is_no_sound, find_sound, play_single, play_pair };

    let mut unit_vtable = Box::new([0u32; 0x80]);
    unit_vtable[0x180 / 4] = v_has_target as *const () as usize as u32;
    unit_vtable[0x184 / 4] = v_can_play as *const () as usize as u32;
    unit_vtable[0x1b0 / 4] = v_prepare as *const () as usize as u32;
    unit_vtable[0x130 / 4] = v_find_anim as *const () as usize as u32;
    unit_vtable[0x134 / 4] = v_find_anim_second as *const () as usize as u32;
    unit_vtable[0x1c0 / 4] = v_set_speed as *const () as usize as u32;
    unit_vtable[0x1c4 / 4] = v_set_limit as *const () as usize as u32;
    let mut cog_vtable = Box::new([0u32; 0x10]);
    cog_vtable[0] = c_valid as *const () as usize as u32;
    cog_vtable[0xc / 4] = c_clear as *const () as usize as u32;
    cog_vtable[0x1c / 4] = c_clear_sub as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x200]);
    block[0] = unit_vtable.as_ptr() as u32;
    block[0x15c / 4] = cog_vtable.as_ptr() as u32;
    let unit = block.as_ptr() as u32;

    // A ring of `n` nodes around a sentinel (node 0); `+0x154` points at the sentinel.
    let mut ring = vec![[0u32; 2]; 8];
    let sentinel = ring.as_ptr() as u32;
    let set_list = |ring: &mut Vec<[u32; 2]>, block: &mut Box<[u32; 0x200]>, n: usize| {
        for i in 0..=n {
            ring[i][0] = if i == n { sentinel } else { sentinel + (i as u32 + 1) * 8 };
        }
        block[0x154 / 4] = sentinel;
    };
    let byte = |block: &Box<[u32; 0x200]>, offset: usize| unsafe { *((block.as_ptr() as usize + offset) as *const u8) };
    let set_byte = |block: &mut Box<[u32; 0x200]>, offset: usize, value: u8| unsafe { *((block.as_ptr() as usize + offset) as *mut u8) = value };

    let empty_extra = Box::new([0x1000u32, 0x1000, 0x1000]);
    let text_extra = Box::new([0x2000u32, 0x2003, 0x2004]);
    let (empty_ptr, text_ptr) = (empty_extra.as_ptr() as u32, text_extra.as_ptr() as u32);
    let this = 0x7777_0000u32;
    let name = 0x3000u32;
    let first = 0xa000u32;
    let second = 0xb000u32;
    let mut done = Box::new(0xaau8);
    let done_ptr = done.as_mut() as *mut u8 as *const u8;

    struct Case {
        label: &'static str,
        flags: &'static [(usize, u8)],
        state: u32,
        state2: u32,
        list: usize,
        seconds: i32,
        extra: u32,
        can_play: bool,
        valid: bool,
        has_target: bool,
        expect_log: &'static [&'static str],
        /// moving (`+0xf8`), stopped (`+0xf9`), done flag, cog invalid (`+0x20e`).
        expect: (u8, u8, u8, u8),
    }
    let c = |label, flags, state, state2, list, seconds, extra, can_play, valid, has_target, expect_log, expect| Case { label, flags, state, state2, list, seconds, extra, can_play, valid, has_target, expect_log, expect };
    let single: &[&str] = &["has_target", "find_anim(0x3000)", "single(0xa000,1,0)", "speed(9)", "limit(33)"];
    // 0xf8 moving, 0xf9 stopped, 0xd7, 0x20c, 0x20d finished, 0x20e cog invalid, 0x1a8 = cog + 0x4c.
    let cases = vec![
        c("finished", &[(0x20d, 1)], 0, 0, 0, 0, empty_ptr, false, false, false, &["has_target"], (0, 0, 1, 0)),
        c("finished prepares", &[(0x20d, 1)], 1, 0, 0, 0, empty_ptr, false, false, false, &["has_target", "prepare(0x5555)"], (0, 0, 1, 0)),
        c("cog invalid", &[(0xf9, 1)], 0, 0, 0, 0, empty_ptr, true, false, false, &["has_target", "can_play", "cog_valid"], (0, 1, 0, 1)),
        c("cog valid, no flag", &[(0xf9, 1)], 0, 0, 0, 0, empty_ptr, true, true, false, &["has_target", "can_play", "cog_valid"], (0, 1, 0xaa, 0)),
        c("cog valid, flag, invalid", &[(0xf9, 1), (0x1a8, 1)], 0, 0, 0, 0, empty_ptr, true, true, false, &["has_target", "can_play", "cog_valid"], (0, 1, 0, 1)),
        c("cog valid, flag, state 1", &[(0xf9, 1), (0x1a8, 1)], 1, 0, 0, 0, empty_ptr, true, true, false, &["has_target", "prepare(0x5555)", "can_play", "cog_valid"], (0, 1, 0xaa, 0)),
        c("stopped, empty list, cannot play", &[(0xf9, 1)], 0, 0, 0, 0, empty_ptr, false, false, false, &["has_target", "can_play"], (0, 1, 0xaa, 0)),
        c("one target stops", &[(0xf9, 1)], 0, 0, 1, 0, empty_ptr, false, false, false, &["has_target", "speed(0)", "stop(1,1)"], (0, 1, 1, 0)),
        c("one target, state2 3 moves", &[(0xf9, 1)], 0, 3, 1, 0, empty_ptr, false, false, false, single, (1, 0, 0, 0)),
        c("two targets single", &[(0xf9, 1)], 0, 0, 2, 0, empty_ptr, false, false, false, single, (1, 0, 0, 0)),
        c("two targets sound", &[(0xf9, 1)], 0, 0, 2, 0, text_ptr, false, false, false, &["has_target", "find_anim(0x3000)", "find_sound", "single(0xa000,1,77)", "speed(9)", "limit(33)"], (1, 0, 0, 0)),
        c("two targets pair", &[(0xf9, 1)], 0, 0, 2, 5, empty_ptr, false, false, false, &["has_target", "find_anim(0x3000)", "find_anim_second", "pair(0xa000,0xb000,1,0,5)", "speed(9)", "limit(33)"], (1, 0, 0, 0)),
        c("moving one target has target", &[(0xf8, 1)], 0, 0, 1, 0, empty_ptr, false, false, true, &["has_target", "cog_clear", "cog_clear_sub"], (0, 1, 1, 0)),
        c("moving empty list", &[(0xf8, 1)], 0, 0, 0, 0, empty_ptr, false, false, false, &["has_target", "speed(0)", "stop(1,0)"], (0, 1, 1, 0)),
        c("moving d7", &[(0xf8, 1), (0xd7, 1)], 0, 0, 2, 0, empty_ptr, false, false, false, &["has_target", "reached", "set_moving(1,0)"], (1, 0, 0xaa, 0)),
        c("moving d7 but 20c", &[(0xf8, 1), (0xd7, 1), (0x20c, 1)], 0, 0, 2, 0, empty_ptr, false, false, false, &["has_target", "reached"], (1, 0, 0xaa, 0)),
        c("moving nothing to do", &[(0xf8, 1)], 0, 0, 2, 0, empty_ptr, false, false, false, &["has_target", "reached"], (1, 0, 0xaa, 0)),
        c("idle", &[], 0, 0, 2, 0, empty_ptr, false, false, false, &["has_target"], (0, 0, 0xaa, 0)),
    ];
    for case in &cases {
        for offset in [0xf8usize, 0xf9, 0xd7, 0x20c, 0x20d, 0x20e, 0x1a8] {
            set_byte(&mut block, offset, 0);
        }
        for &(offset, value) in case.flags {
            set_byte(&mut block, offset, value);
        }
        block[0x170 / 4] = case.state;
        block[0x1bc / 4] = case.state2;
        set_list(&mut ring, &mut block, case.list);
        CAN_PLAY.store(u32::from(case.can_play), Ordering::Relaxed);
        VALID.store(u32::from(case.valid), Ordering::Relaxed);
        HAS_TARGET.store(u32::from(case.has_target), Ordering::Relaxed);
        REACHED.store(0, Ordering::Relaxed);
        NO_SOUND.store(0, Ordering::Relaxed);
        PAIR_RESULT.store(1, Ordering::Relaxed);
        // With a duration the first lookup returns the second animation and the next returns the first.
        let (lookup, lookup_second) = if case.seconds != 0 { (second, first) } else { (first, 0) };
        ANIM_FIRST.store(lookup, Ordering::Relaxed);
        ANIM_SECOND.store(lookup_second, Ordering::Relaxed);
        *done = 0xaa;
        LOG.lock().unwrap().clear();
        let args = FMoveArgs { this, first_param: 0x5555, seconds: case.seconds, unit, done: done_ptr, name, speed: 9, extra: case.extra, reverse: 1, limit: 33 };
        let result = unsafe { f_move(&args, &ops) };
        let got = (byte(&block, 0xf8), byte(&block, 0xf9), *done, byte(&block, 0x20e));
        let expected_log: Vec<String> = case.expect_log.iter().map(|s| s.to_string()).collect();
        if !result || *LOG.lock().unwrap() != expected_log || got != case.expect {
            failures.push(format!("{}: result {result}, log {:?}, state {got:?}; expected log {expected_log:?}, state {:?}", case.label, LOG.lock().unwrap(), case.expect));
        }
    }

    // A missing first animation fails without touching the flags.
    for offset in [0xf8usize, 0xf9, 0xd7, 0x20c, 0x20d, 0x20e] {
        set_byte(&mut block, offset, 0);
    }
    set_byte(&mut block, 0xf9, 1);
    block[0x170 / 4] = 0;
    set_list(&mut ring, &mut block, 2);
    CAN_PLAY.store(0, Ordering::Relaxed);
    HAS_TARGET.store(0, Ordering::Relaxed);
    ANIM_FIRST.store(0, Ordering::Relaxed);
    *done = 0xaa;
    LOG.lock().unwrap().clear();
    let args = FMoveArgs { this, first_param: 0x5555, seconds: 0, unit, done: done_ptr, name, speed: 9, extra: empty_ptr, reverse: 1, limit: 33 };
    let result = unsafe { f_move(&args, &ops) };
    if result || byte(&block, 0xf8) != 0 || byte(&block, 0xf9) != 1 || *done != 0 || *LOG.lock().unwrap() != vec!["has_target".to_string(), "find_anim(0x3000)".to_string()] {
        failures.push(format!("missing animation: result {result}, log {:?}", LOG.lock().unwrap()));
    }
    finish_test(test_name, failures, failure_log)
}
