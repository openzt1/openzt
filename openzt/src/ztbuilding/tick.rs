//! `ZTBuilding`'s tick, animations, sounds, water capacity and events: the per-frame update, the random "used"
//! animation and its timer, the use and ambient sounds, the water tiles behind a building's water exit, the behaviour
//! set name a rented ride carries away, and what a show's events make the audience do.
//!
//! The `ZTScenery` update and dead animation, `sndutil::computeVolumeAndPan`, the sound objects' and sound manager's
//! virtual methods and the mixer's container helpers are still vanilla's. Everything pushed onto the mixer is allocated
//! by vanilla.
//!
//! Where vanilla faults, the port does this instead:
//! - a building without a type: no sound starts, nothing animates and the behaviour set name has no type name;
//! - a type that is not a `ZTBuildingType`: the use and idle animations are not set, the water capacity and water tile
//!   count are `0` and no audience reacts;
//! - no `ZTAIMgr`: the random-use timer is neither read nor set;
//! - no `ZTHabitatMgr`: no show is seen;
//! - a water exit off the map: it has no water;
//! - a draw position past a slot's draw users: nobody is there;
//! - `isAnimalType` on an entity without a type, or whose type is not an animal type: `false`.

use std::{
    collections::HashSet,
    ffi::c_void,
    mem::{offset_of, size_of},
};

use glam::IVec3;
use openzt_detour::{
    generated::{
        bfentitytype::FIND_ANIM,
        bfmap::{GET_NEIGHBOR_0 as MAP_GET_NEIGHBOR, WORLD_TO_TILE},
        bftile::IS_IN_ZOO,
        gxmixer::{SET_BASE_0 as MIXER_SET_BASE, SET_BASE_1 as LINK_VECTOR_INSERT},
        msvc_std_dequegxmixerchain::REALLOCATE_MAP_0X612ABE as QUEUE_GROW_MAP,
        msvc_std_vectorgxmixerlink::{
            CONSTRUCT_0X612956 as LINK_VECTOR_CONSTRUCT, COPY_CTOR_0X4A047E as LINK_VECTOR_COPY, DESTRUCTOR_0X411C0F as LINK_VECTOR_DESTRUCTOR,
        },
        poolalloc::{ALLOCATE as POOLALLOC_ALLOCATE, DEALLOCATE as POOLALLOC_DEALLOCATE},
        sndutil::COMPUTE_VOLUME_AND_PAN,
        standalone::OPERATOR_DELETE,
        ztbuilding::{
            CALCULATE_RELEVANT_WATER_TILES, CALCULATE_WATER_CAPACITY, FORCE_DEAD_ANIM, GET_NUM_RETURN_MOVES, GET_RETURN_BS,
            INIT_RANDOM_USE_TIMER, IS_ANIMAL_TYPE, LISTEN, MIGHT_START_AMBIENT_SOUND, MIGHT_START_IDLE_ANIMATION, MIGHT_START_USE_ANIMATION,
            MIGHT_START_USE_SOUND, MIGHT_STOP_AMBIENT_SOUND, PLAY_USE_ANIMATION, UPDATE, UPDATE_RANDOM_USE_TIMER,
        },
        ztscenery::{FORCE_DEAD_ANIM as SCENERY_FORCE_DEAD_ANIM, UPDATE as SCENERY_UPDATE},
    },
};
use openzt_detour_macro::detour_mod;
use windows::Win32::System::SystemInformation::GetTickCount;

use super::{
    building_offsets,
    lifecycle::{app_byte, new_sound},
    users::{call0, call1, cast, virtual_method, word, ANIMAL_TAG_RVA, GUEST_TAG_RVA, TRASH_CAN_TYPE_ID},
    ZTBuilding, ENTITY_ID, ENTITY_TYPE,
};
use crate::{
    bfentitytype::{ZTBuildingSlotType, ZTBuildingType},
    globals::{get_module_base, globals},
    util::{get_from_memory, low_byte_bool, ref_from_memory, save_to_memory},
    vanilla_string::{c_string_bytes, object_bytes, VanillaString},
    zthabitat::next_game_random,
    zthabitatmgr::ZTHabitatMgr,
    ztguest::ZTGuest,
    ztunit::map_tile,
};


/// `BFEntity` fields: the time of the next random use animation, the mixer, its queue of animation chains, the on-screen
/// position and the rotation.
const NEXT_USE_ANIM_TIME: u32 = 0x138;
const MIXER: u32 = 0x74;
const QUEUE: u32 = 0x8c;
const VIRTUAL_POS: u32 = 0xb4;
const ROTATION: u32 = 0x12c;
/// The queue's `finish` iterator `{cur, first, last, node}`, and the size of its blocks and elements.
const QUEUE_FINISH: u32 = 0x10;
const QUEUE_BLOCK_BYTES: u32 = 0x1f8;
const LINK_VECTOR_BYTES: u32 = 0xc;
/// A `GXMixerLink`: `{anim, sound, flag}`.
const LINK_SIZE: u32 = 12;
/// A loaded animation's last-use time and reference count.
const ANIM_LAST_USE: u32 = 0x1c;
const ANIM_REFCOUNT: u32 = 0x20;
/// RVAs: the cell holding `GLOBAL_BFAnimCache` (current time at `+0x20`), the "used" and "idle" animation names.
const ANIM_CACHE_RVA: u32 = 0x0023_8044;
const ANIM_CACHE_TIME: u32 = 0x20;
const USED_ANIM_RVA: u32 = 0x0023_b050;
const IDLE_ANIM_RVA: u32 = 0x0023_87d4;
/// The `ZTAIMgr` clock.
const AI_CLOCK: u32 = 0xec;

