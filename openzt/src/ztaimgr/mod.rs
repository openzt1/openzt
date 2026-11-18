//! `ZTAIMgr`/`BFAIMgr` reimplementation: the AI manager and the `executeCall` handlers it dispatches to.
//!
//! `ZTAIMgr` extends `BFAIMgr` (`BFAIMgr` -> `ZTAIMgr`, vtable `0x00635324`). The AI manager is a
//! singleton (`GLOBAL_ZTAIMgr`) that un-ported vanilla code reaches through its vtable and reads by raw
//! offset, so nothing here replaces the object: each detour reads and writes vanilla's own memory in place
//! (style 1), and no struct is declared because only fixed dwords are touched (the offsets below).
//!
//! Reimplemented:
//! - Manager methods: `getRandomX`/`getRandomY`, `isSlow`/`isMedium`/`isFastMoveFunction`,
//!   `convertConstantParam` (partial override), `removeEntity`, `removeEntities`, `callDone`, the tile
//!   `getPath` overload, `checkPath`, `initAIParams` (`ZTAIMgr`), `update` (both classes), `save`, `load`,
//!   `loadData`, `create`, the `BFAIMgr` constructor and destructors, and the listener overrides
//!   `sendEvent`/`getEvents`.
//! - Both `executeCall` dispatch chains and every handler they reach, wrappers and behavior halves alike
//!   (`fMove`, `fPlaySpecial`, `fPlaySet`, `fFly`, `fRandomWalk`, ...).
//!
//! Still vanilla: `BFAIMgr::fPlay` (`0x0042296d`) and its mixer helpers, the position-based `getPath`, the
//! entity `fRoutine` bodies, `ZTAIMgr::init` (a bare `jmp [vtable+0x38]` thunk), and `BFAIMgr::init` with the
//! `BFAIMgr::initAIParams` it dispatches to. The constructor calls those two once per manager, while the
//! `BFAIMgr` vtable is still installed. Handlers defer to vanilla for inputs on
//! which vanilla faults (typically a null or wrong-class `this`).
//!
//! Windows layout differs from the macOS build (`ZTAIMgr` fields sit `+4` later, entity-type fields `+0x18`
//! later), so every offset here comes from the Windows decompile, never the macOS one.
//!
//! ## Module layout
//! `leaf` (raw call views, leaf manager methods, `call_virtual_args`), `handlers` (building, predator/prey, routine and
//! face handlers), `dispatch` (name lookup and the two `executeCall` chains), `manager` (lifecycle, `update`, `save`,
//! `load`, `checkPath`, `getPath`), `misc_handlers` (`fPlaySetTank`, `fDustBall`, `fTargetBuilding`, speed wrappers),
//! `movement` (`fMove`), `links` (animation-link vectors), `play` and `play_variants` (the play family), `walk`
//! (`fRandomWalk`), `wrappers` (the thin `...F` forwarders), `ai_params`, and `detours` (the detour glue and the
//! live battery's real-body wrappers). Private items are `pub(super)` so sibling modules reach them through the
//! glob re-exports below.

use std::{
    ffi::{c_void, CStr},
    mem,
};

