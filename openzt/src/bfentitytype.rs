// ------------ BFEntityType, Implementation, and Related Functions ------------ //
use std::{fmt, ops::Deref};

use field_accessor_as_string::FieldAccessorAsString;
use field_accessor_as_string_trait::FieldAccessorAsStringTrait;
use getset::{Getters, Setters};
use num_enum::FromPrimitive;
use tracing::info;

use crate::util::ZTBoundedString;
use crate::{
    command_console::CommandError,
    expansions::is_member,
    lua_fn,
    util::{get_from_memory, get_string_from_memory, map_from_memory, Checkable},
    ztui::get_selected_entity_type_address,
    ztworldmgr,
};

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
    vtable: u32,                      // 0x000
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
    pub avoid_edges: bool,            // 0x074
    // TODO: Add to display impl and test these bits, if they work replace usage of ZTEntityType with BFEntityType
    pad8: [u8; 0x080 - 0x075],        // ----------------------- padding: 11 bytes
    pub bf_config_file_ptr: u32,      // 0x080
    pad9: [u8; 0x098 - 0x084],        // ----------------------- padding: 20 bytes
    pub zt_type: ZTBoundedString,     // 0x098
    pub zt_sub_type: ZTBoundedString, // 0x0A4
    pad10: [u8; 0x0B4 - 0x0A8],       // ----------------------- padding: 4 bytes
    pub footprintx: i32,              // 0x0B4
    pub footprinty: i32,              // 0x0B8
    pub footprintz: i32,              // 0x0BC
    pub placement_footprintx: i32,    // 0x0C0
    pub placement_footprinty: i32,    // 0x0C4
    pub placement_footprintz: i32,    // 0x0C8
    pub available_at_startup: bool,   // 0x0CC
    pad11: [u8; 0x100 - 0x0CD],       // ----------------------- padding: 35 bytes
}

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
    fn print_colorrep(&self) -> String {
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
                self.avoid_edges as u32,
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

// ------------ ZTSceneryType, Implementation, and Related Functions ------------ //

// TODO: Need to confirm if any of the below should be in BFEntityType, should look into BFUnitType struct to figure out what is common
// According to Ghidra BFEntityType is 0x144 bytes, with ZTScenery being an extra 0x20 bytes? -> MacOS has BEEntityType at
// Should be able to use the Type init functions to get very accurate sizes and to confirm what strings correspond to what memory locations
// Can also figure out some of the gaps using *Type::loadCharacteristics()
// Does BF/ZTUnitType also load purchase cost, name id etc?
#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTSceneryType {
    #[deref_field]
    pub bfentitytype: BFEntityType, // bytes: 0x100 - 0x000 = 0x100 = 256 bytes
    pub purchase_cost: f32,          // 0x100
    pub name_id: u32,                // 0x104
    pub help_id: u32,                // 0x108
    pub habitat: u32,                // 0x10C
    pub location: u32,               // 0x110
    pub era: u32,                    // 0x114
    pub max_food_units: u32,         // 0x118
    pub stink: bool,                 // 0x11C
    pad3: [u8; 0x120 - 0x11D],       // ----------------------- padding: 3 bytes
    pub esthetic_weight: u32,        // 0x120
    pad4: [u8; 0x128 - 0x124],       // ----------------------- padding: 4 bytes
    pub selectable: bool,            // 0x128
    pub deletable: bool,             // 0x129
    pub foliage: bool,               // 0x12A
    pad6: [u8; 0x12D - 0x12B],       // ----------------------- padding: 2 bytes
    pub auto_rotate: bool,           // 0x12D
    pub land: bool,                  // 0x12E
    pub swims: bool,                 // 0x12F
    pub underwater: bool,            // 0x130
    pub surface: bool,               // 0x131
    pub submerge: bool,              // 0x132
    pub only_swims: bool,            // 0x133
    pub needs_confirm: bool,         // 0x134
    pub gawk_only_from_front: bool,  // 0x135
    pub dead_on_land: bool,          // 0x136
    pub dead_on_flat_water: bool,    // 0x137
    pub dead_underwater: bool,       // 0x138
    pub uses_tree_rubble: bool,      // 0x139
    pub forces_scenery_rubble: bool, // 0x13A
    pub blocks_los: bool,            // 0x13B
    pad7: [u8; 0x168 - 0x13C],       // ----------------------- padding: 51 bytes
}

impl ZTSceneryType {
    pub fn get_info_image_name(&self) -> String {
        let obj_ptr = self as *const ZTSceneryType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x14C))
    }
}

impl EntityType for ZTSceneryType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncPurchaseCost: {}\ncNameID: {}\ncHelpID: {}\ncHabitat: {}\ncLocation: {}\ncEra: {}\ncMaxFoodUnits: {}\ncStink: {}\ncEstheticWeight: {}\ncSelectable: {}\ncDeletable: {}\ncFoliage: {}\ncAutoRotate: {}\ncLand: {}\ncSwims: {}\ncUnderwater: {}\ncSurface: {}\ncSubmerge: {}\ncOnlySwims: {}\ncNeedsConfirm: {}\ncGawkOnlyFromFront: {}\ncDeadOnLand: {}\ncDeadOnFlatWater: {}\ncDeadUnderwater: {}\ncUsesTreeRubble: {}\ncForcesSceneryRubble: {}\ncBlocksLOS: {}\n",
                self.bfentitytype.print_config_integers(),
                self.purchase_cost,
                self.name_id,
                self.help_id,
                self.habitat,
                self.location,
                self.era,
                self.max_food_units,
                self.stink as u32,
                self.esthetic_weight,
                self.selectable as u32,
                self.deletable as u32,
                self.foliage as u32,
                self.auto_rotate as u32,
                self.land as u32,
                self.swims as u32,
                self.underwater as u32,
                self.surface as u32,
                self.submerge as u32,
                self.only_swims as u32,
                self.needs_confirm as u32,
                self.gawk_only_from_front as u32,
                self.dead_on_land as u32,
                self.dead_on_flat_water as u32,
                self.dead_underwater as u32,
                self.uses_tree_rubble as u32,
                self.forces_scenery_rubble as u32,
                self.blocks_los as u32,
        )
    }

    fn print_config_floats(&self) -> String {
        self.bfentitytype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.bfentitytype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.bfentitytype.print_config_details()
    }
}

impl Deref for ZTSceneryType {
    type Target = BFEntityType;
    fn deref(&self) -> &Self::Target {
        &self.bfentitytype
    }
}

impl Checkable for ZTSceneryType {
    fn check(ptr: u32) -> anyhow::Result<()> {
        let entity_type_vtable = ZTEntityTypeClass::from(get_from_memory::<u32>(ptr));
        if !zt_entity_type_class_is(&entity_type_vtable, &ZTEntityTypeClass::Scenery) {
            return Err(anyhow::anyhow!(
                "Incompatible entity type: expected ZTEntityTypeClass::Scenery, found {:?}({:#x})",
                entity_type_vtable,
                get_from_memory::<u32>(ptr)
            ));
        }
        anyhow::Ok(())
    }
}

// ------------ ZTBuildingType, Implementation, and Related Functions ------------ //
#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
struct ZTBuildingType {
    #[deref_field]
    pub ztscenerytype: ZTSceneryType, // bytes: 0x168 - 0x000 = 0x16C = 364 bytes
    pad0: [u8; 0x16C - 0x168],                      // -------------------------- padding: 4 bytes
    pub i_capacity: i32,                            // 0x16C
    pub toy_satisfaction: i32,                      // 0x170
    pub time_inside: i32,                           // 0x174
    pub default_cost: f32,                          // 0x178
    pub low_cost: f32,                              // 0x17C
    pub med_cost: f32,                              // 0x180
    pub high_cost: f32,                             // 0x184
    pub price_factor: f32,                          // 0x188
    pub upkeep: f32,                                // 0x18C
    pad1: [u8; 0x194 - 0x190],                      // -------------------------- padding: 4 bytes
    pub hide_user: bool,                            // 0x194
    pub set_letter_facing: bool,                    // 0x195
    pub draw_user: bool,                            // 0x196
    pub hide_cost_change: bool,                     // 0x197
    pub hide_commerce_info: bool,                   // 0x198
    pub hide_regular_info: bool,                    // 0x199
    pub holds_onto_user: bool,                      // 0x19A
    pub user_tracker: bool,                         // 0x19B
    pub idler: bool,                                // 0x19C
    pub exhibit_viewer: bool,                       // 0x19D
    pad2: [u8; 0x1A0 - 0x19E],                      // -------------------------- padding: 2 bytes
    pub alternate_panel_title: u32,                 // 0x1A0
    pub direct_entrance: bool,                      // 0x1A4
    pub hide_building: bool,                        // 0x1A5
    pub user_stays_outside: bool,                   // 0x1A6
    pub user_teleports_inside: bool,                // 0x1A7
    pub user_uses_exit: bool,                       // 0x1A8
    pub user_uses_entrance_as_emergency_exit: bool, // 0x1A9
    pad3: [u8; 0x1B8 - 0x1AA],                      // -------------------------- padding: 9 bytes
    pub adult_change: i32,                          // 0x1B8
    pub child_change: i32,                          // 0x1BC
    pub hunger_change: i32,                         // 0x1C0
    pub thirst_change: i32,                         // 0x1C4
    pub bathroom_change: i32,                       // 0x1C8
    pub energy_change: i32,                         // 0x1CC
}

impl EntityType for ZTBuildingType {
    // print [Configuration/Floats] section of the configuration
    fn print_config_floats(&self) -> String {
        format!(
            "{}\n\n[Configuration/Floats]\n\ncDefaultCost: {:.2}\ncLowCost: {:.2}\ncMedCost: {:.2}\ncHighCost: {:.2}\ncPriceFactor: {:.2}\ncUpkeep: {:.2}\n",
            self.ztscenerytype.print_config_floats(),
            self.default_cost,
            self.low_cost,
            self.med_cost,
            self.high_cost,
            self.price_factor,
            self.upkeep,
        )
    }

