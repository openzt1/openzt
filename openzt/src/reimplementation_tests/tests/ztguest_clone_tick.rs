//! Tick-level differential test for `ZTGuest`: clones of a live guest are advanced tick by tick, one by the real
//! vanilla `doConditionals`/`updateStatusVariables`, the other by the port, with the shared RNG pinned to the same
//! value before each tick. After every tick the RNG, the world-mutating calls each side made, and the whole guest
//! (including the contents of its pool vectors) must agree.
//!
//! Unlike the scripted `ZTGUEST_CHECKS_LIVE` cases, no collaborator is faked: the need checks, searches, goal
//! changes and virtual slots all run against the live zoo. Only what would leave the clone and change the world is
//! sandboxed: thoughts (`fGuestThought`) and `throwTrash`/`removeFromMemberLists` are logged and dropped, and a clone
//! is never added to the map or any tile list. A clone copies the source's `ZTUnit` fields the checks read and its
//! `ZTGuest` tail, deep-copying every pool vector through vanilla's allocator so the twins share no buffer with each
//! other or with the source. `updateGoals` is not ticked: vanilla's body dereferences the unit's goal object, which a
//! clone does not carry. The source guests are compared byte for byte before and after.

use std::sync::atomic::Ordering;

use openzt_detour::generated::poolalloc::ALLOCATE as POOLALLOC_ALLOCATE;

use super::{
    ztguest_checks::{install_hooks, SANDBOX, SANDBOX_LOG},
    ztguest_leaf::{compare_guests, guest_words, Lcg},
    ztguide::{base, pin_rng, rng},
};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguest::{
        guest_offsets,
        live_support::{self, checks_real as real, goals_overrides},
        ZTGuest, GUEST_SIZE, GUEST_TYPE_TAG_RVA,
    },
    ztkeeper::unit_offsets,
    ztmegatilemgr::entity_type_matches,
};

const TICKS: u32 = 120;
const MAX_SOURCES: usize = 6;
/// `ZTGuest` pool vectors: visited sites (12-byte records), carried items, saved ids, the return-building string.
const POOL_VECTORS: [u32; 4] = [guest_offsets::VISITED_SITES, guest_offsets::ITEMS, guest_offsets::SAVED_IDS, guest_offsets::RETURN_BUILDING_STRING];
/// `ZTUnit` words the checks read: position, entity type, goal id.
const UNIT_WORDS: [u32; 5] = [0x114, 0x118, 0x11c, unit_offsets::ENTITY_TYPE, unit_offsets::GOAL_ID];
/// `ZTUnit` fields below the guest tail (`+0x200..+0x260`): flags, targets and need bookkeeping.
const UNIT_TAIL: std::ops::Range<u32> = 0x200..0x260;
/// First `ZTGuest` word after the return-building string vector.
const GUEST_REST: std::ops::Range<u32> = 0x294..GUEST_SIZE;
const CLONE_ID_BASE: u32 = 0x7ff0_0000;

/// The live zoo's guest entities, found through the world's entity vector.
fn live_guests() -> Vec<u32> {
    let manager: u32 = get_from_memory(base() + 0x0023_8040);
    let (begin, end): (u32, u32) = (get_from_memory(manager + 0x80), get_from_memory(manager + 0x84));
    let mut guests = Vec::new();
    for slot in (begin..end).step_by(4) {
        let entity: u32 = get_from_memory(slot);
        if entity != 0 && unsafe { entity_type_matches(entity, GUEST_TYPE_TAG_RVA) } {
            guests.push(entity);
        }
    }
    guests
}

/// Copies one `{begin, end, cap}` pool vector into a fresh buffer from vanilla's allocator.
fn clone_vector(source: u32, target: u32) {
    let (begin, end, cap): (u32, u32, u32) = (get_from_memory(source), get_from_memory(source + 4), get_from_memory(source + 8));
    if begin == 0 || cap < begin || end < begin {
        for word in 0..3 {
            save_to_memory(target + word * 4, 0u32);
        }
        return;
    }
    let (used, total) = (end - begin, (cap - begin).max(1));
    let buffer = unsafe { POOLALLOC_ALLOCATE.original()(total) } as u32;
    for offset in 0..used {
        save_to_memory(buffer + offset, get_from_memory::<u8>(begin + offset));
    }
    save_to_memory(target, buffer);
    save_to_memory(target + 4, buffer + used);
    save_to_memory(target + 8, buffer + total);
}

