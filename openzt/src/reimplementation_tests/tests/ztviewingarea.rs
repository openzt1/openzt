//! Compares real vanilla `ZTViewingArea` leaf methods against `zthabitat/viewing_area.rs` on
//! heap-backed fake viewing areas (read-only, Rust-owned memory; nothing is handed to vanilla's allocator).

use std::io::Write;

use tracing::error;

use crate::globals::globals;
use crate::reimplementation_tests::harness::write_success_line;
use crate::util::{get_from_memory, ref_from_memory, save_to_memory};
use crate::zthabitat::viewing_area::{self, live_support, live_support::test_real};
use crate::zthabitatmgr::{walk_tile_list, ZTHabitat};

fn finish(failure_log: &mut Option<std::fs::File>, test_name: &str, failures: Vec<String>) -> bool {
    if failures.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        for msg in &failures {
            error!("{}: {}", test_name, msg);
        }
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, failures.join("; ")).as_bytes());
        }
        true
    }
}

/// `ZTVIEWINGAREA_DETOURS_ENABLED` - every detour installed by `viewing_area::init()` reports enabled.
pub(crate) fn run_ztviewingarea_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let disabled: Vec<&'static str> = live_support::detour_status().into_iter().filter(|(_, on)| !on).map(|(n, _)| n).collect();
    let failures = if disabled.is_empty() { vec![] } else { vec![format!("detours not enabled: {disabled:?}")] };
    finish(failure_log, "ZTVIEWINGAREA_DETOURS_ENABLED", failures)
}

/// `ZTVIEWINGAREA_LEAVES_MATCH_REAL` - extents, `hasOA` and `updateAmbients` agree with real vanilla.
pub(crate) fn run_ztviewingarea_leaves_match_real_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let mut failures = Vec::new();
    let tile_sets: [&[(i32, i32)]; 4] = [&[(5, 7)], &[(3, 9), (-2, 4), (8, 4)], &[(0, 0), (0, 0)], &[(10, -5), (2, 6), (7, -9), (4, 4)]];

    for tiles in tile_sets {
        let fake = live_support::FakeViewingArea::new(tiles, 0, 0, 0, false);
        let ptr = fake.ptr();
        let (real_ew, port_ew) = (live_support::test_real::get_ew_extent(ptr), viewing_area::get_ew_extent(ptr));
        let (real_ns, port_ns) = (live_support::test_real::get_ns_extent(ptr), viewing_area::get_ns_extent(ptr));
        if real_ew != port_ew {
            failures.push(format!("ew {tiles:?}: real={real_ew:?} port={port_ew:?}"));
        }
        if real_ns != port_ns {
            failures.push(format!("ns {tiles:?}: real={real_ns:?} port={port_ns:?}"));
        }
    }

    for oa_count in [0usize, 1, 3] {
        let fake = live_support::FakeViewingArea::new(&[(1, 1)], oa_count, 0, 0, false);
        let (real, port) = (live_support::test_real::has_oa(fake.ptr()), viewing_area::has_oa(fake.ptr()));
        if real != port || port != (oa_count != 0) {
            failures.push(format!("has_oa oa_count={oa_count}: real={real} port={port}"));
        }
    }

    for (guests, with_ambients) in [(0, false), (5, false), (0, true), (5, true)] {
        let real_fake = live_support::FakeViewingArea::new(&[(1, 1)], 0, 0, guests, with_ambients);
        let port_fake = live_support::FakeViewingArea::new(&[(1, 1)], 0, 0, guests, with_ambients);
        live_support::test_real::update_ambients(real_fake.ptr(), 33);
        viewing_area::update_ambients(port_fake.ptr(), 33);
        let (a, b) = (real_fake.snapshot(), port_fake.snapshot());
        // Pointer fields differ between the two heap blocks; compare only the scalar ones.
        for idx in [0x4c / 4, 0x54 / 4] {
            if a[idx] != b[idx] {
                failures.push(format!("update_ambients guests={guests} amb={with_ambients}: word {idx:#x} real={:#x} port={:#x}", a[idx], b[idx]));
            }
        }
    }

    finish(failure_log, "ZTVIEWINGAREA_LEAVES_MATCH_REAL", failures)
}

