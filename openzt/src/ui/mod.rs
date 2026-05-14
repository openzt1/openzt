mod cursor;
mod blit;
mod input_block;
mod live_game;
mod render_hook;
mod tga;
mod vanilla_main;
mod wndproc;
mod zt_image;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use egui::{CursorIcon, Event};
use egui_tiny_skia::TinySkiaBackend;
use tracing::{error, info, warn};
use windows::Win32::Foundation::{HWND, RECT};
use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, GetClientRect};
use windows::core::PCSTR;

static BACKEND: OnceLock<Mutex<TinySkiaBackend>> = OnceLock::new();
static INPUT_EVENTS: OnceLock<Mutex<Vec<Event>>> = OnceLock::new();
static HWND_RAW: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static CAPTURE_STATE: OnceLock<Mutex<InputCaptureState>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();
static LIVE_GAME_ACTIVE: AtomicBool = AtomicBool::new(false);
static SHOW_VANILLA_UI: AtomicBool = AtomicBool::new(true);
const OPENZT_WINDOW_RESIZABLE: bool = true;

#[derive(Default)]
struct InputCaptureState {
    pointer_over_area: bool,
    pointer_over_resize_bounds: bool,
    wants_pointer_input: bool,
    wants_keyboard_input: bool,
    platform_output_warned: bool,
}

pub fn init() {
    info!("Initializing egui overlay");

    if let Some(hwnd) = find_zoo_window() {
        capture_hwnd(hwnd);
    } else {
        info!("egui overlay: Zoo Tycoon HWND not available yet; waiting for window capture");
    }

    cursor::init();
    input_block::init();
    live_game::init();
    wndproc::init();
    render_hook::init();
}

pub fn render_and_blit(hwnd: HWND) {
    if !is_live_game_active() {
        blit::hide_overlay();
        return;
    }

    let Some((width, height)) = client_size(hwnd) else {
        return;
    };

    #[cfg(feature = "debug-blit")]
    {
        use tiny_skia::{Color, Pixmap};

        let mut pixmap = match Pixmap::new(width, height) {
            Some(pixmap) => pixmap,
            None => return,
        };
        pixmap.fill(Color::from_rgba8(255, 0, 0, 255));
        blit::blit_to_hwnd(hwnd, &pixmap);
    }

    #[cfg(not(feature = "debug-blit"))]
    {
        let events = drain_input_events();
        let backend_lock = BACKEND.get_or_init(|| Mutex::new(TinySkiaBackend::new(width, height)));
        let mut backend = match backend_lock.lock() {
            Ok(backend) => backend,
            Err(err) => {
                error!("egui overlay: backend lock poisoned: {err}");
                return;
            }
        };

        if backend.pixmap().width() != width || backend.pixmap().height() != height {
            backend.resize(width, height);
        }

        let raw_input = egui::RawInput {
            screen_rect: Some(egui::Rect::from_min_size(egui::Pos2::ZERO, egui::vec2(width as f32, height as f32))),
            time: Some(START_TIME.get_or_init(Instant::now).elapsed().as_secs_f64()),
            events,
            ..Default::default()
        };

        let output = backend.run_frame(raw_input, |ctx| {
            ctx.style_mut(|style| {
                style.visuals.window_shadow = egui::epaint::Shadow::NONE;
                style.visuals.popup_shadow = egui::epaint::Shadow::NONE;
            });

            if SHOW_VANILLA_UI.load(Ordering::Acquire) {
                vanilla_main::show(ctx, egui::vec2(width as f32, height as f32));
            }

            egui::Window::new("OpenZT")
                .default_size(egui::vec2(240.0, 80.0))
                .min_size(egui::vec2(180.0, 60.0))
                .resizable(true)
                .show(ctx, |ui| {
                    let mut show_vanilla_ui = SHOW_VANILLA_UI.load(Ordering::Acquire);
                    if ui.checkbox(&mut show_vanilla_ui, "Show vanilla UI").changed() {
                        SHOW_VANILLA_UI.store(show_vanilla_ui, Ordering::Release);
                    }
                    ui.allocate_space(ui.available_size());
                });
        });

        {
            let mut state = capture_state();
            let pointer_over_resize_bounds = OPENZT_WINDOW_RESIZABLE && is_resize_cursor(output.platform_output.cursor_icon);
            state.pointer_over_area = backend.context().is_pointer_over_area();
            state.pointer_over_resize_bounds = pointer_over_resize_bounds;
            state.wants_pointer_input = backend.context().wants_pointer_input();
            state.wants_keyboard_input = backend.context().wants_keyboard_input();
            cursor::apply_egui_cursor(
                output.platform_output.cursor_icon,
                state.pointer_over_area || state.pointer_over_resize_bounds || state.wants_pointer_input,
            );
            if !state.platform_output_warned && !output.platform_output.commands.is_empty() {
                warn!("egui overlay: platform output requested but not implemented");
                state.platform_output_warned = true;
            }
        }

        let pixmap = backend.paint(output);
        blit::blit_to_hwnd(hwnd, pixmap);
    }
}

