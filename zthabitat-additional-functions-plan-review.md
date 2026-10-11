Correctness, most important first

1. The re-entry audit can't see this module any more. The script only flags a detour and a .original() call on the same address if both are in the same file. After the split, every #[detour] lives in mgr/mod.rs and every .original() call lives in habitat.rs, support.rs or zthabitatmgr.rs. I wrote a crate-wide check that compares real addresses from generated.rs:
   - Nothing calls .original() on its own detoured address, so there's no recursion today.
   - 10 calls use .original() on an address that another module detours. Debug builds reach vanilla through the hook registry, but release builds reach our Rust port, so the two profiles run different code:
     - zthabitatmgr.rs:1573 (GET_GATE_TILE_IN)
     - habitat.rs:3673 (AMBIENTS_PLAY)
     - habitat.rs:3776 (ZTSHOWINFO_SAVE)
     - zoostatus.rs:408 (LOAD_STRING, which means string-registry behaviour differs between debug and release), plus lines 433, 549, 1459 and 2115
     - ztshow/info/lifecycle.rs:415 and pending_scripts.rs:199 (GET_DATE)
   - Fix: call the port directly (or .hooked()), and make the audit compare addresses. My script is in the scratchpad and could replace the per-file grep.
2. Methods take &self on game memory that the vanilla calls inside them rewrite. For example, get_attractiveness calls RECALCULATE_CHARACTERISTICS.original() and then reads self.attractiveness.
   - Rust tells the optimizer that memory behind a &ZTHabitat never changes during the call, so reading a field after a vanilla call rewrote it is undefined behaviour. It happens to compile correctly today only because no field is read both before and after such a call.
   - [profile.dev] is opt-level = 3, so both the debug and release batteries are exposed.
   - Related cases: writing through mut_from_memory while &self is live (get_needy_nested_tank, get_outermost_tank), and nested &mut to the same habitat when set_dirty_characteristics recurses through a neighbour cycle.
   - Cheap fix: add a zero-sized UnsafeCell<()> field to ZTHabitat, ZTTankExhibit and ZTHabitatMgr. The size stays the same and the "never changes" assumption goes away. Use raw pointers on the recursive &mut paths.
3. A panic inside a detour aborts the game. Dev builds have overflow checks on.
   - additional_scenery_suitability_change: (val1 + val2) * 100 / divisor panics on overflow or a zero divisor, and the += running sums can overflow too. Vanilla wraps instead.
   - get_amount_keeper_food: keeper_food_category_amounts[category as usize] is bounds-checked, although the doc comment says "no validation, matching vanilla".
4. The species-rating cache lock is inconsistent. terrain_changed scopes SPECIES_RATING_CACHE's lock "to avoid re-entrant deadlock", but inside that scope it still calls recalculateCharacteristics and generateFaces. terrain_about_to_be_changed (zthabitatmgr.rs:2580) holds the lock across reset_unit_ai, surrounding_species() and get_species_rating. Take a snapshot and release the lock first. A RefCell would also do, since everything runs on the game thread.
5. The gate-tile lookup is expensive. get_gate_tile_in/_out copy the whole 0x178-byte habitat and then compare it with a PartialEq that allocates two Strings for the exhibit names. Vanilla just compares pointers, and this runs for every guest decision via get_gate_tile_pass_out. Comparing get_habitat_ptr(x, y) == self_ptr and returning the tile pointer would fix it.
6. The disabled create_edge_pairs detour still has no root cause. Porting resize (33 lines) would give a clean place to find it: resize is removeHabitatTiles, addHabitatTiles, createEdgePairs, createViewingAreas, recalculateViewingAreas, and a removeHabitat if the habitat ends up empty. All of those are ported except createViewingAreas and removeHabitat.
7. Stale docs and names:
   - Several comments still say "still un-ported" for functions that are now detoured: updateGates, checkGate/getNextFencePair, recreateOAs, pathRemoved, fencePlaced/fenceRemoved and resetUnitAI (e.g. support.rs:111, support.rs:351, habitat.rs:61, zthabitatmgr.rs:72, zthabitatmgr.rs:3791).
   - habitat.rs:453 says the old gate comes from GET_GATE.original(), but the code calls self.get_gate().
   - The doc comment at zthabitatmgr.rs:1468–1471 has fragments of a deleted wrapper's comment stuck to the front of update_gates' doc.
   - ZTHabitatMgr+0x68 (loaded_marker) is the habitat-number counter: decrementHabitatNum decrements it. ZTHabitat+0x134 is the deterioration level written by setDeterioration, and is still padding in the struct.
   - init() logs a detour failure with info! instead of error!.

Test coverage

