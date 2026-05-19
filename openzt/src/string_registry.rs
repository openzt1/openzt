use std::{path::Path, sync::Mutex};

use openzt_detour_macro::detour_mod;
use std::collections::{BTreeSet, HashMap};
use std::sync::LazyLock;
use tracing::{info, warn};

use crate::lua_fn;

const STRING_REGISTRY_ID_OFFSET: u32 = 100_000;

const GLOBAL_BFAPP: u32 = 0x00638148;

static STRING_REGISTRY: LazyLock<Mutex<Vec<String>>> = LazyLock::new(|| Mutex::new(Vec::new()));
static LANGUAGE_STRINGS: LazyLock<Mutex<Vec<Option<String>>>> = LazyLock::new(|| Mutex::new(Vec::new()));

static STRING_OVERRIDES: LazyLock<Mutex<HashMap<u32, String>>> =
    LazyLock::new(|| Mutex::new(DEFAULT_OVERRIDES.iter().map(|(id, string_override)| (*id, string_override.to_string())).collect()));

const DEFAULT_OVERRIDES: &[(u32, &str)] = &[(3383, "Swamp"), (33383, "Swampy terrain")];

const RT_STRING: u32 = 6;
const IMAGE_DIRECTORY_ENTRY_RESOURCE: usize = 2;
const LOAD_STRING_BUFFER_SIZE: usize = 2048;

pub fn add_override_string_to_registry(string_id: u32, string_val: String) {
    let mut data_mutex = STRING_OVERRIDES.lock().unwrap();
    info!("Added override string to registry: {} -> {}", string_id, string_val.clone());
    data_mutex.insert(string_id, string_val);
}

pub fn get_override_string_from_registry(string_id: u32) -> Option<String> {
    // info!("Getting override string from registry: {}", string_id);
    let data_mutex = STRING_OVERRIDES.lock().unwrap();
    data_mutex.get(&string_id).cloned()
}

pub fn load_language_dll_strings() {
    let paths = crate::dll_dependencies::language_dll_paths();
    let mut loaded_count = 0usize;

    for path in paths {
        match load_strings_from_pe(path) {
            Ok(strings) => {
                loaded_count += strings.len();
                let mut cache = LANGUAGE_STRINGS.lock().unwrap();
                for (id, value) in strings {
                    if id as usize >= cache.len() {
                        cache.resize(id as usize + 1, None);
                    }
                    cache[id as usize] = Some(value);
                }
            }
            Err(err) => warn!("Failed to load language strings from {}: {}", path.display(), err),
        }
    }

    info!("Loaded {} strings from {} language DLL files", loaded_count, paths.len());
}

pub fn get_language_string(string_id: u32) -> Option<String> {
    LANGUAGE_STRINGS.lock().unwrap().get(string_id as usize).cloned().flatten()
}

pub(crate) fn load_string_by_id(string_id: u32) -> Option<String> {
    if string_id >= STRING_REGISTRY_ID_OFFSET
        && let Ok(string) = get_string_from_registry(string_id)
    {
        return Some(string);
    }

    if let Some(string) = get_override_string_from_registry(string_id) {
        return Some(string);
    }

    if let Some(string) = get_language_string(string_id) {
        return Some(string);
    }

    load_string_from_game(string_id)
}

fn find_strings_containing(substring: &str) -> Vec<(u32, String)> {
    let needle = substring.to_lowercase();
    let mut string_ids = BTreeSet::new();

    {
        let language_strings = LANGUAGE_STRINGS.lock().unwrap();
        string_ids.extend(
            language_strings
                .iter()
                .enumerate()
                .filter_map(|(id, string)| string.as_ref().map(|_| id as u32)),
        );
    }

    {
        let overrides = STRING_OVERRIDES.lock().unwrap();
        string_ids.extend(overrides.keys().copied());
    }

    {
        let registry = STRING_REGISTRY.lock().unwrap();
        string_ids.extend((0..registry.len()).map(|index| STRING_REGISTRY_ID_OFFSET + index as u32));
    }

    string_ids
        .into_iter()
        .filter_map(|id| load_string_by_id(id).map(|string| (id, string)))
        .filter(|(_, string)| string.to_lowercase().contains(&needle))
        .collect()
}

pub fn add_string_to_registry(string_val: String) -> u32 {
    let mut data_mutex = STRING_REGISTRY.lock().unwrap();
    info!(
        "Added string to registry: {} -> {}",
        string_val.clone(),
        data_mutex.len() as u32 + STRING_REGISTRY_ID_OFFSET
    );
    data_mutex.push(string_val);
    data_mutex.len() as u32 + STRING_REGISTRY_ID_OFFSET - 1
}

