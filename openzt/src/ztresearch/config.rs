// Native reimplementation of the .cfg-driven research tree loading
// (ZTResearchBranch::loadBranch/ZTResearchCategory::loadCategory/ZTResearchProgram::loadProgram).

pub(crate) mod research_config_reimplementation {
    use std::mem::size_of;
    use openzt_configparser::ini::Ini;
    use openzt_detour_macro::detour_mod;
    use tracing::{debug, error, info};

    use crate::bfconfigfile::ini_compat::{first, first_parse, read_cfg, values};
    use crate::bfconfigfile::BFConfigFile;
    use crate::string_registry::load_string_by_id;
    use crate::util::{get_from_memory, ZTArray, ZTBufferString, ZTString};
    use super::super::models::{ZTResearchBranch, ZTResearchCategory, ZTResearchFundingLevel, ZTResearchProgram};
    use super::super::mgr::ZTResearchMgr;

    #[derive(Debug, Default)]
    struct ReimplementedProgram {
        name_id: i32,
        desc_id: i32,
        icon: Option<String>,
        entity_icon: Option<String>,
        cost: f32,
        order: i32,
        target: i32,
        effect: i32,
        effect_params: (i32, i32, i32),
        help_id: i32,
    }

    #[derive(Debug, Default)]
    struct ReimplementedFundingLevel {
        name_id: i32,
        rate: f32,
        cost: f32,
    }

    #[derive(Debug, Default)]
    struct ReimplementedCategory {
        name_id: i32,
        desc_id: i32,
        icon: Option<String>,
        help_id: i32,
        expansion_id: i32,
        programs: Vec<ReimplementedProgram>,
    }

    #[derive(Debug, Default)]
    struct ReimplementedBranch {
        name_id: i32,
        desc_id: i32,
        icon: Option<String>,
        noprogicon: Option<String>,
        funding: Vec<ReimplementedFundingLevel>,
        categories: Vec<ReimplementedCategory>,
    }

    fn load_program(path: &str) -> Option<ReimplementedProgram> {
        let ini = read_cfg(path)?;
        Some(ReimplementedProgram {
            name_id: first_parse(&ini, "research", "name").unwrap_or_default(),
            desc_id: first_parse(&ini, "research", "desc").unwrap_or_default(),
            icon: first(&ini, "research", "icon"),
            entity_icon: first(&ini, "research", "entityIcon"),
            cost: first_parse(&ini, "research", "cost").unwrap_or_default(),
            order: first_parse(&ini, "research", "order").unwrap_or_default(),
            target: first_parse(&ini, "research", "target").unwrap_or(-1),
            effect: first_parse(&ini, "research", "effect").unwrap_or(-1),
            effect_params: (
                first_parse(&ini, "research", "effectval1").unwrap_or_default(),
                first_parse(&ini, "research", "effectval2").unwrap_or_default(),
                first_parse(&ini, "research", "effectval3").unwrap_or_default(),
            ),
            help_id: first_parse(&ini, "research", "helpid").unwrap_or_default(),
        })
    }

    fn load_category(path: &str) -> Option<ReimplementedCategory> {
        let ini = read_cfg(path)?;
        let programs = values(&ini, "category", "program").iter().filter_map(|p| load_program(p)).collect();
        Some(ReimplementedCategory {
            name_id: first_parse(&ini, "category", "name").unwrap_or_default(),
            desc_id: first_parse(&ini, "category", "desc").unwrap_or_default(),
            icon: first(&ini, "category", "icon"),
            help_id: first_parse(&ini, "category", "helpid").unwrap_or_default(),
            expansion_id: first_parse(&ini, "category", "expansion").unwrap_or_default(),
            programs,
        })
    }

    fn load_branch(ini: &Ini) -> ReimplementedBranch {
        let categories = values(ini, "branch", "category").iter().filter_map(|p| load_category(p)).collect();
        let funding = values(ini, "branch", "funding")
            .iter()
            .map(|block| ReimplementedFundingLevel {
                name_id: first_parse(ini, block, "name").unwrap_or_default(),
                rate: first_parse(ini, block, "work").unwrap_or_default(),
                cost: first_parse(ini, block, "cost").unwrap_or_default(),
            })
            .collect();
        ReimplementedBranch {
            name_id: first_parse(ini, "branch", "name").unwrap_or_default(),
            desc_id: first_parse(ini, "branch", "desc").unwrap_or_default(),
            icon: first(ini, "branch", "icon"),
            noprogicon: first(ini, "branch", "noprogicon"),
            funding,
            categories,
        }
    }

    /// Resolves a string id the same way `load_string_by_id` does, defaulting to an empty string
    /// (matching how `cached_name`/`cached_desc` read back when nothing was ever cached) instead of
    /// `None`, so it can be compared directly against `cached_name()`/`cached_desc()`.
    fn resolved(id: i32) -> String {
        load_string_by_id(id as u32).unwrap_or_default()
    }

