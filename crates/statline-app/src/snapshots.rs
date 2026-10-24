//! Headless rendering of every surface to PNG.
//!
//! `--render-snapshots <dir>` exists because the strip is a layered window: on
//! some display drivers screen capture cannot see it. Rendering the same drawing
//! code into a bitmap is the only reliable way to review the design here.

use statline_core::model::{
    BatterySample, CpuSample, DiskSample, GpuSample, MemorySample, NetworkSample, ProcessEntry,
    ProcessSample, Snapshot, SystemSample, ThermalSample, VolumeSample,
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
            mhz: Some(2400),
        }),
        memory: Some(MemorySample {
            used_bytes: 9_800_000_000,
            total_bytes: 17_180_000_000,
            commit_bytes: 11_200_000_000,
            commit_limit_bytes: 20_000_000_000,
            cached_bytes: 3_100_000_000,
        }),
        gpu: Some(GpuSample {
            utilization_percent: 8.0,
            vram_used_bytes: 1_100_000_000,
            vram_total_bytes: 6_000_000_000,
            ..Default::default()
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
        }),
        network: Some(NetworkSample {
            rx_bytes_per_sec: 1_240_000.0,
            tx_bytes_per_sec: 84_000.0,
            ..Default::default()
        }),
        system: Some(SystemSample {
            uptime_secs: 90_061,
            processes: 248,
            threads: 3_412,
            handles: 112_908,
        }),
        thermal: Some(ThermalSample { system_c: 54.0 }),
        battery: Some(BatterySample {
            percent: 98.0,
            on_ac: true,
            designed_mwh: Some(50_000),
            full_mwh: Some(46_000),
            temperature_c: Some(31.0),
            ..Default::default()
        }),
        processes: Some(ProcessSample {
            total: 248,
            top: vec![
                ProcessEntry {
                    pid: 4820,
                    name: String::from("Code.exe"),
                    cpu_percent: Some(4.2),
                    working_set_bytes: 640_000_000,
                },
                ProcessEntry {
                    pid: 7312,
                    name: String::from("msedge.exe"),
                    cpu_percent: Some(1.8),
                    working_set_bytes: 388_000_000,
                },
                ProcessEntry {
                    pid: 1180,
                    name: String::from("explorer.exe"),
                    cpu_percent: Some(0.9),
                    working_set_bytes: 142_000_000,
                },
                ProcessEntry {
                    pid: 9924,
                    name: String::from("statline.exe"),
                    cpu_percent: Some(0.4),
                    working_set_bytes: 30_000_000,
                },
                ProcessEntry {
                    pid: 644,
                    name: String::from("dwm.exe"),
                    cpu_percent: Some(0.3),
                    working_set_bytes: 96_000_000,
                },
                ProcessEntry {
                    pid: 2860,
                    name: String::from("Teams.exe"),
                    cpu_percent: Some(0.2),
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
        flyout::render(&mut surface, &mut renderer.text, &palette, &model, scale);
        let path = dir.join("flyout-dark-125.png");
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
        flyout::render(&mut surface, &mut renderer.text, &palette, &model, scale);
        let path = dir.join("flyout-dark-125-memory.png");
        surface.save_png_over(&path, Color::rgb(0x1F, 0x1F, 0x1F))?;
        written.push(path);
    }
    // Companion window at 100 %.
    {
        use statline_ui::dashboard;
        use statline_ui::flyout::{self, History};

        let mut history = History::default();
        for index in 0..48 {
            history.cpu.push(24.0 + 10.0 * (index as f32 / 6.0).sin());
            history.memory.push(58.0 + 4.0 * (index as f32 / 9.0).sin());
            history.gpu.push(7.0 + 5.0 * (index as f32 / 4.0).cos());
        }

        let model = flyout::model(&snapshot, &history);
        let scale = 1.0_f32;
        let width = dashboard::width(scale).ceil() as u32;
        let height = dashboard::height(&model, scale).ceil() as u32;
        let palette = Palette::for_theme(true, Color::rgb(0x4C, 0xC2, 0xFF));

        let mut surface = renderer.surface(width, height)?;
        dashboard::render(&mut surface, &mut renderer.text, &palette, &model, scale);
        let path = dir.join("dashboard-dark-100.png");
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
                    label: "Strip placement",
                    value: String::from("Right"),
                },
                settings_ui::Row {
                    label: "Start with Windows",
                    value: String::from("Off"),
                },
            ],
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
    }
    for path in written {
        println!("{}", path.display());
    }
    Ok(())
}
