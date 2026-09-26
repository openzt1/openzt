#![allow(unused_imports)]

use std::collections::HashMap;
use std::mem::size_of;

#[cfg(not(feature = "vanilla-research-save"))]
use openzt_detour_macro::detour_mod;
use tracing::{error, warn};

use crate::bfconfigfile::BFConfigFile;
use crate::util::{get_from_memory, mut_from_memory, ref_from_memory, ZTArray, ZTBufferString};
use super::super::models::{ZTResearchBranch, ZTResearchCategory, ZTResearchFundingLevel, ZTResearchProgram};
use super::ztresearchmgr::{global_ztgamemgr_ptr, ZTResearchMgr};

#[cfg(feature = "reimplementation-tests")]
use crate::{
    globals::get_module_base,
    util::save_to_memory,
};

/// One `(kind, id, value)` tuple from the save stream. `Program`'s value is stored as raw `f32`
/// bits (not `f32` itself) so records - and the `PartialEq`/`HashMap` machinery used to compare
/// them - don't have to special-case `NaN != NaN`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SaveRecord {
    Branch { id: i32, current_funding_level: i32 },
    Category { id: i32, enabled: u8 },
    Program { id: i32, current_progress_bits: u32 },
}

/// Walks a manager's `branch_array`/`category_array`/`program_array` in the same nested order
/// `ZTResearchMgr::save` does (branch, then each of its categories, then each category's
/// programs), producing one record per branch/category/program.
pub(crate) fn snapshot_mgr(mgr: &ZTResearchMgr) -> Vec<SaveRecord> {
    let mut records = Vec::new();
    for branch in mgr.branches() {
        records.push(SaveRecord::Branch { id: branch.id, current_funding_level: branch.current_funding_level });
        for category in branch.categories() {
            records.push(SaveRecord::Category { id: category.id, enabled: category.enabled });
            for program in category.programs() {
                records.push(SaveRecord::Program { id: program.id, current_progress_bits: program.current_progress.to_bits() });
            }
        }
    }
    records
}

/// The exact byte stream `ZTResearchMgr::save` writes for `records`: a leading `int32 0` header, one
/// `(kind, id, value)` tuple per record (`kind` `0`/`1`/`2` for `Branch`/`Category`/`Program`; a
/// `Category`'s `enabled` is a single byte, everything else is a little-endian `int32`/`float32`),
/// and a trailing `int32 -1` terminator. No counts are ever written - the stream is fully
/// self-describing via the `kind` tag.
pub(crate) fn serialize(records: &[SaveRecord]) -> Vec<u8> {
    let mut bytes = Vec::with_capacity(8 + records.len() * 12);
    bytes.extend_from_slice(&0i32.to_le_bytes());
    for record in records {
        match *record {
            SaveRecord::Branch { id, current_funding_level } => {
                bytes.extend_from_slice(&0i32.to_le_bytes());
                bytes.extend_from_slice(&id.to_le_bytes());
                bytes.extend_from_slice(&current_funding_level.to_le_bytes());
            }
            SaveRecord::Category { id, enabled } => {
                bytes.extend_from_slice(&1i32.to_le_bytes());
                bytes.extend_from_slice(&id.to_le_bytes());
                bytes.push(enabled);
            }
            SaveRecord::Program { id, current_progress_bits } => {
                bytes.extend_from_slice(&2i32.to_le_bytes());
                bytes.extend_from_slice(&id.to_le_bytes());
                bytes.extend_from_slice(&current_progress_bits.to_le_bytes());
            }
        }
    }
    bytes.extend_from_slice(&(-1i32).to_le_bytes());
    bytes
}

fn read_i32(bytes: &[u8], cursor: &mut usize) -> Option<i32> {
    let chunk = bytes.get(*cursor..*cursor + 4)?;
    *cursor += 4;
    Some(i32::from_le_bytes(chunk.try_into().unwrap()))
}

