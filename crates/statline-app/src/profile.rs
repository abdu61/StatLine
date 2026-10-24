//! Per-provider timing, for finding where a sampling budget is being spent.

use statline_metrics::Sampler;
use statline_win::console;
use statline_win::metrics;
use statline_win::metrics::disk::DiskCounters;
use std::hint::black_box;
use std::time::Instant;

fn time<F: FnMut()>(name: &str, iterations: u32, mut action: F) {
    for _ in 0..3 {
        action();
    }
    let started = Instant::now();
    for _ in 0..iterations {
        action();
    }
    let per_call_us = started.elapsed().as_secs_f64() * 1_000_000.0 / iterations.max(1) as f64;
    console::write_line(&format!("{name:<28} {per_call_us:8.1} us"));
}

pub fn run(iterations: u32) {
    console::ensure();
    console::write_line(&format!("provider profile - {iterations} iterations each"));
    console::write_line("");

    let disk = DiskCounters::open();
    let interfaces = metrics::net::interfaces();

    console::write_line(&format!("interfaces found: {}", interfaces.len()));
    time("GetSystemTimes", iterations, || {
        black_box(metrics::system_times());
    });
    time("per-core times", iterations, || {
        black_box(metrics::per_core_times());
    });
    time("current_mhz", iterations, || {
        black_box(metrics::current_mhz());
    });
    time("memory_status", iterations, || {
        black_box(metrics::memory_status());
    });
    time("performance_counters", iterations, || {
        black_box(metrics::performance_counters());
    });
    time("interfaces (GetIfTable2)", iterations, || {
        black_box(metrics::net::interfaces());
    });
    let interfaces_for_counters = interfaces.clone();
    time("counters (GetIfEntry2 xN)", iterations, || {
        for interface in &interfaces_for_counters {
            black_box(metrics::net::counters(interface));
        }
    });
    if let Some(disk) = disk.as_ref() {
        time("disk sample (PDH)", iterations, || {
            black_box(disk.sample());
        });
    }
    time("volumes", iterations, || {
        black_box(metrics::disk::volumes());
    });

    let mut sampler = Sampler::new();
    let _ = sampler.sample();
    time("full Sampler::sample", iterations, || {
        black_box(sampler.sample());
    });
}
