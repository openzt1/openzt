use nt_time::{time::OffsetDateTime, FileTime};
use openzt_detour_macro::detour_mod;
use std::fmt;
use tracing::info;

use getset::Getters;

use crate::{
    command_console::CommandError,
    lua_fn,
    util::{get_from_memory, ZTArray, ZTBoundedString, ZTString},
    ztmapview::BFTile,
    ztworldmgr::{read_zt_world_mgr_from_global, Direction},
};

pub const GLOBAL_ZTHABITATMGR_ADDRESS: u32 = 0x0063805c;

/// ZTHabitatMgr struct
#[derive(Debug)]
#[repr(C)]
pub struct ZTHabitatMgr {
    vtable: u32,                       // 0x000
    pad1: [u8; 0x04],                  // ----------------------- padding: 4 bytes
    map_size_x: u32,                   // 0x008
    map_size_y: u32,                   // 0x00c
    zoo_entrance_x: u32,               // 0x010
    zoo_entrance_y: u32,               // 0x014
    pad2: [u8; 0x04],                  // ----------------------- padding: 4 bytes
    exhibit_array: ZTArray<ZTHabitat>, // 0x01c (0xc bytes)
    other_array_start: u32,            // 0x028 //TODO: Use ZTArray; Seems to be some kind of mapping from BFTile to ZTHabitat or a ZTHabitat index
    other_array_end: u32,              // 0x02c
    other_array_buffer_end: u32,       // 0x030
    pad3: [u8; 0x24],                  // ----------------------- padding: 36 bytes
    popularity_scale_factor: f32,
}

impl ZTHabitatMgr {
    // fn get_tank(tile: &BFTile) -> Option<ZTHabitat> {

    // }

    pub fn get_habitat_by_tile(&self, tile: &BFTile) -> Option<ZTHabitat> {
        self.get_habitat(tile.pos.x, tile.pos.y)
    }

    // TODO: Should return Option<ZTExhibit> where ZTExhibit is a enum of ZTHabitat or ZTTankExhibit
    pub fn get_habitat(&self, pos_x: i32, pos_y: i32) -> Option<ZTHabitat> {
        let base_ptr = self.other_array_start;
        let offset_1 = pos_x as u32 * 0xc;
        let intermediate_ptr = get_from_memory::<u32>(base_ptr + offset_1);

        let offset_2 = pos_y as u32 * 0x28;
        let ptr = get_from_memory::<u32>(intermediate_ptr + offset_2);

        // TODO: Check vtable ptr and return ZTHabitat or ZTTankExhibit?

        if ptr != 0 {
            return Some(get_from_memory::<ZTHabitat>(ptr));
        }

        None
    }
}

// int index1 = temp_entity->field_0x34;
// int offset1 = index1 * 0xC;  // 12 bytes per entry
// void* basePointer = GLOBAL_ZTHabitatMgr->mbr_0x28;
// int* intermediatePtr = (int*)(basePointer + offset1);

// int index2 = temp_entity->field_0x38;
// int offset2 = index2 * 0x28;  // 40 bytes per entry
// ZTHabitat** habitatPtrPtr = (ZTHabitat**)(*intermediatePtr + offset2);
// ZTHabitat* this_01 = *habitatPtrPtr;

