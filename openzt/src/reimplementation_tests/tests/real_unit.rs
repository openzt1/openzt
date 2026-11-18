//! T1 fixtures: real `BFUnit`s over a real animal type from the live zoo, so `findAnim`, the mixer and the
//! animation cache run for real instead of against recording stand-ins. Each case builds two units with the vanilla
//! `BFUnit` constructor from identical starting state, runs real vanilla on one and the port on the other, and
//! compares the return value, the call's done flag, the unit's flags, the unit's and mixer's memory (heap pointers
//! may differ, shared pointers may not), and the animation reference counts. The units are then torn down through
//! the real destructor and the counts must return to where they started. The "real" pole's call object is built by
//! vanilla, the port's by the port, so each side's strings come from its own allocator.
//!
//! Needs the live zoo (a real animal type) and `GLOBAL_ZTAIMgr`.

use openzt_detour::generated::{
    bfaimgr::{F_PLAY_WITH_BASE_F, F_SET_BASE_F},
    bfentity::FIND_ANIM as BFENTITY_FIND_ANIM,
    gxmixer::GET_ANIM as GXMIXER_GET_ANIM,
    bfunit::{CONSTRUCTOR as BFUNIT_CONSTRUCTOR, DESTRUCTOR_1 as BFUNIT_DESTRUCTOR_1},
    standalone::OPERATOR_NEW,
};
use windows::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS};

use crate::bfscriptmgr::live_support as bfscriptmgr_live_support;
use crate::globals::{get_module_base, globals};
use crate::reimplementation_tests::harness::finish_test;
use crate::util::{get_from_memory, save_to_memory};
use crate::zoostatus::globals::raw_globals::ANIMAL_TYPE_CHECK_RVA;
use crate::ztaimgr::live_support as ztaimgr_live_support;

const BFUNIT_SIZE: u32 = 0x214;
const UNIT_TYPE: u32 = 0x128;
const UNIT_MOVING: u32 = 0xf8;
const UNIT_STOPPED: u32 = 0xf9;
const UNIT_MIXER: u32 = 0x74;
const MIXER_SIZE: u32 = 0x64;
const CALL_RESULT_FLAG: u32 = 0x18;
const ANIM_REFCOUNT_OFFSET: u32 = 0x20;
const ANIM_STAMP_OFFSET: u32 = 0x1c;
const GLOBAL_ZTAIMGR: u32 = 0x0063_8098;
const RVA_ANIM_CACHE: u32 = 0x0023_8044;
const ANIM_CACHE_STAMP_OFFSET: u32 = 0x20;
/// Animation names real animals commonly have; the ones the chosen type resolves are used as the "found" cases.
const CANDIDATE_ANIMS: &[&str] = &["stand", "walk", "run", "trot", "eat", "lieidle", "lie_idle", "jerk", "shake", "rub"];

fn readable(address: u32, length: u32) -> bool {
    if address < 0x10000 {
        return false;
    }
    let mut info = MEMORY_BASIC_INFORMATION::default();
    let got = unsafe { VirtualQuery(Some(address as *const _), &mut info, std::mem::size_of::<MEMORY_BASIC_INFORMATION>()) };
    got != 0 && info.State.0 == 0x1000 && (info.Protect & (PAGE_NOACCESS | PAGE_GUARD)).0 == 0 && (address as usize + length as usize) <= info.BaseAddress as usize + info.RegionSize
}

/// The type of the first animal in the live world.
fn live_animal_type() -> Option<u32> {
    let world = globals().ztworldmgr();
    let animal_arg = get_module_base("zoo.exe") as u32 + ANIMAL_TYPE_CHECK_RVA;
    let mut slot = world.entity_array_start();
    while slot != world.entity_array_end() {
        let entity: u32 = get_from_memory(slot);
        slot += 4;
        if entity == 0 {
            continue;
        }
        let ty: u32 = get_from_memory(entity + UNIT_TYPE);
        if ty == 0 {
            continue;
        }
        let vtable: u32 = get_from_memory(ty);
        let check = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
        if check(ty, animal_arg) {
            return Some(ty);
        }
    }
    None
}

fn build_unit(ty: u32) -> u32 {
    let unit = unsafe { OPERATOR_NEW.original()(BFUNIT_SIZE) } as u32;
    unsafe { std::ptr::write_bytes(unit as *mut u8, 0, BFUNIT_SIZE as usize) };
    unsafe { BFUNIT_CONSTRUCTOR.original()(unit as *const u32, ty) };
    unit
}

fn destroy_unit(unit: u32) {
    unsafe { BFUNIT_DESTRUCTOR_1.original()(unit as *const u32, 1) };
}

