#![allow(unused_imports)]

pub mod ztresearchmgr;
pub use ztresearchmgr::*;

pub(crate) mod save;
pub(crate) use save as research_save_reimplementation;

use openzt_detour_macro::detour_mod;
use tracing::{error, info, warn};

use super::models::{ZTResearchBranch, ZTResearchCategory, ZTResearchProgram};
use crate::{
    command_console::CommandError,
    globals::{get_module_base, globals},
    lua_fn,
    util::get_from_memory,
};

const MAX_REASONABLE_BRANCHES: usize = 32;
const MAX_REASONABLE_CATEGORIES: usize = 128;
const MAX_REASONABLE_PROGRAMS: usize = 512;

/// Detours `ZTResearchMgr::forceResearch` onto `ZTResearchMgr::force_research`.
pub(crate) mod research_force_research_reimplementation {
    use openzt_detour_macro::detour_mod;

    use super::ztresearchmgr::ZTResearchMgr;

    #[detour_mod]
    mod detours {
        use openzt_detour::generated::ztresearchmgr::FORCE_RESEARCH;

        use super::ZTResearchMgr;
        use crate::util::mut_from_memory;

        #[detour(FORCE_RESEARCH)]
        unsafe extern "thiscall" fn force_research(this: *const u32, continue_program: bool) {
            let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };
            mgr.force_research(continue_program);
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            tracing::error!("Failed to initialise research-force-research-reimplementation detours: {e:?}");
        }
    }
}

/// Detours `ZTResearchProgram::onCompletion`/`reset` onto `ZTResearchProgram::on_completion`/`reset`.
pub(crate) mod research_program_completion_reimplementation {
    use openzt_detour_macro::detour_mod;

    use super::ZTResearchProgram;

    #[detour_mod]
    mod detours {
        use openzt_detour::generated::ztresearchprogram::{ON_COMPLETION, RESET};

        use super::ZTResearchProgram;
        use crate::util::mut_from_memory;

        #[detour(ON_COMPLETION)]
        unsafe extern "thiscall" fn on_completion(this: *const u32) -> bool {
            let program = unsafe { mut_from_memory::<ZTResearchProgram>(this) };
            program.on_completion() != 0
        }

        #[detour(RESET)]
        unsafe extern "thiscall" fn reset(this: *const u32) -> bool {
            let program = unsafe { mut_from_memory::<ZTResearchProgram>(this) };
            program.reset() != 0
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            tracing::error!("Failed to initialise research-program-completion-reimplementation detours: {e:?}");
        }
    }
}

/// Detours `ZTResearchMgr::update` onto `ZTResearchMgr::update`/`ZTResearchBranch::update`.
pub(crate) mod research_update_reimplementation {
    use openzt_detour_macro::detour_mod;

    use super::ztresearchmgr::ZTResearchMgr;

    #[detour_mod]
    mod detours {
        use openzt_detour::generated::ztresearchmgr::UPDATE;

        use super::ZTResearchMgr;
        use crate::util::mut_from_memory;

        #[detour(UPDATE)]
        unsafe extern "thiscall" fn update(this: *const u32, delta_ticks: u32) -> i32 {
            let mgr = unsafe { mut_from_memory::<ZTResearchMgr>(this) };
            mgr.update(delta_ticks);
            0
        }
    }

    pub fn init() {
        if let Err(e) = unsafe { detours::init_detours() } {
            tracing::error!("Failed to initialise research-update-reimplementation detours: {e:?}");
        }
    }
}

/// a command that prints the top-level ZTResearchMgr summary
/// usage: `get_ztresearchmgr`
fn command_get_ztresearchmgr(_args: Vec<&str>) -> Result<String, CommandError> {
    Ok(format!("{}", globals().ztresearchmgr()))
}