    #[cfg(feature = "vanilla-research-config")]
    fn compare_program(path: &str, live: &ZTResearchProgram, reimpl: &ReimplementedProgram, mismatches: &mut Vec<String>) {
        if live.id() != reimpl.name_id {
            mismatches.push(format!("{path}: name id {} != {}", live.id(), reimpl.name_id));
        }
        if live.cached_name() != resolved(reimpl.name_id) {
            mismatches.push(format!("{path}: name text {:?} != {:?}", live.cached_name(), resolved(reimpl.name_id)));
        }
        if live.cached_desc() != resolved(reimpl.desc_id) {
            mismatches.push(format!("{path}: desc text {:?} != {:?}", live.cached_desc(), resolved(reimpl.desc_id)));
        }
        if live.icon() != reimpl.icon {
            mismatches.push(format!("{path}: icon {:?} != {:?}", live.icon(), reimpl.icon));
        }
        if live.entity_icon() != reimpl.entity_icon {
            mismatches.push(format!("{path}: entityIcon {:?} != {:?}", live.entity_icon(), reimpl.entity_icon));
        }
        if (live.target_cost() - reimpl.cost).abs() > f32::EPSILON {
            mismatches.push(format!("{path}: cost {} != {}", live.target_cost(), reimpl.cost));
        }
        if live.priority() != reimpl.order as u32 {
            mismatches.push(format!("{path}: order {} != {}", live.priority(), reimpl.order));
        }
        if live.target_id() != reimpl.target {
            mismatches.push(format!("{path}: target {} != {}", live.target_id(), reimpl.target));
        }
        if live.effect_kind_raw != reimpl.effect {
            mismatches.push(format!("{path}: effect {} != {}", live.effect_kind_raw, reimpl.effect));
        }
        if live.effect_params() != reimpl.effect_params {
            mismatches.push(format!("{path}: effect params {:?} != {:?}", live.effect_params(), reimpl.effect_params));
        }
        if live.help_id() != reimpl.help_id {
            mismatches.push(format!("{path}: helpid {} != {}", live.help_id(), reimpl.help_id));
        }
    }

    #[cfg(feature = "vanilla-research-config")]
    fn compare_category(path: &str, live: &ZTResearchCategory, reimpl: &ReimplementedCategory, mismatches: &mut Vec<String>) {
        if live.id() != reimpl.name_id {
            mismatches.push(format!("{path}: name id {} != {}", live.id(), reimpl.name_id));
        }
        if live.cached_name() != resolved(reimpl.name_id) {
            mismatches.push(format!("{path}: name text {:?} != {:?}", live.cached_name(), resolved(reimpl.name_id)));
        }
        if live.desc() != resolved(reimpl.desc_id) {
            mismatches.push(format!("{path}: desc text {:?} != {:?}", live.desc(), resolved(reimpl.desc_id)));
        }
        if live.icon() != reimpl.icon {
            mismatches.push(format!("{path}: icon {:?} != {:?}", live.icon(), reimpl.icon));
        }
        if live.help_id() != reimpl.help_id {
            mismatches.push(format!("{path}: helpid {} != {}", live.help_id(), reimpl.help_id));
        }
        if live.expansion_id() != reimpl.expansion_id {
            mismatches.push(format!("{path}: expansion {} != {}", live.expansion_id(), reimpl.expansion_id));
        }
        if live.program_count() != reimpl.programs.len() {
            mismatches.push(format!("{path}: program count {} != {}", live.program_count(), reimpl.programs.len()));
        }
        for (i, (live_program, reimpl_program)) in live.programs().zip(reimpl.programs.iter()).enumerate() {
            compare_program(&format!("{path}[program {i}]"), live_program, reimpl_program, mismatches);
        }
    }

    #[cfg(feature = "vanilla-research-config")]
    fn compare_branch(path: &str, live: &ZTResearchBranch, reimpl: &ReimplementedBranch, categories_before: usize, mismatches: &mut Vec<String>) {
        if live.id() != reimpl.name_id {
            mismatches.push(format!("{path}: name id {} != {}", live.id(), reimpl.name_id));
        }
        if live.cached_name() != resolved(reimpl.name_id) {
            mismatches.push(format!("{path}: name text {:?} != {:?}", live.cached_name(), resolved(reimpl.name_id)));
        }
        if live.desc() != resolved(reimpl.desc_id) {
            mismatches.push(format!("{path}: desc text {:?} != {:?}", live.desc(), resolved(reimpl.desc_id)));
        }
        if live.icon() != reimpl.icon {
            mismatches.push(format!("{path}: icon {:?} != {:?}", live.icon(), reimpl.icon));
        }
        if live.noprogicon() != reimpl.noprogicon {
            mismatches.push(format!("{path}: noprogicon {:?} != {:?}", live.noprogicon(), reimpl.noprogicon));
        }
        let live_funding = live.funding_levels();
        if live_funding.len() != reimpl.funding.len() {
            mismatches.push(format!("{path}: funding level count {} != {}", live_funding.len(), reimpl.funding.len()));
        }
        for (i, (live_level, reimpl_level)) in live_funding.iter().zip(reimpl.funding.iter()).enumerate() {
            if live_level.name_id() != reimpl_level.name_id {
                mismatches.push(format!("{path}[funding {i}]: name id {} != {}", live_level.name_id(), reimpl_level.name_id));
            }
            if (live_level.rate() - reimpl_level.rate).abs() > f32::EPSILON {
                mismatches.push(format!("{path}[funding {i}]: work/rate {} != {}", live_level.rate(), reimpl_level.rate));
            }
            if (live_level.cost() - reimpl_level.cost).abs() > f32::EPSILON {
                mismatches.push(format!("{path}[funding {i}]: cost {} != {}", live_level.cost(), reimpl_level.cost));
            }
        }
        let new_category_count = live.category_count().saturating_sub(categories_before);
        if new_category_count != reimpl.categories.len() {
            mismatches.push(format!(
                "{path}: newly appended category count {} != {} (already had {categories_before} before this call)",
                new_category_count,
                reimpl.categories.len()
            ));
        }
        for (i, (live_category, reimpl_category)) in live.categories().skip(categories_before).zip(reimpl.categories.iter()).enumerate() {
            compare_category(&format!("{path}[category {i}]"), live_category, reimpl_category, mismatches);
        }
    }

