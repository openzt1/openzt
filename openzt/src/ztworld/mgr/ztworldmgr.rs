use std::fmt;

use crate::geom::{Direction, IVec3};
use crate::util::get_from_memory;
use crate::ztmapview::BFTile;
use crate::ztworld::entity::BFEntity;

#[derive(Debug)]
#[repr(C)]
pub struct ZTWorldMgr {
    paddin_0: [u8; 0x14], // 0x10?
    zoom_level: i32,
    padding_1: [u8; 0x1c], // Tentative
    pub map_x_size: u32,
    pub map_y_size: u32,
    padding_2: [u8; 0x4],
    tile_array: u32,
    padding_3: [u8; 0x3c],
    pub(crate) entity_array_start: u32,
    pub(crate) entity_array_end: u32,
    entity_array_buffer_end: u32,
    padding_4: [u8; 0xc],
    pub(crate) entity_type_array_start: u32,
    pub(crate) entity_type_array_end: u32,
    entity_type_array_buffer_end: u32,
    pub _live: crate::util::LiveMemory,
}

impl ZTWorldMgr {
    /// The manager's own flat `entity_array` (every live entity in the world) - exposed read-only for
    /// [`crate::zthabitatmgr::ZTHabitatMgr::replace_gate`]'s own real vanilla search (confirming a
    /// stashed gate-fence pointer is still a live world entity before converting it back to a plain
    /// fence; `ZTHabitatMgr_replaceGate.c`/`.asm` walks `entity_array_start`/`_end` directly).
    pub fn entity_array(&self) -> impl Iterator<Item = u32> + '_ {
        let mut ptr = self.entity_array_start;
        std::iter::from_fn(move || {
            if ptr >= self.entity_array_end {
                None
            } else {
                let value = get_from_memory::<u32>(ptr);
                ptr += 4;
                Some(value)
            }
        })
    }
}

impl fmt::Display for ZTWorldMgr {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        write!(
            f,
            "ZTWorldMgr {{ zoom_level: {}, map_x_size: {}, map_y_size: {}, tile_array: {:#x}, entity_array_start: {:#x}, entity_array_end: {:#x}, entity_type_array_start: {:#x}, entity_type_array_end: {:#x} }}",
            self.zoom_level,
            self.map_x_size,
            self.map_y_size,
            self.tile_array,
            self.entity_array_start,
            self.entity_array_end,
            self.entity_type_array_start,
            self.entity_type_array_end,
        )
    }
}

const TILE_SIZE: i32 = 0x40;

/// `BFTile::pos`'s `x`/`y` offsets.
const BFTILE_POS_X_OFFSET: u32 = 0x34;
const BFTILE_POS_Y_OFFSET: u32 = 0x38;

/// `BFMap::getNeighbor`'s `(dx, dy)` direction table; anything outside `0..=7` is `(0, 0)`.
fn neighbour_offset(direction: u32) -> (i32, i32) {
    match direction {
        0 => (0, -1),
        1 => (1, -1),
        2 => (1, 0),
        3 => (1, 1),
        4 => (0, 1),
        5 => (-1, 1),
        6 => (-1, 0),
        7 => (-1, -1),
        _ => (0, 0),
    }
}
const ELEVATION_SCALE: i32 = 0x10; // 16 units per elevation level

impl ZTWorldMgr {
    pub fn zoom_level(&self) -> i32 {
        self.zoom_level
    }

    pub fn set_zoom_level(&mut self, zoom_level: i32) {
        self.zoom_level = zoom_level;
    }

    /// Get the start of the entity array in memory
    pub fn entity_array_start(&self) -> u32 {
        self.entity_array_start
    }

    /// Get the end of the entity array in memory
    pub fn entity_array_end(&self) -> u32 {
        self.entity_array_end
    }

