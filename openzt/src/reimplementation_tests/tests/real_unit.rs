//! T1 fixtures: real `ZTAnimal`s over a real animal type from the live zoo, so `findAnim`, the mixer and the
//! animation cache run for real instead of against recording stand-ins. Each case builds two units with the vanilla
//! `ZTAnimal` constructor over zeroed memory (identical starting state), runs real vanilla on one and the port on the other, and
//! compares the return value, the call's done flag, the unit's flags, the unit's and mixer's memory (heap pointers
//! may differ, shared pointers may not), and the animation reference counts. The units are then torn down through
//! the real destructor and the counts must return to where they started. The "real" pole's call object is built by
//! vanilla, the port's by the port, so each side's strings come from its own allocator.
//!
//! Needs the live zoo (a real animal type) and `GLOBAL_ZTAIMgr`.

use openzt_detour::generated::{
    bfaimgr::{F_PLAY_SPECIAL_F, F_PLAY_WITH_BASE_F, F_SET_BASE_F},
    ztaimgr::{F_DUST_BALL, F_DUST_BALL_F},
    bfentity::FIND_ANIM as BFENTITY_FIND_ANIM,
    gxmixer::GET_ANIM as GXMIXER_GET_ANIM,
    ztanimal::{CONSTRUCTOR as ZTANIMAL_CONSTRUCTOR, DESTRUCTOR_1 as ZTANIMAL_DESTRUCTOR_1},
    standalone::OPERATOR_NEW,
};
use windows::Win32::System::Memory::{VirtualQuery, MEMORY_BASIC_INFORMATION, PAGE_GUARD, PAGE_NOACCESS};

use crate::bfbehaviorset::live_support as bfbehaviorset_live_support;
use crate::bfscriptmgr::live_support as bfscriptmgr_live_support;
use crate::globals::{get_module_base, globals};
use crate::reimplementation_tests::harness::finish_test;
use crate::util::{get_from_memory, save_to_memory};
use crate::zoostatus::globals::raw_globals::ANIMAL_TYPE_CHECK_RVA;
use crate::ztaimgr::live_support as ztaimgr_live_support;

/// `ZTAnimalType::createEntity` allocates this many bytes.
const UNIT_SIZE: u32 = 0x3b8;
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
    build_kind(Kind::Animal, ty)
}

/// Tears down an animal built by [`build_unit`].
fn destroy_unit(unit: u32) {
    destroy_kind(Kind::Animal, unit);
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
    call: bfscriptmgr_live_support::CallSnapshot,
    unit: Vec<u32>,
    chains: Vec<[u32; 3]>,
    links: Vec<[u32; 3]>,
    mixer_anim: u32,
    targets: u32,
    /// Words 2..5 of each target node (what the path stored there).
    node_words: Vec<[u32; 3]>,
    /// The unit's behavior set (`+0x14`) and its nested children.
    sets: Vec<bfbehaviorset_live_support::SetSnapshot>,
}

fn read_triples(begin: u32, end: u32) -> Vec<[u32; 3]> {
    let count = (end.wrapping_sub(begin) / 12).min(64);
    if begin < 0x10000 || end < begin || !readable(begin, count * 12) {
        return Vec::new();
    }
    (0..count).map(|i| [get_from_memory(begin + i * 12), get_from_memory(begin + i * 12 + 4), get_from_memory(begin + i * 12 + 8)]).collect()
}

fn observe(result: u32, unit: u32, call: u32, kind: Kind) -> Observation {
    let mixer = unit + UNIT_MIXER;
    Observation {
        result,
        call_flag: get_from_memory::<u8>(call + CALL_RESULT_FLAG),
        call: bfscriptmgr_live_support::snapshot(call),
        unit: words(unit, kind.size()),
        chains: read_triples(get_from_memory(mixer), get_from_memory(mixer + 4)),
        links: read_triples(get_from_memory(mixer + 0xc), get_from_memory(mixer + 0x10)),
        mixer_anim: unsafe { GXMIXER_GET_ANIM.hooked()(mixer as *const u32) as u32 },
        targets: if kind.is_unit() { target_count(unit) } else { 0 },
        node_words: if kind.is_unit() { target_node_words(unit) } else { Vec::new() },
        sets: bfbehaviorset_live_support::snapshot_chain(unit + 0x14),
    }
}

