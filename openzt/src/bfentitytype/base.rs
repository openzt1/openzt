use field_accessor_as_string::FieldAccessorAsString;
use field_accessor_as_string_trait::FieldAccessorAsStringTrait;
use getset::{Getters, Setters};

use crate::command_console::CommandError;
use crate::util::{get_from_memory, get_string_from_memory, ZTBoundedString};
use crate::ztui::get_selected_entity_type_address;

pub trait EntityType: FieldAccessorAsStringTrait {
    // allows setting the configuration of the entity type
    fn set_config(&mut self, config: &str, value: &str) -> Result<String, CommandError> {
        if !self.is_field(config) {
            return Err(CommandError::new(format!("Invalid field name: {}", config)));
        }
        match self.set_field(config, value) {
            Ok(_) => Ok(format!("Set {} to {}", config, value)),
            Err(err) => Err(CommandError::new(format!("Failed to set {}: {}", config, err))),
        }
    }
    fn print_config_integers(&self) -> String;
    fn print_config_floats(&self) -> String;
    fn print_config_strings(&self) -> String;
    fn print_config_details(&self) -> String;
    fn print_config(&self) -> String {
        format!(
            "[Details]\n{}\n[Configurations/Integers]\n{}\n[Configurations/Floats]\n{}\n[Configurations/Strings]\n{}",
            self.print_config_details(),
            self.print_config_integers(),
            self.print_config_floats(),
            self.print_config_strings(),
        )
    }
}

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct BFEntityType {
    pub(crate) vtable: u32,               // 0x000
    pad1: [u8; 0x034],                // ----------------------- padding: 52 bytes
    pub ncolors: u32,                 // 0x038
    pad2: [u8; 0x050 - 0x03C],        // ----------------------- padding: 20 bytes
    pub icon_zoom: bool,              // 0x050
    pad3: [u8; 0x054 - 0x051],        // ----------------------- padding: 3 bytes
    pub expansion_id: bool,           // 0x054
    pub movable: bool,                // 0x055
    pub walkable: bool,               // 0x056
    pub walkable_by_tall: bool,       // 0x057
    pad4: [u8; 0x059 - 0x058],        // ----------------------- padding: 1 byte
    pub rubbleable: bool,             // 0x059
    pad5: [u8; 0x05B - 0x05A],        // ----------------------- padding: 1 byte
    pub use_numbers_in_name: bool,    // 0x05B
    pub uses_real_shadows: bool,      // 0x05C
    pub has_shadow_images: bool,      // 0x05D
    pub force_shadow_black: bool,     // 0x05E
    pad6: [u8; 0x060 - 0x05F],        // ----------------------- padding: 1 byte
    pub draws_late: bool,             // 0x060
    pad7: [u8; 0x064 - 0x061],        // ----------------------- padding: 3 bytes
    pub height: u32,                  // 0x064
    pub depth: u32,                   // 0x068
    pub has_underwater_section: bool, // 0x06C
    pub is_transient: bool,           // 0x06D
    pub uses_placement_cube: bool,    // 0x06E
    pub show: bool,                   // 0x06F
    pub hit_threshold: u32,           // 0x070
    pub avoid_edges: u32,             // 0x074 (How close to the edge of a tank can an entity be placed)
    // TODO: Add to display impl and test these bits, if they work replace usage of ZTEntityType with BFEntityType
    pad8: [u8; 0x080 - 0x078],        // ----------------------- padding: 8 bytes
    pub bf_config_file_ptr: u32,      // 0x080
    pad9: [u8; 0x098 - 0x084],        // ----------------------- padding: 20 bytes
    pub zt_type: ZTBoundedString,     // 0x098
    pub zt_sub_type: ZTBoundedString, // 0x0A0
    pad10: [u8; 0x0B4 - 0x0A8],       // ----------------------- padding: 12 bytes
    pub footprintx: i32,              // 0x0B4
    pub footprinty: i32,              // 0x0B8
    pub footprintz: i32,              // 0x0BC
    pub placement_footprintx: i32,    // 0x0C0
    pub placement_footprinty: i32,    // 0x0C4
    pub placement_footprintz: i32,    // 0x0C8
    pub available_at_startup: bool,   // 0x0CC
    pad11: [u8; 0x100 - 0x0CD],       // ----------------------- padding: 51 bytes
}

const _: () = assert!(std::mem::size_of::<BFEntityType>() == 0x100);

impl BFEntityType {
    // returns the codename of the entity type
    pub fn get_codename(&self) -> String {
        let obj_ptr = self as *const BFEntityType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x0A4))
    }

    // returns the type name of the entity type
    pub fn get_type_name(&self) -> String {
        let obj_ptr = self as *const BFEntityType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x098))
    }

    pub fn get_info_image_name(&self) -> String {
        let obj_ptr = self as *const BFEntityType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x148))
    }

    // prints [colorrep] section of the configuration
    pub fn print_colorrep(&self) -> String {
        // NOTE: ncolors is part of a separate structure in memory withn BFEntityType, so we need to grab the pointer to it first
        // this is temporary until the struct can be fully implemented
        let entity_type_address = get_selected_entity_type_address(); // grab the address of the selected entity type
        let entity_type_print = get_from_memory::<u32>(entity_type_address); // convert the address to a u32 ptr for printing
        let ncolors_ptr = get_from_memory::<u32>(entity_type_print + 0x038);
        let ncolors = get_from_memory::<u32>(ncolors_ptr);

        format!("\n\n[colorrep]\nncolors: {}\n", ncolors)
    }
}

impl EntityType for BFEntityType {
    fn print_config_integers(&self) -> String {
        format!("cIconZoom: {}\ncExpansionID: {}\ncMovable: {}\ncWalkable: {}\ncWalkableByTall: {}\ncRubbleable: {}\ncUseNumbersInName: {}\ncUsesRealShadows: {}\ncHasShadowImages: {}\ncForceShadowBlack: {}\ncDrawsLate: {}\ncHeight: {}\ncDepth: {}\ncHasUnderwaterSection: {}\ncIsTransient: {}\ncUsesPlacementCube: {}\ncShow: {}\ncHitThreshold: {}\ncAvoidEdges: {}\ncFootprintX: {}\ncFootprintY: {}\ncFootprintZ: {}\ncPlacementFootprintX: {}\ncPlacementFootprintY: {}\ncPlacementFootprintZ: {}\ncAvailableAtStartup: {}\n",
                self.icon_zoom as u32,
                self.expansion_id as u32,
                self.movable as u32,
                self.walkable as u32,
                self.walkable_by_tall as u32,
                self.rubbleable as u32,
                self.use_numbers_in_name as u32,
                self.uses_real_shadows as u32,
                self.has_shadow_images as u32,
                self.force_shadow_black as u32,
                self.draws_late as u32,
                self.height,
                self.depth,
                self.has_underwater_section as u32,
                self.is_transient as u32,
                self.uses_placement_cube as u32,
                self.show as u32,
                self.hit_threshold,
                { self.avoid_edges },
                self.footprintx,
                self.footprinty,
                self.footprintz,
                self.placement_footprintx,
                self.placement_footprinty,
                self.placement_footprintz,
                self.available_at_startup as u32,
        )
    }

    fn print_config_floats(&self) -> String {
        String::new()
    }

    fn print_config_strings(&self) -> String {
        String::new()
    }

    // prints misc details of the entity type
    fn print_config_details(&self) -> String {
        format!(
            "\n[Details]\n\nEntity Type Address: {:#x}\nType Name: {}\nCodename: {}\n",
            self as *const BFEntityType as u32,
            self.get_type_name(),
            self.get_codename(),
        )
    }
}
