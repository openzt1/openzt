//! Redirects the UI calls `ZTAnimal::showEscapedAnimalAlert` makes (`BFUIMgr::getElement`, `UIControl::setAnimation`, the label element's
//! caption slot and `BFUIMgr::confirmDialog`) into an in-memory record while a capture window is open, so the live test can compare what
//! real vanilla and the port would show without opening a dialog. While capturing, `getElement` answers two fake elements: the first call
//! (the icon) and the second (the label). The label's vtable records the caption string it is handed and, like the real slot, takes
//! ownership of it (the test leaks it). Same thread-local capture-window shape as `display_message_recorder.rs`.

use std::cell::{Cell, RefCell};

use openzt_detour::generated::{
    bfuimgr::{CONFIRM_DIALOG_0, GET_ELEMENT_0},
    uicontrol::SET_ANIMATION,
};
use openzt_detour_macro::detour_mod;

use crate::util::get_from_memory;

/// One recorded UI call.
#[derive(Clone, PartialEq, Eq, Debug)]
pub enum AlertCall {
    GetElement(i32),
    SetAnimation { element: &'static str, path: String, flag: bool },
    Caption { text: String, flag: u32 },
    ConfirmDialog { id: i32, a: u32, b: i8, c: i8, entity: u32 },
}

thread_local! {
    static ACTIVE: Cell<bool> = const { Cell::new(false) };
    static ELEMENT_CALLS: Cell<u32> = const { Cell::new(0) };
    static RECORDED: RefCell<Vec<AlertCall>> = const { RefCell::new(Vec::new()) };
}

static mut ICON_OBJECT: [u32; 4] = [0; 4];
static mut LABEL_OBJECT: [u32; 4] = [0; 4];

fn record(call: AlertCall) {
    RECORDED.with(|r| r.borrow_mut().push(call));
}

extern "thiscall" fn fake_caption(_this: u32, begin: u32, end: u32, _capacity: u32, flag: u32) {
    let text = String::from_utf8_lossy(&(begin..end).map(get_from_memory::<u8>).collect::<Vec<u8>>()).into_owned();
    record(AlertCall::Caption { text, flag });
}

pub fn begin_capture() {
    let vtable: &'static mut [u32; 64] = Box::leak(Box::new([0u32; 64]));
    vtable[0xc4 / 4] = fake_caption as *const () as usize as u32;
    unsafe {
        let label = &raw mut LABEL_OBJECT;
        (*label)[0] = vtable.as_ptr() as u32;
    }
    RECORDED.with(|r| r.borrow_mut().clear());
    ELEMENT_CALLS.with(|c| c.set(0));
    ACTIVE.with(|a| a.set(true));
}

pub fn end_capture() -> Vec<AlertCall> {
    ACTIVE.with(|a| a.set(false));
    RECORDED.with(|r| std::mem::take(&mut *r.borrow_mut()))
}

fn capturing() -> bool {
    ACTIVE.with(|a| a.get())
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(GET_ELEMENT_0)]
    unsafe extern "thiscall" fn get_element(this: *const u32, id: i32) -> *const u32 {
        if capturing() {
            record(AlertCall::GetElement(id));
            let call = ELEMENT_CALLS.with(|c| {
                let n = c.get();
                c.set(n + 1);
                n
            });
            return if call == 0 { (&raw const ICON_OBJECT) as *const u32 } else { (&raw const LABEL_OBJECT) as *const u32 };
        }
        unsafe { GET_ELEMENT_0_DETOUR.call(this, id) }
    }

    #[detour(SET_ANIMATION)]
    unsafe extern "thiscall" fn set_animation(this: *const u32, path: *const i8, flag: bool) {
        if capturing() {
            let mut bytes = Vec::new();
            let mut at = path as u32;
            while get_from_memory::<u8>(at) != 0 {
                bytes.push(get_from_memory::<u8>(at));
                at += 1;
            }
            record(AlertCall::SetAnimation { element: "icon", path: String::from_utf8_lossy(&bytes).into_owned(), flag });
            return;
        }
        unsafe { SET_ANIMATION_DETOUR.call(this, path, flag) };
    }

    #[detour(CONFIRM_DIALOG_0)]
    unsafe extern "thiscall" fn confirm_dialog(this: *const u32, id: i32, a: u32, b: i8, c: i8, entity: i32) -> u32 {
        if capturing() {
            record(AlertCall::ConfirmDialog { id, a, b, c, entity: entity as u32 });
            return 0;
        }
        unsafe { CONFIRM_DIALOG_0_DETOUR.call(this, id, a, b, c, entity) }
    }
}

pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        tracing::error!("Failed to initialise alert_recorder detours: {e:?}");
    }
}
