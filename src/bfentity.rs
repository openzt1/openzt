use std::ops::Deref;

use getset::{Getters, Setters};
use tracing::info;
use num_enum::FromPrimitive;
use std::fmt;

use crate::{bfentitytype, console::{add_to_command_register, CommandError}, debug_dll::{get_from_memory, get_string_from_memory, map_from_memory}, ztui::get_selected_entity_type_address, ztworldmgr};
use crate::expansions::is_member;

// --------------- Helper Structs and Enums --------------- //

struct Position {
    x: i32,
    y: i32,
}

// --------------- BFEntity, Implementation, and Related Functions --------------- //

#[derive(Debug, Getters, Setters)]
#[repr(C)]
struct BFEntity {
    vtable: u32, // 0x000
    uvar1: u32, // 0x004
    uvar2: u32, // 0x008
    uvar3: u32, // 0x00C
    bvar4: bool, // 0x010
    bvar5: bool, // 0x011
    bvar6: bool, // 0x012
    bvar7: bool, // 0x013
    uvar8: u32, // 0x014
    uvar9: u32, // 0x018
    uvar10: u32, // 0x01C
    uvar11: u32, // 0x020 // array that points to unknown data
    uvar12: u32, // 0x024 // uvar11 buffer
    uvar13: u32, // 0x028 // uvar11 array end
    pad00: [u8; 0x03C - 0x02C], // ----------------------- padding: 20 bytes
    uvar14: u32, // 0x03C
    uvar15: u32, // 0x040
    uvar16: u32, // 0x044
    uvar17: u32, // 0x048 // array that points to unknown data
    uvar18: u32, // 0x04C // uvar17 buffer
    uvar19: u32, // 0x050 // uvar17 array end
    uvar20: u32, // 0x054 // array that points to unknown data
    uvar21: u32, // 0x058 // uvar20 buffer
    uvar22: u32, // 0x05C // uvar20 array end
    uvar23: u32, // 0x060
    uvar_: u32, // 0x074 <--- ZTScenarioTimer ptr
    pad01: [u8; 0x0B4 - 0x060], // ----------------------- padding: 76 bytes
    position: Position, // 0x0B4 and 0x0B8
    rotation: i32, // 0x0C0 <--- current rotation relative to the entity
    pad002: [u8; 0x0D5 - 0x0C4], // ----------------------- padding: 17 bytes
    stop_animation: bool, // 0x0D5
    state_changed: bool, // 0x0D6 <--- TODO: confirm this
    is_paused: bool, // 0x0D7
    pad003: [u8; 0x108 - 0x0D8], // ----------------------- padding: 41 bytes
    name: String, // 0x108 <--- str ptr start
    svar01: u32, // 0x10C <--- str ptr buffer
    svar02: u32, // 0x110 <--- str ptr end
    camera_position: Position, // 0x114 and 0x118
    pad04: [u8; 0x128 - 0x11C], // ----------------------- padding: 12 bytes
    entity_type: u32, // 0x128
    pad04: [u8; 0x13F - 0x12C], // ----------------------- padding: 20 bytes
    is_visible: bool, // 0x13F
    pad05: [u8; 0x143 - 0x140], // ----------------------- padding: 4 bytes
    is_grabbed: bool, // 0x143
    is_hovered: bool, // 0x144
    is_selected: bool, // 0x145
}



// --------------- ZTBuildingType, Implementation, and Related Functions --------------- //

#[derive(Debug, Getters, Setters)]
#[repr(C)]
struct ZTBuilding {
    bfentity: BFEntity, // 0x145 - 0x0 = 0x145  = 325 bytes
    pad00: [u8; 0x1A0 - 0x146], // ----------------------- padding: 90 bytes
    total_income: i32, // 0x1A0
    pad01: [u8; 0x1AC - 0x1A4], // ----------------------- padding: 8 bytes
    item_sale_price: i32, // 0x1AC
    pad02: [u8; 0x1B0 - 0x1A8], // ----------------------- padding: 4 bytes
    total_upkeep: i32, // 0x1B0
}

impl ZTBuilding {
    fn get_avg_income(&self) -> i32 {
        self.total_income / 12
    }
}