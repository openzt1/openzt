//! `ZTGuest`'s viewing-area and following code: `followGuide`, the viewing-area search (`checkViewingArea`,
//! `findViewingArea`), `viewNearbyExhibit`, `doBuilding`, `setVisible`, `startPuking` and `runScreaming`. Reads and
//! writes guest memory in place through [`ZTGuest`].

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bfentity::{GET_TILE as BFENTITY_GET_TILE, SET_BSNEW},
        bfmap::GET_NEIGHBORS,
        bfunit::{GET_PATH_1 as GET_PATH, GO_TO_DEST_1 as GO_TO_DEST},
        msvc_std::RAND,
        msvc_std_basic_string::BASIC_STRING_RANGE,
        poolalloc::DEALLOCATE_N_4 as POOLALLOC_DEALLOCATE_N_4,
        ztguest::{CHECK_VIEWING_AREA, DO_BUILDING, FIND_VIEWING_AREA, FOLLOW_GUIDE, RUN_SCREAMING, SET_VISIBLE, START_PUKING, VIEW_NEARBY_EXHIBIT},
        standalone::HABITAT_FACING,
        zthabitat::GET_ALL_ANIMALS,
        ztguide::ADD_GUEST as GUIDE_ADD_GUEST,
        ztui_guestinfo::SET_GUEST_VISIBLE,
        ztunit::{DO_BUILDING as ZTUNIT_DO_BUILDING, IS_FUN_SATISFIER},
        ztviewingarea::HAS_OA,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use super::ZTGuest;
use crate::{
    globals::get_module_base,
    util::{get_from_memory, low_byte_bool},
    zthabitat::next_game_random,
    ztkeeper::{call_slot_0, call_slot_1, call_slot_2, call_slot_3},
};

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

/// `GLOBAL_ZTHabitatMgr`, as an RVA, and the per-column table of habitat rows at `+0x28`.
const HABITAT_MGR_RVA: u32 = 0x0023_805c;
const HABITAT_MGR_COLUMNS: u32 = 0x28;
/// `GLOBAL_ZTWorldMgr`, as an RVA; the map sits at `+0x8`.
const WORLD_MGR_RVA: u32 = 0x0023_8040;

mod guest_fields {
    pub const ENTITY_ID: u32 = 0x124;
    pub const GOAL_ID: u32 = 0x170;
    /// A guide's destination tile.
    pub const GUIDE_DESTINATION: u32 = 0x180;
    pub const BUILDING: u32 = 0x244;
    pub const GUIDE_ID: u32 = 0x2e4;
    pub const FOLLOWING: u32 = 0x337;
    pub const FOLLOW_PENDING: u32 = 0x338;
    pub const LEAVING: u32 = 0x339;
    pub const SCREAMING: u32 = 0x33b;
    pub const VIEWING_AREA_DIRECTION: u32 = 0x328;
    pub const VIEWING_AREA: u32 = 0x32c;
    pub const HAPPINESS_DELTA: u32 = 0x2ac;
    pub const VISIBLE: u32 = 0x13f;
    /// The animation name `{begin, end}` the unit is playing.
    pub const ANIMATION_BEGIN: u32 = 0x54;
    pub const ANIMATION_END: u32 = 0x58;
    pub const POSITION_RECORD: u32 = 0x214;
}

mod viewing_slots {
    pub const GOAL_SET: u32 = 0x1d8;
    pub const GET_HABITAT_FROM_TILE: u32 = 0x248;
    pub const IS_DOING_SOMETHING: u32 = 0x288;
    pub const HAS_PRESSING_NEED: u32 = 0x28c;
    pub const SET_TARGET_TILE: u32 = 0x2d8;
    pub const STOP_EVERYTHING: u32 = 0x198;
}

