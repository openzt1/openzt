// Auto-generated Rust function definitions for Zoo Tycoon
// Generated from Ghidra analysis

#![allow(clippy::type_complexity)]

use std::marker::PhantomData;

use crate::FunctionDef;

#[cfg(feature = "detour-validation")]
use openzt_detour_macro::validate_detour;

// AI_cls_0x404fd6 class functions
pub mod ai_cls_0x404fd6 {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ai_cls_0x404fd6/find"))]
    pub const FIND: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004031b1, function_type: PhantomData};
}

// Ambients class functions
pub mod ambients {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ambients/ambients"))]
    pub const AMBIENTS: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x0041e930, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ambients/play"))]
    pub const PLAY: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x0043f445, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ambients/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004495a3, function_type: PhantomData};
}

// AmbientsGroup class functions
pub mod ambientsgroup {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ambientsgroup/play"))]
    pub const PLAY: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x0043f4d5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ambientsgroup/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0045067e, function_type: PhantomData};
}

// BFAIMgr class functions
pub mod bfaimgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/f_move_slow_f"))]
    pub const F_MOVE_SLOW_F: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x00436606, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/execute_call"))]
    pub const EXECUTE_CALL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00437ccd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/f_random_walk"))]
    pub const F_RANDOM_WALK: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0043d948, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/f_play_set"))]
    pub const F_PLAY_SET: FunctionDef<unsafe extern "stdcall" fn(u32, u32, i8, u32) -> u32> = FunctionDef{address: 0x0043ddc1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047b081, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/f_move_medium_f"))]
    pub const F_MOVE_MEDIUM_F: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x004a31de, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/f_move_fast_f"))]
    pub const F_MOVE_FAST_F: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x004a5310, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/bfaimgr"))]
    pub const BFAIMGR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0050374b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00525e90, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/init_aiparams"))]
    pub const INIT_AIPARAMS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005267ec, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/get_path"))]
    pub const GET_PATH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0052d8e8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/f_play_special"))]
    pub const F_PLAY_SPECIAL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, i32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00588f7e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfaimgr/f_play_set_terrain_f"))]
    pub const F_PLAY_SET_TERRAIN_F: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00601d2d, function_type: PhantomData};
}

// BFAnimCache class functions
pub mod bfanimcache {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfanimcache/find_anim"))]
    pub const FIND_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x00401fdd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfanimcache/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00419e91, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfanimcache/init"))]
    pub const INIT: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0052812e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfanimcache/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x005283f4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfanimcache/instantiate"))]
    pub const INSTANTIATE: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00528447, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfanimcache/unload_anims"))]
    pub const UNLOAD_ANIMS: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0061ca3b, function_type: PhantomData};
}

// BFApp class functions
pub mod bfapp {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/load_string"))]
    pub const LOAD_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00404e0a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/handle_messages"))]
    pub const HANDLE_MESSAGES: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, i32) -> i32> = FunctionDef{address: 0x00418f4f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/win_main"))]
    pub const WIN_MAIN: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, i32) -> i32> = FunctionDef{address: 0x0041a8bc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/build_string"))]
    pub const BUILD_STRING: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0041ca7f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/get_installed_expansion"))]
    pub const GET_INSTALLED_EXPANSION: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004ab32c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/load_user_resource"))]
    pub const LOAD_USER_RESOURCE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004f029d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/find_instance"))]
    pub const FIND_INSTANCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005337e6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/init_instance"))]
    pub const INIT_INSTANCE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32) -> bool> = FunctionDef{address: 0x005340ba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/load_lang_dlls"))]
    pub const LOAD_LANG_DLLS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00537333, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/init_window_class"))]
    pub const INIT_WINDOW_CLASS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005376bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/register_class"))]
    pub const REGISTER_CLASS: FunctionDef<unsafe extern "thiscall" fn(u32) -> bool> = FunctionDef{address: 0x005378f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/store_typedata"))]
    pub const STORE_TYPEDATA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0053797b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/create_window_class"))]
    pub const CREATE_WINDOW_CLASS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x00537b37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/load_res_dlls"))]
    pub const LOAD_RES_DLLS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00537ee3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005784f8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/exit_override"))]
    pub const EXIT_OVERRIDE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0057de3e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/bfapp_0"))]
    pub const BFAPP_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00587641, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/destroy_window_class"))]
    pub const DESTROY_WINDOW_CLASS: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x00587661, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/get_screen_center"))]
    pub const GET_SCREEN_CENTER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005fd367, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/bfapp_1"))]
    pub const BFAPP_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x005fd382, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/load_typedata"))]
    pub const LOAD_TYPEDATA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005fd3a0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfapp/toggle_cursors"))]
    pub const TOGGLE_CURSORS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005fd3d1, function_type: PhantomData};
}

// BFBSCall class functions
pub mod bfbscall {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfbscall/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, u32) -> u32> = FunctionDef{address: 0x004109fe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfbscall/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004781e9, function_type: PhantomData};
}

// BFBehaviorSet class functions
pub mod bfbehaviorset {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfbehaviorset/execute_0"))]
    pub const EXECUTE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00435e57, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfbehaviorset/execute_child"))]
    pub const EXECUTE_CHILD: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0043745f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfbehaviorset/init_child"))]
    pub const INIT_CHILD: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x0043de4f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfbehaviorset/execute_1"))]
    pub const EXECUTE_1: FunctionDef<unsafe extern "stdcall" fn(i32, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u32)> = FunctionDef{address: 0x0043e7fc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfbehaviorset/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32) -> u8> = FunctionDef{address: 0x0044fb77, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfbehaviorset/set_entity"))]
    pub const SET_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x004551cb, function_type: PhantomData};
}

// BFCPUSPEED class functions
pub mod bfcpuspeed {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfcpuspeed/get_cpuspeed"))]
    pub const GET_CPUSPEED: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00533be4, function_type: PhantomData};
}

// BFCategory class functions
pub mod bfcategory {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfcategory/get_value"))]
    pub const GET_VALUE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00410ec0, function_type: PhantomData};
}

// BFCogGoal class functions
pub mod bfcoggoal {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfcoggoal/set_tile"))]
    pub const SET_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0041300b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfcoggoal/set_idsub"))]
    pub const SET_IDSUB: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0041308d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfcoggoal/set_tile_sub"))]
    pub const SET_TILE_SUB: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004131c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfcoggoal/clear_target_sub"))]
    pub const CLEAR_TARGET_SUB: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004131f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfcoggoal/set_target_pos"))]
    pub const SET_TARGET_POS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00413c2b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfcoggoal/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00454aaf, function_type: PhantomData};
}

// BFConfigFile class functions
pub mod bfconfigfile {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_string_0"))]
    pub const GET_STRING_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00404c52, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_string_list_ptr"))]
    pub const GET_STRING_LIST_PTR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u8> = FunctionDef{address: 0x00405223, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_string_list"))]
    pub const GET_STRING_LIST: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00405463, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_string_1"))]
    pub const GET_STRING_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> bool> = FunctionDef{address: 0x004098c1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/attempt_0"))]
    pub const ATTEMPT_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00409ac0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_int"))]
    pub const GET_INT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00409c14, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_keys"))]
    pub const GET_KEYS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00409cf3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/release"))]
    pub const RELEASE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040a5bc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/parse"))]
    pub const PARSE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> bool> = FunctionDef{address: 0x0040ade7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/attempt_1"))]
    pub const ATTEMPT_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u32> = FunctionDef{address: 0x0040aeda, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/add_key_val"))]
    pub const ADD_KEY_VAL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32, u32)> = FunctionDef{address: 0x0040af4f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/add_block"))]
    pub const ADD_BLOCK: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32) -> u32> = FunctionDef{address: 0x0040b540, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_int_list"))]
    pub const GET_INT_LIST: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004aabb8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/acquire"))]
    pub const ACQUIRE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004aaf2d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_float"))]
    pub const GET_FLOAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004b37c3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/get_blocks"))]
    pub const GET_BLOCKS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004b435e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/constructor_0"))]
    pub const CONSTRUCTOR_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004b4516, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/constructor_1"))]
    pub const CONSTRUCTOR_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004d56f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigfile/constructor_2"))]
    pub const CONSTRUCTOR_2: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x005fed91, function_type: PhantomData};
}

// BFConfigStringTable class functions
pub mod bfconfigstringtable {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigstringtable/del_string"))]
    pub const DEL_STRING: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0040a6ad, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfconfigstringtable/add_string"))]
    pub const ADD_STRING: FunctionDef<unsafe extern "cdecl" fn(u32, i32) -> u32> = FunctionDef{address: 0x0040ae55, function_type: PhantomData};
}

// BFDiagnostic class functions
pub mod bfdiagnostic {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfdiagnostic/get_cpuspeed"))]
    pub const GET_CPUSPEED: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00533a62, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfdiagnostic/get_memory"))]
    pub const GET_MEMORY: FunctionDef<unsafe extern "stdcall" fn() -> i32> = FunctionDef{address: 0x005368c0, function_type: PhantomData};
}

// BFEntity class functions
pub mod bfentity {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/init_ambient_anims"))]
    pub const INIT_AMBIENT_ANIMS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00401115, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_tile"))]
    pub const GET_TILE: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0040f8ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_footprint"))]
    pub const GET_FOOTPRINT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool) -> u32> = FunctionDef{address: 0x0040f916, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/is_walkable"))]
    pub const IS_WALKABLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040fbbd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/dir_to_set"))]
    pub const DIR_TO_SET: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, i32, u32) -> u32> = FunctionDef{address: 0x0040ff43, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/validate_position"))]
    pub const VALIDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x0040ffc2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_world_pos"))]
    pub const SET_WORLD_POS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00410141, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_draw_dithered"))]
    pub const SET_DRAW_DITHERED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041028c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/init_after_load"))]
    pub const INIT_AFTER_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00410cb0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/find_anim"))]
    pub const FIND_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u32)> = FunctionDef{address: 0x00410ffe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00412693, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0041281b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/init_status_vars"))]
    pub const INIT_STATUS_VARS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004128e0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_grid_pos"))]
    pub const GET_GRID_POS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0041426f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/is_walkable_by"))]
    pub const IS_WALKABLE_BY: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004147fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/check_avoid_edges"))]
    pub const CHECK_AVOID_EDGES: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004149e6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/bfentity_0"))]
    pub const BFENTITY_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0041df2e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_is_removed"))]
    pub const SET_IS_REMOVED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041e0d9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_visible"))]
    pub const SET_VISIBLE: FunctionDef<unsafe extern "thiscall" fn(u32, u8)> = FunctionDef{address: 0x0041e0f0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/is_removed_undo"))]
    pub const IS_REMOVED_UNDO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041e1b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/create_name"))]
    pub const CREATE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041e84f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_blocking_rect"))]
    pub const GET_BLOCKING_RECT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0042721a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_placement_footprint"))]
    pub const GET_PLACEMENT_FOOTPRINT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004272d4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/clear_bs"))]
    pub const CLEAR_BS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004274de, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/clear_queue"))]
    pub const CLEAR_QUEUE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004275bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_inside_pos"))]
    pub const GET_INSIDE_POS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042b875, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/replace_colors"))]
    pub const REPLACE_COLORS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00432fc7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/draw_0"))]
    pub const DRAW_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x00432ff4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_shadow_tile"))]
    pub const GET_SHADOW_TILE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00433d31, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_color_rep_info"))]
    pub const GET_COLOR_REP_INFO: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00433fbd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/update_animation"))]
    pub const UPDATE_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00434899, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/is_removed"))]
    pub const IS_REMOVED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004348b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0043494e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_stop_at_end"))]
    pub const SET_STOP_AT_END: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00434aca, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_bs"))]
    pub const SET_BS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, u8, i8, i32) -> u32> = FunctionDef{address: 0x0043c6c5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/draw_shadow"))]
    pub const DRAW_SHADOW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043f81c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_underwater_section_anim"))]
    pub const GET_UNDERWATER_SECTION_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0043fd53, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_direction"))]
    pub const SET_DIRECTION: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004403ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_icon_zoom"))]
    pub const GET_ICON_ZOOM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004440cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/verify_shape"))]
    pub const VERIFY_SHAPE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00446995, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_name"))]
    pub const SET_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00449215, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0044f866, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00477998, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/force_anim"))]
    pub const FORCE_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00485580, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/morph"))]
    pub const MORPH: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x004865c4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/calc_shadow_world_position"))]
    pub const CALC_SHADOW_WORLD_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00492def, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/draw_underwater_section"))]
    pub const DRAW_UNDERWATER_SECTION: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i32, u32)> = FunctionDef{address: 0x00496b99, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_height"))]
    pub const GET_HEIGHT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00498d30, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_bsnew"))]
    pub const SET_BSNEW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool, u32) -> u32> = FunctionDef{address: 0x004a7e94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/is_on_tile"))]
    pub const IS_ON_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x004e16f1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/draw_selection_graphic"))]
    pub const DRAW_SELECTION_GRAPHIC: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, u32)> = FunctionDef{address: 0x004ed85d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/set_selected"))]
    pub const SET_SELECTED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ee29a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/send_event_0"))]
    pub const SEND_EVENT_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f2bd5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/add_to_map"))]
    pub const ADD_TO_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f421f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_blocking_rect_virt_ztpath"))]
    pub const GET_BLOCKING_RECT_VIRT_ZTPATH: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004fbbee, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/destroy_selection_graphics"))]
    pub const DESTROY_SELECTION_GRAPHICS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005028c5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/snap_to_grid"))]
    pub const SNAP_TO_GRID: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00505763, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/send_event_1"))]
    pub const SEND_EVENT_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, u32)> = FunctionDef{address: 0x0059df63, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_depth"))]
    pub const GET_DEPTH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005af392, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_idle_anim"))]
    pub const GET_IDLE_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ff91c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_icon_anim"))]
    pub const GET_ICON_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ff92b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/is_snap_to_ground"))]
    pub const IS_SNAP_TO_GROUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ff936, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/bfentity_1"))]
    pub const BFENTITY_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x005ff93d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/draw_1"))]
    pub const DRAW_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ff95b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/draw_aiinfo"))]
    pub const DRAW_AIINFO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ff9e8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ff9eb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentity/get_events"))]
    pub const GET_EVENTS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ffa19, function_type: PhantomData};
}

// BFEntityType class functions
pub mod bfentitytype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/find_anim"))]
    pub const FIND_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u8) -> u32> = FunctionDef{address: 0x00402062, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/is_user_type_id"))]
    pub const IS_USER_TYPE_ID: FunctionDef<unsafe extern "cdecl" fn(i32) -> u32> = FunctionDef{address: 0x00404cb4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/is_user_type"))]
    pub const IS_USER_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004055e2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_base_anim"))]
    pub const GET_BASE_ANIM: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x00412ac4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/find_sound"))]
    pub const FIND_SOUND: FunctionDef<unsafe extern "fastcall" fn(u32, u8, u8) -> u32> = FunctionDef{address: 0x0044084b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/load_data"))]
    pub const LOAD_DATA: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004b456c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/load_sounds"))]
    pub const LOAD_SOUNDS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004b473f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_int"))]
    pub const GET_INT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, bool) -> u32> = FunctionDef{address: 0x004b4803, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_string"))]
    pub const GET_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, bool) -> u32> = FunctionDef{address: 0x004b486c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004b4903, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004b4c2a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/load_color_rep_info"))]
    pub const LOAD_COLOR_REP_INFO: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004bb159, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004be045, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/set_type"))]
    pub const SET_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004be24e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/check_type"))]
    pub const CHECK_TYPE: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004be3ec, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/num_icons"))]
    pub const NUM_ICONS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004e9e68, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_icon_name"))]
    pub const GET_ICON_NAME: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x004e9e74, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_unit_count"))]
    pub const GET_UNIT_COUNT: FunctionDef<unsafe extern "cdecl" fn(u32, u32) -> i32> = FunctionDef{address: 0x004f6cc5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/bfentity_type"))]
    pub const BFENTITY_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00500783, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/unload_data"))]
    pub const UNLOAD_DATA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005008bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/unload_animations"))]
    pub const UNLOAD_ANIMATIONS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005008e7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/unload_sounds"))]
    pub const UNLOAD_SOUNDS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005008fe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/load_user_data"))]
    pub const LOAD_USER_DATA: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00597337, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/set_available"))]
    pub const SET_AVAILABLE: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a111d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005fe1c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_user_data"))]
    pub const GET_USER_DATA: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u32> = FunctionDef{address: 0x005fe1ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_base_user_id"))]
    pub const GET_BASE_USER_ID: FunctionDef<unsafe extern "cdecl" fn(i32) -> i32> = FunctionDef{address: 0x005fe25c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfentitytype/get_user_data_index"))]
    pub const GET_USER_DATA_INDEX: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x005fe27a, function_type: PhantomData};
}

// BFEvent class functions
pub mod bfevent {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfevent/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x00485f08, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfevent/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0060182e, function_type: PhantomData};
}

// BFEventInfo class functions
pub mod bfeventinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfeventinfo/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x00485e31, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfeventinfo/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00601768, function_type: PhantomData};
}

// BFEventMgr class functions
pub mod bfeventmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfeventmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047afa5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfeventmgr/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00525f3e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfeventmgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0052603f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfeventmgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00594d17, function_type: PhantomData};
}

// BFFont class functions
pub mod bffont {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bffont/create"))]
    pub const CREATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u32> = FunctionDef{address: 0x00510102, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bffont/bffont"))]
    pub const BFFONT: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00620bfa, function_type: PhantomData};
}

// BFFontCache class functions
pub mod bffontcache {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bffontcache/remove_font"))]
    pub const REMOVE_FONT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005032a6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bffontcache/get_font"))]
    pub const GET_FONT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u32> = FunctionDef{address: 0x00510001, function_type: PhantomData};
}

// BFFontDescription class functions
pub mod bffontdescription {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bffontdescription/get_height_0"))]
    pub const GET_HEIGHT_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00418d4a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bffontdescription/release"))]
    pub const RELEASE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0050241b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bffontdescription/get_avg_char_width_0"))]
    pub const GET_AVG_CHAR_WIDTH_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00510668, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bffontdescription/get_height_1"))]
    pub const GET_HEIGHT_1: FunctionDef<unsafe extern "cdecl" fn(i32) -> u32> = FunctionDef{address: 0x00511c74, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bffontdescription/get_avg_char_width_1"))]
    pub const GET_AVG_CHAR_WIDTH_1: FunctionDef<unsafe extern "stdcall" fn(u8, u8, u8, u8, u8, u32) -> u32> = FunctionDef{address: 0x00511cc2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bffontdescription/create"))]
    pub const CREATE: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x00511d39, function_type: PhantomData};
}

// BFGameApp class functions
pub mod bfgameapp {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/get_graphics"))]
    pub const GET_GRAPHICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00401380, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/handle_messages"))]
    pub const HANDLE_MESSAGES: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, i32) -> i32> = FunctionDef{address: 0x00418f97, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/do_each_loop"))]
    pub const DO_EACH_LOOP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041a5ff, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/restart_graphics_mgr"))]
    pub const RESTART_GRAPHICS_MGR: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x004ca9a4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/toggle_fullscreen"))]
    pub const TOGGLE_FULLSCREEN: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004d603b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/resize_app"))]
    pub const RESIZE_APP: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u32> = FunctionDef{address: 0x004d6ce5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/init_instance_0"))]
    pub const INIT_INSTANCE_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32, i8) -> u32> = FunctionDef{address: 0x00533552, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005786fb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/get_sound"))]
    pub const GET_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0057d5e1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/exit_override"))]
    pub const EXIT_OVERRIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0057de27, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/bfgame_app"))]
    pub const BFGAME_APP: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00604487, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/init_instance_1"))]
    pub const INIT_INSTANCE_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00620686, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/get_update_time"))]
    pub const GET_UPDATE_TIME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0062069f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/init_instance_2"))]
    pub const INIT_INSTANCE_2: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006206a3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/load_typedata"))]
    pub const LOAD_TYPEDATA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006206d4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/get_update_handler"))]
    pub const GET_UPDATE_HANDLER: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x00620708, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/inc_sim_speed"))]
    pub const INC_SIM_SPEED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00620745, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/dec_sim_speed"))]
    pub const DEC_SIM_SPEED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00620762, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgameapp/reset_sim_speed"))]
    pub const RESET_SIM_SPEED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00620792, function_type: PhantomData};
}

// BFGameMgr class functions
pub mod bfgamemgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfgamemgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047aca5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgamemgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x00594cba, function_type: PhantomData};
}

// BFGoal class functions
pub mod bfgoal {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoal/set_goal_tile"))]
    pub const SET_GOAL_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0042224d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoal/get_type"))]
    pub const GET_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004781e5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoal/bfgoal"))]
    pub const BFGOAL: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004a4da2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoal/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x004a4e37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoal/set_goal_target_pos"))]
    pub const SET_GOAL_TARGET_POS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a722c, function_type: PhantomData};
}

// BFGoalFactory class functions
pub mod bfgoalfactory {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoalfactory/create_0"))]
    pub const CREATE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00413acd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoalfactory/create_1"))]
    pub const CREATE_1: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x00413ba8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoalfactory/constructor_0"))]
    pub const CONSTRUCTOR_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00502904, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoalfactory/bfgoal_factory"))]
    pub const BFGOAL_FACTORY: FunctionDef<unsafe extern "thiscall" fn(u32, u8)> = FunctionDef{address: 0x0050290b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfgoalfactory/constructor_1"))]
    pub const CONSTRUCTOR_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00502915, function_type: PhantomData};
}

// BFIniFile class functions
pub mod bfinifile {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfinifile/read"))]
    pub const READ: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0041b55d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinifile/write"))]
    pub const WRITE: FunctionDef<unsafe extern "cdecl" fn(u32, u32)> = FunctionDef{address: 0x004b298d, function_type: PhantomData};
}

