//! Live check of `ZTBuilding::turnToRubble` (`ztbuilding/rubble.rs`): the ported method against the vanilla body.
//!
//! Everything it hands the building's removal and its pieces to is scripted and records its call: the building's
//! footprint and removal slots, the map view, `findTypes` (which still asks the real world unless a name is scripted as
//! missing), `createScenery`, the pieces' name, animation and sound, the `ZTScenery` base and the CRT `rand()`.

use std::{
    ffi::c_void,
    sync::{
        atomic::{AtomicBool, AtomicU32, Ordering},
        Mutex, Once,
    },
};

use openzt_detour::generated::{
    ztmapview::CHECK_UNDO_BUFFER,
    ztrubble::{PLAY_EXPLOSION_ANIMATION, PLAY_EXPLOSION_SOUND},
    ztscenery::TURN_TO_RUBBLE as SCENERY_TURN_TO_RUBBLE,
};

use super::{
    ztbuilding_lifecycle::{
        fake_set_removed, fake_type_id, install_stubs, ready_building, Patches, Scripted, SCENERY_CREATED, TYPE_ID,
    },
    ztbuilding_users::{building_types, log, scripted_vtable, vector, word},
    ztguide_habitats::{install_rand_hook, script_rand, set_rand_intercept},
};
use crate::{
    globals::globals,
    reimplementation_tests::harness::finish_test,
    util::save_to_memory,
    ztbuilding::live_support::{destroy_standalone_building, rubble_real},
};


const BATHROOM_TYPE_ID: u32 = 0x1f48;
const ENTITY_NAME: u32 = 0x108;
const ENTITY_POS: u32 = 0x114;
const FORCES_SCENERY_RUBBLE: u32 = 0x13a;
const FIRST_PIECE_ID: u32 = 0x7e00_0001;

static RUBBLE_SCRIPTING: AtomicBool = AtomicBool::new(false);
static FOOTPRINT: [AtomicU32; 3] = [AtomicU32::new(0), AtomicU32::new(0), AtomicU32::new(0)];
static REAL_FIND_TYPES: AtomicU32 = AtomicU32::new(0);
/// Names `findTypes` finds nothing for.
static MISSING: Mutex<Vec<&'static str>> = Mutex::new(Vec::new());

fn scripting() -> bool {
    RUBBLE_SCRIPTING.load(Ordering::SeqCst)
}

#[openzt_detour_macro::detour_mod]
mod stubs {
    use super::*;

    #[detour(CHECK_UNDO_BUFFER)]
    unsafe extern "thiscall" fn check_undo_buffer(this: *const u32, entity: *const i32) {
        if scripting() {
            log(format!("checkUndoBuffer {:#x}", entity as u32));
        } else {
            unsafe { CHECK_UNDO_BUFFER_DETOUR.call(this, entity) }
        }
    }

    #[detour(SCENERY_TURN_TO_RUBBLE)]
    unsafe extern "thiscall" fn scenery_turn_to_rubble(this: *const c_void) {
        if scripting() {
            log(format!("ZTScenery::turnToRubble {:#x}", this as u32));
        } else {
            unsafe { SCENERY_TURN_TO_RUBBLE_DETOUR.call(this) }
        }
    }

    #[detour(PLAY_EXPLOSION_ANIMATION)]
    unsafe extern "thiscall" fn explosion_animation(this: *const u32) {
        if scripting() {
            let rubble = this as u32;
            let group = word(rubble + 0x158);
            let pieces = if group == 0 { 0 } else { vector(group).len() };
            log(format!("playExplosionAnimation after {pieces} pieces of type {:#x}", word(rubble + 0x15c)));
        } else {
            unsafe { PLAY_EXPLOSION_ANIMATION_DETOUR.call(this) }
        }
    }

    #[detour(PLAY_EXPLOSION_SOUND)]
    unsafe extern "thiscall" fn explosion_sound(this: *const u32) {
        if scripting() {
            log(format!("playExplosionSound {:#x}", word(this as u32 + 0x124)));
        } else {
            unsafe { PLAY_EXPLOSION_SOUND_DETOUR.call(this) }
        }
    }
}

