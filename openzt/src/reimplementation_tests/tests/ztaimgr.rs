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

/// `ZTAIMGR_F_WRAPPER_RETURNS`: each `ZTAIMgr::f*F` wrapper returns its handler's `AL` in vanilla (a tail call),
/// and `ZTAIMgr::executeCall` passes that on. Compares the port's wrapper return against the real inner
/// handler's return over every unit kind and a null unit, for the handlers whose real body is safe on the fake
/// entities (the building, hatch, show/hide-predator and dust-ball handlers are excluded: their real bodies
/// fault on a non-unit, or their inner is typed `void`). `fSprayF` forwards to a `return 1` stub.
pub(crate) fn run_ztaimgr_f_wrapper_returns_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztaimgr::{
        F_CAUGHT_PREY_F, F_DIE_F, F_END_TRICK_F, F_FACE_TOWARD_FOOD_F, F_FACE_TOWARD_TARGET_F, F_GUEST_ROUTINE_F, F_GUIDE_ROUTINE_F, F_KEEPER_ROUTINE_F, F_KILL_PREY_F, F_MAINT_ROUTINE_F, F_SPRAY_F,
    };
    let test_name = "ZTAIMGR_F_WRAPPER_RETURNS";
    let fixture = Fixture::new();
    let mut failures = Vec::new();
    let mut targets: Vec<(String, Option<Kind>, *const u32)> = ALL_KINDS.iter().map(|k| (format!("{k:?}"), Some(*k), fixture.entity(*k))).collect();
    targets.push(("null".to_string(), None, std::ptr::null()));

    for (label, kind, unit) in &targets {
        let mut check = |name: &str, wrapper: bool, real: u8| {
            if wrapper as u8 != real {
                failures.push(format!("{name} {label}: wrapper returned {wrapper}, real inner {real:#x}"));
            }
        };
        let call = Call::new("f", &[]);
        let c = call.ptr() as i32;
        let u = *unit as u32;

        check("fEndTrickF", unsafe { F_END_TRICK_F.hooked()(c, *unit as *const i32) }, run_pole(*unit, live_support::real_f_end_trick).0 as u8);
        check("fKeeperRoutineF", unsafe { F_KEEPER_ROUTINE_F.hooked()(c as u32, *unit as *const i32) }, run_pole(*unit, live_support::real_f_keeper_routine).0 as u8);
        if *kind != Some(Kind::Guest) {
            check("fGuestRoutineF", unsafe { F_GUEST_ROUTINE_F.hooked()(c as u32, u) }, run_pole(*unit, live_support::real_f_guest_routine).0 as u8);
        }
        check("fMaintRoutineF", unsafe { F_MAINT_ROUTINE_F.hooked()(c as u32, u) }, run_pole(*unit, live_support::real_f_maint_routine).0 as u8);
        check("fGuideRoutineF", unsafe { F_GUIDE_ROUTINE_F.hooked()(c as u32, u) }, run_pole(*unit, live_support::real_f_guide_routine).0 as u8);
        check(
            "fKillPreyF",
            unsafe { F_KILL_PREY_F.hooked()(c, *unit as *const u8) },
            run_pole(*unit, |u, d| live_support::real_f_kill_prey(u as *const u8, d as *const u8)).0 as u8,
        );
        check(
            "fCaughtPreyF",
            unsafe { F_CAUGHT_PREY_F.hooked()(c, *unit as *const i32) },
            run_pole(*unit, |u, d| live_support::real_f_caught_prey(u as *const i32, d as *const u8)).0 as u8,
        );
        check("fDieF", unsafe { F_DIE_F.hooked()(c, *unit as *const i32) }, run_pole(*unit, |u, d| live_support::real_f_die(u as *const i32, d as *const u8)).0 as u8);
        check(
            "fFaceTowardFoodF",
            unsafe { F_FACE_TOWARD_FOOD_F.hooked()(c, *unit as *const u8) },
            run_pole(*unit, |u, d| live_support::real_f_face_toward_food(u as *const u8, d as *const u8)).0 as u8,
        );
        if matches!(kind, None | Some(Kind::Scenery)) {
            check(
                "fFaceTowardTargetF",
                unsafe { F_FACE_TOWARD_TARGET_F.hooked()(c, *unit as *const std::ffi::c_void) },
                run_pole(*unit, |u, d| live_support::real_f_face_toward_target(u as *const std::ffi::c_void, d as *const u8)).0 as u8,
            );
        }
        check("fSprayF", unsafe { F_SPRAY_F.hooked()(c, *unit) }, 1);
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_EXECUTE_CALL`: the real dispatch chain against the port for the names whose real handler is safe
/// on a null or scenery unit (the cast unit is then null for every ZT handler), plus names that fall through
/// the whole `BFAIMgr` chain. Compares the returned `AL`, the `done` byte, the call's own flag byte and
/// entity-virtual calls.
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
                let returned = if real {
                    live_support::real_execute_call(fixture.this(), call_ptr, unit_ptr, done_ptr)
                } else {
                    unsafe { EXECUTE_CALL.hooked()(fixture.this(), call_ptr, unit_ptr, done_ptr) }
                };
                outcomes.push((returned, *done, call.raw[6] & 0xff, virtual_log()));
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
// Manager lifecycle: `BFAIMgr::update`/`save`/`loadData`, `ZTAIMgr::update`/`create`.
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

/// `ZTAIMGR_BFAIMGR_CONSTRUCT_DESTRUCT` (live zoo: `BFAIMgr::init` reads the loaded configuration): the real and
/// ported constructor each build a zeroed `0x150`-byte block (every word equal, heap pointers compared only as
/// non-null, `+0x134` skipped: vanilla leaves a stale stack byte there); the real and ported destructor then tear
/// one each down (every word equal, same final `BFMgr` vtable, empty path cache). Blocks are freed with `operator
/// delete`; the real constructor's and destructor's own allocations stay with vanilla's allocator throughout.
pub(crate) fn run_ztaimgr_bfaimgr_construct_destruct_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::{
        bfaimgr::{CONSTRUCTOR as BFAIMGR_CONSTRUCTOR, DESTRUCTOR_0 as BFAIMGR_DESTRUCTOR_0, DESTRUCTOR_1 as BFAIMGR_DESTRUCTOR_1},
        standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    };
    let test_name = "ZTAIMGR_BFAIMGR_CONSTRUCT_DESTRUCT";
    let mut failures: Vec<String> = Vec::new();
    let base = get_module_base("zoo.exe") as u32;
    const SIZE: u32 = 0x150;
    let allocate = || {
        let block = unsafe { OPERATOR_NEW.original()(SIZE) } as u32;
        for offset in (0..SIZE).step_by(4) {
            save_to_memory(block + offset, 0u32);
        }
        block
    };
    let (real, port) = (allocate(), allocate());
    if real == 0 || port == 0 {
        failures.push("operator new failed".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    unsafe {
        live_support::real_bfaimgr_construct(real as *const u32);
        BFAIMGR_CONSTRUCTOR.hooked()(port as *const u32);
    }
    let compare = |phase: &str, failures: &mut Vec<String>| {
        for offset in (0..SIZE).step_by(4) {
            if offset == 0x134 {
                continue;
            }
            let (real_word, port_word) = (get_from_memory::<u32>(real + offset), get_from_memory::<u32>(port + offset));
            let both_heap_pointers = real_word >= 0x10000 && port_word >= 0x10000 && !(real_word >= base && real_word < base + 0x300000);
            if real_word != port_word && !both_heap_pointers {
                failures.push(format!("{phase} +{offset:#x}: real {real_word:#x} != port {port_word:#x}"));
            }
        }
    };
    compare("construct", &mut failures);
    if get_from_memory::<u32>(real + 0xd0) == 0 || get_from_memory::<u32>(port + 0xd0) == 0 {
        failures.push("construct left a null GXLLEAnimSet".to_string());
    }
    unsafe {
        live_support::real_bfaimgr_destruct(real as *const u32);
        BFAIMGR_DESTRUCTOR_0.hooked()(port as *const u32);
    }
    compare("destruct", &mut failures);
    for manager in [real, port] {
        if get_from_memory::<u32>(manager) != base + 0x0023_55c0 {
            failures.push(format!("{manager:#x}: vtable word {:#x}, expected BFMgr's", get_from_memory::<u32>(manager)));
        }
        if get_from_memory::<u32>(manager + 0x130) != 0 {
            failures.push(format!("{manager:#x}: path cache size not zero after destruct"));
        }
    }
    // Deleting destructor: bit 0 clear leaves the block, bit 0 set frees it (nothing to read back afterwards).
    let deleting = allocate();
    unsafe {
        BFAIMGR_CONSTRUCTOR.hooked()(deleting as *const u32);
        let returned = BFAIMGR_DESTRUCTOR_1.hooked()(deleting as *const u32, 0);
        if returned as u32 != deleting {
            failures.push(format!("deleting destructor returned {:#x}, expected {deleting:#x}", returned as u32));
        }
        BFAIMGR_CONSTRUCTOR.hooked()(deleting as *const u32);
        BFAIMGR_DESTRUCTOR_1.hooked()(deleting as *const u32, 1);
        OPERATOR_DELETE.original()(real);
        OPERATOR_DELETE.original()(port);
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

/// `ZTAIMGR_F_RANDOM_WALK`: port only, over a fake unit, manager, map and recording stand-ins. Covers each
/// phase of `fRandomWalk`: queued targets, a standing destination, the random-box candidate walk, the neighbour
/// walk with its path-cost limit, the fall-back to the unit's own position, the guard byte and the deferrals.
pub(crate) fn run_ztaimgr_f_random_walk_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::{collections::VecDeque, sync::Mutex};

    use crate::ztaimgr::{f_random_walk, RandomWalkOps};
    let test_name = "ZTAIMGR_F_RANDOM_WALK";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<String>> = Mutex::new(Vec::new());
    static RAND: Mutex<VecDeque<u32>> = Mutex::new(VecDeque::new());
    static PATH_RESULTS: Mutex<VecDeque<bool>> = Mutex::new(VecDeque::new());
    static WALKABLE: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static NEIGHBORS: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static HAS_DEST: AtomicU32 = AtomicU32::new(0);
    static DEST: Mutex<[i32; 3]> = Mutex::new([0; 3]);
    static ADD_TARGET: AtomicU32 = AtomicU32::new(0);
    static RING: AtomicU32 = AtomicU32::new(0);
    static RANGE: AtomicU32 = AtomicU32::new(1);
    static WORLD: AtomicU32 = AtomicU32::new(0);
    static TILE_BASE: AtomicU32 = AtomicU32::new(0);
    static COST_LIMIT: AtomicU32 = AtomicU32::new(10);
    fn log(entry: String) {
        LOG.lock().unwrap().push(entry);
    }
    fn index(tile: u32) -> u32 {
        (tile - TILE_BASE.load(Ordering::Relaxed)) / 0x8c
    }
    fn triple(ptr: u32) -> [i32; 3] {
        [unsafe { *(ptr as *const i32) }, unsafe { *((ptr + 4) as *const i32) }, unsafe { *((ptr + 8) as *const i32) }]
    }
    unsafe fn rand() -> u32 {
        RAND.lock().unwrap().pop_front().expect("rand queue exhausted")
    }
    unsafe fn world() -> u32 {
        WORLD.load(Ordering::Relaxed)
    }
    unsafe fn neighbors(_: u32, tile: u32) -> Vec<u32> {
        log(format!("neighbors({})", index(tile)));
        NEIGHBORS.lock().unwrap().iter().map(|i| TILE_BASE.load(Ordering::Relaxed) + i * 0x8c).collect()
    }
    unsafe fn path_cost(_: u32, from: u32, to: u32) -> i32 {
        log(format!("cost({},{})", index(from), index(to)));
        (index(to) % 100) as i32
    }
    unsafe fn cost_limit() -> i32 {
        COST_LIMIT.load(Ordering::Relaxed) as i32
    }
    unsafe fn no_destination() -> [i32; 3] {
        [-1, -1, -1]
    }
    extern "thiscall" fn v_walk(_: u32, tile: u32, flag: u32) -> u32 {
        log(format!("walk({},{flag})", index(tile)));
        u32::from(WALKABLE.lock().unwrap().contains(&index(tile)))
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
        u32::from(PATH_RESULTS.lock().unwrap().pop_front().unwrap_or(false))
    }
    extern "thiscall" fn m_range(_: u32, _: u32) -> u32 {
        RANGE.load(Ordering::Relaxed)
    }
    let ops = RandomWalkOps { rand, world, neighbors, path_cost, path_cost_limit: cost_limit, no_destination };

    let mut unit_vtable = Box::new([0u32; 0x100]);
    unit_vtable[0x168 / 4] = v_walk as *const () as usize as u32;
    unit_vtable[0x1f4 / 4] = v_get_dest as *const () as usize as u32;
    unit_vtable[0x1f8 / 4] = v_set_dest as *const () as usize as u32;
    let mut mgr_vtable = Box::new([0u32; 0x20]);
    mgr_vtable[0x18 / 4] = m_get_path as *const () as usize as u32;
    mgr_vtable[0x20 / 4] = m_range as *const () as usize as u32;
    mgr_vtable[0x24 / 4] = m_range as *const () as usize as u32;
    let mut mgr = Box::new([0u32; 0x80]);
    mgr[0] = mgr_vtable.as_ptr() as u32;
    let this = mgr.as_ptr() as u32;

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
    WORLD.store(world_block.as_ptr() as u32, Ordering::Relaxed);
    let mut ring = vec![[0u32; 2]; 2];
    RING.store(ring.as_ptr() as u32, Ordering::Relaxed);

    let mut block = Box::new([0u32; 0x200]);
    block[0] = unit_vtable.as_ptr() as u32;
    block[0x154 / 4] = ring.as_ptr() as u32;
    let unit = block.as_ptr() as u32;
    // Unit at tile (5, 5), height cell 3.
    block[0x114 / 4] = 5 * 64 + 10;
    block[0x118 / 4] = 5 * 64;
    block[0x11c / 4] = 3 * 16 + 2;
    let guard = |block: &Box<[u32; 0x200]>| unsafe { *((block.as_ptr() as usize + 0x20f) as *const u8) };
    let start = [5, 5, 3];
    let tile_of = |x: i32, y: i32| (y * 20 + x) as u32;

    struct Case {
        label: &'static str,
        add_target: bool,
        dest: Option<[i32; 3]>,
        paths: Vec<bool>,
        walkable: Vec<u32>,
        neighbors: Vec<u32>,
        rands: Vec<u32>,
        expect_result: Option<bool>,
        expect_log: Vec<String>,
    }
    let case = |label, add_target, dest, paths: &[bool], walkable: Vec<u32>, neighbors: Vec<u32>, rands: &[u32], expect_result, expect_log: Vec<String>| Case {
        label, add_target, dest, paths: paths.to_vec(), walkable, neighbors, rands: rands.to_vec(), expect_result, expect_log,
    };
    let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
    // Radius 1 box (rand 0 -> half extent 1): perimeter order dx -1..1, dy -1..1, skipping the centre.
    let ring_tiles: Vec<u32> = [(-1, -1), (-1, 0), (-1, 1), (0, -1), (0, 1), (1, -1), (1, 0), (1, 1)].iter().map(|(dx, dy)| tile_of(5 + dx, 5 + dy)).collect();
    let walk_log = |tiles: &[u32]| tiles.iter().map(|t| format!("walk({t},1)")).collect::<Vec<_>>();
    let goal_of = |t: u32| [(t % 20) as i32, (t / 20) as i32, 0];
    let mut radius_log = vec!["get_dest".to_string()];
    radius_log.extend(walk_log(&ring_tiles));
    let pick_goal = goal_of(ring_tiles[2]);
    let mut radius_ok = radius_log.clone();
    radius_ok.push(format!("set_dest({pick_goal:?})"));
    radius_ok.push(format!("path({start:?},{pick_goal:?},0)"));
    let mut fallback = radius_log.clone();
    fallback.extend(s(&["neighbors(105)"]));
    let cases = vec![
        case("queued after get_dest", true, None, &[], vec![], vec![], &[], Some(true), s(&["get_dest"])),
        case("standing destination", false, Some([9, 9, 9]), &[true], vec![], vec![], &[], Some(true), vec!["get_dest".to_string(), format!("path({start:?},[9, 9, 9],1)")]),
        case("destination is own tile", false, Some(start), &[true], vec![tile_of(4, 6)], vec![], &[0, 0, 0], Some(true), {
            let mut v = vec!["get_dest".to_string()];
            v.extend(walk_log(&ring_tiles));
            v.push(format!("set_dest({:?})", goal_of(tile_of(4, 6))));
            v.push(format!("path({start:?},{:?},0)", goal_of(tile_of(4, 6))));
            v
        }),
        case("radius walk picks a candidate", false, None, &[true], vec![ring_tiles[2], ring_tiles[5]], vec![], &[0, 0, 0], Some(true), radius_ok.clone()),
        case("radius path fails, neighbours", false, None, &[false, true], vec![ring_tiles[2], 102], vec![101, 102], &[0, 0, 0, 1], Some(true), {
            let mut v = radius_ok.clone();
            v.push("neighbors(105)".into());
            v.push("walk(102,1)".into());
            v.push("cost(105,102)".into());
            v.push(format!("set_dest({:?})", goal_of(102)));
            v.push(format!("path({start:?},{:?},0)", goal_of(102)));
            v
        }),
        case("neighbour over cost limit", false, None, &[false], vec![150, 160], vec![150, 160], &[0, 0, 0, 0], Some(false), {
            let mut v = radius_log.clone();
            v.push("neighbors(105)".into());
            v.push("walk(150,1)".into());
            v.push("cost(105,150)".into());
            v.push("walk(160,1)".into());
            v.push("cost(105,160)".into());
            v.push(format!("path({start:?},{start:?},0)"));
            v
        }),
        case("neighbour walk starts at the random index and does not wrap", false, None, &[false, false], vec![101, 102], vec![101, 102], &[0, 0, 1], Some(false), {
            let mut v = radius_log.clone();
            v.push("neighbors(105)".into());
            v.push("walk(102,1)".into());
            v.push("cost(105,102)".into());
            v.push(format!("set_dest({:?})", goal_of(102)));
            v.push(format!("path({start:?},{:?},0)", goal_of(102)));
            v.push(format!("path({start:?},{start:?},0)"));
            v
        }),
    ];
    let _ = fallback;
    for case in &cases {
        let ring_ptr = ring.as_ptr() as u32;
        ring[0][0] = ring_ptr;
        ring[1][0] = 0;
        *RAND.lock().unwrap() = case.rands.iter().copied().collect();
        *PATH_RESULTS.lock().unwrap() = case.paths.iter().copied().collect();
        *WALKABLE.lock().unwrap() = case.walkable.clone();
        *NEIGHBORS.lock().unwrap() = case.neighbors.clone();
        HAS_DEST.store(u32::from(case.dest.is_some()), Ordering::Relaxed);
        *DEST.lock().unwrap() = case.dest.unwrap_or([0; 3]);
        ADD_TARGET.store(u32::from(case.add_target), Ordering::Relaxed);
        RANGE.store(1, Ordering::Relaxed);
        LOG.lock().unwrap().clear();
        let result = unsafe { f_random_walk(this, unit, &ops) };
        if result != case.expect_result || *LOG.lock().unwrap() != case.expect_log || guard(&block) != 0 {
            failures.push(format!("{}: result {result:?}, log {:?}, guard {}; expected {:?}, {:?}", case.label, LOG.lock().unwrap(), guard(&block), case.expect_result, case.expect_log));
        }
    }

    // Gates and deferrals.
    ring[0][0] = ring.as_ptr() as u32;
    HAS_DEST.store(0, Ordering::Relaxed);
    ADD_TARGET.store(0, Ordering::Relaxed);
    LOG.lock().unwrap().clear();
    if unsafe { f_random_walk(this, 0, &ops) } != Some(false) {
        failures.push("null unit".into());
    }
    RANGE.store(0, Ordering::Relaxed);
    *RAND.lock().unwrap() = VecDeque::new();
    if unsafe { f_random_walk(this, unit, &ops) }.is_some() {
        failures.push("zero random extent should defer".into());
    }
    RANGE.store(1, Ordering::Relaxed);
    block[0x114 / 4] = 100 * 64;
    if unsafe { f_random_walk(this, unit, &ops) }.is_some() {
        failures.push("unit off the map should defer".into());
    }
    block[0x114 / 4] = 5 * 64 + 10;
    *RAND.lock().unwrap() = VecDeque::from([0, 0, 0]);
    *WALKABLE.lock().unwrap() = vec![];
    *NEIGHBORS.lock().unwrap() = vec![];
    if unsafe { f_random_walk(this, unit, &ops) }.is_some() {
        failures.push("empty neighbour list should defer".into());
    }
    let _ = (&mut ring, &tiles, &world_block);
    finish_test(test_name, failures, failure_log)
}

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
const RVA_RAND_STATE: u32 = 0x0023_8060;

fn rand_state() -> u32 {
    unsafe { *(tag(RVA_RAND_STATE) as *const u32) }
}

fn set_rand_state(value: u32) {
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

/// `ZTAIMGR_BFAIMGR_INIT_AI_PARAMS` (live zoo, so the configuration is loaded): the real and ported
/// `BFAIMgr::initAIParams` over the same fake manager blocks must leave every dword identical, write the two
/// globals the same way and return the same low byte.
pub(crate) fn run_ztaimgr_bfaimgr_init_ai_params_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::bfaimgr::INIT_AIPARAMS;
    let test_name = "ZTAIMGR_BFAIMGR_INIT_AI_PARAMS";
    let mut failures: Vec<String> = Vec::new();
    let freq_cell = tag(0x0023_9028) as *mut u32;
    let count_cell = tag(0x0023_9532) as *mut u8;
    let (saved_freq, saved_count) = unsafe { (*freq_cell, *count_cell) };

    let mut blocks = [Box::new([0u32; 0x60]), Box::new([0u32; 0x60])];
    for block in blocks.iter_mut() {
        for (index, word) in block.iter_mut().enumerate() {
            *word = 0x7000_0000 + index as u32;
        }
    }
    let seed_globals = || unsafe {
        *freq_cell = 0x5555_aaaa;
        *count_cell = 0x77;
    };
    seed_globals();
    let real_ret = live_support::real_bfaimgr_init_ai_params(blocks[0].as_ptr());
    let real_globals = unsafe { (*freq_cell, *count_cell) };
    seed_globals();
    let port_ret = unsafe { INIT_AIPARAMS.hooked()(blocks[1].as_ptr()) };
    let port_globals = unsafe { (*freq_cell, *count_cell) };
    unsafe {
        *freq_cell = saved_freq;
        *count_cell = saved_count;
    }
    if real_ret & 0xff != 1 || port_ret & 0xff != 1 {
        failures.push(format!("return low byte real {:#x} port {:#x}, expected 1", real_ret & 0xff, port_ret & 0xff));
    }
    if real_globals != port_globals {
        failures.push(format!("globals real {real_globals:x?} port {port_globals:x?}"));
    }
    if real_globals == (0x5555_aaaa, 0x77) {
        failures.push("the real initAIParams did not write the globals".to_string());
    }
    for index in 0..0x60 {
        if blocks[0][index] != blocks[1][index] {
            failures.push(format!("+{:#x}: real {:#x} != port {:#x}", index * 4, blocks[0][index], blocks[1][index]));
        }
    }
    for offset in [0xd4usize, 0xd8, 0xdc, 0xe0, 0xe4, 0xf0, 0xf4, 0xf8, 0xfc, 0x100, 0x104, 0x108, 0x10c, 0x110, 0x114, 0x118, 0x11c, 0x120, 0x124, 0x128] {
        if blocks[0][offset / 4] == 0x7000_0000 + (offset / 4) as u32 {
            failures.push(format!("+{offset:#x}: not written by the real initAIParams"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTAIMGR_BFAIMGR_INIT`: the real and ported `BFAIMgr::init` over fake manager blocks whose vtable `+0x38` is a
/// recorder. Compares the tick remainder and counter (zeroed), which target ran (it must be reached through the
/// manager's vtable), the result's low byte and the `rand` state, which both seed from the clock: each must land
/// inside the window of host clock readings around the call.
pub(crate) fn run_ztaimgr_bfaimgr_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use std::sync::Mutex;
    use std::time::{SystemTime, UNIX_EPOCH};

    use openzt_detour::generated::bfaimgr::INIT;
    let test_name = "ZTAIMGR_BFAIMGR_INIT";
    let mut failures: Vec<String> = Vec::new();

    static LOG: Mutex<Vec<u32>> = Mutex::new(Vec::new());
    static RESULT: AtomicU32 = AtomicU32::new(0);
    extern "thiscall" fn fake_init_ai_params(this: u32) -> u32 {
        LOG.lock().unwrap().push(this);
        RESULT.load(Ordering::Relaxed)
    }
    let now = || SystemTime::now().duration_since(UNIX_EPOCH).unwrap().as_secs() as u32;
    let mut vtable = Box::new([0u32; 0x40]);
    vtable[0x38 / 4] = fake_init_ai_params as *const () as usize as u32;
    let saved = rand_state();

    for result in [0x1234_5601u32, 0xabcd_ef00, 0x1, 0x0] {
        let mut outcomes = Vec::new();
        for port in [false, true] {
            let mut block = Box::new([0x5a5a_5a5au32; 0x60]);
            block[0] = vtable.as_ptr() as u32;
            let ptr = block.as_ptr();
            RESULT.store(result, Ordering::Relaxed);
            LOG.lock().unwrap().clear();
            set_rand_state(0xdead_beef);
            let before = now();
            let ret = if port { unsafe { INIT.hooked()(ptr) } } else { live_support::real_bfaimgr_init(ptr) };
            let after = now();
            let seed = rand_state();
            if seed < before || seed > after {
                failures.push(format!("{} seed {seed} outside [{before}, {after}] (result {result:#x})", if port { "port" } else { "real" }));
            }
            outcomes.push((ret, block[0xe8 / 4], block[0xec / 4], LOG.lock().unwrap().clone() == vec![ptr as u32], block[0x20 / 4]));
        }
        let expected = (result & 0xff != 0, 0, 0, true, 0x5a5a_5a5a);
        if outcomes[0] != expected || outcomes[1] != expected {
            failures.push(format!("result {result:#x}: real {:?}, port {:?}, expected {expected:?}", outcomes[0], outcomes[1]));
        }
    }
    set_rand_state(saved);
    finish_test(test_name, failures, failure_log)
}
