//! Compares the reimplemented `ZTHabitat` event leaves (`sendEvent`, `getEvents`), `playShowStartSound`,
//! `createName` and constructor against real vanilla.
//!
//! The "real" pole is always the release-safe `*_real` path through the detour's own trampoline
//! (`hooks_zthabitatmgr::*_real`), so the comparison holds in debug and release alike.
//!
//! The event leaves and the sound call only forward through vtables, so each test points that vtable at a
//! Rust-owned copy whose one interesting slot records its arguments instead of delivering anything.

use std::cell::{Cell, RefCell};

use openzt_detour::generated::{standalone::OPERATOR_NEW, zthabitatmgr::GET_ZOO_ENTRANCE_TILE};

use crate::globals::globals;
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::util::{get_from_memory, ref_from_memory, save_to_memory};
use crate::zthabitat::support::call_vtable_slot_with_u8;
use crate::zthabitatmgr::{hooks_zthabitatmgr, walk_tile_list, TileListNode, ZTHabitat};

/// `ZTHABITAT_LIFECYCLE_DETOURS_ENABLED`: the six detours this file's tests compare are installed.
/// (`ZTHABITATMGR_DETOURS_ENABLED` checks the whole module; this names the ones that would otherwise
/// silently fall through to vanilla and make a comparison pass by coincidence.)
pub(crate) fn run_habitat_lifecycle_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_LIFECYCLE_DETOURS_ENABLED";
    let status = hooks_zthabitatmgr::status();
    let mut failures = Vec::new();
    for wanted in ["SEND_EVENT", "GET_EVENTS", "PLAY_SHOW_START_SOUND", "CREATE_NAME", "CONSTRUCTOR"] {
        match status.iter().find(|(name, _)| *name == wanted) {
            None => failures.push(format!("{wanted}: no such detour in hooks_zthabitatmgr")),
            Some((_, false)) => failures.push(format!("{wanted}: detour not enabled")),
            Some((_, true)) => {}
        }
    }
    finish_test(test_name, failures, failure_log)
}

thread_local! {
    static LISTENER_CALLS: RefCell<Vec<[u32; 9]>> = const { RefCell::new(Vec::new()) };
    static GET_EVENTS_CALLS: RefCell<Vec<[u32; 4]>> = const { RefCell::new(Vec::new()) };
    static SOUND_PLAY_CALLS: RefCell<Vec<[u32; 2]>> = const { RefCell::new(Vec::new()) };
    static SOUND_PREDICATE_RESULT: Cell<u32> = const { Cell::new(0) };
}

extern "thiscall" fn listener_send_stub(this: u32, a: u32, b: u32, c: u32, d: u32, e: u32, f: u32, g: u32, h: u32) {
    LISTENER_CALLS.with(|calls| calls.borrow_mut().push([this, a, b, c, d, e, f, g, h]));
}

extern "thiscall" fn get_events_stub(this: u32, out: u32, id: u32, tag: u32) {
    GET_EVENTS_CALLS.with(|calls| calls.borrow_mut().push([this, out, id, tag]));
}

extern "thiscall" fn sound_predicate_stub(_this: u32) -> u32 {
    SOUND_PREDICATE_RESULT.with(|result| result.get())
}

extern "thiscall" fn sound_play_stub(this: u32, position: u32) {
    SOUND_PLAY_CALLS.with(|calls| calls.borrow_mut().push([this, position]));
}

