//! The two per-habitat viewing-area containers: the `ZTViewingArea*` vector (`+0x34`/`+0x38`/`+0x3c`) and
//! the edge-tile list (`+0x44`, a `list<uint>` of `BFTile*` that `ZTHabitat::createViewingAreas` rebuilds).
//!
//! Both live in Rust-side stores keyed by the `ZTHabitat`'s own address. Vanilla's own fields stay in the
//! state the constructor leaves them in (empty vector, empty list sentinel), which is also what every
//! vanilla reader sees, so the struct layout and vanilla's own teardown of those fields stay valid. Every
//! vanilla reader of either container is ported (`getNearestViewingArea`/`getRandomViewingArea`/
//! `getViewingAreasWithGuests`/`update`/`getCloseOutsideTile`/...), and `ZTTankExhibit::setTankHeight`'s
//! own loop over the vector is `ZTHabitat::recreate_oas` inside its full port.
//!
//! The `ZTViewingArea` objects themselves stay vanilla (constructed and destroyed through vanilla's own
//! constructor/destructor); the store only holds their pointers. Nothing Rust-allocated is ever handed to
//! vanilla.

use std::{
    collections::HashMap,
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

type StoreByHabitat = HashMap<u32, Vec<u32>>;

static VIEWING_AREAS: LazyLock<Mutex<StoreByHabitat>> = LazyLock::new(|| Mutex::new(HashMap::new()));
static EDGE_TILES: LazyLock<Mutex<StoreByHabitat>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn viewing_area_store() -> MutexGuard<'static, StoreByHabitat> {
    VIEWING_AREAS.lock().unwrap_or_else(PoisonError::into_inner)
}

fn edge_tile_store() -> MutexGuard<'static, StoreByHabitat> {
    EDGE_TILES.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Test-only switch routing every accessor below to vanilla's own raw memory instead of the Rust stores,
/// so a live test can run real vanilla (whose own inlined reads and writes see vanilla memory, and whose
/// callbacks into our detours must see the same memory) against the port. Only set through
/// [`with_vanilla_viewing_areas`].
#[cfg(feature = "reimplementation-tests")]
static VANILLA_VIEWING_AREA_BACKEND: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[cfg(feature = "reimplementation-tests")]
fn vanilla_backend() -> bool {
    VANILLA_VIEWING_AREA_BACKEND.load(std::sync::atomic::Ordering::Relaxed)
}

#[cfg(not(feature = "reimplementation-tests"))]
const fn vanilla_backend() -> bool {
    false
}

/// Runs `f` with the vanilla-memory backend active (see [`VANILLA_VIEWING_AREA_BACKEND`]). Containers
/// touched under it must be created, used and emptied under it too.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn with_vanilla_viewing_areas<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            VANILLA_VIEWING_AREA_BACKEND.store(self.0, std::sync::atomic::Ordering::Relaxed);
        }
    }
    let _restore = Restore(VANILLA_VIEWING_AREA_BACKEND.swap(true, std::sync::atomic::Ordering::Relaxed));
    f()
}

/// Copy of `habitat`'s viewing-area pointers, in insertion order. A copy, so a caller can call into vanilla
/// (which may re-enter a detour that mutates the container) while walking it.
pub fn viewing_areas(habitat: u32) -> Vec<u32> {
    if vanilla_backend() {
        return vanilla::viewing_areas(habitat);
    }
    viewing_area_store().get(&habitat).cloned().unwrap_or_default()
}

pub fn viewing_area_count(habitat: u32) -> usize {
    if vanilla_backend() {
        return vanilla::viewing_areas(habitat).len();
    }
    viewing_area_store().get(&habitat).map_or(0, Vec::len)
}

pub fn push_viewing_area(habitat: u32, viewing_area: u32) {
    if vanilla_backend() {
        return vanilla::push_viewing_area(habitat, viewing_area);
    }
    viewing_area_store().entry(habitat).or_default().push(viewing_area);
}

/// Removes the first entry equal to `viewing_area`, shifting later entries down. `false` if absent.
pub fn remove_viewing_area_entry(habitat: u32, viewing_area: u32) -> bool {
    if vanilla_backend() {
        return vanilla::remove_viewing_area_entry(habitat, viewing_area);
    }
    let mut store = viewing_area_store();
    let Some(entries) = store.get_mut(&habitat) else {
        return false;
    };
    match entries.iter().position(|&entry| entry == viewing_area) {
        Some(index) => {
            entries.remove(index);
            true
        }
        None => false,
    }
}

