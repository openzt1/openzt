//! `ZTAnimal::save` against the real body: the byte stream and result over fixtures with planted state, covering both
//! strings (empty, short, at and past the `0x1000` limit), the escape record (absent, present, missing tile) and the
//! home gate tile (absent, present).

use openzt_detour::generated::{bfentity::GET_TILE, ztanimal::SAVE};

use super::ztanimal::{build_animal, live_animal_types, live_animals, Lcg};
use crate::{
    reimplementation_tests::{harness::finish_test, io_redirect},
    util::{get_from_memory, save_to_memory},
    ztanimal::{animal_offsets, live_support::real, ZTAnimal},
};

fn leak_bytes(bytes: &[u8]) -> u32 {
    Box::leak(bytes.to_vec().into_boxed_slice()).as_ptr() as u32
}

/// Fills the saved region of `animal` with seeded bytes and plants the strings, escape record and gate tile.
fn plant(animal: ZTAnimal, seed: u32, lengths: (u32, u32), record: Option<(u32, u32)>, has_flag: bool, gate: u32) {
    let mut rng = Lcg(seed);
    for offset in (0x288u32..0x3b4).filter(|o| !(0x26c..0x290).contains(o)) {
        if (0x384..0x388).contains(&offset) || (0x38c..0x390).contains(&offset) {
            continue;
        }
        save_to_memory(animal.0 + offset, (rng.next() & 0xff) as u8);
    }
    for ((begin_offset, end_offset), length) in [((0x26cu32, 0x270u32), lengths.0), ((0x278, 0x27c), lengths.1)] {
        let bytes: Vec<u8> = (0..length).map(|i| (i * 7 + seed) as u8).collect();
        let begin = leak_bytes(&bytes);
        save_to_memory(animal.0 + begin_offset, begin);
        save_to_memory(animal.0 + end_offset, begin + length);
    }
    save_to_memory(animal.0 + animal_offsets::HAS_ESCAPE_INFO, has_flag as u8);
    let record_address = {
        let memory = leak_bytes(&[0u8; 0x10]);
        for (slot, value) in [(1u32, rng.next() as u8), (2, 0), (3, 1)] {
            save_to_memory(memory + slot, value);
        }
        save_to_memory(memory + 0xc, rng.next());
        if let Some((first, second)) = record {
            save_to_memory(memory + 4, first);
            save_to_memory(memory + 8, second);
        }
        memory
    };
    save_to_memory(animal.0 + animal_offsets::ESCAPE_RECORD, record_address);
    save_to_memory(animal.0 + animal_offsets::HOME_GATE_TILE, gate);
}

/// `ZTANIMAL_SAVE_LIVE`: stream bytes and result of the real body, the port and the hooked address.
pub(crate) fn run_ztanimal_save_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_SAVE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(2);
    let tile = live_animals().first().map(|&a| unsafe { GET_TILE.original()(a as *const u32) } as u32).filter(|&t| t != 0);
    let (Some(&ty), Some(tile)) = (types.first(), tile) else {
        failures.push("the live world holds no animal type or placed animal".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let mut lengths_seen = std::collections::BTreeSet::new();
    let mut cases = 0;
    for (index, lengths) in [(0u32, 0u32), (3, 0), (0, 9), (0xfff, 5), (0x1000, 0), (4, 0x1000), (5000, 5000)].into_iter().enumerate() {
        for (record, has_flag) in [(None, false), (Some((tile, tile)), true), (Some((tile, tile)), false)] {
            for gate in [0u32, tile] {
                let seed = 100 + index as u32;
                let label = format!("lengths {lengths:?} record {record:?} flag {has_flag} gate {gate:#x}");
                let fixtures: Vec<ZTAnimal> = (0..3).map(|_| build_animal(ty)).collect();
                for animal in &fixtures {
                    plant(*animal, seed, lengths, record, has_flag, gate);
                }
                io_redirect::begin_capture();
                let real_ok = real::save(fixtures[0].0, std::ptr::null());
                let real_bytes = io_redirect::end_capture();
                io_redirect::begin_capture();
                let port_ok = fixtures[1].save(std::ptr::null());
                let port_bytes = io_redirect::end_capture();
                io_redirect::begin_capture();
                let hooked_ok = unsafe { SAVE.hooked()(fixtures[2].0 as *const u32, std::ptr::null()) };
                let hooked_bytes = io_redirect::end_capture();
                if real_ok != port_ok || real_ok != hooked_ok {
                    failures.push(format!("{label}: result real {real_ok}, port {port_ok}, hooked {hooked_ok}"));
                }
                if real_bytes != port_bytes || real_bytes != hooked_bytes {
                    failures.push(format!("{label}: bytes real {}, port {}, hooked {} (or contents differ)", real_bytes.len(), port_bytes.len(), hooked_bytes.len()));
                }
                lengths_seen.insert(real_bytes.len());
                cases += 1;
                let _ = get_from_memory::<u32>(fixtures[0].0);
            }
        }
    }
    if lengths_seen.len() < 6 {
        failures.push(format!("only {} distinct stream lengths occurred", lengths_seen.len()));
    }
    failures.truncate(40);
    finish_test(&format!("{test_name} ({cases} cases, {} stream lengths)", lengths_seen.len()), failures, failure_log)
}
