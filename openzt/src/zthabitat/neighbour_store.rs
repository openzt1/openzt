//! Rust-owned storage for a `ZTHabitat`'s three tree containers: the amphibious-neighbour set (`+0x8`), the
//! show-neighbour set (`+0x14`) and the show-portal map (`+0x20`).
//!
//! Vanilla keeps each as a `std::set`/`std::map` inside the habitat; the reimplementation keeps them in a side
//! store keyed by habitat address instead (the same shape as the boundary-pair, viewing-area and suitability
//! stores), created on first insert and removed by [`remove_habitat`] from `ZTHabitat::destruct`. Vanilla's
//! three header nodes stay in the habitat untouched and empty.
//!
//! Every accessor here has two backends. The production backend is the store. Live tests run real vanilla
//! bodies, which read and write the raw trees and call back into our detours, so
//! [`with_vanilla_neighbour_sets`] routes every accessor to the raw trees (through [`crate::vanilla_rbtree`])
//! for the duration of a closure, the same arrangement as `support::with_vanilla_suitability_maps`.
//! [`rebuild_raw_from_store`] and [`load_store_from_raw`] move state between the two before and after a real
//! pole.
//!
//! Reads return owned snapshots: the store sits behind a `Mutex`, and a lazy borrow could not outlive a
//! re-entrant call into another accessor.
//!
//! `ZTHabitat` reaches the three containers only through this module. The raw trees in the habitat stay empty, so
//! vanilla code that has not been ported and still reads them would see no neighbours; the live tests rebuild
//! them from the store around each real vanilla call for exactly that reason (see
//! [`with_real_neighbour_state`]).

use std::{
    collections::{BTreeMap, BTreeSet, HashMap},
    sync::{LazyLock, Mutex, MutexGuard, PoisonError},
};

use crate::util::{get_from_memory, save_to_memory};
use crate::vanilla_rbtree::{
    rb_find, rb_map_erase, rb_map_find_or_insert, rb_set_erase, rb_set_insert, rb_tree_clear, set_members, MAP_NODE_SIZE, SET_NODE_SIZE,
};

/// Offset of the amphibious-neighbour `{head*, size}` container inside a `ZTHabitat`.
const AMPHIBIOUS_OFFSET: u32 = 0x8;
/// Offset of the show-neighbour container.
const SHOW_OFFSET: u32 = 0x14;
/// Offset of the show-portal map container; the mapped value (a `ZTFence*`) is at node `+0x14`.
const PORTAL_OFFSET: u32 = 0x20;
const PORTAL_VALUE_OFFSET: u32 = 0x14;

/// Which of a habitat's two `std::set<ZTHabitat*>` containers an operation targets.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum NeighbourSet {
    Amphibious,
    Show,
}

impl NeighbourSet {
    fn container_offset(self) -> u32 {
        match self {
            NeighbourSet::Amphibious => AMPHIBIOUS_OFFSET,
            NeighbourSet::Show => SHOW_OFFSET,
        }
    }
}

#[derive(Default)]
struct NeighbourSets {
    amphibious: BTreeSet<u32>,
    show: BTreeSet<u32>,
    portals: BTreeMap<u32, u32>,
}

impl NeighbourSets {
    fn set(&self, which: NeighbourSet) -> &BTreeSet<u32> {
        match which {
            NeighbourSet::Amphibious => &self.amphibious,
            NeighbourSet::Show => &self.show,
        }
    }

    fn set_mut(&mut self, which: NeighbourSet) -> &mut BTreeSet<u32> {
        match which {
            NeighbourSet::Amphibious => &mut self.amphibious,
            NeighbourSet::Show => &mut self.show,
        }
    }
}

static NEIGHBOUR_SETS: LazyLock<Mutex<HashMap<u32, NeighbourSets>>> = LazyLock::new(|| Mutex::new(HashMap::new()));

fn store() -> MutexGuard<'static, HashMap<u32, NeighbourSets>> {
    NEIGHBOUR_SETS.lock().unwrap_or_else(PoisonError::into_inner)
}

#[cfg(any(test, feature = "reimplementation-tests"))]
thread_local! {
    static VANILLA_BACKEND: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}

#[cfg(any(test, feature = "reimplementation-tests"))]
fn vanilla_backend() -> bool {
    VANILLA_BACKEND.with(std::cell::Cell::get)
}

#[cfg(not(any(test, feature = "reimplementation-tests")))]
const fn vanilla_backend() -> bool {
    false
}