    // prints the [Configuration/Integers] section of the configuration
    fn print_config_integers(&self) -> String {
        format!("{}\ncCapacity: {}\ncToySatisfaction: {}\ncTimeInside: {}\ncHideUser: {}\ncSetLetterFacing: {}\ncDrawUser: {}\ncHideCostChange: {}\ncHideCommerceInfo: {}\ncHideRegularInfo: {}\ncHoldsOntoUser: {}\ncUserTracker: {}\ncIdler: {}\ncExhibitViewer: {}\ncAlternatePanelTitle: {}\ncDirectEntrance: {}\ncHideBuilding: {}\ncUserStaysOutside: {}\ncUserTeleportsInside: {}\ncUserUsesExit: {}\ncUserUsesEntranceAsEmergencyExit: {}\ncAdultChange: {}\ncChildChange: {}\ncHungerChange: {}\ncThirstChange: {}\ncBathroomChange: {}\ncEnergyChange: {}\n",
                self.ztscenerytype.print_config_integers(),
                self.i_capacity,
                self.toy_satisfaction,
                self.time_inside,
                self.hide_user as u32,
                self.set_letter_facing as u32,
                self.draw_user as u32,
                self.hide_cost_change as u32,
                self.hide_commerce_info as u32,
                self.hide_regular_info as u32,
                self.holds_onto_user as u32,
                self.user_tracker as u32,
                self.idler as u32,
                self.exhibit_viewer as u32,
                self.alternate_panel_title,
                self.direct_entrance as u32,
                self.hide_building as u32,
                self.user_stays_outside as u32,
                self.user_teleports_inside as u32,
                self.user_uses_exit as u32,
                self.user_uses_entrance_as_emergency_exit as u32,
                self.adult_change,
                self.child_change,
                self.hunger_change,
                self.thirst_change,
                self.bathroom_change,
                self.energy_change,
        )
    }

    fn print_config_strings(&self) -> String {
        self.ztscenerytype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztscenerytype.print_config_details()
    }
}

impl Deref for ZTBuildingType {
    type Target = ZTSceneryType;
    fn deref(&self) -> &Self::Target {
        &self.ztscenerytype
    }
}

// ------------ ZTFenceType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTFenceType {
    #[deref_field]
    pub ztscenerytype: ZTSceneryType, // bytes: 0x168 - 0x000 = 0x168 = 360 bytes
    pub strength: i32,          // 0x168
    pub life: i32,              // 0x16C
    pub decayed_life: i32,      // 0x170
    pub decayed_delta: i32,     // 0x174
    pub break_sound_atten: i32, // 0x178
    pub open_sound_atten: i32,  // 0x17C
    // break_sound: String, // 0x184
    // open_sound: String, // 0x188
    pad2: [u8; 0x194 - 0x180], // ----------------------- padding: 20 bytes
    pub see_through: bool,     // 0x194
    pub is_jumpable: bool,     // 0x195
    pub is_climbable: bool,    // 0x196
    pub indestructible: bool,  // 0x197
    pub is_electrified: bool,  // 0x198
    pub no_draw_water: bool,   // 0x199
}

impl ZTFenceType {
    fn get_break_sound(&self) -> String {
        let obj_ptr = self as *const ZTFenceType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x184))
    }

    fn get_open_sound(&self) -> String {
        let obj_ptr = self as *const ZTFenceType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x188))
    }
}

impl EntityType for ZTFenceType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncStrength: {}\ncLife: {}\ncDecayedLife: {}\ncDecayedDelta: {}\ncBreakSoundAtten: {}\ncOpenSoundAtten: {}\ncSeeThrough: {}\ncIsJumpable: {}\ncIsClimbable: {}\ncIndestructible: {}\ncIsElectrified: {}\ncNoDrawWater: {}\n", // cBreakSound: {}\ncOpenSound: {}\n",
                self.ztscenerytype.print_config_integers(),
                self.strength,
                self.life,
                self.decayed_life,
                self.decayed_delta,
                self.break_sound_atten,
                self.open_sound_atten,
                self.see_through as u32,
                self.is_jumpable as u32,
                self.is_climbable as u32,
                self.indestructible as u32,
                self.is_electrified as u32,
                self.no_draw_water as u32,
                // self.get_break_sound(),
                // self.get_open_sound(), // TODO: fix this
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztscenerytype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztscenerytype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztscenerytype.print_config_details()
    }
}

impl Deref for ZTFenceType {
    type Target = ZTSceneryType;
    fn deref(&self) -> &Self::Target {
        &self.ztscenerytype
    }
}

// ------------ ZTTankWallType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTTankWallType {
    #[deref_field]
    pub ztfencetype: ZTFenceType, // bytes: 0x19C - 0x168 = 0x34 = 52 bytes
    // pub portal_open_sound: u32, // 0x19C
    // pub portal_close_sound: u32, // 0x1A0
    pub portal_open_sound_atten: i32,  // 0x1A4
    pub portal_close_sound_atten: i32, // 0x1A8
}

impl ZTTankWallType {
    fn get_portal_open_sound(&self) -> String {
        let obj_ptr = self as *const ZTTankWallType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x1A4))
    }

    fn get_portal_close_sound(&self) -> String {
        let obj_ptr = self as *const ZTTankWallType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x1B0))
    }

    fn print_portal_sounds(&self) -> String {
        format!(
            "\n\n[PortalSounds]\ncPortalOpenSound: {}\ncPortalCloseSound: {}\ncPortalOpenSoundAtten: {}\ncPortalCloseSoundAtten: {}\n\n",
            self.get_portal_open_sound(),
            self.portal_open_sound_atten,
            self.get_portal_close_sound(),
            self.portal_close_sound_atten,
        )
    }
}

impl EntityType for ZTTankWallType {
    fn print_config_integers(&self) -> String {
        self.ztfencetype.print_config_integers()
    }

    fn print_config_floats(&self) -> String {
        self.ztfencetype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztfencetype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        format!("{}\n{}\n", self.ztfencetype.print_config_details(), self.print_portal_sounds())
    }
}

impl Deref for ZTTankWallType {
    type Target = ZTFenceType;
    fn deref(&self) -> &Self::Target {
        &self.ztfencetype
    }
}

// ------------ ZTFoodType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTFoodType {
    #[deref_field]
    pub ztscenerytype: ZTSceneryType,
    // bytes: 0x168 - 0x000 = 0x168 = 360 bytes
    pub keeper_food_type: u32, // 0x168
}

impl EntityType for ZTFoodType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncKeeperFoodType: {}\n", self.ztscenerytype.print_config_integers(), self.keeper_food_type)
    }

    fn print_config_floats(&self) -> String {
        self.ztscenerytype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztscenerytype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztscenerytype.print_config_details()
    }
}

impl Deref for ZTFoodType {
    type Target = ZTSceneryType;
    fn deref(&self) -> &Self::Target {
        &self.ztscenerytype
    }
}

// ------------ ZTTankeFilterType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTTankFilterType {
    #[deref_field]
    pub ztscenerytype: ZTSceneryType, // bytes: 0x168 - 0x000 = 0x168 = 360 bytes
    pub starting_health: i32,             // 0x168
    pub decayed_health: i32,              // 0x16C
    pub decay_time: i32,                  // 0x170
    pub filter_delay: i32,                // 0x174
    pub filter_upkeep: i32,               // 0x178
    pub filter_clean_amount: i32,         // 0x17C
    pub filter_decayed_clean_amount: i32, // 0x180
    // healthy_sound: String, // 0x184
    // decayed_sound: String, // 0x190
    pad1: [u8; 0x19C - 0x184], // ----------------------- padding: 24 bytes
    pub healthy_atten: i32,    // 0x19C
    pub decayed_atten: i32,    // 0x1A0
}

impl ZTTankFilterType {
    fn get_healthy_sound(&self) -> String {
        let obj_ptr = self as *const ZTTankFilterType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x184))
    }

    fn get_decayed_sound(&self) -> String {
        let obj_ptr = self as *const ZTTankFilterType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x190))
    }
}

impl EntityType for ZTTankFilterType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncStartingHealth: {}\ncDecayedHealth: {}\ncDecayTime: {}\ncFilterDelay: {}\ncFilterUpkeep: {}\ncFilterCleanAmount: {}\ncFilterDecayedCleanAmount: {}\ncHealthyAtten: {}\ncDecayedAtten: {}\ncHealthySound: {}\ncDecayedSound: {}\n",
                self.ztscenerytype.print_config_integers(),
                self.starting_health,
                self.decayed_health,
                self.decay_time,
                self.filter_delay,
                self.filter_upkeep,
                self.filter_clean_amount,
                self.filter_decayed_clean_amount,
                self.healthy_atten,
                self.decayed_atten,
                self.get_healthy_sound(),
                self.get_decayed_sound(), // TODO: fix this
        )
    }

    fn print_config_details(&self) -> String {
        format!(
            "{}\n\n[FilterSounds]\n\ncHealthySound: {}\ncHealthyAtten: {}\ncDecayedSound: {}\ncDecayedAtten: {}\n\n",
            self.ztscenerytype.print_config_details(),
            self.get_healthy_sound(),
            self.healthy_atten,
            self.get_decayed_sound(),
            self.decayed_atten
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztscenerytype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztscenerytype.print_config_strings()
    }
}

impl Deref for ZTTankFilterType {
    type Target = ZTSceneryType;
    fn deref(&self) -> &Self::Target {
        &self.ztscenerytype
    }
}

