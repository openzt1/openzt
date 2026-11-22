//! Per-class comparison-test bodies for the reimplementation battery - one file per tested class,
//! mirroring the production `openzt/src/<class>.rs` layout. Each file holds only that class's
//! `run_..._test(failure_log: &mut Option<std::fs::File>) -> bool` functions plus any private
//! proptest strategies/case structs they need; `battery.rs` references them by path when building
//! its ordered `RegisteredTest` lists. See the parent module's doc comment for the "how to add a
//! new test" guide.

pub(crate) mod ambients;
pub(crate) mod bfevent;
pub(crate) mod bfbehaviorset;
pub(crate) mod bfentity;
pub(crate) mod bfentity_io;
pub(crate) mod bfunit;
pub(crate) mod gxmixer;
pub(crate) mod owned_strings;
pub(crate) mod real_unit;
pub(crate) mod bfscriptmgr;
pub(crate) mod footprints;
pub(crate) mod ztadvterrainmgr;
pub(crate) mod ztaimgr;
pub(crate) mod ztawardmgr;
pub(crate) mod ztgamemgr;
pub(crate) mod ztgamemgr_menumusichandler;
pub(crate) mod ztguest;
pub(crate) mod zthabitat_lifecycle;
pub(crate) mod zthabitatmgr;
pub(crate) mod ztmarketing;
pub(crate) mod ztmegatilemgr;
pub(crate) mod zoostatus;
pub(crate) mod ztkeeper;
pub(crate) mod ztanimal;
pub(crate) mod ztmaint;
pub(crate) mod ztguest_leaf;
pub(crate) mod ztguest_lifecycle;
pub(crate) mod ztguest_status;
pub(crate) mod ztguest_checks;
pub(crate) mod ztguest_inventory;
pub(crate) mod ztguest_movement;
pub(crate) mod ztguest_viewing;
pub(crate) mod ztguide;
pub(crate) mod ztguide_lifecycle;
pub(crate) mod ztguide_habitats;
pub(crate) mod ztguide_trigger;
pub(crate) mod ztguide_goals;
pub(crate) mod ztresearch;
pub(crate) mod ztscenariosimplegoal;
pub(crate) mod ztshow;
pub(crate) mod ztshowmgr;
pub(crate) mod ztshowscriptmgr;
pub(crate) mod ztshowscriptstate;
pub(crate) mod ztshowinfo;
pub(crate) mod ztshowstate;
pub(crate) mod ztshowui;
pub(crate) mod ztsoundscape;
pub(crate) mod ztviewingarea;
pub(crate) mod zttankexhibit;
pub(crate) mod ztthoughtmgr;
