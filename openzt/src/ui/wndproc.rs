use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};

use egui::{Event, Modifiers, PointerButton, Pos2, Vec2};
use openzt_detour::generated::bfwindow::ATTACH;
use openzt_detour_macro::detour_mod;
use tracing::{error, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcA, GWLP_WNDPROC, SetWindowLongPtrA, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL, WM_RBUTTONDOWN, WM_RBUTTONUP, WNDPROC,
};

static ORIGINAL_WNDPROC: OnceLock<Mutex<Option<isize>>> = OnceLock::new();

#[detour_mod]
mod window_hooks {
    use super::*;

    #[detour(ATTACH)]
    unsafe extern "thiscall" fn bfwindow_attach(this: *const c_void, hwnd: i32) -> bool {
        let result = unsafe { ATTACH_DETOUR.call(this, hwnd) };
        if result && hwnd != 0 {
            crate::ui::capture_hwnd(HWND(hwnd as usize as *mut c_void));
        }
        result
    }
}

pub fn init() {
    match unsafe { window_hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized window capture detour"),
        Err(err) => error!("egui overlay: failed to initialize window capture detour: {err}"),
    }
}

pub fn subclass(hwnd: HWND) {
    let storage = ORIGINAL_WNDPROC.get_or_init(|| Mutex::new(None));
    let mut original = match storage.lock() {
        Ok(original) => original,
        Err(err) => {
            error!("egui overlay: original WndProc lock poisoned: {err}");
            return;
        }
    };

    if original.is_some() {
        return;
    }

    let previous = unsafe { SetWindowLongPtrA(hwnd, GWLP_WNDPROC, overlay_wndproc as *const () as usize as i32) };
    if previous == 0 {
        warn!("egui overlay: SetWindowLongPtrA returned 0 while subclassing");
        return;
    }

    *original = Some(previous as isize);
    info!("egui overlay: subclassed WndProc");
}

unsafe extern "system" fn overlay_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    match msg {
        WM_MOUSEMOVE => {
            crate::ui::push_event(Event::PointerMoved(pointer_pos(lparam)));
            if crate::ui::wants_pointer_input() {
                return LRESULT(0);
            }
        }
        WM_LBUTTONDOWN => {
            push_pointer_button(lparam, PointerButton::Primary, true);
            if crate::ui::wants_pointer_input() {
                return LRESULT(0);
            }
        }
        WM_LBUTTONUP => {
            push_pointer_button(lparam, PointerButton::Primary, false);
            if crate::ui::wants_pointer_input() {
                return LRESULT(0);
            }
        }
        WM_RBUTTONDOWN => {
            push_pointer_button(lparam, PointerButton::Secondary, true);
            if crate::ui::wants_pointer_input() {
                return LRESULT(0);
            }
        }
        WM_RBUTTONUP => {
            push_pointer_button(lparam, PointerButton::Secondary, false);
            if crate::ui::wants_pointer_input() {
                return LRESULT(0);
            }
        }
        WM_MOUSEWHEEL => {
            let delta = wheel_delta(wparam);
            crate::ui::push_event(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, delta),
                modifiers: Modifiers::default(),
            });
            if crate::ui::wants_pointer_input() {
                return LRESULT(0);
            }
        }
        _ => {}
    }

    call_original(hwnd, msg, wparam, lparam)
}

fn push_pointer_button(lparam: LPARAM, button: PointerButton, pressed: bool) {
    crate::ui::push_event(Event::PointerButton {
        pos: pointer_pos(lparam),
        button,
        pressed,
        modifiers: Modifiers::default(),
    });
}

fn pointer_pos(lparam: LPARAM) -> Pos2 {
    let value = lparam.0 as u32;
    let x = (value & 0xffff) as u16 as i16 as f32;
    let y = ((value >> 16) & 0xffff) as u16 as i16 as f32;
    Pos2::new(x, y)
}

fn wheel_delta(wparam: WPARAM) -> f32 {
    let value = wparam.0 as u32;
    let delta = ((value >> 16) & 0xffff) as u16 as i16;
    delta as f32
}

fn call_original(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    let original = ORIGINAL_WNDPROC.get_or_init(|| Mutex::new(None)).lock().ok().and_then(|original| *original);

    if let Some(original) = original {
        let proc: WNDPROC = unsafe { std::mem::transmute(original) };
        unsafe { CallWindowProcA(proc, hwnd, msg, wparam, lparam) }
    } else {
        LRESULT(0)
    }
}