/// `ZTBuildingType` fields not in the struct: the sound names, the use sound's loop flag and the attenuations, the random
/// use animation's flag and delay range, and how many chains one use animation queues.
const TYPE_USE_SOUND: u32 = 0x1dc;
const TYPE_USE_SOUND_LOOPS: u32 = 0x1e8;
const TYPE_USE_ATTENUATION: u32 = 0x1ec;
const TYPE_AMBIENT_SOUND: u32 = 0x1f0;
const TYPE_AMBIENT_ATTENUATION: u32 = 0x1fc;
const TYPE_RANDOM_USE: u32 = 0x241;
const TYPE_RANDOM_USE_MIN: u32 = 0x244;
const TYPE_RANDOM_USE_MAX: u32 = 0x248;
const TYPE_USE_ANIM_COUNT: u32 = 0x24c;
/// `BFEntityType`'s name string.
const TYPE_NAME: u32 = 0xa4;

/// RVAs: the sound manager cell, the ambient sound throttle (its first-use flag and the next start time) and the
/// behaviour set prefix of a rented ride.
const SOUND_MGR_RVA: u32 = 0x0023_80a8;
const AMBIENT_THROTTLE_FLAG_RVA: u32 = 0x0023_94f0;
const AMBIENT_NEXT_RVA: u32 = 0x0023_94e8;
const RETURN_PREFIX_RVA: u32 = 0x0023_cbb8;
/// How long after an ambient sound starts before the next may, and the quietest total at which one still plays.
const AMBIENT_INTERVAL: u32 = 1000;
const AMBIENT_AUDIBLE: i32 = 0x898;
/// The range of the sounds' fade volumes, silence last, and the fade per millisecond.
const SILENT: i32 = 5000;
const FADE_RATE: i32 = 3;
/// The app byte that keeps users inside a building showing its dead animation.
const APP_KEEP_USERS: u32 = 0x441;

/// Show events: a show starting, and those that have the audience react (`CHEERS` the ones that also end the show).
const EVENT_SHOW_START: u32 = 0x2711;
const EVENT_NOT_WATCHABLE: u32 = 0x2717;
const CHEERS: [u32; 3] = [0x2715, 0x2717, 0x2719];
const REACTIONS: [u32; 7] = [0x2715, 0x2717, 0x2719, 0x272a, 0x272b, 0x272c, 0x272d];
/// The flag a building sends its audience events with, and the size of a `BFEvent`.
const AUDIENCE_FLAG: u8 = 0x55;
const EVENT_SIZE: u32 = 0x1c;

/// Tile fields: the 2x2 occupants, the four fences and the water bits.
const TILE_OCCUPANTS: u32 = 0x04;
const TILE_FENCES: u32 = 0x14;
const TILE_WATER_FLAGS: u32 = 0x83;
const TILE_WATER_MASK: u8 = 0x3;
/// The occupant cell of a neighbouring tile that the building must stand on to have its fences checked.
const NEIGHBOR_CELL: u32 = 0x10;
/// The directions the water search spreads in, each with the offset of the neighbour's fence facing back.
const SPREAD: [(u32, u32); 4] = [(0, 0x1c), (4, 0x14), (2, 0x20), (6, 0x18)];

/// Vtable slots.
mod slots {
    /// `ZTBuilding::getEvents(vector<BFEvent>&)`.
    pub const GET_EVENTS: u32 = 0x8;
    /// Types: `isAmphibious`.
    pub const IS_AMPHIBIOUS: u32 = 0xcc;
    /// `DX8SndMgr`: loads the named sound into an `SNDSound`.
    pub const ACQUIRE_SOUND: u32 = 0x10;
    /// `SNDSound`: reload through a manager, whether it holds a sound, play at a position, move, play looped, set the
    /// base and fade attenuation, the two busy tests and stop.
    pub const RELOAD: u32 = 0x8;
    pub const VALID: u32 = 0x14;
    pub const PLAY: u32 = 0x24;
    pub const MOVE: u32 = 0x30;
    pub const PLAY_LOOPED: u32 = 0x34;
    pub const SET_ATTENUATION: u32 = 0x48;
    pub const SET_FADE: u32 = 0x4c;
    pub const IS_PLAYING: u32 = 0x50;
    pub const IS_IN_PLAY_LIST: u32 = 0x54;
    pub const STOP: u32 = 0x60;
}

fn base() -> u32 {
    get_module_base("zoo.exe") as u32
}

fn byte(address: u32) -> u8 {
    get_from_memory(address)
}

fn answers_true(object: u32, slot: u32) -> bool {
    low_byte_bool(call0(object, slot))
}

/// `(sound, manager, name)` reload.
fn reload(sound: u32, manager: u32, name: u32) {
    unsafe { virtual_method::<extern "thiscall" fn(u32, u32, u32)>(sound, slots::RELOAD)(sound, manager, name) };
}

/// Whether the vector (or string) at `address` is empty.
fn is_empty_range(address: u32) -> bool {
    word(address) == word(address + 4)
}

/// How many fade steps `elapsed` milliseconds make.
fn fade_step(elapsed: u32) -> i32 {
    (elapsed as i32).wrapping_mul(FADE_RATE)
}

/// The water moves a ride gets from `tiles` of water, vanilla's signed rounding: one per ten started, at most ten.
pub(crate) fn return_moves(tiles: i32) -> i32 {
    (tiles.wrapping_add(7) / 10).min(10)
}

