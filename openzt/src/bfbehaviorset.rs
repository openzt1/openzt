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
//! - `killChild` destroys the child through `0x43e9be`, the child's own destructor (it recurses into a
//!   grandchild itself), where vanilla's `killChild` recurses and then repeats the same teardown inline.
//!   The resulting state is identical.

use std::ffi::c_void;

use openzt_detour::{
    generated::{
        bfbehaviorset::{CLEAR, DESTRUCTOR as BEHAVIOR_SET_DESTRUCT, EXECUTE_CALL, EXECUTE_HEAD, KILL_CHILD, REMOVE_HEAD, SET_NAME},
        msvc_std_basic_string::{ASSIGN_RANGE, BASIC_STRING_0},
        msvc_std_vectorbasic_string::ERASE_RANGE_0X410AC3,
        standalone::OPERATOR_DELETE,
    },
};
use openzt_detour_macro::detour_mod;
use tracing::error;

use crate::{
    bfscriptmgr::{parse_function, reset_call, BfFunctionCall, RawString},
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
unsafe fn clear_string(string: *const RawString) {
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
            BEHAVIOR_SET_DESTRUCT.original()(child as *const u32);
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

#[detour_mod]
mod detours {
    use super::*;

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
        pub(crate) fn kill_child(this: u32) {
            unsafe { super::KILL_CHILD_DETOUR.call(this as *const u32) }
        }
        pub(crate) fn execute_head(this: u32, out: *const u8) {
            unsafe { super::EXECUTE_HEAD_DETOUR.call(this as *const u32, out) }
        }
        pub(crate) fn execute_call(this: u32, call: *const u32, out: *const u8) {
            unsafe { super::EXECUTE_CALL_DETOUR.call(this as *const u32, call, out) }
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
    use openzt_detour::generated::{msvc_std_basic_string::BASIC_STRING_2, standalone::OPERATOR_NEW};

    use super::*;
    use crate::bfscriptmgr::live_support as call_support;
    use openzt_detour::generated::msvc_std_vectorbasic_string::INSERT_0X406DE7;

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
