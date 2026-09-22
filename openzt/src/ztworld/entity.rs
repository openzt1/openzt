use getset::Getters;
use num_enum::FromPrimitive;
use std::cmp::max;
use std::fmt;
use std::mem;
use tracing::error;

use crate::bfentitytype::{read_zt_entity_type_from_memory, BFEntityType, ZTAnimalType, ZTEntityType, ZTEntityTypeClass, ZTUnitType};
use crate::geom::{IVec3, Rectangle};
use crate::globals::globals;
use crate::util::{get_from_memory, get_string_from_memory, ref_from_memory, ZTBufferString};
use crate::ztmapview::BFTile;

#[derive(Debug, PartialEq, Eq, FromPrimitive, Clone)]
#[repr(u32)]
pub enum ZTEntityClass {
    Food = 0x62dd08,
    Path = 0x62da88,
    Fences = 0x62d808,
    Building = 0x62e0b0,
    Animal = 0x62ff54,
    Guest = 0x630f88,
    Scenery = 0x62d950,
    Keeper = 0x62f3e4,
    MaintenanceWorker = 0x62ea54,
    TourGuide = 0x62f714,
    Drt = 0x62f0b4,
    Ambient = 0x62d6ec,
    Rubble = 0x62df78,
    TankWall = 0x62dbc0,
    TankFilter = 0x62de40,
    #[num_enum(default)]
    Unknown = 0x0,
}

impl std::str::FromStr for ZTEntityClass {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s.to_lowercase().as_str() {
            "food" => Ok(ZTEntityClass::Food),
            "path" => Ok(ZTEntityClass::Path),
            "fences" => Ok(ZTEntityClass::Fences),
            "building" => Ok(ZTEntityClass::Building),
            "animal" => Ok(ZTEntityClass::Animal),
            "guest" => Ok(ZTEntityClass::Guest),
            "scenery" => Ok(ZTEntityClass::Scenery),
            "keeper" => Ok(ZTEntityClass::Keeper),
            "maintenanceworker" => Ok(ZTEntityClass::MaintenanceWorker),
            "tourguide" => Ok(ZTEntityClass::TourGuide),
            "drt" => Ok(ZTEntityClass::Drt),
            "ambient" => Ok(ZTEntityClass::Ambient),
            "rubble" => Ok(ZTEntityClass::Rubble),
            "tankwall" => Ok(ZTEntityClass::TankWall),
            "tankfilter" => Ok(ZTEntityClass::TankFilter),
            _ => Err(format!("Unknown entity type: {}", s)),
        }
    }
}

// TODO: Make this look like other structs with proper offsets and padding ->
#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
pub struct ZTEntity {
    vtable: u32,
    // Technically, the first 0x154 bytes are BFEntity, should grab what Eric started doing in his PR and embed BFEntity here
    class: ZTEntityClass,
    type_class: ZTEntityType, // TODO: Change to &ZTEntityType at some point?
    name: String,
    pos1: u32,
    pos2: u32,
}

#[derive(Debug)]
pub(crate) struct ZTEntityWithPtr {
    pub(crate) ptr: u32,
    pub(crate) entity: ZTEntity,
}

#[derive(Debug)]
pub(crate) struct ZTEntityTypeWithPtr {
    pub(crate) ptr: u32,
    pub(crate) entity_type: ZTEntityType,
}

impl ZTEntity {
    pub fn is_member(&self, member: String) -> bool {
        self.type_class.is_member(member)
    }
}

