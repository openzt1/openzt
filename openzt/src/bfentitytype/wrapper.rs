use std::fmt;

use getset::Getters;
use num_enum::FromPrimitive;
use tracing::info;

use super::ambient::ZTAmbientType;
use super::base::{BFEntityType, EntityType};
use super::fence::{ZTFenceType, ZTPathType, ZTTankFilterType, ZTTankWallType};
use super::items::{BFOverlayType, ZTFoodType};
use super::scenery::{ZTBuildingType, ZTRubbleType, ZTSceneryType};
use super::staff::{ZTHelicopterType, ZTGuideType, ZTKeeperType, ZTMaintType, ZTStaffType};
use super::units::{BFUnitType, ZTAnimalType, ZTGuestType, ZTUnitType};
use crate::expansions::is_member;
use crate::util::{get_from_memory, get_string_from_memory, map_from_memory};

// This returns a dynamic trait object, which lets us call the methods of the entity type without knowing the exact type
pub(crate) fn get_bfentitytype(address: u32) -> Result<Box<dyn EntityType>, String> {
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

pub(crate) fn map_bfentitytype(address: u32) -> Result<&'static mut dyn EntityType, String> {
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

impl std::str::FromStr for ZTEntityTypeClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "animal" => Ok(ZTEntityTypeClass::Animal),
            "ambient" => Ok(ZTEntityTypeClass::Ambient),
            "guest" => Ok(ZTEntityTypeClass::Guest),
            "fence" => Ok(ZTEntityTypeClass::Fence),
            "tourguide" => Ok(ZTEntityTypeClass::TourGuide),
            "building" => Ok(ZTEntityTypeClass::Building),
            "scenery" => Ok(ZTEntityTypeClass::Scenery),
            "food" => Ok(ZTEntityTypeClass::Food),
            "tankfilter" => Ok(ZTEntityTypeClass::TankFilter),
            "path" => Ok(ZTEntityTypeClass::Path),
            "rubble" => Ok(ZTEntityTypeClass::Rubble),
            "tankwall" => Ok(ZTEntityTypeClass::TankWall),
            "keeper" => Ok(ZTEntityTypeClass::Keeper),
            "maintenanceworker" => Ok(ZTEntityTypeClass::MaintenanceWorker),
            "drt" => Ok(ZTEntityTypeClass::Drt),
            "bfoverlay" => Ok(ZTEntityTypeClass::BFOverlay),
            "bfunit" => Ok(ZTEntityTypeClass::BFUnit),
            "ztunit" => Ok(ZTEntityTypeClass::ZTUnit),
            "staff" => Ok(ZTEntityTypeClass::Staff),
            "bfentity" => Ok(ZTEntityTypeClass::BFEntity),
            _ => Err(format!("Unknown entity type class: {}", s)),
        }
    }
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
