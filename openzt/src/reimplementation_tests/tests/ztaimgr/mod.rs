//! Compares real vanilla `ZTAIMgr` leaf methods against their Rust reimplementation (production file
//! `openzt/src/ztaimgr/`).
//!
//! No live zoo is needed. Each test builds Rust-owned fixtures (a fake `ZTAIMgr` dword block, fake
//! entities whose `BFEntityType` carries a fake vtable, fake `BFFunctionCall` strings) and calls both
//! poles over the same memory: the hooked address (the Rust port) and the real vanilla body, which reads
//! the same fixtures and dispatches through the same fake vtables.

use std::sync::atomic::{AtomicU32, Ordering};

use openzt_detour::generated::ztaimgr::{
    CONVERT_CONSTANT_PARAM, EXECUTE_CALL, F_CAUGHT_PREY, F_DIE, F_EXIT_BUILDING, F_FACE_TOWARD_FOOD, F_FACE_TOWARD_TARGET, F_KILL_PREY, F_END_TRICK, F_ENTER_BUILDING, F_GUEST_ROUTINE, F_GUIDE_ROUTINE, F_KEEPER_ROUTINE, F_MAINT_ROUTINE, F_RETURN_TO_BUILDING, GET_RANDOM_X, GET_RANDOM_Y,
    IS_FAST_MOVE_FUNCTION, IS_MEDIUM_MOVE_FUNCTION, IS_SLOW_MOVE_FUNCTION,
};

use crate::{
    globals::get_module_base,
    reimplementation_tests::harness::{finish_test, write_success_line},
    ztaimgr::live_support,
};

use std::sync::Mutex;

use openzt_detour::generated::{
    bfaimgr::{
        CALL_DONE as BFAIMGR_CALL_DONE, CHECK_PATH as BFAIMGR_CHECK_PATH, GET_PATH_1 as BFAIMGR_GET_PATH_1, LOAD_DATA, REMOVE_ENTITIES as BFAIMGR_REMOVE_ENTITIES,
        SAVE as BFAIMGR_SAVE, UPDATE as BFAIMGR_UPDATE,
    },
    bfeventmgr::CONSTRUCTOR as BFEVENTMGR_CONSTRUCTOR,
    poolalloc::{ALLOCATE as POOL_ALLOCATE, DEALLOCATE as POOL_DEALLOCATE},
    ztaimgr::{CREATE, UPDATE as ZTAIMGR_UPDATE},
};

use crate::{
    bfevent::live_support as bfevent_live_support,
    reimplementation_tests::io_redirect,
    util::{get_from_memory, save_to_memory},
    vanilla_rbtree::{rb_set_insert, rb_tree_clear, walk_nodes},
};

mod fixtures;
mod leaf_handlers;
mod dispatch;
mod manager_fixtures;
mod manager;
mod manager_misc;
mod base_wrappers;
mod string_handlers;
mod movement;
mod play;
mod walk;
mod play_variants;
mod real_vs_port;
mod routing;
mod init;

use fixtures::*;
pub(crate) use leaf_handlers::*;
pub(crate) use dispatch::*;
use manager_fixtures::*;
pub(crate) use manager::*;
pub(crate) use manager_misc::*;
pub(crate) use base_wrappers::*;
pub(crate) use string_handlers::*;
pub(crate) use movement::*;
pub(crate) use play::*;
pub(crate) use walk::*;
pub(crate) use play_variants::*;
pub(crate) use real_vs_port::*;
pub(crate) use routing::*;
pub(crate) use init::*;