impl fmt::Display for ZTEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "Entity Type: {:?}, Name: {}, EntityType {} ({},{}) ({},{})",
            self.class,
            self.name,
            self.type_class,
            self.pos1,
            self.pos2,
            self.pos1 >> 6,
            self.pos2 >> 6
        )
    }
}

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
pub struct BFEntity { // Full size is 0x154 bytes
    vtable: u32,
    padding: [u8; 0x104],
    name: ZTBufferString,      // 0x108
    pos: IVec3,              // 0x114
    height_above_terrain: u32, // 0x120
    id: u32,                   // 0x124 - this entity's own numeric id, used by ZTThought to store thinker/object references (see ztthoughtmgr.rs)
    inner_class_ptr: u32,      // 0x128
    rotation: i32,             // 0x12c
    padding_2: [u8; 0xc],      // ----- padding: 28 bytes
    unknown_flag1: u8,         // 0x13c // isRemoved
    unknown_flag2: u8,         // 0x13d // isRemovedUndo
    unknown_flag3: u8,         // 0x13e
    visible: u8,               // 0x13f
    snap_to_ground: u8,        // 0x140
    selected: u8,              // 0x141
    unknown_flag4: u8,         // 0x142 // Moving? Programmatically?
    unknown_flag5: u8,         // 0x143 // Picked up?
    draw_dithered: u8,         // 0x144
    unknown_flag6: u8,         // 0x145 // If != 0; Draw selection graphic
    stop_at_end: u8,           // 0x146
    padding_3: [u8; 0x9],      // ----- padding: 10 bytes
    map_footprint: i32,        // 0x150
}

const _: () = assert!(mem::size_of::<BFEntity>() == 0x154);

impl fmt::Display for BFEntity {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "BFEntity {{ name: {}, x_coord: {}, y_coord: {}, z_coord: {}, height_above_terrain: {}, rotation: {}, inner_class_ptr: {:#x}, visible: {}, snap_to_ground: {}, selected: {}, draw_dithered: {} }}",
            self.name, self.pos.x, self.pos.y, self.pos.z, self.height_above_terrain, self.rotation, self.inner_class_ptr, self.visible, self.snap_to_ground, self.selected, self.draw_dithered
        )
    }
}

impl BFEntity {
    /// Builds a zero-filled `BFEntity` for tests, with only the fields relevant to
    /// pure (globals()-free) logic populated. Mirrors `BFTile::new` in `ztmapview.rs`.
    pub fn new_for_test(inner_class_ptr: u32, rotation: i32, map_footprint: i32) -> Self {
        let mut entity: BFEntity = unsafe { mem::zeroed() };
        entity.inner_class_ptr = inner_class_ptr;
        entity.rotation = rotation;
        entity.map_footprint = map_footprint;
        entity
    }

    pub fn entity_type_class(&self) -> ZTEntityTypeClass {
        ZTEntityTypeClass::from(get_from_memory::<u32>(self.inner_class_ptr))
    }

