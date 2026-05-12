mod blit;
mod render_hook;
mod wndproc;

use std::ffi::c_void;
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

use egui::Event;
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

#[derive(Default)]
struct InputCaptureState {
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

    wndproc::init();
    render_hook::init();
}

pub fn render_and_blit(hwnd: HWND) {
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

            egui::Window::new("OpenZT")
                .default_size(egui::vec2(320.0, 180.0))
                .min_size(egui::vec2(180.0, 80.0))
                .resizable(true)
                .show(ctx, |ui| {
                    ui.label("Hello from OpenZT");
                    ui.allocate_space(ui.available_size());
                });
        });

        {
            let mut state = capture_state();
            state.wants_pointer_input = backend.context().wants_pointer_input();
            state.wants_keyboard_input = backend.context().wants_keyboard_input();
            if !state.platform_output_warned && (!output.platform_output.commands.is_empty() || output.platform_output.cursor_icon != egui::CursorIcon::Default) {
                warn!("egui overlay: platform output requested but not implemented");
                state.platform_output_warned = true;
            }
        }

        let pixmap = backend.paint(output);
        blit::blit_to_hwnd(hwnd, pixmap);
    }
}

pub fn sync_overlay_position(hwnd: HWND) {
    blit::sync_overlay_position(hwnd);
}

pub fn push_event(event: Event) {
    let events = INPUT_EVENTS.get_or_init(|| Mutex::new(Vec::new()));
    match events.lock() {
        Ok(mut events) => events.push(event),
        Err(err) => error!("egui overlay: input event lock poisoned: {err}"),
    }
}

pub fn wants_pointer_input() -> bool {
    capture_state().wants_pointer_input
}

pub fn wants_keyboard_input() -> bool {
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