fn compare(failures: &mut Vec<String>, label: &str, real: &Observation, port: &Observation, shared: &[u32]) {
    if real.result != port.result {
        failures.push(format!("{label}: return real={} port={}", real.result, port.result));
    }
    if real.call_flag != port.call_flag {
        failures.push(format!("{label}: call flag real={:#x} port={:#x}", real.call_flag, port.call_flag));
    }
    if real.call.name != port.call.name {
        failures.push(format!("{label}: call name real={:?} port={:?}", String::from_utf8_lossy(&real.call.name), String::from_utf8_lossy(&port.call.name)));
    }
    if real.call.args != port.call.args {
        failures.push(format!(
            "{label}: call args real={:?} port={:?}",
            real.call.args.iter().map(|a| String::from_utf8_lossy(a).into_owned()).collect::<Vec<_>>(),
            port.call.args.iter().map(|a| String::from_utf8_lossy(a).into_owned()).collect::<Vec<_>>()
        ));
    }
    if real.call.flags != port.call.flags {
        failures.push(format!("{label}: call flags real={:?} port={:?}", real.call.flags, port.call.flags));
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
    if real.targets != port.targets {
        failures.push(format!("{label}: target list holds real={} port={}", real.targets, port.targets));
    }
    if real.node_words != port.node_words {
        failures.push(format!("{label}: target node words real={:x?} port={:x?}", real.node_words, port.node_words));
    }
    if real.sets.len() != port.sets.len() {
        failures.push(format!("{label}: behavior-set chain length real={} port={}", real.sets.len(), port.sets.len()));
    }
    for (depth, (r, p)) in real.sets.iter().zip(&port.sets).enumerate() {
        let text = |bytes: &[u8]| String::from_utf8_lossy(bytes).into_owned();
        let lists = |list: &[Vec<u8>]| list.iter().map(|b| text(b)).collect::<Vec<_>>();
        if r.name != p.name
            || r.current_call != p.current_call
            || r.call.name != p.call.name
            || r.call.args != p.call.args
            || r.call.flags != p.call.flags
            || r.list != p.list
            || r.stack != p.stack
            || r.has_child != p.has_child
            || r.flags != p.flags
        {
            failures.push(format!(
                "{label}: behavior set depth {depth} real(name={:?} call={:?} list={:?} stack={:?} child={} flags={:?}) port(name={:?} call={:?} list={:?} stack={:?} child={} flags={:?})",
                text(&r.name),
                text(&r.current_call),
                lists(&r.list),
                lists(&r.stack),
                r.has_child,
                r.flags,
                text(&p.name),
                text(&p.current_call),
                lists(&p.list),
                lists(&p.stack),
                p.has_child,
                p.flags
            ));
        }
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
fn run_pair(failures: &mut Vec<String>, label: &str, ty: u32, anims: &[u32], start: Start, text: &str, real: &dyn Fn(u32, u32) -> u32, port: &dyn Fn(u32, u32) -> u32) -> String {
    run_pair_kind(Kind::Animal, failures, label, ty, anims, start, text, real, port)
}

#[allow(clippy::too_many_arguments)]
fn run_pair_kind(
    kind: Kind,
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
    let (real_unit, port_unit) = (build_kind(kind, ty), build_kind(kind, ty));
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

    let (real_obs, port_obs) = (observe(real_result, real_unit, real_call, kind), observe(port_result, port_unit, port_call, kind));
    compare(failures, label, &real_obs, &port_obs, &shared);
    for (i, anim) in anims.iter().enumerate() {
        let (real_delta, port_delta) = (refs_real[i].wrapping_sub(refs_start[i]), refs_port[i].wrapping_sub(refs_real[i]));
        if real_delta != port_delta {
            failures.push(format!("{label}: refcount of {anim:#x} changed by real {real_delta}, port {port_delta}"));
        }
    }
    let summary = format!(
        "{label}: result {}, flag {:#x}, {} link(s), {} chain(s), {} target(s), {} set(s), +{:?} ref(s)",
        real_obs.result,
        real_obs.call_flag,
        real_obs.links.len(),
        real_obs.chains.len(),
        real_obs.targets,
        real_obs.sets.len(),
        refs_real.iter().zip(&refs_start).map(|(a, b)| a.wrapping_sub(*b)).collect::<Vec<_>>()
    );

    destroy_kind(kind, real_unit);
    destroy_kind(kind, port_unit);
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

/// The distinct types of the animals in the live world (up to `limit`).
fn live_animal_types(limit: usize) -> Vec<u32> {
    let world = globals().ztworldmgr();
    let animal_arg = get_module_base("zoo.exe") as u32 + ANIMAL_TYPE_CHECK_RVA;
    let mut types: Vec<u32> = Vec::new();
    let mut slot = world.entity_array_start();
    while slot != world.entity_array_end() && types.len() < limit {
        let entity: u32 = get_from_memory(slot);
        slot += 4;
        if entity == 0 {
            continue;
        }
        let ty: u32 = get_from_memory(entity + UNIT_TYPE);
        if ty == 0 || types.contains(&ty) {
            continue;
        }
        let vtable: u32 = get_from_memory(ty);
        let check = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
        if check(ty, animal_arg) {
            types.push(ty);
        }
    }
    types
}

/// The unit's ball selector (`fDustBall` reads it at `+0x258`).
const UNIT_BALL_SELECTOR: u32 = 0x258;

/// `ZTAIMGR_F_DUST_BALL_REAL_UNIT_LIVE`: `fDustBall` and `fDustBallF` on real animals, for each ball selector (`0`
/// picks dust or water by the unit's tile, `3` surface, `4` submerged), the done flag clear and set, and animal
/// types that do and do not have the ball animations. Both poles reach the port's `fPlaySpecial` for the actual
/// playing, so this checks `fDustBall`'s own choices: which names it asks the type for, what it hands
/// `fPlaySpecial`, and the follow-up catch.
pub(crate) fn run_f_dust_ball_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_F_DUST_BALL_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let ai = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
    let types = live_animal_types(12);
    if types.is_empty() || ai == 0 {
        failures.push("no animal in the live zoo, or GLOBAL_ZTAIMgr is null".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    const BALL_NAMES: [&str; 4] = ["dustball", "waterball", "surfaceball", "submergedball"];
    let mut with_balls: Vec<(u32, Vec<u32>)> = Vec::new();
    let mut without_balls: Option<u32> = None;
    for ty in &types {
        let probe = build_unit(*ty);
        let anims: Vec<u32> = BALL_NAMES.iter().map(|name| resolve_anim(probe, name)).filter(|a| *a != 0).collect();
        destroy_unit(probe);
        if anims.is_empty() {
            without_balls.get_or_insert(*ty);
        } else {
            with_balls.push((*ty, anims));
        }
    }
    let mut subjects: Vec<(u32, Vec<u32>)> = with_balls.iter().take(2).cloned().collect();
    if let Some(ty) = without_balls {
        subjects.push((ty, Vec::new()));
    }

    let mut seen = Vec::new();
    for (ty, anims) in &subjects {
        for selector in [0u32, 3, 4] {
            for done_start in [0u8, 1] {
                for use_f in [false, true] {
                    let label = format!("fDustBall{} type {ty:#x} ({} ball anim(s)) selector {selector} done {done_start}", if use_f { "F" } else { "" }, anims.len());
                    let real = move |call: u32, unit: u32| {
                        save_to_memory(unit + UNIT_BALL_SELECTOR, selector);
                        let done = Box::new(done_start);
                        let result = if use_f {
                            save_to_memory(call + CALL_RESULT_FLAG, done_start);
                            ztaimgr_live_support::real_f_dust_ball_f(ai as *const u32, call as i32, unit as *const i32)
                        } else {
                            ztaimgr_live_support::real_f_dust_ball(ai as *const u32, unit as *const i32, &*done as *const u8 as *const i8)
                        };
                        u32::from(result) | (u32::from(*done) << 8)
                    };
                    let port = move |call: u32, unit: u32| {
                        save_to_memory(unit + UNIT_BALL_SELECTOR, selector);
                        let done = Box::new(done_start);
                        let result = if use_f {
                            save_to_memory(call + CALL_RESULT_FLAG, done_start);
                            unsafe { F_DUST_BALL_F.hooked()(ai as *const u32, call as i32, unit as *const i32) }
                        } else {
                            unsafe { F_DUST_BALL.hooked()(ai as *const u32, unit as *const i32, &*done as *const u8 as *const i8) }
                        };
                        u32::from(result) | (u32::from(*done) << 8)
                    };
                    let summary = run_pair(&mut failures, &label, *ty, anims, Start { stopped: 1, moving: 0 }, "fDustBallF()", &real, &port);
                    if seen.len() < 6 {
                        seen.push(summary);
                    }
                }
            }
        }
    }
    finish_test(&format!("{test_name} ({} subject type(s); first: {})", subjects.len(), seen.join("; ")), failures, failure_log)
}

/// The text of a vanilla global string cell (`{begin, end, cap}` behind a pointer), or `None` when unset.
fn global_string(rva: u32) -> Option<String> {
    let cell = get_from_memory::<u32>(get_module_base("zoo.exe") as u32 + rva);
    if cell == 0 {
        return None;
    }
    let (begin, end) = (get_from_memory::<u32>(cell), get_from_memory::<u32>(cell + 4));
    if begin == 0 || end < begin || end - begin > 64 {
        return None;
    }
    Some(String::from_utf8_lossy(unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }).into_owned())
}

/// `ZTAIMGR_F_PLAY_SPECIAL_F_REAL_UNIT_LIVE`: `fPlaySpecialF(anim, sound, seconds, count, special)` on real animals,
/// across the rewrite paths: the call becoming `fPlayTime`, `fPlay`/`fPlayReverse`/`fPlayPingPong`,
/// `fPlayWithSound` or a plain `fPlaySpecial`, and the failures. Compared as well as the unit: the call object's
/// name, arguments and flags after the rewrite, each pole rewriting its own allocator's strings.
pub(crate) fn run_f_play_special_f_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTAIMGR_F_PLAY_SPECIAL_F_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((ty, found)) = live_type_and_anims(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let ai = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
    let real = |call: u32, unit: u32| u32::from(ztaimgr_live_support::real_f_play_special_f(ai as *const u32, call as *const u32, unit as *const u32));
    let port = |call: u32, unit: u32| u32::from(unsafe { F_PLAY_SPECIAL_F.hooked()(ai as *const u32, call as *const u32, unit as *const u32) });

    let none = global_string(0x0023_950c);
    let specials: Vec<(&str, Option<String>)> = vec![
        ("none", none.clone()),
        ("modeA", global_string(0x0024_bdac)),
        ("modeB", global_string(0x0024_bda8)),
        ("reverse", global_string(0x0024_3620)),
        ("pingPong", global_string(0x0024_3624)),
    ];
    let usable = |text: &str| !text.is_empty() && !text.contains([' ', '"', ',', '(', ')']);
    let (Some(none), true) = (none, specials.iter().all(|(_, t)| t.as_deref().is_some_and(usable))) else {
        failures.push(format!("the special strings are not plain tokens: {specials:?}"));
        return finish_test(test_name, failures, failure_log);
    };
    let special_text = |name: &str| specials.iter().find(|(n, _)| *n == name).and_then(|(_, t)| t.clone()).unwrap_or_default();

    let (anim, other) = (found[0], found[1]);
    let stopped = Start { stopped: 1, moving: 0 };
    let sound = "nosuchsound";
    // (label, anim text, sound text, seconds, count, special name)
    let mut rows: Vec<(String, String, String, &str, &str, &str)> = Vec::new();
    for special in ["none", "modeA", "modeB", "reverse", "pingPong"] {
        rows.push((format!("anim, no sound, no time, {special}"), anim.0.to_string(), none.clone(), "0", "0", special));
        rows.push((format!("anim, no sound, 2s, {special}"), anim.0.to_string(), none.clone(), "2", "0", special));
        rows.push((format!("anim, sound, no time, {special}"), anim.0.to_string(), sound.to_string(), "0", "0", special));
        rows.push((format!("no anim, sound, no time, {special}"), none.clone(), sound.to_string(), "0", "0", special));
        rows.push((format!("anim, sound, 2s, {special}"), anim.0.to_string(), sound.to_string(), "2", "0", special));
        rows.push((format!("anim, no sound, repeat 3, {special}"), anim.0.to_string(), none.clone(), "0", "3", special));
        rows.push((format!("anim, sound, 2s and repeat 3, {special}"), anim.0.to_string(), sound.to_string(), "2", "3", special));
    }
    rows.push(("unresolvable anim, no sound".to_string(), "nosuchanim".to_string(), none.clone(), "0", "0", "none"));
    rows.push(("both anims named".to_string(), other.0.to_string(), none.clone(), "0", "0", "none"));

    let mut seen = Vec::new();
    for (label, anim_text, sound_text, seconds, count, special) in &rows {
        let text = format!("fPlaySpecialF({anim_text},{sound_text},{seconds},{count},{})", special_text(special));
        let summary = run_pair(&mut failures, &format!("fPlaySpecialF {label}"), ty, &[anim.1, other.1], stopped, &text, &real, &port);
        if seen.len() < 4 {
            seen.push(summary);
        }
    }
    finish_test(&format!("{test_name} (type {ty:#x}, {} cases; first: {})", rows.len(), seen.join("; ")), failures, failure_log)
}

// ---------------------------------------------------------------------------------------------------------------
// fMove
// ---------------------------------------------------------------------------------------------------------------

const UNIT_TARGET_LIST: u32 = 0x154;
const UNIT_COG_GOAL: u32 = 0x15c;
const COG_TARGET_TILE: u32 = 0x24;
const COG_GOAL_FLAG: u32 = 0x4c;
const UNIT_STATE: u32 = 0x170;
const UNIT_STATE_2: u32 = 0x1bc;
const UNIT_POS_X: u32 = 0x114;
const UNIT_POS_Y: u32 = 0x118;
const UNIT_POS_Z: u32 = 0x11c;
const UNIT_FLAG_D7: u32 = 0xd7;
const UNIT_FLAG_20C: u32 = 0x20c;
const UNIT_FINISHED: u32 = 0x20d;
const UNIT_COG_INVALID: u32 = 0x20e;
/// `ZTUnit`'s keep-moving predicate also reads this byte.
const UNIT_KEEP_MOVING_BYTE: u32 = 0x25c;
const CALL_PARAM_SIZE: u32 = 12;
const TARGET_NODE_SIZE: u32 = 0x14;

/// The unit's circular target list: `n` zeroed `PoolAlloc` nodes linked around the sentinel the constructor made.
/// Vanilla's `stop` returns them to the pool's freelist, and `~BFUnit` does the same for any that remain.
fn set_target_count(unit: u32, n: u32) {
    let sentinel = get_from_memory::<u32>(unit + UNIT_TARGET_LIST);
    let mut previous = sentinel;
    for _ in 0..n {
        let node = unsafe { openzt_detour::generated::poolalloc::ALLOCATE.original()(TARGET_NODE_SIZE) } as u32;
        unsafe { std::ptr::write_bytes(node as *mut u8, 0, TARGET_NODE_SIZE as usize) };
        save_to_memory(previous, node);
        save_to_memory(node + 4, previous);
        previous = node;
    }
    save_to_memory(previous, sentinel);
    save_to_memory(sentinel + 4, previous);
}

fn target_node_words(unit: u32) -> Vec<[u32; 3]> {
    let sentinel = get_from_memory::<u32>(unit + UNIT_TARGET_LIST);
    let mut node = get_from_memory::<u32>(sentinel);
    let mut words = Vec::new();
    while node != sentinel && words.len() < 64 {
        words.push([get_from_memory(node + 8), get_from_memory(node + 0xc), get_from_memory(node + 0x10)]);
        node = get_from_memory::<u32>(node);
    }
    words
}

fn target_count(unit: u32) -> u32 {
    let sentinel = get_from_memory::<u32>(unit + UNIT_TARGET_LIST);
    let mut node = get_from_memory::<u32>(sentinel);
    let mut count = 0;
    while node != sentinel && count < 64 {
        count += 1;
        node = get_from_memory::<u32>(node);
    }
    count
}

/// The state a case sets on a unit before calling `fMove`.
#[derive(Clone, Copy, Debug)]
struct MoveState {
    stopped: u8,
    moving: u8,
    finished: u8,
    state: u32,
    state_2: u32,
    targets: u32,
    cog_invalid: u8,
    cog_flag: u8,
    d7: u8,
    f20c: u8,
    reached: bool,
    keep_moving: u8,
    /// Tile the unit stands on (its coordinates are set to the tile's centre).
    tile: (u32, u32),
}

impl MoveState {
    const BASE: MoveState = MoveState {
        stopped: 1,
        moving: 0,
        finished: 0,
        state: 0,
        state_2: 0,
        targets: 0,
        cog_invalid: 0,
        cog_flag: 0,
        d7: 0,
        f20c: 0,
        reached: false,
        keep_moving: 0,
        tile: (0, 0),
    };

    fn apply(&self, unit: u32) {
        save_to_memory(unit + UNIT_POS_X, self.tile.0 * 64 + 32);
        save_to_memory(unit + UNIT_POS_Y, self.tile.1 * 64 + 32);
        save_to_memory(unit + UNIT_POS_Z, 0u32);
        save_to_memory(unit + UNIT_STOPPED, self.stopped);
        save_to_memory(unit + UNIT_MOVING, self.moving);
        save_to_memory(unit + UNIT_FINISHED, self.finished);
        save_to_memory(unit + UNIT_STATE, self.state);
        save_to_memory(unit + UNIT_STATE_2, self.state_2);
        save_to_memory(unit + UNIT_COG_INVALID, self.cog_invalid);
        save_to_memory(unit + UNIT_COG_GOAL + COG_GOAL_FLAG, self.cog_flag);
        save_to_memory(unit + UNIT_FLAG_D7, self.d7);
        save_to_memory(unit + UNIT_FLAG_20C, self.f20c);
        save_to_memory(unit + UNIT_KEEP_MOVING_BYTE, self.keep_moving);
        // `BFCogGoal::reachedTarget` compares the unit's tile with the goal's `+0x24`.
        let tile = unsafe { openzt_detour::generated::bfentity::GET_TILE.original()(unit as *const u32) } as u32;
        save_to_memory(unit + UNIT_COG_GOAL + COG_TARGET_TILE, if self.reached { tile } else { tile.wrapping_add(0x8c) });
        set_target_count(unit, self.targets);
    }
}

/// How `fMove` is called besides the unit's state.
#[derive(Clone, Copy, Debug)]
struct MoveArgs {
    seconds: i32,
    /// 0 none, 1 empty, 2 named
    extra: u8,
    reverse: u32,
    anim_found: bool,
}

impl MoveArgs {
    const BASE: MoveArgs = MoveArgs { seconds: 0, extra: 0, reverse: 0, anim_found: true };
}

/// A live-map tile whose water flags (`tile + 0x83`, low two bits) are clear, and one where they are set.
fn land_and_wet_tiles() -> (Option<(u32, u32)>, Option<(u32, u32)>) {
    let world = globals().ztworldmgr();
    let (mut land, mut wet) = (None, None);
    for y in 0..world.map_y_size.min(200) {
        for x in 0..world.map_x_size.min(200) {
            let flags = get_from_memory::<u8>(world.get_tile_ptr(x, y) + 0x83) & 3;
            if flags == 0 && land.is_none() {
                land = Some((x, y));
            } else if flags != 0 && wet.is_none() {
                wet = Some((x, y));
            }
        }
        if land.is_some() && wet.is_some() {
            break;
        }
    }
    (land, wet)
}

/// Whether `ZTAnimal`'s `hasTarget` virtual (`+0x180`) answers true for a unit of this type with state `0x72`.
fn type_has_target(ty: u32) -> bool {
    let probe = build_unit(ty);
    save_to_memory(probe + UNIT_STATE, 0x72u32);
    let target = get_from_memory::<u32>(get_from_memory::<u32>(probe) + 0x180);
    let has_target = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target) }(probe) as u8 != 0;
    destroy_unit(probe);
    has_target
}

/// `ZTAIMGR_F_MOVE_REAL_UNIT_LIVE`: `fMove` on real animals, over the state combinations that select each of its
/// paths: a finished unit, a stopped unit with and without targets (cog goal checks, the one-target stop, the
/// move start with one or two animations, sound and reverse), a moving unit (target reached or not, the
/// `setMoving` condition) and an idle one. The target list is a ring of real `PoolAlloc` nodes that vanilla's
/// `stop` frees; the virtuals (`hasTarget`, `canPlay`, `findAnim`, `setSpeed`) and `BFCogGoal::reachedTarget`
/// are the real ones, driven through the unit's state words.
pub(crate) fn run_f_move_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::bfaimgr::F_MOVE;

    let test_name = "ZTAIMGR_F_MOVE_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((first_ty, _)) = live_type_and_anims(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let ai = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
    let none = global_string(0x0023_950c).unwrap_or_default();
    let (land, wet) = land_and_wet_tiles();
    let Some(land) = land else {
        failures.push("the live map has no land tile".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let base = MoveState { tile: land, ..MoveState::BASE };
    // Subjects: the first animal type, and one whose `hasTarget` answers true (the moving-with-target branches).
    let mut subjects: Vec<u32> = vec![first_ty];
    if let Some(ty) = live_animal_types(40).into_iter().find(|ty| *ty != first_ty && type_has_target(*ty)) {
        subjects.push(ty);
    }

    let mut cases: Vec<(String, MoveState, MoveArgs)> = Vec::new();
    // A finished unit completes at once.
    for (stopped, moving) in [(1u8, 0u8), (0, 1), (0, 0)] {
        for state in [0u32, 1] {
            cases.push((format!("finished stopped={stopped} moving={moving} state={state}"), MoveState { stopped, moving, finished: 1, state, ..base }, MoveArgs::BASE));
        }
    }
    // Stopped, no targets: the cog goal checks.
    for state in [0u32, 1, 0x66, 0x72] {
        for keep_moving in [0u8, 1] {
            for cog_invalid in [0u8, 1] {
                for cog_flag in [0u8, 1] {
                    cases.push((
                        format!("stopped no targets state={state:#x} keepMoving={keep_moving} cogInvalid={cog_invalid} cogFlag={cog_flag}"),
                        MoveState { state, keep_moving, cog_invalid, cog_flag, ..base },
                        MoveArgs::BASE,
                    ));
                }
            }
        }
    }
    // Stopped with one target: stop when the state is not 3, otherwise start the move.
    for state_2 in [0u32, 3] {
        for state in [0u32, 1] {
            cases.push((format!("stopped one target state={state} state2={state_2}"), MoveState { targets: 1, state, state_2, ..base }, MoveArgs::BASE));
        }
    }
    // Stopped with several targets: the move start across its arguments.
    for targets in [2u32, 3] {
        for seconds in [0i32, 2000] {
            for extra in [0u8, 1, 2] {
                for reverse in [0u32, 1] {
                    for anim_found in [true, false] {
                        cases.push((
                            format!("start move targets={targets} seconds={seconds} extra={extra} reverse={reverse} animFound={anim_found}"),
                            MoveState { targets, ..base },
                            MoveArgs { seconds, extra, reverse, anim_found },
                        ));
                    }
                }
            }
        }
    }
    if let Some(wet) = wet {
        for seconds in [0i32, 2000] {
            for anim_found in [true, false] {
                cases.push((
                    format!("start move wet tile seconds={seconds} animFound={anim_found}"),
                    MoveState { targets: 2, tile: wet, ..base },
                    MoveArgs { seconds, anim_found, ..MoveArgs::BASE },
                ));
            }
        }
    }
    cases.push(("start move state=1 (prepare)".to_string(), MoveState { targets: 2, state: 1, ..base }, MoveArgs::BASE));
    // Moving.
    for state in [0u32, 1, 0x72] {
        for state_2 in [0u32, 1, 3] {
            for targets in [0u32, 1, 2] {
                for reached in [false, true] {
                    cases.push((
                        format!("moving state={state:#x} state2={state_2} targets={targets} reached={reached}"),
                        MoveState { stopped: 0, moving: 1, state, state_2, targets, reached, ..base },
                        MoveArgs::BASE,
                    ));
                }
            }
        }
    }
    // The `setMoving` condition also compares two mixer words (`+0x8c`/`+0x9c`, deque iterators) that only the
    // real mixer may set, so they are left as the unit's own mixer has them.
    for d7 in [0u8, 1] {
        for f20c in [0u8, 1] {
            for seconds in [0i32, 2000] {
                cases.push((
                    format!("moving keeps going d7={d7} 20c={f20c} seconds={seconds}"),
                    MoveState { stopped: 0, moving: 1, targets: 2, d7, f20c, ..base },
                    MoveArgs { seconds, ..MoveArgs::BASE },
                ));
            }
        }
    }
    cases.push(("idle".to_string(), MoveState { stopped: 0, moving: 0, targets: 1, ..base }, MoveArgs::BASE));

    let empty: &'static [u32; 3] = {
        static EMPTY_BYTE: u8 = 0;
        let byte = &EMPTY_BYTE as *const u8 as u32;
        Box::leak(Box::new([byte, byte, byte]))
    };
    let mut outcomes: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let mut total = 0usize;
    for (subject_index, ty) in subjects.iter().copied().enumerate() {
        let probe = build_unit(ty);
        let found: Vec<(&str, u32)> = CANDIDATE_ANIMS.iter().map(|name| (*name, resolve_anim(probe, name))).filter(|(_, anim)| *anim != 0).collect();
        destroy_unit(probe);
        let Some(anim) = found.first().copied() else {
            failures.push(format!("the animal type {ty:#x} resolves none of {CANDIDATE_ANIMS:?}"));
            continue;
        };
        for (label, state, args) in &cases {
            let anim_text = if args.anim_found { anim.0 } else { "nosuchanim" };
            let extra_text = match args.extra {
                2 => "nosuchsound".to_string(),
                _ => none.clone(),
            };
            let text = format!("fMove(first,{anim_text},{extra_text})");
            let (state, args) = (*state, *args);
            let invoke = move |port: bool, call: u32, unit: u32| -> u32 {
                state.apply(unit);
                let begin = get_from_memory::<u32>(call + 0xc);
                let extra: u32 = match args.extra {
                    1 => empty.as_ptr() as u32,
                    _ => begin + 2 * CALL_PARAM_SIZE,
                };
                let done = Box::new(0xaau8);
                let done_ptr = &*done as *const u8;
                let result = if port {
                    unsafe { F_MOVE.hooked()(ai as *const u32, begin as i32, args.seconds, unit as *const i32, done_ptr, (begin + CALL_PARAM_SIZE) as i32, 0x55, extra as *const i8, args.reverse, 100) }
                } else {
                    ztaimgr_live_support::real_f_move(ai as *const u32, begin as i32, args.seconds, unit as *const i32, done_ptr, (begin + CALL_PARAM_SIZE) as i32, 0x55, extra as *const i8, args.reverse, 100)
                };
                u32::from(result) | (u32::from(unsafe { done_ptr.read_volatile() }) << 8)
            };
            let anims: Vec<u32> = if args.anim_found { vec![anim.1] } else { Vec::new() };
            let summary = run_pair(&mut failures, &format!("fMove [type {ty:#x}] {label}"), ty, &anims, Start { stopped: 1, moving: 0 }, &text, &|c, u| invoke(false, c, u), &|c, u| invoke(true, c, u));
            let category = label.split(' ').take(2).collect::<Vec<_>>().join(" ");
            let key = summary
                .split_once(": ")
                .map(|(_, rest)| format!("s{subject_index} {category}: {}", rest.split(',').take(5).collect::<Vec<_>>().join(",")))
                .unwrap_or_default();
            *outcomes.entry(key).or_insert(0) += 1;
            total += 1;
        }
    }
    let coverage = outcomes.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join(" | ");
    finish_test(&format!("{test_name} ({} subject type(s), {total} cases; outcomes: {coverage})", subjects.len()), failures, failure_log)
}

// ---------------------------------------------------------------------------------------------------------------
// fPlaySetProb and fRandomWalk
// ---------------------------------------------------------------------------------------------------------------

/// The CRT `rand()` seed the game's `rand` (`0x0040f103`) advances: `seed = seed * 0x343fd + 0x269ec3`.
const RAND_SEED_ADDRESS: u32 = 0x0063_8060;

/// Runs `body` with the game's `rand` seed set to `seed`, restoring the previous seed afterwards.
fn with_rand_seed<R>(seed: u32, body: impl FnOnce() -> R) -> R {
    let previous = get_from_memory::<u32>(RAND_SEED_ADDRESS);
    save_to_memory(RAND_SEED_ADDRESS, seed);
    let result = body();
    save_to_memory(RAND_SEED_ADDRESS, previous);
    result
}

/// A `vector<(string, weight)>` of 16-byte entries for `fPlaySetProb`, in Rust memory (vanilla only reads it).
struct ProbEntries {
    _names: Vec<crate::owned_strings::OwnedString>,
    entries: Vec<[u32; 4]>,
    header: [u32; 3],
}

impl ProbEntries {
    fn new(entries: &[(&str, u32)]) -> Box<Self> {
        let names: Vec<crate::owned_strings::OwnedString> = entries.iter().map(|(name, _)| crate::owned_strings::OwnedString::new(name)).collect();
        let raw: Vec<[u32; 4]> = names
            .iter()
            .zip(entries)
            .map(|(name, (_, weight))| {
                let words = unsafe { *(name.as_ptr() as *const [u32; 3]) };
                [words[0], words[1], words[2], *weight]
            })
            .collect();
        let mut boxed = Box::new(ProbEntries { _names: names, entries: raw, header: [0; 3] });
        let begin = boxed.entries.as_ptr() as u32;
        let len = boxed.entries.len() as u32;
        boxed.header = [begin, begin + len * 16, begin + len * 16];
        boxed
    }

    fn ptr(&self) -> u32 {
        self.header.as_ptr() as u32
    }
}

/// `ZTAIMGR_F_PLAY_SET_PROB_REAL_UNIT_LIVE`: `fPlaySetProb(entries, unit)` on real animals, with real behavior-set
/// names, weight lists that do and do not sum to 100, an empty list and a null unit, over several `rand` seeds
/// (both poles start from the same seed). Both poles reach the port's `fPlaySet`, so what is compared is the pick
/// and the behavior-set chain it builds on the unit (names, calls, lists, child links).
pub(crate) fn run_f_play_set_prob_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::bfaimgr::F_PLAY_SET_PROB;

    let test_name = "ZTAIMGR_F_PLAY_SET_PROB_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((ty, _)) = live_type_and_anims(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let names = ["bIdle1", "bHappy1", "bCalm1", "bIdle2", "bAngry1"];
    let weight_lists: Vec<(&str, Vec<u32>)> = vec![
        ("single 100", vec![100]),
        ("two 50/50", vec![50, 50]),
        ("three 20/30/50", vec![20, 30, 50]),
        ("skewed 10/10/80", vec![10, 10, 80]),
        ("five even", vec![20, 20, 20, 20, 20]),
        ("short sum 50/30", vec![50, 30]),
        ("long sum 60/60", vec![60, 60]),
        ("empty", vec![]),
    ];
    let seeds = [1u32, 7, 12345, 99991, 0xdead_beef, 31337];
    let mut seen = Vec::new();
    for (label, weights) in &weight_lists {
        for seed in seeds {
            for null_unit in [false, true] {
                let entries: Vec<(&str, u32)> = weights.iter().enumerate().map(|(i, w)| (names[i % names.len()], *w)).collect();
                let table = ProbEntries::new(&entries);
                let table_ptr = table.ptr();
                let real = move |_call: u32, unit: u32| {
                    let unit = if null_unit { 0 } else { unit };
                    with_rand_seed(seed, || ztaimgr_live_support::real_f_play_set_prob(table_ptr as *const i32, unit as *const std::ffi::c_void)) & 0xff
                };
                let port = move |_call: u32, unit: u32| {
                    let unit = if null_unit { 0 } else { unit };
                    with_rand_seed(seed, || unsafe { F_PLAY_SET_PROB.hooked()(table_ptr as *const i32, unit as *const std::ffi::c_void) }) & 0xff
                };
                let summary = run_pair(&mut failures, &format!("fPlaySetProb {label} seed {seed:#x} nullUnit={null_unit}"), ty, &[], Start { stopped: 1, moving: 0 }, "fPlaySetProb()", &real, &port);
                if seen.len() < 5 && !null_unit {
                    seen.push(summary);
                }
                drop(table);
            }
        }
    }
    finish_test(&format!("{test_name} (type {ty:#x}, {} cases; first: {})", weight_lists.len() * seeds.len() * 2, seen.join("; ")), failures, failure_log)
}

/// Up to `count` land tiles spread over the live map, then a wet tile and the map corners.
fn sample_tiles(count: usize) -> Vec<(u32, u32)> {
    let world = globals().ztworldmgr();
    let (width, height) = (world.map_x_size, world.map_y_size);
    let mut tiles: Vec<(u32, u32)> = Vec::new();
    let step = (width.max(height) / (count as u32 + 1)).max(1);
    let mut wet = None;
    let mut y = step / 2;
    while y < height && tiles.len() < count {
        let mut x = step / 2;
        while x < width && tiles.len() < count {
            let flags = get_from_memory::<u8>(world.get_tile_ptr(x, y) + 0x83) & 3;
            if flags == 0 {
                tiles.push((x, y));
            } else if wet.is_none() {
                wet = Some((x, y));
            }
            x += step;
        }
        y += step;
    }
    tiles.extend(wet);
    tiles.push((0, 0));
    tiles.push((width.saturating_sub(1), height.saturating_sub(1)));
    tiles
}

/// `ZTAIMGR_F_RANDOM_WALK_REAL_UNIT_LIVE`: `fRandomWalk(unit)` on real animals at real map positions (land tiles
/// spread over the map, a wet tile and the corners), several `rand` seeds each, plus a unit with queued targets.
/// Real vanilla runs its own tile walk, the manager's `getPath` and the neighbour scan against the live map;
/// the port does the same through the same vtables. Compared: the result, the unit's words (destination, walking
/// guard), its target list (count and node words) and the call outcome.
pub(crate) fn run_f_random_walk_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::bfaimgr::F_RANDOM_WALK;

    let test_name = "ZTAIMGR_F_RANDOM_WALK_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((first_ty, _)) = live_type_and_anims(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let ai = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
    let mut subjects: Vec<u32> = vec![first_ty];
    subjects.extend(live_animal_types(40).into_iter().filter(|ty| *ty != first_ty).take(1));
    let tiles = sample_tiles(5);
    let seeds = [3u32, 1000, 77777, 0x1234_5678];
    let mut outcomes: std::collections::BTreeMap<String, u32> = std::collections::BTreeMap::new();
    let mut total = 0usize;
    for ty in subjects.iter().copied() {
        for tile in tiles.iter().copied() {
            for seed in seeds {
                for queued in [0u32, 2] {
                    if queued != 0 && seed != seeds[0] {
                        continue;
                    }
                    let state = MoveState { tile, targets: queued, ..MoveState::BASE };
                    let real = move |_call: u32, unit: u32| {
                        state.apply(unit);
                        with_rand_seed(seed, || ztaimgr_live_support::real_f_random_walk(ai as *const u32, unit as *const i32)) & 0xff
                    };
                    let port = move |_call: u32, unit: u32| {
                        state.apply(unit);
                        with_rand_seed(seed, || unsafe { F_RANDOM_WALK.hooked()(ai as *const u32, unit as *const i32) }) & 0xff
                    };
                    let label = format!("fRandomWalk [type {ty:#x}] tile {tile:?} seed {seed:#x} queued={queued}");
                    let summary = run_pair(&mut failures, &label, ty, &[], Start { stopped: 1, moving: 0 }, "fRandomWalk()", &real, &port);
                    let key = summary.split_once(": ").map(|(_, rest)| rest.split(',').take(1).chain(rest.split(',').skip(4).take(1)).collect::<Vec<_>>().join(",")).unwrap_or_default();
                    *outcomes.entry(key).or_insert(0) += 1;
                    total += 1;
                }
            }
        }
    }
    let coverage = outcomes.iter().map(|(k, v)| format!("{k} x{v}")).collect::<Vec<_>>().join(" | ");
    finish_test(&format!("{test_name} ({} subject type(s), {total} cases; outcomes: {coverage})", subjects.len()), failures, failure_log)
}