pub fn get_string_from_registry(string_id: u32) -> Result<String, &'static str> {
    info!("Getting string from registry: {}", string_id);
    let string = {
        let data_mutex = STRING_REGISTRY.lock().unwrap();
        data_mutex.get((string_id - STRING_REGISTRY_ID_OFFSET) as usize).cloned()
    };
    match string {
        Some(string) => Ok(string),
        None => {
            info!("String not found");
            Err("String not found")
        }
    }
}

fn load_strings_from_pe(path: &Path) -> Result<Vec<(u32, String)>, String> {
    let bytes = std::fs::read(path).map_err(|err| err.to_string())?;
    let pe = PeFile::parse(&bytes)?;
    let Some(resource) = pe.resource_section() else {
        return Ok(Vec::new());
    };

    let mut strings = Vec::new();
    for type_entry in resource.entries(0)? {
        if type_entry.id() != Some(RT_STRING) {
            continue;
        }

        let type_dir = resource.entry_dir_offset(&type_entry)?;
        for name_entry in resource.entries(type_dir)? {
            let Some(block_id) = name_entry.id() else {
                continue;
            };

            let name_dir = resource.entry_dir_offset(&name_entry)?;
            for lang_entry in resource.entries(name_dir)? {
                let data_entry_offset = resource.entry_data_offset(&lang_entry)?;
                let data_rva = read_u32(&bytes, data_entry_offset).ok_or("Invalid string resource data RVA")?;
                let data_size = read_u32(&bytes, data_entry_offset + 4).ok_or("Invalid string resource data size")? as usize;
                let data_offset = pe.rva_to_offset(data_rva).ok_or("String resource RVA is outside PE sections")?;
                let data = bytes
                    .get(data_offset..data_offset + data_size)
                    .ok_or("String resource data is outside file bounds")?;

                strings.extend(parse_string_block(block_id, data));
            }
        }
    }

    Ok(strings)
}

fn parse_string_block(block_id: u32, data: &[u8]) -> Vec<(u32, String)> {
    if block_id == 0 {
        return Vec::new();
    }

    let mut strings = Vec::new();
    let mut offset = 0usize;

    for index in 0..16u32 {
        let Some(len) = read_u16(data, offset) else {
            break;
        };
        offset += 2;

        let byte_len = len as usize * 2;
        let Some(raw) = data.get(offset..offset + byte_len) else {
            break;
        };
        offset += byte_len;

        if len == 0 {
            continue;
        }

        let utf16: Vec<u16> = raw.chunks_exact(2).map(|chunk| u16::from_le_bytes([chunk[0], chunk[1]])).collect();
        let id = (block_id - 1) * 16 + index;
        strings.push((id, String::from_utf16_lossy(&utf16)));
    }

    strings
}

struct PeFile<'a> {
    bytes: &'a [u8],
    sections: Vec<PeSection>,
    resource_rva: u32,
    resource_size: u32,
}

struct PeSection {
    virtual_address: u32,
    virtual_size: u32,
    raw_data_ptr: u32,
    raw_data_size: u32,
}

struct ResourceSection<'a> {
    bytes: &'a [u8],
    base_offset: usize,
}

struct ResourceEntry {
    offset_to_data: u32,
    integer_id: u32,
    named: bool,
}

impl<'a> PeFile<'a> {
    fn parse(bytes: &'a [u8]) -> Result<Self, String> {
        if bytes.get(0..2) != Some(b"MZ") {
            return Err("Missing MZ header".to_string());
        }

        let pe_offset = read_u32(bytes, 0x3c).ok_or("Missing PE header offset")? as usize;
        if bytes.get(pe_offset..pe_offset + 4) != Some(b"PE\0\0") {
            return Err("Missing PE signature".to_string());
        }

        let number_of_sections = read_u16(bytes, pe_offset + 6).ok_or("Missing section count")? as usize;
        let optional_header_size = read_u16(bytes, pe_offset + 20).ok_or("Missing optional header size")? as usize;
        let optional_header_offset = pe_offset + 24;
        let magic = read_u16(bytes, optional_header_offset).ok_or("Missing optional header magic")?;
        let data_directory_offset = match magic {
            0x10b => optional_header_offset + 96,
            0x20b => optional_header_offset + 112,
            _ => return Err(format!("Unsupported PE optional header magic {magic:#x}")),
        };

        let resource_dir_offset = data_directory_offset + IMAGE_DIRECTORY_ENTRY_RESOURCE * 8;
        let resource_rva = read_u32(bytes, resource_dir_offset).ok_or("Missing resource directory RVA")?;
        let resource_size = read_u32(bytes, resource_dir_offset + 4).ok_or("Missing resource directory size")?;

        let section_table_offset = optional_header_offset + optional_header_size;
        let mut sections = Vec::with_capacity(number_of_sections);
        for index in 0..number_of_sections {
            let offset = section_table_offset + index * 40;
            sections.push(PeSection {
                virtual_size: read_u32(bytes, offset + 8).ok_or("Invalid section virtual size")?,
                virtual_address: read_u32(bytes, offset + 12).ok_or("Invalid section virtual address")?,
                raw_data_size: read_u32(bytes, offset + 16).ok_or("Invalid section raw data size")?,
                raw_data_ptr: read_u32(bytes, offset + 20).ok_or("Invalid section raw data pointer")?,
            });
        }

        Ok(Self { bytes, sections, resource_rva, resource_size })
    }