#[derive(Debug, PartialEq, Eq)]
struct GridState {
    cells: Vec<u32>,
    tile_flags: Vec<u8>,
    tiles_in_vector: Vec<u32>,
    recreate_oa_dirty: u8,
    characteristics_dirty: u8,
}

/// `(cell address, tile pointer)` for every grid position within three steps of `(x, y)`, the furthest
/// `addTile` walks.
fn neighbourhood(x: i32, y: i32) -> Vec<(u32, u32)> {
    let mgr = globals().zthabitatmgr();
    let world = globals().ztworldmgr();
    let mut out = Vec::new();
    for ny in (y - 3)..=(y + 3) {
        for nx in (x - 3)..=(x + 3) {
            if nx < 0 || ny < 0 || nx >= world.map_x_size as i32 || ny >= world.map_y_size as i32 {
                continue;
            }
            if let Some(cell) = mgr.get_habitat_cell_addr(nx, ny) {
                out.push((cell, world.get_tile_ptr(nx as u32, ny as u32)));
            }
        }
    }
    out
}

/// Snapshot of the grid cells and tile flags in `region`, plus the viewing area's own vector and dirty bytes
/// when `area` is non-zero (`0` captures the grid only).
fn capture(area: u32, region: &[(u32, u32)]) -> GridState {
    let (begin, end) = if area == 0 { (0, 0) } else { (get_from_memory::<u32>(area + 0x40), get_from_memory::<u32>(area + 0x44)) };
    GridState {
        cells: region.iter().flat_map(|&(cell, _)| (0..10u32).map(move |w| get_from_memory::<u32>(cell + w * 4))).collect(),
        tile_flags: region.iter().map(|&(_, tile)| get_from_memory::<u8>(tile + 0x85)).collect(),
        tiles_in_vector: (begin..end).step_by(4).map(get_from_memory::<u32>).collect(),
        recreate_oa_dirty: if area == 0 { 0 } else { get_from_memory::<u8>(area + 0x25) },
        characteristics_dirty: if area == 0 { 0 } else { get_from_memory::<u8>(area + 0x4c) },
    }
}

fn restore(region: &[(u32, u32)], state: &GridState) {
    for (i, &(cell, tile)) in region.iter().enumerate() {
        for w in 0..10usize {
            save_to_memory(cell + w as u32 * 4, state.cells[i * 10 + w]);
        }
        save_to_memory(tile + 0x85, state.tile_flags[i]);
    }
}

/// `ZTVIEWINGAREA_GRID_WRITERS_MATCH_REAL_LIVE` - for owned tiles of live habitats, runs `addTile`, then
/// `clearTileOfVA`, then `addTile` + `removeTile` on one synthetic viewing area through real vanilla and
/// through the port, each from the same snapshot of the grid cells and tile flags within three steps, and
/// compares the resulting state after every step. The grid is restored afterwards. The viewing area's own
/// tile-vector buffers come from vanilla's allocator and are deliberately leaked.
pub(crate) fn run_ztviewingarea_grid_writers_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTVIEWINGAREA_GRID_WRITERS_MATCH_REAL_LIVE";
    let mgr = globals().zthabitatmgr();
    let mut failures = Vec::new();
    let mut cases = 0;

    let habitats: Vec<u32> = (0..mgr.exhibit_array().len()).map(|i| mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).take(3).collect();
    for habitat_ptr in habitats {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let tiles: Vec<u32> = walk_tile_list(*habitat.owned_tiles_ptr())
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
            .step_by(7)
            .take(3)
            .collect();
        for tile in tiles {
            let (x, y) = (get_from_memory::<i32>(tile + 0x34), get_from_memory::<i32>(tile + 0x38));
            let region = neighbourhood(x, y);
            let mut area_block = Box::new([0u32; 0x5c / 4]);
            let area = area_block.as_mut_ptr() as u32;
            let base_state = {
                area_block[0] = habitat_ptr;
                capture(area, &region)
            };

            let mut run_pole = |real: bool| -> Vec<GridState> {
                restore(&region, &base_state);
                for w in area_block.iter_mut() {
                    *w = 0;
                }
                area_block[0] = habitat_ptr;
                let mut states = Vec::new();
                if real {
                    test_real::add_tile(area, tile);
                } else {
                    viewing_area::add_tile(area, tile);
                }
                states.push(capture(area, &region));
                if real {
                    test_real::clear_tile_of_va(area, tile);
                } else {
                    viewing_area::clear_tile_of_va(area, tile);
                }
                states.push(capture(area, &region));
                if real {
                    test_real::add_tile(area, tile);
                    test_real::remove_tile(area, tile);
                } else {
                    viewing_area::add_tile(area, tile);
                    viewing_area::remove_tile(area, tile);
                }
                states.push(capture(area, &region));
                restore(&region, &base_state);
                states
            };

            let real_states = run_pole(true);
            let port_states = run_pole(false);
            cases += 1;
            for (step, (r, p)) in real_states.iter().zip(&port_states).enumerate() {
                if r != p {
                    failures.push(format!("habitat {habitat_ptr:#x} tile ({x},{y}) step {step}: real={r:?} port={p:?}"));
                }
            }
            // Sanity: the first add must actually have written a slot, or the case proves nothing.
            if real_states[0].cells == base_state.cells {
                failures.push(format!("habitat {habitat_ptr:#x} tile ({x},{y}): addTile wrote no grid slot (vacuous case)"));
            }
        }
    }

    if cases == 0 {
        write_success_line(failure_log, &format!("{test_name} (skipped: no live habitat tiles)"));
        return false;
    }
    finish(failure_log, test_name, failures)
}

