use openzt_detour_macro::detour_mod;
use tracing::info;

#[detour_mod]
mod save_detours {
    use openzt_detour::generated::{
        bfaimgr::SAVE as BFAIMGR_SAVE, bfbscall::SAVE as BFSCALL_SAVE, bfentity::SAVE as BFENTITY_SAVE, bfevent::SAVE as BFEVENT_SAVE,
        bfeventinfo::SAVE as BFEVENTINFO_SAVE, bfeventmgr::SAVE as BFEVENTMGR_SAVE, bfgamemgr::SAVE as BFGAMEMGR_SAVE, bfmap::SAVE as BFMAP_SAVE,
        bfoverlay::SAVE as BFOVERLAY_SAVE, bftile::SAVE as BFTILE_SAVE, bfunit::SAVE as BFUNIT_SAVE, bfworldmgr::SAVE as BFWORLDMGR_SAVE,
        standalone::WRITE_BYTES_TO_FILE, zoostatus::SAVE as ZOOSTATUS_SAVE, ztambient::SAVE as ZTAMBIENT_SAVE, ztanimal::SAVE as ZTANIMAL_SAVE,
        ztawardmgr::SAVE as ZTAWARDMGR_SAVE, ztbuilding::SAVE as ZTBUILDING_SAVE, ztcheat::SAVE as ZTCHEAT_SAVE, ztfence::SAVE as ZTFENCE_SAVE,
        ztgamemgr::SAVE as ZTGAMEMGR_SAVE, ztguest::SAVE as ZTGUEST_SAVE, ztguide::SAVE as ZTGUIDE_SAVE, zthabitat::SAVE as ZTHABITAT_SAVE,
        zthabitatmgr::SAVE as ZTHABITATMGR_SAVE, zthelicopter::SAVE as ZTHELICOPTER_SAVE, ztkeeper::SAVE as ZTKEEPER_SAVE, ztmaint::SAVE as ZTMAINT_SAVE,
        ztmapview::SAVE_STATE as ZTMAPVIEW_SAVE_STATE, ztpath::SAVE as ZTPATH_SAVE, ztresearchmgr::SAVE as ZTRESEARCHMGR_SAVE, ztrubble::SAVE as ZTRUBBLE_SAVE,
        ztscenariotimer::SAVE as ZTSCENARIOTIMER_SAVE, ztscenery::SAVE as ZTSCENERY_SAVE, ztshow::SAVE as ZTSHOW_SAVE, ztshowinfo::SAVE as ZTSHOWINFO_SAVE,
        ztshowmgr::SAVE as ZTSHOWMGR_SAVE, ztshowscript::SAVE as ZTSHOWSCRIPT_SAVE, ztshowscriptitem::SAVE as ZTSHOWSCRIPTITEM_SAVE,
        ztshowscriptmgr::SAVE as ZTSHOWSCRIPTMGR_SAVE, ztshowscriptstate::SAVE as ZTSHOWSCRIPTSTATE_SAVE, ztshowstate::SAVE as ZTSHOWSTATE_SAVE,
        ztstaff::SAVE as ZTSTAFF_SAVE, zttankexhibit::SAVE as ZTTANKEXHIBIT_SAVE, zttankfilter::SAVE as ZTTANKFILTER_SAVE, zttankwall::SAVE as ZTTANKWALL_SAVE,
        ztthoughtmgr::SAVE as ZTTHOUGHTMGR_SAVE, ztunit::SAVE as ZTUNIT_SAVE, ztworldmgr::SAVE as ZTWORLDMGR_SAVE,
    };
    use std::ffi::c_void;
    use tracing::info;

