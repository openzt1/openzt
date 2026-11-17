//! `BFBehaviorSet` stage 2: the small methods `setName`, `removeHead`, `clear`, `killChild`, `executeHead` and
//! `executeCall`. The set is embedded in `BFEntity` at `+0x14`, child sets are separate vanilla-allocated
//! objects, and every string and vector inside is read and written by un-ported vanilla code (`execute`,
//! `executeChild`, `initChild`, `load`, `BFEntity::setBS`), so the struct mirrors vanilla's layout and is
//! operated on in place (style 1). All buffers stay vanilla-allocated: strings are assigned, destroyed and
//! inserted only through vanilla's own helpers.
//!
//! ## Layout (`0x60` bytes)
//! `+0x00` `vector<string>` stack of active child-set names; `+0x0c` the parsed `BFFunctionCall`
//! (see `bfscriptmgr.rs`); `+0x28` `vector<string>` call list; `+0x34` string, the call text being executed;
//! `+0x40` string, the set's name; `+0x4c` child set; `+0x50` entity; `+0x54` pointer derived from the entity;
//! `+0x58` flag set by `clear`; `+0x59` dirty (set by `setName` and `executeCall`); `+0x5a` "child removed /
//! reparse needed", also the `done` out-parameter `ZTAIMgr::executeCall` writes; `+0x5b` entity flag
//! pending; `+0x5c` loaded. Strings are `{begin, end, cap}`.
//!
//! ## Behaviour a reader should know
//! - `executeCall`'s early checks (null entity, empty name, null `+0x54`, not loaded) only store `*out = true`,
//!   and the store is overwritten by the dispatch result below them, so the port omits them.
//! - `executeCall` writes `*out = (AL == 0)` from the return register of `ZTAIMgr::executeCall`. That function
//!   returns its handler's own result (its tail only touches `CL`), and the port reads `AL` from the same
//!   vtable slot. The Rust `ZTAIMgr::executeCall` returns the same value (see `ztaimgr.rs`).
//! - `removeHead` and `executeHead` on an empty call list read through a null/one-past-end element in
//!   vanilla. The port returns without touching anything; `execute` never calls either with an empty list.
//! - `killChild` destroys the child with `destroy_set_members`, the port of `~BFBehaviorSet` (`0x43e9be`, which
//!   recurses into a grandchild through `killChild`), where vanilla's `killChild` recurses and then repeats the
//!   same teardown inline. The resulting state is identical. `~BFEntity` runs the same helper on its embedded
//!   set. `0x43ee6d` (destructor plus `operator delete`) stays vanilla and reaches the port through `0x43e9be`.

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bfbehaviorset::{
            CLEAR, CONSTRUCTOR, DESTRUCTOR, EXECUTE, EXECUTE_CALL, EXECUTE_CHILD, EXECUTE_HEAD, INIT_CHILD, KILL_CHILD,
            LOAD_0, LOAD_1, REMOVE_HEAD, SAVE, SET_NAME,
        },
        bffunctioncall::{CONSTRUCTOR as CALL_CONSTRUCTOR, DESTRUCTOR as CALL_DESTRUCTOR},
        msvc_std_basic_string::{ASSIGN_RANGE, BASIC_STRING_0, BASIC_STRING_RANGE},
        msvc_std_vectorbasic_string::{DESTRUCTOR_0X402466, ERASE_RANGE_0X410AC3},
        poolalloc::DEALLOCATE as POOL_DEALLOCATE,
        standalone::{OPERATOR_DELETE, OPERATOR_NEW},
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    bfscriptmgr::{
        construct_empty_string, load_call, parse_function, push_back_string, read_file_bytes, read_file_count, read_file_string, read_file_string_list,
        reset_call, save_call, write_file_bytes, write_file_string, write_file_string_list, BfFunctionCall, RawString,
        FILE_VERSION_LAST_WITHOUT_BEHAVIOR_SETS,
    },
    util::{get_from_memory, ref_from_memory, LiveMemory},
    vanilla_string::VanillaString,
    write_live,
};