/// A fixture the real constructor built, holding `source`'s checked state.
fn clone_guest(source: u32, id: u32) -> ZTGuest {
    let guest_type: u32 = get_from_memory(source + unit_offsets::ENTITY_TYPE);
    let twin = live_support::build_standalone_guest(guest_type);
    for offset in UNIT_WORDS {
        save_to_memory(twin.0 + offset, get_from_memory::<u32>(source + offset));
    }
    for offset in UNIT_TAIL.step_by(4).chain(GUEST_REST.step_by(4)) {
        save_to_memory(twin.0 + offset, get_from_memory::<u32>(source + offset));
    }
    save_to_memory(twin.0 + guest_offsets::RETURN_BUILDING_BITS, get_from_memory::<u32>(source + guest_offsets::RETURN_BUILDING_BITS));
    for vector in POOL_VECTORS {
        clone_vector(source + vector, twin.0 + vector);
    }
    save_to_memory(twin.0 + 0x124, id);
    twin
}

/// A guest with randomised needs and countdowns, for a zoo with no live guest to clone.
fn synthetic_source(guest_type: u32, seed: u32) -> u32 {
    let mut rng = Lcg(seed);
    let guest = live_support::build_standalone_guest(guest_type);
    save_to_memory(guest.0 + unit_offsets::ENTITY_TYPE, guest_type);
    save_to_memory(guest.0 + guest_offsets::HAPPINESS, (rng.next() % 200) as i32 - 100);
    for offset in [guest_offsets::HUNGER, guest_offsets::THIRST, guest_offsets::ENERGY, guest_offsets::BATHROOM] {
        save_to_memory(guest.0 + offset, (rng.next() % 100) as i32);
    }
    for slot in (0x2e8u32..=0x310).step_by(4) {
        save_to_memory(guest.0 + slot, (rng.next() % 6) as i32);
    }
    save_to_memory(guest.0 + guest_offsets::CHECK_COUNTDOWN, (rng.next() % 4) as i32);
    guest.0
}

/// Every word of the vector's contents, so two clones with different buffers can be compared.
fn vector_bytes(guest: ZTGuest, vector: u32) -> Vec<u8> {
    let (begin, end): (u32, u32) = (get_from_memory(guest.0 + vector), get_from_memory(guest.0 + vector + 4));
    if begin == 0 || end < begin || end - begin > 0x4000 {
        return Vec::new();
    }
    (0..end - begin).map(|offset| get_from_memory::<u8>(begin + offset)).collect()
}

fn compare_pair(label: &str, real_guest: ZTGuest, port_guest: ZTGuest, guest_type: u32, failures: &mut Vec<String>) {
    let before = failures.len();
    compare_guests(label, real_guest, port_guest, &[guest_type], failures);
    for vector in POOL_VECTORS {
        if vector_bytes(real_guest, vector) != vector_bytes(port_guest, vector) {
            failures.push(format!("{label}: pool vector +{vector:#x} differs"));
        }
    }
    failures.truncate(before + 8);
}

fn take_log() -> Vec<String> {
    std::mem::take(&mut *SANDBOX_LOG.lock().unwrap())
}

fn sandbox_thought(_this: u32, id: u32, subject: u32, habitat: u32) -> bool {
    if !SANDBOX.load(Ordering::SeqCst) {
        return false;
    }
    SANDBOX_LOG.lock().unwrap().push(format!("thought {id:#x} {} {}", subject != 0, habitat != 0));
    true
}

/// The object `updateGoals` finds through the guest's `+0x24c` slot: vanilla dereferences it unchecked, and a clone that
/// is not on the map has none. It is never dangerous.
static FAKE_TARGET: std::sync::OnceLock<u32> = std::sync::OnceLock::new();

extern "thiscall" fn fake_target_not_dangerous(_this: u32) -> u32 {
    0
}

extern "thiscall" fn fake_target_slot(_this: u32) -> u32 {
    *FAKE_TARGET.get().unwrap()
}

