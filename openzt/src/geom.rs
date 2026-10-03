use num_enum::FromPrimitive;
use std::fmt;

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct IVec3 {
    pub x: i32,
    pub y: i32,
    pub z: i32,
}

impl IVec3 {
    pub fn new(x: i32, y: i32, z: i32) -> Self {
        IVec3 { x, y, z }
    }
}

impl fmt::Display for IVec3 {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(f, "Vec3 {{ x: {}, y: {}, z: {} }}", self.x, self.y, self.z)
    }
}

#[repr(C)]
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Rectangle {
    pub min_x: i32,
    pub min_y: i32,
    pub max_x: i32,
    pub max_y: i32,
}

impl Rectangle {
    pub fn contains_point(&self, point: &IVec3) -> bool {
        point.x >= self.min_x && point.x <= self.max_x && point.y >= self.min_y && point.y <= self.max_y
    }
}

/// `north_fence`/`east_fence`/`south_fence`/`west_fence` field names - confirmed by cross-referencing
/// `ZTHabitat_addSeedsOnStack.c`/`ZTHabitat_addContiguousSpan.c`'s fence-passability checks against which
/// direction each is paired with (e.g. direction `4` checks `south_fence` on the source tile and
/// `north_fence` on the neighbour it steps to, which only makes sense if direction `4` is `South`). The
/// previous names were a clean 90°/2-step rotation of the correct ones (`West`→`North`,
/// `NorthWest`→`NorthEast`, etc.) - every discriminant value is unchanged, so this only relabels which
/// name refers to which numeric direction; nothing that calls `Direction::from(u32)` changes behavior.
#[derive(Debug, PartialEq, Eq, FromPrimitive, Clone, Copy, Default)]
#[repr(u32)]
pub enum Direction {
    #[default]
    North = 0,
    NorthEast = 1,
    East = 2,
    SouthEast = 3,
    South = 4,
    SouthWest = 5,
    West = 6,
    NorthWest = 7,
}
