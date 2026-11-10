use std::ops::Deref;

use field_accessor_as_string::FieldAccessorAsString;
use getset::{Getters, Setters};

use super::base::EntityType;
use super::units::ZTUnitType;
use crate::util::get_from_memory;

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

impl Deref for ZTKeeperType {
    type Target = ZTStaffType;

    fn deref(&self) -> &Self::Target {
        &self.ztstaff_type
    }
}