    pub fn entity_type(&self) -> &'static BFEntityType {
        unsafe { ref_from_memory(self.inner_class_ptr) }
    }

    // TODO: Hook this and check that it works
    pub fn is_on_tile(&self, tile: &BFTile) -> bool {
        let Some(entity_tile) = self.get_tile() else {
            error!("BFEntity::is_on_tile: Entity {} has no tile", self.name);
            return false;
        };
        if entity_tile == *tile {
            return true;
        }

        let rect = self.get_blocking_rect();
        let tile_size = IVec3 { x: 0x20, y: 0x20, z: 0 };
        rect.contains_point(&globals().ztworldmgr().tile_to_world(tile.pos, tile_size))
    }

    pub fn get_blocking_rect(&self) -> Rectangle {
        // TODO: We shouldn't need the first check
        // Transient entities don't block anything
        if self.inner_class_ptr == 0 || self.entity_type().is_transient || self.entity_type_class() == ZTEntityTypeClass::Path {
            return Rectangle::default(); // Zero rectangle
        }

        let mut footprint = self.vtable_get_footprint();

        if self.rotation % 2 != 0 {
            let max = max(footprint.x, footprint.y);
            footprint.x = max;
            footprint.y = max;
        }

        // Calculate half-dimensions for easier rectangle construction
        let half_width = (footprint.x * 32) / 2; // Scaling factor preserved from original
        let half_height = (footprint.y * 32) / 2;

        // Construct and return the rectangle
        Rectangle {
            min_x: self.pos.x - half_width,
            min_y: self.pos.y - half_height,
            max_x: self.pos.x + half_width,
            max_y: self.pos.y + half_height,
        }
    }

    fn vtable_get_footprint(&self) -> IVec3 {
        let function_address = get_from_memory::<u32>(self.vtable + 0x94);
        let get_footprint_fn =
            unsafe { std::mem::transmute::<u32, extern "thiscall" fn(this: &BFEntity, param_1: &mut IVec3, param_2: u32) -> u32>(function_address) };
        let mut result_footprint = IVec3::default();
        let footprint_ptr = get_footprint_fn(self, &mut result_footprint, 0);
        get_from_memory::<IVec3>(footprint_ptr)
    }

    pub fn get_footprint(&self, _use_map_footprint: bool) -> IVec3 {
        let entity_type = self.entity_type();
        if self.rotation % 4 == 0 {
            IVec3 {
                x: entity_type.footprintx,
                y: entity_type.footprinty,
                z: entity_type.footprintz,
            }
        } else {
            IVec3 {
                x: entity_type.footprinty,
                y: entity_type.footprintx,
                z: entity_type.footprintz,
            }
        }
    }

    pub fn get_tile(&self) -> Option<BFTile> {
        globals().ztworldmgr().get_tile_from_coords(self.pos.x, self.pos.y)
    }

    pub fn check_avoid_edges(&self, tile: &BFTile) -> bool {
        let radius = self.entity_type().avoid_edges as i32 - 1;

        if radius < 0 {
            return false;
        }

        if radius == 0 {
            return tile.north_fence != 0 || tile.east_fence != 0 || tile.south_fence != 0 || tile.west_fence != 0
        }

        // Check all tiles in a square of `radius` around the placement tile
        let world_mgr = globals().ztworldmgr();

        let x_min = tile.pos.x - radius;
        let x_max = tile.pos.x + radius;
        let y_min = tile.pos.y - radius;
        let y_max = tile.pos.y + radius;

        for check_x in x_min..=x_max {
            for check_y in y_min..=y_max {
                // Bounds check — skip tiles outside the map
                if check_x < 0 || check_y < 0 || check_x >= world_mgr.map_x_size as i32 || check_y >= world_mgr.map_y_size as i32 {
                    continue;
                }

                if let Some(tile) = world_mgr.get_tile_from_coords(check_x, check_y)
                    && (tile.north_fence != 0 || tile.east_fence != 0 || tile.south_fence != 0 || tile.west_fence != 0)
                {
                    return true;
                }
            }
        }
        false
    }
}

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
pub(crate) struct BFUnit {
    base: BFEntity, // bytes: 0x154 = 340 bytes
    // TODO
    padding: [u8; 0x214-0x154], // ----- padding: 192 bytes
}

const _: () = assert!(mem::size_of::<BFUnit>() == 0x214);

impl BFUnit {
    /// Test-only fixture, see `BFEntity::new_for_test`.
    pub fn new_for_test(inner_class_ptr: u32, rotation: i32, map_footprint: i32) -> Self {
        BFUnit {
            base: BFEntity::new_for_test(inner_class_ptr, rotation, map_footprint),
            padding: [0; 0x214 - 0x154],
        }
    }
}

impl std::ops::Deref for BFUnit {
    type Target = BFEntity;
    fn deref(&self) -> &BFEntity {
        &self.base
    }
}

impl std::ops::DerefMut for BFUnit {
    fn deref_mut(&mut self) -> &mut BFEntity {
        &mut self.base
    }
}

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
pub(crate) struct ZTUnit {
    base: BFUnit, // bytes: 0x214 = 532 bytes
    padding: [u8; 0x260-0x214], // ----- padding: 76 bytes
}

const _: () = assert!(mem::size_of::<ZTUnit>() == 0x260);

impl ZTUnit {
    /// Test-only fixture, see `BFEntity::new_for_test`.
    pub fn new_for_test(inner_class_ptr: u32, rotation: i32, map_footprint: i32) -> Self {
        ZTUnit {
            base: BFUnit::new_for_test(inner_class_ptr, rotation, map_footprint),
            padding: [0; 0x260 - 0x214],
        }
    }

    pub fn entity_type(&self) -> &'static ZTUnitType {
        unsafe { ref_from_memory(self.inner_class_ptr) }
    }

    pub fn get_footprint(&self, use_map_footprint: bool) -> IVec3 {
        if !use_map_footprint {
            self.base.get_footprint(use_map_footprint)
        } else {
            let map_footprint = self.entity_type().map_footprint;
            IVec3 {
                x: map_footprint,
                y: map_footprint,
                z: 0,
            }
        }
    }
}