// ------------ ZTPathType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTPathType {
    #[deref_field]
    ztscenerytype: ZTSceneryType,
    // bytes: 0x168 - 0x000 = 0x168 = 360 bytes
    pub material: u32, // 0x168
                       // TODO: missing Shapes structure in paths. Could not find.
}

impl EntityType for ZTPathType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncMaterial: {}\n", self.ztscenerytype.print_config_integers(), self.material,)
    }

    fn print_config_floats(&self) -> String {
        self.ztscenerytype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztscenerytype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztscenerytype.print_config_details()
    }
}

impl Deref for ZTPathType {
    type Target = ZTSceneryType;
    fn deref(&self) -> &Self::Target {
        &self.ztscenerytype
    }
}

// ------------ ZTRubbleType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTRubbleType {
    #[deref_field]
    ztscenerytype: ZTSceneryType,
    // bytes: 0x168 - 0x000 = 0x168 = 360 bytes
    // explosion_sound: String, // 0x168
    pad0: [u8; 0x16C - 0x168],
    // ----------------------- padding: 4 bytes
    pub explosion_sound_atten: i32, // 0x16C
}

impl ZTRubbleType {
    fn get_explosion_sound(&self) -> String {
        let obj_ptr = self as *const ZTRubbleType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x168))
    }
}

impl EntityType for ZTRubbleType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncExplosionSound: {}\ncExplosionSoundAtten: {}\n",
            self.ztscenerytype.print_config_integers(),
            self.get_explosion_sound(),
            self.explosion_sound_atten,
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztscenerytype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztscenerytype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztscenerytype.print_config_details()
    }
}

impl Deref for ZTRubbleType {
    type Target = ZTSceneryType;
    fn deref(&self) -> &Self::Target {
        &self.ztscenerytype
    }
}

// ------------ BFUnitType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct BFUnitType {
    #[deref_field]
    bfentitytype: BFEntityType, // bytes: 0x100 - 0x000 = 0x100 = 256 bytes
    pub slow_rate: u32,         // 0x100
    pub medium_rate: u32,       // 0x104
    pub fast_rate: u32,         // 0x108
    pub slow_anim_speed: u16,   // 0x10C
    pub medium_anim_speed: u16, // 0x10E
    pub fast_anim_speed: u16,   // 0x110
    pad0: [u8; 0x114 - 0x112],  // ----------------------- padding: 2 bytes
    pub min_height: u32,        // 0x114 <--- unsure if accurate
    pub max_height: u32,        // 0x118 <--- unsure if accurate
}

impl EntityType for BFUnitType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncSlowRate: {}\ncMediumRate: {}\ncFastRate: {}\ncSlowAnimSpeed: {}\ncMediumAnimSpeed: {}\ncFastAnimSpeed: {}\ncMinHeight: {}\ncMaxHeight: {}\n",
            self.bfentitytype.print_config_integers(),
            self.slow_rate,
            self.medium_rate,
            self.fast_rate,
            self.slow_anim_speed,
            self.medium_anim_speed,
            self.fast_anim_speed,
            self.min_height,
            self.max_height,
        )
    }

    fn print_config_floats(&self) -> String {
        self.bfentitytype.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.bfentitytype.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.bfentitytype.print_config_details()
    }
}

impl Deref for BFUnitType {
    type Target = BFEntityType;
    fn deref(&self) -> &Self::Target {
        &self.bfentitytype
    }
}

// ------------ ZTUnitType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTUnitType {
    #[deref_field]
    pub bfunit_type: BFUnitType, // bytes: 0x11C - 0x100 = 0x1C = 28 bytes
    pad0: [u8; 0x12C - 0x11C],        // ----------------------- padding: 16 bytes
    pub purchase_cost: f32,           // 0x12C
    pub name_id: i32,                 // 0x130
    pub help_id: i32,                 // 0x134
    pad1: [u8; 0x150 - 0x138],        // ----------------------- padding: 24 bytes
    pub map_footprint: i32,           // 0x150
    pub slow_anim_speed_water: u16,   // 0x154
    pub medium_anim_speed_water: u16, // 0x156
    pub fast_anim_speed_water: u16,   // 0x158
    pad2: [u8; 0x17C - 0x15C],        // ----------------------- padding: 32 bytes
    // pub list_image_name: String,    // 0x168 TODO: fix offset for string getters in unittype
    pub swims: bool,               // 0x17C
    pub surface: bool,             // 0x17D
    pub underwater: bool,          // 0x17E
    pub only_underwater: bool,     // 0x17F
    pad3: [u8; 0x180 - 0x17F],     // ----------------------- padding: 1 byte
    pub skip_trick_happiness: u32, // 0x180 TODO: potentially not accurate
    pub skip_trick_chance: i32,    // 0x184
}

impl ZTUnitType {
    pub fn get_list_name(&self) -> String {
        let obj_ptr = self as *const ZTUnitType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x168))
    }
}

impl EntityType for ZTUnitType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncPurchaseCost: {}\ncNameID: {}\ncHelpID: {}\ncMapFootprint: {}\ncSlowAnimSpeedWater: {}\ncMediumAnimSpeedWater: {}\ncFastAnimSpeedWater: {}\ncSwims: {}\ncSurface: {}\ncUnderwater: {}\ncOnlyUnderwater: {}\ncSkipTrickHappiness: {}\ncSkipTrickChance: {}\n",
                self.bfunit_type.print_config_integers(),
                self.purchase_cost,
                self.name_id,
                self.help_id,
                self.map_footprint,
                self.slow_anim_speed_water,
                self.medium_anim_speed_water,
                self.fast_anim_speed_water,
                self.swims as u32,
                self.surface as u32,
                self.underwater as u32,
                self.only_underwater as u32,
                self.skip_trick_happiness,
                self.skip_trick_chance as u32,
        )
    }

    fn print_config_floats(&self) -> String {
        self.bfunit_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.bfunit_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.bfunit_type.print_config_details()
    }
}

// ------------ ZTGuestType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTGuestType {
    #[deref_field]
    pub ztunit_type: ZTUnitType, // bytes: 0x188 - 0x100 = 0x88 = 136 bytes
    pad00: [u8; 0x1B4 - 0x188],                    // ----------------------- padding: 44 bytes
    pub hunger_check: i32,                         // 0x1B4
    pub thirsty_check: i32,                        // 0x1B8
    pub bathroom_check: i32,                       // 0x1BC
    pub leave_zoo_check: i32,                      // 0x1C0
    pub buy_souvenir_check: i32,                   // 0x1C4
    pub energy_check: i32,                         // 0x1C8
    pub chase_check: i32,                          // 0x1CC
    pub trash_check: i32,                          // 0x1D0
    pub like_animals_check: i32,                   // 0x1D4
    pub viewing_area_check: i32,                   // 0x1D8
    pub environment_effect_check: i32,             // 0x1DC
    pub saw_animal_reset: i32,                     // 0x1E0
    pad01: [u8; 0x1E8 - 0x1E4],                    // ----------------------- padding: 4 bytes
    pub initial_happiness: i32,                    // 0x1E8
    pad02: [u8; 0x200 - 0x1EC],                    // ----------------------- padding: 20 bytes
    pub max_energy: i32,                           // 0x200
    pad03: [u8; 0x210 - 0x204],                    // ----------------------- padding: 12 bytes
    pub energy_increment: i32,                     // 0x210
    pub energy_threshold: i32,                     // 0x214
    pub angry_energy_change: i32,                  // 0x218
    pub hunger_increment: i32,                     // 0x21C
    pub hunger_threshold: i32,                     // 0x220
    pub angry_food_change: i32,                    // 0x224
    pub preferred_food_change: i32,                // 0x228
    pub thirst_increment: i32,                     // 0x22C
    pub thirst_threshold: i32,                     // 0x230
    pub angry_thirst_change: i32,                  // 0x234
    pub bathroom_increment: i32,                   // 0x238
    pub bathroom_threshold: i32,                   // 0x23C
    pub angry_bathroom_change: i32,                // 0x240
    pub price_happy1_change: i32,                  // 0x244
    pub price_angry1_change: i32,                  // 0x248
    pub leave_chance_low: i32,                     // 0x24C
    pub leave_chance_med: i32,                     // 0x250
    pub leave_chance_high: i32,                    // 0x254
    pub leave_chance_done: i32,                    // 0x258
    pub buy_souvenir_chance_med: i32,              // 0x25C
    pub buy_souvenir_chance_high: i32,             // 0x260
    pub angry_trash_change: i32,                   // 0x264
    pub trash_in_tile_threshold: i32,              // 0x268
    pub vandalized_objects_in_tile_threshold: i32, // 0x26C
    pub animal_in_row_change: i32,                 // 0x270
    pub different_species_change: i32,             // 0x274
    pub different_species_threshold: i32,          // 0x278
    pub sick_animal_change: i32,                   // 0x27C
    pub crowded_viewing_threshold: i32,            // 0x280
    pub crowded_viewing_change: i32,               // 0x284
    pub preferred_animal_change: i32,              // 0x288
    pub happy_animal_change1: i32,                 // 0x28C
    pub happy_animal_change2: i32,                 // 0x290
    pub angry_animal_change1: i32,                 // 0x294
    pub angry_animal_change2: i32,                 // 0x298
    pub angry_animal_change3: i32,                 // 0x29C
    pub escaped_animal_change: i32,                // 0x2A0
    pub object_esthetic_threshold: i32,            // 0x2A4
    pub happy_esthetic_change: i32,                // 0x2A8
    pub stand_and_eat_change: i32,                 // 0x2AC
    pub stink_threshold: i32,                      // 0x2B0
    pub sick_chance: i32,                          // 0x2B4
    pub sick_change: i32,                          // 0x2B8
    pub mimic_chance: i32,                         // 0x2BC
    pub test_fence_chance: i32,                    // 0x2C0
    pub zap_happiness_hit: i32,                    // 0x2C4
    pub tap_wall_chance: i32,                      // 0x2C8
}