use openzt_detour::generated::{
    bfaimgr::{F_FLY, F_MOVE_REVERSE_F, F_PLAY as BFAIMGR_F_PLAY, F_PLAY_TIME},
    bfcoggoal::REACHED_TARGET as BFCOGGOAL_REACHED_TARGET,
    gxmixerlink::DURATION as LINK_DURATION,
    ztcheat::ENTITY_REMOVED as ZTCHEAT_ENTITY_REMOVED,
    ztguide::F_ROUTINE as ZTGUIDE_F_ROUTINE,
    ztkeeper::F_ROUTINE as ZTKEEPER_F_ROUTINE,
    standalone::{ATOI, TIME},
    ztaimgr::{
        CONVERT_CONSTANT_PARAM, F_END_TRICK, F_END_TRICK_F, F_ENTER_BUILDING, F_ENTER_BUILDING_F, F_GUEST_ROUTINE, F_GUEST_ROUTINE_F, F_GUIDE_ROUTINE, F_GUIDE_ROUTINE_F,
        F_HIDE_PREDATOR, F_HIDE_PREDATOR_F, F_KEEPER_ROUTINE, F_KEEPER_ROUTINE_F, F_MAINT_ROUTINE, F_MAINT_ROUTINE_F, F_RETURN_TO_BUILDING, F_RETURN_TO_BUILDING_F, F_SPRAY_F,
        GET_RANDOM_X, GET_RANDOM_Y, IS_FAST_MOVE_FUNCTION, IS_MEDIUM_MOVE_FUNCTION, IS_SLOW_MOVE_FUNCTION, REMOVE_ENTITY,
    },
    bfentity::GET_TILE as BFENTITY_GET_TILE,
    ztaimgr::{
        F_CAUGHT_PREY, F_CAUGHT_PREY_F, F_DIE, F_DIE_F, F_EXIT_BUILDING, F_EXIT_BUILDING_F, F_HATCH, F_HATCH_F, F_KILL_PREY, F_KILL_PREY_F, F_SHOW_PREDATOR, F_SHOW_PREDATOR_F,
    },
    bfaimgr::{
        CALL_DONE as BFAIMGR_CALL_DONE, CHECK_PATH as BFAIMGR_CHECK_PATH, GET_EVENTS as BFAIMGR_GET_EVENTS, CONSTRUCTOR as BFAIMGR_CONSTRUCTOR, DESTRUCTOR_0 as BFAIMGR_DESTRUCTOR_0, DESTRUCTOR_1 as BFAIMGR_DESTRUCTOR_1, INIT as BFAIMGR_INIT, INIT_AIPARAMS as BFAIMGR_INIT_AIPARAMS, F_PLAY_SET, GET_PATH_1 as BFAIMGR_GET_PATH_1,
        LOAD as BFAIMGR_LOAD, LOAD_DATA as BFAIMGR_LOAD_DATA, REMOVE_ENTITIES as BFAIMGR_REMOVE_ENTITIES, SAVE as BFAIMGR_SAVE, SEND_EVENT as BFAIMGR_SEND_EVENT, UPDATE as BFAIMGR_UPDATE,
    },
    bfinifile::READ as BFINIFILE_READ,
    gxmixer::{CLEAR_ALL as GXMIXER_CLEAR_ALL, GET_ANIM as GXMIXER_GET_ANIM},
    bfpathfinder::SET_NODE_MAX,
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    bfpathfinder::{CONSTRUCTOR as BFPATHFINDER_CONSTRUCTOR, DESTRUCTOR as BFPATHFINDER_DESTRUCTOR},
    gxlleanim::DESTRUCTOR as GXLLEANIM_DESTRUCTOR,
    gxlleanimset::RELEASE_0 as GXLLEANIMSET_RELEASE,
    msvc_std_tree20::CLEAR as TREE20_CLEAR,
    msvc_std_vector_t_28::VECTOR_T_28,
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE_N_28},
    ztaimgr::{CREATE as ZTAIMGR_CREATE, INIT_AIPARAMS as ZTAIMGR_INIT_AIPARAMS},
    bfmap::MOVE_ENTITY_DELTA as BFMAP_MOVE_ENTITY_DELTA,
    bfunit::GET_PATH_COST as BFUNIT_GET_PATH_COST,
    bfmap::GET_NEIGHBORS as BFMAP_GET_NEIGHBORS,
    msvc_std::RAND,
    poolalloc::DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4,
    bfentity::{FIND_ANIM as BFENTITY_FIND_ANIM, FIND_SOUND as BFENTITY_FIND_SOUND},
    gxmixer::{SET_BASE_0 as GXMIXER_SET_BASE, SET_SET as GXMIXER_SET_SET},
    bfeventmgr::{CONSTRUCTOR as BFEVENTMGR_CONSTRUCTOR, GET_EVENTS as BFEVENTMGR_GET_EVENTS, LOAD as BFEVENTMGR_LOAD, POST_EVENT as BFEVENTMGR_POST_EVENT, SAVE as BFEVENTMGR_SAVE, UPDATE as BFEVENTMGR_UPDATE},
    standalone::{DEALLOCATE as READ_BYTES_FROM_FILE, WRITE_BYTES_TO_FILE},
    ztaimgr::UPDATE as ZTAIMGR_UPDATE,
    ztmainttaskpool::LISTEN as ZTMAINTTASKPOOL_LISTEN,
    bfaimgr::{
        EXECUTE_CALL as BFAIMGR_EXECUTE_CALL, F_MOVE, F_PLAY_SPECIAL, F_FACE_DIRECTION_F, F_FLY_F, F_FLY_REVERSE_F, F_HALT_F, F_PLAY_F, F_PLAY_WITH_SOUND, F_PLAY_SOUND, F_MOVE_ENTITY_DELTA_F, F_MOVE_ENTITY_SPECIAL_F, F_MOVE_F,
        F_MOVE_FAST_F, F_MOVE_MEDIUM_F, F_MOVE_SLOW_F, F_MOVE_WITH_SOUND_F, F_PLAY_PING_PONG_F, F_PLAY_REVERSE_F, F_PLAY_SET_F, F_PLAY_SET_PROB, F_PLAY_SET_PROB_F, F_RANDOM_WALK, F_PLAY_SOUND_F,
        F_PLAY_SPECIAL_F, F_PLAY_STOP_AT_END_F, F_PLAY_TIME_F, F_PLAY_WITH_BASE_F, F_PLAY_WITH_SOUND_F, F_SET_BASE_F,
    },
    ztaimgr::F_PLAY_SET_TERRAIN_F as ZTAIMGR_F_PLAY_SET_TERRAIN_F,
    ztanimal::TARGET_BUILDING as ZTANIMAL_TARGET_BUILDING,
    bfentitytype::{FIND_ANIM as BFENTITYTYPE_FIND_ANIM, FIND_SOUND as BFENTITYTYPE_FIND_SOUND},
    ztaimgr::{EXECUTE_CALL, F_DUST_BALL_F, F_FACE_TOWARD_FOOD, F_FACE_TOWARD_FOOD_F, F_FACE_TOWARD_TARGET, F_FACE_TOWARD_TARGET_F, F_DUST_BALL, F_PLAY_SET_TANK_F, F_TARGET_BUILDING, F_TARGET_BUILDING_F},
    ztanimal::{CONSUME as ZTANIMAL_CONSUME, FACE_TOWARD_FOOD, FINISH_HATCH},
    ztguest::F_ROUTINE as ZTGUEST_F_ROUTINE,
    zthabitat::TRIGGER_DEATH_ARRIVED,
    ztunit::{FACE_TOWARD_TARGET, FACE_TOWARD_TARGET_UNIT, F_ZOO_MESSAGE_TILE_0, GET_PREDATOR_UNIT, GET_TARGET_UNIT as ZTUNIT_GET_TARGET_UNIT, SET_IS_REMOVED as ZTUNIT_SET_IS_REMOVED},
    ztmapview::PREPARE_TO_DELETE_ENTITY,
    ztui_general::GET_MAPVIEW,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, save_to_memory, LiveMemory},
    owned_strings::{self, string_fill_new, string_release, OwnedString, StringSlot},
    vanilla_string::VanillaString,
};