/// Runs `f` with every accessor routed to the habitats' raw vanilla trees (see the module docs). Thread-local
/// and nesting-safe; the previous setting is restored even if `f` panics.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) fn with_vanilla_neighbour_sets<R>(f: impl FnOnce() -> R) -> R {
    struct Restore(bool);
    impl Drop for Restore {
        fn drop(&mut self) {
            VANILLA_BACKEND.with(|flag| flag.set(self.0));
        }
    }
    let _restore = Restore(VANILLA_BACKEND.with(|flag| flag.replace(true)));
    f()
}

/// Offset of `ZTHabitat::neighbor_dirty`. The portal map's size dword (`+0x24`) spans `+0x24..+0x28`, so this byte
/// is bits 8-15 of it.
const NEIGHBOR_DIRTY_OFFSET: u32 = 0x25;

/// Empties the raw portal map the way vanilla's tree `clear()` does, but keeps `neighbor_dirty`. The clear zeroes
/// the whole size dword, which overlaps that flag (see [`NEIGHBOR_DIRTY_OFFSET`]); a test mirroring the store
/// into the raw map must not change the flag as a side effect.
fn clear_raw_portals(habitat: u32) {
    let dirty: u8 = get_from_memory(habitat + NEIGHBOR_DIRTY_OFFSET);
    rb_tree_clear(habitat + PORTAL_OFFSET, MAP_NODE_SIZE);
    save_to_memory(habitat + NEIGHBOR_DIRTY_OFFSET, dirty);
}

/// Inserts `other` into `habitat`'s `which` set; returns whether it was newly inserted.
pub fn insert(habitat: u32, which: NeighbourSet, other: u32) -> bool {
    if vanilla_backend() {
        return rb_set_insert(habitat + which.container_offset(), other);
    }
    store().entry(habitat).or_default().set_mut(which).insert(other)
}

/// Removes `other` from `habitat`'s `which` set; an absent key is a no-op.
pub fn remove(habitat: u32, which: NeighbourSet, other: u32) {
    if vanilla_backend() {
        rb_set_erase(habitat + which.container_offset(), other);
        return;
    }
    if let Some(sets) = store().get_mut(&habitat) {
        sets.set_mut(which).remove(&other);
    }
}

/// Empties `habitat`'s `which` set.
pub fn clear(habitat: u32, which: NeighbourSet) {
    if vanilla_backend() {
        rb_tree_clear(habitat + which.container_offset(), SET_NODE_SIZE);
        return;
    }
    if let Some(sets) = store().get_mut(&habitat) {
        sets.set_mut(which).clear();
    }
}

/// A snapshot of `habitat`'s `which` set in ascending key order (vanilla's in-order walk).
pub fn members(habitat: u32, which: NeighbourSet) -> Vec<u32> {
    if vanilla_backend() {
        return set_members(get_from_memory(habitat + which.container_offset())).collect();
    }
    store().get(&habitat).map(|sets| sets.set(which).iter().copied().collect()).unwrap_or_default()
}

/// Whether `other` is in `habitat`'s `which` set.
pub fn contains(habitat: u32, which: NeighbourSet, other: u32) -> bool {
    if vanilla_backend() {
        return rb_find(habitat + which.container_offset(), other).is_some();
    }
    store().get(&habitat).is_some_and(|sets| sets.set(which).contains(&other))
}

/// The number of members of `habitat`'s `which` set. Under the vanilla backend this is the raw size word the
/// vanilla code itself reads.
pub fn len(habitat: u32, which: NeighbourSet) -> u32 {
    if vanilla_backend() {
        return get_from_memory(habitat + which.container_offset() + 4);
    }
    store().get(&habitat).map_or(0, |sets| sets.set(which).len() as u32)
}

/// The fence stored in `habitat`'s show-portal map for `neighbour`, if there is an entry.
pub fn portal(habitat: u32, neighbour: u32) -> Option<u32> {
    if vanilla_backend() {
        return rb_find(habitat + PORTAL_OFFSET, neighbour).map(|node| get_from_memory(node + PORTAL_VALUE_OFFSET));
    }
    store().get(&habitat).and_then(|sets| sets.portals.get(&neighbour).copied())
}

