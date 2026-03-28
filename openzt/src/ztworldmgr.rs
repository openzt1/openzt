use getset::Getters;
use itertools::Itertools;
use num_enum::FromPrimitive;
use openzt_detour_macro::detour_mod;
use std::cmp::max;
use std::str::FromStr;
use std::{collections::HashMap, fmt};
use tracing::{error, info};

use crate::bfentitytype::{ZTAnimalType, ZTEntityTypeClass, ZTUnitType};
use crate::shortcuts::M;
use crate::util::{ZTBufferString, mut_from_memory};
use crate::ztmapview::BFTile;
use crate::{
    bfentitytype::{read_zt_entity_type_from_memory, BFEntityType, ZTEntityType, ZTSceneryType},
    command_console::CommandError,
    globals::globals,
    lua_fn,
    util::{get_from_memory, get_string_from_memory, map_from_memory, ref_from_memory},
};


#[derive(Debug, PartialEq, Eq, FromPrimitive, Clone)]
#[repr(u32)]
pub enum ZTEntityClass {
    Food = 0x62dd08,
    Path = 0x62da88,
    Fences = 0x62d808,
    Building = 0x62e0b0,
    Animal = 0x62ff54,
    Guest = 0x630f88,
    Scenery = 0x62d950,
    Keeper = 0x62f3e4,
    MaintenanceWorker = 0x62ea54,
    TourGuide = 0x62f714,
    Drt = 0x62f0b4,
    Ambient = 0x62d6ec,
    Rubble = 0x62df78,
    TankWall = 0x62dbc0,
    TankFilter = 0x62de40,
    #[num_enum(default)]
    Unknown = 0x0,
}

impl std::str::FromStr for ZTEntityClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "food" => Ok(ZTEntityClass::Food),
            "path" => Ok(ZTEntityClass::Path),
            "fences" => Ok(ZTEntityClass::Fences),
            "building" => Ok(ZTEntityClass::Building),
            "animal" => Ok(ZTEntityClass::Animal),
            "guest" => Ok(ZTEntityClass::Guest),
            "scenery" => Ok(ZTEntityClass::Scenery),
            "keeper" => Ok(ZTEntityClass::Keeper),
            "maintenanceworker" => Ok(ZTEntityClass::MaintenanceWorker),
            "tourguide" => Ok(ZTEntityClass::TourGuide),
            "drt" => Ok(ZTEntityClass::Drt),
            "ambient" => Ok(ZTEntityClass::Ambient),
            "rubble" => Ok(ZTEntityClass::Rubble),
            "tankwall" => Ok(ZTEntityClass::TankWall),
            "tankfilter" => Ok(ZTEntityClass::TankFilter),
            _ => Err(format!("Unknown entity type: {}", s)),
        }
    }
}

// TODO: Make this look like other structs with proper offsets and padding ->
#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
pub struct ZTEntity {
    vtable: u32,
    // Technically, the first 0x154 bytes are BFEntity, should grab what Eric started doing in his PR and embed BFEntity here
    class: ZTEntityClass,
    type_class: ZTEntityType, // TODO: Change to &ZTEntityType at some point?
    name: String,
    pos1: u32,
    pos2: u32,
}

#[derive(Debug)]
struct ZTEntityWithPtr {
    ptr: u32,
    entity: ZTEntity,
}

#[derive(Debug)]
struct ZTEntityTypeWithPtr {
    ptr: u32,
    entity_type: ZTEntityType,
}

// Move to util or use existing implementation
#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct IVec3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl IVec3 {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        IVec3 { x, y, z }
    }
}

impl fmt::Display for IVec3 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Vec3 {{ x: {}, y: {}, z: {} }}", self.x, self.y, self.z)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default)]
pub struct Rectangle {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Rectangle {
    fn contains_point(&self, point: &IVec3) -> bool {
        point.x >= self.min_x && point.x <= self.max_x && point.y >= self.min_y && point.y <= self.max_y
    }
}

// zt_type: get_string_from_memory(get_from_memory::<u32>(zt_entity_type_ptr + 0x98)),
// zt_sub_type: get_string_from_memory(get_from_memory::<u32>(zt_entity_type_ptr + 0xa4)),
// bf_config_file_ptr: get_from_memory::<u32>(zt_entity_type_ptr + 0x80),

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
pub struct BFEntity { // Full size is 0x154 bytes
    vtable: u32,
    padding: [u8; 0x104],
    name: ZTBufferString,      // 0x108
    pos: IVec3,              // 0x114
    height_above_terrain: u32, // 0x120
    padding4: [u8; 0x4],       // ----- padding: 4 bytes
    inner_class_ptr: u32,      // 0x128
    rotation: i32,             // 0x12c
    padding_2: [u8; 0xc],      // ----- padding: 28 bytes
    unknown_flag1: u8,         // 0x13c // isRemoved
    unknown_flag2: u8,         // 0x13d // isRemovedUndo
    unknown_flag3: u8,         // 0x13e
    visible: u8,               // 0x13f
    snap_to_ground: u8,        // 0x140
    selected: u8,              // 0x141
    unknown_flag4: u8,         // 0x142 // Moving? Programmatically?
    unknown_flag5: u8,         // 0x143 // Picked up?
    draw_dithered: u8,         // 0x144
    unknown_flag6: u8,         // 0x145 // If != 0; Draw selection graphic
    stop_at_end: u8,           // 0x146
    padding_3: [u8; 0x9],      // ----- padding: 10 bytes
    map_footprint: i32,        // 0x150
}

impl fmt::Display for BFEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BFEntity {{ name: {}, x_coord: {}, y_coord: {}, z_coord: {}, height_above_terrain: {}, rotation: {}, inner_class_ptr: {:#x}, visible: {}, snap_to_ground: {}, selected: {}, draw_dithered: {} }}",
            self.name, self.pos.x, self.pos.y, self.pos.z, self.height_above_terrain, self.rotation, self.inner_class_ptr, self.visible, self.snap_to_ground, self.selected, self.draw_dithered
        )
    }
}

impl BFEntity {
    pub fn entity_type_class(&self) -> ZTEntityTypeClass {
        ZTEntityTypeClass::from(get_from_memory::<u32>(self.inner_class_ptr))
    }

