use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use egui::{Color32, Context, FontId, Frame, Margin, Pos2, RichText, Stroke, Vec2, pos2, vec2};
use openzt_detour::generated::bfuimgr::{DISPLAY_HELP_0, DISPLAY_HELP_1};
use openzt_detour_macro::detour_mod;
use tracing::{error, info, warn};

use crate::util::get_string_from_memory_bounded;

const DEFAULT_TOOLTIP_DELAY_MS: u64 = 500;
const DEFAULT_TOOLTIP_DURATION_MS: u64 = 10_000;
const MAX_TOOLTIP_CHARS: usize = 512;
const MAX_TOOLTIP_TEXT_BYTES: u32 = 2048;
const MAX_INLINE_STRING_BYTES: u32 = 4096;
const LONG_TOOLTIP_ID_OFFSET: u32 = 30_000;

static TOOLTIP: OnceLock<Mutex<TooltipState>> = OnceLock::new();

#[derive(Default)]
struct TooltipState {
    current: Option<TooltipRequest>,
}

struct TooltipRequest {
    text: String,
    requested_at: Instant,
    anchor: Option<Pos2>,
    delay: Duration,
    duration: Duration,
    allow_blocked_pointer: bool,
}

#[detour_mod]
mod tooltip_hooks {
    use super::*;

    #[detour(DISPLAY_HELP_1)]
    unsafe extern "thiscall" fn bfuimgr_display_help_1(this: *const u32, help_text: *const i32) {
        if !crate::ui::is_live_game_active() {
            unsafe { DISPLAY_HELP_1_DETOUR.call(this, help_text) };
            return;
        }

        if crate::ui::blocks_pointer_input() {
            clear_tooltip();
            return;
        }

        match tooltip_text(help_text) {
            Some(text) => {
                set_tooltip(text, false);
            }
            None => {
                clear_tooltip();
            }
        }
    }

    #[detour(DISPLAY_HELP_0)]
    unsafe extern "thiscall" fn bfuimgr_display_help_0(this: *const u32, help_id: i32, has_long_tooltip: i8) {
        if !crate::ui::is_live_game_active() {
            unsafe { DISPLAY_HELP_0_DETOUR.call(this, help_id, has_long_tooltip) };
            return;
        }

        if crate::ui::blocks_pointer_input() {
            clear_tooltip();
            return;
        }

        match tooltip_text_from_id(help_id, has_long_tooltip) {
            Some(text) => set_tooltip(text, false),
            None => clear_tooltip(),
        }
    }
}

pub fn init() {
    match unsafe { tooltip_hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized tooltip detours"),
        Err(err) => error!("egui overlay: failed to initialize tooltip detours: {err}"),
    }
}

pub fn show(ctx: &Context, screen_size: Vec2) {
    let now = Instant::now();
    let Some((text, anchor)) = active_tooltip(ctx, now) else {
        return;
    };

    let max_width = screen_size.x.min(360.0).max(160.0);
    let pos = tooltip_pos(anchor, screen_size, max_width);

    egui::Area::new("openzt_tooltip".into())
        .order(egui::Order::Tooltip)
        .fixed_pos(pos)
        .interactable(false)
        .show(ctx, |ui| {
            ui.set_max_width(max_width);
            Frame::new()
                .fill(Color32::from_rgb(0x75, 0x69, 0x3a))
                .stroke(Stroke::new(1.0, Color32::from_rgb(0xf6, 0xda, 0x78)))
                .inner_margin(Margin::symmetric(7, 5))
                .show(ui, |ui| {
                    ui.label(RichText::new(text).font(FontId::proportional(13.0)).color(Color32::WHITE));
                });
        });
}

fn active_tooltip(ctx: &Context, now: Instant) -> Option<(String, Pos2)> {
    let egui_pointer = ctx.pointer_hover_pos();
    let hwnd_pointer = crate::ui::current_pointer_pos();
    let cached_pointer = crate::ui::last_pointer_pos();
    let pointer = egui_pointer.or(hwnd_pointer).or(cached_pointer);
    let mut state = tooltip_state().lock().ok()?;

    let request = state.current.as_mut()?;

    if pointer.is_some_and(crate::ui::blocks_pointer_input_at) && !request.allow_blocked_pointer {
        state.current = None;
        return None;
    }

    if let Some(pointer) = pointer {
        request.anchor = Some(pointer);
    }

    let elapsed = now.saturating_duration_since(request.requested_at);
    if elapsed < request.delay {
        ctx.request_repaint_after(request.delay - elapsed);
        return None;
    }

    let visible_for = elapsed - request.delay;
    if visible_for > request.duration {
        state.current = None;
        return None;
    }

    ctx.request_repaint_after(Duration::from_millis(100));
    let anchor = request.anchor.or(pointer).unwrap_or(pos2(24.0, 24.0));
    Some((request.text.clone(), anchor))
}

pub(crate) fn set_overlay_help_tooltip(help_id: i32) {
    match tooltip_text_from_id(help_id, 1) {
        Some(text) => set_tooltip(text, true),
        None => clear_tooltip(),
    }
}

pub(crate) fn set_overlay_tooltip(text: String) {
    set_tooltip(text, true);
}

fn set_tooltip(text: String, allow_blocked_pointer: bool) {
    let text = text.trim();
    if text.is_empty() {
        clear_tooltip();
        return;
    }

    let text = truncate_tooltip(text);
    let mut state = match tooltip_state().lock() {
        Ok(state) => state,
        Err(err) => {
            error!("egui overlay: tooltip state lock poisoned: {err}");
            return;
        }
    };

    let now = Instant::now();
    if let Some(current) = &mut state.current {
        if current.text == text {
            current.duration = tooltip_duration();
            return;
        }
    }

    state.current = Some(TooltipRequest {
        text,
        requested_at: now,
        anchor: crate::ui::current_pointer_pos().or_else(crate::ui::last_pointer_pos),
        delay: tooltip_delay(),
        duration: tooltip_duration(),
        allow_blocked_pointer,
    });
}

