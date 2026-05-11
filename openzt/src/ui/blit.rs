use std::ffi::c_void;
use std::ptr;

use tiny_skia::Pixmap;
use tracing::warn;
use windows::Win32::Foundation::HWND;
use windows::Win32::Graphics::Gdi::{
    BI_RGB, BITMAPINFO, BITMAPINFOHEADER, CreateCompatibleDC, CreateDIBSection, DIB_RGB_COLORS, DeleteDC, DeleteObject, GetDC, HGDIOBJ, ReleaseDC, SelectObject,
    TransparentBlt,
};

const MAGENTA_COLORREF: u32 = 0x00FF00FF;

pub fn blit_to_hwnd(hwnd: HWND, pixmap: &Pixmap) {
    let width = pixmap.width();
    let height = pixmap.height();
    if width == 0 || height == 0 {
        return;
    }

    unsafe {
        let window_dc = GetDC(Some(hwnd));
        if window_dc.is_invalid() {
            warn!("egui overlay: GetDC failed");
            return;
        }

        let mem_dc = CreateCompatibleDC(Some(window_dc));
        if mem_dc.is_invalid() {
            warn!("egui overlay: CreateCompatibleDC failed");
            ReleaseDC(Some(hwnd), window_dc);
            return;
        }

        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: width as i32,
                biHeight: -(height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB.0,
                ..Default::default()
            },
            ..Default::default()
        };

        let mut bits: *mut c_void = ptr::null_mut();
        let bitmap = match CreateDIBSection(Some(window_dc), &info, DIB_RGB_COLORS, &mut bits, None, 0) {
            Ok(bitmap) => bitmap,
            Err(err) => {
                warn!("egui overlay: CreateDIBSection failed: {err}");
                let _ = DeleteDC(mem_dc);
                ReleaseDC(Some(hwnd), window_dc);
                return;
            }
        };

        let bitmap_obj = HGDIOBJ(bitmap.0);
        let old_obj = SelectObject(mem_dc, bitmap_obj);
        if old_obj.is_invalid() {
            warn!("egui overlay: SelectObject failed");
            let _ = DeleteObject(bitmap_obj);
            let _ = DeleteDC(mem_dc);
            ReleaseDC(Some(hwnd), window_dc);
            return;
        }

        if bits.is_null() {
            warn!("egui overlay: DIB section returned null bits");
        } else {
            ptr::copy_nonoverlapping(pixmap.data().as_ptr(), bits.cast::<u8>(), pixmap.data().len());
            if TransparentBlt(
                window_dc,
                0,
                0,
                width as i32,
                height as i32,
                mem_dc,
                0,
                0,
                width as i32,
                height as i32,
                MAGENTA_COLORREF,
            )
            .as_bool()
                == false
            {
                warn!("egui overlay: TransparentBlt failed");
            }
        }

        SelectObject(mem_dc, old_obj);
        let _ = DeleteObject(bitmap_obj);
        let _ = DeleteDC(mem_dc);
        ReleaseDC(Some(hwnd), window_dc);
    }
}
