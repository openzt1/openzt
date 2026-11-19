//! `fPlaySpecial`, `fPlayF` and `fFlyF`.

use super::*;

/// `ZTAIMGR_F_PLAY_SPECIAL`: port only, over a fake unit and recording stand-ins. Covers the special-mode
/// gate, the not-stopped path, the animation and sound resolution (including the default-animation fallback
/// and its failure cases) and the flags and arguments handed to the link builder.
pub(crate) fn run_ztaimgr_f_play_special_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_special, FPlaySpecialArgs, FPlaySpecialOps, SpecialString};
    let test_name = "ZTAIMGR_F_PLAY_SPECIAL";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static FIND_ANIM_RESULT: AtomicU32 = AtomicU32::new(0);
    static PLAY_RESULT: AtomicU32 = AtomicU32::new(0);
    static PLAY_EMPTY: AtomicU32 = AtomicU32::new(0);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    // Strings are identified by their begin word: 1 none, 2 reverse, 3 ping-pong, anything else a real name.
    unsafe fn is_string(which: SpecialString, string: u32) -> bool {
        let id = unsafe { *(string as *const u32) };
        id == match which {
            SpecialString::None => 1,
            SpecialString::Reverse => 2,
            SpecialString::PingPong => 3,
            SpecialString::ModeA => 4,
            SpecialString::ModeB => 5,
        }
    }
    unsafe fn find_anim(_: u32, name: u32) -> u32 {
        log(format!("find_anim({})", unsafe { *(name as *const u32) }));
        FIND_ANIM_RESULT.load(Ordering::Relaxed)
    }
    unsafe fn find_sound(_: u32, name: u32) -> u32 {
        log(format!("find_sound({})", unsafe { *(name as *const u32) }));
        77
    }
    unsafe fn acquire_first(anim: u32) -> u32 {
        log(format!("acquire({anim:#x})"));
        0x42
    }
    unsafe fn release_first(handle: u32) {
        log(format!("release({handle:#x})"));
    }
    unsafe fn play_links(_: u32, _: u32, first: u32, anim: u32, reverse: bool, sound: u32, ping_pong: bool, seconds: u32, count: i32) -> Option<bool> {
        log(format!("links({first:#x},{anim:#x},{reverse},{sound},{ping_pong},{seconds},{count})"));
        if PLAY_EMPTY.load(Ordering::Relaxed) != 0 {
            None
        } else {
            Some(PLAY_RESULT.load(Ordering::Relaxed) != 0)
        }
    }
    extern "thiscall" fn v_default_anim(_: u32, arg: u32) -> u32 {
        log(format!("default_anim({arg})"));
        0xd000
    }
    let ops = FPlaySpecialOps { is_string, find_anim, find_sound, acquire_first, release_first, play_links };

    let mut vtable = Box::new([0u32; 0x80]);
    vtable[0x8c / 4] = v_default_anim as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x200]);
    block[0] = vtable.as_ptr() as u32;
    block[0x128 / 4] = 1;
    let unit = block.as_ptr() as u32;
    let set_byte = |block: &mut Box<[u32; 0x200]>, offset: usize, value: u8| unsafe { *((block.as_ptr() as usize + offset) as *mut u8) = value };
    let byte = |block: &Box<[u32; 0x200]>, offset: usize| unsafe { *((block.as_ptr() as usize + offset) as *const u8) };

    // Strings are `{id, id (same end means empty), cap}`; a non-empty name has end != begin.
    let none = Box::new([1u32, 1, 0]);
    let none_nonempty = Box::new([1u32, 2, 0]);
    let reverse = Box::new([2u32, 3, 0]);
    let ping_pong = Box::new([3u32, 4, 0]);
    let name = Box::new([10u32, 11, 0]);
    let sound_name = Box::new([20u32, 21, 0]);
    let empty_sound = Box::new([30u32, 30, 0]);
    let none_ptr = none.as_ptr() as u32;
    let _ = &none_nonempty;
    let mut this_block = Box::new([0u32; 0x80]);
    this_block[0xd0 / 4] = 0xdef0;
    let this = this_block.as_ptr() as u32;
    let mut done = Box::new(0xaau8);
    let done_ptr = done.as_mut() as *mut u8 as *const u8;

    struct Case {
        label: &'static str,
        anim: u32,
        sound: u32,
        seconds: u32,
        count: i32,
        special: u32,
        stopped: u8,
        moving: u8,
        pos_equal: bool,
        find_anim: u32,
        play_result: bool,
        play_empty: bool,
        expect_result: bool,
        expect_log: &'static [&'static str],
        /// moving, stopped, done flag.
        expect: (u8, u8, u8),
    }
    let n = name.as_ptr() as u32;
    let s = sound_name.as_ptr() as u32;
    let rev = reverse.as_ptr() as u32;
    let pp = ping_pong.as_ptr() as u32;
    let e = empty_sound.as_ptr() as u32;
    let other = name.as_ptr() as u32;
    #[rustfmt::skip]
    let cases = vec![
        Case { label: "gate fails", anim: n, sound: s, seconds: 0, count: 0, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0xa1, play_result: true, play_empty: false, expect_result: false, expect_log: &[], expect: (0, 1, 0xaa) },
        Case { label: "gate bypassed by count", anim: n, sound: s, seconds: 0, count: 2, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0xa1, play_result: true, play_empty: false, expect_result: true,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "find_anim(10)", "find_sound(20)", "links(0x42,0xa1,false,77,false,0,2)", "release(0x42)"], expect: (1, 0, 0) },
        Case { label: "reverse mode", anim: n, sound: s, seconds: 0, count: 0, special: rev, stopped: 1, moving: 0, pos_equal: false, find_anim: 0xa1, play_result: false, play_empty: false, expect_result: false,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "find_anim(10)", "find_sound(20)", "links(0x42,0xa1,true,77,false,0,0)", "release(0x42)"], expect: (1, 0, 0) },
        Case { label: "ping-pong mode", anim: n, sound: s, seconds: 6, count: 0, special: pp, stopped: 1, moving: 0, pos_equal: false, find_anim: 0xa1, play_result: true, play_empty: false, expect_result: true,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "find_anim(10)", "find_sound(20)", "links(0x42,0xa1,false,77,true,6,0)", "release(0x42)"], expect: (1, 0, 0) },
        Case { label: "none anim, real sound, default anim", anim: none_ptr, sound: s, seconds: 0, count: 1, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0, play_result: true, play_empty: false, expect_result: true,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "find_sound(20)", "links(0x42,0xdef0,false,77,false,0,1)", "release(0x42)"], expect: (1, 0, 0) },
        Case { label: "none anim, empty sound", anim: none_ptr, sound: e, seconds: 0, count: 1, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0, play_result: true, play_empty: false, expect_result: false,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "release(0x42)"], expect: (0, 1, 0xaa) },
        Case { label: "none anim, none sound", anim: none_ptr, sound: none_ptr, seconds: 0, count: 1, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0, play_result: true, play_empty: false, expect_result: false,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "release(0x42)"], expect: (0, 1, 0xaa) },
        Case { label: "named anim not found", anim: n, sound: s, seconds: 0, count: 1, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0, play_result: true, play_empty: false, expect_result: false,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "find_anim(10)", "release(0x42)"], expect: (0, 1, 0xaa) },
        Case { label: "empty sound is not looked up", anim: n, sound: e, seconds: 0, count: 1, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0xa1, play_result: true, play_empty: false, expect_result: true,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "find_anim(10)", "links(0x42,0xa1,false,0,false,0,1)", "release(0x42)"], expect: (1, 0, 0) },
        Case { label: "empty vectors", anim: n, sound: s, seconds: 0, count: 1, special: other, stopped: 1, moving: 0, pos_equal: false, find_anim: 0xa1, play_result: true, play_empty: true, expect_result: false,
               expect_log: &["default_anim(0)", "acquire(0xd000)", "find_anim(10)", "find_sound(20)", "links(0x42,0xa1,false,77,false,0,1)", "release(0x42)"], expect: (0, 1, 0xaa) },
        Case { label: "not stopped, idle", anim: n, sound: s, seconds: 0, count: 1, special: other, stopped: 0, moving: 0, pos_equal: true, find_anim: 0xa1, play_result: true, play_empty: false, expect_result: true, expect_log: &[], expect: (0, 0, 0xaa) },
        Case { label: "not stopped, moving, positions differ", anim: n, sound: s, seconds: 0, count: 1, special: other, stopped: 0, moving: 1, pos_equal: false, find_anim: 0xa1, play_result: true, play_empty: false, expect_result: true, expect_log: &[], expect: (1, 0, 0xaa) },
        Case { label: "not stopped, moving, positions match", anim: n, sound: s, seconds: 0, count: 1, special: other, stopped: 0, moving: 1, pos_equal: true, find_anim: 0xa1, play_result: true, play_empty: false, expect_result: true, expect_log: &[], expect: (0, 1, 1) },
    ];
    for case in &cases {
        set_byte(&mut block, 0xf8, case.moving);
        set_byte(&mut block, 0xf9, case.stopped);
        block[0x8c / 4] = 5;
        block[0x9c / 4] = if case.pos_equal { 5 } else { 6 };
        FIND_ANIM_RESULT.store(case.find_anim, Ordering::Relaxed);
        PLAY_RESULT.store(u32::from(case.play_result), Ordering::Relaxed);
        PLAY_EMPTY.store(u32::from(case.play_empty), Ordering::Relaxed);
        *done = 0xaa;
        LOG.lock().unwrap().clear();
        let args = FPlaySpecialArgs { this, anim_name: case.anim, sound_name: case.sound, seconds: case.seconds, count: case.count, special: case.special, unit, done: done_ptr };
        let result = unsafe { f_play_special(&args, &ops) };
        let got = (byte(&block, 0xf8), byte(&block, 0xf9), *done);
        let expected_log: Vec<String> = case.expect_log.iter().map(|s| s.to_string()).collect();
        if result != case.expect_result || *LOG.lock().unwrap() != expected_log || got != case.expect {
            failures.push(format!("{}: result {result}, log {:?}, state {got:?}; expected {}, {expected_log:?}, {:?}", case.label, LOG.lock().unwrap(), case.expect_result, case.expect));
        }
    }

    // No unit type or no default animation: false before anything is built.
    block[0x128 / 4] = 0;
    set_byte(&mut block, 0xf9, 1);
    LOG.lock().unwrap().clear();
    let args = FPlaySpecialArgs { this, anim_name: n, sound_name: s, seconds: 0, count: 1, special: other, unit, done: done_ptr };
    if unsafe { f_play_special(&args, &ops) } || !LOG.lock().unwrap().is_empty() {
        failures.push(format!("no unit type: log {:?}", LOG.lock().unwrap()));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_F`: port only, over a fake unit and recording stand-ins. Covers `fPlayF`,
/// `fPlayWithBaseF` and `fSetBaseF`: parameter-count gates, the not-stopped prefix, the cursor field, which
/// animations reach the second vector with which flags, the `fPlay` base flag, the flag writes and results.
pub(crate) fn run_ztaimgr_f_play_f_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_f, f_play_with_base_f, f_set_base_f, PlayOps};
    let test_name = "ZTAIMGR_F_PLAY_F";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static MIXER_ANIM: AtomicU32 = AtomicU32::new(0);
    static PLAY_RESULT: AtomicU32 = AtomicU32::new(0);
    static PLAY_EMPTY: AtomicU32 = AtomicU32::new(0);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    // A parameter's animation is its first word minus 1000, or 0 (not found) when that is zero.
    unsafe fn find_anim(_: u32, param: u32) -> u32 {
        let id = unsafe { *(param as *const u32) };
        log(format!("find({id})"));
        if id == 1000 { 0 } else { id }
    }
    unsafe fn mixer_anim(_: u32) -> u32 {
        log("mixer_anim".into());
        MIXER_ANIM.load(Ordering::Relaxed)
    }
    unsafe fn acquire_first(anim: u32) -> u32 {
        log(format!("first({anim:#x})"));
        0x42
    }
    unsafe fn release_first(_: u32) {
        log("release".into());
    }
    unsafe fn add_anim(_: u32, anim: u32, reverse: bool, ping_pong: bool) {
        log(format!("add({anim},{reverse},{ping_pong})"));
    }
    unsafe fn play(_: u32, _: u32, _: u32, with_base: bool) -> Option<bool> {
        log(format!("play({with_base})"));
        if PLAY_EMPTY.load(Ordering::Relaxed) != 0 { None } else { Some(PLAY_RESULT.load(Ordering::Relaxed) != 0) }
    }
    unsafe fn set_base(_: u32, anim: u32) {
        log(format!("set_base({anim})"));
    }
    extern "thiscall" fn v_default_anim(_: u32, arg: u32) -> u32 {
        log(format!("default({arg})"));
        0xd000
    }
    let ops = PlayOps { find_anim, mixer_anim, acquire_first, release_first, add_anim, play, set_base };

    let mut vtable = Box::new([0u32; 0x80]);
    vtable[0x8c / 4] = v_default_anim as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x200]);
    block[0] = vtable.as_ptr() as u32;
    let unit = block.as_ptr() as u32;
    let set_byte = |block: &mut Box<[u32; 0x200]>, offset: usize, value: u8| unsafe { *((block.as_ptr() as usize + offset) as *mut u8) = value };
    let byte = |block: &Box<[u32; 0x200]>, offset: usize| unsafe { *((block.as_ptr() as usize + offset) as *const u8) };
    let mut this_block = Box::new([0u32; 0x80]);
    let this = this_block.as_ptr() as u32;

    // `Call::new` strings hold bytes, so the animation id is carried by overwriting each parameter's begin word
    // with the id after construction.
    let make_call = |ids: &[u32]| -> Call {
        let texts: Vec<String> = ids.iter().map(|i| i.to_string()).collect();
        let refs: Vec<&str> = texts.iter().map(|s| s.as_str()).collect();
        let call = Call::new("fPlay", &refs);
        let begin = call.raw[3];
        for (i, id) in ids.iter().enumerate() {
            unsafe { *((begin + i as u32 * 12) as *mut u32) = *id };
        }
        call
    };
    let reset = |block: &mut Box<[u32; 0x200]>, stopped: u8, moving: u8| {
        set_byte(block, 0xf8, moving);
        set_byte(block, 0xf9, stopped);
        block[0x128 / 4] = 1;
        block[0x74 / 4 + 0x28 / 4] = 3;
        block[0x74 / 4 + 0x18 / 4] = 4;
    };

    // fPlayF
    let play_log = |entries: &[&str]| entries.iter().map(|s| s.to_string()).collect::<Vec<_>>();
    struct Case {
        label: &'static str,
        ids: Vec<u32>,
        stopped: u8,
        moving: u8,
        reverse: bool,
        ping_pong: bool,
        mixer_anim: u32,
        play_result: bool,
        play_empty: bool,
        expect_result: bool,
        expect_log: Vec<String>,
        /// moving, stopped, call flag.
        expect: (u8, u8, u8),
    }
    let cases = vec![
        Case { label: "no params", ids: vec![], stopped: 1, moving: 0, reverse: false, ping_pong: false, mixer_anim: 0, play_result: true, play_empty: false, expect_result: false, expect_log: play_log(&[]), expect: (0, 1, 0xaa) },
        Case { label: "not stopped idle", ids: vec![5], stopped: 0, moving: 0, reverse: false, ping_pong: false, mixer_anim: 0, play_result: true, play_empty: false, expect_result: true, expect_log: play_log(&[]), expect: (0, 0, 0xaa) },
        Case { label: "not stopped moving, has anim", ids: vec![5], stopped: 0, moving: 1, reverse: false, ping_pong: false, mixer_anim: 9, play_result: true, play_empty: false, expect_result: true, expect_log: play_log(&["mixer_anim"]), expect: (1, 0, 0xaa) },
        Case { label: "plays", ids: vec![5, 1000, 7], stopped: 1, moving: 0, reverse: true, ping_pong: false, mixer_anim: 0, play_result: true, play_empty: false, expect_result: true,
               expect_log: play_log(&["default(0)", "first(0xd000)", "find(5)", "add(5,true,false)", "find(1000)", "find(7)", "add(7,true,false)", "play(false)", "release"]), expect: (1, 0, 0) },
        Case { label: "ping-pong", ids: vec![5], stopped: 1, moving: 0, reverse: false, ping_pong: true, mixer_anim: 0, play_result: false, play_empty: false, expect_result: false,
               expect_log: play_log(&["default(0)", "first(0xd000)", "find(5)", "add(5,false,true)", "play(false)", "release"]), expect: (1, 0, 0) },
        Case { label: "nothing found", ids: vec![1000], stopped: 1, moving: 0, reverse: false, ping_pong: false, mixer_anim: 0, play_result: true, play_empty: true, expect_result: false,
               expect_log: play_log(&["default(0)", "first(0xd000)", "find(1000)", "play(false)", "release"]), expect: (0, 1, 0xaa) },
    ];
    for case in &cases {
        reset(&mut block, case.stopped, case.moving);
        MIXER_ANIM.store(case.mixer_anim, Ordering::Relaxed);
        PLAY_RESULT.store(u32::from(case.play_result), Ordering::Relaxed);
        PLAY_EMPTY.store(u32::from(case.play_empty), Ordering::Relaxed);
        let call = make_call(&case.ids);
        call.raw_mut_flag(0xaa);
        LOG.lock().unwrap().clear();
        let result = unsafe { f_play_f(this, call.ptr() as u32, unit, case.reverse, case.ping_pong, &ops) };
        let got = (byte(&block, 0xf8), byte(&block, 0xf9), call.flag());
        if result != case.expect_result || *LOG.lock().unwrap() != case.expect_log || got != case.expect {
            failures.push(format!("fPlayF {}: result {result}, log {:?}, state {got:?}; expected {}, {:?}, {:?}", case.label, LOG.lock().unwrap(), case.expect_result, case.expect_log, case.expect));
        }
    }
    // The cursor ends at the parameter-vector end.
    let call = make_call(&[5, 6]);
    reset(&mut block, 1, 0);
    let _ = unsafe { f_play_f(this, call.ptr() as u32, unit, false, false, &ops) };
    if this_block[0xcc / 4] != call.raw[4] {
        failures.push(format!("cursor ends at {:#x}, expected {:#x}", this_block[0xcc / 4], call.raw[4]));
    }

    // Moving unit stopped by a null mixer animation.
    reset(&mut block, 0, 1);
    block[0x74 / 4 + 0x28 / 4] = 1;
    block[0x74 / 4 + 0x18 / 4] = 1;
    MIXER_ANIM.store(0, Ordering::Relaxed);
    let call = make_call(&[5]);
    call.raw_mut_flag(0xaa);
    LOG.lock().unwrap().clear();
    let result = unsafe { f_play_f(this, call.ptr() as u32, unit, false, false, &ops) };
    if !result || byte(&block, 0xf9) != 1 || byte(&block, 0xf8) != 0 || call.flag() != 1 || !LOG.lock().unwrap().is_empty() {
        failures.push(format!("moving, equal counters: result {result}, log {:?}", LOG.lock().unwrap()));
    }
    reset(&mut block, 0, 1);
    MIXER_ANIM.store(9, Ordering::Relaxed);
    LOG.lock().unwrap().clear();
    let result = unsafe { f_play_f(this, call.ptr() as u32, unit, false, false, &ops) };
    if !result || byte(&block, 0xf9) != 0 || byte(&block, 0xf8) != 1 || *LOG.lock().unwrap() != play_log(&["mixer_anim"]) {
        failures.push(format!("moving with animation: result {result}, log {:?}", LOG.lock().unwrap()));
    }
    // No unit type, no default animation.
    reset(&mut block, 1, 0);
    block[0x128 / 4] = 0;
    LOG.lock().unwrap().clear();
    if unsafe { f_play_f(this, call.ptr() as u32, unit, false, false, &ops) } || !LOG.lock().unwrap().is_empty() {
        failures.push("fPlayF without unit type should fail silently".into());
    }

    // fPlayWithBaseF
    reset(&mut block, 1, 0);
    PLAY_RESULT.store(1, Ordering::Relaxed);
    PLAY_EMPTY.store(0, Ordering::Relaxed);
    let one = make_call(&[5]);
    LOG.lock().unwrap().clear();
    if unsafe { f_play_with_base_f(this, one.ptr() as u32, unit, false, false, &ops) } || !LOG.lock().unwrap().is_empty() {
        failures.push("fPlayWithBaseF with one parameter should fail".into());
    }
    let missing_base = make_call(&[1000, 6]);
    LOG.lock().unwrap().clear();
    if unsafe { f_play_with_base_f(this, missing_base.ptr() as u32, unit, false, false, &ops) } || *LOG.lock().unwrap() != play_log(&["find(1000)"]) {
        failures.push(format!("fPlayWithBaseF with a missing base: log {:?}", LOG.lock().unwrap()));
    }
    let with_base = make_call(&[4, 6, 1000, 8]);
    with_base.raw_mut_flag(0xaa);
    LOG.lock().unwrap().clear();
    let result = unsafe { f_play_with_base_f(this, with_base.ptr() as u32, unit, true, true, &ops) };
    let expected = play_log(&["find(4)", "first(0x4)", "find(6)", "add(6,true,true)", "find(1000)", "find(8)", "add(8,true,true)", "play(true)", "release"]);
    if !result || *LOG.lock().unwrap() != expected || byte(&block, 0xf8) != 1 || byte(&block, 0xf9) != 0 || with_base.flag() != 0 {
        failures.push(format!("fPlayWithBaseF: result {result}, log {:?}", LOG.lock().unwrap()));
    }

    // fSetBaseF
    reset(&mut block, 1, 0);
    let two = make_call(&[5, 6]);
    LOG.lock().unwrap().clear();
    if unsafe { f_set_base_f(this, two.ptr() as u32, unit, &ops) } || !LOG.lock().unwrap().is_empty() {
        failures.push("fSetBaseF with two parameters should fail".into());
    }
    reset(&mut block, 0, 1);
    let found = make_call(&[5]);
    found.raw_mut_flag(0xaa);
    if !unsafe { f_set_base_f(this, found.ptr() as u32, unit, &ops) } || byte(&block, 0xf8) != 1 || byte(&block, 0xf9) != 0 || found.flag() != 0xaa {
        failures.push("fSetBaseF on a unit that is not stopped should return true untouched".into());
    }
    reset(&mut block, 1, 1);
    LOG.lock().unwrap().clear();
    let result = unsafe { f_set_base_f(this, found.ptr() as u32, unit, &ops) };
    if !result || *LOG.lock().unwrap() != play_log(&["find(5)", "set_base(5)"]) || byte(&block, 0xf8) != 0 || byte(&block, 0xf9) != 1 || found.flag() != 1 {
        failures.push(format!("fSetBaseF found: result {result}, log {:?}", LOG.lock().unwrap()));
    }
    reset(&mut block, 1, 1);
    let missing = make_call(&[1000]);
    missing.raw_mut_flag(0xaa);
    LOG.lock().unwrap().clear();
    let result = unsafe { f_set_base_f(this, missing.ptr() as u32, unit, &ops) };
    if result || *LOG.lock().unwrap() != play_log(&["find(1000)"]) || byte(&block, 0xf8) != 0 || byte(&block, 0xf9) != 1 || missing.flag() != 1 {
        failures.push(format!("fSetBaseF missing: result {result}, log {:?}", LOG.lock().unwrap()));
    }
    block[0x128 / 4] = 0;
    LOG.lock().unwrap().clear();
    if unsafe { f_set_base_f(this, found.ptr() as u32, unit, &ops) } || !LOG.lock().unwrap().is_empty() {
        failures.push("fSetBaseF without a unit type should fail".into());
    }
    let _ = &mut this_block;
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_FLY_F`: port only, over recording stand-ins. Covers `fFlyF`'s gates and the converted values,
/// strings, reverse flag and default/explicit limit handed to `fFly`, and `fPlaySetProbF`'s pair parsing:
/// odd and short parameter counts, the names and weights of the entry vector, and the result.
pub(crate) fn run_ztaimgr_f_fly_f_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_fly_f, f_play_set_prob_f, FlyOps, SetProbOps};
    let test_name = "ZTAIMGR_F_FLY_F";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static PROB_RESULT: AtomicU32 = AtomicU32::new(0);
    unsafe fn f_fly(this: u32, unit: u32, name: u32, value: u32, extra: u32, done: u32, reverse: u32, limit: u32) -> bool {
        let entry = format!("fly({this:#x},{unit:#x},{},{value},{},{done:#x},{reverse},{limit})", unsafe { read_vanilla_string(name) }, unsafe { read_vanilla_string(extra) });
        LOG.lock().unwrap().push(entry);
        true
    }
    // Weights are plain decimal text.
    unsafe fn parse_int(text: *const u8) -> u32 {
        unsafe { std::ffi::CStr::from_ptr(text as *const _) }.to_str().unwrap().parse().unwrap_or(9999)
    }
    // Reads the entry vector: `{begin, end, cap}` of 16-byte `(string, weight)` entries.
    unsafe fn play_set_prob(vector: u32, unit: u32) -> bool {
        let (begin, end) = (unsafe { *(vector as *const u32) }, unsafe { *((vector + 4) as *const u32) });
        let mut parts = Vec::new();
        let mut entry = begin;
        while entry < end {
            parts.push(format!("{}={}", unsafe { read_vanilla_string(entry) }, unsafe { *((entry + 12) as *const u32) }));
            entry += 16;
        }
        LOG.lock().unwrap().push(format!("prob({unit:#x},{})", parts.join(",")));
        PROB_RESULT.load(Ordering::Relaxed) != 0
    }
    extern "thiscall" fn fake_convert(_this: u32, param: u32, _unit: u32) -> u32 {
        unsafe { read_vanilla_string(param) }.parse().unwrap_or(9999)
    }
    let fly_ops = FlyOps { f_fly };
    let prob_ops = SetProbOps { parse_int, play_set_prob };
    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x34 / 4] = fake_convert as *const () as usize as u32;
    let mgr = Box::new([mgr_vtable.as_ptr() as u32]);
    let this = mgr.as_ptr() as u32;
    let unit = 0x5000_0000u32;

    let two = Call::new("fFly", &["glide", "4"]);
    let three = Call::new("fFly", &["glide", "4", "60"]);
    for (label, call, reverse, limit) in [("two", &two, 0u32, 100u32), ("three", &three, 1, 60)] {
        LOG.lock().unwrap().clear();
        let result = unsafe { f_fly_f(this, call.ptr() as u32, unit, reverse, &fly_ops) };
        let expected = format!("fly({this:#x},{unit:#x},glide,4,,{:#x},{reverse},{limit})", call.ptr() as u32 + 0x18);
        if !result || *LOG.lock().unwrap() != vec![expected.clone()] {
            failures.push(format!("fFlyF {label}: result {result}, log {:?}, expected {expected}", LOG.lock().unwrap()));
        }
    }
    let one = Call::new("fFly", &["glide"]);
    let four = Call::new("fFly", &["glide", "4", "60", "x"]);
    for (label, call, u) in [("null unit", &two, 0), ("one", &one, unit), ("four", &four, unit)] {
        LOG.lock().unwrap().clear();
        let result = unsafe { f_fly_f(this, call.ptr() as u32, u, 0, &fly_ops) };
        if result || !LOG.lock().unwrap().is_empty() {
            failures.push(format!("fFlyF {label}: result {result}, log {:?}", LOG.lock().unwrap()));
        }
    }

    // fPlaySetProbF
    for (label, params, result_flag, expected) in [
        ("pair", vec!["wave", "30", "nod", "70"], 1u32, Some("prob(0x5000,wave=30,nod=70)")),
        ("single pair", vec!["wave", "100"], 0, Some("prob(0x5000,wave=100)")),
        ("odd", vec!["wave", "30", "nod"], 1, None),
        ("one", vec!["wave"], 1, None),
        ("none", vec![], 1, None),
    ] {
        PROB_RESULT.store(result_flag, Ordering::Relaxed);
        let call = Call::new("fPlaySetProb", &params);
        LOG.lock().unwrap().clear();
        let result = unsafe { f_play_set_prob_f(call.ptr() as u32, 0x5000, &prob_ops) };
        let expected_log: Vec<String> = expected.iter().map(|s| s.to_string()).collect();
        let expected_result = expected.is_some() && result_flag != 0;
        if result != expected_result || *LOG.lock().unwrap() != expected_log {
            failures.push(format!("fPlaySetProbF {label}: result {result}, log {:?}, expected {expected_log:?}", LOG.lock().unwrap()));
        }
    }
    finish_test(test_name, failures, failure_log)
}
