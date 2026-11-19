//! Compares real vanilla `BFEvent` (constructor/init/load/save) against its Rust reimplementation
//! (production file `openzt/src/bfevent.rs`) on standalone `0x1c`-byte buffers. `BFEvent` owns no heap
//! memory, so there is no allocator hazard: the "real" pole runs the pre-detour body through
//! `bfevent_live_support::real_*` (valid in every profile) and the "port" pole runs the address through
//! `.hooked()`, which lands in the Rust detour once `bfevent::init()` has installed it.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::bfevent::{CONSTRUCTOR, INIT, LOAD, SAVE};

use crate::bfevent::{live_support as bfevent_live_support, BFEVENT_SIZE};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::reimplementation_tests::io_redirect;
use crate::util::{get_from_memory, save_to_memory};

const WORDS: u32 = BFEVENT_SIZE / 4;

fn snapshot(buf: u32) -> [u32; WORDS as usize] {
    let mut out = [0u32; WORDS as usize];
    for (i, slot) in out.iter_mut().enumerate() {
        *slot = get_from_memory::<u32>(buf + (i as u32) * 4);
    }
    out
}

fn poison(buf: u32, seed: u32) {
    for i in 0..WORDS {
        save_to_memory(buf + i * 4, seed.wrapping_mul(0x0101_0101).wrapping_add(i * 0x1111_1111));
    }
}

/// Writes a recognisable, field-distinct payload over an already-constructed buffer.
fn fill_payload(buf: u32, seed: u32) {
    save_to_memory(buf + 0x08, 0x1000_0000 | seed);
    save_to_memory(buf + 0x0c, 0x2000_0000 | seed);
    save_to_memory(buf + 0x10, 0x3000_0000 | seed);
    save_to_memory(buf + 0x14, 0x4000u16 | seed as u16);
    save_to_memory(buf + 0x16, 0x2714u16);
    save_to_memory(buf + 0x18, 0x6000u16 | seed as u16);
    save_to_memory(buf + 0x1a, 0x70u8 | (seed as u8 & 0x0f));
    save_to_memory(buf + 0x1b, 0x80u8 | (seed as u8 & 0x0f));
}

fn diff(label: &str, real: &[u32], port: &[u32], failures: &mut Vec<String>) {
    for (i, (r, p)) in real.iter().zip(port.iter()).enumerate() {
        if r != p {
            failures.push(format!("{label}: word +{:#x} real {r:#010x} != port {p:#010x}", i * 4));
        }
    }
}

/// `BFEVENT_DETOURS_ENABLED` - wiring check; see the `*_DETOURS_ENABLED` tests for the other classes.
pub(crate) fn run_bfevent_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFEVENT_DETOURS_ENABLED";
    let disabled: Vec<&'static str> =
        bfevent_live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("detours not enabled: {disabled:?}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// `BFEVENT_CONSTRUCT_INIT` - constructor and `init` over differently poisoned buffers: every byte must
/// match, the constructor must return `this`, and `init` must leave both vtable words alone.
pub(crate) fn run_bfevent_construct_init_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFEVENT_CONSTRUCT_INIT";
    let mut failures: Vec<String> = Vec::new();

    for seed in [0u32, 1, 0x5a, 0xdead_beef] {
        let real = bfevent_live_support::standalone_buffer();
        let port = bfevent_live_support::standalone_buffer();
        poison(real, seed);
        poison(port, seed);

        let real_ret = bfevent_live_support::real_constructor(real as *const u32) as u32;
        let port_ret = unsafe { CONSTRUCTOR.hooked()(port as *const u32) } as u32;
        if real_ret != real {
            failures.push(format!("seed {seed:#x}: real constructor returned {real_ret:#x}, expected this {real:#x}"));
        }
        if port_ret != port {
            failures.push(format!("seed {seed:#x}: port constructor returned {port_ret:#x}, expected this {port:#x}"));
        }
        diff(&format!("construct seed {seed:#x}"), &snapshot(real), &snapshot(port), &mut failures);

        let real_vtables = (get_from_memory::<u32>(real), get_from_memory::<u32>(real + 4));
        poison(real, seed ^ 0xff);
        poison(port, seed ^ 0xff);
        save_to_memory(real, real_vtables.0);
        save_to_memory(real + 4, real_vtables.1);
        save_to_memory(port, real_vtables.0);
        save_to_memory(port + 4, real_vtables.1);

        bfevent_live_support::real_init(real as *const u32);
        unsafe { INIT.hooked()(port as *const u32) };
        let (real_after, port_after) = (snapshot(real), snapshot(port));
        diff(&format!("init seed {seed:#x}"), &real_after, &port_after, &mut failures);
        if real_after[0] != real_vtables.0 || real_after[1] != real_vtables.1 {
            failures.push(format!("seed {seed:#x}: init disturbed a vtable word"));
        }
    }

    finish_test(test_name, failures, failure_log)
}

