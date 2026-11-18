//! String-bearing handlers: `fDustBall` and `fTargetBuilding`.

use super::*;

/// A fake unit for the dust-ball and target-building tests: its own entity vtable (slot `+0x128` returns a
/// name string), a type whose `isCastClass` answers `ZTAnimal`/`ZTUnitType` from the type's flag word, and the
/// selector, flag bytes and speed dwords the handlers read.
pub(super) struct StringFixtureUnit {
    pub(super) block: Box<[u32; 0x180]>,
    pub(super) _vtable: Box<[u32; 0x100]>,
    pub(super) _ty: Box<[u32; 0x100]>,
    pub(super) _type_vtable: Box<[u32; 0x20]>,
}

pub(super) static FAKE_NAME_PTR: AtomicU32 = AtomicU32::new(0);

pub(super) extern "thiscall" fn fake_unit_name(_this: u32) -> u32 {
    FAKE_NAME_PTR.load(Ordering::Relaxed)
}

/// Type word 1: bit 0 = is a `ZTAnimal`, bit 1 = is a `ZTUnitType`.
pub(super) extern "thiscall" fn fake_string_type_cast(this: u32, tag_arg: u32) -> bool {
    let flags = unsafe { *((this + 4) as *const u32) };
    (tag_arg == tag(0x0023_8690) && flags & 1 != 0) || (tag_arg == tag(RVA_TAG_ZTUNITTYPE) && flags & 2 != 0)
}

pub(super) const FAKE_UNIT_SPEED: u32 = 0x0abc_0123;

impl StringFixtureUnit {
    pub(super) fn new(animal: bool, unit_type: bool) -> Self {
        let mut type_vtable = Box::new([0u32; 0x20]);
        type_vtable[0x1c / 4] = fake_string_type_cast as *const () as usize as u32;
        let mut ty = Box::new([0u32; 0x100]);
        ty[0] = type_vtable.as_ptr() as u32;
        ty[1] = u32::from(animal) | (u32::from(unit_type) << 1);
        ty[0x108 / 4] = FAKE_UNIT_SPEED;
        let mut vtable = Box::new([0u32; 0x100]);
        vtable[0x128 / 4] = fake_unit_name as *const () as usize as u32;
        let mut block = Box::new([0u32; 0x180]);
        block[0] = vtable.as_ptr() as u32;
        block[0x128 / 4] = ty.as_ptr() as u32;
        Self { block, _vtable: vtable, _ty: ty, _type_vtable: type_vtable }
    }

    pub(super) fn ptr(&self) -> u32 {
        self.block.as_ptr() as u32
    }

    pub(super) fn set_byte(&mut self, offset: usize, value: u8) {
        unsafe { *((self.block.as_ptr() as usize + offset) as *mut u8) = value };
    }

    pub(super) fn byte(&self, offset: usize) -> u8 {
        unsafe { *((self.block.as_ptr() as usize + offset) as *const u8) }
    }
}

/// Reads a vanilla `{begin, end, cap}` string at `ptr`, also checking the NUL terminator.
pub(super) unsafe fn read_vanilla_string(ptr: u32) -> String {
    let (begin, end) = (unsafe { *(ptr as *const u32) }, unsafe { *((ptr + 4) as *const u32) });
    if begin == 0 {
        return "<null>".to_string();
    }
    let text = String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }).into_owned();
    if unsafe { *(end as *const u8) } != 0 {
        return format!("{text}<unterminated>");
    }
    text
}