/// `ZTAIMgr` dword read by [`get_random_x`] for a guest.
const OFFSET_GUEST_RANDOM_X: u32 = 0x140;
/// `ZTAIMgr` dword read by [`get_random_y`] for a guest.
const OFFSET_GUEST_RANDOM_Y: u32 = 0x144;
/// `ZTAIMgr` dword read by [`get_random_x`] for staff.
const OFFSET_STAFF_RANDOM_X: u32 = 0x148;
/// `ZTAIMgr` dword read by [`get_random_y`] for staff.
const OFFSET_STAFF_RANDOM_Y: u32 = 0x14c;
/// `ZTAIMgr` dword read by [`get_random_x`] for any other entity (animals).
const OFFSET_DEFAULT_RANDOM_X: u32 = 0xdc;
/// `ZTAIMgr` dword read by [`get_random_y`] for any other entity (animals).
const OFFSET_DEFAULT_RANDOM_Y: u32 = 0xe0;

/// `BFEntity`'s pointer to its `BFEntityType`.
const OFFSET_ENTITY_TYPE: u32 = 0x128;
/// `BFEntityType` vtable slot `isClass(tag)`: exact-class test.
const TYPE_VTABLE_IS_CLASS: u32 = 0x18;
/// `BFEntityType` vtable slot `isCastClass(tag)`: is-or-derives-from test.
const TYPE_VTABLE_IS_CAST_CLASS: u32 = 0x1c;
/// `BFEntityWorldMgr` vtable slot `removeEntity(entity, bool)`.
const WORLD_MGR_VTABLE_REMOVE_ENTITY: u32 = 0x1c;