impl std::ops::Deref for ZTUnit {
    type Target = BFUnit;
    fn deref(&self) -> &BFUnit {
        &self.base
    }
}

impl std::ops::DerefMut for ZTUnit {
    fn deref_mut(&mut self) -> &mut BFUnit {
        &mut self.base
    }
}

#[derive(Debug, Getters)]
#[get = "pub"]
#[repr(C)]
pub(crate) struct ZTAnimal { // bytes: 0x3a8 = 936 bytes
    base: ZTUnit,  // offset: 0x0000
    _pad_0x0260: [u8; 300],
    food_tile: *const BFTile,  // offset: 0x038c
    _pad_0x0390: [u8; 4],
    is_boxed: bool,  // offset: 0x0394
    is_egg: bool,  // offset: 0x0395
    _pad_0x0396: [u8; 6],
    mbr_0x39c: u8,  // offset: 0x039c
    mbr_0x3a0: u8,  // offset: 0x03a0
    mbr_0x3a4: i8,  // offset: 0x03a4
    mbr_0x3a5: i8,  // offset: 0x03a5
    is_dying: bool,  // offset: 0x03a6
    mbr_0x3a7: i8,  // offset: 0x03a7
    mbr_0x3a8: u8,  // offset: 0x03a8
    mbr_0x3ac: u8,  // offset: 0x03ac
    mbr_0x3b0: u8,  // offset: 0x03b0
    mbr_0x3b4: u8,  // offset: 0x03b4
}

const _: () = assert!(mem::size_of::<ZTAnimal>() == 0x3a8);

impl ZTAnimal {
    /// Test-only fixture, see `BFEntity::new_for_test`. `mem::zeroed()` is safe here since
    /// every field is an integer/bool/byte-array/raw-pointer, none of them references or niche types.
    pub fn new_for_test(inner_class_ptr: u32, rotation: i32, map_footprint: i32, is_egg: bool, is_boxed: bool) -> Self {
        let mut animal: ZTAnimal = unsafe { mem::zeroed() };
        animal.base = ZTUnit::new_for_test(inner_class_ptr, rotation, map_footprint);
        animal.is_egg = is_egg;
        animal.is_boxed = is_boxed;
        animal
    }

    pub fn entity_type(&self) -> &'static ZTAnimalType {
        unsafe { ref_from_memory(self.inner_class_ptr) }
    }

    pub fn get_footprint(&self, use_map_footprint: bool) -> IVec3 {
        if !self.is_egg && !self.is_boxed {
            return self.base.get_footprint(use_map_footprint);
        }

        let type_info = self.entity_type();
        let footprint = if self.is_egg {
            type_info.egg_footprint
        } else {
            type_info.box_footprint
        };

        if self.rotation % 4 == 0 {
            IVec3 { x: footprint.x, y: footprint.y, z: footprint.z }
        } else {
            IVec3 { x: footprint.y, y: footprint.x, z: footprint.z }
        }
    }
}

impl std::ops::Deref for ZTAnimal {
    type Target = ZTUnit;
    fn deref(&self) -> &ZTUnit {
        &self.base
    }
}

impl std::ops::DerefMut for ZTAnimal {
    fn deref_mut(&mut self) -> &mut ZTUnit {
        &mut self.base
    }
}