/// The finished events of a `vector<BFEvent>` go back where vanilla puts them.
fn free_events(begin: u32, capacity: u32) {
    if begin == 0 {
        return;
    }
    let bytes = (capacity.wrapping_sub(begin) as i32 / EVENT_SIZE as i32) as u32 * EVENT_SIZE;
    if bytes > 0x80 {
        unsafe { OPERATOR_DELETE.original()(begin) };
    } else {
        unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) };
    }
}

/// A vector of one link to `anim` (playing `sound`), as vanilla builds it for the mixer, handed to `use_it`; the
/// animation's reference count is back where it started plus whatever `use_it` kept.
// The `1` passed to the insert is an element count in a pointer-typed argument, not a dangling pointer.
#[allow(clippy::manual_dangling_ptr)]
fn with_link_vector(anim: u32, sound: u32, use_it: impl FnOnce(u32)) {
    let adjust = |anim: u32, by: i32| save_to_memory(anim + ANIM_REFCOUNT, get_from_memory::<i32>(anim + ANIM_REFCOUNT).wrapping_add(by));
    let link = [anim, sound, 0];
    adjust(anim, 1);
    save_to_memory(anim + ANIM_LAST_USE, word(word(base() + ANIM_CACHE_RVA) + ANIM_CACHE_TIME));
    let mut links = [0u32; 3];
    let header = links.as_mut_ptr();
    unsafe { LINK_VECTOR_INSERT.original()(header, std::ptr::null(), link.as_ptr() as *const i32, 1 as *const i32) };
    use_it(header as u32);
    // Volatile: vanilla filled the header.
    let [begin, end, capacity]: [u32; 3] = get_from_memory(header as u32);
    for entry in (begin..end).step_by(LINK_SIZE as usize) {
        if word(entry) != 0 {
            adjust(word(entry), -1);
        }
    }
    if begin != 0 {
        let bytes = capacity.wrapping_sub(begin) / LINK_SIZE * LINK_SIZE;
        unsafe { POOLALLOC_DEALLOCATE.original()(begin as *const u32, bytes) };
    }
    adjust(anim, -1);
}

/// `ZTBuilding::isAnimalType` (`0x00446c9a`): whether `entity`'s type is an animal type that is amphibious. Its callers
/// pass animals, not buildings.
pub fn is_animal_type(entity: u32) -> bool {
    if cast(entity, ANIMAL_TAG_RVA) == 0 {
        return false;
    }
    answers_true(word(entity + ENTITY_TYPE), slots::IS_AMPHIBIOUS)
}

impl ZTBuilding {
    /// Makes `anim` the whole base animation of the building's mixer.
    pub(super) fn set_base_animation(self, anim: u32) {
        with_link_vector(anim, 0, |links| unsafe { MIXER_SET_BASE.original()((self.0 + MIXER) as *const u32, links as *const i32, 1) });
    }

    /// Appends a copy of the link vector at `links` to the mixer's queue, as `std::deque::push_back` does.
    fn queue_chain(self, links: u32) {
        let queue = self.0 + QUEUE;
        let finish = queue + QUEUE_FINISH;
        let (cur, last) = (word(finish), word(finish + 8));
        if cur != last.wrapping_sub(LINK_VECTOR_BYTES) {
            if cur != 0 {
                unsafe { LINK_VECTOR_COPY.original()(cur as *const u32, links as *const i32) };
            }
            save_to_memory(finish, cur.wrapping_add(LINK_VECTOR_BYTES));
            return;
        }
        // The last element of a block: the next block and its map slot come first.
        let mut copy = [0u32; 3];
        let copy = copy.as_mut_ptr() as u32;
        unsafe {
            LINK_VECTOR_COPY.original()(copy as *const u32, links as *const i32);
            QUEUE_GROW_MAP.original()(queue as *const u32, 1);
        }
        let block = unsafe { POOLALLOC_ALLOCATE.original()(QUEUE_BLOCK_BYTES) } as u32;
        let node = word(finish + 12);
        save_to_memory(node + 4, block);
        unsafe { LINK_VECTOR_CONSTRUCT.original()(word(finish) as *const u32, copy as *const i32) };
        let node = word(finish + 12) + 4;
        let first = word(node);
        save_to_memory(finish + 12, node);
        save_to_memory(finish + 4, first);
        save_to_memory(finish + 8, first.wrapping_add(QUEUE_BLOCK_BYTES));
        save_to_memory(finish, first);
        unsafe { LINK_VECTOR_DESTRUCTOR.original()(copy as *const u32) };
    }

    /// The named animation of the type when it is a building type other than a trash can's, `0` otherwise.
    fn state_animation(self, name_rva: u32) -> u32 {
        let building_type = self.building_type();
        if building_type == 0 || self.type_id() == TRASH_CAN_TYPE_ID {
            return 0;
        }
        unsafe { FIND_ANIM.hooked()(building_type as *const u32, (base() + name_rva) as *const i8) as u32 }
    }

    /// `ZTBuilding::mightStartUseAnimation` (`0x0042d01c`): the first user switches the building to its "used"
    /// animation.
    pub fn might_start_use_animation(self) {
        #[cfg(feature = "reimplementation-tests")]
        if super::live_support::intercept::might_start_use_animation(self.0) {
            return;
        }
        if self.occupancy() != 1 {
            return;
        }
        let anim = self.state_animation(USED_ANIM_RVA);
        if anim != 0 {
            self.set_base_animation(anim);
        }
    }

