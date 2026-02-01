use getset::Getters;
use itertools::Itertools;
use num_enum::FromPrimitive;
use openzt_detour_macro::detour_mod;
use std::cmp::max;
use std::str::FromStr;
use std::{collections::HashMap, fmt};
use tracing::{error, info};

use crate::bfentitytype::ZTEntityTypeClass;
use crate::util::ZTBufferString;
use crate::ztmapview::BFTile;
use crate::{
    bfentitytype::{read_zt_entity_type_from_memory, BFEntityType, ZTEntityType, ZTSceneryType},
    command_console::CommandError,
    lua_fn,
    util::{get_from_memory, get_string_from_memory, map_from_memory},
};

const GLOBAL_ZTWORLDMGR_ADDRESS: u32 = 0x00638040;

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
pub struct BFEntity {
    vtable: u32,
    padding: [u8; 0x104],
    name: ZTBufferString,      // 0x108
    x_coord: i32,              // 0x114
    y_coord: i32,              // 0x118
    z_coord: i32,              // 0x11c
    height_above_terrain: u32, // 0x120
    padding4: [u8; 0x4],       // ----- padding: 4 bytes
    inner_class_ptr: u32,      // 0x128
    rotation: i32,             // 0x12c
    padding5: [u8; 0x14],      // ----- padding: 28 bytes
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
}

impl fmt::Display for BFEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BFEntity {{ name: {}, x_coord: {}, y_coord: {}, z_coord: {}, height_above_terrain: {}, rotation: {}, inner_class_ptr: {:#x}, visible: {}, snap_to_ground: {}, selected: {}, draw_dithered: {} }}",
            self.name, self.x_coord, self.y_coord, self.z_coord, self.height_above_terrain, self.rotation, self.inner_class_ptr, self.visible, self.snap_to_ground, self.selected, self.draw_dithered
        )
    }
}

impl BFEntity {
    pub fn entity_type_class(&self) -> ZTEntityTypeClass {
        // info!("Getting inner_class_ptr: {:#x} -> {:#x}", self.inner_class_ptr, get_from_memory::<u32>(self.inner_class_ptr));
        ZTEntityTypeClass::from(get_from_memory::<u32>(self.inner_class_ptr))
    }