/// A call object for `name(args)` parsed by real vanilla (`port == false`) or by the port.
fn build_call(port: bool, text: &str) -> u32 {
    let call = if port { bfscriptmgr_live_support::standalone_rust_call() } else { bfscriptmgr_live_support::standalone_call() };
    let source = crate::vanilla_string::VanillaString::new(text);
    if port {
        unsafe { openzt_detour::generated::bfscriptmgr::PARSE_FUNCTION.hooked()(source.as_ptr() as *const i32, call as *const u32) };
    } else {
        bfscriptmgr_live_support::real_parse_function(source.as_ptr(), call);
    }
    call
}

fn first_param(call: u32) -> u32 {
    get_from_memory::<u32>(call + 0xc)
}

/// The animation `findAnim` resolves for `name` on the unit's type, or 0.
fn resolve_anim(unit: u32, name: &str) -> u32 {
    let call = build_call(false, &format!("fProbe({name})"));
    unsafe { BFENTITY_FIND_ANIM.hooked()(unit as *const u32, first_param(call) as *const std::ffi::c_void) as u32 }
}

fn refcount(anim: u32) -> u32 {
    if anim == 0 {
        0
    } else {
        get_from_memory::<u32>(anim + ANIM_REFCOUNT_OFFSET)
    }
}

fn stamp(anim: u32) -> u32 {
    if anim == 0 {
        0
    } else {
        get_from_memory::<u32>(anim + ANIM_STAMP_OFFSET)
    }
}

fn cache_stamp() -> u32 {
    let cache = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + RVA_ANIM_CACHE);
    get_from_memory::<u32>(cache + ANIM_CACHE_STAMP_OFFSET)
}

/// Whether two words from the two poles' memory agree: equal, or both heap pointers neither of which is one of
/// the shared pointers (which must match exactly).
fn equivalent(real: u32, port: u32, shared: &[u32]) -> bool {
    real == port || (real >= 0x10000 && port >= 0x10000 && !shared.contains(&real) && !shared.contains(&port))
}

fn words(address: u32, length: u32) -> Vec<u32> {
    (0..length / 4).map(|i| get_from_memory::<u32>(address + i * 4)).collect()
}

/// What a case compares: the return value, the call's result flag, the unit's words, the mixer's two vectors (the
/// chain entries at `+0x00`, the `{anim, sound, flag}` links at `+0x0c`, valid elements only) and what
/// `GXMixer::getAnim` returns.
struct Observation {
    result: u32,
    call_flag: u8,
    unit: Vec<u32>,
    chains: Vec<[u32; 3]>,
    links: Vec<[u32; 3]>,
    mixer_anim: u32,
}

fn read_triples(begin: u32, end: u32) -> Vec<[u32; 3]> {
    let count = (end.wrapping_sub(begin) / 12).min(64);
    if begin < 0x10000 || end < begin || !readable(begin, count * 12) {
        return Vec::new();
    }
    (0..count).map(|i| [get_from_memory(begin + i * 12), get_from_memory(begin + i * 12 + 4), get_from_memory(begin + i * 12 + 8)]).collect()
}

fn observe(result: u32, unit: u32, call: u32) -> Observation {
    let mixer = unit + UNIT_MIXER;
    Observation {
        result,
        call_flag: get_from_memory::<u8>(call + CALL_RESULT_FLAG),
        unit: words(unit, BFUNIT_SIZE),
        chains: read_triples(get_from_memory(mixer), get_from_memory(mixer + 4)),
        links: read_triples(get_from_memory(mixer + 0xc), get_from_memory(mixer + 0x10)),
        mixer_anim: unsafe { GXMIXER_GET_ANIM.hooked()(mixer as *const u32) as u32 },
    }
}

fn compare(failures: &mut Vec<String>, label: &str, real: &Observation, port: &Observation, shared: &[u32]) {
    if real.result != port.result {
        failures.push(format!("{label}: return real={} port={}", real.result, port.result));
    }
    if real.call_flag != port.call_flag {
        failures.push(format!("{label}: call flag real={:#x} port={:#x}", real.call_flag, port.call_flag));
    }
    let mismatched: Vec<String> = real
        .unit
        .iter()
        .zip(&port.unit)
        .enumerate()
        .filter(|(_, (r, p))| !equivalent(**r, **p, shared))
        .map(|(i, (r, p))| format!("+{:#x}: real {r:#x} port {p:#x}", i * 4))
        .collect();
    if !mismatched.is_empty() {
        failures.push(format!("{label}: unit words differ: {}", mismatched.iter().take(12).cloned().collect::<Vec<_>>().join("; ")));
    }
    if real.mixer_anim != port.mixer_anim {
        failures.push(format!("{label}: GXMixer::getAnim real={:#x} port={:#x}", real.mixer_anim, port.mixer_anim));
    }
    if real.chains.len() != port.chains.len() {
        failures.push(format!("{label}: mixer chain entries real={} port={}", real.chains.len(), port.chains.len()));
    }
    // A link's third word is a flag byte; vanilla's copy leaves the other three bytes as stack garbage.
    let flag_byte = |links: &[[u32; 3]]| -> Vec<[u32; 3]> { links.iter().map(|l| [l[0], l[1], l[2] & 0xff]).collect() };
    if flag_byte(&real.links) != flag_byte(&port.links) {
        failures.push(format!("{label}: mixer links real={:x?} port={:x?}", real.links, port.links));
    }
}

