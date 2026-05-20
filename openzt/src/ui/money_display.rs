use std::sync::{Mutex, OnceLock};

use openzt_detour::generated::ztui_main::SET_MONEY_TEXT;
use openzt_detour_macro::detour_mod;
use tracing::{error, info};

#[detour_mod]
mod hooks {
    use super::*;

    #[detour(SET_MONEY_TEXT)]
    unsafe extern "stdcall" fn ztui_main_set_money_text() {
        unsafe { SET_MONEY_TEXT_DETOUR.call() };
        crate::ui::vanilla_main::set_money_value(crate::globals::globals().ztgamemgr().cash());
    }
}

pub fn init() {
    match unsafe { hooks::init_detours() } {
        Ok(()) => info!("egui overlay: initialized ZTUI::main::setMoneyText detour"),
        Err(err) => error!("egui overlay: failed to initialize money display detour: {err}"),
    }
}

#[derive(Clone)]
pub struct MoneyDisplay {
    pub text: String,
    pub zero: bool,
    pub negative: bool,
}

impl Default for MoneyDisplay {
    fn default() -> Self {
        Self {
            text: "$50,000".to_string(),
            zero: false,
            negative: false,
        }
    }
}

static MONEY_DISPLAY: OnceLock<Mutex<MoneyDisplay>> = OnceLock::new();

pub fn set_value(value: f32) {
    if let Ok(mut display) = MONEY_DISPLAY.get_or_init(|| Mutex::new(MoneyDisplay::default())).lock() {
        display.text = format_money(value);
        display.zero = value == 0.0;
        display.negative = value < 0.0;
    }
}

pub fn current() -> MoneyDisplay {
    MONEY_DISPLAY
        .get_or_init(|| Mutex::new(MoneyDisplay::default()))
        .lock()
        .map(|display| display.clone())
        .unwrap_or_default()
}

fn format_money(value: f32) -> String {
    let rounded = value.round() as i64;
    let negative = rounded < 0;
    let digits = rounded.abs().to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);

    for (index, ch) in digits.chars().rev().enumerate() {
        if index > 0 && index % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }

    let grouped: String = grouped.chars().rev().collect();
    if negative {
        format!("-${grouped}")
    } else {
        format!("${grouped}")
    }
}

#[cfg(test)]
mod tests {
    use super::format_money;

    #[test]
    fn formats_money_with_grouping() {
        assert_eq!(format_money(50000.0), "$50,000");
        assert_eq!(format_money(-1234.0), "-$1,234");
    }
}
