//! Raw call-object views and the leaf manager methods (`getRandomX`/`Y`, the move-class tests, `convertConstantParam`, `removeEntity`).

use super::*;

/// A `{begin, end, capacity}` string as `BFFunctionCall` stores its name and each parameter.
#[repr(C)]
pub(super) struct RawCallString {
    pub(super) begin: *const u8,
    pub(super) end: *const u8,
    pub(super) capacity: *const u8,
    pub(super) _live: LiveMemory,
}

impl RawCallString {
    pub(super) fn bytes(&self) -> &[u8] {
        if self.begin.is_null() || self.end < self.begin {
            return &[];
        }
        unsafe { std::slice::from_raw_parts(self.begin, self.end as usize - self.begin as usize) }
    }
}

/// `BFFunctionCall`: the function name plus its parameter vector (0xc-byte entries).
#[repr(C)]
pub(super) struct RawFunctionCall {
    pub(super) name: RawCallString,
    pub(super) params_begin: *const RawCallString,
    pub(super) params_end: *const RawCallString,
    pub(super) params_capacity: *const RawCallString,
    pub(super) _live: LiveMemory,
}

impl RawFunctionCall {
    /// Parameter `index`, or `None` if the call has fewer parameters.
    pub(super) fn param(&self, index: usize) -> Option<&RawCallString> {
        if self.params_begin.is_null() {
            return None;
        }
        let count = (self.params_end as usize - self.params_begin as usize) / mem::size_of::<RawCallString>();
        if index < count {
            Some(unsafe { &*self.params_begin.add(index) })
        } else {
            None
        }
    }

    /// `name == base`, or `name == "fMove"` with the third parameter equal to `variant`. Vanilla reads the
    /// third parameter without a count check; a call with fewer parameters compares as unequal here.
    pub(super) fn is_move_function(&self, base: &[u8], also: &[&[u8]], variant: &[u8]) -> bool {
        let name = self.name.bytes();
        if name == base || also.contains(&name) {
            return true;
        }
        name == b"fMove" && self.param(2).is_some_and(|p| p.bytes() == variant)
    }
}

/// Calls the thiscall virtual at `slot` of `object`'s vtable with `N` (at most five) `u32` arguments and returns
/// `EAX`. A callee that returns a byte or a bool leaves the upper bytes undefined: mask or truncate the result at
/// the call site (`as u8`, `& 0xff`, `util::low_byte_bool`). A callee that returns nothing yields garbage to ignore.
pub(super) unsafe fn call_virtual_args<const N: usize>(object: u32, slot: u32, args: [u32; N]) -> u32 {
    let target = get_from_memory::<u32>(get_from_memory::<u32>(object) + slot);
    unsafe {
        match N {
            0 => mem::transmute::<u32, extern "thiscall" fn(u32) -> u32>(target)(object),
            1 => mem::transmute::<u32, extern "thiscall" fn(u32, u32) -> u32>(target)(object, args[0]),
            2 => mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32) -> u32>(target)(object, args[0], args[1]),
            3 => mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32) -> u32>(target)(object, args[0], args[1], args[2]),
            4 => mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32) -> u32>(target)(object, args[0], args[1], args[2], args[3]),
            5 => mem::transmute::<u32, extern "thiscall" fn(u32, u32, u32, u32, u32, u32) -> u32>(target)(object, args[0], args[1], args[2], args[3], args[4]),
            _ => unreachable!("call_virtual_args takes at most five arguments"),
        }
    }
}

/// Calls `BFEntityType` vtable `slot` with a class tag (`this = type_ptr`).
pub(super) unsafe fn type_tag_check(type_ptr: u32, slot: u32, tag_rva: u32) -> bool {
    (unsafe { call_virtual_args(type_ptr, slot, [get_module_base("zoo.exe") as u32 + tag_rva]) } & 0xff) != 0
}

/// Picks `guest`, `staff` or `default` by the entity's type. Vanilla dereferences the type without a null
/// check; a null type falls to `default` here.
pub(super) unsafe fn random_extent(this: u32, entity: u32, guest: u32, staff: u32, default: u32) -> i32 {
    let entity_type = get_from_memory::<u32>(entity + OFFSET_ENTITY_TYPE);
    if entity_type == 0 {
        return get_from_memory::<i32>(this + default);
    }
    if unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CLASS, RVA_TAG_ZTGUEST) } {
        get_from_memory::<i32>(this + guest)
    } else if unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CLASS, RVA_TAG_ZTSTAFF) } {
        get_from_memory::<i32>(this + staff)
    } else {
        get_from_memory::<i32>(this + default)
    }
}

