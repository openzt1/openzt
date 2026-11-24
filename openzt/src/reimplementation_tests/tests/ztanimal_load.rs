//! `ZTAnimal::load` against the real body: streams in the layout of every version gate, loaded through the real body,
//! the port and the hooked address into identically built fixtures. Result, bytes consumed, whole-animal memory, both
//! strings, the escape record and the escaped-animal list must agree, for whole streams and for streams cut short.

use openzt_detour::generated::{ztanimal::{INIT_STATUS_VARS, LOAD}, ztunit::SAVE as ZTUNIT_SAVE};

use super::{
    ztanimal::{base, build_animal, live_animal_types, Lcg},
    ztguide::{pin_rng, rng},
};
use crate::{
    reimplementation_tests::{harness::finish_test, io_redirect},
    util::{get_from_memory, save_to_memory},
    ztanimal::{live_support::real, ZTAnimal, ANIMAL_SIZE},
};

/// `DAT_00638fb0`'s RVA: the global holding the sentinel of the escaped-animal list.
const ESCAPE_LIST_RVA: u32 = 0x0023_8fb0;
/// `DAT_00639370`/`DAT_00639374`: the home gate tile coordinates a stream without a gate tile carries.
const DEFAULT_GATE_X_RVA: u32 = 0x0023_9370;
const DEFAULT_GATE_Y_RVA: u32 = 0x0023_9374;
/// Streams of this version and later carry the `ZTUnit` data the capture from `ZTUnit::save` has; older ones would be
/// misread by `ZTUnit::load` (vanilla then dereferences a null lookup), so they stop one byte short of the unit data.
const UNIT_TAIL_VERSION: u32 = 0x64;

#[derive(Clone, Copy)]
struct Config {
    seed: u32,
    lengths: (u32, u32),
    /// Value of the escaped byte (`+0x3a3`).
    escaped: u8,
    /// Values `+0x348` and `+0x34c` carry: huge (clamped) or small (kept).
    big: bool,
    /// `0` no record (flag `0`), `1` record with valid tiles, `2` record with out-of-map tiles, `3` flag value `2`.
    record: u32,
    /// The extra dword version `0x51` and later carries after the record.
    marked: u32,
    /// `0` default gate coordinates, `1` valid tile, `2` out-of-map coordinates.
    gate: u32,
}

/// Builds the animal part of a stream in the layout `version` reads, then `tail`. Returns the stream and
/// the length of the animal's own part.
fn stream(version: u32, config: &Config, tail: &[u8]) -> (Vec<u8>, usize) {
    let mut rng = Lcg(config.seed);
    let mut out: Vec<u8> = Vec::new();
    macro_rules! dword {
        () => {
            out.extend_from_slice(&rng.next().to_le_bytes())
        };
        ($value:expr) => {
            out.extend_from_slice(&($value as u32).to_le_bytes())
        };
    }
    macro_rules! byte {
        () => {
            out.push((rng.next() >> 3) as u8)
        };
    }
    macro_rules! string {
        ($length:expr) => {{
            let length: u32 = $length;
            out.extend_from_slice(&length.to_le_bytes());
            out.extend((0..length).map(|index| b'a' + (index % 26) as u8));
        }};
    }
    for _ in 0..4 {
        dword!();
    }
    for _ in 0..4 {
        byte!();
    }
    if version < 0x10 {
        byte!();
    }
    byte!();
    dword!();
    byte!();
    for _ in 0..5 {
        dword!();
    }
    string!(config.lengths.0);
    string!(config.lengths.1);
    if version >= 5 {
        for _ in 0..11 {
            dword!();
        }
        if version < 0x36 {
            dword!();
        }
        dword!();
        dword!();
    }
    if version >= 0xe {
        dword!();
    }
    if version >= 0x10 {
        byte!();
        byte!();
    }
    if version >= 0x12 {
        out.push(config.escaped);
    }
    if version >= 0x13 {
        dword!();
    }
    if version >= 0x16 {
        byte!();
        byte!();
        dword!();
    }
    if version >= 0x1f {
        dword!();
        dword!();
        dword!();
    }
    if version >= 0x21 {
        for _ in 0..4 {
            dword!();
        }
    }
    if version >= 0x23 {
        byte!();
        byte!();
    }
    if version >= 0x29 {
        dword!();
        dword!();
    }
    if version >= 0x2b {
        if config.big {
            dword!(0x7fff_0000u32);
            dword!(0x7fff_0000u32);
        } else {
            dword!(1u32);
            dword!(2u32);
        }
        dword!();
        byte!();
    }
    if version >= 0x2c {
        byte!();
    }
    if version >= 0x2e {
        dword!();
    }
    if version >= 0x2f {
        match config.record {
            0 => dword!(0u32),
            3 => dword!(2u32),
            record => {
                dword!(1u32);
                let tiles: [i32; 4] = if record == 1 { [1, 1, 2, 3] } else { [-1, 0, 100_000, 1] };
                for tile in tiles {
                    dword!(tile);
                }
                dword!();
                dword!(rng.next() & 1);
                dword!(rng.next() & 1);
                if version >= 0x51 {
                    dword!(config.marked);
                }
            }
        }
    }
    if version >= 0x34 {
        dword!();
        dword!();
        byte!();
        byte!();
    }
    if version >= 0x36 {
        byte!();
    }
    if version == 0x39 || version >= 0x3d {
        byte!();
    }
    if version >= 0x4c {
        byte!();
    }
    if version >= 0x4d {
        dword!();
        dword!();
    }
    if version >= 0x4e {
        byte!();
    }
    if version >= 0x4f {
        byte!();
        dword!();
    }
    if version >= 0x57 {
        dword!();
        dword!();
    }
    if version >= 0x65 {
        match config.gate {
            0 => {
                dword!(get_from_memory::<u32>(base() + DEFAULT_GATE_X_RVA));
                dword!(get_from_memory::<u32>(base() + DEFAULT_GATE_Y_RVA));
            }
            1 => {
                dword!(2u32);
                dword!(2u32);
            }
            _ => {
                dword!(-5i32);
                dword!(1_000_000u32);
            }
        }
    }
    let own = out.len();
    out.extend_from_slice(tail);
    (out, own)
}

