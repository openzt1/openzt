use tracing::info;

use crate::{command_console::CommandError, lua_fn, util::get_from_memory};

const GLOBAL_ZTGAMEMGR_ADDRESS: u32 = 0x00638048;

/// ZTGameMgr struct
#[derive(Debug)]
#[repr(C)]
struct ZTGameMgr {
    pad1: [u8; 0x0C],
    cash: f32,                     // 0x0C
    pad2: [u8; 0x30 - 0x10],       // 0x0C
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
    pad9: [u8; 0x1160 - 0x56],     // 0x54
    zoo_admission_cost: f32,       // 0x1160
    pad10: [u8; 0x1194 - 0x1164],  // 0x1160
    date: Systemtime,              // 0x1194
    pad11: [u8; 0x1400],           // 0x1194
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

/// SYSTEMTIME struct from Windows API
/// TODO: Replace this with the actual SYSTEMTIME struct from the Windows API
#[derive(Debug, Clone)]
#[repr(C)]
struct Systemtime {
    w_year: u16,
    w_month: u16,
    w_day_of_week: u16,
    w_day: u16,
    w_hour: u16,
    w_minute: u16,
    w_second: u16,
    w_milliseconds: u16,
}

impl ZTGameMgr {
    /// enables or disables dev mode
    fn enable_dev_mode(enable: bool) {
        let enable_dev_mode_address = 0x63858A;
        unsafe {
            *(enable_dev_mode_address as *mut bool) = enable;
        }
    }

    /// returns the instance of the ZTGameMgr struct
    fn instance() -> Option<&'static mut ZTGameMgr> {
        unsafe {
            // get the pointer to the ZTGameMgr instance
            let ptr = get_from_memory::<*mut ZTGameMgr>(GLOBAL_ZTGAMEMGR_ADDRESS);

            // is pointer null
            if !ptr.is_null() {
                Some(&mut *ptr)
            } else {
                // pointer is null
                None
            }
        }
    }
}

/// a command that prints the SYSTEMTIME struct in memory in a human-readable format
/// usage: `get_date`
pub fn command_get_date_str(_args: Vec<&str>) -> Result<String, CommandError> {
    let ztgamemgr = ZTGameMgr::instance().ok_or("Failed to get ZTGameMgr instance")?;
    let date = ztgamemgr.date.clone();
    info!("Date: {:#?}", date);

    Ok(format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        date.w_year, date.w_month, date.w_day, date.w_hour, date.w_minute, date.w_second
    ))
}

/// a command that adds cash to the player's account
/// usage: `add_cash <amount>`
pub fn command_add_cash(args: Vec<&str>) -> Result<String, CommandError> {
    let ztgamemgr = ZTGameMgr::instance().ok_or("Failed to get ZTGameMgr instance")?;
    ztgamemgr.cash += args[0].parse::<f32>()?;
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
    let ztgamemgr = ZTGameMgr::instance().ok_or("Failed to get ZTGameMgr instance")?;
    Ok(format!("\nBudget: {}\nAnimals: {}\nSpecies: {}\nTired Guests: {}\nHungry Guests: {}\nThirsty Guests: {}\nGuests Need Restroom: {}\nNum Guests: {}\nZoo Admission Cost: ${}", ztgamemgr.cash, ztgamemgr.num_animals, ztgamemgr.num_species, ztgamemgr.num_tired_guests, ztgamemgr.num_hungry_guests, ztgamemgr.num_thirst_guests, ztgamemgr.num_guests_restroom_need, ztgamemgr.num_guests, ztgamemgr.zoo_admission_cost))
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
