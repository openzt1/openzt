pub mod zthabitatmgr;
pub use zthabitatmgr::*;

use openzt_detour_macro::detour_mod;
use openzt_detour::generated::{
    zthabitat::*,
    zthabitatmgr::*,
    zthabitat::{
        SAVE as ZTHABITAT_SAVE,
        SET_DIRTY_CHARACTERISTICS as ZTHABITAT_SET_DIRTY_CHARACTERISTICS,
        GET_OUTERMOST_TANK as ZTHABITAT_GET_OUTERMOST_TANK,
    },
    zthabitatmgr::{
        SAVE as ZTHABITATMGR_SAVE,
        CREATE_HABITAT as ZTHABITATMGR_CREATE_HABITAT,
        ADD_HABITAT as ZTHABITATMGR_ADD_HABITAT,
        UPDATE as ZTHABITATMGR_UPDATE,
    },
};
use tracing::info;

use crate::{
    command_console::CommandError,
    globals::globals,
    lua_fn,
    util::{mut_from_memory, ref_from_memory},
};
use super::habitat::ZTHabitat;

fn command_get_zt_habitat_mgr(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_habitat_mgr = globals().zthabitatmgr();
    Ok(format!("{}", zt_habitat_mgr))
}

fn command_get_zt_habitats(_args: Vec<&str>) -> Result<String, CommandError> {
    let zt_habitat_mgr = globals().zthabitatmgr();
    let mut result_string = String::new();
    for i in 0..zt_habitat_mgr.exhibit_array.len() {
        let habitat = zt_habitat_mgr.exhibit_array.get(i);
        let habitat_location = zt_habitat_mgr.exhibit_array.get_ptr(i);
        result_string.push_str(&format!("Habitat {} ({:#x}): ", i, habitat_location));
        result_string.push_str(&format!("  popularity: {},\n", habitat.get_popularity()));
        result_string.push_str(&format!("{}\n", habitat));
    }
    Ok(result_string)
}


#[detour_mod]
pub mod hooks_zthabitatmgr {
    use super::*;
    use openzt_detour::generated::{
        bfentity::VF_RETURN1_1 as IS_RIGHT_SALINITY,
        zthabitat::{
            GET_ATTRACTIVENESS, GET_GATE_TILE_IN, GET_GATE_TILE_OUT, GET_GATE_TILE_PASS_IN, GET_GATE_TILE_PASS_OUT,
            GET_NEAR_CLEAR_TILE, GET_NEAREST_CLEAR_TILE, GET_NEAREST_CLEAR_WATER_TILE,
            GET_POPULARITY, GET_SHOW_INFO_ID, HAS_KEEPER_ASSIGNED, IS_SHOW_STOPPED, LISTEN,
            SET_IS_NOT_SHOW_EXHIBIT, SET_IS_SHOW_EXHIBIT, UPDATE, BLOCK_SERVICE,
            GET_NUM_ADULT_ANIMALS_0, GET_NUM_ADULT_ANIMALS_1,
            RECALCULATE_VIEWING_AREAS, ADD_VIEWING_AREA, REMOVE_VIEWING_AREA, REMOVE_FROM_ALL_VAS, RECREATE_OAS,
            ADD_LAND_TILES, ADD_WATER_TILES, ADD_UNDERWATER_TILES,
            GET_LAND_TILES, GET_WATER_TILES, GET_UNDERWATER_TILES,
            GET_NUM_LAND_TILES, GET_NUM_WATER_TILES, GET_NUM_UNDERWATER_TILES, GET_NUM_KEEPER_FOOD_TILES,
            GET_RANDOM_LAND_TILE, GET_RANDOM_WATER_TILE, GET_RANDOM_UNDERWATER_TILE,
            GET_SMALLEST_KEEPER_FOOD, GET_NEAREST_KEEPER_FOOD, GET_RANDOM_KEEPER_FOOD,
            GET_NEAREST_DIRT_PILE, HAS_PORTAL_ANIMAL, NEEDS_SHOW_KEEPER,
            PATH_PLACED as ZTHABITAT_PATH_PLACED,
            GET_NEEDY_NESTED_TANK as ZTHABITAT_GET_NEEDY_NESTED_TANK,
            MOVE_GATE_TO_1, MOVE_GATE_TO_0,
        },
        zthabitatmgr::{
            DO_TANK_CHECK, ENTER_NEW_MONTH, GET_AVERAGE_HABITAT_ATTRACTIVENESS, GET_HABITAT, GET_NUM_FAMILIES, GET_NUM_SPECIES, HABITAT_TILE_CHANGED,
            HIGHLIGHT_HABITAT, REPLACE_FENCE_WITH_GATE, REPLACE_GATE, REPLACE_GATE_WITH_FENCE, SCENERY_ENTITY_CHANGE, TERRAIN_TILE_CHANGED,
            UNHIGHLIGHT_HABITAT, PATH_PLACED as ZTHABITATMGR_PATH_PLACED, PATH_REMOVED as ZTHABITATMGR_PATH_REMOVED, CHECK_ENTER_HABITAT,
            GET_OUTERMOST_TANK, GET_NEEDY_NESTED_TANK, ENTITY_ABOUT_TO_BE_PLACED, ENTITY_ABOUT_TO_BE_REMOVED, ENTITY_PLACED, ENTITY_REMOVED,
            BEFORE_ENTITY_CHANGE, TERRAIN_ABOUT_TO_BE_CHANGED, TERRAIN_CHANGED, CREATE_DOUBLE_FENCE,
            UPDATE_GATES, FORMAT_HABITAT_MESSAGE, FIND_BEST_PLACE_FOR_GATE, FIND_BETTER_GATES_FOR_NEIGHBORS, CHECK_GATE, GET_NEXT_FENCE_PAIR, PLACE_GATE,
        },
    };