    pub fn entity_type(&self) -> BFEntityType {
        get_from_memory(self.inner_class_ptr)
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
        rect.contains_point(&read_zt_world_mgr_from_global().tile_to_world(tile.pos, tile_size))
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
            min_x: self.x_coord - half_width,
            min_y: self.y_coord - half_height,
            max_x: self.x_coord + half_width,
            max_y: self.y_coord + half_height,
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

    pub fn get_footprint(&self) -> IVec3 {
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
        read_zt_world_mgr_from_global().get_tile_from_coords(self.x_coord, self.y_coord)
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
pub mod hooks_ztworldmgr {
    use crate::util::save_to_memory;
    use openzt_detour::gen::bfentity::{GET_BLOCKING_RECT, GET_BLOCKING_RECT_VIRT_ZTPATH, GET_FOOTPRINT, IS_ON_TILE};
    use openzt_detour::gen::bfmap::{GET_NEIGHBOR_1, TILE_TO_WORLD};

    use super::*;

    #[detour(GET_NEIGHBOR_1)]
    unsafe extern "thiscall" fn bfmap_get_neighbour(_this: u32, bftile: u32, direction: u32) -> u32 {
        let ztwm = read_zt_world_mgr_from_global();
        let bftile = get_from_memory::<BFTile>(bftile);
        let direction = Direction::from(direction);
        match ztwm.get_neighbour(&bftile, direction) {
            Some(neighbour) => ztwm.get_ptr_from_bftile(&neighbour),
            None => 0,
        }
    }

    // 0x0040f916 int * __thiscall OOAnalyzer::BFEntity::getFootprint(BFEntity *this,undefined4 *param_1)
    #[detour(GET_FOOTPRINT)]
    unsafe extern "thiscall" fn bfentity_get_footprint(_this: u32, param_1: u32, _param_2: bool) -> u32 {
        let entity = get_from_memory::<BFEntity>(_this);
        let footprint = entity.get_footprint();
        save_to_memory(param_1, footprint.x);
        save_to_memory(param_1 + 0x4, footprint.y);
        save_to_memory(param_1 + 0x8, footprint.z);

        param_1
    }

    // 0x0042721a u32 __thiscall OOAnalyzer::BFEntity::getBlockingRect(BFEntity *this,u32 param_1)
    #[detour(GET_BLOCKING_RECT)]
    unsafe extern "thiscall" fn bfentity_get_blocking_rect(_this: u32, param_1: u32) -> u32 {
        let entity = get_from_memory::<BFEntity>(_this);
        save_to_memory(param_1, entity.get_blocking_rect());
        param_1
    }

    // 0x004fbbee u32 __thiscall OOAnalyzer::BFEntity::getBlockingRect(BFEntity *this,u32 param_1)
    #[detour(GET_BLOCKING_RECT_VIRT_ZTPATH)]
    unsafe extern "thiscall" fn bfentity_get_blocking_rect_ztpath(_this: u32, param_1: u32) -> u32 {
        let entity = get_from_memory::<BFEntity>(_this);
        save_to_memory(param_1, entity.get_blocking_rect());
        param_1
    }

    // // 0040f26c BFPos * __thiscall OOAnalyzer::BFMap::tileToWorld(BFMap *this,BFPos *param_1,BFPos *param_2,BFPos *param_3)
    #[detour(TILE_TO_WORLD)]
    unsafe extern "thiscall" fn bfmap_tile_to_world(_this: u32, param_1: u32, param_2: u32, param_3: u32) -> u32 {
        let ztwm = read_zt_world_mgr_from_global();
        let tile_pos = get_from_memory::<IVec3>(param_2);
        let local_pos = get_from_memory::<IVec3>(param_3);
        let world_pos = ztwm.tile_to_world(tile_pos, local_pos);
        save_to_memory(param_1, world_pos);
        param_1
    }

    // TODO: Remove this when check_tank_placement is fully implemented
    // 004e16f1 bool __thiscall OOAnalyzer::BFEntity::isOnTile(BFEntity *this,BFTile *param_1)
    #[detour(IS_ON_TILE)]
    unsafe extern "thiscall" fn bfentity_is_on_tile(_this: u32, param_1: u32) -> bool {
        let result = unsafe { IS_ON_TILE_DETOUR.call(_this, param_1) };
        let entity = get_from_memory::<BFEntity>(_this);
        let tile = get_from_memory::<BFTile>(param_1);
        let reimimplented_result = entity.is_on_tile(&tile);
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
    // list_entities() - no args
    lua_fn!("list_entities", "Lists all entities in the world", "list_entities()", || {
        match command_get_zt_world_mgr_entities(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // list_entities_2() - no args
    lua_fn!("list_entities_2", "Lists all entities in the world (alternate format)", "list_entities_2()", || {
        match command_get_zt_world_mgr_entities_2(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

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

    unsafe { hooks_ztworldmgr::init_detours().unwrap() };
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

pub fn read_zt_world_mgr_from_global() -> ZTWorldMgr {
    let zt_world_mgr_ptr = get_from_memory::<u32>(GLOBAL_ZTWORLDMGR_ADDRESS);
    get_from_memory::<ZTWorldMgr>(zt_world_mgr_ptr)
}

fn log_zt_world_mgr(zt_world_mgr: &ZTWorldMgr) {
    info!("zt_world_mgr: {:#?}", zt_world_mgr);
}

fn command_get_zt_world_mgr_entities(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = read_zt_world_mgr_from_global();
    let entities = get_zt_world_mgr_entities(&zt_world_mgr);
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

fn command_get_zt_world_mgr_entities_2(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = read_zt_world_mgr_from_global();
    let entities = get_zt_world_mgr_entities_2(&zt_world_mgr);
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

    let zt_world_mgr = read_zt_world_mgr_from_global();
    let entities = get_zt_world_mgr_entities(&zt_world_mgr);

    let mut result = String::new();

    entities
        .iter()
        .map(|entity| (entity.type_class.class.clone(), entity.vtable + vtable_offset))
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

    let zt_world_mgr = read_zt_world_mgr_from_global();
    let entities = get_zt_world_mgr_types(&zt_world_mgr);

    let mut result = String::new();

    entities
        .iter()
        .map(|entity_type| (entity_type.class.clone(), entity_type.vtable + vtable_offset))
        .unique_by(|t| t.1)
        .for_each(|(type_name, vfunc_ptr)| {
            result.push_str(&format!("{:?} -> {:#x}\n", type_name, get_from_memory::<u32>(vfunc_ptr)));
        });

    Ok(result)
}

fn command_get_zt_world_mgr_types(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = read_zt_world_mgr_from_global();
    let types = get_zt_world_mgr_types(&zt_world_mgr);
    info!("Found {} types", types.len());
    if types.is_empty() {
        return Ok("No types found".to_string());
    }
    let mut string_array = Vec::new();
    for zt_type in types {
        string_array.push(zt_type.to_string());
    }
    Ok(string_array.join("\n"))
}

fn command_get_zt_world_mgr(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = read_zt_world_mgr_from_global();
    Ok(zt_world_mgr.to_string())
}

fn command_zt_world_mgr_types_summary(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_world_mgr = read_zt_world_mgr_from_global();
    let types = get_zt_world_mgr_types(&zt_world_mgr);
    let mut summary = "\n".to_string();
    let mut subtype: HashMap<String, u32> = HashMap::new();
    if types.is_empty() {
        return Ok("No types found".to_string());
    }
    let mut current_class = types[0].class.clone();
    for zt_type in types {
        if current_class != zt_type.class {
            let mut string_array = Vec::new();
            let mut total = 0;
            for (class, count) in subtype {
                string_array.push(format!("\t{:?}: {}", class, count));
                total += count;
            }
            summary.push_str(&format!("{:?}: ({})\n{}\n", current_class, total, string_array.join("\n")));
            info!("{:?}: ({})\n{}", current_class, total, string_array.join("\n"));
            subtype = HashMap::new();
            current_class = zt_type.class.clone();
        }
        info!("{:?}, {}", current_class, zt_type.zt_type);
        let count = subtype.entry(zt_type.zt_type).or_insert(0);
        *count += 1;
    }
    Ok(summary)
}

fn get_zt_world_mgr_entities(zt_world_mgr: &ZTWorldMgr) -> Vec<ZTEntity> {
    let entity_array_start = zt_world_mgr.entity_array_start;
    let entity_array_end = zt_world_mgr.entity_array_end;

    let mut entities: Vec<ZTEntity> = Vec::new();
    let mut i = entity_array_start;
    while i < entity_array_end {
        let zt_entity = read_zt_entity_from_memory(get_from_memory::<u32>(i));
        entities.push(zt_entity);
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

fn get_zt_world_mgr_types(zt_world_mgr: &ZTWorldMgr) -> Vec<ZTEntityType> {
    let entity_type_array_start = zt_world_mgr.entity_type_array_start;
    let entity_type_array_end = zt_world_mgr.entity_type_array_end;

    let mut entity_types: Vec<ZTEntityType> = Vec::new();
    let mut i = entity_type_array_start;
    while i < entity_type_array_end {
        info!("Reading entity at {:#x} -> {:#x}", i, get_from_memory::<u32>(i));
        let zt_entity_type = read_zt_entity_type_from_memory(get_from_memory::<u32>(i));
        entity_types.push(zt_entity_type);
        i += 0x4;
    }
    entity_types
}

pub fn get_entity_type_by_id(id: u32) -> u32 {
    let zt_world_mgr = read_zt_world_mgr_from_global();
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
        let entity_type_ptr = entity_type_array_start + i * 0x4;
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
