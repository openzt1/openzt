use std::ops::Deref;

use field_accessor_as_string::FieldAccessorAsString;
use getset::{Getters, Setters};

use super::base::EntityType;
use super::items::BFOverlayType;

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