/// Fake entity type objects answer the `+0x1c` cast slot by comparing the tag with the game's own
/// `CAST_ZTGuest`/`CAST_ZTGuide` addresses.
extern "thiscall" fn fake_cast(this: u32, tag: u32) -> bool {
    let kind = get_from_memory::<u32>(this + 4);
    let base = crate::globals::get_module_base("zoo.exe") as u32;
    (kind == 1 && tag == base + 0x238700) || (kind == 2 && tag == base + 0x238770)
}

#[derive(Clone, Copy)]
enum FakeKind {
    Guest,
    Guide(u32),
    Other,
}

/// Rust-owned tiles, entity lists, entities and type objects, wired the way `recalculateCharacteristics`
/// walks them. Nothing here is ever freed by vanilla.
struct FakeEntityWorld {
    _keep: Vec<Box<[u32; 0x200 / 4]>>,
    _vtable: Box<[u32; 16]>,
    tiles: Vec<u32>,
}

impl FakeEntityWorld {
    fn new(per_tile_kinds: &[&[FakeKind]]) -> Self {
        let mut vtable = Box::new([0u32; 16]);
        vtable[0x1c / 4] = fake_cast as *const () as usize as u32;
        let vtable_ptr = vtable.as_ptr() as u32;
        let mut keep: Vec<Box<[u32; 0x200 / 4]>> = Vec::new();
        let mut tiles = Vec::new();
        let new_block = |keep: &mut Vec<Box<[u32; 0x200 / 4]>>| -> u32 {
            let mut b = Box::new([0u32; 0x200 / 4]);
            let p = b.as_mut_ptr() as u32;
            keep.push(b);
            p
        };
        for kinds in per_tile_kinds {
            let tile = new_block(&mut keep);
            let sentinel = new_block(&mut keep);
            let mut prev = sentinel;
            let mut first = sentinel;
            for kind in kinds.iter() {
                let type_object = new_block(&mut keep);
                save_to_memory(type_object, vtable_ptr);
                match kind {
                    FakeKind::Guest => save_to_memory(type_object + 4, 1u32),
                    FakeKind::Guide(v) => {
                        save_to_memory(type_object + 4, 2u32);
                        save_to_memory(type_object + 0x1f8, *v);
                    }
                    FakeKind::Other => save_to_memory(type_object + 4, 3u32),
                }
                let entity = new_block(&mut keep);
                save_to_memory(entity + 0x128, type_object);
                let node = new_block(&mut keep);
                save_to_memory(node + 8, entity);
                if prev == sentinel {
                    first = node;
                } else {
                    save_to_memory(prev, node);
                }
                prev = node;
            }
            save_to_memory(sentinel, first);
            if prev != sentinel {
                save_to_memory(prev, sentinel);
            }
            save_to_memory(tile, sentinel);
            tiles.push(tile);
        }
        Self { _keep: keep, _vtable: vtable, tiles }
    }
}

