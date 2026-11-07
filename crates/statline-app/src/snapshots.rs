//! Headless rendering of every surface to PNG.
//!
//! `--render-snapshots <dir>` exists because the strip is a layered window: on
//! some display drivers screen capture cannot see it. Rendering the same drawing
//! code into a bitmap is the only reliable way to review the design here.

use statline_core::model::{
    AdapterSample, BatterySample, CpuSample, DiskSample, GpuSample, MemoryModuleInfo, MemorySample,
    NetworkSample, PhysicalDiskSample, ProcessEntry, ProcessSample, Snapshot, SystemSample,
    ThermalSample, VolumeSample,
};
use statline_render::{Color, Palette, Renderer};
use statline_ui::{render, strip};
use std::path::Path;

/// A plausible, fixed snapshot so output is reproducible between runs.
fn synthetic() -> Snapshot {
    Snapshot {
        taken_at: Some(std::time::SystemTime::now()),
        cpu: Some(CpuSample {
            total_percent: 23.0,
            per_core_percent: vec![12.0, 44.0, 8.0, 31.0, 5.0, 61.0, 17.0, 9.0],
            per_core_mhz: vec![3200, 3400, 3100, 3300, 3200, 3500, 3100, 3000],
            mhz: Some(2400),
            name: Some(String::from("AMD Ryzen 7 7840U w/ Radeon 780M Graphics")),
            base_mhz: Some(3300),
            average_1m_percent: 21.0,
            average_5m_percent: 19.0,
            average_15m_percent: 17.0,
        }),
        memory: Some(MemorySample {
            used_bytes: 9_800_000_000,
            total_bytes: 17_180_000_000,
            commit_bytes: 11_200_000_000,
            commit_limit_bytes: 20_000_000_000,
            cached_bytes: 3_100_000_000,
            swap_used_bytes: 1_200_000_000,
            swap_total_bytes: 4_000_000_000,
        }),
        gpu: Some(GpuSample {
            utilization_percent: 8.0,
            vram_used_bytes: 1_100_000_000,
            vram_total_bytes: 6_000_000_000,
            name: Some(String::from("Radeon 780M")),
            temperature_c: Some(62.0),
            fan_percent: Some(41.0),
            fan_rpm: None,
            power_w: Some(35.0),
            core_mhz: Some(1800),
            memory_mhz: Some(1200),
        }),
        disk: Some(DiskSample {
            read_bytes_per_sec: 12_400_000.0,
            write_bytes_per_sec: 3_100_000.0,
            busy_percent: 4.0,
            volumes: vec![VolumeSample {
                mount: String::from("C:"),
                label: String::from("System"),
                free_bytes: 412_000_000_000,
                total_bytes: 931_000_000_000,
                temperature_c: Some(41.0),
            }],
            disks: vec![statline_core::model::DiskActivitySample {
                name: String::from("0 C:"),
                read_bytes_per_sec: 12_400_000.0,
                write_bytes_per_sec: 3_100_000.0,
                busy_percent: 4.0,
            }],
            physical: vec![PhysicalDiskSample {
                number: 0,
                model: String::from("SAMSUNG MZVLQ512HALU-00000"),
                bus: String::from("NVMe"),
                size_bytes: 512_000_000_000,
            }],
        }),
        network: Some(NetworkSample {
            rx_bytes_per_sec: 1_240_000.0,
            tx_bytes_per_sec: 84_000.0,
            adapters: vec![AdapterSample {
                name: String::from("Wi-Fi"),
                rx_bytes_per_sec: 1_240_000.0,
                tx_bytes_per_sec: 84_000.0,
                link_speed_bps: 866_000_000,
                wireless: true,
            }],
            session_rx_bytes: 8_400_000_000,
            session_tx_bytes: 1_200_000_000,
            wifi: Some(statline_core::model::WifiSample {
                ssid: String::from("Statline-5G"),
                signal_percent: 82,
            }),
        }),
        system: Some(SystemSample {
            uptime_secs: 90_061,
            processes: 248,
            threads: 3_412,
            handles: 112_908,
            os: Some(String::from("Windows 11 Pro 24H2 (build 26100.2033)")),
            computer: Some(String::from("STATLINE-DEMO")),
            memory_modules: vec![MemoryModuleInfo {
                slot: String::from("DIMM 0"),
                bank: String::from("P0 CHANNEL A"),
                size_bytes: 16 * 1024 * 1024 * 1024,
                speed_mts: 5600,
                kind: String::from("DDR5"),
            }],
        }),
        thermal: Some(ThermalSample { system_c: 54.0 }),
        battery: Some(BatterySample {
            percent: 98.0,
            on_ac: true,
            designed_mwh: Some(50_000),
            full_mwh: Some(46_000),
            temperature_c: Some(31.0),
            cycle_count: Some(312),
            ..Default::default()
        }),
        processes: Some(ProcessSample {
            total: 248,
            top: vec![
                ProcessEntry {
                    pid: 4820,
                    name: String::from("Code.exe"),
                    threads: 12,
                    instances: 1,
                    cpu_percent: Some(4.2),
                    gpu_percent: Some(7.6),
                    working_set_bytes: 640_000_000,
                },
                ProcessEntry {
                    pid: 7312,
                    name: String::from("msedge.exe"),
                    threads: 12,
                    instances: 1,
                    cpu_percent: Some(1.8),
                    gpu_percent: Some(2.1),
                    working_set_bytes: 388_000_000,
                },
                ProcessEntry {
                    pid: 1180,
                    name: String::from("explorer.exe"),
                    threads: 12,
                    instances: 1,
                    cpu_percent: Some(0.9),
                    gpu_percent: None,
                    working_set_bytes: 142_000_000,
                },
                ProcessEntry {
                    pid: 9924,
                    name: String::from("statline.exe"),
                    threads: 12,
                    instances: 1,
                    cpu_percent: Some(0.4),
                    gpu_percent: None,
                    working_set_bytes: 30_000_000,
                },
                ProcessEntry {
                    pid: 644,
                    name: String::from("dwm.exe"),
                    threads: 12,
                    instances: 1,
                    cpu_percent: Some(0.3),
                    gpu_percent: None,
                    working_set_bytes: 96_000_000,
                },
                ProcessEntry {
                    pid: 2860,
                    name: String::from("Teams.exe"),
                    threads: 12,
                    instances: 1,
                    cpu_percent: Some(0.2),
                    gpu_percent: None,
                    working_set_bytes: 512_000_000,
                },
            ],
        }),
    }
}