/// `GLOBAL_ZTAIMgr`, a pointer to the real manager.
const GLOBAL_ZTAIMGR: u32 = 0x0063_8098;
/// `ZTAIMgr` vtable slot `executeCall(call, entity, done)`.
const AIMGR_VTABLE_EXECUTE_CALL: u32 = 0xc;
/// `BFEntity` vtable slot called after a freshly parsed call was accepted.
const ENTITY_VTABLE_AFTER_PARSE: u32 = 0x4c;
/// Byte in the entity that `execute` sets when it consumes the pending flag.
const ENTITY_PENDING_FLAG: u32 = 0xd6;
/// Vtable slot `BFEntityType::loadBSFunctions(this, vector<const char*>* out, string* name) -> bool` on the
/// entity's type object (`entity+0x128`). Overridden by `ZTGuestType`, `ZTAnimalType`, `BFUnitType` and
/// `ZTUnitType`, so it is called through the vtable.
const MANAGER_VTABLE_GET_SCRIPT: u32 = 0x38;

pub(crate) const BFBEHAVIORSET_SIZE: usize = 0x60;
const STRING_SIZE: u32 = 12;

#[repr(C)]
pub(crate) struct BfBehaviorSet {
    pub(crate) stack_begin: *const u8,
    pub(crate) stack_end: *const u8,
    pub(crate) stack_capacity: *const u8,
    pub(crate) call: BfFunctionCall,
    pub(crate) list_begin: *const u8,
    pub(crate) list_end: *const u8,
    pub(crate) list_capacity: *const u8,
    pub(crate) current_call: RawString,
    pub(crate) name: RawString,
    pub(crate) child: *const BfBehaviorSet,
    pub(crate) entity: *const u32,
    pub(crate) field_54: *const u32,
    pub(crate) flag_58: u8,
    pub(crate) dirty: u8,
    pub(crate) removed: u8,
    pub(crate) pending: u8,
    pub(crate) loaded: u8,
    pub _live: LiveMemory,
}

const _: () = assert!(std::mem::size_of::<BfBehaviorSet>() == BFBEHAVIORSET_SIZE);

/// Empties a string the way vanilla's inlined clear does: terminator at `begin`, `end = begin`.
pub(crate) unsafe fn clear_string(string: *const RawString) {
    unsafe {
        let begin = (*string).begin;
        if begin != (*string).end {
            (begin as *mut u8).write_volatile(0);
            std::ptr::addr_of!((*string).end).cast_mut().write_volatile(begin);
        }
    }
}

impl BfBehaviorSet {
    fn list_len(&self) -> u32 {
        (self.list_end as u32).wrapping_sub(self.list_begin as u32) / STRING_SIZE
    }

    /// Destroys the newest entry of the child-name stack, if any.
    unsafe fn pop_stack(&self) {
        let count = (self.stack_end as u32).wrapping_sub(self.stack_begin as u32) / STRING_SIZE;
        if count != 0 {
            let new_end = unsafe { self.stack_end.sub(STRING_SIZE as usize) };
            write_live!(self, stack_end, new_end);
            unsafe { BASIC_STRING_0.original()(new_end as *const c_void) };
        }
    }

    /// `BFBehaviorSet::setName`: false for an empty name, else assigns `+0x40` and marks the set dirty.
    pub(crate) unsafe fn set_name(&self, name: *const RawString) -> bool {
        let source = unsafe { &*name };
        if source.begin == source.end {
            return false;
        }
        let own = std::ptr::addr_of!(self.name);
        if name != own {
            unsafe { ASSIGN_RANGE.original()(own as *const c_void, source.begin as *const u32, source.end, std::ptr::null()) };
        }
        write_live!(self, dirty, 1u8);
        true
    }

    /// `BFBehaviorSet::removeHead`: erases the first call-list entry and empties the parsed call's name.
    pub(crate) unsafe fn remove_head(&self) {
        if self.list_len() == 0 {
            return;
        }
        let count = self.list_len();
        for i in 1..count {
            let source = unsafe { &*(self.list_begin as *const RawString).add(i as usize) };
            let target = unsafe { (self.list_begin as *const RawString).add(i as usize - 1) };
            unsafe { ASSIGN_RANGE.original()(target as *const c_void, source.begin as *const u32, source.end, std::ptr::null()) };
        }
        let new_end = unsafe { self.list_end.sub(STRING_SIZE as usize) };
        write_live!(self, list_end, new_end);
        unsafe { BASIC_STRING_0.original()(new_end as *const c_void) };
        unsafe { clear_string(std::ptr::addr_of!(self.call) as *const RawString) };
    }

    /// `BFBehaviorSet::killChild`: destroys the child set, pops its name and flags a reparse. A set without a
    /// child is left untouched.
    pub(crate) unsafe fn kill_child(&self) {
        let child = self.child;
        if child.is_null() {
            return;
        }
        unsafe {
            destroy_set_members(child as u32);
            OPERATOR_DELETE.original()(child as u32);
        }
        write_live!(self, child, std::ptr::null());
        unsafe { self.pop_stack() };
        write_live!(self, removed, 1u8);
    }