    fn resource_section(&self) -> Option<ResourceSection<'a>> {
        let base_offset = self.rva_to_offset(self.resource_rva)?;
        self.bytes.get(base_offset..base_offset + self.resource_size as usize)?;
        Some(ResourceSection { bytes: self.bytes, base_offset })
    }

    fn rva_to_offset(&self, rva: u32) -> Option<usize> {
        self.sections.iter().find_map(|section| {
            let size = section.virtual_size.max(section.raw_data_size);
            if rva >= section.virtual_address && rva < section.virtual_address + size {
                Some((section.raw_data_ptr + (rva - section.virtual_address)) as usize)
            } else {
                None
            }
        })
    }
}

impl ResourceEntry {
    fn id(&self) -> Option<u32> {
        (!self.named).then_some(self.integer_id)
    }
}

impl<'a> ResourceSection<'a> {
    fn entries(&self, dir_offset: usize) -> Result<Vec<ResourceEntry>, String> {
        let absolute_dir_offset = self.base_offset + dir_offset;
        let named_count = read_u16(self.bytes, absolute_dir_offset + 12).ok_or("Invalid resource directory named count")? as usize;
        let id_count = read_u16(self.bytes, absolute_dir_offset + 14).ok_or("Invalid resource directory ID count")? as usize;
        let entry_count = named_count + id_count;
        let mut entries = Vec::with_capacity(entry_count);

        for index in 0..entry_count {
            let entry_offset = absolute_dir_offset + 16 + index * 8;
            let name = read_u32(self.bytes, entry_offset).ok_or("Invalid resource entry name")?;
            let offset_to_data = read_u32(self.bytes, entry_offset + 4).ok_or("Invalid resource entry data offset")?;
            entries.push(ResourceEntry {
                integer_id: name & 0xffff,
                named: name & 0x8000_0000 != 0,
                offset_to_data,
            });
        }

        Ok(entries)
    }

    fn entry_dir_offset(&self, entry: &ResourceEntry) -> Result<usize, String> {
        if entry.offset_to_data & 0x8000_0000 == 0 {
            return Err("Expected resource directory entry".to_string());
        }
        Ok((entry.offset_to_data & 0x7fff_ffff) as usize)
    }

    fn entry_data_offset(&self, entry: &ResourceEntry) -> Result<usize, String> {
        if entry.offset_to_data & 0x8000_0000 != 0 {
            return Err("Expected resource data entry".to_string());
        }
        Ok(self.base_offset + entry.offset_to_data as usize)
    }
}

fn read_u16(bytes: &[u8], offset: usize) -> Option<u16> {
    let bytes = bytes.get(offset..offset + 2)?;
    Some(u16::from_le_bytes([bytes[0], bytes[1]]))
}

fn read_u32(bytes: &[u8], offset: usize) -> Option<u32> {
    let bytes = bytes.get(offset..offset + 4)?;
    Some(u32::from_le_bytes([bytes[0], bytes[1], bytes[2], bytes[3]]))
}

fn is_user_type_id(param_1: u32) -> bool {
    (19000..=21999).contains(&param_1) || (49000..=51999).contains(&param_1) || (74000..=76999).contains(&param_1)
}

fn load_string_from_game(string_id: u32) -> Option<String> {
    let bfapp_load_string: extern "thiscall" fn(u32, u32, u32) -> u32 = unsafe { std::mem::transmute(0x00404e0a) };
    let mut buffer = [0u8; LOAD_STRING_BUFFER_SIZE];
    let length = bfapp_load_string(GLOBAL_BFAPP, string_id, buffer.as_mut_ptr() as u32);
    if length == 0 {
        return None;
    }

    let length = (length as usize).min(buffer.len());
    let bytes = &buffer[..length];
    let bytes = match bytes.iter().position(|byte| *byte == 0) {
        Some(null_pos) => &bytes[..null_pos],
        None => bytes,
    };
    if bytes.is_empty() {
        return None;
    }

    Some(crate::encoding_utils::decode_game_text(bytes))
}