impl EntityType for ZTGuestType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncHungerCheck: {}\ncThirstyCheck: {}\ncBathroomCheck: {}\ncLeaveZooCheck: {}\ncBuySouvenirCheck: {}\ncEnergyCheck: {}\ncChaseCheck: {}\ncTrashCheck: {}\ncLikeAnimalsCheck: {}\ncViewingAreaCheck: {}\ncEnvironmentEffectCheck: {}\ncSawAnimalReset: {}\ncInitialHappiness: {}\ncMaxEnergy: {}\ncEnergyIncrement: {}\ncEnergyThreshold: {}\ncAngryEnergyChange: {}\ncHungerIncrement: {}\ncHungerThreshold: {}\ncAngryFoodChange: {}\ncPreferredFoodChange: {}\ncThirstIncrement: {}\ncThirstThreshold: {}\ncAngryThirstChange: {}\ncBathroomIncrement: {}\ncBathroomThreshold: {}\ncAngryBathroomChange: {}\ncPriceHappy1Change: {}\ncPriceAngry1Change: {}\ncLeaveChanceLow: {}\ncLeaveChanceMed: {}\ncLeaveChanceHigh: {}\ncLeaveChanceDone: {}\ncBuySouvenirChanceMed: {}\ncBuySouvenirChanceHigh: {}\ncAngryTrashChange: {}\ncTrashInTileThreshold: {}\ncVandalizedObjectsInTileThreshold: {}\ncAnimalInRowChange: {}\ncDifferentSpeciesChange: {}\ncDifferentSpeciesThreshold: {}\ncSickAnimalChange: {}\ncCrowdedViewingThreshold: {}\ncCrowdedViewingChange: {}\ncPreferredAnimalChange: {}\ncHappyAnimalChange1: {}\ncHappyAnimalChange2: {}\ncAngryAnimalChange1: {}\ncAngryAnimalChange2: {}\ncAngryAnimalChange3: {}\ncEscapedAnimalChange: {}\ncObjectEstheticThreshold: {}\ncHappyEstheticChange: {}\ncStandAndEatChange: {}\ncStinkThreshold: {}\ncSickChance: {}\ncSickChange: {}\ncMimicChance: {}\ncTestFenceChance: {}\ncZapHappinessHit: {}\ncTapWallChance: {}\n",
        self.ztunit_type.print_config_integers(),
        self.hunger_check,
        self.thirsty_check,
        self.bathroom_check,
        self.leave_zoo_check,
        self.buy_souvenir_check,
        self.energy_check,
        self.chase_check,
        self.trash_check,
        self.like_animals_check,
        self.viewing_area_check,
        self.environment_effect_check,
        self.saw_animal_reset,
        self.initial_happiness,
        self.max_energy,
        self.energy_increment,
        self.energy_threshold,
        self.angry_energy_change,
        self.hunger_increment,
        self.hunger_threshold,
        self.angry_food_change,
        self.preferred_food_change,
        self.thirst_increment,
        self.thirst_threshold,
        self.angry_thirst_change,
        self.bathroom_increment,
        self.bathroom_threshold,
        self.angry_bathroom_change,
        self.price_happy1_change,
        self.price_angry1_change,
        self.leave_chance_low,
        self.leave_chance_med,
        self.leave_chance_high,
        self.leave_chance_done,
        self.buy_souvenir_chance_med,
        self.buy_souvenir_chance_high,
        self.angry_trash_change,
        self.trash_in_tile_threshold,
        self.vandalized_objects_in_tile_threshold,
        self.animal_in_row_change,
        self.different_species_change,
        self.different_species_threshold,
        self.sick_animal_change,
        self.crowded_viewing_threshold,
        self.crowded_viewing_change,
        self.preferred_animal_change,
        self.happy_animal_change1,
        self.happy_animal_change2,
        self.angry_animal_change1,
        self.angry_animal_change2,
        self.angry_animal_change3,
        self.escaped_animal_change,
        self.object_esthetic_threshold,
        self.happy_esthetic_change,
        self.stand_and_eat_change,
        self.stink_threshold,
        self.sick_chance,
        self.sick_change,
        self.mimic_chance,
        self.test_fence_chance,
        self.zap_happiness_hit,
        self.tap_wall_chance,
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztunit_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztunit_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztunit_type.print_config_details()
    }
}

impl Deref for ZTGuestType {
    type Target = ZTUnitType;
    fn deref(&self) -> &Self::Target {
        &self.ztunit_type
    }
}

// ------------ ZTAnimalType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTAnimalType {
    #[deref_field]
    pub ztunit_type: ZTUnitType, // bytes: 0x188 - 0x100 = 0x88 = 136 bytes
    pad00: [u8; 0x1D8 - 0x188],         // ----------------------- padding: 72 bytes
    pub box_footprint_x: i32,           // 0x1D8
    pub box_footprint_y: i32,           // 0x1DC
    pub box_footprint_z: i32,           // 0x1E0
    pub family: i32,                    // 0x1E4
    pub genus: i32,                     // 0x1E8
    pad01: [u8; 0x1F0 - 0x1EC],         // ----------------------- padding: 4 bytes
    pub habitat: i32,                   // 0x1F0
    pub location: i32,                  // 0x1F4
    pub era: i32,                       // 0x1F8
    pub breath_threshold: i32,          // 0x1FC
    pub breath_increment: i32,          // 0x200
    pad02: [u8; 0x20C - 0x204],         // ----------------------- padding: 8 bytes
    pub hunger_threshold: i32,          // 0x20C
    pub hungry_health_change: i32,      // 0x210
    pub hunger_increment: i32,          // 0x214
    pub food_unit_value: i32,           // 0x218
    pub keeper_food_units_eaten: i32,   // 0x21C
    pub needed_food: i32,               // 0x220
    pub no_food_change: i32,            // 0x224
    pub initial_happiness: i32,         // 0x228
    pad04: [u8; 0x234 - 0x22C],         // ----------------------- padding: 12 bytes
    pub max_hits: i32,                  // 0x234
    pad004: [u8; 0x23C - 0x238],        // ----------------------- padding: 4 bytes
    pub pct_hits: i32,                  // 0x23C
    pad05: [u8; 0x248 - 0x240],         // ----------------------- padding: 8 bytes
    pub max_energy: i32,                // 0x248
    pad07: [u8; 0x250 - 0x24C],         // ----------------------- padding: 4 bytes
    pub max_dirty: i32,                 // 0x250
    pub min_dirty: i32,                 // 0x254
    pub sick_change: i32,               // 0x258
    pub other_animal_sick_change: i32,  // 0x25C
    pub sick_chance: i32,               // 0x260
    pub sick_random_chance: i32,        // 0x264
    pub crowd: i32,                     // 0x268
    pub crowd_happiness_change: i32,    // 0x26C
    pub zap_happiness_change: i32,      // 0x270
    pub captivity: i32,                 // 0x274
    pub reproduction_chance: i32,       // 0x278
    pub reproduction_interval: i32,     // 0x27C
    pub mating_type: i32,               // 0x280
    pub offspring: i32,                 // 0x284
    pub keeper_frequency: i32,          // 0x288
    pad08: [u8; 0x290 - 0x28C],         // ----------------------- padding: 4 bytes
    pub not_enough_keepers_change: i32, // 0x290
    pub social: i32,                    // 0x294
    pub habitat_size: i32,              // 0x298
    pub number_animals_min: i32,        // 0x29C
    pub number_animals_max: i32,        // 0x2A0
    pad09: [u8; 0x2AC - 0x2A4],         // ----------------------- padding: 8 bytes
    pub number_min_change: i32,         // 0x2AC
    pub number_max_change: i32,         // 0x2B0
    pad10: [u8; 0x2BC - 0x2B4],         // ----------------------- padding: 8 bytes
    pub habitat_preference: i32,        // 0x2BC
    pad11: [u8; 0x31C - 0x2C0],         // ----------------------- padding: 92 bytes
    pub baby_born_change: i32,          // 0x31C
    // pad12: [u8; 0x320 - 0x320],         // ----------------------- padding: 4 bytes
    pub energy_increment: i32, // 0x320
    pub energy_threshold: i32, // 0x324
    pub dirty_increment: i32,  // 0x328
    pub dirty_threshold: i32,  // 0x32C
    // pad13: [u8; 0x330 - 0x330],         // ----------------------- padding: 4 bytes
    pub sick_time: i32,         // 0x330
    pad14: [u8; 0x344 - 0x334], // ----------------------- padding: 16 bytes
    pub baby_to_adult: i32,     // 0x344
    // pad15: [u8; 0x348 - 0x348],         // ----------------------- padding: 4 bytes
    pub other_food: i32,                // 0x348
    pub tree_pref: i32,                 // 0x34C
    pub rock_pref: i32,                 // 0x350
    pub space_pref: i32,                // 0x354
    pub elevation_pref: i32,            // 0x358
    pub depth_min: i32,                 // 0x35C
    pub depth_max: i32,                 // 0x360
    pub depth_change: i32,              // 0x364
    pub salinity_change: i32,           // 0x368
    pub salinity_health_change: i32,    // 0x36C
    pad16: [u8; 0x378 - 0x370],         // ----------------------- padding: 8 bytes
    pub happy_reproduce_threshold: i32, // 0x378
    // pad17: [u8; 0x37C - 0x37C],         // ----------------------- padding: 4 bytes
    pub building_use_chance: i32, // 0x37C
    pub no_mate_change: i32,      // 0x380
    pub time_death: i32,          // 0x384
    pub death_chance: i32,        // 0x388
    pub dirt_chance: i32,         // 0x38C
    pub water_needed: i32,        // 0x390
    pub underwater_needed: i32,   // 0x394
    pub land_needed: i32,         // 0x398
    pub enter_water_chance: i32,  // 0x39C
    pub enter_tank_chance: i32,   // 0x3A0
    pub enter_land_chance: i32,   // 0x3A4
    pub drink_water_chance: i32,  // 0x3A8
    pub chase_animal_chance: i32, // 0x3AC
    pub climbs_cliffs: i32,       // 0x3B0
    pub bash_strength: i32,       // 0x3B4
    pub attractiveness: i32,      // 0x3B8
    pad18: [u8; 0x3C8 - 0x3BC],   // ----------------------- padding: 8 bytes
    pub keeper_food_type: i32,    // 0x3C8
    pub is_climber: bool,         // 0x3CC
    pub is_jumper: bool,          // 0x3CD
    pub small_zoodoo: bool,       // 0x3CE
    pub dino_zoodoo: bool,        // 0x3CF
    pub giant_zoodoo: bool,       // 0x3D0
    pub is_special_animal: bool,  // 0x3D1
    pub need_shelter: bool,       // 0x3D2
    pub need_toys: bool,          // 0x3D3
    pub babies_attack: bool,      // 0x3D4
}