/// `BFEVENT_SAVE_LOAD` - the serialized bytes from real and port `save` must be identical; real and port
/// `load` of those bytes into freshly constructed buffers must agree on the return value, every field and
/// the number of bytes consumed. Also covers the version gate (`<= 0x60` consumes nothing and succeeds) and
/// a truncated stream (both sides keep attempting every field and must fail identically).
pub(crate) fn run_bfevent_save_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFEVENT_SAVE_LOAD";
    let mut failures: Vec<String> = Vec::new();
    let dummy_file: u32 = 0;
    let file_ptr = &dummy_file as *const u32;
    const CURRENT_VERSION: u32 = 0x100;

    for seed in [0u32, 7, 0xabc, 0x00ff_ffff] {
        let source = bfevent_live_support::standalone_buffer();
        bfevent_live_support::real_constructor(source as *const u32);
        fill_payload(source, seed);

        io_redirect::begin_capture();
        let real_ret = bfevent_live_support::real_save(source as *const u32, file_ptr as *const i8);
        let real_bytes = io_redirect::end_capture();
        io_redirect::begin_capture();
        let port_ret = unsafe { SAVE.hooked()(source as *const u32, file_ptr as *const i8) };
        let port_bytes = io_redirect::end_capture();
        if !real_ret || !port_ret {
            failures.push(format!("seed {seed:#x}: save returned real={real_ret} port={port_ret}, expected both true"));
        }
        if real_bytes.len() != 0x14 {
            failures.push(format!("seed {seed:#x}: real save wrote {} bytes, expected 0x14", real_bytes.len()));
        }
        if real_bytes != port_bytes {
            failures.push(format!("seed {seed:#x}: save byte streams differ: real {real_bytes:02x?} port {port_bytes:02x?}"));
        }

        let real_target = bfevent_live_support::standalone_buffer();
        let port_target = bfevent_live_support::standalone_buffer();
        bfevent_live_support::real_constructor(real_target as *const u32);
        bfevent_live_support::real_constructor(port_target as *const u32);

        io_redirect::begin_replay(real_bytes.clone());
        let real_load = bfevent_live_support::real_load(real_target as *const u32, file_ptr, CURRENT_VERSION);
        let real_pos = io_redirect::replay_position();
        io_redirect::end_replay();
        io_redirect::begin_replay(real_bytes.clone());
        let port_load = unsafe { LOAD.hooked()(port_target as *const u32, file_ptr, CURRENT_VERSION) };
        let port_pos = io_redirect::replay_position();
        io_redirect::end_replay();

        if !real_load || !port_load {
            failures.push(format!("seed {seed:#x}: load returned real={real_load} port={port_load}, expected both true"));
        }
        if real_pos != port_pos || real_pos != real_bytes.len() {
            failures.push(format!("seed {seed:#x}: bytes consumed real={real_pos} port={port_pos}, stream len {}", real_bytes.len()));
        }
        diff(&format!("load seed {seed:#x}"), &snapshot(real_target), &snapshot(port_target), &mut failures);
        diff(&format!("roundtrip seed {seed:#x}"), &snapshot(source), &snapshot(port_target), &mut failures);

        // Version gate: an old save carries no payload.
        let old_real = bfevent_live_support::standalone_buffer();
        let old_port = bfevent_live_support::standalone_buffer();
        bfevent_live_support::real_constructor(old_real as *const u32);
        bfevent_live_support::real_constructor(old_port as *const u32);
        let untouched = snapshot(old_real);
        io_redirect::begin_replay(real_bytes.clone());
        let real_old = bfevent_live_support::real_load(old_real as *const u32, file_ptr, 0x60);
        let real_old_pos = io_redirect::replay_position();
        io_redirect::end_replay();
        io_redirect::begin_replay(real_bytes.clone());
        let port_old = unsafe { LOAD.hooked()(old_port as *const u32, file_ptr, 0x60) };
        let port_old_pos = io_redirect::replay_position();
        io_redirect::end_replay();
        if !real_old || !port_old || real_old_pos != 0 || port_old_pos != 0 {
            failures.push(format!(
                "seed {seed:#x}: version 0x60 load real=({real_old}, pos {real_old_pos}) port=({port_old}, pos {port_old_pos}), expected (true, 0)"
            ));
        }
        diff(&format!("old-version real untouched seed {seed:#x}"), &untouched, &snapshot(old_real), &mut failures);
        diff(&format!("old-version port untouched seed {seed:#x}"), &untouched, &snapshot(old_port), &mut failures);

        // Truncated stream: 13 of 20 bytes. Every field is still attempted, so a late 1-byte field can
        // succeed after an earlier 2-byte one fails; both sides must land on the same state.
        let trunc_real = bfevent_live_support::standalone_buffer();
        let trunc_port = bfevent_live_support::standalone_buffer();
        bfevent_live_support::real_constructor(trunc_real as *const u32);
        bfevent_live_support::real_constructor(trunc_port as *const u32);
        let truncated = real_bytes[..13].to_vec();
        io_redirect::begin_replay(truncated.clone());
        let real_trunc = bfevent_live_support::real_load(trunc_real as *const u32, file_ptr, CURRENT_VERSION);
        let real_trunc_pos = io_redirect::replay_position();
        io_redirect::end_replay();
        io_redirect::begin_replay(truncated);
        let port_trunc = unsafe { LOAD.hooked()(trunc_port as *const u32, file_ptr, CURRENT_VERSION) };
        let port_trunc_pos = io_redirect::replay_position();
        io_redirect::end_replay();
        if real_trunc || port_trunc {
            failures.push(format!("seed {seed:#x}: truncated load returned real={real_trunc} port={port_trunc}, expected both false"));
        }
        if real_trunc_pos != port_trunc_pos {
            failures.push(format!("seed {seed:#x}: truncated load consumed real={real_trunc_pos} port={port_trunc_pos}"));
        }
        diff(&format!("truncated seed {seed:#x}"), &snapshot(trunc_real), &snapshot(trunc_port), &mut failures);
    }

    finish_test(test_name, failures, failure_log)
}
