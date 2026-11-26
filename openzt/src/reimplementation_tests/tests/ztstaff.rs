//! Live checks for `ztstaff.rs`. Each comparison builds two staff with the real vanilla constructor over the same
//! real staff type, runs the real body on one and the Rust port on the other, and compares return values and the
//! staff's whole memory (heap pointers may differ, shared pointers may not).

use openzt_detour::generated::{
    poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
    standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    ztstaff::{ADD_TO_MAP, GET_TERRAIN_COST, IS_HABITAT_ASSIGNED, LOAD as STAFF_LOAD},
};

use crate::{
    globals::{get_module_base, globals},
    reimplementation_tests::{
        harness::{finish_test, write_success_line},
        io_redirect,
    },
    util::{get_from_memory, save_to_memory},
    vanilla_list::{self, FREELIST_12_RVA},
    vanilla_vector, ztkeeper,
    ztmegatilemgr::entity_type_matches,
    ztshow::RVA_ANIMAL_TYPE_CHECK,
    ztstaff::{
        live_support::{self, real},
        staff_offsets, ZTStaff, STAFF_SIZE, STAFF_TYPE_TAG_RVA, STAFF_VTABLE_RVA,
    },
};

/// The game's `rand` state (`DAT_00638060`).
const RNG_STATE_RVA: u32 = 0x0023_8060;
const UNIT_POS_X: u32 = 0x114;
const UNIT_POS_Y: u32 = 0x118;
const UNIT_POS_Z: u32 = 0x11c;
const ENTITY_ID_OFFSET: u32 = 0x124;
const ENTITY_TYPE_OFFSET: u32 = 0x128;
const ANIMAL_CAPTURED_BY_OFFSET: u32 = 0x374;

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn alloc(size: u32) -> u32 {
    let memory = unsafe { OPERATOR_NEW.original()(size) } as u32;
    unsafe { std::ptr::write_bytes(memory as *mut u8, 0, size as usize) };
    memory
}

fn words(address: u32) -> Vec<u32> {
    (0..STAFF_SIZE / 4).map(|i| get_from_memory::<u32>(address + i * 4)).collect()
}

/// Whether two words agree: equal, or both heap pointers neither of which is a shared pointer.
fn equivalent(real: u32, port: u32, shared: &[u32]) -> bool {
    real == port || (real >= 0x10000 && port >= 0x10000 && !shared.contains(&real) && !shared.contains(&port))
}

fn compare_memory(label: &str, real: ZTStaff, port: ZTStaff, shared: &[u32], failures: &mut Vec<String>) {
    for (i, (r, p)) in words(real.0).into_iter().zip(words(port.0)).enumerate() {
        if !equivalent(r, p, shared) {
            failures.push(format!("{label}: +{:#x} real {r:#x}, port {p:#x}", i * 4));
        }
    }
}

/// How many nodes were pushed on the 12-byte freelist since its head was `before`.
fn freelist_pushed(before: u32) -> usize {
    let mut node: u32 = get_from_memory(base() + FREELIST_12_RVA);
    let mut count = 0;
    while node != before && node != 0 && count < 64 {
        count += 1;
        node = get_from_memory(node);
    }
    count
}

fn freelist_head() -> u32 {
    get_from_memory(base() + FREELIST_12_RVA)
}

fn pin_rng(seed: u32) {
    save_to_memory(base() + RNG_STATE_RVA, seed);
}

fn rng() -> u32 {
    get_from_memory(base() + RNG_STATE_RVA)
}

/// Every entity type in the live world that is a staff type (keeper, guide, maintenance worker, ...).
fn staff_types() -> Vec<u32> {
    let world = globals().ztworldmgr();
    let tag = base() + STAFF_TYPE_TAG_RVA;
    let mut types = Vec::new();
    let mut slot = world.entity_type_array_start;
    while slot != world.entity_type_array_end {
        let ty: u32 = get_from_memory(slot);
        slot += 4;
        if ty == 0 {
            continue;
        }
        let vtable: u32 = get_from_memory(ty);
        let is_cast_class = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
        if is_cast_class(ty, tag) {
            types.push(ty);
        }
    }
    types
}

/// The sentinel of a vanilla-shaped, vanilla-allocated list of `payloads`, built through the staff's own list.
fn assign_all(staff: ZTStaff, habitats: &[u32]) {
    for &habitat in habitats {
        real::assign_habitat(staff.0, habitat);
    }
}

/// `ZTSTAFF_DETOURS_ENABLED`: every `ZTStaff` detour reports enabled, so the comparisons below exercise the hooked path
/// rather than silently running vanilla.
pub(crate) fn run_ztstaff_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSTAFF_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        finish_test(test_name, vec![format!("detours not enabled: {disabled:?}")], failure_log)
    }
}

/// `ZTSTAFF_FIXTURE_LAYOUT_LIVE`: a staff built by the vanilla constructor over a real staff type has the `ZTStaff`
/// vtable, the type pointer, an empty self-linked assigned-habitat list and clear vectors and countdown fields; a
/// real keeper built by `ZTKeeper`'s own constructor keeps the same list and vector offsets (so the subclass fields
/// start at `STAFF_SIZE`); and `drop` and `clearAssignedHabitats` write exactly the offsets the table names.
pub(crate) fn run_ztstaff_fixture_layout_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSTAFF_FIXTURE_LAYOUT_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = ztkeeper::live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };

    let staff = live_support::build_standalone_staff(keeper_type);
    let vtable: u32 = get_from_memory(staff.0);
    if vtable != base() + STAFF_VTABLE_RVA {
        failures.push(format!("vtable {vtable:#x}, expected {:#x}", base() + STAFF_VTABLE_RVA));
    }
    let entity_type: u32 = get_from_memory(staff.0 + ENTITY_TYPE_OFFSET);
    if entity_type != keeper_type {
        failures.push(format!("entity type {entity_type:#x}, expected {keeper_type:#x}"));
    }
    let sentinel = staff.assigned_list();
    if sentinel == 0 || get_from_memory::<u32>(sentinel) != sentinel || get_from_memory::<u32>(sentinel + 4) != sentinel {
        failures.push(format!("assigned list sentinel {sentinel:#x} is not empty and self-linked"));
    }
    for offset in [
        staff_offsets::GATE_TILES_BEGIN,
        staff_offsets::GATE_TILES_END,
        staff_offsets::GATE_TILES_CAPACITY,
        staff_offsets::SKIP_ENTITIES_BEGIN,
        staff_offsets::SKIP_ENTITIES_END,
        staff_offsets::SKIP_ENTITIES_CAPACITY,
        staff_offsets::SKIP_TILES_BEGIN,
        staff_offsets::SKIP_TILES_END,
        staff_offsets::SKIP_TILES_CAPACITY,
        staff_offsets::ESCAPED_ANIMAL,
    ] {
        let value: u32 = get_from_memory(staff.0 + offset);
        if value != 0 {
            failures.push(format!("+{offset:#x} is {value:#x} after construction"));
        }
    }

    let keeper = ztkeeper::live_support::build_standalone_keeper(keeper_type);
    let keeper_sentinel: u32 = get_from_memory(keeper.0 + staff_offsets::ASSIGNED_HABITATS);
    if keeper_sentinel == 0 || get_from_memory::<u32>(keeper_sentinel) != keeper_sentinel {
        failures.push("a keeper's assigned-habitat list is not empty and self-linked at +0x260".to_string());
    }
    let first_keeper_field = ztkeeper::OWN_DWORD_OFFSETS[0];
    if first_keeper_field != STAFF_SIZE {
        failures.push(format!("keeper's first own field is at {first_keeper_field:#x}, staff size is {STAFF_SIZE:#x}"));
    }
    ztkeeper::live_support::destroy_standalone_keeper(keeper);

    save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_BEGIN, 0x1000u32);
    save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_END, 0x1010u32);
    save_to_memory(staff.0 + staff_offsets::SKIP_TILES_BEGIN, 0x2000u32);
    save_to_memory(staff.0 + staff_offsets::SKIP_TILES_END, 0x2008u32);
    real::drop_staff(staff.0);
    if get_from_memory::<u32>(staff.0 + staff_offsets::SKIP_ENTITIES_END) != 0x1000 || get_from_memory::<u32>(staff.0 + staff_offsets::SKIP_TILES_END) != 0x2000 {
        failures.push("the real drop did not reset the end pointers to the begin pointers".to_string());
    }
    save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_BEGIN, 0u32);
    save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_END, 0u32);
    save_to_memory(staff.0 + staff_offsets::SKIP_TILES_BEGIN, 0u32);
    save_to_memory(staff.0 + staff_offsets::SKIP_TILES_END, 0u32);

    let habitat = alloc(0x40);
    assign_all(staff, &[habitat]);
    if staff.assigned_habitats() != vec![habitat] {
        failures.push(format!("the real assignHabitat left the list {:x?}", staff.assigned_habitats()));
    }
    real::clear_assigned_habitats(staff.0);
    if staff.assigned_list() != sentinel || !staff.assigned_habitats().is_empty() {
        failures.push("the real clearAssignedHabitats did not leave the list empty".to_string());
    }
    live_support::destroy_standalone_staff(staff);
    finish_test(test_name, failures, failure_log)
}

