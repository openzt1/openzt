//! Compares real vanilla `GXLLE::drawLockedD32ZnF0`/`ZnF1` against their Rust reimplementation (production file
//! `openzt/src/gxlle.rs`) over standalone fixtures, and checks the detours are enabled.
//!
//! The comparison draws random frames at random positions (including far off every edge, so every clip path runs)
//! onto two identical canvases and requires the whole buffers to match, including guard rows above and below and the
//! gap between a row's width and its pitch, so a stray write anywhere is caught.

use std::{ffi::c_void, io::Write};

use tracing::error;

use crate::gxlle::live_support::{detour_status, port_f0, port_f1, real};
use crate::reimplementation_tests::harness::{finish_test, write_success_line};

type DrawFn = unsafe extern "thiscall" fn(*const u32, *const u32, *const c_void, i32, i32) -> bool;

/// `GXLLE_DETOURS_ENABLED` - wiring check.
pub(crate) fn run_gxlle_detours_enabled_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "GXLLE_DETOURS_ENABLED";
    let disabled: Vec<&'static str> = detour_status().into_iter().filter(|(_, enabled)| !enabled).map(|(name, _)| name).collect();
    if disabled.is_empty() {
        write_success_line(failure_log, test_name);
        false
    } else {
        let msg = format!("detours not enabled: {disabled:?}");
        error!("{}: {}", test_name, msg);
        if let Some(log_file) = failure_log {
            let _ = log_file.write_all(format!("Test Failed {}: {}\n", test_name, msg).as_bytes());
        }
        true
    }
}

/// Small deterministic generator so a failing case can be reproduced from its index.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u32 {
        self.0 ^= self.0 << 13;
        self.0 ^= self.0 >> 7;
        self.0 ^= self.0 << 17;
        (self.0 >> 16) as u32
    }

    fn below(&mut self, bound: u32) -> u32 {
        self.next() % bound
    }

    fn range(&mut self, low: i32, high: i32) -> i32 {
        low + self.below((high - low) as u32) as i32
    }
}

struct Frame {
    bytes: Vec<u8>,
}

fn random_frame(rng: &mut Rng) -> Frame {
    let rows = rng.below(40) as u16;
    let mut bytes = vec![0u8; 10];
    let mut widest = 0usize;
    for _ in 0..rows {
        let spans = rng.below(6) as usize;
        let mut extent = 0usize;
        let mut row = Vec::new();
        for _ in 0..spans {
            let skip = rng.below(14) as u8;
            let length = rng.below(20) as u8;
            row.push(skip);
            row.push(length);
            for _ in 0..length {
                row.push(rng.next() as u8);
            }
            extent += skip as usize + length as usize;
        }
        widest = widest.max(extent);
        bytes.push(spans as u8);
        bytes.extend(row);
    }
    let header = [rows, widest as u16, rng.range(-25, 25) as i16 as u16, rng.range(-25, 25) as i16 as u16, 0];
    for (index, value) in header.into_iter().enumerate() {
        bytes[index * 2..index * 2 + 2].copy_from_slice(&value.to_le_bytes());
    }
    Frame { bytes }
}

/// A canvas with one guard row above and below its visible rows and a pitch wider than its width.
struct Canvas {
    pixels: Vec<u32>,
    header: Box<[u32; 0x40]>,
}

impl Canvas {
    fn new(width: usize, height: usize, pitch_words: usize, background: u32) -> Self {
        let mut pixels = vec![background; pitch_words * (height + 2)];
        let base = unsafe { pixels.as_mut_ptr().add(pitch_words) } as usize as u32;
        let mut header = Box::new([0u32; 0x40]);
        header[0xc / 4] = height as u32;
        header[0x10 / 4] = width as u32;
        header[0x14 / 4] = (pitch_words * 4) as u32;
        header[0x28 / 4] = base;
        Self { pixels, header }
    }
}

fn run_case(draw: DrawFn, frame: &Frame, table: &[u32], canvas: &Canvas, x: i32, y: i32) -> bool {
    let frame_holder = [frame.bytes.as_ptr() as usize as u32];
    let mut palette_map = [0u32; 0x40];
    palette_map[0x28 / 4] = table.as_ptr() as usize as u32;
    unsafe { draw(frame_holder.as_ptr(), canvas.header.as_ptr(), palette_map.as_ptr() as *const c_void, x, y) }
}

/// `GXLLE_DRAW_LOCKED_D32ZN_MATCHES_REAL` - random frames and positions through the vanilla and ported `F0` and
/// `F1`; the canvases and return values must match.
pub(crate) fn run_gxlle_draw_locked_d32_zn_matches_real_test(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "GXLLE_DRAW_LOCKED_D32ZN_MATCHES_REAL";
    let mut failures: Vec<String> = Vec::new();
    let mut rng = Rng(0x9e37_79b9_7f4a_7c15);
    let table: Vec<u32> = (0..256).map(|_| rng.next() | 1).collect();
    let poles: [(&str, DrawFn, DrawFn); 2] = [("ZnF0", real::draw_locked_d32_zn_f0, port_f0), ("ZnF1", real::draw_locked_d32_zn_f1, port_f1)];
    for case in 0..6000u32 {
        let frame = random_frame(&mut rng);
        let width = 1 + rng.below(48) as usize;
        let height = 1 + rng.below(48) as usize;
        let pitch_words = width + rng.below(4) as usize;
        let x = rng.range(-70, width as i32 + 70);
        let y = rng.range(-70, height as i32 + 70);
        for (name, real_fn, port_fn) in poles {
            let (real_canvas, port_canvas) = (Canvas::new(width, height, pitch_words, 0xdead_beef), Canvas::new(width, height, pitch_words, 0xdead_beef));
            let real_result = run_case(real_fn, &frame, &table, &real_canvas, x, y);
            let port_result = run_case(port_fn, &frame, &table, &port_canvas, x, y);
            if real_result != port_result {
                failures.push(format!("case {case} {name}: return real={real_result} port={port_result}"));
            }
            if real_canvas.pixels != port_canvas.pixels {
                let first = real_canvas.pixels.iter().zip(&port_canvas.pixels).position(|(a, b)| a != b).unwrap_or(0);
                failures.push(format!(
                    "case {case} {name}: canvas differs at word {first} ({width}x{height} pitch {pitch_words} at ({x}, {y}), {} frame bytes)",
                    frame.bytes.len()
                ));
            }
            if failures.len() >= 8 {
                return finish_test(test_name, failures, failure_log);
            }
        }
    }
    finish_test(test_name, failures, failure_log)
}
