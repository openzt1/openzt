//! Live checks for `ztguest.rs`'s leaf accessors and predicates. Fixtures are guests built by the real vanilla
//! constructor over a live guest type, planted with identical pseudo-random state; each method runs through the real
//! body, the port and the hooked address, and the results (and, for setters, the whole guest memory) are compared.
//! The fixtures' vtables are replaced by fakes, so the virtual predicates and the mood-change slot are scripted.

use std::{ffi::c_void, sync::Mutex};

use openzt_detour::generated::ztguest::{
    DIAGONAL_OK, DO_ANGRY_PRICE_CHANGE, DO_HAPPY_PRICE_CHANGE, DO_TOURGUIDE_BONUS, F_ANIMAL_HAPPY, F_TOUR_GUIDE_BONUS, GET_BATHROOM_PCT, GET_COLOR_REP_INFO,
    GET_ENERGY_PCT, GET_HABITAT_FROM_VA, GET_HAPPY_PCT, GET_HUNGER_PCT, GET_RANDOM_OK, GET_RETURN_BUILDING_NAME_ID, GET_RETURN_BUILDING_STRING, GET_THIRST_PCT,
    GET_VIEWING_AREA, HAS_PRESSING_NEED, IS_DOING_SOMETHING, IS_RETURNING, IS_WATER_RECOVERING, IS_WATER_TRAVELING, RANDOM_DONE, SET_RETURN_BUILDING_NAME_ID,
    SET_SPEED, SET_WATER_RECOVERING,
};

use crate::{
    globals::globals,
    reimplementation_tests::harness::{finish_test, write_success_line},
    util::{get_from_memory, save_to_memory},
    ztguest::{
        guest_offsets,
        live_support::{self, real},
        ZTGuest, GUEST_SIZE,
    },
    ztkeeper::unit_offsets,
};

/// `ZTGUEST_DETOURS_ENABLED`: every `ZTGuest` detour reports enabled.
pub(crate) fn run_ztguest_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

/// Deterministic generator for planted field values.
pub(super) struct Lcg(pub u32);

impl Lcg {
    pub(super) fn next(&mut self) -> u32 {
        self.0 = self.0.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        self.0 >> 8
    }
}

pub(super) fn guest_words(guest: ZTGuest) -> Vec<u32> {
    (0..GUEST_SIZE / 4).map(|i| get_from_memory::<u32>(guest.0 + i * 4)).collect()
}

/// Compares two guests' whole memory; words that are heap pointers (above `0x10000`) may differ unless one is in
/// `shared`.
pub(super) fn compare_guests(label: &str, real: ZTGuest, port: ZTGuest, shared: &[u32], failures: &mut Vec<String>) {
    for (i, (r, p)) in guest_words(real).into_iter().zip(guest_words(port)).enumerate() {
        let equivalent = r == p || (r >= 0x10000 && p >= 0x10000 && !shared.contains(&r) && !shared.contains(&p));
        if !equivalent {
            failures.push(format!("{label}: +{:#x} real {r:#x}, port {p:#x}", i * 4));
        }
    }
}

/// Plants pseudo-random state a fixture's predicates read: the packed return-building word, goal id, need levels and
/// flag bytes.
fn plant_state(guest: ZTGuest, rng: &mut Lcg) {
    let name_bits = if rng.next() & 1 == 0 { rng.next() & 0xf_ffff } else { 0 };
    save_to_memory(guest.0 + guest_offsets::RETURN_BUILDING_BITS, name_bits);
    // Id `-1` makes `checkViewingArea` take its no-viewing-area exit; a fixture has no habitat for it to look up.
    save_to_memory(guest.0 + guest_offsets::VIEWING_AREA_ID, -1i32);
    let goals = [0x66u32, 0x67, 0x68, 0x6a, 0x6b, 0x6c, 0x6d, 0x71, 0x75, 0x79, 0x7a, 0];
    save_to_memory(guest.0 + unit_offsets::GOAL_ID, goals[rng.next() as usize % goals.len()]);
    for offset in [guest_offsets::HAPPINESS, guest_offsets::HUNGER, guest_offsets::THIRST, guest_offsets::ENERGY, guest_offsets::BATHROOM] {
        save_to_memory(guest.0 + offset, (rng.next() % 140) as i32 - 20);
    }
    for offset in [0x25cu32, 0x24c, guest_offsets::FLAG_0X336, guest_offsets::FLAG_0X339, guest_offsets::RANDOM_DONE] {
        save_to_memory(guest.0 + offset, (rng.next() % 3 == 0) as u8);
    }
}

