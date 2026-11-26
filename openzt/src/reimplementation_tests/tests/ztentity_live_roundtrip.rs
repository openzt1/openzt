//! Save/load round trips over the live zoo's own entities: every keeper, maintenance worker, guide and guest in the
//! loaded save is saved through the real vanilla body and through the port, and the two streams must match byte for
//! byte. Each stream is then loaded into a fixture by the real body and by the port, and the two loaded fixtures are
//! saved again; the loads must agree on their result and bytes consumed, and the re-saved streams must match each
//! other.
//!
//! Saving does not mutate the entity, so this runs against real data (real pool-vector contents, real goal state)
//! that the synthetic-stream tests do not reach. Loads run into fixtures only, never into a live entity.

use super::ztguide::{pin_rng, rng};
use crate::{
    globals::get_module_base,
    reimplementation_tests::{harness::finish_test, io_redirect},
    util::get_from_memory,
    ztguest::{live_support as guest_support, ZTGuest, GUEST_TYPE_TAG_RVA},
    ztguide::{live_support as guide_support, ZTGuide, GUIDE_TYPE_TAG_RVA},
    ztkeeper::{live_support as keeper_support, ZTKeeper, KEEPER_TYPE_TAG_RVA},
    ztmaint::{live_support as maint_support, ZTMaint, MAINT_TYPE_TAG_RVA},
    ztmegatilemgr::entity_type_matches,
};

/// The newest save version, above every field gate the four `load` bodies test.
const LOAD_VERSION: u32 = 0x61;
/// `GLOBAL_ZTWorldMgr`; the entity pointer vector's begin and end are at `+0x80`/`+0x84`.
const WORLD_MGR_RVA: u32 = 0x0023_8040;

fn live_entities(tag_rva: u32) -> Vec<u32> {
    let manager: u32 = get_from_memory(get_module_base("zoo.exe") as u32 + WORLD_MGR_RVA);
    let (begin, end): (u32, u32) = (get_from_memory(manager + 0x80), get_from_memory(manager + 0x84));
    (begin..end)
        .step_by(4)
        .map(get_from_memory::<u32>)
        .filter(|entity| *entity != 0 && unsafe { entity_type_matches(*entity, tag_rva) })
        .collect()
}

fn capture(body: impl FnOnce() -> bool) -> (bool, Vec<u8>) {
    io_redirect::begin_capture();
    let result = body();
    (result, io_redirect::end_capture())
}

fn replay(bytes: &[u8], body: impl FnOnce() -> bool) -> (bool, usize, u32) {
    pin_rng(0x00c0_ffee);
    io_redirect::begin_replay(bytes.to_vec());
    let result = body();
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed, rng())
}

/// One entity class under test: how to find its entities and type, build a fixture, and run each pole.
struct Class {
    name: &'static str,
    tag_rva: u32,
    entity_type_of: fn(u32) -> u32,
    build: fn(u32) -> u32,
    real_save: fn(u32) -> bool,
    port_save: fn(u32) -> bool,
    real_load: fn(u32, u32) -> bool,
    port_load: fn(u32, u32) -> bool,
}

const ENTITY_TYPE_OFFSET: u32 = 0x128;

fn entity_type(entity: u32) -> u32 {
    get_from_memory(entity + ENTITY_TYPE_OFFSET)
}