/// How a case starts: the unit's stopped and moving flag bytes.
#[derive(Clone, Copy)]
struct Start {
    stopped: u8,
    moving: u8,
}

/// Runs one case: two fresh units over `ty` in `start`, a call parsed from `text` for each pole, `real`/`port` on
/// one unit each, then the comparison and the teardown check for every animation in `anims`. Returns a one-line
/// summary of what the case did.
#[allow(clippy::too_many_arguments)]
fn run_pair(
    failures: &mut Vec<String>,
    label: &str,
    ty: u32,
    anims: &[u32],
    start: Start,
    text: &str,
    real: &dyn Fn(u32, u32) -> u32,
    port: &dyn Fn(u32, u32) -> u32,
) -> String {
    let mut shared = vec![ty];
    shared.extend_from_slice(anims);
    let (real_unit, port_unit) = (build_unit(ty), build_unit(ty));
    for unit in [real_unit, port_unit] {
        save_to_memory(unit + UNIT_STOPPED, start.stopped);
        save_to_memory(unit + UNIT_MOVING, start.moving);
    }
    let (real_call, port_call) = (build_call(false, text), build_call(true, text));
    let refs_start: Vec<u32> = anims.iter().map(|a| refcount(*a)).collect();

    let real_result = real(real_call, real_unit);
    let refs_real: Vec<u32> = anims.iter().map(|a| refcount(*a)).collect();
    let port_result = port(port_call, port_unit);
    let refs_port: Vec<u32> = anims.iter().map(|a| refcount(*a)).collect();

    let (real_obs, port_obs) = (observe(real_result, real_unit, real_call), observe(port_result, port_unit, port_call));
    compare(failures, label, &real_obs, &port_obs, &shared);
    for (i, anim) in anims.iter().enumerate() {
        let (real_delta, port_delta) = (refs_real[i].wrapping_sub(refs_start[i]), refs_port[i].wrapping_sub(refs_real[i]));
        if real_delta != port_delta {
            failures.push(format!("{label}: refcount of {anim:#x} changed by real {real_delta}, port {port_delta}"));
        }
    }
    let summary = format!(
        "{label}: result {}, flag {:#x}, {} link(s), {} chain(s), +{:?} ref(s)",
        real_obs.result,
        real_obs.call_flag,
        real_obs.links.len(),
        real_obs.chains.len(),
        refs_real.iter().zip(&refs_start).map(|(a, b)| a.wrapping_sub(*b)).collect::<Vec<_>>()
    );

    destroy_unit(real_unit);
    destroy_unit(port_unit);
    for (i, anim) in anims.iter().enumerate() {
        if refcount(*anim) != refs_start[i] {
            failures.push(format!("{label}: refcount of {anim:#x} is {} after teardown, started at {}", refcount(*anim), refs_start[i]));
        }
    }
    summary
}