/// Points `guest`'s vtable at a copy whose `+0x24c` slot answers [`fake_target_slot`].
fn install_target_slot(guest: ZTGuest) {
    FAKE_TARGET.get_or_init(|| {
        let mut vtable = vec![0u32; 16];
        vtable[0x20 / 4] = fake_target_not_dangerous as *const () as usize as u32;
        let vtable = Box::leak(vtable.into_boxed_slice());
        let object = Box::leak(vec![0u32; 4].into_boxed_slice());
        object[0] = vtable.as_ptr() as u32;
        object.as_ptr() as u32
    });
    let real_vtable: u32 = get_from_memory(guest.0);
    let mut copy: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    copy[0x24c / 4] = fake_target_slot as *const () as usize as u32;
    save_to_memory(guest.0, Box::leak(copy.into_boxed_slice()).as_ptr() as u32);
}

/// `ZTGUEST_CLONED_TICK_LIVE`: see the module documentation.
pub(crate) fn run_ztguest_cloned_tick_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_CLONED_TICK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_hooks() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let live = live_guests();
    let mut sources: Vec<u32> = live.iter().copied().take(MAX_SOURCES).collect();
    let live_count = sources.len();
    for index in 0..(MAX_SOURCES - live_count).min(3) {
        sources.push(synthetic_source(guest_type, 0x51ed + index as u32 * 977));
    }
    let snapshots: Vec<Vec<u32>> = sources.iter().map(|source| guest_words(ZTGuest(*source))).collect();
    let saved_rng = rng();
    let saved_thought = goals_overrides::THOUGHT.lock().unwrap().replace(sandbox_thought);
    SANDBOX.store(true, Ordering::SeqCst);
    let mut ticks_with_effects = 0u32;
    let mut diverged_sources = 0;
    for (index, &source) in sources.iter().enumerate() {
        let twin_type: u32 = get_from_memory(source + unit_offsets::ENTITY_TYPE);
        let real_guest = clone_guest(source, CLONE_ID_BASE + index as u32 * 2);
        let port_guest = clone_guest(source, CLONE_ID_BASE + index as u32 * 2 + 1);
        install_target_slot(real_guest);
        install_target_slot(port_guest);
        // The twins must start identical, ids aside.
        let mut start = Vec::new();
        compare_pair("start", real_guest, port_guest, twin_type, &mut start);
        start.retain(|message| !message.contains("+0x124"));
        failures.extend(start.into_iter().map(|message| format!("source {index}: {message}")));
        for tick in 0..TICKS {
            let seed = 0x1357_9bdf ^ (index as u32 * 0x10001) ^ tick.wrapping_mul(7919);
            take_log();
            pin_rng(seed);
            real::do_conditionals(real_guest.0);
            real::update_status_variables(real_guest.0);
            live_support::goals_real::update_goals(real_guest.0);
            let (real_rng, real_log) = (rng(), take_log());
            pin_rng(seed);
            port_guest.do_conditionals();
            port_guest.update_status_variables();
            port_guest.update_goals();
            let (port_rng, port_log) = (rng(), take_log());
            ticks_with_effects += (!real_log.is_empty()) as u32;
            let before = failures.len();
            if real_rng != port_rng {
                failures.push(format!("source {index} tick {tick}: rng real {real_rng:#x}, port {port_rng:#x}"));
            }
            if real_log != port_log {
                failures.push(format!("source {index} tick {tick}: calls real {real_log:?}, port {port_log:?}"));
            }
            let mut memory = Vec::new();
            compare_pair(&format!("source {index} tick {tick}"), real_guest, port_guest, twin_type, &mut memory);
            memory.retain(|message| !message.contains("+0x124"));
            failures.extend(memory);
            if failures.len() > before {
                diverged_sources += 1;
                break;
            }
        }
    }
    SANDBOX.store(false, Ordering::SeqCst);
    *goals_overrides::THOUGHT.lock().unwrap() = saved_thought;
    pin_rng(saved_rng);
    for (source, before) in sources.iter().zip(&snapshots).take(live_count) {
        if &guest_words(ZTGuest(*source)) != before {
            failures.push(format!("live guest {source:#x} changed while its clones ran"));
        }
    }
    failures.truncate(12);
    if ticks_with_effects == 0 {
        failures.push("no tick ever logged a sandboxed call, so the checks were not exercised".to_string());
    }
    tracing::info!("{test_name}: {live_count} live and {} synthetic sources, {diverged_sources} diverged, {ticks_with_effects} ticks logged world-mutating calls", sources.len() - live_count);
    finish_test(test_name, failures, failure_log)
}
