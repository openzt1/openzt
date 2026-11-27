//! Live checks for the show and trick group of `ZTUnit` (`ztunit/shows.rs`): twin units, one run through the real body and
//! one through the port, with the unit's virtual slots and the unit type's show-item lookup scripted so the calls made can
//! be compared as well as the memory and the answers.

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Mutex, Once,
};

use openzt_detour::generated::{
    standalone::OPERATOR_NEW,
    ztshowmgr::GET_SHOW_INFO,
    ztunittype::GET_SHOW_ITEM as ZTUNITTYPE_GET_SHOW_ITEM,
};

use super::{
    check, compare_log, compare_memory, leaked_words, log, pin_rng, rng_state,
    targets::{live_entities, no_units},
    take_log, Twins,
};
use crate::{
    globals::get_module_base,
    reimplementation_tests::{harness::finish_test, tests::ztanimal::live_animal_types},
    util::{get_from_memory, save_to_memory},
    ztunit::{live_support::real_shows, unit_offsets, ZTUnit},
};

const BUILDING_TAG_RVA: u32 = 0x0023_8680;
const SHOW_MGR_RVA: u32 = 0x0023_9008;
const FREELIST_TABLE_RVA: u32 = 0x0023_8000;
const BUILDING_NEED_NAME_RVA: u32 = 0x0023_afd8;
const TYPE_SHOW_LIST: u32 = 0x1ac;
const EVENT_SIZE: u32 = 0x1c;

// ---- scripted answers ------------------------------------------------------------------------------------------

static IS_SHOW_UNIT: AtomicU32 = AtomicU32::new(0);
static CHECK_DO_TRICK: AtomicU32 = AtomicU32::new(0);
static IS_TANK: AtomicU32 = AtomicU32::new(0);
static HABITAT: AtomicU32 = AtomicU32::new(0);
static ITEM: AtomicU32 = AtomicU32::new(0);
static SLOT_ANSWER: AtomicU32 = AtomicU32::new(0);
static FIND_BUILDING: AtomicU32 = AtomicU32::new(0);
static LAST_EVENT_BLOCK: AtomicU32 = AtomicU32::new(0);
static EVENTS: Mutex<Vec<(u32, u16)>> = Mutex::new(Vec::new());
static STUBBING: AtomicBool = AtomicBool::new(false);
static TEST_THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);

fn stubbing() -> bool {
    STUBBING.load(Ordering::SeqCst) && *TEST_THREAD.lock().unwrap() == Some(std::thread::current().id())
}

fn start_stubbing() {
    *TEST_THREAD.lock().unwrap() = Some(std::thread::current().id());
    STUBBING.store(true, Ordering::SeqCst);
}

fn stop_stubbing() {
    STUBBING.store(false, Ordering::SeqCst);
    *TEST_THREAD.lock().unwrap() = None;
}

#[openzt_detour_macro::detour_mod]
mod show_stubs {
    use super::*;

    #[detour(ZTUNITTYPE_GET_SHOW_ITEM)]
    unsafe extern "stdcall" fn unit_type_get_show_item(trick: u16, script: u32) -> u32 {
        if stubbing() {
            log(format!("unitType::getShowItem {trick} {script}"));
            ITEM.load(Ordering::SeqCst)
        } else {
            unsafe { ZTUNITTYPE_GET_SHOW_ITEM_DETOUR.call(trick, script) }
        }
    }
}

fn install_stubs() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { show_stubs::init_detours() } {
            result = Err(format!("could not install the show stubs: {e:?}"));
        }
    });
    result
}

// ---- scripted virtual slots ------------------------------------------------------------------------------------

