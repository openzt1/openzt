//! `ZTBuilding::turnToRubble`: the building leaves the map and a field of rubble scenery takes its footprint.
//!
//! The rubble pieces share one vanilla-allocated `vector<uint>` of their entity ids, which belongs to them and is not
//! freed here. As in vanilla, it is leaked when no piece is created.
//!
//! The `ZTScenery` base's `turnToRubble`, `createScenery` and `ZTRubble`'s animation and sound stay vanilla's. Where
//! vanilla faults, the port does this instead:
//! - a building without a type, or no world: nothing happens beyond emptying the building;
//! - no memory for the id vector: the pieces are created without one.

use std::mem::offset_of;

use glam::IVec3;
use openzt_detour::{
    generated::{
        msvc_std::RAND as MSVC_RAND,
        poolalloc::DEALLOCATE as POOLALLOC_DEALLOCATE,
        standalone::OPERATOR_NEW,
        ztapp::GET_APP,
        ztbuilding::TURN_TO_RUBBLE,
        ztmapview::CHECK_UNDO_BUFFER,
        ztrubble::{PLAY_EXPLOSION_ANIMATION, PLAY_EXPLOSION_SOUND},
        ztscenery::TURN_TO_RUBBLE as SCENERY_TURN_TO_RUBBLE,
        ztui_general::GET_MAPVIEW,
        ztworldmgr::CREATE_SCENERY,
    },
    FunctionDef,
};
use openzt_detour_macro::detour_mod;

use super::{
    lifecycle::offset_in_tile,
    users::{call0, virtual_method, word},
    ZTBuilding, ENTITY_ID,
};
use crate::{
    bfentitytype::ZTSceneryType,
    globals::globals,
    util::{get_from_memory, save_to_memory},
    vanilla_string::VanillaString,
    vanilla_vector::push_word,
    ztunit::map_tile,
};

/// `BFApp::buildString(app, out, string id, argument)` (`0x0041ca7f`), variadic; the generated entry stops at the
/// string id.
const BUILD_STRING: FunctionDef<unsafe extern "cdecl" fn(*const u32, *const u32, u32, u32) -> *const u32> = FunctionDef::new(0x0041_ca7f);

/// The string the rubble is named with, formatted with the building's name.
const RUBBLE_NAME_STRING: u32 = 0x2910;
/// The type id of the buildings that leave bathroom rubble.
const BATHROOM_TYPE_ID: u32 = 0x1f48;

/// What `findTypes` is asked for.
const RUBBLE_CLASS: &[u8] = b"rubble";
const RUBBLE_CATEGORY: &[u8] = b"other";
const BATHROOM_RUBBLE: &[u8] = b"bathrubl";
const SMALL_RUBBLE: &[u8] = b"brubls1";
/// The big pieces; the first has to exist, the rest are used when they do.
const BIG_RUBBLE: [&[u8]; 4] = [b"brubble1", b"brubble2", b"brubble3", b"brubble4"];

/// The entity's name (`char*`) and position.
const ENTITY_NAME: u32 = 0x108;
const ENTITY_POS: u32 = 0x114;
const _: () = assert!(offset_of!(ZTSceneryType, forces_scenery_rubble) == 0x13a);
/// Rubble fields: the shared id vector and the type id of what was destroyed.
const RUBBLE_GROUP: u32 = 0x158;
const RUBBLE_SOURCE_TYPE_ID: u32 = 0x15c;

mod slots {
    /// Entities.
    pub const SET_NAME: u32 = 0x1c;
    pub const REMOVE_FROM_MAP: u32 = 0x78;
    pub const GET_FOOTPRINT: u32 = 0x94;
    pub const SET_IS_REMOVED: u32 = 0xa8;
    /// Entity types.
    pub const TYPE_ID: u32 = 0x20;
    /// `BFWorldMgr`.
    pub const FIND_TYPES: u32 = 0x14;
}

