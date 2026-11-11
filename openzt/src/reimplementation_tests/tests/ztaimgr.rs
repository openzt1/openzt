//! Compares real vanilla `ZTAIMgr` leaf methods against their Rust reimplementation (production file
//! `openzt/src/ztaimgr.rs`).
//!
//! No live zoo is needed. Each test builds Rust-owned fixtures (a fake `ZTAIMgr` dword block, fake
//! entities whose `BFEntityType` carries a fake vtable, fake `BFFunctionCall` strings) and calls both
//! poles over the same memory: the hooked address (the Rust port) and the real vanilla body, which reads
//! the same fixtures and dispatches through the same fake vtables.

use std::sync::atomic::{AtomicU32, Ordering};

use openzt_detour::generated::ztaimgr::{
    CONVERT_CONSTANT_PARAM, F_CAUGHT_PREY, F_DIE, F_EXIT_BUILDING, F_KILL_PREY, F_END_TRICK, F_ENTER_BUILDING, F_GUEST_ROUTINE, F_GUIDE_ROUTINE, F_KEEPER_ROUTINE, F_MAINT_ROUTINE, F_RETURN_TO_BUILDING, GET_RANDOM_X, GET_RANDOM_Y,
    IS_FAST_MOVE_FUNCTION, IS_MEDIUM_MOVE_FUNCTION, IS_SLOW_MOVE_FUNCTION,
};

use crate::{
    globals::get_module_base,
    reimplementation_tests::harness::{finish_test, write_success_line},
    ztaimgr::live_support,
};

const RVA_TAG_ZTGUEST: u32 = 0x0023_8700;
const RVA_TAG_ZTSTAFF: u32 = 0x0023_8710;
const RVA_TAG_ZTUNITTYPE: u32 = 0x0023_86b0;
const RVA_TAG_ZTGUIDETYPE: u32 = 0x0023_8770;

#[derive(Clone, Copy, PartialEq, Debug)]
enum Kind {
    Guest,
    Staff,
    Guide,
    Animal,
    Scenery,
}

const ALL_KINDS: [Kind; 5] = [Kind::Guest, Kind::Staff, Kind::Guide, Kind::Animal, Kind::Scenery];

/// A fake `BFEntityType`: dword 0 is the vtable pointer, dword 1 the [`Kind`], and the rest is filled with
/// distinct values so a wrong offset shows up as a wrong number.
#[repr(C)]
struct FakeType {
    words: [u32; 0x100],
}

#[repr(C)]
struct FakeVtable {
    slots: [u32; 0x20],
}

fn tag(rva: u32) -> u32 {
    get_module_base("zoo.exe") as u32 + rva
}

extern "thiscall" fn fake_is_class(this: u32, tag_arg: u32) -> bool {
    let kind = unsafe { *((this + 4) as *const u32) };
    (tag_arg == tag(RVA_TAG_ZTGUEST) && kind == Kind::Guest as u32)
        || (tag_arg == tag(RVA_TAG_ZTSTAFF) && kind == Kind::Staff as u32)
}

/// Every fake type except scenery is a `ZTUnitType`; only the guide kind is a `ZTGuideType`.
extern "thiscall" fn fake_is_cast_class(this: u32, tag_arg: u32) -> bool {
    let kind = unsafe { *((this + 4) as *const u32) };
    (tag_arg == tag(RVA_TAG_ZTUNITTYPE) && kind != Kind::Scenery as u32) || (tag_arg == tag(RVA_TAG_ZTGUIDETYPE) && kind == Kind::Guide as u32)
}

/// Entity-vtable calls made by the fake handlers below: each call appends its slot id as a nibble, and the
/// last `this` is kept, so a comparison sees both which virtual ran and on which object.
static VIRTUAL_LOG: AtomicU32 = AtomicU32::new(0);
static VIRTUAL_THIS: AtomicU32 = AtomicU32::new(0);

fn record_virtual(id: u32, this: u32) {
    VIRTUAL_LOG.store(VIRTUAL_LOG.load(Ordering::Relaxed) * 16 + id, Ordering::Relaxed);
    VIRTUAL_THIS.store(this, Ordering::Relaxed);
}

extern "thiscall" fn fake_enter_building(this: u32) {
    record_virtual(0xa, this);
}
extern "thiscall" fn fake_return_to_building(this: u32) {
    record_virtual(0xb, this);
}
extern "thiscall" fn fake_end_trick(this: u32) {
    record_virtual(0xc, this);
}
extern "thiscall" fn fake_exit_building(this: u32) {
    record_virtual(0xd, this);
}

