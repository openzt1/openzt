//! Compares real vanilla `BFEntity::BFEntity` (`0x412693`) against its Rust reimplementation (production file
//! `openzt/src/bfentity.rs`) on standalone `0x154`-byte buffers. The "real" pole runs the pre-detour body via
//! `bfentity_live_support::real`, the "port" pole runs the address through `.hooked()`. Buffers are leaked: the
//! strings, vectors and the mixer's containers inside belong to vanilla's allocator and are never freed from Rust.

use std::io::Write;

use tracing::error;

use openzt_detour::generated::{
    bfentity::{CONSTRUCTOR, DESTRUCTOR_0, SET_BS},
    msvc_std_dequegxmixerchain::{DESTRUCTOR_0X41DECC, INIT_0X40E4D6},
    poolalloc::{ALLOCATE as POOL_ALLOCATE, DEALLOCATE as POOL_DEALLOCATE},
};

use crate::bfbehaviorset::live_support as bfbehaviorset_live_support;
use crate::bfentity::{live_support as bfentity_live_support, BFENTITY_SIZE};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};
use crate::util::get_from_memory;

/// Byte ranges `[start, end)` holding heap pointers that legitimately differ between the poles: the embedded
/// set's call object and strings, the mixer, the `BFBSCall` and the byte vector at `+0x108`.
const HEAP_RANGES: [(usize, usize); 4] = [(0x20, 0x3c), (0x48, 0x60), (0x74, 0xec), (0x108, 0x114)];

fn is_heap_pointer(word: u32, fill: u32) -> bool {
    word >= 0x10000 && word != fill
}

fn compare_words(label: &str, real: u32, port: u32, fill: u32, failures: &mut Vec<String>) {
    for offset in (0..BFENTITY_SIZE).step_by(4) {
        let (r, p) = (get_from_memory::<u32>(real + offset as u32), get_from_memory::<u32>(port + offset as u32));
        if r == p {
            continue;
        }
        let in_heap_range = HEAP_RANGES.iter().any(|&(start, end)| offset >= start && offset < end);
        if !(in_heap_range && is_heap_pointer(r, fill) && is_heap_pointer(p, fill)) {
            failures.push(format!("{label}: +{offset:#x} real={r:#x} port={p:#x}"));
        }
    }
}

/// `BFENTITY_DETOURS_ENABLED` - wiring check.
pub(crate) fn run_bfentity_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFENTITY_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = bfentity_live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
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

/// `BFENTITY_CONSTRUCTOR` - every word of the object, with clean and dirty memory and two entity types. Heap
/// pointers are compared by contents through the embedded set's snapshot and the byte vector's first byte.
pub(crate) fn run_bfentity_constructor_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFENTITY_CONSTRUCTOR";
    let mut failures: Vec<String> = Vec::new();

    for fill in [0u32, 0xcdcd_cdcd] {
        for entity_type in [0u32, 0x1234_5678] {
            let alloc = || Box::leak(vec![fill; BFENTITY_SIZE / 4].into_boxed_slice()).as_mut_ptr() as u32;
            let (real, port) = (alloc(), alloc());
            let real_ret = bfentity_live_support::real::constructor(real, entity_type);
            let port_ret = unsafe { CONSTRUCTOR.hooked()(port as *const u32, entity_type) } as u32;
            let label = format!("fill={fill:#x} type={entity_type:#x}");
            if real_ret != real || port_ret != port {
                failures.push(format!("{label}: did not return this (real {real_ret:#x} for {real:#x}, port {port_ret:#x} for {port:#x})"));
            }
            compare_words(&label, real, port, fill, &mut failures);

            let (real_set, port_set) = (bfbehaviorset_live_support::snapshot(real + 0x14), bfbehaviorset_live_support::snapshot(port + 0x14));
            if real_set != port_set {
                failures.push(format!("{label}: embedded set real={real_set:?} port={port_set:?}"));
            }
            for (pole, base) in [("real", real), ("port", port)] {
                let first = get_from_memory::<u32>(base + 0x10c);
                if first < 0x10000 || get_from_memory::<u8>(first) != 0 {
                    failures.push(format!("{label}: {pole} +0x108 vector's first byte is not 0 (data {first:#x})"));
                }
            }
        }
    }

    finish_test(test_name, failures, failure_log)
}