const GOAL_GUIDE_DUTIES: u32 = 0x76;
const GOAL_VA_DUTIES: u32 = 0x77;
/// `ZTGuest`'s goal while following a guide.
const GOAL_FOLLOWING: u32 = 0x76;
/// One in this many `followGuide` calls paths straight to the guide's destination.
const DIRECT_PATH_ODDS: u32 = 10;
/// Happiness range an animal must be in to be "overjoyed".
const OVERJOYED_RANGE: std::ops::RangeInclusive<i32> = 0x51..=0x64;
const ANIMAL_HAPPINESS: u32 = 0x2a8;
const THOUGHT_OVERJOYED_ANIMALS: u32 = 0x2772;
const SICK_ANIMATION: &[u8; 6] = b"bSick\0";
const HABITAT_IS_EXHIBIT: u32 = 0x2c;
/// `ZTGuestType` bonus a guest gains for watching overjoyed animals.
const OVERJOYED_BONUS_TYPE_OFFSET: u32 = 0x28c;
/// Tile fields.
const TILE_X: u32 = 0x34;
const TILE_Y: u32 = 0x38;
/// A viewing area's tile row holds eight directions after one leading word, ten words per tile.
const VIEWING_AREA_ROW_WORDS: u32 = 10;

/// Override the live tests install on `followGuide`: answers `true` once it has handled the call.
#[cfg(feature = "reimplementation-tests")]
pub(crate) mod overrides {
    use std::sync::Mutex;

    pub type Follow = fn(this: u32, guide: u32) -> bool;
    /// Replaces `startPuking`'s `setBSNew` call: gets the guest, the behaviour-set name and the three flags.
    pub type SetBs = fn(this: u32, name: &[u8], call_flag: i8, force: i8, clear_mode: i32);

    pub static FOLLOW: Mutex<Option<Follow>> = Mutex::new(None);
    pub static SET_BS: Mutex<Option<SetBs>> = Mutex::new(None);
}

impl ZTGuest {
    fn viewing_tile(self) -> u32 {
        unsafe { BFENTITY_GET_TILE.hooked()(self.0 as *const u32) as u32 }
    }

    /// `ZTGuest::checkViewingArea` (`0x0043b8a2`): remembers `direction` (`+0x328`) and, when the guest's tile lies in
    /// an exhibit habitat whose viewing area in that direction has an observable area, that area (`+0x32c`) and
    /// returns `true`. Otherwise the area is cleared. A tile without a habitat counts as no exhibit.
    pub fn check_viewing_area(self, direction: u32) -> bool {
        let manager: u32 = get_from_memory(base() + HABITAT_MGR_RVA);
        let tile = self.viewing_tile();
        if manager != 0 && tile != 0 && direction != u32::MAX {
            let habitat = call_slot_1(self.0, viewing_slots::GET_HABITAT_FROM_TILE, tile);
            if habitat != 0 && get_from_memory::<u8>(habitat + HABITAT_IS_EXHIBIT) != 0 {
                let x: u32 = get_from_memory(tile + TILE_X);
                let y: u32 = get_from_memory(tile + TILE_Y);
                let columns: u32 = get_from_memory(manager + HABITAT_MGR_COLUMNS);
                let row: u32 = get_from_memory(columns.wrapping_add(x.wrapping_mul(12)));
                let area: u32 = get_from_memory(row.wrapping_add(4).wrapping_add(direction.wrapping_add(y.wrapping_mul(VIEWING_AREA_ROW_WORDS)).wrapping_mul(4)));
                if area != 0 && unsafe { HAS_OA.hooked()(area as *const u32) } {
                    self.write(guest_fields::VIEWING_AREA, area);
                    self.write(guest_fields::VIEWING_AREA_DIRECTION, direction);
                    return true;
                }
            }
        }
        self.write(guest_fields::VIEWING_AREA, 0u32);
        self.write(guest_fields::VIEWING_AREA_DIRECTION, direction);
        false
    }

