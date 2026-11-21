//! Live checks for `ztguide.rs`. Each comparison builds guides with the real vanilla constructor (one for the real
//! body, one for the Rust port), runs the real body on one and the port on the other, and compares return values and
//! the guides' whole memory (heap pointers may differ, shared pointers may not).

use std::sync::{
    atomic::{AtomicU32, Ordering},
    Mutex, Once,
};

use openzt_detour::generated::{
    ztguide::{DO_CONDITIONALS, FENCES_OK, F_ROUTINE, F_VAROUTINE, GET_TERRAIN_COST, IS_SPEAKING, TILE_FILTER},
    ztstaff::DO_CONDITIONALS as ZTSTAFF_DO_CONDITIONALS,
};

use super::ztguide_trigger::{cloned_guide_type, fake_guest, install_follow_hook, plant, set_follow_intercept, take_follow_log, unplant, GUIDE_ID};
use super::ztguide_lifecycle::follower_ids;
use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::harness::{finish_test, write_success_line},
    util::{get_from_memory, save_to_memory},
    ztguide::{
        live_support::{self, real},
        ZTGuide, CROWD_CHECK_COUNTDOWN_OFFSET, FIELD_0X298_OFFSET, FOLLOWERS_OFFSET, GUIDE_SIZE, GUIDE_VTABLE_RVA, VIEWING_AREA_INFORMED_OFFSET,
    },
    ztkeeper::unit_offsets,
};

/// The game's `rand` state (`DAT_00638060`), advanced by `BFUnit::idle`.
const RNG_STATE_RVA: u32 = 0x0023_8060;
const UNIT_POS_X: u32 = 0x114;
const UNIT_POS_Y: u32 = 0x118;
const UNIT_POS_Z: u32 = 0x11c;
/// `ZTUnit` byte `doConditionals` tests on guide duties.
const INFORM_ARMED_OFFSET: u32 = 0x20c;
/// `ZTGuideType` `cCrowdCheck`.
const TYPE_CROWD_CHECK_OFFSET: u32 = 0x1fc;
const TILE_SIZE: usize = 0x8c;

pub(super) fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn words(address: u32) -> Vec<u32> {
    (0..GUIDE_SIZE / 4).map(|i| get_from_memory::<u32>(address + i * 4)).collect()
}

/// Whether two words agree: equal, or both heap pointers neither of which is a shared pointer.
fn equivalent(real: u32, port: u32, shared: &[u32]) -> bool {
    real == port || (real >= 0x10000 && port >= 0x10000 && !shared.contains(&real) && !shared.contains(&port))
}

pub(super) fn compare_memory(label: &str, real: ZTGuide, port: ZTGuide, shared: &[u32], failures: &mut Vec<String>) {
    for (i, (r, p)) in words(real.0).into_iter().zip(words(port.0)).enumerate() {
        if !equivalent(r, p, shared) {
            failures.push(format!("{label}: +{:#x} real {r:#x}, port {p:#x}", i * 4));
        }
    }
}

pub(super) fn pin_rng(seed: u32) {
    save_to_memory(base() + RNG_STATE_RVA, seed);
}

pub(super) fn rng() -> u32 {
    get_from_memory(base() + RNG_STATE_RVA)
}

pub(super) fn place_at_tile(guide: ZTGuide, tile: (u32, u32)) {
    save_to_memory(guide.0 + UNIT_POS_X, tile.0 * 64 + 32);
    save_to_memory(guide.0 + UNIT_POS_Y, tile.1 * 64 + 32);
    save_to_memory(guide.0 + UNIT_POS_Z, 0u32);
}

/// `ZTGUIDE_DETOURS_ENABLED`: every `ZTGuide` detour reports enabled, so the comparisons below exercise the hooked
/// path rather than silently running vanilla.
pub(crate) fn run_ztguide_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