/// The inverse of `serialize`. Returns `None` on any malformed stream: truncated (a size/count
/// doesn't fit), an unrecognized `kind` (anything other than `0`/`1`/`2`/the `-1` terminator), or
/// trailing bytes left over after the terminator.
pub(crate) fn parse(bytes: &[u8]) -> Option<Vec<SaveRecord>> {
    let mut cursor = 0usize;
    read_i32(bytes, &mut cursor)?; // header, discarded - matches `load`, which reads but never uses it
    let mut records = Vec::new();
    loop {
        let kind = read_i32(bytes, &mut cursor)?;
        if kind < 0 {
            break;
        }
        let id = read_i32(bytes, &mut cursor)?;
        match kind {
            0 => {
                let current_funding_level = read_i32(bytes, &mut cursor)?;
                records.push(SaveRecord::Branch { id, current_funding_level });
            }
            1 => {
                let enabled = *bytes.get(cursor)?;
                cursor += 1;
                records.push(SaveRecord::Category { id, enabled });
            }
            2 => {
                let current_progress_bits = read_i32(bytes, &mut cursor)? as u32;
                records.push(SaveRecord::Program { id, current_progress_bits });
            }
            _ => return None,
        }
    }
    (cursor == bytes.len()).then_some(records)
}

/// The funding level/enabled flag/current progress `ZTResearchMgr::load` ends up with for every
/// branch/category/program id it knows about (not just the ones a stream record touched).
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct PredictedState {
    pub(crate) funding_levels: HashMap<i32, i32>,
    pub(crate) enabled: HashMap<i32, u8>,
    pub(crate) progress_bits: HashMap<i32, u32>,
}

/// The save-game format version at which `ZTResearchMgr::load` starts reading/writing research
/// data at all - below this, `load` still runs its unconditional reset (and the
/// `on_completion`/`pick_random_program` tail) but never touches `file`. Shared between
/// `predict_load` (the pure prediction) and `detours::load` (the live implementation)
/// below so the threshold can't drift between the two.
const MIN_VERSION_WITH_RESEARCH_DATA: u32 = 0x28;