    pub fn get_neighbour(&self, bftile: &BFTile, direction: Direction) -> Option<BFTile> {
        let x_offset: i32 = match direction {
            Direction::North => 0,
            Direction::NorthEast => 1,
            Direction::East => 1,
            Direction::SouthEast => 1,
            Direction::South => 0,
            Direction::SouthWest => -1,
            Direction::West => -1,
            Direction::NorthWest => -1,
        };
        let y_offset: i32 = match direction {
            Direction::North => -1,
            Direction::NorthEast => -1,
            Direction::East => 0,
            Direction::SouthEast => 1,
            Direction::South => 1,
            Direction::SouthWest => 1,
            Direction::West => 0,
            Direction::NorthWest => -1,
        };

        let x: i32 = bftile.pos.x + x_offset;
        let y: i32 = bftile.pos.y + y_offset;

        if x < 0 || x >= self.map_x_size as i32 || y < 0 || y >= self.map_y_size as i32 {
            return None;
        }

        Some(get_from_memory::<BFTile>(self.tile_array + (((y as u32 * self.map_x_size) + x as u32) * 0x8c_u32)))
    }

    /// Ports `BFMap::getNeighbor` (`BFMap_getNeighbor_0.c`/`_1.c`) by pointer: returns the address of the
    /// tile one step from `tile_ptr` in `direction`, or `0` for a null `tile_ptr` or a step off the map.
    /// A `direction` outside `0..=7` uses a `(0, 0)` offset, so it returns `tile_ptr` itself - unlike
    /// [`Direction::from`], which maps those values to `North`. Reads only the tile's position, with no
    /// `BFTile` copy.
    pub fn get_neighbour_ptr_raw(&self, tile_ptr: u32, direction: u32) -> u32 {
        if tile_ptr == 0 {
            return 0;
        }
        let (dx, dy) = neighbour_offset(direction);
        let x = get_from_memory::<i32>(tile_ptr + BFTILE_POS_X_OFFSET).wrapping_add(dx);
        let y = get_from_memory::<i32>(tile_ptr + BFTILE_POS_Y_OFFSET).wrapping_add(dy);
        if x < 0 || y < 0 || x >= self.map_x_size as i32 || y >= self.map_y_size as i32 {
            return 0;
        }
        self.get_tile_ptr(x as u32, y as u32)
    }

    pub fn get_ptr_from_bftile(&self, bftile: &BFTile) -> u32 {
        let x = bftile.pos.x as u32;
        let y = bftile.pos.y as u32;
        self.tile_array + ((y * self.map_x_size + x) * 0x8c)
    }

    /// Same address math as [`Self::get_ptr_from_bftile`], for callers that only have raw `(x, y)`
    /// coordinates (e.g. [`crate::zthabitatmgr::ZTHabitatMgr::get_zoo_entrance_tile_ptr`]) rather than
    /// an already-read `BFTile`. No bounds check - callers that need one (real vanilla
    /// `ZTHabitatMgr::getZooEntranceTile` included) check `map_x_size`/`map_y_size` themselves first.
    pub fn get_tile_ptr(&self, x: u32, y: u32) -> u32 {
        self.tile_array + ((y * self.map_x_size + x) * 0x8c)
    }

    pub fn get_tile_from_pos(&self, pos: IVec3) -> Option<BFTile> {
        let x = pos.x as u32;
        let y = pos.y as u32;
        if x >= self.map_x_size || y >= self.map_y_size {
            return None;
        }
        Some(get_from_memory::<BFTile>(self.tile_array + ((y * self.map_x_size + x) * 0x8c)))
    }

    pub fn get_tile_from_coords(&self, x_coord: i32, y_coord: i32) -> Option<BFTile> {
        let x = (x_coord as u32) >> 6; // Convert to tile coordinates
        let y = (y_coord as u32) >> 6; // Convert to tile coordinates
        if x >= self.map_x_size || y >= self.map_y_size {
            return None;
        }
        Some(get_from_memory::<BFTile>(self.tile_array + ((y * self.map_x_size + x) * 0x8c)))
    }

    // TODO: Should borrow both of these IVec3s instead of taking ownership
    pub fn tile_to_world(&self, tile_pos: IVec3, local_pos: IVec3) -> IVec3 {
        let tile_x = tile_pos.x;
        let tile_y = tile_pos.y;

        // Get the tile at the specified position, if it exists and is within bounds
        let tile = self.get_tile_from_pos(tile_pos);

        // Calculate elevation based on tile data
        let world_z = match tile {
            Some(tile_ref) => {
                let local_elevation = tile_ref.get_local_elevation(local_pos);
                local_elevation + tile_ref.pos.z * ELEVATION_SCALE
            }
            None => 0,
        };

        // Convert tile coordinates to world coordinates
        IVec3 {
            x: tile_x * TILE_SIZE + local_pos.x,
            y: tile_y * TILE_SIZE + local_pos.y,
            z: world_z,
        }
    }

