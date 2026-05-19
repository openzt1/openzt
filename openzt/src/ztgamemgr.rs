use nt_time::{
    time::{Date, Month, Time, UtcDateTime},
};
use tracing::info;

use crate::{command_console::CommandError, globals::globals, lua_fn};

/// ZTGameMgr struct TODO: These a definitely u32 not u16
#[derive(Debug)]
#[repr(C)]
pub struct ZTGameMgr {
    pad1: [u8; 0x0C],
    cash: f32,                     // 0x0C
    pad2: [u8; 0x2C - 0x10],       // 0x0C
    zoo_rating: u32,
    num_animals: u16,              // 0x30
    pad3: [u8; 0x38 - 0x32],       // 0x30
    num_species: u16,              // 0x38
    pad4: [u8; 0x3C - 0x3A],       // 0x38
    num_tired_guests: u16,         // 0x3C
    pad5: [u8; 0x40 - 0x3E],       // 0x3C
    num_hungry_guests: u16,        // 0x40
    pad6: [u8; 0x44 - 0x42],       // 0x40
    num_thirst_guests: u16,        // 0x44
    pad7: [u8; 0x48 - 0x46],       // 0x44
    num_guests_restroom_need: u16, // 0x48
    pad8: [u8; 0x54 - 0x4A],       // 0x48
    num_guests: u16,               // 0x54
    pad9: [u8; 0x6c - 0x56],     // 0x54
    animal_rating: u32,          // 0x6c
    guest_rating: u32,           // 0x70
    pad10: [u8; 0x1160 - 0x74],     // 0x70
    zoo_admission_cost: f32,       // 0x1160
    pad11: [u8; 0x1194 - 0x1164],  // 0x1160
    date: SystemTimeFields,        // 0x1194 SYSTEMTIME
    pad12: [u8; 0x1400],           // 0x11a4
                                   // TODO: Below
                                   // admissions_income_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x254),
                                   // concessions_benefit_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x29c),
                                   // recycling_benefit_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x340),
                                   // // net_income maybe?: get_from_memory::<i32>(zt_game_mgr_prt + 0x404),
                                   // income_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x404),
                                   // income_expense_totals_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x44c),
                                   // zoo_rating_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x464),
                                   // unknown_array: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x4c4),
                                   // construction_cost_by_month: get_from_memory::<[f32; 12]>(zt_game_mgr_prt + 0x824),
}

impl ZTGameMgr {
    pub fn cash(&self) -> f32 {
        self.cash
    }

    pub fn zoo_rating(&self) -> u32 {
        self.zoo_rating
    }

    pub fn animal_rating_percent(&self) -> u32 {
        ((self.animal_rating + 100) * 100) / 200
    }

    pub fn guest_rating_percent(&self) -> u32 {
        ((self.guest_rating + 100) * 100) / 200
    }

    pub fn date(&self) -> Option<UtcDateTime> {
        self.date.to_utc_date_time()
    }

    /// enables or disables dev mode
    fn enable_dev_mode(enable: bool) {
        let enable_dev_mode_address = 0x63858A;
        unsafe {
            *(enable_dev_mode_address as *mut bool) = enable;
        }
    }
}

/// a command that prints the date in memory in a human-readable format
/// usage: `get_date`
pub fn command_get_date_str(_args: Vec<&str>) -> Result<String, CommandError> {
    let ztgamemgr = globals().ztgamemgr();
    let date = ztgamemgr.date();
    info!("Date: {:#?}", date);

    Ok(date.map(format_date_time).unwrap_or_else(|| "invalid SYSTEMTIME".to_string()))
}

/// a command that adds cash to the player's account
/// usage: `add_cash <amount>`
pub fn command_add_cash(args: Vec<&str>) -> Result<String, CommandError> {
    let ptr = globals().ztgamemgr_ptr();
    unsafe {
        (*ptr).cash += args[0].parse::<f32>()?;
    }
    Ok(format!("Added ${}", args[0]))
}

/// a command that enables or disables dev mode
/// usage: `enable_dev_mode <true/false>`
pub fn command_enable_dev_mode(args: Vec<&str>) -> Result<String, CommandError> {
    let enable = args[0].parse()?;
    ZTGameMgr::enable_dev_mode(enable);
    Ok(format!("Dev mode enabled: {}", enable))
}