    /// `BFBehaviorSet::clear`.
    pub(crate) unsafe fn clear(&self) {
        unsafe { clear_string(std::ptr::addr_of!(self.name)) };
        write_live!(self, flag_58, 1u8);
        unsafe {
            clear_string(std::ptr::addr_of!(self.current_call));
            reset_call(std::ptr::addr_of!(self.call) as *const u32);
            ERASE_RANGE_0X410AC3.original()(std::ptr::addr_of!(self.list_begin) as *const u32, self.list_begin as *const u32, self.list_end as i32);
            self.kill_child();
        }
        write_live!(self, loaded, 0u8);
        write_live!(self, removed, 0u8);
        write_live!(self, dirty, 0u8);
        write_live!(self, pending, 0u8);
    }

    /// `BFBehaviorSet::executeHead`: runs the first call-list entry through a private copy, since executing it
    /// can change the list.
    pub(crate) unsafe fn execute_head(&self, out: *mut u8) {
        if self.list_len() == 0 {
            return;
        }
        let head = unsafe { &*(self.list_begin as *const RawString) };
        let copy = VanillaString::from_bytes(head.bytes());
        unsafe { self.execute_call(copy.as_ptr(), out) };
    }

    /// `BFBehaviorSet::executeCall`: reparses when needed, dispatches through `GLOBAL_ZTAIMgr`'s vtable, and
    /// reports whether the call is still running (`*out = true` when the dispatch left `AL == 0`).
    pub(crate) unsafe fn execute_call(&self, call_text: *const u32, out: *mut u8) {
        let call_text = call_text as *const RawString;
        let call = std::ptr::addr_of!(self.call);
        let mut parsed = false;
        if self.removed != 0 || self.call.name_begin == self.call.name_end {
            let own = std::ptr::addr_of!(self.current_call);
            if call_text != own {
                let source = unsafe { &*call_text };
                unsafe { ASSIGN_RANGE.original()(own as *const c_void, source.begin as *const u32, source.end, std::ptr::null()) };
            }
            unsafe { parse_function(call_text as *const u32, call as *const u32) };
            write_live!(self, removed, 0u8);
            parsed = true;
        }
        write_live!(self, dirty, 1u8);

        let manager = get_from_memory::<u32>(GLOBAL_ZTAIMGR);
        if manager == 0 {
            unsafe { out.write_volatile(1) };
            return;
        }
        let vtable = get_from_memory::<u32>(manager);
        let slot = get_from_memory::<u32>(vtable + AIMGR_VTABLE_EXECUTE_CALL);
        let dispatch = unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32, u32, u32, u32) -> u8>(slot) };
        let result = unsafe { dispatch(manager, call as u32, self.entity as u32, std::ptr::addr_of!(self.removed) as u32) };
        unsafe { out.write_volatile((result == 0) as u8) };
        if result != 0 && parsed {
            let vtable = get_from_memory::<u32>(self.entity as u32);
            let slot = get_from_memory::<u32>(vtable + ENTITY_VTABLE_AFTER_PARSE);
            unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32)>(slot)(self.entity as u32) };
        }
    }
}

impl BfBehaviorSet {
    pub(crate) unsafe fn erase_list(&self) {
        unsafe { ERASE_RANGE_0X410AC3.original()(std::ptr::addr_of!(self.list_begin) as *const u32, self.list_begin as *const u32, self.list_end as i32) };
    }

    /// The shared tail `execute` runs when a call finished or the list ran dry: reset the set.
    unsafe fn finish(&self) {
        write_live!(self, dirty, 0u8);
        unsafe {
            self.kill_child();
            self.erase_list();
            self.clear();
        }
    }

    /// `BFBehaviorSet::executeChild`: runs the child set and flags a reparse once the child has gone idle.
    pub(crate) unsafe fn execute_child(&self) -> bool {
        let mut result = 0u8;
        if !self.child.is_null() {
            unsafe { (*self.child).execute(&mut result) };
        }
        if !self.child.is_null() && unsafe { (*self.child).dirty } == 0 {
            write_live!(self, removed, 1u8);
        }
        result != 0
    }

