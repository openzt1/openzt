//! `ZTGuest`'s not-yet-seen site queries: `getNonseenHabitats`, `getNonseenAttractions`, `getNonseenGawkScenery`,
//! `getNumNonseenGawkScenery`, `getNumNonseenSites` and `pickGawkScenery`. Reads guest and world memory in place
//! through [`ZTGuest`].
//!
//! The list results are vanilla `list<uint>` handles (a pointer to a 12-byte head node `{next, prev, _}`, nodes
//! `{next, prev, value}`) built from the vanilla pool, and the habitat result is a vanilla `vector<int>` sized
//! with `vector<int>::buy`, because vanilla callers free both through the pool. Temporary lists are freed the way
//! vanilla frees them.

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bfentity::GET_TILE as BFENTITY_GET_TILE,
        msvc_std_listuint::LIST_UINT_CTOR,
        msvc_std_vectorint::BUY,
        poolalloc::{ALLOCATE, DEALLOCATE, DEALLOCATE_N_12, DEALLOCATE_N_4},
        ztguest::{GET_NONSEEN_ATTRACTIONS, GET_NONSEEN_GAWK_SCENERY, GET_NONSEEN_HABITATS, GET_NUM_NONSEEN_GAWK_SCENERY, GET_NUM_NONSEEN_SITES, PICK_GAWK_SCENERY},
        zthabitatmgr::GET_NONEMPTY_NON_WORLD_HABITATS,
        ztworldmgr::{GET_ATTRACTIONS, GET_GAWK_SCENERY},
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::ZTGuest;
use crate::{
    globals::get_module_base,
    util::{get_from_memory, low_byte_bool, save_to_memory},
    ztkeeper::{call_slot_0, unit_offsets},
};

/// `GLOBAL_ZTWorldMgr` and `GLOBAL_ZTHabitatMgr` as RVAs (the globals hold the manager pointers).
const WORLD_MGR_RVA: u32 = 0x0023_8040;
const HABITAT_MGR_RVA: u32 = 0x0023_805c;


mod fields {
    /// `ZTGuestType` word: how much a gawk scenery is worth.
    pub const TYPE_GAWK_VALUE: u32 = 0x120;
    /// Habitat virtual slot (`+0x20`) and field (`+0x4`) that exclude a habitat from the unseen list.
    pub const HABITAT_EXCLUDED_SLOT: u32 = 0x20;
    pub const HABITAT_FIELD: u32 = 0x4;
    /// Tile position words.
    pub const TILE_X: u32 = 0x34;
    pub const TILE_Y: u32 = 0x38;
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn world() -> u32 {
    get_from_memory(base() + WORLD_MGR_RVA)
}

/// A vanilla `list<uint>` handle: the address of the 12-byte head node.
#[derive(Clone, Copy)]
struct List(u32);

impl List {
    /// Builds an empty list in the handle slot at `slot` with the vanilla constructor and returns it.
    fn construct(slot: u32) -> List {
        let dummy = 0u8;
        unsafe { LIST_UINT_CTOR.hooked()(slot as *const c_void, &dummy as *const u8 as u32) };
        List(get_from_memory(slot))
    }

    fn values(self) -> Vec<u32> {
        let mut values = Vec::new();
        let mut node: u32 = get_from_memory(self.0);
        while node != self.0 {
            values.push(get_from_memory(node + 8));
            node = get_from_memory(node);
        }
        values
    }

    fn push_back(self, value: u32) {
        let node = unsafe { ALLOCATE.hooked()(12) } as u32;
        let last: u32 = get_from_memory(self.0 + 4);
        save_to_memory(node, self.0);
        save_to_memory(node + 4, last);
        save_to_memory(node + 8, value);
        save_to_memory(last, node);
        save_to_memory(self.0 + 4, node);
    }