    pub fn entity_type(&self) -> &'static BFEntityType {
        unsafe { ref_from_memory(self.inner_class_ptr) }
    }

    // TODO: Hook this and check that it works
    pub fn is_on_tile(&self, tile: &BFTile) -> bool {
        let Some(entity_tile) = self.get_tile() else {
            error!("BFEntity::is_on_tile: Entity {} has no tile", self.name);
            return false;
        };
        if entity_tile == *tile {
            return true;
        }

        let rect = self.get_blocking_rect();
        let tile_size = IVec3 { x: 0x20, y: 0x20, z: 0 };
        rect.contains_point(&globals().ztworldmgr().tile_to_world(tile.pos, tile_size))
    }

    pub fn get_blocking_rect(&self) -> Rectangle {
        // TODO: We shouldn't need the first check
        // Transient entities don't block anything
        if self.inner_class_ptr == 0 || self.entity_type().is_transient || self.entity_type_class() == ZTEntityTypeClass::Path {
            return Rectangle::default(); // Zero rectangle
        }

        let mut footprint = self.vtable_get_footprint();

        if self.rotation % 2 != 0 {
            let max = max(footprint.x, footprint.y);
            footprint.x = max;
            footprint.y = max;
        }

        // Calculate half-dimensions for easier rectangle construction
        let half_width = (footprint.x * 32) / 2; // Scaling factor preserved from original
        let half_height = (footprint.y * 32) / 2;

        // Construct and return the rectangle
        Rectangle {
            min_x: self.pos.x - half_width,
            min_y: self.pos.y - half_height,
            max_x: self.pos.x + half_width,
            max_y: self.pos.y + half_height,
        }
    }

    fn vtable_get_footprint(&self) -> IVec3 {
        let function_address = get_from_memory::<u32>(self.vtable + 0x94);
        let get_footprint_fn =
            unsafe { std::mem::transmute::<u32, extern "thiscall" fn(this: &BFEntity, param_1: &mut IVec3, param_2: u32) -> u32>(function_address) };
        let mut result_footprint = IVec3::default();
        let footprint_ptr = get_footprint_fn(self, &mut result_footprint, 0);
        get_from_memory::<IVec3>(footprint_ptr)
    }

    pub fn get_footprint(&self, _use_map_footprint: bool) -> IVec3 {
        let entity_type = self.entity_type();
        if self.rotation % 4 == 0 {
            IVec3 {
                x: entity_type.footprintx,
                y: entity_type.footprinty,
                z: entity_type.footprintz,
            }
        } else {
            IVec3 {
                x: entity_type.footprinty,
                y: entity_type.footprintx,
                z: entity_type.footprintz,
            }
        }
    }

    pub fn get_tile(&self) -> Option<BFTile> {
        globals().ztworldmgr().get_tile_from_coords(self.pos.x, self.pos.y)
    }
}

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
struct BFUnit {
    base: BFEntity, // bytes: 0x154 = 340 bytes
    // TODO
    padding: [u8; 0x214-0x154], // ----- padding: 192 bytes
}


impl std::ops::Deref for BFUnit {
    type Target = BFEntity;
    fn deref(&self) -> &BFEntity {
        &self.base
    }
}

impl std::ops::DerefMut for BFUnit {
    fn deref_mut(&mut self) -> &mut BFEntity {
        &mut self.base
    }
}

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
struct ZTUnit {
    base: BFUnit, // bytes: 0x214 = 532 bytes
    padding: [u8; 0x260-0x214], // ----- padding: 76 bytes
}

impl ZTUnit {

    pub fn entity_type(&self) -> &'static ZTUnitType {
        unsafe { ref_from_memory(self.inner_class_ptr) }
    }

    pub fn get_footprint(&self, use_map_footprint: bool) -> IVec3 {
        if !use_map_footprint {
            self.base.get_footprint(use_map_footprint)
        } else {
            IVec3 {
                x: self.map_footprint as i32,
                y: self.map_footprint as i32,
                z: 0,
            }
        }
    }
}

impl std::ops::Deref for ZTUnit {
    type Target = BFUnit;
    fn deref(&self) -> &BFUnit {
        &self.base
    }
}

