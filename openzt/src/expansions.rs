use std::{
    collections::{HashMap, HashSet},
    ffi::CString,
    fmt,
    fmt::Display,
    path::Path,
    sync::{Mutex, MutexGuard},
};

use anyhow::{anyhow, Context};
use maplit::hashset;
use openzt_configparser::ini::Ini;
use openzt_detour_macro::detour_mod;
use std::sync::LazyLock;
use tracing::{debug, error, info};

use crate::{
    animation::Animation,
    bfentitytype::{ZTEntityType, ZTEntityTypeClass},
    command_console::CommandError,
    lua_fn,
    resource_manager::mod_config::get_openzt_config,
    resource_manager::{add_handler, modify_ztfile_as_animation, modify_ztfile_as_ini, Handler, RunStage, OPENZT_DIR0},
    string_registry::add_string_to_registry,
    util::{get_from_memory, get_string_from_memory, get_string_from_memory_bounded, save_to_memory},
    ztui::{get_random_sex, get_selected_sex, BuyTab, Sex},
};

/// List of official ZTD files so we can determine if a given ZTD file is custom content
static OFFICIAL_FILESET: LazyLock<HashSet<&str>> = LazyLock::new(|| {
    hashset! {"animals8.ztd",
        "awards5.ztd",
        "config5.ztd",
        "global03.ztd",
        "items5.ztd",
        "scenari5.ztd",
        "scenery5.ztd",
        "staff5.ztd",
        "ui5.ztd",
        "animals7.ztd",
        "scn40.ztd",
        "scn41.ztd",
        "scn42.ztd",
        "swordfsh.ztd",
        "swordfsh01.ztd",
        "ui7.ztd",
        "wilddog.ztd",
        "yeti.ztd",
        "animalsa.ztd",
        "antelope.ztd",
        "aqtheme1.ztd",
        "Asian Black Bear.ztd",
        "Asian Elephant.ztd",
        "asian_black_bear.ztd",
        "asian_elephant.ztd",
        "asian_elephant01.ztd",
        "asian_elephant02.ztd",
        "baracuda.ztd",
        "blackbuck.ztd",
        "bongo.ztd",
        "config4.ztd",
        "fancybld.ztd",
        "gallim.ztd",
        "gallim01.ztd",
        "global06.ztd",
        "guests7.ztd",
        "items7.ztd",
        "kidsdino.ztd",
        "komodo.ztd",
        "llama.ztd",
        "magnet.ztd",
        "magnet01.ztd",
        "magnet02.ztd",
        "megathrm.ztd",
        "mexipack.ztd",
        "mountainlion.ztd",
        "mountainlion01.ztd",
        "mt_lion.ztd",
        "na_theme.ztd",
        "object06.ztd",
        "plateo.ztd",
        "plateo01.ztd",
        "reindeer.ztd",
        "researm.ztd",
        "scenari7.ztd",
        "scenery7.ztd",
        "scn16.ztd",
        "scn17.ztd",
        "scn18.ztd",
        "ai.ztd",
        "ambient.ztd",
        "animals.ztd",
        "animals2.ztd",
        "awards.ztd",
        "config.ztd",
        "fences.ztd",
        "freeform.ztd",
        "fringe.ztd",
        "global.ztd",
        "guests.ztd",
        "items.ztd",
        "objects.ztd",
        "paths.ztd",
        "paths2.ztd",
        "research.ztd",
        "scenario.ztd",
        "scenery.ztd",
        "select.ztd",
        "sounds.ztd",
        "staff.ztd",
        "terrain.ztd",
        "tiles.ztd",
        "ui.ztd",
        "ztatb00.ztd",
        "ztatb0a.ztd",
        "ztatb0b.ztd",
        "ztatb0d.ztd",
        "ztatb01.ztd",
        "ztatb02.ztd",
        "ztatb03.ztd",
        "ztatb04.ztd",
        "ztatb05.ztd",
        "ztatb06.ztd",
        "ztatb07.ztd",
        "ztatb08.ztd",
        "ztatb09.ztd",
        "ztatb10.ztd",
        "zts.ztd",
    }
});

const CUSTOM_CONTENT_EXPANSION_STRING_PREFIX: &str = "openzt_";
const CUSTOM_CONTENT_EXPANSION_STRING_ALL: &str = "all";
const CUSTOM_CONTENT_EXPANSION_STRING_SUBDIR: &str = "subdir_";