// ---------------------------------------------------------------------------------------------------------------
// Entity-class handlers: fShowPredator, fHidePredator, fCaughtPrey, fKillPrey, fDie, fHatch, fFaceToward*
// ---------------------------------------------------------------------------------------------------------------

/// A class of entity a handler can be run on.
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Kind {
    Animal,
    Guest,
    Keeper,
    Maint,
    Guide,
    Scenery,
}

impl Kind {
    const ALL: [Kind; 6] = [Kind::Animal, Kind::Guest, Kind::Keeper, Kind::Maint, Kind::Guide, Kind::Scenery];

    /// What `<Class>Type::createEntity` allocates.
    fn size(self) -> u32 {
        match self {
            Kind::Animal => 0x3b8,
            Kind::Guest => 0x348,
            Kind::Keeper => 0x2ac,
            Kind::Maint => 0x2c4,
            Kind::Guide => 0x2a4,
            Kind::Scenery => 0x158,
        }
    }

    /// `BFUnit`-derived (has the target list at `+0x154`).
    fn is_unit(self) -> bool {
        self != Kind::Scenery
    }

    /// The class tag the entity type's `isCastClass` test takes.
    fn type_tag_rva(self) -> u32 {
        match self {
            Kind::Animal => 0x0023_8690,
            Kind::Guest => 0x0023_8700,
            Kind::Keeper => 0x0023_8760,
            Kind::Maint => 0x0023_8780,
            Kind::Guide => 0x0023_8770,
            Kind::Scenery => 0x0023_8670,
        }
    }