pub fn read_zt_entity_from_memory(zt_entity_ptr: u32) -> ZTEntity {
    let inner_class_ptr = get_from_memory::<u32>(zt_entity_ptr + 0x128);

    ZTEntity {
        vtable: get_from_memory::<u32>(zt_entity_ptr),
        class: ZTEntityClass::from(get_from_memory::<u32>(zt_entity_ptr)),
        type_class: read_zt_entity_type_from_memory(inner_class_ptr),
        name: get_string_from_memory(get_from_memory::<u32>(zt_entity_ptr + 0x108)),
        pos1: get_from_memory::<u32>(zt_entity_ptr + 0x114),
        pos2: get_from_memory::<u32>(zt_entity_ptr + 0x118),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bfentitytype::BFEntityType;

    // Explicit list (not derived from the enum) so it visibly needs updating when a
    // variant is added and the match arm in `FromStr` is forgotten.
    const ZT_ENTITY_CLASS_CASES: &[(ZTEntityClass, &str)] = &[
        (ZTEntityClass::Food, "food"),
        (ZTEntityClass::Path, "path"),
        (ZTEntityClass::Fences, "fences"),
        (ZTEntityClass::Building, "building"),
        (ZTEntityClass::Animal, "animal"),
        (ZTEntityClass::Guest, "guest"),
        (ZTEntityClass::Scenery, "scenery"),
        (ZTEntityClass::Keeper, "keeper"),
        (ZTEntityClass::MaintenanceWorker, "maintenanceworker"),
        (ZTEntityClass::TourGuide, "tourguide"),
        (ZTEntityClass::Drt, "drt"),
        (ZTEntityClass::Ambient, "ambient"),
        (ZTEntityClass::Rubble, "rubble"),
        (ZTEntityClass::TankWall, "tankwall"),
        (ZTEntityClass::TankFilter, "tankfilter"),
    ];

    #[test]
    fn test_zt_entity_class_from_str_round_trip() {
        for (variant, s) in ZT_ENTITY_CLASS_CASES {
            assert_eq!(s.parse::<ZTEntityClass>().unwrap(), *variant, "failed to parse '{}'", s);
        }
    }

    #[test]
    fn test_zt_entity_class_from_str_case_insensitive() {
        assert_eq!("Animal".parse::<ZTEntityClass>().unwrap(), ZTEntityClass::Animal);
        assert_eq!("ANIMAL".parse::<ZTEntityClass>().unwrap(), ZTEntityClass::Animal);
        assert_eq!("aNiMaL".parse::<ZTEntityClass>().unwrap(), ZTEntityClass::Animal);
    }

    #[test]
    fn test_zt_entity_class_from_str_unknown_is_err() {
        assert!("not_a_real_entity_class".parse::<ZTEntityClass>().is_err());
    }

    fn entity_type_with_avoid_edges(avoid_edges: u32) -> BFEntityType {
        let mut entity_type: BFEntityType = unsafe { mem::zeroed() };
        entity_type.avoid_edges = avoid_edges;
        entity_type
    }

    #[test]
    fn test_check_avoid_edges_zero_always_false() {
        let entity_type = entity_type_with_avoid_edges(0);
        let entity = BFEntity::new_for_test(&entity_type as *const BFEntityType as u32, 0, 0);

        let mut tile = BFTile::new(IVec3::new(0, 0, 0), 0);
        tile.north_fence = 1;
        tile.east_fence = 1;
        tile.south_fence = 1;
        tile.west_fence = 1;

        assert!(!entity.check_avoid_edges(&tile), "avoid_edges == 0 should always return false, regardless of fence state");
    }

    #[test]
    fn test_check_avoid_edges_radius_zero_no_fences() {
        let entity_type = entity_type_with_avoid_edges(1);
        let entity = BFEntity::new_for_test(&entity_type as *const BFEntityType as u32, 0, 0);

        let tile = BFTile::new(IVec3::new(0, 0, 0), 0);

        assert!(!entity.check_avoid_edges(&tile));
    }

    #[test]
    fn test_check_avoid_edges_radius_zero_each_fence_direction_triggers() {
        let entity_type = entity_type_with_avoid_edges(1);
        let entity = BFEntity::new_for_test(&entity_type as *const BFEntityType as u32, 0, 0);
        let pos = IVec3::new(0, 0, 0);

        let mut north = BFTile::new(pos, 0);
        north.north_fence = 1;
        assert!(entity.check_avoid_edges(&north), "north_fence alone should trigger avoid_edges");

        let mut east = BFTile::new(pos, 0);
        east.east_fence = 1;
        assert!(entity.check_avoid_edges(&east), "east_fence alone should trigger avoid_edges");

        let mut south = BFTile::new(pos, 0);
        south.south_fence = 1;
        assert!(entity.check_avoid_edges(&south), "south_fence alone should trigger avoid_edges");

        let mut west = BFTile::new(pos, 0);
        west.west_fence = 1;
        assert!(entity.check_avoid_edges(&west), "west_fence alone should trigger avoid_edges");
    }

    fn entity_type_with_footprint(x: i32, y: i32, z: i32) -> BFEntityType {
        let mut entity_type: BFEntityType = unsafe { mem::zeroed() };
        entity_type.footprintx = x;
        entity_type.footprinty = y;
        entity_type.footprintz = z;
        entity_type
    }

    #[test]
    fn test_ztunit_get_footprint_map_footprint_ignores_rotation() {
        let entity_type = ZTUnitType::new_for_test(IVec3::default(), 42);
        for rotation in [-8, -3, -1, 0, 1, 3, 4, 8] {
            let entity = ZTUnit::new_for_test(&entity_type as *const ZTUnitType as u32, rotation, 0);
            let footprint = entity.get_footprint(true);
            assert_eq!(
                (footprint.x, footprint.y, footprint.z),
                (42, 42, 0),
                "use_map_footprint=true should return {{map_footprint, map_footprint, 0}} regardless of rotation ({})",
                rotation
            );
        }
    }

    #[test]
    fn test_ztunit_get_footprint_delegates_to_base_when_not_using_map_footprint() {
        let entity_type = entity_type_with_footprint(5, 7, 9);

        let unrotated = ZTUnit::new_for_test(&entity_type as *const BFEntityType as u32, 0, 42);
        let footprint = unrotated.get_footprint(false);
        assert_eq!((footprint.x, footprint.y, footprint.z), (5, 7, 9));

        let rotated = ZTUnit::new_for_test(&entity_type as *const BFEntityType as u32, 1, 42);
        let footprint = rotated.get_footprint(false);
        assert_eq!((footprint.x, footprint.y, footprint.z), (7, 5, 9), "odd rotation should swap x/y, matching BFEntity::get_footprint");
    }

    fn animal_type_with_footprints(egg: IVec3, boxed: IVec3) -> ZTAnimalType {
        let mut entity_type: ZTAnimalType = unsafe { mem::zeroed() };
        entity_type.egg_footprint = egg;
        entity_type.box_footprint = boxed;
        entity_type
    }

    #[test]
    fn test_ztanimal_get_footprint_delegates_to_base_when_not_egg_or_boxed() {
        let entity_type = entity_type_with_footprint(5, 7, 9);
        let entity = ZTAnimal::new_for_test(&entity_type as *const BFEntityType as u32, 1, 0, false, false);

        let footprint = entity.get_footprint(false);
        assert_eq!(
            (footprint.x, footprint.y, footprint.z),
            (7, 5, 9),
            "is_egg=false, is_boxed=false should delegate through ZTUnit/BFEntity::get_footprint"
        );
    }

    #[test]
    fn test_ztanimal_get_footprint_egg_uses_egg_footprint_with_rotation_swap() {
        let entity_type = animal_type_with_footprints(IVec3::new(11, 22, 33), IVec3::new(99, 98, 97));

        let unrotated = ZTAnimal::new_for_test(&entity_type as *const ZTAnimalType as u32, 0, 0, true, false);
        let footprint = unrotated.get_footprint(false);
        assert_eq!((footprint.x, footprint.y, footprint.z), (11, 22, 33));

        let rotated = ZTAnimal::new_for_test(&entity_type as *const ZTAnimalType as u32, 1, 0, true, false);
        let footprint = rotated.get_footprint(false);
        assert_eq!((footprint.x, footprint.y, footprint.z), (22, 11, 33));
    }

    #[test]
    fn test_ztanimal_get_footprint_boxed_uses_box_footprint() {
        let entity_type = animal_type_with_footprints(IVec3::new(11, 22, 33), IVec3::new(99, 98, 97));

        let entity = ZTAnimal::new_for_test(&entity_type as *const ZTAnimalType as u32, 0, 0, false, true);
        let footprint = entity.get_footprint(false);
        assert_eq!((footprint.x, footprint.y, footprint.z), (99, 98, 97));
    }

    #[test]
    fn test_ztanimal_get_footprint_egg_and_boxed_prefers_egg() {
        let entity_type = animal_type_with_footprints(IVec3::new(11, 22, 33), IVec3::new(99, 98, 97));

        let entity = ZTAnimal::new_for_test(&entity_type as *const ZTAnimalType as u32, 0, 0, true, true);
        let footprint = entity.get_footprint(false);
        assert_eq!((footprint.x, footprint.y, footprint.z), (11, 22, 33), "is_egg=true, is_boxed=true should prefer egg_footprint");
    }
}
