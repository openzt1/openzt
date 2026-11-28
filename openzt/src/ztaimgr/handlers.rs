//! The entity handlers `fEndTrick`, the building transitions, `fHidePredator`, the `f*Routine` family, the predator/prey handlers and the face-toward handlers.

use super::*;
use crate::ztunit::ZTUnit;

/// `fEndTrick`: ends the trick on a unit, or just finishes for a null/non-unit target.
pub unsafe fn f_end_trick(unit: u32, done: *const bool) -> bool {
    if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        unsafe { call_entity_virtual(unit, ENTITY_VTABLE_END_TRICK) };
    }
    unsafe { set_done(done) };
    true
}

/// `fEnterBuilding`/`fReturnToBuilding`. Returns `false` without acting when `unit` is null or not a unit
/// type: vanilla then calls a virtual through a null object and faults, so the caller defers to it.
pub(super) unsafe fn building_transition(unit: u32, done: *const bool, slot: u32) -> bool {
    unsafe { building_transition_al(unit, done, slot) }.is_some()
}

/// [`building_transition`] keeping the virtual's `AL`, which vanilla's handler returns unchanged (the virtuals
/// are not bool functions, so it can be any byte). `None` when it defers to vanilla.
pub(super) unsafe fn building_transition_al(unit: u32, done: *const bool, slot: u32) -> Option<u8> {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        return None;
    }
    let al = unsafe { call_virtual_args(unit, slot, []) } as u8;
    unsafe { set_done(done) };
    Some(al)
}

pub unsafe fn f_enter_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_ENTER_BUILDING) }
}

pub unsafe fn f_return_to_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_RETURN_TO_BUILDING) }
}

/// [`f_enter_building`] returning the entity virtual's `AL`.
pub unsafe fn f_enter_building_al(unit: u32, done: *const bool) -> Option<u8> {
    unsafe { building_transition_al(unit, done, ENTITY_VTABLE_ENTER_BUILDING) }
}

/// [`f_return_to_building`] returning the entity virtual's `AL`.
pub unsafe fn f_return_to_building_al(unit: u32, done: *const bool) -> Option<u8> {
    unsafe { building_transition_al(unit, done, ENTITY_VTABLE_RETURN_TO_BUILDING) }
}

/// [`f_exit_building`] returning the entity virtual's `AL`.
pub unsafe fn f_exit_building_al(unit: u32, done: *const bool) -> Option<u8> {
    unsafe { building_transition_al(unit, done, ENTITY_VTABLE_EXIT_BUILDING) }
}

/// `fHidePredator` for a guest: hides the guest's predator if it is an animal, then finishes. Returns
/// `false` without acting for a null/non-guest `unit`, which vanilla passes to `getPredatorUnit` as null.
pub unsafe fn f_hide_predator(unit: u32, done: *const bool) -> bool {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUEST) } {
        return false;
    }
    let predator = ZTUnit(unit).get_predator_unit();
    if predator != 0 && unsafe { unit_is(predator, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTANIMAL) } {
        unsafe { call_virtual_args(predator, ENTITY_VTABLE_HIDE_PREDATOR, [0]) };
    }
    unsafe { set_done(done) };
    true
}

/// `fGuestRoutine`/`fMaintRoutine`/`fGuideRoutine`: run the entity's `fRoutine` when its type is exactly
/// `tag_rva`. Always reports success.
pub(super) unsafe fn exact_class_routine(unit: u32, done: *const bool, tag_rva: u32, routine: unsafe fn(u32, *const bool)) -> bool {
    if unsafe { unit_is(unit, TYPE_VTABLE_IS_CLASS, tag_rva) } {
        unsafe { routine(unit, done) };
    }
    true
}

pub(super) unsafe fn guest_f_routine(unit: u32, done: *const bool) {
    crate::ztmaint::ZTMaint::f_routine(unit, done);
}

pub(super) unsafe fn guide_f_routine(unit: u32, done: *const bool) {
    crate::ztguide::ZTGuide::f_routine(unit, done);
}