/// `ZTGUIDE_FIXTURE_LAYOUT_LIVE`: a guide built by the vanilla constructor over a real guide type has the `ZTGuide`
/// vtable and a self-linked, empty follower list, and the countdown accessors read and write the offset they name.
pub(crate) fn run_ztguide_fixture_layout_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_FIXTURE_LAYOUT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let guide = live_support::build_standalone_guide(guide_type);
    let vtable: u32 = get_from_memory(guide.0);
    if vtable != base() + GUIDE_VTABLE_RVA {
        failures.push(format!("vtable {vtable:#x}, expected {:#x}", base() + GUIDE_VTABLE_RVA));
    }
    let sentinel: u32 = get_from_memory(guide.0 + FOLLOWERS_OFFSET);
    if sentinel == 0 || get_from_memory::<u32>(sentinel) != sentinel || get_from_memory::<u32>(sentinel + 4) != sentinel {
        failures.push(format!("the follower list sentinel {sentinel:#x} is not empty and self-linked"));
    }
    guide.set_crowd_check_countdown(77);
    let raw: i32 = get_from_memory(guide.0 + CROWD_CHECK_COUNTDOWN_OFFSET);
    if raw != 77 || guide.crowd_check_countdown() != 77 {
        failures.push(format!("the countdown accessors wrote {raw}"));
    }
    for offset in [FIELD_0X298_OFFSET, VIEWING_AREA_INFORMED_OFFSET] {
        let value: u32 = get_from_memory(guide.0 + offset);
        if value != 0 {
            failures.push(format!("the constructor left +{offset:#x} at {value}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUIDE_F_ROUTINE_LIVE`: `fRoutine` and `fVARoutine` copy the stopped byte into `*done`; `fRoutine` hands over to
/// `fVARoutine` only on goal `0x77` with the cog-goal flag clear and otherwise runs `BFUnit::idle`. Real body, port and
/// the hooked address leave the same `done` byte, result, generator state and guide memory.
pub(crate) fn run_ztguide_f_routine_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_F_ROUTINE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![guide_type];
    let seed = rng();
    for (goal, cog_flag) in [(0x77u32, 0u8), (0x77, 1), (0x76, 0), (0x75, 0), (0, 0)] {
        for stopped in [0u8, 1, 0x80] {
            let fixtures: Vec<ZTGuide> = (0..3).map(|_| live_support::build_standalone_guide(guide_type)).collect();
            for guide in &fixtures {
                save_to_memory(guide.0 + unit_offsets::STOPPED, stopped);
                save_to_memory(guide.0 + unit_offsets::GOAL_ID, goal);
                save_to_memory(guide.0 + unit_offsets::COG_GOAL_FLAG, cog_flag);
            }
            let label = format!("goal {goal:#x}, flag {cog_flag}, stopped {stopped:#x}");
            let (mut real_done, mut port_done, mut hooked_done) = (0xaau8, 0xaau8, 0xaau8);
            pin_rng(seed);
            let real_result = real::f_routine(fixtures[0].0, &mut real_done as *mut u8 as *const bool);
            let real_rng = rng();
            pin_rng(seed);
            let port_result = ZTGuide::f_routine(fixtures[1].0, &mut port_done as *mut u8 as *const bool);
            let port_rng = rng();
            pin_rng(seed);
            let hooked_result = unsafe { F_ROUTINE.hooked()(fixtures[2].0 as *const u32, &mut hooked_done as *mut u8 as *const bool) };
            pin_rng(seed);
            if !real_result || real_result != port_result || real_result != hooked_result {
                failures.push(format!("{label}: results real {real_result}, port {port_result}, hooked {hooked_result}"));
            }
            if real_done != stopped || real_done != port_done || real_done != hooked_done {
                failures.push(format!("{label}: done bytes real {real_done:#x}, port {port_done:#x}, hooked {hooked_done:#x}"));
            }
            if real_rng != port_rng {
                failures.push(format!("{label}: RNG state real {real_rng:#x}, port {port_rng:#x}"));
            }
            compare_memory(&label, fixtures[0], fixtures[1], &shared, &mut failures);
            compare_memory(&format!("{label} hooked"), fixtures[0], fixtures[2], &shared, &mut failures);
        }
    }
    for stopped in [0u8, 1, 0x80] {
        let fixtures: Vec<ZTGuide> = (0..3).map(|_| live_support::build_standalone_guide(guide_type)).collect();
        for guide in &fixtures {
            save_to_memory(guide.0 + unit_offsets::STOPPED, stopped);
        }
        let (mut real_done, mut port_done, mut hooked_done) = (0xaau8, 0xaau8, 0xaau8);
        pin_rng(seed);
        let real_result = real::f_va_routine(fixtures[0].0, &mut real_done);
        let port_result = ZTGuide::f_va_routine(fixtures[1].0, &mut port_done as *mut u8 as *const bool);
        let hooked_result = unsafe { F_VAROUTINE.hooked()(fixtures[2].0 as *const u32, &mut hooked_done) };
        if !real_result || !port_result || !hooked_result {
            failures.push(format!("fVARoutine stopped {stopped:#x}: result real {real_result}, port {port_result}, hooked {hooked_result}"));
        }
        if real_done != stopped || port_done != stopped || hooked_done != stopped {
            failures.push(format!("fVARoutine stopped {stopped:#x}: done bytes real {real_done:#x}, port {port_done:#x}, hooked {hooked_done:#x}"));
        }
        compare_memory(&format!("fVARoutine stopped {stopped:#x}"), fixtures[0], fixtures[1], &shared, &mut failures);
        compare_memory(&format!("fVARoutine stopped {stopped:#x} hooked"), fixtures[0], fixtures[2], &shared, &mut failures);
    }
    pin_rng(seed);
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUIDE_TERRAIN_COST_LIVE`: `getTerrainCost` over every live tile (up to 200x200) from two guide positions, real
/// against port, plus the hooked address on a sample. Fails if every tile cost the same, which would mean the grid
/// exercised a single branch.
pub(crate) fn run_ztguide_terrain_cost_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_TERRAIN_COST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let guide = live_support::build_standalone_guide(guide_type);
    let mut seen = std::collections::BTreeSet::new();
    let mut mismatches = 0;
    for position in [(0u32, 0u32), (world.map_x_size / 2, world.map_y_size / 2)] {
        place_at_tile(guide, position);
        for y in 0..world.map_y_size.min(200) {
            for x in 0..world.map_x_size.min(200) {
                let tile = world.get_tile_ptr(x, y);
                let real_cost = real::get_terrain_cost(guide.0, tile);
                let port_cost = guide.get_terrain_cost(tile);
                seen.insert(real_cost);
                if real_cost != port_cost {
                    mismatches += 1;
                    if mismatches <= 10 {
                        failures.push(format!("guide at {position:?}, tile ({x},{y}): real {real_cost}, port {port_cost}"));
                    }
                }
                if x == y && x % 17 == 0 {
                    let hooked = unsafe { GET_TERRAIN_COST.hooked()(guide.0 as *const u32, tile as *const u32) };
                    if hooked != real_cost {
                        failures.push(format!("tile ({x},{y}): hooked {hooked}, real {real_cost}"));
                    }
                }
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if seen.len() < 2 {
        failures.push(format!("only the costs {seen:?} occurred, so the grid covered a single branch"));
    }
    finish_test(test_name, failures, failure_log)
}

static FENCE_CAST_RESULT: AtomicU32 = AtomicU32::new(1);

extern "thiscall" fn fake_is_cast_class(_this: u32, _tag: u32) -> u32 {
    FENCE_CAST_RESULT.load(Ordering::SeqCst)
}

/// A fake fence entity: its entity type answers `isCastClass` with [`FENCE_CAST_RESULT`], names itself `type_name`,
/// and the fence's passable byte is `passable`. The buffers are leaked.
fn make_fake_fence(type_name: &'static [u8], passable: u8) -> u32 {
    let mut vtable = vec![0u32; 16];
    vtable[7] = fake_is_cast_class as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    let name = Box::leak(type_name.to_vec().into_boxed_slice());
    let fence_type = Box::leak(vec![0u32; 0x80].into_boxed_slice());
    fence_type[0] = vtable.as_ptr() as u32;
    fence_type[0xa4 / 4] = name.as_ptr() as u32;
    let fence = Box::leak(vec![0u8; 0x200].into_boxed_slice());
    save_to_memory(fence.as_ptr() as u32 + 0x128, fence_type.as_ptr() as u32);
    save_to_memory(fence.as_ptr() as u32 + 0x164, passable);
    fence.as_ptr() as u32
}

fn fences_ok_poles(guide: ZTGuide, tile: u32, direction: u32) -> (u32, u8, u32, u8, u32, u8) {
    let (mut real_out, mut port_out, mut hooked_out) = (0xaau8, 0xaau8, 0xaau8);
    let real_result = real::fences_ok(guide.0, tile, direction, &mut real_out as *mut u8 as *const bool);
    let port_result = guide.fences_ok(tile, direction as i32, &mut port_out as *mut u8 as u32) as u32;
    let hooked_result =
        unsafe { FENCES_OK.hooked()(guide.0 as *const u32, tile as *const u32, direction, &mut hooked_out as *mut u8 as *const bool, std::ptr::null()) };
    (real_result as u32, real_out, port_result, port_out, hooked_result as u32, hooked_out)
}

/// `ZTGUIDE_FENCES_OK_LIVE`: `fencesOK` over live tiles in every direction (`0..8`, `-1`) plus a null tile, then a
/// copy of a live tile whose near edge holds a fake fence in each of: open, closed gate (`"gate"`), closed non-gate, and
/// a non-`ZTFence` entity, real against port and the hooked address.
pub(crate) fn run_ztguide_fences_ok_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_FENCES_OK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let guide = live_support::build_standalone_guide(guide_type);
    let mut mismatches = 0;
    let mut closed = 0;
    let check = |label: String, tile: u32, direction: u32, failures: &mut Vec<String>, mismatches: &mut u32| {
        let (real_result, real_out, port_result, port_out, hooked_result, hooked_out) = fences_ok_poles(guide, tile, direction);
        if real_result != port_result || real_result != hooked_result || real_out != 0 || port_out != 0 || hooked_out != 0 {
            *mismatches += 1;
            if *mismatches <= 10 {
                failures.push(format!(
                    "{label}: results real {real_result}, port {port_result}, hooked {hooked_result}; out bytes {real_out}/{port_out}/{hooked_out}"
                ));
            }
        }
        real_result
    };
    for y in 0..world.map_y_size.min(200) {
        for x in 0..world.map_x_size.min(200) {
            let tile = world.get_tile_ptr(x, y);
            for direction in (0u32..8).chain([0xffff_ffff]) {
                if check(format!("tile ({x},{y}) direction {direction:#x}"), tile, direction, &mut failures, &mut mismatches) == 0 {
                    closed += 1;
                }
            }
        }
    }
    check("null tile".to_string(), 0, 2, &mut failures, &mut mismatches);

    // A live interior tile copied into a buffer keeps its coordinates, so the neighbour lookup finds the real
    // neighbour while the near edge slot is ours.
    let live_tile = world.get_tile_ptr(world.map_x_size / 2, world.map_y_size / 2);
    let fake_tile = Box::leak(vec![0u8; TILE_SIZE].into_boxed_slice());
    unsafe { std::ptr::copy_nonoverlapping(live_tile as *const u8, fake_tile.as_mut_ptr(), TILE_SIZE) };
    let fake_tile_address = fake_tile.as_ptr() as u32;
    for (label, type_name, passable, is_fence, expected) in [
        ("open fence", &b"wall\0"[..], 1u8, 1u32, 1u32),
        ("closed gate", b"gate\0", 0, 1, 1),
        ("closed wall", b"wall\0", 0, 1, 0),
        ("not a fence", b"wall\0", 0, 0, 1),
    ] {
        FENCE_CAST_RESULT.store(is_fence, Ordering::SeqCst);
        let fence = make_fake_fence(Box::leak(type_name.to_vec().into_boxed_slice()), passable);
        for direction in [0u32, 2, 4, 6] {
            let slot = fake_tile_address + 0x14 + (direction / 2) * 4;
            let saved: u32 = get_from_memory(slot);
            save_to_memory(slot, fence);
            let result = check(format!("{label} direction {direction}"), fake_tile_address, direction, &mut failures, &mut mismatches);
            if result != expected {
                failures.push(format!("{label} direction {direction}: real result {result}, expected {expected}"));
            }
            save_to_memory(slot, saved);
        }
    }
    FENCE_CAST_RESULT.store(1, Ordering::SeqCst);
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    tracing::info!("{test_name}: {closed} live tile/direction pairs were blocked");
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUIDE_IS_SPEAKING_LIVE`: `isSpeaking` on goal `0x77`, and with the guide's own `+0x54`/`+0x58` string set to
/// `"bSpeak"`, a one-byte difference, a longer string and an empty one; real against port and the hooked address.
pub(crate) fn run_ztguide_is_speaking_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_IS_SPEAKING_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut seen = std::collections::BTreeSet::new();
    for string in [&b"bSpeak"[..], b"bSpeaX", b"bSpeak!", b"", b"bspeak"] {
        let text: &'static [u8] = Box::leak(string.to_vec().into_boxed_slice());
        let begin = text.as_ptr() as u32;
        for goal in [0x76u32, 0x77] {
            let guide = live_support::build_standalone_guide(guide_type);
            save_to_memory(guide.0 + 0x54, begin);
            save_to_memory(guide.0 + 0x58, begin + text.len() as u32);
            save_to_memory(guide.0 + unit_offsets::GOAL_ID, goal);
            let real_result = real::is_speaking(guide.0) as u32;
            let port_result = guide.is_speaking() as u32;
            let hooked_result = unsafe { IS_SPEAKING.hooked()(guide.0 as *const u32) } as u32;
            seen.insert(real_result);
            if real_result != port_result || real_result != hooked_result {
                failures.push(format!(
                    "string {:?}, goal {goal:#x}: real {real_result}, port {port_result}, hooked {hooked_result}",
                    String::from_utf8_lossy(string)
                ));
            }
        }
    }
    if seen.len() < 2 {
        failures.push("every case gave the same answer, so one branch was covered".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// While set, `ZTStaff::doConditionals` is recorded instead of run: the real one runs the whole staff conditional
/// pass, which a bare fixture cannot take.
static COND_INTERCEPT: AtomicU32 = AtomicU32::new(0);
static COND_LOG: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

#[openzt_detour_macro::detour_mod]
mod conditionals_hooks {
    use super::*;

    #[detour(ZTSTAFF_DO_CONDITIONALS)]
    unsafe extern "thiscall" fn staff_do_conditionals(this: *const u32) {
        if COND_INTERCEPT.load(Ordering::SeqCst) != 0 {
            COND_LOG.lock().unwrap().push("staff");
        } else {
            unsafe { ZTSTAFF_DO_CONDITIONALS_DETOUR.call(this) }
        }
    }
}

fn install_conditionals_hooks() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { conditionals_hooks::init_detours() } {
            result = Err(format!("could not install the conditionals hooks: {e:?}"));
        }
    });
    result
}

/// `ZTGUIDE_DO_CONDITIONALS_LIVE`: `doConditionals` over the countdown (`0`, `1`, `5`, `-1`), goal (`0x76`, `0x77`,
/// other), the `+0x20c` byte and a cloned guide type's `cCrowdCheck` (`0` and `7`); real against port and the hooked
/// address. `ZTStaff::doConditionals` goes to a recording hook, and `triggerNearbyGuests` is observed through its
/// effect: a fake guest planted on the guide's tile (follow chance 100, radius 0) reaches the `followGuide` recording
/// hook exactly when it runs. Compared: both logs, the countdown, the follower list and the whole guide. Fails if
/// fewer than four distinct outcomes occurred.
pub(crate) fn run_ztguide_do_conditionals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_DO_CONDITIONALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(message) = install_conditionals_hooks().and_then(|()| install_follow_hook()) {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let Some(real_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let position = (world.map_x_size / 2, world.map_y_size / 2);
    let tile = world.get_tile_ptr(position.0, position.1);
    let guest = fake_guest(0x4001, true, 0, 1, 0);
    plant(tile, guest);
    let mut seen = std::collections::BTreeSet::new();
    COND_INTERCEPT.store(1, Ordering::SeqCst);
    set_follow_intercept(true);
    for period in [0i32, 7] {
        let guide_type = cloned_guide_type(real_type, b"x", period, 0, 100, 5);
        for countdown in [0i32, 1, 5, -1] {
            for goal in [0x76u32, 0x77, 0x75] {
                for armed in [0u8, 1] {
                    let fixtures: Vec<ZTGuide> = (0..3).map(|_| live_support::build_standalone_guide(guide_type)).collect();
                    for guide in &fixtures {
                        guide.set_crowd_check_countdown(countdown);
                        save_to_memory(guide.0 + unit_offsets::GOAL_ID, goal);
                        save_to_memory(guide.0 + INFORM_ARMED_OFFSET, armed);
                        save_to_memory(guide.0 + unit_offsets::ENTITY_ID, GUIDE_ID);
                        place_at_tile(*guide, position);
                    }
                    let label = format!("period {period}, countdown {countdown}, goal {goal:#x}, armed {armed}");
                    let mut logs = Vec::new();
                    for (pole, guide) in fixtures.iter().enumerate() {
                        COND_LOG.lock().unwrap().clear();
                        take_follow_log();
                        match pole {
                            0 => real::do_conditionals(guide.0),
                            1 => guide.do_conditionals(),
                            _ => unsafe { DO_CONDITIONALS.hooked()(guide.0 as *const u32) },
                        }
                        let staff_log = std::mem::take(&mut *COND_LOG.lock().unwrap());
                        let followers = follower_ids(*guide, &mut failures, &format!("{label} pole {pole}"));
                        logs.push((staff_log, take_follow_log(), followers));
                    }
                    if logs[0] != logs[1] || logs[0] != logs[2] {
                        failures.push(format!("{label}: real {:?}, port {:?}, hooked {:?}", logs[0], logs[1], logs[2]));
                    }
                    seen.insert((logs[0].clone(), fixtures[0].crowd_check_countdown()));
                    compare_memory(&label, fixtures[0], fixtures[1], &[guide_type], &mut failures);
                    compare_memory(&format!("{label} hooked"), fixtures[0], fixtures[2], &[guide_type], &mut failures);
                }
            }
        }
    }
    set_follow_intercept(false);
    COND_INTERCEPT.store(0, Ordering::SeqCst);
    unplant(tile, guest);
    if seen.len() < 4 {
        failures.push(format!("only {} distinct outcomes occurred: {seen:?}", seen.len()));
    }
    finish_test(test_name, failures, failure_log)
}

static TILE_ENTITY_BLOCKS: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_tile_entity_blocks(_this: u32) -> u32 {
    TILE_ENTITY_BLOCKS.load(Ordering::SeqCst)
}

/// A leaked fake tile entity whose vtable `+0xb4` answers [`TILE_ENTITY_BLOCKS`].
fn fake_tile_entity() -> u32 {
    let mut vtable = vec![0u32; 64];
    vtable[0xb4 / 4] = fake_tile_entity_blocks as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    let entity = Box::leak(vec![0u32; 0x40].into_boxed_slice());
    entity[0] = vtable.as_ptr() as u32;
    entity.as_ptr() as u32
}

/// `ZTGUIDE_TILE_FILTER_LIVE`: `tileFilter` over every live tile (both flag values, two guide positions), then
/// copies of an in-zoo tile with the `0x8` flag byte, the guide's own tile flag, and the four entity slots (empty,
/// entities answering true and false) varied; real against port and the hooked address.
pub(crate) fn run_ztguide_tile_filter_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::ztguide::live_support::real as real_guide;
    let test_name = "ZTGUIDE_TILE_FILTER_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let guide = live_support::build_standalone_guide(guide_type);
    let mut mismatches = 0;
    let mut seen = std::collections::BTreeSet::new();
    let mut check = |label: String, tile: u32, flag: bool, failures: &mut Vec<String>| {
        let real_result = real_guide::tile_filter(guide.0, tile, flag);
        let port_result = guide.tile_filter(tile, flag);
        let hooked = unsafe { TILE_FILTER.hooked()(guide.0 as *const u32, tile as *const u32, flag) };
        seen.insert((real_result, flag));
        if real_result != port_result || real_result != hooked {
            mismatches += 1;
            if mismatches <= 10 {
                failures.push(format!("{label}: real {real_result}, port {port_result}, hooked {hooked}"));
            }
        }
        real_result
    };
    let mut in_zoo_tile = 0u32;
    for position in [(0u32, 0u32), (world.map_x_size / 2, world.map_y_size / 2)] {
        place_at_tile(guide, position);
        for y in 0..world.map_y_size.min(200) {
            for x in 0..world.map_x_size.min(200) {
                let tile = world.get_tile_ptr(x, y);
                for flag in [false, true] {
                    check(format!("guide at {position:?}, tile ({x},{y}), flag {flag}"), tile, flag, &mut failures);
                }
                if in_zoo_tile == 0 && unsafe { openzt_detour::generated::bftile::IS_IN_ZOO.original()(tile as *const u32, 0) } & 0xff != 0 {
                    in_zoo_tile = tile;
                }
            }
        }
    }
    for flag in [false, true] {
        check(format!("null tile, flag {flag}"), 0, flag, &mut failures);
    }
    if in_zoo_tile == 0 {
        failures.push("the live world holds no in-zoo tile".to_string());
    } else {
        let fake_tile = Box::leak(vec![0u8; TILE_SIZE].into_boxed_slice());
        unsafe { std::ptr::copy_nonoverlapping(in_zoo_tile as *const u8, fake_tile.as_mut_ptr(), TILE_SIZE) };
        let fake = fake_tile.as_ptr() as u32;
        let (blocking, open) = (fake_tile_entity(), fake_tile_entity());
        let own_tile: u32 = unsafe { openzt_detour::generated::bfentity::GET_TILE.original()(guide.0 as *const u32) } as u32;
        let own_flags: u8 = get_from_memory(own_tile + 0x83);
        for own_bit in [0u8, 8] {
            save_to_memory(own_tile + 0x83, (own_flags & !8) | own_bit);
            for target_bit in [0u8, 8] {
                save_to_memory(fake + 0x83, target_bit);
                for slots in [[0u32, 0, 0, 0], [blocking, blocking, blocking, blocking], [open, blocking, blocking, blocking], [blocking, blocking, blocking, open]] {
                    for (i, entity) in slots.into_iter().enumerate() {
                        save_to_memory(fake + 4 + i as u32 * 4, entity);
                    }
                    for answer in [0u32, 1] {
                        TILE_ENTITY_BLOCKS.store(answer, Ordering::SeqCst);
                        for flag in [false, true] {
                            check(format!("own bit {own_bit}, target bit {target_bit}, slots {slots:x?}, answer {answer}, flag {flag}"), fake, flag, &mut failures);
                        }
                    }
                }
            }
        }
        save_to_memory(own_tile + 0x83, own_flags);
        TILE_ENTITY_BLOCKS.store(0, Ordering::SeqCst);
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if !seen.iter().any(|(result, _)| *result) || !seen.iter().any(|(result, _)| !*result) {
        failures.push(format!("only {seen:?} occurred, so one outcome was never covered"));
    }
    finish_test(test_name, failures, failure_log)
}