/// `ZTSTAFF_ASSIGNED_HABITATS_LIVE`: `assignHabitat`, `isHabitatAssigned`, `getAssignedHabitat`,
/// `removeAssignedHabitat`, `removeAssignedHabitatAt` and `clearAssignedHabitats` against the real bodies on twin
/// staff: null, world and duplicate habitats are refused, lookups past the end and with negative indices agree, an
/// absent habitat leaves the list alone, and each erase pushes the same number of nodes on the freelist. The staff's
/// whole memory is compared after every step.
pub(crate) fn run_ztstaff_assigned_habitats_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSTAFF_ASSIGNED_HABITATS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = ztkeeper::live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let habitats: Vec<u32> = (0..4).map(|_| alloc(0x40)).collect();
    let world_habitat = alloc(0x40);
    save_to_memory(world_habitat + 0x2c, 1u8);
    let absent = alloc(0x40);
    let shared = [keeper_type];
    let real_staff = live_support::build_standalone_staff(keeper_type);
    let port_staff = live_support::build_standalone_staff(keeper_type);

    // The sequence is applied to both poles; each step names its own check.
    for habitat in [0, habitats[0], world_habitat, habitats[1], habitats[0], habitats[2], habitats[1], habitats[3]] {
        real::assign_habitat(real_staff.0, habitat);
        port_staff.assign_habitat(habitat);
        let (real_list, port_list) = (real_staff.assigned_habitats(), port_staff.assigned_habitats());
        if real_list.len() != port_list.len() || real_list.iter().zip(&port_list).any(|(a, b)| a != b) {
            failures.push(format!("assignHabitat({habitat:#x}): real {real_list:x?}, port {port_list:x?}"));
        }
    }
    if real_staff.assigned_habitats() != habitats[..4].to_vec() {
        failures.push(format!("the assigned list is {:x?}, expected the four ordinary habitats in order", real_staff.assigned_habitats()));
    }
    compare_memory("after assignHabitat", real_staff, port_staff, &shared, &mut failures);

    for probe in habitats.iter().copied().chain([absent, world_habitat, 0]) {
        let real_result = real::is_habitat_assigned(real_staff.0, probe);
        let port_result = port_staff.is_habitat_assigned(probe);
        let hooked = unsafe { IS_HABITAT_ASSIGNED.hooked()(real_staff.0 as *const u32, probe as *const u32) };
        if real_result != port_result || real_result != hooked {
            failures.push(format!("isHabitatAssigned({probe:#x}): real {real_result}, port {port_result}, hooked {hooked}"));
        }
    }
    for index in [-3i32, -1, 0, 1, 2, 3, 4, 5, 100] {
        let real_result = real::get_assigned_habitat(real_staff.0, index);
        let port_result = port_staff.get_assigned_habitat(index);
        if real_result != port_result {
            failures.push(format!("getAssignedHabitat({index}): real {real_result:#x}, port {port_result:#x}"));
        }
    }

    for (label, habitat) in [("an absent habitat", absent), ("the second habitat", habitats[1]), ("the first habitat", habitats[0]), ("the last habitat", habitats[3])] {
        let real_before = freelist_head();
        real::remove_assigned_habitat(real_staff.0, habitat);
        let real_pushed = freelist_pushed(real_before);
        let port_before = freelist_head();
        port_staff.remove_assigned_habitat(habitat);
        let port_pushed = freelist_pushed(port_before);
        if real_pushed != port_pushed || real_staff.assigned_habitats() != port_staff.assigned_habitats() {
            failures.push(format!(
                "removeAssignedHabitat({label}): real pushed {real_pushed} leaving {:x?}, port pushed {port_pushed} leaving {:x?}",
                real_staff.assigned_habitats(),
                port_staff.assigned_habitats()
            ));
        }
    }
    compare_memory("after removeAssignedHabitat", real_staff, port_staff, &shared, &mut failures);

    assign_all(real_staff, &habitats);
    assign_all(port_staff, &habitats);
    for index in [-1i32, 7, 1, 0, 1, 0, 0] {
        let real_before = freelist_head();
        real::remove_assigned_habitat_at(real_staff.0, index);
        let real_pushed = freelist_pushed(real_before);
        let port_before = freelist_head();
        port_staff.remove_assigned_habitat_at(index);
        let port_pushed = freelist_pushed(port_before);
        if real_pushed != port_pushed || real_staff.assigned_habitats() != port_staff.assigned_habitats() {
            failures.push(format!(
                "removeAssignedHabitatAt({index}): real pushed {real_pushed} leaving {:x?}, port pushed {port_pushed} leaving {:x?}",
                real_staff.assigned_habitats(),
                port_staff.assigned_habitats()
            ));
        }
    }
    if !real_staff.assigned_habitats().is_empty() {
        failures.push(format!("the position-based erases left {:x?}", real_staff.assigned_habitats()));
    }

    assign_all(real_staff, &habitats);
    assign_all(port_staff, &habitats);
    let real_before = freelist_head();
    real::clear_assigned_habitats(real_staff.0);
    let real_pushed = freelist_pushed(real_before);
    let port_before = freelist_head();
    port_staff.clear_assigned_habitats();
    let port_pushed = freelist_pushed(port_before);
    if real_pushed != 4 || port_pushed != 4 {
        failures.push(format!("clearAssignedHabitats pushed real {real_pushed}, port {port_pushed} nodes, expected 4 each"));
    }
    compare_memory("after clearAssignedHabitats", real_staff, port_staff, &shared, &mut failures);
    real::clear_assigned_habitats(real_staff.0);
    port_staff.clear_assigned_habitats();
    compare_memory("after clearing an empty list", real_staff, port_staff, &shared, &mut failures);

    live_support::destroy_standalone_staff(real_staff);
    live_support::destroy_standalone_staff(port_staff);
    finish_test(test_name, failures, failure_log)
}