pub unsafe fn f_guest_routine(unit: u32, done: *const bool) -> bool {
    unsafe { exact_class_routine(unit, done, RVA_TAG_ZTGUEST, guest_f_routine) }
}

/// `ZTMaint` runs the same (ICF-folded) `fRoutine` body as `ZTGuest`.
pub unsafe fn f_maint_routine(unit: u32, done: *const bool) -> bool {
    unsafe { exact_class_routine(unit, done, RVA_TAG_ZTMAINT, guest_f_routine) }
}

pub unsafe fn f_guide_routine(unit: u32, done: *const bool) -> bool {
    unsafe { exact_class_routine(unit, done, RVA_TAG_ZTGUIDE, guide_f_routine) }
}

/// `fKeeperRoutine`: the keeper's `fRoutine` result, or `false` for a non-keeper.
pub unsafe fn f_keeper_routine(unit: u32, done: *const bool) -> bool {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTKEEPER) } {
        return false;
    }
    crate::ztkeeper::ZTKeeper(unit).f_routine(done)
}

/// `ZTUnit` field `fShowPredator` clears and `fCaughtPrey` stores the hunter's id into on the prey.
pub(super) const UNIT_SHOW_PREDATOR_FLAG_OFFSET: u32 = 0x20d;
pub(super) const UNIT_PREY_HUNTER_FIELD_OFFSET: u32 = 0x22c;
/// Entity field read by `fCaughtPrey` and stored into the prey's [`UNIT_PREY_HUNTER_FIELD_OFFSET`].
pub(super) const ENTITY_ID_OFFSET: u32 = 0x124;
/// Entity vtable slots: refresh after the predator/prey change (`+0x254`), find a fallback prey (`+0x25c`),
/// the fallback when there is none (`+0x258`), the unit's habitat (`+0x24c`), exit building (`+0x2ac`).
pub(super) const ENTITY_VTABLE_REFRESH: u32 = 0x254;
pub(super) const ENTITY_VTABLE_FIND_PREY: u32 = 0x25c;
pub(super) const ENTITY_VTABLE_NO_PREY: u32 = 0x258;
pub(super) const ENTITY_VTABLE_GET_HABITAT: u32 = 0x24c;
pub(super) const ENTITY_VTABLE_EXIT_BUILDING: u32 = 0x2ac;
/// Message id and flags `fDie` posts at the animal's tile.
pub(super) const DEATH_MESSAGE_ID: u32 = 0x2802;
pub(super) const DEATH_MESSAGE_KIND: u32 = 2;

pub(super) unsafe fn call_entity_virtual_ret(entity: u32, slot: u32) -> u32 {
    unsafe { call_virtual_args(entity, slot, []) }
}

pub(super) unsafe fn is_animal(unit: u32) -> bool {
    unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTANIMAL) }
}

/// `fShowPredator`: has a guest's predator consume the guest, then refreshes and clears the show flag.
/// `None` (defer to vanilla) for a null/non-guest unit, which vanilla then dereferences as null.
pub unsafe fn f_show_predator(unit: u32, done: *const bool) -> Option<bool> {
    if !unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUEST) } {
        return None;
    }
    let predator = ZTUnit(unit).get_predator_unit();
    if predator != 0 && unsafe { is_animal(predator) } {
        crate::ztanimal::ZTAnimal(predator).consume(unit);
    }
    unsafe { call_entity_virtual(unit, ENTITY_VTABLE_REFRESH) };
    unsafe { ((unit + UNIT_SHOW_PREDATOR_FLAG_OFFSET) as *mut u8).write_volatile(0) };
    unsafe { set_done(done) };
    Some(true)
}