    /// `ZTBuilding::mightStartIdleAnimation` (`0x0042dc52`): an empty building goes back to its "idle" animation.
    pub fn might_start_idle_animation(self) {
        if self.occupancy() != 0 {
            return;
        }
        let anim = self.state_animation(IDLE_ANIM_RVA);
        if anim != 0 {
            self.set_base_animation(anim);
        }
    }

    /// `ZTBuilding::playUseAnimation` (`0x004a067b`): queues the "used" animation as many times as the type says, the
    /// first with the use sound.
    pub fn play_use_animation(self) {
        let building_type = self.entity_type();
        if building_type == 0 {
            return;
        }
        let anim = unsafe { FIND_ANIM.hooked()(building_type as *const u32, (base() + USED_ANIM_RVA) as *const i8) } as u32;
        if anim == 0 {
            return;
        }
        let mut sound = 0;
        let held: u32 = self.read(building_offsets::USE_SOUND);
        if held != 0 {
            if answers_true(held, slots::VALID) {
                sound = self.read(building_offsets::USE_SOUND);
            } else {
                let name = word(building_type + TYPE_USE_SOUND);
                reload(self.read(building_offsets::USE_SOUND), word(base() + SOUND_MGR_RVA), name);
                let held: u32 = self.read(building_offsets::USE_SOUND);
                if answers_true(held, slots::VALID) {
                    let held: u32 = self.read(building_offsets::USE_SOUND);
                    call1(held, slots::SET_ATTENUATION, word(building_type + TYPE_USE_ATTENUATION));
                    sound = self.read(building_offsets::USE_SOUND);
                }
            }
        }
        for _ in 0..word(building_type + TYPE_USE_ANIM_COUNT) {
            with_link_vector(anim, sound, |links| self.queue_chain(links));
            sound = 0;
        }
    }

    /// `ZTBuilding::initRandomUseTimer` (`0x004ab763`): for a type that plays its use animation at random, the next
    /// time is drawn from its delay range.
    pub fn init_random_use_timer(self) {
        let building_type = self.entity_type();
        if building_type == 0 || byte(building_type + TYPE_RANDOM_USE) == 0 {
            return;
        }
        let shortest: i32 = get_from_memory(building_type + TYPE_RANDOM_USE_MIN);
        let range = get_from_memory::<i32>(building_type + TYPE_RANDOM_USE_MAX).wrapping_sub(shortest);
        let extra = if range > 0 { next_game_random() as i32 % range } else { 0 };
        let ai = globals().ztaimgr_ptr() as u32;
        if ai != 0 {
            let clock: i32 = get_from_memory(ai + AI_CLOCK);
            self.write(NEXT_USE_ANIM_TIME, clock.wrapping_add(shortest).wrapping_add(extra));
        }
    }

    /// `ZTBuilding::updateRandomUseTimer` (`0x00434d2a`).
    pub fn update_random_use_timer(self) {
        let building_type = self.entity_type();
        let ai = globals().ztaimgr_ptr() as u32;
        if building_type == 0 || byte(building_type + TYPE_RANDOM_USE) == 0 || ai == 0 {
            return;
        }
        if word(ai + AI_CLOCK) > self.read(NEXT_USE_ANIM_TIME) {
            self.play_use_animation();
            self.init_random_use_timer();
        }
    }

    /// The common start of the use and ambient sounds: nothing while the `SNDSound` in `field` is busy or there is no
    /// sound manager, else the manager loads the named sound into it, making it first. Answers it when it holds a sound.
    fn load_sound(self, field: u32, name: u32) -> Option<u32> {
        let sound: u32 = self.read(field);
        if sound != 0 && answers_true(sound, slots::IS_PLAYING) {
            return None;
        }
        let sound: u32 = self.read(field);
        if sound != 0 && answers_true(sound, slots::IS_IN_PLAY_LIST) {
            return None;
        }
        let manager = word(base() + SOUND_MGR_RVA);
        if manager == 0 {
            return None;
        }
        if self.read::<u32>(field) == 0 {
            self.write(field, new_sound());
        }
        let sound: u32 = self.read(field);
        if sound != 0 {
            unsafe { virtual_method::<extern "thiscall" fn(u32, u32, u32)>(manager, slots::ACQUIRE_SOUND)(manager, sound, word(name)) };
        }
        let sound: u32 = self.read(field);
        (sound != 0 && answers_true(sound, slots::VALID)).then_some(sound)
    }

    /// Starts a loaded sound silent and looped, for [`Self::update`] to fade in.
    fn start_looped(self, field: u32, volume_field: u32) {
        self.write(volume_field, 0i32);
        call1(self.read(field), slots::SET_FADE, 0);
        call1(self.read(field), slots::PLAY_LOOPED, self.0 + VIRTUAL_POS);
    }

    /// `ZTBuilding::mightStartUseSound` (`0x0042cc1a`): starts the use sound, looped for a type that loops it while
    /// anyone is inside.
    pub fn might_start_use_sound(self) {
        #[cfg(feature = "reimplementation-tests")]
        if super::live_support::intercept::might_start_use_sound(self.0) {
            return;
        }
        let building_type = self.entity_type();
        if building_type == 0 || is_empty_range(building_type + TYPE_USE_SOUND) {
            return;
        }
        if byte(building_type + TYPE_USE_SOUND_LOOPS) != 0 && self.occupancy() < 1 {
            return;
        }
        let Some(sound) = self.load_sound(building_offsets::USE_SOUND, building_type + TYPE_USE_SOUND) else {
            return;
        };
        call1(sound, slots::SET_ATTENUATION, word(building_type + TYPE_USE_ATTENUATION));
        if byte(building_type + TYPE_USE_SOUND_LOOPS) != 0 {
            self.start_looped(building_offsets::USE_SOUND, building_offsets::USE_SOUND_VOLUME);
        } else {
            call1(self.read(building_offsets::USE_SOUND), slots::PLAY, self.0 + VIRTUAL_POS);
        }
    }

