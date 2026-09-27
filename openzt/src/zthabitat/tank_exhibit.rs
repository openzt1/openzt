use getset::Getters;
use super::habitat::ZTHabitat;
use crate::util::LiveMemory;

/// `ZTTankExhibit`, the single-inheritance subclass of [`ZTHabitat`] real tank/show exhibits are
/// instantiated as (allocated via `operator_new(0x1e8)` by `ZTTankExhibit::cls_0x40183b`, confirmed at
/// the `.asm` level; see `ZTHabitat`'s own doc comment for the allocator bifurcation).
///
/// embeds the base class at offset 0 (matching real C++ single inheritance layout), so every
/// `ZTHabitat` field/method is reachable through it. Only ever construct this from a pointer already
/// confirmed via `ZTHabitat::is_tank()` - reading it from a real, plain `ZTHabitat` (0x178 bytes) would
/// over-read past that object's actual allocation, exactly the bug this struct split fixes.
///
/// `0x18c` onward was previously modeled as opaque padding; `ZTTankExhibit_isRightSalinity.c`/
/// `_update.c`/`_addRandomSparkle.c` (the last confirmed via Ghidra MCP identification of the former
/// `FUN_00496382`) prove almost all of it is real fields (sparkle-effect bookkeeping plus a
/// current/pending water-type pair), named below with the confidence each decompile actually supports -
/// several names are inferred from call shape, not confirmed by any setter/getter method name, and are
/// flagged as such.
#[derive(Debug, Getters)]
#[repr(C)]
#[get = "pub"]
pub struct ZTTankExhibit {
    pub habitat: ZTHabitat,   // 0x000 - 0x178
    pad_tank1: [u8; 0xc], // ----------------------- padding: 12 bytes (0x178-0x184)
    tank_height: u32,     // 0x184 // Actual structural tank height (ZTTankExhibit::getTankHeight/setTankHeight); not the field checkTankPlacement compares against, see water_level.
    water_level: u32,     // 0x188 // Current water level (ZTTankExhibit::getWaterLevel); this is what checkTankPlacement's height comparisons actually use. Also `addRandomSparkle`'s "tank has any water at all" gate (`!= 0`).
    current_water_type: i32, // 0x18c // Inferred, not confirmed by any method name: `isRightSalinity` flips its salinity comparison when this is `0` (read as a fresh/salt discriminant), and `update` overwrites it from `pending_water_type` exactly when the tank finishes draining (`water_level == 0`), immediately following with `updateAdjustmentCosts`+`fill` - i.e. this is the water type the tank is *currently* filled/configured for, applied lazily on drain rather than immediately on `setBaseTerrainType`.
    pending_water_type: i32, // 0x190 // Inferred: the requested water type staged by (the un-ported) `ZTTankExhibit::setBaseTerrainType`, compared against `current_water_type` and swapped in once the tank fully drains - see `current_water_type`'s own note.
    pad_tank2b: [u8; 0x4], // ----------------------- padding: 4 bytes (0x194-0x198, not read by isRightSalinity/update/addRandomSparkle)
    is_filled: bool,      // 0x198 // Set true by ZTTankExhibit::fill(), false by ZTTankExhibit::drain(). Confirmed against ZTTankExhibit_fill.c/ZTTankExhibit_drain.c/ZTMapView_checkTankPlacement.c (field_0x198, checked as `this_00->field_0x198 == '\0'`) - not flipped between platforms.
    pad_tank3a: [u8; 0xf], // ----------------------- padding: 15 bytes (0x199-0x1a8, not yet reverse-engineered)
    water_purity: i32,     // 0x1a8 // Inferred: current purity level - `update`'s countdown-expiry branch only fires while `> 0` and calls `setWaterPurity(this, water_purity - 1)` on expiry, matching the already-generated `zttankexhibit::SET_WATER_PURITY` setter's own shape (not yet decompiled to directly confirm it writes this exact offset).
    water_purity_timer: i32, // 0x1ac // Inferred: ticks down by `elapsed` each `update()` call; reloaded from `DAT_0063af04` and triggers the `water_purity` decrement above when it drops to `<= 0`.
    pad_tank3b: [u8; 0x14], // ----------------------- padding: 20 bytes (0x1b0-0x1c4, not yet reverse-engineered)
    sparkle_entities_begin: u32, // 0x1c4 // Inferred: begin pointer of a real vanilla `std::vector<std::pair<i32,BFEntity*>>` - `addRandomSparkle`'s own growth path (`cls_0x43f6a9::meth_0x43f6a9` when `end == cap_end`, else in-place `*end = 0; end[1] = entity`) is the same begin/end/cap_end vector-growth shape used elsewhere in this codebase (e.g. `ZTHabitat::viewing_areas_begin`), tracking each spawned sparkle entity this tank owns.
    sparkle_entities_end: u32,  // 0x1c8
    sparkle_entities_cap_end: u32, // 0x1cc
    sparkle_entity_ids_begin: u32, // 0x1d0 // Inferred: begin pointer of a parallel `std::vector<u32>` (`msvc_std::vector_pod<>::_Insert_n` growth path) storing each spawned sparkle's own `+0x124` id field, one entry per `sparkle_entities_*` entry above.
    sparkle_entity_ids_end: u32,   // 0x1d4
    sparkle_entity_ids_cap_end: u32, // 0x1d8
    salt_water_sparkle_vtable_cache: u32, // 0x1dc // Inferred: cached `BFEntity` vtable pointer for the "twaterwv"-named entity type (`BFWorldMgr::findTypes` result, looked up once and reused), constructed via its own vtable slot `+0x24` (a virtual-constructor idiom) when `addRandomSparkle` spawns a non-freshwater sparkle.
    fresh_water_sparkle_vtable_cache: u32, // 0x1e0 // Inferred: same shape as `salt_water_sparkle_vtable_cache`, for the "frshwav"-named entity type used when `addRandomSparkle`'s freshwater branch (`current_water_type == 0`) spawns instead.
    sparkle_spawn_timer: i32, // 0x1e4 // Inferred: ticks down by `elapsed` each `update()` call; on reaching `< 0`, `update` calls `addRandomSparkle` and reloads this from `DAT_006390b8 / owned_tile_count`.
    #[getset(skip)]
    pub _live: LiveMemory, // trailing ZST - see LiveMemory's own docs; required once this struct gets real methods that read live game memory through &self (isRightSalinity/update).
}

const _: () = assert!(std::mem::size_of::<ZTTankExhibit>() == 0x1e8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, current_water_type) == 0x18c);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, pending_water_type) == 0x190);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, is_filled) == 0x198);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity) == 0x1a8);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, water_purity_timer) == 0x1ac);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entities_begin) == 0x1c4);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_entity_ids_begin) == 0x1d0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, salt_water_sparkle_vtable_cache) == 0x1dc);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, fresh_water_sparkle_vtable_cache) == 0x1e0);
const _: () = assert!(std::mem::offset_of!(ZTTankExhibit, sparkle_spawn_timer) == 0x1e4);

impl std::ops::Deref for ZTTankExhibit {
    type Target = ZTHabitat;
    fn deref(&self) -> &ZTHabitat {
        &self.habitat
    }
}
