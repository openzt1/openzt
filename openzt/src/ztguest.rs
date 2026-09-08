//! Reimplementations of `ZTGuest`'s three megatile-reading methods -
//! `fCrowdDensityMegatile`/`fEstheticBonusMegatile`/`fStinkyMegatile`, detoured onto the reimplementations
//! below. These detours remove the last direct pointer-chasing reads of `ZTMegatileMgr`'s grid from
//! vanilla code (see `ztmegatilemgr.rs`'s module doc comment); `ZTMegatileMgr`'s struct layout must still
//! stay byte-exact, since the accessors this file calls
//! (`megatile()`/`guest_count()`/`stink()`/`category_value()`) still read vanilla's own live-owned memory,
//! not a migrated Rust store.
//!
//! No Windows decompile exists for any of the three, nor for their sole caller,
//! `ZTGuest::doEnvironmentEffectCheck` - all four addresses used here were confirmed by direct
//! disassembly of `zoo.dll`. See `private/docs/vtables/ZTGuest.md`'s "Non-virtual confirmed methods"
//! section for the full evidence chain (call-site uniqueness, the
//! `ZTGuestType::environment_effect_check`/`crowded_viewing_threshold`/`object_esthetic_threshold`/
//! `stink_threshold` field-offset matches, and the `GLOBAL_ZTMegatileMgr`/`MegatileRow`/`ZTMegatile`
//! stride matches).

use std::mem;

use openzt_detour::generated::ztguest::{F_CROWD_DENSITY_MEGATILE, F_ESTHETIC_BONUS_MEGATILE, F_STINKY_MEGATILE};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    globals::globals,
    util::{get_from_memory, ref_from_memory},
    ztmapview::BFTile,
    ztmegatilemgr::ZTMegatile,
    ztworldmgr::BFEntity,
};

/// Shared tile-to-megatile lookup for all three methods below: `ZTMegatile`s are indexed by `x/5`, `y/5`
/// (see `ztmegatilemgr.rs`'s own `recalculate_characteristics` doc comment for the same indexing), and a
/// negative tile coordinate (never real in practice, but not representable as the `usize` the accessor
/// takes) is treated the same as an out-of-range one: `None`.
fn tile_megatile(tile: &BFTile) -> Option<&'static ZTMegatile> {
    if tile.pos.x < 0 || tile.pos.y < 0 {
        return None;
    }
    globals().ztmegatilemgr().megatile((tile.pos.x / 5) as usize, (tile.pos.y / 5) as usize)
}

/// `ZTGuest::fCrowdDensityMegatile` (`0x0043b7e3`). Confirmed by the MSVC magic-constant (`0x51eb851f`)
/// division idiom at the tail of the real function, applied to the megatile's `guest_count` field
/// (offset `0x0`) - matching the mac decompile's own `guest_count()*10/25` formula exactly. `0` for a
/// tile with no allocated megatile, matching vanilla's own implicit "empty tile" behavior.
pub fn crowd_density_megatile(tile: &BFTile) -> i32 {
    tile_megatile(tile).map_or(0, |m| m.guest_count() * 10 / 25)
}

/// `ZTGuest::fStinkyMegatile` (`0x0043b84a`). Reads the megatile's `stink` scalar, confirmed by the real
/// function loading `ZTMegatile+0x10` (`fld dword ptr [eax+edx*4+0x10]`) and comparing the result against
/// `ZTGuestType::stink_threshold` (`bfentitytype.rs`, offset `0x2B0`). `0.0` for a tile with no allocated
/// megatile.
pub fn stinky_megatile(tile: &BFTile) -> f32 {
    tile_megatile(tile).map_or(0.0, |m| m.stink())
}

/// `ZTGuest::fEstheticBonusMegatile` (`0x0043b6c0`). Looks up `this`'s own entity type's category id (via
/// [`entity_category_id`]) in the megatile's `category_map` (`ZTMegatile::category_value`) - confirmed by
/// the real function's virtual call through the entity type's vtable slot `+0x20`, then a red-black-tree
/// search matching `category_map`'s existing shape, then a comparison of the found value against
/// `ZTGuestType::object_esthetic_threshold` (`bfentitytype.rs`, offset `0x2A4`). `0.0` for a tile with no
/// allocated megatile, or a category id with no entry in that megatile's map - vanilla's own
/// find-or-fail-then-`fld` idiom would return whatever the FPU stack happened to hold on a failed lookup,
/// a case never actually hit in practice since every real entity type has a populated category before a
/// guest ever calls this; `0.0` is a well-defined, safer stand-in for that unreachable branch.
pub fn esthetic_bonus_megatile(this: &BFEntity, tile: &BFTile) -> f32 {
    let Some(megatile) = tile_megatile(tile) else {
        return 0.0;
    };
    megatile.category_value(entity_category_id(this)).unwrap_or(0.0)
}