    /// The volume of a sound at the building's position, as the view hears it.
    fn volume_here(self) -> i32 {
        let (mut volume, mut pan) = (0i32, 0i32);
        unsafe { COMPUTE_VOLUME_AND_PAN.original()((self.0 + VIRTUAL_POS) as *const IVec3, &mut volume, &mut pan) };
        // Volatile: vanilla wrote it.
        get_from_memory(&volume as *const i32 as u32)
    }

    /// `ZTBuilding::mightStartAmbientSound` (`0x0041e3f8`): starts the ambient sound when the view is near enough, at
    /// most one building a second.
    pub fn might_start_ambient_sound(self) {
        let flag = base() + AMBIENT_THROTTLE_FLAG_RVA;
        let next = base() + AMBIENT_NEXT_RVA;
        if byte(flag) & 1 == 0 {
            save_to_memory(flag, byte(flag) | 1);
            save_to_memory(next, unsafe { GetTickCount() });
        }
        let now = unsafe { GetTickCount() };
        let building_type = self.entity_type();
        if building_type == 0 || is_empty_range(building_type + TYPE_AMBIENT_SOUND) || now < word(next) {
            return;
        }
        let Some(sound) = self.load_sound(building_offsets::AMBIENT_SOUND, building_type + TYPE_AMBIENT_SOUND) else {
            return;
        };
        let volume = self.volume_here();
        let attenuation: i32 = get_from_memory(building_type + TYPE_AMBIENT_ATTENUATION);
        if attenuation.wrapping_add(volume) > AMBIENT_AUDIBLE {
            return;
        }
        call1(sound, slots::SET_ATTENUATION, attenuation as u32);
        self.start_looped(building_offsets::AMBIENT_SOUND, building_offsets::AMBIENT_SOUND_VOLUME);
        save_to_memory(next, now.wrapping_add(AMBIENT_INTERVAL));
    }

    /// `ZTBuilding::mightStopAmbientSound` (`0x004b0bca`): stops the ambient sound once the view is too far to hear it.
    pub fn might_stop_ambient_sound(self) {
        if self.read::<u32>(building_offsets::AMBIENT_SOUND) == 0 {
            return;
        }
        let volume = self.volume_here();
        let building_type = self.entity_type();
        if building_type == 0 || get_from_memory::<i32>(building_type + TYPE_AMBIENT_ATTENUATION).wrapping_add(volume) <= AMBIENT_AUDIBLE {
            return;
        }
        let sound: u32 = self.read(building_offsets::AMBIENT_SOUND);
        if sound != 0 && answers_true(sound, slots::IS_PLAYING) {
            call0(self.read(building_offsets::AMBIENT_SOUND), slots::STOP);
        }
    }

    /// `ZTBuilding::update` (`0x00434c4b`): the scenery update, the random use animation, the sound fades and, for a
    /// ride that has none yet, the water capacity once a water exit has water.
    pub fn update(self, elapsed: u32) {
        unsafe { SCENERY_UPDATE.hooked()(self.0 as *const c_void, elapsed as i32) };
        let building_type = self.building_type();
        self.update_random_use_timer();
        let step = fade_step(elapsed);

        let use_sound: u32 = self.read(building_offsets::USE_SOUND);
        if use_sound != 0
            && building_type != 0
            && byte(building_type + TYPE_USE_SOUND_LOOPS) != 0
            && answers_true(use_sound, slots::IS_PLAYING)
        {
            let volume: i32 = self.read(building_offsets::USE_SOUND_VOLUME);
            // Quieter (towards silence) while empty, louder while used.
            let faded = if self.occupancy() < 1 { volume.wrapping_add(step) } else { volume.wrapping_sub(step) };
            let faded = faded.clamp(0, SILENT);
            self.write(building_offsets::USE_SOUND_VOLUME, faded);
            call1(self.read(building_offsets::USE_SOUND), slots::SET_FADE, faded as u32);
            call1(self.read(building_offsets::USE_SOUND), slots::MOVE, self.0 + VIRTUAL_POS);
            if self.read::<i32>(building_offsets::USE_SOUND_VOLUME) == SILENT {
                call0(self.read(building_offsets::USE_SOUND), slots::STOP);
            }
        }

        let ambient: u32 = self.read(building_offsets::AMBIENT_SOUND);
        if ambient != 0 {
            if !answers_true(ambient, slots::IS_PLAYING) {
                self.might_start_ambient_sound();
            } else {
                // Vanilla stores the faded ambient volume over the use sound's and plays the unfaded one.
                let volume: i32 = self.read(building_offsets::AMBIENT_SOUND_VOLUME);
                self.write(building_offsets::USE_SOUND_VOLUME, volume.wrapping_sub(step).clamp(0, SILENT));
                call1(self.read(building_offsets::AMBIENT_SOUND), slots::SET_FADE, volume as u32);
                call1(self.read(building_offsets::AMBIENT_SOUND), slots::MOVE, self.0 + VIRTUAL_POS);
                self.might_stop_ambient_sound();
            }
        }

        if building_type == 0 || byte(building_type + offset_of!(ZTBuildingType, return_building) as u32) == 0 {
            return;
        }
        if self.read::<i32>(building_offsets::WATER_CAPACITY) != 0 {
            return;
        }
        for slot in 0..self.slot_count().max(0) as u32 {
            if !self.slot_flag(building_type, slot, offset_of!(ZTBuildingSlotType, exit_on_water)) {
                continue;
            }
            let exit = self.slot_exit(slot);
            let tile = map_tile(exit[0] / 64, exit[1] / 64);
            if tile != 0 && byte(tile + TILE_WATER_FLAGS) & TILE_WATER_MASK != 0 {
                let capacity = self.calculate_water_capacity();
                self.write(building_offsets::WATER_CAPACITY, capacity);
            }
        }
    }