- There are 100 habitat live tests, 35 of which are named smoke tests. Some of those are stronger than the name suggests (GET_RANDOM_ANIMAL_SMOKE checks the exact random draw).
- No live test, by design: the gate and fence mutators (move_gate_to*, place_gate, fence_placed/removed/replaced, create_double_fence, snap_tank_walls_inward, morph_exhibit, replace_*_with_*, find_best_place_for_gate), plus trigger_death_arrived and clear_staff_habitat.
- No live test, but look testable: path_placed (both versions), path_removed, remove_from_all_vas, recalculate_viewing_areas, send_maint_worker_cleanup_events, check_gate, get_next_fence_pair, habitat_seen_from_building, and the tile/direction versions of update_*_neighbors.
- Smoke tests worth upgrading to vanilla comparisons: check_amphibious_neighbor/check_show_neighbor (their return values only depend on current state), recalculate_deterioration (compare +0x134 per habitat), get_outermost_tank/get_needy_nested_tank, and update_*_neighbors (compare neighbour-set contents).
- support.rs has no unit tests. The leaked-fixture style already used for has_portal_animal would cover lcg_next, walk_neighbor_tree order, walk_tile_list, get_habitat_cell_addr bounds and rotate_cardinal_direction.

Small related functions still called through vanilla

┌──────────────────────────────────────────┬────────────────┬────────────────────────────────────────────────────────┐
│                 Function                 │ Decompile size │                          Why                           │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTHabitat::getEvents                     │ 8 lines        │ Would make listen fully Rust                           │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTHabitatMgr::decrementHabitatNum        │ 13             │ 4 call sites in create_habitat; confirms what +0x68 is │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTTankExhibit::isRightSalinity           │ 19             │ The base-class version is already detoured             │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTHabitat::setDeterioration              │ 20             │ 3 call sites; names +0x134                             │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTHabitat::resize                        │ 33             │ Composes functions already ported; see finding 6       │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTHabitatMgr::checkExhibitMorph          │ 35             │ Wraps the already-ported morph_exhibit; 5 call sites   │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTHabitat::getSize                       │ 53             │ 3 call sites                                           │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTHabitat::updatePortals + getShowPortal │ 64 + 58        │ hasPortalAnimal is already ported                      │
├──────────────────────────────────────────┼────────────────┼────────────────────────────────────────────────────────┤
│ ZTTankExhibit::update                    │ 54             │ Finishes the tank tick; calls two unidentified helpers │
└──────────────────────────────────────────┴────────────────┴────────────────────────────────────────────────────────┘

The big one is recalculateCharacteristics (2002 lines, 24 call sites). It writes almost every cached field the ported getters read, and together with reviseSpeciesList, constructSurroundingSpeciesList and addSpecies it unlocks most of tier 3 below.

Replacing vanilla data structures with Rust

- Tier 1: internal scratch vectors, no visible behaviour change. Six functions build a vanilla-allocated vector only to read it back themselves: get_num_adult_animals_by_species, add_baby_born_bonus, get_random_clear_tile_for_animal, get_near_clear_tile, get_num_tiles_aggregating and get_random_tiles_aggregating.
  - Add iterator or Vec helpers (species_animals(id), clear_tiles(animal, check_path), tiles_matching(pred)), and keep the out-parameter detours as thin wrappers for vanilla callers.
  - The plan's own rule for internal temporaries already allows this (get_random_tile_in_direction uses a Vec), so the current code is inconsistent anyway.
- Tier 2: typed wrappers over vanilla containers, without taking ownership.
  - Extend the existing, read-only VanillaVector<T> with push and erase. That would replace 5 hand-rolled growth paths (vector_push_pool_alloc4, …_pool_dealloc, push_boundary_tile_pair, add_viewing_area, add_habitat) and 3 erase loops.
  - Add amphibious_neighbors()/show_neighbors() iterators to replace about 25 copies of walk_neighbor_tree followed by the node + 0x10 read.
  - Add one helper for the random-number draw (copied about 8 times) and one for "tile is unoccupied" (about 4 times).
- Tier 3: state owned entirely by Rust, once every reader is ported.
  - Neighbour sets → a BTreeSet<u32> per habitat. Its order matches the MSVC set<ZTHabitat*> pointer order, so random picks stay identical. The blockers are porting add/clear for amphibious and show neighbours (about 320 lines), plus the remaining vanilla readers (recalculateCharacteristics, constructSurroundingSpeciesList, updatePortals, getSize's neighbour branch, and the destructor).
  - boundary_tile_pairs → a Rust Vec. Every known reader is ported. It needs the createEdgePairs detour working, plus a grep for any other raw readers.
  - Not worth it yet: the owned-tile list and all_animals, which unported vanilla code still reads heavily. The pending_* gate queues could move to Rust, but they're plain data, so there's little gain.

Suggested order: finding 1 (audit plus the 10 calls), then 2–3 (UnsafeCell marker and panic-proofing), then 5, then 7, then the tier-1 Vec swaps, then the four trivial ports plus resize (which also narrows down finding 6), then the test gaps. Neighbour sets and recalculateCharacteristics are the longer-term project.