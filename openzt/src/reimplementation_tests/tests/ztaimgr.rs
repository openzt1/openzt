//! Compares real vanilla `ZTAIMgr` leaf methods against their Rust reimplementation (production file
//! `openzt/src/ztaimgr.rs`).
//!
//! No live zoo is needed. Each test builds Rust-owned fixtures (a fake `ZTAIMgr` dword block, fake
//! entities whose `BFEntityType` carries a fake vtable, fake `BFFunctionCall` strings) and calls both
//! poles over the same memory: the hooked address (the Rust port) and the real vanilla body, which reads
//! the same fixtures and dispatches through the same fake vtables.

use std::sync::atomic::{AtomicU32, Ordering};

use openzt_detour::generated::ztaimgr::{
    CONVERT_CONSTANT_PARAM, EXECUTE_CALL, F_CAUGHT_PREY, F_DIE, F_EXIT_BUILDING, F_FACE_TOWARD_FOOD, F_FACE_TOWARD_TARGET, F_KILL_PREY, F_END_TRICK, F_ENTER_BUILDING, F_GUEST_ROUTINE, F_GUIDE_ROUTINE, F_KEEPER_ROUTINE, F_MAINT_ROUTINE, F_RETURN_TO_BUILDING, GET_RANDOM_X, GET_RANDOM_Y,
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
    /// Six words of name and parameter vector, then the result flag byte at `+0x18`.
    raw: Box<[u32; 7]>,
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
        let raw = Box::new([name.raw[0], name.raw[1], name.raw[2], begin, end, end, 0]);
        Self { raw, _name: name, _params: params, _param_words: param_words }
    }

    fn ptr(&self) -> *const u32 {
        self.raw.as_ptr()
    }

    /// The result flag byte at `+0x18`.
    fn flag(&self) -> u8 {
        unsafe { *((self.raw.as_ptr() as u32 + 0x18) as *const u8) }
    }

    fn raw_mut_flag(&self, value: u8) {
        unsafe { *((self.raw.as_ptr() as u32 + 0x18) as *mut u8) = value };
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
        let rust = run_pole(*unit, |u, d| unsafe { F_FACE_TOWARD_FOOD.hooked()(u as *const u8, d as *const u8) });
        let real = run_pole(*unit, |u, d| live_support::real_f_face_toward_food(u as *const u8, d as *const u8));
        if (rust.0 & 0xff, rust.1, rust.2) != (real.0 & 0xff, real.1, real.2) {
            failures.push(format!("fFaceTowardFood {label}: rust {rust:?} != real {real:?}"));
        }
        // Only a non-unit, non-staff/guest target avoids the real `faceTowardTarget` helpers.
        if matches!(kind, None | Some(Kind::Scenery)) {
            let rust = run_pole(*unit, |u, d| unsafe { F_FACE_TOWARD_TARGET.hooked()(u as *const std::ffi::c_void, d as *const u8) });
            let real = run_pole(*unit, |u, d| live_support::real_f_face_toward_target(u as *const std::ffi::c_void, d as *const u8));
            if (rust.0 & 0xff, rust.1, rust.2) != (real.0 & 0xff, real.1, real.2) {
                failures.push(format!("fFaceTowardTarget {label}: rust {rust:?} != real {real:?}"));
            }
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

/// `ZTAIMGR_EXECUTE_CALL`: the real dispatch chain against the port for the names whose real handler is safe
/// on a null or scenery unit (the cast unit is then null for every ZT handler), plus names that fall through
/// the whole `BFAIMgr` chain. Compares the `done` byte, the call's own flag byte and entity-virtual calls.
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
                if real {
                    live_support::real_execute_call(fixture.this(), call_ptr, unit_ptr, done_ptr);
                } else {
                    unsafe { EXECUTE_CALL.hooked()(fixture.this(), call_ptr, unit_ptr, done_ptr) };
                }
                outcomes.push((*done, call.raw[6] & 0xff, virtual_log()));
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

// ---------------------------------------------------------------------------------------------------------------
// Stage 7: `BFAIMgr::update`/`save`/`loadData`, `ZTAIMgr::update`/`create`.
//
// The manager methods read `GLOBAL_ZTWorldMgr`/`GLOBAL_ZTShowMgr` straight from their pointer cells, so the
// tests point those cells at Rust-owned fake managers for the duration of one call (restored afterwards) and
// run the real vanilla body and the port over the same fixtures. Every virtual the bodies call is a fake that
// appends `[slot, id-of-this, args..]` to a shared log, so the comparison sees which virtual ran, in what order,
// on which object, with which arguments.
// ---------------------------------------------------------------------------------------------------------------

use std::sync::Mutex;

use openzt_detour::generated::{
    bfaimgr::{
        CALL_DONE as BFAIMGR_CALL_DONE, CHECK_PATH as BFAIMGR_CHECK_PATH, GET_PATH_1 as BFAIMGR_GET_PATH_1, LOAD_DATA, REMOVE_ENTITIES as BFAIMGR_REMOVE_ENTITIES,
        SAVE as BFAIMGR_SAVE, UPDATE as BFAIMGR_UPDATE,
    },
    bfeventmgr::CONSTRUCTOR as BFEVENTMGR_CONSTRUCTOR,
    poolalloc::{ALLOCATE as POOL_ALLOCATE, DEALLOCATE as POOL_DEALLOCATE},
    ztaimgr::{CREATE, UPDATE as ZTAIMGR_UPDATE},
};

use crate::{
    bfevent::live_support as bfevent_live_support,
    reimplementation_tests::io_redirect,
    util::{get_from_memory, save_to_memory},
    vanilla_rbtree::{rb_set_insert, rb_tree_clear, walk_nodes},
};

const RVA_GLOBAL_WORLD_MGR: u32 = 0x0023_8040;
const RVA_GLOBAL_SHOW_MGR: u32 = 0x0023_9008;
const RVA_PATH_CACHE_NEXT_FLUSH: u32 = 0x0023_94b4;
const RVA_PATH_CACHE_FLUSH_INTERVAL: u32 = 0x0023_9028;
const PATH_CACHE_NODE_SIZE: u32 = 0x18;

const ID_WORLD: u32 = 0x100;
const ID_HABITAT: u32 = 0x200;
const ID_ENTITY: u32 = 0x300;
const ID_MGR: u32 = 0x400;
const ID_SHOW: u32 = 0x500;

static CALL_LOG: Mutex<Vec<[u32; 5]>> = Mutex::new(Vec::new());
/// Address of the fake world whose entity vector `fake_mgr_remove_entities` shrinks by one, or `0`.
static SHRINK_WORLD: AtomicU32 = AtomicU32::new(0);

fn log_call(slot: u32, this: u32, a: u32, b: u32, c: u32) {
    let id = get_from_memory::<u32>(this + 4);
    CALL_LOG.lock().unwrap().push([slot, id, a, b, c]);
}

fn take_log() -> Vec<[u32; 5]> {
    std::mem::take(&mut *CALL_LOG.lock().unwrap())
}

extern "thiscall" fn fake_world_pre_load(this: u32, a: u32) {
    log_call(0x04, this, a, 0, 0);
}
extern "thiscall" fn fake_habitat_check(this: u32) -> u32 {
    log_call(0x10, this, 0, 0, 0);
    get_from_memory::<u32>(this + 8)
}
extern "thiscall" fn fake_habitat_finish(this: u32) {
    log_call(0x58, this, 0, 0, 0);
}
extern "thiscall" fn fake_entity_update(this: u32, dt: u32) {
    log_call(0x28, this, dt, 0, 0);
}
extern "thiscall" fn fake_entity_reset(this: u32, a: u32, b: u32) {
    log_call(0x110, this, a, b, 0);
}
extern "thiscall" fn fake_entity_load_refresh(this: u32) {
    log_call(0x10c, this, 0, 0, 0);
}
extern "thiscall" fn fake_entity_fixup(this: u32, a: u32, b: u32, c: u32) {
    log_call(0x198, this, a, b, c);
}
extern "thiscall" fn fake_mgr_remove_entities(this: u32) {
    log_call(0x40, this, 0, 0, 0);
    let world = SHRINK_WORLD.load(Ordering::Relaxed);
    if world != 0 {
        save_to_memory(world + 0x84, get_from_memory::<u32>(world + 0x84) - 4);
    }
}
extern "thiscall" fn fake_entity_is_removed(this: u32) -> u32 {
    log_call(0xac, this, 0, 0, 0);
    get_from_memory::<u32>(this + 8)
}
extern "thiscall" fn fake_entity_is_removed_undo(this: u32) -> u32 {
    log_call(0xb0, this, 0, 0, 0);
    get_from_memory::<u32>(this + 12)
}
extern "thiscall" fn fake_entity_set_is_removed(this: u32, a: u32, b: u32) {
    log_call(0xa8, this, a, b, 0);
}
extern "thiscall" fn fake_world_remove_entity(this: u32, entity: u32, flag: u32) {
    log_call(0x1c, this, get_from_memory::<u32>(entity + 4), flag, 0);
}

/// World whose entity vector `fake_mgr_remove_entity` erases the removed entity from, or `0`.
static ERASE_WORLD: AtomicU32 = AtomicU32::new(0);
static FAKE_PATH_RESULT: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_mgr_remove_entity(this: u32, entity: u32) {
    log_call(0x44, this, get_from_memory::<u32>(entity + 4), 0, 0);
    let world = ERASE_WORLD.load(Ordering::Relaxed);
    if world == 0 {
        return;
    }
    let end = get_from_memory::<u32>(world + 0x84);
    let mut cursor = get_from_memory::<u32>(world + 0x80);
    while cursor < end {
        if get_from_memory::<u32>(cursor) == entity {
            let mut shift = cursor;
            while shift + 4 < end {
                save_to_memory(shift, get_from_memory::<u32>(shift + 4));
                shift += 4;
            }
            save_to_memory(world + 0x84, end - 4);
            return;
        }
        cursor += 4;
    }
}

extern "thiscall" fn fake_mgr_get_path(this: u32, from: u32, to: u32, unit: u32, flag: u32) -> u32 {
    log_call(0x18 | ((flag & 0xff) << 16), this, from, to, unit);
    FAKE_PATH_RESULT.load(Ordering::Relaxed)
}

extern "thiscall" fn fake_show_update(this: u32) {
    log_call(0x04 + 0x200, this, 0, 0, 0);
}

fn fn_word(function: usize) -> u32 {
    function as u32
}

/// `(kind, byte-vector length)` per entity slot; `None` is a null entry.
type EntitySpec = Vec<Option<(Kind, u32)>>;

struct FakeWorld {
    words: Box<[u32; 0x60]>,
    _world_vtable: Box<[u32; 0x10]>,
    _habitat_vtable: Box<[u32; 0x40]>,
    _entity_vtable: Box<[u32; 0x100]>,
    _type_vtable: Box<FakeVtable>,
    _types: Vec<Box<FakeType>>,
    _habitats: Vec<Box<[u32; 4]>>,
    habitat_ptrs: Box<[u32]>,
    entities: Vec<Option<Box<[u32; 0x80]>>>,
    entity_bufs: Vec<Box<[u8; 8]>>,
    entity_ptrs: Box<[u32]>,
    initial_entity_ptrs: Vec<u32>,
    spec: EntitySpec,
}

impl FakeWorld {
    fn new(habitat_results: &[u32], spec: EntitySpec) -> Self {
        let mut type_vtable = Box::new(FakeVtable { slots: [0; 0x20] });
        type_vtable.slots[0x18 / 4] = fn_word(fake_is_class as *const () as usize);
        type_vtable.slots[0x1c / 4] = fn_word(fake_is_cast_class as *const () as usize);
        let type_vtable_ptr = type_vtable.slots.as_ptr() as u32;
        let mut types = Vec::new();
        for kind in ALL_KINDS {
            let mut ty = Box::new(FakeType { words: [0; 0x100] });
            ty.words[0] = type_vtable_ptr;
            ty.words[1] = kind as u32;
            types.push(ty);
        }

        let mut world_vtable = Box::new([0u32; 0x10]);
        world_vtable[1] = fn_word(fake_world_pre_load as *const () as usize);
        world_vtable[0x1c / 4] = fn_word(fake_world_remove_entity as *const () as usize);
        let mut habitat_vtable = Box::new([0u32; 0x40]);
        habitat_vtable[0x10 / 4] = fn_word(fake_habitat_check as *const () as usize);
        habitat_vtable[0x58 / 4] = fn_word(fake_habitat_finish as *const () as usize);
        let mut entity_vtable = Box::new([0u32; 0x100]);
        entity_vtable[0x28 / 4] = fn_word(fake_entity_update as *const () as usize);
        entity_vtable[0xac / 4] = fn_word(fake_entity_is_removed as *const () as usize);
        entity_vtable[0xb0 / 4] = fn_word(fake_entity_is_removed_undo as *const () as usize);
        entity_vtable[0xa8 / 4] = fn_word(fake_entity_set_is_removed as *const () as usize);
        entity_vtable[0x110 / 4] = fn_word(fake_entity_reset as *const () as usize);
        entity_vtable[0x10c / 4] = fn_word(fake_entity_load_refresh as *const () as usize);
        entity_vtable[0x198 / 4] = fn_word(fake_entity_fixup as *const () as usize);

        let mut habitats = Vec::new();
        for (index, result) in habitat_results.iter().enumerate() {
            let mut habitat = Box::new([0u32; 4]);
            habitat[0] = habitat_vtable.as_ptr() as u32;
            habitat[1] = ID_HABITAT + index as u32;
            habitat[2] = *result;
            habitats.push(habitat);
        }
        let habitat_ptrs: Box<[u32]> = habitats.iter().map(|h| h.as_ptr() as u32).collect();

        let mut entities = Vec::new();
        let mut entity_bufs = Vec::new();
        for (index, slot) in spec.iter().enumerate() {
            entity_bufs.push(Box::new([0x7fu8; 8]));
            match slot {
                Some((kind, _)) => {
                    let mut entity = Box::new([0u32; 0x80]);
                    entity[0] = entity_vtable.as_ptr() as u32;
                    entity[1] = ID_ENTITY + index as u32;
                    entity[0x128 / 4] = types[*kind as usize].words.as_ptr() as u32;
                    entities.push(Some(entity));
                }
                None => entities.push(None),
            }
        }
        let entity_ptrs: Box<[u32]> = entities.iter().map(|e| e.as_ref().map_or(0, |b| b.as_ptr() as u32)).collect();

        let mut words = Box::new([0u32; 0x60]);
        words[0] = world_vtable.as_ptr() as u32;
        words[1] = ID_WORLD;
        let mut world = Self {
            words,
            _world_vtable: world_vtable,
            _habitat_vtable: habitat_vtable,
            _entity_vtable: entity_vtable,
            _type_vtable: type_vtable,
            _types: types,
            _habitats: habitats,
            habitat_ptrs,
            entities,
            entity_bufs,
            initial_entity_ptrs: entity_ptrs.to_vec(),
            entity_ptrs,
            spec,
        };
        world.reset();
        world
    }

    fn ptr(&self) -> u32 {
        self.words.as_ptr() as u32
    }

    /// Restores every mutable field to the initial state, so the second pole starts where the first did.
    fn reset(&mut self) {
        self.entity_ptrs.copy_from_slice(&self.initial_entity_ptrs);
        self.words[0x80 / 4] = self.entity_ptrs.as_ptr() as u32;
        self.words[0x84 / 4] = self.entity_ptrs.as_ptr() as u32 + 4 * self.entity_ptrs.len() as u32;
        self.words[0x98 / 4] = self.habitat_ptrs.as_ptr() as u32;
        self.words[0x9c / 4] = self.habitat_ptrs.as_ptr() as u32 + 4 * self.habitat_ptrs.len() as u32;
        for (index, slot) in self.spec.iter().enumerate() {
            let Some((_, length)) = slot else { continue };
            let buf = self.entity_bufs[index].as_ptr() as u32;
            *self.entity_bufs[index] = [0x7f; 8];
            let entity = self.entities[index].as_mut().unwrap();
            entity[0xdc / 4] = buf;
            entity[0xe0 / 4] = buf + length;
            entity[0xe8 / 4] = 0;
        }
    }

    /// Sets what entity `index` reports for `isRemoved` and `isRemovedUndo` (raw return values).
    fn set_removed(&mut self, index: usize, is_removed: u32, undo: u32) {
        let entity = self.entities[index].as_mut().unwrap();
        entity[2] = is_removed;
        entity[3] = undo;
    }

    /// Ids of the entities still in the world's vector, in order (`0` for a null entry).
    fn live_ids(&self) -> Vec<u32> {
        let (begin, end) = (self.words[0x80 / 4], self.words[0x84 / 4]);
        (begin..end).step_by(4).map(|slot| get_from_memory::<u32>(slot)).map(|entity| if entity == 0 { 0 } else { get_from_memory::<u32>(entity + 4) }).collect()
    }

    /// `[end - begin, flag byte, first byte]` per entity (`u32::MAX` triple for a null entry).
    fn snapshot(&self) -> Vec<[u32; 3]> {
        self.entities
            .iter()
            .map(|slot| match slot {
                Some(entity) => {
                    let (begin, end) = (entity[0xdc / 4], entity[0xe0 / 4]);
                    [end.wrapping_sub(begin), entity[0xe8 / 4] & 0xff, get_from_memory::<u8>(begin) as u32]
                }
                None => [u32::MAX; 3],
            })
            .collect()
    }
}

/// Points the world/show pointer cells at fakes for the duration of `f` (`show == 0` leaves the show cell).
fn with_globals<R>(world: u32, show: u32, f: impl FnOnce() -> R) -> R {
    let base = get_module_base("zoo.exe") as u32;
    let (world_cell, show_cell) = (base + RVA_GLOBAL_WORLD_MGR, base + RVA_GLOBAL_SHOW_MGR);
    let (old_world, old_show) = (get_from_memory::<u32>(world_cell), get_from_memory::<u32>(show_cell));
    save_to_memory(world_cell, world);
    if show != 0 {
        save_to_memory(show_cell, show);
    }
    let result = f();
    save_to_memory(world_cell, old_world);
    save_to_memory(show_cell, old_show);
    result
}

struct FakeShow {
    words: Box<[u32; 4]>,
    _vtable: Box<[u32; 4]>,
}

impl FakeShow {
    fn new() -> Self {
        let mut vtable = Box::new([0u32; 4]);
        vtable[1] = fn_word(fake_show_update as *const () as usize);
        let mut words = Box::new([0u32; 4]);
        words[0] = vtable.as_ptr() as u32;
        words[1] = ID_SHOW;
        Self { words, _vtable: vtable }
    }

    fn ptr(&self) -> u32 {
        self.words.as_ptr() as u32
    }
}

/// A fake `BFAIMgr`/`ZTAIMgr`: tick fields, the path-cache tree container at `+0x12c` over a vanilla pool
/// header, and an embedded `BFEventMgr` whose two event vectors hold `events_a`/`events_b` real `BFEvent`s.
struct FakeAiMgr {
    words: Box<[u32; 0x60]>,
    _vtable: Box<[u32; 0x40]>,
    cache_head: u32,
    _event_storage: Vec<Box<[u32]>>,
}

impl FakeAiMgr {
    fn new(events_a: u32, events_b: u32) -> Self {
        let mut vtable = Box::new([0u32; 0x40]);
        vtable[0x40 / 4] = fn_word(fake_mgr_remove_entities as *const () as usize);
        vtable[0x44 / 4] = fn_word(fake_mgr_remove_entity as *const () as usize);
        vtable[0x18 / 4] = fn_word(fake_mgr_get_path as *const () as usize);
        let head = unsafe { POOL_ALLOCATE.original()(PATH_CACHE_NODE_SIZE) } as u32;
        save_to_memory(head, 0u32);
        save_to_memory(head + 4, 0u32);
        save_to_memory(head + 8, head);
        save_to_memory(head + 0xc, head);

        let mut words = Box::new([0u32; 0x60]);
        words[0] = vtable.as_ptr() as u32;
        words[1] = ID_MGR;
        words[0x12c / 4] = head;
        let base = words.as_ptr() as u32;
        let mut storage: Vec<Box<[u32]>> = Vec::new();
        for (vector_word, count, seed) in [(0xa0 / 4, events_a, 0x10u32), (0xac / 4, events_b, 0x90u32)] {
            let buffer: Box<[u32]> = vec![0u32; 7 * count.max(1) as usize].into_boxed_slice();
            let begin = buffer.as_ptr() as u32;
            for event in 0..count {
                let at = begin + event * 0x1c;
                bfevent_live_support::real_constructor(at as *const u32);
                for field in 2..7u32 {
                    save_to_memory(at + field * 4, seed + event * 0x101 + field * 0x1111);
                }
            }
            words[vector_word] = begin;
            words[vector_word + 1] = begin + count * 0x1c;
            words[vector_word + 2] = begin + count.max(1) * 0x1c;
            storage.push(buffer);
        }
        words[0xb8 / 4] = base + 0xa0;
        words[0xbc / 4] = base + 0xac;
        Self { words, _vtable: vtable, cache_head: head, _event_storage: storage }
    }

    fn ptr(&self) -> *const u32 {
        self.words.as_ptr()
    }

    /// Replaces the embedded event manager with one built by the real `BFEventMgr` constructor: empty event vectors
    /// (so vanilla growth allocates fresh vanilla storage and never frees this fixture's Rust-owned buffers) and a
    /// valid empty listener set. What vanilla allocates is deliberately leaked.
    fn use_real_event_mgr(&mut self) {
        unsafe { BFEVENTMGR_CONSTRUCTOR.original()((self.base() + 0xa0) as *const u32) };
    }

    /// Bytes of every event in both event vectors, `[vector A bytes, vector B bytes]`.
    fn event_bytes(&self) -> [Vec<u8>; 2] {
        let read = |begin: u32, end: u32| (begin..end).map(get_from_memory::<u8>).collect::<Vec<u8>>();
        [read(self.words[0xa0 / 4], self.words[0xa4 / 4]), read(self.words[0xac / 4], self.words[0xb0 / 4])]
    }

    fn base(&self) -> u32 {
        self.words.as_ptr() as u32
    }

    fn container(&self) -> u32 {
        self.base() + 0x12c
    }

    fn set_ticks(&mut self, divisor: u32, remainder: u32, ticks: u32) {
        self.words[0xe4 / 4] = divisor;
        self.words[0xe8 / 4] = remainder;
        self.words[0xec / 4] = ticks;
        let base = self.base();
        self.words[0xb8 / 4] = base + 0xa0;
        self.words[0xbc / 4] = base + 0xac;
    }

    /// Empties the path cache and refills it with `keys` nodes.
    fn reset_cache(&mut self, keys: u32) {
        rb_tree_clear(self.container(), PATH_CACHE_NODE_SIZE);
        for key in 0..keys {
            rb_set_insert(self.container(), 0x1000 + key * 0x10);
        }
    }

    /// Tick fields, path-cache shape and the embedded event manager's words with in-object pointers made
    /// relative, so two separate fixtures (or two runs on one) compare equal.
    fn snapshot(&self) -> Vec<u32> {
        let base = self.base();
        let head = self.words[0x12c / 4];
        let mut snap = vec![self.words[0xe4 / 4], self.words[0xe8 / 4], self.words[0xec / 4], self.words[0x130 / 4]];
        snap.push((get_from_memory::<u32>(head + 4) != 0) as u32);
        snap.push((get_from_memory::<u32>(head + 8) == head) as u32);
        snap.push((get_from_memory::<u32>(head + 0xc) == head) as u32);
        snap.push(walk_nodes(head).count() as u32);
        for index in 0xa0 / 4..=0xbc / 4 {
            let word = self.words[index];
            snap.push(if word >= base && word < base + 0x180 { word - base } else { word });
        }
        snap.push(get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_PATH_CACHE_NEXT_FLUSH));
        snap
    }
}

impl Drop for FakeAiMgr {
    fn drop(&mut self) {
        rb_tree_clear(self.container(), PATH_CACHE_NODE_SIZE);
        unsafe { POOL_DEALLOCATE.original()(self.cache_head as *const u32, PATH_CACHE_NODE_SIZE) };
    }
}

fn standard_entities() -> EntitySpec {
    vec![Some((Kind::Guest, 3)), None, Some((Kind::Scenery, 0)), Some((Kind::Animal, 3)), Some((Kind::Staff, 1))]
}

struct UpdateCase {
    name: &'static str,
    dt: u32,
    remainder: u32,
    ticks: u32,
    divisor: u32,
    next_flush: u32,
    interval: u32,
    keys: u32,
    shrink: bool,
}

fn update_cases() -> Vec<UpdateCase> {
    let case = |name, dt, remainder, ticks, divisor, next_flush, interval, keys, shrink| UpdateCase { name, dt, remainder, ticks, divisor, next_flush, interval, keys, shrink };
    vec![
        case("no tick, no flush", 16, 0, 0, 1000, u32::MAX, 500, 4, false),
        case("carry into ticks, empty cache flush", 1500, 600, 7, 1000, 0, 77, 0, false),
        case("flush frees a populated cache", 1500, 600, 7, 1000, 0, 77, 5, false),
        case("ticks equal next flush", 1000, 0, 9, 1000, 10, 40, 3, false),
        case("one tick short of flush", 1000, 0, 8, 1000, 10, 40, 3, false),
        case("large flush tree", 2000, 1, 1, 1000, 1, 5, 40, false),
        case("wrapping accumulators", u32::MAX, 5, u32::MAX, 1, 2, 3, 2, false),
        case("zero dt", 0, 999, 3, 1000, 4, 10, 2, false),
        case("removeEntities shrinks the entity list", 100, 0, 0, 1000, u32::MAX, 9, 1, true),
    ]
}

fn set_flush_globals(next_flush: u32, interval: u32) {
    let base = get_module_base("zoo.exe") as u32;
    save_to_memory(base + RVA_PATH_CACHE_NEXT_FLUSH, next_flush);
    save_to_memory(base + RVA_PATH_CACHE_FLUSH_INTERVAL, interval);
}

/// Shared driver for the two `update` tests: every case runs `real` then `port` over identical fixtures and
/// compares the call log, the manager snapshot and every entity's mutated fields.
fn compare_update(real: fn(*const u32, u32), port: fn(*const u32, u32), with_show: bool) -> Vec<String> {
    let base = get_module_base("zoo.exe") as u32;
    let saved_next_flush = get_from_memory::<u32>(base + RVA_PATH_CACHE_NEXT_FLUSH);
    let saved_interval = get_from_memory::<u32>(base + RVA_PATH_CACHE_FLUSH_INTERVAL);
    let mut failures: Vec<String> = Vec::new();
    for case in update_cases() {
        let mut world = FakeWorld::new(&[], standard_entities());
        let mut mgr = FakeAiMgr::new(0, 0);
        let show = FakeShow::new();
        let show_ptr = if with_show { show.ptr() } else { 0 };
        let mut outcomes = Vec::new();
        for pole in [real, port] {
            mgr.set_ticks(case.divisor, case.remainder, case.ticks);
            mgr.reset_cache(case.keys);
            world.reset();
            set_flush_globals(case.next_flush, case.interval);
            SHRINK_WORLD.store(if case.shrink { world.ptr() } else { 0 }, Ordering::Relaxed);
            take_log();
            with_globals(world.ptr(), show_ptr, || pole(mgr.ptr(), case.dt));
            SHRINK_WORLD.store(0, Ordering::Relaxed);
            outcomes.push((take_log(), mgr.snapshot(), world.snapshot()));
        }
        if outcomes[0].0 != outcomes[1].0 {
            failures.push(format!("{}: call log real {:x?} != port {:x?}", case.name, outcomes[0].0, outcomes[1].0));
        }
        if outcomes[0].1 != outcomes[1].1 {
            failures.push(format!("{}: manager state real {:x?} != port {:x?}", case.name, outcomes[0].1, outcomes[1].1));
        }
        if outcomes[0].2 != outcomes[1].2 {
            failures.push(format!("{}: entity state differs", case.name));
        }
        let expected_updates = 4 - usize::from(case.shrink);
        let entity_updates = outcomes[0].0.iter().filter(|entry| entry[0] == 0x28).count();
        if entity_updates != expected_updates {
            failures.push(format!("{}: fixture check: {entity_updates} entity updates, expected {expected_updates}", case.name));
        }
        let removals = outcomes[0].0.iter().filter(|entry| entry[0] == 0x40).count();
        if removals != 1 {
            failures.push(format!("{}: fixture check: removeEntities ran {removals} times", case.name));
        }
    }
    set_flush_globals(saved_next_flush, saved_interval);
    failures
}

/// `ZTAIMGR_BFAIMGR_UPDATE`: tick carry and wraparound, the path-cache flush threshold (`>=`) and refill of the
/// next-flush global, `removeEntities` running before the entity count is taken, null entity entries, and the
/// event manager pump. The port skips a zero tick divisor instead of faulting; asserted separately.
pub(crate) fn run_ztaimgr_bfaimgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_UPDATE";
    let mut failures = compare_update(live_support::real_bfaimgr_update, |this, dt| unsafe { BFAIMGR_UPDATE.hooked()(this, dt) }, false);
    let mut world = FakeWorld::new(&[], standard_entities());
    let mut mgr = FakeAiMgr::new(0, 0);
    mgr.set_ticks(0, 123, 456);
    mgr.reset_cache(2);
    world.reset();
    take_log();
    let before = mgr.snapshot();
    with_globals(world.ptr(), 0, || unsafe { BFAIMGR_UPDATE.hooked()(mgr.ptr(), 50) });
    if mgr.snapshot() != before || !take_log().is_empty() {
        failures.push("zero divisor: the port should leave all state alone and run nothing".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_UPDATE` (live zoo, for the real `ZTMaintTaskPool`): the real `ZTAIMgr::update` pumps the real
/// maintenance pool and the fake show manager, then reaches `BFAIMgr::update` at its fixed address, which is
/// the port; so this checks the `ZTAIMgr` half (order and objects of the pumps) and re-checks the base half.
pub(crate) fn run_ztaimgr_update_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let failures = compare_update(live_support::real_ztaimgr_update, |this, dt| unsafe { ZTAIMGR_UPDATE.hooked()(this, dt) }, true);
    finish_test("ZTAIMGR_UPDATE", failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_SAVE`: serialized bytes and return value from the real and ported `save`, for empty and
/// populated event vectors.
pub(crate) fn run_ztaimgr_bfaimgr_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_SAVE";
    let mut failures: Vec<String> = Vec::new();
    let dummy_file: u32 = 0;
    let file = &dummy_file as *const u32 as *const i8;
    for (events_a, events_b) in [(0u32, 0u32), (2, 1), (1, 3), (0, 2)] {
        let mut mgr = FakeAiMgr::new(events_a, events_b);
        mgr.set_ticks(1000, 0x1111_2222, 0x3333_4444);
        io_redirect::begin_capture();
        let real_ret = live_support::real_bfaimgr_save(mgr.ptr(), file);
        let real_bytes = io_redirect::end_capture();
        io_redirect::begin_capture();
        let port_ret = unsafe { BFAIMGR_SAVE.hooked()(mgr.ptr(), file) };
        let port_bytes = io_redirect::end_capture();
        let label = format!("events {events_a}+{events_b}");
        // Two counts, the events, and the one trailing byte `BFEventMgr::save` writes.
        let expected_len = (8 + 4 + events_a * 0x14 + 4 + events_b * 0x14 + 1) as usize;
        if real_bytes.len() != expected_len {
            failures.push(format!("{label}: fixture check: real save wrote {} bytes, expected {expected_len}", real_bytes.len()));
        }
        if real_bytes != port_bytes {
            failures.push(format!("{label}: bytes differ (real {} bytes, port {} bytes)", real_bytes.len(), port_bytes.len()));
        }
        if real_ret != port_ret || !real_ret {
            failures.push(format!("{label}: return real {real_ret} port {port_ret}, expected both true"));
        }
        if real_bytes.len() >= 8 && (real_bytes[0..4] != 0x1111_2222u32.to_le_bytes() || real_bytes[4..8] != 0x3333_4444u32.to_le_bytes()) {
            failures.push(format!("{label}: remainder/tick words not first in the stream"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_LOAD_DATA`: partial and full loads over fake habitats (success bytes with garbage upper
/// bytes, which must not count) and fake entities (unit and non-unit types, empty and non-empty byte vectors,
/// null entries): return value, call order and arguments, and every entity's mutated fields.
pub(crate) fn run_ztaimgr_bfaimgr_load_data_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_LOAD_DATA";
    let mut failures: Vec<String> = Vec::new();
    let habitat_sets: [(&str, Vec<u32>); 5] = [
        ("none", vec![]),
        ("all ok", vec![1, 1, 1]),
        ("one failing low byte, garbage above", vec![1, 0x5500, 1]),
        ("garbage above a passing low byte", vec![0x1234_5601, 0xffff_ff01]),
        ("only failing", vec![0]),
    ];
    let entity_sets: [(&str, EntitySpec); 4] = [
        ("no entities", vec![]),
        ("standard", standard_entities()),
        ("scenery only", vec![Some((Kind::Scenery, 2)), None]),
        ("units", vec![Some((Kind::Guest, 0)), Some((Kind::Guide, 5)), Some((Kind::Staff, 1))]),
    ];
    for (habitat_name, habitats) in &habitat_sets {
        for (entity_name, entities) in &entity_sets {
            for full in [false, true] {
                let label = format!("habitats {habitat_name}, entities {entity_name}, full {full}");
                let mut world = FakeWorld::new(habitats, entities.clone());
                let mut outcomes = Vec::new();
                for port in [false, true] {
                    world.reset();
                    take_log();
                    let result = with_globals(world.ptr(), 0, || {
                        if port {
                            unsafe { LOAD_DATA.hooked()(std::ptr::null(), full) }
                        } else {
                            live_support::real_bfaimgr_load_data(std::ptr::null(), full)
                        }
                    });
                    outcomes.push((result, take_log(), world.snapshot()));
                }
                if outcomes[0].0 != outcomes[1].0 {
                    failures.push(format!("{label}: return real {} port {}", outcomes[0].0, outcomes[1].0));
                }
                if outcomes[0].1 != outcomes[1].1 {
                    failures.push(format!("{label}: call log real {:x?} != port {:x?}", outcomes[0].1, outcomes[1].1));
                }
                if outcomes[0].2 != outcomes[1].2 {
                    failures.push(format!("{label}: entity state real {:?} != port {:?}", outcomes[0].2, outcomes[1].2));
                }
                let finishes = outcomes[0].1.iter().filter(|entry| entry[0] == 0x58).count();
                if finishes != habitats.len() {
                    failures.push(format!("{label}: fixture check: {finishes} habitat finishes, expected {}", habitats.len()));
                }
                let fixups = outcomes[0].1.iter().filter(|entry| entry[0] == 0x198).count();
                let expected_fixups = if full { entities.iter().flatten().filter(|(kind, _)| *kind != Kind::Scenery).count() } else { 0 };
                if fixups != expected_fixups {
                    failures.push(format!("{label}: fixture check: {fixups} unit fixups, expected {expected_fixups}"));
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_CREATE` (live zoo: `initAIParams` reads the loaded configuration): the real and ported `create`
/// each build a manager; both must install the same vtable and listener table, run `initAIParams` to the same
/// values, and agree on every other word (heap pointers compared only as non-null). Both managers are leaked:
/// freeing them would run the live destructor, which this test has no reason to exercise.
pub(crate) fn run_ztaimgr_create_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_CREATE";
    let mut failures: Vec<String> = Vec::new();
    let base = get_module_base("zoo.exe") as u32;
    let real = live_support::real_ztaimgr_create();
    let port = unsafe { CREATE.hooked()() };
    if real == 0 || port == 0 {
        failures.push(format!("create returned real {real:#x} port {port:#x}"));
        return finish_test(test_name, failures, failure_log);
    }
    for manager in [real, port] {
        if get_from_memory::<u32>(manager) != base + 0x0023_5324 {
            failures.push(format!("{manager:#x}: vtable word {:#x}, expected ZTAIMgr's", get_from_memory::<u32>(manager)));
        }
        if get_from_memory::<u32>(manager + 8) != base + 0x0023_2288 {
            failures.push(format!("{manager:#x}: listener table word {:#x}, expected ZTAIMgr's", get_from_memory::<u32>(manager + 8)));
        }
    }
    for offset in (0..0x150u32).step_by(4) {
        let (real_word, port_word) = (get_from_memory::<u32>(real + offset), get_from_memory::<u32>(port + offset));
        let both_heap_pointers = real_word >= 0x10000 && port_word >= 0x10000;
        if real_word != port_word && !both_heap_pointers {
            failures.push(format!("+{offset:#x}: real {real_word:#x} != port {port_word:#x}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

static FIND_PATH_LOG: Mutex<Vec<[u32; 9]>> = Mutex::new(Vec::new());
static FIND_PATH_MGR: AtomicU32 = AtomicU32::new(0);

/// Fake `BFPathFinder::findPath`: records the finder-relative object, the two position pointers, the unit, the
/// trailing flag, and the pathfinder fields `checkPath` is supposed to have set up before this call.
extern "thiscall" fn fake_find_path(this: u32, from: u32, to: u32, unit: u32, flag: u32) -> u32 {
    let mgr = FIND_PATH_MGR.load(Ordering::Relaxed);
    FIND_PATH_LOG.lock().unwrap().push([
        this - mgr,
        from,
        to,
        unit,
        flag,
        get_from_memory::<u32>(this + 0x3c),
        get_from_memory::<u32>(this + 0x8),
        get_from_memory::<u32>(this + 0xc),
        get_from_memory::<u32>(this + 0x10),
    ]);
    0
}

/// `ZTAIMGR_BFAIMGR_CHECK_PATH` (live zoo, manager leaked like `ZTAIMGR_CREATE`'s): a real manager (so the real `setNodeMax` has real buffers to
/// resize) whose pathfinder vtable is swapped for one with a fake `findPath`, and whose result list is filled
/// with 0, 1 or several pool nodes. Real and port `checkPath` must agree on the result byte, the arguments the
/// search is given, the pathfinder fields set up beforehand, and the restored node limit.
pub(crate) fn run_ztaimgr_bfaimgr_check_path_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_CHECK_PATH";
    let mut failures: Vec<String> = Vec::new();
    let mgr = live_support::real_ztaimgr_create();
    if mgr == 0 {
        return finish_test(test_name, vec!["could not build a real manager".to_string()], failure_log);
    }
    FIND_PATH_MGR.store(mgr, Ordering::Relaxed);
    let mut finder_vtable = Box::new([0u32; 4]);
    finder_vtable[0] = fn_word(fake_find_path as *const () as usize);
    let real_finder_vtable = get_from_memory::<u32>(mgr + 0x10);
    save_to_memory(mgr + 0x10, finder_vtable.as_ptr() as u32);

    let mut world = FakeWorld::new(&[], Vec::new());
    world.words[0x34 / 4] = 37;
    world.words[0x38 / 4] = 23;
    let from = Box::new([0u32; 0x20]);
    let to = Box::new([0u32; 0x20]);
    let list = get_from_memory::<u32>(mgr + 0x80);
    let (saved_first, saved_last) = (get_from_memory::<u32>(list), get_from_memory::<u32>(list + 4));

    for (nodes, limit, unit) in [(0u32, 250u32, 0x77u32), (1, 7, 0), (3, 250, 0x1234_5678), (0, 100, 5)] {
        // Replace the result list with `nodes` pool nodes, linked into the head's circular list.
        let mut node_ptrs = Vec::new();
        for _ in 0..nodes {
            node_ptrs.push(unsafe { POOL_ALLOCATE.original()(0x14) } as u32);
        }
        let (mut previous, mut next_of_previous) = (list, list + 0);
        for &node in &node_ptrs {
            save_to_memory(next_of_previous, node);
            save_to_memory(node + 4, previous);
            previous = node;
            next_of_previous = node;
        }
        save_to_memory(next_of_previous, list);
        save_to_memory(list + 4, previous);
        save_to_memory(mgr + 0x4c, limit);

        let mut outcomes = Vec::new();
        for port in [false, true] {
            FIND_PATH_LOG.lock().unwrap().clear();
            let result = with_globals(world.ptr(), 0, || {
                if port {
                    unsafe { BFAIMGR_CHECK_PATH.hooked()(mgr as *const u32, from.as_ptr(), to.as_ptr(), unit as *const u32) }
                } else {
                    live_support::real_bfaimgr_check_path(mgr as *const u32, from.as_ptr(), to.as_ptr(), unit as *const u32)
                }
            });
            outcomes.push((
                result & 0xff,
                std::mem::take(&mut *FIND_PATH_LOG.lock().unwrap()),
                [get_from_memory::<u32>(mgr + 0x18), get_from_memory::<u32>(mgr + 0x1c), get_from_memory::<u32>(mgr + 0x20), get_from_memory::<u32>(mgr + 0x4c)],
            ));
        }
        let label = format!("{nodes} nodes, limit {limit}");
        if outcomes[0].0 != outcomes[1].0 {
            failures.push(format!("{label}: result real {} port {}", outcomes[0].0, outcomes[1].0));
        }
        if outcomes[0].0 != u32::from(nodes > 0) {
            failures.push(format!("{label}: fixture check: real result {} for {nodes} nodes", outcomes[0].0));
        }
        if outcomes[0].1 != outcomes[1].1 {
            failures.push(format!("{label}: findPath calls real {:x?} != port {:x?}", outcomes[0].1, outcomes[1].1));
        }
        if outcomes[0].1.len() != 1 || outcomes[0].1[0][5] != 100 {
            failures.push(format!("{label}: fixture check: findPath calls {:x?}", outcomes[0].1));
        }
        if outcomes[0].2 != outcomes[1].2 {
            failures.push(format!("{label}: pathfinder state real {:x?} != port {:x?}", outcomes[0].2, outcomes[1].2));
        }
        if outcomes[0].2[3] != limit {
            failures.push(format!("{label}: fixture check: node limit {} after the call, expected {limit}", outcomes[0].2[3]));
        }
        for node in node_ptrs {
            unsafe { POOL_DEALLOCATE.original()(node as *const u32, 0x14) };
        }
    }

    save_to_memory(list, saved_first);
    save_to_memory(list + 4, saved_last);
    save_to_memory(mgr + 0x10, real_finder_vtable);
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_REMOVE_ENTITIES`: sweeps over a fake world whose entities report removed / undo-removed
/// through raw return values with garbage upper bytes. The fake manager's `removeEntity` erases the entity from
/// the list, so the index and count stepping is exercised, including consecutive removals and removal of
/// everything.
pub(crate) fn run_ztaimgr_bfaimgr_remove_entities_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_REMOVE_ENTITIES";
    let mut failures: Vec<String> = Vec::new();
    let mgr = FakeAiMgr::new(0, 0);
    // (name, per-entity (isRemoved, isRemovedUndo) raw returns, entity spec)
    let five = standard_entities();
    let cases: Vec<(&str, Vec<(u32, u32)>, EntitySpec)> = vec![
        ("nothing removed", vec![(0, 0); 5], five.clone()),
        ("first removed", vec![(1, 0), (0, 0), (0, 0), (0, 0), (0, 0)], five.clone()),
        ("two consecutive removed", vec![(0, 0), (0, 0), (1, 0), (1, 0), (0, 0)], five.clone()),
        ("last removed", vec![(0, 0), (0, 0), (0, 0), (0, 0), (1, 0)], five.clone()),
        ("everything removed", vec![(1, 0), (0, 0), (1, 0), (1, 0), (1, 0)], five.clone()),
        ("undo-removed is not destroyed", vec![(0, 0), (0, 0), (1, 1), (0, 0), (1, 0)], five.clone()),
        ("garbage above a zero low byte", vec![(0x1234_5600, 0), (0, 0), (0xffff_ff00, 0), (0, 0), (0x5500, 0)], five.clone()),
        ("garbage above a one low byte", vec![(0x1234_5601, 0xabcd_0000), (0, 0), (0xffff_ff01, 0xabcd_0001), (0, 0), (0, 0)], five.clone()),
        ("empty world", Vec::new(), Vec::new()),
    ];
    for (name, flags, spec) in cases {
        let mut world = FakeWorld::new(&[], spec.clone());
        let mut flag_iter = flags.iter();
        let mut expected_removed = 0usize;
        for (index, slot) in spec.iter().enumerate() {
            if slot.is_some() {
                let (removed, undo) = *flag_iter.next().unwrap_or(&(0, 0));
                if removed & 0xff != 0 && undo & 0xff == 0 {
                    expected_removed += 1;
                }
                world.set_removed(index, removed, undo);
            } else {
                let _ = flag_iter.next();
            }
        }
        let mut outcomes = Vec::new();
        for port in [false, true] {
            world.reset();
            take_log();
            ERASE_WORLD.store(world.ptr(), Ordering::Relaxed);
            with_globals(world.ptr(), 0, || {
                if port {
                    unsafe { BFAIMGR_REMOVE_ENTITIES.hooked()(mgr.ptr()) }
                } else {
                    live_support::real_bfaimgr_remove_entities(mgr.ptr())
                }
            });
            ERASE_WORLD.store(0, Ordering::Relaxed);
            outcomes.push((take_log(), world.live_ids()));
        }
        if outcomes[0].0 != outcomes[1].0 {
            failures.push(format!("{name}: call log real {:x?} != port {:x?}", outcomes[0].0, outcomes[1].0));
        }
        if outcomes[0].1 != outcomes[1].1 {
            failures.push(format!("{name}: remaining entities real {:?} != port {:?}", outcomes[0].1, outcomes[1].1));
        }
        if outcomes[0].1.len() != spec.len() - expected_removed {
            failures.push(format!("{name}: fixture check: {} entities left, expected {}", outcomes[0].1.len(), spec.len() - expected_removed));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_GET_PATH_TILES`: the tile overload forwards to vtable slot `+0x18` with both tiles offset to
/// their positions, the unit and the flag, and returns the callee's low byte.
pub(crate) fn run_ztaimgr_bfaimgr_get_path_tiles_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_GET_PATH_TILES";
    let mut failures: Vec<String> = Vec::new();
    let mgr = FakeAiMgr::new(0, 0);
    let from = Box::new([0u32; 0x20]);
    let to = Box::new([0u32; 0x20]);
    for result in [0u32, 1, 0x1234_5601, 0x5500, 0xffff_ff00] {
        for flag in [false, true] {
            for unit in [0u32, 0x77, 0xdead_beef] {
                FAKE_PATH_RESULT.store(result, Ordering::Relaxed);
                take_log();
                let real = live_support::real_bfaimgr_get_path_tiles(mgr.ptr(), from.as_ptr(), to.as_ptr(), unit as *const u32, flag);
                let real_log = take_log();
                let port = unsafe { BFAIMGR_GET_PATH_1.hooked()(mgr.ptr(), from.as_ptr(), to.as_ptr(), unit as *const u32, flag) };
                let port_log = take_log();
                let label = format!("result {result:#x}, flag {flag}, unit {unit:#x}");
                if real != port || real != (result & 0xff != 0) {
                    failures.push(format!("{label}: result real {real} port {port}, expected {}", result & 0xff != 0));
                }
                if real_log != port_log {
                    failures.push(format!("{label}: call log real {real_log:x?} != port {port_log:x?}"));
                }
                let expected = [0x18 | (u32::from(flag) << 16), ID_MGR, from.as_ptr() as u32 + 0x34, to.as_ptr() as u32 + 0x34, unit];
                if port_log != vec![expected] {
                    failures.push(format!("{label}: fixture check: log {port_log:x?}, expected {expected:x?}"));
                }
            }
        }
    }
    FAKE_PATH_RESULT.store(0, Ordering::Relaxed);
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_CALL_DONE`: every combination of the entity flag, the mixer flag and the cursor/end
/// comparison, over a zeroed mixer (so the real `GXMixer::getAnim` finds no animation), plus a null entity.
pub(crate) fn run_ztaimgr_bfaimgr_call_done_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_BFAIMGR_CALL_DONE";
    let mut failures: Vec<String> = Vec::new();
    let mgr = FakeAiMgr::new(0, 0);
    let mut entity = Box::new([0u32; 0x80]);
    for entity_flag in [0u32, 1, 0xff, 0x100] {
        for mixer_flag in [0u32, 1, 0x80] {
            for equal in [false, true] {
                entity[0xf8 / 4] = entity_flag;
                entity[0xd4 / 4] = mixer_flag;
                entity[0x9c / 4] = 0x5000;
                entity[0x8c / 4] = if equal { 0x5000 } else { 0x6000 };
                let ptr = entity.as_ptr();
                let real = live_support::real_bfaimgr_call_done(mgr.ptr(), ptr);
                let port = unsafe { BFAIMGR_CALL_DONE.hooked()(mgr.ptr(), ptr) };
                let expected = entity_flag & 0xff != 0 && mixer_flag & 0xff != 0;
                if real != port || real != expected {
                    failures.push(format!("entity flag {entity_flag:#x}, mixer flag {mixer_flag:#x}, equal {equal}: real {real} port {port}, expected {expected}"));
                }
            }
        }
    }
    let (real, port) = (live_support::real_bfaimgr_call_done(mgr.ptr(), std::ptr::null()), unsafe { BFAIMGR_CALL_DONE.hooked()(mgr.ptr(), std::ptr::null()) });
    if real || port {
        failures.push(format!("null entity: real {real} port {port}"));
    }
    finish_test(test_name, failures, failure_log)
}

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

/// Stage 9 wrappers whose real body is safe to run over fakes: the argument-count guards (which return before
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

/// Stage 9 wrappers' forwarding paths, port only (the real targets need live animation state): the exact
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

/// A fake unit for the dust-ball and target-building tests: its own entity vtable (slot `+0x128` returns a
/// name string), a type whose `isCastClass` answers `ZTAnimal`/`ZTUnitType` from the type's flag word, and the
/// selector, flag bytes and speed dwords the handlers read.
struct StringFixtureUnit {
    block: Box<[u32; 0x180]>,
    _vtable: Box<[u32; 0x100]>,
    _ty: Box<[u32; 0x100]>,
    _type_vtable: Box<[u32; 0x20]>,
}

static FAKE_NAME_PTR: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_unit_name(_this: u32) -> u32 {
    FAKE_NAME_PTR.load(Ordering::Relaxed)
}

/// Type word 1: bit 0 = is a `ZTAnimal`, bit 1 = is a `ZTUnitType`.
extern "thiscall" fn fake_string_type_cast(this: u32, tag_arg: u32) -> bool {
    let flags = unsafe { *((this + 4) as *const u32) };
    (tag_arg == tag(0x0023_8690) && flags & 1 != 0) || (tag_arg == tag(RVA_TAG_ZTUNITTYPE) && flags & 2 != 0)
}

const FAKE_UNIT_SPEED: u32 = 0x0abc_0123;

impl StringFixtureUnit {
    fn new(animal: bool, unit_type: bool) -> Self {
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

    fn ptr(&self) -> u32 {
        self.block.as_ptr() as u32
    }

    fn set_byte(&mut self, offset: usize, value: u8) {
        unsafe { *((self.block.as_ptr() as usize + offset) as *mut u8) = value };
    }

    fn byte(&self, offset: usize) -> u8 {
        unsafe { *((self.block.as_ptr() as usize + offset) as *const u8) }
    }
}

/// Reads a vanilla `{begin, end, cap}` string at `ptr`, also checking the NUL terminator.
unsafe fn read_vanilla_string(ptr: u32) -> String {
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