/// Empties the vector, keeping its backing storage (vanilla's `removeViewingAreas` resets `end` to `begin`).
pub fn clear_viewing_areas(habitat: u32) {
    if vanilla_backend() {
        return vanilla::clear_viewing_areas(habitat);
    }
    if let Some(entries) = viewing_area_store().get_mut(&habitat) {
        entries.clear();
    }
}

/// Removes and returns `habitat`'s viewing-area pointers (the habitat is being destroyed).
pub fn take_viewing_areas(habitat: u32) -> Vec<u32> {
    if vanilla_backend() {
        return vanilla::viewing_areas(habitat);
    }
    viewing_area_store().remove(&habitat).unwrap_or_default()
}

/// Copy of `habitat`'s edge-tile list (`BFTile*` addresses, in list order).
pub fn edge_tiles(habitat: u32) -> Vec<u32> {
    if vanilla_backend() {
        return vanilla::edge_tiles(habitat);
    }
    edge_tile_store().get(&habitat).cloned().unwrap_or_default()
}

pub fn set_edge_tiles(habitat: u32, tiles: Vec<u32>) {
    if vanilla_backend() {
        return vanilla::set_edge_tiles(habitat, &tiles);
    }
    edge_tile_store().insert(habitat, tiles);
}

/// Removes `habitat`'s edge-tile list (the habitat is being destroyed).
pub fn take_edge_tiles(habitat: u32) -> Vec<u32> {
    if vanilla_backend() {
        return vanilla::edge_tiles(habitat);
    }
    edge_tile_store().remove(&habitat).unwrap_or_default()
}

/// Every habitat address with an entry in either store, for the stale-entry live test.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn store_keys() -> Vec<u32> {
    let mut keys: Vec<u32> = viewing_area_store().keys().chain(edge_tile_store().keys()).copied().collect();
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Runs `f` with `habitat`'s store contents mirrored into vanilla's own raw containers and the vanilla
/// backend active, so a real-vs-port comparison over a live habitat gives real vanilla the same data the
/// port reads from the store. The mirror is torn down (buffers freed through vanilla's allocator, the list
/// emptied through vanilla's `erase`) before returning. Anything `f` writes to the containers is discarded
/// with it, so use it for read-only calls; mutating scenarios run whole under [`with_vanilla_viewing_areas`].
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn with_vanilla_mirror<R>(habitat: u32, f: impl FnOnce() -> R) -> R {
    let areas = viewing_areas(habitat);
    let edges = edge_tiles(habitat);
    with_vanilla_viewing_areas(|| {
        for &area in &areas {
            vanilla::push_viewing_area(habitat, area);
        }
        vanilla::set_edge_tiles(habitat, &edges);
        let result = f();
        vanilla::release(habitat);
        result
    })
}

/// Returns `habitat`'s raw vanilla containers to the constructor's empty state (a no-op when they already are).
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn release_vanilla_containers(habitat: u32) {
    with_vanilla_viewing_areas(|| vanilla::release(habitat));
}