    /// The byte at `field` of the record of `slot` in `building_type`, `false` past the records.
    fn slot_flag(self, building_type: u32, slot: u32, field: usize) -> bool {
        let records = building_type + offset_of!(ZTBuildingType, slots) as u32;
        let size = size_of::<ZTBuildingSlotType>() as u32;
        let (begin, end) = (word(records), word(records + 4));
        let count = (end.wrapping_sub(begin) as i32 / size as i32) as u32;
        slot < count && byte(begin + slot * size + field as u32) != 0
    }

    /// `ZTBuilding::forceDeadAnim` (`0x004174dd`): the scenery's dead animation, then the users leave unless the app
    /// says otherwise.
    pub fn force_dead_anim(self) {
        unsafe { SCENERY_FORCE_DEAD_ANIM.hooked()(self.0 as *const c_void) };
        if app_byte(APP_KEEP_USERS) == 0 {
            self.remove_all_users();
        }
    }

    /// `ZTBuilding::listen` (`0x00434cca`): collects the building's events and acts on the ones of its show. An event
    /// whose show the building cannot see counts as `0x2717`.
    pub fn listen(self) {
        let mut events = [0u32; 3];
        let header = events.as_mut_ptr() as u32;
        let get_events = unsafe { virtual_method::<extern "thiscall" fn(u32, u32)>(self.0, slots::GET_EVENTS) };
        get_events(self.0, header);
        // Volatile: vanilla filled the header.
        let [begin, end, capacity]: [u32; 3] = get_from_memory(header);
        let mut event = begin;
        while event != end {
            self.hear(event);
            event = event.wrapping_add(EVENT_SIZE);
        }
        free_events(begin, capacity);
    }

    /// One `BFEvent`: the value at `+0x10`, a half-word at `+0x14` and the kind at `+0x16`.
    fn hear(self, event: u32) {
        let mut kind = u32::from(get_from_memory::<u16>(event + 0x16));
        let habitats = globals().zthabitatmgr_ptr();
        let show = if habitats.is_null() {
            0
        } else {
            unsafe { ref_from_memory::<ZTHabitatMgr>(habitats as *const u32) }.can_see_show_from_building(self.0) as u16
        };
        if show == 0 || show != self.show_id() {
            kind = EVENT_NOT_WATCHABLE;
        }
        if kind == EVENT_SHOW_START {
            self.set_type_state(1);
            self.user_data_changed();
        } else if REACTIONS.contains(&kind) {
            self.audience_reacts(kind, event);
            if CHEERS.contains(&kind) {
                self.set_type_state(0);
                self.user_data_changed();
            }
        }
    }

    /// Sends the event on to each drawn user of the slots that draw theirs, with a random delay.
    fn audience_reacts(self, kind: u32, event: u32) {
        let building_type = self.building_type();
        if building_type == 0 {
            return;
        }
        let cheering = CHEERS.contains(&kind);
        for slot in 0..self.slot_count().max(0) as u32 {
            if !self.slot_flag(building_type, slot, offset_of!(ZTBuildingSlotType, has_draw_slots)) {
                continue;
            }
            // The number of seats is the length of the record's first draw order.
            let records = word(building_type + offset_of!(ZTBuildingType, slots) as u32);
            let order = records + slot * size_of::<ZTBuildingSlotType>() as u32 + offset_of!(ZTBuildingSlotType, draw_orders) as u32;
            let seats = word(order + 4).wrapping_sub(word(order)) >> 2;
            for seat in 0..seats {
                let entry = self.slot(slot);
                let (begin, end) = entry.draw_users();
                if seat >= end.wrapping_sub(begin) >> 2 {
                    continue;
                }
                let user = word(begin + seat * 4);
                if user == 0 {
                    continue;
                }
                let mut delay = next_game_random() % 6;
                if cheering {
                    delay += 0x5a + next_game_random() % 0x23;
                }
                if cast(user, GUEST_TAG_RVA) != 0 {
                    ZTGuest(user).leaving_show();
                }
                let value = word(event + 0x10);
                let extra: u16 = get_from_memory(event + 0x14);
                self.send_event(kind as u16, word(user + ENTITY_ID), AUDIENCE_FLAG, value, extra, delay as u16);
            }
        }
    }

    /// `ZTBuilding::calculateWaterCapacity` (`0x0059398a`): the type's strict capacity when it has one, else the moves
    /// its water allows.
    pub fn calculate_water_capacity(self) -> i32 {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(capacity) = super::live_support::intercept::calculate_water_capacity(self.0) {
            return capacity;
        }
        let building_type = self.building_type();
        if building_type == 0 {
            return 0;
        }
        let strict: i32 = self.type_field(offset_of!(ZTBuildingType, strict_building_capacity));
        if strict > 0 {
            return strict;
        }
        return_moves(self.relevant_water_tiles())
    }

    /// `ZTBuilding::getNumReturnMoves` (`0x00596fb4`).
    pub fn num_return_moves(self) -> i32 {
        return_moves(self.relevant_water_tiles())
    }

