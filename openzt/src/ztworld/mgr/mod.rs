use itertools::Itertools;
use openzt_detour_macro::detour_mod;
use std::collections::HashMap;
use std::str::FromStr;
use tracing::{error, info};

use crate::bfentitytype::{read_zt_entity_type_from_memory, ZTEntityTypeClass, ZTSceneryType};
use crate::command_console::CommandError;
use crate::geom::{Direction, IVec3};
use crate::globals::globals;
use crate::lua_fn;
use crate::util::{get_from_memory, map_from_memory, ref_from_memory};
use crate::ztmapview::BFTile;
use crate::ztworld::entity::{read_zt_entity_from_memory, BFEntity, ZTAnimal, ZTEntityClass, ZTEntityWithPtr, ZTEntityTypeWithPtr, ZTUnit};

pub mod ztworldmgr;
pub use ztworldmgr::*;

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
    use openzt_detour::generated::bfentity::{GET_BLOCKING_RECT, GET_FOOTPRINT, IS_ON_TILE};
    use openzt_detour::generated::ztpath::GET_BLOCKING_RECT as GET_BLOCKING_RECT_VIRT_ZTPATH;
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
    unsafe extern "thiscall" fn bfentity_get_blocking_rect(_this: *const u32, param_1: *const std::ffi::c_void) -> *const u32 {
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
                result, reimimplented_result, entity.name()
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

    // read_entity_offset(offsets..., types..., [entity_type]) - offsets (1 or more), types (1 or same count as offsets), optional entity type filter
    lua_fn!("read_entity_offset", "Read value(s) at offset(s) from entities. Offsets: one or more. Types: one (applied to all) or same count as offsets. Optional: entity_type filter. (types: ptr, u32, i32, u16, i16, u8, i8, f32, bool)", "read_entity_offset(offsets..., [types...], [entity_type])",
        |args: mlua::Variadic<String>| {
            let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match command_read_entity_offset(str_args) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // read_entity_type_offset(offsets..., types..., [entity_type_class]) - offsets (1 or more), types (1 or same count as offsets), optional entity type class filter
    lua_fn!("read_entity_type_offset", "Read value(s) at offset(s) from entity types. Offsets: one or more. Types: one (applied to all) or same count as offsets. Optional: entity_type_class filter. (types: ptr, u32, i32, u16, i16, u8, i8, f32, bool)", "read_entity_type_offset(offsets..., [types...], [entity_type_class])",
        |args: mlua::Variadic<String>| {
            let str_args: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match command_read_entity_type_offset(str_args) {
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

fn log_zt_world_mgr(zt_world_mgr: &ZTWorldMgr) {
    info!("zt_world_mgr: {:#?}", zt_world_mgr);
}

fn command_get_zt_world_mgr_entities(args: Vec<&str>) -> Result<String, CommandError> {
    let filter = if args.len() > 1 {
        return Err(CommandError::new("Too many arguments".to_string()));
    } else if args.len() == 1 {
        Some(args[0].parse::<ZTEntityClass>().map_err(CommandError::new)?)
    } else {
        None
    };

    let zt_world_mgr = globals().ztworldmgr();
    let entities = get_zt_world_mgr_entities(zt_world_mgr);

    let filtered: Vec<_> = if let Some(ref entity_type) = filter {
        entities.iter().filter(|e| e.entity.class() == entity_type).collect()
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

// Helper struct for offset reads
#[derive(Clone)]
struct OffsetRead {
    offset: u32,
    type_str: String,
}

// Helper function to parse offset and type strings into OffsetRead structs
fn parse_offset_reads(
    offset_strs: &[&str],
    type_strs: &[&str],
    valid_types: &[&str],
) -> Result<Vec<OffsetRead>, CommandError> {
    if offset_strs.is_empty() {
        return Err(CommandError::new("At least one offset must be provided".to_string()));
    }

    // Parse offsets
    let mut offsets: Vec<u32> = Vec::new();
    for offset_str in offset_strs {
        let offset = match offset_str.strip_prefix("0x") {
            Some(hex_str) => u32::from_str_radix(hex_str, 16).map_err(|e| CommandError::new(format!("Invalid offset '{}': {}", offset_str, e)))?,
            None => offset_str.parse::<u32>().map_err(|e| CommandError::new(format!("Invalid offset '{}': {}", offset_str, e)))?,
        };
        offsets.push(offset);
    }

    // Validate types
    for type_str in type_strs {
        if !valid_types.contains(type_str) {
            return Err(CommandError::new(format!("Invalid type '{}'. Valid types: {}", type_str, valid_types.join(", "))));
        }
    }

    // Create OffsetRead structs
    let mut offset_reads: Vec<OffsetRead> = Vec::new();
    match type_strs.len() {
        0 => {
            // Default type "u32" for all offsets
            for offset in offsets {
                offset_reads.push(OffsetRead { offset, type_str: "u32".to_string() });
            }
        }
        1 => {
            // Single type applies to all offsets
            let type_str = type_strs[0].to_string();
            for offset in offsets {
                offset_reads.push(OffsetRead { offset, type_str: type_str.clone() });
            }
        }
        n if n == offsets.len() => {
            // One type per offset
            for (i, offset) in offsets.into_iter().enumerate() {
                offset_reads.push(OffsetRead { offset, type_str: type_strs[i].to_string() });
            }
        }
        n => {
            return Err(CommandError::new(format!(
                "Type count ({}) must be 1 or match offset count ({})",
                n, offsets.len()
            )));
        }
    }

    Ok(offset_reads)
}

// Helper function to parse variadic args into (offsets, types, filter)
type ParsedOffsetArgs<'a> = (Vec<&'a str>, Vec<&'a str>, Option<&'a str>);

fn parse_offset_type_filter_args<'a>(
    args: &'a [&'a str],
    valid_types: &[&str],
) -> Result<ParsedOffsetArgs<'a>, CommandError> {
    if args.is_empty() {
        return Err(CommandError::new("At least one offset must be provided".to_string()));
    }

    // Find where types start (first valid type string)
    let mut types_start_idx = args.len();
    for (i, arg) in args.iter().enumerate() {
        // Skip first arg (must be an offset)
        if i == 0 {
            continue;
        }
        if valid_types.contains(arg) {
            types_start_idx = i;
            break;
        }
    }

    // Check if last arg is entity type filter (not a valid type)
    let (offset_strs, type_strs, filter): (&[&str], &[&str], Option<&str>) = if types_start_idx < args.len() {
        // Found type(s)
        let offset_strs = &args[0..types_start_idx];
        let remaining = &args[types_start_idx..];

        // Check if last arg is entity type filter (not a valid type)
        if remaining.len() > 1 && !valid_types.contains(remaining.last().unwrap()) {
            let type_strs = &remaining[..remaining.len() - 1];
            let filter = Some(*remaining.last().unwrap());
            (offset_strs, type_strs, filter)
        } else {
            let type_strs = remaining;
            (offset_strs, type_strs, None)
        }
    } else {
        // No types provided, check if last arg is entity type filter
        let offset_strs = &args[0..args.len()];
        if offset_strs.len() > 1 && !valid_types.contains(offset_strs.last().unwrap()) {
            // Last arg might be filter
            let (offsets, filter) = offset_strs.split_at(offset_strs.len() - 1);
            (offsets, [].as_slice(), Some(filter[0]))
        } else {
            (offset_strs, [].as_slice(), None)
        }
    };

    Ok((offset_strs.to_vec(), type_strs.to_vec(), filter))
}

fn command_read_entity_offset(args: Vec<&str>) -> Result<String, CommandError> {
    let valid_types = ["ptr", "u32", "i32", "u16", "i16", "u8", "i8", "f32", "bool"];

    // Parse: offsets..., [types...], [entity_type]
    let (offset_strs, type_strs, filter) = parse_offset_type_filter_args(&args, &valid_types)?;

    let offset_reads = parse_offset_reads(&offset_strs, &type_strs, &valid_types)?;

    // Parse entity type filter
    let filter = if let Some(filter_str) = filter {
        Some(filter_str.parse::<ZTEntityClass>().map_err(CommandError::new)?)
    } else {
        None
    };

    let zt_world_mgr = globals().ztworldmgr();
    let entities = get_zt_world_mgr_entities(zt_world_mgr);

    let filtered: Vec<_> = if let Some(ref entity_type) = filter {
        entities.iter().filter(|e| e.entity.class() == entity_type).collect()
    } else {
        entities.iter().collect()
    };

    if filtered.is_empty() {
        return Ok("No entities found".to_string());
    }

    let mut string_array = Vec::new();
    for ewp in filtered {
        let mut parts = vec![
            format!("{:#x}", ewp.ptr),
            ewp.entity.name().clone()
        ];

        if filter.is_none() {
            parts.push(format!("{:?}", ewp.entity.class()));
        }

        // Read each offset
        for read in &offset_reads {
            let value_str = match read.type_str.as_str() {
                "ptr" => format!("{:#x}", get_from_memory::<u32>(ewp.ptr + read.offset)),
                "u32" => format!("{}", get_from_memory::<u32>(ewp.ptr + read.offset)),
                "i32" => format!("{}", get_from_memory::<i32>(ewp.ptr + read.offset)),
                "u16" => format!("{}", get_from_memory::<u16>(ewp.ptr + read.offset)),
                "i16" => format!("{}", get_from_memory::<i16>(ewp.ptr + read.offset)),
                "u8" => format!("{}", get_from_memory::<u8>(ewp.ptr + read.offset)),
                "i8" => format!("{}", get_from_memory::<i8>(ewp.ptr + read.offset)),
                "f32" => format!("{}", get_from_memory::<f32>(ewp.ptr + read.offset)),
                "bool" => format!("{}", get_from_memory::<bool>(ewp.ptr + read.offset)),
                _ => unreachable!(),
            };
            parts.push(value_str.to_string());
        }

        string_array.push(parts.join(" | "));
    }
    Ok(string_array.join("\n"))
}

fn command_read_entity_type_offset(args: Vec<&str>) -> Result<String, CommandError> {
    let valid_types = ["ptr", "u32", "i32", "u16", "i16", "u8", "i8", "f32", "bool"];

    // Parse: offsets..., [types...], [entity_type_class]
    let (offset_strs, type_strs, filter) = parse_offset_type_filter_args(&args, &valid_types)?;

    let offset_reads = parse_offset_reads(&offset_strs, &type_strs, &valid_types)?;

    // Parse entity type class filter
    let filter = if let Some(filter_str) = filter {
        Some(filter_str.parse::<ZTEntityTypeClass>().map_err(CommandError::new)?)
    } else {
        None
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
        let mut parts = vec![
            format!("{:#x}", etwp.ptr),
            etwp.entity_type.zt_type.clone(),
            etwp.entity_type.zt_sub_type.clone()
        ];

        if filter.is_none() {
            parts.push(format!("{:?}", etwp.entity_type.class));
        }

        // Read each offset
        for read in &offset_reads {
            let value_str = match read.type_str.as_str() {
                "ptr" => format!("{:#x}", get_from_memory::<u32>(etwp.ptr + read.offset)),
                "u32" => format!("{}", get_from_memory::<u32>(etwp.ptr + read.offset)),
                "i32" => format!("{}", get_from_memory::<i32>(etwp.ptr + read.offset)),
                "u16" => format!("{}", get_from_memory::<u16>(etwp.ptr + read.offset)),
                "i16" => format!("{}", get_from_memory::<i16>(etwp.ptr + read.offset)),
                "u8" => format!("{}", get_from_memory::<u8>(etwp.ptr + read.offset)),
                "i8" => format!("{}", get_from_memory::<i8>(etwp.ptr + read.offset)),
                "f32" => format!("{}", get_from_memory::<f32>(etwp.ptr + read.offset)),
                "bool" => format!("{}", get_from_memory::<bool>(etwp.ptr + read.offset)),
                _ => unreachable!(),
            };
            parts.push(format!("offset{:#x}={}", read.offset, value_str));
        }

        string_array.push(parts.join(" | "));
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
        .map(|ewp| (ewp.entity.type_class().class.clone(), ewp.entity.vtable() + vtable_offset))
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