/// Where the pieces go over a footprint of `width` by `depth` half-tiles centred on `position`: the position of each
/// piece, and whether it is the first of its column. Big pieces cover two half-tiles each way.
fn piece_positions(position: [i32; 3], width: i32, depth: i32, big: bool) -> impl Iterator<Item = ([i32; 3], bool)> {
    let step = if big { 2 } else { 1 };
    let start = |centre: i32, size: i32| centre.wrapping_sub(size.wrapping_mul(32) / 2).wrapping_add(16);
    let (start_x, start_y) = (start(position[0], width), start(position[1], depth));
    (0..width.max(0)).step_by(step).flat_map(move |column| {
        let x = start_x.wrapping_add(column.wrapping_mul(32));
        (0..depth.max(0))
            .step_by(step)
            .map(move |row| ([x, start_y.wrapping_add(row.wrapping_mul(32)), position[2]], row == 0))
    })
}

/// The rubble types `findTypes` has under `name`.
fn rubble_types(world: u32, name: &[u8]) -> Vec<u32> {
    type FindTypes = extern "thiscall" fn(u32, *mut [u32; 3], *const u32, *const u32, *const u32) -> *const [u32; 3];
    let (class, category, name) = (
        VanillaString::from_bytes(RUBBLE_CLASS),
        VanillaString::from_bytes(RUBBLE_CATEGORY),
        VanillaString::from_bytes(name),
    );
    let mut found = crate::pool_vec::PoolVec::four_byte_elements();
    let find_types = unsafe { virtual_method::<FindTypes>(world, slots::FIND_TYPES) };
    find_types(world, found.out_pointer(), class.as_ptr(), category.as_ptr(), name.as_ptr());
    (found.begin()..found.end()).step_by(4).map(word).collect()
}

/// The types to scatter, or `None` when the one the building needs does not exist.
fn candidate_types(world: u32, bathroom: bool, small: bool) -> Option<Vec<u32>> {
    let first = |name: &[u8]| rubble_types(world, name).first().copied();
    if bathroom {
        return Some(vec![first(BATHROOM_RUBBLE)?]);
    }
    if small {
        return Some(vec![first(SMALL_RUBBLE)?]);
    }
    let mut types = vec![first(BIG_RUBBLE[0])?];
    types.extend(BIG_RUBBLE[1..].iter().filter_map(|&name| first(name)));
    Some(types)
}

/// An empty `vector<uint>` header from vanilla's allocator.
fn new_group() -> u32 {
    let group = unsafe { OPERATOR_NEW.original()(0xc) } as u32;
    if group != 0 {
        for offset in [0, 4, 8] {
            save_to_memory(group + offset, 0u32);
        }
    }
    group
}