/// Render the strip for every theme and scale into `dir`.
pub fn run(dir: &Path) -> Result<(), Box<dyn std::error::Error>> {
    std::fs::create_dir_all(dir)?;
    let mut renderer = Renderer::new()?;
    let snapshot = synthetic();
    let segments = strip::segments(&snapshot);

    let mut written = Vec::new();

    for (theme_name, dark) in [("dark", true), ("light", false)] {
        let palette = Palette::for_theme(dark, Color::rgb(0x4C, 0xC2, 0xFF));
        // The taskbar colour the strip is composited over for review.
        let background = if dark {
            Color::rgb(0x1F, 0x1F, 0x1F)
        } else {
            Color::rgb(0xF3, 0xF3, 0xF3)
        };

        for (scale_name, scale) in [("100", 1.0_f32), ("125", 1.25), ("150", 1.5), ("200", 2.0)] {
            let height = (48.0 * scale - 8.0 * scale).max(16.0);
            let measured = render::measure(&mut renderer.text, &segments, scale);
            let width = measured.width.ceil().max(1.0);

            // Normal state.
            {
                let mut surface = renderer.surface(width as u32, height as u32)?;
                render::render(
                    &mut surface,
                    &mut renderer.text,
                    &palette,
                    &segments,
                    None,
                    scale,
                    height,
                );
                let path = dir.join(format!("strip-{theme_name}-{scale_name}.png"));
                surface.save_png_over(&path, background)?;
                written.push(path);
            }

            // Hover on the first segment, to review the plate.
            {
                let mut surface = renderer.surface(width as u32, height as u32)?;
                render::render(
                    &mut surface,
                    &mut renderer.text,
                    &palette,
                    &segments,
                    Some(0),
                    scale,
                    height,
                );
                let path = dir.join(format!("strip-{theme_name}-{scale_name}-hover.png"));
                surface.save_png_over(&path, background)?;
                written.push(path);
            }
        }
    }

    // Also render tall, so edges and radii are easy to inspect.
    {
        let palette = Palette::dark(Color::rgb(0x4C, 0xC2, 0xFF));
        let scale = 2.0_f32;
        let measured = render::measure(&mut renderer.text, &segments, scale);
        let width = measured.width.ceil().max(1.0);
        let mut surface = renderer.surface(width as u32, 80)?;
        render::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &segments,
            Some(2),
            scale,
            80.0,
        );
        let path = dir.join("strip-dark-200-tall.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);
    }

    // Flyout at 125 % with a synthetic one-minute history.
    {
        use statline_ui::flyout::{self, History};

        let mut history = History::default();
        for index in 0..48 {
            let t = index as f32 / 48.0;
            history.cpu.push(22.0 + 14.0 * (t * 12.0).sin());
            history.memory.push(57.0 + 6.0 * t);
            history.gpu.push(6.0 + 9.0 * (t * 22.0).cos());
        }

        let model = flyout::model(&snapshot, &history);
        let scale = 1.25_f32;
        let width = flyout::width(scale).ceil() as u32;
        let height = flyout::height(&model, scale).ceil() as u32;
        let palette = Palette::for_theme(true, Color::rgb(0x4C, 0xC2, 0xFF));

        let mut surface = renderer.surface(width, height)?;
        flyout::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            false,
        );
        let path = dir.join("flyout-dark-125.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);

        // The same panel in Mica mode: slightly translucent over the taskbar.
        let mut surface = renderer.surface(width, height)?;
        flyout::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            true,
        );
        let path = dir.join("flyout-dark-125-mica.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);

        // Focused on memory, as after clicking the MEM segment.
        let model = flyout::model_focused(
            &snapshot,
            &history,
            Some(flyout::Focus::Memory),
            statline_core::units::UnitSystem::Binary,
        );
        let height = flyout::height(&model, scale).ceil() as u32;
        let mut surface = renderer.surface(width, height)?;
        flyout::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            false,
        );
        let path = dir.join("flyout-dark-125-memory.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);

        // Focused on CPU, with per-core bars and the CPU process table.
        let model = flyout::model_focused(
            &snapshot,
            &history,
            Some(flyout::Focus::Cpu),
            statline_core::units::UnitSystem::Binary,
        );
        let height = flyout::height(&model, scale).ceil() as u32;
        let mut surface = renderer.surface(width, height)?;
        flyout::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            false,
        );
        let path = dir.join("flyout-dark-125-cpu.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);

        // Focused on GPU, with the vendor adapter name and clocks.
        let model = flyout::model_focused(
            &snapshot,
            &history,
            Some(flyout::Focus::Gpu),
            statline_core::units::UnitSystem::Binary,
        );
        let height = flyout::height(&model, scale).ceil() as u32;
        let mut surface = renderer.surface(width, height)?;
        flyout::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            false,
        );
        let path = dir.join("flyout-dark-125-gpu.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);
    }
    // Companion window at 100 %: dashboard and a module page.
    {
        use statline_ui::dashboard::{self, Page};
        use statline_ui::flyout::{self, History};

        let mut history = History::default();
        for index in 0..48 {
            history.cpu.push(24.0 + 10.0 * (index as f32 / 6.0).sin());
            history.memory.push(58.0 + 4.0 * (index as f32 / 9.0).sin());
            history.gpu.push(7.0 + 5.0 * (index as f32 / 4.0).cos());
        }

        let palette = Palette::for_theme(true, Color::rgb(0x4C, 0xC2, 0xFF));
        let scale = 1.0_f32;
        let width = dashboard::width(scale).ceil() as u32;

        let model = flyout::model(&snapshot, &history);
        let height = dashboard::height(&model, scale).ceil() as u32;
        let mut surface = renderer.surface(width, height)?;
        dashboard::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            Page::Dashboard,
        );
        let path = dir.join("dashboard-dark-100.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);

        let model = flyout::model_focused(
            &snapshot,
            &history,
            Some(flyout::Focus::Cpu),
            statline_core::units::UnitSystem::Binary,
        );
        let height = dashboard::height(&model, scale).ceil() as u32;
        let mut surface = renderer.surface(width, height)?;
        dashboard::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            Page::Cpu,
        );
        let path = dir.join("dashboard-cpu-100.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);

        // GPU page, with the vendor adapter and clocks.
        let gpu_model = flyout::model_focused(
            &snapshot,
            &history,
            Some(flyout::Focus::Gpu),
            statline_core::units::UnitSystem::Binary,
        );
        let mut surface = renderer.surface(width, height)?;
        dashboard::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &gpu_model,
            scale,
            Page::Gpu,
        );
        let path = dir.join("dashboard-gpu-100.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);

        for (page, focus, file) in [
            (
                Page::Memory,
                flyout::Focus::Memory,
                "dashboard-memory-100.png",
            ),
            (Page::Disk, flyout::Focus::Disk, "dashboard-disk-100.png"),
            (
                Page::Network,
                flyout::Focus::Network,
                "dashboard-network-100.png",
            ),
            (
                Page::Battery,
                flyout::Focus::Battery,
                "dashboard-battery-100.png",
            ),
            (
                Page::System,
                flyout::Focus::System,
                "dashboard-system-100.png",
            ),
        ] {
            let model = flyout::model_focused(
                &snapshot,
                &history,
                Some(focus),
                statline_core::units::UnitSystem::Binary,
            );
            let mut surface = renderer.surface(width, height)?;
            dashboard::render(
                &mut surface,
                &mut renderer.text,
                &palette,
                &model,
                scale,
                page,
            );
            let path = dir.join(file);
            surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
            written.push(path);
        }

        // About page.
        let mut surface = renderer.surface(width, height)?;
        dashboard::render(
            &mut surface,
            &mut renderer.text,
            &palette,
            &model,
            scale,
            Page::About,
        );
        let path = dir.join("dashboard-about-100.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);
    }
    // Settings window at 100 % and 125 %.
    {
        use statline_ui::settings as settings_ui;

        let model = settings_ui::Model {
            rows: vec![
                settings_ui::Row {
                    label: "Theme",
                    value: String::from("System"),
                },
                settings_ui::Row {
                    label: "Update interval",
                    value: String::from("1000 ms"),
                },
                settings_ui::Row {
                    label: "Units",
                    value: String::from("Binary (KiB)"),
                },
                settings_ui::Row {
                    label: "Temperature",
                    value: String::from("Celsius (°C)"),
                },
                settings_ui::Row {
                    label: "Accent",
                    value: String::from("System"),
                },
                settings_ui::Row {
                    label: "Mica window effect",
                    value: String::from("Off"),
                },
                settings_ui::Row {
                    label: "Strip placement",
                    value: String::from("Right"),
                },
                settings_ui::Row {
                    label: "Start with Windows",
                    value: String::from("Off"),
                },
                settings_ui::Row {
                    label: "Strip CPU",
                    value: String::from("On"),
                },
                settings_ui::Row {
                    label: "Strip memory",
                    value: String::from("On"),
                },
                settings_ui::Row {
                    label: "Strip GPU",
                    value: String::from("On"),
                },
                settings_ui::Row {
                    label: "Strip disk",
                    value: String::from("On"),
                },
                settings_ui::Row {
                    label: "Strip network",
                    value: String::from("On"),
                },
            ],
            warning: None,
            footer: String::from("Click a row to change \u{00b7} Esc closes"),
        };
        let palette = Palette::for_theme(true, Color::rgb(0x4C, 0xC2, 0xFF));

        for (name, scale) in [("100", 1.0_f32), ("125", 1.25)] {
            let width = settings_ui::width(scale).ceil() as u32;
            let height = settings_ui::height(&model, scale).ceil() as u32;
            let mut surface = renderer.surface(width, height)?;
            settings_ui::render(&mut surface, &mut renderer.text, &palette, &model, scale);
            let path = dir.join(format!("settings-dark-{name}.png"));
            surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
            written.push(path);
        }

        // Mica on: the row reads On and the resource warning appears.
        let mut mica = model.clone();
        if let Some(row) = mica
            .rows
            .iter_mut()
            .find(|row| row.label == "Mica window effect")
        {
            row.value = String::from("On");
        }
        mica.warning = Some(String::from(
            "Mica blurs the desktop through DWM; it can cost GPU power and battery.",
        ));
        let scale = 1.25_f32;
        let width = settings_ui::width(scale).ceil() as u32;
        let height = settings_ui::height(&mica, scale).ceil() as u32;
        let mut surface = renderer.surface(width, height)?;
        settings_ui::render(&mut surface, &mut renderer.text, &palette, &mica, scale);
        let path = dir.join("settings-dark-125-mica.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);
    }
    for path in written {
        println!("{}", path.display());
    }
    Ok(())
}
