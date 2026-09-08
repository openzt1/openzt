//! Per-class comparison-test bodies for the reimplementation battery - one file per tested class,
//! mirroring the production `openzt/src/<class>.rs` layout. Each file holds only that class's
//! `run_..._test(failure_log: &mut Option<std::fs::File>) -> bool` functions plus any private
//! proptest strategies/case structs they need; `battery.rs` references them by path when building
//! its ordered `RegisteredTest` lists. See the parent module's doc comment for the "how to add a
//! new test" guide.

pub(crate) mod footprints;
pub(crate) mod ztadvterrainmgr;
pub(crate) mod ztawardmgr;
pub(crate) mod ztgamemgr;
pub(crate) mod ztgamemgr_menumusichandler;
pub(crate) mod ztguest;
pub(crate) mod zthabitatmgr;
pub(crate) mod ztmarketing;
pub(crate) mod ztmegatilemgr;
pub(crate) mod zoostatus;
pub(crate) mod ztresearch;
pub(crate) mod ztscenariosimplegoal;
pub(crate) mod ztshow;
pub(crate) mod ztshowmgr;
pub(crate) mod ztshowscriptmgr;
pub(crate) mod ztshowinfo;
pub(crate) mod ztshowstate;
pub(crate) mod ztshowui;
pub(crate) mod ztsoundscape;
pub(crate) mod ztthoughtmgr;