fn classes() -> Vec<Class> {
    vec![
        Class {
            name: "keeper",
            tag_rva: KEEPER_TYPE_TAG_RVA,
            entity_type_of: entity_type,
            build: |ty| keeper_support::build_standalone_keeper(ty).0,
            real_save: |entity| keeper_support::real::save(entity, std::ptr::null()),
            port_save: |entity| ZTKeeper(entity).save(std::ptr::null()),
            real_load: |entity, version| keeper_support::real::load(entity, std::ptr::null(), version),
            port_load: |entity, version| ZTKeeper(entity).load(std::ptr::null(), version),
        },
        Class {
            name: "maint",
            tag_rva: MAINT_TYPE_TAG_RVA,
            entity_type_of: entity_type,
            build: |ty| maint_support::build_standalone_maint(ty).0,
            real_save: |entity| maint_support::real::save(entity, std::ptr::null()),
            port_save: |entity| ZTMaint(entity).save(std::ptr::null()),
            real_load: |entity, version| maint_support::real::load(entity, std::ptr::null(), version),
            port_load: |entity, version| ZTMaint(entity).load(std::ptr::null(), version),
        },
        Class {
            name: "guide",
            tag_rva: GUIDE_TYPE_TAG_RVA,
            entity_type_of: entity_type,
            build: |ty| guide_support::build_standalone_guide(ty).0,
            real_save: |entity| guide_support::real::save(entity, std::ptr::null()),
            port_save: |entity| ZTGuide(entity).save(std::ptr::null()),
            real_load: |entity, version| guide_support::real::load(entity, std::ptr::null(), version),
            port_load: |entity, version| ZTGuide(entity).load(std::ptr::null(), version),
        },
        Class {
            name: "guest",
            tag_rva: GUEST_TYPE_TAG_RVA,
            entity_type_of: entity_type,
            build: |ty| guest_support::build_standalone_guest(ty).0,
            real_save: |entity| guest_support::real::save(entity, std::ptr::null()),
            port_save: |entity| ZTGuest(entity).save(std::ptr::null()),
            real_load: |entity, version| guest_support::real::load(entity, std::ptr::null(), version),
            port_load: |entity, version| ZTGuest(entity).load(std::ptr::null(), version),
        },
    ]
}

/// `ZTENTITY_LIVE_ROUNDTRIP`: see the module documentation. A class with no live entity is reported as a failure
/// only when the zoo holds none of any class.
pub(crate) fn run_ztentity_live_roundtrip_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTENTITY_LIVE_ROUNDTRIP";
    let mut failures: Vec<String> = Vec::new();
    let saved_rng = rng();
    let mut summary = Vec::new();
    let mut total = 0usize;
    for class in classes() {
        let entities = live_entities(class.tag_rva);
        let (mut saved_ok, mut loaded_ok, mut identical_to_source) = (0usize, 0usize, 0usize);
        for &entity in &entities {
            let (real_ok, real_bytes) = capture(|| (class.real_save)(entity));
            let (port_ok, port_bytes) = capture(|| (class.port_save)(entity));
            if real_ok != port_ok || real_bytes != port_bytes {
                failures.push(format!(
                    "{} {entity:#x}: save real {real_ok}/{} bytes, port {port_ok}/{} bytes{}",
                    class.name,
                    real_bytes.len(),
                    port_bytes.len(),
                    first_difference(&real_bytes, &port_bytes)
                ));
                continue;
            }
            saved_ok += real_ok as usize;
            if !real_ok {
                continue;
            }
            let ty = (class.entity_type_of)(entity);
            let (real_fixture, port_fixture) = ((class.build)(ty), (class.build)(ty));
            let real_load = replay(&real_bytes, || (class.real_load)(real_fixture, LOAD_VERSION));
            let port_load = replay(&real_bytes, || (class.port_load)(port_fixture, LOAD_VERSION));
            if real_load != port_load {
                failures.push(format!("{} {entity:#x}: load real {real_load:?}, port {port_load:?}", class.name));
                continue;
            }
            loaded_ok += real_load.0 as usize;
            let (_, real_again) = capture(|| (class.real_save)(real_fixture));
            let (_, port_again) = capture(|| (class.port_save)(port_fixture));
            if real_again != port_again {
                failures.push(format!(
                    "{} {entity:#x}: re-saved streams differ, real {} bytes, port {} bytes{}",
                    class.name,
                    real_again.len(),
                    port_again.len(),
                    first_difference(&real_again, &port_again)
                ));
            }
            identical_to_source += (real_again == real_bytes) as usize;
        }
        total += entities.len();
        summary.push(format!("{} {} live ({saved_ok} saved, {loaded_ok} loaded, {identical_to_source} re-saved identical)", class.name, entities.len()));
        failures.truncate(12);
    }
    pin_rng(saved_rng);
    if total == 0 {
        failures.push("the live zoo holds no keeper, maintenance worker, guide or guest".to_string());
    }
    tracing::info!("{test_name}: {}", summary.join("; "));
    finish_test(test_name, failures, failure_log)
}

fn first_difference(a: &[u8], b: &[u8]) -> String {
    match a.iter().zip(b).position(|(x, y)| x != y) {
        Some(index) => format!(" (first difference at byte {index}: {:#x} vs {:#x})", a[index], b[index]),
        None => String::new(),
    }
}
