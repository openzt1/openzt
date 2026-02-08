use std::fmt;

use openzt_detour::generated::bfuimgr::GET_ELEMENT_0;
use openzt_detour::generated::ztui_general::GET_SELECTED_ENTITY;
use tracing::info;

use crate::{
    command_console::CommandError,
    lua_fn,
    util::{get_from_memory, get_string_from_memory_bounded, ZTBufferString},
    ztworldmgr::read_zt_entity_from_memory,
};

const BFUIMGR_PTR: u32 = 0x00638de0;

/// UIElementId enum for currently used UI elements
#[derive(Debug)]
pub enum UIElementId {
    AnimalScrollingRegion = 2019,
    AnimalTab = 2075,
    ShelterTab = 3255,
    ToysTab = 3254,
    ShowToysTab = 2076,
    BuyObjectScrollingRegion = 3019,
    BuildingTab = 3050,
    SceneryTab = 3053,
    BuildHabitatScrollingRegion = 3208,
    FenceTab = 3251,
    PathTab = 3056,
    FoliageTab = 3252,
    RocksTab = 3256,
    TerraformScrollingRegion = 3350,
    PaintTerrainTab = 3362,
    TerraformTab = 3361,
    StaffScrollingRegion = 3602,
    MaleButton = 2000,
    FemaleButton = 2001,
    DeveloperScrollingRegion = 5003,
}

#[derive(Debug, PartialEq)]
pub enum BuyTab {
    Animal,
    Shelter,
    Toys,
    ShowToys,
    Building,
    Scenery,
    Fence,
    Path,
    Foliage,
    Rocks,
    PaintTerrain,
    Terraform,
    Staff,
    Developer,
}

impl fmt::Display for BuyTab {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let string = match self {
            BuyTab::Animal => "Animal Tab",
            BuyTab::Shelter => "Shelter Tab",
            BuyTab::Toys => "Toys Tab",
            BuyTab::ShowToys => "Show Toys Tab",
            BuyTab::Building => "Building Tab",
            BuyTab::Scenery => "Scenery Tab",
            BuyTab::Fence => "Fence Tab",
            BuyTab::Path => "Path Tab",
            BuyTab::Foliage => "Foliage Tab",
            BuyTab::Rocks => "Rocks Tab",
            BuyTab::PaintTerrain => "Paint Terrain Tab",
            BuyTab::Terraform => "Terraform Tab",
            BuyTab::Staff => "Staff Tab",
            BuyTab::Developer => "Developer Tab",
        };
        write!(f, "{}", string)
    }
}

//TODO: Add support for the buy young hack
pub enum Sex {
    Male,
    Female,
}

impl fmt::Display for Sex {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let char = match self {
            Sex::Male => 'm',
            Sex::Female => 'f',
        };
        write!(f, "{}", char)
    }
}

const RANDOM_SEX_STRING_PTR: u32 = 0x0063e420;