/// The bytes `ZTUnit::save` writes for a fresh fixture of `entity_type`.
fn unit_tail(entity_type: u32) -> Vec<u8> {
    let animal = build_animal(entity_type);
    io_redirect::begin_capture();
    unsafe { ZTUNIT_SAVE.original()(animal.0 as *const u32, std::ptr::null()) };
    io_redirect::end_capture()
}

fn run_load(animal: ZTAnimal, bytes: &[u8], version: u32, pole: usize) -> (bool, usize, u32) {
    pin_rng(0x00c0_ffee);
    io_redirect::begin_replay(bytes.to_vec());
    let result = match pole {
        0 => real::load(animal.0, std::ptr::null(), version),
        1 => animal.load(std::ptr::null(), version),
        _ => unsafe { LOAD.hooked()(animal.0 as *const u32, std::ptr::null(), version) },
    };
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed, rng())
}

fn string_bytes(animal: ZTAnimal, offset: u32) -> Vec<u8> {
    let begin: u32 = get_from_memory(animal.0 + offset);
    let end: u32 = get_from_memory(animal.0 + offset + 4);
    if begin == 0 || end < begin || end - begin > 0x2000 {
        return Vec::new();
    }
    (begin..end).map(get_from_memory::<u8>).collect()
}

/// The escape record (`+0x384`) as bytes, or `None`.
fn record_bytes(animal: ZTAnimal) -> Option<Vec<u8>> {
    let record: u32 = get_from_memory(animal.0 + 0x384);
    (record != 0).then(|| (0..0x10).map(|offset| get_from_memory::<u8>(record + offset)).collect())
}

/// How often `animal` is in the escaped list, and whether the last node is its.
fn escaped_state(animal: ZTAnimal) -> (usize, bool) {
    let sentinel: u32 = get_from_memory(base() + ESCAPE_LIST_RVA);
    let mut count = 0;
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        if get_from_memory::<u32>(node + 8) == animal.0 {
            count += 1;
        }
        node = get_from_memory(node);
    }
    let last: u32 = get_from_memory(sentinel + 4);
    (count, last != sentinel && get_from_memory::<u32>(last + 8) == animal.0)
}

/// Unlinks every node of `animal` from the escaped list (the nodes are leaked).
fn unlink_escaped(animal: ZTAnimal) {
    let sentinel: u32 = get_from_memory(base() + ESCAPE_LIST_RVA);
    let mut node: u32 = get_from_memory(sentinel);
    while node != sentinel {
        let next: u32 = get_from_memory(node);
        if get_from_memory::<u32>(node + 8) == animal.0 {
            let previous: u32 = get_from_memory(node + 4);
            save_to_memory(previous, next);
            save_to_memory(next + 4, previous);
        }
        node = next;
    }
}

