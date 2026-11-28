//! Live checks for the message group of `ZTUnit` (`ztunit/messages.rs`): twin units, one run through the real body and one
//! through the port, with `BFUIMgr::displayMessage` and the unit's `isRemoved` slot scripted so the messages shown can be
//! compared as well as the formatted text and the unit's memory.

use std::sync::{
    atomic::{AtomicBool, AtomicU32, Ordering},
    Mutex, Once,
};

use openzt_detour::generated::bfuimgr::DISPLAY_MESSAGE_1;

use super::{
    check, compare_log, compare_memory, leaked_words, log,
    targets::no_units,
    take_log, Twins,
};
use crate::{
    reimplementation_tests::{harness::finish_test, tests::ztanimal::live_animal_types},
    util::{get_from_memory, save_to_memory},
    vanilla_string::VanillaString,
    ztunit::{live_support::real_messages, ZTUnit},
};

/// Message string ids with and without `%s` (death, hatched, escaped-animal alert, a plain label).
const MESSAGE_IDS: [u32; 5] = [0x2852, 0x2afa, 0x1000, 0x2711, 0x0];
const TYPE_GENDER_STRING: u32 = 0xa4;
const NAME_WORD: u32 = 0x108;

static REMOVED: AtomicU32 = AtomicU32::new(0);
static STUBBING: AtomicBool = AtomicBool::new(false);
static TEST_THREAD: Mutex<Option<std::thread::ThreadId>> = Mutex::new(None);

fn stubbing() -> bool {
    STUBBING.load(Ordering::SeqCst) && *TEST_THREAD.lock().unwrap() == Some(std::thread::current().id())
}

fn text_of(message: u32) -> String {
    let begin: u32 = get_from_memory(message);
    let end: u32 = get_from_memory(message + 4);
    if begin == 0 || end <= begin {
        return String::new();
    }
    (begin..end).map(|address| get_from_memory::<u8>(address) as char).collect()
}

#[openzt_detour_macro::detour_mod]
mod message_stubs {
    use super::*;

    #[detour(DISPLAY_MESSAGE_1)]
    unsafe extern "thiscall" fn display_message(this: *const u32, message: *const i32, kind: i32, tile: *const u32, entity: *const u32, a: bool, b: bool) {
        if stubbing() {
            log(format!("displayMessage {:?} {kind} {:#x} {:#x} {a} {b}", text_of(message as u32), tile as u32, entity as u32));
        } else {
            unsafe { DISPLAY_MESSAGE_1_DETOUR.call(this, message, kind, tile, entity, a, b) }
        }
    }
}

extern "thiscall" fn fake_is_removed(_this: u32) -> u32 {
    log("isRemoved".to_string());
    REMOVED.load(Ordering::SeqCst)
}

fn install_stubs() -> Result<(), String> {
    static INSTALLED: Once = Once::new();
    let mut result = Ok(());
    INSTALLED.call_once(|| {
        if let Err(e) = unsafe { message_stubs::init_detours() } {
            result = Err(format!("could not install the message stubs: {e:?}"));
        }
    });
    result
}

fn script_slot(unit: ZTUnit) {
    let current: u32 = get_from_memory(unit.0);
    let mut vtable: Vec<u32> = (0..197u32).map(|slot| get_from_memory(current + slot * 4)).collect();
    vtable[(0xac / 4) as usize] = fake_is_removed as *const () as u32;
    save_to_memory(unit.0, leaked_words(vtable));
}

/// The calls made, with a twin's own address replaced by `unit` so the two runs compare.
fn normalized(calls: Vec<String>, unit: ZTUnit) -> Vec<String> {
    calls.into_iter().map(|call| call.replace(&format!("{:#x}", unit.0), "unit")).collect()
}

fn leaked_text(text: &[u8]) -> (u32, u32) {
    let buffer = leaked_words(vec![0; text.len() / 4 + 2]);
    for (index, &byte) in text.iter().enumerate() {
        save_to_memory(buffer + index as u32, byte);
    }
    (buffer, buffer + text.len() as u32)
}

