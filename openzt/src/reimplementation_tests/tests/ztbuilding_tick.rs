//! Live checks of `ZTBuilding`'s tick, animations, sounds, water and events (`ztbuilding/tick.rs`): the ported method
//! against the vanilla body on twin or reused fixtures built by the real constructor.
//!
//! The sound objects and the sound manager are scripted and record their calls, as are the `ZTScenery` update and dead
//! animation, the show a building can see and the AI listener its audience events reach. The mixer, its animations and
//! the tiles are the game's own.

use std::{
    ffi::c_void,
    mem::offset_of,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Once,
    },
};

use openzt_detour::generated::{
    bfentitytype::FIND_ANIM,
    poolalloc::ALLOCATE as POOLALLOC_ALLOCATE,
    ztscenery::{FORCE_DEAD_ANIM as SCENERY_FORCE_DEAD_ANIM, UPDATE as SCENERY_UPDATE},
};

use super::{
    ztbuilding_lifecycle::Patches,
    ztbuilding_users::{
        base, building_types, log, pin_rng, rng, scripted_vtable, take_log, vector, word, Guests, ENTITIES,
    },
};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::globals,
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztbuilding::{
        building_offsets,
        live_support::{build_standalone_building, destroy_standalone_building, intercept, lifecycle_real, tick_real},
        tick::is_animal_type,
        ZTBuilding,
    },
    ztguest::live_support::find_guest_type,
    ztmegatilemgr::entity_type_matches,
    ztunit::map_tile,
};

const ANIMAL_TAG_RVA: u32 = 0x0023_8690;
const SOUND_MGR_RVA: u32 = 0x0023_80a8;
const AMBIENT_THROTTLE_FLAG_RVA: u32 = 0x0023_94f0;
const AMBIENT_NEXT_RVA: u32 = 0x0023_94e8;
const APP_KEEP_USERS_ADDRESS: u32 = 0x0063_8589;
/// The sound pointers' `SNDSound` vtable length covered by the script.
const SOUND_SLOTS: usize = 32;

// ---- scripted collaborators ----------------------------------------------------------------------------------------

static SCRIPTING: AtomicBool = AtomicBool::new(false);
static VALID: AtomicU32 = AtomicU32::new(1);
static PLAYING: AtomicU32 = AtomicU32::new(0);
static IN_PLAY_LIST: AtomicU32 = AtomicU32::new(0);
/// What a reload through a manager leaves `VALID` as.
static VALID_AFTER_RELOAD: AtomicU32 = AtomicU32::new(1);
static SHOW_SEEN: AtomicU32 = AtomicU32::new(0);
static CAPACITY: AtomicU32 = AtomicU32::new(0);

#[openzt_detour_macro::detour_mod]
mod stubs {
    use super::*;

    #[detour(SCENERY_UPDATE)]
    unsafe extern "thiscall" fn scenery_update(this: *const c_void, elapsed: i32) {
        if SCRIPTING.load(Ordering::SeqCst) {
            log(format!("sceneryUpdate {elapsed}"));
        } else {
            unsafe { SCENERY_UPDATE_DETOUR.call(this, elapsed) }
        }
    }

    #[detour(SCENERY_FORCE_DEAD_ANIM)]
    unsafe extern "thiscall" fn scenery_force_dead_anim(this: *const c_void) {
        if SCRIPTING.load(Ordering::SeqCst) {
            log("sceneryForceDeadAnim".to_string());
        } else {
            unsafe { SCENERY_FORCE_DEAD_ANIM_DETOUR.call(this) }
        }
    }
}

fn install() -> Result<(), String> {
    super::ztbuilding_users::install()?;
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { stubs::init_detours() } {
            result = Err(format!("could not install the tick stubs: {e:?}"));
        }
    });
    result
}

/// Turns the scripted collaborators on for as long as it lives.
struct Scripted {
    rng: u32,
}

impl Scripted {
    fn begin() -> Scripted {
        take_log();
        SCRIPTING.store(true, Ordering::SeqCst);
        Scripted { rng: rng() }
    }
}

impl Drop for Scripted {
    fn drop(&mut self) {
        SCRIPTING.store(false, Ordering::SeqCst);
        pin_rng(self.rng);
        take_log();
    }
}

/// A scripted sound is `{vtable, number}`.
fn number(sound: u32) -> u32 {
    word(sound + 4)
}

extern "thiscall" fn sound_reload(this: u32, manager: u32, name: u32) {
    log(format!("sound {} reload {} {name:#x}", number(this), manager != 0));
    VALID.store(VALID_AFTER_RELOAD.load(Ordering::SeqCst), Ordering::SeqCst);
}
extern "thiscall" fn sound_valid(this: u32) -> u32 {
    log(format!("sound {} valid", number(this)));
    VALID.load(Ordering::SeqCst)
}
extern "thiscall" fn sound_play(this: u32, position: u32) {
    log(format!("sound {} play {:?}", number(this), [word(position), word(position + 4)]));
}
extern "thiscall" fn sound_move(this: u32, position: u32) {
    log(format!("sound {} move {:?}", number(this), [word(position), word(position + 4)]));
}
extern "thiscall" fn sound_play_looped(this: u32, position: u32) {
    log(format!("sound {} playLooped {:?}", number(this), [word(position), word(position + 4)]));
}
extern "thiscall" fn sound_attenuation(this: u32, value: i32) {
    log(format!("sound {} attenuation {value}", number(this)));
}
extern "thiscall" fn sound_fade(this: u32, value: i32) {
    log(format!("sound {} fade {value}", number(this)));
}
extern "thiscall" fn sound_playing(this: u32) -> u32 {
    log(format!("sound {} isPlaying", number(this)));
    PLAYING.load(Ordering::SeqCst)
}
extern "thiscall" fn sound_in_play_list(this: u32) -> u32 {
    log(format!("sound {} isInPlayList", number(this)));
    IN_PLAY_LIST.load(Ordering::SeqCst)
}
extern "thiscall" fn sound_stop(this: u32) {
    log(format!("sound {} stop", number(this)));
}
extern "thiscall" fn sound_delete(this: u32, flag: u32) {
    log(format!("sound {} delete {}", number(this), flag & 0xff));
}
extern "thiscall" fn manager_acquire(_this: u32, sound: u32, name: u32) {
    // A sound the building has just made is a real `SNDSound`, which has no number.
    let real: bool = word(sound) == base() + 0x0023_0bc0;
    log(format!("acquire real {real} {name:#x}"));
}