/// `ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS_MATCHES_REAL` - on fake entity lists (guests, guides, other
/// entities, several tiles) compares guest count, guide value, dirty bytes and the observable-area counters
/// between real vanilla and the port. Both poles run vanilla's own `recalculateOA` on an empty observable
/// area, so no real game state is touched.
pub(crate) fn run_ztviewingarea_recalculate_characteristics_test(failure_log: &mut Option<std::fs::File>) -> bool {
    use FakeKind::{Guest, Guide, Other};
    let cases: Vec<Vec<Vec<FakeKind>>> = vec![
        vec![vec![]],
        vec![vec![Guest, Guest, Other]],
        vec![vec![Guide(5)], vec![Guest]],
        vec![vec![Guide(0), Guide(5), Guide(7)], vec![Guest, Guide(9)]],
        vec![vec![Other], vec![Guest], vec![Guest, Guest], vec![Guide(3), Guide(4)]],
    ];
    let mut failures = Vec::new();
    for (index, case) in cases.iter().enumerate() {
        let kinds: Vec<&[FakeKind]> = case.iter().map(Vec::as_slice).collect();
        let world = FakeEntityWorld::new(&kinds);
        let tile_vector: Vec<u32> = world.tiles.clone();
        let observe = |real: bool| -> Vec<u32> {
            let mut block = Box::new([0u32; 0x5c / 4]);
            block[0x40 / 4] = tile_vector.as_ptr() as u32;
            block[0x44 / 4] = tile_vector.as_ptr() as u32 + 4 * tile_vector.len() as u32;
            block[0x4c / 4] = 1;
            block[0x54 / 4] = 0xdead;
            block[0x58 / 4] = 0xbeef;
            let ptr = block.as_ptr() as u32;
            if real {
                test_real::recalculate_characteristics(ptr);
            } else {
                viewing_area::recalculate_characteristics(ptr);
            }
            // dirty bytes (0x24/0x25/0x4c share words with neighbours), counters, guests, guide value.
            let mut out = vec![block[0x10 / 4], block[0x14 / 4], block[0x18 / 4], block[0x1c / 4], block[0x24 / 4], block[0x54 / 4], block[0x58 / 4]];
            out.push(get_from_memory::<u8>(ptr + 0x4c) as u32);
            out
        };
        let (real, port) = (observe(true), observe(false));
        if real != port {
            failures.push(format!("case {index}: real={real:x?} port={port:x?}"));
        }
    }
    finish(failure_log, "ZTVIEWINGAREA_RECALCULATE_CHARACTERISTICS_MATCHES_REAL", failures)
}

/// `ZTOBSERVABLEAREA_ADD_TILE_MATCHES_REAL` - repeated and duplicate adds into a fresh observable area give
/// the same tile vector under real vanilla and the port, across enough adds to force several regrowths.
/// The vectors' buffers come from vanilla's allocator and are leaked.
pub(crate) fn run_ztobservablearea_add_tile_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let tiles: Vec<u32> = (1..=40u32).map(|i| 0x1000 + i * 0x8c).collect();
    let sequence: Vec<u32> = tiles.iter().copied().chain([tiles[0], tiles[7], tiles[39]]).collect();
    let run = |real: bool| -> Vec<u32> {
        let mut oa = Box::new([0u32; 0x3c / 4]);
        let ptr = oa.as_mut_ptr() as u32;
        for &t in &sequence {
            if real {
                test_real::observable_area_add_tile(ptr, t);
            } else {
                viewing_area::observable_area_add_tile(ptr, t);
            }
        }
        let (begin, end) = (get_from_memory::<u32>(ptr), get_from_memory::<u32>(ptr + 4));
        (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
    };
    let (real, port) = (run(true), run(false));
    let failures = if real != port || real.len() != tiles.len() { vec![format!("real={real:x?} port={port:x?}")] } else { vec![] };
    finish(failure_log, "ZTOBSERVABLEAREA_ADD_TILE_MATCHES_REAL", failures)
}