/// Class-tag constants (`&CAST_ZT*`) the type checks take, as RVAs into the `0x10`-spaced tag table.
const RVA_TAG_ZTGUEST: u32 = 0x0023_8700;
const RVA_TAG_ZTSTAFF: u32 = 0x0023_8710;
const RVA_TAG_ZTUNITTYPE: u32 = 0x0023_86b0;
const RVA_TAG_ZTANIMAL: u32 = 0x0023_8690;
const RVA_TAG_ZTKEEPER: u32 = 0x0023_8760;
const RVA_TAG_ZTGUIDE: u32 = 0x0023_8770;
const RVA_TAG_ZTMAINT: u32 = 0x0023_8780;

/// Entity vtable slots the building/trick handlers dispatch through (thiscall, no arguments).
const ENTITY_VTABLE_END_TRICK: u32 = 0x2bc;
const ENTITY_VTABLE_ENTER_BUILDING: u32 = 0x2a8;
const ENTITY_VTABLE_RETURN_TO_BUILDING: u32 = 0x2b0;
/// Entity vtable slot `fHidePredator` calls on the predator with `0`.
const ENTITY_VTABLE_HIDE_PREDATOR: u32 = 0xcc;
/// Offset of the `done` flag inside the call object the `F` wrappers receive.
const CALL_DONE_FLAG_OFFSET: u32 = 0x18;


/// `ZTUnitType` dwords `convertConstantParam` returns, in `RANDOM_TIME_NAMES` order.
const UNIT_TYPE_RANDOM_TIME_OFFSETS: [u32; 6] = [0x138, 0x13c, 0x140, 0x144, 0x148, 0x14c];
/// `ZTGuideType` dword `convertConstantParam` returns for `cInformGuestTime`.
const GUIDE_TYPE_INFORM_GUEST_TIME_OFFSET: u32 = 0x1f4;
const INFORM_GUEST_TIME_NAME: &[u8] = b"cInformGuestTime";
const RANDOM_TIME_NAMES: [&[u8]; 6] = [
    b"cWalkRandomlyTime1",
    b"cWalkRandomlyTime2",
    b"cWalkRandomlyTime3",
    b"cRunRandomlyTime1",
    b"cRunRandomlyTime2",
    b"cRunRandomlyTime3",
];

mod leaf;
mod handlers;
mod dispatch;
mod manager;
mod misc_handlers;
mod movement;
mod links;
mod play;
mod play_variants;
mod walk;
mod target_list;
mod wrappers;
mod ai_params;
mod detours;

pub use leaf::*;
pub use handlers::*;
pub use dispatch::*;
pub use manager::*;
pub use misc_handlers::*;
pub use movement::*;
pub(crate) use links::*;
pub use play::*;
pub use play_variants::*;
pub use walk::*;
pub(crate) use target_list::*;
pub use wrappers::*;
pub use ai_params::*;
use detours::ztaimgr_detours;