// BFLog class functions
pub mod bflog {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/log_message"))]
    pub const LOG_MESSAGE: FunctionDef<unsafe extern "cdecl" fn(u32, i32, i32, i8, i32, i32, u32)> = FunctionDef{address: 0x00401363, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/constructor_0"))]
    pub const CONSTRUCTOR_0: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i32, i32, i32, i32, i8, u32)> = FunctionDef{address: 0x00401386, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/constructor_1"))]
    pub const CONSTRUCTOR_1: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x0040514d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/format_log_message"))]
    pub const FORMAT_LOG_MESSAGE: FunctionDef<unsafe extern "cdecl" fn(u32, u32) -> u32> = FunctionDef{address: 0x0040667d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/process_log_options"))]
    pub const PROCESS_LOG_OPTIONS: FunctionDef<unsafe extern "cdecl" fn(i8, i32)> = FunctionDef{address: 0x0052b94a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/write_log_message"))]
    pub const WRITE_LOG_MESSAGE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0052b9a2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/set_log_level"))]
    pub const SET_LOG_LEVEL: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0052ba7b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bflog/write_log_to_file"))]
    pub const WRITE_LOG_TO_FILE: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x006033ba, function_type: PhantomData};
}

// BFMap class functions
pub mod bfmap {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/world_to_virtual_0"))]
    pub const WORLD_TO_VIRTUAL_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0040f041, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/tile_to_world"))]
    pub const TILE_TO_WORLD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0040f26c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_neighbor_0"))]
    pub const GET_NEIGHBOR_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0040fa92, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_height_above_terrain"))]
    pub const GET_HEIGHT_ABOVE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x00410183, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_direction_0"))]
    pub const GET_DIRECTION_0: FunctionDef<unsafe extern "stdcall" fn(i32, i32) -> i32> = FunctionDef{address: 0x00411654, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_direction_1"))]
    pub const GET_DIRECTION_1: FunctionDef<unsafe extern "stdcall" fn(u32) -> i32> = FunctionDef{address: 0x004116a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/distance_cartesian"))]
    pub const DISTANCE_CARTESIAN: FunctionDef<unsafe extern "fastcall" fn(u32, u32, u32, u32) -> i32> = FunctionDef{address: 0x0041180a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/validate_not_blocked"))]
    pub const VALIDATE_NOT_BLOCKED: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00413f43, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/validate"))]
    pub const VALIDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i8) -> u32> = FunctionDef{address: 0x004140c6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_neighbors"))]
    pub const GET_NEIGHBORS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u32> = FunctionDef{address: 0x004171a1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_nearest_direction"))]
    pub const GET_NEAREST_DIRECTION: FunctionDef<unsafe extern "fastcall" fn(u32, u32, i32, i32) -> u64> = FunctionDef{address: 0x00426c39, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_proper_direction"))]
    pub const GET_PROPER_DIRECTION: FunctionDef<unsafe extern "stdcall" fn(u32, i32, i32) -> u32> = FunctionDef{address: 0x0042b79a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_neighbor_1"))]
    pub const GET_NEIGHBOR_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00432236, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x00432dcd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_turning_direction"))]
    pub const GET_TURNING_DIRECTION: FunctionDef<unsafe extern "stdcall" fn(i32, i32, u32) -> i32> = FunctionDef{address: 0x0043a28c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/world_to_virtual_1"))]
    pub const WORLD_TO_VIRTUAL_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004402ed, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/virtual_to_world"))]
    pub const VIRTUAL_TO_WORLD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004441ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/set_selection"))]
    pub const SET_SELECTION: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, u32)> = FunctionDef{address: 0x004446f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_selection"))]
    pub const GET_SELECTION: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x004447e5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/add_edge"))]
    pub const ADD_EDGE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x0044d4f8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/save_tile_terrain"))]
    pub const SAVE_TILE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00459245, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/adjust_cell"))]
    pub const ADJUST_CELL: FunctionDef<unsafe extern "thiscall" fn(u32, i32, bool) -> u32> = FunctionDef{address: 0x0045d935, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/adjust_cell_height"))]
    pub const ADJUST_CELL_HEIGHT: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32, u32)> = FunctionDef{address: 0x0045e04e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004625af, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/restore_terrain"))]
    pub const RESTORE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x00482978, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/copy_full_to_single_tile_terrain"))]
    pub const COPY_FULL_TO_SINGLE_TILE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00488fc7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_changed_elevations"))]
    pub const GET_CHANGED_ELEVATIONS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004891d3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/set_height_above_terrain"))]
    pub const SET_HEIGHT_ABOVE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32)> = FunctionDef{address: 0x00492a9f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/draw_sorter_entity"))]
    pub const DRAW_SORTER_ENTITY: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x004a7272, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/move_entity"))]
    pub const MOVE_ENTITY: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x004a746a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/compute_tiles"))]
    pub const COMPUTE_TILES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004af165, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/set_zoom"))]
    pub const SET_ZOOM: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004af19c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6b83, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/set_map_dimensions"))]
    pub const SET_MAP_DIMENSIONS: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i8)> = FunctionDef{address: 0x004c84d3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004c8782, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/update_terrain_cost"))]
    pub const UPDATE_TERRAIN_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> f32> = FunctionDef{address: 0x004d9c0f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_terrain_paint_cost"))]
    pub const GET_TERRAIN_PAINT_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> f32> = FunctionDef{address: 0x004dc8a4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/add_entity"))]
    pub const ADD_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004f3e7d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/can_paint"))]
    pub const CAN_PAINT: FunctionDef<unsafe extern "stdcall" fn(u32, i32) -> u32> = FunctionDef{address: 0x004f8f0b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/paint_cell"))]
    pub const PAINT_CELL: FunctionDef<unsafe extern "thiscall" fn(u32, i32, bool) -> u32> = FunctionDef{address: 0x004f8fd8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/remove_entity"))]
    pub const REMOVE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fe639, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/remove_edge"))]
    pub const REMOVE_EDGE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004ffdd8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/validate_neighbors"))]
    pub const VALIDATE_NEIGHBORS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0052d76a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/validate_tiles"))]
    pub const VALIDATE_TILES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b302a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/clear_tile_terrain"))]
    pub const CLEAR_TILE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x005b342e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/get_all_neighbors"))]
    pub const GET_ALL_NEIGHBORS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u32> = FunctionDef{address: 0x005b3835, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmap/clear_saved_path"))]
    pub const CLEAR_SAVED_PATH: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00600414, function_type: PhantomData};
}

// BFMgr class functions
pub mod bfmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfmgr/lookup"))]
    pub const LOOKUP: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x004bdd7a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmgr/create_manager"))]
    pub const CREATE_MANAGER: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x004bdd91, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfmgr/registerit"))]
    pub const REGISTERIT: FunctionDef<unsafe extern "cdecl" fn(u32, u32) -> u32> = FunctionDef{address: 0x005770e5, function_type: PhantomData};
}

// BFMoveMod class functions
pub mod bfmovemod {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfmovemod/bfmove_mod"))]
    pub const BFMOVE_MOD: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f9be0, function_type: PhantomData};
}

// BFOldSubGoal class functions
pub mod bfoldsubgoal {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfoldsubgoal/unknown_method"))]
    pub const UNKNOWN_METHOD: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x00413a43, function_type: PhantomData};
}

// BFOverlay class functions
pub mod bfoverlay {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlay/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00420565, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlay/bfoverlay_0"))]
    pub const BFOVERLAY_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00420589, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlay/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0045024d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlay/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047a7ee, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlay/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004fc391, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlay/add_to_map"))]
    pub const ADD_TO_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fc3cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlay/bfoverlay_1"))]
    pub const BFOVERLAY_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00624292, function_type: PhantomData};
}

// BFOverlayType class functions
pub mod bfoverlaytype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlaytype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051cd5e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlaytype/bfoverlay_type"))]
    pub const BFOVERLAY_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057cc7a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfoverlaytype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00624270, function_type: PhantomData};
}

// BFPathFinder class functions
pub mod bfpathfinder {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/clear_lists"))]
    pub const CLEAR_LISTS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041194a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/make_id"))]
    pub const MAKE_ID: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00411a56, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/find_path"))]
    pub const FIND_PATH: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, i8) -> u32> = FunctionDef{address: 0x00414d3f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/validate"))]
    pub const VALIDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00414fef, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/estimate"))]
    pub const ESTIMATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> f32> = FunctionDef{address: 0x00415247, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/cost"))]
    pub const COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32, u32) -> f32> = FunctionDef{address: 0x004153e3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/set_node_max"))]
    pub const SET_NODE_MAX: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004c680d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/bfpath_finder"))]
    pub const BFPATH_FINDER: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00503905, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005265a6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinder/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x0052669c, function_type: PhantomData};
}

// BFPathFinderInfo class functions
pub mod bfpathfinderinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinderinfo/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0052609a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfpathfinderinfo/get"))]
    pub const GET: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00526573, function_type: PhantomData};
}

// BFRegistry class functions
pub mod bfregistry {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfregistry/get"))]
    pub const GET: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004bdca9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfregistry/find"))]
    pub const FIND: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004bdcbf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfregistry/ptr_get"))]
    pub const PTR_GET: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool) -> u32> = FunctionDef{address: 0x004bdd22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfregistry/reg"))]
    pub const REG: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0057766f, function_type: PhantomData};
}

// BFResource class functions
pub mod bfresource {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/release_0"))]
    pub const RELEASE_0: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x00402e14, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/set_handle_0"))]
    pub const SET_HANDLE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00402e1d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x00403891, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/set_handle_1"))]
    pub const SET_HANDLE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004038af, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/prepare"))]
    pub const PREPARE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x004047f4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/set_handle_2"))]
    pub const SET_HANDLE_2: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00404812, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/release_1"))]
    pub const RELEASE_1: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x0040bc8a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/set_handle_3"))]
    pub const SET_HANDLE_3: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0040bc93, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/create_memory"))]
    pub const CREATE_MEMORY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i8)> = FunctionDef{address: 0x004ad366, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/find_0"))]
    pub const FIND_0: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004b41f2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/find_1"))]
    pub const FIND_1: FunctionDef<unsafe extern "cdecl" fn(u32, u32) -> u32> = FunctionDef{address: 0x004bf910, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/shutdown"))]
    pub const SHUTDOWN: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0057e084, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresource/acquire"))]
    pub const ACQUIRE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005fed7c, function_type: PhantomData};
}

// BFResourceDir class functions
pub mod bfresourcedir {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/find_file"))]
    pub const FIND_FILE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x00403a59, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00403b82, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/prepare"))]
    pub const PREPARE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, i8) -> u32> = FunctionDef{address: 0x00404926, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/valid"))]
    pub const VALID: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00404fc8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/find_0"))]
    pub const FIND_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004b9e9f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/find_1"))]
    pub const FIND_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004bfc2f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/bfresource_dir_0"))]
    pub const BFRESOURCE_DIR_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004cbe92, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/bfresource_dir_1"))]
    pub const BFRESOURCE_DIR_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x004cbff4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00529464, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/tree_search"))]
    pub const TREE_SEARCH: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i8, u32) -> u32> = FunctionDef{address: 0x00529796, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/name"))]
    pub const NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006030a0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcedir/index"))]
    pub const INDEX: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x006030a4, function_type: PhantomData};
}

// BFResourceMgr class functions
pub mod bfresourcemgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/get"))]
    pub const GET: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00402e76, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00403817, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/get_resource_ptr"))]
    pub const GET_RESOURCE_PTR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x004038f5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/make_name"))]
    pub const MAKE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00403993, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/prepare"))]
    pub const PREPARE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00404855, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/create_memory"))]
    pub const CREATE_MEMORY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i8) -> u32> = FunctionDef{address: 0x004ad2fc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/find_0"))]
    pub const FIND_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004b9a40, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/find_1"))]
    pub const FIND_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004bf92b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/add_path"))]
    pub const ADD_PATH: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0052870b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0052903f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/shutdown"))]
    pub const SHUTDOWN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0057e0c4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcemgr/bfresource_mgr"))]
    pub const BFRESOURCE_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0057e827, function_type: PhantomData};
}

// BFResourcePtr class functions
pub mod bfresourceptr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_0"))]
    pub const DELREF_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00402e47, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_1"))]
    pub const DELREF_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00402e5e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_2"))]
    pub const DELREF_2: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00402e6a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/deallocate"))]
    pub const DEALLOCATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00402ec7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_3"))]
    pub const DELREF_3: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00402f76, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_4"))]
    pub const DELREF_4: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004038d9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/allocate"))]
    pub const ALLOCATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004043f4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_5"))]
    pub const DELREF_5: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040483c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_6"))]
    pub const DELREF_6: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00406b87, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_7"))]
    pub const DELREF_7: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004072e3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_8"))]
    pub const DELREF_8: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00407330, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_9"))]
    pub const DELREF_9: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00409b55, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_10"))]
    pub const DELREF_10: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040bcbd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_11"))]
    pub const DELREF_11: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040c4bf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_12"))]
    pub const DELREF_12: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041e62e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_13"))]
    pub const DELREF_13: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004aba16, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_14"))]
    pub const DELREF_14: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004abaef, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_15"))]
    pub const DELREF_15: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004abc8f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_16"))]
    pub const DELREF_16: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ac016, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_17"))]
    pub const DELREF_17: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ac060, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_18"))]
    pub const DELREF_18: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ad053, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_19"))]
    pub const DELREF_19: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ad3bd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_20"))]
    pub const DELREF_20: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b31c3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_21"))]
    pub const DELREF_21: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6755, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_22"))]
    pub const DELREF_22: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c67c9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_23"))]
    pub const DELREF_23: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00501628, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_24"))]
    pub const DELREF_24: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00503e66, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_25"))]
    pub const DELREF_25: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00503eb0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_26"))]
    pub const DELREF_26: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00504054, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_27"))]
    pub const DELREF_27: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0050409e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_28"))]
    pub const DELREF_28: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005041ca, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourceptr/delref_29"))]
    pub const DELREF_29: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00504214, function_type: PhantomData};
}

// BFResourceZip class functions
pub mod bfresourcezip {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/find_offset"))]
    pub const FIND_OFFSET: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> i32> = FunctionDef{address: 0x00403a1c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x00403b43, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/prepare"))]
    pub const PREPARE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, i8) -> u32> = FunctionDef{address: 0x004048da, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/find_0"))]
    pub const FIND_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004b9b73, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/find_1"))]
    pub const FIND_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004bf9fb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00528ac1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/valid"))]
    pub const VALID: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00528f94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/bfresource_zip_0"))]
    pub const BFRESOURCE_ZIP_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057e21f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/bfresource_zip_1"))]
    pub const BFRESOURCE_ZIP_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0057e351, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfresourcezip/index"))]
    pub const INDEX: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00602f65, function_type: PhantomData};
}

// BFScenarioMgr class functions
pub mod bfscenariomgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00435a48, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/get_expansion_id"))]
    pub const GET_EXPANSION_ID: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00453530, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/end_scenario"))]
    pub const END_SCENARIO: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0048a726, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/bfscenario_mgr"))]
    pub const BFSCENARIO_MGR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00504e6a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0052428d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/get_crowd_ambients_name"))]
    pub const GET_CROWD_AMBIENTS_NAME: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x00592632, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/get_world_ambients_name"))]
    pub const GET_WORLD_AMBIENTS_NAME: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x00592659, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/get_crowd_config_name"))]
    pub const GET_CROWD_CONFIG_NAME: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x00592680, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/get_world_config_name"))]
    pub const GET_WORLD_CONFIG_NAME: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x005926a7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfscenariomgr/unlock_all_scenarios"))]
    pub const UNLOCK_ALL_SCENARIOS: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x006000d3, function_type: PhantomData};
}

// BFTerrainImage class functions
pub mod bfterrainimage {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfterrainimage/compute_image_size"))]
    pub const COMPUTE_IMAGE_SIZE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004aee5a, function_type: PhantomData};
}

// BFTerrainMgr class functions
pub mod bfterrainmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfterrainmgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0052560a, function_type: PhantomData};
}

// BFTerrainTypeInfo class functions
pub mod bfterraintypeinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfterraintypeinfo/initialize"))]
    pub const INITIALIZE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00523c58, function_type: PhantomData};
}

// BFText class functions
pub mod bftext {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bftext/length_in_characters"))]
    pub const LENGTH_IN_CHARACTERS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00418ccf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftext/convert_to_bytes"))]
    pub const CONVERT_TO_BYTES: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004ec1e3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftext/substr"))]
    pub const SUBSTR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004ec280, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftext/erase"))]
    pub const ERASE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x006242b0, function_type: PhantomData};
}

// BFTile class functions
pub mod bftile {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/get_local_elevation"))]
    pub const GET_LOCAL_ELEVATION: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x0040f24d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/get_corner_elevation"))]
    pub const GET_CORNER_ELEVATION: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> i32> = FunctionDef{address: 0x0040f4f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/is_in_zoo"))]
    pub const IS_IN_ZOO: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x0040fb8d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/validate_positions"))]
    pub const VALIDATE_POSITIONS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool)> = FunctionDef{address: 0x0044a0bf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/get_max_elevation"))]
    pub const GET_MAX_ELEVATION: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0045d8b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/can_change_elevation"))]
    pub const CAN_CHANGE_ELEVATION: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x0045d92d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/can_change_shape"))]
    pub const CAN_CHANGE_SHAPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0045e028, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/raise"))]
    pub const RAISE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, bool)> = FunctionDef{address: 0x0045e416, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/lower"))]
    pub const LOWER: FunctionDef<unsafe extern "thiscall" fn(u32, i32, bool)> = FunctionDef{address: 0x0045e70f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/get_min_elevation"))]
    pub const GET_MIN_ELEVATION: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00492d80, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/snap_to_edge"))]
    pub const SNAP_TO_EDGE: FunctionDef<unsafe extern "cdecl" fn(u32, u32)> = FunctionDef{address: 0x0049336d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/calculate_shape"))]
    pub const CALCULATE_SHAPE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004aea12, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6964, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/remove_all_edges"))]
    pub const REMOVE_ALL_EDGES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6a17, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004c864e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/remove_edge"))]
    pub const REMOVE_EDGE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004f165b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bftile/set_terrain_type"))]
    pub const SET_TERRAIN_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004f17e0, function_type: PhantomData};
}

// BFUIMgr class functions
pub mod bfuimgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/get_element_0"))]
    pub const GET_ELEMENT_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0040157d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/set_control_fore_color"))]
    pub const SET_CONTROL_FORE_COLOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x0040ee08, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/set_cursor"))]
    pub const SET_CURSOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x00418e81, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/hide_busy_cursor"))]
    pub const HIDE_BUSY_CURSOR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00418f2f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/show_busy_cursor"))]
    pub const SHOW_BUSY_CURSOR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00418f43, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8) -> u32> = FunctionDef{address: 0x004193d8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041a16b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/prepare_to_hide_element"))]
    pub const PREPARE_TO_HIDE_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0041ac35, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/remove_timer_element"))]
    pub const REMOVE_TIMER_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0041ad6a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/display_help_0"))]
    pub const DISPLAY_HELP_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x0041b100, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/handle_messages"))]
    pub const HANDLE_MESSAGES: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32) -> i32> = FunctionDef{address: 0x00441d7b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/get_element_1"))]
    pub const GET_ELEMENT_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0044232b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/get_max_tooltip_width"))]
    pub const GET_MAX_TOOLTIP_WIDTH: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x00442d19, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/focus_control"))]
    pub const FOCUS_CONTROL: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0044316b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/capture_element"))]
    pub const CAPTURE_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00443560, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/deselect_element"))]
    pub const DESELECT_ELEMENT: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x00443dd0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/enable_element"))]
    pub const ENABLE_ELEMENT: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x00443e3e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/translate_key"))]
    pub const TRANSLATE_KEY: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0046bbd3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/display_message_0"))]
    pub const DISPLAY_MESSAGE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32, u32, bool, bool)> = FunctionDef{address: 0x0049ccc3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/display_message_1"))]
    pub const DISPLAY_MESSAGE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32, u32, bool, bool)> = FunctionDef{address: 0x0049cec0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/show_last_dialog"))]
    pub const SHOW_LAST_DIALOG: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a09cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/hide_persistent_text"))]
    pub const HIDE_PERSISTENT_TEXT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004b28df, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/destroy_cursors"))]
    pub const DESTROY_CURSORS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b2caa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/clear_messages"))]
    pub const CLEAR_MESSAGES: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004c6d10, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/select_element"))]
    pub const SELECT_ELEMENT: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x004c7794, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/init_cursors"))]
    pub const INIT_CURSORS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d4118, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/resize"))]
    pub const RESIZE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004d41f4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/reload"))]
    pub const RELOAD: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004d594b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/reset_element_cursor"))]
    pub const RESET_ELEMENT_CURSOR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d5bea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/disable_element"))]
    pub const DISABLE_ELEMENT: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x004df425, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/remove_element"))]
    pub const REMOVE_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004e0965, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/delete_element"))]
    pub const DELETE_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004e0e21, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/display_help_1"))]
    pub const DISPLAY_HELP_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004fb613, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/confirm_dialog_0"))]
    pub const CONFIRM_DIALOG_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, i8, i8, i32) -> u32> = FunctionDef{address: 0x004ff63c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/confirm_dialog_1"))]
    pub const CONFIRM_DIALOG_1: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, u32, u32, i8, i8, i32, i32) -> u32> = FunctionDef{address: 0x004fff2c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/delete_all_elements_0"))]
    pub const DELETE_ALL_ELEMENTS_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0050214c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/stop"))]
    pub const STOP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005021bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/delete_all_elements_1"))]
    pub const DELETE_ALL_ELEMENTS_1: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00502796, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/release_sounds"))]
    pub const RELEASE_SOUNDS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0050342a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00511054, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/set_builtin_callback"))]
    pub const SET_BUILTIN_CALLBACK: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00517d55, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/set_help_control"))]
    pub const SET_HELP_CONTROL: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00519cc4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/start"))]
    pub const START: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0051e3b8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00539a66, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/configure_and_show_dialog"))]
    pub const CONFIGURE_AND_SHOW_DIALOG: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, bool) -> u32> = FunctionDef{address: 0x005a1b15, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/bfuimgr_0"))]
    pub const BFUIMGR_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00620515, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfuimgr/bfuimgr_1"))]
    pub const BFUIMGR_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x006207b1, function_type: PhantomData};
}

// BFUnit class functions
pub mod bfunit {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/validate_position"))]
    pub const VALIDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004102fc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_idle_anim"))]
    pub const GET_IDLE_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32, bool) -> i32> = FunctionDef{address: 0x004133d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/is_same_tile"))]
    pub const IS_SAME_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00414530, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_path_cost"))]
    pub const GET_PATH_COST: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32) -> u32> = FunctionDef{address: 0x0041456c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/is_max_cost_for_all"))]
    pub const IS_MAX_COST_FOR_ALL: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004146d4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/idle"))]
    pub const IDLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00421eb7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/go_to_dest_0"))]
    pub const GO_TO_DEST_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i8) -> u32> = FunctionDef{address: 0x00422072, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/go_to_dest_1"))]
    pub const GO_TO_DEST_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00422127, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_path"))]
    pub const GET_PATH: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004221c3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/listen_0"))]
    pub const LISTEN_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00423c03, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_events_0"))]
    pub const GET_EVENTS_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00423cee, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/face_toward_tile"))]
    pub const FACE_TOWARD_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00426d1a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/face_toward_entity"))]
    pub const FACE_TOWARD_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00426d4d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_aiinfo_on"))]
    pub const GET_AIINFO_ON: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043358f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_events_1"))]
    pub const GET_EVENTS_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00434a04, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/update_animation"))]
    pub const UPDATE_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00435bbe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/update_position"))]
    pub const UPDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00435bf8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_events_2"))]
    pub const GET_EVENTS_2: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00435cdb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/listen_1"))]
    pub const LISTEN_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00435cf1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/run_behavior_set"))]
    pub const RUN_BEHAVIOR_SET: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00435cfb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/update_sub_goals"))]
    pub const UPDATE_SUB_GOALS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00436161, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_dest_tile"))]
    pub const GET_DEST_TILE: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00436323, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/set_moving"))]
    pub const SET_MOVING: FunctionDef<unsafe extern "thiscall" fn(u32, i8, i8)> = FunctionDef{address: 0x0043e5a5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/stop"))]
    pub const STOP: FunctionDef<unsafe extern "thiscall" fn(u32, i8, i8)> = FunctionDef{address: 0x0043e642, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_events_3"))]
    pub const GET_EVENTS_3: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043f40d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/cleanup_events"))]
    pub const CLEANUP_EVENTS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043f4a0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/is_non_path_steep"))]
    pub const IS_NON_PATH_STEEP: FunctionDef<unsafe extern "stdcall" fn(i32) -> u32> = FunctionDef{address: 0x0044045e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004547cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00478252, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/set_height_above_terrain"))]
    pub const SET_HEIGHT_ABOVE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00492a51, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/check_elevation"))]
    pub const CHECK_ELEVATION: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32, i32) -> bool> = FunctionDef{address: 0x00493e1c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/set_dest_tile"))]
    pub const SET_DEST_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004a7a90, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/set_selected"))]
    pub const SET_SELECTED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ee4a4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004f568c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/bfunit"))]
    pub const BFUNIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f9c33, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/send_event"))]
    pub const SEND_EVENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u32, u32)> = FunctionDef{address: 0x0059def9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_cost"))]
    pub const GET_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0060238f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_terrain_cost"))]
    pub const GET_TERRAIN_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x006023ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/draw_aiinfo"))]
    pub const DRAW_AIINFO: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32)> = FunctionDef{address: 0x00602481, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/morph"))]
    pub const MORPH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00602929, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunit/get_height_above_terrain"))]
    pub const GET_HEIGHT_ABOVE_TERRAIN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00602c74, function_type: PhantomData};
}

// BFUnitType class functions
pub mod bfunittype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfunittype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004b5de3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunittype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool) -> u32> = FunctionDef{address: 0x004b5faf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunittype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004be68c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunittype/bfunit_type_0"))]
    pub const BFUNIT_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00500b5b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfunittype/bfunit_type_1"))]
    pub const BFUNIT_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00500bda, function_type: PhantomData};
}

// BFVersionInfo class functions
pub mod bfversioninfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfversioninfo/get_version_string"))]
    pub const GET_VERSION_STRING: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004bdfd4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfversioninfo/init_instance"))]
    pub const INIT_INSTANCE: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x005356ad, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfversioninfo/exit_instance_0"))]
    pub const EXIT_INSTANCE_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0057e05d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfversioninfo/exit_instance_1"))]
    pub const EXIT_INSTANCE_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0057e0b3, function_type: PhantomData};
}

// BFWindow class functions
pub mod bfwindow {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d60a8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/show"))]
    pub const SHOW: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> bool> = FunctionDef{address: 0x004d60ff, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/resize"))]
    pub const RESIZE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x004d6141, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/center"))]
    pub const CENTER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d61db, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/change_style"))]
    pub const CHANGE_STYLE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004d62b3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/style_popup"))]
    pub const STYLE_POPUP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d641d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/style_fixed"))]
    pub const STYLE_FIXED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0052f150, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/pointer"))]
    pub const POINTER: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> bool> = FunctionDef{address: 0x005336d5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindow/attach"))]
    pub const ATTACH: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> bool> = FunctionDef{address: 0x00536486, function_type: PhantomData};
}

// BFWindowClass class functions
pub mod bfwindowclass {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindowclass/create_window_0"))]
    pub const CREATE_WINDOW_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x005362a4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindowclass/create_window_1"))]
    pub const CREATE_WINDOW_1: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32, i32, u32) -> u32> = FunctionDef{address: 0x005364ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindowclass/make_icon"))]
    pub const MAKE_ICON: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005379c1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindowclass/constructor_0"))]
    pub const CONSTRUCTOR_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x005379f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindowclass/constructor_1"))]
    pub const CONSTRUCTOR_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00578c6f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfwindowclass/unregister_class"))]
    pub const UNREGISTER_CLASS: FunctionDef<unsafe extern "thiscall" fn(u32) -> bool> = FunctionDef{address: 0x00587672, function_type: PhantomData};
}

// BFWorldMgr class functions
pub mod bfworldmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/verify_entity_0"))]
    pub const VERIFY_ENTITY_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00443ffa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0044f280, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/init_unlocked_entities"))]
    pub const INIT_UNLOCKED_ENTITIES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004533bc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0046285b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00477b4e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6b18, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/unlock_entity"))]
    pub const UNLOCK_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00598cdc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfworldmgr/verify_entity_1"))]
    pub const VERIFY_ENTITY_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005b66c4, function_type: PhantomData};
}

// GXCanvas class functions
pub mod gxcanvas {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/get_dc"))]
    pub const GET_DC: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004014c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/release_dc"))]
    pub const RELEASE_DC: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x004014f5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004050f7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00405792, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/get_pixel"))]
    pub const GET_PIXEL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0040710e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/set_color_key"))]
    pub const SET_COLOR_KEY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004071cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/draw_clipped"))]
    pub const DRAW_CLIPPED: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32, i32) -> u32> = FunctionDef{address: 0x0041981c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxcanvas/string"))]
    pub const STRING: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, i32) -> u32> = FunctionDef{address: 0x004b189d, function_type: PhantomData};
}

// GXDynamicLLE class functions
pub mod gxdynamiclle {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxdynamiclle/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004abfd6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxdynamiclle/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x004aca7f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxdynamiclle/build"))]
    pub const BUILD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, i8)> = FunctionDef{address: 0x004acdda, function_type: PhantomData};
}

// GXGraphicsMgr class functions
pub mod gxgraphicsmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/load_palette_map"))]
    pub const LOAD_PALETTE_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x00403600, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/create_uisurface_0"))]
    pub const CREATE_UISURFACE_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0041b98c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/create_uisurface_1"))]
    pub const CREATE_UISURFACE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0041ba54, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/copy_palette_map"))]
    pub const COPY_PALETTE_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x004abcac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/toggle_fullscreen"))]
    pub const TOGGLE_FULLSCREEN: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x004d5fc6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/start"))]
    pub const START: FunctionDef<unsafe extern "thiscall" fn(u32, i8, i32, i32, i32, i8) -> u32> = FunctionDef{address: 0x004d6c59, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/initialize"))]
    pub const INITIALIZE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x00533e4b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/destroy"))]
    pub const DESTROY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0057de67, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxgraphicsmgr/gxgraphics_mgr"))]
    pub const GXGRAPHICS_MGR: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0057de8e, function_type: PhantomData};
}

// GXImage class functions
pub mod gximage {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gximage/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0061f0c8, function_type: PhantomData};
}

// GXImageBMP class functions
pub mod gximagebmp {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gximagebmp/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x005234da, function_type: PhantomData};
}

// GXImageTGA class functions
pub mod gximagetga {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gximagetga/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004b32a7, function_type: PhantomData};
}

// GXLLE class functions
pub mod gxlle {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxlle/draw_dither"))]
    pub const DRAW_DITHER: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, i32, i8, i32) -> u32> = FunctionDef{address: 0x0047c948, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlle/compress"))]
    pub const COMPRESS: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32, u32, u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004ad3d6, function_type: PhantomData};
}

// GXLLEAnim class functions
pub mod gxlleanim {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanim/release"))]
    pub const RELEASE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00402f13, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanim/prepare"))]
    pub const PREPARE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x0040bbc1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanim/gxlleanim"))]
    pub const GXLLEANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040bc77, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanim/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x00411e21, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanim/acquire"))]
    pub const ACQUIRE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00411eb8, function_type: PhantomData};
}

// GXLLEAnimSet class functions
pub mod gxlleanimset {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanimset/release_0"))]
    pub const RELEASE_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004069d0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanimset/attempt_0"))]
    pub const ATTEMPT_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x00406a22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanimset/release_1"))]
    pub const RELEASE_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00406b0e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanimset/attempt_1"))]
    pub const ATTEMPT_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x0040b967, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanimset/data_size"))]
    pub const DATA_SIZE: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0040bf92, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxlleanimset/acquire"))]
    pub const ACQUIRE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004bddbc, function_type: PhantomData};
}

// GXMixer class functions
pub mod gxmixer {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/get_anim_set"))]
    pub const GET_ANIM_SET: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004010e7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/set_set"))]
    pub const SET_SET: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u8)> = FunctionDef{address: 0x0040165f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/set_base_0"))]
    pub const SET_BASE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x0040de33, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/total_clear"))]
    pub const TOTAL_CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040df80, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x0040e214, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0040e420, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/set_base_1"))]
    pub const SET_BASE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x0040e6c1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/get_anim"))]
    pub const GET_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00411fed, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x004325a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/set_time"))]
    pub const SET_TIME: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x004340e1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixer/clear_all"))]
    pub const CLEAR_ALL: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b1e62, function_type: PhantomData};
}

// GXMixerLink class functions
pub mod gxmixerlink {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixerlink/gxmixer_link"))]
    pub const GXMIXER_LINK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004016c7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxmixerlink/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u8, i32) -> u32> = FunctionDef{address: 0x0040de05, function_type: PhantomData};
}

// GXPalette class functions
pub mod gxpalette {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxpalette/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004abb26, function_type: PhantomData};
}

// GXPaletteMap class functions
pub mod gxpalettemap {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxpalettemap/copy_colors"))]
    pub const COPY_COLORS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00433f66, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxpalettemap/release"))]
    pub const RELEASE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ab9b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxpalettemap/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004aba2d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxpalettemap/set_pixel_format_0"))]
    pub const SET_PIXEL_FORMAT_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004abb05, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxpalettemap/set_pixel_format_1"))]
    pub const SET_PIXEL_FORMAT_1: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004abb84, function_type: PhantomData};
}

