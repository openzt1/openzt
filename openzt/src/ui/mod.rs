mod cursor;
mod blit;
mod input_block;
mod live_game;
mod render_hook;
mod tga;
mod tooltip;
mod vanilla_main;
mod wndproc;
mod zt_image;

use std::ffi::c_void;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use crate::shortcuts::{Ctrl, Shift, U, V};
use egui::{CursorIcon, Event, Pos2, Sense, Stroke, StrokeKind, Vec2};
use egui_tiny_skia::TinySkiaBackend;
use tracing::{error, info, warn};
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::ScreenToClient;
use windows::Win32::UI::WindowsAndMessaging::{FindWindowA, GetClientRect, GetCursorPos};
use windows::core::PCSTR;

static BACKEND: OnceLock<Mutex<TinySkiaBackend>> = OnceLock::new();
static INPUT_EVENTS: OnceLock<Mutex<Vec<Event>>> = OnceLock::new();
static LAST_POINTER_POS: OnceLock<Mutex<Option<Pos2>>> = OnceLock::new();
static LAST_CLIENT_SIZE: OnceLock<Mutex<Option<Vec2>>> = OnceLock::new();
static HWND_RAW: OnceLock<Mutex<Option<isize>>> = OnceLock::new();
static CAPTURE_STATE: OnceLock<Mutex<InputCaptureState>> = OnceLock::new();
static START_TIME: OnceLock<Instant> = OnceLock::new();
static LIVE_GAME_ACTIVE: AtomicBool = AtomicBool::new(false);
static SHOW_VANILLA_UI: AtomicBool = AtomicBool::new(true);
static SHOW_DEBUG_WINDOW: AtomicBool = AtomicBool::new(true);
const OPENZT_WINDOW_RESIZABLE: bool = true;

#[derive(Default)]
struct InputCaptureState {
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
    tooltip::init();
    register_shortcuts();
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
    remember_client_size(width, height);

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
            tooltip::show(ctx, egui::vec2(width as f32, height as f32));

            if SHOW_DEBUG_WINDOW.load(Ordering::Acquire) {
                show_debug_panel(ctx);
            }
        });

        {
            let mut state = capture_state();
            let pointer_over_resize_bounds = OPENZT_WINDOW_RESIZABLE && is_resize_cursor(output.platform_output.cursor_icon);
            let pointer_over_blocking_overlay = current_pointer_pos().is_some_and(blocks_pointer_overlay_at);
            state.pointer_over_resize_bounds = pointer_over_resize_bounds;
            state.wants_pointer_input = backend.context().wants_pointer_input();
            state.wants_keyboard_input = backend.context().wants_keyboard_input();
            cursor::apply_egui_cursor(
                output.platform_output.cursor_icon,
                state.pointer_over_resize_bounds || state.wants_pointer_input || pointer_over_blocking_overlay,
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

    remember_pointer_pos(&event);

    let events = INPUT_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    match events.lock() {
        Ok(mut events) => events.push(event),
        Err(err) => error!("egui overlay: input event lock poisoned: {err}"),
    }
}

pub fn last_pointer_pos() -> Option<Pos2> {
    *LAST_POINTER_POS.get_or_init(|| Mutex::new(None)).lock().ok()?
}

pub fn current_pointer_pos() -> Option<Pos2> {
    let Some(hwnd) = captured_hwnd() else {
        return None;
    };
    let mut point = POINT::default();
    if unsafe { GetCursorPos(&mut point) }.is_err() {
        return None;
    }
    if !unsafe { ScreenToClient(hwnd, &mut point) }.as_bool() {
        return None;
    }

    Some(Pos2::new(point.x as f32, point.y as f32))
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

    frame_blocks_pointer_input()
        || current_pointer_pos()
            .or_else(last_pointer_pos)
            .is_some_and(blocks_pointer_overlay_at)
}

pub fn blocks_pointer_input_at(pos: Pos2) -> bool {
    if !is_live_game_active() {
        return false;
    }

    frame_blocks_pointer_input() || blocks_pointer_overlay_at(pos)
}

fn frame_blocks_pointer_input() -> bool {
    let state = capture_state();
    state.pointer_over_resize_bounds || state.wants_pointer_input
}

fn blocks_pointer_overlay_at(pos: Pos2) -> bool {
    let Some(screen_size) = last_client_size() else {
        return false;
    };

    if SHOW_VANILLA_UI.load(Ordering::Acquire) && vanilla_main::blocks_pointer_at(pos, screen_size) {
        return true;
    }

    SHOW_DEBUG_WINDOW.load(Ordering::Acquire) && debug_panel_rect(screen_size).contains(pos)
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

fn remember_client_size(width: u32, height: u32) {
    if let Ok(mut stored) = LAST_CLIENT_SIZE.get_or_init(|| Mutex::new(None)).lock() {
        *stored = Some(Vec2::new(width as f32, height as f32));
    }
}

fn last_client_size() -> Option<Vec2> {
    *LAST_CLIENT_SIZE.get_or_init(|| Mutex::new(None)).lock().ok()?
}

fn remember_pointer_pos(event: &Event) {
    let pos = match event {
        Event::PointerMoved(pos) => Some(*pos),
        Event::PointerButton { pos, .. } => Some(*pos),
        _ => None,
    };

    if let Some(pos) = pos {
        if let Ok(mut stored) = LAST_POINTER_POS.get_or_init(|| Mutex::new(None)).lock() {
            *stored = Some(pos);
        }
    }
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

fn register_shortcuts() {
    crate::shortcut!("egui-overlay", "Toggle egui debug window", U + Ctrl + Shift, false, || {
        let visible = !SHOW_DEBUG_WINDOW.load(Ordering::Acquire);
        SHOW_DEBUG_WINDOW.store(visible, Ordering::Release);
        info!("egui overlay: debug window {}", if visible { "shown" } else { "hidden" });
    });

    crate::shortcut!("egui-overlay", "Toggle vanilla UI overlay", V + Ctrl + Shift, false, || {
        let visible = !SHOW_VANILLA_UI.load(Ordering::Acquire);
        SHOW_VANILLA_UI.store(visible, Ordering::Release);
        info!("egui overlay: vanilla UI overlay {}", if visible { "shown" } else { "hidden" });
    });
}

fn show_debug_panel(ctx: &egui::Context) {
    egui::Area::new("openzt_debug_panel".into())
        .order(egui::Order::Foreground)
        .anchor(egui::Align2::RIGHT_TOP, egui::vec2(-16.0, 72.0))
        .interactable(true)
        .show(ctx, |ui| {
            let panel_size = Vec2::new(180.0, 38.0);
            let (rect, _) = ui.allocate_exact_size(panel_size, Sense::hover());
            let visuals = ui.visuals();
            ui.painter().rect_filled(rect, 0.0, visuals.panel_fill);
            ui.painter().rect_stroke(rect, 0.0, Stroke::new(1.0, visuals.window_stroke.color), StrokeKind::Inside);

            let inner_rect = rect.shrink(8.0);
            ui.scope_builder(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
                let mut show_vanilla_ui = SHOW_VANILLA_UI.load(Ordering::Acquire);
                if ui.checkbox(&mut show_vanilla_ui, "Show vanilla UI").changed() {
                    SHOW_VANILLA_UI.store(show_vanilla_ui, Ordering::Release);
                }
            });
        });
}

fn debug_panel_rect(screen_size: Vec2) -> egui::Rect {
    egui::Rect::from_min_size(Pos2::new((screen_size.x - 196.0).max(0.0), 72.0), Vec2::new(180.0, 38.0))
}