const POOL_FREELISTS: u32 = 0x0063_8000;
const POOL_BUCKETS: u32 = 16;

/// Length of each size class's freelist (sizes up to 128 bytes), so both poles' frees can be compared.
pub(crate) fn pool_lengths() -> Vec<u32> {
    (0..POOL_BUCKETS)
        .map(|bucket| {
            let mut node = get_from_memory::<u32>(POOL_FREELISTS + bucket * 4);
            let mut length = 0;
            while node != 0 && length < 1_000_000 {
                length += 1;
                node = get_from_memory::<u32>(node);
            }
            length
        })
        .collect()
}

pub(crate) fn pool_buffer(size: u32) -> u32 {
    unsafe { POOL_ALLOCATE.original()(size) as u32 }
}

/// Leaves every size class's freelist with spare blocks, so a measured run never triggers the pool's chunk
/// refill and each class's length changes by exactly its frees minus its allocations.
pub(crate) fn warm_pool() {
    for bucket in 0..POOL_BUCKETS {
        let size = (bucket + 1) * 8;
        let blocks: Vec<u32> = (0..64).map(|_| pool_buffer(size)).collect();
        for block in blocks {
            unsafe { POOL_DEALLOCATE.original()(block as *const u32, size) };
        }
    }
}

pub(crate) fn write_words(address: u32, words: &[u32]) {
    for (i, word) in words.iter().enumerate() {
        unsafe { ((address + 4 * i as u32) as *mut u32).write(*word) };
    }
}

struct Fixture {
    entity: u32,
    /// Fake reference-counted objects the mixer entries point at, each with count 5 at `+0x20`.
    links: Vec<u32>,
}

/// Elements per block of the mixer's `deque<GXMixerChain>`.
const DEQUE_BLOCK_ELEMENTS: u32 = 42;

/// Replaces the mixer's empty deque with one holding `chains` chains of two links each, so that `0x411cad` walks
/// elements across block boundaries (blocks `0x1f8` bytes, 42 twelve-byte elements). Each link points at a fake
/// reference-counted object (count 5 at `+0x20`), appended to `links`.
fn populate_mixer_deque(mixer: u32, chains: usize, links: &mut Vec<u32>) {
    let deque = mixer + 0x18;
    unsafe {
        DESTRUCTOR_0X41DECC.original()(deque as *const u32);
        INIT_0X40E4D6.original()(deque as *const u32, chains as u32);
    }
    let map_node = get_from_memory::<u32>(deque + 0xc);
    for chain in 0..chains as u32 {
        let block = get_from_memory::<u32>(map_node + 4 * (chain / DEQUE_BLOCK_ELEMENTS));
        let element = block + (chain % DEQUE_BLOCK_ELEMENTS) * 12;
        let objects: Vec<u32> = (0..2).map(|_| Box::leak(Box::new([0u32; 16])).as_mut_ptr() as u32).collect();
        for object in &objects {
            write_words(object + 0x20, &[5]);
        }
        let buffer = pool_buffer(24);
        write_words(buffer, &[objects[0], 1, 2, objects[1], 3, 4]);
        write_words(element, &[buffer, buffer + 24, buffer + 24]);
        links.extend(objects);
    }
}

