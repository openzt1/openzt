//! Live check for `ztguest.rs`'s `initStatusVars`. Three poles (real body, port, hooked address) run on identically
//! prepared fixtures with the shared RNG pinned and the CRT `rand()` scripted; the process-wide range caches the method
//! reads and writes are saved and restored around every case.

use openzt_detour::generated::ztguest::INIT_STATUS_VARS;

use super::{
    ztguest_leaf::compare_guests,
    ztguest_lifecycle::container_contents,
    ztguide::{base, pin_rng, rng},
    ztguide_habitats::{install_rand_hook, rand_calls, script_rand, set_rand_intercept},
};
use crate::{
    reimplementation_tests::harness::finish_test,
    util::{get_from_memory, save_to_memory},
    ztguest::{
        guest_offsets,
        live_support::{self, real},
    },
};

/// `DAT_00639114` (the one-in-N chance), the four variation ranges (low, high) as ints.
const STATUS_INTS: [u32; 9] = [0x0023_9114, 0x0023_e2ac, 0x0023_e2b0, 0x0023_e2b8, 0x0023_e2bc, 0x0023_e2c4, 0x0023_e2c8, 0x0023_e2cc, 0x0023_e2d0];
/// `DAT_0063e296` (ranges ready) and the four per-need flag bytes.
const STATUS_BYTES: [u32; 5] = [0x0023_e296, 0x0023_e2b4, 0x0023_e2c0, 0x0023_e2a8, 0x0023_e2a9];
/// Per need: the range's low and high globals, the flag byte, and the guest type's start and end fields.
const RANGES: [(u32, u32, u32, u32, u32); 4] = [
    (0x0023_e2ac, 0x0023_e2b0, 0x0023_e2b4, 0x1f4, 0x1f0),
    (0x0023_e2b8, 0x0023_e2bc, 0x0023_e2c0, 0x1fc, 0x1f8),
    (0x0023_e2c4, 0x0023_e2c8, 0x0023_e2a8, 0x204, 0x200),
    (0x0023_e2cc, 0x0023_e2d0, 0x0023_e2a9, 0x20c, 0x208),
];

fn status_snapshot() -> Vec<u32> {
    let mut values: Vec<u32> = STATUS_INTS.iter().map(|rva| get_from_memory::<u32>(base() + rva)).collect();
    values.extend(STATUS_BYTES.iter().map(|rva| get_from_memory::<u8>(base() + rva) as u32));
    values
}

fn status_restore(values: &[u32]) {
    for (rva, value) in STATUS_INTS.iter().zip(values) {
        save_to_memory(base() + rva, *value);
    }
    for (rva, value) in STATUS_BYTES.iter().zip(&values[STATUS_INTS.len()..]) {
        save_to_memory(base() + rva, *value as u8);
    }
}

/// `ZTGUEST_INIT_STATUS_VARS_LIVE`: `initStatusVars` over the one-in-N chance, the ranges-ready flag (cached and
/// recomputed from the type), the per-need flag bytes, the scripted `rand()` value and several RNG seeds. Whole-guest
/// memory, container contents, the process-wide values it writes, the shared RNG state and the number of `rand()` calls
/// must agree between the poles.
pub(crate) fn run_ztguest_init_status_vars_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTGUEST_INIT_STATUS_VARS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(guest_type) = live_support::find_guest_type() else {
        failures.push("the live world holds no guest type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    if let Err(message) = install_rand_hook() {
        failures.push(message);
        return finish_test(test_name, failures, failure_log);
    }
    let saved_statics = status_snapshot();
    let saved_rng = rng();
    let type_int = |offset: u32| get_from_memory::<i32>(guest_type + offset);
    let shared = [guest_type];
    let mut varied_needs = std::collections::BTreeSet::new();
    let mut cases = 0usize;

    'sweep: for one_in in [0i32, 1, 2, 7] {
        for ready in [0u8, 1] {
            for flag_bits in [0u8, 0b1111, 0b0101, 0b1010] {
                for seed in [1u32, 0x1234_5678, 0xdead_beef, 77, 0x0bad_f00d, 3, 5, 9] {
                    for rand_value in [0u32, 17, 12_345, 32_767] {
                        let label = format!("initStatusVars one_in {one_in} ready {ready} flags {flag_bits:#06b} seed {seed:#x} rand {rand_value}");
                        let prepare_statics = || {
                            save_to_memory(base() + 0x0023_9114, one_in);
                            save_to_memory(base() + 0x0023_e296, ready);
                            for (index, (low, high, flag, start, end)) in RANGES.iter().enumerate() {
                                save_to_memory(base() + low, type_int(*start));
                                save_to_memory(base() + high, type_int(*end));
                                save_to_memory(base() + flag, (flag_bits >> index) & 1);
                            }
                        };
                        let mut outcomes = Vec::new();
                        let mut fixtures = Vec::new();
                        for pole in 0..3usize {
                            let guest = live_support::build_standalone_guest(guest_type);
                            prepare_statics();
                            script_rand(&[rand_value]);
                            set_rand_intercept(true);
                            pin_rng(seed);
                            match pole {
                                0 => real::init_status_vars(guest.0),
                                1 => guest.init_status_vars(),
                                _ => unsafe { INIT_STATUS_VARS.hooked()(guest.0 as *const u32) },
                            }
                            set_rand_intercept(false);
                            outcomes.push((status_snapshot(), rng(), rand_calls()));
                            fixtures.push(guest);
                        }
                        if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                            failures.push(format!("{label}: (statics, RNG, rand calls) real {:x?}, port {:x?}, hooked {:x?}", outcomes[0], outcomes[1], outcomes[2]));
                        }
                        compare_guests(&label, fixtures[0], fixtures[1], &shared, &mut failures);
                        compare_guests(&format!("{label} hooked"), fixtures[0], fixtures[2], &shared, &mut failures);
                        let reference = container_contents(fixtures[0]);
                        if reference != container_contents(fixtures[1]) || reference != container_contents(fixtures[2]) {
                            failures.push(format!("{label}: container contents differ"));
                        }
                        let levels = [guest_offsets::HUNGER, guest_offsets::THIRST, guest_offsets::ENERGY, guest_offsets::BATHROOM];
                        for (need, offset) in levels.into_iter().enumerate() {
                            if get_from_memory::<i32>(fixtures[0].0 + offset) != type_int(RANGES[need].3) {
                                varied_needs.insert(need);
                            }
                        }
                        cases += 1;
                        if failures.len() > 12 {
                            break 'sweep;
                        }
                    }
                }
            }
        }
    }
    status_restore(&saved_statics);
    set_rand_intercept(false);
    pin_rng(saved_rng);
    if failures.is_empty() && varied_needs.len() < 4 {
        failures.push(format!("only the needs {varied_needs:?} were ever varied over {cases} cases"));
    }
    finish_test(test_name, failures, failure_log)
}