    fn construct(self, at: u32, ty: u32) {
        use openzt_detour::generated::{ztguide, ztguest, ztkeeper, ztmaint, ztscenery};
        unsafe {
            match self {
                Kind::Animal => {
                    ZTANIMAL_CONSTRUCTOR.original()(at as *const u32, ty);
                }
                Kind::Guest => {
                    ztguest::CONSTRUCTOR.original()(at as *const u32, ty);
                }
                Kind::Keeper => {
                    ztkeeper::CONSTRUCTOR.original()(at as *const u32, ty as *const u32);
                }
                Kind::Maint => {
                    ztmaint::CONSTRUCTOR.original()(at as *const i32, ty as *const i32);
                }
                Kind::Guide => {
                    ztguide::CONSTRUCTOR.original()(at as *const u32, ty as *const u32);
                }
                Kind::Scenery => {
                    ztscenery::CONSTRUCTOR.original()(at as *const u32, ty as i32);
                }
            }
        }
    }
}

/// A live-world entity type of `kind` (the first the world holds), if any.
fn live_type_of_kind(kind: Kind) -> Option<u32> {
    let world = globals().ztworldmgr();
    let tag = get_module_base("zoo.exe") as u32 + kind.type_tag_rva();
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
        if check(ty, tag) {
            return Some(ty);
        }
    }
    None
}

