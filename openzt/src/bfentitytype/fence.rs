use std::ops::Deref;

use field_accessor_as_string::FieldAccessorAsString;
use getset::{Getters, Setters};

use super::base::EntityType;
use super::scenery::ZTSceneryType;
use crate::util::{get_from_memory, get_string_from_memory};

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
    pub fn get_break_sound(&self) -> String {
        let obj_ptr = self as *const ZTFenceType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x184))
    }

    pub fn get_open_sound(&self) -> String {
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
    pub fn get_portal_open_sound(&self) -> String {
        let obj_ptr = self as *const ZTTankWallType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x1A4))
    }

    pub fn get_portal_close_sound(&self) -> String {
        let obj_ptr = self as *const ZTTankWallType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x1B0))
    }

    pub fn print_portal_sounds(&self) -> String {
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

// ------------ ZTTankFilterType, Implementation, and Related Functions ------------ //

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
    pub fn get_healthy_sound(&self) -> String {
        let obj_ptr = self as *const ZTTankFilterType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x184))
    }

    pub fn get_decayed_sound(&self) -> String {
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
    pub ztscenerytype: ZTSceneryType,
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