/// Stores `fence` for `neighbour` in `habitat`'s show-portal map, inserting the entry when absent.
pub fn set_portal(habitat: u32, neighbour: u32, fence: u32) {
    if vanilla_backend() {
        let node = rb_map_find_or_insert(habitat + PORTAL_OFFSET, neighbour);
        save_to_memory(node + PORTAL_VALUE_OFFSET, fence);
        return;
    }
    store().entry(habitat).or_default().portals.insert(neighbour, fence);
}

/// Removes `habitat`'s show-portal entry for `neighbour`; an absent key is a no-op.
pub fn remove_portal(habitat: u32, neighbour: u32) {
    if vanilla_backend() {
        rb_map_erase(habitat + PORTAL_OFFSET, neighbour);
        return;
    }
    if let Some(sets) = store().get_mut(&habitat) {
        sets.portals.remove(&neighbour);
    }
}

/// Empties `habitat`'s show-portal map.
pub fn clear_portals(habitat: u32) {
    if vanilla_backend() {
        clear_raw_portals(habitat);
        return;
    }
    if let Some(sets) = store().get_mut(&habitat) {
        sets.portals.clear();
    }
}

/// A snapshot of `habitat`'s show-portal map as `(neighbour, fence)` pairs in ascending key order.
pub fn portals(habitat: u32) -> Vec<(u32, u32)> {
    if vanilla_backend() {
        let head: u32 = get_from_memory(habitat + PORTAL_OFFSET);
        return crate::vanilla_rbtree::walk_nodes(head)
            .map(|node| (get_from_memory(node + 0x10), get_from_memory(node + PORTAL_VALUE_OFFSET)))
            .collect();
    }
    store().get(&habitat).map(|sets| sets.portals.iter().map(|(&key, &fence)| (key, fence)).collect()).unwrap_or_default()
}

/// Drops `habitat`'s store entry. Called by `ZTHabitat::destruct`; a handle left behind would be inherited by
/// a later habitat allocated at the same address.
pub fn remove_habitat(habitat: u32) {
    store().remove(&habitat);
}

/// Every habitat address that currently has a store entry, for the live no-stale-entries test.
pub fn store_handles() -> Vec<u32> {
    store().keys().copied().collect()
}

/// Which of a habitat's three containers an operation covers. Containers whose production code has not been
/// moved to the store yet (see [`FLIPPED`]) keep their state in the raw trees, so mirroring them from the store
/// would wipe it.
#[cfg(any(test, feature = "reimplementation-tests"))]
#[derive(Clone, Copy)]
pub(crate) struct Scope {
    pub amphibious: bool,
    pub show: bool,
    pub portals: bool,
}

/// Every container.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) const ALL: Scope = Scope { amphibious: true, show: true, portals: true };

/// The containers production code reads and writes through the store today. Widen this as each stage of the
/// ownership flip lands.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) const FLIPPED: Scope = Scope { amphibious: true, show: true, portals: true };

/// Rewrites `habitat`'s raw vanilla trees covered by `scope` to match the store, so a real vanilla pole reads
/// the state the port holds. Operates on the raw trees regardless of the active backend.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) fn rebuild_raw_from_store_scoped(habitat: u32, scope: Scope) {
    let (amphibious, show, portals) = {
        let guard = store();
        match guard.get(&habitat) {
            Some(sets) => (sets.amphibious.clone(), sets.show.clone(), sets.portals.clone()),
            None => Default::default(),
        }
    };
    for (which, keys, wanted) in [(NeighbourSet::Amphibious, amphibious, scope.amphibious), (NeighbourSet::Show, show, scope.show)] {
        if !wanted {
            continue;
        }
        rb_tree_clear(habitat + which.container_offset(), SET_NODE_SIZE);
        for key in keys {
            rb_set_insert(habitat + which.container_offset(), key);
        }
    }
    if scope.portals {
        clear_raw_portals(habitat);
        for (neighbour, fence) in portals {
            let node = rb_map_find_or_insert(habitat + PORTAL_OFFSET, neighbour);
            save_to_memory(node + PORTAL_VALUE_OFFSET, fence);
        }
    }
}

/// [`rebuild_raw_from_store_scoped`] over every container.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) fn rebuild_raw_from_store(habitat: u32) {
    rebuild_raw_from_store_scoped(habitat, ALL);
}

