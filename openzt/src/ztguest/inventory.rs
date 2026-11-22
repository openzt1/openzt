//! `ZTGuest`'s carried items, visited-site list and the queries over them: `addChanges`, `addVisitedSite`,
//! `removeItem`, `getItem`, `seenHabitat`/`seenShow`/`seenAttraction`, `leavingShow`, `getScenery` and `getGawkTime`.
//!
//! The carried-item vector (`+0x26c`, five `u32` slots) is indexed by the item's need name: hunger (`0x63c4e8` or
//! `0x63c4f8`) slot 0, thirst (`0x63c4c8`) slot 1, `0x63b058` slot 2, souvenir (`0x63c528`) slot 3, trash
//! (`0x63c508`) slot 4. A vector without storage reads as empty.

use openzt_detour::generated::{
    ztguest::{ADD_VISITED_SITE, GET_ITEM},
    zthabitatmgr::{GET_HABITAT as HABITATMGR_GET_HABITAT, HABITAT_SEEN_FROM_BUILDING},
    ztshowmgr::GET_SHOW_INFO,
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::{guest_offsets, ZTGuest};
use crate::{
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    zthabitat::{next_game_random, ZTHabitat},
    ztkeeper::{call_slot_0, unit_offsets},
    ztmegatilemgr::entity_type_matches,
};

/// `CAST_ZTBuilding` and `CAST_ZTSceneryType` as RVAs.
const BUILDING_TAG_RVA: u32 = 0x0023_8680;
const SCENERY_TAG_RVA: u32 = 0x0023_8670;
/// `GLOBAL_ZTWorldMgr` and `GLOBAL_ZTShowMgr` (the globals hold the manager pointers).
const WORLD_MGR_RVA: u32 = 0x0023_8040;
const SHOW_MGR_RVA: u32 = 0x0023_9008;
/// `BFWorldMgr` fields: map size and the tile array, `0x8c` bytes per tile.
const WORLD_MAP_X: u32 = 0x34;
const WORLD_MAP_Y: u32 = 0x38;
const WORLD_TILE_ARRAY: u32 = 0x40;
const TILE_SIZE: u32 = 0x8c;
/// First entity pointer of a tile.
const TILE_ENTITY: u32 = 0x10;
/// Virtual slots: a predicate that ends the seen-query (`+0x264`), the entity type's id getter (`+0x20`), the type's
/// class test (`+0x1c`) and the unit's habitat getter (`+0x240`).
const SLOT_WATER: u32 = 0x264;
const TYPE_SLOT_ID: u32 = 0x20;
const SLOT_HABITAT: u32 = 0x240;
/// Words of the 12-byte visited-site record.
const SITE_SIZE: u32 = 12;
/// A guest remembers at most this many sites (the oldest is dropped past it).
const MAX_SITES: u32 = 10;
/// `ZTGuest` fields.
const CHANGES_MODE: u32 = 0x324;
const SITE_COUNTER: u32 = 0x334;
const HAPPY_DELTA: u32 = 0x2ac;
/// `ZTGuestType` pointers to the gawk time tables.
const TYPE_HAPPINESS_TIMES: u32 = 0x2d0;
const TYPE_ATTRACTIVENESS_TIMES: u32 = 0x2dc;
const TYPE_HAPPY_LOW: u32 = 0x1ec;
/// Position record words and the building grid pointer.
const POSITION_RECORD: u32 = 0x214;
const BUILDING_GRID: u32 = 0x244;
const SCENERY_GRID: u32 = 0x234;
/// Building word naming its show, and the show info's chain to the venue record.
const BUILDING_SHOW_ID: u32 = 0x1c4;
const SHOW_INFO_VENUE: u32 = 0xa0;
const VENUE_SITE: u32 = 0x8c;
const SITE_WORDS: u32 = 0x34;

/// The strings that name each item kind, as RVAs of `{begin, end}` string objects.
const ITEM_SLOTS: [(u32, u32); 6] = [
    (0x0023_c4e8, 0),
    (0x0023_c4f8, 0),
    (0x0023_c4c8, 1),
    (0x0023_b058, 2),
    (0x0023_c528, 3),
    (0x0023_c508, 4),
];

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn bytes_of(string: u32) -> Vec<u8> {
    let begin: u32 = get_from_memory(string);
    let end: u32 = get_from_memory(string + 4);
    (begin..end).map(get_from_memory::<u8>).collect()
}

fn building_at(grid: u32, x: i32, y: i32) -> u32 {
    if grid == 0 || x < 0 || y < 0 || x >= 0x40 || y >= 0x40 {
        return 0;
    }
    get_from_memory(grid + 4 + (((y >> 5) + (x >> 5) * 2) as u32) * 4)
}

impl ZTGuest {
    fn item(self, slot: u32) -> u32 {
        let begin: u32 = self.read(guest_offsets::ITEMS);
        if begin == 0 {
            0
        } else {
            get_from_memory(begin + slot * 4)
        }
    }

    /// `ZTGuest::addChanges` (`0x0042a98f`): folds the six-word change record into the pending changes: word 0 or 1
    /// (by the mode word `+0x324`; neither for other modes) into happiness, then hunger, thirst, bathroom and energy.
    pub fn add_changes(self, changes: u32) -> bool {
        let word = |index: u32| get_from_memory::<i32>(changes + index * 4);
        let add = |offset: u32, amount: i32| self.write(offset, self.read::<i32>(offset).wrapping_add(amount));
        match self.read::<i32>(CHANGES_MODE) {
            0 => add(HAPPY_DELTA, word(0)),
            1 => add(HAPPY_DELTA, word(1)),
            _ => {}
        }
        add(0x2b4, word(2));
        add(0x2bc, word(3));
        add(0x2cc, word(4));
        add(0x2c4, word(5));
        true
    }

    /// `ZTGuest::addVisitedSite` (`0x004a840e`): appends the record, counts it (`+0x334`, 16 bits) and drops the oldest
    /// once more than ten are held.
    pub fn add_visited_site(self, first: u32, second: u32, third: u32) {
        Self::push_site(self.0 + guest_offsets::VISITED_SITES, first, second, third);
        let counter: u16 = self.read(SITE_COUNTER);
        self.write(SITE_COUNTER, counter.wrapping_add(1));
        let vector = self.0 + guest_offsets::VISITED_SITES;
        let begin: u32 = get_from_memory(vector);
        let end: u32 = get_from_memory(vector + 4);
        if (end.wrapping_sub(begin) as i32) / SITE_SIZE as i32 > MAX_SITES as i32 {
            let mut destination = begin;
            while destination + SITE_SIZE != end {
                for word in 0..3 {
                    save_to_memory(destination + word * 4, get_from_memory::<u32>(destination + SITE_SIZE + word * 4));
                }
                destination += SITE_SIZE;
            }
            save_to_memory(vector + 4, end - SITE_SIZE);
        }
    }

    /// `ZTGuest::removeItem` (`0x0042d533`): clears the first slot holding `item` (slots 0, 1, 3, 4, 2).
    pub fn remove_item(self, item: i32) -> bool {
        let begin: u32 = self.read(guest_offsets::ITEMS);
        if begin == 0 {
            return false;
        }
        for slot in [0u32, 1, 3, 4, 2] {
            if get_from_memory::<i32>(begin + slot * 4) == item {
                save_to_memory(begin + slot * 4, 0u32);
                return true;
            }
        }
        false
    }

    /// `ZTGuest::getItem` (`0x0042d6aa`): the item in the slot the need-name string selects; `0` for any other name.
    pub fn get_item(self, name: u32) -> u32 {
        let name = bytes_of(name);
        for (rva, slot) in ITEM_SLOTS {
            if bytes_of(base() + rva) == name {
                return self.item(slot);
            }
        }
        0
    }

    fn visited_sites(self) -> Vec<(i32, i32)> {
        let begin: u32 = self.read(guest_offsets::VISITED_SITES);
        let end: u32 = self.read(guest_offsets::VISITED_SITES + 4);
        (0..end.wrapping_sub(begin) / SITE_SIZE).map(|i| (get_from_memory(begin + i * SITE_SIZE), get_from_memory(begin + i * SITE_SIZE + 4))).collect()
    }

    /// `ZTGuest::seenHabitat` (`0x0042281c`): whether a visited site resolves to `habitat`.
    pub fn seen_habitat(self, habitat: u32) -> bool {
        let manager = globals().zthabitatmgr_ptr() as u32;
        if manager == 0 {
            return false;
        }
        self.visited_sites().into_iter().any(|(x, y)| unsafe { HABITATMGR_GET_HABITAT.hooked()(manager as *const u32, x, y) } == habitat)
    }

    /// `ZTGuest::seenShow` (`0x005a2bcd`): whether a visited site resolves to the habitat the show building overlooks.
    pub fn seen_show(self, building: i32) -> bool {
        let manager = globals().zthabitatmgr_ptr() as u32;
        if manager == 0 {
            return false;
        }
        let habitat = unsafe { HABITAT_SEEN_FROM_BUILDING.hooked()(manager as *const u32, building) } as u32;
        habitat != 0 && self.visited_sites().into_iter().any(|(x, y)| unsafe { HABITATMGR_GET_HABITAT.hooked()(manager as *const u32, x, y) } == habitat)
    }

    /// `ZTGuest::seenAttraction` (`0x004285e2`): with the water predicate (`+0x264`) clear and a scenery given, whether
    /// a visited site's tile holds that scenery or an entity of the same entity-type id. A site outside the map counts
    /// as an empty tile (vanilla faults there).
    pub fn seen_attraction(self, scenery: u32) -> bool {
        if low_byte_bool(call_slot_0(self.0, SLOT_WATER)) || scenery == 0 {
            return false;
        }
        let type_id = |entity: u32| -> u32 {
            let entity_type = if entity == 0 { 0 } else { get_from_memory::<u32>(entity + unit_offsets::ENTITY_TYPE) };
            if entity_type == 0 {
                0
            } else {
                let function = unsafe { std::mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(get_from_memory::<u32>(get_from_memory::<u32>(entity_type) + TYPE_SLOT_ID)) };
                function(entity_type)
            }
        };
        let wanted = type_id(scenery);
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let (size_x, size_y, tiles): (i32, i32, u32) = (get_from_memory(world + WORLD_MAP_X), get_from_memory(world + WORLD_MAP_Y), get_from_memory(world + WORLD_TILE_ARRAY));
        for (x, y) in self.visited_sites() {
            let entity = if x < 0 || y < 0 || x >= size_x || y >= size_y { 0 } else { get_from_memory(tiles + (size_x * y + x) as u32 * TILE_SIZE + TILE_ENTITY) };
            if entity != 0 && unsafe { entity_type_matches(entity, SCENERY_TAG_RVA) } && entity == scenery {
                return true;
            }
            if wanted == type_id(entity) {
                return true;
            }
        }
        false
    }

    /// `ZTGuest::leavingShow` (`0x005a8384`): the building on the guest's grid cell, when it is a building with a
    /// show whose venue record exists, adds that venue's site to the visited list.
    pub fn leaving_show(self) {
        let grid: u32 = self.read(BUILDING_GRID);
        let x: i32 = self.read(POSITION_RECORD);
        let y: i32 = self.read(POSITION_RECORD + 4);
        let building = building_at(grid, x, y);
        if building == 0 || !unsafe { entity_type_matches(building, BUILDING_TAG_RVA) } {
            return;
        }
        let show: u16 = get_from_memory(building + BUILDING_SHOW_ID);
        if show == 0 {
            return;
        }
        let manager: u32 = get_from_memory(base() + SHOW_MGR_RVA);
        let info = unsafe { GET_SHOW_INFO.hooked()(manager as *const u32, show) } as u32;
        if info == 0 {
            return;
        }
        let venue: u32 = get_from_memory(get_from_memory::<u32>(info + SHOW_INFO_VENUE) + VENUE_SITE);
        if venue == 0 {
            return;
        }
        let site = |word: u32| get_from_memory::<u32>(venue + SITE_WORDS + word * 4);
        unsafe { ADD_VISITED_SITE.hooked()(self.0 as *const u32, site(0), site(1), site(2)) };
    }

    /// `ZTGuest::getScenery` (`0x004a6c50`): the scenery entity on the guest's scenery-grid cell (position `-1` reads
    /// as the middle, `0x20`), `0` when there is none or the entity is not scenery.
    pub fn get_scenery(self) -> u32 {
        let grid: u32 = self.read(SCENERY_GRID);
        let mut x: i32 = self.read(POSITION_RECORD);
        let mut y: i32 = self.read(POSITION_RECORD + 4);
        if grid == 0 {
            return 0;
        }
        if x == -1 || y == -1 {
            x = 0x20;
            y = 0x20;
        }
        let entity = building_at(grid, x, y);
        if entity != 0 && unsafe { entity_type_matches(entity, SCENERY_TAG_RVA) } {
            entity
        } else {
            0
        }
    }

    /// `ZTGuest::getGawkTime` (`0x0042e793`): `10 + (draw & 3)`, replaced for a guest type by a table entry picked by
    /// the viewing area's animal happiness (plus one draw modulo 6 outside the lowest band), then raised by the
    /// attractiveness band of the unit's habitat (virtual `+0x240`).
    #[allow(clippy::manual_range_contains)]
    pub fn get_gawk_time(self) -> i32 {
        let happiness = self.animal_happy();
        let mut time = (next_game_random() & 3) as i32 + 10;
        let guest_type = self.guest_type();
        if guest_type == 0 {
            return time;
        }
        let table = |offset: u32, index: u32| -> i32 { get_from_memory(get_from_memory::<u32>(guest_type + offset) + index * 4) };
        if happiness < get_from_memory::<i32>(guest_type + TYPE_HAPPY_LOW) || -0x3c < happiness {
            let band = if happiness < -0x3d || 0x14 < happiness {
                if happiness < 0x15 || 0x3c < happiness {
                    if happiness < 0x3d || 0x50 < happiness {
                        if happiness < 0x51 || 100 < happiness {
                            None
                        } else {
                            Some(4)
                        }
                    } else {
                        Some(3)
                    }
                } else {
                    Some(2)
                }
            } else {
                Some(1)
            };
            if let Some(index) = band {
                time = (next_game_random() % 6) as i32 + table(TYPE_HAPPINESS_TIMES, index);
            }
        } else {
            time = table(TYPE_HAPPINESS_TIMES, 0);
        }
        let habitat = call_slot_0(self.0, SLOT_HABITAT);
        if habitat == 0 {
            return time;
        }
        let attractiveness = unsafe { ref_from_memory::<ZTHabitat>(habitat) }.get_attractiveness();
        let index = match attractiveness {
            a if a < 0x14 => 0,
            a if a < 0x32 => 1,
            a if a < 0x5a => 2,
            a if a < 0x96 => 3,
            a if a < 200 => 4,
            _ => 5,
        };
        time + table(TYPE_ATTRACTIVENESS_TIMES, index)
    }
}

#[detour_mod]
mod detours {
    use super::*;
    use openzt_detour::generated::ztguest::{ADD_CHANGES, ADD_VISITED_SITE, GET_GAWK_TIME, GET_SCENERY, LEAVING_SHOW, REMOVE_ITEM, SEEN_ATTRACTION, SEEN_HABITAT, SEEN_SHOW};

    #[detour(ADD_CHANGES)]
    unsafe extern "thiscall" fn add_changes(this: *const u32, changes: *const u32) -> bool {
        ZTGuest(this as u32).add_changes(changes as u32)
    }

    #[detour(ADD_VISITED_SITE)]
    unsafe extern "thiscall" fn add_visited_site(this: *const u32, first: u32, second: u32, third: u32) {
        ZTGuest(this as u32).add_visited_site(first, second, third);
    }

    #[detour(REMOVE_ITEM)]
    unsafe extern "thiscall" fn remove_item(this: *const u32, item: i32) -> bool {
        ZTGuest(this as u32).remove_item(item)
    }

    #[detour(GET_ITEM)]
    unsafe extern "thiscall" fn get_item(this: *const u32, name: *const i8) -> u32 {
        ZTGuest(this as u32).get_item(name as u32)
    }

    #[detour(SEEN_HABITAT)]
    unsafe extern "thiscall" fn seen_habitat(this: *const u32, habitat: *const u32) -> bool {
        ZTGuest(this as u32).seen_habitat(habitat as u32)
    }

    #[detour(SEEN_SHOW)]
    unsafe extern "thiscall" fn seen_show(this: *const u32, building: i32) -> bool {
        ZTGuest(this as u32).seen_show(building)
    }

    #[detour(SEEN_ATTRACTION)]
    unsafe extern "thiscall" fn seen_attraction(this: *const u32, scenery: *const u32) -> bool {
        ZTGuest(this as u32).seen_attraction(scenery as u32)
    }

    #[detour(LEAVING_SHOW)]
    unsafe extern "thiscall" fn leaving_show(this: *const u32) {
        ZTGuest(this as u32).leaving_show();
    }

    #[detour(GET_SCENERY)]
    unsafe extern "thiscall" fn get_scenery(this: *const u32) -> i32 {
        ZTGuest(this as u32).get_scenery() as i32
    }

    #[detour(GET_GAWK_TIME)]
    unsafe extern "thiscall" fn get_gawk_time(this: *const u32) -> i32 {
        ZTGuest(this as u32).get_gawk_time()
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn add_changes(this: u32, changes: u32) -> bool {
            unsafe { super::ADD_CHANGES_DETOUR.call(this as *const u32, changes as *const u32) }
        }
        pub(crate) fn add_visited_site(this: u32, first: u32, second: u32, third: u32) {
            unsafe { super::ADD_VISITED_SITE_DETOUR.call(this as *const u32, first, second, third) }
        }
        pub(crate) fn remove_item(this: u32, item: i32) -> bool {
            unsafe { super::REMOVE_ITEM_DETOUR.call(this as *const u32, item) }
        }
        pub(crate) fn get_item(this: u32, name: u32) -> u32 {
            unsafe { super::GET_ITEM_DETOUR.call(this as *const u32, name as *const i8) }
        }
        pub(crate) fn seen_habitat(this: u32, habitat: u32) -> bool {
            unsafe { super::SEEN_HABITAT_DETOUR.call(this as *const u32, habitat as *const u32) }
        }
        pub(crate) fn seen_show(this: u32, building: i32) -> bool {
            unsafe { super::SEEN_SHOW_DETOUR.call(this as *const u32, building) }
        }
        pub(crate) fn seen_attraction(this: u32, scenery: u32) -> bool {
            unsafe { super::SEEN_ATTRACTION_DETOUR.call(this as *const u32, scenery as *const u32) }
        }
        pub(crate) fn leaving_show(this: u32) {
            unsafe { super::LEAVING_SHOW_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_scenery(this: u32) -> i32 {
            unsafe { super::GET_SCENERY_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn get_gawk_time(this: u32) -> i32 {
            unsafe { super::GET_GAWK_TIME_DETOUR.call(this as *const u32) }
        }
    }
}

/// Registers the inventory detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest inventory detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub(crate) use super::detours::test_real as real;

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }
}