impl EntityType for ZTAnimalType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncBoxFootprintX: {}\ncBoxFootprintY: {}\ncBoxFootprintZ: {}\ncFamily: {}\ncGenus: {}\ncHabitat: {}\ncLocation: {}\ncEra: {}\ncBreathThreshold: {}\ncBreathIncrement: {}\ncHungerThreshold: {}\ncHungryHealthChange: {}\ncHungerIncrement: {}\ncFoodUnitValue: {}\ncKeeperFoodUnitsEaten: {}\ncNeededFood: {}\ncNoFoodChange: {}\ncInitialHappiness: {}\ncMaxHits: {}\ncPctHits: {}\ncMaxEnergy: {}\ncMaxDirty: {}\ncMinDirty: {}\ncSickChange: {}\ncOtherAnimalSickChange: {}\ncSickChance: {}\ncSickRandomChance: {}\ncCrowd: {}\ncCrowdHappinessChange: {}\ncZapHappinessChange: {}\ncCaptivity: {}\ncReproductionChance: {}\ncReproductionInterval: {}\ncMatingType: {}\ncOffspring: {}\ncKeeperFrequency: {}\ncNotEnoughKeepersChange: {}\ncSocial: {}\ncHabitatSize: {}\ncNumberAnimalsMin: {}\ncNumberAnimalsMax: {}\ncNumberMinChange: {}\ncNumberMaxChange: {}\ncHabitatPreference: {}\ncBabyBornChange: {}\ncEnergyIncrement: {}\ncEnergyThreshold: {}\ncDirtyIncrement: {}\ncDirtyThreshold: {}\ncSickTime: {}\ncBabyToAdult: {}\ncOtherFood: {}\ncTreePref: {}\ncRockPref: {}\ncSpacePref: {}\ncElevationPref: {}\ncDepthMin: {}\ncDepthMax: {}\ncDepthChange: {}\ncSalinityChange: {}\ncSalinityHealthChange: {}\ncHappyReproduceThreshold: {}\ncBuildingUseChance: {}\ncNoMateChange: {}\ncTimeDeath: {}\ncDeathChance: {}\ncDirtChance: {}\ncWaterNeeded: {}\ncUnderwaterNeeded: {}\ncLandNeeded: {}\ncEnterWaterChance: {}\ncEnterTankChance: {}\ncEnterLandChance: {}\ncDrinkWaterChance: {}\ncChaseAnimalChance: {}\ncClimbsCliffs: {}\ncBashStrength: {}\ncAttractiveness: {}\ncKeeperFoodType: {}\ncIsClimber: {}\ncIsJumper: {}\ncSmallZoodoo: {}\ncDinoZoodoo: {}\ncGiantZoodoo: {}\ncIsSpecialAnimal: {}\ncNeedShelter: {}\ncNeedToys: {}\ncBabiesAttack: {}\n",
        self.ztunit_type.print_config_integers(),
        self.box_footprint_x,
        self.box_footprint_y,
        self.box_footprint_z,
        self.family,
        self.genus,
        self.habitat,
        self.location,
        self.era,
        self.breath_threshold,
        self.breath_increment,
        self.hunger_threshold,
        self.hungry_health_change,
        self.hunger_increment,
        self.food_unit_value,
        self.keeper_food_units_eaten,
        self.needed_food,
        self.no_food_change,
        self.initial_happiness,
        self.max_hits,
        self.pct_hits,
        self.max_energy,
        self.max_dirty,
        self.min_dirty,
        self.sick_change,
        self.other_animal_sick_change,
        self.sick_chance,
        self.sick_random_chance,
        self.crowd,
        self.crowd_happiness_change,
        self.zap_happiness_change,
        self.captivity,
        self.reproduction_chance,
        self.reproduction_interval,
        self.mating_type,
        self.offspring,
        self.keeper_frequency,
        self.not_enough_keepers_change,
        self.social,
        self.habitat_size,
        self.number_animals_min,
        self.number_animals_max,
        self.number_min_change,
        self.number_max_change,
        self.habitat_preference,
        self.baby_born_change,
        self.energy_increment,
        self.energy_threshold,
        self.dirty_increment,
        self.dirty_threshold,
        self.sick_time,
        self.baby_to_adult,
        self.other_food,
        self.tree_pref,
        self.rock_pref,
        self.space_pref,
        self.elevation_pref,
        self.depth_min,
        self.depth_max,
        self.depth_change,
        self.salinity_change,
        self.salinity_health_change,
        self.happy_reproduce_threshold,
        self.building_use_chance,
        self.no_mate_change,
        self.time_death,
        self.death_chance,
        self.dirt_chance,
        self.water_needed,
        self.underwater_needed,
        self.land_needed,
        self.enter_water_chance,
        self.enter_tank_chance,
        self.enter_land_chance,
        self.drink_water_chance,
        self.chase_animal_chance,
        self.climbs_cliffs,
        self.bash_strength,
        self.attractiveness,
        self.keeper_food_type,
        self.is_climber as i32,
        self.is_jumper as i32,
        self.small_zoodoo as i32,
        self.dino_zoodoo as i32,
        self.giant_zoodoo as i32,
        self.is_special_animal as i32,
        self.need_shelter as i32,
        self.need_toys as i32,
        self.babies_attack as i32,
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztunit_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztunit_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztunit_type.print_config_details()
    }
}

impl Deref for ZTAnimalType {
    type Target = ZTUnitType;

    fn deref(&self) -> &Self::Target {
        &self.ztunit_type
    }
}

// ------------ ZTStaffType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTStaffType {
    #[deref_field]
    pub ztunit_type: ZTUnitType, // bytes: 0x188 - 0x100 = 0x88 = 136 bytes
    pad01: [u8; 0x1B4 - 0x188], // ----------------------- padding: 44 bytes
    pub work_check: i32,        // 0x1B4
    pub chase_check: i32,       // 0x1B8
    // pad02: [u8; 0x1BC - 0x1BC], // ----------------------- padding: 4 bytes
    pub monthly_cost: f32, // 0x1BC
    // pub training_icon_name: string ptr, // 0x1D8 TODO: implement string ptr as function getter
    pad03: [u8; 0x1E8 - 0x1C0], // ----------------------- padding: 24 bytes
    pub duties_text_id: i32,    // 0x1E8
    pub weapon_range: i32,      // 0x1EC
}

impl EntityType for ZTStaffType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncWorkCheck: {}\ncChaseCheck: {}\ncDutiesTextID: {}\ncWeaponRange: {}\n",
            self.ztunit_type.print_config_integers(),
            self.work_check,
            self.chase_check,
            self.duties_text_id,
            self.weapon_range,
        )
    }

    fn print_config_floats(&self) -> String {
        format!("{}\ncMonthlyCost: {}\n", self.ztunit_type.print_config_floats(), self.monthly_cost)
    }

    fn print_config_strings(&self) -> String {
        self.ztunit_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztunit_type.print_config_details()
    }
}

impl Deref for ZTStaffType {
    type Target = ZTUnitType;

    fn deref(&self) -> &Self::Target {
        &self.ztunit_type
    }
}

// ------------ ZTMaintType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTMaintType {
    #[deref_field]
    pub ztstaff_type: ZTStaffType, // bytes: 0x1F0 - 0x1B4 = 0x3C = 60 bytes
    pad01: [u8; 0x1F4 - 0x1F0],           // ----------------------- padding: 4 bytes
    pub clean_trash_radius: i32,          // 0x1F4
    pub fix_fence_modifier: i32,          // 0x1F8
    pub clear_invalid_list_interval: i32, // 0x1FC
}

impl EntityType for ZTMaintType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncCleanTrashRadius: {}\ncFixFenceModifier: {}\ncClearInvalidListInterval: {}\n",
            self.ztstaff_type.print_config_integers(),
            self.clean_trash_radius,
            self.fix_fence_modifier,
            self.clear_invalid_list_interval,
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztstaff_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztstaff_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztstaff_type.print_config_details()
    }
}

impl Deref for ZTMaintType {
    type Target = ZTStaffType;

    fn deref(&self) -> &Self::Target {
        &self.ztstaff_type
    }
}

// ------------ ZTHelicopterType, Implementation, and Related Functions ------------ //