/// `ZTSTAFF_LEAF_LIVE`: `getMonthlyCost` over every staff type in the live world, `drop` over vectors holding data,
/// `doneCapture` with no escaped animal, an id that resolves to nothing and an id that resolves to a real animal
/// (whose capturing-staff field must be cleared on both poles), and `getRandomAssignedHabitat` under pinned RNG
/// states (return value and the generator state afterwards must match). Whole-memory comparison after each.
pub(crate) fn run_ztstaff_leaf_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSTAFF_LEAF_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = staff_types();
    let Some(&keeper_type) = types.first() else {
        failures.push("the live world holds no staff type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = types.clone();

    for &staff_type in &types {
        let real_staff = live_support::build_standalone_staff(staff_type);
        let port_staff = live_support::build_standalone_staff(staff_type);
        let real_cost = real::get_monthly_cost(real_staff.0);
        let port_cost = port_staff.get_monthly_cost();
        if real_cost.to_bits() != port_cost.to_bits() {
            failures.push(format!("getMonthlyCost over type {staff_type:#x}: real {real_cost}, port {port_cost}"));
        }
        live_support::destroy_standalone_staff(real_staff);
        live_support::destroy_standalone_staff(port_staff);
    }

    for (label, entities, tiles) in [("empty vectors", (0u32, 0u32), (0u32, 0u32)), ("filled vectors", (0x5000, 0x5010), (0x6000, 0x6008))] {
        let real_staff = live_support::build_standalone_staff(keeper_type);
        let port_staff = live_support::build_standalone_staff(keeper_type);
        for staff in [real_staff, port_staff] {
            save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_BEGIN, entities.0);
            save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_END, entities.1);
            save_to_memory(staff.0 + staff_offsets::SKIP_TILES_BEGIN, tiles.0);
            save_to_memory(staff.0 + staff_offsets::SKIP_TILES_END, tiles.1);
        }
        real::drop_staff(real_staff.0);
        port_staff.drop_staff();
        compare_memory(&format!("drop with {label}"), real_staff, port_staff, &shared, &mut failures);
        for staff in [real_staff, port_staff] {
            save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_BEGIN, 0u32);
            save_to_memory(staff.0 + staff_offsets::SKIP_ENTITIES_END, 0u32);
            save_to_memory(staff.0 + staff_offsets::SKIP_TILES_BEGIN, 0u32);
            save_to_memory(staff.0 + staff_offsets::SKIP_TILES_END, 0u32);
        }
        live_support::destroy_standalone_staff(real_staff);
        live_support::destroy_standalone_staff(port_staff);
    }

    let world = globals().ztworldmgr();
    let animal = world.entity_array().find(|&entity| entity != 0 && unsafe { entity_type_matches(entity, RVA_ANIMAL_TYPE_CHECK) });
    let non_animal = world.entity_array().find(|&entity| entity != 0 && !unsafe { entity_type_matches(entity, RVA_ANIMAL_TYPE_CHECK) });
    let mut cases: Vec<(String, u32, Option<u32>)> = vec![("no escaped animal".to_string(), 0, None), ("an id that resolves to nothing".to_string(), 0x7fff_fff0, None)];
    if let Some(animal) = animal {
        cases.push(("a live animal".to_string(), get_from_memory(animal + ENTITY_ID_OFFSET), Some(animal)));
    } else {
        failures.push("the live world holds no animal, so doneCapture's clearing arm is not exercised".to_string());
    }
    if let Some(entity) = non_animal {
        cases.push(("a live entity that is not an animal".to_string(), get_from_memory(entity + ENTITY_ID_OFFSET), Some(entity)));
    }
    for (label, id, marked) in cases {
        let mut after = Vec::new();
        for run_real in [true, false] {
            let staff = live_support::build_standalone_staff(keeper_type);
            save_to_memory(staff.0 + staff_offsets::ESCAPED_ANIMAL, id);
            let original = marked.map(|entity| get_from_memory::<u32>(entity + ANIMAL_CAPTURED_BY_OFFSET));
            if let Some(entity) = marked {
                save_to_memory(entity + ANIMAL_CAPTURED_BY_OFFSET, 0x1234_5678u32);
            }
            if run_real {
                real::done_capture(staff.0);
            } else {
                staff.done_capture();
            }
            let captured_by = marked.map(|entity| get_from_memory::<u32>(entity + ANIMAL_CAPTURED_BY_OFFSET));
            if let (Some(entity), Some(original)) = (marked, original) {
                save_to_memory(entity + ANIMAL_CAPTURED_BY_OFFSET, original);
            }
            after.push((words(staff.0), captured_by));
            live_support::destroy_standalone_staff(staff);
        }
        let (real_after, port_after) = (&after[0], &after[1]);
        if real_after.1 != port_after.1 {
            failures.push(format!("doneCapture with {label}: real left the animal field {:x?}, port {:x?}", real_after.1, port_after.1));
        }
        for (i, (r, p)) in real_after.0.iter().zip(&port_after.0).enumerate() {
            if !equivalent(*r, *p, &shared) {
                failures.push(format!("doneCapture with {label}: +{:#x} real {r:#x}, port {p:#x}", i * 4));
            }
        }
    }

    let real_staff = live_support::build_standalone_staff(keeper_type);
    let port_staff = live_support::build_standalone_staff(keeper_type);
    let habitat_count = globals().zthabitatmgr().exhibit_array().len();
    let mut seen = std::collections::BTreeSet::new();
    for seed in [1u32, 0x1234_5678, 0xdead_beef, 42, 7, 0x0bad_f00d, 99_999, 3] {
        pin_rng(seed);
        let real_result = real::get_random_assigned_habitat(real_staff.0);
        let real_rng = rng();
        pin_rng(seed);
        let port_result = port_staff.get_random_assigned_habitat();
        let port_rng = rng();
        seen.insert(real_result);
        if real_result != port_result || real_rng != port_rng {
            failures.push(format!(
                "getRandomAssignedHabitat at seed {seed:#x}: real {real_result:#x} (rng {real_rng:#x}), port {port_result:#x} (rng {port_rng:#x})"
            ));
        }
    }
    if habitat_count > 1 && seen.len() < 2 {
        failures.push(format!("{habitat_count} habitats but the draws only ever chose {seen:x?}"));
    }
    live_support::destroy_standalone_staff(real_staff);
    live_support::destroy_standalone_staff(port_staff);
    finish_test(test_name, failures, failure_log)
}

fn place_at_tile(staff: ZTStaff, tile: (u32, u32)) {
    save_to_memory(staff.0 + UNIT_POS_X, tile.0 * 64 + 32);
    save_to_memory(staff.0 + UNIT_POS_Y, tile.1 * 64 + 32);
    save_to_memory(staff.0 + UNIT_POS_Z, 0u32);
}