/// A zeroed entity of `kind` built by the vanilla constructor over `ty`.
fn build_kind(kind: Kind, ty: u32) -> u32 {
    let entity = unsafe { OPERATOR_NEW.original()(kind.size()) } as u32;
    unsafe { std::ptr::write_bytes(entity as *mut u8, 0, kind.size() as usize) };
    kind.construct(entity, ty);
    entity
}

/// Tears an entity down through its class's non-virtual deleting destructor (the entity vtables have no destructor
/// slot). `ZTMaint` and `ZTGuide` have no decompiled destructor, so those fixtures are leaked.
fn destroy_kind(kind: Kind, entity: u32) {
    use openzt_detour::generated::{ztguest, ztkeeper, ztscenery};
    unsafe {
        match kind {
            Kind::Animal => {
                ZTANIMAL_DESTRUCTOR_1.original()(entity as *const u32, 1);
            }
            Kind::Guest => {
                ztguest::DESTRUCTOR_1.original()(entity as *const u32, 1);
            }
            Kind::Keeper => {
                ztkeeper::DESTRUCTOR_0.original()(entity as *const u32, 1);
            }
            Kind::Scenery => {
                ztscenery::DESTRUCTOR_1.original()(entity as *const u32, 1);
            }
            Kind::Maint | Kind::Guide => {}
        }
    }
}