extern "thiscall" fn fake_send_event(_this: u32, kind: u32, id: u32, tag: u32, a: u32, b: u32, c: u32) -> u32 {
    log(format!("sendEvent {kind:#x} {id:#x} {tag:#x} {a} {b} {c}"));
    0
}
extern "thiscall" fn fake_get_events(_this: u32, out: u32) {
    let events = EVENTS.lock().unwrap().clone();
    if events.is_empty() {
        save_to_memory(out, 0u32);
        save_to_memory(out + 4, 0u32);
        save_to_memory(out + 8, 0u32);
        LAST_EVENT_BLOCK.store(0, Ordering::SeqCst);
        return;
    }
    let bytes = events.len() as u32 * EVENT_SIZE;
    // Small blocks are leaked at the freelist bucket's full size so vanilla can never overrun one; large ones come from vanilla.
    let block = if bytes > 0x80 {
        unsafe { OPERATOR_NEW.original()(bytes) as u32 }
    } else {
        leaked_words(vec![0; ((((bytes - 1) >> 3) * 8 + 8) / 4) as usize])
    };
    for (index, &(entity, kind)) in events.iter().enumerate() {
        let record = block + index as u32 * EVENT_SIZE;
        save_to_memory(record + 0xc, entity);
        save_to_memory(record + 0x16, kind);
    }
    save_to_memory(out, block);
    save_to_memory(out + 4, block + bytes);
    save_to_memory(out + 8, block + bytes);
    LAST_EVENT_BLOCK.store(block, Ordering::SeqCst);
}
extern "thiscall" fn fake_is_show_unit(_this: u32) -> u32 {
    log("isShowUnit".to_string());
    IS_SHOW_UNIT.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_get_habitat(_this: u32) -> u32 {
    log("getHabitat".to_string());
    HABITAT.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_check_do_trick(_this: u32) -> u32 {
    log("checkDoTrick".to_string());
    CHECK_DO_TRICK.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_do_toy_trick(_this: u32, trick: u32, toy: u32, show: u32) -> u32 {
    log(format!("doToyTrick {} {toy:#x} {}", trick & 0xffff, show & 0xffff));
    SLOT_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_do_food_trick(_this: u32, trick: u32, show: u32) -> u32 {
    log(format!("doFoodTrick {} {}", trick & 0xffff, show & 0xffff));
    SLOT_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_do_behavior_trick(_this: u32, trick: u32, show: u32) -> u32 {
    log(format!("doBehaviorTrick {} {}", trick & 0xffff, show & 0xffff));
    SLOT_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_validate_toy_trick(_this: u32, trick: u32, toy: u32, show: u32) -> u32 {
    log(format!("validateToyTrick {} {toy:#x} {}", trick & 0xffff, show & 0xffff));
    SLOT_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_validate_food_trick(_this: u32, trick: u32, show: u32) -> u32 {
    log(format!("validateFoodTrick {} {}", trick & 0xffff, show & 0xffff));
    SLOT_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_validate_behavior_trick(_this: u32, trick: u32, show: u32) -> u32 {
    log(format!("validateBehaviorTrick {} {}", trick & 0xffff, show & 0xffff));
    SLOT_ANSWER.load(Ordering::SeqCst)
}
extern "thiscall" fn fake_habitat_is_tank(_this: u32) -> u32 {
    log("isTank".to_string());
    IS_TANK.load(Ordering::SeqCst)
}

/// Layers the show slots over the unit's current (already scripted) vtable.
fn script_show_slots(unit: ZTUnit) {
    let current: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    let scripted: [(u32, u32); 11] = [
        (0x0, fake_send_event as *const () as u32),
        (0x8, fake_get_events as *const () as u32),
        (0x204, fake_do_toy_trick as *const () as u32),
        (0x208, fake_do_food_trick as *const () as u32),
        (0x20c, fake_do_behavior_trick as *const () as u32),
        (0x21c, fake_validate_toy_trick as *const () as u32),
        (0x220, fake_validate_food_trick as *const () as u32),
        (0x224, fake_validate_behavior_trick as *const () as u32),
        (0x228, fake_is_show_unit as *const () as u32),
        (0x234, fake_check_do_trick as *const () as u32),
        (0x24c, fake_get_habitat as *const () as u32),
    ];
    for (slot, function) in scripted {
        vtable[(slot / 4) as usize] = function;
    }
    save_to_memory(unit.0, leaked_words(vtable));
}

fn show_twins(unit_type: u32) -> Twins {
    let twins = Twins::new(unit_type, true);
    for unit in twins.both() {
        script_show_slots(unit);
    }
    twins
}

// ---- scripted collaborators ------------------------------------------------------------------------------------

fn scripted_find_any(_unit: u32, tile: u32, name: u32, out: u32, range: i32, filter: i32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    let building = FIND_BUILDING.load(Ordering::SeqCst);
    log(format!("findAnySatisfier {} {} {range:#x} {filter:#x}", tile != 0, name == get_module_base("zoo.exe") as u32 + BUILDING_NEED_NAME_RVA));
    if building != 0 {
        save_to_memory(out, building);
    }
    Some(building != 0)
}

fn scripted_set_bs(_this: u32, begin: u32, end: u32, call_flag: i8, force: i8, clear_mode: i32) -> Option<bool> {
    if !stubbing() {
        return None;
    }
    let name: Vec<u8> = if begin == 0 || end <= begin { Vec::new() } else { (begin..end).map(get_from_memory::<u8>).collect() };
    log(format!("setBS {} {call_flag} {force} {clear_mode}", String::from_utf8_lossy(&name)));
    Some(true)
}

type SavedHooks = (Option<fn(u32, u32, u32, u32, i32, i32) -> Option<bool>>, Option<crate::bfentity::set_bs_override::Hook>);

fn install_hooks() -> SavedHooks {
    use crate::ztunit::live_support::intercept;
    let saved = (*intercept::FIND_ANY_SATISFIER.lock().unwrap(), *crate::bfentity::set_bs_override::HOOK.lock().unwrap());
    *intercept::FIND_ANY_SATISFIER.lock().unwrap() = Some(scripted_find_any);
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = Some(scripted_set_bs);
    saved
}

fn restore_hooks(saved: SavedHooks) {
    use crate::ztunit::live_support::intercept;
    *intercept::FIND_ANY_SATISFIER.lock().unwrap() = saved.0;
    *crate::bfentity::set_bs_override::HOOK.lock().unwrap() = saved.1;
}

// ---- fixtures --------------------------------------------------------------------------------------------------

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// A show item: kind, name, toy id and minimum level.
fn fake_item(kind: u32, name: &[u8], toy: u32, minimum: i32) -> u32 {
    let text = leaked_words(vec![0; name.len() / 4 + 2]);
    for (index, &byte) in name.iter().enumerate() {
        save_to_memory(text + index as u32, byte);
    }
    let item = leaked_words(vec![0; 0x20]);
    save_to_memory(item + 0xc, kind);
    save_to_memory(item + 0x10, text);
    save_to_memory(item + 0x14, text + name.len() as u32);
    save_to_memory(item + 0x18, text + name.len() as u32 + 1);
    save_to_memory(item + 0x40, toy);
    save_to_memory(item + 0x58, minimum);
    item
}

/// A fake habitat whose `isTank` answers from [`IS_TANK`], owned by an object whose `+0x70` is `show`, at tank `level`.
fn fake_habitat(show: u16, level: i32) -> u32 {
    let mut vtable = vec![0u32; 16];
    vtable[(0x20 / 4) as usize] = fake_habitat_is_tank as *const () as u32;
    let owner = leaked_words(vec![0; 0x40]);
    save_to_memory(owner + 0x70, show);
    let habitat = leaked_words(vec![0; 0x80]);
    save_to_memory(habitat, leaked_words(vtable));
    save_to_memory(habitat + 4, owner);
    save_to_memory(habitat + 0x188, level);
    habitat
}

/// Show ids to try: none, ones no show has and the live shows' ids (a query on them is read-only).
fn show_ids() -> Vec<u16> {
    let manager: u32 = get_from_memory(base() + SHOW_MGR_RVA);
    let mut ids = vec![0u16, 0x7777, 0xffff];
    if manager != 0 {
        for id in 1u16..200 {
            if unsafe { GET_SHOW_INFO.hooked()(manager as *const u32, id) } as u32 != 0 && ids.len() < 6 {
                ids.push(id);
            }
        }
    }
    ids
}

fn freelist_head(bytes: u32) -> u32 {
    get_from_memory(base() + FREELIST_TABLE_RVA + ((bytes - 1) >> 3) * 4)
}

fn set_freelist_head(bytes: u32, value: u32) {
    save_to_memory(base() + FREELIST_TABLE_RVA + ((bytes - 1) >> 3) * 4, value);
}

// ---- tests -----------------------------------------------------------------------------------------------------

/// `ZTUNIT_SHOWS_LIVE`: the show and trick methods against the real bodies with scripted twins.
pub(crate) fn run_ztunit_shows_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_SHOWS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    let buildings = live_entities(BUILDING_TAG_RVA);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    let saved = install_hooks();
    let saved_rng = rng_state();
    start_stubbing();
    let ids = show_ids();
    let items = [
        0u32,
        fake_item(0, b"Roll", 0, 0),
        fake_item(1, b"Toy", 0x1234, 3),
        fake_item(3, b"Food", 0, 0),
        fake_item(7, b"Other", 0, 0),
        fake_item(0, b"", 0, 100),
    ];
    let (mut keeper_false, mut events_handled, mut big_blocks, mut toys_found) = (0u32, 0u32, 0u32, 0u32);

    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = show_twins(unit_type);
        let label = format!("type {type_index}");
        let compare_both = |failures: &mut Vec<String>, case: &str, real: u32, port: u32, real_calls: &[String], port_calls: &[String]| {
            check(failures, &format!("{case} value"), real, port);
            compare_log(case, real_calls, port_calls, failures);
            compare_memory(case, twins.real, twins.port, failures);
        };

        // Skip-trick getters, isShowUnit (also over a type with a show list) and doFoodTrick.
        check(&mut failures, &format!("{label} getSkipTrickHappiness"), real_shows::get_skip_trick_happiness(twins.real.0), twins.port.get_skip_trick_happiness());
        check(&mut failures, &format!("{label} getSkipTrickChance"), real_shows::get_skip_trick_chance(twins.real.0), twins.port.get_skip_trick_chance());
        check(&mut failures, &format!("{label} doFoodTrick"), real_shows::do_food_trick(twins.real.0, 1, 2), twins.port.do_food_trick());
        check(&mut failures, &format!("{label} isShowUnit"), real_shows::is_show_unit(twins.real.0) & 0xff, u32::from(twins.port.is_show_unit()));
        let entity_type: u32 = get_from_memory(twins.port.0 + unit_offsets::ENTITY_TYPE);
        let original_list: u32 = get_from_memory(entity_type + TYPE_SHOW_LIST);
        let populated = leaked_words(vec![0; 3]);
        let node = leaked_words(vec![populated, populated]);
        save_to_memory(populated, node);
        save_to_memory(populated + 4, node);
        for list in [populated, 0] {
            save_to_memory(entity_type + TYPE_SHOW_LIST, list);
            if list != 0 {
                check(&mut failures, &format!("{label} isShowUnit populated"), real_shows::is_show_unit(twins.real.0) & 0xff, u32::from(twins.port.is_show_unit()));
                check(&mut failures, &format!("{label} isShowUnit populated answer"), true, twins.port.is_show_unit());
            }
        }
        save_to_memory(entity_type + TYPE_SHOW_LIST, original_list);

        // endTrick / abortTrick send an event to the unit itself.
        for unit in twins.both() {
            save_to_memory(unit.0 + unit_offsets::ENTITY_ID, 0x4242u32);
        }
        take_log();
        real_shows::end_trick(twins.real.0);
        real_shows::abort_trick(twins.real.0);
        let real_calls = take_log();
        twins.port.end_trick();
        twins.port.abort_trick();
        let port_calls = take_log();
        compare_log(&format!("{label} endTrick/abortTrick"), &real_calls, &port_calls, &mut failures);
        check(&mut failures, &format!("{label} two events sent"), 2usize, port_calls.len());

        // isDoingShow / isShowScriptDone / abortShow.
        for is_show_unit in [0u32, 1] {
            for flag in [0u8, 1] {
                for &show in &ids {
                    for unit in twins.both() {
                        save_to_memory(unit.0 + unit_offsets::SHOW_FLAG_B, flag);
                        save_to_memory(unit.0 + unit_offsets::SHOW_INFO_ID, show);
                    }
                    IS_SHOW_UNIT.store(is_show_unit, Ordering::SeqCst);
                    let case = format!("{label} isDoingShow unit {is_show_unit} flag {flag} show {show:#x}");
                    take_log();
                    let real_value = real_shows::is_doing_show(twins.real.0) & 0xff;
                    let real_calls = take_log();
                    let port_value = u32::from(twins.port.is_doing_show());
                    let port_calls = take_log();
                    compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
                    let case = format!("{label} isShowScriptDone unit {is_show_unit} show {show:#x}");
                    let real_value = real_shows::is_show_script_done(twins.real.0) & 0xff;
                    let real_calls = take_log();
                    let port_value = u32::from(twins.port.is_show_script_done());
                    let port_calls = take_log();
                    compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
                }
            }
        }
        // A bogus show answers no info, so aborting is a no-op on both.
        for unit in twins.both() {
            save_to_memory(unit.0 + unit_offsets::SHOW_INFO_ID, 0x7777u16);
        }
        real_shows::abort_show(twins.real.0);
        twins.port.abort_show();
        compare_memory(&format!("{label} abortShow"), twins.real, twins.port, &mut failures);

        // hasShowKeeper over the habitat's owner show.
        for habitat in [0u32].into_iter().chain(ids.iter().map(|&show| fake_habitat(show, 0))) {
            for unit in twins.both() {
                HABITAT.store(habitat, Ordering::SeqCst);
                save_to_memory(unit.0 + unit_offsets::ENTITY_ID, 0x4242u32);
            }
            let case = format!("{label} hasShowKeeper habitat {habitat:#x}");
            take_log();
            let real_value = real_shows::has_show_keeper(twins.real.0) & 0xff;
            let real_calls = take_log();
            let port_value = u32::from(twins.port.has_show_keeper());
            let port_calls = take_log();
            compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
            keeper_false += u32::from(port_value == 0);
        }
        HABITAT.store(0, Ordering::SeqCst);

        // getShowItem, doShowItem and doBehaviorTrick over the scripted items.
        for &item in &items {
            ITEM.store(item, Ordering::SeqCst);
            for &show in ids.iter().take(3) {
                let case = format!("{label} getShowItem item {item:#x} show {show:#x}");
                take_log();
                let real_value = real_shows::get_show_item(twins.real.0, 9, show);
                let real_calls = take_log();
                let port_value = twins.port.get_show_item(9, show);
                let port_calls = take_log();
                compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
            }
            for check_do in [0u32, 1] {
                for answer in [0u32, 3, 5] {
                    CHECK_DO_TRICK.store(check_do, Ordering::SeqCst);
                    SLOT_ANSWER.store(answer, Ordering::SeqCst);
                    let case = format!("{label} doShowItem item {item:#x} check {check_do} answer {answer}");
                    take_log();
                    // Vanilla dereferences a missing item; the port answers 3 there.
                    let real_value = if item == 0 { 3 } else { real_shows::do_show_item(twins.real.0, 9, 4) };
                    let real_calls = if item == 0 { Vec::new() } else { take_log() };
                    let port_value = twins.port.do_show_item(9, 4);
                    let mut port_calls = take_log();
                    if item == 0 {
                        port_calls.clear();
                    }
                    compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
                }
            }
            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::TRICK_TYPE, 0u16);
                save_to_memory(unit.0 + unit_offsets::SHOW_INFO_ID, 0u16);
            }
            let case = format!("{label} doBehaviorTrick item {item:#x}");
            take_log();
            let real_value = real_shows::do_behavior_trick(twins.real.0, 11, 12);
            let real_calls = take_log();
            let port_value = twins.port.do_behavior_trick(11, 12);
            let port_calls = take_log();
            compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
        }

        // validateTrickType over keeper, item kind, tank and level.
        for &item in &items {
            ITEM.store(item, Ordering::SeqCst);
            for habitat in [0u32, fake_habitat(0x7777, 5), fake_habitat(0x7777, 0), fake_habitat(0, 100)] {
                for tank in [0u32, 1] {
                    for answer in [0u32, 3] {
                        HABITAT.store(habitat, Ordering::SeqCst);
                        IS_TANK.store(tank, Ordering::SeqCst);
                        SLOT_ANSWER.store(answer, Ordering::SeqCst);
                        for unit in twins.both() {
                            save_to_memory(unit.0 + unit_offsets::SHOW_INFO_ID, 0x3333u16);
                        }
                        let case = format!("{label} validateTrickType item {item:#x} habitat {habitat:#x} tank {tank} answer {answer}");
                        take_log();
                        let real_value = real_shows::validate_trick_type(twins.real.0, 9, 4);
                        let real_calls = take_log();
                        let port_value = twins.port.validate_trick_type(9, 4);
                        let port_calls = take_log();
                        compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
                    }
                }
            }
        }
        HABITAT.store(0, Ordering::SeqCst);

        // validateToyTrick / doToyTrick over a found and a missing toy, with and without a show habitat.
        let toy_buildings: Vec<u32> = std::iter::once(0).chain(buildings.iter().copied().take(4)).collect();
        for &building in &toy_buildings {
            for &show in &ids {
                FIND_BUILDING.store(building, Ordering::SeqCst);
                for unit in twins.both() {
                    save_to_memory(unit.0 + unit_offsets::NEED_INDEX, 0xffu8);
                    save_to_memory(unit.0 + unit_offsets::TRICK_TYPE, 0u16);
                    save_to_memory(unit.0 + unit_offsets::SHOW_INFO_ID, 0u16);
                }
                let case = format!("{label} validateToyTrick building {building:#x} show {show:#x}");
                take_log();
                let real_value = real_shows::validate_toy_trick(twins.real.0, 5, 0x1234, show);
                let real_calls = take_log();
                let port_value = twins.port.validate_toy_trick(0x1234, show);
                let port_calls = take_log();
                compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
                let case = format!("{label} doToyTrick building {building:#x} show {show:#x}");
                let real_value = real_shows::do_toy_trick(twins.real.0, 5, 0x1234, show);
                let real_calls = take_log();
                let port_value = twins.port.do_toy_trick(5, 0x1234, show);
                let port_calls = take_log();
                compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
                toys_found += u32::from(port_value == 0);
            }
        }

        // handleShowEvents over every event kind, the matching and a foreign entity, and the storage sizes.
        let kinds: [u16; 9] = [0x271a, 0x271b, 0x271c, 0x271e, 0x2722, 0x2723, 0x2725, 0x2724, 0x2719];
        let mut scripts: Vec<Vec<(u32, u16)>> = vec![vec![]];
        for &kind in &kinds {
            scripts.push(vec![(0x4242, kind)]);
            scripts.push(vec![(0x9999, kind)]);
        }
        scripts.push(vec![(0x4242, 0x271a), (0x4242, 0x271e), (0x9999, 0x2722), (0x4242, 0x2725)]);
        scripts.push((0..5).map(|index| (0x4242, kinds[index])).collect());
        scripts.push((0..8).map(|index| (if index % 2 == 0 { 0x4242 } else { 0x1 }, kinds[index])).collect());
        let state_block = |value: u8| {
            let state = leaked_words(vec![0; 8]);
            for offset in 0..0x20u32 {
                save_to_memory(state + offset, value);
            }
            state
        };
        for (index, script) in scripts.iter().enumerate() {
            *EVENTS.lock().unwrap() = script.clone();
            for unit in twins.both() {
                save_to_memory(unit.0 + unit_offsets::ENTITY_ID, 0x4242u32);
                save_to_memory(unit.0 + unit_offsets::SHOW_FLAG_A, 0u8);
                save_to_memory(unit.0 + unit_offsets::SHOW_FLAG_B, 7u8);
            }
            let bytes = script.len() as u32 * EVENT_SIZE;
            let (real_state, port_state) = (state_block(0), state_block(0));
            let small = bytes != 0 && bytes <= 0x80;
            let saved_head = if small { freelist_head(bytes) } else { 0 };
            take_log();
            let real_value = real_shows::handle_show_events(twins.real.0, real_state);
            let real_calls = take_log();
            let real_block = LAST_EVENT_BLOCK.load(Ordering::SeqCst);
            let real_head = if small { freelist_head(bytes) } else { 0 };
            if small {
                set_freelist_head(bytes, saved_head);
            }
            let port_value = twins.port.handle_show_events(port_state);
            let port_calls = take_log();
            let port_block = LAST_EVENT_BLOCK.load(Ordering::SeqCst);
            let port_head = if small { freelist_head(bytes) } else { 0 };
            if small {
                set_freelist_head(bytes, saved_head);
                check(&mut failures, &format!("{label} handleShowEvents #{index} real freelist head"), real_block, real_head);
                check(&mut failures, &format!("{label} handleShowEvents #{index} port freelist head"), port_block, port_head);
            }
            big_blocks += u32::from(bytes > 0x80);
            events_handled += script.len() as u32;
            let case = format!("{label} handleShowEvents #{index} {script:?}");
            compare_both(&mut failures, &case, real_value, port_value, &real_calls, &port_calls);
            for offset in 0..0x20u32 {
                check(&mut failures, &format!("{case} state +{offset:#x}"), get_from_memory::<u8>(real_state + offset), get_from_memory::<u8>(port_state + offset));
            }
        }
        *EVENTS.lock().unwrap() = Vec::new();
        check(&mut failures, &format!("{label} handleShowEvents no state"), real_shows::handle_show_events(twins.real.0, 0), twins.port.handle_show_events(0));
        check(&mut failures, &format!("{label} handleShowEvents no state answer"), u32::MAX, twins.port.handle_show_events(0));

        // The hooked addresses reach the port.
        let hooked = unsafe { openzt_detour::generated::ztunit::GET_SKIP_TRICK_CHANCE.hooked()(twins.port.0 as *const u32) };
        check(&mut failures, &format!("{label} hooked getSkipTrickChance"), hooked, twins.port.get_skip_trick_chance());
        twins.destroy();
    }
    if keeper_false == 0 {
        log("note: no show without a keeper was reachable".to_string());
    }
    if events_handled == 0 || big_blocks == 0 || (toys_found == 0 && !buildings.is_empty()) {
        failures.push(format!("the test did not reach every branch (events {events_handled}, large blocks {big_blocks}, toys {toys_found})"));
    }
    take_log();
    stop_stubbing();
    restore_hooks(saved);
    pin_rng(saved_rng);
    finish_test(test_name, failures, failure_log)
}
