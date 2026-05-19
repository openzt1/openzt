use std::sync::{Mutex, OnceLock};

use openzt_detour::generated::ztui_main::SET_DATE_TEXT;
use openzt_detour_macro::detour_mod;
use nt_time::time::{Month, UtcDateTime, Weekday};
use tracing::{error, info};

#[detour_mod]
mod hooks {
    use super::*;

    #[detour(SET_DATE_TEXT)]
    unsafe extern "stdcall" fn ztui_main_set_date_text() {
        unsafe { SET_DATE_TEXT_DETOUR.call() };
        if let Some(date) = crate::globals::globals().ztgamemgr().date() {
            crate::ui::vanilla_main::set_date_value(date);
        }
    }
}

pub fn init() {
    match unsafe { hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized ZTUI::main::setDateText detour"),
        Err(err) => error!("egui overlay: failed to initialize date display detour: {err}"),
    }
}

static DATE_DISPLAY: OnceLock<Mutex<DateDisplay>> = OnceLock::new();

struct DateDisplay {
    text: String,
    date: Option<UtcDateTime>,
}

pub fn set_value(date: UtcDateTime) {
    if let Ok(mut display) = DATE_DISPLAY.get_or_init(|| Mutex::new(default_display())).lock() {
        display.text = crate::ztgamemgr::format_game_date(date);
        display.date = Some(date);
    }
}

pub fn current() -> String {
    DATE_DISPLAY
        .get_or_init(|| Mutex::new(default_display()))
        .lock()
        .map(|display| display.text.clone())
        .unwrap_or_else(|_| default_text())
}

pub fn tooltip_text() -> Option<String> {
    let display = DATE_DISPLAY.get_or_init(|| Mutex::new(default_display())).lock().ok()?;
    let date = display.date.or_else(|| crate::globals::globals().ztgamemgr().date())?;
    let help_id = if super::tooltip::long_tooltips_enabled() { 31_030 } else { 1_030 };
    let prefix = crate::string_registry::load_string_by_id(help_id).unwrap_or_default();
    Some(format_date_help_text(&prefix, date))
}

fn format_date_help_text(template: &str, date: UtcDateTime) -> String {
    let formatted_date = format_tooltip_date(date);
    if template.contains("%s") {
        template.replace("%s", &formatted_date)
    } else {
        format!("{}{}", template, formatted_date)
    }
}

fn format_tooltip_date(date: UtcDateTime) -> String {
    format!("{}. {} {}", weekday_abbreviation(date.weekday()), month_abbreviation(date.month()), date.day())
}

fn weekday_abbreviation(weekday: Weekday) -> &'static str {
    match weekday {
        Weekday::Monday => "Mon",
        Weekday::Tuesday => "Tue",
        Weekday::Wednesday => "Wed",
        Weekday::Thursday => "Thu",
        Weekday::Friday => "Fri",
        Weekday::Saturday => "Sat",
        Weekday::Sunday => "Sun",
    }
}

fn month_abbreviation(month: Month) -> &'static str {
    match month {
        Month::January => "Jan",
        Month::February => "Feb",
        Month::March => "Mar",
        Month::April => "Apr",
        Month::May => "May",
        Month::June => "Jun",
        Month::July => "Jul",
        Month::August => "Aug",
        Month::September => "Sep",
        Month::October => "Oct",
        Month::November => "Nov",
        Month::December => "Dec",
    }
}

fn default_display() -> DateDisplay {
    DateDisplay {
        text: default_text(),
        date: None,
    }
}

fn default_text() -> String {
    "Jan 1, Year 1".to_string()
}