    // Standard SAVE detours (thiscall, bool(*const u32, *const u32) -> bool)
    #[detour(BFAIMGR_SAVE)]
    unsafe extern "thiscall" fn hook_bfaimgr_save(this: *const u32, file: *const u32) -> bool {
        info!("BFAIMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFAIMGR_SAVE_DETOUR.call(this, file) };
        info!("BFAIMGR::SAVE returned: {}", result);
        result
    }

    #[detour(BFSCALL_SAVE)]
    unsafe extern "thiscall" fn hook_bfbscall_save(this: *const u32, file: *const u32) -> bool {
        info!("BFBSCALL::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFSCALL_SAVE_DETOUR.call(this, file) };
        info!("BFBSCALL::SAVE returned: {}", result);
        result
    }

    #[detour(BFENTITY_SAVE)]
    unsafe extern "thiscall" fn hook_bfentity_save(this: *const u32, file: *const u32) -> bool {
        info!("BFENTITY::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFENTITY_SAVE_DETOUR.call(this, file) };
        info!("BFENTITY::SAVE returned: {}", result);
        result
    }

    #[detour(BFEVENT_SAVE)]
    unsafe extern "thiscall" fn hook_bfevent_save(this: *const u32, file: *const u32) -> bool {
        info!("BFEVENT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFEVENT_SAVE_DETOUR.call(this, file) };
        info!("BFEVENT::SAVE returned: {}", result);
        result
    }

    #[detour(BFEVENTINFO_SAVE)]
    unsafe extern "thiscall" fn hook_bfeventinfo_save(this: *const u32, file: *const u32) -> bool {
        info!("BFEVENTINFO::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFEVENTINFO_SAVE_DETOUR.call(this, file) };
        info!("BFEVENTINFO::SAVE returned: {}", result);
        result
    }

    #[detour(BFEVENTMGR_SAVE)]
    unsafe extern "thiscall" fn hook_bfeventmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("BFEVENTMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFEVENTMGR_SAVE_DETOUR.call(this, file) };
        info!("BFEVENTMGR::SAVE returned: {}", result);
        result
    }

    #[detour(BFGAMEMGR_SAVE)]
    unsafe extern "thiscall" fn hook_bfgamemgr_save(this: *const u32, file: *const u32) -> bool {
        info!("BFGAMEMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFGAMEMGR_SAVE_DETOUR.call(this, file) };
        info!("BFGAMEMGR::SAVE returned: {}", result);
        result
    }

    #[detour(BFMAP_SAVE)]
    unsafe extern "thiscall" fn hook_bfmap_save(this: *const u32, file: *const u32) -> bool {
        info!("BFMAP::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFMAP_SAVE_DETOUR.call(this, file) };
        info!("BFMAP::SAVE returned: {}", result);
        result
    }

    #[detour(BFOVERLAY_SAVE)]
    unsafe extern "thiscall" fn hook_bfoverlay_save(this: *const u32, file: *const u32) -> bool {
        info!("BFOVERLAY::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFOVERLAY_SAVE_DETOUR.call(this, file) };
        info!("BFOVERLAY::SAVE returned: {}", result);
        result
    }

    #[detour(BFTILE_SAVE)]
    unsafe extern "thiscall" fn hook_bftile_save(this: *const u32, file: *const u32) -> bool {
        info!("BFTILE::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFTILE_SAVE_DETOUR.call(this, file) };
        info!("BFTILE::SAVE returned: {}", result);
        result
    }

    #[detour(BFUNIT_SAVE)]
    unsafe extern "thiscall" fn hook_bfunit_save(this: *const u32, file: *const u32) -> bool {
        info!("BFUNIT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFUNIT_SAVE_DETOUR.call(this, file) };
        info!("BFUNIT::SAVE returned: {}", result);
        result
    }

    #[detour(BFWORLDMGR_SAVE)]
    unsafe extern "thiscall" fn hook_bfworldmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("BFWORLDMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { BFWORLDMGR_SAVE_DETOUR.call(this, file) };
        info!("BFWORLDMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZTAMBIENT_SAVE)]
    unsafe extern "thiscall" fn hook_ztambient_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTAMBIENT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTAMBIENT_SAVE_DETOUR.call(this, file) };
        info!("ZTAMBIENT::SAVE returned: {}", result);
        result
    }

    #[detour(ZTANIMAL_SAVE)]
    unsafe extern "thiscall" fn hook_ztanimal_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTANIMAL::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTANIMAL_SAVE_DETOUR.call(this, file) };
        info!("ZTANIMAL::SAVE returned: {}", result);
        result
    }

    #[detour(ZTAWARDMGR_SAVE)]
    unsafe extern "thiscall" fn hook_ztawardmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTAWARDMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTAWARDMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTAWARDMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZTBUILDING_SAVE)]
    unsafe extern "thiscall" fn hook_ztbuilding_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTBUILDING::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTBUILDING_SAVE_DETOUR.call(this, file) };
        info!("ZTBUILDING::SAVE returned: {}", result);
        result
    }

    #[detour(ZTFENCE_SAVE)]
    unsafe extern "thiscall" fn hook_ztfence_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTFENCE::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTFENCE_SAVE_DETOUR.call(this, file) };
        info!("ZTFENCE::SAVE returned: {}", result);
        result
    }

    #[detour(ZTGAMEMGR_SAVE)]
    unsafe extern "thiscall" fn hook_ztgamemgr_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTGAMEMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTGAMEMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTGAMEMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZTGUEST_SAVE)]
    unsafe extern "thiscall" fn hook_ztguest_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTGUEST::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTGUEST_SAVE_DETOUR.call(this, file) };
        info!("ZTGUEST::SAVE returned: {}", result);
        result
    }

    #[detour(ZTGUIDE_SAVE)]
    unsafe extern "thiscall" fn hook_ztguide_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTGUIDE::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTGUIDE_SAVE_DETOUR.call(this, file) };
        info!("ZTGUIDE::SAVE returned: {}", result);
        result
    }

    #[detour(ZTHABITAT_SAVE)]
    unsafe extern "thiscall" fn hook_zthabitat_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTHABITAT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTHABITAT_SAVE_DETOUR.call(this, file) };
        info!("ZTHABITAT::SAVE returned: {}", result);
        result
    }

    #[detour(ZTHABITATMGR_SAVE)]
    unsafe extern "thiscall" fn hook_zthabitatmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTHABITATMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTHABITATMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTHABITATMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZTHELICOPTER_SAVE)]
    unsafe extern "thiscall" fn hook_zthelicopter_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTHELICOPTER::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTHELICOPTER_SAVE_DETOUR.call(this, file) };
        info!("ZTHELICOPTER::SAVE returned: {}", result);
        result
    }

    #[detour(ZTKEEPER_SAVE)]
    unsafe extern "thiscall" fn hook_ztkeeper_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTKEEPER::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTKEEPER_SAVE_DETOUR.call(this, file) };
        info!("ZTKEEPER::SAVE returned: {}", result);
        result
    }

    #[detour(ZTPATH_SAVE)]
    unsafe extern "thiscall" fn hook_ztpath_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTPATH::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTPATH_SAVE_DETOUR.call(this, file) };
        info!("ZTPATH::SAVE returned: {}", result);
        result
    }

    #[detour(ZTRESEARCHMGR_SAVE)]
    unsafe extern "thiscall" fn hook_ztresearchmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTRESEARCHMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTRESEARCHMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTRESEARCHMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZTRUBBLE_SAVE)]
    unsafe extern "thiscall" fn hook_ztrubble_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTRUBBLE::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTRUBBLE_SAVE_DETOUR.call(this, file) };
        info!("ZTRUBBLE::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSCENARIOTIMER_SAVE)]
    unsafe extern "thiscall" fn hook_ztscenariotimer_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSCENARIOTIMER::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSCENARIOTIMER_SAVE_DETOUR.call(this, file) };
        info!("ZTSCENARIOTIMER::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSCENERY_SAVE)]
    unsafe extern "thiscall" fn hook_ztscenery_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSCENERY::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSCENERY_SAVE_DETOUR.call(this, file) };
        info!("ZTSCENERY::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOW_SAVE)]
    unsafe extern "thiscall" fn hook_ztshow_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSHOW::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOW_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOW::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOWINFO_SAVE)]
    unsafe extern "thiscall" fn hook_ztshowinfo_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSHOWINFO::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOWINFO_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOWINFO::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOWMGR_SAVE)]
    unsafe extern "thiscall" fn hook_ztshowmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSHOWMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOWMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOWMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOWSCRIPTITEM_SAVE)]
    unsafe extern "thiscall" fn hook_ztshowscriptitem_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSHOWSCRIPTITEM::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOWSCRIPTITEM_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOWSCRIPTITEM::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOWSCRIPTSTATE_SAVE)]
    unsafe extern "thiscall" fn hook_ztshowscriptstate_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSHOWSCRIPTSTATE::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOWSCRIPTSTATE_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOWSCRIPTSTATE::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOWSTATE_SAVE)]
    unsafe extern "thiscall" fn hook_ztshowstate_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSHOWSTATE::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOWSTATE_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOWSTATE::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSTAFF_SAVE)]
    unsafe extern "thiscall" fn hook_ztstaff_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTSTAFF::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSTAFF_SAVE_DETOUR.call(this, file) };
        info!("ZTSTAFF::SAVE returned: {}", result);
        result
    }

    #[detour(ZTTANKEXHIBIT_SAVE)]
    unsafe extern "thiscall" fn hook_zttankexhibit_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTTANKEXHIBIT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTTANKEXHIBIT_SAVE_DETOUR.call(this, file) };
        info!("ZTTANKEXHIBIT::SAVE returned: {}", result);
        result
    }

    #[detour(ZTTANKFILTER_SAVE)]
    unsafe extern "thiscall" fn hook_zttankfilter_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTTANKFILTER::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTTANKFILTER_SAVE_DETOUR.call(this, file) };
        info!("ZTTANKFILTER::SAVE returned: {}", result);
        result
    }

    #[detour(ZTTANKWALL_SAVE)]
    unsafe extern "thiscall" fn hook_zttankwall_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTTANKWALL::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTTANKWALL_SAVE_DETOUR.call(this, file) };
        info!("ZTTANKWALL::SAVE returned: {}", result);
        result
    }

    #[detour(ZTTHOUGHTMGR_SAVE)]
    unsafe extern "thiscall" fn hook_ztthoughtmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTTHOUGHTMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTTHOUGHTMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTTHOUGHTMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZTUNIT_SAVE)]
    unsafe extern "thiscall" fn hook_ztunit_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTUNIT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTUNIT_SAVE_DETOUR.call(this, file) };
        info!("ZTUNIT::SAVE returned: {}", result);
        result
    }

    #[detour(ZTWORLDMGR_SAVE)]
    unsafe extern "thiscall" fn hook_ztworldmgr_save(this: *const u32, file: *const u32) -> bool {
        info!("ZTWORLDMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTWORLDMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTWORLDMGR::SAVE returned: {}", result);
        result
    }

    #[detour(ZOOSTATUS_SAVE)]
    unsafe extern "thiscall" fn hook_zoostatus_save(this: *const u32, file: *const u32) -> bool {
        info!("ZOOSTATUS::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZOOSTATUS_SAVE_DETOUR.call(this, file) };
        info!("ZOOSTATUS::SAVE returned: {}", result);
        result
    }

    // SAVE_STATE detour
    #[detour(ZTMAPVIEW_SAVE_STATE)]
    unsafe extern "thiscall" fn hook_ztmapview_save_state(this: *const u32, file: *const u32) -> bool {
        info!("ZTMAPVIEW::SAVE_STATE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTMAPVIEW_SAVE_STATE_DETOUR.call(this, file) };
        info!("ZTMAPVIEW::SAVE_STATE returned: {}", result);
        result
    }

    // Non-standard signature SAVE detours
    #[detour(ZTCHEAT_SAVE)]
    unsafe extern "cdecl" fn hook_ztcheat_save(file: *const u32) -> bool {
        info!("ZTCHEAT::SAVE called: file={:p}", file);
        let result = unsafe { ZTCHEAT_SAVE_DETOUR.call(file) };
        info!("ZTCHEAT::SAVE returned: {}", result);
        result
    }

    #[detour(ZTMAINT_SAVE)]
    unsafe extern "thiscall" fn hook_ztmaint_save(this: *const i32, file: *const u32) -> bool {
        info!("ZTMAINT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTMAINT_SAVE_DETOUR.call(this, file) };
        info!("ZTMAINT::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOWSCRIPT_SAVE)]
    unsafe extern "thiscall" fn hook_ztshowscript_save(this: *const c_void, file: *const i8) -> u32 {
        info!("ZTSHOWSCRIPT::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOWSCRIPT_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOWSCRIPT::SAVE returned: {}", result);
        result
    }

    #[detour(ZTSHOWSCRIPTMGR_SAVE)]
    unsafe extern "thiscall" fn hook_ztshowscriptmgr_save(this: *const c_void, file: *const i8) -> u32 {
        info!("ZTSHOWSCRIPTMGR::SAVE called: this={:p}, file={:p}", this, file);
        let result = unsafe { ZTSHOWSCRIPTMGR_SAVE_DETOUR.call(this, file) };
        info!("ZTSHOWSCRIPTMGR::SAVE returned: {}", result);
        result
    }

    // WRITE_BYTES_TO_FILE detour
    #[detour(WRITE_BYTES_TO_FILE)]
    unsafe extern "cdecl" fn hook_write_bytes_to_file(source_ptr: *const u32, size_in_bytes: u32, number_of_occurrences: u32, file_ptr: *const u32) -> u32 {
        info!(
            "WRITE_BYTES_TO_FILE called: source={:p}, size={} bytes, count={}, file={:p}",
            source_ptr, size_in_bytes, number_of_occurrences, file_ptr
        );
        let result = unsafe { WRITE_BYTES_TO_FILE_DETOUR.call(source_ptr, size_in_bytes, number_of_occurrences, file_ptr) };
        result
    }
}

pub fn init() {
    if let Err(e) = unsafe { save_detours::init_detours() } {
        info!("Error initializing save logging detours: {}", e);
    }
}