/// `ZTVIEWINGAREA_CREATE_OA_MATCHES_REAL_LIVE` - for live habitats, builds a synthetic viewing area over a
/// few of the habitat's own tiles (close together, so their ranges overlap and the claim-once removal
/// matters) and compares the rebuilt observable-area tile vector and the dirty byte between real vanilla and
/// the port. A case that observes no tile at all is reported as vacuous.
pub(crate) fn run_ztviewingarea_create_oa_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTVIEWINGAREA_CREATE_OA_MATCHES_REAL_LIVE";
    let mgr = globals().zthabitatmgr();
    let mut failures = Vec::new();
    let mut cases = 0;
    let mut observed_any = false;

    let habitats: Vec<u32> = (0..mgr.exhibit_array().len()).map(|i| mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).take(4).collect();
    for habitat_ptr in habitats {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let tiles: Vec<u32> = walk_tile_list(*habitat.owned_tiles_ptr())
            .map(|node| get_from_memory::<u32>(node + 0x8))
            .filter(|&t| t != 0)
            .take(4)
            .collect();
        if tiles.is_empty() {
            continue;
        }
        let run = |real: bool| -> (Vec<u32>, u32) {
            let mut block = Box::new([0u32; 0x5c / 4]);
            block[0] = habitat_ptr;
            block[0x40 / 4] = tiles.as_ptr() as u32;
            block[0x44 / 4] = tiles.as_ptr() as u32 + 4 * tiles.len() as u32;
            let ptr = block.as_ptr() as u32;
            if real {
                test_real::create_oa(ptr);
            } else {
                viewing_area::create_oa(ptr);
            }
            let (begin, end) = (get_from_memory::<u32>(ptr + 4), get_from_memory::<u32>(ptr + 8));
            ((begin..end).step_by(4).map(get_from_memory::<u32>).collect(), get_from_memory::<u8>(ptr + 0x24) as u32)
        };
        let (real, port) = (run(true), run(false));
        cases += 1;
        observed_any |= !real.0.is_empty();
        if real != port {
            failures.push(format!("habitat {habitat_ptr:#x}: real={real:x?} port={port:x?}"));
        }
    }
    if cases == 0 {
        write_success_line(failure_log, &format!("{test_name} (skipped: no live habitat tiles)"));
        return false;
    }
    if !observed_any {
        failures.push("every case observed no tile (vacuous)".to_string());
    }
    finish(failure_log, test_name, failures)
}

extern "thiscall" fn fake_animal_cast(this: u32, tag: u32) -> bool {
    get_from_memory::<u32>(this + 4) == 1 && tag == crate::globals::get_module_base("zoo.exe") as u32 + 0x238690
}

extern "thiscall" fn fake_species_id(this: u32) -> i32 {
    get_from_memory::<i32>(this + 8)
}

struct AnimalSpec {
    id: u32,
    species: i32,
    flag_a: bool,
    flag_b: bool,
}

struct Outcome {
    counters: [u32; 4],
    species_ids: Vec<u32>,
    seen_ids: Vec<u32>,
    weights: Vec<[i32; 3]>,
    rng: u32,
    dirty_bytes: [u8; 2],
}

fn vector_contents(vector_ptr: u32) -> Vec<u32> {
    let (begin, end) = (get_from_memory::<u32>(vector_ptr), get_from_memory::<u32>(vector_ptr + 4));
    (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
}

impl PartialEq for Outcome {
    fn eq(&self, other: &Self) -> bool {
        self.counters == other.counters
            && self.species_ids == other.species_ids
            && self.seen_ids == other.seen_ids
            && self.weights == other.weights
            && self.rng == other.rng
            && self.dirty_bytes == other.dirty_bytes
    }
}

impl std::fmt::Debug for Outcome {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "counters={:?} species={:?} seen={:?} weights={:?} rng={:#x} dirty={:?}",
            self.counters, self.species_ids, self.seen_ids, self.weights, self.rng, self.dirty_bytes
        )
    }
}