// TODO: DRT staff are not selectable in-game, so this struct needs a bit more testing to ensure it works as expected.
// For now, assumptions are that the offets are correct and the struct is implemented correctly.

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTHelicopterType {
    #[deref_field]
    pub ztstaff_type: ZTStaffType, // bytes: 0x1F0 - 0x1B4 = 0x3C = 60 bytes
    pad01: [u8; 0x1F4 - 0x1F0], // ----------------------- padding: 4 bytes
    // pub loop_sound_name: i32, // 0x1F4 TODO: implement string ptr as function getter
    pad02: [u8; 0x1F8 - 0x1F4], // ----------------------- padding: 4 bytes
    pub loop_sound_atten: i32,  // 0x1F8
}

impl EntityType for ZTHelicopterType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncLoopSoundAtten: {}\n", self.ztstaff_type.print_config_integers(), self.loop_sound_atten,)
    }

    fn print_config_floats(&self) -> String {
        self.ztstaff_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztstaff_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztstaff_type.print_config_details()
    }
}

impl Deref for ZTHelicopterType {
    type Target = ZTStaffType;

    fn deref(&self) -> &Self::Target {
        &self.ztstaff_type
    }
}

// ------------ ZTGuideType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTGuideType {
    #[deref_field]
    pub ztstaff_type: ZTStaffType, // bytes: 0x1F0 - 0x1B4 = 0x3C = 60 bytes
    pad01: [u8; 0x1F4 - 0x1F0], // ----------------------- padding: 4 bytes
    pub inform_guest_time: i32, // 0x1F4
    pub tour_guide_bonus: i32,  // 0x1F8
    pub crowd_check: i32,       // 0x1FC
    pub crowd_radius: i32,      // 0x200
    pub follow_chance: i32,     // 0x204
    pub max_group_size: i32,    // 0x208
}

impl EntityType for ZTGuideType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncInformGuestTime: {}\ncTourGuideBonus: {}\ncCrowdCheck: {}\ncCrowdRadius: {}\ncFollowChance: {}\ncMaxGroupSize: {}\n",
            self.ztstaff_type.print_config_integers(),
            self.inform_guest_time,
            self.tour_guide_bonus,
            self.crowd_check,
            self.crowd_radius,
            self.follow_chance,
            self.max_group_size,
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztstaff_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztstaff_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztstaff_type.print_config_details()
    }
}

impl Deref for ZTGuideType {
    type Target = ZTStaffType;

    fn deref(&self) -> &Self::Target {
        &self.ztstaff_type
    }
}

// ------------ ZTKeeperType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTKeeperType {
    #[deref_field]
    pub ztstaff_type: ZTStaffType, // bytes: 0x1F0 - 0x1B4 = 0x3C = 60 bytes
    pad01: [u8; 0x1F4 - 0x1F0], // ----------------------- padding: 4 bytes
    pub food_units_second: i32, // 0x1F4
    pub clean_time: i32,        // 0x1F8
    pub heal_units_second: i32, // 0x1FC
    pub food_per_tile: i32,     // 0x200
    // pub sickly_animal_pct: i32, // 0x6386F8
    pub clean_tank_pct: i32, // 0x204
    pub clean_tank_threshold: i32, // 0x208
                             // pub dirt: i16, // 0x20C TODO: Appears to pull from a different address, possibly a different struct
}

impl ZTKeeperType {
    // TODO: fix sickly_animal_pct, currently crashes when trying to access it
    pub fn get_sickly_animal_pct(&self) -> i32 {
        unsafe {
            let ptr = get_from_memory::<*mut i32>(0x6386F8);
            if !ptr.is_null() {
                *ptr
            } else {
                0
            }
        }
    }
}

impl EntityType for ZTKeeperType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncFoodUnitsSecond: {}\ncCleanTime: {}\ncHealUnitsSecond: {}\ncFoodPerTile: {}\ncCleanTankPct: {}\ncCleanTankThreshold: {}\n", //cDirt: {}\n", //cSicklyAnimalPct: {}\n",
            self.ztstaff_type.print_config_integers(),
            self.food_units_second,
            self.clean_time,
            self.heal_units_second,
            self.food_per_tile,
            self.clean_tank_pct,
            self.clean_tank_threshold,
            // self.dirt,
            //self.get_sickly_animal_pct(),
        )
    }

    fn print_config_floats(&self) -> String {
        self.ztstaff_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.ztstaff_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.ztstaff_type.print_config_details()
    }
}

// ------------ BFOverlayType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct BFOverlayType {
    #[deref_field]
    pub bfentity_type: BFEntityType, // bytes: 0x100 - 0x0 = 0x100 = 256 bytes
}

impl EntityType for BFOverlayType {
    fn print_config_integers(&self) -> String {
        self.bfentity_type.print_config_integers()
    }

    fn print_config_floats(&self) -> String {
        self.bfentity_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.bfentity_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.bfentity_type.print_config_details()
    }
}

impl Deref for BFOverlayType {
    type Target = BFEntityType;

    fn deref(&self) -> &Self::Target {
        &self.bfentity_type
    }
}

// ------------ ZTAmbientType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTAmbientType {
    #[deref_field]
    pub bfoverlay_type: BFOverlayType, // bytes: 0x100 - 0x0 = 0x100 = 256 bytes
    pub name_id: i32,   // 0x100
    pub help_id: i32,   // 0x104
    pub speed: i32,     // 0x108
    pub frequency: i32, // 0x10C
    pub sound_loop: bool, // 0x110
                        // pub sound_name: i32, // 0x111 TODO: implement string ptr with ZTString
}

impl EntityType for ZTAmbientType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncNameID: {}\ncHelpID: {}\ncSpeed: {}\ncFrequency: {}\ncSoundLoop: {}\n",
            self.bfoverlay_type.print_config_integers(),
            self.name_id,
            self.help_id,
            self.speed,
            self.frequency,
            self.sound_loop as i32,
        )
    }

    fn print_config_floats(&self) -> String {
        self.bfoverlay_type.print_config_floats()
    }

    fn print_config_strings(&self) -> String {
        self.bfoverlay_type.print_config_strings()
    }

    fn print_config_details(&self) -> String {
        self.bfoverlay_type.print_config_details()
    }
}

impl Deref for ZTAmbientType {
    type Target = BFOverlayType;

    fn deref(&self) -> &Self::Target {
        &self.bfoverlay_type
    }
}

// ------------ Custom Command Implementation ------------ //

fn command_sel_type(args: Vec<&str>) -> Result<String, CommandError> {
    let entity_type_address = get_selected_entity_type_address();
    if entity_type_address == 0 {
        return Err(CommandError::new("No entity selected".to_string()));
    }

    let entity_type = map_bfentitytype(entity_type_address)?;

    if args.is_empty() {
        Ok(entity_type.print_config_details())
    } else if args[0] == "-v" {
        // if -v flag is used, print the entity type configuration and other details
        info!("Printing configuration for entity type at address {:#x}", entity_type_address);
        // print the entity type configuration for the selected entity type
        Ok(entity_type.print_config())
    } else if args.len() == 2 {
        // parse the subargs for the entity type
        Ok(entity_type.set_config(args[0], args[1])?)
    } else {
        Ok("Invalid argument".to_string())
    }
}

fn print_info_image_name(entity_type: &BFEntityType, config: &mut String) {
    info!("Checking for cInfoImageName...");
    // TODO: move cInfoImageName to a separate struct (probably ZTSceneryType). crashes when trying to access it from guests
    if entity_type.get_info_image_name() != "" {
        info!("Entity type has cInfoImageName: {}", entity_type.get_info_image_name());
        config.push_str("\n[Characteristics/Strings]\n");
        config.push_str(&entity_type.get_info_image_name());
    }
}

// enum ZTEntityType {
//     Animal(ZTAnimalType),
//     Ambient(ZTAmbientType),
//     Guest(ZTGuestType),
//     Fence(ZTFenceType),
//     TourGuide(ZTGuideType),
//     Building(ZTBuildingType),
//     Scenery(ZTSceneryType),
//     Food(ZTFoodType),
//     TankFilter(ZTTankFilterType),
//     Path(ZTPathType),
//     Rubble(ZTRubbleType),
//     TankWall(ZTTankWallType),
//     Keeper(ZTKeeperType),
//     MaintenanceWorker(ZTMaintType),
//     Drt(ZTHelicopterType),
//     BFOverlay(BFOverlayType),
//     BFUnit(BFUnitType),
//     ZTUnit(ZTUnitType),
//     Staff(ZTStaffType),
//     BFEntity(BFEntityType),
//     Unknown,
// }

