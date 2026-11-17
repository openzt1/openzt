//! Compares real vanilla `BFEntity::save` (`0x477998`) and `BFEntity::load` (`0x44f866`) against their Rust
//! reimplementation (production file `openzt/src/bfentity.rs`). Both go through the in-memory file redirect, so
//! `save`'s bytes are captured and `load` replays the same bytes into a real-pole and a port-pole entity. The
//! entities come from the real constructor around the behavior-set tests' fake type.

use crate::bfbehaviorset::live_support as bfbehaviorset_live_support;
use crate::bfentity::live_support as bfentity_live_support;
use crate::reimplementation_tests::harness::finish_test;
use crate::reimplementation_tests::io_redirect;
use crate::reimplementation_tests::tests::bfbehaviorset::{build_fake_type, SCRIPT_LINES, SCRIPT_RESULT};
use crate::reimplementation_tests::tests::bfentity::{build_set_bs_entity, normalised_set, pool_buffer, write_words};
use crate::util::get_from_memory;
use openzt_detour::generated::{
    bfbehaviorset::SAVE as SET_SAVE,
    bfentity::{LOAD, SAVE},
    msvc_std_basic_string::ASSIGN_RANGE,
};
use std::sync::atomic::Ordering;

const GLOBAL_ZTAIMGR: u32 = 0x0063_8098;
/// The counter that numbers entities of saves older than `0xb`.
const ID_COUNTER: u32 = 0x0063_8738;

/// Distinct values for every saved scalar, so a reordered or misplaced field changes the bytes.
fn fill_save_scalars(entity: u32) {
    write_words(entity + 0x114, &[0x201, 0x302, 0x403]);
    write_words(entity + 0x12c, &[0x5a5a_0007]);
    write_words(entity + 0x124, &[0x1234_5678]);
    write_words(entity + 0x138, &[0xdead_0138]);
    unsafe {
        ((entity + 0x13f) as *mut u8).write(0x3f);
        ((entity + 0x13c) as *mut u8).write(0x3c);
    }
}

/// Assigns `name` to the string at `+0x108` and gives the `+0xfc` buffer `extra` as its contents.
fn fill_buffers(entity: u32, name: &[u8], extra: &[u8]) {
    let range = name.as_ptr_range();
    unsafe { ASSIGN_RANGE.original()((entity + 0x108) as *const std::ffi::c_void, range.start as *const u32, range.end, std::ptr::null()) };
    if !extra.is_empty() {
        let buffer = pool_buffer(extra.len() as u32);
        for (i, byte) in extra.iter().enumerate() {
            unsafe { ((buffer + i as u32) as *mut u8).write(*byte) };
        }
        let end = buffer + extra.len() as u32;
        write_words(entity + 0xfc, &[buffer, end, end]);
    }
}

fn capture_bytes(run: impl FnOnce() -> bool) -> (bool, Vec<u8>) {
    io_redirect::begin_capture();
    let ok = run();
    (ok, io_redirect::end_capture())
}