/// Builds a viewing area + observable area from Rust-owned fakes, then runs one pole of
/// `recalculateOA`. Observable tiles are copies of real world tiles (so `BFMap` sees real positions) whose
/// entity list is replaced by fake animals; viewing-area tiles are real world tiles, read only.
/// `lists[i]` holds indices into `animals` for observable tile `i`; an animal listed twice is one shared
/// entity, exercising the by-id de-duplication.
fn run_recalculate_oa_pole(real: bool, animals: &[AnimalSpec], lists: &[&[usize]], viewing_positions: &[(u32, u32)], guests: i32) -> Outcome {
    let world = globals().ztworldmgr();
    let base = crate::globals::get_module_base("zoo.exe") as u32;
    let mut keep: Vec<Box<[u32; 0x400 / 4]>> = Vec::new();
    let block = |keep: &mut Vec<Box<[u32; 0x400 / 4]>>| -> u32 {
        let mut b = Box::new([0u32; 0x400 / 4]);
        let p = b.as_mut_ptr() as u32;
        keep.push(b);
        p
    };
    let mut vtable = Box::new([0u32; 16]);
    vtable[0x1c / 4] = fake_animal_cast as *const () as usize as u32;
    vtable[0x20 / 4] = fake_species_id as *const () as usize as u32;
    let vtable_ptr = vtable.as_ptr() as u32;

    let entities: Vec<u32> = animals
        .iter()
        .map(|a| {
            let type_object = block(&mut keep);
            save_to_memory(type_object, vtable_ptr);
            save_to_memory(type_object + 4, 1u32);
            save_to_memory(type_object + 8, a.species);
            let entity = block(&mut keep);
            save_to_memory(entity + 0x124, a.id);
            save_to_memory(entity + 0x128, type_object);
            save_to_memory(entity + 0x3aa, a.flag_a as u8);
            save_to_memory(entity + 0x3a7, a.flag_b as u8);
            entity
        })
        .collect();

    let mut oa_tiles: Vec<u32> = Vec::new();
    for (i, list) in lists.iter().enumerate() {
        let tile = block(&mut keep);
        let real_tile = world.get_tile_ptr(5 + i as u32, 5);
        for w in 0..0x8c / 4 {
            save_to_memory(tile + w * 4, get_from_memory::<u32>(real_tile + w * 4));
        }
        let sentinel = block(&mut keep);
        let mut previous = sentinel;
        let mut first = sentinel;
        for &index in *list {
            let node = block(&mut keep);
            save_to_memory(node + 8, entities[index]);
            if previous == sentinel {
                first = node;
            } else {
                save_to_memory(previous, node);
            }
            previous = node;
        }
        save_to_memory(sentinel, first);
        if previous != sentinel {
            save_to_memory(previous, sentinel);
        }
        save_to_memory(tile, sentinel);
        oa_tiles.push(tile);
    }

    let viewing_tiles: Vec<u32> = viewing_positions.iter().map(|&(x, y)| world.get_tile_ptr(x, y)).collect();
    let mut va = Box::new([0u32; 0x5c / 4]);
    let va_ptr = va.as_mut_ptr() as u32;
    va[0x20 / 4] = va_ptr;
    va[0x40 / 4] = viewing_tiles.as_ptr() as u32;
    va[0x44 / 4] = viewing_tiles.as_ptr() as u32 + 4 * viewing_tiles.len() as u32;
    va[0x54 / 4] = guests as u32;
    let oa = va_ptr + 4;
    va[1] = oa_tiles.as_ptr() as u32;
    va[2] = oa_tiles.as_ptr() as u32 + 4 * oa_tiles.len() as u32;
    va[0x24 / 4] = 1; // observable-area dirty-characteristics byte (OA + 0x20)

    let rng_addr = base + crate::zthabitat::support::GAME_RNG_RVA;
    save_to_memory(rng_addr, 0x1234_5678u32);
    if real {
        test_real::recalculate_observable_area(oa);
    } else {
        viewing_area::recalculate_observable_area(oa);
    }

    Outcome {
        counters: [0xc, 0x10, 0x14, 0x18].map(|o| get_from_memory::<u32>(oa + o)),
        species_ids: vector_contents(oa + 0x24),
        seen_ids: vector_contents(oa + 0x30),
        weights: entities.iter().map(|&e| [0x368, 0x36c, 0x370].map(|o| get_from_memory::<i32>(e + o))).collect(),
        rng: get_from_memory::<u32>(rng_addr),
        dirty_bytes: [get_from_memory::<u8>(oa + 0x20), get_from_memory::<u8>(oa + 0x21)],
    }
}