    /// Frees every node and the head through the pool.
    fn free(self) {
        let mut node: u32 = get_from_memory(self.0);
        while node != self.0 {
            let next: u32 = get_from_memory(node);
            unsafe { DEALLOCATE_N_12.hooked()(node as *const u32, 1) };
            node = next;
        }
        save_to_memory(self.0, self.0);
        save_to_memory(self.0 + 4, self.0);
        unsafe { DEALLOCATE_N_12.hooked()(self.0 as *const u32, 1) };
    }
}

impl ZTGuest {
    /// Collects the world's scenery from `fill`, keeps those the guest has not seen and returns them as a fresh
    /// list written into `out`'s handle slot.
    fn nonseen_scenery_list(self, out: u32, fill: unsafe fn(u32, u32)) -> u32 {
        let world = world();
        if world == 0 {
            List::construct(out);
            return out;
        }
        let mut source_slot = 0u32;
        let source = List::construct(&mut source_slot as *mut u32 as u32);
        unsafe { fill(world, &source_slot as *const u32 as u32) };
        let mut kept_slot = 0u32;
        let kept = List::construct(&mut kept_slot as *mut u32 as u32);
        for scenery in source.values() {
            if !self.seen_attraction(scenery) {
                kept.push_back(scenery);
            }
        }
        let result = List::construct(out);
        for scenery in kept.values() {
            result.push_back(scenery);
        }
        kept.free();
        source.free();
        out
    }

    /// `ZTGuest::getNonseenAttractions` (`0x00427d5c`): the world's attractions the guest has not seen.
    pub fn get_nonseen_attractions(self, out: u32) -> u32 {
        self.nonseen_scenery_list(out, |world, slot| unsafe { GET_ATTRACTIONS.hooked()(world as *const u32, slot as *const i32) })
    }

    /// `ZTGuest::getNonseenGawkScenery` (`0x0042956e`): the world's gawk scenery the guest has not seen.
    pub fn get_nonseen_gawk_scenery(self, out: u32) -> u32 {
        self.nonseen_scenery_list(out, |world, slot| unsafe { GET_GAWK_SCENERY.hooked()(world as *const u32, slot as *const i32) })
    }

    /// `ZTGuest::getNonseenHabitats` (`0x00427b08`): the non-empty, non-world habitats the guest has not seen, as a
    /// `vector<int>` written into `out`. A habitat is skipped when its virtual `+0x20` holds and its field `+0x4` is set.
    pub fn get_nonseen_habitats(self, out: u32) -> u32 {
        let manager: u32 = get_from_memory(base() + HABITAT_MGR_RVA);
        let dummy = 0u32;
        if manager == 0 {
            unsafe { BUY.hooked()(out as *const i32, 0, &dummy as *const u32 as u32) };
            let begin: u32 = get_from_memory(out);
            save_to_memory(out + 4, begin);
            return out;
        }
        let mut nonempty = [0u32; 3];
        unsafe { GET_NONEMPTY_NON_WORLD_HABITATS.hooked()(manager as *const u32, nonempty.as_mut_ptr() as *const c_void) };
        let count = (nonempty[1].wrapping_sub(nonempty[0]) as i32) >> 2;
        let mut kept = Vec::new();
        if count > 0 {
            for index in 0..count as u32 {
                let habitat: u32 = get_from_memory(nonempty[0] + index * 4);
                let excluded = low_byte_bool(call_slot_0(habitat, fields::HABITAT_EXCLUDED_SLOT)) && get_from_memory::<u32>(habitat + fields::HABITAT_FIELD) != 0;
                if !excluded && !self.seen_habitat(habitat) {
                    kept.push(habitat);
                }
            }
        }
        unsafe { BUY.hooked()(out as *const i32, kept.len() as i32, &dummy as *const u32 as u32) };
        let begin: u32 = get_from_memory(out);
        for (index, habitat) in kept.iter().enumerate() {
            save_to_memory(begin + index as u32 * 4, *habitat);
        }
        save_to_memory(out + 4, begin + kept.len() as u32 * 4);
        unsafe { DEALLOCATE_N_4.hooked()(nonempty[0] as *const u32, (nonempty[2].wrapping_sub(nonempty[0]) as i32) >> 2) };
        out
    }