    /// `ZTBuilding::calculateRelevantWaterTiles` (`0x005938c7`): the number of water tiles in the zoo connected to the
    /// water at the first water exit, without crossing a fence or an occupied tile. A building whose exit tile is dry,
    /// occupied or fenced off from the building has none.
    pub fn relevant_water_tiles(self) -> i32 {
        let building_type = self.building_type();
        if building_type == 0 {
            return 0;
        }
        let Some(slot) = (0..self.slot_count().max(0) as u32)
            .find(|&slot| self.slot_flag(building_type, slot, offset_of!(ZTBuildingSlotType, exit_on_water)))
        else {
            return 0;
        };
        let map = globals().ztworldmgr_ptr() as u32 + 8;
        let exit = self.slot_exit(slot);
        let mut position = [0i32; 3];
        unsafe { WORLD_TO_TILE.hooked()(position.as_mut_ptr(), exit.as_ptr()) };
        // Volatile: vanilla wrote it.
        let [x, y, _]: [i32; 3] = get_from_memory(position.as_ptr() as u32);
        let start = map_tile(x, y);
        if start == 0 || !is_open_water(start) {
            return 0;
        }

        let rotation: u32 = self.read(ROTATION);
        let neighbor = |tile: u32, direction: u32| unsafe { MAP_GET_NEIGHBOR.hooked()(map as *const u32, tile as *const u32, direction) };
        let beside = neighbor(start, rotation);
        if beside != 0 && word(beside + NEIGHBOR_CELL) == self.0 {
            let fenced = |tile: u32, direction: u32| word(tile.wrapping_add_signed((direction as i32 / 2) * 4) + TILE_FENCES) != 0;
            if rotation != u32::MAX && (fenced(start, rotation) || fenced(beside, rotation.wrapping_sub(4) & 7)) {
                return 0;
            }
        }

        let mut seen = HashSet::new();
        let mut pending = vec![start];
        while let Some(tile) = pending.pop() {
            seen.insert(tile);
            // Each side is reached only where this tile has no fence.
            let sides = SPREAD.map(|(direction, back)| {
                let fence = word(tile + TILE_FENCES + direction / 2 * 4);
                (if fence == 0 { neighbor(tile, direction) } else { 0 }, back)
            });
            for (side, back) in sides {
                if side != 0
                    && byte(side + TILE_WATER_FLAGS) & TILE_WATER_MASK != 0
                    && low_byte_bool(unsafe { IS_IN_ZOO.hooked()(side as *const u32, 1) })
                    && is_unoccupied(side)
                    && word(side + back) == 0
                    && !seen.contains(&side)
                {
                    pending.push(side);
                }
            }
        }
        seen.len() as i32
    }

    /// `ZTBuilding::getReturnBS` (`0x00597028`): the behaviour set a guest taking this building away runs, "bReturn",
    /// the type's name and the upgrade index.
    pub fn return_behavior_set(self) -> Vec<u8> {
        #[cfg(feature = "reimplementation-tests")]
        if let Some(name) = super::live_support::intercept::get_return_bs(self.0) {
            return name;
        }
        let mut name = c_string_bytes(base() + RETURN_PREFIX_RVA);
        let building_type = self.entity_type();
        if building_type != 0 {
            name.extend(object_bytes(building_type + TYPE_NAME));
        }
        name.extend(self.read::<i32>(building_offsets::UPGRADE_INDEX).to_string().into_bytes());
        name
    }
}

/// Whether a tile holds no entity in any of its quarters.
fn is_unoccupied(tile: u32) -> bool {
    (0..4).all(|quarter| word(tile + TILE_OCCUPANTS + quarter * 4) == 0)
}

fn is_open_water(tile: u32) -> bool {
    byte(tile + TILE_WATER_FLAGS) & TILE_WATER_MASK != 0 && is_unoccupied(tile)
}

#[detour_mod]
mod tick_detours {
    use super::*;

    #[detour(UPDATE)]
    unsafe extern "thiscall" fn update(this: *const u32, elapsed: u32) {
        ZTBuilding(this as u32).update(elapsed);
    }

    #[detour(LISTEN)]
    unsafe extern "thiscall" fn listen(this: *const u32) {
        ZTBuilding(this as u32).listen();
    }

    #[detour(INIT_RANDOM_USE_TIMER)]
    unsafe extern "thiscall" fn init_random_use_timer(this: *const u32) {
        ZTBuilding(this as u32).init_random_use_timer();
    }

    #[detour(UPDATE_RANDOM_USE_TIMER)]
    unsafe extern "thiscall" fn update_random_use_timer(this: *const u32) {
        ZTBuilding(this as u32).update_random_use_timer();
    }

    #[detour(PLAY_USE_ANIMATION)]
    unsafe extern "thiscall" fn play_use_animation(this: *const u32) {
        ZTBuilding(this as u32).play_use_animation();
    }

    #[detour(MIGHT_START_USE_SOUND)]
    unsafe extern "thiscall" fn might_start_use_sound(this: *const u32) {
        ZTBuilding(this as u32).might_start_use_sound();
    }

    #[detour(MIGHT_START_USE_ANIMATION)]
    unsafe extern "thiscall" fn might_start_use_animation(this: *const u32) {
        ZTBuilding(this as u32).might_start_use_animation();
    }

    #[detour(MIGHT_START_IDLE_ANIMATION)]
    unsafe extern "thiscall" fn might_start_idle_animation(this: *const u32) {
        ZTBuilding(this as u32).might_start_idle_animation();
    }