    #[cfg(feature = "vanilla-research-config")]
    fn peek_branch_id(path: &str) -> Option<i32> {
        let ini = read_cfg(path)?;
        first_parse(&ini, "branch", "name")
    }

    fn load_manifest(ini: &Ini) -> Vec<String> {
        values(ini, "branches", "branch")
    }

    #[cfg(feature = "vanilla-research-config")]
    struct ManifestEntry {
        path: String,
        id: i32,
        categories_before: usize,
    }

    #[cfg(feature = "vanilla-research-config")]
    fn manifest_entries(manifest_ini: &Ini, mgr_before: &ZTResearchMgr) -> Vec<ManifestEntry> {
        load_manifest(manifest_ini)
            .into_iter()
            .filter_map(|path| {
                let Some(id) = peek_branch_id(&path) else {
                    error!("research-config-reimplementation: failed to peek id from '{path}' listed in manifest");
                    return None;
                };
                let categories_before = mgr_before.get_branch(id).map(|b| b.category_count()).unwrap_or(0);
                Some(ManifestEntry { path, id, categories_before })
            })
            .collect()
    }

    #[cfg(feature = "vanilla-research-config")]
    fn compare_load_branches(manifest_path: &str, ids_before: &std::collections::HashSet<i32>, mgr_after: &ZTResearchMgr, entries: &[ManifestEntry], mismatches: &mut Vec<String>) {
        let new_id_count = entries.iter().map(|e| e.id).collect::<std::collections::HashSet<_>>().difference(ids_before).count();
        let expected_count = ids_before.len() + new_id_count;
        if mgr_after.branch_count() != expected_count {
            mismatches.push(format!(
                "{manifest_path}: branch_array has {} entries after, expected {} ({} before + {} new)",
                mgr_after.branch_count(),
                expected_count,
                ids_before.len(),
                new_id_count
            ));
        }

        for entry in entries {
            let Some(live_branch) = mgr_after.get_branch(entry.id) else {
                mismatches.push(format!("{}: branch id {} not found in branch_array after loadBranches", entry.path, entry.id));
                continue;
            };
            let Some(ini) = read_cfg(&entry.path) else {
                error!("research-config-reimplementation: failed to independently parse '{}' for comparison", entry.path);
                continue;
            };
            let reimpl = load_branch(&ini);
            compare_branch(&entry.path, live_branch, &reimpl, entry.categories_before, mismatches);
        }
    }

    #[cfg(not(feature = "vanilla-research-config"))]
    mod raw_mem {
        use std::ffi::{c_char, CString};

        use super::*;

        pub(super) fn alloc_buffer_string(text: &str) -> ZTBufferString {
            let mut bytes = text.as_bytes().to_vec();
            let len = bytes.len() as u32;
            bytes.push(0);
            let cap = bytes.capacity() as u32;
            let ptr = bytes.as_mut_ptr() as u32;
            std::mem::forget(bytes);
            ZTBufferString::from_raw_parts(ptr, ptr + len, ptr + cap)
        }

        pub(super) fn free_buffer_string(s: &ZTBufferString) {
            let (start, _end, buffer_end) = s.raw_parts();
            if start == 0 {
                return;
            }
            let cap = (buffer_end - start) as usize;
            drop(unsafe { Vec::<u8>::from_raw_parts(start as *mut u8, cap, cap) });
        }

        pub(super) fn alloc_owned_cstring(text: Option<&str>) -> u32 {
            match text.and_then(|t| CString::new(t).ok()) {
                Some(cstring) => cstring.into_raw() as u32,
                None => 0,
            }
        }

        pub(super) fn free_owned_cstring(ptr: u32) {
            if ptr != 0 {
                drop(unsafe { CString::from_raw(ptr as *mut c_char) });
            }
        }

        pub(super) fn vec_from_ptr_array<T>(array: &ZTArray<T>) -> Vec<u32> {
            let (start, end, buffer_end) = array.raw_parts();
            if start == 0 {
                return Vec::new();
            }
            let len = ((end - start) / 4) as usize;
            let cap = ((buffer_end - start) / 4) as usize;
            unsafe { Vec::from_raw_parts(start as *mut u32, len, cap) }
        }

        pub(super) fn ptr_array_from_vec<T>(mut vec: Vec<u32>) -> ZTArray<T> {
            if vec.is_empty() {
                return ZTArray::from_raw_parts(0, 0, 0);
            }
            let ptr = vec.as_mut_ptr() as u32;
            let len = vec.len() as u32;
            let cap = vec.capacity() as u32;
            std::mem::forget(vec);
            ZTArray::from_raw_parts(ptr, ptr + len * 4, ptr + cap * 4)
        }