    /// Resolves a `BFEntity`'s numeric `id` (see `BFEntity::id`, `+0x124`) back to its live pointer,
    /// via `ZTWorldMgr`'s own vtable slot 9 (offset `0x24`, target address `0x0041176c`, inherited
    /// unchanged from `BFWorldMgr`) - the mechanism `ZTThought::populate`/`load` use to re-resolve
    /// `thinker_ptr`/`object_ptr` after a save/load round-trip, since only the numeric id (not the raw
    /// pointer) survives a save. Modeled on `BFEntity::vtable_get_footprint`'s pattern of reading a
    /// function pointer at `vtable + offset` and transmuting it to an `extern "thiscall" fn`; unlike
    /// that method, `ZTWorldMgr` has no named `vtable` field (it's folded into `paddin_0`), so this
    /// reads the vtable pointer from `self`'s own address instead.
    ///
    /// The trailing flag byte the vtable function itself takes is hardcoded to `1` here rather than
    /// exposed as a parameter - every call site seen in the decompile passes `1`.
    pub fn resolve_entity_by_id(&self, id: u32) -> *mut BFEntity {
        let vtable = get_from_memory::<u32>(self as *const Self as u32);
        let function_address = get_from_memory::<u32>(vtable + 0x24);
        let resolve_fn = unsafe {
            std::mem::transmute::<u32, extern "thiscall" fn(this: *const ZTWorldMgr, id: u32, flag: u8) -> *mut BFEntity>(function_address)
        };
        resolve_fn(self, id, 1)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const TILE_STRIDE: usize = 0x8c;

    /// A zeroed `ZTWorldMgr` over a `width` x `height` tile buffer whose tiles carry their own positions.
    fn fixture(width: u32, height: u32) -> (ZTWorldMgr, Vec<u32>) {
        let mut tiles = vec![0u32; (width * height) as usize * TILE_STRIDE / 4];
        for y in 0..height {
            for x in 0..width {
                let base = (y * width + x) as usize * TILE_STRIDE / 4;
                tiles[base + BFTILE_POS_X_OFFSET as usize / 4] = x;
                tiles[base + BFTILE_POS_Y_OFFSET as usize / 4] = y;
            }
        }
        let mut world: ZTWorldMgr = unsafe { std::mem::zeroed() };
        world.map_x_size = width;
        world.map_y_size = height;
        world.tile_array = tiles.as_ptr() as u32;
        (world, tiles)
    }

    #[test]
    fn get_neighbour_ptr_raw_steps_in_each_direction() {
        let (world, _tiles) = fixture(3, 3);
        let centre = world.get_tile_ptr(1, 1);
        let expected = [(1, 0), (2, 0), (2, 1), (2, 2), (1, 2), (0, 2), (0, 1), (0, 0)];
        for (direction, (x, y)) in expected.into_iter().enumerate() {
            assert_eq!(world.get_neighbour_ptr_raw(centre, direction as u32), world.get_tile_ptr(x, y), "direction {direction}");
        }
    }

    #[test]
    fn get_neighbour_ptr_raw_out_of_range_direction_returns_input_tile() {
        let (world, _tiles) = fixture(3, 3);
        let corner = world.get_tile_ptr(0, 0);
        for direction in [8, 9, 0xff, 0xffff_ffff] {
            assert_eq!(world.get_neighbour_ptr_raw(corner, direction), corner, "direction {direction:#x}");
        }
    }

    #[test]
    fn get_neighbour_ptr_raw_off_map_or_null_returns_zero() {
        let (world, _tiles) = fixture(3, 3);
        assert_eq!(world.get_neighbour_ptr_raw(world.get_tile_ptr(0, 0), 0), 0);
        assert_eq!(world.get_neighbour_ptr_raw(world.get_tile_ptr(2, 2), 3), 0);
        assert_eq!(world.get_neighbour_ptr_raw(0, 0), 0);
    }
}