const EXPANSION_LIST_START: u32 = 0x00639030;
const EXPANSION_SIZE: u32 = 0x14;
const EXPANSION_CURRENT: u32 = 0x00638d4c;

const EXPANSION_ZT_RESOURCE_PREFIX: &str = "ui/sharedui/listbk/";
const EXPANSION_OPENZT_RESOURCE_PREFIX: &str = "openzt.patches.expansion";
const EXPANSION_RESOURCE_ANI: &str = "listbk.ani";
const EXPANSION_RESOURCE_LYT: &str = "ui/xpac.lyt";
const EXPANSION_RESOURCE_PAL: &str = "listbk.pal";
const EXPANSION_RESOURCE_ANIMATION: &str = "listbk.animation";

/// Mutex to store the contents of each `member` set, determined by the `member` section in the `uca`, `ucs`, `ucb`, and `ai` files
static MEMBER_SETS: LazyLock<Mutex<HashMap<String, HashSet<String>>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

/// Mutex to store entity-to-archive mapping for custom expansion filtering
/// Key: entity filename (without extension), Value: archive filename (e.g., "my_mod.ztd")
static ENTITY_TO_ARCHIVE: LazyLock<Mutex<HashMap<String, String>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn add_member(entity_name: String, member: String) {
    let mut data_mutex = MEMBER_SETS.lock().unwrap();

    let set = data_mutex.entry(member).or_default();
    set.insert(entity_name);
}

pub fn is_member(entity_name: &str, member: &str) -> bool {
    let data_mutex = MEMBER_SETS.lock().unwrap();
    match data_mutex.get(member) {
        Some(set) => set.contains(entity_name),
        None => false,
    }
}

pub fn get_members(member: &str) -> Option<HashSet<String>> {
    let data_mutex = MEMBER_SETS.lock().unwrap();
    data_mutex.get(member).cloned()
}

/// Add entity-to-archive mapping
fn add_entity_to_archive_mapping(entity_name: String, archive_name: String) {
    let mut mapping = ENTITY_TO_ARCHIVE.lock().unwrap();
    mapping.insert(entity_name, archive_name);
}

/// Get all entities from a specific archive
/// Returns a HashSet of entity filenames (without extension)
fn get_entities_from_archive(archive_name: &str) -> Option<HashSet<String>> {
    let mapping = ENTITY_TO_ARCHIVE.lock().unwrap();

    // Normalize archive name for matching (case-insensitive, strip .ztd if present)
    let normalized_archive = if archive_name.to_lowercase().ends_with(".ztd") {
        archive_name.to_lowercase()
    } else {
        format!("{}.ztd", archive_name.to_lowercase())
    };

    // Collect all entities from this archive
    let entities: HashSet<String> = mapping
        .iter()
        .filter(|(_, archive)| {
            // Strip "zip::" prefix if present for comparison
            let archive_stripped = archive.strip_prefix("zip::").unwrap_or(archive);
            archive_stripped.to_lowercase() == normalized_archive
        })
        .map(|(entity, _)| entity.clone())
        .collect();

    if entities.is_empty() {
        None
    } else {
        Some(entities)
    }
}

fn get_cc_expansion_name_all() -> String {
    CUSTOM_CONTENT_EXPANSION_STRING_PREFIX.to_string() + CUSTOM_CONTENT_EXPANSION_STRING_ALL
}

fn get_cc_expansion_name(subdir: &str) -> String {
    CUSTOM_CONTENT_EXPANSION_STRING_PREFIX.to_string() + CUSTOM_CONTENT_EXPANSION_STRING_SUBDIR + subdir
}

/// Mutex containing all expansions
static EXPANSION_ARRAY: LazyLock<Mutex<Vec<Expansion>>> = LazyLock::new(|| Mutex::new(Vec::new()));

/// Adds to the expansion mutex and saves to ZT memory
fn add_expansion(expansion: Expansion, save_to_memory: bool) -> anyhow::Result<()> {
    let mut data_mutex = EXPANSION_ARRAY.lock().unwrap();

    data_mutex.push(expansion);

    data_mutex.sort_by_key(|k| k.expansion_id);

    if save_to_memory {
        inner_save_mutex(data_mutex);
    }

    Ok(())
}

fn get_expansion(expansion_id: u32) -> Option<Expansion> {
    let data_mutex = EXPANSION_ARRAY.lock().unwrap();
    data_mutex.iter().find(|expansion| expansion.expansion_id == expansion_id).cloned()
}