        pub(super) fn free_ptr_array<T>(array: &ZTArray<T>) {
            let (start, _end, buffer_end) = array.raw_parts();
            if start == 0 {
                return;
            }
            let cap = ((buffer_end - start) / 4) as usize;
            drop(unsafe { Vec::<u32>::from_raw_parts(start as *mut u32, cap, cap) });
        }

        pub(super) fn funding_table_from_vec(mut vec: Vec<ZTResearchFundingLevel>) -> (u32, u32, u32) {
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

        pub(super) fn free_funding_table(branch: &ZTResearchBranch) {
            let start = branch.funding_table_start;
            if start == 0 {
                return;
            }
            let stride = size_of::<ZTResearchFundingLevel>() as u32;
            let cap = ((branch.funding_table_capacity - start) / stride) as usize;
            drop(unsafe { Vec::<ZTResearchFundingLevel>::from_raw_parts(start as *mut ZTResearchFundingLevel, cap, cap) });
        }
    }

    #[cfg(not(feature = "vanilla-research-config"))]
    mod construction {
        use super::{raw_mem::*, *};

        fn construct_program(reimpl: &ReimplementedProgram) -> *mut ZTResearchProgram {
            let program = Box::new(ZTResearchProgram {
                config_file: BFConfigFile::default(),
                cached_name: alloc_buffer_string(&resolved(reimpl.name_id)),
                cached_desc: alloc_buffer_string(&resolved(reimpl.desc_id)),
                desc_id: reimpl.desc_id,
                icon_ptr: alloc_owned_cstring(reimpl.icon.as_deref()),
                entity_icon_ptr: alloc_owned_cstring(reimpl.entity_icon.as_deref()),
                id: reimpl.name_id,
                target_cost: reimpl.cost,
                current_progress: 0.0,
                priority: reimpl.order as u32,
                target_id: reimpl.target,
                effect_kind_raw: reimpl.effect,
                effect_param_0: reimpl.effect_params.0,
                effect_param_1: reimpl.effect_params.1,
                effect_param_2: reimpl.effect_params.2,
                help_id: reimpl.help_id,
            });
            let ptr = Box::into_raw(program);
            unsafe { (*ptr).reset() };
            ptr
        }

        fn construct_category(reimpl: &ReimplementedCategory) -> *mut ZTResearchCategory {
            let mut programs = Vec::with_capacity(reimpl.programs.len());
            for program in &reimpl.programs {
                programs.push(construct_program(program) as u32);
            }
            let category = Box::new(ZTResearchCategory {
                config_file: BFConfigFile::default(),
                id: reimpl.name_id,
                cached_name: alloc_buffer_string(&resolved(reimpl.name_id)),
                cached_desc: alloc_buffer_string(&resolved(reimpl.desc_id)),
                icon_ptr: alloc_owned_cstring(reimpl.icon.as_deref()),
                help_id: reimpl.help_id,
                expansion_id: reimpl.expansion_id,
                enabled: 1,
                pad2: [0; 3],
                program_array: ptr_array_from_vec(programs),
            });
            Box::into_raw(category)
        }

        fn append_branch(mgr: &mut ZTResearchMgr, ptr: u32) {
            let mut branches = vec_from_ptr_array(&mgr.branch_array);
            branches.push(ptr);
            mgr.branch_array = ptr_array_from_vec(branches);
        }

        fn apply_branch(mgr: &mut ZTResearchMgr, reimpl: &ReimplementedBranch) {
            let id = reimpl.name_id;
            let found: Option<*mut ZTResearchBranch> = mgr.branches_mut().find(|b| b.id() == id).map(|b| b as *mut ZTResearchBranch);
            let ptr: *mut ZTResearchBranch = match found {
                Some(existing) => existing,
                None => {
                    let branch = Box::new(ZTResearchBranch {
                        config_file: BFConfigFile::default(),
                        id,
                        cached_name: alloc_buffer_string(""),
                        cached_desc: alloc_buffer_string(""),
                        icon_ptr: 0,
                        noprogicon_ptr: 0,
                        current_category_ptr: 0,
                        current_program_ptr: 0,
                        category_array: ZTArray::from_raw_parts(0, 0, 0),
                        current_funding_level: 0,
                        funding_table_start: 0,
                        funding_table_end: 0,
                        funding_table_capacity: 0,
                    });
                    let ptr = Box::into_raw(branch);
                    append_branch(mgr, ptr as u32);
                    ptr
                }
            };

            let branch = unsafe { &mut *ptr };

            branch.id = id;
            free_buffer_string(&branch.cached_name);
            branch.cached_name = alloc_buffer_string(&resolved(reimpl.name_id));
            free_buffer_string(&branch.cached_desc);
            branch.cached_desc = alloc_buffer_string(&resolved(reimpl.desc_id));
            free_owned_cstring(branch.icon_ptr);
            branch.icon_ptr = alloc_owned_cstring(reimpl.icon.as_deref());
            free_owned_cstring(branch.noprogicon_ptr);
            branch.noprogicon_ptr = alloc_owned_cstring(reimpl.noprogicon.as_deref());

            let mut categories = vec_from_ptr_array(&branch.category_array);
            for category in &reimpl.categories {
                categories.push(construct_category(category) as u32);
            }
            branch.category_array = ptr_array_from_vec(categories);

            free_funding_table(branch);
            let mut funding = Vec::with_capacity(reimpl.funding.len());
            for level in &reimpl.funding {
                funding.push(ZTResearchFundingLevel { name_id: level.name_id, rate: level.rate, cost: level.cost });
            }
            let (start, end, capacity) = funding_table_from_vec(funding);
            branch.funding_table_start = start;
            branch.funding_table_end = end;
            branch.funding_table_capacity = capacity;
            branch.current_funding_level = 0;

            branch.pick_random_program();
        }