    /// `BFBehaviorSet::execute`: advances the script by one or more calls. `*out` is true when the set has
    /// nothing more to run.
    pub(crate) unsafe fn execute(&self, out: *mut u8) {
        let name_empty = self.name.begin == self.name.end;
        if self.loaded == 0 || self.entity.is_null() || name_empty || self.field_54.is_null() {
            unsafe { out.write_volatile(1) };
            return;
        }
        let mut result = 0u8;
        loop {
            let take_pending = |set: &BfBehaviorSet| {
                unsafe { ((set.entity as u32 + ENTITY_PENDING_FLAG) as *mut u8).write_volatile(1) };
                write_live!(set, pending, 0u8);
            };
            if self.dirty != 0 && self.removed != 0 {
                if self.pending != 0 {
                    take_pending(self);
                } else {
                    unsafe { self.remove_head() };
                }
            } else if self.pending != 0 {
                take_pending(self);
            }

            if self.list_len() == 0 {
                unsafe { self.finish() };
            } else {
                if self.child.is_null() {
                    unsafe { self.execute_head(&mut result) };
                } else {
                    result = unsafe { self.execute_child() } as u8;
                    let child = self.child;
                    if !child.is_null() && unsafe { (*child).dirty } == 0 {
                        unsafe { self.kill_child() };
                    }
                }
                if result != 0 {
                    write_live!(self, removed, 1u8);
                    unsafe { self.finish() };
                }
            }
            if self.removed == 0 || self.list_len() == 0 {
                break;
            }
        }
        unsafe { out.write_volatile(result) };
    }

    /// Binds the set to `entity`, as vanilla's inlined `setEntity` does.
    unsafe fn bind_entity(&self, entity: u32) {
        if entity == 0 {
            return;
        }
        write_live!(self, entity, entity as *const u32);
        let behavior = get_from_memory::<u32>(entity + 0x128);
        let field = if get_from_memory::<u32>(behavior + 0x84) == 0 { std::ptr::null() } else { (behavior + 0x80) as *const u32 };
        write_live!(self, field_54, field);
    }

    /// `BFBehaviorSet::load()`: fetches the call list for the current name from the entity's behavior
    /// manager. False when the set has no entity, name or manager.
    pub(crate) unsafe fn load(&self) -> bool {
        if self.entity.is_null() || self.name.begin == self.name.end || self.field_54.is_null() {
            return false;
        }
        let manager = get_from_memory::<u32>(self.entity as u32 + 0x128);
        if manager == 0 {
            return false;
        }
        unsafe { self.erase_list() };

        let mut lines: [u32; 3] = [0; 3];
        let vtable = get_from_memory::<u32>(manager);
        let slot = get_from_memory::<u32>(vtable + MANAGER_VTABLE_GET_SCRIPT);
        let fetch = unsafe { std::mem::transmute::<u32, unsafe extern "thiscall" fn(u32, *mut [u32; 3], *const RawString) -> u8>(slot) };
        let mut result = unsafe { fetch(manager, &mut lines, std::ptr::addr_of!(self.name)) } != 0;

        let mut cursor = lines[0];
        while cursor != lines[1] {
            let text = get_from_memory::<u32>(cursor);
            let mut length = 0u32;
            while get_from_memory::<u8>(text + length) != 0 {
                length += 1;
            }
            let mut line: [u32; 3] = [0; 3];
            unsafe {
                BASIC_STRING_RANGE.original()(line.as_mut_ptr() as *const c_void, text as *const u32, (text + length) as i32);
                push_back_string(std::ptr::addr_of!(self.list_begin) as *const u32, line.as_ptr());
                BASIC_STRING_0.original()(line.as_ptr() as *const c_void);
            }
            cursor += 4;
        }

        if self.list_len() == 0 {
            write_live!(self, dirty, 0u8);
        } else {
            result = true;
            write_live!(self, loaded, 1u8);
        }
        if lines[0] != 0 {
            unsafe { POOL_DEALLOCATE.original()(lines[0] as *const u32, (lines[2] - lines[0]) & !3) };
        }
        result
    }

    /// `BFBehaviorSet::initChild`: allocates a child set under the deepest existing child, names it, loads
    /// its call list and records the name on this set's stack. False only for a null set or a failed
    /// allocation.
    pub(crate) unsafe fn init_child(&self, entity: u32, name: *const RawString) -> bool {
        let mut parent = self as *const BfBehaviorSet;
        while unsafe { !(*parent).child.is_null() } {
            parent = unsafe { (*parent).child };
        }
        let raw = unsafe { OPERATOR_NEW.original()(BFBEHAVIORSET_SIZE as u32) } as *mut u32;
        if raw.is_null() {
            return false;
        }
        unsafe { construct_set(raw, entity) };
        let child = unsafe { &*(raw as *const BfBehaviorSet) };
        write_live!(unsafe { &*parent }, child, raw as *const BfBehaviorSet);
        unsafe { child.set_name(name) };
        write_live!(child, removed, 0u8);
        unsafe { child.load() };
        unsafe { push_back_string(self as *const BfBehaviorSet as *const u32, name as *const u32) };
        true
    }