fn save_mutex() {
    let data_mutex = EXPANSION_ARRAY.lock().unwrap();
    inner_save_mutex(data_mutex)
}

fn inner_save_mutex(mut mutex_guard: MutexGuard<Vec<Expansion>>) {
    let array_ptr = mutex_guard.as_mut_ptr();
    let array_end_ptr = unsafe { array_ptr.offset(isize::try_from(mutex_guard.len()).unwrap()) };
    let array_buffer_end_ptr = unsafe { array_ptr.offset(isize::try_from(mutex_guard.capacity()).unwrap()) };
    info!(
        "Saving expansions to {:#x} to {:#x}; {:#x}",
        array_ptr as u32, array_end_ptr as u32, array_buffer_end_ptr as u32
    );

    save_expansion_list_to_memory(ExpansionList {
        array_start: array_ptr as u32,
        array_end: array_end_ptr as u32,
        buffer_end: array_end_ptr as u32,
    });
}

fn get_expansions() -> Vec<Expansion> {
    EXPANSION_ARRAY.lock().unwrap().clone()
}

#[derive(Debug)]
#[repr(C)]
struct ExpansionList {
    array_start: u32,
    array_end: u32,
    buffer_end: u32,
}

#[derive(Debug, Clone)]
#[repr(C)]
pub struct Expansion {
    expansion_id: u32,
    name_id: u32,
    name_string_start_ptr: u32,
    name_string_end_ptr: u32,
    name_string_buffer_end_ptr: u32,
}

impl Expansion {
    fn name_string(&self) -> String {
        get_string_from_memory_bounded(self.name_string_start_ptr, self.name_string_end_ptr, self.name_string_buffer_end_ptr)
    }
}

fn read_expansion_list_from_memory() -> ExpansionList {
    get_from_memory(EXPANSION_LIST_START)
}

fn read_expansion_from_memory(address: u32) -> Expansion {
    get_from_memory(address)
}

fn read_expansions_from_memory() -> Vec<Expansion> {
    let expansion_list = read_expansion_list_from_memory();
    info!(
        "Reading expansions from {:#x} to {:#x}, len {}",
        expansion_list.array_start,
        expansion_list.array_end,
        (expansion_list.array_end - expansion_list.array_start) / EXPANSION_SIZE
    );
    let mut expansions = Vec::new();
    let mut current_expansion_address = expansion_list.array_start;
    while current_expansion_address < expansion_list.array_end {
        expansions.push(read_expansion_from_memory(current_expansion_address));
        current_expansion_address += EXPANSION_SIZE;
    }
    expansions
}

fn read_current_expansion() -> Option<Expansion> {
    let current_expansion_id = get_from_memory(EXPANSION_CURRENT);
    match get_expansion(current_expansion_id) {
        Some(expansion) => Some(expansion),
        None => {
            info!("Current expansion not found");
            None
        }
    }
}

fn save_current_expansion(expansion_id: u32) {
    save_to_memory(EXPANSION_CURRENT, expansion_id);
}

fn save_expansion_list_to_memory(expansion_list: ExpansionList) {
    save_to_memory(EXPANSION_LIST_START, expansion_list);
}

impl Display for Expansion {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Expansion {{ expansion_id: {:#x} name_id: {:#x} name_string: {} }}",
            self.expansion_id,
            self.name_id,
            get_string_from_memory(self.name_string_start_ptr)
        )
    }
}

impl Display for ExpansionList {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ExpansionList {{ array_start: {:#x} array_end: {:#x} buffer_end: {:#x} }}",
            self.array_start, self.array_end, self.buffer_end
        )
    }
}

#[detour_mod]
pub mod custom_expansion {
    use tracing::debug;
    // use openzt_detour::{ZTUI_GENERAL_ENTITY_TYPE_IS_DISPLAYED, ZTUI_EXPANSIONSELECT_SETUP};
    use openzt_detour::gen::ztui_expansionselect::SETUP;
    use openzt_detour::gen::ztui_general::ENTITY_TYPE_IS_DISPLAYED;

    use super::{initialise_expansions, read_current_expansion};
    use crate::{bfentitytype::read_zt_entity_type_from_memory, ztui::get_current_buy_tab};

