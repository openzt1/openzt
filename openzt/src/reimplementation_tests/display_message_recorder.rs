//! Redirects `BFUIMgr::displayMessage` (both overloads, `bfuimgr::DISPLAY_MESSAGE_0`/`_1`) into an in-memory
//! record of `(text, priority, tile_ptr, entity_ptr)` instead of showing a toast, so the live tank tests can
//! compare the messages real vanilla and the port would display. The string-id overload is recorded as
//! `id:0x...`; the `std::string` overload as its text. Same thread-local capture-window shape as
//! `send_event_recorder.rs`.

use std::cell::{Cell, RefCell};

use openzt_detour::generated::bfuimgr::{DISPLAY_MESSAGE_0, DISPLAY_MESSAGE_1};
use openzt_detour_macro::detour_mod;

use crate::util::get_from_memory;

pub type RecordedMessage = (String, i32, u32, u32);

thread_local! {
    static REDIRECT_ACTIVE: Cell<bool> = const { Cell::new(false) };
    static RECORDED: RefCell<Vec<RecordedMessage>> = const { RefCell::new(Vec::new()) };
}

pub fn begin_capture() {
    RECORDED.with(|r| r.borrow_mut().clear());
    REDIRECT_ACTIVE.with(|a| a.set(true));
}

pub fn end_capture() -> Vec<RecordedMessage> {
    REDIRECT_ACTIVE.with(|a| a.set(false));
    RECORDED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

fn capturing() -> bool {
    REDIRECT_ACTIVE.with(|a| a.get())
}

fn record(message: RecordedMessage) {
    RECORDED.with(|r| r.borrow_mut().push(message));
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(DISPLAY_MESSAGE_0)]
    unsafe extern "thiscall" fn display_message_0(this: *const u32, string_id: u32, priority: i32, tile: *const u32, entity: *const u32, flag_a: bool, flag_b: bool) {
        if capturing() {
            record((format!("id:{string_id:#x}"), priority, tile as u32, entity as u32));
            return;
        }
        unsafe { DISPLAY_MESSAGE_0_DETOUR.call(this, string_id, priority, tile, entity, flag_a, flag_b) };
    }

    #[detour(DISPLAY_MESSAGE_1)]
    unsafe extern "thiscall" fn display_message_1(this: *const u32, message: *const i32, priority: i32, tile: *const u32, entity: *const u32, flag_a: bool, flag_b: bool) {
        if capturing() {
            let (begin, end) = (get_from_memory::<u32>(message as u32), get_from_memory::<u32>(message as u32 + 4));
            let bytes: Vec<u8> = (begin..end).map(get_from_memory::<u8>).collect();
            record((String::from_utf8_lossy(&bytes).into_owned(), priority, tile as u32, entity as u32));
            return;
        }
        unsafe { DISPLAY_MESSAGE_1_DETOUR.call(this, message, priority, tile, entity, flag_a, flag_b) };
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        tracing::error!("Failed to initialise display_message_recorder detours: {e:?}");
    }
}
