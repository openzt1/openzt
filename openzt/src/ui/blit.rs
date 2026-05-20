use std::ffi::c_void;
use std::ptr;
use std::sync::{Mutex, OnceLock};

use tiny_skia::Pixmap;
use tracing::{info, warn};
use windows::Win32::Foundation::{COLORREF, HINSTANCE, HWND, LPARAM, LRESULT, POINT, RECT, SIZE, WPARAM};
use windows::Win32::Graphics::Gdi::{
    AC_SRC_ALPHA, BI_RGB, BITMAPINFO, BITMAPINFOHEADER, BLENDFUNCTION, ClientToScreen, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject,
    GetDC, HGDIOBJ, ReleaseDC, SelectObject,
};
use windows::Win32::System::LibraryLoader::GetModuleHandleA;
use windows::Win32::UI::WindowsAndMessaging::{
    CS_HREDRAW, CS_VREDRAW, CreateWindowExA, DefWindowProcA, RegisterClassA, SW_HIDE, SW_SHOWNOACTIVATE, SWP_NOACTIVATE, SWP_NOZORDER, SWP_SHOWWINDOW, SetWindowPos,
    ShowWindow, ULW_ALPHA, UpdateLayeredWindow, WNDCLASSA, WS_EX_LAYERED, WS_EX_NOACTIVATE, WS_EX_TOOLWINDOW, WS_EX_TRANSPARENT, WS_POPUP,
};
use windows::core::PCSTR;

const OVERLAY_CLASS: PCSTR = windows::core::s!("OpenZTLayeredOverlay");

static OVERLAY: OnceLock<Mutex<LayeredOverlay>> = OnceLock::new();
static CLASS_REGISTERED: OnceLock<()> = OnceLock::new();

#[derive(Default)]
struct LayeredOverlay {
    hwnd: Option<isize>,
    owner: Option<isize>,
}

pub fn blit_to_hwnd(hwnd: HWND, pixmap: &Pixmap) {
    let Some(rect) = client_screen_rect(hwnd) else {
        return;
    };

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    if width <= 0 || height <= 0 || pixmap.width() != width as u32 || pixmap.height() != height as u32 {
        return;
    }

    let overlay = OVERLAY.get_or_init(|| Mutex::new(LayeredOverlay::default()));
    let mut overlay = match overlay.lock() {
        Ok(overlay) => overlay,
        Err(err) => {
            warn!("egui overlay: layered overlay lock poisoned: {err}");
            return;
        }
    };

    let Some(overlay_hwnd) = overlay.ensure(hwnd) else {
        return;
    };

    if let Err(err) = update_overlay_window(overlay_hwnd, rect, pixmap) {
        warn!("egui overlay: UpdateLayeredWindow failed: {err}");
    }
}

pub fn sync_overlay_position(owner: HWND) {
    let Some(rect) = client_screen_rect(owner) else {
        return;
    };

    let overlay = OVERLAY.get_or_init(|| Mutex::new(LayeredOverlay::default()));
    let mut overlay = match overlay.lock() {
        Ok(overlay) => overlay,
        Err(err) => {
            warn!("egui overlay: layered overlay lock poisoned: {err}");
            return;
        }
    };

    let Some(overlay_hwnd) = overlay.ensure(owner) else {
        return;
    };

    if let Err(err) = position_overlay_window(overlay_hwnd, rect) {
        warn!("egui overlay: SetWindowPos failed while syncing position: {err}");
    }
}

pub fn hide_overlay() {
    let Some(overlay) = OVERLAY.get() else {
        return;
    };

    let overlay = match overlay.lock() {
        Ok(overlay) => overlay,
        Err(err) => {
            warn!("egui overlay: layered overlay lock poisoned: {err}");
            return;
        }
    };

    if let Some(hwnd_raw) = overlay.hwnd {
        unsafe {
            let _ = ShowWindow(HWND(hwnd_raw as *mut c_void), SW_HIDE);
        }
    }
}

impl LayeredOverlay {
    fn ensure(&mut self, owner: HWND) -> Option<HWND> {
        let owner_raw = owner.0 as isize;
        if self.owner == Some(owner_raw)
            && let Some(hwnd_raw) = self.hwnd
        {
            return Some(HWND(hwnd_raw as *mut c_void));
        }

        register_overlay_class();

        let module = match unsafe { GetModuleHandleA(PCSTR::null()) } {
            Ok(module) => HINSTANCE(module.0),
            Err(err) => {
                warn!("egui overlay: GetModuleHandleA failed: {err}");
                return None;
            }
        };

        let hwnd = match unsafe {
            CreateWindowExA(
                WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_TOOLWINDOW | WS_EX_NOACTIVATE,
                OVERLAY_CLASS,
                windows::core::s!("OpenZT Overlay"),
                WS_POPUP,
                0,
                0,
                1,
                1,
                Some(owner),
                None,
                Some(module),
                None,
            )
        } {
            Ok(hwnd) => hwnd,
            Err(err) => {
                warn!("egui overlay: CreateWindowExA failed: {err}");
                return None;
            }
        };

        unsafe {
            let _ = ShowWindow(hwnd, SW_SHOWNOACTIVATE);
        }

        self.owner = Some(owner_raw);
        self.hwnd = Some(hwnd.0 as isize);
        info!("egui overlay: created layered overlay HWND {:#x}", hwnd.0 as isize);
        Some(hwnd)
    }
}