/// A real animal type of the live zoo and the candidate animation names it resolves.
fn live_type_and_anims(failures: &mut Vec<String>) -> Option<(u32, Vec<(&'static str, u32)>)> {
    let Some(ty) = live_animal_type() else {
        failures.push("no animal in the live zoo".to_string());
        return None;
    };
    if get_from_memory::<u32>(GLOBAL_ZTAIMGR) == 0 {
        failures.push("GLOBAL_ZTAIMgr is null".to_string());
        return None;
    }
    let probe = build_unit(ty);
    let found: Vec<(&str, u32)> = CANDIDATE_ANIMS.iter().map(|name| (*name, resolve_anim(probe, name))).filter(|(_, anim)| *anim != 0).collect();
    destroy_unit(probe);
    if found.len() < 2 {
        failures.push(format!("the animal type {ty:#x} resolves fewer than two of {CANDIDATE_ANIMS:?}"));
        return None;
    }
    Some((ty, found))
}

/// `ZTAIMGR_F_SET_BASE_F_REAL_UNIT_LIVE`: `fSetBaseF` on real units over a real animal type. Cases: animations the
/// type resolves and a name it does not, on a stopped unit, plus the not-stopped branches and the wrong
/// parameter counts. A stopped unit with the animation installs it as the mixer's set and base.
pub(crate) fn run_f_set_base_f_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_F_SET_BASE_F_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((ty, found)) = live_type_and_anims(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let ai = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
    let real = |call: u32, unit: u32| u32::from(ztaimgr_live_support::real_f_set_base_f(ai as *const u32, call as *const u32, unit as *const u32));
    let port = |call: u32, unit: u32| u32::from(unsafe { F_SET_BASE_F.hooked()(ai as *const u32, call as *const u32, unit as *const u32) });
    let (first, second) = (found[0], found[1]);
    let stopped = Start { stopped: 1, moving: 0 };
    let mut seen = Vec::new();
    let cases: Vec<(String, Vec<u32>, Start, String)> = vec![
        (format!("stopped {}", first.0), vec![first.1], stopped, format!("fSetBaseF({})", first.0)),
        (format!("stopped {}", second.0), vec![second.1], stopped, format!("fSetBaseF({})", second.0)),
        ("stopped nosuchanim".to_string(), vec![], stopped, "fSetBaseF(nosuchanim)".to_string()),
        (format!("moving {}", first.0), vec![first.1], Start { stopped: 0, moving: 1 }, format!("fSetBaseF({})", first.0)),
        (format!("idle {}", first.0), vec![first.1], Start { stopped: 0, moving: 0 }, format!("fSetBaseF({})", first.0)),
        ("no parameters".to_string(), vec![], stopped, "fSetBaseF()".to_string()),
        ("two parameters".to_string(), vec![], stopped, format!("fSetBaseF({},{})", first.0, second.0)),
    ];
    for (label, anims, start, text) in &cases {
        seen.push(run_pair(&mut failures, &format!("fSetBaseF {label}"), ty, anims, *start, text, &real, &port));
    }
    finish_test(&format!("{test_name} (type {ty:#x}; {})", seen.join("; ")), failures, failure_log)
}

/// `ZTAIMGR_F_PLAY_WITH_BASE_F_REAL_UNIT_LIVE`: `fPlayWithBaseF` on real units over a real animal type, through
/// the real `fPlay` and mixer. Stopped units with a resolvable base and one or two played animations (plain,
/// reverse, ping-pong), an unresolvable base, nothing resolvable to play, and the not-stopped, moving and
/// parameter-count branches.
pub(crate) fn run_f_play_with_base_f_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_F_PLAY_WITH_BASE_F_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((ty, found)) = live_type_and_anims(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let ai = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
    let (base, played) = (found[0], found[1]);
    let third = found.get(2).copied().unwrap_or(played);
    let stopped = Start { stopped: 1, moving: 0 };
    let mut seen = Vec::new();
    for (reverse, ping_pong) in [(0i8, 0i8), (1, 0), (0, 1), (1, 1)] {
        let real = |call: u32, unit: u32| u32::from(ztaimgr_live_support::real_f_play_with_base_f(ai as *const u32, call as *const u32, unit as *const u32, reverse, ping_pong));
        let port = |call: u32, unit: u32| u32::from(unsafe { F_PLAY_WITH_BASE_F.hooked()(ai as *const u32, call as *const u32, unit as *const u32, reverse, ping_pong) });
        let tag = format!("r{reverse}p{ping_pong}");
        let cases: Vec<(String, Vec<u32>, Start, String)> = vec![
            (format!("{tag} base {} plays {}", base.0, played.0), vec![base.1, played.1], stopped, format!("fPlayWithBaseF({},{})", base.0, played.0)),
            (format!("{tag} base {} plays {} and {}", base.0, played.0, third.0), vec![base.1, played.1, third.1], stopped, format!("fPlayWithBaseF({},{},{})", base.0, played.0, third.0)),
            (format!("{tag} base only resolves"), vec![base.1], stopped, format!("fPlayWithBaseF({},nosuchanim)", base.0)),
            (format!("{tag} base unresolved"), vec![played.1], stopped, format!("fPlayWithBaseF(nosuchanim,{})", played.0)),
            (format!("{tag} moving"), vec![base.1, played.1], Start { stopped: 0, moving: 1 }, format!("fPlayWithBaseF({},{})", base.0, played.0)),
            (format!("{tag} idle"), vec![base.1, played.1], Start { stopped: 0, moving: 0 }, format!("fPlayWithBaseF({},{})", base.0, played.0)),
            (format!("{tag} one parameter"), vec![], stopped, format!("fPlayWithBaseF({})", base.0)),
        ];
        for (label, anims, start, text) in &cases {
            seen.push(run_pair(&mut failures, &format!("fPlayWithBaseF {label}"), ty, anims, *start, text, &real, &port));
        }
    }
    finish_test(&format!("{test_name} (type {ty:#x}, {} cases; first: {})", seen.len(), seen.first().cloned().unwrap_or_default()), failures, failure_log)
}
