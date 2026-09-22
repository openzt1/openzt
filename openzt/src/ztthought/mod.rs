//! Structs and methods for the vanilla `ZTThoughtMgr`/`ZTThought` classes, which track the "thought
//! bubble" messages guests/animals display over their heads (e.g. "caught prey", template string id
//! `0x280a`) - a simple, small `BFMgr`-derived class that vanilla implements as a single intrusive,
//! sentinel-terminated linked list of `ZTThought` records.

pub mod thought;
pub mod mgr;

pub use thought::*;
pub use mgr::*;

/// Registers this module's live detours.
pub fn init() {
    mgr::init();
}