/// Replaces the parts of `habitat`'s store entry covered by `scope` with the contents of its raw vanilla trees,
/// so the port reads the state a real vanilla mutator just produced. Operates on the raw trees regardless of
/// the active backend.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) fn load_store_from_raw_scoped(habitat: u32, scope: Scope) {
    // A habitat whose tree container was never initialised (a hand-built test fixture) has a null head and no members.
    let read_set = |which: NeighbourSet| -> BTreeSet<u32> {
        let head: u32 = get_from_memory(habitat + which.container_offset());
        if head == 0 { BTreeSet::new() } else { set_members(head).collect() }
    };
    let mut guard = store();
    let sets = guard.entry(habitat).or_default();
    if scope.amphibious {
        sets.amphibious = read_set(NeighbourSet::Amphibious);
    }
    if scope.show {
        sets.show = read_set(NeighbourSet::Show);
    }
    if scope.portals {
        let head: u32 = get_from_memory(habitat + PORTAL_OFFSET);
        sets.portals = if head == 0 {
            BTreeMap::new()
        } else {
            crate::vanilla_rbtree::walk_nodes(head).map(|node| (get_from_memory(node + 0x10), get_from_memory(node + PORTAL_VALUE_OFFSET))).collect()
        };
    }
}

/// [`load_store_from_raw_scoped`] over every container.
#[cfg(any(test, feature = "reimplementation-tests"))]
pub(crate) fn load_store_from_raw(habitat: u32) {
    load_store_from_raw_scoped(habitat, ALL);
}

/// Runs `f` (a call into real vanilla code) with every live habitat's raw trees mirroring the store and every
/// accessor routed to the raw trees, then empties the raw trees again. Real vanilla bodies read the raw trees
/// directly and call back into our detours, which now read through the accessors, so both must see the same
/// state. The store is not touched by anything `f` does through the accessors; a test that compares the state a
/// real mutator left behind must read it inside an outer `with_real_neighbour_state` closure, because the
/// outermost call clears the raw trees on exit. Nested calls reuse the outer mirror.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn with_real_neighbour_state<R>(f: impl FnOnce() -> R) -> R {
    run_with_real_neighbour_state(false, f)
}

/// [`with_real_neighbour_state`], but the outermost call copies the raw trees back into the store before
/// emptying them, so what a real mutator did persists for the steps that follow. For tests that apply a
/// sequence of operations cumulatively to a graph driven by real vanilla.
#[cfg(feature = "reimplementation-tests")]
pub(crate) fn with_real_neighbour_state_committed<R>(f: impl FnOnce() -> R) -> R {
    run_with_real_neighbour_state(true, f)
}

