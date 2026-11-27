//! `ZTUnit` zoo-message helpers (group I of `plans/ztunit-scoping-plan.md`): the unit-name message formatter, the
//! `fZooMessage*` family that shows a formatted message through `BFUIMgr::displayMessage`, and `getSellPrice`.
//!
//! A message string id is looked up with `BFApp::loadString`, formatted by `BFLog::FormatLogMessage` with the unit's name
//! string (`+0x108`) as its one argument, and the result is a vanilla `std::string` owned by the caller. A type whose
//! second name string (`type + 0xa4`) starts with `f` uses the feminine variant of the message, `30000` ids higher.

use openzt_detour::generated::{
    bfapp::LOAD_STRING,
    bfuimgr::DISPLAY_MESSAGE_1,
    ztunit::{FORMAT_NAME_MESSAGE, F_ZOO_MESSAGE, F_ZOO_MESSAGE_ENTITY, F_ZOO_MESSAGE_NAME, F_ZOO_MESSAGE_TILE_0, GET_SELL_PRICE},
};
use openzt_detour_macro::detour_mod;
use std::mem::transmute;
use tracing::error;

use super::{base, unit_offsets, ZTUnit, APP_RVA};
use crate::{util::get_from_memory, vanilla_string::VanillaString, ztkeeper::vtable_slot};

/// `GLOBAL_BFUIMgr` (the object itself), as an RVA.
const BFUIMGR_RVA: u32 = 0x0023_8de0;
/// `BFLog::FormatLogMessage` (`0x0040667d`), cdecl `(destination, format, <name string by value>)`.
const FORMAT_LOG_MESSAGE_ADDRESS: u32 = 0x0040_667d;
/// `DAT_00634eb8`, the factor `getSellPrice` scales the type's purchase cost by, as an RVA.
const SELL_FACTOR_RVA: u32 = 0x0023_4eb8;

/// Unit and type fields.
const NAME_BEGIN: u32 = 0x108;
/// `BFEntityType`'s second name string (`{begin, end}`); its first byte `f` selects the feminine message.
const TYPE_GENDER_STRING: u32 = 0xa4;
const FEMININE_MESSAGE_OFFSET: u32 = 30000;
/// Capacity of the stack buffers `loadString` and `FormatLogMessage` fill.
const TEXT_BUFFER_BYTES: usize = 512;

/// `BFEntity` slot `isRemoved`; `ZTUnitType` slot `getPurchaseCost`.
const SLOT_IS_REMOVED: u32 = 0xac;
const TYPE_SLOT_PURCHASE_COST: u32 = 0xbc;

fn read_string(begin_address: u32) -> Vec<u8> {
    let begin: u32 = get_from_memory(begin_address);
    let end: u32 = get_from_memory(begin_address + 4);
    if begin == 0 || end <= begin {
        return Vec::new();
    }
    unsafe { std::slice::from_raw_parts(begin as *const u8, (end - begin) as usize) }.to_vec()
}

/// The text of message `id` formatted with `name` (the by-value string argument `FormatLogMessage` reads).
fn format_message(id: u32, name: &VanillaString) -> VanillaString {
    let mut format = [0u8; TEXT_BUFFER_BYTES + 8];
    unsafe { LOAD_STRING.hooked()((base() + APP_RVA) as *const u32, (id & 0xffff) as *const u32, format.as_mut_ptr()) };
    let mut text = [0u8; TEXT_BUFFER_BYTES + 8];
    let [begin, end, capacity] = name.words();
    unsafe {
        let format_log_message = transmute::<u32, unsafe extern "cdecl" fn(*mut u8, *const u8, u32, u32, u32) -> u32>(FORMAT_LOG_MESSAGE_ADDRESS);
        format_log_message(text.as_mut_ptr(), format.as_ptr(), begin, end, capacity);
    }
    let length = text.iter().position(|&byte| byte == 0).unwrap_or(text.len());
    VanillaString::from_bytes(&text[..length])
}

fn display_message(message: *const u32, kind: i32, tile: u32, entity: u32) {
    unsafe { DISPLAY_MESSAGE_1.hooked()((base() + BFUIMGR_RVA) as *const u32, message as *const i32, kind, tile as *const u32, entity as *const u32, true, false) };
}