pub unsafe fn get_random_x(this: u32, entity: u32) -> i32 {
    unsafe { random_extent(this, entity, OFFSET_GUEST_RANDOM_X, OFFSET_STAFF_RANDOM_X, OFFSET_DEFAULT_RANDOM_X) }
}

pub unsafe fn get_random_y(this: u32, entity: u32) -> i32 {
    unsafe { random_extent(this, entity, OFFSET_GUEST_RANDOM_Y, OFFSET_STAFF_RANDOM_Y, OFFSET_DEFAULT_RANDOM_Y) }
}

pub unsafe fn is_slow_move_function(call: *const u32) -> bool {
    unsafe { &*(call as *const RawFunctionCall) }.is_move_function(b"fMoveSlow", &[b"fWalk"], b"walk")
}

pub unsafe fn is_medium_move_function(call: *const u32) -> bool {
    unsafe { &*(call as *const RawFunctionCall) }.is_move_function(b"fMoveMedium", &[b"fTrot"], b"trot")
}

pub unsafe fn is_fast_move_function(call: *const u32) -> bool {
    unsafe { &*(call as *const RawFunctionCall) }.is_move_function(b"fMoveFast", &[b"fRun"], b"run")
}

/// The value vanilla returns for the eight named constants, or `None` when the name is not one of them
/// (vanilla then parses it as a number) or when a named constant is requested from an entity whose type is
/// not a `ZTUnitType` (vanilla then dereferences null); the caller defers to the real function for `None`,
/// so those cases behave exactly as vanilla.
pub unsafe fn convert_constant_param(name: *const c_void, entity: u32) -> Option<i32> {
    let name = unsafe { &*(name as *const RawCallString) }.bytes();
    let entity_type = get_from_memory::<u32>(entity + OFFSET_ENTITY_TYPE);
    let unit_type = if entity_type != 0 && unsafe { type_tag_check(entity_type, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTUNITTYPE) } {
        entity_type
    } else {
        0
    };

    if let Some(index) = RANDOM_TIME_NAMES.iter().position(|candidate| *candidate == name) {
        return (unit_type != 0).then(|| get_from_memory::<i32>(unit_type + UNIT_TYPE_RANDOM_TIME_OFFSETS[index]));
    }
    if name == INFORM_GUEST_TIME_NAME {
        let is_guide = unit_type != 0 && unsafe { type_tag_check(unit_type, TYPE_VTABLE_IS_CAST_CLASS, RVA_TAG_ZTGUIDE) };
        return Some(if is_guide { get_from_memory::<i32>(unit_type + GUIDE_TYPE_INFORM_GUEST_TIME_OFFSET) } else { 0 });
    }
    None
}

/// Removes `entity` from the world: runs the cheat hook, lets the map view drop any reference to it, then
/// dispatches the world manager's own `removeEntity(entity, true)`. Returns `false` if there is no world.
pub unsafe fn remove_entity(entity: u32) -> bool {
    let world = globals().ztworldmgr_ptr() as u32;
    unsafe { ZTCHEAT_ENTITY_REMOVED.original()(entity as i32) };
    if world == 0 {
        return false;
    }
    let mapview = unsafe { GET_MAPVIEW.original()() };
    if !mapview.is_null() {
        unsafe { PREPARE_TO_DELETE_ENTITY.original()(mapview, entity as *const u32) };
    }
    low_byte_bool(unsafe { call_virtual_args(world, WORLD_MGR_VTABLE_REMOVE_ENTITY, [entity, 1]) })
}

/// Calls a no-argument thiscall virtual at `slot` of the entity's own vtable.
pub(super) unsafe fn call_entity_virtual(entity: u32, slot: u32) {
    unsafe { call_virtual_args(entity, slot, []) };
}

pub(super) unsafe fn set_done(done: *const bool) {
    unsafe { (done as *mut bool).write_volatile(true) };
}

/// `unit` is non-null and its entity type passes the type's `slot` test against `tag_rva`.
pub(super) unsafe fn unit_is(unit: u32, slot: u32, tag_rva: u32) -> bool {
    if unit == 0 {
        return false;
    }
    let entity_type = get_from_memory::<u32>(unit + OFFSET_ENTITY_TYPE);
    unsafe { type_tag_check(entity_type, slot, tag_rva) }
}
