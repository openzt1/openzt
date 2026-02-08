use core::fmt;
use num_enum::FromPrimitive;
use openzt_detour_macro::detour_mod;
use tracing::info;

use crate::bfentitytype::{zt_entity_type_class_is, ZTEntityTypeClass};
use crate::util::get_from_memory;
use crate::zthabitatmgr::read_zt_habitat_mgr_from_memory;
use crate::ztworldmgr::{BFEntity, IVec3};
// use crate::{
//     util::get_from_memory,
// };

// 0049ccc3
// void __thiscall BFUIMgr::displayMessage(void *this,uint param_1,int param_2,BFTile *param_3,BFEntity *param_4,bool param_5, bool param_6)

// #[derive(Debug, PartialEq, Eq, FromPrimitive, Clone)]
// #[repr(u32)]
// pub enum ZTEntityTypeClass {
//     Animal = 0x630268,
//     Ambient = 0x62e1e8,
//     Guest = 0x62e330,
//     Fences = 0x63034c,
//     TourGuide = 0x62e8ac,
//     Building = 0x6307e4,
//     Scenery = 0x6303f4,
//     Food = 0x630544,
//     TankFilter = 0x630694,
//     Path = 0x63049c,
//     Rubble = 0x63073c,
//     TankWall = 0x6305ec,
//     Keeper = 0x62e7d8,
//     MaintenanceWorker = 0x62e704,
//     Drt = 0x62e980,
//     #[num_enum(default)]
//     Unknown = 0x0,
// }

// TODO: Impl Store for this, create own macro that ignores the padding OR type alias for the padding with a nop impl of Store
#[derive(Debug, Clone, Copy)]
#[repr(C)]
pub struct BFTile {
    padding: [u8; 0x34],
    pub pos: IVec3, // 0x034 + 0xc
    padding_2: [u8; 0x40],
    unknown_byte_1: u8,     // 0x080
    pub unknown_byte_2: u8, // 0x081
    unknown_byte_3: u8,     // 0x082
    unknown_byte_4: u8,     // 0x083
    unknown_byte_5: u8,     // 0x084
    padding_3: [u8; 0x8],   // Full size 0x8c
}

impl PartialEq for BFTile {
    fn eq(&self, other: &Self) -> bool {
        self.pos.x == other.pos.x && self.pos.y == other.pos.y
    }
}

impl fmt::Display for BFTile {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "BFTile {{ x: {}, y: {} }}", self.pos.x, self.pos.y)
    }
}

impl BFTile {
    pub fn new(pos: IVec3, unknown_byte_2: u8) -> Self {
        BFTile {
            padding: [0; 0x34],
            pos,
            padding_2: [0; 0x40],
            unknown_byte_1: 0,
            unknown_byte_2,
            unknown_byte_3: 0,
            unknown_byte_4: 0,
            unknown_byte_5: 0,
            padding_3: [0; 0x8],
        }
    }

