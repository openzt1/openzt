//! `GXLLE`'s 32-bit, no-zoom locked blits: `drawLockedD32ZnF0` (`0x4fe96e`) and its mirrored sibling
//! `drawLockedD32ZnF1` (`0x4febe9`).
//!
//! `GXLLE` is a thin handle: `*this` is a pointer to a run-length-encoded frame, and the blits only read the frame,
//! a palette map and a few canvas fields, and write pixels into the canvas's locked surface. Nothing is allocated and
//! no vanilla layout is kept, so the functions are plain raw-pointer code.
//!
//! ## Layouts used here
//! - Frame: ten header bytes read as little-endian `u16`s `[rows | 0x8000 = monolith, width, row origin, column
//!   origin, unused]`, then per row a span-count byte followed by `(skip, length, length palette indices)` spans.
//! - Canvas (`GXCanvas`): height at `+0x0c`, width at `+0x10`, pitch in bytes at `+0x14`, locked surface base at
//!   `+0x28`.
//! - Palette map: a pointer at `+0x28` to 256 32-bit colours.
//!
//! Both blits clip against the canvas and always return `true` (the vanilla functions return `1` on every path).
//! `F1` draws each span leftwards from the draw position.
//!
//! Differences from vanilla that do not change the output: spans are expanded in groups of eight or four palette
//! indices (one read per four indices, all table reads before the group's writes, the tail covered by an overlapping
//! group), and `F1` clips each span once instead of testing every pixel.

use std::ffi::c_void;

use openzt_detour::generated::gxlle::{DRAW_LOCKED_D32ZN_F0, DRAW_LOCKED_D32ZN_F1};
use openzt_detour_macro::detour_mod;
use tracing::error;

#[inline(always)]
unsafe fn read_i32(base: *const u32, byte_offset: usize) -> i32 {
    unsafe { (base as *const u8).add(byte_offset).cast::<i32>().read_unaligned() }
}

#[inline(always)]
unsafe fn read_u16(base: *const u8, byte_offset: usize) -> u16 {
    unsafe { base.add(byte_offset).cast::<u16>().read_unaligned() }
}

/// Advances `data` past one row of spans without drawing it.
#[inline(always)]
unsafe fn skip_row(mut data: *const u8) -> *const u8 {
    unsafe {
        let spans = *data;
        data = data.add(1);
        for _ in 0..spans {
            data = data.add(*data.add(1) as usize + 2);
        }
    }
    data
}

/// Writes `len` palette-expanded pixels from the index bytes at `src` to `dst`, `dst + 1`, ... (or `dst`, `dst - 1`,
/// ... when `BACKWARD`). Long spans go eight pixels at a time with all eight table reads issued before the writes, so
/// the reads can overlap; the remainder is a plain loop.
#[inline(always)]
unsafe fn expand_span<const BACKWARD: bool>(dst: *mut u32, src: *const u8, len: usize, table: *const u32) {
    unsafe {
        let at = |index: usize| if BACKWARD { dst.wrapping_sub(index) } else { dst.wrapping_add(index) };
        let mut from = 0usize;
        while from + 8 <= len {
            let group = src.add(from);
            let c0 = *table.add(*group as usize);
            let c1 = *table.add(*group.add(1) as usize);
            let c2 = *table.add(*group.add(2) as usize);
            let c3 = *table.add(*group.add(3) as usize);
            let c4 = *table.add(*group.add(4) as usize);
            let c5 = *table.add(*group.add(5) as usize);
            let c6 = *table.add(*group.add(6) as usize);
            let c7 = *table.add(*group.add(7) as usize);
            at(from).write(c0);
            at(from + 1).write(c1);
            at(from + 2).write(c2);
            at(from + 3).write(c3);
            at(from + 4).write(c4);
            at(from + 5).write(c5);
            at(from + 6).write(c6);
            at(from + 7).write(c7);
            from += 8;
        }
        while from < len {
            at(from).write(*table.add(*src.add(from) as usize));
            from += 1;
        }
    }
}