/// Records the six-dword record the mood-change slot (`+0x1fc`) receives.
static MOOD_LOG: Mutex<Vec<[u32; 6]>> = Mutex::new(Vec::new());

extern "thiscall" fn fake_mood_change(_this: u32, record: u32) -> u32 {
    let mut words = [0u32; 6];
    for (i, word) in words.iter_mut().enumerate() {
        *word = get_from_memory(record + i as u32 * 4);
    }
    MOOD_LOG.lock().unwrap().push(words);
    0
}

extern "thiscall" fn fake_predicate_true(_this: u32) -> u32 {
    1
}

extern "thiscall" fn fake_predicate_false(_this: u32) -> u32 {
    0
}

/// Gives `guest` a leaked fake vtable whose `+0x264`/`+0x270` predicates answer `a`/`b` and whose `+0x1fc` records the
/// mood-change record.
fn install_fake_vtable(guest: ZTGuest, a: bool, b: bool) {
    let answer = |value: bool| if value { fake_predicate_true as *const () as usize as u32 } else { fake_predicate_false as *const () as usize as u32 };
    // A copy of the guest's real vtable (197 slots), so every other virtual call still reaches vanilla.
    let real_vtable: u32 = get_from_memory(guest.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(real_vtable + slot * 4)).collect();
    vtable[0x264 / 4] = answer(a);
    vtable[0x270 / 4] = answer(b);
    vtable[0x1fc / 4] = fake_mood_change as *const () as usize as u32;
    let vtable = Box::leak(vtable.into_boxed_slice());
    save_to_memory(guest.0, vtable.as_ptr() as u32);
}

fn check(failures: &mut Vec<String>, label: &str, real: i64, port: i64, hooked: i64) {
    if real != port || real != hooked {
        failures.push(format!("{label}: real {real}, port {port}, hooked {hooked}"));
    }
}