    /// `BFBehaviorSet::save(file)` (`0x4777a7`): writes the set in the order `load_file` reads it. Every write
    /// is attempted; the result is true only when all succeeded (a string of `0x1000` bytes or more is skipped
    /// and fails).
    pub(crate) unsafe fn save(&self, file: *const i8) -> bool {
        let mut ok = unsafe { write_file_string(file, std::ptr::addr_of!(self.name)) };
        ok &= unsafe { write_file_bytes(std::ptr::addr_of!(self.flag_58), 1, file) };
        unsafe {
            ok &= write_file_string_list(file, std::ptr::addr_of!(self.stack_begin) as *const u32);
            ok &= write_file_string(file, std::ptr::addr_of!(self.current_call));
            ok &= save_call(std::ptr::addr_of!(self.call) as *const u32, file);
        }
        let has_child = !self.child.is_null() as u8;
        ok &= unsafe { write_file_bytes(&has_child, 1, file) };
        if has_child != 0 {
            ok &= unsafe { (*self.child).save(file) };
        }
        unsafe {
            ok &= write_file_string_list(file, std::ptr::addr_of!(self.list_begin) as *const u32);
            for flag in [std::ptr::addr_of!(self.loaded), std::ptr::addr_of!(self.removed), std::ptr::addr_of!(self.dirty), std::ptr::addr_of!(self.pending)] {
                ok &= write_file_bytes(flag, 1, file);
            }
        }
        ok
    }

    /// `BFBehaviorSet::load(file, version, entity)` (`0x44fb77`): restores a saved set. Saves older than
    /// version `0x3f` hold nothing and load as success. Every read is attempted even after an earlier one
    /// failed, except where vanilla stops: after the header, after the child flag and after the call-list
    /// count. The result is true only when every read succeeded.
    ///
    /// Field order: name, flag `+0x58`, name stack, current call, parsed call, child flag and child, call
    /// list, then the flags `+0x5c`, `+0x5a`, `+0x59`, `+0x5b`.
    pub(crate) unsafe fn load_file(&self, file: *const u8, version: u32, entity: i32) -> bool {
        if version <= FILE_VERSION_LAST_WITHOUT_BEHAVIOR_SETS {
            return true;
        }
        let mut ok = unsafe { read_file_string(file, std::ptr::addr_of!(self.name)) };
        ok &= unsafe { read_file_bytes(std::ptr::addr_of!(self.flag_58) as *mut u8, 1, file) };
        let stack_count = unsafe { read_file_count(file) };
        ok &= stack_count.is_some();
        if !ok {
            return false;
        }
        let stack = std::ptr::addr_of!(self.stack_begin) as *const u32;
        unsafe {
            ERASE_RANGE_0X410AC3.original()(stack, self.stack_begin as *const u32, self.stack_end as i32);
            ok &= read_file_string_list(file, stack, stack_count.unwrap_or(0));
            ok &= read_file_string(file, std::ptr::addr_of!(self.current_call));
            load_call(std::ptr::addr_of!(self.call) as *const u32, file, version);
        }

        let mut has_child = 0u8;
        ok &= unsafe { read_file_bytes(&mut has_child, 1, file) };
        if !ok {
            return false;
        }
        if has_child != 0 {
            let raw = unsafe { OPERATOR_NEW.original()(BFBEHAVIORSET_SIZE as u32) } as *mut u32;
            if !raw.is_null() {
                unsafe { construct_set(raw, entity as u32) };
            }
            write_live!(self, child, raw as *const BfBehaviorSet);
            ok &= unsafe { (*self.child).load_file(file, version, entity) };
        }

        let list_count = unsafe { read_file_count(file) };
        ok &= list_count.is_some();
        if !ok {
            return false;
        }
        unsafe {
            self.erase_list();
            ok &= read_file_string_list(file, std::ptr::addr_of!(self.list_begin) as *const u32, list_count.unwrap_or(0));
            for flag in [std::ptr::addr_of!(self.loaded), std::ptr::addr_of!(self.removed), std::ptr::addr_of!(self.dirty), std::ptr::addr_of!(self.pending)] {
                ok &= read_file_bytes(flag as *mut u8, 1, file);
            }
        }
        ok
    }
}