impl ZTBuilding {
    /// `ZTBuilding::turnToRubble` (`0x00611bd6`).
    pub fn turn_to_rubble(self) {
        let building_type = self.entity_type();
        self.empty_grandstands();
        self.remove_all_users();
        if building_type == 0 {
            return;
        }
        if get_from_memory::<u8>(building_type + offset_of!(ZTSceneryType, forces_scenery_rubble) as u32) != 0 {
            unsafe { SCENERY_TURN_TO_RUBBLE.hooked()(self.0 as *const std::ffi::c_void) };
            return;
        }

        let name = VanillaString::rvo_target();
        unsafe {
            BUILD_STRING.original()(GET_APP.original()(), name.as_ptr(), RUBBLE_NAME_STRING, word(self.0 + ENTITY_NAME));
        }
        let world = globals().ztworldmgr_ptr() as u32;
        if world == 0 {
            return;
        }
        let position: [i32; 3] = std::array::from_fn(|axis| get_from_memory(self.0 + ENTITY_POS + axis as u32 * 4));
        let mut footprint = [0i32; 3];
        let (width, depth) = unsafe {
            type GetFootprint = extern "thiscall" fn(u32, *mut [i32; 3], u32) -> *const [i32; 3];
            let result = virtual_method::<GetFootprint>(self.0, slots::GET_FOOTPRINT)(self.0, &mut footprint, 0);
            (get_from_memory::<i32>(result as u32), get_from_memory::<i32>(result as u32 + 4))
        };
        let source_type_id = call0(self.entity_type(), slots::TYPE_ID);

        unsafe {
            virtual_method::<extern "thiscall" fn(u32, u32)>(self.0, slots::REMOVE_FROM_MAP)(self.0, 0);
            virtual_method::<extern "thiscall" fn(u32, u32, u32)>(self.0, slots::SET_IS_REMOVED)(self.0, 1, 0);
            let map_view = GET_MAPVIEW.hooked()() as u32;
            if map_view != 0 {
                CHECK_UNDO_BUFFER.hooked()(map_view as *const u32, self.0 as *const i32);
            }
        }

        let bathroom = call0(self.entity_type(), slots::TYPE_ID) == BATHROOM_TYPE_ID;
        let small = !bathroom && (width <= 1 || depth <= 1);
        let Some(types) = candidate_types(world, bathroom, small) else {
            return;
        };

        let group = new_group();
        for (piece, first_of_column) in piece_positions(position, width, depth, !small) {
            let tile = map_tile(piece[0] / 64, piece[1] / 64);
            if tile == 0 {
                continue;
            }
            let rubble_type = types[unsafe { MSVC_RAND.hooked()() } as usize % types.len()];
            let offset = if small { offset_in_tile(piece) } else { IVec3::new(32, 32, 0) };
            let rubble =
                unsafe { CREATE_SCENERY.hooked()(world as *const u32, rubble_type as *const u32, 0, tile as *const u32, offset) } as u32;
            if rubble == 0 {
                continue;
            }
            unsafe { virtual_method::<extern "thiscall" fn(u32, *const u32)>(rubble, slots::SET_NAME)(rubble, name.as_ptr()) };
            save_to_memory(rubble + RUBBLE_GROUP, group);
            save_to_memory(rubble + RUBBLE_SOURCE_TYPE_ID, source_type_id);
            unsafe { PLAY_EXPLOSION_ANIMATION.hooked()(rubble as *const u32) };
            if group != 0 {
                push_word(group, word(rubble + ENTITY_ID), |begin, bytes| {
                    if begin != 0 {
                        unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) }
                    }
                });
            }
            if first_of_column {
                unsafe { PLAY_EXPLOSION_SOUND.hooked()(rubble as *const u32) };
            }
        }
    }
}

#[detour_mod]
mod rubble_detours {
    use super::*;

    #[detour(TURN_TO_RUBBLE)]
    unsafe extern "thiscall" fn turn_to_rubble(this: *const u32) {
        ZTBuilding(this as u32).turn_to_rubble();
    }

    /// The vanilla body, for the live tests.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn turn_to_rubble(this: u32) {
            unsafe { super::TURN_TO_RUBBLE_DETOUR.call(this as *const u32) }
        }
    }
}

/// Installs the rubble detour.
pub(super) fn init() {
    if let Err(e) = unsafe { rubble_detours::init_detours() } {
        tracing::error!("Failed to initialise ztbuilding rubble detour: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    rubble_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use rubble_detours::test_real;

#[cfg(test)]
mod tests {
    use super::piece_positions;

    fn pieces(width: i32, depth: i32, big: bool) -> Vec<([i32; 3], bool)> {
        piece_positions([128, 256, -3], width, depth, big).collect()
    }

    #[test]
    fn small_pieces_cover_every_half_tile() {
        assert_eq!(pieces(1, 2, false), vec![([128, 240, -3], true), ([128, 272, -3], false)]);
    }

    #[test]
    fn big_pieces_cover_every_other_half_tile_rounding_up() {
        assert_eq!(
            pieces(3, 2, true),
            vec![([96, 240, -3], true), ([160, 240, -3], true)],
        );
    }

    #[test]
    fn empty_footprints_have_no_pieces() {
        assert!(pieces(0, 4, false).is_empty());
        assert!(pieces(-2, 4, true).is_empty());
        assert!(pieces(4, -1, true).is_empty());
    }
}
