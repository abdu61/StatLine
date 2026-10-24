//! Command-line modes: `--help`, `--version`, `--probe`, `--bench`, `--profile`.

use statline_core::units::{
    UnitSystem, format_bytes, format_frequency, format_percent, format_percent_precise,
    format_rate, format_temperature,
};
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
        "  --profile [iters]     Time each provider in isolation (default 20)",
        "  --tray-only           Do not attach the taskbar strip",
        "  --render-snapshots D  Write PNGs of every surface to directory D",
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

/// Print `count` samples, one block each, then return.
pub fn probe(count: u32, interval: Duration) {
    console::ensure();
    let mut sampler = Sampler::new();
    // The probe is the diagnostic mode; include the detail providers it would
    // otherwise only collect while a detail surface is visible.
    sampler.set_detail_surfaces(true);
    let mut last = None;

    console::write_line(&format!(
        "{} - {count} samples, {:.0} ms apart",
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
                    "cpu {:>4} {mhz:>9}  {cores}  avg {} / {} / {}",
                    format_percent(cpu.total_percent),
                    format_percent(cpu.average_1m_percent),
                    format_percent(cpu.average_5m_percent),
                    format_percent(cpu.average_15m_percent)
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

        let disk_text = match snapshot.disk.as_ref() {
            Some(disk) => {
                let volume = disk.volumes.first();
                let used = volume
                    .map(|volume| {
                        format!(
                            " {}/{}",
                            format_bytes(volume.used_bytes(), UnitSystem::Binary),
                            format_bytes(volume.total_bytes, UnitSystem::Binary)
                        )
                    })
                    .unwrap_or_default();
                let temperature = volume
                    .and_then(|volume| volume.temperature_c)
                    .map(|celsius| format!("  {}", format_temperature(celsius)))
                    .unwrap_or_default();
                format!(
                    "disk R {} W {}{used}{temperature}",
                    format_rate(disk.read_bytes_per_sec, UnitSystem::Binary),
                    format_rate(disk.write_bytes_per_sec, UnitSystem::Binary)
                )
            }
            None => String::from("disk n/a"),
        };

        let net_text = match snapshot.network.as_ref() {
            Some(network) => format!(
                "net down {} up {}",
                format_rate(network.rx_bytes_per_sec, UnitSystem::Binary),
                format_rate(network.tx_bytes_per_sec, UnitSystem::Binary)
            ),
            None => String::from("net  n/a"),
        };

        let system_text = snapshot
            .system
            .as_ref()
            .map(|system| format!("procs {}", system.processes))
            .unwrap_or_default();

        let thermal_text = snapshot
            .thermal
            .as_ref()
            .map(|thermal| format!("  thermal {}", format_temperature(thermal.system_c)))
            .unwrap_or_default();

        let gpu_text = match snapshot.gpu.as_ref() {
            Some(gpu) => format!(
                "gpu {}  vram {} / {}",
                format_percent(gpu.utilization_percent),
                format_bytes(gpu.vram_used_bytes, UnitSystem::Binary),
                format_bytes(gpu.vram_total_bytes, UnitSystem::Binary)
            ),
            None => String::from("gpu  n/a"),
        };

        let battery_text = match snapshot.battery.as_ref() {
            Some(battery) => {
                let health = battery
                    .health_percent()
                    .map(|percent| format!("  {percent:.0}% health"))
                    .unwrap_or_default();
                let temperature = battery
                    .temperature_c
                    .map(|celsius| format!("  {}", format_temperature(celsius)))
                    .unwrap_or_default();
                let cycles = battery
                    .cycle_count
                    .map(|count| format!("  {count} cycles"))
                    .unwrap_or_default();
                format!(
                    "  battery {} {}{health}{temperature}{cycles}",
                    format_percent(battery.percent),
                    if battery.on_ac { "on AC" } else { "on battery" }
                )
            }
            None => String::new(),
        };

        console::write_line(&format!("#{:<2} {cpu_text}", index + 1));
        console::write_line(&format!("      {memory_text}"));
        console::write_line(&format!("      {disk_text}"));
        console::write_line(&format!("      {gpu_text}"));
        console::write_line(&format!(
            "      {net_text}  {system_text}{battery_text}{thermal_text}"
        ));
        if index + 1 < count {
            console::write_line("");
            std::thread::sleep(interval);
        }
        last = Some(snapshot);
    }

    if let Some(processes) = last
        .as_ref()
        .and_then(|snapshot| snapshot.processes.as_ref())
    {
        console::write_line("");
        console::write_line(&format!(
            "top processes ({} total, CPU share of all cores)",
            processes.total
        ));
        for entry in processes.top.iter().take(5) {
            let cpu = entry
                .cpu_percent
                .map(format_percent_precise)
                .unwrap_or_else(|| String::from("--"));
            console::write_line(&format!(
                "  {:<28} {:>5}  {}",
                entry.name,
                cpu,
                format_bytes(entry.working_set_bytes, UnitSystem::Binary)
            ));
        }
    }
}

/// Measure the cost of one sample, repeatedly.
pub fn bench(count: u32, interval: Duration) {
    console::ensure();
    let mut sampler = Sampler::new();

    // Warm up: the first sample allocates the per-core buffers and opens PDH.
    let _ = sampler.sample();

    let cpu_start = statline_win::metrics::process_cpu_time().unwrap_or_default();
    let mut durations = Vec::with_capacity(count as usize);
    let mut cpu_samples = Vec::with_capacity(count as usize);
    for index in 0..count {
        let before = statline_win::metrics::process_cpu_time().unwrap_or_default();
        let started = Instant::now();
        let _snapshot = sampler.sample();
        durations.push(started.elapsed());
        let after = statline_win::metrics::process_cpu_time().unwrap_or_default();
        cpu_samples.push(after.saturating_sub(before));

        if index + 1 < count {
            std::thread::sleep(interval);
        }
    }

    let cpu_total = statline_win::metrics::process_cpu_time()
        .unwrap_or_default()
        .saturating_sub(cpu_start);
    let cpu_mean_ms = cpu_total.as_secs_f64() * 1000.0 / count.max(1) as f64;
    cpu_samples.sort_unstable();
    let cpu_sample_min = cpu_samples.first().copied().unwrap_or_default();
    let cpu_sample_max = cpu_samples.last().copied().unwrap_or_default();
    let cpu_sample_sum: Duration = cpu_samples.iter().sum();
    let cpu_sample_mean = cpu_sample_sum.as_secs_f64() * 1000.0 / cpu_samples.len().max(1) as f64;

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

    console::write_line(&format!("{} - bench", label()));
    console::write_line(&format!("samples      {count}"));
    console::write_line(&format!(
        "interval     {:.0} ms",
        interval.as_secs_f64() * 1000.0
    ));
    console::write_line(&format!("mean         {mean:.3} ms"));
    console::write_line(&format!("p95          {p95:.3} ms"));
    console::write_line(&format!("max          {max:.3} ms"));
    console::write_line(&format!(
        "cpu/sample   min {:.3}  mean {:.3}  max {:.3} ms",
        cpu_sample_min.as_secs_f64() * 1000.0,
        cpu_sample_mean,
        cpu_sample_max.as_secs_f64() * 1000.0
    ));

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

    let interval_ms = interval.as_secs_f64() * 1000.0;
    let core_share = if interval_ms > 0.0 {
        cpu_mean_ms / interval_ms * 100.0
    } else {
        0.0
    };
    console::write_line(&format!(
        "cpu mean     {cpu_mean_ms:.3} ms  ({core_share:.3}% of one core)"
    ));
    console::write_line("");
    console::write_line("development gate: cpu mean < 2.000 ms per sample, working set < 30 MB");
    console::write_line(if cpu_mean_ms < 2.0 {
        "result: within budget"
    } else {
        "result: OVER BUDGET"
    });
}

/// Print one line from a non-UI command (startup, diagnostics).
pub fn print_line(text: &str) {
    console::ensure();
    console::write_line(text);
}