pub fn init() {
    // get_selected_entity() - no args
    lua_fn!(
        "get_selected_entity",
        "Returns details of the currently selected entity",
        "get_selected_entity()",
        || {
            match command_get_selected_entity(vec![]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // get_element(id) - single u32 arg
    lua_fn!("get_element", "Returns UI element details by ID", "get_element(id)", |id: u32| {
        let id_str = id.to_string();
        match command_get_element(vec![&id_str]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // get_buy_tab() - no args
    lua_fn!("get_buy_tab", "Returns the currently active buy tab", "get_buy_tab()", || {
        match command_get_current_buy_tab(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // ui(callback_name) - single string arg
    lua_fn!("ui", "Calls a UI callback function", "ui(callback_name)", |callback: String| {
        match command_call_ui_callback(vec![&callback]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });
}

fn command_get_selected_entity(_args: Vec<&str>) -> Result<String, CommandError> {
    let get_selected_entity_fn = unsafe { GET_SELECTED_ENTITY.original() };
    let entity_address = unsafe { get_selected_entity_fn() };
    if entity_address == 0 {
        return Err(Into::into("No entity selected"));
    }
    let entity = read_zt_entity_from_memory(entity_address);
    Ok(format!("{:#?}", entity))
}

fn command_get_element(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() != 1 {
        return Err(Into::into("Expected 1 argument"));
    }
    let address = args[0].parse()?;
    let get_element_fn = unsafe { GET_ELEMENT_0.original() };
    let ui_element_addr = unsafe { get_element_fn(BFUIMGR_PTR, address) };
    if ui_element_addr == 0 {
        return Err(Into::into("No element found"));
    }
    let element: UIElement = get_from_memory(ui_element_addr);
    info!("{:#x} {:#x}", address, ui_element_addr);
    Ok(format!("{}", element))
}

fn get_element(id: UIElementId) -> Option<UIElement> {
    let get_element_fn = unsafe { GET_ELEMENT_0.original() };
    let ui_element_addr = unsafe { get_element_fn(BFUIMGR_PTR, id as i32) };
    if ui_element_addr == 0 {
        return None;
    }
    Some(get_from_memory(ui_element_addr))
}

fn command_get_current_buy_tab(_args: Vec<&str>) -> Result<String, CommandError> {
    let tab = get_current_buy_tab();
    Ok(format!("{:?}", tab))
}

pub fn get_current_buy_tab() -> Option<BuyTab> {
    if let Some(asr) = get_element(UIElementId::AnimalScrollingRegion) {
        if !asr.state.is_hidden() {
            if get_element(UIElementId::AnimalTab)?.state.is_selected() {
                return Some(BuyTab::Animal);
            }
            if get_element(UIElementId::ShelterTab)?.state.is_selected() {
                return Some(BuyTab::Shelter);
            }
            if get_element(UIElementId::ToysTab)?.state.is_selected() {
                return Some(BuyTab::Toys);
            }
            if get_element(UIElementId::ShowToysTab)?.state.is_selected() {
                return Some(BuyTab::ShowToys);
            }
        }
    }
    if let Some(osr) = get_element(UIElementId::BuyObjectScrollingRegion) {
        if !osr.state.is_hidden() {
            if get_element(UIElementId::BuildingTab)?.state.is_selected() {
                return Some(BuyTab::Building);
            }
            if get_element(UIElementId::SceneryTab)?.state.is_selected() {
                return Some(BuyTab::Scenery);
            }
        }
    }
    if let Some(hsr) = get_element(UIElementId::BuildHabitatScrollingRegion) {
        if !hsr.state.is_hidden() {
            if get_element(UIElementId::FenceTab)?.state.is_selected() {
                return Some(BuyTab::Fence);
            }
            if get_element(UIElementId::PathTab)?.state.is_selected() {
                return Some(BuyTab::Path);
            }
            if get_element(UIElementId::FoliageTab)?.state.is_selected() {
                return Some(BuyTab::Foliage);
            }
            if get_element(UIElementId::RocksTab)?.state.is_selected() {
                return Some(BuyTab::Rocks);
            }
        }
    }
    if let Some(tsr) = get_element(UIElementId::TerraformScrollingRegion) {
        if !tsr.state.is_hidden() {
            if get_element(UIElementId::PaintTerrainTab)?.state.is_selected() {
                return Some(BuyTab::PaintTerrain);
            }
            if get_element(UIElementId::TerraformTab)?.state.is_selected() {
                return Some(BuyTab::Terraform);
            }
        }
    }
    if let Some(ssr) = get_element(UIElementId::StaffScrollingRegion) {
        if !ssr.state.is_hidden() {
            return Some(BuyTab::Staff);
        }
    }
    if let Some(developer_tab) = get_element(UIElementId::DeveloperScrollingRegion) {
        if !developer_tab.state.is_hidden() {
            return Some(BuyTab::Developer);
        }
    }
    None
}

pub fn get_selected_sex() -> Option<Sex> {
    if get_element(UIElementId::MaleButton)?.state.is_selected() {
        return Some(Sex::Male);
    }
    if get_element(UIElementId::FemaleButton)?.state.is_selected() {
        return Some(Sex::Female);
    }
    None
}

pub fn get_random_sex() -> Option<Sex> {
    let string_address = get_from_memory::<u32>(RANDOM_SEX_STRING_PTR);
    match get_string_from_memory_bounded(string_address, string_address + 4, string_address + 8).as_str() {
        "m" => Some(Sex::Male),
        "f" => Some(Sex::Female),
        _ => None,
    }
}

/// returns the address of the selected entity
pub fn get_selected_entity() -> u32 {
    let get_selected_entity_fn = unsafe { GET_SELECTED_ENTITY.original() };
    unsafe { get_selected_entity_fn() }
}

/// returns the address of the selected entity type
pub fn get_selected_entity_type_address() -> u32 {
    let selected_entity = get_selected_entity();
    if selected_entity == 0 {
        return 0;
    }

    get_from_memory(selected_entity + 0x128)
}

#[derive(Debug)]
#[repr(C)]
pub struct UIElement {
    vftable: u32,
    unknown_u32_1: u32,
    unknown_u32_2: u32,
    unknown_string_1: ZTBufferString,
    string_content: ZTBufferString,
    element_name: ZTBufferString,
    // 25 unknown u32s
    padding: [u8; 76],
    state: UIState,
}

impl fmt::Display for UIElement {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UIElement {{ unknown_u32_1: {:#x}, unknown_u32_2: {:#x}, unknown_string_1: {}, string_content: {}, element_name: {}, state: {} }}",
            self.unknown_u32_1, self.unknown_u32_2, self.unknown_string_1, self.string_content, self.element_name, self.state
        )
    }
}

/// UIState struct for the state of a UI element
/// UIState is a bitfield with the following bits:
/// 0b1: hidden
/// 0b10: disabled
/// 0b100: highlighted
/// 0b1000: selected
/// 0b10_000: extra hidden?
/// 0b10_0000_0000: focused
#[derive(Debug)]
#[repr(C)]
pub struct UIState {
    state: u16,
}

impl UIState {
    fn is_hidden(&self) -> bool {
        0b1 & self.state != 0
    }
    fn is_disabled(&self) -> bool {
        0b10 & self.state != 0
    }
    fn is_highlighted(&self) -> bool {
        0b100 & self.state != 0
    }
    fn is_selected(&self) -> bool {
        0b1000 & self.state != 0
    }
    fn is_extra_hidden(&self) -> bool {
        0b10_000 & self.state != 0
    }
    fn is_focused(&self) -> bool {
        0b10_0000_0000 & self.state != 0
    }
}

impl fmt::Display for UIState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(
            f,
            "UIState {{ hidden: {}, disabled: {}, highlighted: {}, selected: {}, focused: {} }}",
            self.is_hidden(),
            self.is_disabled(),
            self.is_highlighted(),
            self.is_selected(),
            self.is_focused()
        )
    }
}

fn command_call_ui_callback(args: Vec<&str>) -> Result<String, CommandError> {
    if args.len() != 1 {
        return Err(Into::into("Expected 1 argument"));
    }
    let callback_function = match args[0] {
        "click_continue" => unsafe { openzt_detour::generated::ztui::CLICK_CONTINUE.original() },
        "list" => {
            return Ok("click_continue".to_string());
        }
        _ => return Err(Into::into("Unknown UI callback")),
    };
    unsafe {
        callback_function();
    }
    Ok("Success".to_string())
}

fn click_ui_element(_id: UIElementId) {
    // let click_element_fn = unsafe { openzt_detour::ZTUI_CLICK_ELEMENT.original() };
    // unsafe {
    //     click_element_fn(BFUIMGR_PTR, id as u32);
    // }
}
