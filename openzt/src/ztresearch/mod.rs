//! Structs and methods for the vanilla `ZTResearchMgr` / `ZTResearchBranch` / `ZTResearchCategory` /
//! `ZTResearchProgram` classes, which drive the zoo's research tree: picking which program a branch
//! (e.g. "Animal Care") is currently working towards, tracking funding and progress, and applying a
//! program's effect once it completes.

#![allow(unused_imports)]

pub mod config;
pub mod mgr;
pub mod models;

#[allow(unused_imports)]
pub(crate) use config::research_config_reimplementation;
pub use mgr::*;
#[allow(unused_imports)]
pub(crate) use mgr::research_force_research_reimplementation;
#[allow(unused_imports)]
pub(crate) use mgr::research_program_completion_reimplementation;
pub(crate) use mgr::research_save_reimplementation;
#[allow(unused_imports)]
pub(crate) use mgr::research_update_reimplementation;
pub use models::*;

pub fn init() {
    config::research_config_reimplementation::init();
    mgr::init();
}
