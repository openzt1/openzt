//! Per-detour call counts and timings, compiled in only with the `perf-stats` feature.
//!
//! With the feature on, `#[detour_mod]` wraps every `#[detour(..)]` body in a guard from this module (see
//! `openzt-detour-macro`). Each detour gets one static [`Stat`] holding its call count, inclusive time, self
//! time (inclusive minus time spent in other instrumented detours it called) and the slowest call. The Lua
//! commands `perf_stats()`, `perf_stats_save(path)` and `perf_stats_reset()` read them.
//!
//! Timing uses `rdtsc` (a few ns per read) and converts to nanoseconds at report time against `Instant`, so it
//! needs a constant-rate TSC. Counters are updated with plain relaxed load/store pairs, not read-modify-write
//! operations, to keep the per-call cost low; a detour hit concurrently from two threads can therefore lose a
//! count. The game thread dominates, so this is accepted. The wrapper itself costs time per call (the report
//! prints the measured figure), so numbers for functions of a few tens of nanoseconds are mostly wrapper cost:
//! compare builds, do not read them as absolute times.
//!
//! A wrapped function's return value is whatever the typed inner function returns. A detour declared `-> ()`
//! whose vanilla callers happen to read a leftover `EAX` is not preserved by the wrapper.

use std::cell::Cell;
use std::fmt::Write as _;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Mutex, OnceLock};
use std::time::Instant;

/// Counters for one detour.
pub struct Stat {
    name: &'static str,
    calls: AtomicU64,
    self_ticks: AtomicU64,
    incl_ticks: AtomicU64,
    max_ticks: AtomicU64,
}

impl Stat {
    pub const fn new(name: &'static str) -> Self {
        Self {
            name,
            calls: AtomicU64::new(0),
            self_ticks: AtomicU64::new(0),
            incl_ticks: AtomicU64::new(0),
            max_ticks: AtomicU64::new(0),
        }
    }

    /// Starts timing a call; the returned guard records it when dropped.
    #[inline(always)]
    pub fn enter(&'static self) -> Guard {
        let parent_child = CHILD_TICKS.with(|c| c.replace(0));
        Guard { stat: self, parent_child, start: rdtsc() }
    }

    fn reset(&self) {
        self.calls.store(0, Ordering::Relaxed);
        self.self_ticks.store(0, Ordering::Relaxed);
        self.incl_ticks.store(0, Ordering::Relaxed);
        self.max_ticks.store(0, Ordering::Relaxed);
    }
}

pub struct Guard {
    stat: &'static Stat,
    parent_child: u64,
    start: u64,
}

impl Drop for Guard {
    #[inline(always)]
    fn drop(&mut self) {
        let elapsed = rdtsc().wrapping_sub(self.start);
        let child = CHILD_TICKS.with(|c| {
            let child = c.get();
            c.set(self.parent_child.wrapping_add(elapsed));
            child
        });
        let stat = self.stat;
        stat.calls.store(stat.calls.load(Ordering::Relaxed).wrapping_add(1), Ordering::Relaxed);
        stat.self_ticks.store(
            stat.self_ticks.load(Ordering::Relaxed).wrapping_add(elapsed.saturating_sub(child)),
            Ordering::Relaxed,
        );
        stat.incl_ticks.store(stat.incl_ticks.load(Ordering::Relaxed).wrapping_add(elapsed), Ordering::Relaxed);
        if elapsed > stat.max_ticks.load(Ordering::Relaxed) {
            stat.max_ticks.store(elapsed, Ordering::Relaxed);
        }
    }
}

thread_local! {
    /// Ticks spent in instrumented detours called by the call currently running on this thread.
    static CHILD_TICKS: Cell<u64> = const { Cell::new(0) };
}

#[inline(always)]
fn rdtsc() -> u64 {
    #[allow(unused_unsafe)]
    unsafe {
        core::arch::x86::_rdtsc()
    }
}

static REGISTRY: Mutex<Vec<&'static Stat>> = Mutex::new(Vec::new());
static CLOCK_BASE: OnceLock<(u64, Instant)> = OnceLock::new();

/// Adds a detour's stat to the list the report reads. Called from the generated `init_detours()`.
pub fn register(stat: &'static Stat) {
    CLOCK_BASE.get_or_init(|| (rdtsc(), Instant::now()));
    if let Ok(mut registry) = REGISTRY.lock() {
        registry.push(stat);
    }
}

/// Nanoseconds per TSC tick, from the time since the first registration.
fn ns_per_tick() -> Option<f64> {
    let (tsc0, t0) = CLOCK_BASE.get()?;
    let ticks = rdtsc().wrapping_sub(*tsc0);
    let ns = t0.elapsed().as_nanos() as f64;
    (ticks > 0 && ns > 1_000_000.0).then(|| ns / ticks as f64)
}

/// Cost of one enter/leave pair in nanoseconds, measured on a private stat.
fn wrapper_overhead_ns(ns_per_tick: f64) -> f64 {
    static PROBE: Stat = Stat::new("probe");
    const N: u64 = 20_000;
    let start = rdtsc();
    for _ in 0..N {
        let _guard = PROBE.enter();
    }
    let total = rdtsc().wrapping_sub(start) as f64;
    PROBE.reset();
    total / N as f64 * ns_per_tick
}