/// Predicts what `ZTResearchMgr::load` leaves in `current_funding_level`/`enabled`/
/// `current_progress` (excluding the `on_completion`/`pick_random_program` side effects - see the
/// module doc comment above), given the ids it already knows about and the stream's records.
pub(crate) fn predict_load(
    branch_ids: &[i32],
    category_ids: &[i32],
    program_ids: &[i32],
    funding_level_counts: &HashMap<i32, usize>,
    records: &[SaveRecord],
    version: u32,
) -> PredictedState {
    let mut funding_levels: HashMap<i32, i32> = branch_ids.iter().map(|&id| (id, 0)).collect();
    let mut enabled: HashMap<i32, u8> = category_ids.iter().map(|&id| (id, 1)).collect();
    let mut progress_bits: HashMap<i32, u32> = program_ids.iter().map(|&id| (id, 0.0f32.to_bits())).collect();

    if version >= MIN_VERSION_WITH_RESEARCH_DATA {
        for record in records {
            match *record {
                SaveRecord::Branch { id, current_funding_level } => {
                    if let Some(slot) = funding_levels.get_mut(&id) {
                        let count = funding_level_counts.get(&id).copied().unwrap_or(0) as u32;
                        *slot = if (current_funding_level as u32) < count { current_funding_level } else { 0 };
                    }
                }
                SaveRecord::Category { id, enabled: value } => {
                    if let Some(slot) = enabled.get_mut(&id) {
                        *slot = (value != 0) as u8;
                    }
                }
                SaveRecord::Program { id, current_progress_bits } => {
                    if let Some(slot) = progress_bits.get_mut(&id) {
                        *slot = current_progress_bits;
                    }
                }
            }
        }
    }

    PredictedState { funding_levels, enabled, progress_bits }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn serialize_parse_round_trip() {
        let records = vec![
            SaveRecord::Branch { id: 100, current_funding_level: 2 },
            SaveRecord::Category { id: 200, enabled: 1 },
            SaveRecord::Program { id: 300, current_progress_bits: 12.5f32.to_bits() },
            SaveRecord::Branch { id: -5, current_funding_level: -1 },
            SaveRecord::Category { id: i32::MAX, enabled: 0 },
            SaveRecord::Program { id: i32::MIN, current_progress_bits: f32::NAN.to_bits() },
        ];
        let bytes = serialize(&records);
        assert_eq!(parse(&bytes), Some(records));
    }

    #[test]
    fn empty_record_list_round_trips() {
        let bytes = serialize(&[]);
        assert_eq!(bytes.len(), 8);
        assert_eq!(parse(&bytes), Some(vec![]));
    }

    #[test]
    fn parse_rejects_truncated_stream() {
        let mut bytes = serialize(&[SaveRecord::Branch { id: 1, current_funding_level: 2 }]);
        bytes.pop();
        assert_eq!(parse(&bytes), None);
    }

    #[test]
    fn parse_rejects_unknown_kind() {
        let mut bytes = 0i32.to_le_bytes().to_vec();
        bytes.extend_from_slice(&3i32.to_le_bytes());
        bytes.extend_from_slice(&0i32.to_le_bytes());
        assert_eq!(parse(&bytes), None);
    }

    #[test]
    fn parse_rejects_trailing_garbage() {
        let mut bytes = serialize(&[]);
        bytes.push(0xff);
        assert_eq!(parse(&bytes), None);
    }

    #[test]
    fn predict_load_resets_untouched_ids_and_applies_matching_records() {
        let funding_level_counts = HashMap::from([(1, 3usize)]);
        let records = vec![
            SaveRecord::Branch { id: 1, current_funding_level: 2 },
            SaveRecord::Branch { id: 1, current_funding_level: 99 },
            SaveRecord::Category { id: 10, enabled: 0 },
            SaveRecord::Program { id: 100, current_progress_bits: 5.0f32.to_bits() },
            SaveRecord::Branch { id: 999, current_funding_level: 1 },
        ];
        let predicted = predict_load(&[1, 2], &[10, 20], &[100], &funding_level_counts, &records, 0x28);
        assert_eq!(predicted.funding_levels, HashMap::from([(1, 0), (2, 0)]));
        assert_eq!(predicted.enabled, HashMap::from([(10, 0), (20, 1)]));
        assert_eq!(predicted.progress_bits, HashMap::from([(100, 5.0f32.to_bits())]));
    }

    #[test]
    fn predict_load_below_version_threshold_only_resets() {
        let funding_level_counts = HashMap::from([(1, 3usize)]);
        let records = vec![SaveRecord::Branch { id: 1, current_funding_level: 2 }];
        let predicted = predict_load(&[1], &[], &[], &funding_level_counts, &records, 0x27);
        assert_eq!(predicted.funding_levels, HashMap::from([(1, 0)]));
    }
}

#[cfg(not(feature = "vanilla-research-save"))]
mod stream_io {
    use openzt_detour::generated::standalone::DEALLOCATE;

    use crate::util::get_from_memory;

    pub(super) fn is_eof(file: *const u32) -> bool {
        get_from_memory::<u32>((file as u32) + 0xc) & 0x20 != 0
    }

    pub(super) fn read_i32(file: *const u32) -> Option<i32> {
        let mut buf = 0i32;
        let ok = unsafe { DEALLOCATE.hooked()(&mut buf as *mut i32 as *const u32, 4, 1, file as *const u8) };
        (ok == 1).then_some(buf)
    }

    pub(super) fn read_u8(file: *const u32) -> Option<u8> {
        let mut buf = 0u8;
        let ok = unsafe { DEALLOCATE.hooked()(&mut buf as *mut u8 as *const u32, 1, 1, file as *const u8) };
        (ok == 1).then_some(buf)
    }
}

#[cfg(not(feature = "vanilla-research-save"))]
#[detour_mod]
mod detours {
    use openzt_detour::generated::standalone;
    use openzt_detour::generated::ztresearchmgr::{LOAD, SAVE};
    use tracing::{error, warn};

    use super::{stream_io, *};
    use crate::util::{mut_from_memory, ref_from_memory};

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save(this: *const u32, file: *const u32) -> bool {
        let mgr = unsafe { ref_from_memory::<ZTResearchMgr>(this) };
        let bytes = serialize(&snapshot_mgr(mgr));

        let ok = unsafe { standalone::WRITE_BYTES_TO_FILE.hooked()(bytes.as_ptr() as *const u32, bytes.len() as u32, 1, file as *const i8) } == 1;
        if !ok {
            error!("research-save-reimplementation: WriteBytesToFile failed writing {} research bytes", bytes.len());
        }
        ok
    }