/// a command that resolves the ZTResearchMgr global step by step, logging each hop
/// usage: `debug_research_ptr`
fn command_debug_research_ptr(_args: Vec<&str>) -> Result<String, CommandError> {
    let base = get_module_base("zoo.exe");
    info!("[research-debug] module base (\"zoo.exe\") = {:#x}", base);

    let global_slot = base as u32 + 0x0023_9010;
    info!("[research-debug] global slot address (base + 0x239010) = {:#x}", global_slot);

    let raw_slot_value = get_from_memory::<u32>(global_slot);
    info!("[research-debug] value stored at global slot = {:#x}", raw_slot_value);

    if raw_slot_value == 0 {
        error!("[research-debug] global slot is null; ZTResearchMgr not constructed yet, or the address/indirection is wrong");
        return Err(CommandError::new("global slot at base + 0x239010 is null".to_string()));
    }

    let mgr_ptr = globals().ztresearchmgr_ptr();
    info!("[research-debug] globals().ztresearchmgr_ptr() = {:#x}", mgr_ptr as u32);

    if mgr_ptr.is_null() {
        error!("[research-debug] resolved ZTResearchMgr pointer is null");
        return Err(CommandError::new("resolved ZTResearchMgr pointer is null".to_string()));
    }

    Ok(format!(
        "module_base={:#x} global_slot={:#x} raw_slot_value={:#x} mgr_ptr={:#x}",
        base, global_slot, raw_slot_value, mgr_ptr as u32
    ))
}

/// a command that prints every branch/category/program in the research tree
/// usage: `list_research`
fn command_list_research(_args: Vec<&str>) -> Result<String, CommandError> {
    let mgr_ptr = globals().ztresearchmgr_ptr();
    info!("[research-debug] ztresearchmgr_ptr() = {:#x}", mgr_ptr as u32);
    if mgr_ptr.is_null() {
        error!("[research-debug] ZTResearchMgr pointer is null; run debug_research_ptr() for details");
        return Err(CommandError::new("ZTResearchMgr pointer is null".to_string()));
    }

    let mgr = globals().ztresearchmgr();
    info!("[research-debug] mgr.elapsed_ticks = {}", mgr.elapsed_ticks);

    let branch_count = mgr.branch_array.len();
    info!("[research-debug] mgr.branch_array.len() = {}", branch_count);
    if branch_count > MAX_REASONABLE_BRANCHES {
        error!(
            "[research-debug] branch_array.len() = {} exceeds MAX_REASONABLE_BRANCHES ({}); bailing out before dereferencing to avoid a crash - the global address/indirection is probably wrong",
            branch_count, MAX_REASONABLE_BRANCHES
        );
        return Err(CommandError::new(format!("implausible branch count {}", branch_count)));
    }

    let mut result = format!("{}\n", mgr);

    for i in 0..branch_count {
        let branch_ptr = mgr.branch_array.get_ptr(i);
        info!("[research-debug] branch[{}] ptr = {:#x}", i, branch_ptr);
        if branch_ptr == 0 {
            warn!("[research-debug] branch[{}] is null, skipping", i);
            continue;
        }
        let branch = mgr.branch(i);
        result.push_str(&format!("{}\n", branch));

        for (level_index, level) in branch.funding_levels().iter().enumerate() {
            result.push_str(&format!(
                "  FundingLevel[{}]: name={:?} rate(work)={} cost={}\n",
                level_index,
                level.name(),
                level.rate(),
                level.cost()
            ));
        }

        let category_count = branch.category_array.len();
        info!("[research-debug] branch[{}] (id={}) category_array.len() = {}", i, branch.id, category_count);
        if category_count > MAX_REASONABLE_CATEGORIES {
            error!(
                "[research-debug] branch[{}].category_array.len() = {} exceeds MAX_REASONABLE_CATEGORIES ({}); skipping this branch's categories",
                i, category_count, MAX_REASONABLE_CATEGORIES
            );
            continue;
        }

        for j in 0..category_count {
            let category_ptr = branch.category_array.get_ptr(j);
            info!("[research-debug] branch[{}].category[{}] ptr = {:#x}", i, j, category_ptr);
            if category_ptr == 0 {
                warn!("[research-debug] branch[{}].category[{}] is null, skipping", i, j);
                continue;
            }
            let category = branch.category(j);
            result.push_str(&format!("{}\n", category));

            let program_count = category.program_array.len();
            info!(
                "[research-debug] branch[{}].category[{}] (id={}) program_array.len() = {}",
                i, j, category.id, program_count
            );
            if program_count > MAX_REASONABLE_PROGRAMS {
                error!(
                    "[research-debug] branch[{}].category[{}].program_array.len() = {} exceeds MAX_REASONABLE_PROGRAMS ({}); skipping this category's programs",
                    i, j, program_count, MAX_REASONABLE_PROGRAMS
                );
                continue;
            }

            for k in 0..program_count {
                let program_ptr = category.program_array.get_ptr(k);
                info!("[research-debug] branch[{}].category[{}].program[{}] ptr = {:#x}", i, j, k, program_ptr);
                if program_ptr == 0 {
                    warn!("[research-debug] branch[{}].category[{}].program[{}] is null, skipping", i, j, k);
                    continue;
                }
                let program = category.program(k);
                result.push_str(&format!("{}\n", program));
            }
        }
    }

    Ok(result)
}