/// a command that prints various stats about the zoo
/// usage: `zoostats`
pub fn command_zoostats(_args: Vec<&str>) -> Result<String, CommandError> {
    let ztgamemgr = globals().ztgamemgr();
    Ok(format!("\nBudget: {}\nAnimals: {}\nSpecies: {}\nTired Guests: {}\nHungry Guests: {}\nThirsty Guests: {}\nGuests Need Restroom: {}\nNum Guests: {}\nZoo Admission Cost: ${}\nZoo Rating: {}\nAnimal Rating: {}\nGuest Rating: {}", ztgamemgr.cash, ztgamemgr.num_animals, ztgamemgr.num_species, ztgamemgr.num_tired_guests, ztgamemgr.num_hungry_guests, ztgamemgr.num_thirst_guests, ztgamemgr.num_guests_restroom_need, ztgamemgr.num_guests, ztgamemgr.zoo_admission_cost, ztgamemgr.zoo_rating, ((ztgamemgr.animal_rating+100)*100)/200, ((ztgamemgr.guest_rating+100)*100)/200))
}

/// registers the Lua functions
pub fn init() {
    // get_date() - no args
    lua_fn!("get_date", "Returns current in-game date/time", "get_date()", || {
        match command_get_date_str(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // add_cash(amount) - single f32 arg
    lua_fn!("add_cash", "Adds cash to player's budget", "add_cash(amount)", |amount: f32| {
        let amount_str = amount.to_string();
        match command_add_cash(vec![&amount_str]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });

    // enable_dev_mode(enabled) - bool arg
    lua_fn!(
        "enable_dev_mode",
        "Enables/disables developer mode",
        "enable_dev_mode(true/false)",
        |enabled: bool| {
            let enabled_str = enabled.to_string();
            match command_enable_dev_mode(vec![&enabled_str]) {
                Ok(result) => Ok((Some(result), None::<String>)),
                Err(e) => Ok((None::<String>, Some(e.to_string()))),
            }
        }
    );

    // zoostats() - no args
    lua_fn!("zoostats", "Returns zoo statistics", "zoostats()", || {
        match command_zoostats(vec![]) {
            Ok(result) => Ok((Some(result), None::<String>)),
            Err(e) => Ok((None::<String>, Some(e.to_string()))),
        }
    });
}

pub fn format_game_date(date_time: UtcDateTime) -> String {
    format!(
        "{}, Year {}",
        month_abbreviation(date_time.month()),
        game_year(date_time.year())
    )
}

fn format_date_time(date_time: UtcDateTime) -> String {
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date_time.year(),
        date_time.month() as u8,
        date_time.day(),
        date_time.hour(),
        date_time.minute(),
        date_time.second()
    )
}

#[derive(Clone, Copy, Debug)]
#[repr(C)]
struct SystemTimeFields {
    year: u16,
    month: u16,
    day_of_week: u16,
    day: u16,
    hour: u16,
    minute: u16,
    second: u16,
    milliseconds: u16,
}

impl SystemTimeFields {
    fn to_utc_date_time(self) -> Option<UtcDateTime> {
        let month = Month::try_from(self.month as u8).ok()?;
        let date = Date::from_calendar_date(self.year as i32, month, self.day as u8).ok()?;
        let time = Time::from_hms_milli(self.hour as u8, self.minute as u8, self.second as u8, self.milliseconds).ok()?;
        Some(UtcDateTime::new(date, time))
    }
}

fn game_year(year: i32) -> i32 {
    year - 2000
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

#[cfg(test)]
mod tests {
    use nt_time::time::{Date, Month, Time, UtcDateTime};

    use super::{format_date_time, format_game_date, SystemTimeFields};

    fn date_time() -> UtcDateTime {
        UtcDateTime::new(
            Date::from_calendar_date(2001, Month::January, 1).unwrap(),
            Time::from_hms(0, 0, 0).unwrap(),
        )
    }

    #[test]
    fn decodes_system_time() {
        let system_time = SystemTimeFields {
            year: 2001,
            month: 1,
            day_of_week: 1,
            day: 1,
            hour: 0,
            minute: 0,
            second: 0,
            milliseconds: 0,
        };
        assert_eq!(system_time.to_utc_date_time(), Some(date_time()));
    }

    #[test]
    fn formats_game_date() {
        assert_eq!(format_game_date(date_time()), "Jan, Year 1");
    }

    #[test]
    fn formats_console_date_time() {
        assert_eq!(format_date_time(date_time()), "2001-01-01 00:00:00");
    }
}