/// Direct access to vanilla's own memory for the two containers; only reachable under
/// [`with_vanilla_viewing_areas`].
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod vanilla {
    use openzt_detour::generated::msvc_std_listuint::{ERASE as MSVC_LIST_UINT_ERASE, INSERT as MSVC_LIST_UINT_INSERT};

    use crate::{
        util::{get_from_memory, save_to_memory},
        vanilla_vector,
        zthabitat::support::{free_event_vector_buffer, walk_tile_list},
    };

    const VECTOR_OFFSET: u32 = 0x34;
    const EDGE_LIST_OFFSET: u32 = 0x44;

    pub(crate) fn viewing_areas(habitat: u32) -> Vec<u32> {
        let begin: u32 = get_from_memory(habitat + VECTOR_OFFSET);
        let end: u32 = get_from_memory(habitat + VECTOR_OFFSET + 4);
        (begin..end).step_by(4).map(get_from_memory::<u32>).collect()
    }

    pub(crate) fn push_viewing_area(habitat: u32, viewing_area: u32) {
        vanilla_vector::push_word(habitat + VECTOR_OFFSET, viewing_area, free_event_vector_buffer);
    }

    pub(crate) fn remove_viewing_area_entry(habitat: u32, viewing_area: u32) -> bool {
        let begin: u32 = get_from_memory(habitat + VECTOR_OFFSET);
        let end: u32 = get_from_memory(habitat + VECTOR_OFFSET + 4);
        match (begin..end).step_by(4).find(|&cursor| get_from_memory::<u32>(cursor) == viewing_area) {
            Some(cursor) => {
                vanilla_vector::erase_element(habitat + VECTOR_OFFSET, cursor, 4);
                true
            }
            None => false,
        }
    }

    pub(crate) fn clear_viewing_areas(habitat: u32) {
        let begin: u32 = get_from_memory(habitat + VECTOR_OFFSET);
        save_to_memory(habitat + VECTOR_OFFSET + 4, begin);
    }

    pub(crate) fn edge_tiles(habitat: u32) -> Vec<u32> {
        let sentinel: u32 = get_from_memory(habitat + EDGE_LIST_OFFSET);
        walk_tile_list(sentinel).map(|node| get_from_memory::<u32>(node + 8)).collect()
    }

    /// Returns both raw containers to the empty constructor state: the vector's buffer goes back to the
    /// pool it came from and the list is emptied through vanilla's `erase`.
    pub(crate) fn release(habitat: u32) {
        let begin: u32 = get_from_memory(habitat + VECTOR_OFFSET);
        let cap_end: u32 = get_from_memory(habitat + VECTOR_OFFSET + 8);
        if begin != 0 {
            free_event_vector_buffer(begin, cap_end - begin);
        }
        for word in 0..3 {
            save_to_memory::<u32>(habitat + VECTOR_OFFSET + word * 4, 0);
        }
        set_edge_tiles(habitat, &[]);
    }

    /// Empties vanilla's list with its own `list<uint>::erase` and re-inserts `tiles` before the sentinel
    /// through vanilla's own `insert`, so every node stays on vanilla's allocator.
    pub(crate) fn set_edge_tiles(habitat: u32, tiles: &[u32]) {
        let list_ptr = habitat + EDGE_LIST_OFFSET;
        let sentinel: u32 = get_from_memory(list_ptr);
        let first: u32 = get_from_memory(sentinel);
        let mut erase_result: i32 = 0;
        unsafe {
            MSVC_LIST_UINT_ERASE.original()(
                list_ptr as *const std::ffi::c_void,
                &mut erase_result as *mut i32 as *const i32,
                first as *const i32,
                sentinel as *const i32,
            )
        };
        for &tile in tiles {
            let mut insert_result: i32 = 0;
            let value = tile as i32;
            unsafe { MSVC_LIST_UINT_INSERT.original()(&mut insert_result as *mut i32 as *const i32, sentinel as i32, &value as *const i32) };
        }
    }
}

#[cfg(not(feature = "reimplementation-tests"))]
mod vanilla {
    pub(crate) fn viewing_areas(_habitat: u32) -> Vec<u32> {
        unreachable!("the vanilla backend is test-only")
    }
    pub(crate) fn push_viewing_area(_habitat: u32, _viewing_area: u32) {
        unreachable!("the vanilla backend is test-only")
    }
    pub(crate) fn remove_viewing_area_entry(_habitat: u32, _viewing_area: u32) -> bool {
        unreachable!("the vanilla backend is test-only")
    }
    pub(crate) fn clear_viewing_areas(_habitat: u32) {
        unreachable!("the vanilla backend is test-only")
    }
    pub(crate) fn edge_tiles(_habitat: u32) -> Vec<u32> {
        unreachable!("the vanilla backend is test-only")
    }
    pub(crate) fn set_edge_tiles(_habitat: u32, _tiles: &[u32]) {
        unreachable!("the vanilla backend is test-only")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const HABITAT: u32 = 0x1000_0001;
    const OTHER: u32 = 0x1000_0002;

    #[test]
    fn viewing_area_store_round_trip() {
        assert!(viewing_areas(HABITAT).is_empty());
        push_viewing_area(HABITAT, 1);
        push_viewing_area(HABITAT, 2);
        push_viewing_area(HABITAT, 3);
        assert_eq!(viewing_areas(HABITAT), vec![1, 2, 3]);
        assert_eq!(viewing_area_count(HABITAT), 3);
        assert!(remove_viewing_area_entry(HABITAT, 2));
        assert!(!remove_viewing_area_entry(HABITAT, 2));
        assert_eq!(viewing_areas(HABITAT), vec![1, 3]);
        clear_viewing_areas(HABITAT);
        assert!(viewing_areas(HABITAT).is_empty());
        push_viewing_area(HABITAT, 9);
        assert_eq!(take_viewing_areas(HABITAT), vec![9]);
        assert!(viewing_areas(HABITAT).is_empty());
    }

    #[test]
    fn edge_tile_store_is_per_habitat() {
        set_edge_tiles(OTHER, vec![7, 8, 7]);
        assert_eq!(edge_tiles(OTHER), vec![7, 8, 7]);
        assert!(edge_tiles(HABITAT + 100).is_empty());
        set_edge_tiles(OTHER, vec![5]);
        assert_eq!(take_edge_tiles(OTHER), vec![5]);
        assert!(edge_tiles(OTHER).is_empty());
    }
}