/// Positions an entity at the centre of a tile.
fn place_at(entity: u32, tile: (u32, u32)) {
    save_to_memory(entity + UNIT_POS_X, tile.0 * 64 + 32);
    save_to_memory(entity + UNIT_POS_Y, tile.1 * 64 + 32);
    save_to_memory(entity + UNIT_POS_Z, 0u32);
}

/// A live-map tile that belongs to a habitat, if there is one.
fn habitat_tile() -> Option<(u32, u32)> {
    let world = globals().ztworldmgr();
    let habitats = globals().zthabitatmgr();
    for y in 0..world.map_y_size.min(200) {
        for x in 0..world.map_x_size.min(200) {
            if habitats.get_habitat_ptr(x as i32, y as i32) != 0 {
                return Some((x, y));
            }
        }
    }
    None
}

type HandlerFn = fn(unit: u32, done: u32) -> u32;

/// One handler under test: its real and port entry points, the classes it is run on, the classes on which the port
/// must defer to vanilla (checked without running vanilla, which would dereference null for them), and how.
struct Handler {
    name: &'static str,
    real: HandlerFn,
    port: HandlerFn,
    run_on: &'static [Kind],
    /// `Some(check)`: `check(kind)` runs the port's pure function on a standalone entity of that kind and says whether it
    /// deferred.
    defers_on: &'static [Kind],
    defers: Option<fn(unit: u32, done: u32) -> bool>,
}

fn handlers() -> Vec<Handler> {
    use openzt_detour::generated::ztaimgr::{F_CAUGHT_PREY, F_DIE, F_FACE_TOWARD_FOOD, F_FACE_TOWARD_TARGET, F_HATCH, F_HIDE_PREDATOR, F_KILL_PREY, F_SHOW_PREDATOR};
    const ALL: &[Kind] = &[Kind::Animal, Kind::Guest, Kind::Keeper, Kind::Maint, Kind::Guide, Kind::Scenery];
    const NOT_GUEST: &[Kind] = &[Kind::Animal, Kind::Keeper, Kind::Maint, Kind::Guide, Kind::Scenery];
    const NOT_ANIMAL: &[Kind] = &[Kind::Guest, Kind::Keeper, Kind::Maint, Kind::Guide, Kind::Scenery];
    vec![
        Handler {
            name: "fShowPredator",
            real: |u, d| ztaimgr_live_support::real_f_show_predator(u as *const i32, d as *const u8),
            port: |u, d| unsafe { F_SHOW_PREDATOR.hooked()(u as *const i32, d as *const u8) },
            run_on: &[Kind::Guest],
            defers_on: NOT_GUEST,
            defers: Some(|u, d| unsafe { crate::ztaimgr::f_show_predator(u, d as *const bool) }.is_none()),
        },
        Handler {
            name: "fHidePredator",
            real: |u, d| u32::from(ztaimgr_live_support::real_f_hide_predator(u as *const u32, d as *const bool)),
            port: |u, d| u32::from(unsafe { F_HIDE_PREDATOR.hooked()(u as *const u32, d as *const bool) }),
            run_on: &[Kind::Guest],
            defers_on: NOT_GUEST,
            defers: Some(|u, d| !unsafe { crate::ztaimgr::f_hide_predator(u, d as *const bool) }),
        },
        Handler {
            name: "fCaughtPrey",
            real: |u, d| ztaimgr_live_support::real_f_caught_prey(u as *const i32, d as *const u8),
            port: |u, d| unsafe { F_CAUGHT_PREY.hooked()(u as *const i32, d as *const u8) },
            run_on: ALL,
            defers_on: &[],
            defers: None,
        },
        Handler {
            name: "fKillPrey",
            real: |u, d| ztaimgr_live_support::real_f_kill_prey(u as *const u8, d as *const u8),
            port: |u, d| unsafe { F_KILL_PREY.hooked()(u as *const u8, d as *const u8) },
            run_on: ALL,
            defers_on: &[],
            defers: None,
        },
        Handler {
            name: "fDie",
            real: |u, d| ztaimgr_live_support::real_f_die(u as *const i32, d as *const u8),
            port: |u, d| unsafe { F_DIE.hooked()(u as *const i32, d as *const u8) },
            run_on: ALL,
            defers_on: &[],
            defers: None,
        },
        Handler {
            name: "fHatch",
            real: |u, d| ztaimgr_live_support::real_f_hatch(u as *const i32, d as *const u8),
            port: |u, d| unsafe { F_HATCH.hooked()(u as *const i32, d as *const u8) },
            run_on: &[Kind::Animal],
            defers_on: NOT_ANIMAL,
            defers: Some(|u, d| !unsafe { crate::ztaimgr::f_hatch(u, d as *const bool) }),
        },
        Handler {
            name: "fFaceTowardFood",
            real: |u, d| ztaimgr_live_support::real_f_face_toward_food(u as *const u8, d as *const u8),
            port: |u, d| unsafe { F_FACE_TOWARD_FOOD.hooked()(u as *const u8, d as *const u8) },
            run_on: ALL,
            defers_on: &[],
            defers: None,
        },
        Handler {
            name: "fFaceTowardTarget",
            real: |u, d| ztaimgr_live_support::real_f_face_toward_target(u as *const std::ffi::c_void, d as *const u8),
            port: |u, d| unsafe { F_FACE_TOWARD_TARGET.hooked()(u as *const std::ffi::c_void, d as *const u8) },
            run_on: ALL,
            defers_on: &[],
            defers: None,
        },
    ]
}

/// Offsets of the tile pointers `fFaceTowardFood` (animal `+0x38c`) and `fFaceTowardTarget` (unit `+0x234`) read.
const ANIMAL_FOOD_TILE: u32 = 0x38c;
const UNIT_TARGET_TILE: u32 = 0x234;

/// How a handler case starts besides the entity's class.
#[derive(Clone, Copy, Debug)]
enum Variant {
    /// Entity on a land tile, no targets.
    Plain,
    /// Entity on a land tile with food/target tile pointers set to another tile.
    WithTargetTile,
    /// Entity on a habitat tile (so `fDie` reaches the habitat notification).
    InHabitat,
}

