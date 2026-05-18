use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};

use egui::{Event, Modifiers, PointerButton, Pos2, Vec2};
use openzt_detour::generated::bfwindow::ATTACH;
use openzt_detour_macro::detour_mod;
use tracing::{error, info, warn};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::UI::Input::KeyboardAndMouse::{ReleaseCapture, SetCapture};
use windows::Win32::UI::WindowsAndMessaging::{
    CallWindowProcA, GWLP_WNDPROC, SetWindowLongPtrA, WM_CAPTURECHANGED, WM_LBUTTONDBLCLK, WM_LBUTTONDOWN, WM_LBUTTONUP, WM_MOUSEMOVE, WM_MOUSEWHEEL,
    WM_MOVE, WM_MOVING, WM_RBUTTONDBLCLK, WM_RBUTTONDOWN, WM_RBUTTONUP, WM_SIZE, WM_WINDOWPOSCHANGED, WNDPROC,
};

static ORIGINAL_WNDPROC: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static POINTER_CAPTURED: AtomicBool = AtomicBool::new(false);

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
    let sync_position_after_original = matches!(msg, WM_MOVE | WM_MOVING | WM_SIZE | WM_WINDOWPOSCHANGED);

    match msg {
        WM_MOUSEMOVE => {
            let pos = pointer_pos(lparam);
            crate::ui::push_event(Event::PointerMoved(pos));
            if should_consume_pointer_event_at(pos) {
                return LRESULT(0);
            }
        }
        WM_LBUTTONDOWN => {
            let pos = pointer_pos(lparam);
            push_pointer_button(pos, PointerButton::Primary, true);
            if should_consume_pointer_event_at(pos) {
                capture_pointer(hwnd);
                return LRESULT(0);
            }
        }
        WM_LBUTTONDBLCLK => {
            let pos = pointer_pos(lparam);
            push_pointer_button(pos, PointerButton::Primary, true);
            if should_consume_pointer_event_at(pos) {
                capture_pointer(hwnd);
                return LRESULT(0);
            }
        }
        WM_LBUTTONUP => {
            let pos = pointer_pos(lparam);
            push_pointer_button(pos, PointerButton::Primary, false);
            if should_consume_pointer_event_at(pos) {
                release_pointer_capture();
                return LRESULT(0);
            }
            release_pointer_capture();
        }
        WM_RBUTTONDOWN => {
            let pos = pointer_pos(lparam);
            push_pointer_button(pos, PointerButton::Secondary, true);
            if should_consume_pointer_event_at(pos) {
                capture_pointer(hwnd);
                return LRESULT(0);
            }
        }
        WM_RBUTTONDBLCLK => {
            let pos = pointer_pos(lparam);
            push_pointer_button(pos, PointerButton::Secondary, true);
            if should_consume_pointer_event_at(pos) {
                capture_pointer(hwnd);
                return LRESULT(0);
            }
        }
        WM_RBUTTONUP => {
            let pos = pointer_pos(lparam);
            push_pointer_button(pos, PointerButton::Secondary, false);
            if should_consume_pointer_event_at(pos) {
                release_pointer_capture();
                return LRESULT(0);
            }
            release_pointer_capture();
        }
        WM_MOUSEWHEEL => {
            let delta = wheel_delta(wparam);
            let pos = crate::ui::current_pointer_pos().or_else(crate::ui::last_pointer_pos);
            crate::ui::push_event(Event::MouseWheel {
                unit: egui::MouseWheelUnit::Point,
                delta: Vec2::new(0.0, delta),
                modifiers: Modifiers::default(),
            });
            if pos.map_or_else(should_consume_pointer_event, should_consume_pointer_event_at) {
                return LRESULT(0);
            }
        }
        WM_CAPTURECHANGED => {
            POINTER_CAPTURED.store(false, Ordering::Relaxed);
        }
        _ => {}
    }

    let result = call_original(hwnd, msg, wparam, lparam);
    if sync_position_after_original {
        crate::ui::sync_overlay_position(hwnd);
    }
    result
}

fn should_consume_pointer_event() -> bool {
    POINTER_CAPTURED.load(Ordering::Relaxed) || crate::ui::blocks_pointer_input()
}

fn should_consume_pointer_event_at(pos: Pos2) -> bool {
    POINTER_CAPTURED.load(Ordering::Relaxed) || crate::ui::blocks_pointer_input_at(pos)
}

fn capture_pointer(hwnd: HWND) {
    unsafe {
        SetCapture(hwnd);
    }
    POINTER_CAPTURED.store(true, Ordering::Relaxed);
}

fn release_pointer_capture() {
    if POINTER_CAPTURED.swap(false, Ordering::Relaxed) {
        unsafe {
            let _ = ReleaseCapture();
        }
    }
}

fn push_pointer_button(pos: Pos2, button: PointerButton, pressed: bool) {
    crate::ui::push_event(Event::PointerButton {
        pos,
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
