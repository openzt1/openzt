//! Live check for `ztguide.rs`'s `updateGoals`: twin guides, one running the real body and one the port, over every
//! input the branches read, through recording fakes for the virtual calls. `ZTUnit::updateGoals` is recorded by the
//! keeper test's hook, the guide's cog goal answers `valid` from a value the test controls, and the habitat manager
//! global is swapped for a fake so both habitat-count branches run.

use std::sync::atomic::{AtomicU32, Ordering};

use super::{
    ztguide::{base, compare_memory, pin_rng, rng},
    ztkeeper::{goals_find_predator, goals_goal_set, goals_set_target_tile, install_goals_hook, GOALS_INTERCEPT, GOALS_LOG, GOALS_PREDATOR},
};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguide::{
        live_support::{self, real},
        GUIDE_VTABLE_RVA, VIEWING_AREA_INFORMED_OFFSET,
    },
    ztkeeper::unit_offsets,
};

const VTABLE_SLOTS: u32 = 204;
/// `GLOBAL_ZTHabitatMgr` as an RVA; the habitat pointer array's begin and end sit at `+0x1c` and `+0x20`.
const HABITAT_MGR_RVA: u32 = 0x0023_805c;
/// An entity id no live guest follows, so `releaseGuests` finds no one.
const FIXTURE_ENTITY_ID: u32 = 0x7fff_0001;

static VALID_ANSWER: AtomicU32 = AtomicU32::new(0);

extern "thiscall" fn fake_valid(_cog: u32) -> u32 {
    VALID_ANSWER.load(Ordering::SeqCst)
}