fn sound_vtable() -> u32 {
    let mut vtable = vec![0u32; SOUND_SLOTS];
    let entries: [(usize, *const ()); 11] = [
        (0x0, sound_delete as *const ()),
        (0x8, sound_reload as *const ()),
        (0x14, sound_valid as *const ()),
        (0x24, sound_play as *const ()),
        (0x30, sound_move as *const ()),
        (0x34, sound_play_looped as *const ()),
        (0x48, sound_attenuation as *const ()),
        (0x4c, sound_fade as *const ()),
        (0x50, sound_playing as *const ()),
        (0x54, sound_in_play_list as *const ()),
        (0x60, sound_stop as *const ()),
    ];
    for (offset, function) in entries {
        vtable[offset / 4] = function as u32;
    }
    Box::leak(vtable.into_boxed_slice()).as_ptr() as u32
}

/// A scripted sound numbered `number`.
fn fake_sound(number: u32) -> u32 {
    Box::leak(Box::new([sound_vtable(), number])).as_ptr() as u32
}

fn fake_sound_manager() -> u32 {
    let mut vtable = vec![0u32; 16];
    vtable[0x10 / 4] = manager_acquire as *const () as u32;
    let vtable = Box::leak(vtable.into_boxed_slice()).as_ptr() as u32;
    Box::leak(Box::new([vtable])).as_ptr() as u32
}

/// Deletes the building's own sounds and leaves both fields null.
fn drop_own_sounds(building: ZTBuilding) {
    for offset in [building_offsets::USE_SOUND, building_offsets::AMBIENT_SOUND] {
        let own = word(building.0 + offset);
        if own != 0 {
            let delete: extern "thiscall" fn(u32, u32) = unsafe { std::mem::transmute(word(word(own))) };
            delete(own, 1);
        }
        save_to_memory(building.0 + offset, 0u32);
    }
}

/// A fixture with its slots built, standing in the middle of the map, without sounds.
fn fixture(building_type: u32) -> ZTBuilding {
    let building = build_standalone_building(building_type);
    lifecycle_real::init_status_vars(building.0);
    drop_own_sounds(building);
    let (width, height) = (globals().ztworldmgr().map_x_size, globals().ztworldmgr().map_y_size);
    save_to_memory(building.0 + 0x114, width / 2 * 64 + 32);
    save_to_memory(building.0 + 0x118, height / 2 * 64 + 32);
    save_to_memory(building.0 + 0x11c, 0u32);
    building
}

/// Destroys a fixture whose sound fields hold scripted or no sounds.
fn destroy(building: ZTBuilding) {
    save_to_memory(building.0 + building_offsets::USE_SOUND, 0u32);
    save_to_memory(building.0 + building_offsets::AMBIENT_SOUND, 0u32);
    destroy_standalone_building(building);
}

fn compare<T: PartialEq + std::fmt::Debug>(failures: &mut Vec<String>, label: impl FnOnce() -> String, real: T, port: T) {
    if real != port && failures.len() < 30 {
        failures.push(format!("{}: real {real:x?}, port {port:x?}", label()));
    }
}

/// The type pointers whose animation called `name` exists.
fn types_with_animation(name: &[u8]) -> Vec<u32> {
    building_types()
        .into_iter()
        .filter(|&building_type| !unsafe { FIND_ANIM.hooked()(building_type as *const u32, name.as_ptr() as *const i8) }.is_null())
        .collect()
}

fn find_animation(building_type: u32, name: &[u8]) -> u32 {
    unsafe { FIND_ANIM.hooked()(building_type as *const u32, name.as_ptr() as *const i8) as u32 }
}

// ---- the mixer -----------------------------------------------------------------------------------------------------

/// The `{anim, own sound, flag}` triples of the `vector<GXMixerLink>` at `address`.
fn links(building: ZTBuilding, address: u32) -> Vec<(u32, u32, u8)> {
    let sound = word(building.0 + building_offsets::USE_SOUND);
    (word(address)..word(address + 4))
        .step_by(12)
        .map(|link| {
            let held = word(link + 4);
            let held = if held != 0 && held == sound { 1 } else { held };
            (word(link), held, get_from_memory::<u8>(link + 8))
        })
        .collect()
}

/// The queued chains, walked from the queue's `start` iterator to its `finish` one.
fn queue(building: ZTBuilding) -> Vec<Vec<(u32, u32, u8)>> {
    let start = building.0 + 0x8c;
    let end = word(building.0 + 0x9c);
    let (mut cur, mut last, mut node) = (word(start), word(start + 8), word(start + 12));
    let mut chains = Vec::new();
    while cur != end && chains.len() < 1000 {
        chains.push(links(building, cur));
        cur += 0xc;
        if cur == last {
            node += 4;
            cur = word(node);
            last = cur + 0x1f8;
        }
    }
    chains
}