fn reset_virtual_log() {
    VIRTUAL_LOG.store(0, Ordering::Relaxed);
    VIRTUAL_THIS.store(0, Ordering::Relaxed);
}

fn virtual_log() -> (u32, u32) {
    (VIRTUAL_LOG.load(Ordering::Relaxed), VIRTUAL_THIS.load(Ordering::Relaxed))
}

struct Fixture {
    _entity_vtable: Box<[u32; 0x100]>,
    ai_mgr: Box<[u32; 0x60]>,
    _vtable: Box<FakeVtable>,
    _types: Vec<(Kind, Box<FakeType>)>,
    entities: Vec<(Kind, Box<[u32; 0x60]>)>,
}

impl Fixture {
    fn new() -> Self {
        let mut ai_mgr = Box::new([0u32; 0x60]);
        for (i, word) in ai_mgr.iter_mut().enumerate() {
            *word = 0x1000 + i as u32;
        }
        let mut vtable = Box::new(FakeVtable { slots: [0; 0x20] });
        vtable.slots[0x18 / 4] = fake_is_class as *const () as usize as u32;
        vtable.slots[0x1c / 4] = fake_is_cast_class as *const () as usize as u32;
        let vtable_ptr = vtable.slots.as_ptr() as u32;

        let mut entity_vtable = Box::new([0u32; 0x100]);
        entity_vtable[0x2a8 / 4] = fake_enter_building as *const () as usize as u32;
        entity_vtable[0x2b0 / 4] = fake_return_to_building as *const () as usize as u32;
        entity_vtable[0x2bc / 4] = fake_end_trick as *const () as usize as u32;
        entity_vtable[0x2ac / 4] = fake_exit_building as *const () as usize as u32;
        let entity_vtable_ptr = entity_vtable.as_ptr() as u32;

        let mut types = Vec::new();
        let mut entities = Vec::new();
        for kind in ALL_KINDS {
            let mut ty = Box::new(FakeType { words: [0; 0x100] });
            for (i, word) in ty.words.iter_mut().enumerate() {
                *word = 0x5000 + (kind as u32) * 0x100 + i as u32;
            }
            ty.words[0] = vtable_ptr;
            ty.words[1] = kind as u32;
            let mut entity = Box::new([0u32; 0x60]);
            entity[0] = entity_vtable_ptr;
            entity[0x128 / 4] = ty.words.as_ptr() as u32;
            types.push((kind, ty));
            entities.push((kind, entity));
        }
        Self { _entity_vtable: entity_vtable, ai_mgr, _vtable: vtable, _types: types, entities }
    }

    fn this(&self) -> *const u32 {
        self.ai_mgr.as_ptr()
    }

    fn entity(&self, kind: Kind) -> *const u32 {
        self.entities.iter().find(|(k, _)| *k == kind).unwrap().1.as_ptr()
    }
}

/// A `{begin, end, capacity}` string backed by a NUL-terminated buffer (the numeric fallback parses by
/// pointer), keeping the buffer alive alongside it.
struct CallString {
    raw: [u32; 3],
    _bytes: Vec<u8>,
}

impl CallString {
    fn new(text: &str) -> Self {
        let mut bytes = text.as_bytes().to_vec();
        bytes.push(0);
        let begin = bytes.as_ptr() as u32;
        let end = begin + text.len() as u32;
        Self { raw: [begin, end, end + 1], _bytes: bytes }
    }
}

/// A fake `BFFunctionCall`: name string, then a parameter vector of 12-byte `{begin,end,cap}` entries.
struct Call {
    raw: Box<[u32; 6]>,
    _name: CallString,
    _params: Vec<CallString>,
    _param_words: Vec<u32>,
}

impl Call {
    fn new(name: &str, params: &[&str]) -> Self {
        let name = CallString::new(name);
        let params: Vec<CallString> = params.iter().map(|p| CallString::new(p)).collect();
        let param_words: Vec<u32> = params.iter().flat_map(|p| p.raw).collect();
        let begin = param_words.as_ptr() as u32;
        let end = begin + (param_words.len() * 4) as u32;
        let raw = Box::new([name.raw[0], name.raw[1], name.raw[2], begin, end, end]);
        Self { raw, _name: name, _params: params, _param_words: param_words }
    }

    fn ptr(&self) -> *const u32 {
        self.raw.as_ptr()
    }
}

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

type PoleOutcome<R> = (R, bool, (u32, u32));

/// Runs `pole` on `unit` with a fresh `done` flag, returning `(result, done, (virtual log, last this))`.
fn run_pole<R>(unit: *const u32, pole: impl FnOnce(*const u32, *const bool) -> R) -> PoleOutcome<R> {
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