/// `ZTGUIDE_UPDATE_GOALS_LIVE`: pending flag, goal id, predator result, cog validity, habitat count (none, some, no
/// manager) and the dirty `+0x2a0`/`+0x20e` bytes. Compares the recorded call sequence and the whole guide.
pub(crate) fn run_ztguide_update_goals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUIDE_UPDATE_GOALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guide_type) = live_support::find_guide_type() else {
        failures.push("the live world holds no guide type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_goals_hook() {
        failures.push(message);
    }
    GOALS_INTERCEPT.store(1, Ordering::SeqCst);
    let shared = vec![guide_type];
    let real_vtable_address = base() + GUIDE_VTABLE_RVA;
    let mut fake_vtable: Vec<u32> = (0..VTABLE_SLOTS).map(|i| get_from_memory::<u32>(real_vtable_address + i * 4)).collect();
    fake_vtable[0x2fc / 4] = goals_find_predator as *const () as usize as u32;
    fake_vtable[0x1d8 / 4] = goals_goal_set as *const () as usize as u32;
    fake_vtable[0x2d8 / 4] = goals_set_target_tile as *const () as usize as u32;
    let (real_guide, port_guide) = (live_support::build_standalone_guide(guide_type), live_support::build_standalone_guide(guide_type));
    for guide in [real_guide, port_guide] {
        save_to_memory(guide.0, fake_vtable.as_ptr() as u32);
        save_to_memory(guide.0 + unit_offsets::ENTITY_ID, FIXTURE_ENTITY_ID);
    }
    let original_cog_vtable: u32 = get_from_memory(real_guide.0 + unit_offsets::COG_GOAL);
    let mut cog_vtable: Vec<u32> = (0..16).map(|i| get_from_memory::<u32>(original_cog_vtable + i * 4)).collect();
    cog_vtable[0] = fake_valid as *const () as usize as u32;
    for guide in [real_guide, port_guide] {
        save_to_memory(guide.0 + unit_offsets::COG_GOAL, cog_vtable.as_ptr() as u32);
    }
    let original_manager: u32 = get_from_memory(base() + HABITAT_MGR_RVA);
    let habitat_array: Vec<u32> = vec![0; 4];
    let begin = habitat_array.as_ptr() as u32;
    let mut managers: Vec<(&str, u32, Vec<u32>)> = Vec::new();
    for (label, end) in [("no habitats", begin), ("habitats", begin + 8)] {
        let mut manager = vec![0u32; 0x40];
        manager[0x1c / 4] = begin;
        manager[0x20 / 4] = end;
        managers.push((label, manager.as_ptr() as u32, manager));
    }
    managers.push(("no manager", 0, Vec::new()));

    let seed = rng();
    let mut sequences = std::collections::BTreeSet::new();
    let mut cases = 0;
    for (manager_label, manager, _keep) in &managers {
        save_to_memory(base() + HABITAT_MGR_RVA, *manager);
        for pending in [0u8, 1] {
            for goal in [0x75u32, 0x6e] {
                for predator in [0u32, 0x1234] {
                    for valid in [0u32, 1] {
                        cases += 1;
                        GOALS_PREDATOR.store(predator, Ordering::SeqCst);
                        VALID_ANSWER.store(valid, Ordering::SeqCst);
                        let mut logs = Vec::new();
                        for (pole, guide) in [real_guide, port_guide].into_iter().enumerate() {
                            save_to_memory(guide.0 + unit_offsets::PREDATOR_CHECK_PENDING, pending);
                            save_to_memory(guide.0 + unit_offsets::COG_GOAL + 0x14, goal);
                            save_to_memory(guide.0 + unit_offsets::GOAL_ID, goal);
                            save_to_memory(guide.0 + VIEWING_AREA_INFORMED_OFFSET, 1u8);
                            save_to_memory(guide.0 + unit_offsets::COG_INVALID, 1u8);
                            save_to_memory(guide.0 + unit_offsets::PREDATOR, 0u32);
                            GOALS_LOG.lock().unwrap().clear();
                            pin_rng(seed);
                            if pole == 0 {
                                real::update_goals(guide.0);
                            } else {
                                guide.update_goals();
                            }
                            logs.push(GOALS_LOG.lock().unwrap().clone());
                        }
                        let case = format!("{manager_label}, pending {pending}, goal {goal:#x}, predator {predator:#x}, valid {valid}");
                        if logs[0] != logs[1] {
                            failures.push(format!("{case}: real calls {:?}, port calls {:?}", logs[0], logs[1]));
                        }
                        sequences.insert(format!("{:?}", logs[0]));
                        compare_memory(&case, real_guide, port_guide, &shared, &mut failures);
                    }
                }
            }
        }
    }
    for (label, wanted) in [
        ("predator goal", "[4, 117, 3, 0]"),
        ("no-habitat goal", "[4, 1, 1, 0]"),
        ("guide duties goal", "[4, 118, 4, 0]"),
        ("retarget", "[5, 0, 1, 0]"),
    ] {
        if !sequences.iter().any(|sequence| sequence.contains(wanted)) {
            failures.push(format!("no case reached the {label} ({wanted}) in {cases} cases"));
        }
    }
    if !sequences.iter().any(|sequence| sequence == "[[6, 0, 0, 0]]") {
        failures.push("no case ended with only ZTUnit::updateGoals".to_string());
    }
    GOALS_LOG.lock().unwrap().clear();
    unsafe { openzt_detour::generated::ztguide::UPDATE_GOALS.hooked()(port_guide.0 as *const u32) };
    if GOALS_LOG.lock().unwrap().is_empty() {
        failures.push("the hooked address made no calls".to_string());
    }
    save_to_memory(base() + HABITAT_MGR_RVA, original_manager);
    for guide in [real_guide, port_guide] {
        save_to_memory(guide.0, real_vtable_address);
        save_to_memory(guide.0 + unit_offsets::COG_GOAL, original_cog_vtable);
    }
    pin_rng(seed);
    GOALS_INTERCEPT.store(0, Ordering::SeqCst);
    // The fixtures are leaked: `~ZTGuide` frees the follower list through helpers `generated.rs` does not expose.
    finish_test(test_name, failures, failure_log)
}