impl std::ops::DerefMut for ZTUnit {
    fn deref_mut(&mut self) -> &mut BFUnit {
        &mut self.base
    }
}

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
struct ZTAnimal {
    base: ZTUnit,  // offset: 0x0000
    _pad_0x0260: [u8; 300],
    food_tile: *const BFTile,  // offset: 0x038c
    _pad_0x0390: [u8; 4],
    is_boxed: bool,  // offset: 0x0394
    is_egg: bool,  // offset: 0x0395
    _pad_0x0396: [u8; 6],
    mbr_0x39c: u8,  // offset: 0x039c
    mbr_0x3a0: u8,  // offset: 0x03a0
    mbr_0x3a4: i8,  // offset: 0x03a4
    mbr_0x3a5: i8,  // offset: 0x03a5
    is_dying: bool,  // offset: 0x03a6
    mbr_0x3a7: i8,  // offset: 0x03a7
    mbr_0x3a8: u8,  // offset: 0x03a8
    mbr_0x3ac: u8,  // offset: 0x03ac
    mbr_0x3b0: u8,  // offset: 0x03b0
    mbr_0x3b4: u8,  // offset: 0x03b4
}

impl ZTAnimal {

    pub fn entity_type(&self) -> &'static ZTAnimalType {
        unsafe { ref_from_memory(self.inner_class_ptr) }
    }

    pub fn get_footprint(&self, use_map_footprint: bool) -> IVec3 {
        if !self.is_egg && !self.is_boxed {
            return self.base.get_footprint(use_map_footprint);
        }

        let type_info = self.entity_type();
        let footprint = if self.is_egg {
            type_info.egg_footprint
        } else {
            type_info.box_footprint
        };

        if self.rotation % 4 == 0 {
            IVec3 { x: footprint.y, y: footprint.x, z: footprint.z }
        } else {
            IVec3 { x: footprint.x, y: footprint.y, z: footprint.z }
        }
    }
}

impl std::ops::Deref for ZTAnimal {
    type Target = ZTUnit;
    fn deref(&self) -> &ZTUnit {
        &self.base
    }
}

impl std::ops::DerefMut for ZTAnimal {
    fn deref_mut(&mut self) -> &mut ZTUnit {
        &mut self.base
    }
}


// ZTAnimal -> 0x3a6 = animalDying

impl ZTEntity {
    pub fn is_member(&self, member: String) -> bool {
        self.type_class.is_member(member)
    }
}

impl fmt::Display for ZTEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Entity Type: {:?}, Name: {}, EntityType {} ({},{}) ({},{})",
            self.class,
            self.name,
            self.type_class,
            self.pos1,
            self.pos2,
            self.pos1 >> 6,
            self.pos2 >> 6
        )
    }
}

#[derive(Debug)]
#[repr(C)]
pub struct ZTWorldMgr {
    padding_1: [u8; 0x34],
    map_x_size: u32,
    map_y_size: u32,
    padding_2: [u8; 0x4],
    tile_array: u32,
    padding_3: [u8; 0x3c],
    entity_array_start: u32,
    entity_array_end: u32,
    entity_array_buffer_end: u32,
    padding_4: [u8; 0xc],
    entity_type_array_start: u32,
    entity_type_array_end: u32,
    entity_type_array_buffer_end: u32,
}

impl fmt::Display for ZTWorldMgr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ZTWorldMgr {{ map_x_size: {}, map_y_size: {}, tile_array: {:#x}, entity_array_start: {:#x}, entity_array_end: {:#x}, entity_type_array_start: {:#x}, entity_type_array_end: {:#x} }}",
            self.map_x_size,
            self.map_y_size,
            self.tile_array,
            self.entity_array_start,
            self.entity_array_end,
            self.entity_type_array_start,
            self.entity_type_array_end,
        )
    }
}

// TODO: Move to util or better named crate
#[derive(Debug, PartialEq, Eq, FromPrimitive, Clone)]
#[repr(u32)]
pub enum Direction {
    #[default]
    West = 0,
    NorthWest = 1,
    North = 2,
    NorthEast = 3,
    East = 4,
    SouthEast = 5,
    South = 6,
    SouthWest = 7,
}

const TILE_SIZE: i32 = 0x40;
const ELEVATION_SCALE: i32 = 0x10; // 16 units per elevation level

impl ZTWorldMgr {
    /// Get the start of the entity array in memory
    pub fn entity_array_start(&self) -> u32 {
        self.entity_array_start
    }

    /// Get the end of the entity array in memory
    pub fn entity_array_end(&self) -> u32 {
        self.entity_array_end
    }

    pub fn get_neighbour(&self, bftile: &BFTile, direction: Direction) -> Option<BFTile> {
        let x_offset: i32 = match direction {
            Direction::West => 0,
            Direction::NorthWest => 1,
            Direction::North => 1,
            Direction::NorthEast => 1,
            Direction::East => 0,
            Direction::SouthEast => -1,
            Direction::South => -1,
            Direction::SouthWest => -1,
        };
        let y_offset: i32 = match direction {
            Direction::West => -1,
            Direction::NorthWest => -1,
            Direction::North => 0,
            Direction::NorthEast => 1,
            Direction::East => 1,
            Direction::SouthEast => 1,
            Direction::South => 0,
            Direction::SouthWest => -1,
        };

        let x: i32 = bftile.pos.x + x_offset;
        let y: i32 = bftile.pos.y + y_offset;

        if x < 0 || x >= self.map_x_size as i32 || y < 0 || y >= self.map_y_size as i32 {
            return None;
        }

        Some(get_from_memory::<BFTile>(self.tile_array + (((y as u32 * self.map_x_size) + x as u32) * 0x8c_u32)))
    }

    pub fn get_ptr_from_bftile(&self, bftile: &BFTile) -> u32 {
        let x = bftile.pos.x as u32;
        let y = bftile.pos.y as u32;
        self.tile_array + ((y * self.map_x_size + x) * 0x8c)
    }