/// `ZTGUEST_LEAF_LIVE`: the value-returning accessors and predicates agree between the real body, the port and the
/// hooked address over many planted states.
pub(crate) fn run_ztguest_leaf_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_LEAF_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut rng = Lcg(0x5eed);
    let rounds = 200u32;
    let mut varied = [0u32; 7];
    for round in 0..rounds {
        let guest = live_support::build_standalone_guest(guest_type);
        plant_state(guest, &mut rng);
        install_fake_vtable(guest, rng.next() & 1 == 0, rng.next() & 1 == 0);
        let this = guest.0 as *const u32;
        let c = guest.0 as *const c_void;
        let i = guest.0 as i32;
        let label = |name: &str| format!("round {round} {name}");
        unsafe {
            check(&mut failures, &label("isReturning"), real::is_returning(guest.0) as i64, guest.is_returning() as i64, IS_RETURNING.hooked()(this) as i64);
            check(
                &mut failures,
                &label("isWaterRecovering"),
                real::is_water_recovering(guest.0) as i64,
                guest.is_water_recovering() as i64,
                IS_WATER_RECOVERING.hooked()(this) as i64,
            );
            check(
                &mut failures,
                &label("isWaterTraveling"),
                real::is_water_traveling(guest.0) as i64,
                guest.is_water_traveling() as i64,
                IS_WATER_TRAVELING.hooked()(this) as i64,
            );
            check(
                &mut failures,
                &label("getReturnBuildingNameID"),
                real::get_return_building_name_id(guest.0) as i64,
                guest.return_building_name_id() as i64,
                GET_RETURN_BUILDING_NAME_ID.hooked()(this) as i64,
            );
            check(
                &mut failures,
                &label("getReturnBuildingString"),
                real::get_return_building_string(guest.0) as i64,
                guest.return_building_string() as i64,
                GET_RETURN_BUILDING_STRING.hooked()(this) as i64,
            );
            check(&mut failures, &label("getRandomOK"), real::get_random_ok(guest.0) as i64, guest.random_ok() as i64, GET_RANDOM_OK.hooked()(this) as i64);
            check(&mut failures, &label("getViewingArea"), real::get_viewing_area(guest.0) as i64, guest.viewing_area() as i64, GET_VIEWING_AREA.hooked()(this) as i64);
            check(
                &mut failures,
                &label("getHabitatFromVA"),
                real::get_habitat_from_va(guest.0) as i64,
                guest.habitat_from_va() as i64,
                GET_HABITAT_FROM_VA.hooked()(this) as i64,
            );
            check(&mut failures, &label("getColorRepInfo"), real::get_color_rep_info(guest.0) as i64, guest.color_rep_info() as i64, GET_COLOR_REP_INFO.hooked()(this) as i64);
            check(&mut failures, &label("getHappyPct"), real::get_happy_pct(guest.0) as i64, guest.happy_pct() as i64, GET_HAPPY_PCT.hooked()(i) as i64);
            check(&mut failures, &label("getHungerPct"), real::get_hunger_pct(guest.0) as i64, guest.hunger_pct() as i64, GET_HUNGER_PCT.hooked()(i) as i64);
            check(&mut failures, &label("getThirstPct"), real::get_thirst_pct(guest.0) as i64, guest.thirst_pct() as i64, GET_THIRST_PCT.hooked()(i) as i64);
            check(&mut failures, &label("getEnergyPct"), real::get_energy_pct(guest.0) as i64, guest.energy_pct() as i64, GET_ENERGY_PCT.hooked()(i) as i64);
            check(&mut failures, &label("getBathroomPct"), real::get_bathroom_pct(guest.0) as i64, guest.bathroom_pct() as i64, GET_BATHROOM_PCT.hooked()(i) as i64);
            let real_pressing = real::has_pressing_need(guest.0);
            check(&mut failures, &label("hasPressingNeed"), real_pressing as i64, guest.has_pressing_need() as i64, HAS_PRESSING_NEED.hooked()(this) as i64);
            let real_busy = real::is_doing_something(guest.0);
            check(&mut failures, &label("isDoingSomething"), real_busy as i64, guest.is_doing_something() as i64, IS_DOING_SOMETHING.hooked()(this) as i64);
            check(&mut failures, &label("fAnimalHappy"), real::f_animal_happy(guest.0) as i64, guest.animal_happy() as i64, F_ANIMAL_HAPPY.hooked()(c) as i64);
            check(&mut failures, &label("fTourGuideBonus"), real::f_tour_guide_bonus(guest.0) as i64, guest.tour_guide_bonus() as i64, F_TOUR_GUIDE_BONUS.hooked()(c) as i64);
            check(&mut failures, &label("diagonalOK"), real::diagonal_ok(guest.0) as i64, guest.diagonal_ok() as i64, DIAGONAL_OK.hooked()(this) as i64);
            varied[0] += real_pressing as u32;
            varied[1] += real_busy as u32;
            varied[2] += guest.is_returning() as u32;
            varied[3] += guest.is_water_recovering() as u32;
            varied[4] += guest.is_water_traveling() as u32;
            varied[5] += guest.random_ok() as u32;
            varied[6] += (guest.hunger_pct() != 0) as u32;
        }
    }
    if varied.iter().any(|&n| n == 0 || n == rounds) {
        failures.push(format!("a predicate never varied across {rounds} rounds: {varied:?}"));
    }

    // Live guests carry real viewing areas, so the viewing-area-dependent methods get non-trivial answers.
    let mut with_viewing_area = 0;
    for (guest_ptr, _) in live_support::find_live_guests().into_iter().take(300) {
        let tile = unsafe { openzt_detour::generated::bfentity::GET_TILE.original()(guest_ptr as *const u32) } as u32;
        // Vanilla's `checkViewingArea` dereferences the habitat `getHabitatFromTile` (vtable `+0x248`) returns for
        // the guest's tile without a null check, so a guest on a tile with no habitat is skipped.
        let id: i32 = get_from_memory(guest_ptr + guest_offsets::VIEWING_AREA_ID);
        if id != -1 && crate::ztkeeper::call_slot_1(guest_ptr, 0x248, tile) == 0 {
            continue;
        }
        let guest = ZTGuest(guest_ptr);
        let this = guest_ptr as *const u32;
        let c = guest_ptr as *const c_void;
        let label = |name: &str| format!("live guest {guest_ptr:#x} {name}");
        unsafe {
            check(&mut failures, &label("getViewingArea"), real::get_viewing_area(guest_ptr) as i64, guest.viewing_area() as i64, GET_VIEWING_AREA.hooked()(this) as i64);
            check(&mut failures, &label("getHabitatFromVA"), real::get_habitat_from_va(guest_ptr) as i64, guest.habitat_from_va() as i64, GET_HABITAT_FROM_VA.hooked()(this) as i64);
            check(&mut failures, &label("fAnimalHappy"), real::f_animal_happy(guest_ptr) as i64, guest.animal_happy() as i64, F_ANIMAL_HAPPY.hooked()(c) as i64);
            check(&mut failures, &label("fTourGuideBonus"), real::f_tour_guide_bonus(guest_ptr) as i64, guest.tour_guide_bonus() as i64, F_TOUR_GUIDE_BONUS.hooked()(c) as i64);
            check(&mut failures, &label("getHappyPct"), real::get_happy_pct(guest_ptr) as i64, guest.happy_pct() as i64, GET_HAPPY_PCT.hooked()(guest_ptr as i32) as i64);
            check(&mut failures, &label("getHungerPct"), real::get_hunger_pct(guest_ptr) as i64, guest.hunger_pct() as i64, GET_HUNGER_PCT.hooked()(guest_ptr as i32) as i64);
            check(&mut failures, &label("getThirstPct"), real::get_thirst_pct(guest_ptr) as i64, guest.thirst_pct() as i64, GET_THIRST_PCT.hooked()(guest_ptr as i32) as i64);
            check(&mut failures, &label("getEnergyPct"), real::get_energy_pct(guest_ptr) as i64, guest.energy_pct() as i64, GET_ENERGY_PCT.hooked()(guest_ptr as i32) as i64);
            check(&mut failures, &label("getBathroomPct"), real::get_bathroom_pct(guest_ptr) as i64, guest.bathroom_pct() as i64, GET_BATHROOM_PCT.hooked()(guest_ptr as i32) as i64);
            check(&mut failures, &label("hasPressingNeed"), real::has_pressing_need(guest_ptr) as i64, guest.has_pressing_need() as i64, HAS_PRESSING_NEED.hooked()(this) as i64);
            check(&mut failures, &label("isDoingSomething"), real::is_doing_something(guest_ptr) as i64, guest.is_doing_something() as i64, IS_DOING_SOMETHING.hooked()(this) as i64);
            check(&mut failures, &label("isReturning"), real::is_returning(guest_ptr) as i64, guest.is_returning() as i64, IS_RETURNING.hooked()(this) as i64);
            check(&mut failures, &label("isWaterTraveling"), real::is_water_traveling(guest_ptr) as i64, guest.is_water_traveling() as i64, IS_WATER_TRAVELING.hooked()(this) as i64);
            check(&mut failures, &label("diagonalOK"), real::diagonal_ok(guest_ptr) as i64, guest.diagonal_ok() as i64, DIAGONAL_OK.hooked()(this) as i64);
            check(&mut failures, &label("getColorRepInfo"), real::get_color_rep_info(guest_ptr) as i64, guest.color_rep_info() as i64, GET_COLOR_REP_INFO.hooked()(this) as i64);
        }
        with_viewing_area += (guest.viewing_area() != 0) as u32;
    }
    let _ = with_viewing_area;
    finish_test(test_name, failures, failure_log)
}