/// What an animation method leaves in the mixer: both link arrays, the queue, the playback words and the animation's
/// reference count change.
type MixerState = (Vec<(u32, u32, u8)>, Vec<(u32, u32, u8)>, Vec<Vec<(u32, u32, u8)>>, Vec<u32>, i32);

fn mixer_state(building: ZTBuilding, anim: u32, references: i32) -> MixerState {
    let words = (0x4c..0x68).step_by(4).map(|offset| word(building.0 + 0x74 + offset)).collect();
    let now = if anim == 0 { 0 } else { get_from_memory::<i32>(anim + 0x20) };
    (links(building, building.0 + 0x74), links(building, building.0 + 0x80), queue(building), words, now.wrapping_sub(references))
}

fn references(anim: u32) -> i32 {
    if anim == 0 {
        0
    } else {
        get_from_memory(anim + 0x20)
    }
}

// ---- tests ---------------------------------------------------------------------------------------------------------

/// `ZTBUILDING_TIMER_LIVE`: the random use timer, its update, and the behaviour set name and `isAnimalType`.
pub(crate) fn run_ztbuilding_timer_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_TIMER_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let _scripted = Scripted::begin();
    let ai = globals().ztaimgr_ptr() as u32;
    let types = building_types();
    if ai == 0 || types.is_empty() {
        return finish_test(test_name, vec!["no ZTAIMgr or no building type".to_string()], failure_log);
    }
    let clock = word(ai + 0xec);
    let (mut drawn, mut played, mut named) = (0, 0, 0);
    for &building_type in &types {
        let building = fixture(building_type);
        let this = building.0;
        for upgrade in [0u32, 1, 12, 0xffff_ffff] {
            save_to_memory(this + building_offsets::UPGRADE_INDEX, upgrade);
            let (real, port) = (tick_real::return_behavior_set(this), building.return_behavior_set());
            named += u32::from(port.len() > 8);
            compare(&mut failures, || format!("type {building_type:#x} getReturnBS upgrade {upgrade}"), real, port);
        }
        save_to_memory(this + building_offsets::UPGRADE_INDEX, 0u32);

        for (enabled, shortest, longest) in [(0u8, 10u32, 20u32), (1, 100, 5000), (1, 500, 500), (1, 700, 100), (1, 0, 1)] {
            let mut patches = Patches::default();
            patches.set_byte(building_type + 0x241, enabled);
            patches.set(building_type + 0x244, shortest);
            patches.set(building_type + 0x248, longest);
            for seed in [1u32, 0x1234_5678, 0xfeed_f00d] {
                let results = [true, false].map(|real| {
                    save_to_memory(this + 0x138, 0xdead_u32);
                    pin_rng(seed);
                    if real {
                        tick_real::init_random_use_timer(this);
                    } else {
                        building.init_random_use_timer();
                    }
                    (word(this + 0x138), rng())
                });
                drawn += u32::from(results[1].0 != 0xdead);
                compare(&mut failures, || format!("type {building_type:#x} initRandomUseTimer {enabled} {shortest} {longest} {seed}"), results[0], results[1]);
            }
            // The timer is due only once the clock is past it. A due timer plays the animation, so none is queued here.
            patches.set(building_type + 0x24c, 0);
            for next in [0, clock.saturating_sub(1), clock, clock + 1, u32::MAX] {
                let results = [true, false].map(|real| {
                    save_to_memory(this + 0x138, next);
                    pin_rng(7);
                    if real {
                        tick_real::update_random_use_timer(this);
                    } else {
                        building.update_random_use_timer();
                    }
                    (word(this + 0x138), rng(), take_log())
                });
                played += u32::from(results[1].0 != next);
                compare(&mut failures, || format!("type {building_type:#x} updateRandomUseTimer {enabled} next {next}"), results[0].clone(), results[1].clone());
            }
        }
        destroy(building);
    }

    let world = globals().ztworldmgr();
    let (mut animals, mut amphibious) = (0u32, 0u32);
    for entity in (world.entity_array_start()..world.entity_array_end()).step_by(4).map(word) {
        if entity == 0 || !unsafe { entity_type_matches(entity, ANIMAL_TAG_RVA) } {
            continue;
        }
        animals += 1;
        let entity_type = word(entity + 0x128);
        let (real, port) = (tick_real::is_animal_type(entity), is_animal_type(entity));
        amphibious += u32::from(port);
        compare(&mut failures, || format!("isAnimalType {entity_type:#x}"), real, port);
        if animals >= 64 {
            break;
        }
    }
    let building = build_standalone_building(types[0]);
    if is_animal_type(building.0) {
        failures.push("isAnimalType of a building".to_string());
    }
    destroy_standalone_building(building);

    tracing::info!("{test_name}: {drawn} timers drawn, {played} played, {named} names, {animals} animals ({amphibious} amphibious)");
    if drawn == 0 || played == 0 || named == 0 {
        failures.push(format!("a branch was never exercised (drawn {drawn}, played {played}, named {named})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTBUILDING_ANIM_LIVE`: the use and idle animations set on the mixer, and the use animation queued across several
/// queue blocks, with and without a use sound.
pub(crate) fn run_ztbuilding_anim_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_ANIM_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let _scripted = Scripted::begin();
    let used = types_with_animation(b"used\0");
    if used.is_empty() {
        return finish_test(test_name, vec!["no building type with a used animation".to_string()], failure_log);
    }
    let (mut based, mut queued, mut sounded) = (0u32, 0usize, 0u32);
    let mut manager = Patches::default();
    manager.set(base() + SOUND_MGR_RVA, fake_sound_manager());

    for &building_type in used.iter().take(12) {
        let used_anim = find_animation(building_type, b"used\0");
        let idle_anim = find_animation(building_type, b"idle\0");
        for occupancy in [0u32, 1, 2] {
            for (name, anim) in [("use", used_anim), ("idle", idle_anim)] {
                let results = [true, false].map(|real| {
                    let building = fixture(building_type);
                    save_to_memory(building.0 + building_offsets::OCCUPANCY, occupancy);
                    let before = references(anim);
                    match (name, real) {
                        ("use", true) => tick_real::might_start_use_animation(building.0),
                        ("use", false) => building.might_start_use_animation(),
                        (_, true) => tick_real::might_start_idle_animation(building.0),
                        (_, false) => building.might_start_idle_animation(),
                    }
                    let state = mixer_state(building, anim, before);
                    destroy(building);
                    state
                });
                based += u32::from(results[1].4 != 0);
                compare(&mut failures, || format!("type {building_type:#x} {name} animation, occupancy {occupancy}"), results[0].clone(), results[1].clone());
            }
        }

        // (count, sound field, valid before, valid after a reload)
        for (count, sound, valid, after) in
            [(0u32, false, 0, 0), (1, false, 0, 0), (3, true, 1, 1), (2, true, 0, 1), (2, true, 0, 0), (100, true, 1, 1), (90, false, 0, 0)]
        {
            let mut patches = Patches::default();
            patches.set(building_type + 0x24c, count);
            patches.set(building_type + 0x1ec, 0xffff_ff9c);
            let results = [true, false].map(|real| {
                let building = fixture(building_type);
                if sound {
                    save_to_memory(building.0 + building_offsets::USE_SOUND, fake_sound(1));
                }
                VALID.store(valid, Ordering::SeqCst);
                VALID_AFTER_RELOAD.store(after, Ordering::SeqCst);
                take_log();
                let before = references(used_anim);
                // Twice, so the second run starts part way into a block.
                for _ in 0..2 {
                    if real {
                        tick_real::play_use_animation(building.0);
                    } else {
                        building.play_use_animation();
                    }
                }
                let state = mixer_state(building, used_anim, before);
                let calls = take_log();
                // The destructor empties the queue the port or vanilla filled.
                let own = word(building.0 + building_offsets::USE_SOUND);
                destroy(building);
                let released = references(used_anim) - before;
                (state, calls, released, own != 0)
            });
            queued = queued.max(results[1].0 .2.len());
            sounded += u32::from(results[1].0 .2.iter().flatten().any(|link| link.1 == 1));
            compare(&mut failures, || format!("type {building_type:#x} playUseAnimation count {count} sound {sound} {valid}/{after}"), results[0].clone(), results[1].clone());
        }
    }
    drop(manager);
    tracing::info!("{test_name}: {based} base animations set, up to {queued} chains queued, {sounded} with sound");
    if based == 0 || queued <= 84 || sounded == 0 {
        failures.push(format!("a branch was never exercised (based {based}, queued {queued}, sounded {sounded})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// The sound fields and what a sound method did to them: both volumes, which fields hold a sound, whether the ambient
/// throttle moved on.
fn sound_state(building: ZTBuilding) -> (u32, u32, bool, bool, bool) {
    let next = word(base() + AMBIENT_NEXT_RVA);
    (
        word(building.0 + building_offsets::USE_SOUND_VOLUME),
        word(building.0 + building_offsets::AMBIENT_SOUND_VOLUME),
        word(building.0 + building_offsets::USE_SOUND) != 0,
        word(building.0 + building_offsets::AMBIENT_SOUND) != 0,
        next != 0 && next != u32::MAX,
    )
}

/// One sound scenario.
#[derive(Clone, Copy, Debug)]
struct Sounds {
    named: bool,
    loops: u8,
    occupancy: u32,
    field: bool,
    playing: u32,
    in_play_list: u32,
    valid: u32,
    manager: bool,
    attenuation: i32,
    throttled: bool,
}

const PLAIN: Sounds =
    Sounds { named: true, loops: 0, occupancy: 1, field: true, playing: 0, in_play_list: 0, valid: 1, manager: true, attenuation: -1_000_000, throttled: false };

const SOUND_SCENARIOS: [Sounds; 13] = [
    PLAIN,
    Sounds { named: false, ..PLAIN },
    Sounds { loops: 1, ..PLAIN },
    Sounds { loops: 1, occupancy: 0, ..PLAIN },
    Sounds { occupancy: 0, ..PLAIN },
    Sounds { field: false, ..PLAIN },
    Sounds { playing: 1, ..PLAIN },
    Sounds { in_play_list: 1, ..PLAIN },
    Sounds { valid: 0, ..PLAIN },
    Sounds { manager: false, ..PLAIN },
    Sounds { attenuation: 1_000_000, ..PLAIN },
    Sounds { throttled: true, ..PLAIN },
    Sounds { playing: 1, attenuation: 1_000_000, ..PLAIN },
];

/// `ZTBUILDING_SOUND_LIVE`: starting the use and ambient sounds and stopping the ambient one, through scripted sounds
/// and a scripted sound manager.
pub(crate) fn run_ztbuilding_sound_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_SOUND_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let _scripted = Scripted::begin();
    let Some(&building_type) = building_types().first() else {
        return finish_test(test_name, vec!["no building type".to_string()], failure_log);
    };
    let (mut started, mut looped, mut stopped) = (0u32, 0u32, 0u32);
    let mut throttle = Patches::default();
    throttle.set(base() + AMBIENT_NEXT_RVA, 0);
    throttle.set_byte(base() + AMBIENT_THROTTLE_FLAG_RVA, 1);
    let manager = fake_sound_manager();

    for scenario in SOUND_SCENARIOS {
        let mut patches = Patches::default();
        let name: &'static [u8] = if scenario.named { b"a.wav" } else { b"" };
        patches.set_string(building_type + 0x1dc, name);
        patches.set_string(building_type + 0x1f0, name);
        patches.set_byte(building_type + 0x1e8, scenario.loops);
        patches.set(building_type + 0x1ec, scenario.attenuation as u32);
        patches.set(building_type + 0x1fc, scenario.attenuation as u32);
        patches.set(base() + SOUND_MGR_RVA, if scenario.manager { manager } else { 0 });
        for method in ["use", "ambient", "stopAmbient"] {
            let results = [true, false].map(|real| {
                let building = fixture(building_type);
                save_to_memory(building.0 + building_offsets::OCCUPANCY, scenario.occupancy);
                save_to_memory(building.0 + building_offsets::USE_SOUND_VOLUME, 77u32);
                save_to_memory(building.0 + building_offsets::AMBIENT_SOUND_VOLUME, 88u32);
                if scenario.field {
                    save_to_memory(building.0 + building_offsets::USE_SOUND, fake_sound(1));
                    save_to_memory(building.0 + building_offsets::AMBIENT_SOUND, fake_sound(2));
                }
                PLAYING.store(scenario.playing, Ordering::SeqCst);
                IN_PLAY_LIST.store(scenario.in_play_list, Ordering::SeqCst);
                VALID.store(scenario.valid, Ordering::SeqCst);
                save_to_memory(base() + AMBIENT_NEXT_RVA, if scenario.throttled { u32::MAX } else { 0 });
                take_log();
                match (method, real) {
                    ("use", true) => tick_real::might_start_use_sound(building.0),
                    ("use", false) => building.might_start_use_sound(),
                    ("ambient", true) => tick_real::might_start_ambient_sound(building.0),
                    ("ambient", false) => building.might_start_ambient_sound(),
                    (_, true) => tick_real::might_stop_ambient_sound(building.0),
                    (_, false) => building.might_stop_ambient_sound(),
                }
                let outcome = (sound_state(building), take_log());
                // Sounds the method made are real ones, for the destructor; the scripted ones are not.
                if scenario.field {
                    destroy(building);
                } else {
                    destroy_standalone_building(building);
                }
                outcome
            });
            let calls = &results[1].1;
            started += u32::from(calls.iter().any(|call| call.contains(" play ")));
            looped += u32::from(calls.iter().any(|call| call.contains("playLooped")));
            stopped += u32::from(calls.iter().any(|call| call.ends_with(" stop")));
            compare(&mut failures, || format!("{method} {scenario:?}"), results[0].clone(), results[1].clone());
        }
    }

    tracing::info!("{test_name}: {started} played, {looped} looped, {stopped} stopped");
    if started == 0 || looped == 0 || stopped == 0 {
        failures.push(format!("a branch was never exercised (played {started}, looped {looped}, stopped {stopped})"));
    }
    finish_test(test_name, failures, failure_log)
}

fn scripted_capacity(_building: u32) -> Option<i32> {
    SCRIPTING.load(Ordering::SeqCst).then(|| {
        log("calculateWaterCapacity".to_string());
        CAPACITY.load(Ordering::SeqCst) as i32
    })
}

/// `ZTBUILDING_UPDATE_LIVE`: the sound fades, the ambient restart and the water capacity of the per-frame update, over
/// a scripted `ZTScenery::update`.
pub(crate) fn run_ztbuilding_update_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_UPDATE_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let _scripted = Scripted::begin();
    let Some(building_type) = building_types().into_iter().find(|&building_type| {
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        word(records) != word(records + 4)
    }) else {
        return finish_test(test_name, vec!["no building type with slots".to_string()], failure_log);
    };
    let previous_hook = intercept::CALCULATE_WATER_CAPACITY.lock().unwrap().replace(scripted_capacity);
    let manager = fake_sound_manager();
    let mut patches = Patches::default();
    patches.set(base() + SOUND_MGR_RVA, manager);
    patches.set(base() + AMBIENT_NEXT_RVA, 0);
    patches.set_byte(base() + AMBIENT_THROTTLE_FLAG_RVA, 1);
    patches.set_string(building_type + 0x1f0, b"a.wav");
    patches.set(building_type + 0x1fc, 0);
    patches.set_byte(building_type + 0x241, 0);
    let record = word(building_type + offset_of!(ZTBuildingType, slots) as u32);
    patches.set_byte(record + offset_of!(ZTBuildingSlotType, exit_on_water) as u32, 1);
    patches.set(record + offset_of!(ZTBuildingSlotType, exit_position) as u32, 0);
    patches.set(record + offset_of!(ZTBuildingSlotType, exit_position) as u32 + 4, 0);
    patches.set(record + offset_of!(ZTBuildingSlotType, exit_position) as u32 + 8, 0);

    let (mut faded, mut stopped, mut watered) = (0u32, 0u32, 0u32);
    for loops in [0u8, 1] {
        patches.set_byte(building_type + 0x1e8, loops);
        for returns in [0u8, 1] {
            patches.set_byte(building_type + 0x240, returns);
            for (playing, occupancy, volume, ambient, elapsed, capacity, water) in [
                (1u32, 0u32, 100u32, 200u32, 10u32, 0u32, true),
                (1, 1, 100, 4999, 10, 0, true),
                (1, 0, 4990, 20, 1000, 5, true),
                (1, 3, 10, 5000, 100, 0, false),
                (0, 0, 100, 200, 10, 0, true),
                (1, 1, 5000, 0, 0, 0, true),
                (1, 1, 100, 200, u32::MAX, 0, true),
            ] {
                let results = [true, false].map(|real| {
                    let building = fixture(building_type);
                    let this = building.0;
                    let tile = map_tile((word(this + 0x114) / 64) as i32, (word(this + 0x118) / 64) as i32);
                    let mut tiles = Patches::default();
                    if tile != 0 {
                        tiles.set_byte(tile + 0x83, if water { 0x41 } else { 0x40 });
                    }
                    save_to_memory(this + building_offsets::OCCUPANCY, occupancy);
                    save_to_memory(this + building_offsets::USE_SOUND_VOLUME, volume);
                    save_to_memory(this + building_offsets::AMBIENT_SOUND_VOLUME, ambient);
                    save_to_memory(this + building_offsets::WATER_CAPACITY, capacity);
                    save_to_memory(this + building_offsets::USE_SOUND, fake_sound(1));
                    save_to_memory(this + building_offsets::AMBIENT_SOUND, fake_sound(2));
                    PLAYING.store(playing, Ordering::SeqCst);
                    IN_PLAY_LIST.store(0, Ordering::SeqCst);
                    VALID.store(1, Ordering::SeqCst);
                    CAPACITY.store(7, Ordering::SeqCst);
                    save_to_memory(base() + AMBIENT_NEXT_RVA, 0u32);
                    pin_rng(11);
                    take_log();
                    if real {
                        tick_real::update(this, elapsed);
                    } else {
                        building.update(elapsed);
                    }
                    let outcome = (sound_state(building), word(this + building_offsets::WATER_CAPACITY), rng(), take_log());
                    drop(tiles);
                    destroy(building);
                    outcome
                });
                faded += u32::from(results[1].3.iter().any(|call| call.contains("fade")));
                stopped += u32::from(results[1].3.iter().any(|call| call.ends_with(" stop")));
                watered += u32::from(results[1].3.iter().any(|call| call == "calculateWaterCapacity"));
                compare(
                    &mut failures,
                    || format!("update loops {loops} returns {returns} playing {playing} occupancy {occupancy} volume {volume} ambient {ambient} {elapsed} ms capacity {capacity}"),
                    results[0].clone(),
                    results[1].clone(),
                );
            }
        }
    }

    *intercept::CALCULATE_WATER_CAPACITY.lock().unwrap() = previous_hook;
    tracing::info!("{test_name}: {faded} faded, {stopped} stopped, {watered} capacities recounted");
    if faded == 0 || stopped == 0 || watered == 0 {
        failures.push(format!("a branch was never exercised (faded {faded}, stopped {stopped}, watered {watered})"));
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTBUILDING_WATER_LIVE`: the water tiles, the water capacity and the return moves over random patches of water,
/// fences and occupied tiles around a fixture whose exit is on the map.
pub(crate) fn run_ztbuilding_water_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_WATER_LIVE";
    let mut failures = Vec::new();
    // No scripting: the water capacity must run for real.
    let Some(building_type) = building_types().into_iter().find(|&building_type| {
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        word(records) != word(records + 4)
    }) else {
        return finish_test(test_name, vec!["no building type with slots".to_string()], failure_log);
    };
    let mut patches = Patches::default();
    let record = word(building_type + offset_of!(ZTBuildingType, slots) as u32);
    for axis in 0..3 {
        patches.set(record + offset_of!(ZTBuildingSlotType, exit_position) as u32 + axis * 4, 0);
    }
    let building = fixture(building_type);
    let this = building.0;
    let (centre_x, centre_y) = ((word(this + 0x114) / 64) as i32, (word(this + 0x118) / 64) as i32);
    const SIDE: i32 = 7;
    let (mut found, mut dry, mut most) = (0u32, 0u32, 0i32);
    let mut seed = 0x2545_f491_u32;
    let mut draw = move |limit: u32| {
        seed = seed.wrapping_mul(0x343fd).wrapping_add(0x269ec3);
        (seed >> 16) % limit
    };

    for round in 0..240u32 {
        let mut tiles = Patches::default();
        // Wet, in the zoo, unoccupied and unfenced, then spoiled at random.
        let wetness = [100, 85, 60][(round % 3) as usize];
        for x in centre_x - SIDE..=centre_x + SIDE {
            for y in centre_y - SIDE..=centre_y + SIDE {
                let tile = map_tile(x, y);
                if tile == 0 {
                    continue;
                }
                let edge = (x - centre_x).abs() == SIDE || (y - centre_y).abs() == SIDE;
                let wet = !edge && draw(100) < wetness;
                let in_zoo = draw(100) < 93;
                let flags = get_from_memory::<u8>(tile + 0x83) & !0x43;
                tiles.set_byte(tile + 0x83, flags | u8::from(wet) | if in_zoo { 0x40 } else { 0 });
                for quarter in 0..4 {
                    tiles.set(tile + 4 + quarter * 4, u32::from(draw(100) < 2));
                }
                for side in 0..4 {
                    tiles.set(tile + 0x14 + side * 4, u32::from(draw(100) < 6));
                }
            }
        }
        let rotation = [0u32, 2, 4, 6][draw(4) as usize];
        save_to_memory(this + 0x12c, rotation);
        let exit_on_water = u8::from(round % 17 != 0);
        tiles.set_byte(record + offset_of!(ZTBuildingSlotType, exit_on_water) as u32, exit_on_water);
        let start = map_tile(centre_x, centre_y);
        if round % 4 != 0 {
            // A clean start, so most rounds search.
            tiles.set_byte(start + 0x83, 0x41);
            for quarter in 0..4 {
                tiles.set(start + 4 + quarter * 4, 0);
            }
        }
        // Every few rounds the building stands on the tile beside the exit, which then must not be fenced off.
        let (dx, dy) = [(0, -1), (1, 0), (0, 1), (-1, 0)][(rotation / 2) as usize];
        let beside = map_tile(centre_x + dx, centre_y + dy);
        if round % 5 == 0 && beside != 0 {
            tiles.set(beside + 0x10, this);
        }
        let strict = [0, 0, 0, 5, -1][draw(5) as usize];
        tiles.set(building_type + offset_of!(ZTBuildingType, strict_building_capacity) as u32, strict as u32);

        let real = (tick_real::relevant_water_tiles(this), tick_real::num_return_moves(this), tick_real::calculate_water_capacity(this));
        let port = (building.relevant_water_tiles(), building.num_return_moves(), building.calculate_water_capacity());
        compare(&mut failures, || format!("round {round} rotation {rotation} strict {strict}"), real, port);
        found += u32::from(port.0 > 0);
        dry += u32::from(port.0 == 0);
        most = most.max(port.0);
    }
    destroy(building);
    tracing::info!("{test_name}: {found} searches, {dry} dry, up to {most} tiles");
    if found < 20 || dry < 20 || most < 30 {
        failures.push(format!("the water search was too one-sided ({found} found, {dry} dry, most {most})"));
    }
    finish_test(test_name, failures, failure_log)
}

// ---- events --------------------------------------------------------------------------------------------------------

/// The `{value, half, kind}` events the scripted `getEvents` hands out.
static EVENTS: std::sync::Mutex<Vec<(u32, u16, u16)>> = std::sync::Mutex::new(Vec::new());

/// Fills the caller's `vector<BFEvent>` from [`EVENTS`], in vanilla memory.
extern "thiscall" fn fake_get_events(_this: u32, vector: u32) {
    let events = EVENTS.lock().unwrap();
    if events.is_empty() {
        return;
    }
    let bytes = events.len() as u32 * 0x1c;
    let buffer = unsafe { POOLALLOC_ALLOCATE.original()(bytes) } as u32;
    unsafe { std::ptr::write_bytes(buffer as *mut u8, 0, bytes as usize) };
    for (index, &(value, half, kind)) in events.iter().enumerate() {
        let event = buffer + index as u32 * 0x1c;
        save_to_memory(event + 0x10, value);
        save_to_memory(event + 0x14, half);
        save_to_memory(event + 0x16, kind);
    }
    for (index, word) in [buffer, buffer + bytes, buffer + bytes].into_iter().enumerate() {
        save_to_memory(vector + index as u32 * 4, word);
    }
}

#[allow(clippy::too_many_arguments)]
extern "thiscall" fn fake_listener(this: u32, a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32) {
    log(format!("sendEvent {:?}", [this, a & 0xffff, b, c & 0xff, d, e & 0xff, f, g & 0xffff, h & 0xffff]));
}

fn scripted_show(_building: u32) -> Option<u32> {
    SCRIPTING.load(Ordering::SeqCst).then(|| SHOW_SEEN.load(Ordering::SeqCst))
}

fn scripted_remove_user(building: u32, unit: u32, eject: bool) -> Option<bool> {
    if !SCRIPTING.load(Ordering::SeqCst) {
        return None;
    }
    log(format!("removeUser {unit:#x} {eject}"));
    let end = building + building_offsets::USERS + 4;
    save_to_memory(end, word(end) - 8);
    Some(true)
}

/// `ZTBUILDING_SHOW_EVENTS_LIVE`: what `listen` does with each event kind, with and without a visible show and with
/// users in the drawn slots, and `forceDeadAnim`.
pub(crate) fn run_ztbuilding_show_events_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_SHOW_EVENTS_LIVE";
    let mut failures = Vec::new();
    if let Err(e) = install() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let ai = globals().ztaimgr_ptr() as u32;
    let Some(guest_type) = find_guest_type() else {
        return finish_test(test_name, vec!["no guest type".to_string()], failure_log);
    };
    // A type with slots, whose first record is made to draw its users.
    let with_slots: Vec<u32> = building_types()
        .into_iter()
        .filter(|&building_type| {
            let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
            word(records) != word(records + 4)
        })
        .collect();
    let Some(&building_type) = with_slots.first() else {
        return finish_test(test_name, vec!["no building type with slots".to_string()], failure_log);
    };
    if ai == 0 {
        return finish_test(test_name, vec!["no ZTAIMgr".to_string()], failure_log);
    }
    let guests = Guests::new(guest_type);
    let _scripted = Scripted::begin();
    let previous_show = intercept::SEEN_SHOW.lock().unwrap().replace(scripted_show);
    let previous_remove = intercept::REMOVE_USER.lock().unwrap().replace(scripted_remove_user);

    let record = word(building_type + offset_of!(ZTBuildingType, slots) as u32);
    let mut patches = Patches::default();
    patches.set_byte(record + offset_of!(ZTBuildingSlotType, has_draw_slots) as u32, 1);
    patches.set(record + offset_of!(ZTBuildingSlotType, capacity) as u32, 4);
    // Four seats in the first draw order, over four draw users. Vanilla reads past the draw users for a longer one.
    let seats: &'static [u32; 5] = Box::leak(Box::new([0, 1, 2, 3, 4]));
    patches.set(record + offset_of!(ZTBuildingSlotType, draw_orders) as u32, seats.as_ptr() as u32);
    patches.set(record + offset_of!(ZTBuildingSlotType, draw_orders) as u32 + 4, seats.as_ptr() as u32 + 16);
    let listener = ai + 0x8;
    let mut listener_vtable: Vec<u32> = (0..32).map(|slot| word(word(listener) + slot * 4)).collect();
    listener_vtable[1] = fake_listener as *const () as u32;
    let listener_vtable = Box::leak(listener_vtable.into_boxed_slice()).as_ptr() as u32;
    patches.set(listener, listener_vtable);

    let building = fixture(building_type);
    let this = building.0;
    let real_vtable = word(this);
    save_to_memory(this, scripted_vtable(this, 78, &[(0x8, fake_get_events as *const () as u32)]));
    save_to_memory(this + 0x124, 0x4242u32);
    let slot = word(this + building_offsets::SLOTS);
    let draws = word(slot + 0x18);
    let draw_count = (word(slot + 0x1c) - draws) / 4;
    for index in 0..draw_count {
        let guest = guests.guests.get(index as usize).copied().unwrap_or(0);
        save_to_memory(draws + index * 4, if index == 1 { 0 } else { guest });
    }

    let kinds = [0x2710u16, 0x2711, 0x2712, 0x2715, 0x2716, 0x2717, 0x2719, 0x2721, 0x272a, 0x272b, 0x272c, 0x272d, 0x272e, 0xffff];
    let (mut sent, mut changed) = (0u32, 0u32);
    for (own_show, seen) in [(0u16, 0u32), (3, 0), (3, 3), (3, 4), (0, 3)] {
        save_to_memory(this + building_offsets::SHOW_ID, own_show);
        SHOW_SEEN.store(seen, Ordering::SeqCst);
        let batches: Vec<Vec<(u32, u16, u16)>> = kinds
            .iter()
            .map(|&kind| vec![(0x5555_0000 | u32::from(kind), 9, kind)])
            .chain([Vec::new(), vec![(1, 2, 0x2711), (3, 4, 0x2715), (5, 6, 0x272d)], (0..6).map(|index| (index, 1, 0x2719)).collect()])
            .collect();
        for batch in batches {
            *EVENTS.lock().unwrap() = batch.clone();
            let results = [true, false].map(|real| {
                building.set_type_state(7);
                pin_rng(0x99);
                take_log();
                if real {
                    tick_real::listen(this);
                } else {
                    building.listen();
                }
                (building.type_state(), rng(), take_log(), vector(slot + 0x18))
            });
            sent += u32::from(results[1].2.iter().any(|call| call.starts_with("sendEvent")));
            changed += u32::from(results[1].0 != 7);
            compare(&mut failures, || format!("listen show {own_show} seen {seen} events {batch:x?}"), results[0].clone(), results[1].clone());
        }
    }
    // The port only: a seat past the draw users has nobody in it.
    patches.set(record + offset_of!(ZTBuildingSlotType, draw_orders) as u32 + 4, seats.as_ptr() as u32 + 20);
    save_to_memory(this + building_offsets::SHOW_ID, 3u16);
    SHOW_SEEN.store(3, Ordering::SeqCst);
    *EVENTS.lock().unwrap() = vec![(1, 2, 0x272a)];
    take_log();
    building.listen();
    let extra = take_log().iter().filter(|call| call.starts_with("sendEvent")).count();
    let seated = (0..draw_count).filter(|index| word(draws + index * 4) != 0).count();
    // Only the first record draws its users in this fixture, unless the type's others already do.
    if extra < seated {
        failures.push(format!("five seats over {draw_count} draw users: {extra} events for {seated} users"));
    }
    EVENTS.lock().unwrap().clear();
    save_to_memory(this, real_vtable);
    for index in 0..draw_count {
        save_to_memory(draws + index * 4, 0u32);
    }

    // `forceDeadAnim` ejects the users unless the app byte says to keep them.
    let mut ejected = 0u32;
    for keep in [0u8, 1] {
        let mut app = Patches::default();
        app.set_byte(APP_KEEP_USERS_ADDRESS, keep);
        let results = [true, false].map(|real| {
            let users = [word(guests.guests[0] + 0x124), 0, word(guests.guests[1] + 0x124), 1];
            let buffer = users.to_vec().leak();
            let begin = buffer.as_ptr() as u32;
            for (index, word) in [begin, begin + 16, begin + 16].into_iter().enumerate() {
                save_to_memory(this + building_offsets::USERS + index as u32 * 4, word);
            }
            take_log();
            if real {
                tick_real::force_dead_anim(this);
            } else {
                building.force_dead_anim();
            }
            let left = (word(this + building_offsets::USERS + 4) - begin) / 8;
            // The buffer is Rust's, so the destructor must not see it.
            for index in 0..3 {
                save_to_memory(this + building_offsets::USERS + index * 4, 0u32);
            }
            (left, take_log())
        });
        ejected += u32::from(results[1].1.iter().any(|call| call.starts_with("removeUser")));
        compare(&mut failures, || format!("forceDeadAnim keep {keep}"), results[0].clone(), results[1].clone());
    }

    destroy(building);
    *intercept::SEEN_SHOW.lock().unwrap() = previous_show;
    *intercept::REMOVE_USER.lock().unwrap() = previous_remove;
    ENTITIES.lock().unwrap().clear();
    guests.destroy();
    tracing::info!("{test_name}: {sent} audiences, {changed} state changes, {ejected} ejections");
    if sent == 0 || changed == 0 || ejected != 1 {
        failures.push(format!("a branch was never exercised (sent {sent}, changed {changed}, ejected {ejected})"));
    }
    finish_test(test_name, failures, failure_log)
}