/// `ZTVIEWINGAREA_RECALCULATE_OA_MATCHES_REAL` - real vanilla vs the port on Rust-owned fake animals spread
/// over several observable tiles (shared entities, repeated species, both flags, several guest counts and
/// viewing-tile layouts, near and far enough for the distance decay to matter). Compares the four counters,
/// the species-id and seen-animal vectors, every animal's three accumulators, the RNG state and the dirty
/// bytes. The game RNG is reseeded before each pole and left reseeded afterwards.
pub(crate) fn run_ztviewingarea_recalculate_oa_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let world = globals().ztworldmgr();
    let far = (world.map_x_size.saturating_sub(2).min(40), world.map_y_size.saturating_sub(2).min(40));
    let animals = [
        AnimalSpec { id: 11, species: 3, flag_a: true, flag_b: false },
        AnimalSpec { id: 12, species: 3, flag_a: false, flag_b: true },
        AnimalSpec { id: 13, species: 9, flag_a: true, flag_b: true },
        AnimalSpec { id: 14, species: 4, flag_a: false, flag_b: false },
    ];
    let layouts: [(&[&[usize]], &[(u32, u32)]); 3] = [
        (&[&[0, 1], &[1, 2], &[3]], &[(5, 5)]),
        (&[&[0], &[2, 3], &[]], &[(5, 5), far]),
        (&[&[0, 1, 2, 3], &[0, 1, 2, 3], &[0]], &[(7, 9), (6, 5), far]),
    ];
    let mut failures = Vec::new();
    let mut vacuous = true;
    for (layout_index, (lists, viewing)) in layouts.iter().enumerate() {
        for guests in [0, 1, 7, 40] {
            let real = run_recalculate_oa_pole(true, &animals, lists, viewing, guests);
            let port = run_recalculate_oa_pole(false, &animals, lists, viewing, guests);
            if guests > 0 && real.counters[1] > 0 && real.weights.iter().any(|w| w[0] != 0) {
                vacuous = false;
            }
            if real != port {
                failures.push(format!("layout {layout_index} guests {guests}: real={real:?} port={port:?}"));
            }
        }
    }
    if vacuous {
        failures.push("no case produced a non-zero weight (vacuous)".to_string());
    }
    finish(failure_log, "ZTVIEWINGAREA_RECALCULATE_OA_MATCHES_REAL", failures)
}

#[derive(Debug, PartialEq, Eq)]
struct LifecycleOutcome {
    habitat: u32,
    seed_in_zoo_fields: Vec<u32>,
    dirty_bytes: [u8; 3],
    tiles: Vec<u32>,
    observable_tiles: Vec<u32>,
    vector_words: Vec<u32>,
    ambients: Option<(u32, [i32; 3])>,
    grid_after_construct: GridState,
    grid_cells_after_destruct: Vec<u32>,
    flags_after_destruct: Vec<u8>,
}

/// Normalises a captured grid so the two poles' differing viewing-area addresses compare equal.
fn normalise_grid(mut state: GridState, area: u32) -> GridState {
    for word in state.cells.iter_mut() {
        if *word == area {
            *word = 1;
        }
    }
    state
}