/// `ZTAIMGR_ENTITY_HANDLERS_REAL_UNIT_LIVE`: `fShowPredator`, `fHidePredator`, `fCaughtPrey`, `fKillPrey`, `fDie`,
/// `fHatch`, `fFaceTowardFood` and `fFaceTowardTarget` on real standalone entities of each class (animal, guest,
/// keeper, maintenance worker, guide, scenery), each built by its vanilla constructor over a real type from the live
/// zoo. Compared: the return value, the done byte and the entity's whole memory. Where the port defers to vanilla
/// (which would dereference null), the port's own decision is checked instead. The live zoo is reloaded afterwards
/// because `fDie` notifies a live habitat.
pub(crate) fn run_entity_handlers_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::battery::detour_zoo_main::run_load_live_zoo;

    let test_name = "ZTAIMGR_ENTITY_HANDLERS_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types: Vec<(Kind, u32)> = Kind::ALL.iter().filter_map(|kind| live_type_of_kind(*kind).map(|ty| (*kind, ty))).collect();
    let (land, _) = land_and_wet_tiles();
    let Some(land) = land else {
        failures.push("the live map has no land tile".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let other_tile = sample_tiles(3).get(1).copied().unwrap_or((land.0 + 1, land.1));
    let world = globals().ztworldmgr();
    let (tile_a, tile_b) = (world.get_tile_ptr(other_tile.0, other_tile.1), world.get_tile_ptr(land.0, land.1));
    let _ = tile_b;
    let habitat = habitat_tile();
    let missing: Vec<Kind> = Kind::ALL.iter().copied().filter(|k| !types.iter().any(|(kind, _)| kind == k)).collect();

    let mut seen: Vec<String> = Vec::new();
    for handler in handlers() {
        for (kind, ty) in &types {
            if !handler.run_on.contains(kind) {
                continue;
            }
            let mut variants = vec![Variant::Plain];
            if matches!(handler.name, "fFaceTowardFood" | "fFaceTowardTarget" | "fCaughtPrey" | "fKillPrey") {
                variants.push(Variant::WithTargetTile);
            }
            if handler.name == "fDie" && habitat.is_some() {
                variants.push(Variant::InHabitat);
            }
            for variant in variants {
                let label = format!("{} on {kind:?} ({variant:?})", handler.name);
                let tile = match variant {
                    Variant::InHabitat => habitat.unwrap_or(land),
                    _ => land,
                };
                let (real, port) = (handler.real, handler.port);
                let prepare = move |entity: u32| {
                    place_at(entity, tile);
                    if matches!(variant, Variant::WithTargetTile) {
                        if *kind == Kind::Animal {
                            save_to_memory(entity + ANIMAL_FOOD_TILE, tile_a);
                        }
                        if kind.is_unit() {
                            save_to_memory(entity + UNIT_TARGET_TILE, tile_a);
                        }
                    }
                };
                let invoke = move |is_port: bool, unit: u32| -> u32 {
                    prepare(unit);
                    let done = Box::new(0u8);
                    let done_ptr = &*done as *const u8 as u32;
                    let result = if is_port { port(unit, done_ptr) } else { real(unit, done_ptr) };
                    (result & 0xff) | (u32::from(unsafe { (done_ptr as *const u8).read_volatile() }) << 8)
                };
                let summary = run_pair_kind(*kind, &mut failures, &label, *ty, &[], Start { stopped: 1, moving: 0 }, "fProbe()", &|_c, u| invoke(false, u), &|_c, u| invoke(true, u));
                seen.push(format!("{label} => {}", summary.split_once(": ").map(|(_, rest)| rest.split(',').next().unwrap_or("")).unwrap_or("")));
            }
        }
        // A null unit, for the handlers vanilla also takes it for.
        if matches!(handler.name, "fCaughtPrey" | "fKillPrey" | "fDie" | "fFaceTowardFood" | "fFaceTowardTarget") {
            let (real, port) = (handler.real, handler.port);
            let (done_real, done_port) = (Box::new(0u8), Box::new(0u8));
            let (real_result, port_result) = (real(0, &*done_real as *const u8 as u32) & 0xff, port(0, &*done_port as *const u8 as u32) & 0xff);
            if real_result != port_result || *done_real != *done_port {
                failures.push(format!("{} on a null unit: result real={real_result} port={port_result}, done real={} port={}", handler.name, *done_real, *done_port));
            }
        }
        // Classes the port defers on: its own decision, with no vanilla call.
        if let Some(defers) = handler.defers {
            for kind in handler.defers_on {
                let Some((_, ty)) = types.iter().find(|(k, _)| k == kind) else { continue };
                let unit = build_kind(*kind, *ty);
                let done = Box::new(0u8);
                if !defers(unit, &*done as *const u8 as u32) {
                    failures.push(format!("{} on {kind:?}: the port did not defer to vanilla", handler.name));
                }
                if *done != 0 {
                    failures.push(format!("{} on {kind:?}: the port touched the done flag while deferring", handler.name));
                }
                destroy_kind(*kind, unit);
            }
        }
    }
    if !run_load_live_zoo(&mut None) {
        failures.push("could not reload the live zoo afterwards".to_string());
    }
    finish_test(
        &format!("{test_name} ({} classes with a live type, missing {missing:?}; habitat tile {habitat:?}; first: {})", types.len(), seen.join("; ")),
        failures,
        failure_log,
    )
}

/// The first `count` live animals (entities whose type passes the animal test and that are not flagged removed).
fn live_animal_entities(count: usize) -> Vec<u32> {
    let world = globals().ztworldmgr();
    let animal_arg = get_module_base("zoo.exe") as u32 + ANIMAL_TYPE_CHECK_RVA;
    let mut found = Vec::new();
    let mut slot = world.entity_array_start();
    while slot != world.entity_array_end() && found.len() < count {
        let entity: u32 = get_from_memory(slot);
        slot += 4;
        if entity == 0 || get_from_memory::<u8>(entity + 0x142) != 0 || get_from_memory::<u8>(entity + 0x143) != 0 || get_from_memory::<u32>(entity + 0x124) == 0 {
            continue;
        }
        let ty: u32 = get_from_memory(entity + UNIT_TYPE);
        if ty == 0 {
            continue;
        }
        let vtable: u32 = get_from_memory(ty);
        let check = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
        if check(ty, animal_arg) {
            found.push(entity);
        }
    }
    found
}

/// Words of a live entity that a handler changed: `(offset, before, after)`.
fn changed_words(before: &[u32], after: &[u32]) -> Vec<(u32, u32, u32)> {
    before.iter().zip(after).enumerate().filter(|(_, (b, a))| b != a).map(|(i, (b, a))| (i as u32 * 4, *b, *a)).collect()
}

/// Whether two poles changed two (different, same-class) entities the same way: the same offsets, and at each the
/// same new value or the same change (heap pointers count as equal).
fn same_change(real: &[(u32, u32, u32)], port: &[(u32, u32, u32)]) -> bool {
    real.len() == port.len()
        && real.iter().zip(port).all(|(r, p)| {
            r.0 == p.0 && (equivalent(r.2, p.2, &[]) || r.2.wrapping_sub(r.1) == p.2.wrapping_sub(p.1))
        })
}

/// `ZTAIMGR_HUNTER_PREY_REAL_UNIT_LIVE`: `fCaughtPrey` and `fKillPrey` with a real target. A standalone hunter animal
/// has the entity id of a live animal in its target field (`+0x178`), so `getTargetUnit` finds it through the world;
/// the real pole acts on one live animal and the port on another (they cannot share one, since the first changes
/// it). Compared: the hunter, the result and done flag, and how each prey changed (same words, same new values or
/// changes). The live zoo is reloaded afterwards.
pub(crate) fn run_hunter_prey_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::battery::detour_zoo_main::run_load_live_zoo;
    use std::cell::RefCell;

    let test_name = "ZTAIMGR_HUNTER_PREY_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some((hunter_ty, _)) = live_type_and_anims(&mut failures) else {
        return finish_test(test_name, failures, failure_log);
    };
    let (land, _) = land_and_wet_tiles();
    let land = land.unwrap_or((0, 0));
    let mut seen = Vec::new();

    for handler in handlers().into_iter().filter(|h| matches!(h.name, "fCaughtPrey" | "fKillPrey")) {
        let preys = live_animal_entities(4);
        if preys.len() < 4 {
            failures.push(format!("{}: need four live animals, found {}", handler.name, preys.len()));
            continue;
        }
        // Two poles, two prey each time so the order of the poles does not matter: (real, port) = (0, 1) then (2, 3).
        for (round, (real_prey, port_prey)) in [(preys[0], preys[1]), (preys[2], preys[3])].into_iter().enumerate() {
            let changes: RefCell<Vec<Vec<(u32, u32, u32)>>> = RefCell::new(Vec::new());
            let (real, port) = (handler.real, handler.port);
            let invoke = |is_port: bool, unit: u32| -> u32 {
                let prey = if is_port { port_prey } else { real_prey };
                place_at(unit, land);
                save_to_memory(unit + 0x178, get_from_memory::<u32>(prey + 0x124));
                let before = words(prey, Kind::Animal.size());
                let done = Box::new(0u8);
                let done_ptr = &*done as *const u8 as u32;
                let result = if is_port { port(unit, done_ptr) } else { real(unit, done_ptr) };
                changes.borrow_mut().push(changed_words(&before, &words(prey, Kind::Animal.size())));
                (result & 0xff) | (u32::from(unsafe { (done_ptr as *const u8).read_volatile() }) << 8)
            };
            let label = format!("{} with a real target (round {round})", handler.name);
            let summary = run_pair_kind(Kind::Animal, &mut failures, &label, hunter_ty, &[], Start { stopped: 1, moving: 0 }, "fProbe()", &|_c, u| invoke(false, u), &|_c, u| invoke(true, u));
            let changes = changes.into_inner();
            if changes.len() == 2 {
                if !same_change(&changes[0], &changes[1]) {
                    failures.push(format!("{label}: the prey changed differently: real {:x?} port {:x?}", changes[0], changes[1]));
                }
                seen.push(format!("{label}: {}, prey words changed real {} port {}", summary.split_once(": ").map(|(_, r)| r.split(',').next().unwrap_or("")).unwrap_or(""), changes[0].len(), changes[1].len()));
            } else {
                failures.push(format!("{label}: expected two prey observations, got {}", changes.len()));
            }
        }
    }
    if !run_load_live_zoo(&mut None) {
        failures.push("could not reload the live zoo afterwards".to_string());
    }
    finish_test(&format!("{test_name} ({})", seen.join("; ")), failures, failure_log)
}