        pub(super) fn parse_manifest(manifest_path: &str) -> Option<Vec<ReimplementedBranch>> {
            let manifest_ini = read_cfg(manifest_path)?;
            let paths = load_manifest(&manifest_ini);

            let mut parsed = Vec::with_capacity(paths.len());
            for path in &paths {
                let ini = read_cfg(path)?;
                parsed.push(load_branch(&ini));
            }
            Some(parsed)
        }

        pub(super) fn apply_all(mgr: &mut ZTResearchMgr, parsed: &[ReimplementedBranch]) {
            for reimpl in parsed {
                apply_branch(mgr, reimpl);
            }
        }
    }

    #[cfg(not(feature = "vanilla-research-config"))]
    pub(crate) mod destruction {
        use super::{raw_mem::*, *};

        fn destroy_each_ptr<T>(start_ptr: u32, end_ptr: u32, mut destroy: impl FnMut(*mut T)) {
            let mut addr = start_ptr;
            while addr < end_ptr {
                let ptr = get_from_memory::<u32>(addr) as *mut T;
                if !ptr.is_null() {
                    destroy(ptr);
                }
                addr += 4;
            }
        }

        pub(super) unsafe fn destroy_program(ptr: *mut ZTResearchProgram) {
            if ptr.is_null() {
                return;
            }
            let program = unsafe { &*ptr };
            free_buffer_string(&program.cached_name);
            free_buffer_string(&program.cached_desc);
            free_owned_cstring(program.icon_ptr);
            free_owned_cstring(program.entity_icon_ptr);
            drop(unsafe { Box::from_raw(ptr) });
        }

        pub(crate) fn reset_category_contents(category: &mut ZTResearchCategory) {
            category.id = -1;
            free_buffer_string(&category.cached_name);
            category.cached_name = alloc_buffer_string("");
            free_buffer_string(&category.cached_desc);
            category.cached_desc = alloc_buffer_string("");
            category.enabled = 1;
            free_owned_cstring(category.icon_ptr);
            category.icon_ptr = 0;
            category.help_id = 0;
            category.expansion_id = 0;
            let (start, end, buffer_end) = category.program_array.raw_parts();
            destroy_each_ptr::<ZTResearchProgram>(start, end, |p| unsafe { destroy_program(p) });
            category.program_array = ZTArray::from_raw_parts(start, start, buffer_end);
        }

        pub(super) unsafe fn destroy_category(ptr: *mut ZTResearchCategory) {
            if ptr.is_null() {
                return;
            }
            let category = unsafe { &mut *ptr };
            reset_category_contents(category);
            free_ptr_array(&category.program_array);
            free_buffer_string(&category.cached_desc);
            free_buffer_string(&category.cached_name);
            drop(unsafe { Box::from_raw(ptr) });
        }

        pub(crate) fn reset_branch_contents(branch: &mut ZTResearchBranch) {
            branch.id = -1;
            free_buffer_string(&branch.cached_name);
            branch.cached_name = alloc_buffer_string("");
            free_buffer_string(&branch.cached_desc);
            branch.cached_desc = alloc_buffer_string("");
            free_owned_cstring(branch.icon_ptr);
            branch.icon_ptr = 0;
            free_owned_cstring(branch.noprogicon_ptr);
            branch.noprogicon_ptr = 0;
            branch.current_category_ptr = 0;
            branch.current_program_ptr = 0;
            let (start, end, buffer_end) = branch.category_array.raw_parts();
            destroy_each_ptr::<ZTResearchCategory>(start, end, |p| unsafe { destroy_category(p) });
            branch.category_array = ZTArray::from_raw_parts(start, start, buffer_end);
            branch.current_funding_level = 0;
            branch.funding_table_end = branch.funding_table_start;
        }

        pub(super) unsafe fn destroy_branch(ptr: *mut ZTResearchBranch) {
            if ptr.is_null() {
                return;
            }
            let branch = unsafe { &mut *ptr };
            reset_branch_contents(branch);
            free_funding_table(branch);
            free_ptr_array(&branch.category_array);
            free_buffer_string(&branch.cached_desc);
            free_buffer_string(&branch.cached_name);
            drop(unsafe { Box::from_raw(ptr) });
        }

        pub(super) fn clear_branches(mgr: &mut ZTResearchMgr) {
            let (start, end, buffer_end) = mgr.branch_array.raw_parts();
            destroy_each_ptr::<ZTResearchBranch>(start, end, |p| unsafe { destroy_branch(p) });
            mgr.branch_array = ZTArray::from_raw_parts(start, start, buffer_end);
            mgr.elapsed_ticks = 0;
        }

        #[cfg(test)]
        mod tests {
            use super::*;

            fn build_test_program(id: i32) -> *mut ZTResearchProgram {
                Box::into_raw(Box::new(ZTResearchProgram {
                    config_file: BFConfigFile::default(),
                    cached_name: alloc_buffer_string("program name"),
                    cached_desc: alloc_buffer_string("program desc"),
                    desc_id: 0,
                    icon_ptr: alloc_owned_cstring(Some("icon.bmp")),
                    entity_icon_ptr: alloc_owned_cstring(Some("entity_icon.bmp")),
                    id,
                    target_cost: 0.0,
                    current_progress: 0.0,
                    priority: 0,
                    target_id: -1,
                    effect_kind_raw: -1,
                    effect_param_0: 0,
                    effect_param_1: -1,
                    effect_param_2: 0,
                    help_id: 0,
                }))
            }