// pub fn get_as_entity_type(ptr: u32) -> ZTEntityType {
//     let entity_type_vtable: u32 = get_from_memory(address);
//     ZTEntityType = match ZTEntityTypeClass::from(entity_type_vtable) {
//         ZTEntityTypeClass::Animal => ZTEntityType::Animal(get_from_memory::<ZTAnimalType>(address)),
//         ZTEntityTypeClass::Ambient => ZTEntityType::Ambient(get_from_memory::<ZTAmbientType>(address)),
//         ZTEntityTypeClass::Guest => ZTEntityType::Guest(get_from_memory::<ZTGuestType>(address)),
//         ZTEntityTypeClass::Fence => ZTEntityType::Fence(get_from_memory::<ZTFenceType>(address)),
//         ZTEntityTypeClass::TourGuide => ZTEntityType::TourGuide(get_from_memory::<ZTGuideType>(address)),
//         ZTEntityTypeClass::Building => ZTEntityType::Building(get_from_memory::<ZTBuildingType>(address)),
//         ZTEntityTypeClass::Scenery => ZTEntityType::Scenery(get_from_memory::<ZTSceneryType>(address)),
//         ZTEntityTypeClass::Food => ZTEntityType::Food(get_from_memory::<ZTFoodType>(address)),
//         ZTEntityTypeClass::TankFilter => ZTEntityType::TankFilter(get_from_memory::<ZTTankFilterType>(address)),
//         ZTEntityTypeClass::Path => ZTEntityType::Path(get_from_memory::<ZTPathType>(address)),
//         ZTEntityTypeClass::Rubble => ZTEntityType::Rubble(get_from_memory::<ZTRubbleType>(address)),
//         ZTEntityTypeClass::TankWall => ZTEntityType::TankWall(get_from_memory::<ZTTankWallType>(address)),
//         ZTEntityTypeClass::Keeper => ZTEntityType::Keeper(get_from_memory::<ZTKeeperType>(address)),
//         ZTEntityTypeClass::MaintenanceWorker => ZTEntityType::MaintenanceWorker(get_from_memory::<ZTMaintType>(address)),
//         ZTEntityTypeClass::Drt => ZTEntityType::Drt(get_from_memory::<ZTHelicopterType>(address)),
//         ZTEntityTypeClass::BFOverlay => ZTEntityType::BFOverlay(get_from_memory::<BFOverlayType>(address)),
//         ZTEntityTypeClass::BFUnit => ZTEntityType::BFUnit(get_from_memory::<BFUnitType>(address)),
//         ZTEntityTypeClass::ZTUnit => ZTEntityType::ZTUnit(get_from_memory::<ZTUnitType>(address)),
//         ZTEntityTypeClass::Staff => ZTEntityType::Staff(get_from_memory::<ZTStaffType>(address)),
//         ZTEntityTypeClass::BFEntity => ZTEntityType::BFEntity(get_from_memory::<BFEntityType>(address)),
//         ZTEntityTypeClass::Unknown => ZTEntityType::Unknown,
//     }
// }

// pub fn get_as_specific_entity_type<T: ZTEntityTypeClass>(ptr: u32) -> Result<ZTEntityType, String> {
//     let entity_type_vtable = ZTEntityTypeClass::from(get_from_memory(ptr));
//     if !is(entity_type_vtable, T) {
//         return Err(format!("Incompatible entity type: expected {:?}, found {:?}", T, entity_type_vtable));
//     }

//     let result = match T {
//         ZTEntityTypeClass::Animal => ZTEntityType::Animal(get_from_memory::<ZTAnimalType>(address)),
//         ZTEntityTypeClass::Ambient => ZTEntityType::Ambient(get_from_memory::<ZTAmbientType>(address)),
//         ZTEntityTypeClass::Guest => ZTEntityType::Guest(get_from_memory::<ZTGuestType>(address)),
//         ZTEntityTypeClass::Fence => ZTEntityType::Fence(get_from_memory::<ZTFenceType>(address)),
//         ZTEntityTypeClass::TourGuide => ZTEntityType::TourGuide(get_from_memory::<ZTGuideType>(address)),
//         ZTEntityTypeClass::Building => ZTEntityType::Building(get_from_memory::<ZTBuildingType>(address)),
//         ZTEntityTypeClass::Scenery => ZTEntityType::Scenery(get_from_memory::<ZTSceneryType>(address)),
//         ZTEntityTypeClass::Food => ZTEntityType::Food(get_from_memory::<ZTFoodType>(address)),
//         ZTEntityTypeClass::TankFilter => ZTEntityType::TankFilter(get_from_memory::<ZTTankFilterType>(address)),
//         ZTEntityTypeClass::Path => ZTEntityType::Path(get_from_memory::<ZTPathType>(address)),
//         ZTEntityTypeClass::Rubble => ZTEntityType::Rubble(get_from_memory::<ZTRubbleType>(address)),
//         ZTEntityTypeClass::TankWall => ZTEntityType::TankWall(get_from_memory::<ZTTankWallType>(address)),
//         ZTEntityTypeClass::Keeper => ZTEntityType::Keeper(get_from_memory::<ZTKeeperType>(address)),
//         ZTEntityTypeClass::MaintenanceWorker => ZTEntityType::MaintenanceWorker(get_from_memory::<ZTMaintType>(address)),
//         ZTEntityTypeClass::Drt => ZTEntityType::Drt(get_from_memory::<ZTHelicopterType>(address)),
//         ZTEntityTypeClass::BFOverlay => ZTEntityType::BFOverlay(get_from_memory::<BFOverlayType>(address)),
//         ZTEntityTypeClass::BFUnit => ZTEntityType::BFUnit(get_from_memory::<BFUnitType>(address)),
//         ZTEntityTypeClass::ZTUnit => ZTEntityType::ZTUnit(get_from_memory::<ZTUnitType>(address)),
//         ZTEntityTypeClass::Staff => ZTEntityType::Staff(get_from_memory::<ZTStaffType>(address)),
//         ZTEntityTypeClass::BFEntity => ZTEntityType::BFEntity(get_from_memory::<BFEntityType>(address)),
//         ZTEntityTypeClass::Unknown => {
//             return Err("Unknown entity type".to_string());
//         }
//     };
//     Ok(result)

// }

// This returns a dynamic trait object, which lets us call the methods of the entity type without knowing the exact type
fn get_bfentitytype(address: u32) -> Result<Box<dyn EntityType>, String> {
    // create a copied instance of the entity type
    let entity_type_vtable: u32 = get_from_memory(address);
    let entity: Box<dyn EntityType> = match ZTEntityTypeClass::from(entity_type_vtable) {
        ZTEntityTypeClass::Animal => Box::new(get_from_memory::<ZTAnimalType>(address)),
        ZTEntityTypeClass::Ambient => Box::new(get_from_memory::<ZTUnitType>(address)),
        ZTEntityTypeClass::Guest => Box::new(get_from_memory::<ZTGuestType>(address)),
        ZTEntityTypeClass::Fence => Box::new(get_from_memory::<ZTFenceType>(address)),
        ZTEntityTypeClass::TourGuide => Box::new(get_from_memory::<ZTGuideType>(address)),
        ZTEntityTypeClass::Building => Box::new(get_from_memory::<ZTBuildingType>(address)),
        ZTEntityTypeClass::Scenery => Box::new(get_from_memory::<ZTSceneryType>(address)),
        ZTEntityTypeClass::Food => Box::new(get_from_memory::<ZTFoodType>(address)),
        ZTEntityTypeClass::TankFilter => Box::new(get_from_memory::<ZTTankFilterType>(address)),
        ZTEntityTypeClass::Path => Box::new(get_from_memory::<ZTPathType>(address)),
        ZTEntityTypeClass::Rubble => Box::new(get_from_memory::<ZTRubbleType>(address)),
        ZTEntityTypeClass::TankWall => Box::new(get_from_memory::<ZTTankWallType>(address)),
        ZTEntityTypeClass::Keeper => Box::new(get_from_memory::<ZTKeeperType>(address)),
        ZTEntityTypeClass::MaintenanceWorker => Box::new(get_from_memory::<ZTMaintType>(address)),
        ZTEntityTypeClass::Drt => Box::new(get_from_memory::<ZTHelicopterType>(address)),
        ZTEntityTypeClass::BFOverlay => Box::new(get_from_memory::<BFOverlayType>(address)),
        ZTEntityTypeClass::BFUnit => Box::new(get_from_memory::<BFUnitType>(address)),
        ZTEntityTypeClass::ZTUnit => Box::new(get_from_memory::<ZTUnitType>(address)),
        ZTEntityTypeClass::Staff => Box::new(get_from_memory::<ZTStaffType>(address)),
        ZTEntityTypeClass::BFEntity => Box::new(get_from_memory::<BFEntityType>(address)),
        ZTEntityTypeClass::Unknown => return Err("Unknown entity type".to_string()),
    };
    Ok(entity)
}

fn map_bfentitytype(address: u32) -> Result<&'static mut dyn EntityType, String> {
    // create a copied instance of the entity type
    info!("Mapping entity type at address {:#x}", address);
    let entity_type_vtable: u32 = get_from_memory(address);
    info!("Entity type vtable: {:#x}", entity_type_vtable);
    let entity: &mut dyn EntityType = match ZTEntityTypeClass::from(entity_type_vtable) {
        ZTEntityTypeClass::Animal => map_from_memory::<ZTAnimalType>(address),
        ZTEntityTypeClass::Ambient => map_from_memory::<ZTAmbientType>(address),
        ZTEntityTypeClass::Guest => map_from_memory::<ZTGuestType>(address),
        ZTEntityTypeClass::Fence => map_from_memory::<ZTFenceType>(address),
        ZTEntityTypeClass::TourGuide => map_from_memory::<ZTGuideType>(address),
        ZTEntityTypeClass::Building => map_from_memory::<ZTBuildingType>(address),
        ZTEntityTypeClass::Scenery => map_from_memory::<ZTSceneryType>(address),
        ZTEntityTypeClass::Food => map_from_memory::<ZTFoodType>(address),
        ZTEntityTypeClass::TankFilter => map_from_memory::<ZTTankFilterType>(address),
        ZTEntityTypeClass::Path => map_from_memory::<ZTPathType>(address),
        ZTEntityTypeClass::Rubble => map_from_memory::<ZTRubbleType>(address),
        ZTEntityTypeClass::TankWall => map_from_memory::<ZTTankWallType>(address),
        ZTEntityTypeClass::Keeper => map_from_memory::<ZTKeeperType>(address),
        ZTEntityTypeClass::MaintenanceWorker => map_from_memory::<ZTMaintType>(address),
        ZTEntityTypeClass::Drt => map_from_memory::<ZTHelicopterType>(address),
        ZTEntityTypeClass::BFOverlay => map_from_memory::<BFOverlayType>(address),
        ZTEntityTypeClass::BFUnit => map_from_memory::<BFUnitType>(address),
        ZTEntityTypeClass::ZTUnit => map_from_memory::<ZTUnitType>(address),
        ZTEntityTypeClass::Staff => map_from_memory::<ZTStaffType>(address),
        ZTEntityTypeClass::BFEntity => map_from_memory::<BFEntityType>(address),
        ZTEntityTypeClass::Unknown => return Err("Unknown entity type".to_string()),
    };
    Ok(entity)
}

