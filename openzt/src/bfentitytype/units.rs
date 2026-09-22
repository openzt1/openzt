use std::ops::Deref;

use field_accessor_as_string::FieldAccessorAsString;
use getset::{Getters, Setters};

use super::base::{BFEntityType, EntityType};
use crate::geom::IVec3;
use crate::util::{get_from_memory, get_string_from_memory};

// ------------ BFUnitType, Implementation, and Related Functions ------------ //

#[derive(Debug, Getters, Setters, FieldAccessorAsString)]
#[repr(C)]
pub struct BFUnitType {
    #[deref_field]
    pub bfentitytype: BFEntityType, // bytes: 0x100 - 0x000 = 0x100 = 256 bytes
    pub slow_rate: u32,         // 0x100
    pub medium_rate: u32,       // 0x104
    pub fast_rate: u32,         // 0x108
    pub slow_anim_speed: u16,   // 0x10C
    pub medium_anim_speed: u16, // 0x10E
    pub fast_anim_speed: u16,   // 0x110
    pad0: [u8; 0x114 - 0x112],  // ----------------------- padding: 2 bytes
    pub min_height: u32,        // 0x114 <--- unsure if accurate
    pub max_height: u32,        // 0x118 <--- unsure if accurate
    pad1: [u8; 0x12C - 0x11C],        // ----------------------- padding: 16 bytes
    pub purchase_cost: f32,           // 0x12C
    pub name_id: i32,                 // 0x130
    pub help_id: i32,                 // 0x134
    pad2: [u8; 0x144 - 0x138],        // ----------------------- padding: 16 bytes
}

const _: () = assert!(std::mem::size_of::<BFUnitType>() == 0x144);