// GXVideoManager class functions
pub mod gxvideomanager {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/flip"))]
    pub const FLIP: FunctionDef<unsafe extern "fastcall" fn(u32) -> bool> = FunctionDef{address: 0x004013bf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/create_surface"))]
    pub const CREATE_SURFACE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0041ba74, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/flip_to_gdi"))]
    pub const FLIP_TO_GDI: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x004cc682, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/switch_mode"))]
    pub const SWITCH_MODE: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x004d601e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/start"))]
    pub const START: FunctionDef<unsafe extern "thiscall" fn(u32, i8, i32, i32, i32, i8) -> u32> = FunctionDef{address: 0x004d64ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/release_surfaces"))]
    pub const RELEASE_SURFACES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d67a2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/setup_fullscreen"))]
    pub const SETUP_FULLSCREEN: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x004d6894, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/create_flipping_chain"))]
    pub const CREATE_FLIPPING_CHAIN: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x004d6ae1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/setup_windowed"))]
    pub const SETUP_WINDOWED: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x0052f1d0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/create_blt_surfaces"))]
    pub const CREATE_BLT_SURFACES: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x0052f2a6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/create_direct_draw"))]
    pub const CREATE_DIRECT_DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00533d0a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/initialize"))]
    pub const INITIALIZE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x00533db5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/stop"))]
    pub const STOP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0057df69, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/gxvideo_manager"))]
    pub const GXVIDEO_MANAGER: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0057dfc7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("gxvideomanager/release_direct_draw"))]
    pub const RELEASE_DIRECT_DRAW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0057e01e, function_type: PhantomData};
}

// HTTPCallbackInterface class functions
pub mod httpcallbackinterface {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("httpcallbackinterface/check_completed"))]
    pub const CHECK_COMPLETED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0059f93f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httpcallbackinterface/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00626b1d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httpcallbackinterface/notify_done"))]
    pub const NOTIFY_DONE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00626b35, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httpcallbackinterface/check_error"))]
    pub const CHECK_ERROR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00626b4f, function_type: PhantomData};
}

// HTTPUtil class functions
pub mod httputil {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/parse_url"))]
    pub const PARSE_URL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, i32, u32, u32, u32)> = FunctionDef{address: 0x00627585, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/init_socket"))]
    pub const INIT_SOCKET: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u16, u32) -> u32> = FunctionDef{address: 0x00627699, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/send_string"))]
    pub const SEND_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00627775, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/get_httpmemory"))]
    pub const GET_HTTPMEMORY: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x006277ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/get_httpfile"))]
    pub const GET_HTTPFILE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00627a17, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/re_get_file"))]
    pub const RE_GET_FILE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00627c0c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/send_httpget"))]
    pub const SEND_HTTPGET: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00627c5d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/read_httpto_memory"))]
    pub const READ_HTTPTO_MEMORY: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, u32, i8) -> u32> = FunctionDef{address: 0x00627cc4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("httputil/read_httpto_file"))]
    pub const READ_HTTPTO_FILE: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x00627ef2, function_type: PhantomData};
}

// IScroller class functions
pub mod iscroller {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("iscroller/init_sb"))]
    pub const INIT_SB: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004d4971, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("iscroller/load_sb"))]
    pub const LOAD_SB: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00510ba2, function_type: PhantomData};
}

// MenuMusicHandler class functions
pub mod menumusichandler {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("menumusichandler/start_fade"))]
    pub const START_FADE: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x00592570, function_type: PhantomData};
}

// SNDSoundResource class functions
pub mod sndsoundresource {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("sndsoundresource/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0061ff52, function_type: PhantomData};
}

// SNDSoundResourceWAV class functions
pub mod sndsoundresourcewav {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("sndsoundresourcewav/attempt"))]
    pub const ATTEMPT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0041c4c2, function_type: PhantomData};
}

// Sorter class functions
pub mod sorter {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("sorter/insert_map"))]
    pub const INSERT_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i8, u32)> = FunctionDef{address: 0x00431c3e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("sorter/point_in_entity"))]
    pub const POINT_IN_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32, i32) -> u32> = FunctionDef{address: 0x004333c2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("sorter/set_map"))]
    pub const SET_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0044afe0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("sorter/acquire_anis"))]
    pub const ACQUIRE_ANIS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00461609, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("sorter/draw_tank1"))]
    pub const DRAW_TANK1: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32)> = FunctionDef{address: 0x004cd3a8, function_type: PhantomData};
}

// SoundGroup class functions
pub mod soundgroup {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("soundgroup/play"))]
    pub const PLAY: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x0043f513, function_type: PhantomData};
}

// UIBarGraphRenderer class functions
pub mod uibargraphrenderer {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uibargraphrenderer/render_graph_0"))]
    pub const RENDER_GRAPH_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0053211d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uibargraphrenderer/point_to_graph_pos"))]
    pub const POINT_TO_GRAPH_POS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, i32, i32, i32, u32, u32)> = FunctionDef{address: 0x005324c5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uibargraphrenderer/render_xlabels"))]
    pub const RENDER_XLABELS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005325f5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uibargraphrenderer/render_graph_1"))]
    pub const RENDER_GRAPH_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x006239d9, function_type: PhantomData};
}

// UIButton class functions
pub mod uibutton {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uibutton/validate_selected_button"))]
    pub const VALIDATE_SELECTED_BUTTON: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x004179de, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uibutton/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u8)> = FunctionDef{address: 0x004197a0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uibutton/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004d35ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uibutton/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004e9989, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uibutton/copy"))]
    pub const COPY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004e99ee, function_type: PhantomData};
}

// UICallbackMgr class functions
pub mod uicallbackmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uicallbackmgr/do_action"))]
    pub const DO_ACTION: FunctionDef<unsafe extern "stdcall" fn(u32, i32, i32)> = FunctionDef{address: 0x0041ab24, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicallbackmgr/stop"))]
    pub const STOP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f1daf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicallbackmgr/add_action"))]
    pub const ADD_ACTION: FunctionDef<unsafe extern "stdcall" fn(u32, i32, u32, u8)> = FunctionDef{address: 0x00512687, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicallbackmgr/start"))]
    pub const START: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0051f409, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicallbackmgr/add_global_trigger"))]
    pub const ADD_GLOBAL_TRIGGER: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u8)> = FunctionDef{address: 0x0058ec81, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicallbackmgr/remove_action"))]
    pub const REMOVE_ACTION: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x00620891, function_type: PhantomData};
}

// UIControl class functions
pub mod uicontrol {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/get_dc"))]
    pub const GET_DC: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0040e969, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/release_dc"))]
    pub const RELEASE_DC: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0040e9cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/set_text_0"))]
    pub const SET_TEXT_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, i8)> = FunctionDef{address: 0x0040eabf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/uiload_animation"))]
    pub const UILOAD_ANIMATION: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x004176ce, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/use_tooltip_id"))]
    pub const USE_TOOLTIP_ID: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00418a49, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x004195ae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/render"))]
    pub const RENDER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041a808, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/show"))]
    pub const SHOW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041acbb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/set_text_1"))]
    pub const SET_TEXT_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x0041afb5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/fill_canvases"))]
    pub const FILL_CANVASES: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x0041bce8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/dynamic_render"))]
    pub const DYNAMIC_RENDER: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32)> = FunctionDef{address: 0x004328ae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/hit_test"))]
    pub const HIT_TEST: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32) -> i8> = FunctionDef{address: 0x004420b2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/handle_mouse_move"))]
    pub const HANDLE_MOUSE_MOVE: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00442493, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/select_trigger"))]
    pub const SELECT_TRIGGER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00443365, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/handle_left_button_up"))]
    pub const HANDLE_LEFT_BUTTON_UP: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004435b9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/handle_left_button_down"))]
    pub const HANDLE_LEFT_BUTTON_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u32, u32, u32) -> u32> = FunctionDef{address: 0x0044361a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/deselect_trigger"))]
    pub const DESELECT_TRIGGER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004438dd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/set_image_0"))]
    pub const SET_IMAGE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004b1a32, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/set_animation"))]
    pub const SET_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool)> = FunctionDef{address: 0x004b1aa0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/use_animation"))]
    pub const USE_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32, i32, bool)> = FunctionDef{address: 0x004b1f89, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/add_image_name"))]
    pub const ADD_IMAGE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004d2b40, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004d32c9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/adjust_dimensions"))]
    pub const ADJUST_DIMENSIONS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004d483e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004d4909, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/uicontrol"))]
    pub const UICONTROL: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004e0996, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/constructor_0"))]
    pub const CONSTRUCTOR_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004e95ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/constructor_1"))]
    pub const CONSTRUCTOR_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0050fe83, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/get_text"))]
    pub const GET_TEXT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0058c4aa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uicontrol/set_image_1"))]
    pub const SET_IMAGE_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00620b61, function_type: PhantomData};
}

// UIElement class functions
pub mod uielement {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/set_flag"))]
    pub const SET_FLAG: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x004014ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/dirty"))]
    pub const DIRTY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00401802, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/hide"))]
    pub const HIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004018c6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/get_element_0"))]
    pub const GET_ELEMENT_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040760c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/sort"))]
    pub const SORT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040eab5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/load_state"))]
    pub const LOAD_STATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041215f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/get_screen_pos"))]
    pub const GET_SCREEN_POS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00418c5f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/enable"))]
    pub const ENABLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041914b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/disable"))]
    pub const DISABLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00419191, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/show"))]
    pub const SHOW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041ac49, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0041adf1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/handle_double_click"))]
    pub const HANDLE_DOUBLE_CLICK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041eabe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/handle_middle_button_down"))]
    pub const HANDLE_MIDDLE_BUTTON_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00429d0f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/get_element_1"))]
    pub const GET_ELEMENT_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004422fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/unhighlight"))]
    pub const UNHIGHLIGHT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00442562, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/highlight"))]
    pub const HIGHLIGHT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004425ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/unfocus"))]
    pub const UNFOCUS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004431f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/focus"))]
    pub const FOCUS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00443235, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/deselect"))]
    pub const DESELECT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004433d8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/select"))]
    pub const SELECT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00443414, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/handle_left_button_up"))]
    pub const HANDLE_LEFT_BUTTON_UP: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004435a8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/handle_left_button_down"))]
    pub const HANDLE_LEFT_BUTTON_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0044360a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/set_tooltip_id"))]
    pub const SET_TOOLTIP_ID: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00443bfc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/set_layout"))]
    pub const SET_LAYOUT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004b203f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/reload"))]
    pub const RELOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d5777, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/handle_mouse_wheel"))]
    pub const HANDLE_MOUSE_WHEEL: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u8, u8, u32) -> u32> = FunctionDef{address: 0x004de381, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/set_alias_element"))]
    pub const SET_ALIAS_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x004ffb8b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0050f8e7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/registerit"))]
    pub const REGISTERIT: FunctionDef<unsafe extern "cdecl" fn(u32, u32) -> u32> = FunctionDef{address: 0x005774bf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/set_global_grayed"))]
    pub const SET_GLOBAL_GRAYED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a0d64, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uielement/handle_key_down"))]
    pub const HANDLE_KEY_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00606ce7, function_type: PhantomData};
}

// UIGraphRenderer class functions
pub mod uigraphrenderer {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uigraphrenderer/calc_display_ranges"))]
    pub const CALC_DISPLAY_RANGES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0052f46b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uigraphrenderer/round_up_max"))]
    pub const ROUND_UP_MAX: FunctionDef<unsafe extern "stdcall" fn(i32) -> i32> = FunctionDef{address: 0x0052fe2d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uigraphrenderer/render"))]
    pub const RENDER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00531dbd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uigraphrenderer/render_point_label"))]
    pub const RENDER_POINT_LABEL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i32, i32)> = FunctionDef{address: 0x00531ec5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uigraphrenderer/render_labels"))]
    pub const RENDER_LABELS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005325da, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uigraphrenderer/render_axis_lines"))]
    pub const RENDER_AXIS_LINES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005328f4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uigraphrenderer/render_ylabels"))]
    pub const RENDER_YLABELS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0053296d, function_type: PhantomData};
}

// UIImage class functions
pub mod uiimage {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uiimage/render"))]
    pub const RENDER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00443cbf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiimage/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004d3509, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiimage/uiimage"))]
    pub const UIIMAGE: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0050295e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiimage/create"))]
    pub const CREATE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00606fd2, function_type: PhantomData};
}

// UIImageSet class functions
pub mod uiimageset {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uiimageset/set_status"))]
    pub const SET_STATUS: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32)> = FunctionDef{address: 0x004f1b13, function_type: PhantomData};
}

// UILayout class functions
pub mod uilayout {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0041952b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/remove_element"))]
    pub const REMOVE_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0041ae42, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/bring_to_front"))]
    pub const BRING_TO_FRONT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0041aeb1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/show"))]
    pub const SHOW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b1b86, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/add_element"))]
    pub const ADD_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004b206c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/load_element"))]
    pub const LOAD_ELEMENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0050f61a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/handle_left_button_up"))]
    pub const HANDLE_LEFT_BUTTON_UP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0058c88e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilayout/handle_left_button_down"))]
    pub const HANDLE_LEFT_BUTTON_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32) -> u32> = FunctionDef{address: 0x0058c8b4, function_type: PhantomData};
}

// UIListBox class functions
pub mod uilistbox {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/recalc_scroll_bar"))]
    pub const RECALC_SCROLL_BAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00417645, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/absolute_scroll"))]
    pub const ABSOLUTE_SCROLL: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8, i8)> = FunctionDef{address: 0x00417695, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/local_render"))]
    pub const LOCAL_RENDER: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x00417a8e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/add_string_0"))]
    pub const ADD_STRING_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u8, u32, u32, u32)> = FunctionDef{address: 0x00417f79, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/item_now_visible"))]
    pub const ITEM_NOW_VISIBLE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x00418372, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/add_string_1"))]
    pub const ADD_STRING_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00418504, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004188de, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/set_selected"))]
    pub const SET_SELECTED: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x004189e8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/show"))]
    pub const SHOW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00418a71, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/set_item_visible"))]
    pub const SET_ITEM_VISIBLE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x00418bfb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/get_item"))]
    pub const GET_ITEM: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0041ee59, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d393f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/absolute_to_visible"))]
    pub const ABSOLUTE_TO_VISIBLE: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> i32> = FunctionDef{address: 0x004ed6d2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/restore_state"))]
    pub const RESTORE_STATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ef81c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/add_string_2"))]
    pub const ADD_STRING_2: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u8, u32, u32, u32) -> u32> = FunctionDef{address: 0x004f0331, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/get_item_string"))]
    pub const GET_ITEM_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x004f05b2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/item_now_invisible"))]
    pub const ITEM_NOW_INVISIBLE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x004f984e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/remove_string"))]
    pub const REMOVE_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x004f9908, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/increment"))]
    pub const INCREMENT: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x004f9ffa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/set_item_string"))]
    pub const SET_ITEM_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x005aecdc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/is_item_selected"))]
    pub const IS_ITEM_SELECTED: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x00622dcb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistbox/find_item_absolute_id"))]
    pub const FIND_ITEM_ABSOLUTE_ID: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> i32> = FunctionDef{address: 0x00622e92, function_type: PhantomData};
}

// UIListBoxEntry class functions
pub mod uilistboxentry {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxentry/calc_size"))]
    pub const CALC_SIZE: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x00417c8c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxentry/set_normal_icon"))]
    pub const SET_NORMAL_ICON: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00417e94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxentry/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u32, u8) -> u32> = FunctionDef{address: 0x004181f7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxentry/set_selected_icon"))]
    pub const SET_SELECTED_ICON: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0047098b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxentry/set_grayed_icon"))]
    pub const SET_GRAYED_ICON: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00474c26, function_type: PhantomData};
}

// UIListBoxItem class functions
pub mod uilistboxitem {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32, u32, u8, u32, u32, u32) -> u32> = FunctionDef{address: 0x00418000, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/calc_size"))]
    pub const CALC_SIZE: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0041848c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/set_justify"))]
    pub const SET_JUSTIFY: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x004185f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/render_text_entries"))]
    pub const RENDER_TEXT_ENTRIES: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, u32, i32, u32)> = FunctionDef{address: 0x004ecba7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/render_icon_entries"))]
    pub const RENDER_ICON_ENTRIES: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32)> = FunctionDef{address: 0x004ece25, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/get_string"))]
    pub const GET_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004f0571, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/add_entry"))]
    pub const ADD_ENTRY: FunctionDef<unsafe extern "thiscall" fn(u32, i8, u32, u32, u32, u32, u32, u32)> = FunctionDef{address: 0x00531863, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/disable"))]
    pub const DISABLE: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0058f57c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uilistboxitem/set_string"))]
    pub const SET_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x005aebf5, function_type: PhantomData};
}

// UIMessageQueue class functions
pub mod uimessagequeue {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uimessagequeue/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x004197fc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uimessagequeue/add_message"))]
    pub const ADD_MESSAGE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32, u32, bool, bool) -> u32> = FunctionDef{address: 0x0049cd46, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uimessagequeue/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x004c6d4e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uimessagequeue/render"))]
    pub const RENDER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fad0f, function_type: PhantomData};
}

// UIPointGraphRenderer class functions
pub mod uipointgraphrenderer {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uipointgraphrenderer/render_graph"))]
    pub const RENDER_GRAPH: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00623eb8, function_type: PhantomData};
}

// UIRadioSet class functions
pub mod uiradioset {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uiradioset/select_button"))]
    pub const SELECT_BUTTON: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004438fb, function_type: PhantomData};
}

// UIScrollBar class functions
pub mod uiscrollbar {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollbar/recalc_thumb_position"))]
    pub const RECALC_THUMB_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041755a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollbar/render"))]
    pub const RENDER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b22a2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollbar/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00511212, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollbar/copy"))]
    pub const COPY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00620636, function_type: PhantomData};
}

// UIScrollingRegion class functions
pub mod uiscrollingregion {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/copy"))]
    pub const COPY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004010e1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/hide"))]
    pub const HIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00443981, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d3e23, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004d4c85, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/clear_selection"))]
    pub const CLEAR_SELECTION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004df638, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/depopulate"))]
    pub const DEPOPULATE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004e0cd1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/populate"))]
    pub const POPULATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, i8) -> u32> = FunctionDef{address: 0x004e90c2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/show"))]
    pub const SHOW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ea015, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/uiscrolling_region_0"))]
    pub const UISCROLLING_REGION_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00502fe5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/uiscrolling_region_1"))]
    pub const UISCROLLING_REGION_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00503003, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x005125d8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/handle_char"))]
    pub const HANDLE_CHAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00622ef8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiscrollingregion/select_next_button"))]
    pub const SELECT_NEXT_BUTTON: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00622f18, function_type: PhantomData};
}

// UIStatusImage class functions
pub mod uistatusimage {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uistatusimage/set_status"))]
    pub const SET_STATUS: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32)> = FunctionDef{address: 0x0041cefc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uistatusimage/clear_for_load"))]
    pub const CLEAR_FOR_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d3de0, function_type: PhantomData};
}

// UIText class functions
pub mod uitext {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uitext/recalc_scroll_bar"))]
    pub const RECALC_SCROLL_BAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040eaa0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uitext/render"))]
    pub const RENDER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041a476, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uitext/fit_to_text"))]
    pub const FIT_TO_TEXT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041b0bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uitext/set_scroll_bar"))]
    pub const SET_SCROLL_BAR: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004d4c5d, function_type: PhantomData};
}

// UIView class functions
pub mod uiview {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("uiview/constructor_0"))]
    pub const CONSTRUCTOR_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0052059c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiview/copy"))]
    pub const COPY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0060a8d2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("uiview/constructor_1"))]
    pub const CONSTRUCTOR_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00620a90, function_type: PhantomData};
}

// UpdateHandler class functions
pub mod updatehandler {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/parse_index_0"))]
    pub const PARSE_INDEX_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ab2f7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00624e20, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/prune_update_groups"))]
    pub const PRUNE_UPDATE_GROUPS: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x00624fb6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/destination_from_url"))]
    pub const DESTINATION_FROM_URL: FunctionDef<unsafe extern "stdcall" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x0062534b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/is_item_downloaded"))]
    pub const IS_ITEM_DOWNLOADED: FunctionDef<unsafe extern "stdcall" fn(i32) -> u32> = FunctionDef{address: 0x006256d2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/finalize_updates"))]
    pub const FINALIZE_UPDATES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00625907, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/move_file_from_tmp"))]
    pub const MOVE_FILE_FROM_TMP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00625a9b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/parse_index_1"))]
    pub const PARSE_INDEX_1: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x00625d13, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/start_index_download"))]
    pub const START_INDEX_DOWNLOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00626205, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/start_download"))]
    pub const START_DOWNLOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00626247, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/download_file_group"))]
    pub const DOWNLOAD_FILE_GROUP: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00626350, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/delete_bad_files"))]
    pub const DELETE_BAD_FILES: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x006264bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("updatehandler/notify_done"))]
    pub const NOTIFY_DONE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006265c3, function_type: PhantomData};
}

// ZLibControl class functions
pub mod zlibcontrol {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zlibcontrol/get_buffer"))]
    pub const GET_BUFFER: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0040782e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zlibcontrol/decompress"))]
    pub const DECOMPRESS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32) -> u32> = FunctionDef{address: 0x00408c1f, function_type: PhantomData};
}

// ZTAIMgr class functions
pub mod ztaimgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_guest_routine_f"))]
    pub const F_GUEST_ROUTINE_F: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x004224f5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_face_toward_food"))]
    pub const F_FACE_TOWARD_FOOD: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0042662c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_face_toward_food_f"))]
    pub const F_FACE_TOWARD_FOOD_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x00426665, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_face_toward_target"))]
    pub const F_FACE_TOWARD_TARGET: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00426e50, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_face_toward_target_f"))]
    pub const F_FACE_TOWARD_TARGET_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x00426ed9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_play_set_terrain_f"))]
    pub const F_PLAY_SET_TERRAIN_F: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004314d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/execute_call"))]
    pub const EXECUTE_CALL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x00436065, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_exit_building"))]
    pub const F_EXIT_BUILDING: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x004a7744, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_exit_building_f"))]
    pub const F_EXIT_BUILDING_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x004a777e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_keeper_routine_f"))]
    pub const F_KEEPER_ROUTINE_F: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x004a8763, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_maint_routine_f"))]
    pub const F_MAINT_ROUTINE_F: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x004a945f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/ztaimgr"))]
    pub const ZTAIMGR: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00503857, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_show_predator"))]
    pub const F_SHOW_PREDATOR: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0050e6fa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_show_predator_f"))]
    pub const F_SHOW_PREDATOR_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x0050e764, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_hide_predator_f"))]
    pub const F_HIDE_PREDATOR_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x0050e879, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/create"))]
    pub const CREATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00525961, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/init_aiparams"))]
    pub const INIT_AIPARAMS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0052599f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_caught_prey"))]
    pub const F_CAUGHT_PREY: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00588c2d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_dust_ball"))]
    pub const F_DUST_BALL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00588d47, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_dust_ball_f"))]
    pub const F_DUST_BALL_F: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x00588f69, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_guide_routine_f"))]
    pub const F_GUIDE_ROUTINE_F: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x0058a998, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_caught_prey_f"))]
    pub const F_CAUGHT_PREY_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x005a593f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_target_building_f"))]
    pub const F_TARGET_BUILDING_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32) -> u32> = FunctionDef{address: 0x005a5b71, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_end_trick_f"))]
    pub const F_END_TRICK_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x005a6a22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_enter_building_f"))]
    pub const F_ENTER_BUILDING_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x005ad42c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_play_set_tank_f"))]
    pub const F_PLAY_SET_TANK_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32) -> u32> = FunctionDef{address: 0x005b2be5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_spray_f"))]
    pub const F_SPRAY_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x0060caad, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_hatch_f"))]
    pub const F_HATCH_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x0060cacb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_hatch"))]
    pub const F_HATCH: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0060cae0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_kill_prey_f"))]
    pub const F_KILL_PREY_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x0060cb25, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_kill_prey"))]
    pub const F_KILL_PREY: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0060cb3a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_die_f"))]
    pub const F_DIE_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x0060cbb3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_die"))]
    pub const F_DIE: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0060cbc8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztaimgr/f_return_to_building_f"))]
    pub const F_RETURN_TO_BUILDING_F: FunctionDef<unsafe extern "stdcall" fn(i32, u32)> = FunctionDef{address: 0x0060cc66, function_type: PhantomData};
}

// ZTAdvTerrainMgr class functions
pub mod ztadvterrainmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00419e14, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/setup_render"))]
    pub const SETUP_RENDER: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00463e60, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/set_ground_image"))]
    pub const SET_GROUND_IMAGE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i32) -> i8> = FunctionDef{address: 0x004ac218, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/render_shape"))]
    pub const RENDER_SHAPE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004adff8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/render_pass"))]
    pub const RENDER_PASS: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32, i32, i32, u32) -> u32> = FunctionDef{address: 0x004ae4b3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/set_aux_image"))]
    pub const SET_AUX_IMAGE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x004aecb2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/set_image"))]
    pub const SET_IMAGE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004aede5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/stop"))]
    pub const STOP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005039cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/stop2d"))]
    pub const STOP2D: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00503b7a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/ztadv_terrain_mgr_0"))]
    pub const ZTADV_TERRAIN_MGR_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0050435b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/ztadv_terrain_mgr_1"))]
    pub const ZTADV_TERRAIN_MGR_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0050458d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/start"))]
    pub const START: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00522470, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/load_textures_0"))]
    pub const LOAD_TEXTURES_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005224b9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/load_textures_1"))]
    pub const LOAD_TEXTURES_1: FunctionDef<unsafe extern "stdcall" fn(u32, u8, u8, u8, u8, u8, u32, u8, u8, u8, u32, u8, u32)> = FunctionDef{address: 0x005228b3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/start2d"))]
    pub const START2D: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00522bd4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/start_d3d"))]
    pub const START_D3D: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00523ba3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/create"))]
    pub const CREATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00525470, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztadvterrainmgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "fastcall" fn(u8)> = FunctionDef{address: 0x005254af, function_type: PhantomData};
}

// ZTAmbient class functions
pub mod ztambient {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/ztambient_0"))]
    pub const ZTAMBIENT_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00420519, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/ztambient_1"))]
    pub const ZTAMBIENT_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00420547, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/calc_shadow_world_position"))]
    pub const CALC_SHADOW_WORLD_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00433cad, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/update_position"))]
    pub const UPDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00434e9c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00450261, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047a7fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fc145, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/init_status_vars"))]
    pub const INIT_STATUS_VARS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fc160, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambient/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004fc3aa, function_type: PhantomData};
}

// ZTAmbientType class functions
pub mod ztambienttype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztambienttype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c2ec9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambienttype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fc358, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambienttype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x0051ca20, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambienttype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051cd83, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambienttype/ztambient_type_0"))]
    pub const ZTAMBIENT_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057cc85, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztambienttype/ztambient_type_1"))]
    pub const ZTAMBIENT_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0057cccb, function_type: PhantomData};
}