    #[detour(MIGHT_START_AMBIENT_SOUND)]
    unsafe extern "thiscall" fn might_start_ambient_sound(this: *const u32) {
        ZTBuilding(this as u32).might_start_ambient_sound();
    }

    #[detour(MIGHT_STOP_AMBIENT_SOUND)]
    unsafe extern "thiscall" fn might_stop_ambient_sound(this: *const u32) {
        ZTBuilding(this as u32).might_stop_ambient_sound();
    }

    #[detour(FORCE_DEAD_ANIM)]
    unsafe extern "thiscall" fn force_dead_anim(this: *const u32) {
        ZTBuilding(this as u32).force_dead_anim();
    }

    #[detour(CALCULATE_WATER_CAPACITY)]
    unsafe extern "thiscall" fn calculate_water_capacity(this: *const u32) -> i32 {
        ZTBuilding(this as u32).calculate_water_capacity()
    }

    #[detour(CALCULATE_RELEVANT_WATER_TILES)]
    unsafe extern "thiscall" fn calculate_relevant_water_tiles(this: *const u32) -> i32 {
        ZTBuilding(this as u32).relevant_water_tiles()
    }

    #[detour(GET_NUM_RETURN_MOVES)]
    unsafe extern "fastcall" fn get_num_return_moves(this: *const u32) -> u32 {
        ZTBuilding(this as u32).num_return_moves() as u32
    }

    #[detour(GET_RETURN_BS)]
    unsafe extern "thiscall" fn get_return_bs(this: *const c_void, out: *const c_void) -> *const c_void {
        let name = VanillaString::from_bytes(&ZTBuilding(this as u32).return_behavior_set()).release();
        for (index, word) in name.into_iter().enumerate() {
            save_to_memory(out as u32 + index as u32 * 4, word);
        }
        out
    }

    #[detour(IS_ANIMAL_TYPE)]
    unsafe extern "thiscall" fn is_animal_type_detour(this: *const c_void) -> bool {
        is_animal_type(this as u32)
    }

    /// The vanilla bodies, for the live tests.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use std::ffi::c_void;

        pub(crate) fn update(this: u32, elapsed: u32) {
            unsafe { super::UPDATE_DETOUR.call(this as *const u32, elapsed) }
        }

        pub(crate) fn listen(this: u32) {
            unsafe { super::LISTEN_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn init_random_use_timer(this: u32) {
            unsafe { super::INIT_RANDOM_USE_TIMER_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn update_random_use_timer(this: u32) {
            unsafe { super::UPDATE_RANDOM_USE_TIMER_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn play_use_animation(this: u32) {
            unsafe { super::PLAY_USE_ANIMATION_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn might_start_use_sound(this: u32) {
            unsafe { super::MIGHT_START_USE_SOUND_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn might_start_use_animation(this: u32) {
            unsafe { super::MIGHT_START_USE_ANIMATION_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn might_start_idle_animation(this: u32) {
            unsafe { super::MIGHT_START_IDLE_ANIMATION_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn might_start_ambient_sound(this: u32) {
            unsafe { super::MIGHT_START_AMBIENT_SOUND_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn might_stop_ambient_sound(this: u32) {
            unsafe { super::MIGHT_STOP_AMBIENT_SOUND_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn force_dead_anim(this: u32) {
            unsafe { super::FORCE_DEAD_ANIM_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn calculate_water_capacity(this: u32) -> i32 {
            unsafe { super::CALCULATE_WATER_CAPACITY_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn relevant_water_tiles(this: u32) -> i32 {
            unsafe { super::CALCULATE_RELEVANT_WATER_TILES_DETOUR.call(this as *const u32) }
        }

        pub(crate) fn num_return_moves(this: u32) -> i32 {
            unsafe { super::GET_NUM_RETURN_MOVES_DETOUR.call(this as *const u32) as i32 }
        }

        /// The vanilla name, with the string vanilla built freed.
        pub(crate) fn return_behavior_set(this: u32) -> Vec<u8> {
            let mut out = [0u32; 3];
            let out = out.as_mut_ptr() as u32;
            unsafe { super::GET_RETURN_BS_DETOUR.call(this as *const c_void, out as *const c_void) };
            let [begin, end, capacity]: [u32; 3] = crate::util::get_from_memory(out);
            let name = crate::vanilla_string::range_bytes(begin, end);
            if begin != 0 {
                unsafe { openzt_detour::generated::poolalloc::DEALLOCATE.original()(begin as *const u32, capacity - begin) };
            }
            name
        }

        pub(crate) fn is_animal_type(this: u32) -> bool {
            unsafe { super::IS_ANIMAL_TYPE_DETOUR.call(this as *const c_void) }
        }
    }
}

/// Installs the tick detours.
pub(super) fn init() {
    if let Err(e) = unsafe { tick_detours::init_detours() } {
        tracing::error!("Failed to initialise ztbuilding tick detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    tick_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use tick_detours::test_real;

#[cfg(test)]
mod tests {
    use super::{fade_step, return_moves};

    #[test]
    fn one_move_per_ten_tiles_started_at_most_ten() {
        assert_eq!(return_moves(0), 0);
        assert_eq!(return_moves(2), 0);
        assert_eq!(return_moves(3), 1);
        assert_eq!(return_moves(93), 10);
        assert_eq!(return_moves(5000), 10);
        assert_eq!(return_moves(-30), -2);
    }

    #[test]
    fn the_fade_is_three_per_millisecond() {
        assert_eq!(fade_step(10), 30);
        assert_eq!(fade_step(u32::MAX), -3);
    }
}