pub struct Row {
    pub name: &'static str,
    pub calls: u64,
    pub self_ns: f64,
    pub incl_ns: f64,
    pub max_ns: f64,
}

/// All detours that were called at least once, slowest by self time first.
pub fn snapshot() -> Result<(Vec<Row>, f64), String> {
    let ns_per_tick = ns_per_tick().ok_or("perf stats clock is not ready yet (no detours registered or <1 ms elapsed)")?;
    let registry = REGISTRY.lock().map_err(|_| "perf stats registry poisoned".to_string())?;
    let mut rows: Vec<Row> = registry
        .iter()
        .filter_map(|s| {
            let calls = s.calls.load(Ordering::Relaxed);
            (calls > 0).then(|| Row {
                name: s.name,
                calls,
                self_ns: s.self_ticks.load(Ordering::Relaxed) as f64 * ns_per_tick,
                incl_ns: s.incl_ticks.load(Ordering::Relaxed) as f64 * ns_per_tick,
                max_ns: s.max_ticks.load(Ordering::Relaxed) as f64 * ns_per_tick,
            })
        })
        .collect();
    rows.sort_by(|a, b| b.self_ns.total_cmp(&a.self_ns));
    Ok((rows, wrapper_overhead_ns(ns_per_tick)))
}

pub fn reset() {
    if let Ok(registry) = REGISTRY.lock() {
        for stat in registry.iter() {
            stat.reset();
        }
    }
}

/// Fixed-width table of the `top` detours by self time.
pub fn report(top: usize) -> Result<String, String> {
    let (rows, overhead) = snapshot()?;
    let total_self: f64 = rows.iter().map(|r| r.self_ns).sum();
    let mut out = String::new();
    let _ = writeln!(
        out,
        "{} detours called, {:.1} ms total self time, wrapper overhead ~{:.0} ns per call (included in the figures)",
        rows.len(),
        total_self / 1e6,
        overhead
    );
    let _ = writeln!(
        out,
        "{:>10} {:>10} {:>9} {:>10} {:>9} {:>10}  function",
        "calls", "self ms", "self ns", "incl ms", "incl ns", "max us"
    );
    for r in rows.iter().take(top) {
        let _ = writeln!(
            out,
            "{:>10} {:>10.2} {:>9.0} {:>10.2} {:>9.0} {:>10.1}  {}",
            r.calls,
            r.self_ns / 1e6,
            r.self_ns / r.calls as f64,
            r.incl_ns / 1e6,
            r.incl_ns / r.calls as f64,
            r.max_ns / 1e3,
            r.name
        );
    }
    Ok(out)
}

/// Every called detour as CSV (`function,calls,self_ns,incl_ns,max_ns`) for diffing between builds.
pub fn csv() -> Result<String, String> {
    let (rows, _) = snapshot()?;
    let mut out = String::from("function,calls,self_ns,incl_ns,max_ns\n");
    for r in &rows {
        let _ = writeln!(out, "{},{},{:.0},{:.0},{:.0}", r.name, r.calls, r.self_ns, r.incl_ns, r.max_ns);
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use super::*;

    static OUTER: Stat = Stat::new("outer");
    static INNER: Stat = Stat::new("inner");

    fn spin(iterations: u32) -> u32 {
        let mut acc = 0u32;
        for i in 0..iterations {
            acc = acc.wrapping_add(std::hint::black_box(i));
        }
        acc
    }

    fn inner() -> u32 {
        let _guard = INNER.enter();
        spin(2_000_000)
    }

    fn outer() -> u32 {
        let _guard = OUTER.enter();
        spin(1_000_000).wrapping_add(inner())
    }

    #[test]
    fn nested_self_time_excludes_callees() {
        register(&OUTER);
        register(&INNER);
        OUTER.reset();
        INNER.reset();
        for _ in 0..5 {
            std::hint::black_box(outer());
        }
        assert_eq!(OUTER.calls.load(Ordering::Relaxed), 5);
        assert_eq!(INNER.calls.load(Ordering::Relaxed), 5);
        let outer_self = OUTER.self_ticks.load(Ordering::Relaxed);
        let outer_incl = OUTER.incl_ticks.load(Ordering::Relaxed);
        let inner_incl = INNER.incl_ticks.load(Ordering::Relaxed);
        // outer's inclusive time is its own work plus inner's inclusive time
        let sum = outer_self + inner_incl;
        let diff = sum.abs_diff(outer_incl);
        assert!(diff * 100 <= outer_incl, "self {outer_self} + inner incl {inner_incl} != outer incl {outer_incl}");
        assert!(inner_incl > outer_self, "inner does twice the work of outer's own part");
    }

    #[test]
    fn report_and_csv_list_called_detours() {
        register(&OUTER);
        OUTER.reset();
        std::thread::sleep(std::time::Duration::from_millis(5));
        std::hint::black_box(outer());
        let report = report(10).unwrap();
        assert!(report.contains("outer"), "{report}");
        assert!(csv().unwrap().starts_with("function,calls,self_ns,incl_ns,max_ns\n"));
    }
}
