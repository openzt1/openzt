use tracing::error;

use crate::util::{patch_nop, save_to_protected_memory};

const ZOOWALL_MAP_EDGE_CRASH_ADDRESS: u32 = 0x050b260;

const ZOOFENCE_ONE_TILE_FROM_MAP_EDGE_CRASH_ADDRESS_1: u32 = 0x4a1fc0;
const ZOOFENCE_ONE_TILE_FROM_MAP_EDGE_CRASH_ADDRESS_2: u32 = 0x4a1fe7;
// const ZOOFENCE_ONE_TILE_FROM_MAP_EDGE_CRASH_ADDRESS_1: u32 = ;

const LOWERCASE_LOOKUP_TABLE_SIGNED_CHAR_BUG: [u32; 6] = [
    0x004036b9, // FUN_004036ae (may be unused)
    0x004039e9, // makeName
    0x00404240, // BFResourceZip::load
    0x0040697f, // BFResourceZip::prepare
    0x00528d0f, // BFResourceZip::BFResourceZip
    0x00529cc7, // dirsearch
];

// Addresses where the vanilla game references its 128-entry lowercase table
// These need to be patched to point to our 256-entry table instead
const LOWERCASE_TABLE_REFERENCE_ADDRESSES: [u32; 6] = [
    0x004036be, // FUN_004036ae (may be unused)
    0x004039ee, // makeName
    0x00404245, // BFResourceZip::load
    0x00406984, // BFResourceZip::prepare
    0x00528d14, // BFResourceZip::BFResourceZip
    0x00529ccc, // dirsearch
];

pub fn init() {
    if let Err(e) = fix_zoowall_map_edge_crash() {
        error!("Failed to fix ZooWall map edge crash: {}", e);
    }
    if let Err(e) = fix_fence_one_tile_from_map_edge_crash() {
        error!("Failed to fix ZooFence one tile from map edge crash: {}", e);
    }

    if let Err(e) = fix_lowercase_lookup_table_signed_char_bug() {
        error!("Failed to fix lowercase lookup table signed char bug: {}", e);
    }

    if let Err(e) = patch_lowercase_table_references() {
        error!("Failed to patch lowercase table references: {}", e);
    }
}

fn fix_zoowall_map_edge_crash() -> anyhow::Result<()> {
    // We change a jump address to fix a bug trying access a null pointer
    save_to_protected_memory(ZOOWALL_MAP_EDGE_CRASH_ADDRESS, 0xfffffcfeu32 as i32)?;
    Ok(())
}

fn fix_fence_one_tile_from_map_edge_crash() -> anyhow::Result<()> {
    // This changes an if statement to cover the entire inner loop
    save_to_protected_memory::<u8>(ZOOFENCE_ONE_TILE_FROM_MAP_EDGE_CRASH_ADDRESS_1, 0x45)?;
    // The above change makes the second if statement redundant so we can add in a check for the null pointer
    save_to_protected_memory::<u8>(ZOOFENCE_ONE_TILE_FROM_MAP_EDGE_CRASH_ADDRESS_2, 0x85)?;
    save_to_protected_memory::<u8>(ZOOFENCE_ONE_TILE_FROM_MAP_EDGE_CRASH_ADDRESS_2 + 1, 0xC0)?;
    patch_nop(ZOOFENCE_ONE_TILE_FROM_MAP_EDGE_CRASH_ADDRESS_2 + 2)?;
    Ok(())
}

fn fix_lowercase_lookup_table_signed_char_bug() -> anyhow::Result<()> {
    for address in LOWERCASE_LOOKUP_TABLE_SIGNED_CHAR_BUG {
        // This changes a movsx to a movzx to fix signed char bug in lowercase lookup table
        save_to_protected_memory::<u8>(address, 0xB6)?;
    }
    Ok(())
}

/// Patch the vanilla game to use our 256-entry locale-aware lowercase table
///
/// The vanilla game uses a 128-entry ASCII-only lowercase table, which breaks
/// when handling filenames with non-ASCII characters from FindFirstFileA/FindNextFileA.
///
/// This function patches all references to the vanilla table to point to our
/// extended 256-entry table that properly handles the system's ANSI code page.
fn patch_lowercase_table_references() -> anyhow::Result<()> {
    let table_ptr = crate::encoding_utils::get_lowercase_table_ptr();

    for &address in &LOWERCASE_TABLE_REFERENCE_ADDRESSES {
        save_to_protected_memory::<u32>(address, table_ptr)?;
    }

    Ok(())
}

// Leaving this in incase future bugfixes require inline assembly

// use std::arch::global_asm;
//
// pub mod fence_crash_asm {
//     global_asm!(r#"
//         .global fence_crash_trmp
//       :
//         call DWORD PTR ds:0x40fa92
//         TEST EAX, EAX
//         JZ 0x01
//         JMP DWORD PTR
//         JMP DWORD PTR ds:<past original if>

//         jmp back to original if
//     "#);
// }

// the symbols `foo` and `bar` are global, no matter where
// `global_asm!` was used.
// extern "C" {
//     fn fence_crash_trmp();
// }