/// `ZTAIMGR_F_DUST_BALL`: port only, over recording stand-ins. Checks the effect name by selector and tile, the
/// animation/sound fallback chain (base name, then `dustball`, then `"0"`), the order of the vanilla calls, the
/// strings and arguments handed to `fPlaySpecial`, the follow-up `fCaughtPrey` on a set done flag, and the
/// deferrals.
pub(crate) fn run_ztaimgr_f_dust_ball_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::{ffi::CStr, sync::Mutex};

    use crate::ztaimgr::{f_dust_ball, DustBallOps};
    let test_name = "ZTAIMGR_F_DUST_BALL";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ANIMS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    static SOUNDS: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());
    static TILE: AtomicU32 = AtomicU32::new(0);
    static SET_DONE: AtomicU32 = AtomicU32::new(0);
    static PLAY_RESULT: AtomicU32 = AtomicU32::new(0);
    unsafe fn tile_flags(_: u32) -> u8 {
        LOG.lock().unwrap().push("tile".to_string());
        TILE.load(Ordering::Relaxed) as u8
    }
    unsafe fn find_anim(_: u32, name: *const u8) -> bool {
        let name = unsafe { CStr::from_ptr(name as *const _) }.to_str().unwrap().to_string();
        LOG.lock().unwrap().push(format!("anim?{name}"));
        ANIMS.lock().unwrap().contains(&name.as_str())
    }
    unsafe fn find_sound(_: u32, name: *const u8) -> bool {
        let name = unsafe { CStr::from_ptr(name as *const _) }.to_str().unwrap().to_string();
        LOG.lock().unwrap().push(format!("sound?{name}"));
        SOUNDS.lock().unwrap().contains(&name.as_str())
    }
    unsafe fn play_special(this: u32, anim: u32, sound: u32, extra: u32, unit: u32, done: u32) -> bool {
        let entry = format!("play({:#x},{},{},{},{:#x})", this, unsafe { read_vanilla_string(anim) }, unsafe { read_vanilla_string(sound) }, unsafe { read_vanilla_string(extra) }, unit);
        LOG.lock().unwrap().push(entry);
        unsafe { *(done as *mut u8) = SET_DONE.load(Ordering::Relaxed) as u8 };
        PLAY_RESULT.load(Ordering::Relaxed) != 0
    }
    unsafe fn caught_prey(unit: u32, _: *const bool) {
        LOG.lock().unwrap().push(format!("caught({unit:#x})"));
    }
    unsafe fn must_not_run_tile(_: u32) -> u8 {
        panic!("tile lookup must not run when the port defers")
    }
    let ops = DustBallOps { tile_flags, find_anim, find_sound, play_special, caught_prey };

    let mut unit = StringFixtureUnit::new(false, true);
    let this = 0x1234_5670u32;
    let mut done = Box::new(0u8);
    let done_ptr = done.as_mut() as *mut u8 as *const bool;
    let unit_ptr = unit.ptr();

    for (selector, tile, base) in [(3u32, 0u32, "surfaceball"), (3, 3, "surfaceball"), (4, 0, "submergedball"), (0, 0, "dustball"), (0, 0xfc, "dustball"), (0, 1, "waterball"), (9, 2, "waterball")] {
        unit.block[0x258 / 4] = selector;
        TILE.store(tile, Ordering::Relaxed);
        for anim_set in [vec![], vec![base], vec!["dustball"], vec![base, "dustball"]] {
            for sound_set in [vec![], vec![base], vec!["dustball"]] {
                for (set_done, play_result) in [(0u32, true), (1, false), (0, false), (1, true)] {
                    LOG.lock().unwrap().clear();
                    *ANIMS.lock().unwrap() = anim_set.clone();
                    *SOUNDS.lock().unwrap() = sound_set.clone();
                    SET_DONE.store(set_done, Ordering::Relaxed);
                    PLAY_RESULT.store(u32::from(play_result), Ordering::Relaxed);
                    let result = unsafe { f_dust_ball(this, unit_ptr, done_ptr, &ops) };

                    let mut expected: Vec<String> = Vec::new();
                    if selector != 3 && selector != 4 {
                        expected.push("tile".to_string());
                    }
                    let mut chain = |kind: &str, set: &Vec<&str>| -> String {
                        expected.push(format!("{kind}?{base}"));
                        if set.contains(&base) {
                            return base.to_string();
                        }
                        expected.push(format!("{kind}?dustball"));
                        if set.contains(&"dustball") {
                            "dustball".to_string()
                        } else {
                            "0".to_string()
                        }
                    };
                    let anim = chain("anim", &anim_set);
                    let sound = chain("sound", &sound_set);
                    expected.push(format!("play({this:#x},{anim},{sound},0,{unit_ptr:#x})"));
                    if set_done != 0 {
                        expected.push(format!("caught({unit_ptr:#x})"));
                    }
                    if result != Some(play_result) || *LOG.lock().unwrap() != expected {
                        failures.push(format!(
                            "selector {selector} tile {tile:#x} anims {anim_set:?} sounds {sound_set:?} done {set_done}: result {result:?}, log {:?}, expected {expected:?}",
                            LOG.lock().unwrap()
                        ));
                    }
                }
            }
        }
    }

    // Deferral: a null or non-unit target, with nothing called and the flag untouched.
    *done = 0xaa;
    let deferring = DustBallOps { tile_flags: must_not_run_tile, ..ops };
    let non_unit = StringFixtureUnit::new(false, false);
    for target in [0u32, non_unit.ptr()] {
        if unsafe { f_dust_ball(this, target, done_ptr, &deferring) }.is_some() || *done != 0xaa {
            failures.push(format!("target {target:#x} should defer"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_F_TARGET_BUILDING`: port only, over recording stand-ins. Covers every branch of `fTargetBuilding`
/// (non-animal, target acquisition failing and succeeding, an already-acquired target, a non-`ZTUnitType`
/// animal) and `fTargetBuildingF`'s own gate, including the strings, speed and flags handed to `fMove`.
pub(crate) fn run_ztaimgr_f_target_building_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;

    use crate::ztaimgr::{f_target_building, f_target_building_f, TargetBuildingOps};
    let test_name = "ZTAIMGR_F_TARGET_BUILDING";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static ACQUIRE_RESULT: AtomicU32 = AtomicU32::new(0);
    static MOVE_RESULT: AtomicU32 = AtomicU32::new(0);
    unsafe fn target_building(unit: u32) -> bool {
        LOG.lock().unwrap().push(format!("acquire({unit:#x})"));
        ACQUIRE_RESULT.load(Ordering::Relaxed) != 0
    }
    unsafe fn f_move(this: u32, none: u32, unit: u32, done: u32, name: u32, speed: u32, empty: u32) -> bool {
        let entry = format!(
            "move({this:#x},{},{unit:#x},{done:#x},{},{speed:#x},{})",
            unsafe { read_vanilla_string(none) },
            unsafe { read_vanilla_string(name) },
            unsafe { read_vanilla_string(empty) }
        );
        LOG.lock().unwrap().push(entry);
        MOVE_RESULT.load(Ordering::Relaxed) != 0
    }
    let ops = TargetBuildingOps { target_building, f_move };
    let this = 0x2468_ace0u32;

    let name_text = "Elephant House";
    let mut name_bytes = name_text.as_bytes().to_vec();
    name_bytes.push(0);
    let name_begin = name_bytes.as_ptr() as u32;
    let name_string = Box::new([name_begin, name_begin + name_text.len() as u32, name_begin + name_text.len() as u32 + 1]);
    FAKE_NAME_PTR.store(name_string.as_ptr() as u32, Ordering::Relaxed);

    let mut done = Box::new(0u8);
    let done_ptr = done.as_mut() as *mut u8 as *const bool;

    // Non-animals finish at once with result false, whatever the unit type.
    let non_animal = StringFixtureUnit::new(false, true);
    for target in [0u32, non_animal.ptr()] {
        *done = 0;
        LOG.lock().unwrap().clear();
        let result = unsafe { f_target_building(this, target, done_ptr, &ops) };
        if result || *done != 1 || !LOG.lock().unwrap().is_empty() {
            failures.push(format!("non-animal {target:#x}: result {result}, done {}, log {:?}", *done, LOG.lock().unwrap()));
        }
    }

    // (initially acquired, acquire result, is a unit type)
    for (case, (acquired, acquire_result, unit_type)) in [(0u8, 0u32, true), (0, 1, true), (1, 0, true), (0, 1, false), (1, 1, false), (1, 5, true)].into_iter().enumerate() {
        let move_result = case % 2 == 0;
        MOVE_RESULT.store(u32::from(move_result), Ordering::Relaxed);
        let mut unit = StringFixtureUnit::new(true, unit_type);
        unit.set_byte(0x3b1, acquired);
        ACQUIRE_RESULT.store(acquire_result, Ordering::Relaxed);
        *done = 0;
        LOG.lock().unwrap().clear();
        let result = unsafe { f_target_building(this, unit.ptr(), done_ptr, &ops) };
        let log = LOG.lock().unwrap().clone();
        let acquires = acquired == 0;
        let acquired_now = !acquires || acquire_result != 0;
        let (expected_result, expected_done, expected_flag, moves) = if !acquired_now {
            (true, 1u8, 0u8, false)
        } else if !unit_type {
            (false, 1, 0, false)
        } else {
            (move_result, 0, 1, true)
        };
        let mut expected_log: Vec<String> = Vec::new();
        if acquires {
            expected_log.push(format!("acquire({:#x})", unit.ptr()));
        }
        if moves {
            expected_log.push(format!("move({this:#x},0,{:#x},{:#x},Elephant House,{:#x},)", unit.ptr(), done_ptr as u32, FAKE_UNIT_SPEED));
        }
        if result != expected_result || *done != expected_done || unit.byte(0x3b1) != expected_flag || log != expected_log {
            failures.push(format!(
                "acquired {acquired} result {acquire_result:#x} unit_type {unit_type}: result {result}, done {}, flag {}, log {log:?}; expected {expected_result}, {expected_done}, {expected_flag}, {expected_log:?}",
                *done,
                unit.byte(0x3b1)
            ));
        }
    }

    // fTargetBuildingF's gate.
    let call = Call::new("fTargetBuilding", &[]);
    for target in [0u32, non_animal.ptr()] {
        call.raw_mut_flag(0);
        LOG.lock().unwrap().clear();
        let result = unsafe { f_target_building_f(this, call.ptr() as u32, target, &ops) };
        if result || call.flag() != 1 || !LOG.lock().unwrap().is_empty() {
            failures.push(format!("wrapper non-animal {target:#x}: result {result}, flag {}", call.flag()));
        }
    }
    let mut idle = StringFixtureUnit::new(true, true);
    idle.set_byte(0x3b0, 0);
    idle.set_byte(0x3b1, 1);
    call.raw_mut_flag(0);
    LOG.lock().unwrap().clear();
    let result = unsafe { f_target_building_f(this, call.ptr() as u32, idle.ptr(), &ops) };
    if !result || call.flag() != 1 || idle.byte(0x3b1) != 0 || !LOG.lock().unwrap().is_empty() {
        failures.push(format!("wrapper idle animal: result {result}, flag {}, acquired {}", call.flag(), idle.byte(0x3b1)));
    }
    let mut active = StringFixtureUnit::new(true, true);
    active.set_byte(0x3b0, 1);
    active.set_byte(0x3b1, 1);
    call.raw_mut_flag(0);
    LOG.lock().unwrap().clear();
    MOVE_RESULT.store(1, Ordering::Relaxed);
    let result = unsafe { f_target_building_f(this, call.ptr() as u32, active.ptr(), &ops) };
    let expected_move = format!("move({this:#x},0,{:#x},{:#x},Elephant House,{:#x},)", active.ptr(), call.ptr() as u32 + 0x18, FAKE_UNIT_SPEED);
    if !result || *LOG.lock().unwrap() != vec![expected_move.clone()] {
        failures.push(format!("wrapper active animal: result {result}, log {:?}, expected {expected_move}", LOG.lock().unwrap()));
    }
    finish_test(test_name, failures, failure_log)
}