impl ZTUnit {
    fn is_removed_slot(self) -> bool {
        let function = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> u32>(vtable_slot(self.0, SLOT_IS_REMOVED)) };
        function(self.0) as u8 != 0
    }

    /// `ZTUnit::formatNameMessage` (`0x0049cfeb`): message `id` (plus `30000` for a type whose second name string starts with
    /// `f`) with the unit's name substituted, as a new vanilla string.
    pub fn format_name_message(self, id: u32) -> VanillaString {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        let feminine = entity_type != 0 && read_string(entity_type + TYPE_GENDER_STRING).first() == Some(&b'f');
        let id = if feminine { id.wrapping_add(FEMININE_MESSAGE_OFFSET) } else { id };
        let name = VanillaString::from_bytes(&read_string(self.0 + NAME_BEGIN));
        format_message(id, &name)
    }

    /// `ZTUnit::fZooMessage` (`0x0049d16d`): shows an already-formatted message string with this unit as its subject.
    pub fn f_zoo_message(self, message: *const u32, kind: i32, tile: u32) {
        display_message(message, kind, tile, self.0);
    }

    /// `ZTUnit::fZooMessageName` (`0x0049d18e`): the unit's name message `id`, unless the unit is removed and `force` is clear.
    pub fn f_zoo_message_name(self, id: u32, kind: i32, force: u8) {
        if force == 0 && self.is_removed_slot() {
            return;
        }
        let text = self.format_name_message(id);
        self.f_zoo_message(text.as_ptr(), kind, 0);
    }

    /// `ZTUnit::fZooMessageTile` (`0x004807c2`): as [`Self::f_zoo_message_name`], but anchored on `tile` and `entity`.
    pub fn f_zoo_message_tile(self, id: u32, kind: i32, tile: u32, entity: u32, force: u8) {
        if force == 0 && self.is_removed_slot() {
            return;
        }
        let text = self.format_name_message(id);
        display_message(text.as_ptr(), kind, tile, entity);
    }

    /// `ZTUnit::fZooMessageEntity` (`0x00612f22`): message `id` formatted with `entity`'s name (not this unit's), shown for
    /// `entity`; nothing for a null entity, or (unless `force`) a removed unit. No feminine variant.
    pub fn f_zoo_message_entity(self, id: u32, kind: i32, entity: u32, force: u8) {
        if entity == 0 || (force == 0 && self.is_removed_slot()) {
            return;
        }
        let name = VanillaString::from_bytes(&read_string(entity + NAME_BEGIN));
        let text = format_message(id, &name);
        display_message(text.as_ptr(), kind, 0, entity);
    }

    /// `ZTUnit::getSellPrice` (`0x0061338b`): the type's purchase cost times a global factor.
    pub fn get_sell_price(self) -> f32 {
        let entity_type: u32 = self.read(unit_offsets::ENTITY_TYPE);
        if entity_type == 0 {
            return 0.0;
        }
        let cost = unsafe { transmute::<u32, extern "thiscall" fn(u32) -> f32>(vtable_slot(entity_type, TYPE_SLOT_PURCHASE_COST))(entity_type) };
        cost * get_from_memory::<f32>(base() + SELL_FACTOR_RVA)
    }
}

#[detour_mod]
mod message_detours {
    use super::*;

    #[detour(FORMAT_NAME_MESSAGE)]
    unsafe extern "thiscall" fn format_name_message(this: *const u32, out: *const u32, id: u32) -> *const u32 {
        let words = ZTUnit(this as u32).format_name_message(id).release();
        unsafe { std::ptr::copy_nonoverlapping(words.as_ptr(), out as *mut u32, 3) };
        out
    }

    #[detour(F_ZOO_MESSAGE)]
    unsafe extern "thiscall" fn f_zoo_message(this: *const u32, message: *const i32, kind: u32, tile: u32) {
        ZTUnit(this as u32).f_zoo_message(message as *const u32, kind as i32, tile);
    }

    #[detour(F_ZOO_MESSAGE_NAME)]
    unsafe extern "thiscall" fn f_zoo_message_name(this: *const u32, id: u32, kind: u32, force: i8) {
        ZTUnit(this as u32).f_zoo_message_name(id, kind as i32, force as u8);
    }

    #[detour(F_ZOO_MESSAGE_TILE_0)]
    unsafe extern "thiscall" fn f_zoo_message_tile(this: *const u32, id: u32, kind: u32, tile: u32, entity: i32, force: i8) {
        ZTUnit(this as u32).f_zoo_message_tile(id, kind as i32, tile, entity as u32, force as u8);
    }

    #[detour(F_ZOO_MESSAGE_ENTITY)]
    unsafe extern "thiscall" fn f_zoo_message_entity(this: *const u32, id: u32, kind: u32, entity: i32, force: i8) {
        ZTUnit(this as u32).f_zoo_message_entity(id, kind as i32, entity as u32, force as u8);
    }

    #[detour(GET_SELL_PRICE)]
    unsafe extern "thiscall" fn get_sell_price(this: *const u32) -> f32 {
        ZTUnit(this as u32).get_sell_price()
    }

    /// Live-test access to the real vanilla bodies.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        pub(crate) fn format_name_message(this: u32, out: *const u32, id: u32) {
            unsafe { super::FORMAT_NAME_MESSAGE_DETOUR.call(this as *const u32, out, id) };
        }

        pub(crate) fn f_zoo_message_name(this: u32, id: u32, kind: u32, force: i8) {
            unsafe { super::F_ZOO_MESSAGE_NAME_DETOUR.call(this as *const u32, id, kind, force) }
        }

        pub(crate) fn f_zoo_message_tile(this: u32, id: u32, kind: u32, tile: u32, entity: i32, force: i8) {
            unsafe { super::F_ZOO_MESSAGE_TILE_0_DETOUR.call(this as *const u32, id, kind, tile, entity, force) }
        }

        pub(crate) fn f_zoo_message_entity(this: u32, id: u32, kind: u32, entity: i32, force: i8) {
            unsafe { super::F_ZOO_MESSAGE_ENTITY_DETOUR.call(this as *const u32, id, kind, entity, force) }
        }

        pub(crate) fn get_sell_price(this: u32) -> f32 {
            unsafe { super::GET_SELL_PRICE_DETOUR.call(this as *const u32) }
        }
    }
}

/// Installs the message detours.
pub(super) fn init() {
    if let Err(e) = unsafe { message_detours::init_detours() } {
        error!("Failed to initialise ztunit message detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(super) fn status() -> Vec<(&'static str, bool)> {
    message_detours::status()
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) use message_detours::test_real;
