//! Real-vs-port comparisons over real vanilla bodies for the move and play families and the wrappers.

use super::*;

/// `ZTAIMGR_F_MOVE_REVERSE_F`: calls the real `fMoveReverseF` address (hooked, so it enters the port, which
/// forwards to the `fMoveF` port with `reverse` set) with a call of the wrong parameter count. `fMoveF` fails
/// that call before touching the unit, so the result is `0` and the stack must come back balanced (`RET 8`).
pub(crate) fn run_ztaimgr_f_move_reverse_f_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_F_MOVE_REVERSE_F";
    let mut failures: Vec<String> = Vec::new();
    let reverse = unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32, *const u32, u32) -> u32>(0x004a_75a7) };
    let marker = std::hint::black_box(0x1357_9bdfu32);
    for params in [&[][..], &["only"][..], &["a", "b", "c"][..]] {
        let call = Call::new("fMoveReverse", params);
        let result = unsafe { reverse(0x1000, call.ptr(), 0) };
        if result as u8 != 0 {
            failures.push(format!("{} parameters: result {result:#x}, expected 0", params.len()));
        }
    }
    if std::hint::black_box(marker) != 0x1357_9bdf {
        failures.push("marker corrupted".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_MOVE_REAL_VS_PORT`: runs the real vanilla `fMove` and the port over identical fake units
/// (own vtable, cog goal and target ring) on every path that touches only flags and fake virtuals. Compares
/// the result, the done byte, the unit flags `fMove` writes, whether the target list was emptied and the order
/// of fake-virtual calls. `BFUnit::stop` runs for real on both sides (its second argument also reaches
/// `idle`, whose `getIdleAnim` virtual is faked to return no animation), so the target-list nodes come from
/// `PoolAlloc` in the 24-byte size class `stop` returns them to. The paths that reach the mixer or
/// `BFCogGoal::reachedTarget` stay in the port-only test.
pub(crate) fn run_ztaimgr_f_move_real_vs_port_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    let test_name = "ZTAIMGR_F_MOVE_REAL_VS_PORT";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static HAS_TARGET: AtomicU32 = AtomicU32::new(0);
    static CAN_PLAY: AtomicU32 = AtomicU32::new(0);
    static VALID: AtomicU32 = AtomicU32::new(0);
    static ANIM_FIRST: AtomicU32 = AtomicU32::new(0);
    fn log(entry: &str) {
        LOG.lock().unwrap().push(entry.to_string());
    }
    extern "thiscall" fn v_has_target(_: u32) -> u32 {
        log("has_target");
        HAS_TARGET.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_can_play(_: u32) -> u32 {
        log("can_play");
        CAN_PLAY.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_prepare(_: u32, arg: u32) -> u32 {
        LOG.lock().unwrap().push(format!("prepare({arg:#x})"));
        0
    }
    extern "thiscall" fn v_find_anim(_: u32, _name: u32) -> u32 {
        log("find_anim");
        ANIM_FIRST.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_find_anim_second(_: u32) -> u32 {
        log("find_anim_second");
        ANIM_FIRST.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_set_speed(_: u32, arg: u32) -> u32 {
        LOG.lock().unwrap().push(format!("set_speed({arg})"));
        0
    }
    extern "thiscall" fn v_set_limit(_: u32, arg: u32) -> u32 {
        LOG.lock().unwrap().push(format!("set_limit({arg})"));
        0
    }
    extern "thiscall" fn v_idle_anim(_: u32, arg: u32) -> u32 {
        LOG.lock().unwrap().push(format!("idle_anim({})", arg as u8));
        0
    }
    extern "thiscall" fn c_valid(_: u32) -> u32 {
        log("cog_valid");
        VALID.load(Ordering::Relaxed)
    }
    extern "thiscall" fn c_clear(_: u32) -> u32 {
        log("cog_clear");
        0
    }
    extern "thiscall" fn c_clear_sub(_: u32) -> u32 {
        log("cog_clear_sub");
        0
    }

    let mut unit_vtable = Box::new([0u32; 0x80]);
    unit_vtable[0x180 / 4] = v_has_target as *const () as usize as u32;
    unit_vtable[0x184 / 4] = v_can_play as *const () as usize as u32;
    unit_vtable[0x1b0 / 4] = v_prepare as *const () as usize as u32;
    unit_vtable[0x130 / 4] = v_find_anim as *const () as usize as u32;
    unit_vtable[0x134 / 4] = v_find_anim_second as *const () as usize as u32;
    unit_vtable[0x1c0 / 4] = v_set_speed as *const () as usize as u32;
    unit_vtable[0x1c4 / 4] = v_set_limit as *const () as usize as u32;
    unit_vtable[0x8c / 4] = v_idle_anim as *const () as usize as u32;
    let mut cog_vtable = Box::new([0u32; 0x10]);
    cog_vtable[0] = c_valid as *const () as usize as u32;
    cog_vtable[0xc / 4] = c_clear as *const () as usize as u32;
    cog_vtable[0x1c / 4] = c_clear_sub as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x200]);
    let unit = block.as_ptr() as u32;
    let mut ring = vec![[0u32; 2]; 8];
    let sentinel = ring.as_ptr() as u32;
    let empty_extra = Box::new([0x1000u32, 0x1000, 0x1000]);
    let this_block = Box::new([0u32; 0x40]);
    let mut done = Box::new(0xaau8);
    let done_ptr = done.as_mut() as *mut u8 as *const u8;
    let byte = |block: &[u32; 0x200], offset: usize| unsafe { *((block.as_ptr() as usize + offset) as *const u8) };

    struct Case {
        label: &'static str,
        flags: &'static [(usize, u8)],
        state: u32,
        state2: u32,
        list: usize,
        can_play: bool,
        valid: bool,
        has_target: bool,
        anim_first: u32,
    }
    let c = |label, flags, state, state2, list, can_play, valid, has_target, anim_first| Case { label, flags, state, state2, list, can_play, valid, has_target, anim_first };
    let cases = [
        c("finished", &[(0x20d, 1)][..], 0, 0, 0, false, false, false, 0xa000),
        c("finished prepares", &[(0x20d, 1)], 1, 0, 0, false, false, false, 0xa000),
        c("cog invalid", &[(0xf9, 1)], 0, 0, 0, true, false, false, 0xa000),
        c("cog valid, no flag", &[(0xf9, 1)], 0, 0, 0, true, true, false, 0xa000),
        c("cog valid, flag, invalid", &[(0xf9, 1), (0x1a8, 1)], 0, 0, 0, true, true, false, 0xa000),
        c("cog valid, flag, state 1", &[(0xf9, 1), (0x1a8, 1)], 1, 0, 0, true, true, false, 0xa000),
        c("stopped, empty list, cannot play", &[(0xf9, 1)], 0, 0, 0, false, false, false, 0xa000),
        c("stopped, cog already invalid", &[(0xf9, 1), (0x20e, 1)], 0, 0, 0, true, true, false, 0xa000),
        c("moving one target has target", &[(0xf8, 1)], 0, 0, 1, false, false, true, 0xa000),
        c("moving two targets has target", &[(0xf8, 1)], 0, 0, 2, false, false, true, 0xa000),
        c("idle", &[], 0, 0, 2, false, false, false, 0xa000),
        c("idle has target", &[], 1, 0, 0, false, false, true, 0xa000),
        c("missing animation", &[(0xf9, 1)], 0, 0, 2, false, false, false, 0),
        c("stopped, one target: stops", &[(0xf9, 1)], 0, 0, 1, false, false, false, 0xa000),
        c("stopped, one target, state 3 (missing animation)", &[(0xf9, 1)], 0, 3, 1, false, false, false, 0),
        c("moving, empty list: stops", &[(0xf8, 1)], 0, 0, 0, false, false, false, 0xa000),
        c("moving, empty list, flag 20c", &[(0xf8, 1), (0x20c, 1)], 0, 0, 0, false, false, false, 0xa000),
    ];

    let node_ptrs = std::cell::RefCell::new(Vec::<u32>::new());
    let setup = |block: &mut Box<[u32; 0x200]>, ring: &mut Vec<[u32; 2]>, case: &Case| {
        block.fill(0);
        block[0] = unit_vtable.as_ptr() as u32;
        block[0x15c / 4] = cog_vtable.as_ptr() as u32;
        for &(offset, value) in case.flags {
            unsafe { *((block.as_ptr() as usize + offset) as *mut u8) = value };
        }
        block[0x170 / 4] = case.state;
        block[0x1bc / 4] = case.state2;
        let mut nodes = node_ptrs.borrow_mut();
        nodes.clear();
        for _ in 0..case.list {
            nodes.push(unsafe { POOL_ALLOCATE.original()(24) } as u32);
        }
        ring[0] = [nodes.first().copied().unwrap_or(sentinel), nodes.last().copied().unwrap_or(sentinel)];
        for (i, &node) in nodes.iter().enumerate() {
            let next = nodes.get(i + 1).copied().unwrap_or(sentinel);
            let prev = if i == 0 { sentinel } else { nodes[i - 1] };
            unsafe { *(node as *mut [u32; 2]) = [next, prev] };
        }
        block[0x154 / 4] = sentinel;
        CAN_PLAY.store(u32::from(case.can_play), Ordering::Relaxed);
        VALID.store(u32::from(case.valid), Ordering::Relaxed);
        HAS_TARGET.store(u32::from(case.has_target), Ordering::Relaxed);
        ANIM_FIRST.store(case.anim_first, Ordering::Relaxed);
        LOG.lock().unwrap().clear();
    };

    for case in &cases {
        for seconds in [0i32, 5] {
            let mut outcomes: Vec<(bool, Vec<u8>, Vec<String>)> = Vec::new();
            for port in [false, true] {
                setup(&mut block, &mut ring, case);
                *done = 0xaa;
                let result = if port {
                    let args = live_support::FMoveArgs { this: this_block.as_ptr() as u32, first_param: 0x5555, seconds, unit, done: done_ptr, name: 0x3000, speed: 9, extra: empty_extra.as_ptr() as u32, reverse: 1, limit: 33 };
                    unsafe { live_support::f_move(&args, &live_support::F_MOVE_VANILLA_OPS) }
                } else {
                    live_support::real_f_move(this_block.as_ptr(), 0x5555, seconds, unit as *const i32, done_ptr, 0x3000, 9, empty_extra.as_ptr() as *const i8, 1, 33)
                };
                let emptied = ring[0] == [sentinel, sentinel];
                let flags: Vec<u8> = [0xf8usize, 0xf9, 0xd4, 0xd6, 0xd7, 0x20c, 0x20d, 0x20e, 0x1a8].iter().map(|&o| byte(&block, o)).chain([*done, u8::from(emptied)]).collect();
                if !emptied {
                    for &node in node_ptrs.borrow().iter() {
                        unsafe { POOL_DEALLOCATE.original()(node as *const u32, 24) };
                    }
                }
                outcomes.push((result, flags, LOG.lock().unwrap().clone()));
            }
            if outcomes[0] != outcomes[1] {
                failures.push(format!("{} (seconds {seconds}): real {:?}, port {:?}", case.label, outcomes[0], outcomes[1]));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_PLAY_FAMILY_REAL_VS_PORT`: real vanilla against the port (through the hooked address) for
/// `fPlaySpecial`, `fPlayF`, `fPlayWithBaseF`, `fPlayTime`, `fPlayWithSound` and `fFly`, over one fake unit
/// whose virtuals log their calls. Covers every path that never reaches the real mixer, `findAnim` or `BFUnit`
/// methods: the gates, the not-stopped prefix, a missing entity type, a missing default animation and `fFly`'s
/// failed lookup. Compares the result, the unit flags, the done or call flag, the parameter cursor and the
/// virtual-call log.
pub(crate) fn run_ztaimgr_play_family_real_vs_port_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::ffi::c_void;
    use std::sync::Mutex;

    use openzt_detour::generated::bfaimgr::{F_FLY, F_PLAY_F, F_PLAY_SPECIAL, F_PLAY_TIME, F_PLAY_WITH_BASE_F, F_PLAY_WITH_SOUND};

    let test_name = "ZTAIMGR_PLAY_FAMILY_REAL_VS_PORT";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    fn log(entry: &str) {
        LOG.lock().unwrap().push(entry.to_string());
    }
    extern "thiscall" fn v_default_anim(_: u32, _arg: u32) -> u32 {
        log("default_anim");
        0
    }
    extern "thiscall" fn v_find_anim(_: u32, _name: u32) -> u32 {
        log("find_anim");
        0
    }
    extern "thiscall" fn v_set_speed(_: u32, arg: u32) -> u32 {
        LOG.lock().unwrap().push(format!("set_speed({arg})"));
        0
    }
    let mut vtable = Box::new([0u32; 0x80]);
    vtable[0x8c / 4] = v_default_anim as *const () as usize as u32;
    vtable[0x130 / 4] = v_find_anim as *const () as usize as u32;
    vtable[0x1c0 / 4] = v_set_speed as *const () as usize as u32;
    let mut block = Box::new([0u32; 0x200]);
    // An empty target ring: the real `BFUnit::stop` walks it and resets it.
    let mut target_ring = Box::new([0u32; 2]);
    let mut this_block = Box::new([0u32; 0x80]);
    let mut done = Box::new(0xaau8);
    let unit = block.as_mut_ptr() as u32;
    let this = this_block.as_mut_ptr() as u32;
    let done_ptr = done.as_mut() as *mut u8;

    let wb = |offset: u32, value: u8| unsafe { *((unit + offset) as *mut u8) = value };
    let rb = |offset: u32| unsafe { *((unit + offset) as *const u8) };
    let ww = |offset: u32, value: u32| unsafe { *((unit + offset) as *mut u32) = value };

    #[derive(Clone, Copy)]
    struct State {
        stopped: u8,
        moving: u8,
        has_type: bool,
        pos_equal: bool,
        mixer_equal: bool,
    }
    let st = |stopped, moving, has_type, pos_equal, mixer_equal| State { stopped, moving, has_type, pos_equal, mixer_equal };
    let states = [
        ("stopped", st(1, 0, true, false, false)),
        ("stopped, no type", st(1, 0, false, false, false)),
        ("idle", st(0, 0, true, false, false)),
        ("moving, positions differ", st(0, 1, true, false, false)),
        ("moving, positions match", st(0, 1, true, true, true)),
    ];
    let mut apply = |s: State| {
        for offset in 0..0x200u32 {
            ww(offset * 4, 0);
        }
        ww(0, vtable.as_ptr() as u32);
        let ring = target_ring.as_mut_ptr() as u32;
        target_ring[0] = ring;
        target_ring[1] = ring;
        ww(0x154, ring);
        wb(0xf8, s.moving);
        wb(0xf9, s.stopped);
        ww(0x128, u32::from(s.has_type));
        ww(0x8c, 5);
        ww(0x9c, if s.pos_equal { 5 } else { 6 });
        // Mixer counters at `+0x74 + 0x28` and `+0x74 + 0x18`.
        ww(0x74 + 0x28, 3);
        ww(0x74 + 0x18, if s.mixer_equal { 3 } else { 4 });
        for offset in 0..0x80u32 {
            unsafe { *((this + offset * 4) as *mut u32) = 0 };
        }
        unsafe { *done_ptr = 0xaa };
        LOG.lock().unwrap().clear();
    };
    let observe = |result: bool, call: Option<&Call>| -> (bool, Vec<u8>, u32, Vec<String>) {
        let mut bytes: Vec<u8> = [0xf8u32, 0xf9, 0xd4, 0xd6, 0xd7, 0x20c, 0x20d, 0x20e].iter().map(|&o| rb(o)).collect();
        bytes.push(unsafe { *done_ptr });
        bytes.push(call.map_or(0, |c| c.flag()));
        (result, bytes, unsafe { *((this + 0xcc) as *const u32) }, LOG.lock().unwrap().clone())
    };

    // fPlaySpecial
    let read_cell = |rva: u32| unsafe { *(tag(rva) as *const u32) };
    let none = read_cell(0x0023_950c);
    let reverse = read_cell(0x0024_3620);
    let ping_pong = read_cell(0x0024_3624);
    let name = CallString::new("anim");
    let sound = CallString::new("snd");
    let (name_ptr, sound_ptr) = (name.raw.as_ptr() as u32, sound.raw.as_ptr() as u32);
    if none == 0 || reverse == 0 || ping_pong == 0 {
        failures.push("a special string cell is null".to_string());
    } else {
        struct Special {
            label: &'static str,
            anim: u32,
            sound: u32,
            seconds: i32,
            count: i32,
            special: u32,
        }
        let sp = |label, anim, sound, seconds, count, special| Special { label, anim, sound, seconds, count, special };
        let specials = [
            sp("gate fails", name_ptr, sound_ptr, 0, 0, name_ptr),
            sp("gate fails on no seconds", name_ptr, sound_ptr, 0, 0, none),
            sp("gate fails on none sound", name_ptr, none, 5, 0, name_ptr),
            sp("gate bypassed by seconds", name_ptr, sound_ptr, 5, 0, name_ptr),
            sp("gate bypassed by count", name_ptr, sound_ptr, 0, 2, name_ptr),
            sp("none anim", none, sound_ptr, 0, 0, name_ptr),
            sp("reverse", name_ptr, sound_ptr, 0, 0, reverse),
            sp("ping-pong", name_ptr, sound_ptr, 0, 0, ping_pong),
        ];
        for sp in &specials {
            for (state_label, state) in &states {
                let mut outcomes = Vec::new();
                for port in [false, true] {
                    apply(*state);
                    let (t, a, s, sa, u) = (this as *const u32, sp.anim as *const i8, sp.sound as *const i8, sp.special as *const i8, unit as *const i32);
                    let result = if port {
                        unsafe { F_PLAY_SPECIAL.hooked()(t, a, s, sp.seconds, sp.count, sa, u, done_ptr) }
                    } else {
                        live_support::real_f_play_special(t, a, s, sp.seconds, sp.count, sa, u, done_ptr)
                    };
                    outcomes.push(observe(result, None));
                }
                if outcomes[0] != outcomes[1] {
                    failures.push(format!("fPlaySpecial {} / {state_label}: real {:?}, port {:?}", sp.label, outcomes[0], outcomes[1]));
                }
            }
        }
    }

    // fPlayF and fPlayWithBaseF: parameter-count gates and the states above.
    let names = ["a", "b", "c"];
    for count in 0..=3usize {
        let call = Call::new("fPlay", &names[..count]);
        for (state_label, state) in &states {
            for (reverse_flag, ping_flag) in [(0i8, 0i8), (1, 0), (0, 1)] {
                for with_base in [false, true] {
                    // `fPlayWithBaseF` resolves its base animation through the real `findAnim` before anything
                    // else, which would dereference the fake entity type.
                    if with_base && count >= 2 && state.stopped == 1 && state.has_type {
                        continue;
                    }
                    let mut outcomes = Vec::new();
                    for port in [false, true] {
                        apply(*state);
                        call.raw_mut_flag(0xaa);
                        let (c, u, t) = (call.ptr(), unit as *const u32, this as *const u32);
                        let result = match (port, with_base) {
                            (false, false) => live_support::real_f_play_f(t, c, u, reverse_flag, ping_flag),
                            (true, false) => unsafe { F_PLAY_F.hooked()(t, c, u, reverse_flag, ping_flag) },
                            (false, true) => live_support::real_f_play_with_base_f(t, c, u, reverse_flag, ping_flag),
                            (true, true) => unsafe { F_PLAY_WITH_BASE_F.hooked()(t, c, u, reverse_flag, ping_flag) },
                        };
                        outcomes.push(observe(result, Some(&call)));
                    }
                    if outcomes[0] != outcomes[1] {
                        failures.push(format!(
                            "{} {count} params / {state_label} ({reverse_flag},{ping_flag}): real {:?}, port {:?}",
                            if with_base { "fPlayWithBaseF" } else { "fPlayF" },
                            outcomes[0],
                            outcomes[1]
                        ));
                    }
                }
            }
        }
    }

    // fPlayTime and fPlayWithSound.
    for (state_label, state) in &states {
        for millis in [0i32, 500] {
            let mut outcomes = Vec::new();
            for port in [false, true] {
                apply(*state);
                let (n, d) = (name_ptr as *const c_void, done_ptr as *const i8);
                let result = if port { unsafe { F_PLAY_TIME.hooked()(this as *const u32, n, millis, unit as *const u32, d, 0) } } else { live_support::real_f_play_time(this as *const u32, n, millis, unit as *const u32, d, 0) };
                outcomes.push(observe(result, None));
            }
            if outcomes[0] != outcomes[1] {
                failures.push(format!("fPlayTime {millis} / {state_label}: real {:?}, port {:?}", outcomes[0], outcomes[1]));
            }
        }
        let mut outcomes = Vec::new();
        for port in [false, true] {
            apply(*state);
            let (n, s, d) = (name_ptr as *const c_void, sound_ptr as *const c_void, done_ptr as *const i8);
            let result = if port { unsafe { F_PLAY_WITH_SOUND.hooked()(n, s, unit as *const u32, d) } } else { live_support::real_f_play_with_sound(n, s, unit as *const u32, d) };
            outcomes.push(observe(result, None));
        }
        if outcomes[0] != outcomes[1] {
            failures.push(format!("fPlayWithSound / {state_label}: real {:?}, port {:?}", outcomes[0], outcomes[1]));
        }
    }

    // fFly: a failed lookup and the not-stopped paths. The position words are `+0x114..` and the destination
    // words `+0x1dc..`; the moving case keeps them apart. The arrived and set-moving branches run the real
    // `BFUnit::stop` and `setMoving`, which touch only the unit's own bytes and the empty target ring.
    let extra = CallString::new("");
    let extra_ptr = extra.raw.as_ptr() as u32;
    let fly_states = [("stopped, no animation", st(1, 0, true, false, false), 0), ("idle", st(0, 0, true, false, false), 0), ("moving, away from destination", st(0, 1, true, false, false), 1), ("moving, arrived", st(0, 1, true, false, false), 0), ("moving, set moving", st(0, 1, true, false, false), 2), ("moving, flag d7 and 20c, arrived", st(0, 1, true, false, false), 3)];
    for (label, state, variant) in &fly_states {
        for reverse_flag in [0i32, 1] {
            let mut outcomes = Vec::new();
            for port in [false, true] {
                apply(*state);
                match variant {
                    1 => ww(0x114, 1),
                    2 => wb(0xd7, 1),
                    3 => {
                        wb(0xd7, 1);
                        wb(0x20c, 1);
                    }
                    _ => {}
                }
                let (n, e, d) = (name_ptr as *const c_void, extra_ptr as *const c_void, done_ptr as *const i8);
                let result = if port {
                    unsafe { F_FLY.hooked()(this as *const u32, unit as *const u32, n, 7, e, d, reverse_flag, 100) }
                } else {
                    live_support::real_f_fly(this as *const u32, unit as *const u32, n, 7, e, d, reverse_flag, 100)
                };
                outcomes.push(observe(result, None));
            }
            if outcomes[0] != outcomes[1] {
                failures.push(format!("fFly {label} (reverse {reverse_flag}): real {:?}, port {:?}", outcomes[0], outcomes[1]));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_WRAPPERS_REAL_VS_PORT`: real vanilla against the port (hooked address) for the `fMove*F` wrappers,
/// `fTargetBuilding`/`fTargetBuildingF`, `fSetBaseF`, `fPlaySet` and `fPlaySpecialF`, over fake units that never
/// reach the mixer, `findAnim` or `ZTAnimal` methods: a finished unit (so `fMove` returns at once), an idle unit
/// (so `fPlaySpecial` and its siblings return at once) and units without a type. Calls fake manager and unit
/// virtuals that log, so the order and arguments of the conversions are compared too. `fPlaySpecialF`'s rewrite
/// paths are not run: vanilla would free the call's strings through its own allocator. `fDustBall` is not run
/// at all: every path past its type check reaches the real `BFEntityType::findAnim`.
pub(crate) fn run_ztaimgr_wrappers_real_vs_port_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::ffi::c_void;
    use std::sync::Mutex;

    use openzt_detour::generated::bfaimgr::{F_MOVE_FAST_F, F_MOVE_F, F_MOVE_MEDIUM_F, F_MOVE_SLOW_F, F_MOVE_WITH_SOUND_F, F_PLAY_SET, F_PLAY_SPECIAL_F, F_SET_BASE_F};
    use openzt_detour::generated::ztaimgr::{F_TARGET_BUILDING, F_TARGET_BUILDING_F};

    let test_name = "ZTAIMGR_WRAPPERS_REAL_VS_PORT";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    extern "thiscall" fn fake_convert(_this: u32, param: u32, _unit: u32) -> u32 {
        let text = unsafe { read_vanilla_string(param) };
        LOG.lock().unwrap().push(format!("convert({text})"));
        text.parse().unwrap_or(9999)
    }
    extern "thiscall" fn v_name(_: u32) -> u32 {
        LOG.lock().unwrap().push("name".to_string());
        FAKE_NAME_PTR.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_name_120(_: u32) -> u32 {
        LOG.lock().unwrap().push("name120".to_string());
        FAKE_NAME_PTR.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_limit(_: u32) -> u32 {
        LOG.lock().unwrap().push("limit".to_string());
        55
    }
    extern "thiscall" fn v_has_target(_: u32) -> u32 {
        LOG.lock().unwrap().push("has_target".to_string());
        0
    }
    extern "thiscall" fn v_default_anim(_: u32, _: u32) -> u32 {
        LOG.lock().unwrap().push("default_anim".to_string());
        0
    }

    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x34 / 4] = fake_convert as *const () as usize as u32;
    let mut mgr_block = Box::new([0u32; 0x80]);
    mgr_block[0] = mgr_vtable.as_ptr() as u32;
    let this = mgr_block.as_ptr() as u32;

    let walk = "walk anim";
    let mut walk_bytes = walk.as_bytes().to_vec();
    walk_bytes.push(0);
    let walk_begin = walk_bytes.as_ptr() as u32;
    let walk_string = Box::new([walk_begin, walk_begin + walk.len() as u32, walk_begin + walk.len() as u32 + 1]);
    FAKE_NAME_PTR.store(walk_string.as_ptr() as u32, Ordering::Relaxed);

    // A fixture unit: animal/unit-type flags on its fake type, and every log-only virtual installed.
    let make_unit = |animal: bool, unit_type: bool| -> StringFixtureUnit {
        let mut unit = StringFixtureUnit::new(animal, unit_type);
        for step in 0..3usize {
            unit._vtable[0x120 / 4 + step] = if step == 0 { v_name_120 } else { v_name } as *const () as usize as u32;
            unit._vtable[0x140 / 4 + step] = v_limit as *const () as usize as u32;
            unit._ty[0x100 / 4 + step] = 0x100 + step as u32;
        }
        unit._vtable[0x128 / 4] = v_name as *const () as usize as u32;
        unit._vtable[0x180 / 4] = v_has_target as *const () as usize as u32;
        unit._vtable[0x8c / 4] = v_default_anim as *const () as usize as u32;
        unit
    };
    let reset = |unit: &mut StringFixtureUnit, finished: bool, stopped: bool, moving: bool, acquired: bool, targeting: bool| {
        for offset in [0xf8usize, 0xf9, 0x20c, 0x20d, 0x20e, 0x3b0, 0x3b1] {
            unit.set_byte(offset, 0);
        }
        unit.set_byte(0x20d, u8::from(finished));
        unit.set_byte(0xf9, u8::from(stopped));
        unit.set_byte(0xf8, u8::from(moving));
        unit.set_byte(0x3b1, u8::from(acquired));
        unit.set_byte(0x3b0, u8::from(targeting));
        LOG.lock().unwrap().clear();
    };
    type Outcome = (u32, Vec<u8>, Vec<u8>, Vec<String>);
    let observe = |result: u32, unit: &StringFixtureUnit, call: Option<&Call>, done: Option<&u8>| -> Outcome {
        let flags: Vec<u8> = [0xf8usize, 0xf9, 0x20c, 0x20d, 0x3b1].iter().map(|&o| unit.byte(o)).collect();
        let mut tail = Vec::new();
        if let Some(call) = call {
            tail.push(call.flag());
            tail.push(unsafe { *((call.ptr() as u32 + 0x19) as *const u8) });
        }
        if let Some(done) = done {
            tail.push(unsafe { std::ptr::read_volatile(done) });
        }
        (result, flags, tail, LOG.lock().unwrap().clone())
    };
    // `fMoveF`/`fMoveWithSoundF` call the unit's `+0x120` virtual before every gate and ignore the result; the
    // port skips that call, so it is removed from the real log for those two.
    let drop_name_120 = |mut outcome: Outcome| -> Outcome {
        outcome.3.retain(|entry| entry != "name120");
        outcome
    };
    let compare = |failures: &mut Vec<String>, label: String, real: Outcome, port: Outcome| {
        if real != port {
            failures.push(format!("{label}: real {real:?}, port {port:?}"));
        }
    };

    let texts: [&str; 7] = ["a", "2", "walk", "5", "purr", "70", "1"];
    let mut finished_unit = make_unit(false, true);
    let mut non_unit = make_unit(false, false);
    let finished_ptr = finished_unit.ptr();

    // fMoveF (counts 3..=6, both directions), fMoveWithSoundF (counts 4..=7).
    for count in 3..=7usize {
        for first in ["a", ""] {
            let mut params: Vec<&str> = texts[..count].to_vec();
            params[0] = first;
            let call = Call::new("fMove", &params);
            for reverse in [false, true] {
                let mut outcomes = Vec::new();
                for port in [false, true] {
                    reset(&mut finished_unit, true, false, false, false, false);
                    call.raw_mut_flag(0xaa);
                    let (c, u, t) = (call.ptr(), finished_ptr as *const u32, this as *const u32);
                    let result = if port { unsafe { F_MOVE_F.hooked()(t, c, u, reverse) } } else { live_support::real_f_move_f(t, c, u, reverse) };
                    outcomes.push(observe(result & 0xff, &finished_unit, Some(&call), None));
                }
                compare(&mut failures, format!("fMoveF {count} params first {first:?} reverse {reverse}"), drop_name_120(outcomes.remove(0)), outcomes.remove(0));
            }
            let mut outcomes = Vec::new();
            for port in [false, true] {
                reset(&mut finished_unit, true, false, false, false, false);
                call.raw_mut_flag(0xaa);
                let (c, u, t) = (call.ptr(), finished_ptr as *const u32, this as *const u32);
                let result = if port { unsafe { F_MOVE_WITH_SOUND_F.hooked()(t, c, u) } } else { live_support::real_f_move_with_sound_f(t, c, u) };
                outcomes.push(observe(result & 0xff, &finished_unit, Some(&call), None));
            }
            compare(&mut failures, format!("fMoveWithSoundF {count} params first {first:?}"), drop_name_120(outcomes.remove(0)), outcomes.remove(0));
        }
    }
    // fMoveF with a null unit and a unit that is not a `ZTUnitType`.
    let four = Call::new("fMove", &texts[..4]);
    for (label, u) in [("null unit", 0u32), ("non-unit type", non_unit.ptr())] {
        let mut outcomes = Vec::new();
        for port in [false, true] {
            reset(&mut non_unit, true, false, false, false, false);
            four.raw_mut_flag(0xaa);
            let (c, t) = (four.ptr(), this as *const u32);
            let result = if port { unsafe { F_MOVE_F.hooked()(t, c, u as *const u32, false) } } else { live_support::real_f_move_f(t, c, u as *const u32, false) };
            outcomes.push(observe(result & 0xff, &non_unit, Some(&four), None));
        }
        compare(&mut failures, format!("fMoveF {label}"), drop_name_120(outcomes.remove(0)), outcomes.remove(0));
    }

    // fMoveSlowF / fMoveMediumF / fMoveFastF.
    for count in 0..=3usize {
        for first in ["a", ""] {
            let mut params: Vec<&str> = texts[..count].to_vec();
            if count > 0 {
                params[0] = first;
            }
            let call = Call::new("fWalk", &params);
            for (which, unit_ptr, is_finished) in [("unit", finished_ptr, true), ("non-unit", non_unit.ptr(), true), ("null", 0, false)] {
                for variant in 0..3 {
                    let mut outcomes = Vec::new();
                    for port in [false, true] {
                        reset(&mut finished_unit, is_finished, false, false, false, false);
                        reset(&mut non_unit, is_finished, false, false, false, false);
                        call.raw_mut_flag(0xaa);
                        let (c, u, t) = (call.ptr() as i32, unit_ptr as *const i32, this as *const u32);
                        let result = unsafe {
                            match (variant, port) {
                                (0, false) => live_support::real_f_move_slow_f(t, c, u),
                                (0, true) => F_MOVE_SLOW_F.hooked()(t, c, u),
                                (1, false) => live_support::real_f_move_medium_f(t, c, u),
                                (1, true) => F_MOVE_MEDIUM_F.hooked()(t, c, u),
                                (_, false) => live_support::real_f_move_fast_f(t, c, u),
                                (_, true) => F_MOVE_FAST_F.hooked()(t, c, u),
                            }
                        };
                        outcomes.push(observe(result & 0xff, &finished_unit, Some(&call), None));
                    }
                    compare(&mut failures, format!("fMove speed {variant} F {count} params first {first:?} {which}"), outcomes.remove(0), outcomes.remove(0));
                }
            }
        }
    }

    // fTargetBuilding and fTargetBuildingF.
    let mut animal_unit = make_unit(true, true);
    let mut animal_not_unit = make_unit(true, false);
    let mut other = make_unit(false, true);
    let mut done = Box::new(0xaau8);
    let done_ptr = done.as_mut() as *mut u8;
    for (which, unit_index) in [("animal unit-type", 0), ("animal, not unit-type", 1), ("non-animal", 2)] {
        for (acquired, targeting) in [(false, false), (true, false), (true, true), (false, true)] {
            // `acquired == false` would call the real `ZTAnimal::targetBuilding`; only the acquired state is run.
            for use_f in [false, true] {
                if !acquired && !(use_f && !targeting) && !(unit_index == 2) {
                    continue;
                }
                let mut outcomes = Vec::new();
                let call = Call::new("fTargetBuilding", &[]);
                for port in [false, true] {
                    let unit = match unit_index {
                        0 => &mut animal_unit,
                        1 => &mut animal_not_unit,
                        _ => &mut other,
                    };
                    reset(unit, true, false, false, acquired, targeting);
                    unsafe { *done_ptr = 0xaa };
                    call.raw_mut_flag(0xaa);
                    let (t, u) = (this as *const u32, unit.ptr());
                    let result = if use_f {
                        if port { unsafe { F_TARGET_BUILDING_F.hooked()(t, call.ptr() as i32, u as *const i32) } } else { live_support::real_f_target_building_f(t, call.ptr() as i32, u as *const i32) }
                    } else if port {
                        unsafe { F_TARGET_BUILDING.hooked()(t, u as *const u32, done_ptr) }
                    } else {
                        live_support::real_f_target_building(t, u as *const u32, done_ptr)
                    };
                    outcomes.push(observe(u32::from(result), unit, Some(&call), Some(&*done)));
                }
                compare(&mut failures, format!("{} {which} acquired {acquired} targeting {targeting}", if use_f { "fTargetBuildingF" } else { "fTargetBuilding" }), outcomes.remove(0), outcomes.remove(0));
            }
        }
    }

    // fSetBaseF: gates and the not-stopped path.
    let mut set_base_unit = make_unit(false, false);
    for count in 0..=2usize {
        let call = Call::new("fSetBase", &texts[..count]);
        for (label, stopped) in [("stopped, no type", true), ("idle", false)] {
            let mut outcomes = Vec::new();
            for port in [false, true] {
                reset(&mut set_base_unit, false, stopped, false, false, false);
                unsafe { *((set_base_unit.ptr() + 0x128) as *mut u32) = 0 };
                call.raw_mut_flag(0xaa);
                let (c, u, t) = (call.ptr(), set_base_unit.ptr() as *const u32, this as *const u32);
                let result = if port { unsafe { F_SET_BASE_F.hooked()(t, c, u) } } else { live_support::real_f_set_base_f(t, c, u) };
                outcomes.push(observe(u32::from(result), &set_base_unit, Some(&call), None));
            }
            compare(&mut failures, format!("fSetBaseF {count} params {label}"), outcomes.remove(0), outcomes.remove(0));
        }
    }

    // fPlaySet: a null unit and an empty name.
    let name = CallString::new("set");
    let empty = CallString::new("");
    for (label, name_ptr, unit_ptr) in [("null unit", name.raw.as_ptr() as u32, 0u32), ("empty name", empty.raw.as_ptr() as u32, finished_ptr)] {
        for child in [0i8, 1] {
            let mut outcomes = Vec::new();
            for port in [false, true] {
                reset(&mut finished_unit, false, true, false, false, false);
                let result = if port {
                    unsafe { F_PLAY_SET.hooked()(name_ptr as *const i32, unit_ptr as *const c_void, child, 0) }
                } else {
                    live_support::real_f_play_set(name_ptr as *const i32, unit_ptr as *const c_void, child, 0)
                };
                outcomes.push(observe(u32::from(result), &finished_unit, None, None));
            }
            compare(&mut failures, format!("fPlaySet {label} child {child}"), outcomes.remove(0), outcomes.remove(0));
        }
    }

    // fPlaySpecialF: the call's result flag gates, and the paths that reach `fPlaySpecial` without rewriting
    // the call. The special strings are copied from vanilla's own.
    let global_text = |rva: u32| -> String {
        let cell = unsafe { *(tag(rva) as *const u32) };
        if cell == 0 {
            String::new()
        } else {
            unsafe { read_vanilla_string(cell) }
        }
    };
    let none_text = global_text(0x0023_950c);
    let mode_a = global_text(0x0024_bdac);
    let mut special_unit = make_unit(false, false);
    let rows: Vec<(&str, Vec<String>, u8)> = vec![
        ("four with flag", ["a", "s", "0", "0"].iter().map(|s| s.to_string()).collect(), 1),
        ("six with flag", ["a", "s", "0", "0", "m", "x"].iter().map(|s| s.to_string()).collect(), 1),
        ("seconds and repeat", vec!["a".into(), "s".into(), "2".into(), "3".into(), mode_a.clone()], 0),
        ("repeat", vec!["a".into(), "s".into(), "0".into(), "2".into(), mode_a.clone()], 0),
        ("seconds, named sound", vec!["a".into(), "s".into(), "3".into(), "0".into(), mode_a.clone()], 0),
        ("none anim, named sound", vec![none_text.clone(), "s".into(), "0".into(), "0".into(), mode_a.clone()], 0),
        ("none anim, named sound, flag set", vec![none_text.clone(), "s".into(), "0".into(), "0".into(), mode_a.clone()], 1),
    ];
    for (label, params, flag) in &rows {
        for (state_label, stopped, has_type) in [("idle", false, false), ("stopped, no type", true, false), ("stopped, typed", true, true)] {
            let refs: Vec<&str> = params.iter().map(|s| s.as_str()).collect();
            let call = Call::new("fPlaySpecial", &refs);
            let mut outcomes = Vec::new();
            for port in [false, true] {
                reset(&mut special_unit, false, stopped, false, false, false);
                unsafe { *((special_unit.ptr() + 0x128) as *mut u32) = if has_type { special_unit._ty.as_ptr() as u32 } else { 0 } };
                call.raw_mut_flag(*flag);
                unsafe { *((call.ptr() as u32 + 0x19) as *mut u8) = 0 };
                let (c, u, t) = (call.ptr(), special_unit.ptr() as *const u32, this as *const u32);
                let result = if port { unsafe { F_PLAY_SPECIAL_F.hooked()(t, c, u) } } else { live_support::real_f_play_special_f(t, c, u) };
                outcomes.push(observe(u32::from(result), &special_unit, Some(&call), None));
            }
            compare(&mut failures, format!("fPlaySpecialF {label} / {state_label}"), outcomes.remove(0), outcomes.remove(0));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// The game's `rand` state: one dword at `0x00638060`.
pub(super) const RVA_RAND_STATE: u32 = 0x0023_8060;

pub(super) fn rand_state() -> u32 {
    unsafe { *(tag(RVA_RAND_STATE) as *const u32) }
}

pub(super) fn set_rand_state(value: u32) {
    unsafe { *(tag(RVA_RAND_STATE) as *mut u32) = value };
}

/// `ZTAIMGR_F_PLAY_SET_PROB_REAL_VS_PORT`: real vanilla `fPlaySetProb` against the port, both starting from the
/// same snapshot of the game's `rand` state. The unit is null, so `fPlaySet` returns false without side
/// effects whichever entry is chosen; what is compared is the result and the final `rand` state, which encodes
/// how many rolls the walk consumed. Weights summing to 100 (several shapes), not 100, and empty.
pub(crate) fn run_ztaimgr_f_play_set_prob_real_vs_port_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::ffi::c_void;

    use openzt_detour::generated::bfaimgr::F_PLAY_SET_PROB;

    let test_name = "ZTAIMGR_F_PLAY_SET_PROB_REAL_VS_PORT";
    let mut failures: Vec<String> = Vec::new();
    let saved = rand_state();
    let mut advanced = 0u32;

    let shapes: [&[u32]; 11] = [&[100], &[30, 70], &[10, 10, 80], &[50, 50], &[0, 100], &[20, 30, 50], &[1, 99], &[25, 25, 25, 25], &[99, 1], &[1, 1, 1, 1, 96], &[30, 60]];
    let names = ["a", "b", "c", "d", "e"];
    for (shape_index, weights) in shapes.iter().enumerate() {
        let strings: Vec<CallString> = weights.iter().enumerate().map(|(i, _)| CallString::new(names[i])).collect();
        let words: Vec<u32> = strings.iter().zip(weights.iter()).flat_map(|(s, w)| [s.raw[0], s.raw[1], s.raw[2], *w]).collect();
        let begin = words.as_ptr() as u32;
        let header = [begin, begin + words.len() as u32 * 4, begin + words.len() as u32 * 4];
        for seed in 0..64u32 {
            let state = seed.wrapping_mul(0x9e37_79b1).wrapping_add(0x1234_5678);
            set_rand_state(state);
            let real = live_support::real_f_play_set_prob(header.as_ptr() as *const i32, std::ptr::null::<c_void>()) & 0xff;
            let real_end = rand_state();
            set_rand_state(state);
            let port = unsafe { F_PLAY_SET_PROB.hooked()(header.as_ptr() as *const i32, std::ptr::null::<c_void>()) } as u32 & 0xff;
            let port_end = rand_state();
            advanced += u32::from(real_end != state);
            if (real, real_end) != (port, port_end) {
                failures.push(format!("shape {shape_index} {weights:?} seed {seed}: real ({real}, {real_end:#x}), port ({port}, {port_end:#x})"));
            }
        }
    }
    // Empty vector.
    let empty = [0u32; 3];
    set_rand_state(0x55);
    let real = (live_support::real_f_play_set_prob(empty.as_ptr() as *const i32, std::ptr::null::<c_void>()) & 0xff, rand_state());
    set_rand_state(0x55);
    let port = (unsafe { F_PLAY_SET_PROB.hooked()(empty.as_ptr() as *const i32, std::ptr::null::<c_void>()) } as u32 & 0xff, rand_state());
    if real != port || port.1 != 0x55 {
        failures.push(format!("empty: real {real:?}, port {port:?}"));
    }
    set_rand_state(saved);
    if advanced == 0 {
        failures.push("the rand state never advanced".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_RANDOM_WALK_REAL_VS_PORT`: real vanilla `fRandomWalk` against the port over a fake 20x20 map
/// (the world global is pointed at it), a fake unit and manager whose virtuals log, both starting from the same
/// `rand` snapshot. Only the first two phases are run (a queued target, a standing destination, the random
/// perimeter walk): the neighbour scan calls the real `getNeighbors`/`getPathCost`, which are not safe on a
/// fake map. Every configuration keeps at least one walkable perimeter tile and every path attempt succeeds, so
/// the neighbour scan is never reached. Compares the result, the guard byte, the call log and the final `rand`
/// state.
pub(crate) fn run_ztaimgr_f_random_walk_real_vs_port_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use openzt_detour::generated::bfaimgr::F_RANDOM_WALK;

    let test_name = "ZTAIMGR_F_RANDOM_WALK_REAL_VS_PORT";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static HAS_DEST: AtomicU32 = AtomicU32::new(0);
    static DEST: Mutex<[i32; 3]> = Mutex::new([0; 3]);
    static ADD_TARGET: AtomicU32 = AtomicU32::new(0);
    static RING: AtomicU32 = AtomicU32::new(0);
    static RANGE_X: AtomicU32 = AtomicU32::new(1);
    static RANGE_Y: AtomicU32 = AtomicU32::new(1);
    static TILE_BASE: AtomicU32 = AtomicU32::new(0);
    static SALT: AtomicU32 = AtomicU32::new(0);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    fn index(tile: u32) -> u32 {
        (tile - TILE_BASE.load(Ordering::Relaxed)) / 0x8c
    }
    fn triple(ptr: u32) -> [i32; 3] {
        [unsafe { *(ptr as *const i32) }, unsafe { *((ptr + 4) as *const i32) }, unsafe { *((ptr + 8) as *const i32) }]
    }
    extern "thiscall" fn v_walk(_: u32, tile: u32, flag: u32) -> u32 {
        let idx = index(tile);
        log(format!("walk({idx},{flag})"));
        u32::from((idx + SALT.load(Ordering::Relaxed)) % 4 != 0)
    }
    extern "thiscall" fn v_get_dest(_: u32, out: u32) -> u32 {
        log("get_dest".into());
        if ADD_TARGET.load(Ordering::Relaxed) != 0 {
            let ring = RING.load(Ordering::Relaxed);
            unsafe {
                *(ring as *mut u32) = ring + 8;
                *((ring + 8) as *mut u32) = ring;
            }
        }
        if HAS_DEST.load(Ordering::Relaxed) != 0 {
            let dest = *DEST.lock().unwrap();
            for (i, v) in dest.iter().enumerate() {
                unsafe { *((out + i as u32 * 4) as *mut i32) = *v };
            }
        }
        HAS_DEST.load(Ordering::Relaxed)
    }
    extern "thiscall" fn v_set_dest(_: u32, goal: u32) -> u32 {
        log(format!("set_dest({:?})", triple(goal)));
        0
    }
    extern "thiscall" fn m_get_path(_: u32, start: u32, goal: u32, _: u32, flag: u32) -> u32 {
        log(format!("path({:?},{:?},{flag})", triple(start), triple(goal)));
        1
    }
    extern "thiscall" fn m_range_x(_: u32, _: u32) -> u32 {
        log("range_x".into());
        RANGE_X.load(Ordering::Relaxed)
    }
    extern "thiscall" fn m_range_y(_: u32, _: u32) -> u32 {
        log("range_y".into());
        RANGE_Y.load(Ordering::Relaxed)
    }

    let mut unit_vtable = Box::new([0u32; 0x100]);
    unit_vtable[0x168 / 4] = v_walk as *const () as usize as u32;
    unit_vtable[0x1f4 / 4] = v_get_dest as *const () as usize as u32;
    unit_vtable[0x1f8 / 4] = v_set_dest as *const () as usize as u32;
    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x18 / 4] = m_get_path as *const () as usize as u32;
    mgr_vtable[0x20 / 4] = m_range_x as *const () as usize as u32;
    mgr_vtable[0x24 / 4] = m_range_y as *const () as usize as u32;
    let mut mgr = Box::new([0u32; 0x80]);
    mgr[0] = mgr_vtable.as_ptr() as u32;
    let this = mgr.as_mut_ptr() as u32;

    let mut tiles = vec![0u8; 20 * 20 * 0x8c];
    for i in 0..400usize {
        let base = i * 0x8c + 0x34;
        for (k, v) in [(i % 20) as i32, (i / 20) as i32, 0].iter().enumerate() {
            tiles[base + k * 4..base + k * 4 + 4].copy_from_slice(&v.to_le_bytes());
        }
    }
    TILE_BASE.store(tiles.as_ptr() as u32, Ordering::Relaxed);
    let mut world_block = Box::new([0u32; 0x40]);
    world_block[(8 + 0x2c) / 4] = 20;
    world_block[(8 + 0x30) / 4] = 20;
    world_block[(8 + 0x38) / 4] = tiles.as_ptr() as u32;
    let world = world_block.as_ptr() as u32;
    let ring = vec![[0u32; 2]; 2];
    RING.store(ring.as_ptr() as u32, Ordering::Relaxed);
    let ring_ptr = ring.as_ptr() as u32;

    let mut block = Box::new([0u32; 0x200]);
    block[0] = unit_vtable.as_ptr() as u32;
    block[0x154 / 4] = ring_ptr;
    let unit = block.as_mut_ptr() as u32;
    block[0x114 / 4] = 5 * 64 + 10;
    block[0x118 / 4] = 5 * 64;
    block[0x11c / 4] = 3 * 16 + 2;
    let guard = || unsafe { *((unit + 0x20f) as *const u8) };
    let reset_ring = || unsafe {
        *(ring_ptr as *mut u32) = ring_ptr;
        *((ring_ptr + 8) as *mut u32) = 0;
    };

    struct Config {
        label: &'static str,
        add_target: bool,
        dest: Option<[i32; 3]>,
    }
    let configs = [
        Config { label: "plain", add_target: false, dest: None },
        Config { label: "queued", add_target: true, dest: None },
        Config { label: "standing destination", add_target: false, dest: Some([9, 9, 9]) },
        Config { label: "destination is own tile", add_target: false, dest: Some([5, 5, 3]) },
    ];
    let saved = rand_state();
    let _ = (&tiles, &world_block, &ring);
    let mut compared = 0u32;
    let mut advanced = 0u32;
    for config in &configs {
        for (range_x, range_y) in [(1u32, 1u32), (2, 3), (5, 2), (3, 1)] {
            for (extra_x, extra_y) in [(0u32, 0u32), (1, 2)] {
                for seed in 0..12u32 {
                    mgr[0xd4 / 4] = extra_x;
                    mgr[0xd8 / 4] = extra_y;
                    RANGE_X.store(range_x, Ordering::Relaxed);
                    RANGE_Y.store(range_y, Ordering::Relaxed);
                    SALT.store(seed % 4, Ordering::Relaxed);
                    HAS_DEST.store(u32::from(config.dest.is_some()), Ordering::Relaxed);
                    *DEST.lock().unwrap() = config.dest.unwrap_or([0; 3]);
                    ADD_TARGET.store(u32::from(config.add_target), Ordering::Relaxed);
                    let state = seed.wrapping_mul(0x9e37_79b1).wrapping_add(0xdead_beef);
                    let mut outcomes = Vec::new();
                    for port in [false, true] {
                        reset_ring();
                        block[0x20f / 4] = 0;
                        LOG.lock().unwrap().clear();
                        set_rand_state(state);
                        let result = with_globals(world, 0, || {
                            if port {
                                unsafe { F_RANDOM_WALK.hooked()(this as *const u32, unit as *const i32) }
                            } else {
                                live_support::real_f_random_walk(this as *const u32, unit as *const i32)
                            }
                        });
                        outcomes.push((result & 0xff, guard(), rand_state(), LOG.lock().unwrap().clone()));
                    }
                    compared += 1;
                    advanced += u32::from(outcomes[0].2 != state);
                    if outcomes[0] != outcomes[1] {
                        failures.push(format!("{} range ({range_x},{range_y}) extra ({extra_x},{extra_y}) seed {seed}: real {:?}, port {:?}", config.label, outcomes[0], outcomes[1]));
                        if failures.len() > 8 {
                            break;
                        }
                    }
                }
            }
        }
    }
    set_rand_state(saved);
    if compared == 0 || advanced == 0 {
        failures.push(format!("vacuous: compared {compared}, rand advanced in {advanced}"));
    }
    finish_test(test_name, failures, failure_log)
}