    #[detour(LOAD)]
    unsafe extern "thiscall" fn load(this: *const u32, file: *const u32, version: u32) -> bool {
        let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };

        for branch in mgr.branches_mut() {
            branch.current_funding_level = 0;
            for category in branch.categories_mut() {
                category.set_enabled(true);
                for program in category.programs_mut() {
                    program.reset();
                }
            }
        }

        let mut read_ok = true;
        if version >= MIN_VERSION_WITH_RESEARCH_DATA {
            read_ok = stream_io::read_i32(file).is_some();
            while read_ok && !stream_io::is_eof(file) {
                let Some(kind) = stream_io::read_i32(file) else {
                    read_ok = false;
                    break;
                };
                if kind < 0 {
                    break;
                }
                let Some(id) = stream_io::read_i32(file) else {
                    read_ok = false;
                    break;
                };
                if kind > 2 {
                    read_ok = false;
                    break;
                }
                match kind {
                    0 => {
                        let Some(value) = stream_io::read_i32(file) else {
                            read_ok = false;
                            break;
                        };
                        if let Some(branch) = mgr.get_branch_mut(id) {
                            let count = branch.funding_level_count() as u32;
                            branch.current_funding_level = if (value as u32) < count { value } else { 0 };
                        }
                    }
                    1 => {
                        let Some(value) = stream_io::read_u8(file) else {
                            read_ok = false;
                            break;
                        };
                        if let Some(category) = mgr.get_category_mut(id) {
                            category.set_enabled(value != 0);
                        }
                    }
                    2 => {
                        let Some(value) = stream_io::read_i32(file) else {
                            read_ok = false;
                            break;
                        };
                        if let Some(program) = mgr.get_program_mut(id) {
                            program.current_progress = f32::from_bits(value as u32);
                        }
                    }
                    _ => unreachable!("kind already range-checked above"),
                }
            }
        }

        if !read_ok {
            warn!("research-save-reimplementation: ZTResearchMgr::load stream read failed (version {version}); aborting without the on_completion/pick_random_program tail, matching vanilla");
            return false;
        }

        for program in mgr.branches_mut().flat_map(|b| b.categories_mut()).flat_map(|c| c.programs_mut()) {
            if program.is_complete() {
                program.on_completion();
            }
        }
        for branch in mgr.branches_mut() {
            branch.pick_random_program();
        }

        true
    }
}

#[cfg(not(feature = "vanilla-research-save"))]
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise research-save-reimplementation detours: {e:?}");
    }
}