/// `ZTHABITAT_SEND_EVENT_MATCHES_REAL_LIVE`: swaps the live `ZTAIMgr`'s embedded listener vtable
/// (`+0x8`) for a copy whose slot `+0x4` records the nine values (`this` plus eight stack arguments)
/// real `sendEvent` and the port each forward, then restores it. The null-`GLOBAL_ZTAIMgr` early return
/// cannot be forced without destabilising the battery and is not covered.
pub(crate) fn run_habitat_send_event_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_SEND_EVENT_MATCHES_REAL_LIVE";
    let ai_mgr = globals().ztaimgr_ptr() as u32;
    if ai_mgr == 0 {
        write_success_line(failure_log, &format!("{test_name} (skipped: no GLOBAL_ZTAIMgr)"));
        return false;
    }
    let listener = ai_mgr + 0x8;
    let real_vtable: u32 = get_from_memory(listener);
    let mut fake_vtable = [0u32; 32];
    for (slot, word) in fake_vtable.iter_mut().enumerate() {
        *word = get_from_memory(real_vtable + slot as u32 * 4);
    }
    fake_vtable[1] = listener_send_stub as *const () as usize as u32;

    let dummy_habitat = vec![0u32; 0x178 / 4];
    let habitat_ptr = dummy_habitat.as_ptr() as u32;
    let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
    let cases: [(u16, u32, u8, u32, u16, u16); 3] = [
        (0x2730, 0, 0x4d, 0x1234, 5, 1),
        (0x2716, 7, 0x53, 0, 0, 0),
        (0xffff, 0xffff_ffff, 0xff, 0xdead_beef, 0xffff, 0xffff),
    ];
    // Argument slots narrower than a dword hold whatever upper bytes the caller pushed.
    let normalise = |call: [u32; 9]| {
        [call[0], call[1] & 0xffff, call[2], call[3] & 0xff, call[4] & 0xffff, call[5], call[6], call[7] & 0xffff, call[8] & 0xffff]
    };

    save_to_memory(listener, fake_vtable.as_ptr() as u32);
    let mut results = Vec::new();
    for &(event_id, unused, category, value, value2, flag) in &cases {
        LISTENER_CALLS.with(|calls| calls.borrow_mut().clear());
        hooks_zthabitatmgr::send_event_real(habitat_ptr as *const u32, event_id, unused, category, value, value2, flag);
        let real = LISTENER_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()));
        habitat.send_event(event_id, unused, category, value, value2, flag);
        let port = LISTENER_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()));
        results.push((event_id, real, port));
    }
    save_to_memory(listener, real_vtable);

    let mut failures = Vec::new();
    for (event_id, real, port) in results {
        let real: Vec<_> = real.into_iter().map(normalise).collect();
        let port: Vec<_> = port.into_iter().map(normalise).collect();
        if real.len() != 1 || real != port {
            failures.push(format!("event {event_id:#x}: real forwarded {real:x?}, port forwarded {port:x?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTHABITAT_GET_EVENTS_MATCHES_REAL_LIVE`: a Rust-owned habitat block with a recording vtable slot
/// `+0xc`, with and without a `+4` show-info object.
pub(crate) fn run_habitat_get_events_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_GET_EVENTS_MATCHES_REAL_LIVE";
    let mut fake_vtable = [0u32; 16];
    fake_vtable[3] = get_events_stub as *const () as usize as u32;
    let mut show_info = vec![0u8; 0x80];
    show_info[0x70] = 0x34;
    show_info[0x71] = 0x12;
    let out_marker = 0xfeed_0000u32;

    let mut failures = Vec::new();
    for (label, show_info_ptr) in [("no show info", 0u32), ("show info", show_info.as_ptr() as u32)] {
        let mut block = vec![0u32; 0x178 / 4];
        block[0] = fake_vtable.as_ptr() as u32;
        block[1] = show_info_ptr;
        let habitat_ptr = block.as_ptr() as u32;

        GET_EVENTS_CALLS.with(|calls| calls.borrow_mut().clear());
        hooks_zthabitatmgr::get_events_real(habitat_ptr as *const u32, out_marker as *const u32);
        let real = GET_EVENTS_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()));
        unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) }.get_events(out_marker as *const u32);
        let port = GET_EVENTS_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()));

        let expected_id = if show_info_ptr == 0 { 0 } else { 0x1234 };
        if real != vec![[habitat_ptr, out_marker, expected_id, 0x48]] {
            failures.push(format!("{label}: real vanilla recorded {real:x?}"));
        }
        if real != port {
            failures.push(format!("{label}: real {real:x?} != port {port:x?}"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTHABITAT_PLAY_SHOW_START_SOUND_MATCHES_REAL_LIVE`: every live habitat's start sound (`+0x160`) is
/// swapped for a fake whose vtable `+0x14` predicate returns a scripted dword and whose `+0x24` records
/// its position argument, so the call is compared for each predicate result (including ones whose low
/// byte and full value disagree - vanilla tests `AL` only), then restored. Only habitats with a gate
/// reach the play call; the success line reports how many did.
pub(crate) fn run_habitat_play_show_start_sound_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_PLAY_SHOW_START_SOUND_MATCHES_REAL_LIVE";
    let habitat_mgr = globals().zthabitatmgr();
    let mut fake_vtable = [0u32; 16];
    fake_vtable[5] = sound_predicate_stub as *const () as usize as u32;
    fake_vtable[9] = sound_play_stub as *const () as usize as u32;
    let fake_sound = [fake_vtable.as_ptr() as u32, 0];
    let fake_sound_ptr = fake_sound.as_ptr() as u32;

    let mut failures = Vec::new();
    let (mut habitats, mut plays) = (0u32, 0u32);
    for i in 0..habitat_mgr.exhibit_array().len() {
        let ptr = habitat_mgr.exhibit_array().get_ptr(i);
        if ptr == 0 {
            continue;
        }
        habitats += 1;
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(ptr) };
        let original_sound: u32 = get_from_memory(ptr + 0x160);
        for (label, sound, predicate) in [
            ("no sound", 0, 1),
            ("predicate 0", fake_sound_ptr, 0),
            ("predicate low byte clear", fake_sound_ptr, 0xabcd_ef00),
            ("predicate low byte set", fake_sound_ptr, 0xabcd_ef01),
            ("predicate 1", fake_sound_ptr, 1),
        ] {
            save_to_memory(ptr + 0x160, sound);
            SOUND_PREDICATE_RESULT.with(|result| result.set(predicate));
            SOUND_PLAY_CALLS.with(|calls| calls.borrow_mut().clear());
            hooks_zthabitatmgr::play_show_start_sound_real(ptr as *const u32);
            let real = SOUND_PLAY_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()));
            habitat.play_show_start_sound();
            let port = SOUND_PLAY_CALLS.with(|calls| std::mem::take(&mut *calls.borrow_mut()));
            plays += port.len() as u32;
            if real != port {
                failures.push(format!("habitat {ptr:#010x} ({label}): real {real:x?} != port {port:x?}"));
            }
        }
        save_to_memory(ptr + 0x160, original_sound);
    }
    if failures.is_empty() {
        write_success_line(failure_log, &format!("{test_name} (habitats: {habitats}, play calls compared: {plays})"));
        false
    } else {
        finish_test(test_name, failures, failure_log)
    }
}

/// The disposable habitat fixture: a zeroed `0x178` block built by the port at `tile`.
fn build_fixture(tile: u32) -> u32 {
    let raw = unsafe { OPERATOR_NEW.original()(0x178) } as u32;
    for offset in (0..0x178).step_by(4) {
        save_to_memory(raw + offset, 0u32);
    }
    ZTHabitat::construct(raw, tile, false)
}

/// Frees a fixture through its own scalar-deleting destructor (vtable slot `+0x18`).
fn free_fixture(habitat: u32) {
    unsafe { call_vtable_slot_with_u8(habitat, 0x18, 1) };
}

fn name_bytes(habitat: u32) -> Vec<u8> {
    let (begin, end): (u32, u32) = (get_from_memory(habitat + 0x154), get_from_memory(habitat + 0x158));
    (begin..end).map(get_from_memory::<u8>).collect()
}

fn habitat_counter_addr() -> u32 {
    globals().zthabitatmgr_ptr() as u32 + std::mem::offset_of!(crate::zthabitatmgr::ZTHabitatMgr, habitat_num) as u32
}

/// `ZTHABITAT_CREATE_NAME_MATCHES_REAL_LIVE`: on a disposable habitat built at the zoo entrance tile,
/// runs real `createName` then the port - for the world habitat and for whichever non-world kind the
/// fixture's own tank/show checks select - restoring the manager's name counter before each pole and
/// overwriting the name between them, so a port that does nothing cannot pass on real's output. Compares
/// the name bytes and the counter each pole leaves behind. The tank/show prefix variants cannot be forced
/// on a fresh single-tile fixture, so the success line reports which kind was exercised.
pub(crate) fn run_habitat_create_name_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_CREATE_NAME_MATCHES_REAL_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let seed_tile = unsafe { GET_ZOO_ENTRANCE_TILE.original()(mgr_ptr) } as u32;
    if seed_tile == 0 {
        write_success_line(failure_log, &format!("{test_name} (skipped: no zoo entrance tile)"));
        return false;
    }
    let counter_addr = habitat_counter_addr();
    let fixture = build_fixture(seed_tile);
    let habitat = unsafe { ref_from_memory::<ZTHabitat>(fixture) };
    let kind = if habitat.do_tank_check() { "tank" } else { "plain" };

    let mut failures = Vec::new();
    for (label, world) in [("non-world", 0u8), ("world", 1u8)] {
        save_to_memory(fixture + 0x2c, world);
        let counter_before: u32 = get_from_memory(counter_addr);

        hooks_zthabitatmgr::create_name_real(fixture as *const u32);
        let (real_name, real_counter): (Vec<u8>, u32) = (name_bytes(fixture), get_from_memory(counter_addr));
        if habitat.zt_show_info_ptr != 0 {
            habitat.set_is_not_show_exhibit();
        }

        save_to_memory(counter_addr, counter_before);
        let (begin, end): (u32, u32) = (get_from_memory(fixture + 0x154), get_from_memory(fixture + 0x158));
        for byte_ptr in begin..end {
            save_to_memory(byte_ptr, b'#');
        }
        habitat.create_name();
        let (port_name, port_counter): (Vec<u8>, u32) = (name_bytes(fixture), get_from_memory(counter_addr));
        if habitat.zt_show_info_ptr != 0 {
            habitat.set_is_not_show_exhibit();
        }
        save_to_memory(counter_addr, counter_before);

        if real_name != port_name {
            failures.push(format!(
                "{label}: real name {:?} != port name {:?}",
                String::from_utf8_lossy(&real_name),
                String::from_utf8_lossy(&port_name)
            ));
        }
        if real_counter != port_counter {
            failures.push(format!("{label}: real left counter {real_counter}, port left {port_counter} (from {counter_before})"));
        }
        if world == 1 && port_name != b"World" {
            failures.push(format!("world habitat named {:?}, expected \"World\"", String::from_utf8_lossy(&port_name)));
        }
    }
    free_fixture(fixture);

    if failures.is_empty() {
        write_success_line(failure_log, &format!("{test_name} (non-world fixture kind: {kind})"));
        false
    } else {
        finish_test(test_name, failures, failure_log)
    }
}

/// Offsets of the dwords [`run_habitat_constructor_matches_real_live_test`] masks: the heads of the
/// containers vanilla's own constructors allocate (their sentinel addresses differ between the two builds).
const POINTER_WORDS: [u32; 10] = [0x8, 0x14, 0x20, 0x40, 0x44, 0x148, 0x154, 0x158, 0x15c, 0x16c];

struct ConstructedSnapshot {
    bytes: Vec<u8>,
    name: Vec<u8>,
    owned_tiles: Vec<u32>,
}

fn snapshot_constructed(habitat: u32) -> ConstructedSnapshot {
    let mut bytes: Vec<u8> = (0..0x178).map(|offset| get_from_memory::<u8>(habitat + offset)).collect();
    for word in POINTER_WORDS {
        bytes[word as usize..word as usize + 4].fill(0);
    }
    let owned_tiles = walk_tile_list(get_from_memory(habitat + 0x40)).map(|node| get_from_memory::<TileListNode>(node).payload).collect();
    ConstructedSnapshot { bytes, name: name_bytes(habitat), owned_tiles }
}

/// `ZTHABITAT_CONSTRUCTOR_MATCHES_REAL_LIVE`: builds a habitat at the zoo entrance tile in a zeroed
/// `0x178` block through real vanilla's constructor and through the port (one after the other, freeing
/// the first before building the second so both see the tile unclaimed), restoring the manager's name
/// counter between them, and compares every byte outside the container heads in [`POINTER_WORDS`], the
/// name text and the owned-tile list. The world flag is covered by the `createName` test; a world
/// habitat has no seed tile to build against here.
pub(crate) fn run_habitat_constructor_matches_real_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTHABITAT_CONSTRUCTOR_MATCHES_REAL_LIVE";
    let mgr_ptr = globals().zthabitatmgr_ptr() as *const u32;
    let seed_tile = unsafe { GET_ZOO_ENTRANCE_TILE.original()(mgr_ptr) } as u32;
    if seed_tile == 0 {
        write_success_line(failure_log, &format!("{test_name} (skipped: no zoo entrance tile)"));
        return false;
    }
    let counter_addr = habitat_counter_addr();
    let counter_before: u32 = get_from_memory(counter_addr);

    let build = |use_port: bool| -> (ConstructedSnapshot, u32) {
        save_to_memory(counter_addr, counter_before);
        let raw = unsafe { OPERATOR_NEW.original()(0x178) } as u32;
        for offset in (0..0x178).step_by(4) {
            save_to_memory(raw + offset, 0u32);
        }
        if use_port {
            ZTHabitat::construct(raw, seed_tile, false);
        } else {
            hooks_zthabitatmgr::habitat_constructor_real(raw as *const u32, seed_tile as *const u32, false);
        }
        let snapshot = snapshot_constructed(raw);
        let counter_after: u32 = get_from_memory(counter_addr);
        free_fixture(raw);
        (snapshot, counter_after)
    };
    let (real, real_counter) = build(false);
    let (port, port_counter) = build(true);
    save_to_memory(counter_addr, counter_before);

    let mut failures = Vec::new();
    for offset in 0..0x178usize {
        if real.bytes[offset] != port.bytes[offset] {
            failures.push(format!("byte +{offset:#x}: real {:#04x}, port {:#04x}", real.bytes[offset], port.bytes[offset]));
        }
    }
    if real.name != port.name {
        failures.push(format!("name: real {:?}, port {:?}", String::from_utf8_lossy(&real.name), String::from_utf8_lossy(&port.name)));
    }
    if real.owned_tiles != port.owned_tiles {
        failures.push(format!("owned tiles: real {:x?}, port {:x?}", real.owned_tiles, port.owned_tiles));
    }
    if real_counter != port_counter {
        failures.push(format!("name counter after: real {real_counter}, port {port_counter}"));
    }
    finish_test(test_name, failures, failure_log)
}