pub(crate) fn clear_tooltip() {
    if let Ok(mut state) = tooltip_state().lock() {
        state.current = None;
    }
}

fn tooltip_state() -> &'static Mutex<TooltipState> {
    TOOLTIP.get_or_init(|| Mutex::new(TooltipState::default()))
}

fn tooltip_pos(anchor: Pos2, screen_size: Vec2, _max_width: f32) -> Pos2 {
    let offset = vec2(16.0, 20.0);
    let x = (anchor.x + offset.x).max(8.0);
    let y = (anchor.y + offset.y).max(8.0);
    pos2(x.min(screen_size.x - 8.0).max(8.0), y.min(screen_size.y - 8.0).max(8.0))
}

fn tooltip_delay() -> Duration {
    Duration::from_millis(read_tooltip_setting("tooltipDelay", DEFAULT_TOOLTIP_DELAY_MS))
}

fn tooltip_duration() -> Duration {
    Duration::from_millis(read_tooltip_setting("tooltipDuration", DEFAULT_TOOLTIP_DURATION_MS))
}

fn read_tooltip_setting(key: &str, default_ms: u64) -> u64 {
    for section in crate::settings::zoo_setting_sections() {
        match crate::settings::get_zoo_setting_i32(&section, key) {
            Ok(Some(value)) if value >= 0 => return value as u64,
            Ok(Some(value)) => {
                warn!("egui overlay: ignoring negative {key} value {value} in zoo.ini section [{section}]");
                return default_ms;
            }
            Ok(None) => {}
            Err(err) => {
                warn!("egui overlay: ignoring invalid {key} value in zoo.ini section [{section}]: {err}");
                return default_ms;
            }
        }
    }
    default_ms
}

pub(crate) fn long_tooltips_enabled() -> bool {
    match crate::settings::get_zoo_setting_i32("UI", "helpType") {
        Ok(Some(1)) => true,
        Ok(Some(_)) | Ok(None) => false,
        Err(err) => {
            warn!("egui overlay: ignoring invalid helpType value in zoo.ini section [UI]: {err}");
            false
        }
    }
}

pub(crate) fn tooltip_text_from_id(help_id: i32, has_long_tooltip: i8) -> Option<String> {
    let string_id = tooltip_string_id(help_id, has_long_tooltip, long_tooltips_enabled())?;
    crate::string_registry::load_string_by_id(string_id).and_then(|text| {
        let text = text.trim().to_string();
        if text.is_empty() { None } else { Some(text) }
    })
}

fn tooltip_string_id(help_id: i32, has_long_tooltip: i8, long_tooltips_enabled: bool) -> Option<u32> {
    let help_id = u32::try_from(help_id).ok()?;
    if help_id == 0 {
        return None;
    }

    if long_tooltips_enabled && has_long_tooltip == 1 {
        help_id.checked_add(LONG_TOOLTIP_ID_OFFSET)
    } else {
        Some(help_id)
    }
}

fn tooltip_text(help_text: *const i32) -> Option<String> {
    let address = help_text as u32;
    if address == 0 {
        return None;
    }

    let text = read_zt_string(address).unwrap_or_else(|| {
        get_string_from_memory_bounded(
            address,
            address.saturating_add(MAX_TOOLTIP_TEXT_BYTES),
            address.saturating_add(MAX_TOOLTIP_TEXT_BYTES),
        )
    });
    let text = text.trim().to_string();
    if text.is_empty() { None } else { Some(text) }
}

fn read_zt_string(address: u32) -> Option<String> {
    let start = crate::util::get_from_memory::<u32>(address);
    let end = crate::util::get_from_memory::<u32>(address.saturating_add(4));
    let buffer_end = crate::util::get_from_memory::<u32>(address.saturating_add(8));

    if !plausible_string_range(start, end, buffer_end) {
        return None;
    }

    let text = get_string_from_memory_bounded(start, end, buffer_end);
    if text.is_empty() { None } else { Some(text) }
}

fn plausible_string_range(start: u32, end: u32, buffer_end: u32) -> bool {
    start != 0 && end >= start && buffer_end >= end && end - start <= MAX_TOOLTIP_TEXT_BYTES && buffer_end - start <= MAX_INLINE_STRING_BYTES
}

fn truncate_tooltip(text: &str) -> String {
    if text.chars().count() <= MAX_TOOLTIP_CHARS {
        return text.to_string();
    }

    let mut truncated = text.chars().take(MAX_TOOLTIP_CHARS.saturating_sub(3)).collect::<String>();
    truncated.push_str("...");
    truncated
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn display_help_0_uses_short_id_without_long_setting() {
        assert_eq!(tooltip_string_id(3383, 1, false), Some(3383));
    }

    #[test]
    fn display_help_0_uses_short_id_without_exact_long_flag() {
        assert_eq!(tooltip_string_id(3383, 2, true), Some(3383));
        assert_eq!(tooltip_string_id(3383, 0, true), Some(3383));
    }

    #[test]
    fn display_help_0_uses_long_id_only_when_setting_and_flag_match() {
        assert_eq!(tooltip_string_id(3383, 1, true), Some(33383));
    }

    #[test]
    fn display_help_0_zero_or_negative_id_clears_tooltip() {
        assert_eq!(tooltip_string_id(0, 1, true), None);
        assert_eq!(tooltip_string_id(-1, 1, true), None);
    }
}