pub fn sync_overlay_position(hwnd: HWND) {
    if !is_live_game_active() {
        return;
    }

    blit::sync_overlay_position(hwnd);
}

pub fn push_event(event: Event) {
    if !is_live_game_active() {
        return;
    }

    let events = INPUT_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    match events.lock() {
        Ok(mut events) => events.push(event),
        Err(err) => error!("egui overlay: input event lock poisoned: {err}"),
    }
}

pub fn wants_pointer_input() -> bool {
    if !is_live_game_active() {
        return false;
    }

    capture_state().wants_pointer_input
}

pub fn blocks_pointer_input() -> bool {
    if !is_live_game_active() {
        return false;
    }

    let state = capture_state();
    state.pointer_over_area || state.pointer_over_resize_bounds || state.wants_pointer_input
}

pub fn wants_keyboard_input() -> bool {
    if !is_live_game_active() {
        return false;
    }

    capture_state().wants_keyboard_input
}

pub fn capture_hwnd(hwnd: HWND) {
    if hwnd.0.is_null() {
        return;
    }

    let hwnd_raw = hwnd.0 as isize;
    let storage = HWND_RAW.get_or_init(|| Mutex::new(None));
    let mut should_subclass = false;
    match storage.lock() {
        Ok(mut stored) => {
            if *stored != Some(hwnd_raw) {
                *stored = Some(hwnd_raw);
                should_subclass = true;
            }
        }
        Err(err) => {
            error!("egui overlay: HWND storage lock poisoned: {err}");
            return;
        }
    }

    if should_subclass {
        info!("egui overlay: captured HWND {hwnd_raw:#x}");
        wndproc::subclass(hwnd);
    }
}

pub fn captured_hwnd() -> Option<HWND> {
    let hwnd_raw = *HWND_RAW.get_or_init(|| Mutex::new(None)).lock().ok()?;
    hwnd_raw.map(|raw| HWND(raw as *mut c_void))
}

pub fn set_live_game_active(active: bool) {
    if LIVE_GAME_ACTIVE.swap(active, Ordering::AcqRel) == active {
        return;
    }

    if active {
        info!("egui overlay: live game started");
    } else {
        info!("egui overlay: live game stopped");
        drain_input_events();
        reset_capture_state();
        blit::hide_overlay();
        cursor::clear_egui_cursor();
    }
}

pub fn is_live_game_active() -> bool {
    LIVE_GAME_ACTIVE.load(Ordering::Acquire)
}

fn drain_input_events() -> Vec<Event> {
    let events = INPUT_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    match events.lock() {
        Ok(mut events) => events.drain(..).collect(),
        Err(err) => {
            error!("egui overlay: input event lock poisoned: {err}");
            Vec::new()
        }
    }
}

fn capture_state() -> std::sync::MutexGuard<'static, InputCaptureState> {
    CAPTURE_STATE
        .get_or_init(|| Mutex::new(InputCaptureState::default()))
        .lock()
        .expect("egui overlay capture state lock poisoned")
}

fn reset_capture_state() {
    *capture_state() = InputCaptureState::default();
}

fn client_size(hwnd: HWND) -> Option<(u32, u32)> {
    let mut rect = RECT::default();
    if let Err(err) = unsafe { GetClientRect(hwnd, &mut rect) } {
        warn!("egui overlay: GetClientRect failed: {err}");
        return None;
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 {
        return None;
    }

    Some((width as u32, height as u32))
}

fn find_zoo_window() -> Option<HWND> {
    unsafe { FindWindowA(PCSTR::null(), windows::core::s!("Zoo Tycoon")).ok().filter(|hwnd| !hwnd.0.is_null()) }
}

fn is_resize_cursor(cursor_icon: CursorIcon) -> bool {
    matches!(
        cursor_icon,
        CursorIcon::ResizeHorizontal
            | CursorIcon::ResizeNeSw
            | CursorIcon::ResizeNwSe
            | CursorIcon::ResizeVertical
            | CursorIcon::ResizeEast
            | CursorIcon::ResizeSouthEast
            | CursorIcon::ResizeSouth
            | CursorIcon::ResizeSouthWest
            | CursorIcon::ResizeWest
            | CursorIcon::ResizeNorthWest
            | CursorIcon::ResizeNorth
            | CursorIcon::ResizeNorthEast
            | CursorIcon::ResizeColumn
            | CursorIcon::ResizeRow
    )
}