            fn build_test_category(id: i32, program_count: usize) -> *mut ZTResearchCategory {
                let programs: Vec<u32> = (0..program_count).map(|i| build_test_program(i as i32) as u32).collect();
                Box::into_raw(Box::new(ZTResearchCategory {
                    config_file: BFConfigFile::default(),
                    id,
                    cached_name: alloc_buffer_string("category name"),
                    cached_desc: alloc_buffer_string("category desc"),
                    icon_ptr: alloc_owned_cstring(Some("category_icon.bmp")),
                    help_id: 0,
                    expansion_id: 0,
                    enabled: 1,
                    pad2: [0; 3],
                    program_array: ptr_array_from_vec(programs),
                }))
            }

            fn build_test_branch(id: i32, category_count: usize, funding_level_count: usize) -> *mut ZTResearchBranch {
                let categories: Vec<u32> = (0..category_count).map(|i| build_test_category(i as i32, 0) as u32).collect();
                let funding_table = vec![ZTResearchFundingLevel { name_id: 0, rate: 0.0, cost: 0.0 }; funding_level_count];
                let (funding_table_start, funding_table_end, funding_table_capacity) = funding_table_from_vec(funding_table);
                Box::into_raw(Box::new(ZTResearchBranch {
                    config_file: BFConfigFile::default(),
                    id,
                    cached_name: alloc_buffer_string("branch name"),
                    cached_desc: alloc_buffer_string("branch desc"),
                    icon_ptr: alloc_owned_cstring(Some("branch_icon.bmp")),
                    noprogicon_ptr: alloc_owned_cstring(Some("noprogicon.bmp")),
                    current_category_ptr: 0,
                    current_program_ptr: 0,
                    category_array: ptr_array_from_vec(categories),
                    current_funding_level: 0,
                    funding_table_start,
                    funding_table_end,
                    funding_table_capacity,
                }))
            }

            #[test]
            fn reset_category_contents_clears_and_reallocates_in_place() {
                let category_ptr = build_test_category(42, 2);
                let category = unsafe { &mut *category_ptr };
                let original_program_array_capacity = category.program_array.capacity();
                assert!(original_program_array_capacity > 0);

                reset_category_contents(category);

                assert_eq!(category.id, -1);
                let (name_start, _, _) = category.cached_name.raw_parts();
                assert_ne!(name_start, 0, "cached_name must be re-allocated, not left null");
                assert_eq!(category.cached_name.copy_to_string(), "");
                let (desc_start, _, _) = category.cached_desc.raw_parts();
                assert_ne!(desc_start, 0, "cached_desc must be re-allocated, not left null");
                assert_eq!(category.cached_desc.copy_to_string(), "");
                assert_eq!(category.enabled, 1);
                assert_eq!(category.icon_ptr, 0);
                assert_eq!(category.program_array.len(), 0);
                assert_eq!(category.program_array.capacity(), original_program_array_capacity);

                free_ptr_array(&category.program_array);
                free_buffer_string(&category.cached_desc);
                free_buffer_string(&category.cached_name);
                drop(unsafe { Box::from_raw(category_ptr) });
            }

            #[test]
            fn reset_branch_contents_clears_and_reallocates_in_place() {
                let branch_ptr = build_test_branch(7, 2, 3);
                let branch = unsafe { &mut *branch_ptr };
                branch.current_category_ptr = 0x1234;
                branch.current_program_ptr = 0x5678;
                let original_category_array_capacity = branch.category_array.capacity();
                let original_funding_table_capacity = branch.funding_table_capacity;
                assert!(original_category_array_capacity > 0);

                reset_branch_contents(branch);

                assert_eq!(branch.id, -1);
                let (name_start, _, _) = branch.cached_name.raw_parts();
                assert_ne!(name_start, 0, "cached_name must be re-allocated, not left null");
                assert_eq!(branch.cached_name.copy_to_string(), "");
                let (desc_start, _, _) = branch.cached_desc.raw_parts();
                assert_ne!(desc_start, 0, "cached_desc must be re-allocated, not left null");
                assert_eq!(branch.cached_desc.copy_to_string(), "");
                assert_eq!(branch.icon_ptr, 0);
                assert_eq!(branch.noprogicon_ptr, 0);
                assert_eq!(branch.current_category_ptr, 0);
                assert_eq!(branch.current_program_ptr, 0);
                assert_eq!(branch.category_array.len(), 0);
                assert_eq!(branch.category_array.capacity(), original_category_array_capacity);
                assert_eq!(branch.current_funding_level, 0);
                assert_eq!(branch.funding_table_end, branch.funding_table_start);
                assert_eq!(branch.funding_table_capacity, original_funding_table_capacity, "funding table capacity must be retained, not freed");

                free_funding_table(branch);
                free_ptr_array(&branch.category_array);
                free_buffer_string(&branch.cached_desc);
                free_buffer_string(&branch.cached_name);
                drop(unsafe { Box::from_raw(branch_ptr) });
            }