// ZTAnimal class functions
pub mod ztanimal {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/calc_habitat"))]
    pub const CALC_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00410675, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/get_footprint"))]
    pub const GET_FOOTPRINT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool)> = FunctionDef{address: 0x00410803, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/get_habitat_rating"))]
    pub const GET_HABITAT_RATING: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x00411afd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/set_home_habitat"))]
    pub const SET_HOME_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00411b92, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/get_idle_anim"))]
    pub const GET_IDLE_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> i32> = FunctionDef{address: 0x00413582, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/preys_on_man"))]
    pub const PREYS_ON_MAN: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004138f0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/is_in_prey_list"))]
    pub const IS_IN_PREY_LIST: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0041391d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/set_terrain_mode"))]
    pub const SET_TERRAIN_MODE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i8)> = FunctionDef{address: 0x004139c9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/get_terrain_cost"))]
    pub const GET_TERRAIN_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00414b13, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/is_walkable"))]
    pub const IS_WALKABLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00414c7f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/recalc_home_habitat"))]
    pub const RECALC_HOME_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00416185, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/is_unhappy_with_habitat"))]
    pub const IS_UNHAPPY_WITH_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00418b8c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/face_toward_food"))]
    pub const FACE_TOWARD_FOOD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042661b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/is_preattack"))]
    pub const IS_PREATTACK: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0042fad3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/f_check_reproduction"))]
    pub const F_CHECK_REPRODUCTION: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x0043103b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/is_walkable_by"))]
    pub const IS_WALKABLE_BY: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00431696, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x00433933, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00436f11, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/update_position"))]
    pub const UPDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00436f32, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/update_goals"))]
    pub const UPDATE_GOALS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00436f5c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_ambient_anims"))]
    pub const DO_AMBIENT_ANIMS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00437102, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_conditionals"))]
    pub const DO_CONDITIONALS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00437620, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_boxed_check"))]
    pub const DO_BOXED_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004377fc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_egg_check"))]
    pub const DO_EGG_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00437860, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/update_status_variables"))]
    pub const UPDATE_STATUS_VARIABLES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004378df, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_sleep_check"))]
    pub const DO_SLEEP_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004384ba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_escaped_check"))]
    pub const DO_ESCAPED_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00438514, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_reproduce_check"))]
    pub const DO_REPRODUCE_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004385e5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_baby_to_adult_check"))]
    pub const DO_BABY_TO_ADULT_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00438663, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_social_check"))]
    pub const DO_SOCIAL_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004386a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_health_check"))]
    pub const DO_HEALTH_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004389aa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_breath_check"))]
    pub const DO_BREATH_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004389f7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_murky_water_check"))]
    pub const DO_MURKY_WATER_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00438a37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_hunger_check"))]
    pub const DO_HUNGER_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00438a79, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_bored_check"))]
    pub const DO_BORED_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00438ad7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_habitat_check"))]
    pub const DO_HABITAT_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00438c47, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_keeper_check"))]
    pub const DO_KEEPER_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00438d31, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_other_check"))]
    pub const DO_OTHER_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00438e70, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_zap_check"))]
    pub const DO_ZAP_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00438ebc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_building_use_check"))]
    pub const DO_BUILDING_USE_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00438f20, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/do_water_check"))]
    pub const DO_WATER_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00438f6c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/set_escaped"))]
    pub const SET_ESCAPED: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004409f8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/add_to_map"))]
    pub const ADD_TO_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00449c94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00453cc6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00455533, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00478cff, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/get_undesirable_scenery_in_habitat"))]
    pub const GET_UNDESIRABLE_SCENERY_IN_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00483979, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/chase_done"))]
    pub const CHASE_DONE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004a4d5f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/stop_chase"))]
    pub const STOP_CHASE: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x004a5e00, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/put_down_dirt"))]
    pub const PUT_DOWN_DIRT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x004a8f89, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/set_name"))]
    pub const SET_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004e1d6b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/get_icon_anim"))]
    pub const GET_ICON_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ee5be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/ztanimal_0"))]
    pub const ZTANIMAL_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x004f9d83, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/ztanimal_1"))]
    pub const ZTANIMAL_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f9dac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/ztanimal_2"))]
    pub const ZTANIMAL_2: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x004f9e18, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/is_selectable"))]
    pub const IS_SELECTABLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fb70d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/stop_eating"))]
    pub const STOP_EATING: FunctionDef<unsafe extern "thiscall" fn(u32, u8)> = FunctionDef{address: 0x004fd86e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/f_eat"))]
    pub const F_EAT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004fd951, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fe296, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/box"))]
    pub const BOX: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00505941, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/consume"))]
    pub const CONSUME: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0050e648, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/within_preattack_range"))]
    pub const WITHIN_PREATTACK_RANGE: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0050eae2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/show_escaped_animal_alert"))]
    pub const SHOW_ESCAPED_ANIMAL_ALERT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0058cb88, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/drop_test_fence_chance"))]
    pub const DROP_TEST_FENCE_CHANCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0059c4ba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/set_egg"))]
    pub const SET_EGG: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a5283, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/set_dying"))]
    pub const SET_DYING: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a5606, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/draw_aiinfo"))]
    pub const DRAW_AIINFO: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32)> = FunctionDef{address: 0x00613641, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/break_escape_fence"))]
    pub const BREAK_ESCAPE_FENCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006137fe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/change_to_dirt"))]
    pub const CHANGE_TO_DIRT: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x00613c0c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/hatch_egg"))]
    pub const HATCH_EGG: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00613ca6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimal/finish_hatch"))]
    pub const FINISH_HATCH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00613d24, function_type: PhantomData};
}

// ZTAnimalType class functions
pub mod ztanimaltype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimaltype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00453d41, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimaltype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b659e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimaltype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x004b6c51, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimaltype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004be6ff, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimaltype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004bece3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztanimaltype/ztanimal_type"))]
    pub const ZTANIMAL_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00500be5, function_type: PhantomData};
}

// ZTApp class functions
pub mod ztapp {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_app"))]
    pub const GET_APP: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004010c7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/redraw_screen"))]
    pub const REDRAW_SCREEN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00419312, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/update_ui"))]
    pub const UPDATE_UI: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00419d8c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/do_each_loop"))]
    pub const DO_EACH_LOOP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041a6cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/update_sim"))]
    pub const UPDATE_SIM: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0041a6d1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_screen_center"))]
    pub const GET_SCREEN_CENTER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043f30d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/handle_messages"))]
    pub const HANDLE_MESSAGES: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, u32) -> i32> = FunctionDef{address: 0x00441880, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/handle_char2"))]
    pub const HANDLE_CHAR2: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x0046bdd6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/init_instance"))]
    pub const INIT_INSTANCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004bc17f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/restart_graphics_mgr"))]
    pub const RESTART_GRAPHICS_MGR: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x004caa0f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/reload_ui"))]
    pub const RELOAD_UI: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004d5a49, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/toggle_fullscreen"))]
    pub const TOGGLE_FULLSCREEN: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004d5c7f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/force_redraw_0"))]
    pub const FORCE_REDRAW_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004d5dfe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/resize_app"))]
    pub const RESIZE_APP: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x004d6dce, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/force_redraw_1"))]
    pub const FORCE_REDRAW_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004d6f85, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/handle_key_down"))]
    pub const HANDLE_KEY_DOWN: FunctionDef<unsafe extern "cdecl" fn(i32) -> u32> = FunctionDef{address: 0x004f0b72, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/exit_override"))]
    pub const EXIT_OVERRIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00501303, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_ai"))]
    pub const GET_AI: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00514ae1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/toggle_entity_visibility"))]
    pub const TOGGLE_ENTITY_VISIBILITY: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0052df81, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_next_screenshot_name"))]
    pub const GET_NEXT_SCREENSHOT_NAME: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005330cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/load_typedata"))]
    pub const LOAD_TYPEDATA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0053768a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/toggle_cursors"))]
    pub const TOGGLE_CURSORS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0060442d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_update_time"))]
    pub const GET_UPDATE_TIME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00604480, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_world"))]
    pub const GET_WORLD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006206bc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_game"))]
    pub const GET_GAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006206c2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_scenario"))]
    pub const GET_SCENARIO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006206c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztapp/get_script"))]
    pub const GET_SCRIPT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006206ce, function_type: PhantomData};
}

// ZTAwardMgr class functions
pub mod ztawardmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztawardmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047a064, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztawardmgr/start"))]
    pub const START: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005141e1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztawardmgr/get_award"))]
    pub const GET_AWARD: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> i32> = FunctionDef{address: 0x005a12ec, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztawardmgr/add_award"))]
    pub const ADD_AWARD: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005a13ed, function_type: PhantomData};
}

// ZTBuilding class functions
pub mod ztbuilding {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/user_data_changed"))]
    pub const USER_DATA_CHANGED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00412513, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/force_dead_anim"))]
    pub const FORCE_DEAD_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004174dd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/might_start_ambient_sound"))]
    pub const MIGHT_START_AMBIENT_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041e3f8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/has_open_slot"))]
    pub const HAS_OPEN_SLOT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00429330, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/gx_pos_from_xypos_0"))]
    pub const GX_POS_FROM_XYPOS_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0042a10c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_valid_exit"))]
    pub const GET_VALID_EXIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0042aad5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/can_add_user"))]
    pub const CAN_ADD_USER: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042ac87, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/gx_pos_from_xypos_1"))]
    pub const GX_POS_FROM_XYPOS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042afd4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_slot_entrances"))]
    pub const GET_SLOT_ENTRANCES: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042b101, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_valid_entrance_for_exit"))]
    pub const GET_VALID_ENTRANCE_FOR_EXIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0042b350, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/find_nearest_open_slot"))]
    pub const FIND_NEAREST_OPEN_SLOT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042c02b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/might_start_use_sound"))]
    pub const MIGHT_START_USE_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042cc1a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/add_user"))]
    pub const ADD_USER: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042ce0a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/might_start_use_animation"))]
    pub const MIGHT_START_USE_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042d01c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/remove_items"))]
    pub const REMOVE_ITEMS: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u8> = FunctionDef{address: 0x0042d181, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/buy_item"))]
    pub const BUY_ITEM: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0042d1e2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/remove_user"))]
    pub const REMOVE_USER: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x0042d858, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/yank_user"))]
    pub const YANK_USER: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x0042da64, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/draw_0"))]
    pub const DRAW_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x00433aaf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00434c4b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/update_random_use_timer"))]
    pub const UPDATE_RANDOM_USE_TIMER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00434d2a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/reset_info"))]
    pub const RESET_INFO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00450dbf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/reset_construction_date"))]
    pub const RESET_CONSTRUCTION_DATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00450edd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/init_status_vars"))]
    pub const INIT_STATUS_VARS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00450f17, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045184a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/add_member"))]
    pub const ADD_MEMBER: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0048cd9b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/play_use_animation"))]
    pub const PLAY_USE_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a067b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/receive_income"))]
    pub const RECEIVE_INCOME: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004a2e96, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_slot_exit"))]
    pub const GET_SLOT_EXIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004a705e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/exit_via_exit"))]
    pub const EXIT_VIA_EXIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u8> = FunctionDef{address: 0x004a75bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/init_random_use_timer"))]
    pub const INIT_RANDOM_USE_TIMER: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ab763, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/might_stop_ambient_sound"))]
    pub const MIGHT_STOP_AMBIENT_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b0bca, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/drop"))]
    pub const DROP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004e5282, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x0050023a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/calculate_water_capacity"))]
    pub const CALCULATE_WATER_CAPACITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0059398a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/init_after_load"))]
    pub const INIT_AFTER_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00593fd3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/remove_user_if_present"))]
    pub const REMOVE_USER_IF_PRESENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0059be7b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_fun_gift_if_available"))]
    pub const GET_FUN_GIFT_IF_AVAILABLE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0059cd02, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/draw_1"))]
    pub const DRAW_1: FunctionDef<unsafe extern "stdcall" fn(u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u32)> = FunctionDef{address: 0x0059fb8f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/remove_member"))]
    pub const REMOVE_MEMBER: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005a08ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_slot_draw_loc"))]
    pub const GET_SLOT_DRAW_LOC: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32)> = FunctionDef{address: 0x005a954d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_slot_facing"))]
    pub const GET_SLOT_FACING: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x005ad288, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_slot_loc"))]
    pub const GET_SLOT_LOC: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x005ad347, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/get_slot_has_draw_slots"))]
    pub const GET_SLOT_HAS_DRAW_SLOTS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x005ad441, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/remove_all_users"))]
    pub const REMOVE_ALL_USERS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ae745, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/yank_user_if_present"))]
    pub const YANK_USER_IF_PRESENT: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x00611a98, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/turn_to_rubble"))]
    pub const TURN_TO_RUBBLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00611bd6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding/empty_grandstands"))]
    pub const EMPTY_GRANDSTANDS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006127b8, function_type: PhantomData};
}

// ZTBuilding::Slot class functions
pub mod ztbuilding_slot {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding_slot/remove_draw_user"))]
    pub const REMOVE_DRAW_USER: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0042dcd7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding_slot/get_top_user"))]
    pub const GET_TOP_USER: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004a7412, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding_slot/add_draw_user"))]
    pub const ADD_DRAW_USER: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32)> = FunctionDef{address: 0x005a93fe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuilding_slot/set_top_user"))]
    pub const SET_TOP_USER: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x005ac37c, function_type: PhantomData};
}

// ZTBuildingType class functions
pub mod ztbuildingtype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuildingtype/get_slot_entrances"))]
    pub const GET_SLOT_ENTRANCES: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042b29f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuildingtype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x0045e8f2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuildingtype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0045ecdf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuildingtype/get_slot_is_exit_on_water"))]
    pub const GET_SLOT_IS_EXIT_ON_WATER: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004a4a0f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuildingtype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c042c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuildingtype/add_upgrade"))]
    pub const ADD_UPGRADE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, i32, i32, i32)> = FunctionDef{address: 0x0058ff14, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztbuildingtype/remove_upgrade"))]
    pub const REMOVE_UPGRADE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0059012d, function_type: PhantomData};
}

// ZTCheat class functions
pub mod ztcheat {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00435282, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/rename_entity"))]
    pub const RENAME_ENTITY: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32)> = FunctionDef{address: 0x0044940b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/save"))]
    pub const SAVE: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x0047a10c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004c6a3e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/guest_enters"))]
    pub const GUEST_ENTERS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f7ebe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00514554, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/you_get"))]
    pub const YOU_GET: FunctionDef<unsafe extern "cdecl" fn(i32, bool)> = FunctionDef{address: 0x00591325, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/start_of_game"))]
    pub const START_OF_GAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00591426, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztcheat/rename_exhibit"))]
    pub const RENAME_EXHIBIT: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32)> = FunctionDef{address: 0x005aee66, function_type: PhantomData};
}

// ZTFence class functions
pub mod ztfence {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/set_animation"))]
    pub const SET_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u32)> = FunctionDef{address: 0x00416ee8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/get_esthetic_bonus"))]
    pub const GET_ESTHETIC_BONUS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041fb50, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/update_animation"))]
    pub const UPDATE_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00434ad7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00434b1d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/ztfence_0"))]
    pub const ZTFENCE_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00443f37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/ztfence_1"))]
    pub const ZTFENCE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00443f57, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/dirty_habitat_escapability"))]
    pub const DIRTY_HABITAT_ESCAPABILITY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00449ab4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/validate_position"))]
    pub const VALIDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x00449fb2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/add_to_map"))]
    pub const ADD_TO_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044cb57, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0044ff2d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/make_gate"))]
    pub const MAKE_GATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0046600c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00477e25, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/can_deteriorate"))]
    pub const CAN_DETERIORATE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004857b9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/set_broken"))]
    pub const SET_BROKEN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004858a4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/morph"))]
    pub const MORPH: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0048676c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/get_mwtile"))]
    pub const GET_MWTILE: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0049e3e2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/make_fence"))]
    pub const MAKE_FENCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f93da, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/get_sell_price"))]
    pub const GET_SELL_PRICE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> f32> = FunctionDef{address: 0x004f945b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ffd6e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/init_after_load"))]
    pub const INIT_AFTER_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u32)> = FunctionDef{address: 0x00594060, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/get_height"))]
    pub const GET_HEIGHT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005997fa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/set_healthy"))]
    pub const SET_HEALTHY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b391c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfence/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x005b3a04, function_type: PhantomData};
}

// ZTFenceType class functions
pub mod ztfencetype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztfencetype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004606f3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfencetype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004bb6f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfencetype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004bb8ed, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfencetype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004bf505, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfencetype/ztfence_type"))]
    pub const ZTFENCE_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0050118e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfencetype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005b3a55, function_type: PhantomData};
}

// ZTFood class functions
pub mod ztfood {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztfood/ztfood_0"))]
    pub const ZTFOOD_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0041e080, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfood/ztfood_1"))]
    pub const ZTFOOD_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x004f9f46, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfood/ztfood_2"))]
    pub const ZTFOOD_2: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f9f64, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfood/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x004fd1be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfood/changed_food_value"))]
    pub const CHANGED_FOOD_VALUE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fd3c1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfood/get_fullness"))]
    pub const GET_FULLNESS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004fd494, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfood/set_animation"))]
    pub const SET_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32, u8)> = FunctionDef{address: 0x004fd512, function_type: PhantomData};
}

// ZTFoodType class functions
pub mod ztfoodtype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztfoodtype/get_class"))]
    pub const GET_CLASS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00401fa4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfoodtype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004653d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfoodtype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00465634, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfoodtype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c2473, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfoodtype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004fd1de, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfoodtype/ztfood_type_0"))]
    pub const ZTFOOD_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00504c06, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztfoodtype/ztfood_type_1"))]
    pub const ZTFOOD_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00504c24, function_type: PhantomData};
}

// ZTGameMgr class functions
pub mod ztgamemgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/get_date"))]
    pub const GET_DATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0040e7e0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/add_cash"))]
    pub const ADD_CASH: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0040f018, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/is_real_world_date"))]
    pub const IS_REAL_WORLD_DATE: FunctionDef<unsafe extern "stdcall" fn(i32, u32) -> u32> = FunctionDef{address: 0x00412c22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0041a154, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/subtract_cash"))]
    pub const SUBTRACT_CASH: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0041ef68, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/hours_ago"))]
    pub const HOURS_AGO: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> u64> = FunctionDef{address: 0x0041f075, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/time_ago"))]
    pub const TIME_AGO: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0042620a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/update_sim"))]
    pub const UPDATE_SIM: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00435055, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047acc5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/removed_zoo_doo"))]
    pub const REMOVED_ZOO_DOO: FunctionDef<unsafe extern "stdcall" fn(u8, u8, u32, u8, u32, u32, u32, u32, u8, u8, u32)> = FunctionDef{address: 0x004a2ee1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/start_menu_music"))]
    pub const START_MENU_MUSIC: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004bded9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/start_menu_music_fade_0"))]
    pub const START_MENU_MUSIC_FADE_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c9d67, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/start_menu_music_fade_1"))]
    pub const START_MENU_MUSIC_FADE_1: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004ca478, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/start_menu_music_fade_2"))]
    pub const START_MENU_MUSIC_FADE_2: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004cc59d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/goto_start"))]
    pub const GOTO_START: FunctionDef<unsafe extern "stdcall" fn(u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u32, u32) -> u32> = FunctionDef{address: 0x004cc5b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/is_game_date"))]
    pub const IS_GAME_DATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004f5f7a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/stop"))]
    pub const STOP: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004fa123, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/ztgame_mgr_0"))]
    pub const ZTGAME_MGR_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00504dd8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/ztgame_mgr_1"))]
    pub const ZTGAME_MGR_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00504e4c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/set_new_game_defaults"))]
    pub const SET_NEW_GAME_DEFAULTS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x0058f39c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/start"))]
    pub const START: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x00592283, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005948c6, function_type: PhantomData};
}

// ZTGameMgr::MenuMusicHandler class functions
pub mod ztgamemgr_menumusichandler {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr_menumusichandler/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0041a13a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgamemgr_menumusichandler/menu_music_handler"))]
    pub const MENU_MUSIC_HANDLER: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x00504e27, function_type: PhantomData};
}

// ZTGoalAvoid class functions
pub mod ztgoalavoid {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalavoid/ztgoal_avoid_0"))]
    pub const ZTGOAL_AVOID_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0041226c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalavoid/exit_tank"))]
    pub const EXIT_TANK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00431834, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalavoid/check_done"))]
    pub const CHECK_DONE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a5471, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalavoid/decide"))]
    pub const DECIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a557f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalavoid/ztgoal_avoid_1"))]
    pub const ZTGOAL_AVOID_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00604185, function_type: PhantomData};
}

// ZTGoalBagDoo class functions
pub mod ztgoalbagdoo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalbagdoo/complete"))]
    pub const COMPLETE: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x004a2c98, function_type: PhantomData};
}

// ZTGoalBuildingExit class functions
pub mod ztgoalbuildingexit {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalbuildingexit/complete"))]
    pub const COMPLETE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042dddf, function_type: PhantomData};
}

// ZTGoalChase class functions
pub mod ztgoalchase {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalchase/get_type"))]
    pub const GET_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a4c06, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalchase/check_done"))]
    pub const CHECK_DONE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a4c0c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalchase/decide"))]
    pub const DECIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a4d6c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalchase/ztgoal_chase"))]
    pub const ZTGOAL_CHASE: FunctionDef<unsafe extern "thiscall" fn(u32, u8)> = FunctionDef{address: 0x004a4d98, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalchase/complete"))]
    pub const COMPLETE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a4dd2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalchase/f_caught"))]
    pub const F_CAUGHT: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004a6043, function_type: PhantomData};
}

// ZTGoalChaseAnimal class functions
pub mod ztgoalchaseanimal {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalchaseanimal/complete"))]
    pub const COMPLETE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0050db25, function_type: PhantomData};
}

// ZTGoalEmptyTrash class functions
pub mod ztgoalemptytrash {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalemptytrash/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0048b3f2, function_type: PhantomData};
}

// ZTGoalFactory class functions
pub mod ztgoalfactory {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalfactory/create_sub"))]
    pub const CREATE_SUB: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00413aa7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalfactory/create"))]
    pub const CREATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00413bec, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalfactory/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00603e6b, function_type: PhantomData};
}

// ZTGoalFixFilter class functions
pub mod ztgoalfixfilter {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalfixfilter/fix_filter"))]
    pub const FIX_FILTER: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00595cde, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalfixfilter/complete"))]
    pub const COMPLETE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00595d17, function_type: PhantomData};
}

// ZTGoalGawk class functions
pub mod ztgoalgawk {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalgawk/complete"))]
    pub const COMPLETE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042e9f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalgawk/left_va"))]
    pub const LEFT_VA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042ea14, function_type: PhantomData};
}

// ZTGoalHeliReturn class functions
pub mod ztgoalhelireturn {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalhelireturn/check_done"))]
    pub const CHECK_DONE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a4556, function_type: PhantomData};
}

// ZTGoalLeave class functions
pub mod ztgoalleave {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalleave/decide"))]
    pub const DECIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004233ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalleave/find_exit"))]
    pub const FIND_EXIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004a6889, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalleave/complete"))]
    pub const COMPLETE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fa85d, function_type: PhantomData};
}

// ZTGoalPreattack class functions
pub mod ztgoalpreattack {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalpreattack/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0061b2d6, function_type: PhantomData};
}

// ZTGoalSite class functions
pub mod ztgoalsite {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalsite/decide"))]
    pub const DECIDE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004281be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalsite/set_site_goal"))]
    pub const SET_SITE_GOAL: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004282e1, function_type: PhantomData};
}

// ZTGoalWaterDrown class functions
pub mod ztgoalwaterdrown {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalwaterdrown/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0061a705, function_type: PhantomData};
}

// ZTGoalZooDoo class functions
pub mod ztgoalzoodoo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztgoalzoodoo/get_type"))]
    pub const GET_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a2746, function_type: PhantomData};
}

// ZTGuest class functions
pub mod ztguest {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/seen_habitat"))]
    pub const SEEN_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0042281c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/f_guest_thought"))]
    pub const F_GUEST_THOUGHT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x004231c1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/get_nonseen_habitats"))]
    pub const GET_NONSEEN_HABITATS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00427b08, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/get_num_nonseen_sites"))]
    pub const GET_NUM_NONSEEN_SITES: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00427c88, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/get_nonseen_attractions"))]
    pub const GET_NONSEEN_ATTRACTIONS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00427d5c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/get_num_nonseen_gawk_scenery"))]
    pub const GET_NUM_NONSEEN_GAWK_SCENERY: FunctionDef<unsafe extern "thiscall" fn(u32, bool, bool) -> u32> = FunctionDef{address: 0x00428039, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/seen_attraction"))]
    pub const SEEN_ATTRACTION: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004285e2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/pick_gawk_scenery"))]
    pub const PICK_GAWK_SCENERY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0042945f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/get_nonseen_gawk_scenery"))]
    pub const GET_NONSEEN_GAWK_SCENERY: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0042956e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/consume_item"))]
    pub const CONSUME_ITEM: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0042d380, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/remove_item"))]
    pub const REMOVE_ITEM: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> bool> = FunctionDef{address: 0x0042d533, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/add_item"))]
    pub const ADD_ITEM: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i32) -> u32> = FunctionDef{address: 0x0042d54e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/get_need_item"))]
    pub const GET_NEED_ITEM: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0042d74e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/check_predator"))]
    pub const CHECK_PREDATOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004302ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x00434029, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/get_color_rep_info"))]
    pub const GET_COLOR_REP_INFO: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0043408c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/listen"))]
    pub const LISTEN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043a698, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/update_goals"))]
    pub const UPDATE_GOALS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043a6e4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/check_leave_zoo"))]
    pub const CHECK_LEAVE_ZOO: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004a6acc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004f5fd5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/init_status_vars"))]
    pub const INIT_STATUS_VARS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f6372, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/create_name"))]
    pub const CREATE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f70e4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/ztguest"))]
    pub const ZTGUEST: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004fa44c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/do_angry_price_change"))]
    pub const DO_ANGRY_PRICE_CHANGE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00533480, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/add_trash"))]
    pub const ADD_TRASH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0059d2c9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/seen_show"))]
    pub const SEEN_SHOW: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x005a2bcd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguest/do_happy_price_change"))]
    pub const DO_HAPPY_PRICE_CHANGE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00619245, function_type: PhantomData};
}

// ZTGuestType class functions
pub mod ztguesttype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztguesttype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c47dd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguesttype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004c4cb9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguesttype/ztguest_type"))]
    pub const ZTGUEST_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004cbb9c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguesttype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f6060, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguesttype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x005213de, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguesttype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x005216f1, function_type: PhantomData};
}

// ZTGuide class functions
pub mod ztguide {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztguide/get_random_assigned_habitat"))]
    pub const GET_RANDOM_ASSIGNED_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0058af73, function_type: PhantomData};
}

// ZTGuideType class functions
pub mod ztguidetype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztguidetype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004c3ee2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguidetype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004c406f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguidetype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051c2d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguidetype/ztguide_type_0"))]
    pub const ZTGUIDE_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0057cb67, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztguidetype/ztguide_type_1"))]
    pub const ZTGUIDE_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057cb85, function_type: PhantomData};
}