/// `GXLLE::drawLockedD32ZnF0(canvas, paletteMap, x, y)`: draws the frame left to right with its column origin
/// subtracted from `x`.
pub(crate) unsafe extern "thiscall" fn draw_locked_d32_zn_f0(this: *const u32, canvas: *const u32, palette_map: *const c_void, x: i32, y: i32) -> bool {
    unsafe {
        let frame = this.read() as usize as *const u8;
        let canvas_height = read_i32(canvas, 0xc);
        let canvas_width = read_i32(canvas, 0x10);
        let pitch = read_i32(canvas, 0x14);
        let surface = read_i32(canvas, 0x28) as usize as *mut u8;
        let table = read_i32(palette_map as *const u32, 0x28) as usize as *const u32;

        let mut rows = (read_u16(frame, 0) & 0x7fff) as i32;
        let frame_width = read_u16(frame, 2) as i32;
        let mut row = y - read_u16(frame, 4) as i16 as i32;
        let column = x - read_u16(frame, 6) as i16 as i32;
        let mut data = frame.add(10);

        if !(row < canvas_height && 0 < rows + row && column < canvas_width && 0 < frame_width + column) {
            return true;
        }

        while row < 0 && rows > 0 {
            data = skip_row(data);
            row += 1;
            rows -= 1;
        }

        let mut row_start = surface.wrapping_offset((row as isize).wrapping_mul(pitch as isize)).wrapping_offset(column as isize * 4).cast::<u32>();
        while row < canvas_height && rows > 0 {
            let spans = *data;
            data = data.add(1);
            let mut span_column = column;
            let mut dst = row_start;
            for _ in 0..spans {
                let skip = *data as i32;
                let length = *data.add(1) as i32;
                let src = data.add(2);
                data = src.add(length as usize);
                span_column += skip;
                dst = dst.wrapping_offset(skip as isize);
                // The common case: the whole span is inside the canvas.
                if span_column >= 0 && span_column + length <= canvas_width {
                    if length > 0 {
                        expand_span::<false>(dst, src, length as usize, table);
                        dst = dst.wrapping_offset(length as isize);
                        span_column += length;
                    }
                    continue;
                }
                if span_column >= canvas_width {
                    continue;
                }
                if length < -span_column {
                    span_column += length;
                    dst = dst.wrapping_offset(length as isize);
                    continue;
                }
                let (mut length, mut src) = (length, src);
                if span_column < 0 {
                    length += span_column;
                    src = src.offset(-(span_column as isize));
                    dst = dst.offset(-(span_column as isize));
                    span_column = 0;
                }
                if canvas_width <= length + span_column {
                    length = canvas_width - span_column;
                }
                span_column += length;
                if length > 0 {
                    expand_span::<false>(dst, src, length as usize, table);
                    dst = dst.wrapping_offset(length as isize);
                }
            }
            row_start = row_start.wrapping_byte_offset(pitch as isize);
            row += 1;
            rows -= 1;
        }
    }
    true
}

