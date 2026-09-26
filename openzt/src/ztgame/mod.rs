//! `ztgame` module - vanilla `ZTGameMgr` and `MenuMusicHandler` reimplementation.

pub mod menu_music_handler;
pub mod mgr;

pub use mgr::*;

/// Registers live detours and commands for ZTGameMgr and MenuMusicHandler.
pub fn init() {
    menu_music_handler::init();
    mgr::init();
}
