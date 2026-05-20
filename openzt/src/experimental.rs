use openzt_detour_macro::detour_mod;
use tracing::info;

// use crate::{
//     util::get_from_memory,
// };

// 0049ccc3
// void __thiscall BFUIMgr::displayMessage(void *this,uint param_1,int param_2,BFTile *param_3,BFEntity *param_4,bool param_5, bool param_6)

#[detour_mod]
pub mod zoo_experimental {
    use std::marker::PhantomData;

    use openzt_detour::{FunctionDef, generated::bfuimgr::DISPLAY_MESSAGE_0, generated::ztscenerytype::GET_HELP_ID};
    use tracing::info;
    use crate::util::{Addr, get_from_memory};


    // uint __thiscall FUN_0061c95c(void *this,char **param_1)
    const ZTSHOWSCRIPTSTATE_SAVE: FunctionDef<unsafe extern "thiscall" fn(u32, u32) -> u8> = FunctionDef{address: 0x0061c95c, function_type: PhantomData};

    #[detour(ZTSHOWSCRIPTSTATE_SAVE)]
    unsafe extern "thiscall" fn zt_show_script_state(_this_ptr: u32, param_1: u32) -> u8 {
        info!("FUN_0061c95c called with params: {:#x}, {:#x}", _this_ptr, param_1);
        let result = unsafe { ZTSHOWSCRIPTSTATE_SAVE_DETOUR.call(_this_ptr, param_1) };
        info!("FUN_0061c95c called with params: {:#x}, {:#x} -> {}", _this_ptr, param_1, result);
        result
    }

    #[detour(DISPLAY_MESSAGE_0)]
    unsafe extern "thiscall" fn prt_get(_this_prt: *const u32, param_1: u32, param_2: i32, param_3: *const u32, param_4: *const u32, param_5: bool, param_6: bool) {
        info!(
            "BFUIMgr::displayMessage called with params: {}, {}, {}, {}, {}, {}",
            param_1, param_2, Addr::of(param_3), Addr::of(param_4), param_5, param_6
        );
        unsafe { DISPLAY_MESSAGE_0_DETOUR.call(_this_prt, param_1, param_2, param_3, param_4, param_5, param_6) };
    }

    #[detour(GET_HELP_ID)]
    unsafe extern "thiscall" fn get_help_id(_this_ptr: *const u32) -> u32 {
        // Log this pointer in hex
        info!("GET_HELP_ID called, this: {:#x}", _this_ptr as u32);

        // Read what this pointer points to (first u32 value at that address)
        let this_value = get_from_memory::<u32>(_this_ptr);
        info!("  this points to: {:#x}", this_value);

        // Call original function
        let result = unsafe { GET_HELP_ID_DETOUR.call(_this_ptr) };

        // Log return value in both hex and decimal
        info!("  returned: {:#x} ({})", result, result);

        result
    }

    // // 0x431c3e : void __thiscall FUN_00431c3e(void *this,int *param_1,int *param_2,char param_3,int **param_4)
    // #[hook(unsafe extern "thiscall" FUN_00431c3e, offset = 0x00031c3e)]
    // fn fun_00431c3e(_this: u32, param_1: u32, param_2: u32, param_3: u8, param_4: u32) {
    //     info!("FUN_00431c3e called with params: {:#x}, {:#x}, {:#x}, {:#x}", _this, param_1, param_2, param_4);
    //     unsafe { FUN_00431c3e.call(_this, param_1, param_2, param_3, param_4) };
    // }

    // 0x45b92f : cls_0x6312bc * __thiscall ZTTankExhibit::ZTTankExhibit(ZTTankExhibit *this,BFTile *param_1,bool param_2,bool param_3)
    // #[hook(unsafe extern "thiscall" ZTTankExhibit_ctor, offset = 0x0005b92f)]
    // fn zt_tank_exhibit_ctor(_this: u32, param_1: u32, param_2: bool, param_3: bool) -> u32 {
    //     info!("ZTTankExhibit::ZTTankExhibit called with params: {:#x}, {:#x}, {}, {}", _this, param_1, param_2, param_3);
    //     let result = unsafe { ZTTankExhibit_ctor.call(_this, param_1, param_2, param_3) };
    //     info!("ZTTankExhibit::ZTTankExhibit result: {:#x} {:#x}", result, get_from_memory::<u32>(result));
    //     result
    // }

    // // 0x00411fed void * __thiscall GXMixer::getAnim(void *this)
    // #[hook(unsafe extern "thiscall" GXMixer_get_anim, offset = 0x00011fed)]
    // fn gxmixer_get_anim(_this: u32) -> u32 {
    //     info!("GXMixer::getAnim called with params: {:#x} {:#x}", _this, get_from_memory::<u32>(_this));
    //     unsafe { GXMixer_get_anim.call(_this) }
    // }
}

pub fn init() {
    // if let Err(e) = unsafe { zoo_experimental::init_detours() } {
    //     info!("Error initialising experimental detours: {}", e);
    // };
}