fn string_at(string: u32) -> String {
    (word(string)..word(string + 4)).map(|address| crate::util::get_from_memory::<u8>(address) as char).collect()
}

extern "thiscall" fn fake_footprint(_this: u32, out: *mut [u32; 3], flag: u32) -> *const [u32; 3] {
    log(format!("getFootprint {}", flag & 0xff));
    unsafe { *out = std::array::from_fn(|axis| FOOTPRINT[axis].load(Ordering::SeqCst)) };
    out
}

extern "thiscall" fn fake_remove_from_map(this: u32, keep: u32) {
    log(format!("removeFromMap {this:#x} {}", keep & 0xff));
}

extern "thiscall" fn fake_find_types(this: u32, out: *mut [u32; 3], class: u32, category: u32, name: u32) -> *const [u32; 3] {
    let wanted = string_at(name);
    log(format!("findTypes {:?} {:?} {wanted:?}", string_at(class), string_at(category)));
    if MISSING.lock().unwrap().contains(&wanted.as_str()) {
        unsafe { *out = [0; 3] };
        return out;
    }
    let real: extern "thiscall" fn(u32, *mut [u32; 3], u32, u32, u32) -> *const [u32; 3] =
        unsafe { std::mem::transmute(REAL_FIND_TYPES.load(Ordering::SeqCst)) };
    real(this, out, class, category, name)
}

/// A piece's `setName`; each piece takes the next entity id.
extern "thiscall" fn fake_piece_set_name(this: u32, name: u32) {
    save_to_memory(this + 0x124, word(this + 0x124) + 1);
    log(format!("setName {:?} on {:#x}", string_at(name), word(this + 0x124)));
}

fn fake_piece() -> u32 {
    let mut vtable = vec![0u32; 16];
    vtable[0x1c / 4] = fake_piece_set_name as *const () as u32;
    let vtable = Box::leak(vtable.into_boxed_slice()).as_ptr() as u32;
    let piece = Box::leak(vec![0u32; 0x60].into_boxed_slice()).as_ptr() as u32;
    save_to_memory(piece, vtable);
    piece
}