/// `ZTUNIT_MESSAGES_LIVE`: `formatNameMessage`, the `fZooMessage*` family and `getSellPrice` against the real bodies.
pub(crate) fn run_ztunit_messages_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTUNIT_MESSAGES_LIVE";
    let mut failures: Vec<String> = Vec::new();
    if let Err(e) = install_stubs() {
        return finish_test(test_name, vec![e], failure_log);
    }
    let types = live_animal_types(2);
    if types.is_empty() {
        return no_units(test_name, failure_log);
    }
    *TEST_THREAD.lock().unwrap() = Some(std::thread::current().id());
    STUBBING.store(true, Ordering::SeqCst);
    let (plain, plain_end) = leaked_text(b"Rex");
    let (feminine, feminine_end) = leaked_text(b"female");
    let (other, other_end) = leaked_text(b"male");
    let mut shown = 0u32;

    for (type_index, &unit_type) in types.iter().enumerate() {
        let twins = Twins::new(unit_type, true);
        let saved_names = twins.both().map(|unit| [0u32, 4, 8].map(|offset| get_from_memory::<u32>(unit.0 + NAME_WORD + offset)));
        for unit in twins.both() {
            script_slot(unit);
            save_to_memory(unit.0 + NAME_WORD, plain);
            save_to_memory(unit.0 + NAME_WORD + 4, plain_end);
            save_to_memory(unit.0 + NAME_WORD + 8, plain_end);
        }
        // The entity whose name `fZooMessageEntity` formats: another leaked name holder.
        let entity = leaked_words(vec![0; 0x80]);
        save_to_memory(entity + NAME_WORD, other);
        save_to_memory(entity + NAME_WORD + 4, other_end);
        save_to_memory(entity + NAME_WORD + 8, other_end);
        let saved_gender = [get_from_memory::<u32>(unit_type + TYPE_GENDER_STRING), get_from_memory::<u32>(unit_type + TYPE_GENDER_STRING + 4)];

        for (gender_index, gender) in [None, Some((feminine, feminine_end)), Some((other, other_end))].into_iter().enumerate() {
            if let Some((begin, end)) = gender {
                save_to_memory(unit_type + TYPE_GENDER_STRING, begin);
                save_to_memory(unit_type + TYPE_GENDER_STRING + 4, end);
            } else {
                save_to_memory(unit_type + TYPE_GENDER_STRING, saved_gender[0]);
                save_to_memory(unit_type + TYPE_GENDER_STRING + 4, saved_gender[1]);
            }
            for &id in &MESSAGE_IDS {
                let label = format!("type {type_index} gender {gender_index} id {id:#x}");
                // formatNameMessage: the text.
                let real_text = VanillaString::rvo_target();
                real_messages::format_name_message(twins.real.0, real_text.as_ptr(), id);
                let port_text = twins.port.format_name_message(id);
                check(&mut failures, &format!("{label} formatNameMessage"), real_text.bytes(), port_text.bytes());

                for removed in [0u32, 1] {
                    REMOVED.store(removed, Ordering::SeqCst);
                    for force in [0i8, 1] {
                        let case = format!("{label} removed {removed} force {force}");
                        take_log();
                        real_messages::f_zoo_message_name(twins.real.0, id, 2, force);
                        let real_calls = normalized(take_log(), twins.real);
                        twins.port.f_zoo_message_name(id, 2, force as u8);
                        let port_calls = normalized(take_log(), twins.port);
                        shown += u32::from(real_calls.iter().any(|call| call.starts_with("displayMessage")));
                        compare_log(&format!("{case} fZooMessageName"), &real_calls, &port_calls, &mut failures);

                        real_messages::f_zoo_message_tile(twins.real.0, id, 4, 0x1234, 0x5678, force);
                        let real_calls = normalized(take_log(), twins.real);
                        twins.port.f_zoo_message_tile(id, 4, 0x1234, 0x5678, force as u8);
                        let port_calls = normalized(take_log(), twins.port);
                        compare_log(&format!("{case} fZooMessageTile"), &real_calls, &port_calls, &mut failures);

                        for target in [0u32, entity] {
                            real_messages::f_zoo_message_entity(twins.real.0, id, 3, target as i32, force);
                            let real_calls = normalized(take_log(), twins.real);
                            twins.port.f_zoo_message_entity(id, 3, target, force as u8);
                            let port_calls = normalized(take_log(), twins.port);
                            compare_log(&format!("{case} fZooMessageEntity target {target:#x}"), &real_calls, &port_calls, &mut failures);
                        }
                    }
                }
                REMOVED.store(0, Ordering::SeqCst);
            }
        }
        save_to_memory(unit_type + TYPE_GENDER_STRING, saved_gender[0]);
        save_to_memory(unit_type + TYPE_GENDER_STRING + 4, saved_gender[1]);

        // getSellPrice.
        check(&mut failures, &format!("type {type_index} getSellPrice"), real_messages::get_sell_price(twins.real.0).to_bits(), twins.port.get_sell_price().to_bits());
        compare_memory(&format!("type {type_index} messages"), twins.real, twins.port, &mut failures);
        for (unit, name) in twins.both().into_iter().zip(saved_names) {
            for (index, word) in name.into_iter().enumerate() {
                save_to_memory(unit.0 + NAME_WORD + index as u32 * 4, word);
            }
        }
    
        twins.destroy();
    }
    if shown == 0 {
        failures.push("no message was shown".to_string());
    }
    take_log();
    STUBBING.store(false, Ordering::SeqCst);
    *TEST_THREAD.lock().unwrap() = None;
    finish_test(test_name, failures, failure_log)
}
