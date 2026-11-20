//! Timing baseline for the vanilla 32-bit, no-zoom locked blits `GXLLE::drawLockedD32ZnF0` and
//! `GXLLE::drawLockedD32ZnF1`, over standalone fixtures (a fake canvas, a fake palette map and a hand-built RLE
//! frame), so a later reimplementation can be timed with the same harness and compared like for like.
//!
//! Each case first draws once onto a zeroed canvas and checks the number of written pixels against the number the
//! frame encodes, so a fixture the vanilla code rejects early cannot produce a meaningless timing. Results go to the
//! battery log as `Bench ...` lines: the minimum and median nanoseconds per call over repeated batches, and the
//! same per painted pixel.
//!
//! Frame layout (as read by the vanilla blits): ten header bytes `[rows | 0x8000 = monolith, width, row origin,
//! column origin, unused]` as little-endian `u16`s, then per row a span-count byte followed by `(skip, length,
//! length palette indices)` spans.

use std::{ffi::c_void, hint::black_box, io::Write, time::Instant};

use openzt_detour::generated::gxlle::{DRAW_LOCKED_D32ZN_F0, DRAW_LOCKED_D32ZN_F1};
use tracing::info;

use crate::gxlle::live_support::{port_f0, port_f1, real};
use crate::reimplementation_tests::harness::finish_test;

const CANVAS_WIDTH: usize = 640;
const CANVAS_HEIGHT: usize = 480;
const SAMPLES: usize = 15;
const MIN_BATCH_MILLIS: u128 = 10;

type DrawFn = unsafe extern "thiscall" fn(*const u32, *const u32, *const c_void, i32, i32) -> bool;

struct Case {
    label: &'static str,
    width: usize,
    height: usize,
    /// `None` for one opaque span per row; `Some((skip, length))` repeats that span across the row; `Some((0, 0))`
    /// draws pseudo-random spans of 1-20 pixels after skips of 0-6.
    pattern: Option<(u8, u8)>,
    /// Draw position so that half of the sprite is off the right edge (`F0`) or left edge (`F1`).
    clipped: bool,
}

const CASES: &[Case] = &[
    Case { label: "16x16 opaque", width: 16, height: 16, pattern: None, clipped: false },
    Case { label: "64x64 opaque", width: 64, height: 64, pattern: None, clipped: false },
    Case { label: "128x128 opaque", width: 128, height: 128, pattern: None, clipped: false },
    Case { label: "128x128 sparse 6-of-8", width: 128, height: 128, pattern: Some((2, 6)), clipped: false },
    Case { label: "128x128 sparse 3-of-4", width: 128, height: 128, pattern: Some((1, 3)), clipped: false },
    Case { label: "128x128 sparse 12-of-16", width: 128, height: 128, pattern: Some((4, 12)), clipped: false },
    Case { label: "128x128 random spans 1-20", width: 128, height: 128, pattern: Some((0, 0)), clipped: false },
    Case { label: "128x128 opaque, half clipped", width: 128, height: 128, pattern: None, clipped: true },
];

/// Builds the RLE bytes for `case` and returns them with the number of palette-indexed pixels they encode.
fn build_frame(case: &Case) -> (Vec<u8>, usize) {
    let mut bytes = Vec::new();
    for value in [case.height as u16, case.width as u16, 0, 0, 0] {
        bytes.extend_from_slice(&value.to_le_bytes());
    }
    let mut painted = 0usize;
    let mut seed = 0x2545_f491u32;
    let mut next = move || {
        seed ^= seed << 13;
        seed ^= seed >> 17;
        seed ^= seed << 5;
        seed
    };
    for row in 0..case.height {
        let mut spans: Vec<(u8, u8)> = Vec::new();
        match case.pattern {
            None => spans.push((0, case.width as u8)),
            Some((0, 0)) => {
                let mut column = 0usize;
                loop {
                    let (skip, length) = ((next() % 7) as u8, (next() % 20) as u8 + 1);
                    if column + skip as usize + length as usize > case.width {
                        break;
                    }
                    spans.push((skip, length));
                    column += skip as usize + length as usize;
                }
            }
            Some((skip, length)) => {
                let mut column = 0usize;
                while column + (skip as usize) + (length as usize) <= case.width {
                    spans.push((skip, length));
                    column += (skip as usize) + (length as usize);
                }
            }
        }
        bytes.push(spans.len() as u8);
        let mut column = 0usize;
        for (skip, length) in spans {
            bytes.push(skip);
            bytes.push(length);
            for _ in 0..length {
                // Index 0 maps to a zero colour, which would be indistinguishable from the cleared canvas.
                bytes.push(((row * 7 + column) % 255) as u8 + 1);
                column += 1;
                painted += 1;
            }
            column += skip as usize;
        }
    }
    (bytes, painted)
}

struct Fixture {
    // Kept alive for the raw pointers stored in the fake structs below.
    _frame: Vec<u8>,
    _palette: Vec<u32>,
    pixels: Vec<u32>,
    frame_holder: Box<[u32; 1]>,
    canvas: Box<[u32; 0x40]>,
    palette_map: Box<[u32; 0x40]>,
    painted: usize,
}