impl fmt::Display for ZTHabitatMgr {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTHabitatMgr ({:#x}) {{", self.vtable)?;
        writeln!(f, "  map_size_x: {},", self.map_size_x)?;
        writeln!(f, "  map_size_y: {},", self.map_size_y)?;
        writeln!(f, "  zoo_entrance_x: {},", self.zoo_entrance_x)?;
        writeln!(f, "  zoo_entrance_y: {},", self.zoo_entrance_y)?;
        writeln!(f, "  exhibit_array length: {},", self.exhibit_array.len())?;
        writeln!(f, "  other_array_start: {:#x},", self.other_array_start)?;
        writeln!(
            f,
            "  other_array_end: {:#x} ({}),",
            self.other_array_end,
            (self.other_array_end - self.other_array_start) / 12
        )?;
        writeln!(
            f,
            "  other_array_buffer_end: {:#x} ({}),",
            self.other_array_buffer_end,
            (self.other_array_buffer_end - self.other_array_start) / 12
        )?;
        writeln!(f, "  popularity_scale_factor: {},", self.popularity_scale_factor)?;
        write!(f, "}}")
    }
}

pub fn read_zt_habitat_mgr_from_memory() -> ZTHabitatMgr {
    get_from_memory::<ZTHabitatMgr>(get_from_memory(GLOBAL_ZTHABITATMGR_ADDRESS))
}

#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTHabitat {
    vtable: u32,                 // 0x000
    zt_show_info_ptr: u32,       // 0x004
    pad1: [u8; 0x38],            // ----------------------- padding: 60 bytes
    exhibit_tile_ptr: u32,       // 0x040 // Seems incorrect?
    pad2: [u8; 0x48],            // ----------------------- padding: 72 bytes
    entrance_tile_ptr: u32,      // 0x08c
    entrance_rotation: u32,      // 0x090
    pad3: [u8; 0x58],            // ----------------------- padding: 88 bytes
    unknown_u32: u32,            // 0x0ec
    pad4: [u8; 0xc],             // ----------------------- padding: 12 bytes
    current_donactions: f32,     // 0xfc
    last_donactions: f32,        // 0x100
    total_donactions: f32,       // 0x104
    current_upkeep: f32,         // 0x108
    last_upkeep: f32,            // 0x10c
    total_upkeep: f32,           // 0x110
    unknown_u32_2: u32,          // 0x114
    unknown_u32_3: u32,          // 0x118
    unknown_u32_4: u32,          // 0x11c
    created_timestamp: FileTime, // 0x120
    unknown_nt_time: FileTime,   // 0x128
    pad5: [u8; 0x24],            // ----------------------- padding: 36 bytes
    exhibit_name: ZTBoundedString,
}

impl ZTHabitat {
    pub fn get_gate_tile_in(&self) -> Option<BFTile> {
        if self.entrance_tile_ptr == 0 {
            return None;
        }
        // info!("ZTHabitat: {}", self);
        // info!("Entrance tile ptr: {:#x}", self.entrance_tile_ptr);
        let tile = get_from_memory::<BFTile>(self.entrance_tile_ptr);
        // info!("Entrance tile: {}", tile);

        let zthm = read_zt_habitat_mgr_from_memory();
        if let Some(gate_habitat) = zthm.get_habitat_by_tile(&tile)
            && gate_habitat == *self {
                return Some(tile);
            }
        let ztwm = read_zt_world_mgr_from_global();
        ztwm.get_neighbour(&tile, Direction::from(self.entrance_rotation))
    }
}

impl PartialEq for ZTHabitat {
    fn eq(&self, other: &Self) -> bool {
        self.exhibit_tile_ptr == other.exhibit_tile_ptr
            && self.entrance_rotation == other.entrance_rotation
            && self.entrance_tile_ptr == other.entrance_tile_ptr
            && self.exhibit_name.copy_to_string() == other.exhibit_name.copy_to_string()
    }
}

impl fmt::Display for ZTHabitat {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        writeln!(f, "ZTHabitat {{",)?;
        writeln!(f, "  vtable: {:#x},", self.vtable)?;
        writeln!(f, "  zt_show_info_ptr: {:#x},", self.zt_show_info_ptr)?;
        writeln!(f, "  exhibit_tile_ptr: {:#x},", self.exhibit_tile_ptr)?;
        writeln!(f, "  entrance_tile_ptr: {:#x},", self.entrance_tile_ptr)?;
        writeln!(f, "  entrance_rotation: {:#x},", self.entrance_rotation)?;
        writeln!(f, "  unknown_u32: {:#x},", self.unknown_u32)?;
        writeln!(f, "  current_donactions: {},", self.current_donactions)?;
        writeln!(f, "  last_donactions: {},", self.last_donactions)?;
        writeln!(f, "  total_donactions: {},", self.total_donactions)?;
        writeln!(f, "  current_upkeep: {},", self.current_upkeep)?;
        writeln!(f, "  last_upkeep: {},", self.last_upkeep)?;
        writeln!(f, "  total_upkeep: {},", self.total_upkeep)?;
        writeln!(f, "  unknown_u32_2: {:#x},", self.unknown_u32_2)?;
        writeln!(f, "  unknown_u32_3: {:#x},", self.unknown_u32_3)?;
        writeln!(f, "  unknown_u32_4: {:#x},", self.unknown_u32_4)?;
        writeln!(f, "  created_timestamp: {},", OffsetDateTime::try_from(self.created_timestamp).unwrap())?;
        writeln!(
            f,
            "  unknown_nt_time: {} ({}, {}, {}),",
            OffsetDateTime::try_from(self.unknown_nt_time).unwrap(),
            self.unknown_nt_time.to_raw() as f64,
            self.unknown_nt_time.to_raw() as u32,
            (self.unknown_nt_time.to_raw() >> 32) as u32
        )?;
        writeln!(f, "  exhibit_name: {},", self.exhibit_name.copy_to_string())?;