#[detour_mod]
pub mod zoo_string {
    use openzt_detour::generated::bfapp::LOAD_STRING;
    use tracing::info;

    use super::{get_language_string, get_override_string_from_registry, is_user_type_id, STRING_REGISTRY_ID_OFFSET};
    use crate::{string_registry::get_string_from_registry, util::{save_string_to_memory, Addr}};

    #[detour(LOAD_STRING)]
    unsafe extern "thiscall" fn bf_app_load_string(this_ptr: *const u32, string_id: *const u32, string_buffer: *const u8) -> u32 {
        let string_id_val = string_id as u32;
        if is_user_type_id(string_id_val) {
            info!("BFApp::loadString {:#x} {} {:#x}", Addr::of(this_ptr), string_id_val, Addr::of(string_buffer));
        }
        if string_id_val >= STRING_REGISTRY_ID_OFFSET
            && let Ok(string) = get_string_from_registry(string_id_val) {
                info!("BFApp::loadString string_id: {}, override: {} -> {}", string_id_val, string, string.len());
                save_string_to_memory(string_buffer, &string);
                return string.len() as u32 + 1;
            }
        if let Some(override_string) = get_override_string_from_registry(string_id_val) {
            save_string_to_memory(string_buffer, &override_string);
            return override_string.len() as u32 + 1;
        }
        if let Some(language_string) = get_language_string(string_id_val) {
            save_string_to_memory(string_buffer, &language_string);
            return language_string.len() as u32 + 1;
        }
        unsafe { LOAD_STRING_DETOUR.call(this_ptr, string_id, string_buffer) }
    }

    // #[hook(unsafe extern "thiscall" BFWorldMgr_unknown, offset = 0x00010d48)]
    // fn bf_world_mgr_unknown(this_ptr: u32, base_user_id: u32) -> u32 {
    //     let return_value = unsafe { BFWorldMgr_unknown.call(this_ptr, base_user_id) };
    //     info!("BFWorldMgr::unknown {:#x} {} -> {:#x}", this_ptr, base_user_id, return_value);
    //     return_value
    // }

    // #[hook(unsafe extern "cdecl" BFEntityType_getUserDataIndex, offset = 0x0001fe27a)]
    // fn bf_entity_type_get_user_data_index(param_1: u32) -> u8 {
    //     let return_value = unsafe { BFEntityType_getUserDataIndex.call(param_1) };
    //     info!("BFEntityType::getUserDataIndex {} -> {}", param_1, return_value);
    //     return_value
    // }

    // #[hook(unsafe extern "thiscall" BFEntityType_getUserData, offset = 0x0001fe1ea)]
    // fn bf_entity_type_get_user_data(this_ptr: u32, user_data_index: u32, param_2: u32) -> u32 {
    //     let return_value =
    //         unsafe { BFEntityType_getUserData.call(this_ptr, user_data_index, param_2) };
    //     info!(
    //         "BFEntityType::getUserData {:#x} {} {} -> {:#x}",
    //         this_ptr, user_data_index, param_2, return_value
    //     );
    //     return_value
    // }
}

pub fn init() {
    load_language_dll_strings();

    if unsafe { zoo_string::init_detours() }.is_err() {
        info!("Failed to initialize string_registry detours");
    }

    lua_fn!(
        "get_string",
        "Look up a game string by ID",
        "get_string(id)",
        |id: u32| {
            match load_string_by_id(id) {
                Some(string) => Ok((Some(string), None::<String>)),
                None => Ok((None::<String>, Some(format!("String {} not found", id)))),
            }
        }
    );

    lua_fn!(
        "find_strings",
        "Find loaded game strings containing a substring",
        "find_strings(substring)",
        |substring: String| {
            let substring = substring.trim();
            if substring.is_empty() {
                return Ok((None::<String>, Some("Search substring cannot be empty".to_string())));
            }

            let matches = find_strings_containing(substring);
            if matches.is_empty() {
                return Ok((Some(format!("No strings found containing '{}'", substring)), None::<String>));
            }

            let result = matches
                .into_iter()
                .map(|(id, string)| format!("{}: {}", id, string))
                .collect::<Vec<_>>()
                .join("\n");

            Ok((Some(result), None::<String>))
        }
    );
}