    pub fn get_local_elevation(&self, pos: IVec3) -> i32 {
        // Helper function to perform integer division by 64
        // Equivalent to: 16 * val / 64
        fn scale_and_divide(val: i32) -> i32 {
            16 * val / 64
        }

        // Helper function for negative scaling with proper rounding
        // Equivalent to: ((((-16 * val) >> 31) & 0x3F) - 16 * val) >> 6
        fn negative_scale_and_divide(val: i32) -> i32 {
            let neg_scaled = -16 * val;
            ((neg_scaled >> 31) & (0x3f + neg_scaled)) >> 6
        }

        let x = pos.x;
        let y = pos.y;

        match self.unknown_byte_2 {
            0x1 => {
                if 64 - x > y {
                    // LABEL_12: return 16 * (-x) / 64 - 16 * y / 64
                    scale_and_divide(-x) - scale_and_divide(y)
                } else {
                    -16
                }
            }

            0x4 => {
                if x >= y {
                    0
                } else {
                    scale_and_divide(y) - scale_and_divide(x)
                }
            }

            0x5 => negative_scale_and_divide(x),

            0x10 => {
                if 64 - x >= y {
                    0
                } else {
                    scale_and_divide(y) + scale_and_divide(x) - 16
                }
            }

            0x11 => {
                if 64 - x <= y {
                    scale_and_divide(y) + scale_and_divide(x) - 32
                } else {
                    // LABEL_12: return 16 * (-x) / 64 - 16 * y / 64
                    scale_and_divide(-x) - scale_and_divide(y)
                }
            }

            0x14 => scale_and_divide(y),

            0x15 => {
                if x <= y {
                    0
                } else {
                    scale_and_divide(y) - scale_and_divide(x)
                }
            }

            0x19 => scale_and_divide(y) - scale_and_divide(x),

            0x40 => {
                if x <= y {
                    0
                } else {
                    // LABEL_17: return 16 * x / 64 - 16 * y / 64
                    scale_and_divide(x) - scale_and_divide(y)
                }
            }

            0x41 => negative_scale_and_divide(y),

            0x44 => {
                if x >= y {
                    // LABEL_17: return 16 * x / 64 - 16 * y / 64
                    scale_and_divide(x) - scale_and_divide(y)
                } else {
                    scale_and_divide(y) - scale_and_divide(x)
                }
            }

            0x45 => {
                if 64 - x >= y {
                    0
                } else {
                    negative_scale_and_divide(x) - scale_and_divide(y) + 16
                }
            }

            0x46 => scale_and_divide(-x) - scale_and_divide(y),

            0x50 => scale_and_divide(x),

            0x51 => {
                if x >= y {
                    0
                } else {
                    // LABEL_17: return 16 * x / 64 - 16 * y / 64
                    scale_and_divide(x) - scale_and_divide(y)
                }
            }

            0x54 => {
                if 64 - x > y {
                    scale_and_divide(x) + scale_and_divide(y)
                } else {
                    16
                }
            }

            0x64 => scale_and_divide(x) + scale_and_divide(y), // Note: was 100 in original, but 0x64 = 100

            0x91 => scale_and_divide(x) - scale_and_divide(y),

            _ => 0,
        }
    }
}

#[detour_mod]
pub mod zoo_ztmapview {
    use tracing::info;

    use crate::util::get_from_memory;
    use crate::ztmapview::{BFTile, ErrorStringId, ZTMapView};
    use crate::ztworldmgr::IVec3;
    use openzt_detour::generated::bftile::GET_LOCAL_ELEVATION;
    use openzt_detour::generated::ztmapview::CHECK_TANK_PLACEMENT;

    // use crate::{
    //     bfregistry::{add_to_registry, get_from_registry},
    //     util::{get_from_memory, get_string_from_memory},
    // };

    //004df688
    #[detour(CHECK_TANK_PLACEMENT)]
    // fn check_tank_placement(ZTMapView *other_this, BFEntity *param_2, BFTile *param_3, int *param_4)
    unsafe extern "stdcall" fn check_tank_placement(temp_entity_ptr: u32, tile: u32, response_ptr: *mut u32) -> bool {
        let result = unsafe { CHECK_TANK_PLACEMENT_DETOUR.call(temp_entity_ptr, tile, response_ptr) };

        // let entity = get_from_memory(temp_entity);

        let bf_tile = get_from_memory::<BFTile>(tile);

        // let zt_map_view = get_from_memory::<ZTMapView>(_this);

        if let Err(reimplemented_result) = ZTMapView::check_tank_placement(temp_entity_ptr, &bf_tile) {
            if reimplemented_result == ErrorStringId::from(unsafe { *response_ptr }) {
                info!("ZTMapView::checkTankPlacement success {:?}", reimplemented_result);
            } else {
                info!("Fail {:?}", ErrorStringId::from(unsafe { *response_ptr }));
            }
            // info!("ZTMapView::checkTankPlacement 1 -> {:?}", reimplemented_result);
        } else {
            info!("ZTMapView::checkTankPlacement 0 -> 0");
        }

        // info!("ZTMapView::checkTankPlacement {}, {:p} -> {:#x}", result, response_ptr, unsafe{*response_ptr});
        result
        // true
    }

    // #[hook(unsafe extern "thiscall" BFUIMgr_display_message, offset = 0x0009ccc3)]
    // fn prt_get(_this_prt: u32, param_1: u32, param_2: i32, param_3: u32, param_4: u32, param_5: bool, param_6: bool) {
    //     info!("BFUIMgr::displayMessage called with params: {}, {}, {}, {}, {}, {}", param_1, param_2, param_3, param_4, param_5, param_6);
    //     unsafe { BFUIMgr_display_message.call(_this_prt, param_1, param_2, param_3, param_4, param_5, param_6) };
    // }