#[cfg(feature = "reimplementation-tests")]
fn run_with_real_neighbour_state<R>(commit: bool, f: impl FnOnce() -> R) -> R {
    thread_local! {
        static DEPTH: std::cell::Cell<u32> = const { std::cell::Cell::new(0) };
    }
    struct Leave;
    impl Drop for Leave {
        fn drop(&mut self) {
            DEPTH.with(|depth| depth.set(depth.get() - 1));
        }
    }
    let live_habitats = || -> Vec<u32> {
        let mgr = crate::globals::globals().zthabitatmgr();
        let mut habitats: Vec<u32> = (0..mgr.exhibit_array().len()).map(|i| mgr.exhibit_array().get_ptr(i)).filter(|&p| p != 0).collect();
        habitats.extend(store_handles());
        habitats.sort_unstable();
        habitats.dedup();
        habitats
    };
    let outermost = DEPTH.with(|depth| depth.get()) == 0;
    if outermost {
        for habitat in live_habitats() {
            rebuild_raw_from_store_scoped(habitat, FLIPPED);
        }
    }
    DEPTH.with(|depth| depth.set(depth.get() + 1));
    let _leave = Leave;
    let result = with_vanilla_neighbour_sets(f);
    if outermost {
        for habitat in live_habitats() {
            if commit {
                load_store_from_raw_scoped(habitat, FLIPPED);
            }
        }
        with_vanilla_neighbour_sets(|| {
            for habitat in live_habitats() {
                if FLIPPED.amphibious {
                    clear(habitat, NeighbourSet::Amphibious);
                }
                if FLIPPED.show {
                    clear(habitat, NeighbourSet::Show);
                }
                if FLIPPED.portals {
                    clear_portals(habitat);
                }
            }
        });
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Leaks a zeroed 0x40-byte habitat prefix with three empty vanilla trees (red head, no root, leftmost and
    /// rightmost pointing back at the head) at the three container offsets. Returns its address; unique per call,
    /// so tests sharing the global store never collide.
    fn fake_habitat() -> u32 {
        let block: &'static mut [u8] = Box::leak(vec![0u8; 0x40].into_boxed_slice());
        let habitat = block.as_ptr() as u32;
        for offset in [AMPHIBIOUS_OFFSET, SHOW_OFFSET, PORTAL_OFFSET] {
            let head_block: &'static mut [u8] = Box::leak(vec![0u8; MAP_NODE_SIZE as usize].into_boxed_slice());
            let head = head_block.as_ptr() as u32;
            save_to_memory(habitat + offset, head);
            save_to_memory(head + 0x8, head);
            save_to_memory(head + 0xc, head);
        }
        habitat
    }

    struct Rng(u32);
    impl Rng {
        fn next(&mut self) -> u32 {
            self.0 ^= self.0 << 13;
            self.0 ^= self.0 >> 17;
            self.0 ^= self.0 << 5;
            self.0
        }
    }

    fn snapshot(habitat: u32) -> (Vec<u32>, Vec<u32>, Vec<(u32, u32)>, u32, u32) {
        (
            members(habitat, NeighbourSet::Amphibious),
            members(habitat, NeighbourSet::Show),
            portals(habitat),
            len(habitat, NeighbourSet::Amphibious),
            len(habitat, NeighbourSet::Show),
        )
    }

    #[test]
    fn missing_entry_reads_empty_and_creates_nothing() {
        let habitat = fake_habitat();
        assert!(members(habitat, NeighbourSet::Show).is_empty());
        assert!(!contains(habitat, NeighbourSet::Amphibious, 0x10));
        assert_eq!(len(habitat, NeighbourSet::Amphibious), 0);
        assert_eq!(portal(habitat, 0x10), None);
        remove(habitat, NeighbourSet::Show, 0x10);
        clear(habitat, NeighbourSet::Amphibious);
        clear_portals(habitat);
        assert!(!store_handles().contains(&habitat), "reads and removals must not create an entry");
    }

    #[test]
    fn insert_reports_novelty_and_members_are_sorted() {
        let habitat = fake_habitat();
        assert!(insert(habitat, NeighbourSet::Amphibious, 0x30));
        assert!(insert(habitat, NeighbourSet::Amphibious, 0x10));
        assert!(!insert(habitat, NeighbourSet::Amphibious, 0x30));
        assert!(insert(habitat, NeighbourSet::Show, 0x20));
        assert_eq!(members(habitat, NeighbourSet::Amphibious), vec![0x10, 0x30]);
        assert_eq!(members(habitat, NeighbourSet::Show), vec![0x20]);
        assert_eq!(len(habitat, NeighbourSet::Amphibious), 2);
        assert!(contains(habitat, NeighbourSet::Amphibious, 0x10));
        assert!(!contains(habitat, NeighbourSet::Show, 0x10));
        remove_habitat(habitat);
    }

    #[test]
    fn remove_habitat_drops_the_entry() {
        let habitat = fake_habitat();
        insert(habitat, NeighbourSet::Show, 0x10);
        set_portal(habitat, 0x10, 0x99);
        assert!(store_handles().contains(&habitat));
        remove_habitat(habitat);
        assert!(!store_handles().contains(&habitat));
        assert!(members(habitat, NeighbourSet::Show).is_empty());
        assert_eq!(portal(habitat, 0x10), None);
    }

    #[test]
    fn portal_map_set_get_overwrite_remove() {
        let habitat = fake_habitat();
        set_portal(habitat, 0x20, 0xaa);
        set_portal(habitat, 0x10, 0xbb);
        set_portal(habitat, 0x20, 0xcc);
        assert_eq!(portal(habitat, 0x20), Some(0xcc));
        assert_eq!(portals(habitat), vec![(0x10, 0xbb), (0x20, 0xcc)]);
        remove_portal(habitat, 0x20);
        remove_portal(habitat, 0x77);
        assert_eq!(portals(habitat), vec![(0x10, 0xbb)]);
        clear_portals(habitat);
        assert!(portals(habitat).is_empty());
        remove_habitat(habitat);
    }

    /// Applies the same random operation sequence to a store-backed habitat and, under the vanilla backend, to
    /// a second habitat's raw trees, and requires identical snapshots after every step.
    #[test]
    fn store_and_vanilla_backends_agree_over_random_operations() {
        for seed in [3u32, 0xfeed_beef, 0x2468_ace0] {
            let mut rng = Rng(seed);
            let via_store = fake_habitat();
            let via_raw = fake_habitat();
            for step in 0..1500 {
                let key = 0x1000 + (rng.next() % 41) * 4;
                let op = rng.next() % 12;
                let value = rng.next();
                let apply = |habitat: u32| match op {
                    0..=2 => {
                        insert(habitat, NeighbourSet::Amphibious, key);
                    }
                    3..=4 => {
                        insert(habitat, NeighbourSet::Show, key);
                    }
                    5 => remove(habitat, NeighbourSet::Amphibious, key),
                    6 => remove(habitat, NeighbourSet::Show, key),
                    7..=8 => set_portal(habitat, key, value),
                    9 => remove_portal(habitat, key),
                    10 => {
                        if rng_clear(step) {
                            clear(habitat, NeighbourSet::Amphibious);
                            clear(habitat, NeighbourSet::Show);
                            clear_portals(habitat);
                        }
                    }
                    _ => {}
                };
                apply(via_store);
                with_vanilla_neighbour_sets(|| apply(via_raw));
                let raw_snapshot = with_vanilla_neighbour_sets(|| snapshot(via_raw));
                assert_eq!(snapshot(via_store), raw_snapshot, "step {step} op {op}");
                for which in [NeighbourSet::Amphibious, NeighbourSet::Show] {
                    let raw_has = with_vanilla_neighbour_sets(|| contains(via_raw, which, key));
                    assert_eq!(contains(via_store, which, key), raw_has, "contains step {step}");
                }
                assert_eq!(portal(via_store, key), with_vanilla_neighbour_sets(|| portal(via_raw, key)), "portal step {step}");
            }
            remove_habitat(via_store);
            with_vanilla_neighbour_sets(|| {
                clear(via_raw, NeighbourSet::Amphibious);
                clear(via_raw, NeighbourSet::Show);
                clear_portals(via_raw);
            });
        }
    }

    /// Clearing every few hundred steps keeps the random walk from sitting at a fixed size.
    fn rng_clear(step: u32) -> bool {
        step % 211 == 0
    }

    #[test]
    fn rebuild_raw_from_store_then_load_store_from_raw_round_trips() {
        let habitat = fake_habitat();
        for key in [0x50, 0x10, 0x30] {
            insert(habitat, NeighbourSet::Amphibious, key);
        }
        insert(habitat, NeighbourSet::Show, 0x20);
        set_portal(habitat, 0x20, 0xdead);
        set_portal(habitat, 0x10, 0xbeef);
        let before = snapshot(habitat);

        rebuild_raw_from_store(habitat);
        assert_eq!(with_vanilla_neighbour_sets(|| snapshot(habitat)), before, "raw trees mirror the store");

        // Change the raw side only, then pull it back into the store.
        with_vanilla_neighbour_sets(|| {
            remove(habitat, NeighbourSet::Amphibious, 0x30);
            insert(habitat, NeighbourSet::Show, 0x40);
            remove_portal(habitat, 0x20);
        });
        load_store_from_raw(habitat);
        assert_eq!(members(habitat, NeighbourSet::Amphibious), vec![0x10, 0x50]);
        assert_eq!(members(habitat, NeighbourSet::Show), vec![0x20, 0x40]);
        assert_eq!(portals(habitat), vec![(0x10, 0xbeef)]);

        // Rebuilding again replaces, rather than merges with, whatever the raw trees held.
        rebuild_raw_from_store(habitat);
        assert_eq!(with_vanilla_neighbour_sets(|| snapshot(habitat)), snapshot(habitat));
        remove_habitat(habitat);
        with_vanilla_neighbour_sets(|| {
            clear(habitat, NeighbourSet::Amphibious);
            clear(habitat, NeighbourSet::Show);
            clear_portals(habitat);
        });
    }

    #[test]
    fn vanilla_backend_scope_nests_and_restores() {
        let habitat = fake_habitat();
        insert(habitat, NeighbourSet::Show, 0x10); // store
        with_vanilla_neighbour_sets(|| {
            assert!(members(habitat, NeighbourSet::Show).is_empty(), "raw trees are separate from the store");
            with_vanilla_neighbour_sets(|| insert(habitat, NeighbourSet::Show, 0x20));
            assert_eq!(members(habitat, NeighbourSet::Show), vec![0x20], "inner scope leaves the outer one active");
        });
        assert_eq!(members(habitat, NeighbourSet::Show), vec![0x10], "store untouched and backend restored");
        remove_habitat(habitat);
        with_vanilla_neighbour_sets(|| clear(habitat, NeighbourSet::Show));
    }
}