/// `GXLLE::drawLockedD32ZnF1(canvas, paletteMap, x, y)`: draws the frame right to left, starting at `x` plus its
/// column origin.
pub(crate) unsafe extern "thiscall" fn draw_locked_d32_zn_f1(this: *const u32, canvas: *const u32, palette_map: *const c_void, x: i32, y: i32) -> bool {
    unsafe {
        let frame = this.read() as usize as *const u8;
        let canvas_height = read_i32(canvas, 0xc);
        let canvas_width = read_i32(canvas, 0x10);
        let pitch = read_i32(canvas, 0x14);
        let surface = read_i32(canvas, 0x28) as usize as *mut u8;
        let table = read_i32(palette_map as *const u32, 0x28) as usize as *const u32;

        let mut rows = (read_u16(frame, 0) & 0x7fff) as i32;
        let frame_width = read_u16(frame, 2) as i32;
        let mut row = y - read_u16(frame, 4) as i16 as i32;
        let column = x + read_u16(frame, 6) as i16 as i32;
        let mut data = frame.add(10);

        if !(row < canvas_height && 0 < rows + row && -1 < column && column - frame_width < canvas_width) {
            return true;
        }

        while row < 0 && rows > 0 {
            data = skip_row(data);
            row += 1;
            rows -= 1;
        }

        let mut row_start = surface.wrapping_offset((row as isize).wrapping_mul(pitch as isize)).wrapping_offset(column as isize * 4).cast::<u32>();
        while row < canvas_height && rows > 0 {
            let spans = *data;
            data = data.add(1);
            let mut span_column = column;
            for _ in 0..spans {
                let skip = *data as i32;
                let length = *data.add(1) as i32;
                let src = data.add(2);
                data = src.add(length as usize);
                span_column -= skip;
                // Pixel `i` of the span lands at column `span_column - i`; keep those inside the canvas.
                let first = (span_column - canvas_width + 1).max(0);
                let last = (length - 1).min(span_column);
                if first <= last {
                    let dst = row_start.wrapping_offset((span_column - first - column) as isize);
                    expand_span::<true>(dst, src.add(first as usize), (last - first + 1) as usize, table);
                }
                span_column -= length;
            }
            row_start = row_start.wrapping_byte_offset(pitch as isize);
            row += 1;
            rows -= 1;
        }
    }
    true
}

#[detour_mod]
mod detours {
    use super::*;

    #[detour(DRAW_LOCKED_D32ZN_F0)]
    unsafe extern "thiscall" fn draw_locked_d32_zn_f0_detour(this: *const u32, canvas: *const u32, palette_map: *const c_void, x: i32, y: i32) -> bool {
        unsafe { draw_locked_d32_zn_f0(this, canvas, palette_map, x, y) }
    }

    #[detour(DRAW_LOCKED_D32ZN_F1)]
    unsafe extern "thiscall" fn draw_locked_d32_zn_f1_detour(this: *const u32, canvas: *const u32, palette_map: *const c_void, x: i32, y: i32) -> bool {
        unsafe { draw_locked_d32_zn_f1(this, canvas, palette_map, x, y) }
    }

    /// Real vanilla bodies for the live battery's "real" pole, valid in every profile.
    #[cfg(feature = "reimplementation-tests")]
    pub(crate) mod test_real {
        use super::*;

        pub(crate) unsafe extern "thiscall" fn draw_locked_d32_zn_f0(this: *const u32, canvas: *const u32, palette_map: *const c_void, x: i32, y: i32) -> bool {
            unsafe { super::DRAW_LOCKED_D32ZN_F0_DETOUR.call(this, canvas, palette_map, x, y) }
        }

        pub(crate) unsafe extern "thiscall" fn draw_locked_d32_zn_f1(this: *const u32, canvas: *const u32, palette_map: *const c_void, x: i32, y: i32) -> bool {
            unsafe { super::DRAW_LOCKED_D32ZN_F1_DETOUR.call(this, canvas, palette_map, x, y) }
        }
    }
}

/// Installs this module's detours. Called from both `lib.rs`'s boot cascade and
/// `reimplementation_tests::init()`.
pub fn init() {
    if let Err(e) = unsafe { detours::init_detours() } {
        error!("Failed to initialise gxlle detours: {e:?}");
    }
}

#[cfg(feature = "reimplementation-tests")]
pub(crate) mod live_support {
    use super::*;

    pub(crate) use super::detours::test_real as real;
    pub(crate) use super::{draw_locked_d32_zn_f0 as port_f0, draw_locked_d32_zn_f1 as port_f1};

    /// `(name, is_enabled)` per detour.
    pub(crate) fn detour_status() -> Vec<(&'static str, bool)> {
        detours::status()
    }
}