// initializes the custom command
pub fn init() {
    // sel_type([key], [value]) - optional arguments
    lua_fn!(
        "sel_type",
        "Gets selected entity type config, with optional key/value to set",
        "sel_type([key], [value]) or sel_type(\"-v\")",
        |args: mlua::Variadic<String>| {
            let args_vec: Vec<&str> = args.iter().map(|s| s.as_str()).collect();
            match command_sel_type(args_vec) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // make_sel(id) - single u32 arg
    lua_fn!("make_sel", "Makes entity type selectable", "make_sel(id)", |id: u32| {
        let id_str = id.to_string();
        match command_make_sel(vec![&id_str]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });
}

#[derive(Debug, PartialEq, Eq, FromPrimitive, Clone)]
#[repr(u32)]
pub enum ZTEntityTypeClass {
    Animal = 0x630268,
    Ambient = 0x62e1e8,
    Guest = 0x62e330,
    Fence = 0x63034c,
    TourGuide = 0x62e8ac,
    Building = 0x6307e4,
    Scenery = 0x6303f4,
    Food = 0x630544,
    TankFilter = 0x630694,
    Path = 0x63049c,
    Rubble = 0x63073c,
    TankWall = 0x6305ec,
    Keeper = 0x62e7d8,
    MaintenanceWorker = 0x62e704,
    Drt = 0x62e980,
    BFOverlay = 0x62e58c,
    BFUnit = 0x62e4d8,
    ZTUnit = 0x62e404,
    Staff = 0x62e630,
    BFEntity = 0x62e28c,
    #[num_enum(default)]
    Unknown = 0x0,
}

pub fn zt_entity_type_class_is(original_class: &ZTEntityTypeClass, class: &ZTEntityTypeClass) -> bool {
    match class {
        ZTEntityTypeClass::Animal => *original_class == ZTEntityTypeClass::Animal,
        ZTEntityTypeClass::Ambient => *original_class == ZTEntityTypeClass::Ambient,
        ZTEntityTypeClass::Guest => *original_class == ZTEntityTypeClass::Guest,
        ZTEntityTypeClass::Fence => matches!(*original_class, ZTEntityTypeClass::Fence | ZTEntityTypeClass::TankWall),
        ZTEntityTypeClass::TourGuide => *original_class == ZTEntityTypeClass::TourGuide,
        ZTEntityTypeClass::Building => *original_class == ZTEntityTypeClass::Building,
        ZTEntityTypeClass::Scenery => matches!(
            *original_class,
            ZTEntityTypeClass::Scenery
                | ZTEntityTypeClass::Food
                | ZTEntityTypeClass::Path
                | ZTEntityTypeClass::Rubble
                | ZTEntityTypeClass::TankFilter
                | ZTEntityTypeClass::TankWall
                | ZTEntityTypeClass::Building
                | ZTEntityTypeClass::Fence
        ),
        ZTEntityTypeClass::Food => *original_class == ZTEntityTypeClass::Food,
        ZTEntityTypeClass::TankFilter => *original_class == ZTEntityTypeClass::TankFilter,
        ZTEntityTypeClass::Path => *original_class == ZTEntityTypeClass::Path,
        ZTEntityTypeClass::Rubble => *original_class == ZTEntityTypeClass::Rubble,
        ZTEntityTypeClass::TankWall => *original_class == ZTEntityTypeClass::TankWall,
        ZTEntityTypeClass::Keeper => *original_class == ZTEntityTypeClass::Keeper,
        ZTEntityTypeClass::MaintenanceWorker => *original_class == ZTEntityTypeClass::MaintenanceWorker,
        ZTEntityTypeClass::Drt => *original_class == ZTEntityTypeClass::Drt,
        ZTEntityTypeClass::Unknown => false,
        ZTEntityTypeClass::BFOverlay => matches!(*original_class, ZTEntityTypeClass::BFOverlay | ZTEntityTypeClass::Ambient),
        ZTEntityTypeClass::BFUnit => matches!(
            *original_class,
            ZTEntityTypeClass::BFUnit
                | ZTEntityTypeClass::ZTUnit
                | ZTEntityTypeClass::Staff
                | ZTEntityTypeClass::Keeper
                | ZTEntityTypeClass::MaintenanceWorker
                | ZTEntityTypeClass::Drt
                | ZTEntityTypeClass::TourGuide
                | ZTEntityTypeClass::Guest
                | ZTEntityTypeClass::Animal
        ),
        ZTEntityTypeClass::ZTUnit => matches!(
            *original_class,
            ZTEntityTypeClass::ZTUnit
                | ZTEntityTypeClass::Staff
                | ZTEntityTypeClass::Keeper
                | ZTEntityTypeClass::MaintenanceWorker
                | ZTEntityTypeClass::Drt
                | ZTEntityTypeClass::TourGuide
                | ZTEntityTypeClass::Guest
                | ZTEntityTypeClass::Animal
        ),
        ZTEntityTypeClass::Staff => matches!(
            *original_class,
            ZTEntityTypeClass::Staff | ZTEntityTypeClass::Keeper | ZTEntityTypeClass::MaintenanceWorker | ZTEntityTypeClass::Drt | ZTEntityTypeClass::TourGuide
        ),
        ZTEntityTypeClass::BFEntity => matches!(
            *original_class,
            ZTEntityTypeClass::BFEntity
                | ZTEntityTypeClass::BFUnit
                | ZTEntityTypeClass::ZTUnit
                | ZTEntityTypeClass::Staff
                | ZTEntityTypeClass::Keeper
                | ZTEntityTypeClass::MaintenanceWorker
                | ZTEntityTypeClass::Drt
                | ZTEntityTypeClass::TourGuide
                | ZTEntityTypeClass::Guest
                | ZTEntityTypeClass::Animal
                | ZTEntityTypeClass::Scenery
                | ZTEntityTypeClass::Food
                | ZTEntityTypeClass::Path
                | ZTEntityTypeClass::Rubble
                | ZTEntityTypeClass::TankFilter
                | ZTEntityTypeClass::TankWall
                | ZTEntityTypeClass::Building
                | ZTEntityTypeClass::Fence
                | ZTEntityTypeClass::BFOverlay
                | ZTEntityTypeClass::Ambient
        ),
    }
}

// TODO: Convert into struct with offsets that can be read straight from memory, should also be called BFEntity
#[derive(Debug, Getters)]
#[get = "pub"]
pub struct ZTEntityType {
    pub vtable: u32,
    pub class_string: u32,
    pub class: ZTEntityTypeClass,
    pub zt_type: String,
    pub zt_sub_type: String,
    pub bf_config_file_ptr: u32,
}

impl ZTEntityType {
    pub fn is_member(&self, member: String) -> bool {
        match self.class {
            ZTEntityTypeClass::Animal
            | ZTEntityTypeClass::Guest
            | ZTEntityTypeClass::Fence
            | ZTEntityTypeClass::TourGuide
            | ZTEntityTypeClass::TankFilter
            | ZTEntityTypeClass::TankWall
            | ZTEntityTypeClass::Keeper
            | ZTEntityTypeClass::MaintenanceWorker
            | ZTEntityTypeClass::Drt => is_member(&self.zt_type, &member),
            ZTEntityTypeClass::Building
            | ZTEntityTypeClass::Scenery
            | ZTEntityTypeClass::Food
            | ZTEntityTypeClass::Path
            | ZTEntityTypeClass::Rubble
            | ZTEntityTypeClass::Ambient => is_member(&self.zt_sub_type, &member),

            ZTEntityTypeClass::Unknown
            | ZTEntityTypeClass::BFOverlay
            | ZTEntityTypeClass::BFUnit
            | ZTEntityTypeClass::ZTUnit
            | ZTEntityTypeClass::Staff
            | ZTEntityTypeClass::BFEntity => false,
        }
    }
}

pub fn read_zt_entity_type_from_memory(zt_entity_type_ptr: u32) -> ZTEntityType {
    let class_string = get_from_memory::<u32>(zt_entity_type_ptr);
    let class = ZTEntityTypeClass::from(class_string);

    ZTEntityType {
        vtable: class_string,
        class_string,
        class,
        zt_type: get_string_from_memory(get_from_memory::<u32>(zt_entity_type_ptr + 0x98)),
        zt_sub_type: get_string_from_memory(get_from_memory::<u32>(zt_entity_type_ptr + 0xa4)),
        bf_config_file_ptr: get_from_memory::<u32>(zt_entity_type_ptr + 0x80),
    }
}

impl fmt::Display for ZTEntityType {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Class String: {:#x}, Class: {:?}, ZT Type: {}, ZT Sub Type: {}, ptr {:#x}, config_file_ptr {:#x}",
            self.class_string, self.class, self.zt_type, self.zt_sub_type, self.vtable, self.bf_config_file_ptr
        )
    }
}

pub fn command_make_sel(args: Vec<&str>) -> Result<String, CommandError> {
    if args.is_empty() {
        Err(Into::into("Usage: make_sel <id>"))
    } else {
        let id = args[0].parse::<u32>()?;
        let entity_type_ptr = ztworldmgr::get_entity_type_by_id(id);
        if entity_type_ptr == 0 {
            return Err(Into::into("Entity type not found"));
        }
        let entity_type = map_from_memory::<ZTSceneryType>(entity_type_ptr);
        if entity_type.selectable {
            return Ok(format!("Entity type {} is already selectable", entity_type.bfentitytype.get_type_name()));
        }
        entity_type.selectable = true;
        Ok(format!("Entity type {} is now selectable", entity_type.bfentitytype.get_type_name()))
    }
}