    // 0040f24d int __thiscall OOAnalyzer::BFTile::getLocalElevation(BFTile *this,BFPos *param_1)
    #[detour(GET_LOCAL_ELEVATION)]
    unsafe extern "thiscall" fn get_local_elevation(_this: u32, pos: u32) -> i32 {
        let tile = get_from_memory::<BFTile>(_this);
        let pos_vec = get_from_memory::<IVec3>(pos);
        tile.get_local_elevation(pos_vec)
    }
}

pub fn init() {
    if let Err(e) = unsafe { zoo_ztmapview::init_detours() } {
        info!("Error initialising zoo_ztmapview detours: {}", e);
    };
}

pub struct ZTMapView {
    pad: [u8; 0x5ec], // Not currently using this struct, so just padding it out to the size of the class
}

#[derive(Debug, PartialEq, Eq, FromPrimitive, Clone)]
#[repr(u32)]
pub enum ErrorStringId {
    #[default]
    UnknownError = 0x0000, // Unknown error
    ObjectTooCloseToLadderOrPlatform = 0x2942,    // Objects cannot be placed too close to the ladder or platform of the tank.
    ObjectCannotBePlacedInTank = 0x293f,          // This object can not be placed in a tank.
    ObjectMustBePlacedInADeeperTank = 0x2940,     // This object must be placed in a deeper tank.
    EggsMustBePlacedOnLand = 0x294a,              // Eggs must be placed on land.
    AnimalMustBePlacedInATankWithWater = 0x293a,  // This animal must be placed in a tank with water in it.
    AnimalMustBePlacedOnLand = 0x2939,            // This animal doesn't swim in tanks and must be placed in a land exhibit.
    StaffCannotBePlacedInTank = 0x2943,           // This staff member can not be placed in a tank.
    ShowObjectMustBePlacedInShowTank = 0x293e,    // Show objects can only be placed in show exhibits.
    ObjectCannotBePlacedAgainstTankWall = 0x2967, // This object cannot be placed next to a tank wall.
    ObjectCannotBePlacedInShowTank = 0x2941,      // Only show toys and animals that can do tricks can be placed in show tanks.
}

impl ZTMapView {
    pub fn check_tank_placement(temp_entity_ptr: u32, tile: &BFTile) -> Result<(), ErrorStringId> {
        info!("Entity Ptr {:#x} -> {:#x}", temp_entity_ptr, get_from_memory::<u32>(temp_entity_ptr));
        let temp_entity: BFEntity = get_from_memory(temp_entity_ptr);
        let habitat_mgr = read_zt_habitat_mgr_from_memory();
        let Some(habitat) = habitat_mgr.get_habitat(tile.pos.x, tile.pos.y) else {
            info!("No habitat found at tile position: {:?}", tile.pos);
            return Ok(());
        };
        let entity_type_class = temp_entity.entity_type_class();
        info!("Checking tank placement for entity type class: {:?}", entity_type_class);
        if !zt_entity_type_class_is(&entity_type_class, &ZTEntityTypeClass::Keeper) {
            info!("Not keeper, checking gate tile");
            if let Some(t) = habitat.get_gate_tile_in() {
                if temp_entity.is_on_tile(&t) {
                    return Err(ErrorStringId::ObjectTooCloseToLadderOrPlatform);
                }
            }
        }
        // TODO: Fix this, currently we are trying to read a ZTSceneryType from the address of a ZTScenery entity instance
        // if zt_entity_type_class_is(&entity_type_class, &ZTEntityTypeClass::Scenery) {
        //     let scenery_entity = match checked_get_from_memory::<ZTSceneryType>(temp_entity_ptr) {
        //         Ok(entity) => entity,
        //         Err(e) => {
        //             panic!("Failed to get ZTSceneryType from memory for entity at ptr: {:#x}, error: {}", temp_entity_ptr, e);
        //         }
        //     };

        //     if !scenery_entity.underwater || !scenery_entity.surface {
        //         return Err(ErrorStringId::ObjectCannotBePlacedInTank);
        //     }
        // }
        // match  {
        //     ZTEntityTypeClass::ZTKeeper
        // }

        Ok(())
    }
}