/// `fCaughtPrey`: the animal consumes its target (or the fallback prey vanilla looks up), the prey records
/// the hunter, and the prey refreshes. Always reports success.
pub unsafe fn f_caught_prey(unit: u32, done: *const bool) -> bool {
    if unit != 0 && unsafe { is_animal(unit) } {
        let mut prey = ZTUnit(unit).get_target_unit();
        if prey == 0 {
            prey = unsafe { call_entity_virtual_ret(unit, ENTITY_VTABLE_FIND_PREY) };
            if prey == 0 {
                unsafe { call_entity_virtual(unit, ENTITY_VTABLE_NO_PREY) };
                unsafe { set_done(done) };
                return true;
            }
        }
        crate::ztanimal::ZTAnimal(unit).consume(prey);
        let hunter_id = get_from_memory::<u32>(unit + ENTITY_ID_OFFSET);
        unsafe { ((prey + UNIT_PREY_HUNTER_FIELD_OFFSET) as *mut u32).write_volatile(hunter_id) };
        unsafe { call_entity_virtual(prey, ENTITY_VTABLE_REFRESH) };
    }
    unsafe { set_done(done) };
    true
}

/// `fKillPrey`: removes the animal's target. `true` only when a target was killed.
pub unsafe fn f_kill_prey(unit: u32, done: *const bool) -> bool {
    let mut killed = false;
    if unit != 0 && unsafe { is_animal(unit) } {
        let prey = ZTUnit(unit).get_target_unit();
        if prey != 0 {
            unsafe { call_virtual_args(prey, ENTITY_VTABLE_HIDE_PREDATOR, [0]) };
            ZTUnit(prey).set_is_removed(1);
            killed = true;
        }
    }
    unsafe { set_done(done) };
    killed
}

/// `fDie`: removes a dying animal, posts the death message at its tile and tells its habitat. `true` only
/// when the animal was removed here.
pub unsafe fn f_die(unit: u32, done: *const bool) -> bool {
    let mut removed = false;
    if unit != 0 && unsafe { is_animal(unit) } {
        if unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
            ZTUnit(unit).set_is_removed(1);
            let tile = unsafe { BFENTITY_GET_TILE.original()(unit as *const u32) };
            ZTUnit(unit).f_zoo_message_tile(DEATH_MESSAGE_ID, DEATH_MESSAGE_KIND as i32, tile as u32, 0, 1);
            removed = true;
        }
        let habitat = unsafe { call_entity_virtual_ret(unit, ENTITY_VTABLE_GET_HABITAT) };
        if habitat != 0 {
            unsafe { TRIGGER_DEATH_ARRIVED.hooked()(habitat as *const u32, 0) };
        }
    }
    unsafe { set_done(done) };
    removed
}

/// `fHatch`: finishes the animal's hatch. `false` (defer to vanilla) for a null/non-animal unit, where
/// vanilla calls `finishHatch` with a null `this`.
pub unsafe fn f_hatch(unit: u32, done: *const bool) -> bool {
    if !unsafe { is_animal(unit) } {
        return false;
    }
    crate::ztanimal::ZTAnimal(unit).finish_hatch();
    unsafe { set_done(done) };
    true
}

pub unsafe fn f_exit_building(unit: u32, done: *const bool) -> bool {
    unsafe { building_transition(unit, done, ENTITY_VTABLE_EXIT_BUILDING) }
}

/// `fFaceTowardFood`: an animal faces its food; anything else just finishes. Always reports success.
pub unsafe fn f_face_toward_food(unit: u32, done: *const bool) -> bool {
    if unsafe { is_animal(unit) } {
        unsafe { FACE_TOWARD_FOOD.original()(unit as *const u32) };
    }
    unsafe { set_done(done) };
    true
}

/// `fFaceTowardTarget`: staff and guests face their target by position, other units face their target
/// unit; a non-unit just finishes. `false` (defer to vanilla) for a staff/guest target that is not a unit
/// type, where vanilla calls `faceTowardTarget` with a null `this`.
pub unsafe fn f_face_toward_target(unit: u32, done: *const bool) -> bool {
    let is_staff = unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTSTAFF) };
    let is_guest = unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUEST) };
    let is_unit = unsafe { unit_is(unit, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) };
    if !is_unit {
        if is_staff || is_guest {
            return false;
        }
    } else if is_staff || is_guest {
        ZTUnit(unit).face_toward_target();
    } else {
        ZTUnit(unit).face_toward_target_unit();
    }
    unsafe { set_done(done) };
    true
}