#[cfg(feature = "vanilla-research-save")]
pub fn init() {}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    pub(crate) struct GeneratedProgram {
        pub(crate) id: i32,
        pub(crate) target_cost: f32,
        pub(crate) current_progress: f32,
        pub(crate) effect_kind_raw: i32,
    }

    pub(crate) struct GeneratedCategory {
        pub(crate) id: i32,
        pub(crate) enabled: u8,
        pub(crate) programs: Vec<GeneratedProgram>,
    }

    pub(crate) struct GeneratedBranch {
        pub(crate) id: i32,
        pub(crate) current_funding_level: i32,
        pub(crate) funding_level_count: usize,
        pub(crate) categories: Vec<GeneratedCategory>,
    }

    fn ptr_array_from_vec<T>(mut vec: Vec<u32>) -> ZTArray<T> {
        if vec.is_empty() {
            return ZTArray::from_raw_parts(0, 0, 0);
        }
        let ptr = vec.as_mut_ptr() as u32;
        let len = vec.len() as u32;
        let cap = vec.capacity() as u32;
        std::mem::forget(vec);
        ZTArray::from_raw_parts(ptr, ptr + len * 4, ptr + cap * 4)
    }

    fn vec_from_ptr_array<T>(array: &ZTArray<T>) -> Vec<u32> {
        let (start, end, buffer_end) = array.raw_parts();
        if start == 0 {
            return Vec::new();
        }
        let len = ((end - start) / 4) as usize;
        let cap = ((buffer_end - start) / 4) as usize;
        unsafe { Vec::from_raw_parts(start as *mut u32, len, cap) }
    }

    fn funding_table_from_vec(mut vec: Vec<ZTResearchFundingLevel>) -> (u32, u32, u32) {
        if vec.is_empty() {
            return (0, 0, 0);
        }
        let stride = size_of::<ZTResearchFundingLevel>() as u32;
        let ptr = vec.as_mut_ptr() as u32;
        let len = vec.len() as u32;
        let cap = vec.capacity() as u32;
        std::mem::forget(vec);
        (ptr, ptr + len * stride, ptr + cap * stride)
    }

    fn free_funding_table(start: u32, capacity_end: u32) {
        if start == 0 {
            return;
        }
        let stride = size_of::<ZTResearchFundingLevel>() as u32;
        let cap = ((capacity_end - start) / stride) as usize;
        drop(unsafe { Vec::<ZTResearchFundingLevel>::from_raw_parts(start as *mut ZTResearchFundingLevel, cap, cap) });
    }

    fn build_program(spec: &GeneratedProgram) -> *mut ZTResearchProgram {
        const NO_MATCHING_ENTITY: i32 = -1;
        let program = Box::new(ZTResearchProgram {
            config_file: BFConfigFile::default(),
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            desc_id: 0,
            icon_ptr: 0,
            entity_icon_ptr: 0,
            id: spec.id,
            target_cost: spec.target_cost,
            current_progress: spec.current_progress,
            priority: 0,
            target_id: NO_MATCHING_ENTITY,
            effect_kind_raw: spec.effect_kind_raw,
            effect_param_0: 0,
            effect_param_1: NO_MATCHING_ENTITY,
            effect_param_2: 0,
            help_id: 0,
        });
        Box::into_raw(program)
    }

    fn destroy_program(ptr: *mut ZTResearchProgram) {
        if ptr.is_null() {
            return;
        }
        drop(unsafe { Box::from_raw(ptr) });
    }

    pub(crate) fn build_standalone_program(effect_kind_raw: i32) -> *mut ZTResearchProgram {
        const NO_MATCHING_ENTITY: i32 = -1;
        Box::into_raw(Box::new(ZTResearchProgram {
            config_file: BFConfigFile::default(),
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            desc_id: 0,
            icon_ptr: 0,
            entity_icon_ptr: 0,
            id: 0,
            target_cost: 0.0,
            current_progress: 0.0,
            priority: 0,
            target_id: NO_MATCHING_ENTITY,
            effect_kind_raw,
            effect_param_0: 0,
            effect_param_1: NO_MATCHING_ENTITY,
            effect_param_2: 0,
            help_id: 0,
        }))
    }

    pub(crate) fn destroy_standalone_program(ptr: *mut ZTResearchProgram) {
        if ptr.is_null() {
            return;
        }
        drop(unsafe { Box::from_raw(ptr) });
    }

    pub(crate) fn build_standalone_funding_branch(current_funding_level: i32, levels: &[(i32, f32)]) -> *mut ZTResearchBranch {
        let funding_table = levels.iter().map(|&(name_id, cost)| ZTResearchFundingLevel { name_id, rate: 0.0, cost }).collect();
        let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);
        Box::into_raw(Box::new(ZTResearchBranch {
            config_file: BFConfigFile::default(),
            id: 0,
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            icon_ptr: 0,
            noprogicon_ptr: 0,
            current_category_ptr: 0,
            current_program_ptr: 0,
            category_array: ZTArray::from_raw_parts(0, 0, 0),
            current_funding_level,
            funding_table_start,
            funding_table_end,
            funding_table_capacity,
        }))
    }

    pub(crate) fn destroy_standalone_funding_branch(ptr: *mut ZTResearchBranch) {
        if ptr.is_null() {
            return;
        }
        let branch = unsafe { &*ptr };
        free_funding_table(branch.funding_table_start, branch.funding_table_capacity);
        drop(unsafe { Box::from_raw(ptr) });
    }

    fn build_category(spec: &GeneratedCategory) -> *mut ZTResearchCategory {
        let mut programs = Vec::with_capacity(spec.programs.len());
        for program in &spec.programs {
            programs.push(build_program(program) as u32);
        }
        let category = Box::new(ZTResearchCategory {
            config_file: BFConfigFile::default(),
            id: spec.id,
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            icon_ptr: 0,
            help_id: 0,
            expansion_id: 0,
            enabled: spec.enabled,
            pad2: [0; 3],
            program_array: ptr_array_from_vec(programs),
        });
        Box::into_raw(category)
    }

    fn destroy_category(ptr: *mut ZTResearchCategory) {
        if ptr.is_null() {
            return;
        }
        let category = unsafe { &*ptr };
        for program_ptr in vec_from_ptr_array(&category.program_array) {
            destroy_program(program_ptr as *mut ZTResearchProgram);
        }
        drop(unsafe { Box::from_raw(ptr) });
    }

    fn build_branch(spec: &GeneratedBranch) -> *mut ZTResearchBranch {
        let mut categories = Vec::with_capacity(spec.categories.len());
        for category in &spec.categories {
            categories.push(build_category(category) as u32);
        }
        let funding_table = vec![ZTResearchFundingLevel { name_id: 0, rate: 0.0, cost: 0.0 }; spec.funding_level_count];
        let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);
        let branch = Box::new(ZTResearchBranch {
            config_file: BFConfigFile::default(),
            id: spec.id,
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            icon_ptr: 0,
            noprogicon_ptr: 0,
            current_category_ptr: 0,
            current_program_ptr: 0,
            category_array: ptr_array_from_vec(categories),
            current_funding_level: spec.current_funding_level,
            funding_table_start,
            funding_table_end,
            funding_table_capacity,
        });
        Box::into_raw(branch)
    }

    fn destroy_branch(ptr: *mut ZTResearchBranch) {
        if ptr.is_null() {
            return;
        }
        let branch = unsafe { &*ptr };
        for category_ptr in vec_from_ptr_array(&branch.category_array) {
            destroy_category(category_ptr as *mut ZTResearchCategory);
        }
        free_funding_table(branch.funding_table_start, branch.funding_table_capacity);
        drop(unsafe { Box::from_raw(ptr) });
    }

    pub(crate) fn with_synthetic_branches<R>(mgr: &mut ZTResearchMgr, specs: &[GeneratedBranch], f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
        let original_branch_array_raw_parts = mgr.branch_array.raw_parts();

        let mut branch_ptrs = Vec::with_capacity(specs.len());
        for spec in specs {
            branch_ptrs.push(build_branch(spec) as u32);
        }
        mgr.branch_array = ptr_array_from_vec(branch_ptrs);

        let result = f(mgr);

        for branch_ptr in vec_from_ptr_array(&mgr.branch_array) {
            destroy_branch(branch_ptr as *mut ZTResearchBranch);
        }
        let (start, end, buffer_end) = original_branch_array_raw_parts;
        mgr.branch_array = ZTArray::from_raw_parts(start, end, buffer_end);

        result
    }

    pub(crate) fn with_standalone_mgr<R>(specs: &[GeneratedBranch], f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
        let mut mgr = Box::new(ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 0, branch_array: ZTArray::from_raw_parts(0, 0, 0) });
        with_synthetic_branches(&mut mgr, specs, f)
    }

    fn global_slot_address() -> u32 {
        get_module_base("zoo.exe") as u32 + 0x0023_9010
    }

    pub(crate) fn with_global_ztresearchmgr_ptr<R>(mgr: &mut ZTResearchMgr, f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
        let slot = global_slot_address();
        let original = get_from_memory::<u32>(slot);
        save_to_memory(slot, mgr as *mut ZTResearchMgr as u32);

        let result = f(mgr);

        save_to_memory(slot, original);
        result
    }

    pub(crate) fn build_update_test_branch(target_cost: f32, initial_progress: f32, funding_rate: f32, funding_cost: f32) -> *mut ZTResearchBranch {
        let program_ptr = build_program(&GeneratedProgram { id: 0, target_cost, current_progress: initial_progress, effect_kind_raw: -1 });

        let category_ptr = Box::into_raw(Box::new(ZTResearchCategory {
            config_file: BFConfigFile::default(),
            id: 0,
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            icon_ptr: 0,
            help_id: 0,
            expansion_id: 0,
            enabled: 1,
            pad2: [0; 3],
            program_array: ptr_array_from_vec(vec![program_ptr as u32]),
        }));

        let funding_table = vec![ZTResearchFundingLevel { name_id: 0, rate: funding_rate, cost: funding_cost }];
        let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);

        Box::into_raw(Box::new(ZTResearchBranch {
            config_file: BFConfigFile::default(),
            id: 0,
            cached_name: ZTBufferString::from_raw_parts(0, 0, 0),
            cached_desc: ZTBufferString::from_raw_parts(0, 0, 0),
            icon_ptr: 0,
            noprogicon_ptr: 0,
            current_category_ptr: category_ptr as u32,
            current_program_ptr: program_ptr as u32,
            category_array: ptr_array_from_vec(vec![category_ptr as u32]),
            current_funding_level: 0,
            funding_table_start,
            funding_table_end,
            funding_table_capacity,
        }))
    }

    pub(crate) fn destroy_update_test_branch(ptr: *mut ZTResearchBranch) {
        destroy_branch(ptr);
    }

    #[repr(C)]
    struct MgrWithZeroedExpansionFlag {
        mgr: ZTResearchMgr,
        always_check_expansion_flag: u8,
        _pad: [u8; 3],
    }

    pub(crate) fn with_update_test_branch<R>(
        target_cost: f32,
        initial_progress: f32,
        funding_rate: f32,
        funding_cost: f32,
        f: impl FnOnce(&mut ZTResearchMgr) -> R,
    ) -> R {
        let mut wrapper = Box::new(MgrWithZeroedExpansionFlag {
            mgr: ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 0, branch_array: ZTArray::from_raw_parts(0, 0, 0) },
            always_check_expansion_flag: 0,
            _pad: [0; 3],
        });
        let branch_ptr = build_update_test_branch(target_cost, initial_progress, funding_rate, funding_cost);
        wrapper.mgr.branch_array = ptr_array_from_vec(vec![branch_ptr as u32]);

        let result = with_global_ztresearchmgr_ptr(&mut wrapper.mgr, f);

        destroy_update_test_branch(branch_ptr);
        result
    }

    #[derive(Debug)]
    pub(crate) struct UpdateTestBranchSpec {
        pub(crate) target_cost: f32,
        pub(crate) initial_progress: f32,
        pub(crate) funding_rate: f32,
        pub(crate) funding_cost: f32,
    }

    fn build_update_test_branches(specs: &[UpdateTestBranchSpec]) -> Vec<*mut ZTResearchBranch> {
        specs
            .iter()
            .map(|spec| build_update_test_branch(spec.target_cost, spec.initial_progress, spec.funding_rate, spec.funding_cost))
            .collect()
    }

    pub(crate) fn with_update_test_branches<R>(specs: &[UpdateTestBranchSpec], f: impl FnOnce(&mut ZTResearchMgr) -> R) -> R {
        let mut wrapper = Box::new(MgrWithZeroedExpansionFlag {
            mgr: ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 0, branch_array: ZTArray::from_raw_parts(0, 0, 0) },
            always_check_expansion_flag: 0,
            _pad: [0; 3],
        });
        let branch_ptrs = build_update_test_branches(specs);
        wrapper.mgr.branch_array = ptr_array_from_vec(branch_ptrs.iter().map(|&ptr| ptr as u32).collect());

        let result = with_global_ztresearchmgr_ptr(&mut wrapper.mgr, f);

        for ptr in branch_ptrs {
            destroy_update_test_branch(ptr);
        }
        result
    }

    pub(crate) fn with_ztgamemgr_cash<R>(cash: f32, f: impl FnOnce() -> R) -> R {
        let gamemgr = unsafe { &mut *global_ztgamemgr_ptr() };
        let original = gamemgr.cash();
        gamemgr.set_cash(cash);

        let result = f();

        unsafe { &mut *global_ztgamemgr_ptr() }.set_cash(original);
        result
    }

    pub(crate) fn ztgamemgr_ptr_is_null() -> bool {
        global_ztgamemgr_ptr().is_null()
    }
}