type Observed = (Vec<u8>, Vec<u8>, Option<Vec<u8>>, (usize, bool));

fn observe(animal: ZTAnimal) -> Observed {
    let state = escaped_state(animal);
    unlink_escaped(animal);
    (string_bytes(animal, 0x26c), string_bytes(animal, 0x278), record_bytes(animal), state)
}

fn summary(observed: &Observed) -> String {
    format!("(string lengths {} {}, record {:?}, escaped {:?})", observed.0.len(), observed.1.len(), observed.2, observed.3)
}

/// Compares whole animals; words that are heap pointers (above `0x10000`) may differ.
fn compare_animals(label: &str, real: ZTAnimal, other: ZTAnimal, failures: &mut Vec<String>) {
    // `ZTUnit::load` draws fresh entity ids from process-wide counters for streams that do not carry them.
    for offset in [0x124u32, 0x148] {
        let id: u32 = get_from_memory(real.0 + offset);
        save_to_memory(other.0 + offset, id);
    }
    for (index, (r, p)) in animal_words(real).into_iter().zip(animal_words(other)).enumerate() {
        let equivalent = r == p || (r >= 0x10000 && p >= 0x10000 && !is_shared(r) && !is_shared(p));
        if !equivalent {
            failures.push(format!("{label}: +{:#x} real {r:#x}, other {p:#x}", index * 4));
        }
    }
}

/// Words every fixture holds the same value of that are pointers into shared data (the world's tile array, the type).
fn is_shared(value: u32) -> bool {
    SHARED.lock().unwrap().iter().any(|&(low, high)| (low..high).contains(&value))
}

static SHARED: std::sync::Mutex<Vec<(u32, u32)>> = std::sync::Mutex::new(Vec::new());

fn animal_words(animal: ZTAnimal) -> Vec<u32> {
    (0..ANIMAL_SIZE / 4).map(|i| get_from_memory::<u32>(animal.0 + i * 4)).collect()
}

/// The range of addresses the world's tile array occupies, plus the animal type's memory.
fn set_shared_ranges(entity_type: u32) {
    let world: u32 = get_from_memory(base() + 0x0023_8040);
    let tiles: u32 = get_from_memory(world + 0x40);
    let width: i32 = get_from_memory(world + 0x34);
    let height: i32 = get_from_memory(world + 0x38);
    let size = (width.max(0) as u32).saturating_mul(height.max(0) as u32).saturating_mul(0x8c);
    *SHARED.lock().unwrap() = vec![(tiles, tiles.saturating_add(size)), (entity_type, entity_type + 0x800)];
}

fn version_list() -> Vec<u32> {
    vec![
        1, 4, 5, 0xd, 0xe, 0xf, 0x10, 0x11, 0x12, 0x13, 0x15, 0x16, 0x1e, 0x1f, 0x20, 0x21, 0x22, 0x23, 0x28, 0x29, 0x2a, 0x2b, 0x2c, 0x2d, 0x2e, 0x2f, 0x33, 0x34, 0x35, 0x36,
        0x39, 0x3a, 0x3c, 0x3d, 0x4b, 0x4c, 0x4d, 0x4e, 0x4f, 0x50, 0x51, 0x56, 0x57, 0x64, 0x65, 0x66,
    ]
}