/// `ZTBUILDING_RUBBLE_LIVE`: `turnToRubble` over big, small and bathroom footprints, in the middle and at the edges of
/// the map, with rubble types missing, pieces refused and scenery rubble forced.
pub(crate) fn run_ztbuilding_rubble_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTBUILDING_RUBBLE_LIVE";
    let mut failures = Vec::new();
    static INSTALLED: Once = Once::new();
    let mut installed = Ok(());
    INSTALLED.call_once(|| installed = unsafe { stubs::init_detours() }.map_err(|e| format!("could not install the rubble stubs: {e:?}")));
    if let Err(e) = installed.and_then(|()| install_stubs()).and_then(|()| install_rand_hook()) {
        return finish_test(test_name, vec![e], failure_log);
    }
    let Some(building_type) = building_types().first().copied() else {
        return finish_test(test_name, vec!["no building type".to_string()], failure_log);
    };

    let building = ready_building(building_type);
    let world = globals().ztworldmgr_ptr() as u32;
    let (width, height) = (globals().ztworldmgr().map_x_size as i32, globals().ztworldmgr().map_y_size as i32);
    let own_type_id = {
        let function: extern "thiscall" fn(u32) -> u32 = unsafe { std::mem::transmute(word(word(building_type) + 0x20)) };
        function(building_type)
    };

    let mut patches = Patches::default();
    patches.set(building_type, scripted_vtable(building_type, 40, &[(0x20, fake_type_id as *const () as u32)]));
    REAL_FIND_TYPES.store(word(word(world) + 0x14), Ordering::SeqCst);
    patches.set(world, scripted_vtable(world, 40, &[(0x14, fake_find_types as *const () as u32)]));
    patches.set(
        building.0,
        scripted_vtable(
            building.0,
            78,
            &[
                (0x78, fake_remove_from_map as *const () as u32),
                (0x94, fake_footprint as *const () as u32),
                (0xa8, fake_set_removed as *const () as u32),
            ],
        ),
    );
    patches.set(building.0 + ENTITY_NAME, c"Test building".as_ptr() as u32);

    let piece = fake_piece();
    let scripted = Scripted::begin();
    RUBBLE_SCRIPTING.store(true, Ordering::SeqCst);
    set_rand_intercept(true);

    // Runs one side and reports what the pieces were left sharing.
    let run = |turn: &dyn Fn()| {
        for offset in [0x158, 0x15c] {
            save_to_memory(piece + offset, 0u32);
        }
        save_to_memory(piece + 0x124, FIRST_PIECE_ID);
        script_rand(&[0, 1, 2, 3, 7, 5, 11]);
        turn();
        let group = word(piece + 0x158);
        (group != 0, if group == 0 { Vec::new() } else { vector(group) })
    };

    let middle = [width / 2 * 64 + 32, height / 2 * 64 + 32, 0];
    let odd = [width / 2 * 64 + 5, height / 2 * 64 + 61, -21];
    let corner = [5, 7, 19];
    let far = [width * 64 - 1, height * 64 - 1, -16];
    // `(width, depth, bathroom, forced, created, missing, position)`.
    type Scenario = (i32, i32, bool, u8, bool, &'static [&'static str], [i32; 3]);
    let scenarios: [Scenario; 18] = [
        (4, 4, false, 0, true, &[], middle),
        (5, 3, false, 0, true, &[], odd),
        (2, 2, false, 0, true, &[], corner),
        (6, 4, false, 0, true, &[], far),
        (1, 1, false, 0, true, &[], middle),
        (1, 5, false, 0, true, &[], odd),
        (4, 1, false, 0, true, &[], corner),
        (3, 0, false, 0, true, &[], far),
        (0, 4, false, 0, true, &[], middle),
        (1, 1, true, 0, true, &[], odd),
        (5, 6, true, 0, true, &[], middle),
        (4, 4, false, 1, true, &[], middle),
        (4, 4, false, 0, false, &[], middle),
        (4, 4, false, 0, true, &["brubble1"], middle),
        (4, 4, false, 0, true, &["brubble2", "brubble4"], middle),
        (4, 4, false, 0, true, &["brubble2", "brubble3", "brubble4"], middle),
        (1, 4, false, 0, true, &["brubls1"], middle),
        (4, 4, true, 0, true, &["bathrubl"], middle),
    ];
    let mut pieces = 0;
    for (footprint_width, depth, bathroom, forced, created, missing, position) in scenarios {
        let label = format!("{footprint_width}x{depth} bathroom {bathroom} forced {forced} created {created} missing {missing:?} at {position:?}");
        FOOTPRINT[0].store(footprint_width as u32, Ordering::SeqCst);
        FOOTPRINT[1].store(depth as u32, Ordering::SeqCst);
        TYPE_ID.store(if bathroom { BATHROOM_TYPE_ID } else { own_type_id }, Ordering::SeqCst);
        SCENERY_CREATED.store(if created { piece } else { 0 }, Ordering::SeqCst);
        *MISSING.lock().unwrap() = missing.to_vec();
        patches.set_byte(building_type + FORCES_SCENERY_RUBBLE, forced);
        for (axis, value) in position.into_iter().enumerate() {
            save_to_memory(building.0 + ENTITY_POS + axis as u32 * 4, value);
        }

        super::ztbuilding_users::take_log();
        let real = run(&|| rubble_real::turn_to_rubble(building.0));
        let real_log = super::ztbuilding_users::take_log();
        let port = run(&|| building.turn_to_rubble());
        let port_log = super::ztbuilding_users::take_log();
        if real_log != port_log && failures.len() < 10 {
            failures.push(format!("{label} calls: real {real_log:?}, port {port_log:?}"));
        }
        if real != port && failures.len() < 10 {
            failures.push(format!("{label} group: real {real:x?}, port {port:x?}"));
        }
        pieces += port.1.len();
    }
    if pieces == 0 {
        failures.push("no scenario created a piece".to_string());
    }

    set_rand_intercept(false);
    RUBBLE_SCRIPTING.store(false, Ordering::SeqCst);
    SCENERY_CREATED.store(1, Ordering::SeqCst);
    MISSING.lock().unwrap().clear();
    drop(scripted);
    drop(patches);
    destroy_standalone_building(building);
    finish_test(test_name, failures, failure_log)
}