impl Fixture {
    fn new(case: &Case) -> Self {
        let (frame, painted) = build_frame(case);
        let palette: Vec<u32> = (0..256u32).map(|index| 0xFF00_0000 | index.wrapping_mul(0x0001_0101)).collect();
        let mut pixels = vec![0u32; CANVAS_WIDTH * CANVAS_HEIGHT];
        let frame_holder = Box::new([frame.as_ptr() as usize as u32]);
        let mut canvas = Box::new([0u32; 0x40]);
        canvas[0xc / 4] = CANVAS_HEIGHT as u32;
        canvas[0x10 / 4] = CANVAS_WIDTH as u32;
        canvas[0x14 / 4] = (CANVAS_WIDTH * 4) as u32;
        canvas[0x28 / 4] = pixels.as_mut_ptr() as usize as u32;
        let mut palette_map = Box::new([0u32; 0x40]);
        palette_map[0x28 / 4] = palette.as_ptr() as usize as u32;
        Self { _frame: frame, _palette: palette, pixels, frame_holder, canvas, palette_map, painted }
    }

    fn clear(&mut self) {
        self.pixels.fill(0);
    }

    fn painted_on_canvas(&self) -> usize {
        self.pixels.iter().filter(|pixel| **pixel != 0).count()
    }

    /// Draw position: centred, or half off the right (`F0`) / left (`F1`) edge for a clipped case.
    fn position(case: &Case, flipped: bool) -> (i32, i32) {
        let x = match (case.clipped, flipped) {
            (false, false) => 200,
            (false, true) => 320,
            (true, false) => (CANVAS_WIDTH - case.width / 2) as i32,
            (true, true) => (case.width / 2 - 1) as i32,
        };
        (x, 200)
    }

    fn draw(&self, draw: DrawFn, x: i32, y: i32) -> bool {
        unsafe {
            draw(
                self.frame_holder.as_ptr(),
                self.canvas.as_ptr(),
                self.palette_map.as_ptr() as *const c_void,
                x,
                y,
            )
        }
    }
}

/// Expected number of visible pixels after one draw.
fn expected_visible(case: &Case, painted: usize) -> usize {
    if case.clipped { painted / 2 } else { painted }
}

/// Minimum and median nanoseconds per call for `draw` over `SAMPLES` batches.
fn time_draw(fixture: &Fixture, draw: DrawFn, x: i32, y: i32) -> (f64, f64) {
    for _ in 0..200 {
        black_box(fixture.draw(draw, x, y));
    }
    let mut iterations = 16u64;
    loop {
        let started = Instant::now();
        for _ in 0..iterations {
            black_box(fixture.draw(draw, x, y));
        }
        if started.elapsed().as_millis() >= MIN_BATCH_MILLIS {
            break;
        }
        iterations *= 2;
    }
    let mut per_call: Vec<f64> = (0..SAMPLES)
        .map(|_| {
            let started = Instant::now();
            for _ in 0..iterations {
                black_box(fixture.draw(draw, x, y));
            }
            started.elapsed().as_nanos() as f64 / iterations as f64
        })
        .collect();
    per_call.sort_by(|a, b| a.partial_cmp(b).unwrap());
    (per_call[0], per_call[SAMPLES / 2])
}

fn log_line(failure_log: &mut Option<std::fs::File>, line: &str) {
    info!("{}", line);
    if let Some(log_file) = failure_log {
        let _ = log_file.write_all(format!("{line}\n").as_bytes());
    }
}

/// Times `draw` (named `name` in the log) over every [`CASES`] entry. `flipped` selects the draw position convention.
pub(crate) fn run_bench(name: &str, draw: DrawFn, flipped: bool, failure_log: &mut Option<std::fs::File>, failures: &mut Vec<String>) {
    for case in CASES {
        let mut fixture = Fixture::new(case);
        let (x, y) = Fixture::position(case, flipped);
        fixture.clear();
        fixture.draw(draw, x, y);
        let (got, expected) = (fixture.painted_on_canvas(), expected_visible(case, fixture.painted));
        if got != expected {
            failures.push(format!("{name} {}: drew {got} pixels, expected {expected}; fixture invalid", case.label));
            continue;
        }
        let (min_ns, median_ns) = time_draw(&fixture, draw, x, y);
        log_line(
            failure_log,
            &format!(
                "Bench {name} [{}]: min {min_ns:.0} ns/call, median {median_ns:.0} ns/call, {:.3} ns/px (min), {expected} px",
                case.label,
                min_ns / expected as f64
            ),
        );
    }
}

/// `GXLLE_DRAW_BENCH` - times `drawLockedD32ZnF0`/`ZnF1` three ways: real vanilla through the detour trampoline, the
/// port called directly, and the port reached through the hooked game address (the path the game takes).
pub(crate) fn run_gxlle_draw_bench(failure_log: &mut Option<std::fs::File>) -> bool {
    let test_name = "GXLLE_DRAW_BENCH";
    let mut failures: Vec<String> = Vec::new();
    log_line(
        failure_log,
        &format!("Bench header: canvas {CANVAS_WIDTH}x{CANVAS_HEIGHT} x32bpp, {SAMPLES} samples, build={}", if cfg!(debug_assertions) { "debug" } else { "release" }),
    );
    let (hooked_f0, hooked_f1) = unsafe { (DRAW_LOCKED_D32ZN_F0.hooked(), DRAW_LOCKED_D32ZN_F1.hooked()) };
    run_bench("vanilla ZnF0 (trampoline)", real::draw_locked_d32_zn_f0, false, failure_log, &mut failures);
    run_bench("port ZnF0 (direct)", port_f0, false, failure_log, &mut failures);
    run_bench("port ZnF0 (hooked address)", hooked_f0, false, failure_log, &mut failures);
    run_bench("vanilla ZnF1 (trampoline)", real::draw_locked_d32_zn_f1, true, failure_log, &mut failures);
    run_bench("port ZnF1 (direct)", port_f1, true, failure_log, &mut failures);
    run_bench("port ZnF1 (hooked address)", hooked_f1, true, failure_log, &mut failures);
    finish_test(test_name, failures, failure_log)
}