// ZTHabitat class functions
pub mod zthabitat {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/recalculate_viewing_areas"))]
    pub const RECALCULATE_VIEWING_AREAS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040f55a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/set_dirty_characteristics"))]
    pub const SET_DIRTY_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040f5a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/is_show_tank"))]
    pub const IS_SHOW_TANK: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0040fba2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_show_info_id"))]
    pub const GET_SHOW_INFO_ID: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0040fbc7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_gate_tile_in"))]
    pub const GET_GATE_TILE_IN: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00410349, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_all_animals"))]
    pub const GET_ALL_ANIMALS: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> u32> = FunctionDef{address: 0x00410def, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_gate_tile_out"))]
    pub const GET_GATE_TILE_OUT: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00411285, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_num_animals"))]
    pub const GET_NUM_ANIMALS: FunctionDef<unsafe extern "thiscall" fn(u32, bool) -> i32> = FunctionDef{address: 0x00412167, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_habitat_rating"))]
    pub const GET_HABITAT_RATING: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i8) -> f32> = FunctionDef{address: 0x00415dd7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/clear_amphibious_neighbors"))]
    pub const CLEAR_AMPHIBIOUS_NEIGHBORS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00417460, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/set_is_not_show_exhibit"))]
    pub const SET_IS_NOT_SHOW_EXHIBIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041748d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/accept_donation"))]
    pub const ACCEPT_DONATION: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0042ec49, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/update_portals"))]
    pub const UPDATE_PORTALS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0043578f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004357c6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_size"))]
    pub const GET_SIZE: FunctionDef<unsafe extern "thiscall" fn(u32, bool) -> i32> = FunctionDef{address: 0x0044097c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/recalculate_characteristics"))]
    pub const RECALCULATE_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00444827, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/revise_species_list"))]
    pub const REVISE_SPECIES_LIST: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00447dc1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/add_species"))]
    pub const ADD_SPECIES: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00448f6b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/resize"))]
    pub const RESIZE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0044b900, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/create_edge_pairs"))]
    pub const CREATE_EDGE_PAIRS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044b95c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/create_viewing_areas"))]
    pub const CREATE_VIEWING_AREAS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044ba57, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool) -> u32> = FunctionDef{address: 0x0044dc0d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/create_name"))]
    pub const CREATE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044df34, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/path_placed"))]
    pub const PATH_PLACED: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0044ea19, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/add_viewing_area"))]
    pub const ADD_VIEWING_AREA: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0044efa4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/add_show_unit"))]
    pub const ADD_SHOW_UNIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00458610, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/remove_show_unit"))]
    pub const REMOVE_SHOW_UNIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004586b9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/clear_show_neighbors"))]
    pub const CLEAR_SHOW_NEIGHBORS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00458a88, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/move_gate_to"))]
    pub const MOVE_GATE_TO: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00466236, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/has_keeper_assigned"))]
    pub const HAS_KEEPER_ASSIGNED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00468454, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_popularity"))]
    pub const GET_POPULARITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00468732, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00479b73, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_undesirable_scenery"))]
    pub const GET_UNDESIRABLE_SCENERY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32)> = FunctionDef{address: 0x00483a67, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_gate"))]
    pub const GET_GATE: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00492ca3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_species_rating"))]
    pub const GET_SPECIES_RATING: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32) -> f32> = FunctionDef{address: 0x004d92db, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_surrounding_species"))]
    pub const GET_SURROUNDING_SPECIES: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004fb11c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/remove_species"))]
    pub const REMOVE_SPECIES: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004fbe4f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/add_amphibious_neighbor"))]
    pub const ADD_AMPHIBIOUS_NEIGHBOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005078d3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/remove_amphibious_neighbor"))]
    pub const REMOVE_AMPHIBIOUS_NEIGHBOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00507a90, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/remove_from_all_vas"))]
    pub const REMOVE_FROM_ALL_VAS: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0050bb53, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/remove_viewing_area"))]
    pub const REMOVE_VIEWING_AREA: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0050bc94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/get_show_portal"))]
    pub const GET_SHOW_PORTAL: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x0059e0a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/remove_show_portal"))]
    pub const REMOVE_SHOW_PORTAL: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005aa4d9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/remove_show_neighbor"))]
    pub const REMOVE_SHOW_NEIGHBOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x005aa75e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/add_show_neighbor"))]
    pub const ADD_SHOW_NEIGHBOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x005ab1e7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/add_show_portal"))]
    pub const ADD_SHOW_PORTAL: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x005ab354, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/set_is_show_exhibit"))]
    pub const SET_IS_SHOW_EXHIBIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ab661, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/set_name"))]
    pub const SET_NAME: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005aedbd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/recreate_oas"))]
    pub const RECREATE_OAS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b2ffb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/trigger_death_arrived"))]
    pub const TRIGGER_DEATH_ARRIVED: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00605b8e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/hilite_show_neighbors"))]
    pub const HILITE_SHOW_NEIGHBORS: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x00605c02, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitat/hilite_amphibious_neighbors"))]
    pub const HILITE_AMPHIBIOUS_NEIGHBORS: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x00605c80, function_type: PhantomData};
}

// ZTHabitatMgr class functions
pub mod zthabitatmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/get_habitat"))]
    pub const GET_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32) -> u32> = FunctionDef{address: 0x00410bf9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/get_zoo_entrance_tile"))]
    pub const GET_ZOO_ENTRANCE_TILE: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00410d04, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/scenery_entity_change"))]
    pub const SCENERY_ENTITY_CHANGE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00417295, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/replace_gate"))]
    pub const REPLACE_GATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041ec94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/get_nonempty_non_world_habitats"))]
    pub const GET_NONEMPTY_NON_WORLD_HABITATS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00440d9a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/recalculate_deterioration"))]
    pub const RECALCULATE_DETERIORATION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00449920, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/add_habitat"))]
    pub const ADD_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0044c7b2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/do_tank_check"))]
    pub const DO_TANK_CHECK: FunctionDef<unsafe extern "stdcall" fn(i32) -> u32> = FunctionDef{address: 0x0044c8ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/morph_exhibit"))]
    pub const MORPH_EXHIBIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x0044ca8c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/fence_placed"))]
    pub const FENCE_PLACED: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0044ce67, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/can_find_path"))]
    pub const CAN_FIND_PATH: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u8, u8) -> u32> = FunctionDef{address: 0x0044d598, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/clear_pathfinding"))]
    pub const CLEAR_PATHFINDING: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044dab9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/do_show_check"))]
    pub const DO_SHOW_CHECK: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x0044e1e8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/create_habitat"))]
    pub const CREATE_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32)> = FunctionDef{address: 0x0044e389, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/name_habitat"))]
    pub const NAME_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0044e43e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/place_gate"))]
    pub const PLACE_GATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0044e4eb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/get_next_num"))]
    pub const GET_NEXT_NUM: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0044e9f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/decrement_habitat_num"))]
    pub const DECREMENT_HABITAT_NUM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045bbf6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/snap_tank_walls_inward"))]
    pub const SNAP_TANK_WALLS_INWARD: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0045c80a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/check_exhibit_morph"))]
    pub const CHECK_EXHIBIT_MORPH: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0045ce9a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/update_amphibious_neighbors_0"))]
    pub const UPDATE_AMPHIBIOUS_NEIGHBORS_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x0045cf03, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/update_show_neighbors_0"))]
    pub const UPDATE_SHOW_NEIGHBORS_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x0045cf65, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/instantiate"))]
    pub const INSTANTIATE: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x004635aa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/replace_fence_with_gate"))]
    pub const REPLACE_FENCE_WITH_GATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004666c5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00479a54, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/enter_new_month"))]
    pub const ENTER_NEW_MONTH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0048430c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/merge_tanks"))]
    pub const MERGE_TANKS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x0048712c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/get_merge_tank_walls"))]
    pub const GET_MERGE_TANK_WALLS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x00487498, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/split_tank"))]
    pub const SPLIT_TANK: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004877bf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/get_next_fence_pair"))]
    pub const GET_NEXT_FENCE_PAIR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, bool) -> u32> = FunctionDef{address: 0x00487f34, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6b2c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004c7e6c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/before_entity_change"))]
    pub const BEFORE_ENTITY_CHANGE: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x004d8a4d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/entity_about_to_be_placed"))]
    pub const ENTITY_ABOUT_TO_BE_PLACED: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004d8cd2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/terrain_about_to_be_changed"))]
    pub const TERRAIN_ABOUT_TO_BE_CHANGED: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i32)> = FunctionDef{address: 0x004db967, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/terrain_changed"))]
    pub const TERRAIN_CHANGED: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004dbc79, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/zthabitat_mgr"))]
    pub const ZTHABITAT_MGR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0050363d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/entity_about_to_be_removed"))]
    pub const ENTITY_ABOUT_TO_BE_REMOVED: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0050a5db, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/replace_gate_with_fence"))]
    pub const REPLACE_GATE_WITH_FENCE: FunctionDef<unsafe extern "stdcall" fn(u32) -> bool> = FunctionDef{address: 0x0050b0c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/path_removed"))]
    pub const PATH_REMOVED: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0050bae5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/fence_removed"))]
    pub const FENCE_REMOVED: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0050c3d8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/remove_habitat_0"))]
    pub const REMOVE_HABITAT_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0050c515, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/clear_staff_habitat"))]
    pub const CLEAR_STAFF_HABITAT: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x0050c6b8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/empty_grandstands"))]
    pub const EMPTY_GRANDSTANDS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0050c813, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/create_world_habitat"))]
    pub const CREATE_WORLD_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32, i32)> = FunctionDef{address: 0x0058e607, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/mark_zoo_exterior"))]
    pub const MARK_ZOO_EXTERIOR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00594729, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/fill_zoo_exterior"))]
    pub const FILL_ZOO_EXTERIOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00594848, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/can_see_habitat_from_building"))]
    pub const CAN_SEE_HABITAT_FROM_BUILDING: FunctionDef<unsafe extern "cdecl" fn(u32, i32) -> u32> = FunctionDef{address: 0x0059e349, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/habitat_seen_from_building"))]
    pub const HABITAT_SEEN_FROM_BUILDING: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x005a2b61, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/check_show_neighbor"))]
    pub const CHECK_SHOW_NEIGHBOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x005aaed2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/remove_all_habitats"))]
    pub const REMOVE_ALL_HABITATS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b11d2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/update_show_neighbors_1"))]
    pub const UPDATE_SHOW_NEIGHBORS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005b2d3a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/update_amphibious_neighbors_1"))]
    pub const UPDATE_AMPHIBIOUS_NEIGHBORS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005b2e8a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/check_amphibious_neighbor"))]
    pub const CHECK_AMPHIBIOUS_NEIGHBOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x005b2fa1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/habitat_tile_changed"))]
    pub const HABITAT_TILE_CHANGED: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005b3a77, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/remove_habitat_1"))]
    pub const REMOVE_HABITAT_1: FunctionDef<unsafe extern "stdcall" fn(i32, i32)> = FunctionDef{address: 0x005b66ea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/split_tank_into_land"))]
    pub const SPLIT_TANK_INTO_LAND: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x006044a5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthabitatmgr/terrain_tile_changed"))]
    pub const TERRAIN_TILE_CHANGED: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0060497a, function_type: PhantomData};
}

// ZTHelicopter class functions
pub mod zthelicopter {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicopter/init_after_load"))]
    pub const INIT_AFTER_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u32)> = FunctionDef{address: 0x00482cf1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicopter/get_next_pos"))]
    pub const GET_NEXT_POS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0048c0c6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicopter/start_loop_sound"))]
    pub const START_LOOP_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0048d645, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicopter/validate_tank_position"))]
    pub const VALIDATE_TANK_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005067be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicopter/set_dest_tile"))]
    pub const SET_DEST_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0061a117, function_type: PhantomData};
}

// ZTHelicopterType class functions
pub mod zthelicoptertype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicoptertype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004c42ff, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicoptertype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051c7b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicoptertype/zthelicopter_type_0"))]
    pub const ZTHELICOPTER_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0057cb90, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zthelicoptertype/zthelicopter_type_1"))]
    pub const ZTHELICOPTER_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057cbae, function_type: PhantomData};
}

// ZTItem class functions
pub mod ztitem {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztitem/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00464b90, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztitem/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00464f00, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztitem/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004651cf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztitem/get_item"))]
    pub const GET_ITEM: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x004a84f3, function_type: PhantomData};
}

// ZTKeeper class functions
pub mod ztkeeper {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztkeeper/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00458759, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztkeeper/notify_habitat_not_coming"))]
    pub const NOTIFY_HABITAT_NOT_COMING: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a879f, function_type: PhantomData};
}

// ZTKeeperType class functions
pub mod ztkeepertype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztkeepertype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c32bd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztkeepertype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004c34ae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztkeepertype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051bde7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztkeepertype/ztkeeper_type_0"))]
    pub const ZTKEEPER_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057ca95, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztkeepertype/ztkeeper_type_1"))]
    pub const ZTKEEPER_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0057cae0, function_type: PhantomData};
}

// ZTMVTempEntityList class functions
pub mod ztmvtempentitylist {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztmvtempentitylist/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00443e12, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmvtempentitylist/add_entity"))]
    pub const ADD_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004df44e, function_type: PhantomData};
}

// ZTMaint class functions
pub mod ztmaint {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztmaint/get_terrain_cost"))]
    pub const GET_TERRAIN_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00439c73, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmaint/finish_task"))]
    pub const FINISH_TASK: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x004a97e5, function_type: PhantomData};
}

// ZTMaintTaskPool class functions
pub mod ztmainttaskpool {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztmainttaskpool/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b11a2, function_type: PhantomData};
}

// ZTMaintType class functions
pub mod ztmainttype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztmainttype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c3bb3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmainttype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004c3d44, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmainttype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051bf9e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmainttype/ztmaint_type_0"))]
    pub const ZTMAINT_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057cafe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmainttype/ztmaint_type_1"))]
    pub const ZTMAINT_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0057cb49, function_type: PhantomData};
}

// ZTMapView class functions
pub mod ztmapview {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/clear_conflicting_entities"))]
    pub const CLEAR_CONFLICTING_ENTITIES: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0040248a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_highlighted_entity"))]
    pub const SET_HIGHLIGHTED_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0040fcdb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/scroll_map"))]
    pub const SCROLL_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, i8)> = FunctionDef{address: 0x00418da5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x00419729, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00419eb2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/prepare_to_delete_entity"))]
    pub const PREPARE_TO_DELETE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0041e145, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/use_cursor_0"))]
    pub const USE_CURSOR_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0043231a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/render_0"))]
    pub const RENDER_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00432bd7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/show_current_cost"))]
    pub const SHOW_CURRENT_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00433918, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/check_mouse_over_entity"))]
    pub const CHECK_MOUSE_OVER_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00433b34, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/use_cursor_1"))]
    pub const USE_CURSOR_1: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x00443d97, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/highlight_map"))]
    pub const HIGHLIGHT_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u8)> = FunctionDef{address: 0x00443daf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/cancel_entity_move"))]
    pub const CANCEL_ENTITY_MOVE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00443df7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_map_selection_size"))]
    pub const SET_MAP_SELECTION_SIZE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00449739, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/has_undo_action"))]
    pub const HAS_UNDO_ACTION: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32) -> u32> = FunctionDef{address: 0x00458b5f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/get_habitat"))]
    pub const GET_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> i32> = FunctionDef{address: 0x00468a7d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/pick_up_selected_entity"))]
    pub const PICK_UP_SELECTED_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0046ab75, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/handle_char"))]
    pub const HANDLE_CHAR: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0046bd7d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/save_state"))]
    pub const SAVE_STATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047abeb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/place_entity_on_map_0"))]
    pub const PLACE_ENTITY_ON_MAP_0: FunctionDef<unsafe extern "stdcall" fn(u8, u8, u8, u8, u8, u32, u8, u8, u8, u32, u32, u8) -> u32> = FunctionDef{address: 0x004868cf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/follow_selected_entity"))]
    pub const FOLLOW_SELECTED_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0048c5e9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/remove_selected_entity"))]
    pub const REMOVE_SELECTED_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x0048d065, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/do_snake"))]
    pub const DO_SNAKE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x0048e56c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_map_view"))]
    pub const SET_MAP_VIEW: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004b0384, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_map_selection"))]
    pub const SET_MAP_SELECTION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b05ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/rotate_map"))]
    pub const ROTATE_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004b06be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/zoom_map"))]
    pub const ZOOM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004b072d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_selected_entity_0"))]
    pub const SET_SELECTED_ENTITY_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool)> = FunctionDef{address: 0x004b2322, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/check_zoom_options"))]
    pub const CHECK_ZOOM_OPTIONS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004b2868, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/destroy_cursors"))]
    pub const DESTROY_CURSORS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004b2c84, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6c4c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004d3f9e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/init_cursors"))]
    pub const INIT_CURSORS: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x004d40c4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/handle_left_button_down"))]
    pub const HANDLE_LEFT_BUTTON_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32) -> u32> = FunctionDef{address: 0x004d83d7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/handle_left_button_up"))]
    pub const HANDLE_LEFT_BUTTON_UP: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32) -> u32> = FunctionDef{address: 0x004d8533, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/place_entity_on_map_1"))]
    pub const PLACE_ENTITY_ON_MAP_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, f32, i32) -> u32> = FunctionDef{address: 0x004d9088, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/commit_terraform"))]
    pub const COMMIT_TERRAFORM: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x004da19c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/undo_last_action_0"))]
    pub const UNDO_LAST_ACTION_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004de527, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_entity_position"))]
    pub const SET_ENTITY_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, u32, u32)> = FunctionDef{address: 0x004ded90, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/check_tank_placement"))]
    pub const CHECK_TANK_PLACEMENT: FunctionDef<unsafe extern "stdcall" fn(u32, u32, *mut u32) -> bool> = FunctionDef{address: 0x004df688, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/render_1"))]
    pub const RENDER_1: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x004df7e8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_place_gate_mode"))]
    pub const SET_PLACE_GATE_MODE: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x004e1064, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/fill_canvases"))]
    pub const FILL_CANVASES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ec8bf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/handle_key_up"))]
    pub const HANDLE_KEY_UP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f0b1e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/handle_key_down"))]
    pub const HANDLE_KEY_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f0ee4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/clear_undo_actions"))]
    pub const CLEAR_UNDO_ACTIONS: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004f16f0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_temp_entity"))]
    pub const SET_TEMP_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, bool, bool, bool)> = FunctionDef{address: 0x004f189a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/remove_entity_from_map_0"))]
    pub const REMOVE_ENTITY_FROM_MAP_0: FunctionDef<unsafe extern "stdcall" fn(i32, u32, u32, u32, i32, u32, i32, u32, u32, i8, u32, u8, f32, i8, u32, i8)> = FunctionDef{address: 0x004f94be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/handle_right_button_down"))]
    pub const HANDLE_RIGHT_BUTTON_DOWN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f95e4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/cancel_current_operation"))]
    pub const CANCEL_CURRENT_OPERATION: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004f9621, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/handle_right_button_up"))]
    pub const HANDLE_RIGHT_BUTTON_UP: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f96fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/check_undo_buffer"))]
    pub const CHECK_UNDO_BUFFER: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004fa68c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/render_current_cost"))]
    pub const RENDER_CURRENT_COST: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fb175, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/cancel_drag"))]
    pub const CANCEL_DRAG: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004ffa6b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/ztmap_view_0"))]
    pub const ZTMAP_VIEW_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00502431, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/ztmap_view_1"))]
    pub const ZTMAP_VIEW_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x005026cf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_bulldozer"))]
    pub const SET_BULLDOZER: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x005096e8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/remove_entity_from_map_1"))]
    pub const REMOVE_ENTITY_FROM_MAP_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32, f32, i8, u32, i8)> = FunctionDef{address: 0x0050a0b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/remove_entity"))]
    pub const REMOVE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x0050a432, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/add_habitat_name_undo_actions"))]
    pub const ADD_HABITAT_NAME_UNDO_ACTIONS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0050ae37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_map"))]
    pub const SET_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00519ba0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0051f80c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/create"))]
    pub const CREATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005205bc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/take_snapshot"))]
    pub const TAKE_SNAPSHOT: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x00532c75, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/load_state"))]
    pub const LOAD_STATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00592907, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_view_center"))]
    pub const SET_VIEW_CENTER: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32)> = FunctionDef{address: 0x0059b90c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/add_undo_action"))]
    pub const ADD_UNDO_ACTION: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, u32, u32, u32, u32, u32, u32, u32, u32, u32, i32, i32)> = FunctionDef{address: 0x005b32de, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/set_selected_entity_1"))]
    pub const SET_SELECTED_ENTITY_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005b669e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/undo_last_action_1"))]
    pub const UNDO_LAST_ACTION_1: FunctionDef<unsafe extern "cdecl" fn(u32, u8, u8, u8, u8, u8, u8, u8, u32)> = FunctionDef{address: 0x005b6710, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/cancel_terraform"))]
    pub const CANCEL_TERRAFORM: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x006097ef, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/draw_back_cube"))]
    pub const DRAW_BACK_CUBE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00609ae0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/draw_front_cube"))]
    pub const DRAW_FRONT_CUBE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00609ee1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/draw_left_cube"))]
    pub const DRAW_LEFT_CUBE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0060a2e2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmapview/draw_right_cube"))]
    pub const DRAW_RIGHT_CUBE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0060a5da, function_type: PhantomData};
}

// ZTMegatileMgr class functions
pub mod ztmegatilemgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztmegatilemgr/recalculate_characteristics"))]
    pub const RECALCULATE_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041ffdc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmegatilemgr/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0043525b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztmegatilemgr/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0058e82d, function_type: PhantomData};
}

// ZTMiniMap class functions
pub mod ztminimap {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/rotate_xy"))]
    pub const ROTATE_XY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32)> = FunctionDef{address: 0x0044458a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004445ca, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/get_tile_color"))]
    pub const GET_TILE_COLOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0048ff22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/set_center"))]
    pub const SET_CENTER: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32)> = FunctionDef{address: 0x004aff88, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/ztmini_map"))]
    pub const ZTMINI_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00503494, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/set_dimensions"))]
    pub const SET_DIMENSIONS: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0058e416, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00619477, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztminimap/copy"))]
    pub const COPY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x006198d7, function_type: PhantomData};
}

// ZTObservableArea class functions
pub mod ztobservablearea {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztobservablearea/recalculate_characteristics"))]
    pub const RECALCULATE_CHARACTERISTICS: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x00447a00, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztobservablearea/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0044ef63, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztobservablearea/add_tile"))]
    pub const ADD_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004f3b00, function_type: PhantomData};
}

// ZTPath class functions
pub mod ztpath {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztpath/validate_position"))]
    pub const VALIDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0044a261, function_type: PhantomData};
}

// ZTPathType class functions
pub mod ztpathtype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztpathtype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00465a0a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztpathtype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00465c62, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztpathtype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c24cf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztpathtype/ztpath_type_0"))]
    pub const ZTPATH_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00504c2f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztpathtype/ztpath_type_1"))]
    pub const ZTPATH_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00504ce0, function_type: PhantomData};
}

// ZTResearchBranch class functions
pub mod ztresearchbranch {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchbranch/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0041f1ba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchbranch/pick_random_program"))]
    pub const PICK_RANDOM_PROGRAM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b09f6, function_type: PhantomData};
}

// ZTResearchCategory class functions
pub mod ztresearchcategory {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchcategory/load_category"))]
    pub const LOAD_CATEGORY: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00590b76, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchcategory/clear_category"))]
    pub const CLEAR_CATEGORY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b0d44, function_type: PhantomData};
}

// ZTResearchMgr class functions
pub mod ztresearchmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> i32> = FunctionDef{address: 0x00435a6f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0047a923, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/force_research"))]
    pub const FORCE_RESEARCH: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0047e657, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/ztresearch_mgr"))]
    pub const ZTRESEARCH_MGR: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00504d37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00593c3c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/get_program"))]
    pub const GET_PROGRAM: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x005944b6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/get_category"))]
    pub const GET_CATEGORY: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x00594595, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/get_branch"))]
    pub const GET_BRANCH: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0059466e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchmgr/set_effect_discount"))]
    pub const SET_EFFECT_DISCOUNT: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32) -> u32> = FunctionDef{address: 0x0059b71d, function_type: PhantomData};
}

// ZTResearchProgram class functions
pub mod ztresearchprogram {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchprogram/ztresearch_program"))]
    pub const ZTRESEARCH_PROGRAM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00501a3e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchprogram/on_completion"))]
    pub const ON_COMPLETION: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0058fb83, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchprogram/reset"))]
    pub const RESET: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005900d8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchprogram/load_program"))]
    pub const LOAD_PROGRAM: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00590e00, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztresearchprogram/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0059109b, function_type: PhantomData};
}

// ZTRubble class functions
pub mod ztrubble {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztrubble/play_explosion_sound"))]
    pub const PLAY_EXPLOSION_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0047c863, function_type: PhantomData};
}

// ZTRubbleType class functions
pub mod ztrubbletype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztrubbletype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004c42a2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztrubbletype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051ce38, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztrubbletype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051d2cb, function_type: PhantomData};
}

// ZTScenarioMgr class functions
pub mod ztscenariomgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariomgr/end_scenario"))]
    pub const END_SCENARIO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0048a408, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariomgr/begin_scenario"))]
    pub const BEGIN_SCENARIO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c998b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariomgr/common_setup"))]
    pub const COMMON_SETUP: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0058ed1f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariomgr/set_cash"))]
    pub const SET_CASH: FunctionDef<unsafe extern "stdcall" fn(f32)> = FunctionDef{address: 0x005b0f17, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariomgr/add_cash"))]
    pub const ADD_CASH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0060ccc2, function_type: PhantomData};
}

// ZTScenarioSimpleGoal class functions
pub mod ztscenariosimplegoal {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/eval"))]
    pub const EVAL: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0041d665, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/eval00"))]
    pub const EVAL00: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x0041da81, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger10"))]
    pub const TRIGGER10: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0047e76a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger"))]
    pub const TRIGGER: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x004fc783, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/do_dialog"))]
    pub const DO_DIALOG: FunctionDef<unsafe extern "thiscall" fn(u32, i32, bool)> = FunctionDef{address: 0x004fc812, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger02"))]
    pub const TRIGGER02: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a0a10, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger04"))]
    pub const TRIGGER04: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a0fc2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger07"))]
    pub const TRIGGER07: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a1139, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger03"))]
    pub const TRIGGER03: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a13c5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger06"))]
    pub const TRIGGER06: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a154a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger05"))]
    pub const TRIGGER05: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x005a17a8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/eval11"))]
    pub const EVAL11: FunctionDef<unsafe extern "fastcall" fn(i32) -> u32> = FunctionDef{address: 0x0061bb06, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger01"))]
    pub const TRIGGER01: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0061c139, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger08"))]
    pub const TRIGGER08: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0061c27d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariosimplegoal/trigger09"))]
    pub const TRIGGER09: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0061c408, function_type: PhantomData};
}

// ZTScenarioTimer class functions
pub mod ztscenariotimer {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/get_state"))]
    pub const GET_STATE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0040f121, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/size"))]
    pub const SIZE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004113d7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0042425d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0047a04c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/display_goal"))]
    pub const DISPLAY_GOAL: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004f042c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/get_text"))]
    pub const GET_TEXT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f0484, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/get_icon_name"))]
    pub const GET_ICON_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f0488, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0058e218, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i32)> = FunctionDef{address: 0x005950ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/ztscenario_timer"))]
    pub const ZTSCENARIO_TIMER: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x005b130f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenariotimer/load_state"))]
    pub const LOAD_STATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0061c430, function_type: PhantomData};
}

// ZTScenery class functions
pub mod ztscenery {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenery/force_idle_anim"))]
    pub const FORCE_IDLE_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00410540, function_type: PhantomData};
}