    /// `ZTGuest::getNumNonseenGawkScenery` (`0x00428039`): with `from_world` set, the summed gawk values of the
    /// world's unseen gawk scenery (`0` without a world); otherwise the number of entries `getNonseenGawkScenery`
    /// returns. With a positive total and both flags set, the total is cut to a hundredth (at least `1`).
    pub fn get_num_nonseen_gawk_scenery(self, from_world: bool, scaled: bool) -> i32 {
        let mut total = 0i32;
        if from_world {
            let world = world();
            if world == 0 {
                return 0;
            }
            let mut source_slot = 0u32;
            let source = List::construct(&mut source_slot as *mut u32 as u32);
            unsafe { GET_GAWK_SCENERY.hooked()(world as *const u32, std::ptr::from_ref(&source_slot).cast()) };
            let mut kept_slot = 0u32;
            let kept = List::construct(&mut kept_slot as *mut u32 as u32);
            for scenery in source.values() {
                if scenery != 0 && !self.seen_attraction(scenery) {
                    let entity_type: u32 = get_from_memory(scenery + unit_offsets::ENTITY_TYPE);
                    total = total.wrapping_add(get_from_memory::<i32>(entity_type + fields::TYPE_GAWK_VALUE));
                }
            }
            kept.free();
            source.free();
        } else {
            let mut slot = 0u32;
            self.get_nonseen_gawk_scenery(&mut slot as *mut u32 as u32);
            let list = List(slot);
            total = list.values().len() as i32;
            list.free();
        }
        if total > 0 && from_world && scaled {
            let hundredth = total / 100;
            return if hundredth > 0 { hundredth } else { 1 };
        }
        total
    }

    /// `ZTGuest::getNumNonseenSites` (`0x00427c88`): unseen habitats, unseen attractions and the scaled gawk
    /// total.
    pub fn get_num_nonseen_sites(self) -> i32 {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = overrides::NUM_SITES.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        let mut habitats = [0u32; 3];
        self.get_nonseen_habitats(habitats.as_mut_ptr() as u32);
        let habitat_count = (habitats[1].wrapping_sub(habitats[0]) as i32) >> 2;
        let mut attractions_slot = 0u32;
        self.get_nonseen_attractions(&mut attractions_slot as *mut u32 as u32);
        let attractions = List(attractions_slot);
        let attraction_count = attractions.values().len() as i32;
        let gawk = self.get_num_nonseen_gawk_scenery(true, true);
        let total = gawk.wrapping_add(habitat_count).wrapping_add(attraction_count);
        attractions.free();
        if habitats[0] != 0 {
            let size = ((habitats[2].wrapping_sub(habitats[0]) as i32) >> 2) as u32 * 4;
            unsafe { DEALLOCATE.hooked()(habitats[0] as *const u32, size) };
        }
        total
    }

    /// `ZTGuest::pickGawkScenery` (`0x0042945f`): the unseen gawk scenery within a squared tile distance of `1000`
    /// with the best score `((value + 100) * 10 - distance) / 200`, `0` when none scores above `-1`.
    pub fn pick_gawk_scenery(self) -> u32 {
        let own_tile = unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) } as u32;
        // Vanilla totals the world's gawk values first and discards the result.
        self.get_num_nonseen_gawk_scenery(true, false);
        let (own_x, own_y): (i32, i32) = if own_tile == 0 { (0, 0) } else { (get_from_memory(own_tile + fields::TILE_X), get_from_memory(own_tile + fields::TILE_Y)) };
        let mut slot = 0u32;
        self.get_nonseen_gawk_scenery(&mut slot as *mut u32 as u32);
        let list = List(slot);
        let mut best = -1i32;
        let mut picked = 0u32;
        for scenery in list.values() {
            if scenery == 0 {
                continue;
            }
            let tile = unsafe { BFENTITY_GET_TILE.hooked()(scenery as *const u32) } as u32;
            if tile == 0 {
                continue;
            }
            let dx = get_from_memory::<i32>(tile + fields::TILE_X).wrapping_sub(own_x);
            let dy = get_from_memory::<i32>(tile + fields::TILE_Y).wrapping_sub(own_y);
            let distance = dx.wrapping_mul(dx).wrapping_add(dy.wrapping_mul(dy));
            if distance < 1000 {
                let entity_type: u32 = get_from_memory(scenery + unit_offsets::ENTITY_TYPE);
                let value: i32 = get_from_memory(entity_type + fields::TYPE_GAWK_VALUE);
                let score = value.wrapping_add(100).wrapping_mul(10).wrapping_sub(distance) / 200;
                if best < score {
                    best = score;
                    picked = scenery;
                }
            }
        }
        list.free();
        picked
    }
}