    // 00410349 BFTile * __thiscall OOAnalyzer::ZTHabitat::getGateTileIn(ZTHabitat *this)
    #[detour(GET_GATE_TILE_IN)]
    unsafe extern "thiscall" fn get_gate_tile_in(_this: *const u32) -> *const u32 {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(_this) };
        match habitat.get_gate_tile_in() {
            Some(tile) => globals().ztworldmgr().get_ptr_from_bftile(&tile) as *const u32,
            None => std::ptr::null(),
        }
    }

    #[detour(GET_GATE_TILE_OUT)]
    unsafe extern "thiscall" fn get_gate_tile_out(_this: *const u32) -> i32 {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(_this) };
        match habitat.get_gate_tile_out() {
            Some(tile) => globals().ztworldmgr().get_ptr_from_bftile(&tile) as i32,
            None => 0,
        }
    }

    /// Both gate-pass resolvers compose a gate-tile getter with `getAdjacentClearTile`, whose picked
    /// tile rides through as the return - the `.asm` tails never touch `EAX` and every real caller
    /// consumes it (`ZTGoalKeeperHabitat`/`ZTGoalTrickFood::decide`, `ZTGuest`/`ZTGuide::pickRandomDest`),
    /// so the detours keep `generated.rs`'s honest `BFTile*` return.
    #[detour(GET_GATE_TILE_PASS_IN)]
    unsafe extern "thiscall" fn get_gate_tile_pass_in(this: *const u32, unit: *const u32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_gate_tile_pass_in(unit as u32) as *const u32
    }

    #[detour(GET_GATE_TILE_PASS_OUT)]
    unsafe extern "thiscall" fn get_gate_tile_pass_out(this: *const u32, unit: *const u32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_gate_tile_pass_out(unit as u32) as *const u32
    }

    #[detour(GET_GATE)]
    unsafe extern "thiscall" fn get_gate(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_gate() as i32
    }

    #[detour(GET_ATTRACTIVENESS)]
    unsafe extern "thiscall" fn get_attractiveness(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_attractiveness()
    }

    #[detour(HAS_KEEPER_ASSIGNED)]
    unsafe extern "thiscall" fn has_keeper_assigned(this: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.has_keeper_assigned()
    }

    #[detour(GET_SHOW_INFO_ID)]
    unsafe extern "thiscall" fn get_show_info_id(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_show_info_id() as u32
    }

    #[detour(DO_TANK_CHECK)]
    unsafe extern "stdcall" fn do_tank_check(habitat_ptr: i32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr as u32) }.do_tank_check()
    }

    /// `ZTHabitat::isTank`'s base-slot port (`vtable +0x20` - see
    /// [`ZTHabitat::is_tank_base_default`]), matching the real slot's own signature
    /// (`generated.rs`'s `standalone::VF_RETURN_FALSE`, `thiscall fn(*const c_void) -> u8`) -
    /// implemented but **deliberately left un-hooked**, same precedent as this module's own
    /// `create_edge_pairs`, but for a harder reason than a suspected live bug: the address
    /// *cannot* be detoured at all. The stub is 3 bytes (`XOR AL,AL; RET`), and retour's patcher
    /// needs either 5 inline bytes or a 5-byte all-padding hot-patch area above the function -
    /// both fail here (`0x004016d4..d5` are the first 2 bytes of the next function, whose 6 direct
    /// callers would be corrupted by any inline patch; `0x004016cc..d1` are the live tail of the
    /// previous one), so `GenericDetour::new` returns `NoPatchArea` and the detour macro's own
    /// `LazyLock`-`.unwrap()` would panic the game inside `init_detours`. (`isRightSalinity`'s
    /// shared base `VF_RETURN1_1` only hooks cleanly because it is exactly 5 bytes.) A `#[detour]`
    /// on the shared stub would also fire for the ~150 other vtable slots pointing at it, so the
    /// body must stay a `self`-blind constant regardless. Real `isTank` dispatch is covered by
    /// the battery's `ZTHABITAT_IS_TANK_LIVE` instead.
    #[allow(dead_code)]
    unsafe extern "thiscall" fn is_tank(this: *const std::ffi::c_void) -> u8 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.is_tank_base_default() as u8
    }

    #[detour(IS_SHOW_STOPPED)]
    unsafe extern "fastcall" fn is_show_stopped(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.is_show_stopped() as u32
    }

    #[detour(GET_POPULARITY)]
    unsafe extern "thiscall" fn get_popularity(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_popularity()
    }

    #[detour(GET_HABITAT)]
    unsafe extern "thiscall" fn get_habitat(this: *const u32, pos_x: i32, pos_y: i32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_habitat_ptr(pos_x, pos_y)
    }

    /// `ZTHabitat::isRightSalinity`'s base slot (vtable `+0x28`) is `generated.rs`'s `bfentity::VF_RETURN1_1`,
    /// a constant-`true` stub other vtables share - see [`ZTHabitat::is_right_salinity`].
    #[detour(IS_RIGHT_SALINITY)]
    unsafe extern "thiscall" fn is_right_salinity(this: *const u32, animal_type: u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.is_right_salinity(animal_type as *const u32)
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn listen(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.listen()
    }

    #[detour(SET_IS_SHOW_EXHIBIT)]
    unsafe extern "thiscall" fn set_is_show_exhibit(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_is_show_exhibit()
    }

    #[detour(SET_IS_NOT_SHOW_EXHIBIT)]
    unsafe extern "thiscall" fn set_is_not_show_exhibit(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_is_not_show_exhibit()
    }

    #[detour(VALIDATE_POSITIONS)]
    unsafe extern "thiscall" fn validate_positions(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.validate_positions()
    }

    #[detour(REMOVE_HABITAT_TILES)]
    unsafe extern "thiscall" fn remove_habitat_tiles(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_habitat_tiles()
    }

    #[detour(RESET_UNIT_AI)]
    unsafe extern "thiscall" fn reset_unit_ai(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.reset_unit_ai()
    }

    /// `other_habitat` is unused - see [`ZTHabitat::add_habitat_tiles`]'s own doc comment on why the
    /// real third parameter is always `this` in practice.
    #[detour(ADD_HABITAT_TILES)]
    unsafe extern "thiscall" fn add_habitat_tiles(this: *const u32, seed_tile: *const u32, _other_habitat: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.add_habitat_tiles(seed_tile as u32)
    }

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, elapsed: u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.update(elapsed)
    }

    #[detour(BLOCK_SERVICE)]
    unsafe extern "thiscall" fn block_service(this: *const u32, keeper: *const u32, skip_tank_depth_check: bool, check_tank_and_neighbors: bool) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.block_service(keeper as u32, skip_tank_depth_check, check_tank_and_neighbors)
    }

    /// `generated.rs`'s own entry types `include_neighbors` as plain `bool` - matches real vanilla's own
    /// `TEST AL,AL` byte-only read (`ZTHabitat_getNumAnimals.asm`), no `low_byte_bool` masking needed.
    #[detour(GET_NUM_ANIMALS)]
    unsafe extern "thiscall" fn get_num_animals(this: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_animals(include_neighbors)
    }

    #[detour(GET_NUM_ADULT_ANIMALS_0)]
    unsafe extern "thiscall" fn get_num_adult_animals(this: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_adult_animals(include_neighbors)
    }

    #[detour(GET_NUM_ADULT_ANIMALS_1)]
    unsafe extern "thiscall" fn get_num_adult_animals_by_species(this: *const u32, species_id: i32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_adult_animals_by_species(species_id, include_neighbors)
    }

    #[detour(GET_SPECIES_ANIMALS)]
    unsafe extern "thiscall" fn get_species_animals(this: *const u32, species_id: i32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_species_animals(species_id, out_vector as u32)
    }

    /// `generated.rs`'s own return type is `*const u32` (real vanilla's dead `EAX` = `this` - see
    /// [`ZTHabitat::get_adult_gender_species_animals`]'s own doc comment).
    #[detour(GET_ADULT_GENDER_SPECIES_ANIMALS)]
    unsafe extern "thiscall" fn get_adult_gender_species_animals(this: *const u32, gender_str: *const i8, species_id: i32, out_vector: *const i32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_adult_gender_species_animals(gender_str as u32, species_id, out_vector as u32);
        this
    }

    /// `generated.rs`'s own entry is `fastcall` (`ECX = this`) returning the picked `ZTAnimal*`
    /// (null on an empty habitat).
    #[detour(GET_RANDOM_ANIMAL)]
    unsafe extern "fastcall" fn get_random_animal(this: *const std::ffi::c_void) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_animal()
    }

    /// `generated.rs`'s own entry is `thiscall` returning the picked `BFTile*` (null on an empty
    /// owned-tile list).
    #[detour(GET_RANDOM_TILE)]
    unsafe extern "thiscall" fn get_random_tile(this: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_tile()
    }

    /// `generated.rs`'s own entry is `thiscall` (`BFTile*` from-tile, `EDirection`) returning the
    /// picked `BFTile*` (the `getRandomTile` fallback's result, null on an empty owned-tile list).
    #[detour(GET_RANDOM_TILE_IN_DIRECTION)]
    unsafe extern "thiscall" fn get_random_tile_in_direction(this: *const u32, from_tile: *const u32, direction: u32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_tile_in_direction(from_tile as u32, direction) as *const i32
    }

    /// `generated.rs`'s own entry is `thiscall` (`ZTUnit*`) returning the picked `BFTile*` (null on an
    /// empty owned-tile list).
    #[detour(GET_RANDOM_CLEAR_TILE_AHEAD)]
    unsafe extern "thiscall" fn get_random_clear_tile_ahead(this: *const u32, unit: *const u32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_clear_tile_ahead(unit as u32) as *const i32
    }

    /// `generated.rs`'s own entry takes the out-vector as `i32` and returns nothing.
    #[detour(ADD_CLEAR_TILES)]
    unsafe extern "thiscall" fn add_clear_tiles(this: *const u32, out_vector: i32, animal: *const u32, check_path: bool) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.add_clear_tiles(out_vector as u32, animal as u32, check_path)
    }

    /// The `(check_path, subhabs)` delegator - one `getRandomAnimal` draw forwarded into the
    /// animal-taking overload, returning the picked `BFTile*` (both real callers read it).
    #[detour(GET_RANDOM_CLEAR_TILE_0)]
    unsafe extern "thiscall" fn get_random_clear_tile_default(this: *const u32, check_path: bool, subhabs: bool) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_clear_tile_default(check_path, subhabs) as *const u32
    }

    /// `generated.rs`'s own entry is `thiscall` (nullable `ZTAnimal*`) returning the picked `BFTile*`
    /// (null when the candidate pool comes out empty).
    #[detour(GET_RANDOM_CLEAR_TILE_1)]
    unsafe extern "thiscall" fn get_random_clear_tile_for_animal(this: *const u32, animal: *const u32, check_path: bool, subhabs: bool) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_clear_tile_for_animal(animal as u32, check_path, subhabs)
    }

    #[detour(ADD_BABY_BORN_BONUS)]
    unsafe extern "thiscall" fn add_baby_born_bonus(this: *const u32, species_type: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.add_baby_born_bonus(species_type as u32)
    }

    /// `generated.rs`'s own entry is `stdcall`, no `this` - see [`ZTHabitat::get_adjacent_clear_tile`]'s
    /// own doc comment for why real vanilla needs none here.
    #[detour(GET_ADJACENT_CLEAR_TILE)]
    unsafe extern "stdcall" fn get_adjacent_clear_tile(unit: *const u32, base_tile: *const u32) -> i32 {
        ZTHabitat::get_adjacent_clear_tile(unit as u32, base_tile as u32) as i32
    }

    /// `generated.rs`'s own entry is `thiscall` (`ZTUnit*`) returning the tile-as-int (`0` when
    /// nothing qualifies or the early guards fail).
    #[detour(GET_NEAREST_CLEAR_TILE)]
    unsafe extern "thiscall" fn get_nearest_clear_tile(this: *const u32, unit: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_nearest_clear_tile(unit as u32) as i32
    }

    /// `generated.rs`'s own entry is `thiscall` (`ZTUnit*` mover, nullable `ZTAnimal*`
    /// checkPath source - not a `BFTile*` ref tile, see [`ZTHabitat::get_near_clear_tile`]'s own doc
    /// comment) returning the tile-as-int.
    #[detour(GET_NEAR_CLEAR_TILE)]
    unsafe extern "thiscall" fn get_near_clear_tile(this: *const u32, unit: *const u32, animal: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_near_clear_tile(unit as u32, animal as u32) as i32
    }

    /// `generated.rs`'s own entry is `thiscall` (`BFTile*` reference point - not a `ZTUnit*`, see
    /// [`ZTHabitat::get_nearest_clear_water_tile`]'s own doc comment) returning the tile-as-int
    /// (`0` when the guard fails or nothing qualifies).
    #[detour(GET_NEAREST_CLEAR_WATER_TILE)]
    unsafe extern "thiscall" fn get_nearest_clear_water_tile(this: *const u32, ref_tile: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_nearest_clear_water_tile(ref_tile as u32) as i32
    }

    /// The three biome-classification filters share one `thiscall` out-param shape and the same
    /// `GET_SICKLY_ANIMALS`-style void signature - see [`ZTHabitat::add_land_tiles`]'s own doc comment
    /// for the union-vs-exclusivity note.
    #[detour(ADD_LAND_TILES)]
    unsafe extern "thiscall" fn add_land_tiles(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.add_land_tiles(out_vector as u32)
    }

    #[detour(ADD_WATER_TILES)]
    unsafe extern "thiscall" fn add_water_tiles(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.add_water_tiles(out_vector as u32)
    }

    #[detour(ADD_UNDERWATER_TILES)]
    unsafe extern "thiscall" fn add_underwater_tiles(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.add_underwater_tiles(out_vector as u32)
    }

    /// The three recursive biome-tile aggregators share the add-filters' `thiscall` out-param shape -
    /// see [`ZTHabitat::get_land_tiles`]'s own doc comment.
    #[detour(GET_LAND_TILES)]
    unsafe extern "thiscall" fn get_land_tiles(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_land_tiles(out_vector as u32)
    }

    #[detour(GET_WATER_TILES)]
    unsafe extern "thiscall" fn get_water_tiles(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_water_tiles(out_vector as u32)
    }

    #[detour(GET_UNDERWATER_TILES)]
    unsafe extern "thiscall" fn get_underwater_tiles(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_underwater_tiles(out_vector as u32)
    }

    /// The three biome-tile count getters share one `fastcall` shape (`ECX = this`, bare `RET`, no
    /// stack args) and each is a thin scratch-vector wrapper over the matching aggregator above -
    /// see [`ZTHabitat::get_num_land_tiles`]'s own doc comment. `generated.rs`'s u32-vs-c_void
    /// `this` spread across the three entries is a regeneration wart; each detour matches its own
    /// `FunctionDef`'s declared type verbatim (same precedent as `get_random_animal` above).
    #[detour(GET_NUM_LAND_TILES)]
    unsafe extern "fastcall" fn get_num_land_tiles(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_land_tiles()
    }

    #[detour(GET_NUM_WATER_TILES)]
    unsafe extern "fastcall" fn get_num_water_tiles(this: *const std::ffi::c_void) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_water_tiles()
    }

    #[detour(GET_NUM_UNDERWATER_TILES)]
    unsafe extern "fastcall" fn get_num_underwater_tiles(this: *const std::ffi::c_void) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_underwater_tiles()
    }

    /// The three biome-tile random getters share one `fastcall` shape (`ECX = this`, bare `RET`, no
    /// stack args) and each is a thin scratch-vector + one-LCG-step wrapper over the matching
    /// aggregator above - see [`ZTHabitat::get_random_tiles_aggregating`]. `generated.rs`'s `-> i32`
    /// return is an ABI-identical wart (EAX carries a tile pointer or null), as is the u32-vs-c_void
    /// `this` spread across the three entries; each detour matches its own `FunctionDef`'s declared
    /// type verbatim and casts at the boundary (same precedent as `get_num_land_tiles` above).
    #[detour(GET_RANDOM_LAND_TILE)]
    unsafe extern "fastcall" fn get_random_land_tile(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_land_tile() as i32
    }

    #[detour(GET_RANDOM_WATER_TILE)]
    unsafe extern "fastcall" fn get_random_water_tile(this: *const std::ffi::c_void) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_water_tile() as i32
    }

    #[detour(GET_RANDOM_UNDERWATER_TILE)]
    unsafe extern "fastcall" fn get_random_underwater_tile(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_underwater_tile() as i32
    }

    #[detour(GET_ANIMALS)]
    unsafe extern "thiscall" fn get_animals(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_animals() as i32
    }

    #[detour(GET_AMOUNT_KEEPER_FOOD)]
    unsafe extern "thiscall" fn get_amount_keeper_food(this: *const u32, category: u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_amount_keeper_food(category, include_neighbors)
    }

    #[detour(GET_FOOD_TO_LEAVE)]
    unsafe extern "thiscall" fn get_food_to_leave(this: *const u32, category: u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_food_to_leave(category as i32, include_neighbors)
    }

    #[detour(GET_NUM_KEEPER_FOOD_TILES)]
    unsafe extern "thiscall" fn get_num_keeper_food_tiles(this: *const u32, category: u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_keeper_food_tiles(category)
    }

    /// The keeper-food targeting triplet (`ZTGoalKeeperFood::decide`'s own three heuristics) - thin
    /// pointer-casting wrappers over the ports, `generated.rs`'s `-> i32` return being the established
    /// pointer-as-integer wart (EAX carries the picked food entity or null), same precedent as
    /// [`get_random_land_tile`].
    #[detour(GET_SMALLEST_KEEPER_FOOD)]
    unsafe extern "thiscall" fn get_smallest_keeper_food(this: *const u32, tile: *const u32, category: u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_smallest_keeper_food(tile as u32, category, include_neighbors) as i32
    }

    #[detour(GET_NEAREST_KEEPER_FOOD)]
    unsafe extern "thiscall" fn get_nearest_keeper_food(this: *const u32, tile: *const u32, category: u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_nearest_keeper_food(tile as u32, category, include_neighbors) as i32
    }

    #[detour(GET_RANDOM_KEEPER_FOOD)]
    unsafe extern "thiscall" fn get_random_keeper_food(this: *const u32, tile: *const u32, category: u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_random_keeper_food(tile as u32, category, include_neighbors) as i32
    }

    #[detour(GET_NUM_KEEPERS)]
    unsafe extern "thiscall" fn get_num_keepers(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_keepers()
    }

    #[detour(IS_BEING_SERVICED)]
    unsafe extern "thiscall" fn is_being_serviced(this: *const u32) -> u8 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.is_being_serviced() as u8
    }

    #[detour(SEND_MAINT_WORKER_CLEANUP_EVENTS)]
    unsafe extern "thiscall" fn send_maint_worker_cleanup_events(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.send_maint_worker_cleanup_events()
    }

    #[detour(GET_NUM_HUNGRY_FOODLESS_ANIMALS)]
    unsafe extern "thiscall" fn get_num_hungry_foodless_animals(this: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_hungry_foodless_animals(include_neighbors)
    }

    #[detour(GET_NUM_SICKLY_ANIMALS)]
    unsafe extern "thiscall" fn get_num_sickly_animals(this: *const u32, keeper: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_sickly_animals(keeper as u32, include_neighbors)
    }

    #[detour(GET_NUM_ANGRY_ANIMALS)]
    unsafe extern "thiscall" fn get_num_angry_animals(this: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_angry_animals(include_neighbors)
    }

    #[detour(GET_NUM_SICK_ANIMALS)]
    unsafe extern "thiscall" fn get_num_sick_animals(this: *const u32, include_neighbors: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_num_sick_animals(include_neighbors)
    }

    #[detour(GET_AVG_ANIMAL_HAPPINESS)]
    unsafe extern "thiscall" fn get_avg_animal_happiness(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_avg_animal_happiness()
    }

    #[detour(GET_SICKLY_ANIMALS)]
    unsafe extern "thiscall" fn get_sickly_animals(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_sickly_animals(out_vector as u32)
    }

    #[detour(GET_NEAREST_SICK_ANIMAL)]
    unsafe extern "thiscall" fn get_nearest_sick_animal(this: *const u32, keeper: i32, check_can_see: i8) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_nearest_sick_animal(keeper as u32, check_can_see != 0) as *const i32
    }

    #[detour(GET_NEAREST_DIRT_PILE)]
    unsafe extern "thiscall" fn get_nearest_dirt_pile(this: *const u32, keeper: *const u32, check_can_see: bool) -> i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_nearest_dirt_pile(keeper as u32, check_can_see) as i32
    }

    #[detour(NEEDS_SHOW_KEEPER)]
    unsafe extern "thiscall" fn needs_show_keeper(this: *const u32, keeper: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.needs_show_keeper(keeper as u32) as u32
    }

    #[detour(GET_VIEWING_AREAS_WITH_GUESTS)]
    unsafe extern "thiscall" fn get_viewing_areas_with_guests(this: *const u32, out_vector: *const i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_viewing_areas_with_guests(out_vector as u32)
    }

    #[detour(HAS_BLDG)]
    unsafe extern "thiscall" fn has_bldg(this: *const u32, entity: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.has_bldg(entity as u32)
    }

    #[detour(REMOVE_VIEWING_AREAS)]
    unsafe extern "thiscall" fn remove_viewing_areas(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_viewing_areas()
    }

    /// Real vanilla returns `&this->field_0x6c` itself (`.asm`-confirmed `LEA EAX,[ESI+0x6c]`) rather
    /// than the vector's own begin pointer - real callers dereference the result once more to reach the
    /// actual `ZTAnimal**` (see [`ZTHabitat::get_all_animals`]'s own doc comment). Reproduced here via
    /// [`Self`]'s own field offset rather than a stored constant, since it must track the struct layout.
    #[detour(GET_ALL_ANIMALS)]
    unsafe extern "thiscall" fn get_all_animals(this: *const u32, sort: i8) -> *const i32 {
        let _ = unsafe { ref_from_memory::<ZTHabitat>(this) }.get_all_animals(sort != 0);
        (this as u32 + 0x6c) as *const i32
    }

    #[detour(GET_SURROUNDING_SPECIES)]
    unsafe extern "thiscall" fn get_surrounding_species(this: *const u32) -> *const i32 {
        let _ = unsafe { ref_from_memory::<ZTHabitat>(this) }.surrounding_species();
        (this as u32 + 0x13c) as *const i32
    }

    #[detour(REMOVE_SPECIES)]
    unsafe extern "thiscall" fn remove_species(this: *const u32, species_key: i32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_species(species_key as u32)
    }

    #[detour(REMOVE_FOOD_TARGET)]
    unsafe extern "stdcall" fn remove_food_target(animal: *const std::ffi::c_void) -> u32 {
        ZTHabitat::remove_food_target(animal as u32) as u32
    }

    #[detour(REMOVE_FOOD_TARGET_FOR_ALL)]
    unsafe extern "thiscall" fn remove_food_target_for_all(this: *const u32, food_entity: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.remove_food_target_for_all(food_entity as u32) as u32
    }

    /// `generated.rs`'s own `-> u32` return is real vanilla's undefined-upper-bytes bool render
    /// (`MOV AL,1` / `XOR AL,AL`; the C packs garbage into the upper 3 bytes) - real callers
    /// `TEST AL,AL` only, so the detour widens the port's clean bool to match the declared
    /// signature (see [`ZTHabitat::has_portal_animal`]'s own doc comment).
    #[detour(HAS_PORTAL_ANIMAL)]
    unsafe extern "thiscall" fn has_portal_animal(this: *const u32, target_habitat: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.has_portal_animal(target_habitat as u32) as u32
    }

    #[detour(ZTHABITAT_SET_DIRTY_CHARACTERISTICS)]
    unsafe extern "thiscall" fn set_dirty_characteristics(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_dirty_characteristics()
    }

    #[detour(ACCEPT_DONATION)]
    unsafe extern "thiscall" fn accept_donation(this: *const u32, amount: f32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.accept_donation(amount)
    }

    #[detour(SET_TIME_LAST_SERVICED)]
    unsafe extern "thiscall" fn set_time_last_serviced(this: *const u32, time: u32, propagate: bool) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.set_time_last_serviced(time, propagate)
    }

    #[detour(TRIGGER_KEEPER_ARRIVED)]
    unsafe extern "thiscall" fn trigger_keeper_arrived(this: *const u32, keeper: *const u32, scheduled: bool) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.trigger_keeper_arrived(keeper as u32, scheduled)
    }

    /// Detoured (byte-for-byte reproducing real vanilla's own call graph adds no new risk over baseline),
    /// but deliberately **not** exercised by an active live test - see
    /// [`ZTHabitat::trigger_death_arrived`]'s own doc comment: its `species_key == 0` branch marks every
    /// animal in the habitat with an unidentified "death arrived" flag, and this pass found no reader for
    /// that flag anywhere in the decompile corpus, so the real consequence of setting it on a live zoo's
    /// real animals is unconfirmed - same "no known safe way to exercise this live" reasoning already
    /// applied to [`ZTHabitatMgr::clear_staff_habitat`]/[`ZTHabitatMgr::can_see_habitat_from_building`].
    #[detour(TRIGGER_DEATH_ARRIVED)]
    unsafe extern "thiscall" fn trigger_death_arrived(this: *const u32, species_key: i32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.trigger_death_arrived(species_key)
    }

    /// `generated.rs`'s own entry types the `file` parameter as `*const u32` rather than `*const i8` -
    /// an ABI-identical wart (both are 32-bit pointers under `thiscall`), cast at the call site rather
    /// than treated as a real signature difference (per `CLAUDE.md`'s own note on this generator
    /// quirk - never hand-edit `generated.rs` itself to "fix" it).
    #[detour(ZTHABITAT_SAVE)]
    unsafe extern "thiscall" fn zthabitat_save(this: *const u32, file: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.save(file as *const i8)
    }

    #[detour(ZTHABITATMGR_SAVE)]
    unsafe extern "thiscall" fn zthabitatmgr_save(this: *const u32, file: *const i8) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.save(file)
    }

    #[detour(ZTHABITATMGR_ADD_HABITAT)]
    unsafe extern "thiscall" fn zthabitatmgr_add_habitat(this: *const u32, habitat_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.add_habitat(habitat_ptr as u32)
    }

    #[detour(ZTHABITATMGR_CREATE_HABITAT)]
    unsafe extern "thiscall" fn zthabitatmgr_create_habitat(this: *const u32, seed_tile: *const u8, resize_tile: *const u32, gate_tile: u32, name_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.create_habitat(seed_tile as u32, resize_tile as u32, gate_tile, name_ptr as u32)
    }

    #[detour(GET_ZOO_ENTRANCE_TILE)]
    unsafe extern "thiscall" fn get_zoo_entrance_tile(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_zoo_entrance_tile_ptr() as i32
    }

    #[detour(GET_AVERAGE_HABITAT_ATTRACTIVENESS)]
    unsafe extern "fastcall" fn get_average_habitat_attractiveness(this: i32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this as u32) }.get_average_habitat_attractiveness()
    }

    #[detour(GET_NUM_FAMILIES)]
    unsafe extern "fastcall" fn get_num_families(this: i32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this as u32) }.get_num_families()
    }

    #[detour(GET_NUM_SPECIES)]
    unsafe extern "fastcall" fn get_num_species(this: *const u32) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_num_species()
    }

    /// Real vanilla is a free `stdcall` helper taking a `ZTHabitat*` directly, not a `ZTHabitatMgr`
    /// method - see `ZTHabitat::highlight`'s own doc comment. Null-checked here rather than inside
    /// `highlight` itself, matching real vanilla's own `if (param_1 != NULL)` guard at the call boundary.
    #[detour(HIGHLIGHT_HABITAT)]
    unsafe extern "stdcall" fn highlight_habitat(habitat: *const u32, hilite: bool) {
        if habitat.is_null() {
            return;
        }
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.highlight(hilite)
    }

    /// See [`highlight_habitat`]'s own doc comment - same free-function/null-guard shape, real vanilla's
    /// `ZTHabitatMgr::unhighlightHabitat`.
    #[detour(UNHIGHLIGHT_HABITAT)]
    unsafe extern "stdcall" fn unhighlight_habitat(habitat: *const u32) {
        if habitat.is_null() {
            return;
        }
        unsafe { ref_from_memory::<ZTHabitat>(habitat) }.unhighlight()
    }

    #[detour(ENTER_NEW_MONTH)]
    unsafe extern "thiscall" fn enter_new_month(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.enter_new_month()
    }

    /// Both known correctness bugs are fixed (see `ZTHabitat::move_gate_to`/`move_gate_to_inner`'s own
    /// doc comments). Enabled for manual/interactive live verification - moving a gate mutates real
    /// habitat/fence state with no known synthetic-safe candidate, so this has no automated live test.
    #[detour(MOVE_GATE_TO_1)]
    unsafe extern "thiscall" fn move_gate_to(this: *const u32, tile_ptr: *const u32, direction_raw: u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.move_gate_to(tile_ptr as u32, direction_raw)
    }

    /// `ZTHabitatMgr::place_gate` is fully ported (see its own doc comment, resolving the plan's own
    /// longest-standing "corrupted, needs `.asm`-level work" blocker via a macOS decompile cross-check),
    /// but converts a fence into a gate and moves it, and briefly creates/destroys a live keeper unit -
    /// same "no synthetic-safe input" class as `move_gate_to`/`create_double_fence` above. Enabled for
    /// manual/interactive live verification, not covered by an automated live test.
    #[detour(PLACE_GATE)]
    unsafe extern "thiscall" fn place_gate(
        this: *const u32,
        habitat_ptr: *const u32,
        seed_tile_ptr: *const u32,
        resize_tile_ptr: *const u32,
        gate_hint_tile_ptr: *const u32,
    ) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.place_gate(habitat_ptr as u32, seed_tile_ptr as u32, resize_tile_ptr as u32, gate_hint_tile_ptr as u32)
    }

    /// `ZTHabitatMgr::create_double_fence` is fully ported (see its own doc comment) but calls
    /// `ZTMapView::addUndoAction` directly, making it a live-UI function in its own right regardless of
    /// its caller ([`ZTHabitatMgr::snap_tank_walls_inward`]) - same "unsafe for automated testing" class
    /// as `nameHabitat`/`morphExhibit`. Enabled for manual/interactive live verification.
    #[detour(CREATE_DOUBLE_FENCE)]
    unsafe extern "stdcall" fn create_double_fence(fence_ptr: i32) -> *const i32 {
        ZTHabitatMgr::create_double_fence(fence_ptr as u32) as *const i32
    }

    /// `ZTHabitatMgr::snap_tank_walls_inward` is fully ported (see its own doc comment) but can swap real
    /// fence positions, spawn a double fence, and post a live map-editor undo action on its own -
    /// same "unsafe for automated testing" class as `create_double_fence`/`fence_placed`. Enabled for
    /// manual/interactive live verification, not covered by an automated live test.
    #[detour(SNAP_TANK_WALLS_INWARD)]
    unsafe extern "thiscall" fn snap_tank_walls_inward(this: *const u32, tank_ptr: *const std::ffi::c_void) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.snap_tank_walls_inward(tank_ptr as u32);
        true
    }

    /// `ZTHabitatMgr::morph_exhibit` is fully ported (see its own doc comment) but creates a real undo
    /// action and destroys/recreates real habitat state on its main path - same "unsafe for automated
    /// testing" class as `move_gate_to`/`create_double_fence`/`place_gate` above. Enabled for
    /// manual/interactive live verification, not covered by an automated live test.
    #[detour(MORPH_EXHIBIT)]
    unsafe extern "thiscall" fn morph_exhibit(this: *const u32, habitat_ptr: *const u32, tile_2_ptr: *const u32, tile_3_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.morph_exhibit(habitat_ptr as u32, tile_2_ptr as u32, tile_3_ptr as u32)
    }

    /// Real vanilla is a plain free `stdcall` helper (`generated.rs`'s own signature has no `this`) -
    /// see [`crate::zthabitatmgr::ZTHabitatMgr::replace_gate_with_fence`]'s own doc comment.
    #[detour(REPLACE_GATE_WITH_FENCE)]
    unsafe extern "stdcall" fn replace_gate_with_fence(fence: *const i32) -> bool {
        ZTHabitatMgr::replace_gate_with_fence(fence as u32)
    }

    #[detour(REPLACE_FENCE_WITH_GATE)]
    unsafe extern "thiscall" fn replace_fence_with_gate(this: *const u32, fence: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.replace_fence_with_gate(fence as u32) as u32
    }

    #[detour(REPLACE_GATE)]
    unsafe extern "thiscall" fn replace_gate(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.replace_gate()
    }

    #[detour(HABITAT_TILE_CHANGED)]
    unsafe extern "thiscall" fn habitat_tile_changed(this: *const u32, tile: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.habitat_tile_changed(tile as u32)
    }

    #[detour(TERRAIN_TILE_CHANGED)]
    unsafe extern "thiscall" fn terrain_tile_changed(this: *const u32, tile: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.terrain_tile_changed(tile as u32)
    }

    #[detour(SCENERY_ENTITY_CHANGE)]
    unsafe extern "thiscall" fn scenery_entity_change(this: *const u32, tile: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.scenery_entity_change(tile as u32)
    }

    #[detour(ENTITY_ABOUT_TO_BE_PLACED)]
    unsafe extern "thiscall" fn entity_about_to_be_placed(this: *const u32, entity: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_about_to_be_placed(entity as u32)
    }

    #[detour(ENTITY_PLACED)]
    unsafe extern "thiscall" fn entity_placed(this: *const u32, entity: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_placed(entity as u32)
    }

    #[detour(ENTITY_ABOUT_TO_BE_REMOVED)]
    unsafe extern "thiscall" fn entity_about_to_be_removed(this: *const u32, entity: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_about_to_be_removed(entity as u32)
    }

    #[detour(ENTITY_REMOVED)]
    unsafe extern "thiscall" fn entity_removed(this: *const u32, tile: *const u32, entity_type: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.entity_removed(tile as u32, entity_type as u32)
    }

    /// Real vanilla is a plain free `stdcall` helper (`generated.rs`'s own signature has no `this`) -
    /// see [`crate::zthabitatmgr::ZTHabitatMgr::before_entity_change`]'s own doc comment.
    #[detour(BEFORE_ENTITY_CHANGE)]
    unsafe extern "stdcall" fn before_entity_change(habitat_ptr: *const i32) {
        ZTHabitatMgr::before_entity_change(habitat_ptr as u32)
    }

    #[detour(TERRAIN_ABOUT_TO_BE_CHANGED)]
    unsafe extern "thiscall" fn terrain_about_to_be_changed(this: *const u32, x: i32, y: i32, size: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.terrain_about_to_be_changed(x, y, size)
    }

    /// `generated.rs`'s own `TERRAIN_CHANGED` entry now correctly reflects real vanilla's own `RET 0xc` -
    /// `doDepthSuitability`'s own `.asm` pushes the identical `(x, y, size)` triple it just passed to
    /// `terrainAboutToBeChanged` moments earlier. None of the three are read anywhere in `terrainChanged`'s
    /// own decompiled body once the corrected prototype is applied (Ghidra confirms they're genuinely
    /// dead - popped for stack balance, never used) - [`ZTHabitatMgr::terrain_changed`] itself takes none,
    /// so they're accepted here and discarded, matching real vanilla's own behavior exactly rather than
    /// guessing at a use for them.
    #[detour(TERRAIN_CHANGED)]
    unsafe extern "thiscall" fn terrain_changed(this: *const u32, _x: i32, _y: i32, _size: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.terrain_changed()
    }

    #[detour(HILITE_AMPHIBIOUS_NEIGHBORS)]
    unsafe extern "thiscall" fn hilite_amphibious_neighbors(this: *const u32, hilite: i8) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.hilite_amphibious_neighbors(hilite != 0)
    }

    #[detour(HILITE_SHOW_NEIGHBORS)]
    unsafe extern "thiscall" fn hilite_show_neighbors(this: *const u32, hilite: i8) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.hilite_show_neighbors(hilite != 0)
    }

    #[detour(CHECK_AMPHIBIOUS_NEIGHBOR)]
    unsafe extern "thiscall" fn check_amphibious_neighbor(this: *const u32, habitat_a: *const u32, tile_a: *const u32, tile_b: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.check_amphibious_neighbor(habitat_a as u32, tile_a as u32, tile_b as u32) as u32
    }

    #[detour(UPDATE_AMPHIBIOUS_NEIGHBORS_1)]
    unsafe extern "thiscall" fn update_amphibious_neighbors_1(this: *const u32, habitat: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_amphibious_neighbors(habitat as u32)
    }

    #[detour(UPDATE_AMPHIBIOUS_NEIGHBORS_0)]
    unsafe extern "thiscall" fn update_amphibious_neighbors_0(this: *const u32, tile: i32, direction: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_amphibious_neighbors_from_tile(tile as u32, direction)
    }

    #[detour(CHECK_SHOW_NEIGHBOR)]
    unsafe extern "thiscall" fn check_show_neighbor(this: *const u32, habitat_a: *const u32, tile_a: *const u32, tile_b: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.check_show_neighbor(habitat_a as u32, tile_a as u32, tile_b as u32) as u32
    }

    #[detour(UPDATE_SHOW_NEIGHBORS_1)]
    unsafe extern "thiscall" fn update_show_neighbors_1(this: *const u32, habitat: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_show_neighbors(habitat as u32)
    }

    #[detour(UPDATE_SHOW_NEIGHBORS_0)]
    unsafe extern "thiscall" fn update_show_neighbors_0(this: *const u32, tile: i32, direction: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_show_neighbors_from_tile(tile as u32, direction)
    }

    #[detour(DO_SHOW_CHECK)]
    unsafe extern "thiscall" fn do_show_check(this: *const u32, habitat: *const i32, remove_illegal: i8) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.do_show_check(habitat as u32, remove_illegal != 0)
    }

    #[detour(CAN_SEE_SHOW_FROM_BUILDING)]
    unsafe extern "thiscall" fn can_see_show_from_building(this: *const u32, building: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.can_see_show_from_building(building as u32)
    }

    #[detour(HABITAT_SEEN_FROM_BUILDING)]
    unsafe extern "thiscall" fn habitat_seen_from_building(this: *const u32, building: i32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.habitat_seen_from_building(building as u32) as *const i32
    }

    #[detour(CAN_FIND_PATH)]
    unsafe extern "thiscall" fn can_find_path(this: *const u32, tile_a: *const u32, tile_b: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.can_find_path(tile_a as u32, tile_b as u32)
    }

    #[detour(CLEAR_PATHFINDING)]
    unsafe extern "thiscall" fn clear_pathfinding(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.clear_pathfinding()
    }

    /// Real vanilla is a plain free `stdcall` helper (no `this`) - see
    /// [`ZTHabitatMgr::clear_staff_habitat`]'s own doc comment.
    #[detour(CLEAR_STAFF_HABITAT)]
    unsafe extern "stdcall" fn clear_staff_habitat(habitat: *const u32) {
        ZTHabitatMgr::clear_staff_habitat(habitat as u32)
    }

    #[detour(GET_TANK)]
    unsafe extern "thiscall" fn get_tank(this: *const u32, tile: *const u32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_tank(tile as u32) as *const i32
    }

    #[detour(ZTHABITAT_GET_OUTERMOST_TANK)]
    unsafe extern "thiscall" fn get_outermost_tank(this: *const u32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_outermost_tank() as *const u32
    }

    #[detour(GET_OUTERMOST_TANK)]
    unsafe extern "thiscall" fn zthabitatmgr_get_outermost_tank(this: *const u32, habitat: *const u32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_outermost_tank(habitat as u32) as *const u32
    }

    #[detour(ZTHABITAT_GET_NEEDY_NESTED_TANK)]
    unsafe extern "thiscall" fn get_needy_nested_tank(this: *const u32, keeper: *const u32) -> *const i32 {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.get_needy_nested_tank(keeper as u32) as *const i32
    }

    #[detour(GET_NEEDY_NESTED_TANK)]
    unsafe extern "thiscall" fn zthabitatmgr_get_needy_nested_tank(this: *const u32, habitat: *const std::ffi::c_void, keeper: *const i32) -> *const u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_needy_nested_tank(habitat as u32, keeper as u32) as *const u32
    }

    #[detour(LEADS_TO)]
    unsafe extern "thiscall" fn leads_to(this: *const u32, habitat_a: *const u32, habitat_b: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.leads_to(habitat_a as u32, habitat_b as u32) as u32
    }

    /// Real vanilla is a plain free `stdcall` helper (no `this`) - see
    /// [`ZTHabitatMgr::break_amphibious_connection`]'s own doc comment.
    #[detour(BREAK_AMPHIBIOUS_CONNECTION)]
    unsafe extern "stdcall" fn break_amphibious_connection(tile_a: *const u32, tile_b: i32) {
        ZTHabitatMgr::break_amphibious_connection(tile_a as u32, tile_b as u32)
    }

    #[detour(FENCE_REPLACED)]
    unsafe extern "thiscall" fn fence_replaced(this: *const u32, tile: *const u32, direction: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.fence_replaced(tile as u32, direction)
    }

    /// `ZTHabitatMgr::fence_placed` is fully ported (see its own doc comment), but on its main path
    /// mutates real habitat/fence state and can destroy/recreate a habitat - same "no synthetic-safe
    /// input" class as `place_gate`/`create_habitat` above. Enabled for manual/interactive live
    /// verification, not covered by an automated live test.
    #[detour(FENCE_PLACED)]
    unsafe extern "thiscall" fn fence_placed(this: *const u32, tile: *const u32, direction: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.fence_placed(tile as u32, direction)
    }

    /// `ZTHabitatMgr::fence_removed` is fully ported (see its own doc comment), but on its main path
    /// mutates real habitat/fence state and can destroy/merge habitats - same "no synthetic-safe input"
    /// class as `fence_placed`/`place_gate`/`create_habitat` above. Enabled for manual/interactive live
    /// verification, not covered by an automated live test. `direction` is declared `*const i32` in
    /// `generated.rs` (a real vanilla ABI wart - the C++ source passes `EDirection` through a parameter
    /// slot OOAnalyzer typed as a pointer; ABI-identical to a plain `u32`, cast at this call site).
    #[detour(FENCE_REMOVED)]
    unsafe extern "thiscall" fn fence_removed(this: *const u32, tile: *const u32, direction: *const i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.fence_removed(tile as u32, direction as u32)
    }

    /// Real vanilla is a plain free `stdcall` helper (no `this`) - see
    /// [`ZTHabitatMgr::check_enter_habitat`]'s own doc comment.
    #[detour(CHECK_ENTER_HABITAT)]
    unsafe extern "stdcall" fn check_enter_habitat(habitat: *const u32, unit: *const u32) -> u32 {
        ZTHabitatMgr::check_enter_habitat(habitat as u32, unit as u32)
    }

    #[detour(RECALCULATE_DETERIORATION)]
    unsafe extern "thiscall" fn recalculate_deterioration(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitatMgr>(this) }.recalculate_deterioration()
    }

    #[detour(FILL_ZOO_EXTERIOR)]
    unsafe extern "thiscall" fn fill_zoo_exterior(this: *const u32, tile: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.fill_zoo_exterior(tile as u32)
    }

    #[detour(MARK_ZOO_EXTERIOR)]
    unsafe extern "thiscall" fn mark_zoo_exterior(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.mark_zoo_exterior()
    }

    #[detour(ZTHABITATMGR_UPDATE)]
    unsafe extern "thiscall" fn zthabitatmgr_update(this: *const u32, elapsed: u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update(elapsed)
    }

    #[detour(CAN_SEE_HABITAT_FROM_BUILDING)]
    unsafe extern "cdecl" fn can_see_habitat_from_building(habitat_ptr: u32, building_ptr: i32) -> u32 {
        unsafe { ZTHabitatMgr::can_see_habitat_from_building(habitat_ptr, building_ptr as u32) }
    }

    /// Implemented and confirmed correct against real vanilla via a direct call
    /// (`ZTHABITAT_CREATE_EDGE_PAIRS_MATCHES_REAL_LIVE`), but **deliberately left un-hooked** - same
    /// "implemented but detour disabled" precedent this file's own `ZTHabitatMgr::add_habitat`/
    /// `create_habitat` already established. Bisected directly: with every other stage-6h detour enabled,
    /// hooking this one specifically hangs `run_load_live_zoo` dead before it can even log (real, un-ported
    /// `ZTHabitat::resize` calls through to this address during real vanilla's own habitat reconstruction
    /// path); disabling only this one detour (all 8 others enabled) lets the full battery pass
    /// (205/205). Not yet root-caused *why* the hook alone misbehaves when the direct call doesn't -
    /// candidates worth checking first: whether the real vanilla-reconstructed habitat this gets called on
    /// mid-load has an already-consistent `boundary_tile_pairs_begin`/`_end`/`_cap_end` triple at that
    /// exact point, or whether `PoolAlloc::allocate`/`deallocate`'s own real behavior during that specific
    /// load phase differs from a live, already-loaded zoo's steady state.
    #[allow(dead_code)]
    unsafe extern "thiscall" fn create_edge_pairs(this: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.create_edge_pairs()
    }

    #[detour(RECALCULATE_VIEWING_AREAS)]
    unsafe extern "thiscall" fn recalculate_viewing_areas(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.recalculate_viewing_areas()
    }

    #[detour(ADD_VIEWING_AREA)]
    unsafe extern "thiscall" fn add_viewing_area(this: *const u32, va_ptr: u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.add_viewing_area(va_ptr)
    }

    #[detour(REMOVE_VIEWING_AREA)]
    unsafe extern "thiscall" fn remove_viewing_area(this: *const u32, va_ptr: *const u32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_viewing_area(va_ptr as u32)
    }

    #[detour(REMOVE_FROM_ALL_VAS)]
    unsafe extern "thiscall" fn remove_from_all_vas(this: *const u32, tile_ptr: i32) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.remove_from_all_vas(tile_ptr as u32)
    }

    #[detour(RECREATE_OAS)]
    unsafe extern "thiscall" fn recreate_oas(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.recreate_oas()
    }

    #[detour(ZTHABITAT_PATH_PLACED)]
    unsafe extern "thiscall" fn zthabitat_path_placed(this: *const u32, tile_ptr: *const std::ffi::c_void) {
        unsafe { mut_from_memory::<ZTHabitat>(this) }.path_placed(tile_ptr as u32)
    }

    #[detour(ZTHABITATMGR_PATH_PLACED)]
    unsafe extern "thiscall" fn zthabitatmgr_path_placed(this: *const u32, tile_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.path_placed(tile_ptr as u32)
    }

    #[detour(ZTHABITATMGR_PATH_REMOVED)]
    unsafe extern "thiscall" fn zthabitatmgr_path_removed(this: *const u32, tile_ptr: i32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.path_removed(tile_ptr as u32)
    }

    #[detour(MOVE_GATE_TO_0)]
    unsafe extern "thiscall" fn move_gate_to_0(this: *const u32, fence_ptr: *const u32) -> bool {
        unsafe { ref_from_memory::<ZTHabitat>(this) }.move_gate_to_fence(fence_ptr as u32)
    }

    #[detour(UPDATE_GATES)]
    unsafe extern "thiscall" fn update_gates(this: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.update_gates()
    }

    #[detour(FORMAT_HABITAT_MESSAGE)]
    unsafe extern "stdcall" fn format_habitat_message(out_vec: *const u32, string_id: u32, habitat_ptr: i32) -> *const u32 {
        ZTHabitatMgr::format_habitat_message(out_vec as *mut u32, string_id, habitat_ptr as u32) as *const u32
    }

    #[detour(FIND_BEST_PLACE_FOR_GATE)]
    unsafe extern "thiscall" fn find_best_place_for_gate(this: *const u32, habitat_ptr: *const u32) -> u32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.find_best_place_for_gate(habitat_ptr as u32) as u32
    }

    #[detour(FIND_BETTER_GATES_FOR_NEIGHBORS)]
    unsafe extern "thiscall" fn find_better_gates_for_neighbors(this: *const u32, tank_ptr: *const u32) {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.find_better_gates_for_neighbors(tank_ptr as u32)
    }

    #[detour(CHECK_GATE)]
    unsafe extern "thiscall" fn check_gate(
        this: *const u32,
        tile_a: *const u32,
        tile_b: *const u32,
        keeper: *const u32,
        out_cost: *const i32,
        flag: bool,
    ) -> i32 {
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.check_gate(
            tile_a as u32,
            tile_b as u32,
            keeper as u32,
            out_cost as *mut i32,
            flag,
        )
    }

    #[detour(GET_NEXT_FENCE_PAIR)]
    unsafe extern "thiscall" fn get_next_fence_pair(
        this: *const u32,
        habitat_ptr: *const u32,
        cand_a: u32,
        cand_b: u32,
        check_in_zoo: bool,
    ) -> u32 {
        let cand_a_ref = unsafe { &mut *(cand_a as *mut u32) };
        let cand_b_ref = unsafe { &mut *(cand_b as *mut u32) };
        unsafe { ref_from_memory::<ZTHabitatMgr>(this) }.get_next_fence_pair(
            habitat_ptr as u32,
            cand_a_ref,
            cand_b_ref,
            check_in_zoo,
        ) as u32
    }
}



pub fn register_lua_commands() {
    // get_zthabitatmgr() - no args
    lua_fn!("get_zthabitatmgr", "Returns ZTHabitatMgr debug info", "get_zthabitatmgr()", || {
        match command_get_zt_habitat_mgr(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // list_exhibits() - no args
    lua_fn!("list_exhibits", "Lists all zoo exhibits/habitats", "list_exhibits()", || {
        match command_get_zt_habitats(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });
}

pub fn init() {
    register_lua_commands();

    if let Err(e) = unsafe { hooks_zthabitatmgr::init_detours() } {
        info!("Error initialising zthabitatmgr detours: {}", e);
    }
}