/// `ZTSTAFF_TERRAIN_COST_LIVE`: `getTerrainCost` for a base-`ZTStaff` unit over every tile of the live map's first
/// 200x200 tiles, with the unit standing in two places, against the real body, and through the hooked address on a
/// diagonal sample. At least two distinct costs must occur so more than one branch is covered.
pub(crate) fn run_ztstaff_terrain_cost_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSTAFF_TERRAIN_COST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = ztkeeper::live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let staff = live_support::build_standalone_staff(keeper_type);
    let mut seen = std::collections::BTreeSet::new();
    let mut mismatches = 0;
    for position in [(0u32, 0u32), (world.map_x_size / 2, world.map_y_size / 2)] {
        place_at_tile(staff, position);
        for y in 0..world.map_y_size.min(200) {
            for x in 0..world.map_x_size.min(200) {
                let tile = world.get_tile_ptr(x, y);
                let real_cost = real::get_terrain_cost(staff.0, tile);
                let port_cost = staff.get_terrain_cost(tile);
                seen.insert(real_cost);
                if real_cost != port_cost {
                    mismatches += 1;
                    if mismatches <= 10 {
                        failures.push(format!("staff at {position:?}, tile ({x},{y}): real {real_cost}, port {port_cost}"));
                    }
                }
                if x == y && x % 17 == 0 {
                    let hooked = unsafe { GET_TERRAIN_COST.hooked()(staff.0 as *const u32, tile as *const u32) };
                    if hooked != real_cost {
                        failures.push(format!("tile ({x},{y}): hooked {hooked}, real {real_cost}"));
                    }
                }
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if seen.len() < 2 {
        failures.push(format!("only the costs {seen:?} occurred, so the grid covered a single branch"));
    }
    live_support::destroy_standalone_staff(staff);
    finish_test(test_name, failures, failure_log)
}

/// Chain length of each of the pool's 16 small-block freelists (`DAT_00638000 + 4 * ((bytes - 1) >> 3)`).
fn pool_buckets() -> Vec<usize> {
    (0..16)
        .map(|bucket| {
            let mut node: u32 = get_from_memory(base() + 0x0023_8000 + bucket * 4);
            let mut count = 0;
            while node != 0 && count < 5000 {
                count += 1;
                node = get_from_memory(node);
            }
            count
        })
        .collect()
}

fn pool_growth(before: &[usize], after: &[usize]) -> Vec<isize> {
    before.iter().zip(after).map(|(b, a)| *a as isize - *b as isize).collect()
}

fn gate_vector(staff: ZTStaff) -> Vec<u32> {
    let begin: u32 = get_from_memory(staff.0 + staff_offsets::GATE_TILES_BEGIN);
    let end: u32 = get_from_memory(staff.0 + staff_offsets::GATE_TILES_END);
    if begin == 0 || end < begin {
        return Vec::new();
    }
    (0..(end - begin) / 4).map(|i| get_from_memory::<u32>(begin + i * 4)).collect()
}

/// Gives `staff` three assigned habitats, three gate-tile records and pool buffers for both skip vectors, so a
/// destructor has something of every kind to return to the pool.
fn populate(staff: ZTStaff, habitats: &[u32]) {
    assign_all(staff, habitats);
    for record in [[1u32, 2, 0], [3, 4, 0], [5, 6, 0]] {
        vanilla_vector::push_words(staff.0 + staff_offsets::GATE_TILES_BEGIN, record, |begin, byte_capacity| {
            if begin != 0 {
                unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, byte_capacity / 12 * 12) };
            }
        });
    }
    for (begin_offset, bytes) in [(staff_offsets::SKIP_ENTITIES_BEGIN, 16u32), (staff_offsets::SKIP_TILES_BEGIN, 24)] {
        let buffer = unsafe { POOLALLOC_ALLOCATE.original()(bytes) } as u32;
        save_to_memory(staff.0 + begin_offset, buffer);
        save_to_memory(staff.0 + begin_offset + 4, buffer + 8);
        save_to_memory(staff.0 + begin_offset + 8, buffer + bytes);
    }
}