/// A vanilla-constructed entity; when `populated`, its name stack, call list, parsed call, event vector and both
/// mixer vectors hold pool-allocated data, `with_child` adds a child set, and `chains` fills the mixer's deque.
fn build_entity(populated: bool, with_child: bool, chains: usize) -> Fixture {
    let entity = Box::leak(vec![0xcdcd_cdcdu32; BFENTITY_SIZE / 4].into_boxed_slice()).as_mut_ptr() as u32;
    bfentity_live_support::real::constructor(entity, 0x1234_5678);
    let mut links = Vec::new();
    if populated {
        let set = entity + 0x14;
        bfbehaviorset_live_support::push_stack(set, b"parent");
        bfbehaviorset_live_support::push_list(set, b"fWalk(1)");
        bfbehaviorset_live_support::push_list(set, b"fRun(2, 3)");
        bfbehaviorset_live_support::prime_call(set, "fWalk(1, 2)");
        if with_child {
            bfbehaviorset_live_support::push_stack(set, b"child");
            let child = bfbehaviorset_live_support::build_set(b"child", b"fKid(1)", &[b"fKid(1)"], &[], 0);
            write_words(set + 0x4c, &[child]);
        }

        let events = pool_buffer(56);
        write_words(entity + 4, &[events, events + 0x1c, events + 56]);

        for vector in [entity + 0x74, entity + 0x80] {
            let objects: Vec<u32> = (0..2).map(|_| Box::leak(Box::new([0u32; 16])).as_mut_ptr() as u32).collect();
            for object in &objects {
                write_words(object + 0x20, &[5]);
            }
            let buffer = pool_buffer(40);
            write_words(buffer, &[objects[0], 1, 2, 0, 0, 0, objects[1], 3, 4]);
            write_words(vector, &[buffer, buffer + 36, buffer + 40]);
            links.extend(objects);
        }
    }
    if chains > 0 {
        populate_mixer_deque(entity + 0x74, chains, &mut links);
    }
    Fixture { entity, links }
}

/// `BFENTITY_DESTRUCTOR` - the destructor against vanilla on constructed entities: every word of the object,
/// the reference counts the mixer entries drop, and how many blocks each size class's freelist gains.
pub(crate) fn run_bfentity_destructor_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "BFENTITY_DESTRUCTOR";
    let mut failures: Vec<String> = Vec::new();

    let cases = [
        ("bare", false, false, 0),
        ("populated", true, false, 0),
        ("with child", true, true, 0),
        ("one chain", true, false, 1),
        ("deque across two blocks", true, true, 50),
        ("deque ending on a block boundary", true, false, 84),
    ];
    for (label, populated, with_child, chains) in cases {
        let (real, port) = (build_entity(populated, with_child, chains), build_entity(populated, with_child, chains));

        warm_pool();
        let before = pool_lengths();
        bfentity_live_support::real::destructor(real.entity);
        let middle = pool_lengths();
        unsafe { DESTRUCTOR_0.hooked()(port.entity as *const u32) };
        let after = pool_lengths();

        let real_delta: Vec<u32> = middle.iter().zip(&before).map(|(a, b)| a.wrapping_sub(*b)).collect();
        let port_delta: Vec<u32> = after.iter().zip(&middle).map(|(a, b)| a.wrapping_sub(*b)).collect();
        if real_delta != port_delta {
            failures.push(format!("{label}: freelist growth real={real_delta:?} port={port_delta:?}"));
        }
        if populated && real_delta.iter().all(|delta| *delta == 0) {
            failures.push(format!("{label}: vanilla freed nothing, the fixture is not exercising the teardown"));
        }

        let counts = |fixture: &Fixture| fixture.links.iter().map(|link| get_from_memory::<u32>(link + 0x20)).collect::<Vec<_>>();
        let (real_counts, port_counts) = (counts(&real), counts(&port));
        if real_counts != port_counts {
            failures.push(format!("{label}: reference counts real={real_counts:?} port={port_counts:?}"));
        }
        if chains > 0 && real_counts.len() < 2 * chains {
            failures.push(format!("{label}: fixture lost its deque links ({} counts)", real_counts.len()));
        }
        if populated && real_counts.iter().any(|count| *count != 4) {
            failures.push(format!("{label}: vanilla did not drop every count to 4: {real_counts:?}"));
        }

        for offset in (0..BFENTITY_SIZE).step_by(4) {
            let (r, p) = (get_from_memory::<u32>(real.entity + offset as u32), get_from_memory::<u32>(port.entity + offset as u32));
            let both_heap = r >= 0x0100_0000 && p >= 0x0100_0000 && r != 0xcdcd_cdcd && p != 0xcdcd_cdcd;
            if r != p && !both_heap {
                failures.push(format!("{label}: +{offset:#x} real={r:#x} port={p:#x}"));
            }
        }
    }

    finish_test(test_name, failures, failure_log)
}