        // writeln!(f, "  entrance_x: {},", self.entrance_x)?;
        // writeln!(f, "  entrance_y: {},", self.entrance_y)?;
        // writeln!(f, "  entrance_rotation: {},", self.entrance_rotation)?;
        // writeln!(f, "  unknown_ptr: {:#x},", self.unknown_ptr)?;
        // writeln!(f, "  unknown_ptr2: {:#x},", self.unknown_ptr2)?;
        // writeln!(f, "  unknown_ptr3: {:#x},", self.unknown_ptr3)?;
        // writeln!(f, "  current_donations: {},", self.current_donations)?;
        // writeln!(f, "  last_donations: {},", self.last_donations)?;
        // writeln!(f, "  total_donations: {},", self.total_donations)?;
        // writeln!(f, "  current_upkeep: {},", self.current_upkeep)?;
        // writeln!(f, "  last_upkeep: {},", self.last_upkeep)?;
        // writeln!(f, "  total_upkeep: {},", self.total_upkeep)?;
        // writeln!(f, " unknown_ptr4: {:#x},", self.unknown_ptr4)?;
        // writeln!(f, " unknown_ptr5: {:#x},", self.unknown_ptr5)?;
        // writeln!(f, " unknown_ptr6: {:#x},", self.unknown_ptr6)?;
        // writeln!(f, " created_timestamp: {:#x},", self.created_timestamp)?;
        writeln!(f, "}}")
    }
}

fn command_get_zt_habitat_mgr(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_habitat_mgr = read_zt_habitat_mgr_from_memory();
    Ok(format!("{}", zt_habitat_mgr))
}

fn command_get_zt_habitats(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_habitat_mgr = read_zt_habitat_mgr_from_memory();
    let mut result_string = String::new();
    for i in 0..zt_habitat_mgr.exhibit_array.len() {
        let habitat = zt_habitat_mgr.exhibit_array.get(i);
        let habitat_location = zt_habitat_mgr.exhibit_array.get_ptr(i);
        let popularity_scale_factor = zt_habitat_mgr.popularity_scale_factor;
        result_string.push_str(&format!("Habitat {} ({:#x}): ", i, habitat_location));
        result_string.push_str(&format!(
            "  exhibit_popularity?: {}, {}, {}),\n",
            (habitat.unknown_nt_time.to_raw() as f64) / popularity_scale_factor as f64,
            (habitat.unknown_nt_time.to_raw() as f32) / popularity_scale_factor,
            ((habitat.unknown_nt_time.to_raw() >> 32) as f32) / popularity_scale_factor
        ));
        result_string.push_str(&format!("{}\n", habitat));
    }
    // zt_habitat_mgr.exhibit_array.get_vec().iter().enumerate().for_each(|(i, habitat)| {
    //     result_string.push_str(&format!("Habitat {}: ", i));
    //     result_string.push_str(&format!("{}\n", habitat));
    // });
    Ok(result_string)
}

#[detour_mod]
pub mod hooks_zthabitatmgr {
    use super::*;
    use openzt_detour::generated::zthabitat::GET_GATE_TILE_IN;

    // 00410349 BFTile * __thiscall OOAnalyzer::ZTHabitat::getGateTileIn(ZTHabitat *this)
    #[detour(GET_GATE_TILE_IN)]
    unsafe extern "thiscall" fn get_gate_tile_in(_this: u32) -> u32 {
        let habitat = get_from_memory::<ZTHabitat>(_this);
        match habitat.get_gate_tile_in() {
            Some(tile) => read_zt_world_mgr_from_global().get_ptr_from_bftile(&tile),
            None => 0,
        }
    }
}

pub fn init() {
    // get_zthabitatmgr() - no args
    lua_fn!("get_zthabitatmgr", "Returns ZTHabitatMgr debug info", "get_zthabitatmgr()", || {
        match command_get_zt_habitat_mgr(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // list_exhibits() - no args
    lua_fn!("list_exhibits", "Lists all zoo exhibits/habitats", "list_exhibits()", || {
        match command_get_zt_habitats(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    if let Err(e) = unsafe { hooks_zthabitatmgr::init_detours() } {
        info!("Error initialising zthabitatmgr detours: {}", e);
    }
}