/// `ZTSTAFF_LIFECYCLE_LIVE`: the Rust constructor leaves the same memory as the real one; the destructor and the
/// scalar-deleting destructor (flags `0` and `1`) of a staff holding assigned habitats, gate-tile records and both skip
/// vectors return the same buffers to each pool freelist and return the unit.
pub(crate) fn run_ztstaff_lifecycle_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztstaff::DESTRUCTOR_1;

    let test_name = "ZTSTAFF_LIFECYCLE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = staff_types();
    if types.is_empty() {
        failures.push("the live world holds no staff type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let shared = types.clone();
    for &staff_type in &types {
        let (real_staff, port_staff) = (live_support::build_standalone_staff(staff_type), live_support::build_standalone_staff_port(staff_type));
        compare_memory(&format!("constructor over type {staff_type:#x}"), real_staff, port_staff, &shared, &mut failures);
        live_support::destroy_standalone_staff(real_staff);
        live_support::destroy_standalone_staff(port_staff);
    }

    let staff_type = types[0];
    let habitats: Vec<u32> = (0..3).map(|_| alloc(0x40)).collect();
    for (label, flags) in [("non-deleting", None), ("deleting, flags 0", Some(0u8)), ("deleting, flags 1", Some(1u8))] {
        let mut growth = Vec::new();
        let mut returned = Vec::new();
        for run_real in [true, false] {
            let staff = live_support::build_standalone_staff(staff_type);
            populate(staff, &habitats);
            let before = pool_buckets();
            let result = match (flags, run_real) {
                (None, true) => real::destructor(staff.0),
                (None, false) => staff.destruct(),
                (Some(flags), true) => real::destructor_deleting(staff.0, flags),
                (Some(flags), false) => unsafe { DESTRUCTOR_1.hooked()(staff.0 as *const u32, flags) },
            } as u32;
            growth.push(pool_growth(&before, &pool_buckets()));
            returned.push(result == staff.0);
            if flags != Some(1) {
                unsafe { OPERATOR_DELETE.original()(staff.0) };
            }
        }
        if growth[0] != growth[1] {
            failures.push(format!("{label} destructor: the pool freelists grew by {:?} (real) and {:?} (port)", growth[0], growth[1]));
        }
        if returned[0] != returned[1] {
            failures.push(format!("{label} destructor: returned the unit {} (real), {} (port)", returned[0], returned[1]));
        }
        if growth[0].iter().sum::<isize>() < 6 {
            failures.push(format!("{label} destructor returned only {:?} blocks to the pool, so the fixture did not exercise it", growth[0]));
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTSTAFF_INIT_STATUS_VARS_LIVE`: `initStatusVars` against the real body over staff types with each combination of
/// the type's two periods zero and non-zero, under pinned RNG states: the return value (vanilla's `EAX`), the generator
/// state afterwards and the staff's whole memory must match. The type's periods are restored afterwards.
pub(crate) fn run_ztstaff_init_status_vars_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTSTAFF_INIT_STATUS_VARS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = staff_types();
    if types.is_empty() {
        failures.push("the live world holds no staff type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let shared = types.clone();
    for &staff_type in types.iter().take(3) {
        let original: [i32; 2] = [get_from_memory(staff_type + 0x1b4), get_from_memory(staff_type + 0x1b8)];
        for (first, second) in [(0, 0), (0, 5), (7, 0), (7, 13), (1000, 1000), (1, 1), (0x7fff, 3)] {
            save_to_memory(staff_type + 0x1b4, first);
            save_to_memory(staff_type + 0x1b8, second);
            for seed in [1u32, 0x1234_5678, 0xdead_beef, 77] {
                let (real_staff, port_staff) = (live_support::build_standalone_staff(staff_type), live_support::build_standalone_staff_port(staff_type));
                for staff in [real_staff, port_staff] {
                    save_to_memory(staff.0 + staff_offsets::ESCAPED_ANIMAL, 0x77u32);
                    save_to_memory(staff.0 + staff_offsets::INITIAL_COUNTDOWN, 0x55u32);
                }
                pin_rng(seed);
                let real_result = real::init_status_vars(real_staff.0);
                let real_rng = rng();
                pin_rng(seed);
                let port_result = port_staff.init_status_vars();
                let port_rng = rng();
                let case = format!("type {staff_type:#x} periods ({first}, {second}) seed {seed:#x}");
                if real_result != port_result {
                    failures.push(format!("{case}: EAX real {real_result:#x}, port {port_result:#x}"));
                }
                if real_rng != port_rng {
                    failures.push(format!("{case}: rng real {real_rng:#x}, port {port_rng:#x}"));
                }
                compare_memory(&case, real_staff, port_staff, &shared, &mut failures);
                live_support::destroy_standalone_staff(real_staff);
                live_support::destroy_standalone_staff(port_staff);
            }
        }
        save_to_memory(staff_type + 0x1b4, original[0]);
        save_to_memory(staff_type + 0x1b8, original[1]);
    }
    finish_test(test_name, failures, failure_log)
}

/// Live habitats that have a gate tile resolving back to themselves: `(habitat, gate x, gate y)`.
fn habitats_with_gate_tiles() -> Vec<(u32, i32, i32)> {
    use openzt_detour::generated::zthabitat::GET_GATE_TILE_IN;

    let manager = globals().zthabitatmgr();
    let exhibits = manager.exhibit_array();
    let mut found = Vec::new();
    for index in 0..exhibits.len() {
        let habitat = exhibits.get_ptr(index);
        if get_from_memory::<u8>(habitat + 0x2c) != 0 {
            continue;
        }
        let tile = unsafe { GET_GATE_TILE_IN.hooked()(habitat as *const u32) } as u32;
        if tile == 0 {
            continue;
        }
        let (x, y): (i32, i32) = (get_from_memory(tile + 0x34), get_from_memory(tile + 0x38));
        if manager.get_habitat_ptr(x, y) == habitat {
            found.push((habitat, x, y));
        }
        if found.len() == 3 {
            break;
        }
    }
    found
}

fn run_staff_load(staff: ZTStaff, bytes: &[u8], version: u32, pole: usize) -> (bool, usize) {
    io_redirect::begin_replay(bytes.to_vec());
    let result = match pole {
        0 => real::load(staff.0, std::ptr::null(), version),
        1 => staff.load(std::ptr::null(), version),
        _ => unsafe { STAFF_LOAD.hooked()(staff.0 as *const u32, std::ptr::null(), version) },
    };
    let consumed = io_redirect::replay_position();
    io_redirect::end_replay();
    (result, consumed)
}

/// `ZTSTAFF_SAVE_LOAD_LIVE`: `save` of a staff with live assigned habitats writes the real body's bytes and result;
/// `load` is compared over every version boundary (`0x0f`, `0x10`, `0x15`, `0x16`, `0x19`, `0x1a`, `0x4a`, `0x4b`) with
/// a stream holding the staff data and a real `ZTUnit` tail, one byte short, and (from `0x1a`) cut inside the gate-tile
/// records; result, bytes consumed, the whole staff and the gate-tile records must match across the real body, the port
/// and the hooked address. A real save is then loaded into twins (the staff part must be read in full; a bare
/// fixture's `ZTUnit` tail may legitimately fail to load) and `reassignHabitats` and `initAfterLoad` must restore the
/// original assignments.
pub(crate) fn run_ztstaff_save_load_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztstaff::INIT_AFTER_LOAD;

    let test_name = "ZTSTAFF_SAVE_LOAD_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = staff_types();
    let gated = habitats_with_gate_tiles();
    let (Some(&staff_type), false) = (types.first(), gated.is_empty()) else {
        failures.push(format!("need a staff type and a habitat with a gate tile, found {} types and {} habitats", types.len(), gated.len()));
        return finish_test(test_name, failures, failure_log);
    };
    let shared = types.clone();
    let habitats: Vec<u32> = gated.iter().map(|(habitat, _, _)| *habitat).collect();

    let build_dirty = |port: bool| {
        let staff = if port { live_support::build_standalone_staff_port(staff_type) } else { live_support::build_standalone_staff(staff_type) };
        assign_all(staff, &habitats);
        save_to_memory(staff.0 + staff_offsets::INITIAL_COUNTDOWN, 0x1112_1314u32);
        save_to_memory(staff.0 + staff_offsets::CONDITIONALS_COUNTDOWN, 0x2122_2324u32);
        save_to_memory(staff.0 + staff_offsets::ESCAPED_ANIMAL, 0x3132_3334u32);
        staff
    };
    let (real_staff, port_staff) = (build_dirty(false), build_dirty(true));
    io_redirect::begin_capture();
    let real_ok = real::save(real_staff.0, std::ptr::null());
    let real_bytes = io_redirect::end_capture();
    io_redirect::begin_capture();
    let port_ok = port_staff.save(std::ptr::null());
    let port_bytes = io_redirect::end_capture();
    if real_ok != port_ok {
        failures.push(format!("save: real returned {real_ok}, port {port_ok}"));
    }
    if real_bytes != port_bytes {
        failures.push(format!("save: real wrote {} bytes, port {}, or the bytes differ", real_bytes.len(), port_bytes.len()));
    }
    let count = habitats.len();
    let mut expected_prefix = vec![0x14u8, 0x13, 0x12, 0x11, 0x24, 0x23, 0x22, 0x21, count as u8, 0, 0, 0];
    for (_, x, y) in &gated {
        expected_prefix.extend_from_slice(&x.to_le_bytes());
        expected_prefix.extend_from_slice(&y.to_le_bytes());
    }
    expected_prefix.extend_from_slice(&[0x34, 0x33, 0x32, 0x31]);
    if !port_bytes.starts_with(&expected_prefix) {
        failures.push(format!("save: leading bytes {:02x?}, expected {:02x?}", &port_bytes[..port_bytes.len().min(expected_prefix.len())], expected_prefix));
    }
    live_support::destroy_standalone_staff(real_staff);
    live_support::destroy_standalone_staff(port_staff);

    let staff_part = expected_prefix.len();
    let unit_tail = port_bytes[staff_part.min(port_bytes.len())..].to_vec();
    let stream_for = |version: u32| -> Vec<u8> {
        let mut bytes = Vec::new();
        if version >= 0x10 {
            bytes.extend_from_slice(&expected_prefix[0..4]);
        }
        if version >= 0x16 {
            bytes.extend_from_slice(&expected_prefix[4..8]);
        }
        if version >= 0x1a {
            bytes.extend_from_slice(&expected_prefix[8..staff_part - 4]);
        }
        if version >= 0x4b {
            bytes.extend_from_slice(&expected_prefix[staff_part - 4..]);
        }
        bytes.extend_from_slice(&unit_tail);
        bytes
    };
    for version in [0x0f_u32, 0x10, 0x15, 0x16, 0x19, 0x1a, 0x4a, 0x4b] {
        let exact = stream_for(version);
        let mut streams = vec![("exact", exact.clone()), ("one byte short", exact[..exact.len() - 1].to_vec())];
        if version >= 0x1a && count > 1 {
            streams.push(("cut inside the records", exact[..12 + 8 + 4].to_vec()));
        }
        streams.push(("empty", Vec::new()));
        for (label, bytes) in streams {
            let case = format!("load v{version:#x} ({label})");
            let mut outcomes = Vec::new();
            for pole in [0usize, 1, 2] {
                let staff = live_support::build_standalone_staff(staff_type);
                let (result, consumed) = run_staff_load(staff, &bytes, version, pole);
                outcomes.push((staff, result, consumed, gate_vector(staff)));
            }
            for pole in 1..3 {
                if outcomes[0].1 != outcomes[pole].1 || outcomes[0].2 != outcomes[pole].2 || outcomes[0].3 != outcomes[pole].3 {
                    failures.push(format!(
                        "{case}: pole {pole} gave ({}, {} bytes, records {:x?}), real ({}, {} bytes, records {:x?})",
                        outcomes[pole].1, outcomes[pole].2, outcomes[pole].3, outcomes[0].1, outcomes[0].2, outcomes[0].3
                    ));
                }
                compare_memory(&format!("{case} pole {pole}"), outcomes[0].0, outcomes[pole].0, &shared, &mut failures);
            }
            for (staff, ..) in outcomes {
                live_support::destroy_standalone_staff(staff);
            }
        }
    }

    for (label, pole) in [("real", 0usize), ("port", 1), ("hooked", 2)] {
        let staff = live_support::build_standalone_staff(staff_type);
        let (result, consumed) = run_staff_load(staff, &port_bytes, 0x4b, pole);
        let records = gate_vector(staff);
        if consumed < staff_part || records.len() != 3 * count {
            failures.push(format!("round trip ({label}): result {result}, consumed {consumed} of {} (staff part {staff_part}), {} record words", port_bytes.len(), records.len()));
        }
        let reassigned = match pole {
            0 => real::reassign_habitats(staff.0),
            _ => staff.reassign_habitats(),
        };
        if reassigned & 0xff != 1 || staff.assigned_habitats() != habitats || !gate_vector(staff).is_empty() {
            failures.push(format!(
                "round trip ({label}): reassignHabitats returned {reassigned:#x}, assigned {:x?} (expected {habitats:x?}), {} record words left",
                staff.assigned_habitats(),
                gate_vector(staff).len()
            ));
        }
        live_support::destroy_standalone_staff(staff);
    }

    let mut after_load = Vec::new();
    for pole in [0usize, 1, 2] {
        let staff = live_support::build_standalone_staff(staff_type);
        run_staff_load(staff, &port_bytes, 0x4b, 1);
        place_at_tile(staff, (5, 5));
        let result = match pole {
            0 => real::init_after_load(staff.0, 0),
            1 => staff.init_after_load(0),
            _ => unsafe { INIT_AFTER_LOAD.hooked()(staff.0 as *const u32, 0) },
        };
        after_load.push((staff, result));
    }
    for pole in 1..3 {
        if after_load[0].1 != after_load[pole].1 || after_load[0].0.assigned_habitats() != after_load[pole].0.assigned_habitats() {
            failures.push(format!(
                "initAfterLoad: pole {pole} gave {} and {:x?}, real {} and {:x?}",
                after_load[pole].1,
                after_load[pole].0.assigned_habitats(),
                after_load[0].1,
                after_load[0].0.assigned_habitats()
            ));
        }
        compare_memory(&format!("initAfterLoad pole {pole}"), after_load[0].0, after_load[pole].0, &shared, &mut failures);
    }
    for (staff, _) in after_load {
        live_support::destroy_standalone_staff(staff);
    }
    finish_test(test_name, failures, failure_log)
}

/// The global staff list's values (`DAT_00639474`).
fn global_staff() -> Vec<u32> {
    let sentinel: u32 = get_from_memory(base() + 0x0023_9474);
    vanilla_list::nodes(sentinel).map(vanilla_list::value).collect()
}

/// `ZTSTAFF_ADD_REMOVE_LIVE`: a fixture staff is put on the live map by the real body, the port and the hooked address,
/// on several tiles, with and without the `+0x142`/`+0x143` exclusions; the result and where the staff then appears (its
/// tile's entity list, the global staff list, the world's entity vector) must agree. Each placement is undone with
/// `removeFromMap` through the same pole, after which the three lists must equal their snapshots, so the zoo is left as
/// found. A final pass checks `removeFromMap`'s release of a live escaped animal for both values of its flag.
pub(crate) fn run_ztstaff_add_remove_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztstaff::REMOVE_FROM_MAP;

    let test_name = "ZTSTAFF_ADD_REMOVE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = staff_types();
    let Some(&staff_type) = types.first() else {
        failures.push("the live world holds no staff type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let world_entities = || -> Vec<u32> {
        let manager: u32 = get_from_memory(base() + 0x0023_8040);
        let (begin, end): (u32, u32) = (get_from_memory(manager + 0x80), get_from_memory(manager + 0x84));
        (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
    };
    let tiles = [(5u32, 5u32), (world.map_x_size / 2, world.map_y_size / 2), (20, 30), (1, 1)];
    let (mut added, mut listed) = (0, 0);
    for &(x, y) in &tiles {
        let tile = world.get_tile_ptr(x, y);
        let on_tile = || vanilla_list::nodes(get_from_memory::<u32>(tile)).map(vanilla_list::value).collect::<Vec<u32>>();
        for (flag_offset, excluded) in [(0x142u32, false), (0x142, true), (0x143, true)] {
            let mut outcomes = vec![(false, false, false, false, 0usize); 3];
            for pole in [1usize, 2, 0] {
                let staff = live_support::build_standalone_staff(staff_type);
                place_at_tile(staff, (x, y));
                save_to_memory(staff.0 + flag_offset, excluded as u8);
                let (entities_before, tile_before, list_before) = (world_entities(), on_tile(), global_staff());
                let result = match pole {
                    0 => real::add_to_map(staff.0),
                    1 => staff.add_to_map(),
                    _ => unsafe { ADD_TO_MAP.hooked()(staff.0 as *const u32) },
                };
                let mut outcome = (result, on_tile().contains(&staff.0), global_staff().contains(&staff.0), world_entities().contains(&staff.0), 0usize);
                if result {
                    let freelist = freelist_head();
                    match pole {
                        0 => real::remove_from_map(staff.0, false),
                        1 => staff.remove_from_map(false),
                        _ => unsafe { REMOVE_FROM_MAP.hooked()(staff.0 as *const u32, false) },
                    }
                    outcome.4 = freelist_pushed(freelist);
                }
                let label = format!("({x},{y}) flag {flag_offset:#x}={excluded} pole {pole}");
                if world_entities() != entities_before || on_tile() != tile_before || global_staff() != list_before {
                    failures.push(format!("{label}: the zoo was not restored by removeFromMap (outcome {outcome:?})"));
                }
                outcomes[pole] = outcome;
            }
            added += outcomes[0].0 as u32;
            listed += outcomes[0].2 as u32;
            if outcomes[0] != outcomes[1] || outcomes[0] != outcomes[2] {
                failures.push(format!(
                    "({x},{y}) flag {flag_offset:#x}={excluded}: (result, on tile, in staff list, in world, nodes freed by remove) real {:?}, port {:?}, hooked {:?}",
                    outcomes[0], outcomes[1], outcomes[2]
                ));
            }
        }
    }
    if added == 0 || listed == 0 {
        failures.push(format!("coverage too thin: {added} placements succeeded, {listed} reached the staff list"));
    }

    let animal = world.entity_array().find(|&entity| entity != 0 && unsafe { entity_type_matches(entity, RVA_ANIMAL_TYPE_CHECK) });
    if let Some(animal) = animal {
        let id: u32 = get_from_memory(animal + ENTITY_ID_OFFSET);
        let original: u32 = get_from_memory(animal + ANIMAL_CAPTURED_BY_OFFSET);
        for keep in [false, true] {
            let mut outcomes = Vec::new();
            for run_real in [true, false] {
                let staff = live_support::build_standalone_staff(staff_type);
                place_at_tile(staff, (5, 5));
                save_to_memory(staff.0 + 0x142, 1u8);
                if !(if run_real { real::add_to_map(staff.0) } else { staff.add_to_map() }) {
                    failures.push(format!("escaped-animal pass: the staff did not go onto the map (keep {keep})"));
                    live_support::destroy_standalone_staff(staff);
                    continue;
                }
                save_to_memory(staff.0 + staff_offsets::ESCAPED_ANIMAL, id);
                save_to_memory(animal + ANIMAL_CAPTURED_BY_OFFSET, 0x1234_5678u32);
                if run_real {
                    real::remove_from_map(staff.0, keep);
                } else {
                    staff.remove_from_map(keep);
                }
                outcomes.push((get_from_memory::<u32>(staff.0 + staff_offsets::ESCAPED_ANIMAL), get_from_memory::<u32>(animal + ANIMAL_CAPTURED_BY_OFFSET)));
                save_to_memory(animal + ANIMAL_CAPTURED_BY_OFFSET, original);
                live_support::destroy_standalone_staff(staff);
            }
            if outcomes.len() == 2 && outcomes[0] != outcomes[1] {
                failures.push(format!("removeFromMap keep={keep} with an escaped animal: (escaped id, animal field) real {:x?}, port {:x?}", outcomes[0], outcomes[1]));
            }
            if outcomes.len() == 2 && outcomes[0] != if keep { (id, 0x1234_5678) } else { (0, 0) } {
                failures.push(format!("removeFromMap keep={keep}: unexpected outcome {:x?}", outcomes[0]));
            }
        }
    } else {
        failures.push("the live world holds no animal, so removeFromMap's escaped-animal arm is not exercised".to_string());
    }
    finish_test(test_name, failures, failure_log)
}

const UNIT_GOAL_ID: u32 = 0x170;
const UNIT_TARGET_TILE: u32 = 0x234;
const TILE_FLAGS_85: u32 = 0x85;
const ANIMAL_ESCAPED_FLAG: u32 = 0x3a3;
const ESCAPED_ANIMAL_LIST_RVA: u32 = 0x0023_8fb0;

fn centre_tile() -> (u32, u32) {
    let world = globals().ztworldmgr();
    (world.map_x_size / 2, world.map_y_size / 2)
}

/// Tiles a unit can stand on that resolve to a habitat (vanilla reads through a null habitat): gate tiles of habitats
/// and, when `world_tiles` is non-zero, that many tiles of the world habitat.
fn habitat_positions(world_tiles: usize) -> Vec<(u32, u32)> {
    let manager = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let mut found: Vec<(u32, u32)> = habitats_with_gate_tiles().into_iter().map(|(_, x, y)| (x as u32, y as u32)).collect();
    let mut world_found = 0;
    'scan: for y in (0..world.map_y_size).step_by(7) {
        for x in (0..world.map_x_size).step_by(5) {
            let habitat = manager.get_habitat_ptr(x as i32, y as i32);
            if habitat != 0 && get_from_memory::<u8>(habitat + 0x2c) != 0 {
                found.push((x, y));
                world_found += 1;
                if world_found == world_tiles {
                    break 'scan;
                }
            }
        }
    }
    found
}

/// `ZTSTAFF_DO_CONDITIONALS_LIVE`: `doConditionals` over every staff type for countdowns that only decrement, reload
/// on the predator goal and reload and ask `findPredator`; real body, port and hooked address agree on every byte of
/// the unit.
pub(crate) fn run_ztstaff_do_conditionals_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztstaff::DO_CONDITIONALS;

    let test_name = "ZTSTAFF_DO_CONDITIONALS_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = staff_types();
    if types.is_empty() {
        failures.push("the live world holds no staff type".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let shared = types.clone();
    let Some(&position) = habitat_positions(1).first() else {
        failures.push("no tile of the live map resolves to a habitat".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    for &staff_type in types.iter().take(4) {
        for (countdown, goal) in [(3i32, 0x10u32), (1, 0x10), (0, 0x75), (0, 0x10), (-1, 0x10), (0, 0)] {
            let staffs = [0, 1, 2].map(|_| live_support::build_standalone_staff(staff_type));
            for staff in staffs {
                place_at_tile(staff, position);
                save_to_memory(staff.0 + staff_offsets::CONDITIONALS_COUNTDOWN, countdown);
                save_to_memory(staff.0 + UNIT_GOAL_ID, goal);
            }
            real::do_conditionals(staffs[0].0);
            staffs[1].do_conditionals();
            unsafe { DO_CONDITIONALS.hooked()(staffs[2].0 as *const u32) };
            let label = format!("type {staff_type:#x}, countdown {countdown}, goal {goal:#x}");
            compare_memory(&format!("{label} (port)"), staffs[0], staffs[1], &shared, &mut failures);
            compare_memory(&format!("{label} (hooked)"), staffs[0], staffs[2], &shared, &mut failures);
            for staff in staffs {
                live_support::destroy_standalone_staff(staff);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// The `+0x85` bytes of `tiles` (null entries read as `0`).
fn tile_flag_bytes(tiles: &[u32]) -> Vec<u8> {
    tiles.iter().map(|&tile| if tile == 0 { 0 } else { get_from_memory::<u8>(tile + TILE_FLAGS_85) }).collect()
}

fn restore_tile_flag_bytes(tiles: &[u32], bytes: &[u8]) {
    for (&tile, &byte) in tiles.iter().zip(bytes) {
        if tile != 0 {
            save_to_memory(tile + TILE_FLAGS_85, byte);
        }
    }
}

/// `ZTSTAFF_SET_TARGET_TILE_LIVE`: `setTargetTile` for every combination of old target (none, a tile, the same tile), new
/// target and `flag`: the unit's bytes and the `+0x85` byte of every tile involved match the real body, and the hooked
/// address agrees.
pub(crate) fn run_ztstaff_set_target_tile_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztstaff::SET_TARGET_TILE;

    let test_name = "ZTSTAFF_SET_TARGET_TILE_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = ztkeeper::live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = staff_types();
    let world = globals().ztworldmgr();
    let tiles = [0u32, world.get_tile_ptr(2, 2), world.get_tile_ptr(10, 7)];
    let initial = tile_flag_bytes(&tiles);
    for &old in &tiles {
        for &new in &tiles {
            for flag in [false, true] {
                let staffs = [0, 1, 2].map(|_| live_support::build_standalone_staff(keeper_type));
                let mut after: Vec<Vec<u8>> = Vec::new();
                for (pole, staff) in staffs.iter().enumerate() {
                    save_to_memory(staff.0 + UNIT_TARGET_TILE, old);
                    if old != 0 {
                        save_to_memory(old + TILE_FLAGS_85, get_from_memory::<u8>(old + TILE_FLAGS_85) | 0x4);
                    }
                    match pole {
                        0 => real::set_target_tile(staff.0, new, flag),
                        1 => staff.set_target_tile(new, flag),
                        _ => unsafe { SET_TARGET_TILE.hooked()(staff.0 as *const u32, new as *const u32, flag) },
                    }
                    after.push(tile_flag_bytes(&tiles));
                    restore_tile_flag_bytes(&tiles, &initial);
                }
                let label = format!("old {old:#x}, new {new:#x}, flag {flag}");
                for pole in 1..3 {
                    if after[pole] != after[0] {
                        failures.push(format!("{label}: pole {pole} tile flags {:?}, real {:?}", after[pole], after[0]));
                    }
                    compare_memory(&format!("{label} (pole {pole})"), staffs[0], staffs[pole], &shared, &mut failures);
                }
                for staff in staffs {
                    live_support::destroy_standalone_staff(staff);
                }
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTSTAFF_PICK_RANDOM_DEST_LIVE`: `pickRandomDest` with the unit standing on habitat gate tiles (the pass-out branch),
/// on world tiles (the exit-gate branch and the `ZTUnit` fallback) and at the map's corners: the return's low byte, the
/// three output words, the shared RNG and the unit's bytes match the real body, and the hooked address agrees.
pub(crate) fn run_ztstaff_pick_random_dest_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztstaff::PICK_RANDOM_DEST;

    let test_name = "ZTSTAFF_PICK_RANDOM_DEST_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = ztkeeper::live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let shared = staff_types();
    let positions = habitat_positions(4);
    if habitats_with_gate_tiles().is_empty() {
        failures.push("the live world has no habitat with a gate tile, so the pass-out branch is uncovered".to_string());
    }
    for (index, &position) in positions.iter().enumerate() {
        for seed in [0x1234_5678u32, 0x0bad_cafe] {
            let staffs = [0, 1, 2].map(|_| live_support::build_standalone_staff(keeper_type));
            let mut results: Vec<(u32, [u32; 3], u32)> = Vec::new();
            for (pole, staff) in staffs.iter().enumerate() {
                place_at_tile(*staff, position);
                pin_rng(seed);
                let buffer = [0xdead_beefu32; 3];
                let address = buffer.as_ptr() as u32;
                let result = match pole {
                    0 => real::pick_random_dest(staff.0, address),
                    1 => staff.pick_random_dest(address),
                    _ => unsafe { PICK_RANDOM_DEST.hooked()(staff.0 as *const u32, address as *const glam::i32::IVec3) },
                };
                results.push((result & 0xff, [0, 1, 2].map(|i| get_from_memory::<u32>(address + i * 4)), rng()));
            }
            let label = format!("position {position:?} (#{index}), seed {seed:#x}");
            for pole in 1..3 {
                if results[pole] != results[0] {
                    failures.push(format!("{label}: pole {pole} (result, out, rng) {:x?}, real {:x?}", results[pole], results[0]));
                }
                compare_memory(&format!("{label} (pole {pole})"), staffs[0], staffs[pole], &shared, &mut failures);
            }
            for staff in staffs {
                live_support::destroy_standalone_staff(staff);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}

/// `ZTSTAFF_FENCES_OK_LIVE`: `fencesOK` over every tile of the live map in every direction (and `-1`), plus a null tile:
/// the result's low byte and both output bytes match the real body (the outputs are pre-filled so a missing clear shows),
/// and the hooked address agrees on a sample. Both a true and a false result must occur.
pub(crate) fn run_ztstaff_fences_ok_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::ztstaff::FENCES_OK;

    let test_name = "ZTSTAFF_FENCES_OK_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let Some(keeper_type) = ztkeeper::live_support::find_keeper_type() else {
        failures.push("the live world holds no keeper type".to_string());
        return finish_test(test_name, failures, failure_log);
    };
    let world = globals().ztworldmgr();
    let staff = live_support::build_standalone_staff(keeper_type);
    let mut seen = std::collections::BTreeSet::new();
    let mut mismatches = 0;
    let mut check = |tile: u32, direction: i32, sample: bool| {
        let (mut real_out, mut port_out, mut hooked_out) = ([0xaau8; 2], [0xaau8; 2], [0xaau8; 2]);
        let real_result = real::fences_ok(staff.0, tile, direction as u32, real_out.as_mut_ptr() as u32, unsafe { real_out.as_mut_ptr().add(1) } as u32) & 0xff;
        let port_result = staff.fences_ok(tile, direction, port_out.as_mut_ptr() as u32, unsafe { port_out.as_mut_ptr().add(1) } as u32) as u32;
        seen.insert((real_result, real_out));
        let mut bad = real_result != port_result || real_out != port_out;
        if sample {
            let hooked = unsafe {
                FENCES_OK.hooked()(staff.0 as *const u32, tile as *const u32, direction as u32, hooked_out.as_mut_ptr() as *const bool, hooked_out.as_mut_ptr().add(1) as *const bool)
            } & 0xff;
            bad |= hooked != real_result || hooked_out != real_out;
        }
        if bad {
            mismatches += 1;
            if mismatches <= 10 {
                failures.push(format!("tile {tile:#x}, direction {direction}: real ({real_result}, {real_out:?}), port ({port_result}, {port_out:?}), hooked out {hooked_out:?}"));
            }
        }
    };
    check(0, 3, true);
    for y in 0..world.map_y_size {
        for x in 0..world.map_x_size {
            let tile = world.get_tile_ptr(x, y);
            for direction in -1..8 {
                check(tile, direction, (x + y).wrapping_add(direction as u32) % 29 == 0);
            }
        }
    }
    if mismatches > 10 {
        failures.push(format!("{mismatches} mismatches in all"));
    }
    if !seen.iter().any(|(result, _)| *result == 0) || !seen.iter().any(|(result, _)| *result != 0) {
        failures.push(format!("the map produced only the outcomes {seen:?}"));
    }
    live_support::destroy_standalone_staff(staff);
    finish_test(test_name, failures, failure_log)
}

/// Up to `count` animal entities of the live world.
fn live_animal_entities(count: usize) -> Vec<u32> {
    let world = globals().ztworldmgr();
    let animal_arg = base() + RVA_ANIMAL_TYPE_CHECK;
    let mut found = Vec::new();
    let mut slot = world.entity_array_start();
    while slot != world.entity_array_end() && found.len() < count {
        let entity: u32 = get_from_memory(slot);
        slot += 4;
        if entity == 0 || get_from_memory::<u8>(entity + 0x142) != 0 || get_from_memory::<u8>(entity + 0x143) != 0 || get_from_memory::<u32>(entity + ENTITY_ID_OFFSET) == 0 {
            continue;
        }
        let ty: u32 = get_from_memory(entity + ENTITY_TYPE_OFFSET);
        if ty == 0 {
            continue;
        }
        let vtable: u32 = get_from_memory(ty);
        let check = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> bool>(get_from_memory::<u32>(vtable + 0x1c)) };
        if check(ty, animal_arg) {
            found.push(entity);
        }
    }
    found
}

/// `ZTSTAFF_FOUND_ESCAPED_ANIMAL_LIVE`: `foundEscapedAnimal` over a stand-in escaped-animal list holding live animals (the
/// global is swapped for the test and restored), with every animal flagged escaped or none, with a chaser id naming no
/// entity (free) or a live entity (taken), for each staff type: the result's low byte, the animals' `+0x374` words and the
/// unit's bytes match the real body, and the hooked address agrees. At least one animal must be capturable somewhere.
pub(crate) fn run_ztstaff_found_escaped_animal_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use openzt_detour::generated::{msvc_std_listuint::LIST_UINT_CTOR, ztstaff::FOUND_ESCAPED_ANIMAL};

    let test_name = "ZTSTAFF_FOUND_ESCAPED_ANIMAL_LIVE";
    let mut failures: Vec<String> = Vec::new();
    let types = staff_types();
    let animals = live_animal_entities(6);
    if types.is_empty() || animals.len() < 2 {
        failures.push("the live world holds no staff type or fewer than two animals".to_string());
        return finish_test(test_name, failures, failure_log);
    }
    let shared = types.clone();
    let saved: Vec<(u32, u8)> = animals.iter().map(|&a| (get_from_memory::<u32>(a + ANIMAL_CAPTURED_BY_OFFSET), get_from_memory::<u8>(a + ANIMAL_ESCAPED_FLAG))).collect();
    let mut holder = 0u32;
    unsafe { LIST_UINT_CTOR.original()(&mut holder as *mut u32 as *const std::ffi::c_void, 0) };
    let sentinel = holder;
    let list_slot = base() + ESCAPED_ANIMAL_LIST_RVA;
    let original_list: u32 = get_from_memory(list_slot);
    save_to_memory(list_slot, sentinel);

    let live_id: u32 = get_from_memory(animals[animals.len() - 1] + ENTITY_ID_OFFSET);
    let mut positives = 0;
    // (list holds the animals, animals flagged escaped, chaser of the first animal)
    for (listed, flagged, first_chaser) in [(false, true, 0u32), (true, true, 0), (true, true, 0x7fff_0000), (true, true, live_id), (true, false, 0)] {
        if listed {
            for &animal in &animals {
                vanilla_list::push_back(sentinel, animal);
            }
        }
        for &staff_type in &types {
            for position in [centre_tile(), (2, 2)] {
                let staffs = [0, 1, 2].map(|_| live_support::build_standalone_staff(staff_type));
                let mut outcomes: Vec<(u32, Vec<u32>)> = Vec::new();
                for (pole, staff) in staffs.iter().enumerate() {
                    for (index, &animal) in animals.iter().enumerate() {
                        save_to_memory(animal + ANIMAL_CAPTURED_BY_OFFSET, if index == 0 { first_chaser } else { 0u32 });
                        save_to_memory(animal + ANIMAL_ESCAPED_FLAG, flagged as u8);
                    }
                    place_at_tile(*staff, position);
                    let result = match pole {
                        0 => real::found_escaped_animal(staff.0),
                        1 => staff.found_escaped_animal() as u32,
                        _ => unsafe { FOUND_ESCAPED_ANIMAL.hooked()(staff.0 as *const u32) },
                    } & 0xff;
                    outcomes.push((result, animals.iter().map(|&a| get_from_memory::<u32>(a + ANIMAL_CAPTURED_BY_OFFSET)).collect()));
                }
                positives += (outcomes[0].0 != 0) as u32;
                let label = format!("listed {listed}, flagged {flagged}, chaser {first_chaser:#x}, type {staff_type:#x}, position {position:?}");
                for pole in 1..3 {
                    if outcomes[pole] != outcomes[0] {
                        failures.push(format!("{label}: pole {pole} {:x?}, real {:x?}", outcomes[pole], outcomes[0]));
                    }
                    compare_memory(&format!("{label} (pole {pole})"), staffs[0], staffs[pole], &shared, &mut failures);
                }
                for staff in staffs {
                    live_support::destroy_standalone_staff(staff);
                }
            }
        }
        vanilla_list::clear(sentinel);
    }
    for (&animal, &(chaser, flag)) in animals.iter().zip(&saved) {
        save_to_memory(animal + ANIMAL_CAPTURED_BY_OFFSET, chaser);
        save_to_memory(animal + ANIMAL_ESCAPED_FLAG, flag);
    }
    save_to_memory(list_slot, original_list);
    unsafe { POOLALLOC_DEALLOCATE.original()(sentinel as *const u32, 0xc) };
    if positives == 0 {
        failures.push("no animal was capturable by any staff type, so the claiming branch is uncovered".to_string());
    }
    finish_test(test_name, failures, failure_log)
}