/// An entity built by the real constructor around the fake type from the behavior-set tests (so `load` finds its
/// scripts), bound to its own embedded set, which holds a name, a call list, a parsed call, a name stack and
/// optionally a child; `flags` are the set's `+0x58..+0x5c` bytes.
pub(crate) fn build_set_bs_entity(type_object: u32, with_child: bool, flags: [u8; 5]) -> u32 {
    let entity = Box::leak(vec![0xcdcd_cdcdu32; BFENTITY_SIZE / 4].into_boxed_slice()).as_mut_ptr() as u32;
    bfentity_live_support::real::constructor(entity, type_object);
    let set = entity + 0x14;
    bfbehaviorset_live_support::bind(set, entity);
    bfbehaviorset_live_support::push_stack(set, b"parent");
    bfbehaviorset_live_support::push_list(set, b"fOld(1)");
    bfbehaviorset_live_support::prime_call(set, "fWalk(1, 2)");
    let old_name = crate::vanilla_string::VanillaString::new("oldName");
    bfbehaviorset_live_support::real::set_name(set, old_name.as_ptr() as *const i32);
    if with_child {
        bfbehaviorset_live_support::push_stack(set, b"child");
        let child = bfbehaviorset_live_support::build_set(b"child", b"fKid(1)", &[b"fKid(1)"], &[], entity);
        write_words(set + 0x4c, &[child]);
    }
    bfbehaviorset_live_support::set_flags(set, flags);
    entity
}

/// A vanilla string handed over by value: its three words, with ownership passing to the callee, which frees it.
fn string_words(value: &str) -> [u32; 3] {
    let string = crate::vanilla_string::VanillaString::new(value);
    let base = string.as_ptr() as u32;
    let words = [get_from_memory::<u32>(base), get_from_memory::<u32>(base + 4), get_from_memory::<u32>(base + 8)];
    std::mem::forget(string);
    words
}

