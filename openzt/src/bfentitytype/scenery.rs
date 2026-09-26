use std::ops::Deref;

use field_accessor_as_string::FieldAccessorAsString;
use getset::{Getters, Setters};

use super::base::{BFEntityType, EntityType};
use super::wrapper::{zt_entity_type_class_is, ZTEntityTypeClass};
use crate::util::{get_from_memory, get_string_from_memory, Checkable};

// ------------ ZTSceneryType, Implementation, and Related Functions ------------ //

// TODO: Need to confirm if any of the below should be in BFEntityType, should look into BFUnitType struct to figure out what is common
// According to Ghidra BFEntityType is 0x144 bytes, with ZTScenery being an extra 0x20 bytes? -> MacOS has BEEntityType at
// Should be able to use the Type init functions to get very accurate sizes and to confirm what strings correspond to what memory locations
// Can also figure out some of the gaps using *Type::loadCharacteristics()
// Does BF/ZTUnitType also load purchase cost, name id etc?
#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
#[get = "pub"]
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
    pad7: [u8; 0x168 - 0x13C],       // ----------------------- padding: 44 bytes
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
pub struct ZTBuildingType {
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

// ------------ ZTRubbleType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct ZTRubbleType {
    #[deref_field]
    pub ztscenerytype: ZTSceneryType,
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