fn run_lifecycle_pole(real: bool, habitat: u32, tile: u32, region: &[(u32, u32)], base: &GridState) -> LifecycleOutcome {
    use openzt_detour::generated::standalone::{OPERATOR_DELETE, OPERATOR_NEW};
    restore(region, base);
    let area = unsafe { OPERATOR_NEW.original()(0x5c) } as u32;
    for word in 0..0x5c / 4 {
        save_to_memory(area + word * 4, 0xAAAA_AAAAu32);
    }
    if real {
        test_real::construct(area, habitat, tile);
    } else {
        viewing_area::construct(area, habitat, tile);
    }
    let ambients_ptr = get_from_memory::<u32>(area + 0x50);
    let grid_after_construct = normalise_grid(capture(area, region), area);
    let outcome_fields = LifecycleOutcome {
        habitat: get_from_memory::<u32>(area),
        seed_in_zoo_fields: vec![get_from_memory::<u32>(area + 0x20) == area, get_from_memory::<u32>(area + 0x20 + 0x0) == get_from_memory::<u32>(area + 0x4 + 0x1c)]
            .into_iter()
            .map(u32::from)
            .collect(),
        dirty_bytes: [get_from_memory::<u8>(area + 0x24), get_from_memory::<u8>(area + 0x25), get_from_memory::<u8>(area + 0x4c)],
        tiles: vector_contents(area + 0x40),
        observable_tiles: vector_contents(area + 0x04),
        // species-id and seen-animal vectors (viewing area +0x28..+0x40) are emptied by the constructor.
        vector_words: (0x28..0x40u32).step_by(4).map(|o| get_from_memory::<u32>(area + o)).collect(),
        ambients: (ambients_ptr != 0).then(|| {
            let group_count = (get_from_memory::<u32>(ambients_ptr + 4) - get_from_memory::<u32>(ambients_ptr)) / 4;
            (group_count, [get_from_memory::<i32>(ambients_ptr + 0xc), get_from_memory::<i32>(ambients_ptr + 0x10), get_from_memory::<i32>(ambients_ptr + 0x14)])
        }),
        grid_after_construct,
        grid_cells_after_destruct: Vec::new(),
        flags_after_destruct: Vec::new(),
    };
    if real {
        test_real::destruct(area);
    } else {
        viewing_area::destruct(area);
    }
    let after = capture(0, region);
    unsafe { OPERATOR_DELETE.original()(area) };
    let outcome = LifecycleOutcome { grid_cells_after_destruct: after.cells.clone(), flags_after_destruct: after.tile_flags.clone(), ..outcome_fields };
    restore(region, base);
    outcome
}

/// `ZTVIEWINGAREA_CONSTRUCT_DESTRUCT_MATCHES_REAL_LIVE` - builds a viewing area over the first owned tile of
/// live habitats through real vanilla and through the port (each from the same grid snapshot, on a block of
/// uninitialised-pattern memory), compares every field the constructor defines, the `Ambients` it built, the
/// tile and observable-tile vectors and the grid slots it wrote, then destroys each with its own pole's
/// destructor and compares the grid afterwards (which must be back to the snapshot's slots). Everything is
/// allocated and freed by vanilla's allocators on both poles, so nothing is leaked.
pub(crate) fn run_ztviewingarea_lifecycle_live_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "ZTVIEWINGAREA_CONSTRUCT_DESTRUCT_MATCHES_REAL_LIVE";
    let mgr = globals().zthabitatmgr();
    let mut failures = Vec::new();
    let mut cases = 0;
    let mut saw_ambients = false;

    let habitats: Vec<u32> = (0..mgr.exhibit_array().len()).map(|i| mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).take(3).collect();
    for habitat_ptr in habitats {
        let habitat = unsafe { ref_from_memory::<ZTHabitat>(habitat_ptr) };
        let tile = walk_tile_list(*habitat.owned_tiles_ptr()).map(|node| get_from_memory::<u32>(node + 0x8)).find(|&t| t != 0);
        let Some(tile) = tile else { continue };
        let (x, y) = (get_from_memory::<i32>(tile + 0x34), get_from_memory::<i32>(tile + 0x38));
        let region = neighbourhood(x, y);
        let base = capture(0, &region);
        let real = run_lifecycle_pole(true, habitat_ptr, tile, &region, &base);
        let port = run_lifecycle_pole(false, habitat_ptr, tile, &region, &base);
        cases += 1;
        saw_ambients |= real.ambients.is_some();
        if real != port {
            failures.push(format!("habitat {habitat_ptr:#x} tile ({x},{y}): real={real:?} port={port:?}"));
        }
        if real.grid_cells_after_destruct != base.cells || port.grid_cells_after_destruct != base.cells {
            failures.push(format!("habitat {habitat_ptr:#x}: grid slots not restored by the destructor"));
        }
    }
    if cases == 0 {
        write_success_line(failure_log, &format!("{test_name} (skipped: no live habitat tiles)"));
        return false;
    }
    if !saw_ambients {
        failures.push("no case built an Ambients object (vacuous)".to_string());
    }
    finish(failure_log, test_name, failures)
}