    pub fn get_tile_from_pos(&self, pos: IVec3) -> Option<BFTile> {
        let x = pos.x as u32;
        let y = pos.y as u32;
        if x >= self.map_x_size || y >= self.map_y_size {
            return None;
        }
        Some(get_from_memory::<BFTile>(self.tile_array + ((y * self.map_x_size + x) * 0x8c)))
    }

    pub fn get_tile_from_coords(&self, x_coord: i32, y_coord: i32) -> Option<BFTile> {
        let x = (x_coord as u32) >> 6; // Convert to tile coordinates
        let y = (y_coord as u32) >> 6; // Convert to tile coordinates
        if x >= self.map_x_size || y >= self.map_y_size {
            return None;
        }
        Some(get_from_memory::<BFTile>(self.tile_array + ((y * self.map_x_size + x) * 0x8c)))
    }

    // TODO: Should borrow both of these IVec3s instead of taking ownership
    pub fn tile_to_world(&self, tile_pos: IVec3, local_pos: IVec3) -> IVec3 {
        let tile_x = tile_pos.x;
        let tile_y = tile_pos.y;

        // Get the tile at the specified position, if it exists and is within bounds
        let tile = self.get_tile_from_pos(tile_pos);

        // Calculate elevation based on tile data
        let world_z = match tile {
            Some(tile_ref) => {
                let local_elevation = tile_ref.get_local_elevation(local_pos);
                local_elevation + tile_ref.pos.z * ELEVATION_SCALE
            }
            None => 0,
        };

        // Convert tile coordinates to world coordinates
        IVec3 {
            x: tile_x * TILE_SIZE + local_pos.x,
            y: tile_y * TILE_SIZE + local_pos.y,
            z: world_z,
        }
    }
}

#[detour_mod]
pub mod hooks_ztunit {
    use super::*;

    use crate::util::save_to_memory;
    use openzt_detour::generated::ztunit::GET_FOOTPRINT;

    #[detour(GET_FOOTPRINT)]
    unsafe extern "thiscall" fn ztunit_get_footprint(_this: *const u32, param_1: *const u32, use_map_footprint: bool) -> *const u32 {
        let entity = unsafe { ref_from_memory::<ZTUnit>(_this) };
        let footprint: IVec3 = entity.get_footprint(use_map_footprint);

        save_to_memory(param_1 as u32, footprint.x);
        save_to_memory(param_1 as u32 + 0x4, footprint.y);
        save_to_memory(param_1 as u32 + 0x8, footprint.z);

        param_1
    }
}

#[detour_mod]
pub mod hooks_ztanimal {
    use super::*;

    use crate::util::save_to_memory;
    use openzt_detour::generated::ztanimal::GET_FOOTPRINT;

    #[detour(GET_FOOTPRINT)]
    unsafe extern "thiscall" fn ztanimal_get_footprint(_this: *const u32, param_1: *const u32, use_map_footprint: bool) -> *const u32 {
        let entity = unsafe { ref_from_memory::<ZTAnimal>(_this) };
        let footprint: IVec3 = entity.get_footprint(use_map_footprint);

        save_to_memory(param_1 as u32, footprint.x);
        save_to_memory(param_1 as u32 + 0x4, footprint.y);
        save_to_memory(param_1 as u32 + 0x8, footprint.z);

        param_1
    }
}

#[detour_mod]
pub mod hooks_ztworldmgr {
    use crate::util::save_to_memory;
    use openzt_detour::generated::bfentity::{GET_BLOCKING_RECT, GET_BLOCKING_RECT_VIRT_ZTPATH, GET_FOOTPRINT, IS_ON_TILE};
    use openzt_detour::generated::bfmap::{GET_NEIGHBOR_1, TILE_TO_WORLD};

    use super::*;

    #[detour(GET_NEIGHBOR_1)]
    unsafe extern "thiscall" fn bfmap_get_neighbour(_this: *const u32, bftile: *const u32, direction: u32) -> u32 {
        let ztwm = globals().ztworldmgr();
        let bftile = unsafe { ref_from_memory::<BFTile>(bftile) };
        let direction = Direction::from(direction);
        match ztwm.get_neighbour(bftile, direction) {
            Some(neighbour) => ztwm.get_ptr_from_bftile(&neighbour),
            None => 0,
        }
    }

