use std::{fmt, fmt::Display};

use tracing::info;

use crate::{
    command_console::CommandError,
    lua_fn,
    util::{get_from_memory, ZTBufferString},
};

const GLOBAL_ZTADVTERRAINMGR_ADDRESS: u32 = 0x00638058;
const BFTERRAINTYPEINFO_SIZE: usize = 0x30;

#[derive(Debug)]
#[repr(C)]
struct ZTAdvTerrainMgr_raw {
    vtable: u32,
    unknown_u32_1: u32,
    unknown_u32_2: u32,
    unknown_u32_3: u32,
    bf_terrain_type_info_array_start: u32, // TODO: Use ZTArray
    bf_terrain_type_info_array_end: u32,
    bf_terrain_type_info_buffer_end: u32,
    // Total size is 0x1dc
}

struct ZTAdvTerrainMgr {
    bf_terrain_type_info_array: Vec<BFTerrainTypeInfo>,
}

impl From<ZTAdvTerrainMgr_raw> for ZTAdvTerrainMgr {
    fn from(raw: ZTAdvTerrainMgr_raw) -> Self {
        info!(
            "Reading terrain types from {:#x} to {:#x}",
            raw.bf_terrain_type_info_array_start, raw.bf_terrain_type_info_array_end
        );
        let mut bf_terrain_type_info_array = Vec::new();
        let mut current_bf_terrain_type_info_address = raw.bf_terrain_type_info_array_start;
        while current_bf_terrain_type_info_address < raw.bf_terrain_type_info_array_end {
            bf_terrain_type_info_array.push(read_bfterraintypeinfo_from_memory(current_bf_terrain_type_info_address));
            current_bf_terrain_type_info_address += BFTERRAINTYPEINFO_SIZE as u32;
        }
        ZTAdvTerrainMgr { bf_terrain_type_info_array }
    }
}

impl Display for BFTerrainTypeInfo {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BFTerrainTypeInfo {{ vtable: {:#x} type_id: {} cost: {} blend: {} water: {} ?: {:#x} ?: {} ?: {} help_id: {} icon_string: {} }}",
            self.vtable, self.type_id, self.cost, self.blend, self.water, self.unknown_ptr, self.unknown_u32_6, self.unknown_u32_7, self.help_id, self.icon_string,
        )
    }
}

#[derive(Debug)]
#[repr(C)]
struct BFTerrainTypeInfo {
    vtable: u32,
    type_id: u32,
    cost: f32,
    blend: u32,
    water: u32,
    unknown_ptr: u32,
    unknown_u32_6: u32,
    unknown_u32_7: u32,
    help_id: u32,
    icon_string: ZTBufferString,
}

fn read_ztadvterrainmgr_raw_from_memory() -> ZTAdvTerrainMgr_raw {
    get_from_memory(get_from_memory::<u32>(GLOBAL_ZTADVTERRAINMGR_ADDRESS))
}

fn read_ztadvterrainmgr_from_memory() -> ZTAdvTerrainMgr {
    ZTAdvTerrainMgr::from(read_ztadvterrainmgr_raw_from_memory())
}

fn read_bfterraintypeinfo_from_memory(address: u32) -> BFTerrainTypeInfo {
    get_from_memory(address)
}

fn command_get_bfterraintypeinfo(_args: Vec<&str>) -> Result<String, CommandError> {
    let ztadvterrainmgr = read_ztadvterrainmgr_from_memory();
    info!("Found {} BFTerrainTypeInfo", ztadvterrainmgr.bf_terrain_type_info_array.len());
    let mut string_array = Vec::new();
    for bfterraintypeinfo in ztadvterrainmgr.bf_terrain_type_info_array {
        string_array.push(bfterraintypeinfo.to_string());
    }
    Ok(string_array.join("\n"))
}

pub fn init() {
    // list_bfterraintypeinfo() - no args
    lua_fn!("list_bfterraintypeinfo", "Lists terrain type info", "list_bfterraintypeinfo()", || {
        match command_get_bfterraintypeinfo(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });
}
