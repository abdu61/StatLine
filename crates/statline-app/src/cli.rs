//! Command-line modes: `--help`, `--version`, `--probe`, `--bench`.

use statline_core::units::{UnitSystem, format_bytes, format_frequency, format_percent};
use statline_core::{APP_NAME, VERSION};
use statline_metrics::Sampler;
use statline_win::console;
use std::time::{Duration, Instant};

fn label() -> String {
    format!("{APP_NAME} {VERSION}")
}

pub fn print_help() {
    console::ensure();
    let label = label();
    let lines = [
        label.as_str(),
        "",
        "Usage: statline [options]",
        "",
        "  --help, -h            Show this help",
        "  --version, -V         Show the version",
        "  --probe               Print a few samples and exit",
        "  --bench [samples]     Measure sampling cost (default 60)",
        "",
        "With no options, Statline starts in the notification area.",
    ];
    for line in lines {
        console::write_line(line);
    }
}

pub fn print_version() {
    console::ensure();
    console::write_line(&label());
}

/// Print `count` samples, one line each, then return.
pub fn probe(count: u32, interval: Duration) {
    console::ensure();
    let mut sampler = Sampler::new();

    console::write_line(&format!(
        "{} — {count} samples, {:.0} ms apart",
        label(),
        interval.as_secs_f64() * 1000.0
    ));
    console::write_line("");

    for index in 0..count {
        let snapshot = sampler.sample();

        let cpu_text = match snapshot.cpu.as_ref() {
            Some(cpu) => {
                let cores = if cpu.per_core_percent.is_empty() {
                    String::from("(warming up)")
                } else {
                    let peak = cpu.per_core_percent.iter().copied().fold(0.0_f32, f32::max);
                    format!(
                        "{} cores, peak {}",
                        cpu.per_core_percent.len(),
                        format_percent(peak)
                    )
                };
                let mhz = cpu.mhz.map(format_frequency).unwrap_or_default();
                format!(
                    "cpu {:>4}  {mhz:>9}  {cores}",
                    format_percent(cpu.total_percent)
                )
            }
            None => String::from("cpu  n/a"),
        };

        let memory_text = match snapshot.memory.as_ref() {
            Some(memory) => format!(
                "mem {} / {} ({})",
                format_bytes(memory.used_bytes, UnitSystem::Binary),
                format_bytes(memory.total_bytes, UnitSystem::Binary),
                format_percent(memory.percent())
            ),
            None => String::from("mem  n/a"),
        };

        let system_text = snapshot
            .system
            .as_ref()
            .map(|system| format!("procs {}", system.processes))
            .unwrap_or_default();

        console::write_line(&format!(
            "#{:<2} {cpu_text}  {memory_text}  {system_text}",
            index + 1
        ));

        if index + 1 < count {
            std::thread::sleep(interval);
        }
    }
}

/// Measure the cost of one sample, repeatedly.
pub fn bench(count: u32, interval: Duration) {
    console::ensure();
    let mut sampler = Sampler::new();

    // Warm up: the first sample allocates the per-core buffers.
    let _ = sampler.sample();

    let mut durations = Vec::with_capacity(count as usize);
    for index in 0..count {
        let started = Instant::now();
        let _snapshot = sampler.sample();
        durations.push(started.elapsed());

        if index + 1 < count {
            std::thread::sleep(interval);
        }
    }

    durations.sort_unstable();
    let total: Duration = durations.iter().sum();
    let mean = total.as_secs_f64() * 1000.0 / durations.len().max(1) as f64;
    let p95_index =
        ((durations.len() as f64 * 0.95) as usize).min(durations.len().saturating_sub(1));
    let p95 = durations
        .get(p95_index)
        .copied()
        .unwrap_or_default()
        .as_secs_f64()
        * 1000.0;
    let max = durations.last().copied().unwrap_or_default().as_secs_f64() * 1000.0;

    let memory = statline_win::metrics::current_process_memory();

    console::write_line(&format!("{} — bench", label()));
    console::write_line(&format!("samples      {count}"));
    console::write_line(&format!(
        "interval     {:.0} ms",
        interval.as_secs_f64() * 1000.0
    ));
    console::write_line(&format!("mean         {mean:.3} ms"));
    console::write_line(&format!("p95          {p95:.3} ms"));
    console::write_line(&format!("max          {max:.3} ms"));

    if let Some(memory) = memory {
        console::write_line(&format!(
            "working set  {:.1} MB",
            memory.working_set_bytes as f64 / (1024.0 * 1024.0)
        ));
        console::write_line(&format!(
            "private      {:.1} MB",
            memory.private_bytes as f64 / (1024.0 * 1024.0)
        ));
    }

    console::write_line("");
    console::write_line("budget: p95 < 1.500 ms, working set < 30 MB");
    console::write_line(if p95 < 1.5 {
        "result: within budget"
    } else {
        "result: OVER BUDGET"
    });
}