/// Installs the `ZTAIMgr` detours.
pub fn init() {
    if let Err(e) = unsafe { ztaimgr_detours::init_detours() } {
        error!("Failed to initialise ztaimgr detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use std::ffi::c_void;

    use super::*;

    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        ztaimgr_detours::status()
    }

    pub(crate) fn real_get_random_x(this: *const u32, entity: *const u32) -> i32 {
        ztaimgr_detours::test_real::get_random_x(this, entity)
    }

    pub(crate) fn real_get_random_y(this: *const u32, entity: *const u32) -> i32 {
        ztaimgr_detours::test_real::get_random_y(this, entity)
    }

    pub(crate) fn real_is_slow_move_function(this: *const u32, call: *const u32) -> bool {
        ztaimgr_detours::test_real::is_slow_move_function(this, call)
    }

    pub(crate) fn real_is_medium_move_function(this: *const u32, call: *const u32) -> bool {
        ztaimgr_detours::test_real::is_medium_move_function(this, call)
    }

    pub(crate) fn real_is_fast_move_function(this: *const u32, call: *const u32) -> bool {
        ztaimgr_detours::test_real::is_fast_move_function(this, call)
    }

    pub(crate) fn real_convert_constant_param(this: *const u32, name: *const c_void, entity: *const u32) -> i32 {
        ztaimgr_detours::test_real::convert_constant_param(this, name, entity)
    }

    pub(crate) use super::{f_enter_building, f_exit_building, f_hatch, f_return_to_building, f_show_predator};
    pub(crate) use ztaimgr_detours::test_real::{
        bfaimgr_get_events as real_bfaimgr_get_events, bfaimgr_load as real_bfaimgr_load, bfaimgr_send_event as real_bfaimgr_send_event, bfaimgr_call_done as real_bfaimgr_call_done, bfaimgr_check_path as real_bfaimgr_check_path, bfaimgr_get_path_tiles as real_bfaimgr_get_path_tiles,
        bfaimgr_remove_entities as real_bfaimgr_remove_entities, ztaimgr_init_ai_params as real_ztaimgr_init_ai_params, bfaimgr_load_data as real_bfaimgr_load_data, bfaimgr_save as real_bfaimgr_save, bfaimgr_update as real_bfaimgr_update, ztaimgr_create as real_ztaimgr_create, bfaimgr_construct as real_bfaimgr_construct, bfaimgr_destruct as real_bfaimgr_destruct,
        ztaimgr_update as real_ztaimgr_update,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        base_execute_call as real_base_execute_call, execute_call as real_execute_call, f_caught_prey as real_f_caught_prey, f_die as real_f_die, f_face_toward_food as real_f_face_toward_food, f_face_toward_target as real_f_face_toward_target, f_exit_building as real_f_exit_building, f_kill_prey as real_f_kill_prey,
    };
    pub(crate) use ztaimgr_detours::test_real::{bfaimgr_init as real_bfaimgr_init, bfaimgr_init_ai_params as real_bfaimgr_init_ai_params};
    pub(crate) use ztaimgr_detours::test_real::{f_play_set_prob as real_f_play_set_prob, f_random_walk as real_f_random_walk};
    pub(crate) use ztaimgr_detours::test_real::{
        f_move_f as real_f_move_f, f_move_fast_f as real_f_move_fast_f, f_move_medium_f as real_f_move_medium_f, f_move_slow_f as real_f_move_slow_f, f_move_with_sound_f as real_f_move_with_sound_f, f_play_set as real_f_play_set, f_play_special_f as real_f_play_special_f,
        f_set_base_f as real_f_set_base_f, f_target_building as real_f_target_building, f_target_building_f as real_f_target_building_f,
        f_dust_ball as real_f_dust_ball, f_dust_ball_f as real_f_dust_ball_f,
        f_hatch as real_f_hatch, f_hide_predator as real_f_hide_predator, f_show_predator as real_f_show_predator,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        f_fly as real_f_fly, f_move as real_f_move, f_play_f as real_f_play_f, f_play_special as real_f_play_special, f_play_time as real_f_play_time, f_play_with_base_f as real_f_play_with_base_f, f_play_with_sound as real_f_play_with_sound,
    };
    pub(crate) use super::{f_move, FMoveArgs, F_MOVE_VANILLA_OPS};
    pub(crate) use super::target_list::{TargetList, VanillaPool};
    pub(crate) use super::{acquire_first_vanilla, add_anim_vanilla, play_links_with, play_pair_with, play_single_with, play_timed_with, play_two_with, play_with, release_first_vanilla, set_base_with};
    pub(crate) use ztaimgr_detours::test_real::{
        f_end_trick as real_f_end_trick, f_enter_building as real_f_enter_building, f_guest_routine as real_f_guest_routine, f_guide_routine as real_f_guide_routine,
        f_keeper_routine as real_f_keeper_routine, f_maint_routine as real_f_maint_routine, f_return_to_building as real_f_return_to_building,
    };
    pub(crate) use ztaimgr_detours::test_real::{
        f_face_direction_f as real_f_face_direction_f, f_fly_reverse_f as real_f_fly_reverse_f, f_move_entity_special_f as real_f_move_entity_special_f, f_play_ping_pong_f as real_f_play_ping_pong_f, f_play_reverse_f as real_f_play_reverse_f,
        f_play_set_f as real_f_play_set_f, f_play_sound_f as real_f_play_sound_f, f_play_stop_at_end_f as real_f_play_stop_at_end_f, f_play_time_f as real_f_play_time_f,
    };
}

#[cfg(test)]
mod tests;