    /// `ZTGuest::findViewingArea` (`0x0043b92e`): one draw from the shared RNG picks a starting direction, and the
    /// eight directions are tried from there until one has a viewing area.
    pub fn find_viewing_area(self) -> bool {
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(answer) = crate::ztguest::check_overrides::FIND_VIEWING_AREA.lock().ok().and_then(|hook| *hook).and_then(|hook| hook(self.0)) {
                return answer;
            }
        }
        let start = next_game_random() & 7;
        (0..8u32).any(|step| unsafe { CHECK_VIEWING_AREA.hooked()(self.0 as *const u32, (step + start) & 7) })
    }

    /// Looks for a tile next to `destination` the guest can path to: from a random start among the neighbours (each
    /// tried at most once, the start tried again at the end) the first one inside an exhibit habitat that
    /// `getPath` accepts. Returns the tile on success.
    fn path_to_neighbour(self, destination: u32) -> Option<u32> {
        let world: u32 = get_from_memory(base() + WORLD_MGR_RVA);
        let mut neighbours = [0u32; 3];
        unsafe { GET_NEIGHBORS.hooked()((world + 8) as *const u32, neighbours.as_mut_ptr() as *const u32, destination as i32) };
        let count = (neighbours[1].wrapping_sub(neighbours[0]) as i32) >> 2;
        let mut found = None;
        if count > 0 {
            let mut index = (next_game_random() as i32).wrapping_rem(count);
            while index <= count {
                let tile: u32 = get_from_memory(neighbours[0] + (index.wrapping_rem(count) as u32) * 4);
                let habitat = call_slot_1(self.0, viewing_slots::GET_HABITAT_FROM_TILE, tile);
                if tile != 0 && habitat != 0 && get_from_memory::<u8>(habitat + HABITAT_IS_EXHIBIT) != 0 && unsafe { GET_PATH.hooked()(self.0 as *const u32, tile as *const u32, false) } {
                    found = Some(tile);
                    break;
                }
                index += 1;
            }
        }
        if neighbours[0] != 0 {
            unsafe { POOLALLOC_DEALLOCATE_N_4.original()(neighbours[0] as *const u32, count) };
        }
        found
    }

    /// `ZTGuest::followGuide` (`0x0058d79e`): a guest that is not busy and has no pressing need, offered a guide on
    /// guide or viewing-area duties (and not already following another one), paths to the guide's destination (one
    /// time in ten) or to a neighbouring exhibit tile, switches to the following goal (`0x76`), heads off, records the
    /// guide (`+0x2e4`, telling the guide through `addGuest` when it is new) and sets the following flag (`+0x337`).
    pub fn follow_guide(self, guide: u32) {
        if guide == 0 {
            return;
        }
        let guide_id: u32 = get_from_memory(guide + guest_fields::ENTITY_ID);
        if self.read::<u8>(guest_fields::FOLLOWING) != 0 && self.read::<u32>(guest_fields::GUIDE_ID) != guide_id {
            return;
        }
        if low_byte_bool(call_slot_0(self.0, viewing_slots::IS_DOING_SOMETHING)) || low_byte_bool(call_slot_0(self.0, viewing_slots::HAS_PRESSING_NEED)) {
            return;
        }
        let goal: u32 = get_from_memory(guide + guest_fields::GOAL_ID);
        if goal != GOAL_GUIDE_DUTIES && goal != GOAL_VA_DUTIES {
            return;
        }
        let destination: u32 = get_from_memory(guide + guest_fields::GUIDE_DESTINATION);
        if destination == 0 {
            return;
        }
        let target = if next_game_random().is_multiple_of(DIRECT_PATH_ODDS) {
            unsafe { GET_PATH.hooked()(self.0 as *const u32, destination as *const u32, false) }.then_some(destination)
        } else {
            self.path_to_neighbour(destination)
        };
        let Some(target) = target else {
            return;
        };
        call_slot_2(self.0, viewing_slots::GOAL_SET, GOAL_FOLLOWING, 3);
        if !unsafe { GO_TO_DEST.hooked()(self.0 as *const u32, false, 0) } {
            return;
        }
        call_slot_2(self.0, viewing_slots::SET_TARGET_TILE, target, 1);
        if self.read::<u32>(guest_fields::GUIDE_ID) != guide_id {
            let own_id: u32 = self.read(guest_fields::ENTITY_ID);
            self.write(guest_fields::GUIDE_ID, guide_id);
            unsafe { GUIDE_ADD_GUEST.hooked()(guide as *const u32, own_id as i32) };
        }
        self.write(guest_fields::FOLLOWING, 1u8);
        self.write(guest_fields::FOLLOW_PENDING, 0u8);
    }

    /// `ZTGuest::viewNearbyExhibit` (`0x005a07bc`): a guest facing a non-exhibit habitat picks (by a `rand` draw) one
    /// of its overjoyed animals (happiness `0x51..=0x64`), takes the guest type's bonus into its happiness change
    /// (`+0x2ac`) and thinks `0x2772` about it.
    pub fn view_nearby_exhibit(self) {
        let habitat = unsafe { HABITAT_FACING.hooked()(self.0 as *const u32) } as u32;
        if habitat == 0 || get_from_memory::<u8>(habitat + HABITAT_IS_EXHIBIT) != 0 {
            return;
        }
        let animals = unsafe { GET_ALL_ANIMALS.hooked()(habitat as *const u32, 0) } as u32;
        let bytes = get_from_memory::<u32>(animals + 4).wrapping_sub(get_from_memory::<u32>(animals));
        if (bytes & !3) as i32 <= 0 {
            return;
        }
        let roll = unsafe { RAND.hooked()() } % (bytes >> 2);
        let begin: u32 = get_from_memory(animals);
        let count = get_from_memory::<u32>(animals + 4).wrapping_sub(begin) >> 2;
        let mut chosen = 0;
        for index in 0..count {
            let animal: u32 = get_from_memory(begin + index * 4);
            if OVERJOYED_RANGE.contains(&get_from_memory::<i32>(animal + ANIMAL_HAPPINESS)) {
                chosen = animal;
                if index >= roll {
                    break;
                }
            }
        }
        if chosen == 0 {
            return;
        }
        let guest_type = self.guest_type();
        if guest_type != 0 {
            let delta: i32 = self.read(guest_fields::HAPPINESS_DELTA);
            self.write(guest_fields::HAPPINESS_DELTA, delta.wrapping_add(get_from_memory::<i32>(guest_type + OVERJOYED_BONUS_TYPE_OFFSET)));
        }
        self.thought(THOUGHT_OVERJOYED_ANIMALS, chosen);
    }

    /// `ZTGuest::doBuilding` (`0x0042aa88`): a guest whose position record lies on a fun satisfier of its building
    /// (`+0x244`) records the building's position as a visited site, then `ZTUnit::doBuilding` runs.
    pub fn do_building(self) {
        let building: u32 = self.read(guest_fields::BUILDING);
        let record: [u32; 3] = [
            self.read(guest_fields::POSITION_RECORD),
            self.read(guest_fields::POSITION_RECORD + 4),
            self.read(guest_fields::POSITION_RECORD + 8),
        ];
        if low_byte_bool(unsafe { IS_FUN_SATISFIER.hooked()(building as i32, record.as_ptr()) }) && building != 0 {
            self.add_visited_site(get_from_memory(building + 0x34), get_from_memory(building + 0x38), get_from_memory(building + 0x3c));
        }
        unsafe { ZTUNIT_DO_BUILDING.hooked()(self.0 as *const u32) };
    }

    /// `ZTGuest::setVisible` (`0x0042aa0a`): stores the flag (`+0x13f`) and tells the guest info panel.
    pub fn set_visible(self, visible: bool) {
        self.write(guest_fields::VISIBLE, visible as u8);
        unsafe { SET_GUEST_VISIBLE.hooked()(self.0 as *const u32, visible as i8) };
    }

    /// `ZTGuest::runScreaming` (`0x006191f9`): marks the guest screaming (`+0x33b`) and, off the water, leaving
    /// (`+0x339`).
    pub fn run_screaming(self) {
        self.write(guest_fields::SCREAMING, 1u8);
        if !self.is_on_water() {
            self.write(guest_fields::LEAVING, 1u8);
        }
    }

    /// `ZTGuest::startPuking` (`0x00619110`): a guest not already playing the `"bSick"` animation stops what it is
    /// doing (virtual `+0x198`) and switches to that behaviour set.
    pub fn start_puking(self) {
        let begin: u32 = self.read(guest_fields::ANIMATION_BEGIN);
        let end: u32 = self.read(guest_fields::ANIMATION_END);
        let name = &SICK_ANIMATION[..SICK_ANIMATION.len() - 1];
        let already = end.wrapping_sub(begin) as usize == name.len() && unsafe { std::slice::from_raw_parts(begin as *const u8, name.len()) } == name;
        if already {
            return;
        }
        call_slot_3(self.0, viewing_slots::STOP_EVERYTHING, 1, 0, 1);
        let mut string = [0u32; 3];
        let start = SICK_ANIMATION.as_ptr();
        unsafe {
            BASIC_STRING_RANGE.hooked()(string.as_mut_ptr() as *const c_void, start as *const u32, start.add(name.len()) as i32);
        }
        #[cfg(feature = "reimplementation-tests")]
        {
            if let Some(hook) = overrides::SET_BS.lock().ok().and_then(|hook| *hook) {
                hook(self.0, name, 1, 1, 2);
                unsafe { openzt_detour::generated::poolalloc::DEALLOCATE.original()(string[0] as *const u32, string[2].wrapping_sub(string[0])) };
                return;
            }
        }
        unsafe { SET_BSNEW.hooked()(self.0 as *const u32, string[0] as *const u32, string[1] as *const u32, string[2] as *const u32, 1, 1, 2) };
    }
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(CHECK_VIEWING_AREA)]
    unsafe extern "thiscall" fn check_viewing_area(this: *const u32, direction: u32) -> bool {
        ZTGuest(this as u32).check_viewing_area(direction)
    }

    #[detour(FIND_VIEWING_AREA)]
    unsafe extern "thiscall" fn find_viewing_area(this: *const u32) -> bool {
        ZTGuest(this as u32).find_viewing_area()
    }

    #[detour(FOLLOW_GUIDE)]
    unsafe extern "thiscall" fn follow_guide(this: *const u32, guide: *const c_void) {
        #[cfg(feature = "reimplementation-tests")]
        {
            if super::overrides::FOLLOW.lock().ok().and_then(|hook| *hook).is_some_and(|hook| hook(this as u32, guide as u32)) {
                return;
            }
        }
        ZTGuest(this as u32).follow_guide(guide as u32);
    }

    #[detour(VIEW_NEARBY_EXHIBIT)]
    unsafe extern "thiscall" fn view_nearby_exhibit(this: *const u32) {
        ZTGuest(this as u32).view_nearby_exhibit();
    }

    #[detour(DO_BUILDING)]
    unsafe extern "thiscall" fn do_building(this: *const u32) {
        ZTGuest(this as u32).do_building();
    }

    #[detour(SET_VISIBLE)]
    unsafe extern "thiscall" fn set_visible(this: *const u32, visible: bool) {
        ZTGuest(this as u32).set_visible(visible);
    }

    #[detour(RUN_SCREAMING)]
    unsafe extern "thiscall" fn run_screaming(this: *const u32) {
        ZTGuest(this as u32).run_screaming();
    }

    #[detour(START_PUKING)]
    unsafe extern "thiscall" fn start_puking(this: *const u32) {
        ZTGuest(this as u32).start_puking();
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn check_viewing_area(this: u32, direction: u32) -> bool {
            unsafe { super::CHECK_VIEWING_AREA_DETOUR.call(this as *const u32, direction) }
        }
        pub(crate) fn find_viewing_area(this: u32) -> bool {
            unsafe { super::FIND_VIEWING_AREA_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn follow_guide(this: u32, guide: u32) {
            unsafe { super::FOLLOW_GUIDE_DETOUR.call(this as *const u32, guide as *const c_void) }
        }
        pub(crate) fn view_nearby_exhibit(this: u32) {
            unsafe { super::VIEW_NEARBY_EXHIBIT_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn do_building(this: u32) {
            unsafe { super::DO_BUILDING_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn set_visible(this: u32, visible: bool) {
            unsafe { super::SET_VISIBLE_DETOUR.call(this as *const u32, visible) }
        }
        pub(crate) fn run_screaming(this: u32) {
            unsafe { super::RUN_SCREAMING_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn start_puking(this: u32) {
            unsafe { super::START_PUKING_DETOUR.call(this as *const u32) }
        }
    }
}

/// Registers the viewing-area detours.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise ztguest viewing detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    pub(crate) use super::detours::test_real as real;

    pub fn detour_status() -> Vec<(&'static str, bool)> {
        super::detours::status()
    }
}
