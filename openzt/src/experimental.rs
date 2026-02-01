use openzt_detour_macro::detour_mod;
use tracing::info;

// use crate::{
//     util::get_from_memory,
// };

// 0049ccc3
// void __thiscall BFUIMgr::displayMessage(void *this,uint param_1,int param_2,BFTile *param_3,BFEntity *param_4,bool param_5, bool param_6)

#[detour_mod]
pub mod zoo_experimental {
    use openzt_detour::gen::bfuimgr::DISPLAY_MESSAGE_0;
    use tracing::info;

    // use crate::{
    //     bfregistry::{add_to_registry, get_from_registry},
    //     util::{get_from_memory, get_string_from_memory},
    // };

    #[detour(DISPLAY_MESSAGE_0)]
    unsafe extern "thiscall" fn prt_get(_this_prt: u32, param_1: u32, param_2: i32, param_3: u32, param_4: u32, param_5: bool, param_6: bool) {
        info!(
            "BFUIMgr::displayMessage called with params: {}, {}, {}, {}, {}, {}",
            param_1, param_2, param_3, param_4, param_5, param_6
        );
        unsafe { DISPLAY_MESSAGE_0_DETOUR.call(_this_prt, param_1, param_2, param_3, param_4, param_5, param_6) };
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
    if let Err(e) = unsafe { zoo_experimental::init_detours() } {
        info!("Error initialising experimental detours: {}", e);
    };
}
