//! `fPlaySpecialF`, `fPlaySound`, `fPlaySetProb`, `fPlaySet`, `fFly`, `fPlayWithSound`, `fPlayTime`.

use super::*;

/// `ZTAIMGR_F_PLAY_SPECIAL_F`: port only, over a fake call and recording stand-ins. Covers every branch of the
/// five-parameter rewrite (which strings replace the parameters, the new name, the handler chain, the second
/// flag and the `fPlaySpecial` arguments), the both-numbers failure and the parameter-count guards.
pub(crate) fn run_ztaimgr_f_play_special_f_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_special_f, FPlaySpecialArgs, PlaySpecialFOps, SpecialHandler, SpecialString};
    let test_name = "ZTAIMGR_F_PLAY_SPECIAL_F";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    unsafe fn text(string: u32) -> String {
        let (begin, end) = (unsafe { *(string as *const u32) }, unsafe { *((string + 4) as *const u32) });
        String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }).into_owned()
    }
    // Strings are identified by content: none, rev, pp, ma, mb.
    unsafe fn is_string(which: SpecialString, string: u32) -> bool {
        let expected = match which {
            SpecialString::None => "none",
            SpecialString::Reverse => "rev",
            SpecialString::PingPong => "pp",
            SpecialString::ModeA => "ma",
            SpecialString::ModeB => "mb",
        };
        unsafe { text(string) == expected }
    }
    unsafe fn convert(_: u32, param: u32, _: u32) -> i32 {
        unsafe { text(param) }.parse().unwrap_or(0)
    }
    unsafe fn replace_params(_: u32, strings: &[u32]) {
        log(format!("params[{}]", strings.iter().map(|s| unsafe { text(*s) }).collect::<Vec<_>>().join(",")));
    }
    unsafe fn set_name(_: u32, name: &[u8]) {
        log(format!("name:{}", String::from_utf8_lossy(name)));
    }
    unsafe fn forward(handler: SpecialHandler, _: u32, _: u32, _: u32) -> bool {
        log(format!("fwd:{handler:?}"));
        true
    }
    unsafe fn play_special(args: &FPlaySpecialArgs) -> bool {
        log(format!("special({},{},{},{},{})", unsafe { text(args.anim_name) }, unsafe { text(args.sound_name) }, args.seconds as i32, args.count, unsafe { text(args.special) }));
        true
    }
    let ops = PlaySpecialFOps { is_string, convert, replace_params, set_name, forward, play_special };

    let run = |params: [&str; 5]| -> (Option<bool>, Vec<String>, u8) {
        LOG.lock().unwrap().clear();
        let call = Call::new("fPlaySpecial", &params);
        unsafe { *((call.ptr() as u32 + 0x19) as *mut u8) = 0xff };
        let result = unsafe { f_play_special_f(0x1000, call.ptr() as u32, 0x2000, &ops) };
        let flag_b = unsafe { *((call.ptr() as u32 + 0x19) as *const u8) };
        (result, LOG.lock().unwrap().clone(), flag_b)
    };
    let s = |items: &[&str]| items.iter().map(|i| i.to_string()).collect::<Vec<_>>();

    #[allow(clippy::type_complexity)]
    let cases: Vec<([&str; 5], Option<bool>, Vec<String>, u8)> = vec![
        (["a", "none", "0", "0", "ma"], Some(true), s(&["params[a]", "name:fPlay", "fwd:Play", "fwd:PlayPingPong"]), 0xff),
        (["a", "none", "0", "0", "mb"], Some(true), s(&["params[a]", "name:fPlay", "fwd:Play", "fwd:PlayPingPong"]), 0xff),
        (["a", "none", "0", "0", "none"], Some(true), s(&["params[a]", "name:fPlay", "fwd:Play", "fwd:PlayPingPong"]), 0xff),
        (["a", "none", "0", "0", "rev"], Some(true), s(&["params[a]", "name:fPlayReverse", "fwd:PlayReverse", "fwd:PlayPingPong"]), 0xff),
        (["a", "none", "0", "0", "pp"], Some(true), s(&["params[a]", "name:fPlayPingPong", "fwd:PlayPingPong"]), 0xff),
        (["a", "none", "0", "0", "zz"], Some(true), s(&["params[a]", "fwd:PlayPingPong"]), 0xff),
        (["a", "snd", "0", "0", "ma"], Some(true), s(&["params[a,snd]", "name:fPlayWithSound", "fwd:PlayWithSound"]), 0xff),
        (["none", "snd", "0", "0", "mb"], Some(true), s(&["special(none,snd,0,0,mb)"]), 1),
        (["none", "snd", "0", "0", "ma"], Some(true), s(&["special(none,snd,0,0,ma)"]), 0),
        (["a", "none", "2", "0", "mb"], Some(true), s(&["params[a,2]", "name:fPlayTime", "fwd:PlayTime"]), 1),
        (["a", "none", "2", "0", "ma"], Some(true), s(&["params[a,2]", "name:fPlayTime", "fwd:PlayTime"]), 0),
        (["a", "none", "2", "0", "none"], Some(true), s(&["params[a,2]", "name:fPlayTime", "fwd:PlayTime"]), 0),
        (["a", "none", "2", "0", "zz"], Some(true), s(&["special(a,none,2000,0,zz)"]), 0xff),
        (["a", "snd", "2", "0", "ma"], Some(true), s(&["special(a,snd,2000,0,ma)"]), 0xff),
        (["a", "none", "0", "3", "ma"], Some(true), s(&["special(a,none,0,3,ma)"]), 0xff),
        (["a", "snd", "2", "3", "ma"], Some(false), s(&[]), 0xff),
    ];
    for (params, expected_result, expected_log, expected_flag_b) in cases {
        let (result, entries, flag_b) = run(params);
        if result != expected_result || entries != expected_log || flag_b != expected_flag_b {
            failures.push(format!("{params:?}: got {result:?} {entries:?} flagB {flag_b:#x}, expected {expected_result:?} {expected_log:?} flagB {expected_flag_b:#x}"));
        }
    }

    // Parameter-count guards: a set result flag with another count fails, a clear one defers.
    LOG.lock().unwrap().clear();
    let short = Call::new("fPlaySpecial", &["a", "none", "0", "0"]);
    if unsafe { f_play_special_f(0x1000, short.ptr() as u32, 0x2000, &ops) } != None {
        failures.push("four parameters with a clear flag should defer".to_string());
    }
    short.raw_mut_flag(1);
    if unsafe { f_play_special_f(0x1000, short.ptr() as u32, 0x2000, &ops) } != Some(false) {
        failures.push("four parameters with the flag set should fail".to_string());
    }
    if !LOG.lock().unwrap().is_empty() {
        failures.push(format!("guards must not touch the ops: {:?}", LOG.lock().unwrap()));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_SOUND`: port only, over a recording `fPlaySpecial` stand-in. `fPlaySound` must call it with
/// the animation text `0`, the caller's sound string, no duration or count, the plain-mode string pointer, the
/// unit and the done pointer, and return its result.
pub(crate) fn run_ztaimgr_f_play_sound_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_sound, FPlaySpecialArgs};
    let test_name = "ZTAIMGR_F_PLAY_SOUND";
    let mut failures: Vec<String> = Vec::new();

    static SEEN: Mutex<Vec<(u32, String, u32, u32, i32, u32, u32, u32)>> = Mutex::new(Vec::new());
    static RESULT: AtomicU32 = AtomicU32::new(0);
    unsafe fn record(args: &FPlaySpecialArgs) -> bool {
        let (begin, end) = (unsafe { *(args.anim_name as *const u32) }, unsafe { *((args.anim_name + 4) as *const u32) });
        let anim = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }).into_owned();
        SEEN.lock().unwrap().push((args.this, anim, args.sound_name, args.seconds, args.count, args.special, args.unit, args.done as u32));
        RESULT.load(Ordering::Relaxed) != 0
    }

    let done = [0u8; 4];
    for result in [true, false] {
        SEEN.lock().unwrap().clear();
        RESULT.store(u32::from(result), Ordering::Relaxed);
        let got = unsafe { f_play_sound(0x10, 0x20, 0x30, done.as_ptr(), 0x40, record) };
        let expected = vec![(0x10, "0".to_string(), 0x20, 0, 0, 0x40, 0x30, done.as_ptr() as u32)];
        if got != result || *SEEN.lock().unwrap() != expected {
            failures.push(format!("result {got} (expected {result}), saw {:x?}, expected {expected:x?}", SEEN.lock().unwrap()));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_SET_PROB`: port only, over a scripted `rand` and a recording `fPlaySet`. Covers the first-roll
/// hit, rerolls after misses (and how many `rand` calls each takes), the weight-sum and empty guards, and the
/// returned low byte.
pub(crate) fn run_ztaimgr_f_play_set_prob_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_set_prob, SetProbPickOps};
    let test_name = "ZTAIMGR_F_PLAY_SET_PROB";
    let mut failures: Vec<String> = Vec::new();

    static RANDS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static PLAYED: Mutex<Vec<(String, u32)>> = Mutex::new(Vec::new());
    static PLAY_RESULT: AtomicU32 = AtomicU32::new(1);
    unsafe fn rand() -> u32 {
        let mut rands = RANDS.lock().unwrap();
        assert!(!rands.is_empty(), "rand called more often than scripted");
        rands.remove(0)
    }
    unsafe fn play_set(name: u32, unit: u32) -> u32 {
        let (begin, end) = (unsafe { *(name as *const u32) }, unsafe { *((name + 4) as *const u32) });
        let text = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }).into_owned();
        PLAYED.lock().unwrap().push((text, unit));
        PLAY_RESULT.load(Ordering::Relaxed)
    }
    let ops = SetProbPickOps { rand, play_set };

    // Each entry is {begin, end, cap, weight} over a leaked name buffer.
    let make = |entries: &[(&'static str, u32)]| -> Box<[u32]> {
        entries
            .iter()
            .flat_map(|(name, weight)| {
                let begin = name.as_ptr() as u32;
                [begin, begin + name.len() as u32, begin + name.len() as u32, *weight]
            })
            .collect()
    };
    let run = |entries: &[(&'static str, u32)], rands: &[u32], result: u32| -> (bool, Vec<(String, u32)>, usize) {
        let words = make(entries);
        let header = [words.as_ptr() as u32, words.as_ptr() as u32 + words.len() as u32 * 4, words.as_ptr() as u32 + words.len() as u32 * 4];
        *RANDS.lock().unwrap() = rands.to_vec();
        PLAYED.lock().unwrap().clear();
        PLAY_RESULT.store(result, Ordering::Relaxed);
        let got = unsafe { f_play_set_prob(header.as_ptr() as u32, 0x99, &ops) };
        let left = RANDS.lock().unwrap().len();
        (got, PLAYED.lock().unwrap().clone(), left)
    };
    let one = |name: &str, unit: u32| vec![(name.to_string(), unit)];

    let two = [("a", 30), ("b", 70)];
    let three = [("x", 10), ("y", 10), ("z", 80)];
    let checks: Vec<(&str, (bool, Vec<(String, u32)>, usize), (bool, Vec<(String, u32)>, usize))> = vec![
        ("first roll hits", run(&two, &[0, 9], 1), (true, one("a", 0x99), 1)),
        ("roll 30 is the edge of the first entry", run(&two, &[29, 9], 1), (true, one("a", 0x99), 1)),
        ("reroll lands on the second", run(&two, &[40, 5], 1), (true, one("b", 0x99), 0)),
        ("two rerolls", run(&three, &[95, 89, 79], 1), (true, one("z", 0x99), 0)),
        ("play result 0 is false", run(&two, &[0], 0), (false, one("a", 0x99), 0)),
        ("low byte only", run(&two, &[0], 0x100), (false, one("a", 0x99), 0)),
        ("weights not 100", run(&[("a", 30), ("b", 60)], &[], 1), (false, vec![], 0)),
        ("no entries", run(&[], &[], 1), (false, vec![], 0)),
    ];
    for (label, got, expected) in checks {
        if got != expected {
            failures.push(format!("{label}: got {got:?}, expected {expected:?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_SET`: port only, over a fake unit and recording stand-ins for `initChild`, `setBS` and the
/// pending-set setter. The `BFBSCall` is built by the real vanilla constructor, so the stand-ins also check its
/// text and flag. Covers the null-unit and empty-name guards, the child path and all three non-child branches.
pub(crate) fn run_ztaimgr_f_play_set_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_set, PlaySetOps};
    let test_name = "ZTAIMGR_F_PLAY_SET";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static CHILD_RESULT: AtomicU32 = AtomicU32::new(0);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    unsafe fn describe(call: u32) -> String {
        let (begin, end) = (unsafe { *(call as *const u32) }, unsafe { *((call + 4) as *const u32) });
        let text = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }).into_owned();
        format!("{text}/{}", unsafe { *((call + 0xc) as *const u8) })
    }
    unsafe fn init_child(unit: u32, name: u32) -> bool {
        log(format!("child({unit:#x},{})", unsafe { describe(name) }.split('/').next().unwrap()));
        CHILD_RESULT.load(Ordering::Relaxed) != 0
    }
    unsafe fn set_bs(unit: u32, call: u32, force: u32, mode: u32) {
        log(format!("setBS({unit:#x},{},{force},{mode})", unsafe { describe(call) }));
    }
    unsafe fn set_pending(unit: u32, call: u32, force: u32) {
        log(format!("pending({unit:#x},{},{force})", unsafe { describe(call) }));
    }
    let ops = PlaySetOps { init_child, set_bs, set_pending };

    // A fake unit: pending set byte at +0x6d, pending call {begin, end, cap, flag} at +0xdc.
    let pending_text = b"old";
    let mut block = Box::new([0u32; 0x80]);
    let unit = block.as_ptr() as u32;
    let configure = |block: &mut Box<[u32; 0x80]>, has_pending_set: bool, pending_present: bool| {
        unsafe { *((block.as_ptr() as usize + 0x6d) as *mut u8) = u8::from(has_pending_set) };
        let begin = pending_text.as_ptr() as u32;
        block[0xdc / 4] = begin;
        block[0xe0 / 4] = if pending_present { begin + 3 } else { begin };
        block[0xe4 / 4] = begin + 3;
        block[0xe8 / 4] = 0;
    };
    let name_text = b"walk";
    let name = [name_text.as_ptr() as u32, name_text.as_ptr() as u32 + 4, name_text.as_ptr() as u32 + 4];
    let empty_name = [name_text.as_ptr() as u32; 3];
    let run = |unit: u32, name: &[u32; 3], child: bool| -> (bool, Vec<String>) {
        LOG.lock().unwrap().clear();
        let result = unsafe { f_play_set(name.as_ptr() as u32, unit, child, 1, &ops) };
        (result, LOG.lock().unwrap().clone())
    };
    let s = |items: &[&str]| items.iter().map(|i| i.to_string()).collect::<Vec<_>>();
    let pending_at = unit + 0xdc;
    let _ = pending_at;

    configure(&mut block, false, true);
    let mut checks: Vec<(&str, (bool, Vec<String>), (bool, Vec<String>))> = vec![
        ("null unit", run(0, &name, false), (false, vec![])),
        ("empty name", run(unit, &empty_name, false), (false, vec![])),
    ];
    CHILD_RESULT.store(1, Ordering::Relaxed);
    checks.push(("child true", run(unit, &name, true), (true, s(&[&format!("child({unit:#x},walk)")]))));
    CHILD_RESULT.store(0, Ordering::Relaxed);
    checks.push(("child false", run(unit, &name, true), (false, s(&[&format!("child({unit:#x},walk)")]))));

    configure(&mut block, true, false);
    checks.push(("pending set, empty pending", run(unit, &name, false), (true, s(&[&format!("pending({unit:#x},walk/1,0)")]))));
    configure(&mut block, true, true);
    checks.push(("pending set, pending present", run(unit, &name, false), (true, vec![])));
    configure(&mut block, false, false);
    checks.push(("no pending set, empty pending", run(unit, &name, false), (true, s(&[&format!("setBS({unit:#x},walk/1,0,2)")]))));
    configure(&mut block, false, true);
    checks.push((
        "no pending set, pending present",
        run(unit, &name, false),
        (true, s(&[&format!("setBS({unit:#x},old/0,0,2)"), &format!("pending({unit:#x},walk/1,0)")])),
    ));
    for (label, got, expected) in checks {
        if got != expected {
            failures.push(format!("{label}: got {got:?}, expected {expected:?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_FLY`: port only, over a fake unit with a fake vtable and recording stand-ins. Covers the three
/// not-stopped outcomes (set moving, arrived, still flying) and the start path (no animation, no/none/named
/// sound, the link arguments, speed and limit virtuals and the final flags).
pub(crate) fn run_ztaimgr_f_fly_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_fly, FFlyArgs, FlyCoreOps};
    let test_name = "ZTAIMGR_F_FLY";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ANIM: AtomicU32 = AtomicU32::new(0);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    extern "thiscall" fn v_find_anim(_: u32, name: u32) -> u32 {
        log(format!("find_anim({name:#x})"));
        ANIM.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_speed(_: u32, value: u32) -> u32 {
        log(format!("speed({value})"));
        0
    }
    extern "thiscall" fn v_limit(_: u32, value: u32) -> u32 {
        log(format!("limit({value})"));
        0
    }
    unsafe fn stop(_: u32, a: i8, b: i8) {
        log(format!("stop({a},{b})"));
    }
    unsafe fn set_moving(_: u32, a: i8, b: i8) {
        log(format!("moving({a},{b})"));
    }
    // A string whose first byte is `n` is the "no sound" string.
    unsafe fn is_no_sound(string: u32) -> bool {
        let begin = unsafe { *(string as *const u32) };
        unsafe { *(begin as *const u8) == b'n' }
    }
    unsafe fn find_sound(_: u32, string: u32) -> u32 {
        log(format!("find_sound({})", unsafe { *(*(string as *const u32) as *const u8) } as char));
        0x77
    }
    unsafe fn play_single(_: u32, anim: u32, flag: u8, sound: u32) {
        log(format!("single({anim:#x},{flag},{sound:#x})"));
    }
    let ops = FlyCoreOps { stop, set_moving, is_no_sound, find_sound, play_single };

    let mut vtable = Box::new([0u32; 0x80]);
    vtable[0x130 / 4] = v_find_anim as *const () as usize as u32;
    vtable[0x1c0 / 4] = v_speed as *const () as usize as u32;
    vtable[0x1c4 / 4] = v_limit as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x100]);
    block[0] = vtable.as_ptr() as u32;
    let unit = block.as_ptr() as u32;
    let byte_at = |offset: u32| unsafe { *((unit + offset) as *const u8) };
    let set_byte = |offset: u32, value: u8| unsafe { *((unit + offset) as *mut u8) = value };
    let reset = |block: &mut Box<[u32; 0x100]>, stopped: u8, moving: u8, d7: u8, c20: u8, at_target: bool| {
        set_byte(0xf9, stopped);
        set_byte(0xf8, moving);
        set_byte(0xd7, d7);
        set_byte(0x20c, c20);
        for word in 0..3 {
            block[0x114 / 4 + word] = 10 + word as u32;
            block[0x1dc / 4 + word] = if at_target { 10 + word as u32 } else { 99 };
        }
    };
    let done = Box::new([0u8; 4]);
    let letters = b"nxyz";
    let none = [letters.as_ptr() as u32, letters.as_ptr() as u32 + 1, letters.as_ptr() as u32 + 1];
    let named = [letters.as_ptr() as u32 + 1, letters.as_ptr() as u32 + 2, letters.as_ptr() as u32 + 2];
    let empty = [letters.as_ptr() as u32 + 2; 3];
    let run = |extra: &[u32; 3]| -> (bool, Vec<String>) {
        LOG.lock().unwrap().clear();
        let args = FFlyArgs { unit, name: 0x5a, value: 7, extra: extra.as_ptr() as u32, done: done.as_ptr(), reverse: 1, limit: 33 };
        (unsafe { f_fly(&args, &ops) }, LOG.lock().unwrap().clone())
    };
    let s = |items: &[&str]| items.iter().map(|i| i.to_string()).collect::<Vec<_>>();

    reset(&mut block, 0, 1, 1, 0, false);
    let got = run(&none);
    if got != (true, s(&["moving(1,0)"])) || byte_at(0xf8) != 1 {
        failures.push(format!("set moving: {got:?}"));
    }
    reset(&mut block, 0, 1, 0, 0, false);
    let got = run(&none);
    if got != (true, vec![]) {
        failures.push(format!("still flying: {got:?}"));
    }
    reset(&mut block, 0, 0, 1, 0, true);
    let got = run(&none);
    if got != (true, vec![]) {
        failures.push(format!("not moving: {got:?}"));
    }
    reset(&mut block, 0, 1, 0, 0, true);
    let got = run(&none);
    if got != (true, s(&["speed(0)", "stop(1,0)"])) || byte_at(0xf9) != 1 || byte_at(0xf8) != 0 || done[0] != 1 {
        failures.push(format!("arrived: {got:?}, stopped {} moving {} done {}", byte_at(0xf9), byte_at(0xf8), done[0]));
    }

    reset(&mut block, 1, 0, 0, 0, false);
    ANIM.store(0, Ordering::Relaxed);
    let got = run(&none);
    if got != (false, s(&["find_anim(0x5a)"])) || byte_at(0xf8) != 0 {
        failures.push(format!("no animation: {got:?}"));
    }
    ANIM.store(0x1234, Ordering::Relaxed);
    for (label, extra, sound_log, sound) in [("none", &none, None, 0u32), ("empty", &empty, None, 0), ("named", &named, Some("find_sound(x)"), 0x77)] {
        reset(&mut block, 1, 0, 0, 0, false);
        let got = run(extra);
        let mut expected = vec!["find_anim(0x5a)".to_string()];
        expected.extend(sound_log.map(str::to_string));
        expected.extend(s(&[&format!("single(0x1234,1,{sound:#x})"), "speed(7)", "limit(33)"]));
        if got != (true, expected.clone()) || byte_at(0xf8) != 1 || byte_at(0xf9) != 0 {
            failures.push(format!("start with {label} sound: {got:?}, expected {expected:?}, moving {} stopped {}", byte_at(0xf8), byte_at(0xf9)));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_WITH_SOUND`: port only, over a fake unit and recording stand-ins. Covers the not-stopped
/// outcomes, each early failure (no entity type, no default animation, no animation) and the three play results
/// with the flags and `done` byte they leave.
pub(crate) fn run_ztaimgr_f_play_with_sound_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_with_sound, PlayWithSoundOps};
    let test_name = "ZTAIMGR_F_PLAY_WITH_SOUND";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static DEFAULT_ANIM: AtomicU32 = AtomicU32::new(0);
    static FOUND_ANIM: AtomicU32 = AtomicU32::new(0);
    static PLAY_RESULT: AtomicU32 = AtomicU32::new(0); // 0 none, 1 false, 2 true
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    extern "thiscall" fn v_default_anim(_: u32, arg: u32) -> u32 {
        log(format!("default({arg})"));
        DEFAULT_ANIM.load(Ordering::Relaxed)
    }
    unsafe fn find_anim(_: u32, name: u32) -> u32 {
        log(format!("find_anim({name:#x})"));
        FOUND_ANIM.load(Ordering::Relaxed)
    }
    unsafe fn find_sound(_: u32, name: u32) -> u32 {
        log(format!("find_sound({name:#x})"));
        0x77
    }
    unsafe fn play_two(_: u32, first: u32, second: u32, sound: u32) -> Option<bool> {
        log(format!("play({first:#x},{second:#x},{sound:#x})"));
        match PLAY_RESULT.load(Ordering::Relaxed) {
            0 => None,
            1 => Some(false),
            _ => Some(true),
        }
    }
    let ops = PlayWithSoundOps { find_anim, find_sound, play_two };

    let mut vtable = Box::new([0u32; 0x80]);
    vtable[0x8c / 4] = v_default_anim as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x100]);
    block[0] = vtable.as_ptr() as u32;
    let unit = block.as_ptr() as u32;
    let byte_at = |offset: u32| unsafe { *((unit + offset) as *const u8) };
    let set_byte = |offset: u32, value: u8| unsafe { *((unit + offset) as *mut u8) = value };
    let done = Box::new([5u8; 4]);
    let setup = |block: &mut Box<[u32; 0x100]>, stopped: u8, moving: u8, positions_equal: bool, unit_type: u32| {
        set_byte(0xf9, stopped);
        set_byte(0xf8, moving);
        block[0x8c / 4] = 1;
        block[0x9c / 4] = if positions_equal { 1 } else { 2 };
        block[0x128 / 4] = unit_type;
        unsafe { *(done.as_ptr() as *mut u8) = 5 };
    };
    let run = || -> (bool, Vec<String>) {
        LOG.lock().unwrap().clear();
        (unsafe { f_play_with_sound(0xa1, 0xb2, unit, done.as_ptr(), &ops) }, LOG.lock().unwrap().clone())
    };
    let s = |items: &[&str]| items.iter().map(|i| i.to_string()).collect::<Vec<_>>();

    setup(&mut block, 0, 0, true, 1);
    let got = run();
    if got != (true, vec![]) || byte_at(0xf9) != 0 || done[0] != 5 {
        failures.push(format!("not moving: {got:?}"));
    }
    setup(&mut block, 0, 1, false, 1);
    let got = run();
    if got != (true, vec![]) || byte_at(0xf8) != 1 || done[0] != 5 {
        failures.push(format!("moving, positions differ: {got:?}"));
    }
    setup(&mut block, 0, 1, true, 1);
    let got = run();
    if got != (true, vec![]) || byte_at(0xf9) != 1 || byte_at(0xf8) != 0 || done[0] != 1 {
        failures.push(format!("arrived: {got:?}, stopped {} moving {} done {}", byte_at(0xf9), byte_at(0xf8), done[0]));
    }

    DEFAULT_ANIM.store(0x100, Ordering::Relaxed);
    FOUND_ANIM.store(0x200, Ordering::Relaxed);
    setup(&mut block, 1, 0, true, 0);
    let got = run();
    if got != (false, vec![]) {
        failures.push(format!("no entity type: {got:?}"));
    }
    DEFAULT_ANIM.store(0, Ordering::Relaxed);
    setup(&mut block, 1, 0, true, 1);
    let got = run();
    if got != (false, s(&["default(0)"])) {
        failures.push(format!("no default animation: {got:?}"));
    }
    DEFAULT_ANIM.store(0x100, Ordering::Relaxed);
    FOUND_ANIM.store(0, Ordering::Relaxed);
    setup(&mut block, 1, 0, true, 1);
    let got = run();
    if got != (false, s(&["default(0)", "find_anim(0xa1)"])) {
        failures.push(format!("no animation: {got:?}"));
    }
    FOUND_ANIM.store(0x200, Ordering::Relaxed);
    for (label, result, expected_result, flags_set) in [("played true", 2, true, true), ("played false", 1, false, true), ("nothing played", 0, false, false)] {
        PLAY_RESULT.store(result, Ordering::Relaxed);
        setup(&mut block, 1, 0, true, 1);
        let got = run();
        let expected_log = s(&["default(0)", "find_anim(0xa1)", "find_sound(0xb2)", "play(0x100,0x200,0x77)"]);
        let expected_flags = if flags_set { (1, 0, 0) } else { (0, 1, 5) };
        if got != (expected_result, expected_log.clone()) || (byte_at(0xf8), byte_at(0xf9), done[0]) != expected_flags {
            failures.push(format!("{label}: {got:?}, flags {:?}", (byte_at(0xf8), byte_at(0xf9), done[0])));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_TIME`: port only, over a fake unit and recording stand-ins. Covers the not-stopped outcomes,
/// each early failure, the milliseconds handed to the link builder and the three play results with the flags
/// and `done` byte they leave.
pub(crate) fn run_ztaimgr_f_play_time_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_play_time, PlayTimeOps};
    let test_name = "ZTAIMGR_F_PLAY_TIME";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static DEFAULT_ANIM: AtomicU32 = AtomicU32::new(0);
    static FOUND_ANIM: AtomicU32 = AtomicU32::new(0);
    static PLAY_RESULT: AtomicU32 = AtomicU32::new(0); // 0 none, 1 false, 2 true
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    extern "thiscall" fn v_default_anim(_: u32, arg: u32) -> u32 {
        log(format!("default({arg})"));
        DEFAULT_ANIM.load(Ordering::Relaxed)
    }
    unsafe fn find_anim(_: u32, name: u32) -> u32 {
        log(format!("find_anim({name:#x})"));
        FOUND_ANIM.load(Ordering::Relaxed)
    }
    unsafe fn play_timed(_: u32, first: u32, second: u32, millis: i32) -> Option<bool> {
        log(format!("play({first:#x},{second:#x},{millis})"));
        match PLAY_RESULT.load(Ordering::Relaxed) {
            0 => None,
            1 => Some(false),
            _ => Some(true),
        }
    }
    let ops = PlayTimeOps { find_anim, play_timed };

    let mut vtable = Box::new([0u32; 0x80]);
    vtable[0x8c / 4] = v_default_anim as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x100]);
    block[0] = vtable.as_ptr() as u32;
    let unit = block.as_ptr() as u32;
    let byte_at = |offset: u32| unsafe { *((unit + offset) as *const u8) };
    let set_byte = |offset: u32, value: u8| unsafe { *((unit + offset) as *mut u8) = value };
    let done = Box::new([5u8; 4]);
    let setup = |block: &mut Box<[u32; 0x100]>, stopped: u8, moving: u8, positions_equal: bool, unit_type: u32| {
        set_byte(0xf9, stopped);
        set_byte(0xf8, moving);
        block[0x8c / 4] = 1;
        block[0x9c / 4] = if positions_equal { 1 } else { 2 };
        block[0x128 / 4] = unit_type;
        unsafe { *(done.as_ptr() as *mut u8) = 5 };
    };
    let run = || -> (bool, Vec<String>) {
        LOG.lock().unwrap().clear();
        (unsafe { f_play_time(0xa1, 7000, unit, done.as_ptr(), &ops) }, LOG.lock().unwrap().clone())
    };
    let s = |items: &[&str]| items.iter().map(|i| i.to_string()).collect::<Vec<_>>();

    setup(&mut block, 0, 0, true, 1);
    let got = run();
    if got != (true, vec![]) || byte_at(0xf9) != 0 || done[0] != 5 {
        failures.push(format!("not moving: {got:?}"));
    }
    setup(&mut block, 0, 1, false, 1);
    let got = run();
    if got != (true, vec![]) || byte_at(0xf8) != 1 || done[0] != 5 {
        failures.push(format!("moving, positions differ: {got:?}"));
    }
    setup(&mut block, 0, 1, true, 1);
    let got = run();
    if got != (true, vec![]) || byte_at(0xf9) != 1 || byte_at(0xf8) != 0 || done[0] != 1 {
        failures.push(format!("arrived: {got:?}, stopped {} moving {} done {}", byte_at(0xf9), byte_at(0xf8), done[0]));
    }

    DEFAULT_ANIM.store(0x100, Ordering::Relaxed);
    FOUND_ANIM.store(0x200, Ordering::Relaxed);
    setup(&mut block, 1, 0, true, 0);
    let got = run();
    if got != (false, vec![]) {
        failures.push(format!("no entity type: {got:?}"));
    }
    DEFAULT_ANIM.store(0, Ordering::Relaxed);
    setup(&mut block, 1, 0, true, 1);
    let got = run();
    if got != (false, s(&["default(0)"])) {
        failures.push(format!("no default animation: {got:?}"));
    }
    DEFAULT_ANIM.store(0x100, Ordering::Relaxed);
    FOUND_ANIM.store(0, Ordering::Relaxed);
    setup(&mut block, 1, 0, true, 1);
    let got = run();
    if got != (false, s(&["default(0)", "find_anim(0xa1)"])) {
        failures.push(format!("no animation: {got:?}"));
    }
    FOUND_ANIM.store(0x200, Ordering::Relaxed);
    for (label, result, expected_result, flags_set) in [("played true", 2, true, true), ("played false", 1, false, true), ("nothing played", 0, false, false)] {
        PLAY_RESULT.store(result, Ordering::Relaxed);
        setup(&mut block, 1, 0, true, 1);
        let got = run();
        let expected_log = s(&["default(0)", "find_anim(0xa1)", "play(0x100,0x200,7000)"]);
        let expected_flags = if flags_set { (1, 0, 0) } else { (0, 1, 5) };
        if got != (expected_result, expected_log.clone()) || (byte_at(0xf8), byte_at(0xf9), done[0]) != expected_flags {
            failures.push(format!("{label}: {got:?}, flags {:?}", (byte_at(0xf8), byte_at(0xf9), done[0])));
        }
    }
    finish_test(test_name, failures, failure_log)
}