/// `ZTGUEST_LEAF_SETTERS_LIVE`: `setWaterRecovering`, `setReturnBuildingNameID`, `randomDone`, `setSpeed` (variable
/// speeds on and off, across the energy thresholds) and the three mood-change methods leave the same memory and mood
/// records through the real body, the port and the hooked address.
pub(crate) fn run_ztguest_leaf_setters_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_LEAF_SETTERS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = vec![guest_type];
    let aimgr = globals().ztaimgr_ptr() as u32;
    let variable_flag_address = aimgr + 0x11c;
    let original_flag: u32 = get_from_memory(variable_flag_address);

    let mut run = |label: &str, seed: u32, body: &dyn Fn(ZTGuest, usize)| {
        let fixtures: Vec<ZTGuest> = (0..3).map(|_| live_support::build_standalone_guest(guest_type)).collect();
        let mut logs = Vec::new();
        for (pole, guest) in fixtures.iter().enumerate() {
            plant_state(*guest, &mut Lcg(seed));
            install_fake_vtable(*guest, false, false);
            MOOD_LOG.lock().unwrap().clear();
            body(*guest, pole);
            logs.push(MOOD_LOG.lock().unwrap().clone());
        }
        compare_guests(label, fixtures[0], fixtures[1], &shared, &mut failures);
        compare_guests(&format!("{label} hooked"), fixtures[0], fixtures[2], &shared, &mut failures);
        if logs[0] != logs[1] || logs[0] != logs[2] {
            failures.push(format!("{label}: mood records real {:?}, port {:?}, hooked {:?}", logs[0], logs[1], logs[2]));
        }
    };

    for seed in 1..40u32 {
        for recovering in [false, true] {
            run(&format!("setWaterRecovering({recovering}) seed {seed}"), seed, &|guest, pole| unsafe {
                match pole {
                    0 => real::set_water_recovering(guest.0, recovering),
                    1 => guest.set_water_recovering(recovering),
                    _ => SET_WATER_RECOVERING.hooked()(guest.0 as *const u32, recovering),
                }
            });
        }
        let id = (seed.wrapping_mul(0x9e37) & 0xf_ffff) as i32;
        run(&format!("setReturnBuildingNameID seed {seed}"), seed, &|guest, pole| unsafe {
            match pole {
                0 => real::set_return_building_name_id(guest.0, id),
                1 => guest.set_return_building_name_id(id),
                _ => SET_RETURN_BUILDING_NAME_ID.hooked()(guest.0 as *const u32, id),
            }
        });
        run(&format!("randomDone seed {seed}"), seed, &|guest, pole| unsafe {
            match pole {
                0 => real::random_done(guest.0),
                1 => guest.random_done(),
                _ => RANDOM_DONE.hooked()(guest.0 as *const u32),
            }
        });
        type Method = (&'static str, fn(u32), fn(ZTGuest));
        let methods: [Method; 3] = [
            ("doAngryPriceChange", real::do_angry_price_change, ZTGuest::do_angry_price_change),
            ("doHappyPriceChange", real::do_happy_price_change, ZTGuest::do_happy_price_change),
            ("doTourguideBonus", real::do_tourguide_bonus, ZTGuest::do_tourguide_bonus),
        ];
        let hooked = [&DO_ANGRY_PRICE_CHANGE, &DO_HAPPY_PRICE_CHANGE, &DO_TOURGUIDE_BONUS];
        for ((name, vanilla, port), hooked) in methods.into_iter().zip(hooked) {
            run(&format!("{name} seed {seed}"), seed, &|guest, pole| unsafe {
                match pole {
                    0 => vanilla(guest.0),
                    1 => port(guest),
                    _ => hooked.hooked()(guest.0 as *const u32),
                }
            });
        }
    }

    for variable in [0u32, 1] {
        save_to_memory(variable_flag_address, variable);
        for energy in [-5i32, 0, 10, 0x13, 0x14, 40, 0x4a, 0x4b, 90, 120] {
            for speed in [0u32, 7, 40, 250, 0xffff_fff0] {
                for base_byte in [0u8, 3, 5, 200] {
                    run(&format!("setSpeed var {variable} energy {energy} speed {speed} base {base_byte}"), 3, &|guest, pole| unsafe {
                        save_to_memory(guest.0 + guest_offsets::ENERGY, energy);
                        save_to_memory(guest.0 + 0x204, base_byte);
                        match pole {
                            0 => real::set_speed(guest.0, speed),
                            1 => guest.set_speed(speed),
                            _ => SET_SPEED.hooked()(guest.0 as *const u32, speed),
                        }
                    });
                }
            }
        }
    }
    save_to_memory(variable_flag_address, original_flag);
    finish_test(test_name, failures, failure_log)
}