/// Overrides the live tests install on the detours.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod overrides {
    use std::sync::Mutex;

    pub type NumSites = fn(this: u32) -> Option<i32>;
    pub static NUM_SITES: Mutex<Option<NumSites>> = Mutex::new(None);
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_NONSEEN_HABITATS)]
    unsafe extern "thiscall" fn get_nonseen_habitats(this: *const u32, out: *const u32) -> *const u32 {
        ZTGuest(this as u32).get_nonseen_habitats(out as u32) as *const u32
    }

    #[detour(GET_NONSEEN_ATTRACTIONS)]
    unsafe extern "thiscall" fn get_nonseen_attractions(this: *const u32, out: *const i32) -> *const i32 {
        ZTGuest(this as u32).get_nonseen_attractions(out as u32) as *const i32
    }

    #[detour(GET_NONSEEN_GAWK_SCENERY)]
    unsafe extern "thiscall" fn get_nonseen_gawk_scenery(this: *const u32, out: *const i32) -> *const i32 {
        ZTGuest(this as u32).get_nonseen_gawk_scenery(out as u32) as *const i32
    }

    #[detour(GET_NUM_NONSEEN_GAWK_SCENERY)]
    unsafe extern "thiscall" fn get_num_nonseen_gawk_scenery(this: *const u32, from_world: bool, scaled: bool) -> i32 {
        ZTGuest(this as u32).get_num_nonseen_gawk_scenery(from_world, scaled)
    }

    #[detour(GET_NUM_NONSEEN_SITES)]
    unsafe extern "thiscall" fn get_num_nonseen_sites(this: *const u32) -> i32 {
        ZTGuest(this as u32).get_num_nonseen_sites()
    }

    #[detour(PICK_GAWK_SCENERY)]
    unsafe extern "thiscall" fn pick_gawk_scenery(this: *const u32) -> *const i32 {
        ZTGuest(this as u32).pick_gawk_scenery() as *const i32
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn get_nonseen_habitats(this: u32, out: u32) -> u32 {
            unsafe { super::GET_NONSEEN_HABITATS_DETOUR.call(this as *const u32, out as *const u32) as u32 }
        }
        pub(crate) fn get_nonseen_attractions(this: u32, out: u32) -> u32 {
            unsafe { super::GET_NONSEEN_ATTRACTIONS_DETOUR.call(this as *const u32, out as *const i32) as u32 }
        }
        pub(crate) fn get_nonseen_gawk_scenery(this: u32, out: u32) -> u32 {
            unsafe { super::GET_NONSEEN_GAWK_SCENERY_DETOUR.call(this as *const u32, out as *const i32) as u32 }
        }
        pub(crate) fn get_num_nonseen_gawk_scenery(this: u32, from_world: bool, scaled: bool) -> i32 {
            unsafe { super::GET_NUM_NONSEEN_GAWK_SCENERY_DETOUR.call(this as *const u32, from_world, scaled) }
        }
        pub(crate) fn get_num_nonseen_sites(this: u32) -> i32 {
            unsafe { super::GET_NUM_NONSEEN_SITES_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn pick_gawk_scenery(this: u32) -> u32 {
            unsafe { super::PICK_GAWK_SCENERY_DETOUR.call(this as *const u32) as u32 }
        }
    }
}

/// Registers the unseen-site detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest site detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub(crate) use super::detours::test_real as real;

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }
}
