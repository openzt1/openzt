use std::ops::Deref;

use field_accessor_as_string::FieldAccessorAsString;
use getset::{Getters, Setters};

use super::base::{BFEntityType, EntityType};
use super::scenery::ZTSceneryType;

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