fn register_overlay_class() {
    CLASS_REGISTERED.get_or_init(|| {
        let module = match unsafe { GetModuleHandleA(PCSTR::null()) } {
            Ok(module) => HINSTANCE(module.0),
            Err(err) => {
                warn!("egui overlay: GetModuleHandleA failed while registering overlay class: {err}");
                return;
            }
        };

        let class = WNDCLASSA {
            style: CS_HREDRAW | CS_VREDRAW,
            lpfnWndProc: Some(overlay_wndproc),
            hInstance: module,
            lpszClassName: OVERLAY_CLASS,
            ..Default::default()
        };

        let atom = unsafe { RegisterClassA(&class) };
        if atom == 0 {
            warn!("egui overlay: RegisterClassA returned 0 for overlay class");
        }
    });
}

unsafe extern "system" fn overlay_wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    unsafe { DefWindowProcA(hwnd, msg, wparam, lparam) }
}

fn client_screen_rect(hwnd: HWND) -> Option<RECT> {
    let mut rect = RECT::default();
    if let Err(err) = unsafe { windows::Win32::UI::WindowsAndMessaging::GetClientRect(hwnd, &mut rect) } {
        warn!("egui overlay: GetClientRect failed: {err}");
        return None;
    }

    let mut origin = POINT { x: rect.left, y: rect.top };
    if !unsafe { ClientToScreen(hwnd, &mut origin) }.as_bool() {
        warn!("egui overlay: ClientToScreen failed");
        return None;
    }

    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    Some(RECT {
        left: origin.x,
        top: origin.y,
        right: origin.x + width,
        bottom: origin.y + height,
    })
}

fn update_overlay_window(hwnd: HWND, rect: RECT, pixmap: &Pixmap) -> windows::core::Result<()> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;

    unsafe {
        position_overlay_window(hwnd, rect)?;

        let screen_dc = GetDC(None);
        if screen_dc.is_invalid() {
            warn!("egui overlay: GetDC(None) failed");
            return Ok(());
        }

        let mem_dc = CreateCompatibleDC(Some(screen_dc));
        if mem_dc.is_invalid() {
            warn!("egui overlay: CreateCompatibleDC failed");
            ReleaseDC(None, screen_dc);
            return Ok(());
        }

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width,
                biHeight: -height,
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(screen_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => bitmap,
            Err(err) => {
                let _ = DeleteDC(mem_dc);
                ReleaseDC(None, screen_dc);
                return Err(err);
            }
        };

        let bitmap_obj = HGDIOBJ(bitmap.0);
        let old_obj = SelectObject(mem_dc, bitmap_obj);
        if old_obj.is_invalid() {
            warn!("egui overlay: SelectObject failed");
            let _ = DeleteObject(bitmap_obj);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(None, screen_dc);
            return Ok(());
        }

        if bits.is_null() {
            warn!("egui overlay: DIB section returned null bits");
        } else {
            copy_pixmap_to_layered_dib(pixmap, bits.cast::<u8>());
            let destination = POINT { x: rect.left, y: rect.top };
            let size = SIZE { cx: width, cy: height };
            let source = POINT { x: 0, y: 0 };
            let blend = BLENDFUNCTION {
                BlendOp: 0,
                BlendFlags: 0,
                SourceConstantAlpha: 255,
                AlphaFormat: AC_SRC_ALPHA as u8,
            };

            UpdateLayeredWindow(
                hwnd,
                Some(screen_dc),
                Some(&destination),
                Some(&size),
                Some(mem_dc),
                Some(&source),
                COLORREF(0),
                Some(&blend),
                ULW_ALPHA,
            )?;
        }

        SelectObject(mem_dc, old_obj);
        let _ = DeleteObject(bitmap_obj);
        let _ = DeleteDC(mem_dc);
        ReleaseDC(None, screen_dc);
    }

    Ok(())
}

fn position_overlay_window(hwnd: HWND, rect: RECT) -> windows::core::Result<()> {
    let width = rect.right - rect.left;
    let height = rect.bottom - rect.top;
    unsafe { SetWindowPos(hwnd, None, rect.left, rect.top, width, height, SWP_NOACTIVATE | SWP_NOZORDER | SWP_SHOWWINDOW) }
}

fn copy_pixmap_to_layered_dib(pixmap: &Pixmap, dib_bits: *mut u8) {
    for (index, pixel) in pixmap.pixels().iter().enumerate() {
        let offset = index * 4;
        unsafe {
            *dib_bits.add(offset) = pixel.blue();
            *dib_bits.add(offset + 1) = pixel.green();
            *dib_bits.add(offset + 2) = pixel.red();
            *dib_bits.add(offset + 3) = pixel.alpha();
        }
    }
}