/// Calls `entity`'s own entity type's vtable slot `+0x20` - confirmed `ZTGuestType::0x00401fb7`
/// (`private/docs/vtables/ZTGuestType.md`), a trivial `mov eax,[ecx+0x130]; ret` getter - to resolve the
/// category id [`esthetic_bonus_megatile`] looks up. Same vtable-slot call-through shape as
/// `ztmegatilemgr::entity_type_matches`.
///
/// Offset `0x130` on a `ZTGuestType`/`ZTUnitType` instance is also exactly `BFUnitType::name_id`
/// (`bfentitytype.rs:765`, a string resource id, not obviously a `BFCategory`-style category id in the
/// `0x251f..0x2523` range `recalculate_characteristics` populates the map with) - this may be a genuine
/// "category id" field that happens to share `name_id`'s slot by coincidence of this project's own field
/// naming, or evidence the vtable-slot identification needs another look. Not asserted as settled; the
/// live `ZTGUEST_MEGATILE_METHODS_LIVE` comparison in `reimplementation_tests` is the actual verification
/// for this call, not this doc comment.
fn entity_category_id(entity: &BFEntity) -> i32 {
    let entity_type_ptr = *entity.inner_class_ptr();
    let vtable = get_from_memory::<u32>(entity_type_ptr);
    let get_category_id_fn = unsafe { mem::transmute::<u32, extern "thiscall" fn(u32) -> i32>(get_from_memory::<u32>(vtable + 0x20)) };
    get_category_id_fn(entity_type_ptr)
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(F_CROWD_DENSITY_MEGATILE)]
    unsafe extern "thiscall" fn f_crowd_density_megatile(this: *const u32) -> i32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(this) };
        let Some(tile) = entity.get_tile() else {
            return 0;
        };
        crowd_density_megatile(&tile)
    }

    #[detour(F_STINKY_MEGATILE)]
    unsafe extern "thiscall" fn f_stinky_megatile(this: *const u32) -> f32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(this) };
        let Some(tile) = entity.get_tile() else {
            return 0.0;
        };
        stinky_megatile(&tile)
    }

    #[detour(F_ESTHETIC_BONUS_MEGATILE)]
    unsafe extern "thiscall" fn f_esthetic_bonus_megatile(this: *const u32) -> f32 {
        let entity = unsafe { ref_from_memory::<BFEntity>(this) };
        let Some(tile) = entity.get_tile() else {
            return 0.0;
        };
        esthetic_bonus_megatile(entity, &tile)
    }
}

/// Registers this module's live detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest detours: {e:?}");
    }
}

/// Live-comparison test support for `reimplementation_tests`.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;
    use crate::bfentitytype::ZTEntityTypeClass;

    /// Finds every live `ZTGuest` in the world's entity array (via `BFEntity::entity_type_class()`, the
    /// same coarse type check `list_entities`/`get_zt_world_mgr_entities` already use elsewhere),
    /// returning each one's raw pointer and current tile - so a live comparison can sample the whole
    /// live population (different tiles, different megatiles, different entity-type category ids)
    /// instead of just whichever guest happens to be first in the array. Empty if no guest is currently
    /// on a resolvable tile (e.g. an empty/guest-less save, or a save loaded before any guest has
    /// spawned).
    pub(crate) fn find_live_guests() -> Vec<(u32, BFTile)> {
        let world = globals().ztworldmgr();
        let mut addr = world.entity_array_start();
        let end = world.entity_array_end();
        let mut guests = Vec::new();
        while addr < end {
            let entity_ptr = get_from_memory::<u32>(addr);
            if entity_ptr != 0 {
                let entity = unsafe { ref_from_memory::<BFEntity>(entity_ptr) };
                if entity.entity_type_class() == ZTEntityTypeClass::Guest
                    && let Some(tile) = entity.get_tile()
                {
                    guests.push((entity_ptr, tile));
                }
            }

            addr += 4;
        }
        guests
    }
}