// ZTSceneryType class functions
pub mod ztscenerytype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_purchase_cost"))]
    pub const GET_PURCHASE_COST: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040fbb6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/check_dead_state"))]
    pub const CHECK_DEAD_STATE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0041038b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_tank"))]
    pub const GET_TANK: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00410460, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/is_snap_to_ground"))]
    pub const IS_SNAP_TO_GROUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004104ca, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/validate_position"))]
    pub const VALIDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004104cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/ztscenery_0"))]
    pub const ZTSCENERY_0: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0041294b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/init_status_vars"))]
    pub const INIT_STATUS_VARS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00412972, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00412977, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u32)> = FunctionDef{address: 0x00412999, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/ztscenery_1"))]
    pub const ZTSCENERY_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0041e075, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_sell_price"))]
    pub const GET_SELL_PRICE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041ec7f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_esthetic_bonus"))]
    pub const GET_ESTHETIC_BONUS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041f05c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/create_name"))]
    pub const CREATE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00427340, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x0043306b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00434a75, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/set_name"))]
    pub const SET_NAME: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00449515, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/force_dead_anim"))]
    pub const FORCE_DEAD_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044a6ae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0044f625, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00477b03, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/turn_to_rubble"))]
    pub const TURN_TO_RUBBLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0047c2ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/morph"))]
    pub const MORPH: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x0048663a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_height"))]
    pub const GET_HEIGHT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004937a4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/is_zoo_gate"))]
    pub const IS_ZOO_GATE: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x004a4389, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004ba5b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004bf2d4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_help_id"))]
    pub const GET_HELP_ID: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004e9e89, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/add_to_map"))]
    pub const ADD_TO_MAP: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f40af, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/set_food_units"))]
    pub const SET_FOOD_UNITS: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004f429d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_only_swims"))]
    pub const GET_ONLY_SWIMS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f84b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_swims"))]
    pub const GET_SWIMS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f84c2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/is_selectable"))]
    pub const IS_SELECTABLE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fb59e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004fe778, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/ztscenery_type"))]
    pub const ZTSCENERY_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00501080, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x0051b091, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/init_after_load"))]
    pub const INIT_AFTER_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00593f72, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztscenerytype/get_habitat"))]
    pub const GET_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006067d1, function_type: PhantomData};
}

// ZTShow class functions
pub mod ztshow {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0046df38, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004cca2c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0059e773, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/clear_show_script_states"))]
    pub const CLEAR_SHOW_SCRIPT_STATES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0059f79e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/stop_0"))]
    pub const STOP_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a3db4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/stop_1"))]
    pub const STOP_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005a4057, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/gather_units"))]
    pub const GATHER_UNITS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005a4519, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/send_event_generic"))]
    pub const SEND_EVENT_GENERIC: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x005a6f37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/start"))]
    pub const START: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a85e9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/clear_unit_goals"))]
    pub const CLEAR_UNIT_GOALS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a8a93, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/reinit"))]
    pub const REINIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a8b25, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/ztshow"))]
    pub const ZTSHOW: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x00604d36, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00612c6e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00612c8c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshow/abort_show"))]
    pub const ABORT_SHOW: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00612d20, function_type: PhantomData};
}

// ZTShowInfo class functions
pub mod ztshowinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u8> = FunctionDef{address: 0x0046d779, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/update_from_load"))]
    pub const UPDATE_FROM_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00484ec8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/enter_new_month"))]
    pub const ENTER_NEW_MONTH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0048b57e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/set_show_frequency"))]
    pub const SET_SHOW_FREQUENCY: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004cc86a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004cc890, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/recalculate_schedule"))]
    pub const RECALCULATE_SCHEDULE: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004f2947, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/get_scheduled_show_script"))]
    pub const GET_SCHEDULED_SHOW_SCRIPT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0059dff6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/is_started"))]
    pub const IS_STARTED: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0059fa05, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/get_show_unit_list"))]
    pub const GET_SHOW_UNIT_LIST: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x005a20ae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/increment_receipts"))]
    pub const INCREMENT_RECEIPTS: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x005a95be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/increment_attendance"))]
    pub const INCREMENT_ATTENDANCE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005a9826, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/add_unit_to_list"))]
    pub const ADD_UNIT_TO_LIST: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x005a9a48, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/add_unit"))]
    pub const ADD_UNIT: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x005a9c81, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/remove_unit"))]
    pub const REMOVE_UNIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x005a9c96, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowinfo/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00610076, function_type: PhantomData};
}

// ZTShowMgr class functions
pub mod ztshowmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowmgr/get_show_info"))]
    pub const GET_SHOW_INFO: FunctionDef<unsafe extern "thiscall" fn(u32, u16) -> u32> = FunctionDef{address: 0x0041ebfd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00479fa4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowmgr/enter_new_month"))]
    pub const ENTER_NEW_MONTH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004842a2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowmgr/init_show_params"))]
    pub const INIT_SHOW_PARAMS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0051f59b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowmgr/destroy"))]
    pub const DESTROY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0057db2e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowmgr/unregister_show"))]
    pub const UNREGISTER_SHOW: FunctionDef<unsafe extern "thiscall" fn(u32, u16, u32, bool) -> u32> = FunctionDef{address: 0x005aaa95, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowmgr/register_show"))]
    pub const REGISTER_SHOW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool) -> u32> = FunctionDef{address: 0x005abb26, function_type: PhantomData};
}

// ZTShowScript class functions
pub mod ztshowscript {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowscript/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0061c825, function_type: PhantomData};
}

// ZTShowScriptItem class functions
pub mod ztshowscriptitem {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowscriptitem/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004b9024, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowscriptitem/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004b9690, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowscriptitem/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0061c471, function_type: PhantomData};
}

// ZTShowScriptMgr class functions
pub mod ztshowscriptmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowscriptmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00479f44, function_type: PhantomData};
}

// ZTShowState class functions
pub mod ztshowstate {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowstate/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x0046de22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowstate/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004cc9c6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowstate/ztshow_state"))]
    pub const ZTSHOW_STATE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f28c3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztshowstate/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0059f72f, function_type: PhantomData};
}

// ZTSoundscape class functions
pub mod ztsoundscape {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztsoundscape/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004352dd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztsoundscape/ztsoundscape"))]
    pub const ZTSOUNDSCAPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005003e2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztsoundscape/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, u32)> = FunctionDef{address: 0x005922fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztsoundscape/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00592596, function_type: PhantomData};
}

// ZTStaff class functions
pub mod ztstaff {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztstaff/get_terrain_cost"))]
    pub const GET_TERRAIN_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i32> = FunctionDef{address: 0x00439c80, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstaff/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00457a0c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstaff/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004586f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstaff/ztstaff"))]
    pub const ZTSTAFF: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f9d09, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstaff/reassign_habitats"))]
    pub const REASSIGN_HABITATS: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00595237, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstaff/init_after_load"))]
    pub const INIT_AFTER_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u32) -> u32> = FunctionDef{address: 0x005952dc, function_type: PhantomData};
}

// ZTStaffType class functions
pub mod ztstafftype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztstafftype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004c3081, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstafftype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004c3783, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstafftype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x0051b581, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstafftype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051bd29, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstafftype/ztstaff_type"))]
    pub const ZTSTAFF_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057ca11, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztstafftype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x006112d9, function_type: PhantomData};
}

// ZTTankExhibit class functions
pub mod zttankexhibit {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/get_water_level"))]
    pub const GET_WATER_LEVEL: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004110fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/get_tank_height"))]
    pub const GET_TANK_HEIGHT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00411111, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/is_pure_water"))]
    pub const IS_PURE_WATER: FunctionDef<unsafe extern "thiscall" fn(u32) -> bool> = FunctionDef{address: 0x004111a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/is_extremely_murky_water"))]
    pub const IS_EXTREMELY_MURKY_WATER: FunctionDef<unsafe extern "thiscall" fn(u32) -> bool> = FunctionDef{address: 0x00411254, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_base_terrain_type"))]
    pub const SET_BASE_TERRAIN_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00458bea, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/remove_illegal_entities"))]
    pub const REMOVE_ILLEGAL_ENTITIES: FunctionDef<unsafe extern "thiscall" fn(u32, i32, bool) -> u32> = FunctionDef{address: 0x00458c2b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_base_level"))]
    pub const SET_BASE_LEVEL: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x00458c6a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/get_all_entities"))]
    pub const GET_ALL_ENTITIES: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004599a0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/update_tank_wall_info"))]
    pub const UPDATE_TANK_WALL_INFO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00459ab2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/update_adjustment_costs"))]
    pub const UPDATE_ADJUSTMENT_COSTS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00459b19, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/fill"))]
    pub const FILL: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00459d94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/check_entities_for_fill"))]
    pub const CHECK_ENTITIES_FOR_FILL: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00459e09, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/can_be_in_tank"))]
    pub const CAN_BE_IN_TANK: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x00459f52, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_is_not_show_exhibit"))]
    pub const SET_IS_NOT_SHOW_EXHIBIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045b7ec, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, bool, bool) -> u32> = FunctionDef{address: 0x0045b92f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/level_all_tiles"))]
    pub const LEVEL_ALL_TILES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045bbbb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/update_tank_info"))]
    pub const UPDATE_TANK_INFO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045bc0f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/is_illegal_entity"))]
    pub const IS_ILLEGAL_ENTITY: FunctionDef<unsafe extern "stdcall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0045da65, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/get_illegal_entities"))]
    pub const GET_ILLEGAL_ENTITIES: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32)> = FunctionDef{address: 0x0045da8b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/reclaim_tiles"))]
    pub const RECLAIM_TILES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00487a63, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_water_purity"))]
    pub const SET_WATER_PURITY: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0049340d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0049625f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/get_ladder_base_inside_pos"))]
    pub const GET_LADDER_BASE_INSIDE_POS: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0049b1cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/remove_owned_transients"))]
    pub const REMOVE_OWNED_TRANSIENTS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00505099, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/remove_all_sparkles"))]
    pub const REMOVE_ALL_SPARKLES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005050e2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/zttank_exhibit"))]
    pub const ZTTANK_EXHIBIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00505148, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_water_clean"))]
    pub const SET_WATER_CLEAN: FunctionDef<unsafe extern "fastcall" fn(u32)> = FunctionDef{address: 0x005052b1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/remove_all_sparkles_by_id"))]
    pub const REMOVE_ALL_SPARKLES_BY_ID: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005052bd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_is_show_exhibit"))]
    pub const SET_IS_SHOW_EXHIBIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ab683, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_water_level"))]
    pub const SET_WATER_LEVEL: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005ae565, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/remove_water_ripples"))]
    pub const REMOVE_WATER_RIPPLES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ae615, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/get_min_depth"))]
    pub const GET_MIN_DEPTH: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x005af226, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_tank_height"))]
    pub const SET_TANK_HEIGHT: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005af39e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/do_depth_suitability"))]
    pub const DO_DEPTH_SUITABILITY: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005af4b9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/add_water_ripples"))]
    pub const ADD_WATER_RIPPLES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005af78f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/add_water_ripple"))]
    pub const ADD_WATER_RIPPLE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x005af81a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00606398, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/set_salinity"))]
    pub const SET_SALINITY: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0060644b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/drain"))]
    pub const DRAIN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006064ed, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/check_entities_for_drain"))]
    pub const CHECK_ENTITIES_FOR_DRAIN: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x0060656e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankexhibit/restore_owned_transients"))]
    pub const RESTORE_OWNED_TRANSIENTS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x006066eb, function_type: PhantomData};
}

// ZTTankFilter class functions
pub mod zttankfilter {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/set_current_animation"))]
    pub const SET_CURRENT_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045d234, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/add_to_map"))]
    pub const ADD_TO_MAP: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0045d384, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u8> = FunctionDef{address: 0x0045d468, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00497e0f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/start_appropriate_sound"))]
    pub const START_APPROPRIATE_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00497f5c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/update_sound_volumes"))]
    pub const UPDATE_SOUND_VOLUMES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00497fb2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/start_decayed_sound"))]
    pub const START_DECAYED_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004980be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/stop_all_sounds"))]
    pub const STOP_ALL_SOUNDS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00498270, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/start_healthy_sound"))]
    pub const START_HEALTHY_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a7a38, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/remove_bubbles"))]
    pub const REMOVE_BUBBLES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004fb151, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0050558c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/set_healthy"))]
    pub const SET_HEALTHY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00595c8e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/service"))]
    pub const SERVICE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00595cad, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00615b31, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/set_broken"))]
    pub const SET_BROKEN: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00615c0f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfilter/decay"))]
    pub const DECAY: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00615c29, function_type: PhantomData};
}

// ZTTankFilterType class functions
pub mod zttankfiltertype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfiltertype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c44aa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfiltertype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051d8b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfiltertype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0051de26, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfiltertype/zttank_filter_type_0"))]
    pub const ZTTANK_FILTER_TYPE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0057d29e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankfiltertype/zttank_filter_type_1"))]
    pub const ZTTANK_FILTER_TYPE_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057d2bc, function_type: PhantomData};
}

// ZTTankWall class functions
pub mod zttankwall {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_images"))]
    pub const SET_IMAGES: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044a8b1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/update_mixer_facing"))]
    pub const UPDATE_MIXER_FACING: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0044aadb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_wall_height"))]
    pub const SET_WALL_HEIGHT: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0044ac6e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/validate_position"))]
    pub const VALIDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0044ac9c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_is_combined_connector"))]
    pub const SET_IS_COMBINED_CONNECTOR: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x00458b9f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_gate_mixers"))]
    pub const SET_GATE_MIXERS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00458bc4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_middle_fence"))]
    pub const SET_MIDDLE_FENCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00459fdf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_right_edge_fence"))]
    pub const SET_RIGHT_EDGE_FENCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045a409, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_left_edge_fence"))]
    pub const SET_LEFT_EDGE_FENCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045a833, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_both_edges_fence"))]
    pub const SET_BOTH_EDGES_FENCE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0045afed, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, i32) -> u32> = FunctionDef{address: 0x0045cd8d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/morph"))]
    pub const MORPH: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004868a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/draw"))]
    pub const DRAW: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x00492bd9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/draw_platform"))]
    pub const DRAW_PLATFORM: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32, u32)> = FunctionDef{address: 0x00495007, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_is_open_portal"))]
    pub const SET_IS_OPEN_PORTAL: FunctionDef<unsafe extern "thiscall" fn(u32, bool, bool)> = FunctionDef{address: 0x0059ea94, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/set_portal_animation"))]
    pub const SET_PORTAL_ANIMATION: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0059f39e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/start_portal_close_sound"))]
    pub const START_PORTAL_CLOSE_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a333b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/start_portal_open_sound"))]
    pub const START_PORTAL_OPEN_SOUND: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005a35e1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/zttank_wall_0"))]
    pub const ZTTANK_WALL_0: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005ae7a0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/zttank_wall_1"))]
    pub const ZTTANK_WALL_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x005ae9e4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwall/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00605b38, function_type: PhantomData};
}

// ZTTankWallType class functions
pub mod zttankwalltype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwalltype/load_characteristics"))]
    pub const LOAD_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c2c4d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zttankwalltype/load_type_members"))]
    pub const LOAD_TYPE_MEMBERS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x0051d2fe, function_type: PhantomData};
}

// ZTTerraformMode class functions
pub mod ztterraformmode {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztterraformmode/commit_terraform"))]
    pub const COMMIT_TERRAFORM: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004da252, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztterraformmode/do_terrain_paint"))]
    pub const DO_TERRAIN_PAINT: FunctionDef<unsafe extern "thiscall" fn(u32, i32, i32, bool)> = FunctionDef{address: 0x004dce9a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztterraformmode/cancel_terraform"))]
    pub const CANCEL_TERRAFORM: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00619368, function_type: PhantomData};
}

// ZTThought class functions
pub mod ztthought {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztthought/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, i32, i32) -> u32> = FunctionDef{address: 0x00423043, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztthought/get_string"))]
    pub const GET_STRING: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004edbfc, function_type: PhantomData};
}

// ZTThoughtMgr class functions
pub mod ztthoughtmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztthoughtmgr/get_thoughts_by_habitat"))]
    pub const GET_THOUGHTS_BY_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0046863a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztthoughtmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x0047a1a2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztthoughtmgr/remove_thoughts_by_habitat"))]
    pub const REMOVE_THOUGHTS_BY_HABITAT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x004ffe56, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztthoughtmgr/ztthought_mgr"))]
    pub const ZTTHOUGHT_MGR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057d815, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztthoughtmgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x0059435b, function_type: PhantomData};
}

// ZTUI class functions
pub mod ztui {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a281, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/click_continue"))]
    pub const CLICK_CONTINUE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00485c7e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/reinit_on_load"))]
    pub const REINIT_ON_LOAD: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004c735c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/show_game_opts"))]
    pub const SHOW_GAME_OPTS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004d81b8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/destroy"))]
    pub const DESTROY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00504346, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051275a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00514daa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui/hide_game_opts"))]
    pub const HIDE_GAME_OPTS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005b14c5, function_type: PhantomData};
}

// ZTUI::animalinfo class functions
pub mod ztui_animalinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_animalinfo/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x0041a76f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_animalinfo/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00427996, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_animalinfo/show"))]
    pub const SHOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ee820, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_animalinfo/set_can_sell"))]
    pub const SET_CAN_SELL: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x00592fe3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_animalinfo/unfollow"))]
    pub const UNFOLLOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00618c47, function_type: PhantomData};
}

// ZTUI::buya class functions
pub mod ztui_buya {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buya/update_display_list"))]
    pub const UPDATE_DISPLAY_LIST: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004eb68b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buya/clear_selections"))]
    pub const CLEAR_SELECTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f0a84, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buya/get_info_plaque"))]
    pub const GET_INFO_PLAQUE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f187a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buya/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00515b78, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buya/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051a2c9, function_type: PhantomData};
}

// ZTUI::buyh class functions
pub mod ztui_buyh {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buyh/clear_selections"))]
    pub const CLEAR_SELECTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f0a49, function_type: PhantomData};
}

// ZTUI::buyobj class functions
pub mod ztui_buyobj {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buyobj/update_display_list"))]
    pub const UPDATE_DISPLAY_LIST: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004eb401, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buyobj/clear_selections"))]
    pub const CLEAR_SELECTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f0abf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buyobj/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00515dc5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_buyobj/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051a2e4, function_type: PhantomData};
}

// ZTUI::cbuildinginfo class functions
pub mod ztui_cbuildinginfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_cbuildinginfo/show"))]
    pub const SHOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00488e4c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_cbuildinginfo/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051503f, function_type: PhantomData};
}

// ZTUI::credits class functions
pub mod ztui_credits {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_credits/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a430, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_credits/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00516e9f, function_type: PhantomData};
}

// ZTUI::developer class functions
pub mod ztui_developer {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_developer/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00517739, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_developer/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0058ed04, function_type: PhantomData};
}

// ZTUI::expansionselect class functions
pub mod ztui_expansionselect {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/get_any_expansions_disabled"))]
    pub const GET_ANY_EXPANSIONS_DISABLED: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x0041eee3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/is_expansion_disabled"))]
    pub const IS_EXPANSION_DISABLED: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0041ef86, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/get_expansion_list"))]
    pub const GET_EXPANSION_LIST: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004b1d62, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/hide"))]
    pub const HIDE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e0f09, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/set_expansion_id"))]
    pub const SET_EXPANSION_ID: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004f06a5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00516b0a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00517014, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_expansionselect/setup"))]
    pub const SETUP: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005291fb, function_type: PhantomData};
}

// ZTUI::gameopts class functions
pub mod ztui_gameopts {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/load_file"))]
    pub const LOAD_FILE: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x00453000, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/save_game"))]
    pub const SAVE_GAME: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004769ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/start_new_game"))]
    pub const START_NEW_GAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ca30e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/trigger_load"))]
    pub const TRIGGER_LOAD: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004cb9fa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/load_game"))]
    pub const LOAD_GAME: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004cc0d7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/check_video_options"))]
    pub const CHECK_VIDEO_OPTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004d7e4d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/stop_game"))]
    pub const STOP_GAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00501dfb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005164c3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00518375, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gameopts/return_to_main"))]
    pub const RETURN_TO_MAIN: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0059c339, function_type: PhantomData};
}

// ZTUI::gamescrn class functions
pub mod ztui_gamescrn {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gamescrn/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051595d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_gamescrn/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00519b19, function_type: PhantomData};
}

// ZTUI::general class functions
pub mod ztui_general {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/get_mapview"))]
    pub const GET_MAPVIEW: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004017e5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/update_display_lists"))]
    pub const UPDATE_DISPLAY_LISTS: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0040d37f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/get_selected_entity"))]
    pub const GET_SELECTED_ENTITY: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00410f84, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/hide_misc_panels"))]
    pub const HIDE_MISC_PANELS: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00443af8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/deselect_bulldozer"))]
    pub const DESELECT_BULLDOZER: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00443b60, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/hide_plaques"))]
    pub const HIDE_PLAQUES: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0044410c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/remove_selected_entity"))]
    pub const REMOVE_SELECTED_ENTITY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0048d052, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/hide_info_panels"))]
    pub const HIDE_INFO_PANELS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004b2355, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/entity_type_is_displayed"))]
    pub const ENTITY_TYPE_IS_DISPLAYED: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x004e8cc8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/get_info_image_name"))]
    pub const GET_INFO_IMAGE_NAME: FunctionDef<unsafe extern "cdecl" fn(i32) -> u32> = FunctionDef{address: 0x004f85d2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00519d24, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/hide_object_panels"))]
    pub const HIDE_OBJECT_PANELS: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x005b135b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/hide_multi_panels"))]
    pub const HIDE_MULTI_PANELS: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x005b1426, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_general/hide_all_layouts"))]
    pub const HIDE_ALL_LAYOUTS: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x005b14a4, function_type: PhantomData};
}

// ZTUI::guestinfo class functions
pub mod ztui_guestinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_guestinfo/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a78a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_guestinfo/show"))]
    pub const SHOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0046aa50, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_guestinfo/unfollow"))]
    pub const UNFOLLOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00618b0c, function_type: PhantomData};
}

// ZTUI::habitatinfo class functions
pub mod ztui_habitatinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a384, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/add_habitat"))]
    pub const ADD_HABITAT: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0044e48a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/select_tank_button"))]
    pub const SELECT_TANK_BUTTON: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00468b9d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/remove_habitat"))]
    pub const REMOVE_HABITAT: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0050c715, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051512d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/set_habitat"))]
    pub const SET_HABITAT: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0058b5ce, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/set_tank_button_enabled"))]
    pub const SET_TANK_BUTTON_ENABLED: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x0058b5f2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/refill_habitat_list"))]
    pub const REFILL_HABITAT_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005aeb82, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/remove_all_habitats"))]
    pub const REMOVE_ALL_HABITATS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005b1244, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_habitatinfo/get_icon_name"))]
    pub const GET_ICON_NAME: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x005b3149, function_type: PhantomData};
}

// ZTUI::heliinfo class functions
pub mod ztui_heliinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_heliinfo/show"))]
    pub const SHOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0048cd7a, function_type: PhantomData};
}

// ZTUI::help class functions
pub mod ztui_help {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_help/show"))]
    pub const SHOW: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x00484a32, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_help/hide"))]
    pub const HIDE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004b2972, function_type: PhantomData};
}

// ZTUI::hirestaff class functions
pub mod ztui_hirestaff {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_hirestaff/clear_selections"))]
    pub const CLEAR_SELECTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004c7486, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_hirestaff/randomize_sex"))]
    pub const RANDOMIZE_SEX: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e770d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_hirestaff/update_display_list"))]
    pub const UPDATE_DISPLAY_LIST: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004eb4cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_hirestaff/get_info_plaque"))]
    pub const GET_INFO_PLAQUE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f188a, function_type: PhantomData};
}

// ZTUI::infoplaque class functions
pub mod ztui_infoplaque {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_infoplaque/hide"))]
    pub const HIDE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004440d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_infoplaque/set_animal"))]
    pub const SET_ANIMAL: FunctionDef<unsafe extern "cdecl" fn(i32, i8)> = FunctionDef{address: 0x00483157, function_type: PhantomData};
}

// ZTUI::keeperinfo class functions
pub mod ztui_keeperinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_keeperinfo/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a331, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_keeperinfo/hide"))]
    pub const HIDE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004440f1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_keeperinfo/set_animal"))]
    pub const SET_ANIMAL: FunctionDef<unsafe extern "cdecl" fn(u32, bool)> = FunctionDef{address: 0x004831b2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_keeperinfo/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00483714, function_type: PhantomData};
}

// ZTUI::main class functions
pub mod ztui_main {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/set_money_text"))]
    pub const SET_MONEY_TEXT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0040ee3d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/set_animal_rating"))]
    pub const SET_ANIMAL_RATING: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0041d08b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/set_guest_rating"))]
    pub const SET_GUEST_RATING: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0041d15d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/set_zoo_rating"))]
    pub const SET_ZOO_RATING: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0041d22f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/set_date_text"))]
    pub const SET_DATE_TEXT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0041d565, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/clear_selections"))]
    pub const CLEAR_SELECTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004c77bb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00515979, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00519c1b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/pause_game"))]
    pub const PAUSE_GAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0059c6b8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_main/unpause_game"))]
    pub const UNPAUSE_GAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0059c7a9, function_type: PhantomData};
}

// ZTUI::mapselect class functions
pub mod ztui_mapselect {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_mapselect/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00516db1, function_type: PhantomData};
}

// ZTUI::multianimal class functions
pub mod ztui_multianimal {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_multianimal/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00418c35, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_multianimal/add_entity"))]
    pub const ADD_ENTITY: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x00453b38, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_multianimal/remove_entity"))]
    pub const REMOVE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x004fe532, function_type: PhantomData};
}

// ZTUI::ncbuildinginfo class functions
pub mod ztui_ncbuildinginfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_ncbuildinginfo/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0058ff06, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_ncbuildinginfo/show"))]
    pub const SHOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x006183ff, function_type: PhantomData};
}

// ZTUI::objective class functions
pub mod ztui_objective {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_objective/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00516ed0, function_type: PhantomData};
}

// ZTUI::rescon class functions
pub mod ztui_rescon {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_rescon/update_display_list"))]
    pub const UPDATE_DISPLAY_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004eb3f7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_rescon/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00514f6c, function_type: PhantomData};
}

// ZTUI::scenario class functions
pub mod ztui_scenario {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_scenario/start_scenario"))]
    pub const START_SCENARIO: FunctionDef<unsafe extern "cdecl" fn(i32) -> u32> = FunctionDef{address: 0x004c9bc9, function_type: PhantomData};
}

// ZTUI::showpanel class functions
pub mod ztui_showpanel {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a750, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/show_0"))]
    pub const SHOW_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004681b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/fill_exhibit_info_0"))]
    pub const FILL_EXHIBIT_INFO_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00468968, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/set_exhibit"))]
    pub const SET_EXHIBIT: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00468a5b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/fill_exhibit_info_1"))]
    pub const FILL_EXHIBIT_INFO_1: FunctionDef<unsafe extern "thiscall" fn(u32, u8, u8, u8, u8, u8, u8, u8, u8, u8, u8, u32, u32, u32)> = FunctionDef{address: 0x00474400, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/select_show_tab"))]
    pub const SELECT_SHOW_TAB: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00474646, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/show_1"))]
    pub const SHOW_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00474684, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/set_species"))]
    pub const SET_SPECIES: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00474ced, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/fill_trick_lists"))]
    pub const FILL_TRICK_LISTS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004751dc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005167ca, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_showpanel/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005a996f, function_type: PhantomData};
}

// ZTUI::staffinfo class functions
pub mod ztui_staffinfo {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_staffinfo/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a733, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_staffinfo/show"))]
    pub const SHOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00471813, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_staffinfo/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005b0f93, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_staffinfo/unfollow"))]
    pub const UNFOLLOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00618847, function_type: PhantomData};
}

// ZTUI::startup class functions
pub mod ztui_startup {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_startup/trigger_load"))]
    pub const TRIGGER_LOAD: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004cc606, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_startup/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00514e7e, function_type: PhantomData};
}

// ZTUI::tankmodify class functions
pub mod ztui_tankmodify {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_tankmodify/set_tank"))]
    pub const SET_TANK: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x0046895a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_tankmodify/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00516c04, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_tankmodify/fetch_values_from_tank"))]
    pub const FETCH_VALUES_FROM_TANK: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005ae509, function_type: PhantomData};
}

// ZTUI::terraform class functions
pub mod ztui_terraform {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_terraform/set_money_text"))]
    pub const SET_MONEY_TEXT: FunctionDef<unsafe extern "cdecl" fn(f32)> = FunctionDef{address: 0x004d9cae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_terraform/set_unit_cost"))]
    pub const SET_UNIT_COST: FunctionDef<unsafe extern "cdecl" fn(f32)> = FunctionDef{address: 0x004d9dbd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_terraform/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00516318, function_type: PhantomData};
}

// ZTUI::zooitems class functions
pub mod ztui_zooitems {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_zooitems/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0041a45b, function_type: PhantomData};
}