            #[test]
            fn destroy_program_does_not_panic() {
                let program_ptr = build_test_program(1);
                unsafe { destroy_program(program_ptr) };
            }

            #[test]
            fn destroy_category_does_not_panic() {
                let category_ptr = build_test_category(1, 2);
                unsafe { destroy_category(category_ptr) };
            }

            #[test]
            fn destroy_branch_does_not_panic() {
                let branch_ptr = build_test_branch(1, 2, 2);
                unsafe { destroy_branch(branch_ptr) };
            }

            #[test]
            fn clear_branches_empties_array_and_resets_elapsed_ticks() {
                let branches: Vec<u32> = (0..2).map(|i| build_test_branch(i, 1, 1) as u32).collect();
                let mut mgr = ZTResearchMgr { pad0: [0; 8], elapsed_ticks: 123, branch_array: ptr_array_from_vec(branches) };
                let original_branch_array_capacity = mgr.branch_array.capacity();
                assert!(original_branch_array_capacity > 0);

                clear_branches(&mut mgr);

                assert_eq!(mgr.elapsed_ticks(), 0);
                assert_eq!(mgr.branch_array.len(), 0);
                assert_eq!(mgr.branch_array.capacity(), original_branch_array_capacity);

                free_ptr_array(&mgr.branch_array);
            }
        }
    }

    #[cfg(feature = "vanilla-research-config")]
    #[detour_mod]
    pub(crate) mod detours {
        use std::{collections::HashSet, ffi::CStr};

        use openzt_detour::generated::{
            ztresearchbranch::{CLEAR_BRANCH, DESTRUCTOR as ZTRESEARCHBRANCH_DESTRUCTOR},
            ztresearchcategory::{CLEAR_CATEGORY, DESTRUCTOR as ZTRESEARCHCATEGORY_DESTRUCTOR},
            ztresearchmgr::{CLEAR_BRANCHES, LOAD_BRANCHES},
            ztresearchprogram::DESTRUCTOR as ZTRESEARCHPROGRAM_DESTRUCTOR,
        };

        use super::*;
        use crate::util::ref_from_memory;

        #[detour(LOAD_BRANCHES)]
        unsafe extern "thiscall" fn load_branches(this: *const u32, manifest_path: *const i8) -> bool {
            let manifest_path_str = unsafe { CStr::from_ptr(manifest_path) }.to_string_lossy().into_owned();

            let (ids_before, entries) = match super::read_cfg(&manifest_path_str) {
                Some(manifest_ini) => {
                    let mgr_before = unsafe { ref_from_memory::<ZTResearchMgr>(this) };
                    let ids_before: HashSet<i32> = mgr_before.branches().map(|b| b.id()).collect();
                    let entries = super::manifest_entries(&manifest_ini, mgr_before);
                    (Some(ids_before), Some(entries))
                }
                None => {
                    error!("research-config-reimplementation: failed to independently parse manifest '{manifest_path_str}'");
                    (None, None)
                }
            };

            let result = unsafe { LOAD_BRANCHES_DETOUR.call(this, manifest_path) };
            debug!("research-config-reimplementation: ZTResearchMgr::loadBranches(\"{manifest_path_str}\") called, this={:#x}, vanilla result: {result}", this as u32);

            if !result {
                return result;
            }
            let (Some(ids_before), Some(entries)) = (ids_before, entries) else {
                return result;
            };

            let mgr_after = unsafe { ref_from_memory::<ZTResearchMgr>(this) };
            let mut mismatches = Vec::new();
            super::compare_load_branches(&manifest_path_str, &ids_before, mgr_after, &entries, &mut mismatches);
            if mismatches.is_empty() {
                if !entries.is_empty() {
                    info!(
                        "research-config-reimplementation: '{manifest_path_str}' matches reimplementation ({} branches in manifest)",
                        entries.len()
                    );
                } else {
                    debug!("research-config-reimplementation: '{manifest_path_str}' matches reimplementation (0 branches in manifest)");
                }
            } else {
                for mismatch in &mismatches {
                    error!("research-config-reimplementation mismatch: {mismatch}");
                }
            }

            result
        }

        #[detour(CLEAR_BRANCHES)]
        unsafe extern "thiscall" fn clear_branches(this: *const u32) {
            unsafe { CLEAR_BRANCHES_DETOUR.call(this) }
        }

        #[detour(CLEAR_BRANCH)]
        unsafe extern "thiscall" fn clear_branch(this: *const u32) {
            unsafe { CLEAR_BRANCH_DETOUR.call(this) }
        }

        #[detour(ZTRESEARCHBRANCH_DESTRUCTOR)]
        unsafe extern "thiscall" fn ztresearch_branch_dtor(this: *const u32) {
            unsafe { ZTRESEARCHBRANCH_DESTRUCTOR_DETOUR.call(this) }
        }

        #[detour(CLEAR_CATEGORY)]
        unsafe extern "thiscall" fn clear_category(this: *const u32) {
            unsafe { CLEAR_CATEGORY_DETOUR.call(this) }
        }

        #[detour(ZTRESEARCHCATEGORY_DESTRUCTOR)]
        unsafe extern "thiscall" fn ztresearch_category_dtor(this: *const u32) {
            unsafe { ZTRESEARCHCATEGORY_DESTRUCTOR_DETOUR.call(this) }
        }