impl EntityType for BFUnitType {
    fn print_config_integers(&self) -> String {
        format!(
            "{}\ncSlowRate: {}\ncMediumRate: {}\ncFastRate: {}\ncSlowAnimSpeed: {}\ncMediumAnimSpeed: {}\ncFastAnimSpeed: {}\ncMinHeight: {}\ncMaxHeight: {}\ncPurchaseCost: {}\ncNameID: {}\ncHelpID: {}\n",
            self.bfentitytype.print_config_integers(),
            self.slow_rate,
            self.medium_rate,
            self.fast_rate,
            self.slow_anim_speed,
            self.medium_anim_speed,
            self.fast_anim_speed,
            self.min_height,
            self.max_height,
            self.purchase_cost,
            self.name_id,
            self.help_id,
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
    pub bfunit_type: BFUnitType, // bytes: 0x144 - 0x100 = 0x44 = 68 bytes
    pad1: [u8; 0x150 - 0x144],        // ----------------------- padding: 24 bytes
    pub map_footprint: i32,           // 0x150
    pub slow_anim_speed_water: u16,   // 0x154
    pub medium_anim_speed_water: u16, // 0x156
    pub fast_anim_speed_water: u16,   // 0x158
    pad2: [u8; 0x17C - 0x15A],        // ----------------------- padding: 32 bytes
    // pub list_image_name: String,    // 0x168 TODO: fix offset for string getters in unittype
    pub swims: bool,               // 0x17C
    pub surface: bool,             // 0x17D
    pub underwater: bool,          // 0x17E
    pub only_underwater: bool,     // 0x17F
    pub skip_trick_happiness: u32, // 0x180 TODO: potentially not accurate
    pub skip_trick_chance: i32,    // 0x184
}

const _: () = assert!(std::mem::size_of::<ZTUnitType>() == 0x188);

impl ZTUnitType {
    /// Confirmed real vtable VA for `ZTUnitType` in zoo.exe (see `private/docs/vtables/ZTUnitType.md`).
    /// `ZTUnitType` overrides `BFEntityType`'s `+0x1c` slot with a different function (confirmed by
    /// the vtable diff docs), so fixtures must use this class's own vtable, not `BFEntityType`'s
    /// - see `ztunit-ztanimal-footprint-crash-investigation.md`.
    const VTABLE_PTR: u32 = 0x0062e404;

    /// Test-only fixture: a zero-filled `ZTUnitType` with a real vtable pointer, needed because
    /// `ZTUnit::getFootprint` virtually dispatches through `entity_type`'s vtable (not its own) in
    /// its `use_map_footprint=true` branch. `mem::zeroed()` is otherwise safe: every other field is
    /// an integer/bool/byte-array.
    pub fn new_for_test(footprint: IVec3, map_footprint: i32) -> Self {
        let mut entity_type: ZTUnitType = unsafe { std::mem::zeroed() };
        entity_type.bfunit_type.bfentitytype.vtable = Self::VTABLE_PTR;
        entity_type.bfunit_type.bfentitytype.footprintx = footprint.x;
        entity_type.bfunit_type.bfentitytype.footprinty = footprint.y;
        entity_type.bfunit_type.bfentitytype.footprintz = footprint.z;
        entity_type.map_footprint = map_footprint;
        entity_type
    }

    pub fn get_list_name(&self) -> String {
        let obj_ptr = self as *const ZTUnitType as u32;
        get_string_from_memory(get_from_memory::<u32>(obj_ptr + 0x168))
    }
}

impl EntityType for ZTUnitType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncMapFootprint: {}\ncSlowAnimSpeedWater: {}\ncMediumAnimSpeedWater: {}\ncFastAnimSpeedWater: {}\ncSwims: {}\ncSurface: {}\ncUnderwater: {}\ncOnlyUnderwater: {}\ncSkipTrickHappiness: {}\ncSkipTrickChance: {}\n",
                self.bfunit_type.print_config_integers(),
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

impl Deref for ZTUnitType {
    type Target = BFUnitType;
    fn deref(&self) -> &Self::Target {
        &self.bfunit_type
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
    pub box_footprint: IVec3,           // 0x1D8
    pub family: i32,                    // 0x1E4
    pub genus: i32,                     // 0x1E8
    pub species: i32,                   // 0x1EC <--- unconfirmed: completes family/genus/species triplet, matches species-rating-cache key read at entity_type+0x1ec (see species-rating-cache-identification-handover.md)
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
    pad19: [u8; 0x410 - 0x3D8],   // ----------------------- padding: 8 bytes
    pub egg_footprint: IVec3,         // 0x410
}

const _: () = assert!(std::mem::size_of::<ZTAnimalType>() == 0x41c);

impl EntityType for ZTAnimalType {
    fn print_config_integers(&self) -> String {
        format!("{}\ncBoxFootprintX: {}\ncBoxFootprintY: {}\ncBoxFootprintZ: {}\ncFamily: {}\ncGenus: {}\ncHabitat: {}\ncLocation: {}\ncEra: {}\ncBreathThreshold: {}\ncBreathIncrement: {}\ncHungerThreshold: {}\ncHungryHealthChange: {}\ncHungerIncrement: {}\ncFoodUnitValue: {}\ncKeeperFoodUnitsEaten: {}\ncNeededFood: {}\ncNoFoodChange: {}\ncInitialHappiness: {}\ncMaxHits: {}\ncPctHits: {}\ncMaxEnergy: {}\ncMaxDirty: {}\ncMinDirty: {}\ncSickChange: {}\ncOtherAnimalSickChange: {}\ncSickChance: {}\ncSickRandomChance: {}\ncCrowd: {}\ncCrowdHappinessChange: {}\ncZapHappinessChange: {}\ncCaptivity: {}\ncReproductionChance: {}\ncReproductionInterval: {}\ncMatingType: {}\ncOffspring: {}\ncKeeperFrequency: {}\ncNotEnoughKeepersChange: {}\ncSocial: {}\ncHabitatSize: {}\ncNumberAnimalsMin: {}\ncNumberAnimalsMax: {}\ncNumberMinChange: {}\ncNumberMaxChange: {}\ncHabitatPreference: {}\ncBabyBornChange: {}\ncEnergyIncrement: {}\ncEnergyThreshold: {}\ncDirtyIncrement: {}\ncDirtyThreshold: {}\ncSickTime: {}\ncBabyToAdult: {}\ncOtherFood: {}\ncTreePref: {}\ncRockPref: {}\ncSpacePref: {}\ncElevationPref: {}\ncDepthMin: {}\ncDepthMax: {}\ncDepthChange: {}\ncSalinityChange: {}\ncSalinityHealthChange: {}\ncHappyReproduceThreshold: {}\ncBuildingUseChance: {}\ncNoMateChange: {}\ncTimeDeath: {}\ncDeathChance: {}\ncDirtChance: {}\ncWaterNeeded: {}\ncUnderwaterNeeded: {}\ncLandNeeded: {}\ncEnterWaterChance: {}\ncEnterTankChance: {}\ncEnterLandChance: {}\ncDrinkWaterChance: {}\ncChaseAnimalChance: {}\ncClimbsCliffs: {}\ncBashStrength: {}\ncAttractiveness: {}\ncKeeperFoodType: {}\ncIsClimber: {}\ncIsJumper: {}\ncSmallZoodoo: {}\ncDinoZoodoo: {}\ncGiantZoodoo: {}\ncIsSpecialAnimal: {}\ncNeedShelter: {}\ncNeedToys: {}\ncBabiesAttack: {}\n EggFootprintX: {}\ncEggFootprintY: {}\ncEggFootprintZ: {}\n",
        self.ztunit_type.print_config_integers(),
        self.box_footprint.x,
        self.box_footprint.y,
        self.box_footprint.z,
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
        self.egg_footprint.x,
        self.egg_footprint.y,
        self.egg_footprint.z,
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

impl ZTAnimalType {
    /// Confirmed real vtable VA for `ZTAnimalType` in zoo.exe (see `private/docs/vtables/ZTAnimalType.md`).
    /// See `ZTUnitType::VTABLE_PTR` for why the type's own vtable is required, not `BFEntityType`'s.
    const VTABLE_PTR: u32 = 0x00630268;

    /// Test-only fixture, see `ZTUnitType::new_for_test`.
    pub fn new_for_test(footprint: IVec3, map_footprint: i32, box_footprint: IVec3, egg_footprint: IVec3) -> Self {
        let mut entity_type: ZTAnimalType = unsafe { std::mem::zeroed() };
        entity_type.ztunit_type.bfunit_type.bfentitytype.vtable = Self::VTABLE_PTR;
        entity_type.ztunit_type.bfunit_type.bfentitytype.footprintx = footprint.x;
        entity_type.ztunit_type.bfunit_type.bfentitytype.footprinty = footprint.y;
        entity_type.ztunit_type.bfunit_type.bfentitytype.footprintz = footprint.z;
        entity_type.ztunit_type.map_footprint = map_footprint;
        entity_type.box_footprint = box_footprint;
        entity_type.egg_footprint = egg_footprint;
        entity_type
    }
}

impl Deref for ZTAnimalType {
    type Target = ZTUnitType;

    fn deref(&self) -> &Self::Target {
        &self.ztunit_type
    }
}