/// `BFENTITY_SET_BS` - `setBS` against vanilla: the gate (finished unforced set, running set, forced), an empty
/// and a real name, every clear mode, scripts that load, come back empty or fail, and a child set to tear down.
/// Compares the return value, the embedded set's state, the flag bytes at `+0xf8`/`+0xf9` and the freelist growth.
pub(crate) fn run_bfentity_set_bs_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::tests::bfbehaviorset::{build_fake_type, SCRIPT_LINES, SCRIPT_RESULT};
    use std::sync::atomic::Ordering;

    let test_name = "BFENTITY_SET_BS";
    let mut failures: Vec<String> = Vec::new();
    let type_object = build_fake_type();
    // `[+0x58, +0x59, +0x5a, +0x5b, +0x5c]`: reparse pending (gate open), finished unforced (gate closed), running.
    let gates: [(&str, [u8; 5]); 3] = [("open", [0, 0, 0, 0, 0]), ("closed", [0, 1, 0, 1, 1]), ("running", [1, 1, 1, 1, 1])];
    let scripts: [&[&'static [u8]]; 2] = [&[], &[b"fA(1)\0", b"fB()\0"]];

    for (gate, flags) in gates {
        for with_child in [false, true] {
            for force in [0u8, 1] {
                for name in ["", "walk"] {
                    for mode in [0i32, 1, 2] {
                        for script in scripts {
                            for result in [0u8, 1] {
                                *SCRIPT_LINES.lock().unwrap() = script.to_vec();
                                SCRIPT_RESULT.store(result, Ordering::SeqCst);
                                let label = format!("gate={gate} child={with_child} force={force} name={name:?} mode={mode} lines={} result={result}", script.len());
                                let (real, port) = (build_set_bs_entity(type_object, with_child, flags), build_set_bs_entity(type_object, with_child, flags));

                                warm_pool();
                                let before = pool_lengths();
                                let real_ret = bfentity_live_support::real::set_bs(real, string_words(name), 1, force, mode);
                                let middle = pool_lengths();
                                let [w0, w1, w2] = string_words(name);
                                let port_ret = unsafe { SET_BS.hooked()(port as *const u32, w0 as *const u32, w1 as *const u32, w2 as *const u32, 1, force as i8, mode) };
                                let after = pool_lengths();

                                if real_ret != port_ret {
                                    failures.push(format!("{label}: returned real={real_ret} port={port_ret}"));
                                }
                                let (mut real_set, mut port_set) = (bfbehaviorset_live_support::snapshot(real + 0x14), bfbehaviorset_live_support::snapshot(port + 0x14));
                                // Each pole's set points at its own entity.
                                real_set.entity = real_set.entity.wrapping_sub(real);
                                port_set.entity = port_set.entity.wrapping_sub(port);
                                if real_set != port_set {
                                    failures.push(format!("{label}: set real={real_set:?} port={port_set:?}"));
                                }
                                for offset in [0xf8u32, 0xf9] {
                                    let (r, p) = (get_from_memory::<u8>(real + offset), get_from_memory::<u8>(port + offset));
                                    if r != p {
                                        failures.push(format!("{label}: +{offset:#x} real={r} port={p}"));
                                    }
                                }
                                let real_delta: Vec<u32> = middle.iter().zip(&before).map(|(a, b)| a.wrapping_sub(*b)).collect();
                                let port_delta: Vec<u32> = after.iter().zip(&middle).map(|(a, b)| a.wrapping_sub(*b)).collect();
                                if real_delta != port_delta {
                                    failures.push(format!("{label}: freelist growth real={real_delta:?} port={port_delta:?}"));
                                }
                            }
                        }
                    }
                }
            }
        }
    }
    SCRIPT_LINES.lock().unwrap().clear();
    finish_test(test_name, failures, failure_log)
}

/// A `setBS`-style entity whose set also holds current-call text, and whose queued `BFBSCall` string reads
/// `"abc"` with its flag byte clear.
pub(crate) fn build_clear_entity(type_object: u32, with_child: bool, flags: [u8; 5]) -> u32 {
    use openzt_detour::generated::msvc_std_basic_string::ASSIGN_RANGE;
    let entity = build_set_bs_entity(type_object, with_child, flags);
    let text = b"fWalk(1, 2)";
    unsafe { ASSIGN_RANGE.original()((entity + 0x14 + 0x34) as *const std::ffi::c_void, text.as_ptr() as *const u32, text.as_ptr_range().end, std::ptr::null()) };
    let begin = get_from_memory::<u32>(entity + 0xdc);
    for (i, byte) in b"abc\0".iter().enumerate() {
        unsafe { ((begin + i as u32) as *mut u8).write(*byte) };
    }
    write_words(entity + 0xe0, &[begin + 3]);
    unsafe { ((entity + 0xe8) as *mut u8).write(0) };
    entity
}

pub(crate) fn normalised_set(entity: u32) -> crate::bfbehaviorset::live_support::SetSnapshot {
    let mut snapshot = bfbehaviorset_live_support::snapshot(entity + 0x14);
    snapshot.entity = snapshot.entity.wrapping_sub(entity);
    snapshot
}

/// `BFENTITY_CLEAR_BS` - `clearBS` against vanilla with and without a child, and with the flags clear and set:
/// the return value, the embedded set's state and the freelist growth.
pub(crate) fn run_bfentity_clear_bs_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::tests::bfbehaviorset::build_fake_type;
    use openzt_detour::generated::bfentity::CLEAR_BS;

    let test_name = "BFENTITY_CLEAR_BS";
    let mut failures: Vec<String> = Vec::new();
    let type_object = build_fake_type();
    for with_child in [false, true] {
        for flags in [[0u8, 0, 0, 0, 0], [1, 1, 1, 1, 1]] {
            let label = format!("child={with_child} flags={flags:?}");
            let (real, port) = (build_clear_entity(type_object, with_child, flags), build_clear_entity(type_object, with_child, flags));
            warm_pool();
            let before = pool_lengths();
            let real_ret = bfentity_live_support::real::clear_bs(real);
            let middle = pool_lengths();
            let port_ret = unsafe { CLEAR_BS.hooked()(port as *const u32) };
            let after = pool_lengths();

            if real_ret & 0xff != 1 || port_ret & 0xff != 1 {
                failures.push(format!("{label}: AL real={real_ret:#x} port={port_ret:#x}, expected 1"));
            }
            let (real_set, port_set) = (normalised_set(real), normalised_set(port));
            if real_set != port_set {
                failures.push(format!("{label}: set real={real_set:?} port={port_set:?}"));
            }
            let real_delta: Vec<u32> = middle.iter().zip(&before).map(|(a, b)| a.wrapping_sub(*b)).collect();
            let port_delta: Vec<u32> = after.iter().zip(&middle).map(|(a, b)| a.wrapping_sub(*b)).collect();
            if real_delta != port_delta {
                failures.push(format!("{label}: freelist growth real={real_delta:?} port={port_delta:?}"));
            }
            if real_set.name.len() + real_set.current_call.len() + real_set.list.len() + real_set.call.name.len() != 0 || real_set.has_child {
                failures.push(format!("{label}: vanilla left the set populated: {real_set:?}"));
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn queue_string(entity: u32) -> (Vec<u8>, u8) {
    let (begin, end) = (get_from_memory::<u32>(entity + 0xdc), get_from_memory::<u32>(entity + 0xe0));
    let bytes = (0..end.wrapping_sub(begin)).map(|i| get_from_memory::<u8>(begin + i)).collect();
    (bytes, get_from_memory::<u8>(entity + 0xe8))
}

/// `BFENTITY_SET_BS_NEW` - `setBSNew` against vanilla: a name equal to the set's current name (no-op, true), a
/// name equal to the queued call (an empty call is queued), and unrelated and empty names, across gates, force
/// and a child set. Compares the return value, the set, the queued string and flag, and the freelist growth.
pub(crate) fn run_bfentity_set_bs_new_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::tests::bfbehaviorset::{build_fake_type, SCRIPT_LINES, SCRIPT_RESULT};
    use openzt_detour::generated::bfentity::SET_BSNEW;
    use std::sync::atomic::Ordering;

    let test_name = "BFENTITY_SET_BS_NEW";
    let mut failures: Vec<String> = Vec::new();
    let type_object = build_fake_type();
    let gates: [(&str, [u8; 5]); 3] = [("open", [0, 0, 0, 0, 0]), ("closed", [0, 1, 0, 1, 1]), ("running", [1, 1, 1, 1, 1])];
    let scripts: [&[&'static [u8]]; 2] = [&[], &[b"fA(1)\0", b"fB()\0"]];

    for (gate, flags) in gates {
        for with_child in [false, true] {
            for force in [0u8, 1] {
                for name in ["oldName", "abc", "walk", ""] {
                    for mode in [0i32, 2] {
                        for script in scripts {
                            *SCRIPT_LINES.lock().unwrap() = script.to_vec();
                            SCRIPT_RESULT.store(1, Ordering::SeqCst);
                            let label = format!("gate={gate} child={with_child} force={force} name={name:?} mode={mode} lines={}", script.len());
                            let (real, port) = (build_clear_entity(type_object, with_child, flags), build_clear_entity(type_object, with_child, flags));

                            warm_pool();
                            let before = pool_lengths();
                            let real_ret = bfentity_live_support::real::set_bs_new(real, string_words(name), 1, force, mode);
                            let middle = pool_lengths();
                            let [w0, w1, w2] = string_words(name);
                            let port_ret = unsafe { SET_BSNEW.hooked()(port as *const u32, w0 as *const u32, w1 as *const u32, w2 as *const u32, 1, force as i8, mode) };
                            let after = pool_lengths();

                            if real_ret != port_ret {
                                failures.push(format!("{label}: returned real={real_ret} port={port_ret}"));
                            }
                            let (real_set, port_set) = (normalised_set(real), normalised_set(port));
                            if real_set != port_set {
                                failures.push(format!("{label}: set real={real_set:?} port={port_set:?}"));
                            }
                            let (real_queue, port_queue) = (queue_string(real), queue_string(port));
                            if real_queue != port_queue {
                                failures.push(format!("{label}: queue real={real_queue:?} port={port_queue:?}"));
                            }
                            for offset in [0xf8u32, 0xf9] {
                                let (r, p) = (get_from_memory::<u8>(real + offset), get_from_memory::<u8>(port + offset));
                                if r != p {
                                    failures.push(format!("{label}: +{offset:#x} real={r} port={p}"));
                                }
                            }
                            let real_delta: Vec<u32> = middle.iter().zip(&before).map(|(a, b)| a.wrapping_sub(*b)).collect();
                            let port_delta: Vec<u32> = after.iter().zip(&middle).map(|(a, b)| a.wrapping_sub(*b)).collect();
                            if real_delta != port_delta {
                                failures.push(format!("{label}: freelist growth real={real_delta:?} port={port_delta:?}"));
                            }
                        }
                    }
                }
            }
        }
    }
    SCRIPT_LINES.lock().unwrap().clear();
    finish_test(test_name, failures, failure_log)
}

/// `BFENTITY_CLEAR_QUEUE` - `clearQueue` against vanilla: the queued call's string is emptied, its flag byte
/// set, and the result is true in both.
pub(crate) fn run_bfentity_clear_queue_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::tests::bfbehaviorset::build_fake_type;
    use openzt_detour::generated::bfentity::CLEAR_QUEUE;

    let test_name = "BFENTITY_CLEAR_QUEUE";
    let mut failures: Vec<String> = Vec::new();
    let type_object = build_fake_type();
    let (real, port) = (build_clear_entity(type_object, false, [0; 5]), build_clear_entity(type_object, false, [0; 5]));
    let real_ret = bfentity_live_support::real::clear_queue(real);
    let port_ret = unsafe { CLEAR_QUEUE.hooked()(port as *const u32) };
    if !real_ret || !port_ret {
        failures.push(format!("result real={real_ret} port={port_ret}, expected true"));
    }
    for (pole, entity) in [("real", real), ("port", port)] {
        let begin = get_from_memory::<u32>(entity + 0xdc);
        let end = get_from_memory::<u32>(entity + 0xe0);
        if end != begin || get_from_memory::<u8>(begin) != 0 {
            failures.push(format!("{pole}: queue string not emptied (len {}, first byte {})", end.wrapping_sub(begin), get_from_memory::<u8>(begin)));
        }
        if get_from_memory::<u8>(entity + 0xe8) != 1 {
            failures.push(format!("{pole}: flag byte {} not set", get_from_memory::<u8>(entity + 0xe8)));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `BFENTITY_SET_PENDING_BS` - `BFEntity::setQueuedCall` (`0x4300f6`) against vanilla: the queued call's flag byte
/// clear and set, forced and not, with names that fit the string's capacity, are empty, or force it to grow.
/// Compares the return value and the queued text and flag, and the freelist growth where neither pole reallocates.
pub(crate) fn run_bfentity_set_pending_bs_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::tests::bfbehaviorset::build_fake_type;
    use openzt_detour::generated::bfentity::SET_QUEUED_CALL;

    let test_name = "BFENTITY_SET_PENDING_BS";
    let mut failures: Vec<String> = Vec::new();
    let type_object = build_fake_type();
    // The fixture's queued string is "abc" in a buffer of capacity 8.
    for (name, grows) in [("", false), ("x", false), ("walkAbc", false), ("aMuchLongerBehaviorName", true)] {
        for queued_flag in [0u8, 1] {
            for force in [0u8, 1] {
                for call_flag in [0u8, 1, 7] {
                    let label = format!("name={name:?} queued_flag={queued_flag} force={force} call_flag={call_flag}");
                    let (real, port) = (build_clear_entity(type_object, false, [0; 5]), build_clear_entity(type_object, false, [0; 5]));
                    unsafe {
                        ((real + 0xe8) as *mut u8).write(queued_flag);
                        ((port + 0xe8) as *mut u8).write(queued_flag);
                    }

                    warm_pool();
                    let before = pool_lengths();
                    let real_ret = bfentity_live_support::real::set_pending_bs(real, string_words(name), call_flag, force);
                    let middle = pool_lengths();
                    let [w0, w1, w2] = string_words(name);
                    let port_ret = unsafe { SET_QUEUED_CALL.hooked()(port as *const u32, w0 as *const u32, w1 as *const u32, w2 as *const u32, call_flag as i8, force as i8) };
                    let after = pool_lengths();

                    if real_ret != port_ret {
                        failures.push(format!("{label}: returned real={real_ret} port={port_ret}"));
                    }
                    let (real_queue, port_queue) = (queue_string(real), queue_string(port));
                    if real_queue != port_queue {
                        failures.push(format!("{label}: queue real={real_queue:?} port={port_queue:?}"));
                    }
                    let nul_ok = |entity: u32| get_from_memory::<u8>(get_from_memory::<u32>(entity + 0xe0)) == 0;
                    if !nul_ok(real) || !nul_ok(port) {
                        failures.push(format!("{label}: queued text not NUL-terminated"));
                    }
                    let stored = queued_flag != 0 || force != 0;
                    if !(grows && stored) {
                        let real_delta: Vec<u32> = middle.iter().zip(&before).map(|(a, b)| a.wrapping_sub(*b)).collect();
                        let port_delta: Vec<u32> = after.iter().zip(&middle).map(|(a, b)| a.wrapping_sub(*b)).collect();
                        if real_delta != port_delta {
                            failures.push(format!("{label}: freelist growth real={real_delta:?} port={port_delta:?}"));
                        }
                    }
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

fn entity_name(entity: u32) -> Vec<u8> {
    let (begin, end) = (get_from_memory::<u32>(entity + 0x108), get_from_memory::<u32>(entity + 0x10c));
    (0..end.wrapping_sub(begin)).map(|i| get_from_memory::<u8>(begin + i)).collect()
}

/// `BFENTITY_SET_NAME` - `BFEntity::setName` (`0x449215`) against vanilla: empty, blank, graphic and mixed names,
/// names that fit the name's capacity of 8, outgrow it, or shorten an existing longer name, and a name that is
/// the entity's own. Compares the resulting name text and that it is NUL-terminated.
pub(crate) fn run_bfentity_set_name_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use crate::reimplementation_tests::tests::bfbehaviorset::build_fake_type;
    use crate::vanilla_string::VanillaString;
    use openzt_detour::generated::bfentity::SET_NAME;

    let test_name = "BFENTITY_SET_NAME";
    let mut failures: Vec<String> = Vec::new();
    let type_object = build_fake_type();
    let names = ["", " ", "   ", "a", "  a ", "Leo", "Buddy the Great", "x\t", "\u{e9}t\u{e9}"];
    for existing in ["", "Old", "A Rather Long Existing Name"] {
        for name in names {
            for own in [false, true] {
                if own && name != "Leo" {
                    continue;
                }
                let label = format!("existing={existing:?} name={name:?} own={own}");
                let (real, port) = (build_clear_entity(type_object, false, [0; 5]), build_clear_entity(type_object, false, [0; 5]));
                let prime = VanillaString::new(existing);
                for entity in [real, port] {
                    bfentity_live_support::real::set_name(entity, prime.as_ptr() as u32);
                }
                let (real_arg, port_arg) = (VanillaString::new(name), VanillaString::new(name));
                let (real_ptr, port_ptr) = if own { (real + 0x108, port + 0x108) } else { (real_arg.as_ptr() as u32, port_arg.as_ptr() as u32) };
                bfentity_live_support::real::set_name(real, real_ptr);
                unsafe { SET_NAME.hooked()(port as *const u32, port_ptr as *const u32) };

                let (real_name, port_name) = (entity_name(real), entity_name(port));
                if real_name != port_name {
                    failures.push(format!("{label}: name real={real_name:?} port={port_name:?}"));
                }
                for (pole, entity) in [("real", real), ("port", port)] {
                    if get_from_memory::<u8>(get_from_memory::<u32>(entity + 0x10c)) != 0 {
                        failures.push(format!("{label}: {pole} name not NUL-terminated"));
                    }
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