/// Builds a set in place at `raw` (`0x60` bytes of uninitialised memory): empty strings and vectors, no child,
/// reset flags, bound to `entity` when it is non-null. Shared by the constructor and `initChild`.
unsafe fn construct_set(raw: *mut u32, entity: u32) {
    unsafe {
        std::ptr::write_bytes(raw, 0, BFBEHAVIORSET_SIZE / 4);
        construct_set_members(raw as u32);
        (&*(raw as *const BfBehaviorSet)).bind_entity(entity);
    }
}

/// Writes exactly what the set's constructor writes, as `BFEntity`'s constructor inlines it at `+0x14`: both
/// vectors zeroed, the call object and strings built, no child, no entity, then `clear`. `+0x54` is never
/// written, so a set built without an entity keeps whatever was there.
pub(crate) unsafe fn construct_set_members(base: u32) {
    unsafe {
        for offset in [0x0u32, 0x4, 0x8, 0x28, 0x2c, 0x30, 0x4c, 0x50] {
            ((base + offset) as *mut u32).write_volatile(0);
        }
        CALL_CONSTRUCTOR.original()((base + 0xc) as *const u32);
        construct_empty_string((base + 0x34) as *mut RawString);
        construct_empty_string((base + 0x40) as *mut RawString);
        (&*(base as *const BfBehaviorSet)).clear();
    }
}

/// Tears down the set at `base` the way `BFEntity`'s destructor inlines `~BFBehaviorSet`: the child (with its
/// name-stack entry and the removed flag), then the two strings, the call list, the call object and the name
/// stack. Every buffer is freed through vanilla's own destructors.
pub(crate) unsafe fn destroy_set_members(base: u32) {
    unsafe {
        (&*(base as *const BfBehaviorSet)).kill_child();
        BASIC_STRING_0.original()((base + 0x40) as *const c_void);
        BASIC_STRING_0.original()((base + 0x34) as *const c_void);
        DESTRUCTOR_0X402466.original()((base + 0x28) as *const u32);
        CALL_DESTRUCTOR.original()((base + 0xc) as *const u32);
        DESTRUCTOR_0X402466.original()(base as *const u32);
    }
}