/// `ZTAIMGR_TARGET_LIST_REAL_UNIT_LIVE`: the target-list helper against vanilla's own list handling on a real unit.
/// Nodes pushed through the helper come from `PoolAlloc`'s 20-byte freelist; vanilla's `BFUnit::stop` must free
/// exactly those nodes back to it and empty the list, and the helper's own `clear` must do the same. The next push
/// after a free reuses the last node freed, as vanilla's inlined freelist pop would.
pub(crate) fn run_target_list_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::tests::bfentity::{pool_lengths, warm_pool};
    use ztaimgr_live_support::{TargetList, VanillaPool};

    let test_name = "ZTAIMGR_TARGET_LIST_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    // Size class of 20-byte blocks: `(20 - 1) >> 3`.
    const BUCKET: usize = 2;

    if let Some(ty) = live_animal_type() {
        let unit = build_unit(ty);
        let list = TargetList::of_unit(unit);
        if !list.is_empty() {
            failures.push(format!("a new unit's list holds {} nodes", list.len()));
        }

        warm_pool();
        let before = pool_lengths();
        let mut pool = VanillaPool;
        let wanted = [[11, 12, 13], [22, 23, 24], [33, 34, 35]];
        for position in wanted {
            unsafe { list.push_back(&mut pool, position) };
        }
        if list.len() != 3 || list.positions() != wanted {
            failures.push(format!("after three pushes: len {} positions {:?}", list.len(), list.positions()));
        }
        let pushed = pool_lengths();
        if pushed[BUCKET] != before[BUCKET] - 3 {
            failures.push(format!("three pushes took {} blocks from the 20-byte class, expected 3", before[BUCKET] as i64 - pushed[BUCKET] as i64));
        }
        let last_node = {
            let sentinel = get_from_memory::<u32>(unit + 0x154);
            get_from_memory::<u32>(sentinel + 4)
        };

        crate::bfunit::live_support::real::stop(unit, 0, 0);
        if !list.is_empty() {
            failures.push(format!("vanilla BFUnit::stop left {} nodes", list.len()));
        }
        let stopped = pool_lengths();
        if stopped != before {
            failures.push(format!("vanilla stop did not return exactly the pushed nodes: before {:?} after {:?}", &before[..4], &stopped[..4]));
        }

        unsafe { list.push_back(&mut pool, [44, 0, 0]) };
        let reused = get_from_memory::<u32>(get_from_memory::<u32>(unit + 0x154) + 4);
        if reused != last_node {
            failures.push(format!("the next push got {reused:#x}, expected the last node vanilla freed ({last_node:#x})"));
        }
        unsafe { list.push_back(&mut pool, [55, 0, 0]) };
        unsafe { list.clear(&mut pool) };
        if !list.is_empty() || pool_lengths() != before {
            failures.push(format!("the helper's clear left the list or the pool different: len {} pool {:?}", list.len(), &pool_lengths()[..4]));
        }
        destroy_unit(unit);
    } else {
        failures.push("no animal in the live zoo".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

const MOVEMENT_FLAGS: [u32; 6] = [0xd4, 0xd6, 0x20c, 0x20d, 0x20e, 0x210];

fn movement_flags(unit: u32) -> [u8; 6] {
    MOVEMENT_FLAGS.map(|offset| get_from_memory::<u8>(unit + offset))
}

/// Two units over the same animal type with identical movement flags and `nodes` target-list nodes.
fn movement_pair(ty: u32, flags: [u8; 6], nodes: u32) -> (u32, u32) {
    use ztaimgr_live_support::{TargetList, VanillaPool};
    let (real, port) = (build_unit(ty), build_unit(ty));
    for unit in [real, port] {
        for (offset, value) in MOVEMENT_FLAGS.iter().zip(flags) {
            save_to_memory(unit + offset, value);
        }
        let list = TargetList::of_unit(unit);
        for index in 0..nodes as i32 {
            unsafe { list.push_back(&mut VanillaPool, [index, index + 1, index + 2]) };
        }
    }
    (real, port)
}

/// Runs `run_real` on one unit and `run_port` on the other and compares the flag bytes, the target list and the pool.
fn compare_movement(failures: &mut Vec<String>, label: &str, real: u32, port: u32, run_real: impl FnOnce(u32), run_port: impl FnOnce(u32)) {
    use crate::reimplementation_tests::tests::bfentity::{pool_lengths, warm_pool};
    use ztaimgr_live_support::TargetList;

    warm_pool();
    let before = pool_lengths();
    run_real(real);
    let middle = pool_lengths();
    run_port(port);
    let after = pool_lengths();

    if movement_flags(real) != movement_flags(port) {
        failures.push(format!("{label}: flags real={:?} port={:?}", movement_flags(real), movement_flags(port)));
    }
    let (real_len, port_len) = (TargetList::of_unit(real).len(), TargetList::of_unit(port).len());
    if real_len != port_len {
        failures.push(format!("{label}: list length real={real_len} port={port_len}"));
    }
    let real_delta: Vec<u32> = middle.iter().zip(&before).map(|(a, b)| a.wrapping_sub(*b)).collect();
    let port_delta: Vec<u32> = after.iter().zip(&middle).map(|(a, b)| a.wrapping_sub(*b)).collect();
    if real_delta != port_delta {
        failures.push(format!("{label}: freelist growth real={real_delta:?} port={port_delta:?}"));
    }
}

/// `BFUNIT_STOP_REAL_UNIT_LIVE`: `BFUnit::stop` against vanilla on real units, with and without queued target
/// nodes, every combination of the clear and idle arguments and several starting flags.
pub(crate) fn run_bfunit_stop_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFUNIT_STOP_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Some(ty) = live_animal_type() {
        for nodes in [0u32, 1, 4] {
            for clear in [0u8, 1] {
                for idle in [0u8, 1] {
                    for flags in [[1u8, 0, 1, 0, 0, 0], [1, 0, 0, 1, 1, 1], [0, 1, 1, 1, 0, 0]] {
                        let label = format!("nodes={nodes} clear={clear} idle={idle} flags={flags:?}");
                        let (real, port) = movement_pair(ty, flags, nodes);
                        compare_movement(
                            &mut failures,
                            &label,
                            real,
                            port,
                            |unit| crate::bfunit::live_support::real::stop(unit, clear, idle),
                            |unit| unsafe { crate::bfunit::stop(unit, clear, idle) },
                        );
                        destroy_unit(real);
                        destroy_unit(port);
                    }
                }
            }
        }
    } else {
        failures.push("no animal in the live zoo".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFUNIT_SET_MOVING_REAL_UNIT_LIVE`: `BFUnit::setMoving` against vanilla for every combination of the request,
/// force and the finished flag, including a request byte other than 0 or 1.
pub(crate) fn run_bfunit_set_moving_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFUNIT_SET_MOVING_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Some(ty) = live_animal_type() {
        for moving in [0u8, 1, 7] {
            for force in [0u8, 1] {
                for finished in [0u8, 1] {
                    let label = format!("moving={moving} force={force} finished={finished}");
                    let (real, port) = movement_pair(ty, [1, 0, 9, finished, 0, 0], 0);
                    compare_movement(
                        &mut failures,
                        &label,
                        real,
                        port,
                        |unit| crate::bfunit::live_support::real::set_moving(unit, moving, force),
                        |unit| unsafe { crate::bfunit::set_moving(unit, moving, force) },
                    );
                    destroy_unit(real);
                    destroy_unit(port);
                }
            }
        }
    } else {
        failures.push("no animal in the live zoo".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFUNIT_CLEAR_GOAL_REAL_UNIT_LIVE`: the goal clear at `0x43e6c0` against vanilla, with a queued list and set
/// invalid-goal flags. The embedded cog goal's bytes must also match.
pub(crate) fn run_bfunit_clear_goal_real_unit_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFUNIT_CLEAR_GOAL_REAL_UNIT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Some(ty) = live_animal_type() {
        for nodes in [0u32, 3] {
            for flags in [[0u8, 0, 0, 0, 0, 0], [1, 1, 1, 1, 1, 1]] {
                let label = format!("nodes={nodes} flags={flags:?}");
                let (real, port) = movement_pair(ty, flags, nodes);
                compare_movement(&mut failures, &label, real, port, crate::bfunit::live_support::real::clear_goal, |unit| unsafe { crate::bfunit::clear_goal(unit) });
                // The cog goal holds a vector at `+0x4` (`begin, end, cap`) whose heap pointers differ between units:
                // compare its sizes, and every other word exactly.
                let cog = |unit: u32| {
                    let words: Vec<u32> = (0..16u32).map(|i| get_from_memory::<u32>(unit + 0x15c + i * 4)).collect();
                    let mut normalised = words.clone();
                    normalised[1] = 0;
                    normalised[2] = words[2].wrapping_sub(words[1]);
                    normalised[3] = words[3].wrapping_sub(words[1]);
                    normalised
                };
                if cog(real) != cog(port) {
                    failures.push(format!("{label}: cog goal words real={:x?} port={:x?}", cog(real), cog(port)));
                }
                if movement_flags(real)[4] != 0 || movement_flags(real)[5] != 0 {
                    failures.push(format!("{label}: vanilla left the invalid-goal flags set"));
                }
                destroy_unit(real);
                destroy_unit(port);
            }
        }
    } else {
        failures.push("no animal in the live zoo".to_string());
    }
    finish_test(test_name, failures, failure_log)
}