    // 0x0040f916 int * __thiscall OOAnalyzer::BFEntity::getFootprint(BFEntity *this,undefined4 *param_1)
    #[detour(GET_FOOTPRINT)]
    unsafe extern "thiscall" fn bfentity_get_footprint(_this: *const u32, param_1: *const u32, use_map_footprint: bool) -> *const u32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(_this) };
        let footprint: IVec3 = entity.get_footprint(use_map_footprint);
        save_to_memory(param_1 as u32, footprint.x);
        save_to_memory(param_1 as u32 + 0x4, footprint.y);
        save_to_memory(param_1 as u32 + 0x8, footprint.z);

        param_1
    }

    // 0x0042721a u32 __thiscall OOAnalyzer::BFEntity::getBlockingRect(BFEntity *this,u32 param_1)
    #[detour(GET_BLOCKING_RECT)]
    unsafe extern "thiscall" fn bfentity_get_blocking_rect(_this: *const u32, param_1: *const i32) -> *const u32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(_this) };
        save_to_memory(param_1, entity.get_blocking_rect());
        param_1 as *const u32
    }

    // 0x004fbbee u32 __thiscall OOAnalyzer::BFEntity::getBlockingRect(BFEntity *this,u32 param_1)
    #[detour(GET_BLOCKING_RECT_VIRT_ZTPATH)]
    unsafe extern "thiscall" fn bfentity_get_blocking_rect_ztpath(_this: *const u32, param_1: *const i32) -> *const u32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(_this) };
        save_to_memory(param_1, entity.get_blocking_rect());
        param_1 as *const u32
    }

    // // 0040f26c BFPos * __thiscall OOAnalyzer::BFMap::tileToWorld(BFMap *this,BFPos *param_1,BFPos *param_2,BFPos *param_3)
    #[detour(TILE_TO_WORLD)]
    unsafe extern "thiscall" fn bfmap_tile_to_world(_this: *const u32, param_1: *const i32, param_2: *const i32, param_3: *const i32) -> *const i32 {
        let ztwm = globals().ztworldmgr();
        let tile_pos = get_from_memory::<IVec3>(param_2);
        let local_pos = get_from_memory::<IVec3>(param_3);
        let world_pos = ztwm.tile_to_world(tile_pos, local_pos);
        save_to_memory(param_1, world_pos);
        param_1
    }

    // TODO: Remove this when check_tank_placement is fully implemented
    // 004e16f1 bool __thiscall OOAnalyzer::BFEntity::isOnTile(BFEntity *this,BFTile *param_1)
    #[detour(IS_ON_TILE)]
    unsafe extern "thiscall" fn bfentity_is_on_tile(_this: *const u32, param_1: *const u32) -> bool {
        let result = unsafe { IS_ON_TILE_DETOUR.call(_this, param_1) };
        let entity = unsafe { ref_from_memory::<BFEntity>(_this) };
        let tile = unsafe { ref_from_memory::<BFTile>(param_1) };
        let reimimplented_result = entity.is_on_tile(tile);
        if result != reimimplented_result {
            error!(
                "BFEntity::is_on_tile: Detour result ({}) does not match reimplemented result ({}) for entity {}",
                result, reimimplented_result, entity.name
            );
        }
        reimimplented_result
    }
}

