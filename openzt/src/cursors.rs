#[cfg(target_os = "windows")]
mod imp {
    use std::ffi::c_void;
    use std::sync::{Mutex, OnceLock};

    use openzt_detour::generated::{
        bfuimgr::{
            DESTROY_CURSORS as BFUIMGR_DESTROY_CURSORS, HIDE_BUSY_CURSOR, INIT_CURSORS as BFUIMGR_INIT_CURSORS,
            RESET_ELEMENT_CURSOR, SET_CURSOR, SHOW_BUSY_CURSOR,
        },
        ztmapview::{DESTROY_CURSORS as ZTMAPVIEW_DESTROY_CURSORS, INIT_CURSORS as ZTMAPVIEW_INIT_CURSORS, USE_CURSOR_0},
        ztui_general::GET_MAPVIEW,
    };
    use openzt_detour_macro::detour_mod;
    use tracing::{error, info, warn};
    use windows::core::{PCSTR, PCWSTR};
    use windows::Win32::Foundation::{HINSTANCE, HWND};
    use windows::Win32::UI::WindowsAndMessaging::{
        DestroyCursor, FindWindowA, GetClassLongA, LoadCursorW, LoadImageA, SetClassLongA, SetCursor, GCL_HCURSOR, HCURSOR, IDC_ARROW, IDC_IBEAM,
        IMAGE_CURSOR,
    };

    use crate::util::{get_from_memory, save_to_memory};

    const GLOBAL_BFAPP: u32 = 0x00638148;
    const GLOBAL_BFUIMGR: u32 = 0x00638de0;
    const CURSOR_RESOURCE_OFFSET_ENABLED: u32 = 0x0063e9dc;
    const CURSOR_RESOURCE_OFFSET: u32 = 0x0063e9e0;
    const MAP_CURSOR_IDS: u32 = 0x006358ac;
    const MAP_CURSOR_HANDLES: u32 = 0x006392c4;
    const EDIT_TEXT_CURSOR_HANDLE: u32 = 0x0063ea48;
    const EDIT_TEXT_CURSOR_BORROWED_FLAG: u32 = 0x0063ea4c;

    const BFAPP_RESOURCE_DLLS_START: u32 = 0x44c;
    const BFAPP_RESOURCE_DLLS_END: u32 = 0x450;
    const BFAPP_HWND: u32 = 0x4c4;

    const BFUIMGR_ACTIVE_ELEMENT: u32 = 0x10;
    const BFUIMGR_CURRENT_CURSOR: u32 = 0x50;
    const BFUIMGR_BUSY_CURSOR: u32 = 0x54;
    const BFUIMGR_DEFAULT_CURSOR: u32 = 0x58;
    const BFUIMGR_DEFAULT_CURSOR_BORROWED_FLAG: u32 = 0x5c;

    const UIELEMENT_CURSOR: u32 = 0xa8;
    const ZTMAPVIEW_CURRENT_CURSOR_INDEX: u32 = 0x450;

    const DEFAULT_CURSOR_RESOURCE_ID: u32 = 200;
    const BUSY_CURSOR_RESOURCE_ID: u32 = 0xd3;
    const TEXT_CURSOR_RESOURCE_ID: u32 = 0xc9;
    const MAP_CURSOR_COUNT: usize = 9;

    static MANAGER: OnceLock<Mutex<CursorManager>> = OnceLock::new();

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    pub struct CursorHandle(u32);

    impl CursorHandle {
        pub fn from_raw(raw: u32) -> Option<Self> {
            (raw != 0).then_some(Self(raw))
        }

        pub fn raw(self) -> u32 {
            self.0
        }