/// `ZTANIMAL_LOAD_LIVE`: every version gate, with string lengths at, below and past the `0x1000` limit, escaped
/// animals (list insert), clamped and unclamped words, escape records with in-map and out-of-map tiles, a record flag
/// that is neither `0` nor `1`, and home gate tiles that are the default, valid or out of the map; whole streams and
/// streams cut at chosen offsets.
pub(crate) fn run_ztanimal_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(1);
    let Some(&ty) = types.first() else {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    set_shared_ranges(ty);
    let tail = unit_tail(ty);
    let base_config = Config { seed: 1, lengths: (5, 3), escaped: 0, big: false, record: 0, marked: 0, gate: 0 };
    let mut configs: Vec<Config> = vec![base_config];
    for (index, tweak) in [
        Config { lengths: (0, 0), ..base_config },
        Config { lengths: (0xfff, 9), ..base_config },
        Config { lengths: (0x1000, 4), ..base_config },
        Config { lengths: (4, 0x1001), ..base_config },
        Config { escaped: 1, ..base_config },
        Config { big: true, ..base_config },
        Config { record: 1, ..base_config },
        Config { record: 1, marked: 1, ..base_config },
        Config { record: 2, marked: 1, ..base_config },
        Config { record: 3, ..base_config },
        Config { gate: 1, ..base_config },
        Config { gate: 2, ..base_config },
        Config { escaped: 1, big: true, record: 1, marked: 1, gate: 1, ..base_config },
    ]
    .into_iter()
    .enumerate()
    {
        configs.push(Config { seed: 10 + index as u32, ..tweak });
    }
    let mut compared = 0usize;
    let mut full_reads = 0usize;
    let mut full_summary: Vec<String> = Vec::new();
    let mut seen_escaped = false;
    let mut seen_record = false;
    for version in version_list() {
        for config in &configs {
            let (bytes, own) = stream(version, config, &tail);
            let mut cuts: Vec<usize> = vec![0, own / 4, own / 2, own.saturating_sub(1), own, bytes.len()];
            if version == 0x66 || version == 0x2f {
                cuts = (0..own + 4).chain([bytes.len()]).collect();
            }
            if version < UNIT_TAIL_VERSION {
                cuts.retain(|&cut| cut < own);
            }
            cuts.sort_unstable();
            cuts.dedup();
            for cut in cuts {
                let label = format!("load v{version:#x} lengths {:?} escaped {} big {} record {} gate {} cut {cut}", config.lengths, config.escaped, config.big, config.record, config.gate);
                let fixtures: Vec<ZTAnimal> = (0..3).map(|_| build_animal(ty)).collect();
                let mut outcomes = Vec::new();
                let mut observed: Vec<Observed> = Vec::new();
                for (pole, animal) in fixtures.iter().enumerate() {
                    outcomes.push(run_load(*animal, &bytes[..cut], version, pole));
                    observed.push(observe(*animal));
                }
                compare_animals(&label, fixtures[0], fixtures[1], &mut failures);
                compare_animals(&format!("{label} hooked"), fixtures[0], fixtures[2], &mut failures);
                if cut == bytes.len() {
                    if outcomes[0].1 + 1 >= own {
                        full_reads += 1;
                    }
                    full_summary.push(format!("v{version:#x}: {:?} consumed {} of {}", outcomes[0].0, outcomes[0].1, bytes.len()));
                    seen_escaped |= observed[0].3 .0 > 0;
                    seen_record |= observed[0].2.is_some();
                }
                if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                    failures.push(format!("{label}: (result, consumed, RNG) real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
                }
                if observed[0] != observed[1] || observed[0] != observed[2] {
                    failures.push(format!("{label}: strings, record or escaped list differ: {:?}", observed.iter().map(summary).collect::<Vec<_>>()));
                }
                compared += 1;
                if failures.len() > 20 {
                    return finish_test(test_name, failures, failure_log);
                }
            }
        }
    }

    // A second load over a populated animal replaces both strings the same way.
    let (first, _) = stream(0x66, &configs[1], &tail);
    let (second, _) = stream(0x66, &Config { lengths: (2, 7), ..configs[0] }, &tail);
    let fixtures: Vec<ZTAnimal> = (0..3).map(|_| build_animal(ty)).collect();
    let mut outcomes = Vec::new();
    let mut observed: Vec<Observed> = Vec::new();
    for (pole, animal) in fixtures.iter().enumerate() {
        run_load(*animal, &first, 0x66, pole);
        unlink_escaped(*animal);
        outcomes.push(run_load(*animal, &second, 0x66, pole));
        observed.push(observe(*animal));
    }
    if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
        failures.push(format!("reload: real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
    }
    compare_animals("reload", fixtures[0], fixtures[1], &mut failures);
    if observed[0] != observed[1] || observed[0] != observed[2] {
        failures.push("reload: strings, record or escaped list differ".to_string());
    }

    if full_reads == 0 {
        failures.push(format!("no complete stream was consumed in full over {compared} cases: {full_summary:?}"));
    }
    if !seen_escaped || !seen_record {
        failures.push(format!("coverage gap: escaped list insert seen {seen_escaped}, escape record seen {seen_record}"));
    }
    failures.truncate(40);
    finish_test(&format!("{test_name} ({compared} cases)"), failures, failure_log)
}

/// `DAT_006393fc`-style period globals `initStatusVars` draws modulo, in vanilla's order (see `ztanimal::init_layout`).
const PERIOD_RVAS: [u32; 16] = [
    0x0023_940c, 0x0023_9450, 0x0023_941c, 0x0023_93fc, 0x0023_93e0, 0x0023_9424, 0x0023_9410, 0x0023_93f8, 0x0023_9400, 0x0023_9414, 0x0023_9420, 0x0023_9404, 0x0023_93e8,
    0x0023_9408, 0x0023_9418, 0x0023_93e4,
];

/// `ZTANIMAL_INIT_STATUS_VARS_LIVE`: `initStatusVars` over fixtures whose saved region holds noise, for every live
/// animal type with its name's first character forced to `m`, `f`, `p`, another letter and left alone, with none, the
/// first and the last period global forced to zero, and several RNG states. The result memory, both strings and the
/// RNG state afterwards must agree between the real body, the port and the hooked address.
pub(crate) fn run_ztanimal_init_status_vars_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTANIMAL_INIT_STATUS_VARS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = live_animal_types(4);
    if types.is_empty() {
        failures.push("the live world holds no animal type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    set_shared_ranges(types[0]);
    SHARED.lock().unwrap().extend(types.iter().map(|&ty| (ty, ty + 0x800)));
    let letters: [Option<&'static [u8; 2]>; 5] = [None, Some(b"m\0"), Some(b"f\0"), Some(b"p\0"), Some(b"x\0")];
    let mut cases = 0usize;
    let mut genders: std::collections::BTreeSet<Vec<u8>> = std::collections::BTreeSet::new();
    for &ty in &types {
        let original_name: u32 = get_from_memory(ty + 0xa4);
        for letter in letters {
            if let Some(text) = letter {
                save_to_memory(ty + 0xa4, text.as_ptr() as u32);
            }
            for zero in [None, Some(0usize), Some(15)] {
                let saved_period = zero.map(|index| get_from_memory::<u32>(base() + PERIOD_RVAS[index]));
                if let Some(index) = zero {
                    save_to_memory(base() + PERIOD_RVAS[index], 0u32);
                }
                for seed in [1u32, 0x1234_5678, 0x7fff_ffff, 0xdead_beef, 0x0001_0000, 77] {
                    let fixtures: Vec<ZTAnimal> = (0..3).map(|_| build_animal(ty)).collect();
                    let mut outcomes = Vec::new();
                    for (pole, animal) in fixtures.iter().enumerate() {
                        let mut noise = Lcg(seed);
                        for offset in (0x288u32..0x384).chain(0x390..0x3ac) {
                            save_to_memory(animal.0 + offset, (noise.next() & 0xff) as u8);
                        }
                        pin_rng(seed);
                        match pole {
                            0 => real::init_status_vars(animal.0),
                            1 => animal.init_status_vars(),
                            _ => unsafe { INIT_STATUS_VARS.hooked()(animal.0 as *const u32) },
                        }
                        outcomes.push((rng(), string_bytes(*animal, 0x26c), string_bytes(*animal, 0x278)));
                    }
                    let label = format!("initStatusVars type {ty:#x} name {letter:?} zero {zero:?} seed {seed:#x}");
                    if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                        failures.push(format!("{label}: (rng, strings) real {:?}, port {:?}, hooked {:?}", outcomes[0], outcomes[1], outcomes[2]));
                    }
                    compare_animals(&label, fixtures[0], fixtures[1], &mut failures);
                    compare_animals(&format!("{label} hooked"), fixtures[0], fixtures[2], &mut failures);
                    genders.insert(outcomes[0].1.clone());
                    cases += 1;
                    if failures.len() > 20 {
                        return finish_test(test_name, failures, failure_log);
                    }
                }
                if let (Some(index), Some(value)) = (zero, saved_period) {
                    save_to_memory(base() + PERIOD_RVAS[index], value);
                }
            }
        }
        save_to_memory(ty + 0xa4, original_name);
    }
    if genders.len() < 2 {
        failures.push(format!("only {} distinct gender strings occurred: {genders:?}", genders.len()));
    }
    failures.truncate(40);
    finish_test(&format!("{test_name} ({cases} cases)"), failures, failure_log)
}