/// `BFENTITY_SAVE` - the bytes `save` writes and its result against vanilla, for entities with and without a
/// populated set and child, name string, and `+0xfc` buffer, plus a name too long to save.
pub(crate) fn run_bfentity_save_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFENTITY_SAVE";
    let mut failures: Vec<String> = Vec::new();
    let type_object = build_fake_type();
    let long_name = vec![b'n'; 0x1000];
    let cases: [(&str, &[u8], &[u8], bool); 6] = [
        ("empty", b"", b"", false),
        ("named", b"Elephant", b"", false),
        ("extra bytes", b"", b"\x01\x02\x03\x04\x05", false),
        ("everything", b"Giraffe", b"\x09\x08\x07", true),
        ("long name", &long_name, b"\x01", false),
        ("just under the limit", &long_name[..0xfff], b"", true),
    ];
    for (label, name, extra, with_child) in cases {
        let build = || {
            let entity = build_set_bs_entity(type_object, with_child, [0, 0, 0, 0, 0]);
            fill_save_scalars(entity);
            fill_buffers(entity, name, extra);
            entity
        };
        let (real, port) = (build(), build());
        let (real_ok, real_bytes) = capture_bytes(|| bfentity_live_support::real::save(real, std::ptr::null()));
        let (port_ok, port_bytes) = capture_bytes(|| unsafe { SAVE.hooked()(port as *const u32, std::ptr::null()) });
        if real_ok != port_ok {
            failures.push(format!("{label}: result real={real_ok} port={port_ok}"));
        }
        if real_bytes != port_bytes {
            failures.push(format!("{label}: bytes differ (real {} bytes, port {} bytes)", real_bytes.len(), port_bytes.len()));
        }
        if label == "long name" && real_ok {
            failures.push(format!("{label}: vanilla saved a name of 0x1000 bytes"));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// The bytes `load` reads for a version, in file order.
struct LoadStream {
    name: &'static [u8],
    legacy: &'static [u8],
    extra: Vec<u8>,
    set_bytes: Vec<u8>,
}

impl LoadStream {
    fn bytes(&self, version: u32) -> Vec<u8> {
        let mut out = Vec::new();
        for word in [0x201u32, 0x302, 0x403] {
            out.extend_from_slice(&word.to_le_bytes());
        }
        out.extend_from_slice(&0x5a5a_0007u32.to_le_bytes());
        if version >= 7 {
            out.extend_from_slice(&0x1234_5678u32.to_le_bytes());
        }
        out.extend_from_slice(&(self.name.len() as u32).to_le_bytes());
        out.extend_from_slice(self.name);
        if version < 0x3f {
            out.extend_from_slice(&(self.legacy.len() as u32).to_le_bytes());
            out.extend_from_slice(self.legacy);
        }
        if version >= 0x23 {
            out.push(0x3f);
            out.extend_from_slice(&0u32.to_le_bytes());
        }
        if version >= 0x3a {
            out.push(0x3c);
        }
        if version >= 0x3f {
            out.extend_from_slice(&self.set_bytes);
        }
        if version >= 0x43 {
            out.extend_from_slice(&(self.extra.len() as u32).to_le_bytes());
            out.extend_from_slice(&self.extra);
        }
        out
    }
}

/// Everything `load` writes that the poles must agree on.
#[derive(Debug, PartialEq, Eq)]
struct LoadState {
    position: [u32; 4],
    scalars: [u32; 5],
    name: Vec<u8>,
    extra: Vec<u8>,
    flags: [u8; 3],
    set: crate::bfbehaviorset::live_support::SetSnapshot,
    counter_delta: u32,
}

fn load_state(entity: u32, counter_delta: u32) -> LoadState {
    let word = |offset: u32| get_from_memory::<u32>(entity + offset);
    let name_begin = word(0x108);
    let extra_begin = word(0xfc);
    LoadState {
        position: [word(0x114), word(0x118), word(0x11c), word(0x120)],
        scalars: [word(0x12c), word(0x124), word(0x148), word(0x14c), word(0x138)],
        name: (0..word(0x10c).wrapping_sub(name_begin)).map(|i| get_from_memory::<u8>(name_begin + i)).collect(),
        extra: (0..word(0x100).wrapping_sub(extra_begin)).map(|i| get_from_memory::<u8>(extra_begin + i)).collect(),
        flags: [get_from_memory::<u8>(entity + 0x13c), get_from_memory::<u8>(entity + 0x13f), get_from_memory::<u8>(entity + 0xf9)],
        set: normalised_set(entity),
        counter_delta,
    }
}

/// `BFENTITY_LOAD` - `load` against vanilla for every version boundary, with and without a legacy behavior name
/// (including the renamed `bAvoidPredator`), a `+0xfc` buffer shorter and longer than the one already present,
/// and every truncation of two complete streams. Compares the result, the bytes consumed, the loaded fields, the
/// set and the entity-id counter.
pub(crate) fn run_bfentity_load_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFENTITY_LOAD";
    let mut failures: Vec<String> = Vec::new();
    if get_from_memory::<u32>(GLOBAL_ZTAIMGR) == 0 {
        failures.push("GLOBAL_ZTAIMgr is null".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let type_object = build_fake_type();
    *SCRIPT_LINES.lock().unwrap() = vec![b"fA(1)\0", b"fB()\0"];
    SCRIPT_RESULT.store(1, Ordering::SeqCst);

    let set_bytes = {
        let set = bfbehaviorset_live_support::build_set(b"walk", b"fA(1)", &[b"fA(1)", b"fB()"], &[b"child"], 0);
        capture_bytes(|| unsafe { SET_SAVE.hooked()(set as *const u32, std::ptr::null()) }).1
    };

    let run = |label: &str, version: u32, stream: &LoadStream, truncate: Option<usize>, failures: &mut Vec<String>| {
        let mut bytes = stream.bytes(version);
        if let Some(length) = truncate {
            bytes.truncate(length);
        }
        let build = || {
            let entity = build_set_bs_entity(type_object, false, [0, 1, 0, 0, 0]);
            fill_buffers(entity, b"", b"\xaa\xbb\xcc\xdd");
            entity
        };
        let (real, port) = (build(), build());

        // Both poles start from the same counter value, so renumbered ids can be compared directly.
        write_words(ID_COUNTER, &[1000]);
        let before = get_from_memory::<u32>(ID_COUNTER);
        io_redirect::begin_replay(bytes.clone());
        let real_ok = bfentity_live_support::real::load(real, std::ptr::null(), version);
        let real_used = io_redirect::replay_position();
        io_redirect::end_replay();
        let middle = get_from_memory::<u32>(ID_COUNTER);
        write_words(ID_COUNTER, &[1000]);
        io_redirect::begin_replay(bytes);
        let port_ok = unsafe { LOAD.hooked()(port as *const u32, std::ptr::null(), version) };
        let port_used = io_redirect::replay_position();
        io_redirect::end_replay();
        let after = get_from_memory::<u32>(ID_COUNTER);

        let label = format!("{label} v={version:#x} truncate={truncate:?}");
        if real_ok != port_ok {
            failures.push(format!("{label}: result real={real_ok} port={port_ok}"));
        }
        if real_used != port_used {
            failures.push(format!("{label}: bytes consumed real={real_used} port={port_used}"));
        }
        let (real_state, port_state) = (load_state(real, middle.wrapping_sub(before)), load_state(port, after.wrapping_sub(1000)));
        if real_state != port_state {
            failures.push(format!("{label}: state real={real_state:?} port={port_state:?}"));
        }
    };

    let versions = [6u32, 7, 0xa, 0xb, 0x16, 0x17, 0x22, 0x23, 0x39, 0x3a, 0x3e, 0x3f, 0x42, 0x43];
    let streams = [
        ("plain", LoadStream { name: b"Tapir", legacy: b"", extra: vec![], set_bytes: set_bytes.clone() }),
        ("legacy", LoadStream { name: b"", legacy: b"walk", extra: vec![1, 2], set_bytes: set_bytes.clone() }),
        ("predator", LoadStream { name: b"Okapi", legacy: b"bAvoidPredator", extra: vec![1, 2, 3, 4, 5, 6], set_bytes: set_bytes.clone() }),
    ];
    for (label, stream) in &streams {
        for version in versions {
            run(label, version, stream, None, &mut failures);
        }
    }
    for version in [0x10u32, 0x43] {
        let stream = &streams[2].1;
        for length in 0..stream.bytes(version).len() {
            run("truncated", version, stream, Some(length), &mut failures);
        }
    }

    SCRIPT_LINES.lock().unwrap().clear();
    finish_test(test_name, failures, failure_log)
}