        #[detour(ZTRESEARCHPROGRAM_DESTRUCTOR)]
        unsafe extern "thiscall" fn ztresearch_program_dtor(this: *const u32) {
            unsafe { ZTRESEARCHPROGRAM_DESTRUCTOR_DETOUR.call(this) }
        }

        pub(crate) unsafe fn call_original_clear_branch(this: *const u32) {
            unsafe { CLEAR_BRANCH_DETOUR.call(this) }
        }
        pub(crate) unsafe fn call_original_clear_category(this: *const u32) {
            unsafe { CLEAR_CATEGORY_DETOUR.call(this) }
        }
    }

    #[cfg(not(feature = "vanilla-research-config"))]
    #[detour_mod]
    pub(crate) mod detours {
        use std::{ffi::CStr, panic::AssertUnwindSafe};

        use openzt_detour::generated::{
            ztresearchbranch::{CLEAR_BRANCH, DESTRUCTOR as ZTRESEARCHBRANCH_DESTRUCTOR},
            ztresearchcategory::{CLEAR_CATEGORY, DESTRUCTOR as ZTRESEARCHCATEGORY_DESTRUCTOR},
            ztresearchmgr::{CLEAR_BRANCHES, LOAD_BRANCHES},
            ztresearchprogram::DESTRUCTOR as ZTRESEARCHPROGRAM_DESTRUCTOR,
        };

        use super::*;
        use crate::util::mut_from_memory;

        #[detour(LOAD_BRANCHES)]
        unsafe extern "thiscall" fn load_branches(this: *const u32, manifest_path: *const i8) -> bool {
            let manifest_path_str = unsafe { CStr::from_ptr(manifest_path) }.to_string_lossy().into_owned();
            debug!("research-config-reimplementation: loadBranches(\"{manifest_path_str}\") called, this={:#x}", this as u32);

            let parsed = match std::panic::catch_unwind(AssertUnwindSafe(|| super::construction::parse_manifest(&manifest_path_str))) {
                Ok(Some(parsed)) => parsed,
                Ok(None) | Err(_) => {
                    error!("research-config-reimplementation: failed to parse '{manifest_path_str}', falling back to vanilla");
                    return unsafe { LOAD_BRANCHES_DETOUR.call(this, manifest_path) };
                }
            };

            let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };
            let branch_count = parsed.len();
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::construction::apply_all(mgr, &parsed))).is_err() {
                error!("research-config-reimplementation: panic while applying '{manifest_path_str}'; branch_array may be partially updated, not falling back");
            } else if branch_count > 0 {
                info!("research-config-reimplementation: replaced loadBranches(\"{manifest_path_str}\") natively ({branch_count} branches in manifest)");
            } else {
                debug!("research-config-reimplementation: replaced loadBranches(\"{manifest_path_str}\") natively (0 branches in manifest)");
            }
            true
        }

        #[detour(CLEAR_BRANCHES)]
        unsafe extern "thiscall" fn clear_branches(this: *const u32) {
            let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::destruction::clear_branches(mgr))).is_err() {
                error!("research-config-reimplementation: panic while clearing branches; memory may be leaked");
            }
        }

        #[detour(CLEAR_BRANCH)]
        unsafe extern "thiscall" fn clear_branch(this: *const u32) {
            let branch = unsafe { mut_from_memory::<ZTResearchBranch>(this) };
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::destruction::reset_branch_contents(branch))).is_err() {
                error!("research-config-reimplementation: panic while resetting branch contents; memory may be leaked");
            }
        }

        #[detour(ZTRESEARCHBRANCH_DESTRUCTOR)]
        unsafe extern "thiscall" fn ztresearch_branch_dtor(this: *const u32) {
            let ptr = this as *mut ZTResearchBranch;
            if std::panic::catch_unwind(AssertUnwindSafe(|| unsafe { super::destruction::destroy_branch(ptr) })).is_err() {
                error!("research-config-reimplementation: panic while destroying branch; memory may be leaked");
            }
        }

        #[detour(CLEAR_CATEGORY)]
        unsafe extern "thiscall" fn clear_category(this: *const u32) {
            let category = unsafe { mut_from_memory::<ZTResearchCategory>(this) };
            if std::panic::catch_unwind(AssertUnwindSafe(|| super::destruction::reset_category_contents(category))).is_err() {
                error!("research-config-reimplementation: panic while resetting category contents; memory may be leaked");
            }
        }

        #[detour(ZTRESEARCHCATEGORY_DESTRUCTOR)]
        unsafe extern "thiscall" fn ztresearch_category_dtor(this: *const u32) {
            let ptr = this as *mut ZTResearchCategory;
            if std::panic::catch_unwind(AssertUnwindSafe(|| unsafe { super::destruction::destroy_category(ptr) })).is_err() {
                error!("research-config-reimplementation: panic while destroying category; memory may be leaked");
            }
        }

        #[detour(ZTRESEARCHPROGRAM_DESTRUCTOR)]
        unsafe extern "thiscall" fn ztresearch_program_dtor(this: *const u32) {
            let ptr = this as *mut ZTResearchProgram;
            if std::panic::catch_unwind(AssertUnwindSafe(|| unsafe { super::destruction::destroy_program(ptr) })).is_err() {
                error!("research-config-reimplementation: panic while destroying program; memory may be leaked");
            }
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            error!("Failed to initialise research-config-reimplementation detours: {e:?}");
        }
    }
}