    #[detour(ENTITY_TYPE_IS_DISPLAYED)]
    pub unsafe extern "cdecl" fn ztui_general_entity_type_is_displayed(bf_entity: u32, param_1: u32, param_2: u32) -> bool {
        // TODO: Put this call and subsequent log behind OpenZT debug flag)
        let result = unsafe { ENTITY_TYPE_IS_DISPLAYED_DETOUR.call(bf_entity, param_1, param_2) };

        let Some(current_expansion) = read_current_expansion() else {
            return false;
        };

        let entity = read_zt_entity_type_from_memory(bf_entity);

        let Some(current_buy_tab) = get_current_buy_tab() else {
            return false;
        };

        let reimplemented_result = super::filter_entity_type(&current_buy_tab, &current_expansion, &entity);

        // TODO: Put this log behind OpenZT debug flag (and ignore if a custom expansion is selected)
        if result != reimplemented_result {
            debug!("Filtering mismatch {} {} ({} vs {})", entity, current_buy_tab, result, reimplemented_result);
        }

        reimplemented_result
    }

    #[detour(SETUP)]
    pub unsafe extern "stdcall" fn ztui_expansionselect_setup() {
        unsafe { SETUP_DETOUR.call() }; //TODO: Remove this call once all functionality has been replicated, need to figure out why removing is causes crashes currently

        initialise_expansions();
    }
}

fn create_custom_expansions() {
    info!("create_custom_expansions() - starting");
    info!("create_custom_expansions() - getting config");
    let config = get_openzt_config();
    info!(
        "create_custom_expansions() - got config, {} custom expansions to process",
        config.expansions.custom.len()
    );
    let mut expansion_id = 0x5; // Start after official expansion IDs

    for (expansion_name, items) in &config.expansions.custom {
        info!("create_custom_expansions() - processing expansion: '{}'", expansion_name);
        if expansion_id >= 0x1000 {
            error!("Maximum custom expansions reached");
            break;
        }

        // Create member set name for this expansion
        let member_set_name = format!("openzt_custom_{}", expansion_id);

        // Process each item in the expansion's list
        for item in items {
            if item.to_lowercase().ends_with(".ztd") {
                // Item is an archive - add all entities from it
                info!("create_custom_expansions() - getting entities from archive: '{}'", item);
                if let Some(entities) = get_entities_from_archive(item) {
                    let entity_count = entities.len();
                    for entity_name in entities {
                        add_member(entity_name, member_set_name.clone());
                    }
                    info!("Added {} entities from archive '{}' to expansion '{}'", entity_count, item, expansion_name);
                } else {
                    info!("No entities found from archive '{}' for expansion '{}'", item, expansion_name);
                }
            } else {
                // Item is an entity name
                add_member(item.clone(), member_set_name.clone());
                info!("Added entity '{}' to expansion '{}'", item, expansion_name);
            }
        }

        // Check if we have any members before creating expansion
        if let Some(members) = get_members(&member_set_name) {
            if !members.is_empty() {
                info!(
                    "create_custom_expansions() - adding expansion to game: '{}' with {} members",
                    expansion_name,
                    members.len()
                );
                add_expansion_with_string_value(
                    expansion_id,
                    member_set_name.clone(),
                    expansion_name.clone(),
                    false, // Don't save yet
                );
                info!(
                    "Created custom expansion '{}' (ID: {:#x}) with {} members",
                    expansion_name,
                    expansion_id,
                    members.len()
                );
                expansion_id += 1;
            } else {
                info!("Skipping expansion '{}' - no valid entities found", expansion_name);
            }
        }
    }
    info!("create_custom_expansions() - finished");
}

fn initialise_expansions() {
    info!("initialise_expansions() - starting");
    info!("initialise_expansions() - adding 'all' expansion");
    add_expansion_with_string_id(0x0, "all".to_string(), 0x5974, false);
    info!("initialise_expansions() - checking for custom content members");
    if let Some(member_hash) = get_members(&get_cc_expansion_name_all()) {
        if !member_hash.is_empty() {
            info!("initialise_expansions() - adding 'Custom Content' expansion");
            add_expansion_with_string_value(0x4, get_cc_expansion_name_all(), "Custom Content".to_string(), false);
        }
    }

    // Add custom expansions from config
    info!("initialise_expansions() - about to call create_custom_expansions()");
    create_custom_expansions();
    info!("initialise_expansions() - create_custom_expansions() returned");

    // Save all expansions to memory and resize dropdown
    info!("initialise_expansions() - saving mutex");
    save_mutex();
    let number_of_expansions = get_expansions().len();
    info!("initialise_expansions() - total expansions: {}", number_of_expansions);

    if number_of_expansions > 4 {
        info!("initialise_expansions() - resizing dropdown");
        resize_expansion_dropdown(number_of_expansions as u32);
    }

    info!("initialise_expansions() - setting current expansion to 0x0");
    save_current_expansion(0x0);
    info!("initialise_expansions() - finished");
}

fn resize_expansion_dropdown(number_of_expansions: u32) {
    let mut number_of_additional_expansions = number_of_expansions as i32 - 4;
    if number_of_additional_expansions > 22 {
        number_of_additional_expansions = 22;
    }
    info!("Resizing expansion dropdown to fit {} extra expansions", number_of_additional_expansions);

    if let Err(err) = modify_ztfile_as_ini(EXPANSION_RESOURCE_LYT, |cfg| {
        cfg.set("background", "animation", Some(EXPANSION_OPENZT_RESOURCE_PREFIX.to_string() + "." + "listbk"));
        cfg.set("background", "layer", Some("1".to_string()));
        let additional_spots_needed = number_of_additional_expansions - 2;
        if additional_spots_needed <= 0 {
            return Ok(());
        }

        let old_y = cfg.get_parse::<i32>("list", "dy").unwrap_or(Some(90)).unwrap_or(90);
        let new_y = old_y + (additional_spots_needed * 15) + 5;
        cfg.set("list", "dy", Some(new_y.to_string()));
        cfg.set("list", "scrollbar", Some("22904".to_string()));
        cfg.set("list", "layer", Some("2".to_string()));
        cfg.set("list", "x", Some("13".to_string()));
        cfg.set("list", "dx", Some("101".to_string()));
        cfg.set("LayoutInfo", "dy", Some(new_y.to_string()));

        cfg.set("scrollbar", "type", Some("UIScrollBar".to_string()));
        cfg.set("scrollbar", "id", Some("22904".to_string()));
        cfg.set("scrollbar", "animation", Some("ui/sharedui/scrolbck/scrolbck".to_string()));
        cfg.set("scrollbar", "upArrow", Some("ui/sharedui/arowup/arowup".to_string()));
        cfg.set("scrollbar", "thumb", Some("ui/sharedui/thumb/thumb".to_string()));
        cfg.set("scrollbar", "downArrow", Some("ui/sharedui/arowdn/arowdn".to_string()));
        cfg.set("scrollbar", "layer", Some("0".to_string()));

        Ok(())
    }) {
        info!("Error resizing expansion dropdown 'ani' file: {}", err);
    }

    let animation_resource_string = format!("{}.{}", EXPANSION_OPENZT_RESOURCE_PREFIX, EXPANSION_RESOURCE_ANIMATION);

    if let Err(err) = modify_ztfile_as_ini(&format!("{}.{}", EXPANSION_OPENZT_RESOURCE_PREFIX, EXPANSION_RESOURCE_ANI), |cfg| {
        let old_y0 = cfg.get_parse::<i32>("animation", "y0").unwrap_or(Some(-34)).unwrap_or(-34);
        let new_y0 = old_y0 - (number_of_additional_expansions * 10);
        let old_y1 = cfg.get_parse::<i32>("animation", "y1").unwrap_or(Some(34)).unwrap_or(34);
        let new_y1 = old_y1 + (number_of_additional_expansions * 10);
        cfg.set("animation", "y0", Some(new_y0.to_string()));
        cfg.set("animation", "y1", Some(new_y1.to_string()));
        cfg.set("animation", "dir0", Some(OPENZT_DIR0.to_string()));
        cfg.set("animation", "dir1", Some(animation_resource_string.clone()));
        cfg.remove_key("animation", "dir2");
        Ok(())
    }) {
        info!("Error resizing expansion dropdown 'ani' file: {}", err);
    }
    info!("Check");
    let animation_result = modify_ztfile_as_animation(&animation_resource_string, |animation| {
        for _ in 0..number_of_additional_expansions {
            animation
                .duplicate_pixel_rows(0, 10, 25)
                .map_err(|e| anyhow!("Error duplicating pixel rows when modifying animation: {}", e))?;
        }
        animation.frames[0].vertical_offset_y += number_of_additional_expansions as u16 * 10;
        animation.set_palette_filename(format!("{}.{}", EXPANSION_OPENZT_RESOURCE_PREFIX, EXPANSION_RESOURCE_PAL));
        Ok(())
    });
    if let Err(e) = animation_result {
        info!("Error resizing expansion dropdown animation: {}", e);
    }
}

fn filter_entity_type(buy_tab: &BuyTab, current_expansion: &Expansion, entity: &ZTEntityType) -> bool {
    // TODO: ZTEntityType should also be considered when filtering, not just members
    match buy_tab {
        BuyTab::Animal => {
            if !entity.is_member("animals".to_string()) {
                return false;
            }
            match get_selected_sex() {
                Some(sex) => {
                    if &sex.to_string() != entity.zt_sub_type() {
                        return false;
                    }
                }
                None => return false,
            }
        }
        BuyTab::Shelter => {
            if !entity.is_member("shelters".to_string()) {
                return false;
            }
        }
        BuyTab::Toys => {
            if !entity.is_member("toys".to_string()) {
                return false;
            }
        }
        BuyTab::ShowToys => {
            if !entity.is_member("showtoys".to_string()) {
                return false;
            }
        }
        BuyTab::Building => {
            if !entity.is_member("structures".to_string()) {
                return false;
            }
        }
        BuyTab::Scenery => {
            if !entity.is_member("scenery".to_string()) {
                return false;
            }
            // TODO: Make member name a combination of name and class so name double-ups don't cause this issue
            if entity.class() == &ZTEntityTypeClass::Scenery && entity.zt_type() == "other" && entity.zt_sub_type() == "fountain" {
                return false;
            }
        }
        BuyTab::Fence => {
            if !entity.is_member("fence".to_string()) {
                return false;
            }
            if entity.zt_sub_type() == "g" {
                return false;
            }
        }
        BuyTab::Path => {
            if !entity.is_member("paths".to_string()) {
                return false;
            }
        }
        BuyTab::Foliage => {
            if !entity.is_member("foliage".to_string()) || entity.class == ZTEntityTypeClass::Food {
                return false;
            }
        }
        BuyTab::Rocks => {
            if !entity.is_member("rocks".to_string()) {
                return false;
            }
        }
        BuyTab::Staff => {
            if !entity.is_member("staff".to_string()) {
                return false;
            }
            if (matches!(entity.zt_sub_type().as_str(), "m" | "f") && entity.zt_sub_type() != &get_random_sex().unwrap_or(Sex::Male).to_string()) {
                return false;
            }
        }
        BuyTab::Developer => {
            if !entity.is_member("developer".to_string()) {
                return false;
            }
        }
        BuyTab::PaintTerrain | BuyTab::Terraform => return false,
    }

    if buy_tab != &BuyTab::Path {
        if current_expansion.expansion_id == 0x1 {
            for expansion in get_expansions() {
                if expansion.expansion_id > 0x1 && entity.is_member(expansion.name_string()) && !entity.is_member("zoo".to_string()) {
                    return false;
                }
            }
        }
        if current_expansion.expansion_id > 0x1 && !entity.is_member(current_expansion.name_string()) {
            return false;
        }
    }

    true
}

fn add_expansion_with_string_id(id: u32, name: String, string_id: u32, save_to_memory: bool) {
    let name_len = name.len();
    let name_ptr = match CString::new(name.clone()) {
        Ok(name_string_c_string) => name_string_c_string.into_raw() as u32,
        Err(e) => {
            error!("Error creating CString from name {}, expansion not added: {}", name, e);
            return;
        }
    };
    let name_ptr_end = name_ptr + name_len as u32 + 1;
    if let Err(err) = add_expansion(
        Expansion {
            expansion_id: id,
            name_id: string_id,
            name_string_start_ptr: name_ptr,
            name_string_end_ptr: name_ptr_end,
            name_string_buffer_end_ptr: name_ptr_end,
        },
        save_to_memory,
    ) {
        error!("Error adding expansion: {}", err);
    }
}

fn add_expansion_with_string_value(expansion_id: u32, name: String, string_value: String, save_to_memory: bool) {
    let name_len = name.len();
    let Ok(name_string_c_string) = CString::new(name.clone()) else {
        error!("Error creating CString from name: {}", name);
        return;
    };
    let name_string_start_ptr = name_string_c_string.into_raw() as u32;
    let name_string_end_ptr = name_string_start_ptr + name_len as u32 + 1;
    let name_id = add_string_to_registry(string_value.clone());
    if let Err(err) = add_expansion(
        Expansion {
            expansion_id,
            name_id,
            name_string_start_ptr,
            name_string_end_ptr,
            name_string_buffer_end_ptr: name_string_end_ptr,
        },
        save_to_memory,
    ) {
        error!("Error adding expansion: {}", err);
    }
}

fn parse_member_config(path: &str, file_name: &str, file: Ini) -> anyhow::Result<()> {
    debug!("Parsing member config {} {}", path, file_name);
    let filename = Path::new(&file_name.to_ascii_lowercase())
        .file_stem()
        .with_context(|| format!("failed to parse member config {}", file_name))?
        .to_str()
        .with_context(|| format!("failed to parse member config {}", file_name))?
        .to_string();

    // Extract archive name from path and add entity-to-archive mapping
    let archive_path = Path::new(path.strip_prefix("zip::").unwrap_or(path));
    if let Some(archive_file_name) = archive_path.file_name() {
        if let Some(archive_str) = archive_file_name.to_str() {
            add_entity_to_archive_mapping(filename.clone(), archive_str.to_string());
        }
    }

    // TODO: get_keys shouldn't need a mutable ini
    if let Some(keys) = file.clone().get_keys("Member") {
        for key in keys {
            add_member(filename.clone(), key);
        }
    }

    if is_cc(path) {
        add_member(filename, CUSTOM_CONTENT_EXPANSION_STRING_PREFIX.to_string() + CUSTOM_CONTENT_EXPANSION_STRING_ALL);
    }

    Ok(())
}

fn is_cc(archive: &str) -> bool {
    let path = Path::new(archive.strip_prefix("zip::").unwrap_or(archive));
    let Some(parent) = path.parent() else {
        return false;
    };

    match parent.file_name().unwrap_or_default().to_str().unwrap_or_default() {
        "zupdate" | "xpack1" | "zupdate1" | "xpack2" => false,
        "dlupdate" | "dupdate" | "updates" | "" => match path.file_name().unwrap_or_default().to_str().unwrap_or_default() {
            "" => false,
            file_name => !OFFICIAL_FILESET.contains(file_name),
        },
        _ => true,
    }
}

fn parse_expansion_config(expansion_cfg: &Ini) -> anyhow::Result<()> {
    debug!("Parsing expansion config");
    let mut id: u32 = expansion_cfg
        .get_parse("expansion", "id")
        .map_err(anyhow::Error::msg)?
        .context("No id found in expansion config")?;
    id += 1;
    let name = expansion_cfg
        .get("expansion", "name")
        .context("No name found in expansion config")?
        .to_ascii_lowercase();
    let name_ptr = match CString::new(name.clone()) {
        Ok(name_string_c_string) => name_string_c_string.into_raw() as u32,
        Err(e) => {
            error!("Error creating CString from name: {} -> {}", name, e);
            return Ok(());
        }
    };
    let listid: u32 = expansion_cfg
        .get_parse("expansion", "listid")
        .map_err(anyhow::Error::msg)?
        .context("No listid found in expansion config")?;

    info!("Adding expansion: {}", name);
    add_expansion(
        Expansion {
            expansion_id: id,
            name_id: listid,
            name_string_start_ptr: name_ptr,
            name_string_end_ptr: name_ptr + name.len() as u32 + 1,
            name_string_buffer_end_ptr: name_ptr + name.len() as u32 + 1,
        },
        false,
    )?;

    Ok(())
}

fn handle_expansion_config(path: &str, _: &str, file: Ini) -> Option<(String, String, Ini)> {
    if let Err(e) = parse_expansion_config(&file) {
        error!("Error parsing expansion config: {} {}", path, e);
    }
    None
}

fn handle_member_parsing(path: &str, file_name: &str, file: Ini) -> Option<(String, String, Ini)> {
    if let Err(e) = parse_member_config(path, file_name, file) {
        error!("Error parsing member config: {} {}", path, e)
    }
    None
}

fn handle_expansion_dropdown_ani(path: &str, file_name: &str, file: Ini) -> Option<(String, String, Ini)> {
    let new_file_string = format!(
        "{}.{}",
        EXPANSION_OPENZT_RESOURCE_PREFIX,
        file_name.strip_prefix(EXPANSION_ZT_RESOURCE_PREFIX).unwrap_or(file_name)
    );
    let file_path = Path::new(&new_file_string);
    let Some(file_path_string) = file_path.to_str() else {
        error!("Error converting file path to string");
        return None;
    };
    Some((path.to_owned(), file_path_string.to_owned(), file))
}

fn handle_expansion_dropdown_raw_bytes(path: &str, file_name: &str, file: Box<[u8]>) -> Option<(String, String, Box<[u8]>)> {
    let new_file_string = format!(
        "{}.{}",
        EXPANSION_OPENZT_RESOURCE_PREFIX,
        file_name.strip_prefix(EXPANSION_ZT_RESOURCE_PREFIX).unwrap_or(file_name)
    );
    let file_path = Path::new(&new_file_string);
    let Some(file_path_string) = file_path.to_str() else {
        error!("Error converting file path to string");
        return None;
    };
    Some((path.to_owned(), file_path_string.to_owned(), file))
}

fn handle_expansion_dropdown_animation(path: &str, _: &str, file: Animation) -> Option<(String, String, Animation)> {
    let new_file_string = format!("{}.{}", EXPANSION_OPENZT_RESOURCE_PREFIX, EXPANSION_RESOURCE_ANIMATION);
    let file_path = Path::new(&new_file_string);
    let Some(file_path_string) = file_path.to_str() else {
        error!("Error converting file path to string");
        return None;
    };
    Some((path.to_owned(), file_path_string.to_owned(), file))
}

fn command_get_members(_: Vec<&str>) -> Result<String, CommandError> {
    let data_mutex = MEMBER_SETS.lock().unwrap();
    let mut result = String::new();

    for (set_name, members) in data_mutex.iter() {
        let members_as_string: Vec<String> = members.iter().cloned().collect();
        result.push_str(&format!("Set: {} -> Members: {}\n", set_name, members_as_string.join(", ")));
    }

    Ok(result)
}

fn command_get_expansions(_args: Vec<&str>) -> Result<String, CommandError> {
    let mut string_array = Vec::new();
    for expansion in read_expansions_from_memory() {
        string_array.push(expansion.to_string());
    }

    Ok(string_array.join("\n"))
}

fn command_get_current_expansion(_args: Vec<&str>) -> Result<String, CommandError> {
    match read_current_expansion() {
        Some(expansion) => Ok(expansion.to_string()),
        None => Ok("No current expansion".to_string()),
    }
}

pub fn init() {
    // list_expansion() - no args
    lua_fn!("list_expansion", "Lists all loaded expansions", "list_expansion()", || {
        match command_get_expansions(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // get_current_expansion() - no args
    lua_fn!("get_current_expansion", "Returns current active expansion", "get_current_expansion()", || {
        match command_get_current_expansion(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // get_members() - no args
    lua_fn!("get_members", "Lists expansion member sets", "get_members()", || {
        match command_get_members(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });
    add_handler(
        Handler::builder()
            .prefix("xpac")
            .suffix("cfg")
            .run_stage(RunStage::BeforeOpenZTMods)
            .ini_handler(handle_expansion_config)
            .build(),
    );
    add_handler(
        Handler::builder()
            .suffix("uca")
            .run_stage(RunStage::AfterFiltering)
            .ini_handler(handle_member_parsing)
            .build(),
    );
    add_handler(
        Handler::builder()
            .suffix("ucs")
            .run_stage(RunStage::AfterFiltering)
            .ini_handler(handle_member_parsing)
            .build(),
    );
    add_handler(
        Handler::builder()
            .suffix("ucb")
            .run_stage(RunStage::AfterFiltering)
            .ini_handler(handle_member_parsing)
            .build(),
    );
    add_handler(
        Handler::builder()
            .suffix("ai")
            .run_stage(RunStage::AfterFiltering)
            .ini_handler(handle_member_parsing)
            .build(),
    );
    add_handler(
        Handler::builder()
            .prefix(EXPANSION_ZT_RESOURCE_PREFIX)
            .run_stage(RunStage::BeforeOpenZTMods)
            .ini_handler(handle_expansion_dropdown_ani)
            .raw_bytes_handler(handle_expansion_dropdown_raw_bytes)
            .animation_handler(handle_expansion_dropdown_animation)
            .build(),
    );
    if unsafe { custom_expansion::init_detours() }.is_err() {
        error!("Error initialising custom expansion detours");
    };
}
