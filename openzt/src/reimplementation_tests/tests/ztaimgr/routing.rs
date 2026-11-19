//! Dispatch routing and animation refcount balance.

use super::*;

/// `ZTAIMGR_DISPATCH_ROUTING`: runs `ZTAIMgr::executeCall` and `BFAIMgr::executeCall` over a recording handler
/// table (no vanilla handler runs) for every call name, with a unit that is a `ZTUnitType` and one that is not.
/// Checks the handler reached, the `this`/call/unit arguments (the play family gets the unit as given, move,
/// fly, halt and every ZT handler the cast unit, null for a non-`ZTUnitType`), the trailing constants, that the
/// result is the handler's low byte and that the call's result flag is copied to `done`; unknown names reach
/// nothing; `fPlaySetTerrain` goes through the manager's vtable `+0x14` and `fKeepControl` answers from the cast.
pub(crate) fn run_ztaimgr_dispatch_routing_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::cell::RefCell;
    use std::sync::Mutex;

    use crate::ztaimgr::{execute_base_call_with, execute_call_with, Handler, Handlers};

    let test_name = "ZTAIMGR_DISPATCH_ROUTING";
    let mut failures: Vec<String> = Vec::new();

    type Seen = (Handler, u32, u32, u32, [u32; 2]);
    struct Recorder {
        seen: RefCell<Vec<Seen>>,
    }
    impl Handlers for Recorder {
        unsafe fn invoke(&self, handler: Handler, this: u32, call: u32, unit: u32, trailing: [u32; 2]) -> u32 {
            self.seen.borrow_mut().push((handler, this, call, unit, trailing));
            0xabcd_0000 | (handler as u32 + 1)
        }
    }
    let recorder = Recorder { seen: RefCell::new(Vec::new()) };

    static TERRAIN: Mutex<Vec<[u32; 3]>> = Mutex::new(Vec::new());
    extern "thiscall" fn fake_terrain(this: u32, call: u32, unit: u32) -> u32 {
        TERRAIN.lock().unwrap().push([this, call, unit]);
        0x7777_7742
    }
    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x14 / 4] = fake_terrain as *const () as usize as u32;
    let mgr = Box::new([mgr_vtable.as_ptr() as u32]);
    let this = mgr.as_ptr() as u32;

    let typed = StringFixtureUnit::new(false, true);
    let untyped = StringFixtureUnit::new(false, false);
    let units = [("unit type", typed.ptr(), typed.ptr()), ("other type", untyped.ptr(), 0)];

    use Handler as H;
    // (name, handler, cast unit, trailing) for the base chain.
    let z = [0u32, 0];
    #[rustfmt::skip]
    let base: Vec<(&str, H, bool, [u32; 2])> = vec![
        ("fPlay", H::PlayF, false, z), ("fSetBase", H::SetBaseF, false, z), ("fPlayWithBase", H::PlayWithBaseF, false, z),
        ("fMoveEntityDelta", H::MoveEntityDeltaF, false, z), ("fMoveEntitySpecial", H::MoveEntitySpecialF, false, z),
        ("fPlayTime", H::PlayTimeF, false, z), ("fPlaySound", H::PlaySoundF, false, z), ("fPlaySpecial", H::PlaySpecialF, false, z),
        ("fPlaySp", H::PlaySpecialF, false, z), ("fPlaySetProb", H::PlaySetProbF, false, z), ("fPlayReverse", H::PlayReverseF, false, z),
        ("fPlayPingPong", H::PlayPingPongF, false, z), ("fPingPong", H::PlayPingPongF, false, z), ("fPlayWithSound", H::PlayWithSoundF, false, z),
        ("fPlaySFX", H::PlayWithSoundF, false, z), ("fPlayStopAtEnd", H::PlayStopAtEndF, false, z), ("fPlaySet", H::PlaySetF, false, z),
        ("fMoveSlow", H::MoveSlowF, true, z), ("fMoveMedium", H::MoveMediumF, true, z), ("fMove", H::MoveF, true, z),
        ("fMoveReverse", H::MoveReverseF, true, z), ("fMoveFast", H::MoveFastF, true, z), ("fMoveWithSound", H::MoveWithSoundF, true, z),
        ("fFaceDirection", H::FaceDirectionF, false, z), ("fFly", H::FlyF, true, z), ("fFlyReverse", H::FlyReverseF, true, z),
        ("fHalt", H::HaltF, true, z),
    ];
    #[rustfmt::skip]
    let zt: Vec<(&str, H)> = vec![
        ("fWalk", H::MoveSlowF), ("fSwim", H::MoveSlowF), ("fTrot", H::MoveMediumF), ("fRun", H::MoveFastF), ("fSpray", H::SprayF),
        ("fKeeperRoutine", H::KeeperRoutineF), ("fGuestRoutine", H::GuestRoutineF), ("fGuideRoutine", H::GuideRoutineF),
        ("fMaintRoutine", H::MaintRoutineF), ("fFaceTowardFood", H::FaceTowardFoodF), ("fFaceTowardTarget", H::FaceTowardTargetF),
        ("fShowPredator", H::ShowPredatorF), ("fHidePredator", H::HidePredatorF), ("fEnterBuilding", H::EnterBuildingF),
        ("fExitBuilding", H::ExitBuildingF), ("fDustBall", H::DustBallF), ("fKillPrey", H::KillPreyF), ("fCaughtPrey", H::CaughtPreyF),
        ("fDie", H::DieF), ("fTargetBuilding", H::TargetBuildingF), ("fHatch", H::HatchF), ("fEndTrick", H::EndTrickF),
        ("fReturnToBuilding", H::ReturnToBuildingF), ("fPlaySetTank", H::PlaySetTankF),
    ];

    let run = |zt_chain: bool, name: &str, unit: u32| -> (Vec<Seen>, u8, u8, u32) {
        let call = Call::new(name, &[]);
        call.raw_mut_flag(0x5a);
        let call_ptr = call.ptr() as u32;
        let mut done = 0u8;
        recorder.seen.borrow_mut().clear();
        let result = unsafe {
            if zt_chain {
                execute_call_with(&recorder, this, call_ptr, unit, &mut done);
                0
            } else {
                execute_base_call_with(&recorder, this, call_ptr, unit, &mut done)
            }
        };
        let seen = recorder.seen.borrow().clone();
        (seen, result, done, call_ptr)
    };

    // The base chain, called directly.
    for (name, handler, cast, trailing) in &base {
        for (unit_label, unit, cast_unit) in &units {
            let call = Call::new(name, &[]);
            call.raw_mut_flag(0x5a);
            let mut done = 0u8;
            recorder.seen.borrow_mut().clear();
            let result = unsafe { execute_base_call_with(&recorder, this, call.ptr() as u32, *unit, &mut done) };
            let expected_unit = if *cast { *cast_unit } else { *unit };
            let expected: Vec<Seen> = vec![(*handler, this, call.ptr() as u32, expected_unit, *trailing)];
            let expected_result = (0xabcd_0000u32 | (*handler as u32 + 1)) as u8;
            if *recorder.seen.borrow() != expected || result != expected_result || done != 0x5a {
                failures.push(format!("base {name} / {unit_label}: seen {:?}, result {result:#x}, done {done:#x}; expected {expected:?}, {expected_result:#x}", recorder.seen.borrow()));
            }
        }
    }
    // The ZT chain: ZT names reach their handler with the cast unit, everything else falls through to the base chain.
    for (name, handler) in &zt {
        for (unit_label, unit, cast_unit) in &units {
            let (seen, _, done, call_ptr) = run(true, name, *unit);
            let expected: Vec<Seen> = vec![(*handler, this, call_ptr, *cast_unit, [0, 0])];
            if seen != expected || done != 0x5a {
                failures.push(format!("zt {name} / {unit_label}: seen {seen:?}, done {done:#x}; expected {expected:?}"));
            }
        }
    }
    for (name, handler, cast, trailing) in &base {
        for (unit_label, unit, cast_unit) in &units {
            let (seen, _, done, call_ptr) = run(true, name, *unit);
            let expected_unit = if *cast { *cast_unit } else { *unit };
            let expected: Vec<Seen> = vec![(*handler, this, call_ptr, expected_unit, *trailing)];
            if seen != expected || done != 0x5a {
                failures.push(format!("zt chain falls through to {name} / {unit_label}: seen {seen:?}, done {done:#x}; expected {expected:?}"));
            }
        }
    }

    // Unknown names, an empty name and near misses reach nothing; the result is 0 and the flag is still copied.
    for name in ["", "fBogus", "fplay", "fPlayx", "fWalkx", "FPlay", "fMov"] {
        for zt_chain in [false, true] {
            let (seen, result, done, _) = run(zt_chain, name, typed.ptr());
            if !seen.is_empty() || result != 0 || done != 0x5a {
                failures.push(format!("unknown {name:?} (zt chain {zt_chain}): seen {seen:?}, result {result:#x}, done {done:#x}"));
            }
        }
    }

    // fKeepControl answers from the cast unit and calls nothing.
    for (unit_label, unit, cast_unit) in &units {
        let (seen, result, done, _) = run(false, "fKeepControl", *unit);
        if !seen.is_empty() || result != u8::from(*cast_unit != 0) || done != 0x5a {
            failures.push(format!("fKeepControl / {unit_label}: seen {seen:?}, result {result}, done {done:#x}"));
        }
    }

    // fPlaySetTerrain goes through the manager's vtable +0x14 with the unit as given, and nothing else runs.
    for (unit_label, unit, _) in &units {
        TERRAIN.lock().unwrap().clear();
        let call = Call::new("fPlaySetTerrain", &[]);
        call.raw_mut_flag(0x5a);
        let mut done = 0u8;
        recorder.seen.borrow_mut().clear();
        let result = unsafe { execute_base_call_with(&recorder, this, call.ptr() as u32, *unit, &mut done) };
        let terrain = TERRAIN.lock().unwrap().clone();
        if !recorder.seen.borrow().is_empty() || terrain != vec![[this, call.ptr() as u32, *unit]] || result != 0x42 || done != 0x5a {
            failures.push(format!("fPlaySetTerrain / {unit_label}: seen {:?}, vtable calls {terrain:x?}, result {result:#x}, done {done:#x}", recorder.seen.borrow()));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_ANIM_REFCOUNT_BALANCE`: runs every animation-link helper (`play_single`, `play_pair`, `play_links`,
/// `play_two`, `play_timed`, `set_base` and the `acquire_first`/`add_anim`/`play`/`release_first` handle
/// sequence) over fake animations whose reference counts start at 5, with the vanilla `fPlay`, mixer and duration
/// calls replaced by recorders. After every helper each animation's count must be back at 5 (a drift leaks an
/// animation or frees it early) and the animation cache stamp must have been written; the recorders also check
/// how many links each vector held when `fPlay` ran.
pub(crate) fn run_ztaimgr_anim_refcount_balance_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::MixerOps;

    let test_name = "ZTAIMGR_ANIM_REFCOUNT_BALANCE";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static DURATION: AtomicU32 = AtomicU32::new(100);
    fn link_count(vector: u32) -> u32 {
        unsafe { (*((vector + 4) as *const u32) - *(vector as *const u32)) / 12 }
    }
    unsafe fn ai_play(_this: u32, second: u32, first: u32, _unit: u32, with_base: bool) -> bool {
        LOG.lock().unwrap().push(format!("play(second {}, first {}, base {with_base})", link_count(second), link_count(first)));
        true
    }
    unsafe fn duration(_link: u32) -> i32 {
        DURATION.load(Ordering::Relaxed) as i32
    }
    unsafe fn clear_all(_mixer: u32) {
        LOG.lock().unwrap().push("clear_all".into());
    }
    unsafe fn set_set(_mixer: u32, _set: u32, _out: u8) {
        LOG.lock().unwrap().push("set_set".into());
    }
    unsafe fn set_base(_mixer: u32, vector: u32) {
        LOG.lock().unwrap().push(format!("set_base({})", link_count(vector)));
    }
    let mixer = MixerOps { ai_play, duration, clear_all, set_set, set_base };
    extern "thiscall" fn fake_dir_to_set(_this: u32, _direction: u32, out: *mut u32, _anim: u32, _flag: u32) -> u32 {
        unsafe { *out = 0 };
        0
    }

    // Three fake animations (reference count at +0x20, cache stamp at +0x1c) and a fake animation cache.
    let mut anims = vec![[0u32; 16]; 3];
    for anim in anims.iter_mut() {
        anim[0x20 / 4] = 5;
    }
    let anim_ptr = |i: usize| anims.as_ptr() as u32 + i as u32 * 64;
    let counts = |anims: &Vec<[u32; 16]>| -> Vec<u32> { anims.iter().map(|a| unsafe { std::ptr::read_volatile(&a[0x20 / 4]) }).collect() };
    let mut cache = Box::new([0u32; 16]);
    cache[0x20 / 4] = 0x1234;
    let cache_cell = tag(0x0023_8044) as *mut u32;
    let saved_cache = unsafe { *cache_cell };
    unsafe { *cache_cell = cache.as_ptr() as u32 };

    let mut vtable = Box::new([0u32; 0x80]);
    vtable[0x80 / 4] = fake_dir_to_set as *const () as usize as u32;
    let mut unit_block = Box::new([0u32; 0x200]);
    unit_block[0] = vtable.as_ptr() as u32;
    let unit = unit_block.as_ptr() as u32;
    let this = 0x7000u32;
    let (a, b, c) = (anim_ptr(0), anim_ptr(1), anim_ptr(2));

    let check = |failures: &mut Vec<String>, label: &str, expected_log: &[&str], anims: &Vec<[u32; 16]>| {
        let got = LOG.lock().unwrap().clone();
        let expected: Vec<String> = expected_log.iter().map(|s| s.to_string()).collect();
        let balance = counts(anims);
        if got != expected || balance != vec![5, 5, 5] {
            failures.push(format!("{label}: log {got:?}, expected {expected:?}; reference counts {balance:?}"));
        }
        LOG.lock().unwrap().clear();
        for anim in unsafe { std::slice::from_raw_parts_mut(anims.as_ptr() as *mut [u32; 16], anims.len()) } {
            anim[0x1c / 4] = 0;
        }
    };

    unsafe {
        use crate::ztaimgr::live_support::{acquire_first_vanilla, add_anim_vanilla, play_links_with, play_pair_with, play_single_with, play_timed_with, play_two_with, play_with, release_first_vanilla, set_base_with};

        LOG.lock().unwrap().clear();
        DURATION.store(100, Ordering::Relaxed);

        play_single_with(&mixer, unit, a, 1, 9);
        let stamped = (0x1234, anims[0][0x1c / 4]);
        if stamped.0 != stamped.1 {
            failures.push(format!("play_single did not stamp the animation: {stamped:?}"));
        }
        check(&mut failures, "play_single", &["set_base(1)"], &anims);

        for (seconds, extra) in [(300, 2), (301, 3), (100, 0), (0, 0)] {
            let played = play_pair_with(&mixer, this, unit, a, b, 1, 9, seconds);
            let second = 1 + extra;
            check(&mut failures, &format!("play_pair {seconds} ms (returned {played})"), &[&format!("play(second {second}, first 1, base false)")], &anims);
        }
        DURATION.store(0, Ordering::Relaxed);
        play_pair_with(&mixer, this, unit, a, b, 0, 0, 300);
        check(&mut failures, "play_pair with a zero duration", &["play(second 2, first 1, base false)"], &anims);
        DURATION.store(100, Ordering::Relaxed);

        // (seconds, count, ping_pong, expected links in the second vector)
        for (seconds, count, ping_pong, links) in [(300u32, 0, false, 3), (500, 0, true, 6), (0, 3, false, 3), (0, 3, true, 6), (0, 1, false, 1), (0, 1, true, 2), (0, 0, false, 1)] {
            let first = acquire_first_vanilla(a);
            let played = play_links_with(&mixer, this, unit, first, b, false, 7, ping_pong, seconds, count);
            release_first_vanilla(first);
            check(&mut failures, &format!("play_links {seconds} ms x{count} pp {ping_pong} (returned {played:?})"), &[&format!("play(second {links}, first 1, base false)")], &anims);
        }

        play_two_with(&mixer, unit, a, b, 7);
        check(&mut failures, "play_two", &["play(second 1, first 1, base false)"], &anims);

        play_timed_with(&mixer, unit, a, b, 350);
        check(&mut failures, "play_timed", &["play(second 4, first 1, base false)"], &anims);
        DURATION.store(0, Ordering::Relaxed);
        play_timed_with(&mixer, unit, a, b, 350);
        check(&mut failures, "play_timed with a zero duration", &["play(second 2, first 1, base false)"], &anims);
        DURATION.store(100, Ordering::Relaxed);

        set_base_with(&mixer, unit, c);
        check(&mut failures, "set_base", &["clear_all", "set_set", "set_base(1)"], &anims);

        // The handle sequence `fPlayF` uses: first link, parameter animations (one with ping-pong), play, release.
        let handle = acquire_first_vanilla(a);
        add_anim_vanilla(handle, b, false, false);
        add_anim_vanilla(handle, c, true, true);
        let played = play_with(&mixer, this, unit, handle, true);
        release_first_vanilla(handle);
        check(&mut failures, &format!("handle sequence (returned {played:?})"), &["play(second 3, first 1, base true)"], &anims);

        // A handle released without being played.
        let handle = acquire_first_vanilla(b);
        add_anim_vanilla(handle, a, false, true);
        release_first_vanilla(handle);
        check(&mut failures, "handle released unplayed", &[], &anims);
    }

    unsafe { *cache_cell = saved_cache };
    let _ = (&mut unit_block, &cache, &vtable);
    finish_test(test_name, failures, failure_log)
}