/// a command that prints each branch's currently selected category/program and funding level
/// usage: `current_research`
fn command_current_research(_args: Vec<&str>) -> Result<String, CommandError> {
    let mgr = globals().ztresearchmgr();
    let mut result = String::new();

    for branch in mgr.branches() {
        result.push_str(&format!(
            "Branch {} ({:?}) - funding level {}",
            branch.id(),
            branch.name(),
            branch.current_funding_level()
        ));
        match branch.current_funding_rate() {
            Some(rate) => result.push_str(&format!(" (rate {}):\n", rate)),
            None => result.push_str(" (rate unavailable):\n"),
        }

        match branch.current_category() {
            Some(category) => {
                result.push_str(&format!("  category: id={} name={:?}\n", category.id(), category.name()))
            }
            None => result.push_str("  category: none selected\n"),
        }

        match branch.current_program() {
            Some(program) => {
                result.push_str(&format!(
                    "  program: id={}, progress={:.1}/{:.1}",
                    program.id(),
                    program.current_progress(),
                    program.target_cost()
                ));
                match branch.pct_remaining_on_program() {
                    Some(pct) => result.push_str(&format!(", {}% remaining", pct)),
                    None => result.push_str(", % remaining unavailable"),
                }
                match branch.days_remaining_on_program() {
                    Some(days) => result.push_str(&format!(", {:.1} days remaining\n", days)),
                    None => result.push_str(", days remaining unavailable\n"),
                }
            }
            None => result.push_str("  program: none selected\n"),
        }
    }

    Ok(result)
}

/// a command that would restore research progress from the active save file via `ZTResearchMgr::load`.
/// usage: `load_research`
fn command_load_research(_args: Vec<&str>) -> Result<String, CommandError> {
    Err(CommandError::new(
        "load_research is currently unsupported: ZTResearchMgr::load needs a real save-file stream pointer and version, which aren't available from the console".to_string(),
    ))
}

/// a command that triggers the same "force research" cheat the in-game button does
/// usage: `force_research`
fn command_force_research(_args: Vec<&str>) -> Result<String, CommandError> {
    force_research_cheat();
    Ok("Forced research completion".to_string())
}

pub fn register_lua_commands() {
    lua_fn!("get_ztresearchmgr", "Returns ZTResearchMgr debug info", "get_ztresearchmgr()", || {
        match command_get_ztresearchmgr(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    lua_fn!(
        "debug_research_ptr",
        "Resolves the ZTResearchMgr global step by step, logging each hop",
        "debug_research_ptr()",
        || {
            match command_debug_research_ptr(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    lua_fn!(
        "list_research",
        "Lists every research branch/category/program, logging every pointer as it's walked",
        "list_research()",
        || {
            match command_list_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    lua_fn!(
        "current_research",
        "Shows each branch's currently selected category/program and funding level",
        "current_research()",
        || {
            match command_current_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    lua_fn!(
        "load_research",
        "Restores research progress from the active save file",
        "load_research()",
        || {
            match command_load_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    lua_fn!(
        "force_research",
        "Triggers the same 'force research' cheat the in-game button does",
        "force_research()",
        || {
            match command_force_research(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );
}

pub fn init() {
    register_lua_commands();
    save::init();
    research_force_research_reimplementation::init();
    research_program_completion_reimplementation::init();
    research_update_reimplementation::init();
}