// ZTUI::zoostatus class functions
pub mod ztui_zoostatus {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_zoostatus/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x00401644, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_zoostatus/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0041739a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_zoostatus/add_callbacks"))]
    pub const ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00517320, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztui_zoostatus/add_completed_research"))]
    pub const ADD_COMPLETED_RESEARCH: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0058fc10, function_type: PhantomData};
}

// ZTUndoBuffer class functions
pub mod ztundobuffer {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztundobuffer/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004f15c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztundobuffer/add_action"))]
    pub const ADD_ACTION: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, u32, u32, u32, u32, u32, u32, u32, u32, u32, i32, i32)> = FunctionDef{address: 0x004f8a1f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztundobuffer/remove_action"))]
    pub const REMOVE_ACTION: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004fa670, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztundobuffer/ztundo_buffer"))]
    pub const ZTUNDO_BUFFER: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00502713, function_type: PhantomData};
}

// ZTUnit class functions
pub mod ztunit {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/validate_position"))]
    pub const VALIDATE_POSITION: FunctionDef<unsafe extern "thiscall" fn(u32, i8)> = FunctionDef{address: 0x004102b3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/get_footprint"))]
    pub const GET_FOOTPRINT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i8)> = FunctionDef{address: 0x0041070b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/is_on_water"))]
    pub const IS_ON_WATER: FunctionDef<unsafe extern "fastcall" fn(u32) -> u32> = FunctionDef{address: 0x00410fe0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/get_predator_unit"))]
    pub const GET_PREDATOR_UNIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x00412259, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/add_to_map"))]
    pub const ADD_TO_MAP: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00412c70, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/get_idle_anim"))]
    pub const GET_IDLE_ANIM: FunctionDef<unsafe extern "thiscall" fn(u32, i8) -> i32> = FunctionDef{address: 0x0041338a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/face_toward_target"))]
    pub const FACE_TOWARD_TARGET: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00426f21, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/remove_from_map"))]
    pub const REMOVE_FROM_MAP: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004277c9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/find_nearest_satisfier"))]
    pub const FIND_NEAREST_SATISFIER: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u32, u32, i32, i32) -> u32> = FunctionDef{address: 0x004290f3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/face_toward_target_unit"))]
    pub const FACE_TOWARD_TARGET_UNIT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042fa2c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/abort_show"))]
    pub const ABORT_SHOW: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0042fa42, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/update_building_tiles"))]
    pub const UPDATE_BUILDING_TILES: FunctionDef<unsafe extern "thiscall" fn(u32, bool) -> u32> = FunctionDef{address: 0x0043b2f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x00454488, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00478501, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/f_zoo_message_tile_0"))]
    pub const F_ZOO_MESSAGE_TILE_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32, i32, i8)> = FunctionDef{address: 0x004807c2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/f_zoo_message_tile_1"))]
    pub const F_ZOO_MESSAGE_TILE_1: FunctionDef<unsafe extern "stdcall" fn(u32, u32, i32, u32, u32, u32, u32, i32)> = FunctionDef{address: 0x0048083d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/f_exit_building"))]
    pub const F_EXIT_BUILDING: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0048c522, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/check_elevation"))]
    pub const CHECK_ELEVATION: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32, u32, i32) -> bool> = FunctionDef{address: 0x004935aa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/format_name_message"))]
    pub const FORMAT_NAME_MESSAGE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0049cfeb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/f_zoo_message_name"))]
    pub const F_ZOO_MESSAGE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, i8)> = FunctionDef{address: 0x0049d18e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/do_random_walk"))]
    pub const DO_RANDOM_WALK: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0049dafe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/get_target_unit"))]
    pub const GET_TARGET_UNIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> i32> = FunctionDef{address: 0x004a4b53, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/in_friendly_chase"))]
    pub const IN_FRIENDLY_CHASE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004a4bb7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/chase_done"))]
    pub const CHASE_DONE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> bool> = FunctionDef{address: 0x004a4ca0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/avoid_done"))]
    pub const AVOID_DONE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004a5508, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/get_path_to_target"))]
    pub const GET_PATH_TO_TARGET: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004a5b4d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/get_path_away_from_target"))]
    pub const GET_PATH_AWAY_FROM_TARGET: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004a639e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/create_name"))]
    pub const CREATE_NAME: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004e1b58, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004f578d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/ztunit"))]
    pub const ZTUNIT: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004f9cf0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/set_is_removed"))]
    pub const SET_IS_REMOVED: FunctionDef<unsafe extern "thiscall" fn(u32, u8)> = FunctionDef{address: 0x004fa78e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/init_after_load"))]
    pub const INIT_AFTER_LOAD: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005951b3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/remove_from_building_if_present"))]
    pub const REMOVE_FROM_BUILDING_IF_PRESENT: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005ae65f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/reset_ai"))]
    pub const RESET_AI: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005ae68b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunit/get_terrain_cost"))]
    pub const GET_TERRAIN_COST: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x00612f15, function_type: PhantomData};
}

// ZTUnitType class functions
pub mod ztunittype {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/set_available"))]
    pub const SET_AVAILABLE: FunctionDef<unsafe extern "thiscall" fn(u32, bool)> = FunctionDef{address: 0x0040d3ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/get_dirt_type"))]
    pub const GET_DIRT_TYPE: FunctionDef<unsafe extern "stdcall" fn() -> i32> = FunctionDef{address: 0x00410438, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/get_dino_dirt_type"))]
    pub const GET_DINO_DIRT_TYPE: FunctionDef<unsafe extern "stdcall" fn() -> i32> = FunctionDef{address: 0x0041044c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/load_characteristics_0"))]
    pub const LOAD_CHARACTERISTICS_0: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x004b5681, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/load_characteristics_1"))]
    pub const LOAD_CHARACTERISTICS_1: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004b5c71, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x004be558, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/ztunit_type"))]
    pub const ZTUNIT_TYPE: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x00500a69, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/get_trash_type"))]
    pub const GET_TRASH_TYPE: FunctionDef<unsafe extern "stdcall" fn() -> i32> = FunctionDef{address: 0x0052cfbe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztunittype/create_entity"))]
    pub const CREATE_ENTITY: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x006151b3, function_type: PhantomData};
}

// ZTViewingArea class functions
pub mod ztviewingarea {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/update_ambients"))]
    pub const UPDATE_AMBIENTS: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004358f2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/recalculate_characteristics"))]
    pub const RECALCULATE_CHARACTERISTICS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00447957, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/recalculate_oa"))]
    pub const RECALCULATE_OA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004479fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/add_tile"))]
    pub const ADD_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x0044ebe6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/get_ewextent"))]
    pub const GET_EWEXTENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0044ee46, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0044ee8e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/get_nsextent"))]
    pub const GET_NSEXTENT: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x0044f0f2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/clear_tile_of_va"))]
    pub const CLEAR_TILE_OF_VA: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004590c7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/ztviewing_area"))]
    pub const ZTVIEWING_AREA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00459196, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/create_oa"))]
    pub const CREATE_OA: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f380b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztviewingarea/remove_tile"))]
    pub const REMOVE_TILE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0050bc1a, function_type: PhantomData};
}

// ZTWorldMgr class functions
pub mod ztworldmgr {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/satisfies"))]
    pub const SATISFIES: FunctionDef<unsafe extern "cdecl" fn(i32, u32, u32) -> u32> = FunctionDef{address: 0x0041239d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/get_building_list"))]
    pub const GET_BUILDING_LIST: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x00413db8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/get_attractions"))]
    pub const GET_ATTRACTIONS: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x00427f43, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/get_gawk_scenery"))]
    pub const GET_GAWK_SCENERY: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x00427fd9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/get_num_sites"))]
    pub const GET_NUM_SITES: FunctionDef<unsafe extern "stdcall" fn() -> i32> = FunctionDef{address: 0x004281fd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/get_need_from_index"))]
    pub const GET_NEED_FROM_INDEX: FunctionDef<unsafe extern "cdecl" fn(u32, u8) -> u32> = FunctionDef{address: 0x0042aeaf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/get_need_index"))]
    pub const GET_NEED_INDEX: FunctionDef<unsafe extern "cdecl" fn(u32) -> i8> = FunctionDef{address: 0x0043aa46, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/find_nearest_unseen_starting_show"))]
    pub const FIND_NEAREST_UNSEEN_STARTING_SHOW: FunctionDef<unsafe extern "stdcall" fn(u32) -> u32> = FunctionDef{address: 0x00440f0c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/add_to_gawk_scenery_list"))]
    pub const ADD_TO_GAWK_SCENERY_LIST: FunctionDef<unsafe extern "stdcall" fn(i32) -> bool> = FunctionDef{address: 0x00451785, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u8> = FunctionDef{address: 0x00452e13, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/constructor"))]
    pub const CONSTRUCTOR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x004611d4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/create"))]
    pub const CREATE: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x0046498e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u8> = FunctionDef{address: 0x0047704c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/remove_all_tank_owned_transients"))]
    pub const REMOVE_ALL_TANK_OWNED_TRANSIENTS: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x00479eba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/remove_transients"))]
    pub const REMOVE_TRANSIENTS: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x00479efa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/remove_all_water_effects"))]
    pub const REMOVE_ALL_WATER_EFFECTS: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x00479f0b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/restore_all_tank_owned_transients"))]
    pub const RESTORE_ALL_TANK_OWNED_TRANSIENTS: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0047a162, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/load_types"))]
    pub const LOAD_TYPES: FunctionDef<unsafe extern "thiscall" fn(u32, bool) -> u32> = FunctionDef{address: 0x004bf19a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/clear"))]
    pub const CLEAR: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c6a77, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/init_building_map"))]
    pub const INIT_BUILDING_MAP: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004c75ae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/guesticide"))]
    pub const GUESTICIDE: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004c9d8c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/reset_guests_without_removing"))]
    pub const RESET_GUESTS_WITHOUT_REMOVING: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004c9eae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/reset_scenery"))]
    pub const RESET_SCENERY: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x004c9f8a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/reset_animal_counts"))]
    pub const RESET_ANIMAL_COUNTS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004c9fd4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/add_to_object_list"))]
    pub const ADD_TO_OBJECT_LIST: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x004fdf04, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/remove_from_all_building_lists"))]
    pub const REMOVE_FROM_ALL_BUILDING_LISTS: FunctionDef<unsafe extern "stdcall" fn(u32) -> u32> = FunctionDef{address: 0x005002ac, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/remove_from_building_list"))]
    pub const REMOVE_FROM_BUILDING_LIST: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x00500339, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/ztworld_mgr"))]
    pub const ZTWORLD_MGR: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x0057cce9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/add_ambient_effect"))]
    pub const ADD_AMBIENT_EFFECT: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x0059148e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/rubble_all_cars"))]
    pub const RUBBLE_ALL_CARS: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0060d8e3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("ztworldmgr/box_up_and_find_aplace_for_animal"))]
    pub const BOX_UP_AND_FIND_APLACE_FOR_ANIMAL: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0060da6b, function_type: PhantomData};
}

// ZooStatus class functions
pub mod zoostatus {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/calculate_sums"))]
    pub const CALCULATE_SUMS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041f881, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/rating_checks"))]
    pub const RATING_CHECKS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0041fcc1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/message_checks"))]
    pub const MESSAGE_CHECKS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00420347, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/newguest_checks"))]
    pub const NEWGUEST_CHECKS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00424375, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/f_chance"))]
    pub const F_CHANCE: FunctionDef<unsafe extern "stdcall" fn(i32) -> u32> = FunctionDef{address: 0x004244ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/admission_message"))]
    pub const ADMISSION_MESSAGE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x00429d68, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/buy_people_food"))]
    pub const BUY_PEOPLE_FOOD: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0042df22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/increase_donations"))]
    pub const INCREASE_DONATIONS: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0042ebbe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/update"))]
    pub const UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x004351a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/save"))]
    pub const SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u32> = FunctionDef{address: 0x0047ad4e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/spend_maint_wages"))]
    pub const SPEND_MAINT_WAGES: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x00483d34, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/finance_checks"))]
    pub const FINANCE_CHECKS: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00483f11, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/increase_endowment"))]
    pub const INCREASE_ENDOWMENT: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0048442b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/spend_guide_wages"))]
    pub const SPEND_GUIDE_WAGES: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0048bd8b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/refund_animal_cost"))]
    pub const REFUND_ANIMAL_COST: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0048d2da, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/spend_building_upkeep"))]
    pub const SPEND_BUILDING_UPKEEP: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x0049bd80, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/f_zoo_message"))]
    pub const F_ZOO_MESSAGE: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, i32)> = FunctionDef{address: 0x0049ce6b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/init"))]
    pub const INIT: FunctionDef<unsafe extern "thiscall" fn(u32, u32)> = FunctionDef{address: 0x004c2683, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/reset_finance_info"))]
    pub const RESET_FINANCE_INFO: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004c9f13, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/spend_construction"))]
    pub const SPEND_CONSTRUCTION: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x004d9250, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/spend_keeper_wages_0"))]
    pub const SPEND_KEEPER_WAGES_0: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x004e1fde, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/f_create_guest"))]
    pub const F_CREATE_GUEST: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004f6e3c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/refund_construction"))]
    pub const REFUND_CONSTRUCTION: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x004f9329, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/animal_escaped"))]
    pub const ANIMAL_ESCAPED: FunctionDef<unsafe extern "fastcall" fn(i32)> = FunctionDef{address: 0x0050cde4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/load"))]
    pub const LOAD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x0059497f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/increase_show_admission"))]
    pub const INCREASE_SHOW_ADMISSION: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x005a9718, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/spend_keeper_wages_1"))]
    pub const SPEND_KEEPER_WAGES_1: FunctionDef<unsafe extern "thiscall" fn(u32, f32)> = FunctionDef{address: 0x005ad038, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/change_endowment_members"))]
    pub const CHANGE_ENDOWMENT_MEMBERS: FunctionDef<unsafe extern "thiscall" fn(u32, i32)> = FunctionDef{address: 0x005ad160, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("zoostatus/f_grant_donation"))]
    pub const F_GRANT_DONATION: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00613e4a, function_type: PhantomData};
}

// bfinternat class functions
pub mod bfinternat {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/get_money_text_0"))]
    pub const GET_MONEY_TEXT_0: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x0040eca1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/set_money_text_0"))]
    pub const SET_MONEY_TEXT_0: FunctionDef<unsafe extern "cdecl" fn(i32, f32, i8)> = FunctionDef{address: 0x0040ed88, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/get_number_text"))]
    pub const GET_NUMBER_TEXT: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x00417879, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/get_date_text"))]
    pub const GET_DATE_TEXT: FunctionDef<unsafe extern "cdecl" fn(u32, i32, i32, i32) -> u32> = FunctionDef{address: 0x0041d39c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/set_date_text"))]
    pub const SET_DATE_TEXT: FunctionDef<unsafe extern "cdecl" fn(i32, i32, i32, i32)> = FunctionDef{address: 0x0041d4df, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/get_money_text_1"))]
    pub const GET_MONEY_TEXT_1: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i8) -> u32> = FunctionDef{address: 0x004ef4d4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/set_money_text_1"))]
    pub const SET_MONEY_TEXT_1: FunctionDef<unsafe extern "cdecl" fn(i32, u32, i8)> = FunctionDef{address: 0x004ef5eb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/set_number_text"))]
    pub const SET_NUMBER_TEXT: FunctionDef<unsafe extern "cdecl" fn(i32, u32, i8)> = FunctionDef{address: 0x004efe4f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/get_text_resource"))]
    pub const GET_TEXT_RESOURCE: FunctionDef<unsafe extern "cdecl" fn(u32, u32) -> u32> = FunctionDef{address: 0x004f00f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/set_text_resource"))]
    pub const SET_TEXT_RESOURCE: FunctionDef<unsafe extern "cdecl" fn(i32, u32)> = FunctionDef{address: 0x004f0264, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/init"))]
    pub const INIT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005358b7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("bfinternat/get_currency_symbol"))]
    pub const GET_CURRENCY_SYMBOL: FunctionDef<unsafe extern "thiscall" fn(u32) -> u32> = FunctionDef{address: 0x005fe02d, function_type: PhantomData};
}

// ph_BFPathMove class functions
pub mod ph_bfpathmove {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("ph_bfpathmove/do_progress"))]
    pub const DO_PROGRESS: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> i8> = FunctionDef{address: 0x00439cee, function_type: PhantomData};
}

// sndutil class functions
pub mod sndutil {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("sndutil/compute_volume_and_pan"))]
    pub const COMPUTE_VOLUME_AND_PAN: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32)> = FunctionDef{address: 0x0043f5ba, function_type: PhantomData};
}

// std::__vector_deleter<> class functions
pub mod std_vector_deleter {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("std_vector_deleter/vector_deleter"))]
    pub const VECTOR_DELETER: FunctionDef<unsafe extern "thiscall" fn(u32, u8) -> u32> = FunctionDef{address: 0x0060e4f5, function_type: PhantomData};
}

// std::basic_string<> class functions
pub mod std_basic_string {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("std_basic_string/basic_string"))]
    pub const BASIC_STRING: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004012ba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("std_basic_string/compare"))]
    pub const COMPARE: FunctionDef<unsafe extern "thiscall" fn(u32, u32, i32)> = FunctionDef{address: 0x00401a94, function_type: PhantomData};
}

// Standalone functions
pub mod standalone {
    use super::*;

    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_0"))]
    pub const NULLSUB_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040100b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_1"))]
    pub const NULLSUB_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0040100c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/vf_return_true_0"))]
    pub const VF_RETURN_TRUE_0: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00401302, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/vf_return_false"))]
    pub const VF_RETURN_FALSE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x004016d1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/find_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std722_tree_q23std135pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding_q33std380map_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding_q23std73less_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std155allocator_q23std135pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding13value_compare_q23std155allocator_q23std135pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding_frcq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q33std722_tree_q23std135pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding_q33std380map_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding_q23std73less_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std155allocator_q23std135pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding13value_compare_q23std155allocator_q23std135pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std52list_p10ztbuilding_q23std24allocator_p10ztbuilding21_generic_iterator0"))]
    pub const FIND_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD722_TREE_Q23STD135PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING_Q33STD380MAP_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING_Q23STD73LESS_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD155ALLOCATOR_Q23STD135PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING13VALUE_COMPARE_Q23STD155ALLOCATOR_Q23STD135PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING_FRCQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q33STD722_TREE_Q23STD135PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING_Q33STD380MAP_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING_Q23STD73LESS_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD155ALLOCATOR_Q23STD135PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING13VALUE_COMPARE_Q23STD155ALLOCATOR_Q23STD135PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD52LIST_P10ZTBUILDING_Q23STD24ALLOCATOR_P10ZTBUILDING21_GENERIC_ITERATOR0: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x00401b9f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/search_config_method"))]
    pub const SEARCH_CONFIG_METHOD: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x00401eb1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/operator_new"))]
    pub const OPERATOR_NEW: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x00402a5a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_2"))]
    pub const NULLSUB_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00402e53, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_3"))]
    pub const NULLSUB_3: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00402f82, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/crc32"))]
    pub const CRC32: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> u32> = FunctionDef{address: 0x004036cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/bfresource_hash_key"))]
    pub const BFRESOURCE_HASH_KEY: FunctionDef<unsafe extern "cdecl" fn(u32, u32)> = FunctionDef{address: 0x00403802, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/deallocate"))]
    pub const DEALLOCATE: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x00403e06, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/vf_return_true_1"))]
    pub const VF_RETURN_TRUE_1: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00404a7e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/load_string_from_resource"))]
    pub const LOAD_STRING_FROM_RESOURCE: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, i32) -> i32> = FunctionDef{address: 0x00404e72, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_4"))]
    pub const NULLSUB_4: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x00404fcf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_5"))]
    pub const NULLSUB_5: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004072ef, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_6"))]
    pub const NULLSUB_6: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0040733c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_7"))]
    pub const NULLSUB_7: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00409b61, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_8"))]
    pub const NULLSUB_8: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0040bcc9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ver_query_value_a"))]
    pub const VER_QUERY_VALUE_A: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, u32) -> bool> = FunctionDef{address: 0x0040dcfb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_needs_order_recalculate"))]
    pub const SET_NEEDS_ORDER_RECALCULATE: FunctionDef<unsafe extern "stdcall" fn(u8)> = FunctionDef{address: 0x0040f15b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/scale_rect"))]
    pub const SCALE_RECT: FunctionDef<unsafe extern "cdecl" fn(u32, i32, i32, i32, i32, i32, i32, i32) -> u32> = FunctionDef{address: 0x0040f33d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/expand_rect"))]
    pub const EXPAND_RECT: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i32)> = FunctionDef{address: 0x0040f38d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_ccw"))]
    pub const CLICK_ROTATE_CCW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00416cb7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/listable_object"))]
    pub const LISTABLE_OBJECT: FunctionDef<unsafe extern "cdecl" fn(i32, u32) -> u32> = FunctionDef{address: 0x004172e3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_9"))]
    pub const NULLSUB_9: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00418d1e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/null_startup"))]
    pub const NULL_STARTUP: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004190ff, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/uirender_text_0"))]
    pub const UIRENDER_TEXT_0: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i32, i32, i32, u32, i32, u32, u32, u32, u32, i8, i32)> = FunctionDef{address: 0x00419b0c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/uirender_text_1"))]
    pub const UIRENDER_TEXT_1: FunctionDef<unsafe extern "cdecl" fn(u32, u32, i32, i32, i32, u32, i32, u32, u32, u32, u32, i32, u32, u32, i32)> = FunctionDef{address: 0x00419c3d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/startup_update"))]
    pub const STARTUP_UPDATE: FunctionDef<unsafe extern "stdcall" fn(u8)> = FunctionDef{address: 0x0041a3d5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/maybe_snprintf"))]
    pub const MAYBE_SNPRINTF: FunctionDef<unsafe extern "cdecl" fn(u32, i32, u32) -> u32> = FunctionDef{address: 0x0041b460, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/load_ini_value_string"))]
    pub const LOAD_INI_VALUE_STRING: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0041b4bc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/maybe_sscanf"))]
    pub const MAYBE_SSCANF: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32)> = FunctionDef{address: 0x0041b958, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_status"))]
    pub const SET_STATUS: FunctionDef<unsafe extern "cdecl" fn(i32, u32, i32, i32)> = FunctionDef{address: 0x0041d058, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_10"))]
    pub const NULLSUB_10: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0041e63a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/tile_within_ava"))]
    pub const TILE_WITHIN_AVA: FunctionDef<unsafe extern "cdecl" fn(i32, i32) -> u32> = FunctionDef{address: 0x0044ed0c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/check_characteristics"))]
    pub const CHECK_CHARACTERISTICS: FunctionDef<unsafe extern "cdecl" fn(u32) -> bool> = FunctionDef{address: 0x004537fe, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/refill_animal_display"))]
    pub const REFILL_ANIMAL_DISPLAY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0045390c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/make_shape_idx"))]
    pub const MAKE_SHAPE_IDX: FunctionDef<unsafe extern "cdecl" fn(i32, i32, i32, i32) -> u32> = FunctionDef{address: 0x0045e493, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztmegatile_mgr"))]
    pub const CREATE_ZTMEGATILE_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00463e1e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_zthabitat_mgr"))]
    pub const CREATE_ZTHABITAT_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00464925, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztthought_mgr"))]
    pub const CREATE_ZTTHOUGHT_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00464b34, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_info_0"))]
    pub const UPDATE_INFO_0: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004673b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/fill_list_box_0"))]
    pub const FILL_LIST_BOX_0: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x00467a33, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/refill_habitat_info"))]
    pub const REFILL_HABITAT_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00467c6f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/refill_thoughts_list"))]
    pub const REFILL_THOUGHTS_LIST: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x00467e76, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/refill_animal_list"))]
    pub const REFILL_ANIMAL_LIST: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x00467fc8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_selected_habitat"))]
    pub const SET_SELECTED_HABITAT: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00468178, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_info_tab"))]
    pub const CLICK_INFO_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004681d0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_habitat"))]
    pub const HIDE_HABITAT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004682f1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_habitat"))]
    pub const SHOW_HABITAT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0046836c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_admission"))]
    pub const UPDATE_ADMISSION: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00468865, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_tank_modify"))]
    pub const CLICK_TANK_MODIFY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00468c00, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/tankmodify_adjust_tank_base"))]
    pub const TANKMODIFY_ADJUST_TANK_BASE: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0046937e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_thoughts_tab"))]
    pub const CLICK_THOUGHTS_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0046981e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_donations_tab"))]
    pub const CLICK_DONATIONS_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004699d2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_animals_tab"))]
    pub const CLICK_ANIMALS_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00469af2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/fill_list_box_1"))]
    pub const FILL_LIST_BOX_1: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0046a040, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_grab_animal"))]
    pub const CLICK_GRAB_ANIMAL: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0046af10, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_conservation"))]
    pub const CLICK_CONSERVATION: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00470a71, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_research"))]
    pub const CLICK_RESEARCH: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0047104b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_info_1"))]
    pub const UPDATE_INFO_1: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0047108a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_exhibit_info"))]
    pub const UPDATE_EXHIBIT_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00473b1f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_stats"))]
    pub const UPDATE_STATS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00473b61, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/disable_everything"))]
    pub const DISABLE_EVERYTHING: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004745a7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/copy_list_to_script"))]
    pub const COPY_LIST_TO_SCRIPT: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00475d92, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_save_0"))]
    pub const CLICK_SAVE_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00477004, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_save_1"))]
    pub const CLICK_SAVE_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00477041, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_save_file_name_a"))]
    pub const GET_SAVE_FILE_NAME_A: FunctionDef<unsafe extern "stdcall" fn(u32) -> bool> = FunctionDef{address: 0x00477046, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/write_bytes_to_file"))]
    pub const WRITE_BYTES_TO_FILE: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32, u32) -> u32> = FunctionDef{address: 0x0047772e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/force_research"))]
    pub const FORCE_RESEARCH: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0047ebd4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_info_0"))]
    pub const HIDE_INFO_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0047ecc0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_developer"))]
    pub const HIDE_DEVELOPER: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0047ed35, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_info_labels_0"))]
    pub const HIDE_INFO_LABELS_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0047ed53, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/depopulate_obj_list_0"))]
    pub const DEPOPULATE_OBJ_LIST_0: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x0047edc8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_developer_list"))]
    pub const HIDE_DEVELOPER_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0047ee2b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_developer"))]
    pub const SHOW_DEVELOPER: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0047ee5d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_developer_list"))]
    pub const CLICK_DEVELOPER_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0047ee74, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/repopulate_obj_list_0"))]
    pub const REPOPULATE_OBJ_LIST_0: FunctionDef<unsafe extern "cdecl" fn(u32, u32)> = FunctionDef{address: 0x0047ee98, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_developer_list"))]
    pub const SHOW_DEVELOPER_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x0047f0a2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/staffplaque_set_staff"))]
    pub const STAFFPLAQUE_SET_STAFF: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00481648, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/check_keeper_info_button"))]
    pub const CHECK_KEEPER_INFO_BUTTON: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004830c0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_keeper_info"))]
    pub const UNCLICK_KEEPER_INFO: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x0048330d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_info_plaque"))]
    pub const CLICK_INFO_PLAQUE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004833e3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_main_text"))]
    pub const SET_MAIN_TEXT: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x00483481, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_general_info"))]
    pub const SHOW_GENERAL_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004834b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_info_2"))]
    pub const UPDATE_INFO_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004834cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_keeper_info"))]
    pub const CLICK_KEEPER_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00483b21, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_topic_list"))]
    pub const CLICK_TOPIC_LIST: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0048482b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_about"))]
    pub const CLICK_ABOUT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00484b9a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_cbuilding_info"))]
    pub const HIDE_CBUILDING_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004881a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_info_3"))]
    pub const UPDATE_INFO_3: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00488446, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_cbuilding_info"))]
    pub const SHOW_CBUILDING_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00488b79, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/s_set_key"))]
    pub const S_SET_KEY: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0048a80a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_animal_info_follow"))]
    pub const CLICK_ANIMAL_INFO_FOLLOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0048c7ef, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_sell_animal"))]
    pub const CLICK_SELL_ANIMAL: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x0048d0b4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_right_0"))]
    pub const CLICK_ROTATE_RIGHT_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0048e40f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_info_4"))]
    pub const UPDATE_INFO_4: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x004a02f7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_res_con"))]
    pub const HIDE_RES_CON: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004a02fc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_res_con"))]
    pub const SHOW_RES_CON: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004a031a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_messages"))]
    pub const CLICK_MESSAGES: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004a0399, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/persistent_text_callback"))]
    pub const PERSISTENT_TEXT_CALLBACK: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004a09f8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_11"))]
    pub const NULLSUB_11: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004aba22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_12"))]
    pub const NULLSUB_12: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004abafb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_13"))]
    pub const NULLSUB_13: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004abc9b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_14"))]
    pub const NULLSUB_14: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ac022, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_15"))]
    pub const NULLSUB_15: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ac06c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_neighbor"))]
    pub const SET_NEIGHBOR: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32, i32, u32, i32, u32)> = FunctionDef{address: 0x004ac3f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_16"))]
    pub const NULLSUB_16: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ad05f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_cw"))]
    pub const CLICK_ROTATE_CW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004b071a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_zoom_out"))]
    pub const CLICK_ZOOM_OUT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004b0779, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_zoom_in"))]
    pub const CLICK_ZOOM_IN: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004b081b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/fps"))]
    pub const FPS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004b1949, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/save_ini_value_string"))]
    pub const SAVE_INI_VALUE_STRING: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32)> = FunctionDef{address: 0x004b294a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_17"))]
    pub const NULLSUB_17: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004b31cf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/time"))]
    pub const TIME: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004bc0c7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_18"))]
    pub const NULLSUB_18: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004c6761, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_19"))]
    pub const NULLSUB_19: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004c67d5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_go_0"))]
    pub const CLICK_GO_0: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x004ca15d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_go_1"))]
    pub const CLICK_GO_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ca34e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_map_list"))]
    pub const CLICK_MAP_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ca58e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/s_get_key"))]
    pub const S_GET_KEY: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x004cab6a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/fill_scenario_list"))]
    pub const FILL_SCENARIO_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004cacf3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_scenario"))]
    pub const SHOW_SCENARIO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004caf09, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_cash"))]
    pub const SET_CASH: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004cb384, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_map_select"))]
    pub const SHOW_MAP_SELECT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004cb4b1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/make_initial_dir"))]
    pub const MAKE_INITIAL_DIR: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x004cb6b6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_save_as_0"))]
    pub const CLICK_SAVE_AS_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004cb8ed, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_save_as_1"))]
    pub const CLICK_SAVE_AS_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004cb9b4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/imm_get_default_imewnd"))]
    pub const IMM_GET_DEFAULT_IMEWND: FunctionDef<unsafe extern "stdcall" fn(u32) -> u32> = FunctionDef{address: 0x004cc6cb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/make_file_filter"))]
    pub const MAKE_FILE_FILTER: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004cc6eb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_open_file_name_a"))]
    pub const GET_OPEN_FILE_NAME_A: FunctionDef<unsafe extern "stdcall" fn(u32) -> bool> = FunctionDef{address: 0x004cc7cb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_load_0"))]
    pub const CLICK_LOAD_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004cc7d1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/commit_if_possible"))]
    pub const COMMIT_IF_POSSIBLE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004da2e0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_terraform_layout"))]
    pub const SHOW_TERRAFORM_LAYOUT: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x004da60d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/convert_terrain_type_vector"))]
    pub const CONVERT_TERRAIN_TYPE_VECTOR: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x004da780, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_buy_habitat_list"))]
    pub const CLICK_BUY_HABITAT_LIST: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004da916, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_info_0"))]
    pub const SHOW_INFO_0: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004dab14, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/repopulate_obj_list_1"))]
    pub const REPOPULATE_OBJ_LIST_1: FunctionDef<unsafe extern "stdcall" fn(u32, u32)> = FunctionDef{address: 0x004dae5c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_paint"))]
    pub const CLICK_PAINT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004dcb50, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_modify"))]
    pub const CLICK_MODIFY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004dd68c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/terraform_suspend"))]
    pub const TERRAFORM_SUSPEND: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004de496, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_undo"))]
    pub const CLICK_UNDO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004de9ce, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/general_set_temp_entity_rotation"))]
    pub const GENERAL_SET_TEMP_ENTITY_ROTATION: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004df56d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_expansion_list"))]
    pub const HIDE_EXPANSION_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e0ee9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_info_1"))]
    pub const HIDE_INFO_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e0f12, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_buy_habitat_list"))]
    pub const UNCLICK_BUY_HABITAT_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e0fe6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/depopulate_obj_list_1"))]
    pub const DEPOPULATE_OBJ_LIST_1: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x004e1006, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_buy_habitat_list"))]
    pub const HIDE_BUY_HABITAT_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e1079, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/deselected_tab_0"))]
    pub const DESELECTED_TAB_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e1088, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/record_scroll_pos_0"))]
    pub const RECORD_SCROLL_POS_0: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e27a9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_info_plaque"))]
    pub const SHOW_INFO_PLAQUE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e2865, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_info_labels_1"))]
    pub const HIDE_INFO_LABELS_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e28c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_info_2"))]
    pub const HIDE_INFO_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e2967, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_buy_animal_list"))]
    pub const HIDE_BUY_ANIMAL_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e2ba6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_buy_animal_list"))]
    pub const UNCLICK_BUY_ANIMAL_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e2bb5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_buy_animal_list"))]
    pub const SHOW_BUY_ANIMAL_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e2bda, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/depopulate_obj_list_2"))]
    pub const DEPOPULATE_OBJ_LIST_2: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x004e2bec, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_buy_animal"))]
    pub const HIDE_BUY_ANIMAL: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e2cdd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_buy_animal_list"))]
    pub const CLICK_BUY_ANIMAL_LIST: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e2cf8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_info_1"))]
    pub const SHOW_INFO_1: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e2f17, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_info_labels_0"))]
    pub const SHOW_INFO_LABELS_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e32c6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/repopulate_obj_list_2"))]
    pub const REPOPULATE_OBJ_LIST_2: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e334b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_subtitle"))]
    pub const SET_SUBTITLE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e3842, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/deselected_tab_1"))]
    pub const DESELECTED_TAB_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3867, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_buy_animal"))]
    pub const SHOW_BUY_ANIMAL: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3917, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_animal_tab"))]
    pub const CLICK_ANIMAL_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3bc1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_rotate_buttons_0"))]
    pub const HIDE_ROTATE_BUTTONS_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3c83, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_sex_buttons"))]
    pub const SHOW_SEX_BUTTONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3d22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_sex_button"))]
    pub const CLICK_SEX_BUTTON: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3ec3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_info_3"))]
    pub const HIDE_INFO_3: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3f64, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_info_labels_2"))]
    pub const HIDE_INFO_LABELS_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e3fd9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_buy_obj_list"))]
    pub const HIDE_BUY_OBJ_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e4012, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/depopulate_obj_list_3"))]
    pub const DEPOPULATE_OBJ_LIST_3: FunctionDef<unsafe extern "cdecl" fn(i8)> = FunctionDef{address: 0x004e4021, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_buy_object_list"))]
    pub const UNCLICK_BUY_OBJECT_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e4084, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_buy_object"))]
    pub const HIDE_BUY_OBJECT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e40d9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_buy_object_list"))]
    pub const CLICK_BUY_OBJECT_LIST: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e40f4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/repopulate_obj_list_3"))]
    pub const REPOPULATE_OBJ_LIST_3: FunctionDef<unsafe extern "stdcall" fn(u32)> = FunctionDef{address: 0x004e4343, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_grouping_title"))]
    pub const SET_GROUPING_TITLE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e4890, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/deselected_tab_2"))]
    pub const DESELECTED_TAB_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e48b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_buy_obj_list"))]
    pub const SHOW_BUY_OBJ_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e48be, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_building_tab"))]
    pub const CLICK_BUILDING_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e4951, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_buy_object"))]
    pub const SHOW_BUY_OBJECT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e49f5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/select_expansion"))]
    pub const SELECT_EXPANSION: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e4c4d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_expansion_list"))]
    pub const SHOW_EXPANSION_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e4c97, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_scenery_tab"))]
    pub const CLICK_SCENERY_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e5090, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_right_1"))]
    pub const CLICK_ROTATE_RIGHT_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e5134, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/record_scroll_pos_1"))]
    pub const RECORD_SCROLL_POS_1: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e5835, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_rotate_buttons"))]
    pub const SHOW_ROTATE_BUTTONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e58f9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_shelters_tab"))]
    pub const CLICK_SHELTERS_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e60e8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_toys_tab"))]
    pub const CLICK_TOYS_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e6366, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_show_toys_tab"))]
    pub const CLICK_SHOW_TOYS_TAB: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e6414, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_rotate_buttons_1"))]
    pub const HIDE_ROTATE_BUTTONS_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e65ab, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/depopulate_obj_list_4"))]
    pub const DEPOPULATE_OBJ_LIST_4: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e673b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_hire_staff_list"))]
    pub const HIDE_HIRE_STAFF_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e6794, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_hire_staff_list"))]
    pub const CLICK_HIRE_STAFF_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x004e69b8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_info_2"))]
    pub const SHOW_INFO_2: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x004e6ad3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_info_labels_1"))]
    pub const SHOW_INFO_LABELS_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e6b43, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/repopulate_obj_list_4"))]
    pub const REPOPULATE_OBJ_LIST_4: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004e6bbd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_info_5"))]
    pub const UPDATE_INFO_5: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x004eae69, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_branch"))]
    pub const GET_BRANCH: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x004eb21b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/fill_list"))]
    pub const FILL_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004eb270, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/refilll_animal_display"))]
    pub const REFILLL_ANIMAL_DISPLAY: FunctionDef<unsafe extern "stdcall" fn(u8, u8, u8, u32, u32, u32)> = FunctionDef{address: 0x004ed7f1, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_animal_info_close"))]
    pub const CLICK_ANIMAL_INFO_CLOSE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ed80a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_animal_info"))]
    pub const SHOW_ANIMAL_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004ee665, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_tab"))]
    pub const CLICK_TAB: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x004ee81b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/general_set_selected_entity"))]
    pub const GENERAL_SET_SELECTED_ENTITY: FunctionDef<unsafe extern "cdecl" fn(u32, i8)> = FunctionDef{address: 0x004ef75b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_scenario_objectives"))]
    pub const SHOW_SCENARIO_OBJECTIVES: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004efdf6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_scenario_info"))]
    pub const UPDATE_SCENARIO_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f004a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/check_trigger_ele"))]
    pub const CHECK_TRIGGER_ELE: FunctionDef<unsafe extern "cdecl" fn(i32) -> u32> = FunctionDef{address: 0x004f052c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_multi_building_list"))]
    pub const UPDATE_MULTI_BUILDING_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f0979, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_progress"))]
    pub const UPDATE_PROGRESS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x004f202e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_20"))]
    pub const NULLSUB_20: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00501634, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_new_0"))]
    pub const CLICK_NEW_0: FunctionDef<unsafe extern "stdcall" fn(i8) -> u32> = FunctionDef{address: 0x00501699, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_new_1"))]
    pub const CLICK_NEW_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00501713, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_load_1"))]
    pub const CLICK_LOAD_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00501d71, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_load_2"))]
    pub const CLICK_LOAD_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00501df6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_21"))]
    pub const NULLSUB_21: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00503e72, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_22"))]
    pub const NULLSUB_22: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00503ebc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_23"))]
    pub const NULLSUB_23: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00504060, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_24"))]
    pub const NULLSUB_24: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005040aa, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_25"))]
    pub const NULLSUB_25: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005041d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_26"))]
    pub const NULLSUB_26: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00504220, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_bulldozer"))]
    pub const CLICK_BULLDOZER: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00509722, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_bulldozer"))]
    pub const UNCLICK_BULLDOZER: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005097b8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/check_last_file"))]
    pub const CHECK_LAST_FILE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0050ef37, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_startup"))]
    pub const SHOW_STARTUP: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0050f1c8, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_selected_map"))]
    pub const SET_SELECTED_MAP: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0050f396, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_download_state"))]
    pub const SET_DOWNLOAD_STATE: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00512c87, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/clear_q23std539_tree_q23std90pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_13superkeeper_t_q33std289map_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_13superkeeper_t_q23std73less_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std109allocator_q23std90pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_13superkeeper_t13value_compare_q23std109allocator_q23std90pair_cq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_13superkeeper_t_fv"))]
    pub const CLEAR_Q23STD539_TREE_Q23STD90PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_13SUPERKEEPER_T_Q33STD289MAP_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_13SUPERKEEPER_T_Q23STD73LESS_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD109ALLOCATOR_Q23STD90PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_13SUPERKEEPER_T13VALUE_COMPARE_Q23STD109ALLOCATOR_Q23STD90PAIR_CQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_13SUPERKEEPER_T_FV: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x005145cc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/animalinfo_add_callbacks"))]
    pub const ANIMALINFO_ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00515f1f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/buyh_add_callbacks"))]
    pub const BUYH_ADD_CALLBACKS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051611c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/check_terrain_options"))]
    pub const CHECK_TERRAIN_OPTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00517854, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/save_terrain_options"))]
    pub const SAVE_TERRAIN_OPTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005178d7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/adjust_terrain"))]
    pub const ADJUST_TERRAIN: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00517b9a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/enumerate_display_modes"))]
    pub const ENUMERATE_DISPLAY_MODES: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00518cbb, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/init_terrain_options"))]
    pub const INIT_TERRAIN_OPTIONS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00518d7f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztscript_mgr"))]
    pub const CREATE_ZTSCRIPT_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051e0d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztresearch_mgr"))]
    pub const CREATE_ZTRESEARCH_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051e293, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztshow_mgr"))]
    pub const CREATE_ZTSHOW_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0051f79e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztmessage_queue"))]
    pub const CREATE_ZTMESSAGE_QUEUE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00520b61, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztmini_map"))]
    pub const CREATE_ZTMINI_MAP: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00520d1c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/load_ini_value_float"))]
    pub const LOAD_INI_VALUE_FLOAT: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32, f32) -> f32> = FunctionDef{address: 0x0052211d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_alpha_surface"))]
    pub const CREATE_ALPHA_SURFACE: FunctionDef<unsafe extern "cdecl" fn(u32, u32, u32) -> bool> = FunctionDef{address: 0x00522a31, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/load_image"))]
    pub const LOAD_IMAGE: FunctionDef<unsafe extern "stdcall" fn(u8, u8, u8, u8, u32, u8, u8, u8, u8, u8, u32)> = FunctionDef{address: 0x00523326, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztmarketing_mgr"))]
    pub const CREATE_ZTMARKETING_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005257e7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztscenario_mgr"))]
    pub const CREATE_ZTSCENARIO_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00525e27, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_ztgame_mgr"))]
    pub const CREATE_ZTGAME_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00527dc7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/create_dxsnd_mgr"))]
    pub const CREATE_DXSND_MGR: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005280c3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/dir_search"))]
    pub const DIR_SEARCH: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x00529b75, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/setup_progress_bar"))]
    pub const SETUP_PROGRESS_BAR: FunctionDef<unsafe extern "cdecl" fn(i32, i32)> = FunctionDef{address: 0x0052ae16, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_log_level_string"))]
    pub const GET_LOG_LEVEL_STRING: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x0052ba65, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_marketing_info"))]
    pub const UPDATE_MARKETING_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0052f686, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_finance_info"))]
    pub const UPDATE_FINANCE_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0052fa39, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_graphs"))]
    pub const UPDATE_GRAPHS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0052ff4d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_zoo_status_info"))]
    pub const UPDATE_ZOO_STATUS_INFO: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0053002b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_zoo_status"))]
    pub const SHOW_ZOO_STATUS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005314f3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_awards"))]
    pub const SHOW_AWARDS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0053167f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_snapshot"))]
    pub const CLICK_SNAPSHOT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00532df9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ijl_init_0"))]
    pub const IJL_INIT_0: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x00532f9b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ijl_write"))]
    pub const IJL_WRITE: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x00533041, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ijl_free_0"))]
    pub const IJL_FREE_0: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x00533047, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ijl_init_1"))]
    pub const IJL_INIT_1: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x0053304d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ijl_free_1"))]
    pub const IJL_FREE_1: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x005330ae, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ijl_init_2"))]
    pub const IJL_INIT_2: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x005330b4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_27"))]
    pub const NULLSUB_27: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005330ba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/ijl_init_3"))]
    pub const IJL_INIT_3: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x00533180, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/init_video_quality"))]
    pub const INIT_VIDEO_QUALITY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00533a78, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/load_string_to_global_buffer"))]
    pub const LOAD_STRING_TO_GLOBAL_BUFFER: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x005349b0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_file_version_info_size_a"))]
    pub const GET_FILE_VERSION_INFO_SIZE_A: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> i32> = FunctionDef{address: 0x005357d6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_file_version_info_a"))]
    pub const GET_FILE_VERSION_INFO_A: FunctionDef<unsafe extern "stdcall" fn(u32, i32, i32, u32) -> bool> = FunctionDef{address: 0x005357dc, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/init_terrain_quality"))]
    pub const INIT_TERRAIN_QUALITY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00536599, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/init_sound_quality"))]
    pub const INIT_SOUND_QUALITY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0053672f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_dxversion"))]
    pub const GET_DXVERSION: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00536bce, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/localtime"))]
    pub const LOCALTIME: FunctionDef<unsafe extern "stdcall" fn(u32) -> u32> = FunctionDef{address: 0x0053833a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/sinit_zt_cpp_0"))]
    pub const SINIT_ZT_CPP_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005784d3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/sinit_zt_cpp_1"))]
    pub const SINIT_ZT_CPP_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00579322, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/load_ini_debug_settings"))]
    pub const LOAD_INI_DEBUG_SETTINGS: FunctionDef<unsafe extern "stdcall" fn() -> u32> = FunctionDef{address: 0x00579f4c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/initialize_application_heap"))]
    pub const INITIALIZE_APPLICATION_HEAP: FunctionDef<unsafe extern "cdecl" fn(i32) -> u32> = FunctionDef{address: 0x0057becf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/sinit_zoo_tycoon_app_cp"))]
    pub const SINIT_ZOO_TYCOON_APP_CP: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0057c528, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/exit"))]
    pub const EXIT: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x0057e4b7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/init_global_bfregistry"))]
    pub const INIT_GLOBAL_BFREGISTRY: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005886cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_left_0"))]
    pub const CLICK_ROTATE_LEFT_0: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00589cf4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_toggle_entity_tooltips"))]
    pub const SET_TOGGLE_ENTITY_TOOLTIPS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0058b67d, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/force_update"))]
    pub const FORCE_UPDATE: FunctionDef<unsafe extern "thiscall" fn(u32)> = FunctionDef{address: 0x0058c78e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_avail"))]
    pub const SET_AVAIL: FunctionDef<unsafe extern "cdecl" fn(i32, u32)> = FunctionDef{address: 0x0058fd9f, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_building_upgrade"))]
    pub const SET_BUILDING_UPGRADE: FunctionDef<unsafe extern "cdecl" fn(i32, i32, u32, i32, i32, i32, i8) -> u32> = FunctionDef{address: 0x0058feb7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_startup"))]
    pub const HIDE_STARTUP: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0059274c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/is_zoo_wall"))]
    pub const IS_ZOO_WALL: FunctionDef<unsafe extern "cdecl" fn(u32) -> bool> = FunctionDef{address: 0x005947b2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/is_zoo_gate"))]
    pub const IS_ZOO_GATE: FunctionDef<unsafe extern "cdecl" fn(u32) -> bool> = FunctionDef{address: 0x0059486c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_habitat_list"))]
    pub const CLICK_HABITAT_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00599ae7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_effect_discount"))]
    pub const SET_EFFECT_DISCOUNT: FunctionDef<unsafe extern "cdecl" fn(i32, i32, i32, i32) -> u32> = FunctionDef{address: 0x0059b6b5, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_pause"))]
    pub const CLICK_PAUSE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0059c7a4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/animalinfo_set_animal_dying"))]
    pub const ANIMALINFO_SET_ANIMAL_DYING: FunctionDef<unsafe extern "cdecl" fn(u32, i8)> = FunctionDef{address: 0x005a55f6, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/flyingobject"))]
    pub const FLYINGOBJECT: FunctionDef<unsafe extern "cdecl" fn(u32) -> u32> = FunctionDef{address: 0x005a58ff, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/nullsub_28"))]
    pub const NULLSUB_28: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005aacaf, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_fullscreen"))]
    pub const SET_FULLSCREEN: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005ac651, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_window"))]
    pub const SET_WINDOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005aff13, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_map_select"))]
    pub const HIDE_MAP_SELECT: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005b0f12, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/update_info_6"))]
    pub const UPDATE_INFO_6: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005b108b, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/check_name"))]
    pub const CHECK_NAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x005b10f4, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/fast_error_exit"))]
    pub const FAST_ERROR_EXIT: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x005fcf1c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/rtl_unwind"))]
    pub const RTL_UNWIND: FunctionDef<unsafe extern "stdcall" fn(u32, u32, u32, u32)> = FunctionDef{address: 0x005fd361, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/george_w"))]
    pub const GEORGE_W: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00607174, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/colloseum"))]
    pub const COLLOSEUM: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x006074ed, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hitchcock"))]
    pub const HITCHCOCK: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00607552, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/poo_rock"))]
    pub const POO_ROCK: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00607871, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/permanentflyingobject"))]
    pub const PERMANENTFLYINGOBJECT: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x00607a78, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/insert_one_q23std244_tree_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std73less_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_q23std78allocator_q23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc_frcq23std59basic_stringc_q23std14char_traitsc_q23std12allocatorc"))]
    pub const INSERT_ONE_Q23STD244_TREE_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD73LESS_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_Q23STD78ALLOCATOR_Q23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC_FRCQ23STD59BASIC_STRINGC_Q23STD14CHAR_TRAITSC_Q23STD12ALLOCATORC: FunctionDef<unsafe extern "thiscall" fn(u32, u32, u32)> = FunctionDef{address: 0x00607ab7, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/assign_if_positive"))]
    pub const ASSIGN_IF_POSITIVE: FunctionDef<unsafe extern "thiscall" fn(u32, i32, u8)> = FunctionDef{address: 0x0060875c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_download"))]
    pub const CLICK_DOWNLOAD: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0060b7f0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/download_completed"))]
    pub const DOWNLOAD_COMPLETED: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x0060bb54, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_animal_list"))]
    pub const CLICK_ANIMAL_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00610755, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/change_habitat_name"))]
    pub const CHANGE_HABITAT_NAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x006107ef, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/tankmodify_fill_tank"))]
    pub const TANKMODIFY_FILL_TANK: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x006115f2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/enable_everything"))]
    pub const ENABLE_EVERYTHING: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00615794, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/set_show_species"))]
    pub const SET_SHOW_SPECIES: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x006158a0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_animal_info_follow"))]
    pub const UNCLICK_ANIMAL_INFO_FOLLOW: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00616231, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/complete_current_goal"))]
    pub const COMPLETE_CURRENT_GOAL: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00616242, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_credits"))]
    pub const SHOW_CREDITS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00616290, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_page"))]
    pub const SHOW_PAGE: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x006162c0, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/hide_credits"))]
    pub const HIDE_CREDITS: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00616348, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_cash_up"))]
    pub const CLICK_CASH_UP: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00616350, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_cash_down"))]
    pub const CLICK_CASH_DOWN: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x006163b2, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/edit_starting_cash"))]
    pub const EDIT_STARTING_CASH: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00616414, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_developer_list"))]
    pub const UNCLICK_DEVELOPER_LIST: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x006169ba, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_left_1"))]
    pub const CLICK_ROTATE_LEFT_1: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x006169df, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_right_2"))]
    pub const CLICK_ROTATE_RIGHT_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00616a9a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/adjust_cash"))]
    pub const ADJUST_CASH: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x00616b58, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/get_animal_unhappy_with_scenery_string"))]
    pub const GET_ANIMAL_UNHAPPY_WITH_SCENERY_STRING: FunctionDef<unsafe extern "cdecl" fn(u32, i32, u32, u32) -> u32> = FunctionDef{address: 0x00616c3e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_category_list"))]
    pub const CLICK_CATEGORY_LIST: FunctionDef<unsafe extern "cdecl" fn(i32)> = FunctionDef{address: 0x006173f3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/show_ncbuilding_info"))]
    pub const SHOW_NCBUILDING_INFO: FunctionDef<unsafe extern "stdcall" fn(u8)> = FunctionDef{address: 0x0061794a, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_terraform_cancel"))]
    pub const CLICK_TERRAFORM_CANCEL: FunctionDef<unsafe extern "stdcall" fn(i32)> = FunctionDef{address: 0x00618a22, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/change_animal_name"))]
    pub const CHANGE_ANIMAL_NAME: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00618b27, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_animal_more_info_button"))]
    pub const CLICK_ANIMAL_MORE_INFO_BUTTON: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00618c02, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/click_rotate_left_2"))]
    pub const CLICK_ROTATE_LEFT_2: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00618c9c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/unclick_info_plaque"))]
    pub const UNCLICK_INFO_PLAQUE: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00618d57, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/thread_download"))]
    pub const THREAD_DOWNLOAD: FunctionDef<unsafe extern "cdecl" fn(u32) -> bool> = FunctionDef{address: 0x0062671e, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/configure_keyboard_settings"))]
    pub const CONFIGURE_KEYBOARD_SETTINGS: FunctionDef<unsafe extern "cdecl" fn(u32)> = FunctionDef{address: 0x00629a38, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/exit_application_gracefully"))]
    pub const EXIT_APPLICATION_GRACEFULLY: FunctionDef<unsafe extern "cdecl" fn()> = FunctionDef{address: 0x00629b1c, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/imm_release_context"))]
    pub const IMM_RELEASE_CONTEXT: FunctionDef<unsafe extern "stdcall" fn(u32, u32) -> bool> = FunctionDef{address: 0x0062c7cd, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/imm_notify_ime"))]
    pub const IMM_NOTIFY_IME: FunctionDef<unsafe extern "stdcall" fn(u32, i32, i32, i32) -> bool> = FunctionDef{address: 0x0062c7d3, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/imm_get_open_status"))]
    pub const IMM_GET_OPEN_STATUS: FunctionDef<unsafe extern "stdcall" fn(u32) -> bool> = FunctionDef{address: 0x0062c7d9, function_type: PhantomData};
    #[cfg_attr(feature = "detour-validation", validate_detour("standalone/imm_get_context"))]
    pub const IMM_GET_CONTEXT: FunctionDef<unsafe extern "stdcall" fn(u32) -> u32> = FunctionDef{address: 0x0062c7df, function_type: PhantomData};
    // #[cfg_attr(feature = "detour-validation", validate_detour("standalone/vscode_exit"))]
    pub const EXIT_3: FunctionDef<unsafe extern "stdcall" fn()> = FunctionDef{address: 0x00501224, function_type: PhantomData};
}