/// Entry point for other ported code: `initChild` on the set at `set`.
pub(crate) unsafe fn init_child(set: *const u32, entity: u32, name: u32) -> bool {
    if set.is_null() {
        return false;
    }
    unsafe { ref_from_memory::<BfBehaviorSet>(set as u32).init_child(entity, name as *const RawString) }
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(CONSTRUCTOR)]
    unsafe extern "thiscall" fn constructor_detour(this: *const u32, entity: i32) -> *const u32 {
        unsafe { construct_set(this as *mut u32, entity as u32) };
        this
    }

    #[detour(SAVE)]
    unsafe extern "thiscall" fn save_detour(this: *const u32, file: *const i8) -> bool {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).save(file) }
    }

    #[detour(LOAD_1)]
    unsafe extern "thiscall" fn load_file_detour(this: *const u32, file: *const u8, version: u32, entity: i32) -> bool {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).load_file(file, version, entity) }
    }

    #[detour(EXECUTE)]
    unsafe extern "thiscall" fn execute_detour(this: *const u32, out: *const bool) {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).execute(out as *mut u8) }
    }

    #[detour(EXECUTE_CHILD)]
    unsafe extern "thiscall" fn execute_child_detour(this: *const u32) -> bool {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).execute_child() }
    }

    #[detour(LOAD_0)]
    unsafe extern "thiscall" fn load_detour(this: *const u32) -> bool {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).load() }
    }

    #[detour(INIT_CHILD)]
    unsafe extern "thiscall" fn init_child_detour(this: *const u32, entity: i32, name: *const i32) -> bool {
        unsafe { init_child(this, entity as u32, name as u32) }
    }

    #[detour(SET_NAME)]
    unsafe extern "thiscall" fn set_name_detour(this: *const u32, name: *const i32) -> bool {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).set_name(name as *const RawString) }
    }

    #[detour(REMOVE_HEAD)]
    unsafe extern "thiscall" fn remove_head_detour(this: *const u32) {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).remove_head() }
    }

    #[detour(CLEAR)]
    unsafe extern "thiscall" fn clear_detour(this: *const u32) {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).clear() }
    }

    #[detour(DESTRUCTOR)]
    unsafe extern "thiscall" fn destructor_detour(this: *const u32) {
        unsafe { destroy_set_members(this as u32) }
    }

    #[detour(KILL_CHILD)]
    unsafe extern "thiscall" fn kill_child_detour(this: *const u32) {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).kill_child() }
    }

    #[detour(EXECUTE_HEAD)]
    unsafe extern "thiscall" fn execute_head_detour(this: *const u32, out: *const u8) {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).execute_head(out as *mut u8) }
    }

    #[detour(EXECUTE_CALL)]
    unsafe extern "thiscall" fn execute_call_detour(this: *const u32, call: *const u32, out: *const u8) {
        unsafe { ref_from_memory::<BfBehaviorSet>(this as u32).execute_call(call, out as *mut u8) }
    }

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn set_name(this: u32, name: *const i32) -> bool {
            unsafe { super::SET_NAME_DETOUR.call(this as *const u32, name) }
        }
        pub(crate) fn remove_head(this: u32) {
            unsafe { super::REMOVE_HEAD_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn clear(this: u32) {
            unsafe { super::CLEAR_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn destructor(this: u32) {
            unsafe { super::DESTRUCTOR_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn kill_child(this: u32) {
            unsafe { super::KILL_CHILD_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn execute_head(this: u32, out: *const u8) {
            unsafe { super::EXECUTE_HEAD_DETOUR.call(this as *const u32, out) }
        }
        pub(crate) fn execute_call(this: u32, call: *const u32, out: *const u8) {
            unsafe { super::EXECUTE_CALL_DETOUR.call(this as *const u32, call, out) }
        }
        pub(crate) fn execute(this: u32, out: *const u8) {
            unsafe { super::EXECUTE_DETOUR.call(this as *const u32, out as *const bool) }
        }
        pub(crate) fn execute_child(this: u32) -> bool {
            unsafe { super::EXECUTE_CHILD_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn load(this: u32) -> bool {
            unsafe { super::LOAD_0_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn constructor(this: u32, entity: u32) -> u32 {
            unsafe { super::CONSTRUCTOR_DETOUR.call(this as *const u32, entity as i32) as u32 }
        }
        pub(crate) fn save(this: u32, file: *const i8) -> bool {
            unsafe { super::SAVE_DETOUR.call(this as *const u32, file) }
        }
        pub(crate) fn load_file(this: u32, file: *const u8, version: u32, entity: u32) -> bool {
            unsafe { super::LOAD_1_DETOUR.call(this as *const u32, file, version, entity as i32) }
        }
        pub(crate) fn init_child(this: u32, entity: u32, name: *const i32) -> bool {
            unsafe { super::INIT_CHILD_DETOUR.call(this as *const u32, entity as i32, name) }
        }
    }
}

/// Installs this module's detours. Called from both `lib.rs`'s boot cascade and
/// `reimplementation_tests::init()`.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise bfbehaviorset detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use openzt_detour::generated::{msvc_std_basic_string::BASIC_STRING_2, msvc_std_vectorbasic_string::INSERT_0X406DE7};

    use super::*;
    use crate::bfscriptmgr::live_support as call_support;

    /// Everything the stage-2 methods read or write.
    #[derive(Debug, PartialEq, Eq)]
    pub(crate) struct SetSnapshot {
        pub name: Vec<u8>,
        pub current_call: Vec<u8>,
        pub call: call_support::CallSnapshot,
        pub list: Vec<Vec<u8>>,
        pub stack: Vec<Vec<u8>>,
        pub has_child: bool,
        pub flags: [u8; 5],
        pub entity: u32,
        pub field_54: u32,
    }

    /// Points `set` and every nested child at `entity`, deriving `+0x54` from its behavior manager.
    pub(crate) fn bind(set: u32, entity: u32) {
        for node in chain(set) {
            unsafe {
                ((node + 0x50) as *mut u32).write(entity);
                let manager = get_from_memory::<u32>(entity + 0x128);
                ((node + 0x54) as *mut u32).write(manager + 0x80);
            }
        }
    }

    /// `set` followed by its nested children.
    pub(crate) fn chain(set: u32) -> Vec<u32> {
        let mut nodes = Vec::new();
        let mut current = set;
        while current != 0 {
            nodes.push(current);
            current = get_from_memory::<u32>(current + 0x4c);
        }
        nodes
    }

    pub(crate) fn snapshot_chain(set: u32) -> Vec<SetSnapshot> {
        chain(set).into_iter().map(snapshot).collect()
    }

    fn construct_string(at: *mut RawString, text: &[u8]) {
        let range = text.as_ptr_range();
        unsafe { BASIC_STRING_2.original()(at as *const c_void, range.start as *const u32, range.end as i32, 0) };
    }

    fn push_string(vector: *const u32, text: &[u8]) {
        let value = VanillaString::from_bytes(text);
        let end = unsafe { std::ptr::read_volatile((vector as u32 + 4) as *const u32) };
        unsafe { INSERT_0X406DE7.original()(vector, end as *const u32, value.as_ptr(), 1) };
    }

    /// A set built from vanilla-allocated strings, itself allocated through vanilla's `operator_new` so
    /// `killChild` can delete it when it is a child. `children` is the chain of nested child names. A root set
    /// is leaked: its strings and vectors belong to vanilla's allocator and are never freed from Rust.
    pub(crate) fn build_set(name: &[u8], current_call: &[u8], list: &[&[u8]], children: &[&[u8]], entity: u32) -> u32 {
        let set = unsafe { OPERATOR_NEW.original()(BFBEHAVIORSET_SIZE as u32) } as *mut u32;
        unsafe { std::ptr::write_bytes(set, 0, BFBEHAVIORSET_SIZE / 4) };
        let set = set as u32;
        init_set(set, name, current_call, list, entity);
        if let Some((first, rest)) = children.split_first() {
            let child = build_set(first, b"", &[], rest, entity);
            push_string(set as *const u32, first);
            unsafe { ((set + 0x4c) as *mut u32).write(child) };
        }
        set
    }

    fn init_set(set: u32, name: &[u8], current_call: &[u8], list: &[&[u8]], entity: u32) {
        unsafe {
            call_support::construct_call(set + 0xc);
            construct_string((set + 0x34) as *mut RawString, current_call);
            construct_string((set + 0x40) as *mut RawString, name);
            ((set + 0x50) as *mut u32).write(entity);
        }
        for entry in list {
            push_string((set + 0x28) as *const u32, entry);
        }
    }

    pub(crate) fn push_list(set: u32, text: &[u8]) {
        push_string((set + 0x28) as *const u32, text);
    }

    pub(crate) fn push_stack(set: u32, text: &[u8]) {
        push_string(set as *const u32, text);
    }

    pub(crate) fn set_flags(set: u32, flags: [u8; 5]) {
        for (i, flag) in flags.iter().enumerate() {
            unsafe { ((set + 0x58 + i as u32) as *mut u8).write(*flag) };
        }
    }

    pub(crate) fn name_ptr(set: u32) -> *const i32 {
        (set + 0x40) as *const i32
    }

    pub(crate) fn snapshot(set: u32) -> SetSnapshot {
        let set_ref = unsafe { &*(set as *const BfBehaviorSet) };
        let strings = |begin: *const u8, end: *const u8| {
            let count = (end as usize - begin as usize) / STRING_SIZE as usize;
            (0..count).map(|i| unsafe { &*(begin as *const RawString).add(i) }.bytes().to_vec()).collect::<Vec<_>>()
        };
        SetSnapshot {
            name: set_ref.name.bytes().to_vec(),
            current_call: set_ref.current_call.bytes().to_vec(),
            call: call_support::snapshot(set + 0xc),
            list: strings(set_ref.list_begin, set_ref.list_end),
            stack: strings(set_ref.stack_begin, set_ref.stack_end),
            has_child: !set_ref.child.is_null(),
            flags: [set_ref.flag_58, set_ref.dirty, set_ref.removed, set_ref.pending, set_ref.loaded],
            entity: set_ref.entity as u32,
            field_54: set_ref.field_54 as u32,
        }
    }

    /// Parses `text` into the set's call object, so the call name is non-empty.
    pub(crate) fn prime_call(set: u32, text: &str) {
        let source = VanillaString::new(text);
        call_support::real_parse_function(source.as_ptr(), set + 0xc);
    }

    pub(crate) use super::detours::test_real as real;

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        detours::status()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn layout_offsets_match_vanilla() {
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, call), 0x0c);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, list_begin), 0x28);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, current_call), 0x34);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, name), 0x40);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, child), 0x4c);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, entity), 0x50);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, flag_58), 0x58);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, removed), 0x5a);
        assert_eq!(std::mem::offset_of!(BfBehaviorSet, loaded), 0x5c);
    }
}