        fn as_hcursor(self) -> HCURSOR {
            HCURSOR(self.0 as *mut c_void)
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum CursorOwnership {
        Owned,
        Borrowed,
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    struct ManagedCursor {
        handle: CursorHandle,
        ownership: CursorOwnership,
    }

    impl ManagedCursor {
        fn owned(raw: u32) -> Option<Self> {
            CursorHandle::from_raw(raw).map(|handle| Self {
                handle,
                ownership: CursorOwnership::Owned,
            })
        }

        fn borrowed(raw: u32) -> Option<Self> {
            CursorHandle::from_raw(raw).map(|handle| Self {
                handle,
                ownership: CursorOwnership::Borrowed,
            })
        }
    }

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    #[repr(u8)]
    pub enum MapCursor {
        Cursor0,
        Cursor1,
        Cursor2,
        Cursor3,
        Cursor4,
        Cursor5,
        Cursor6,
        Cursor7,
        Cursor8,
    }

    impl MapCursor {
        fn from_i32(index: i32) -> Option<Self> {
            match index {
                0 => Some(Self::Cursor0),
                1 => Some(Self::Cursor1),
                2 => Some(Self::Cursor2),
                3 => Some(Self::Cursor3),
                4 => Some(Self::Cursor4),
                5 => Some(Self::Cursor5),
                6 => Some(Self::Cursor6),
                7 => Some(Self::Cursor7),
                8 => Some(Self::Cursor8),
                _ => None,
            }
        }

        fn index(self) -> usize {
            self as usize
        }
    }

    #[derive(Default)]
    struct CursorManager {
        default_cursor: Option<ManagedCursor>,
        busy_cursor: Option<ManagedCursor>,
        text_cursor: Option<ManagedCursor>,
        map_cursors: [Option<ManagedCursor>; MAP_CURSOR_COUNT],
        current_cursor: Option<CursorHandle>,
    }

    impl CursorManager {
        fn init_bfuimgr_cursors(&mut self, bfuimgr: *const u32) {
            self.busy_cursor = load_cursor_resource(BUSY_CURSOR_RESOURCE_ID).and_then(ManagedCursor::owned);
            save_to_memory(bfuimgr as u32 + BFUIMGR_BUSY_CURSOR, raw_cursor(self.busy_cursor));

            self.default_cursor = load_cursor_resource(DEFAULT_CURSOR_RESOURCE_ID)
                .and_then(ManagedCursor::owned)
                .or_else(|| class_cursor().and_then(ManagedCursor::borrowed))
                .or_else(|| system_cursor(IDC_ARROW).and_then(ManagedCursor::borrowed));

            save_to_memory(bfuimgr as u32 + BFUIMGR_DEFAULT_CURSOR, raw_cursor(self.default_cursor));
            save_to_memory(
                bfuimgr as u32 + BFUIMGR_DEFAULT_CURSOR_BORROWED_FLAG,
                cursor_is_borrowed(self.default_cursor) as u32,
            );

            if let Some(hwnd) = main_window() {
                unsafe {
                    SetClassLongA(hwnd, GCL_HCURSOR, 0);
                }
            }

            self.set_cursor(bfuimgr, self.default_cursor.map(|cursor| cursor.handle), true);
            self.init_text_cursor();
        }

        fn init_text_cursor(&mut self) {
            self.text_cursor = load_cursor_resource(TEXT_CURSOR_RESOURCE_ID)
                .and_then(ManagedCursor::owned)
                .or_else(|| system_cursor(IDC_IBEAM).and_then(ManagedCursor::borrowed));

            save_to_memory(EDIT_TEXT_CURSOR_HANDLE, raw_cursor(self.text_cursor));
            save_to_memory(EDIT_TEXT_CURSOR_BORROWED_FLAG, cursor_is_borrowed(self.text_cursor) as u8);
        }

        fn init_map_cursors(&mut self, refresh_map_view: bool) {
            for index in 0..MAP_CURSOR_COUNT {
                let resource_id = get_from_memory::<u16>(MAP_CURSOR_IDS + (index * 2) as u32) as u32;
                self.map_cursors[index] = load_cursor_resource(resource_id).and_then(ManagedCursor::owned);
                save_to_memory(MAP_CURSOR_HANDLES + (index * 4) as u32, raw_cursor(self.map_cursors[index]));
            }

            if refresh_map_view {
                let map_view = unsafe { GET_MAPVIEW.original()() };
                if !map_view.is_null() {
                    self.use_map_cursor(map_view, -1);
                }
            }
        }

        fn set_cursor(&mut self, bfuimgr: *const u32, cursor: Option<CursorHandle>, apply_now: bool) {
            self.current_cursor = cursor;
            save_to_memory(bfuimgr as u32 + BFUIMGR_CURRENT_CURSOR, cursor.map_or(0, CursorHandle::raw));

            if apply_now {
                let visible_cursor = cursor.or_else(|| self.default_cursor.map(|cursor| cursor.handle));
                unsafe {
                    SetCursor(visible_cursor.map(CursorHandle::as_hcursor));
                }
            }
        }

        fn show_busy(&mut self, bfuimgr: *const u32) {
            self.set_cursor(bfuimgr, self.busy_cursor.map(|cursor| cursor.handle), true);
        }

        fn hide_busy(&mut self, bfuimgr: *const u32) {
            if self.current_cursor.map(CursorHandle::raw) == self.busy_cursor.map(|cursor| cursor.handle.raw()) {
                self.set_cursor(bfuimgr, None, true);
            }
        }

        fn reset_element_cursor(&mut self, bfuimgr: *const u32) {
            let active_element = get_from_memory::<u32>(bfuimgr as u32 + BFUIMGR_ACTIVE_ELEMENT);
            if active_element == 0 {
                return;
            }

            let cursor = CursorHandle::from_raw(get_from_memory::<u32>(active_element + UIELEMENT_CURSOR));
            self.set_cursor(bfuimgr, cursor, true);
        }

        fn set_element_cursor(&mut self, element: *const u32, cursor: Option<CursorHandle>) {
            let old_cursor = CursorHandle::from_raw(get_from_memory::<u32>(element as u32 + UIELEMENT_CURSOR));
            if old_cursor == cursor {
                return;
            }

            save_to_memory(element as u32 + UIELEMENT_CURSOR, cursor.map_or(0, CursorHandle::raw));
            if cursor.is_some() {
                self.set_cursor(GLOBAL_BFUIMGR as *const u32, cursor, true);
            }
        }

        fn use_map_cursor(&mut self, map_view: *const u32, mut index: i32) {
            if index == -1 {
                index = get_from_memory::<i32>(map_view as u32 + ZTMAPVIEW_CURRENT_CURSOR_INDEX);
            }

            let Some(cursor_index) = MapCursor::from_i32(index) else {
                return;
            };

            save_to_memory(map_view as u32 + ZTMAPVIEW_CURRENT_CURSOR_INDEX, index);
            let cursor = self.map_cursors[cursor_index.index()].map(|cursor| cursor.handle);
            self.set_element_cursor(map_view, cursor);
        }

        fn destroy_bfuimgr_cursors(&mut self) {
            destroy_managed(self.busy_cursor.take());
            destroy_managed(self.default_cursor.take());
            destroy_managed(self.text_cursor.take());
            self.current_cursor = None;
            save_to_memory(EDIT_TEXT_CURSOR_HANDLE, 0u32);
            save_to_memory(EDIT_TEXT_CURSOR_BORROWED_FLAG, 0u8);
        }

        fn destroy_map_cursors(&mut self) {
            for cursor in &mut self.map_cursors {
                destroy_managed(cursor.take());
            }

            for index in 0..MAP_CURSOR_COUNT {
                save_to_memory(MAP_CURSOR_HANDLES + (index * 4) as u32, 0u32);
            }
        }
    }

    #[detour_mod]
    mod cursor_hooks {
        use super::*;

        #[detour(BFUIMGR_INIT_CURSORS)]
        unsafe extern "thiscall" fn bfuimgr_init_cursors(this: *const u32) {
            with_manager(|manager| manager.init_bfuimgr_cursors(this));
        }

        #[detour(BFUIMGR_DESTROY_CURSORS)]
        unsafe extern "thiscall" fn bfuimgr_destroy_cursors(_this: *const u32) {
            with_manager(CursorManager::destroy_bfuimgr_cursors);
        }

        #[detour(SET_CURSOR)]
        unsafe extern "thiscall" fn bfuimgr_set_cursor(this: *const u32, cursor: i32, apply_now: i8) {
            with_manager(|manager| manager.set_cursor(this, CursorHandle::from_raw(cursor as u32), apply_now != 0));
        }

        #[detour(SHOW_BUSY_CURSOR)]
        unsafe extern "thiscall" fn bfuimgr_show_busy_cursor(this: *const u32) {
            with_manager(|manager| manager.show_busy(this));
        }

        #[detour(HIDE_BUSY_CURSOR)]
        unsafe extern "thiscall" fn bfuimgr_hide_busy_cursor(this: *const u32) {
            with_manager(|manager| manager.hide_busy(this));
        }

        #[detour(RESET_ELEMENT_CURSOR)]
        unsafe extern "thiscall" fn bfuimgr_reset_element_cursor(this: *const u32) {
            with_manager(|manager| manager.reset_element_cursor(this));
        }

        #[detour(ZTMAPVIEW_INIT_CURSORS)]
        unsafe extern "cdecl" fn ztmapview_init_cursors(refresh_map_view: i8) {
            with_manager(|manager| manager.init_map_cursors(refresh_map_view != 0));
        }

        #[detour(ZTMAPVIEW_DESTROY_CURSORS)]
        unsafe extern "stdcall" fn ztmapview_destroy_cursors() {
            with_manager(CursorManager::destroy_map_cursors);
        }

        #[detour(USE_CURSOR_0)]
        unsafe extern "thiscall" fn ztmapview_use_cursor(this: *const u32, index: i32) {
            with_manager(|manager| manager.use_map_cursor(this, index));
        }
    }

    pub fn init() {
        match unsafe { cursor_hooks::init_detours() } {
            Ok(()) => info!("Initialized Rust cursor management detours"),
            Err(err) => error!("Error initializing Rust cursor management detours: {err}"),
        }
    }

    pub fn set_default_cursor() {
        with_manager(|manager| {
            manager.set_cursor(
                GLOBAL_BFUIMGR as *const u32,
                manager.default_cursor.map(|cursor| cursor.handle),
                true,
            )
        });
    }

    pub fn reset_global_element_cursor() {
        with_manager(|manager| manager.reset_element_cursor(GLOBAL_BFUIMGR as *const u32));
    }

    fn with_manager(action: impl FnOnce(&mut CursorManager)) {
        let storage = MANAGER.get_or_init(|| Mutex::new(CursorManager::default()));
        match storage.lock() {
            Ok(mut manager) => action(&mut manager),
            Err(err) => error!("cursor manager lock poisoned: {err}"),
        }
    }

    fn raw_cursor(cursor: Option<ManagedCursor>) -> u32 {
        cursor.map_or(0, |cursor| cursor.handle.raw())
    }

    fn cursor_is_borrowed(cursor: Option<ManagedCursor>) -> bool {
        matches!(
            cursor,
            Some(ManagedCursor {
                ownership: CursorOwnership::Borrowed,
                ..
            })
        )
    }

    fn load_cursor_resource(resource_id: u32) -> Option<u32> {
        let id = adjusted_resource_id(resource_id) & 0xffff;
        let start = get_from_memory::<u32>(GLOBAL_BFAPP + BFAPP_RESOURCE_DLLS_START);
        let end = get_from_memory::<u32>(GLOBAL_BFAPP + BFAPP_RESOURCE_DLLS_END);
        if start == 0 || end < start {
            return None;
        }

        let mut cursor = start;
        while cursor < end {
            let instance = get_from_memory::<u32>(cursor);
            if instance != 0 {
                let loaded = unsafe {
                    LoadImageA(
                        Some(HINSTANCE(instance as *mut c_void)),
                        PCSTR(id as usize as *const u8),
                        IMAGE_CURSOR,
                        0,
                        0,
                        Default::default(),
                    )
                };
                if let Ok(handle) = loaded
                    && !handle.0.is_null()
                {
                    return Some(handle.0 as u32);
                }
            }
            cursor += 4;
        }

        None
    }

    fn adjusted_resource_id(resource_id: u32) -> u32 {
        if get_from_memory::<u32>(CURSOR_RESOURCE_OFFSET_ENABLED) != 0 {
            resource_id.wrapping_add(get_from_memory::<u32>(CURSOR_RESOURCE_OFFSET))
        } else {
            resource_id
        }
    }

    fn class_cursor() -> Option<u32> {
        let hwnd = main_window()?;
        let cursor = unsafe { GetClassLongA(hwnd, GCL_HCURSOR) };
        (cursor != 0).then_some(cursor as u32)
    }

    fn system_cursor(id: PCWSTR) -> Option<u32> {
        match unsafe { LoadCursorW(None, id) } {
            Ok(cursor) if !cursor.0.is_null() => Some(cursor.0 as u32),
            Ok(_) => None,
            Err(err) => {
                warn!("failed to load system cursor: {err}");
                None
            }
        }
    }

    fn main_window() -> Option<HWND> {
        let hwnd = get_from_memory::<u32>(GLOBAL_BFAPP + BFAPP_HWND);
        if hwnd != 0 {
            return Some(HWND(hwnd as *mut c_void));
        }
        unsafe { FindWindowA(PCSTR::null(), windows::core::s!("Zoo Tycoon")).ok().filter(|hwnd| !hwnd.0.is_null()) }
    }

    fn destroy_managed(cursor: Option<ManagedCursor>) {
        let Some(cursor) = cursor else {
            return;
        };

        if cursor.ownership == CursorOwnership::Owned {
            unsafe {
                if let Err(err) = DestroyCursor(cursor.handle.as_hcursor()) {
                    warn!("failed to destroy cursor {:#x}: {err}", cursor.handle.raw());
                }
            }
        }
    }
}

#[cfg(target_os = "windows")]
pub use imp::*;

#[cfg(not(target_os = "windows"))]
pub fn init() {}