pub fn init() {
    // list_entities([entity_type]) - optional arg
    lua_fn!("list_entities", "Lists all entities in the world", "list_entities([entity_type])",
        |entity_type: Option<String>| {
            let args = entity_type.as_ref().map(|s| vec![s.as_str()]).unwrap_or_default();
            match command_get_zt_world_mgr_entities(args) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // list_entities_2() - no args
    lua_fn!("list_entities_2", "Lists all entities in the world (alternate format)", "list_entities_2()", || {
        match command_get_zt_world_mgr_entities_2(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // read_entity_offset(offset [, type] [, entity_type]) - required arg, optional type and filter
    lua_fn!("read_entity_offset", "Read value at offset from entities (types: ptr, u32, i32, u16, i16, u8, i8, f32, bool)", "read_entity_offset(offset [, type] [, entity_type])",
        |offset: String, type_arg: Option<String>, entity_type: Option<String>| {
            let mut args = vec![offset.as_str()];
            if let Some(t) = &type_arg {
                args.push(t.as_str());
            }
            if let Some(et) = &entity_type {
                args.push(et.as_str());
            }
            match command_read_entity_offset(args) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // read_entity_type_offset(offset [, type] [, entity_type_class]) - required arg, optional type and filter
    lua_fn!("read_entity_type_offset", "Read value at offset from entity types (types: ptr, u32, i32, u16, i16, u8, i8, f32, bool)", "read_entity_type_offset(offset [, type] [, entity_type_class])",
        |offset: String, type_arg: Option<String>, entity_type_class: Option<String>| {
            let mut args = vec![offset.as_str()];
            if let Some(t) = &type_arg {
                args.push(t.as_str());
            }
            if let Some(et) = &entity_type_class {
                args.push(et.as_str());
            }
            match command_read_entity_type_offset(args) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // list_types() - no args
    lua_fn!("list_types", "Lists all entity types in the world", "list_types()", || {
        match command_get_zt_world_mgr_types(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // get_zt_world_mgr() - no args
    lua_fn!("get_zt_world_mgr", "Returns world manager details", "get_zt_world_mgr()", || {
        match command_get_zt_world_mgr(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // get_types_summary() - no args
    lua_fn!("get_types_summary", "Returns summary of all entity types", "get_types_summary()", || {
        match command_zt_world_mgr_types_summary(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // get_entity_vtable_entry(offset) - single string arg
    lua_fn!(
        "get_entity_vtable_entry",
        "Returns unique entity vtable entries at offset",
        "get_entity_vtable_entry(offset)",
        |offset: String| {
            match command_get_entity_unique_vtable_entries(vec![&offset]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // get_entity_type_vtable_entry(offset) - single string arg
    lua_fn!(
        "get_entity_type_vtable_entry",
        "Returns unique entity type vtable entries at offset",
        "get_entity_type_vtable_entry(offset)",
        |offset: String| {
            match command_get_entity_type_unique_vtable_entries(vec![&offset]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    unsafe {
        hooks_ztworldmgr::init_detours().unwrap();
        hooks_ztunit::init_detours().unwrap();
        hooks_ztanimal::init_detours().unwrap();
    };
}

pub fn read_zt_entity_from_memory(zt_entity_ptr: u32) -> ZTEntity {
    let inner_class_ptr = get_from_memory::<u32>(zt_entity_ptr + 0x128);

    ZTEntity {
        vtable: get_from_memory::<u32>(zt_entity_ptr),
        class: ZTEntityClass::from(get_from_memory::<u32>(zt_entity_ptr)),
        type_class: read_zt_entity_type_from_memory(inner_class_ptr),
        name: get_string_from_memory(get_from_memory::<u32>(zt_entity_ptr + 0x108)),
        pos1: get_from_memory::<u32>(zt_entity_ptr + 0x114),
        pos2: get_from_memory::<u32>(zt_entity_ptr + 0x118),
    }
}


fn log_zt_world_mgr(zt_world_mgr: &ZTWorldMgr) {
    info!("zt_world_mgr: {:#?}", zt_world_mgr);
}

fn command_get_zt_world_mgr_entities(args: Vec<&str>) -> Result<String, CommandError> {
    let filter = if args.len() > 1 {
        return Err(CommandError::new("Too many arguments".to_string()));
    } else if args.len() == 1 {
        Some(args[0].parse::<ZTEntityClass>().map_err(|e| CommandError::new(e))?)
    } else {
        None
    };

    let zt_world_mgr = globals().ztworldmgr();
    let entities = get_zt_world_mgr_entities(zt_world_mgr);

    let filtered: Vec<_> = if let Some(ref entity_type) = filter {
        entities.iter().filter(|e| e.entity.class == *entity_type).collect()
    } else {
        entities.iter().collect()
    };

    info!("Found {} entities (filtered from {})", filtered.len(), entities.len());
    if filtered.is_empty() {
        return Ok("No entities found".to_string());
    }

    let mut string_array = Vec::new();
    for ewp in filtered {
        string_array.push(ewp.entity.to_string());
    }
    Ok(string_array.join("\n"))
}

fn command_read_entity_offset(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() < 1 || args.len() > 3 {
        return Err(CommandError::new("Usage: read_entity_offset(offset [, type] [, entity_type])".to_string()));
    }

    // Parse offset
    let offset = match args[0].strip_prefix("0x") {
        Some(hex_str) => u32::from_str_radix(hex_str, 16).map_err(|e| CommandError::new(format!("Invalid offset: {}", e)))?,
        None => args[0].parse::<u32>().map_err(|e| CommandError::new(format!("Invalid offset: {}", e)))?,
    };

    // Valid types
    let valid_types = ["ptr", "u32", "i32", "u16", "i16", "u8", "i8", "f32", "bool"];

    // Parse optional type and entity type filter
    // args[1] could be either type or entity_type
    // args[2] would be entity_type if args[1] was type
    let (type_str, filter) = match args.len() {
        1 => ("u32", None), // default
        2 => {
            // Check if args[1] is a valid type
            if valid_types.contains(&args[1]) {
                (args[1], None)
            } else {
                // args[1] is entity_type
                ("u32", Some(args[1].parse::<ZTEntityClass>().map_err(|e| CommandError::new(e))?))
            }
        }
        3 => {
            // args[1] is type, args[2] is entity_type
            if !valid_types.contains(&args[1]) {
                return Err(CommandError::new(format!("Invalid type: {}. Valid types: {}", args[1], valid_types.join(", "))));
            }
            (args[1], Some(args[2].parse::<ZTEntityClass>().map_err(|e| CommandError::new(e))?))
        }
        _ => unreachable!(),
    };

    let zt_world_mgr = globals().ztworldmgr();
    let entities = get_zt_world_mgr_entities(zt_world_mgr);

    let filtered: Vec<_> = if let Some(ref entity_type) = filter {
        entities.iter().filter(|e| e.entity.class == *entity_type).collect()
    } else {
        entities.iter().collect()
    };

    if filtered.is_empty() {
        return Ok("No entities found".to_string());
    }

    let mut string_array = Vec::new();
    for ewp in filtered {
        let value_str = match type_str {
            "ptr" => format!("{:#x}", get_from_memory::<u32>(ewp.ptr + offset)),
            "u32" => format!("{}", get_from_memory::<u32>(ewp.ptr + offset)),
            "i32" => format!("{}", get_from_memory::<i32>(ewp.ptr + offset)),
            "u16" => format!("{}", get_from_memory::<u16>(ewp.ptr + offset)),
            "i16" => format!("{}", get_from_memory::<i16>(ewp.ptr + offset)),
            "u8" => format!("{}", get_from_memory::<u8>(ewp.ptr + offset)),
            "i8" => format!("{}", get_from_memory::<i8>(ewp.ptr + offset)),
            "f32" => format!("{}", get_from_memory::<f32>(ewp.ptr + offset)),
            "bool" => format!("{}", get_from_memory::<bool>(ewp.ptr + offset)),
            _ => unreachable!(),
        };

        if filter.is_some() {
            // Filtered: addr, name, value only
            string_array.push(format!("{:#x} | {} | {}", ewp.ptr, ewp.entity.name, value_str));
        } else {
            // No filter: mem address, entity name, type, value
            string_array.push(format!("{:#x} | {} | {:?} | {}", ewp.ptr, ewp.entity.name, ewp.entity.class, value_str));
        }
    }
    Ok(string_array.join("\n"))
}

fn command_read_entity_type_offset(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() < 1 || args.len() > 3 {
        return Err(CommandError::new("Usage: read_entity_type_offset(offset [, type] [, entity_type_class])".to_string()));
    }

    // Parse offset
    let offset = match args[0].strip_prefix("0x") {
        Some(hex_str) => u32::from_str_radix(hex_str, 16).map_err(|e| CommandError::new(format!("Invalid offset: {}", e)))?,
        None => args[0].parse::<u32>().map_err(|e| CommandError::new(format!("Invalid offset: {}", e)))?,
    };

    // Valid types
    let valid_types = ["ptr", "u32", "i32", "u16", "i16", "u8", "i8", "f32", "bool"];

    // Parse optional type and entity type class filter
    let (type_str, filter) = match args.len() {
        1 => ("u32", None), // default
        2 => {
            // Check if args[1] is a valid type
            if valid_types.contains(&args[1]) {
                (args[1], None)
            } else {
                // args[1] is entity_type_class
                ("u32", Some(args[1].parse::<ZTEntityTypeClass>().map_err(|e| CommandError::new(e))?))
            }
        }
        3 => {
            // args[1] is type, args[2] is entity_type_class
            if !valid_types.contains(&args[1]) {
                return Err(CommandError::new(format!("Invalid type: {}. Valid types: {}", args[1], valid_types.join(", "))));
            }
            (args[1], Some(args[2].parse::<ZTEntityTypeClass>().map_err(|e| CommandError::new(e))?))
        }
        _ => unreachable!(),
    };

    let zt_world_mgr = globals().ztworldmgr();
    let entity_types = get_zt_world_mgr_types(zt_world_mgr);

    let filtered: Vec<_> = if let Some(ref entity_type_class) = filter {
        entity_types.iter().filter(|et| et.entity_type.class == *entity_type_class).collect()
    } else {
        entity_types.iter().collect()
    };

    if filtered.is_empty() {
        return Ok("No entity types found".to_string());
    }

    let mut string_array = Vec::new();
    for etwp in filtered {
        let value_str = match type_str {
            "ptr" => format!("{:#x}", get_from_memory::<u32>(etwp.ptr + offset)),
            "u32" => format!("{}", get_from_memory::<u32>(etwp.ptr + offset)),
            "i32" => format!("{}", get_from_memory::<i32>(etwp.ptr + offset)),
            "u16" => format!("{}", get_from_memory::<u16>(etwp.ptr + offset)),
            "i16" => format!("{}", get_from_memory::<i16>(etwp.ptr + offset)),
            "u8" => format!("{}", get_from_memory::<u8>(etwp.ptr + offset)),
            "i8" => format!("{}", get_from_memory::<i8>(etwp.ptr + offset)),
            "f32" => format!("{}", get_from_memory::<f32>(etwp.ptr + offset)),
            "bool" => format!("{}", get_from_memory::<bool>(etwp.ptr + offset)),
            _ => unreachable!(),
        };

        if filter.is_some() {
            // Filtered: addr, type, subtype, value only
            string_array.push(format!("{:#x} | {} | {} | {}", etwp.ptr, etwp.entity_type.zt_type, etwp.entity_type.zt_sub_type, value_str));
        } else {
            // No filter: mem address, type, subtype, class, value
            string_array.push(format!("{:#x} | {} | {} | {:?} | {}", etwp.ptr, etwp.entity_type.zt_type, etwp.entity_type.zt_sub_type, etwp.entity_type.class, value_str));
        }
    }
    Ok(string_array.join("\n"))
}

fn command_get_zt_world_mgr_entities_2(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = globals().ztworldmgr();
    let entities = get_zt_world_mgr_entities_2(zt_world_mgr);
    info!("Found {} entities", entities.len());
    if entities.is_empty() {
        return Ok("No entities found".to_string());
    }
    let mut string_array = Vec::new();
    for entity in entities {
        string_array.push(entity.to_string());
    }
    Ok(string_array.join("\n"))
}

// TODO: Both below commands should use a static list of EntityVtables and EntityTypeVtables (or just make a command in Ghidra?)
fn command_get_entity_unique_vtable_entries(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() != 1 {
        return Err(CommandError::new("Vtable offset required".to_string()));
    }

    let vtable_offset = match args[0].strip_prefix("0x") {
        Some(hex_str) => u32::from_str_radix(hex_str, 16)?,
        None => u32::from_str(args[0])?,
    };

    let zt_world_mgr = globals().ztworldmgr();
    let entities = get_zt_world_mgr_entities(zt_world_mgr);

    let mut result = String::new();

    entities
        .iter()
        .map(|ewp| (ewp.entity.type_class.class.clone(), ewp.entity.vtable + vtable_offset))
        .unique_by(|t| t.1)
        .for_each(|(type_name, vfunc_ptr)| {
            result.push_str(&format!("{:?} -> {:#x}\n", type_name, get_from_memory::<u32>(vfunc_ptr)));
        });

    Ok(result)
}

fn command_get_entity_type_unique_vtable_entries(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() != 1 {
        return Err(CommandError::new("Vtable offset required".to_string()));
    }

    let vtable_offset = match args[0].strip_prefix("0x") {
        Some(hex_str) => u32::from_str_radix(hex_str, 16)?,
        None => u32::from_str(args[0])?,
    };

    let zt_world_mgr = globals().ztworldmgr();
    let entity_types = get_zt_world_mgr_types(zt_world_mgr);

    let mut result = String::new();

    entity_types
        .iter()
        .map(|etwp| (etwp.entity_type.class.clone(), etwp.entity_type.vtable + vtable_offset))
        .unique_by(|t| t.1)
        .for_each(|(type_name, vfunc_ptr)| {
            result.push_str(&format!("{:?} -> {:#x}\n", type_name, get_from_memory::<u32>(vfunc_ptr)));
        });

    Ok(result)
}

fn command_get_zt_world_mgr_types(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = globals().ztworldmgr();
    let types = get_zt_world_mgr_types(zt_world_mgr);
    info!("Found {} types", types.len());
    if types.is_empty() {
        return Ok("No types found".to_string());
    }
    let mut string_array = Vec::new();
    for etwp in types {
        string_array.push(etwp.entity_type.to_string());
    }
    Ok(string_array.join("\n"))
}

fn command_get_zt_world_mgr(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = globals().ztworldmgr();
    Ok(zt_world_mgr.to_string())
}

fn command_zt_world_mgr_types_summary(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = globals().ztworldmgr();
    let types = get_zt_world_mgr_types(zt_world_mgr);
    let mut summary = "\n".to_string();
    let mut subtype: HashMap<String, u32> = HashMap::new();
    if types.is_empty() {
        return Ok("No types found".to_string());
    }
    let mut current_class = types[0].entity_type.class.clone();
    for etwp in types {
        if current_class != etwp.entity_type.class {
            let mut string_array = Vec::new();
            let mut total = 0;
            for (class, count) in subtype {
                string_array.push(format!("\t{:?}: {}", class, count));
                total += count;
            }
            summary.push_str(&format!("{:?}: ({})\n{}\n", current_class, total, string_array.join("\n")));
            info!("{:?}: ({})\n{}", current_class, total, string_array.join("\n"));
            subtype = HashMap::new();
            current_class = etwp.entity_type.class.clone();
        }
        info!("{:?}, {}", current_class, etwp.entity_type.zt_type);
        let count = subtype.entry(etwp.entity_type.zt_type.clone()).or_insert(0);
        *count += 1;
    }
    Ok(summary)
}

fn get_zt_world_mgr_entities(zt_world_mgr: &ZTWorldMgr) -> Vec<ZTEntityWithPtr> {
    let entity_array_start = zt_world_mgr.entity_array_start;
    let entity_array_end = zt_world_mgr.entity_array_end;

    let mut entities: Vec<ZTEntityWithPtr> = Vec::new();
    let mut i = entity_array_start;
    while i < entity_array_end {
        let entity_ptr = get_from_memory::<u32>(i);
        let zt_entity = read_zt_entity_from_memory(entity_ptr);
        entities.push(ZTEntityWithPtr { ptr: entity_ptr, entity: zt_entity });
        i += 0x4;
    }
    entities
}

fn get_zt_world_mgr_entities_2(zt_world_mgr: &ZTWorldMgr) -> Vec<BFEntity> {
    let entity_array_start = zt_world_mgr.entity_array_start;
    let entity_array_end = zt_world_mgr.entity_array_end;

    let mut entities: Vec<BFEntity> = Vec::new();
    let mut i = entity_array_start;
    while i < entity_array_end {
        let bf_entity = get_from_memory(get_from_memory::<u32>(i));
        entities.push(bf_entity);
        i += 0x4;
    }
    entities
}

fn get_zt_world_mgr_types(zt_world_mgr: &ZTWorldMgr) -> Vec<ZTEntityTypeWithPtr> {
    let entity_type_array_start = zt_world_mgr.entity_type_array_start;
    let entity_type_array_end = zt_world_mgr.entity_type_array_end;

    let mut entity_types: Vec<ZTEntityTypeWithPtr> = Vec::new();
    let mut i = entity_type_array_start;
    while i < entity_type_array_end {
        let type_ptr = get_from_memory::<u32>(i);
        info!("Reading entity at {:#x} -> {:#x}", i, type_ptr);
        let zt_entity_type = read_zt_entity_type_from_memory(type_ptr);
        entity_types.push(ZTEntityTypeWithPtr { ptr: type_ptr, entity_type: zt_entity_type });
        i += 0x4;
    }
    entity_types
}

pub fn get_entity_type_by_id(id: u32) -> u32 {
    let zt_world_mgr = globals().ztworldmgr();
    let entity_type_array_start = zt_world_mgr.entity_type_array_start;
    let entity_type_array_end = zt_world_mgr.entity_type_array_end;

    let mut i = (entity_type_array_end - entity_type_array_start) / 0x4;

    info!("Searching {} entity types for id {}", i, id);

    i -= 1;

    // TODO: Currently this function only works with Scenery types. We need to generalize it to work with all entity types.
    // This section defines three sets of entity types each with distinct cName ID offsets.
    // let scenery_types: HashSet<&str> = ["Fences", "Path", "Rubble", "TankWall", "TankFilter", "Scenery", "Building"].iter().cloned().collect();
    // let unit_types: HashSet<&str> = ["Animal", "Guest", "Keeper", "MaintenanceWorker", "DRT", "TourGuide"].iter().cloned().collect();
    // let overlay_types: HashSet<&str> = ["Ambient"].iter().cloned().collect();

    while i > 0 {
        let array_entry = entity_type_array_start + i * 0x4;
        let entity_type_ptr = get_from_memory::<u32>(array_entry);
        info!("Checking entity type at {:#x}", entity_type_ptr);
        let entity_type = map_from_memory::<ZTSceneryType>(entity_type_ptr);
        info!("Entity type name id: {}", entity_type.name_id);
        if entity_type.name_id == id {
            info!("Found entity type {}", entity_type.bfentitytype.get_type_name());
            return entity_type_ptr;
        } else {
            info!("Entity type {} does not match", entity_type.bfentitytype.get_type_name());
            i -= 1;
        }
    }
    0
}

// struct BFMap {
//     padding: [u8; 0x5c],
// }
