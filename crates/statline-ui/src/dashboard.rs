//! The companion window: left navigation plus a page per module.
//!
//! The window is a normal framed Windows app; content is drawn into a DIB and
//! blitted. The nav is pure layout plus hit-testing, so the page logic is
//! testable without a window.

use crate::flyout::{Focus, Model, Row, core_grid, core_grid_height, sparkline};
use statline_core::{APP_NAME, VERSION};
use statline_render::{Align, Palette, Surface, TextEngine, TextStyle, radius, space};

/// Pages in nav order. Settings and About are the last two nav items.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Page {
    #[default]
    Dashboard,
    Cpu,
    Memory,
    Gpu,
    Disk,
    Network,
    Battery,
    System,
    About,
}

impl Page {
    pub const ALL: [Self; 8] = [
        Self::Dashboard,
        Self::Cpu,
        Self::Memory,
        Self::Gpu,
        Self::Disk,
        Self::Network,
        Self::Battery,
        Self::System,
    ];

    pub fn index(self) -> usize {
        match self {
            Self::Dashboard => 0,
            Self::Cpu => 1,
            Self::Memory => 2,
            Self::Gpu => 3,
            Self::Disk => 4,
            Self::Network => 5,
            Self::Battery => 6,
            Self::System => 7,
            Self::About => ABOUT_ITEM,
        }
    }

    pub fn from_index(index: usize) -> Option<Self> {
        if index == ABOUT_ITEM {
            return Some(Self::About);
        }
        if index == SETTINGS_ITEM {
            return None;
        }
        Self::ALL.get(index).copied()
    }

    pub fn label(self) -> &'static str {
        match self {
            Self::Dashboard => "Dashboard",
            Self::Cpu => "CPU",
            Self::Memory => "Memory",
            Self::Gpu => "GPU",
            Self::Disk => "Disk",
            Self::Network => "Network",
            Self::Battery => "Battery",
            Self::System => "System",
            Self::About => "About",
        }
    }

    /// Focus passed to the flyout model builder for this page.
    pub fn focus(self) -> Option<Focus> {
        match self {
            Self::Dashboard | Self::About => None,
            Self::Cpu => Some(Focus::Cpu),
            Self::Memory => Some(Focus::Memory),
            Self::Gpu => Some(Focus::Gpu),
            Self::Disk => Some(Focus::Disk),
            Self::Network => Some(Focus::Network),
            Self::Battery => Some(Focus::Battery),
            Self::System => Some(Focus::System),
        }
    }
}

pub const NAV_ITEMS: [&str; 10] = [
    "Dashboard",
    "CPU",
    "Memory",
    "GPU",
    "Disk",
    "Network",
    "Battery",
    "System",
    "Settings",
    "About",
];
pub const SETTINGS_ITEM: usize = 8;
pub const ABOUT_ITEM: usize = 9;

pub const NAV_WIDTH: f32 = 168.0;
const NAV_TOP: f32 = 96.0;
const NAV_ITEM: f32 = 34.0;
/// The companion window's design height. The renderer fills the client area.
pub const DESIGN_HEIGHT: f32 = 680.0;

pub fn width(scale: f32) -> f32 {
    860.0 * scale
}

pub fn height(_model: &Model, scale: f32) -> f32 {
    DESIGN_HEIGHT * scale
}

/// Nav item under a client-space point, if any.
pub fn nav_at(x: f32, y: f32, scale: f32) -> Option<usize> {
    if x < 0.0 || x > NAV_WIDTH * scale {
        return None;
    }
    let top = NAV_TOP * scale;
    if y < top {
        return None;
    }
    let index = ((y - top) / (NAV_ITEM * scale)) as usize;
    (index < NAV_ITEMS.len()).then_some(index)
}

fn nav_style(scale: f32) -> TextStyle {
    TextStyle::new(11.0 * scale, true, Align::Left)
}

fn label_style(scale: f32) -> TextStyle {
    TextStyle::new(10.0 * scale, true, Align::Left)
}

fn value_style(scale: f32) -> TextStyle {
    TextStyle::new(22.0 * scale, true, Align::Right)
}

fn detail_style(scale: f32) -> TextStyle {
    TextStyle::new(10.0 * scale, false, Align::Left)
}

fn title_style(scale: f32) -> TextStyle {
    TextStyle::new(18.0 * scale, true, Align::Left)
}

fn footer_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Left)
}

fn process_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Left)
}

fn process_value_style(scale: f32) -> TextStyle {
    TextStyle::new(9.5 * scale, false, Align::Right)
}

fn draw_nav(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    page: Page,
    scale: f32,
    height: f32,
) {
    let nav_width = NAV_WIDTH * scale;
    let pad = 20.0 * scale;

    // A faintly different panel behind the nav separates it without a hard box.
    surface.fill_rect(
        0.0,
        0.0,
        nav_width,
        height,
        palette.hover.scaled_alpha(0.22),
    );

    let _ = text.draw(
        surface,
        "Statline",
        (pad, 22.0 * scale, nav_width - pad * 2.0, 24.0 * scale),
        title_style(scale),
        palette.text_primary,
    );
    let _ = text.draw(
        surface,
        "System monitor",
        (pad, 48.0 * scale, nav_width - pad * 2.0, 14.0 * scale),
        footer_style(scale),
        palette.text_muted,
    );

    // A small live chip keeps the shell feeling alive without animating.
    surface.fill_round_rect(
        pad,
        67.0 * scale,
        48.0 * scale,
        16.0 * scale,
        8.0 * scale,
        palette.accent.scaled_alpha(0.16),
    );
    surface.fill_round_rect(
        pad + 8.0 * scale,
        73.0 * scale,
        4.0 * scale,
        4.0 * scale,
        2.0 * scale,
        palette.accent,
    );
    let _ = text.draw(
        surface,
        "LIVE",
        (
            pad + 17.0 * scale,
            68.0 * scale,
            nav_width - pad * 2.0,
            12.0 * scale,
        ),
        TextStyle::new(7.5 * scale, true, Align::Left),
        palette.accent,
    );

    let selected = page.index();
    for (index, label) in NAV_ITEMS.iter().enumerate() {
        let y = NAV_TOP * scale + index as f32 * NAV_ITEM * scale;
        let active = index == selected;
        if active {
            surface.fill_round_rect(
                12.0 * scale,
                y + 2.0 * scale,
                nav_width - 24.0 * scale,
                28.0 * scale,
                radius::HOVER * scale,
                palette.hover,
            );
            surface.fill_round_rect(
                8.0 * scale,
                y + 9.0 * scale,
                2.0 * scale,
                14.0 * scale,
                1.0 * scale,
                palette.accent,
            );
        }
        let _ = text.draw(
            surface,
            label,
            (pad, y + 8.0 * scale, nav_width - pad * 2.0, 16.0 * scale),
            nav_style(scale),
            if active {
                palette.accent
            } else {
                palette.text_primary
            },
        );
    }

    surface.fill_rect(nav_width, 0.0, 1.0, height, palette.surface_border);
}

/// Page title and snapshot header. Returns the y coordinate for content.
#[allow(clippy::too_many_arguments)]
fn draw_header(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    title: &str,
    subtitle: &str,
    x: f32,
    width: f32,
    scale: f32,
) -> f32 {
    let _ = text.draw(
        surface,
        title,
        (x, 22.0 * scale, width, 28.0 * scale),
        title_style(scale),
        palette.text_primary,
    );
    let _ = text.draw(
        surface,
        subtitle,
        (x, 52.0 * scale, width, 14.0 * scale),
        footer_style(scale),
        palette.text_muted,
    );
    // A short accent rule gives the page a clear masthead without a box.
    surface.fill_round_rect(
        x,
        73.0 * scale,
        28.0 * scale,
        2.0 * scale,
        1.0 * scale,
        palette.accent,
    );
    88.0 * scale
}

fn card(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    row: &Row,
    scale: f32,
    (x, y, w, h): (f32, f32, f32, f32),
) {
    let pad = space::XL * scale;
    surface.fill_round_rect(x, y, w, h, radius::CARD * scale, palette.surface_border);
    surface.fill_round_rect(
        x + scale,
        y + scale,
        w - 2.0 * scale,
        h - 2.0 * scale,
        (radius::CARD * scale - scale).max(1.0),
        palette.hover,
    );

    let _ = text.draw(
        surface,
        row.label,
        (x + pad, y + pad * 0.7, w - pad * 2.0, 16.0 * scale),
        label_style(scale),
        palette.text_muted,
    );
    let _ = text.draw(
        surface,
        &row.value,
        (x + w / 2.0, y + pad * 0.55, w / 2.0 - pad, 28.0 * scale),
        value_style(scale),
        row.value_color(palette),
    );
    let _ = text.draw(
        surface,
        &row.detail,
        (x + pad, y + 38.0 * scale, w - pad * 2.0, 16.0 * scale),
        detail_style(scale),
        palette.text_muted,
    );

    if !row.series.is_empty() {
        sparkline(
            surface,
            &row.series,
            (x + pad, y + h - 20.0 * scale, w - pad * 2.0, 14.0 * scale),
            row.chart_color(palette).scaled_alpha(0.75),
            palette.meter_track,
        );
    }
}

/// Two-column process table. Returns the y coordinate after the table.
#[allow(clippy::too_many_arguments)]
fn draw_processes(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    processes: &[crate::flyout::ProcessRow],
    x: f32,
    width: f32,
    mut y: f32,
    scale: f32,
    limit: usize,
    heading: &str,
) -> f32 {
    let count = processes.len().min(limit);
    if count == 0 {
        return y;
    }

    surface.fill_rect(x, y, width, 1.0, palette.surface_border);
    y += 1.0 + space::S * scale;
    let _ = text.draw(
        surface,
        heading,
        (x, y, width, 14.0 * scale),
        label_style(scale),
        palette.text_muted,
    );
    y += 16.0 * scale + space::XXS * scale;

    let column_gap = space::XL * scale;
    let column_width = (width - column_gap) / 2.0;
    let memory_width = 82.0 * scale;
    let cpu_width = 42.0 * scale;
    let inner_gap = space::S * scale;
    let name_width = column_width - memory_width - cpu_width - inner_gap * 2.0;
    let lines = count.div_ceil(2);

    if lines > 1 {
        surface.fill_rect(
            x + column_width + column_gap / 2.0,
            y,
            1.0,
            lines as f32 * 20.0 * scale,
            palette.surface_border,
        );
    }

    for (index, process) in processes.iter().take(count).enumerate() {
        let column = index % 2;
        let line = index / 2;
        let column_x = x + column as f32 * (column_width + column_gap);
        let row_y = y + line as f32 * 20.0 * scale;

        let _ = text.draw(
            surface,
            &process.name,
            (column_x, row_y, name_width, 14.0 * scale),
            process_style(scale),
            palette.text_primary,
        );
        let _ = text.draw(
            surface,
            &process.cpu,
            (
                column_x + column_width - memory_width - inner_gap - cpu_width,
                row_y,
                cpu_width,
                14.0 * scale,
            ),
            process_value_style(scale),
            palette.text_primary,
        );
        let _ = text.draw(
            surface,
            &process.memory,
            (
                column_x + column_width - memory_width,
                row_y,
                memory_width,
                14.0 * scale,
            ),
            process_value_style(scale),
            palette.text_muted,
        );
    }

    y + lines as f32 * 20.0 * scale
}

fn draw_footer(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    x: f32,
    width: f32,
    height: f32,
    scale: f32,
) {
    let _ = text.draw(
        surface,
        "Esc closes \u{00b7} Ctrl+Tab pages \u{00b7} Ctrl+1-9 jumps to a page",
        (x, height - 28.0 * scale, width, 14.0 * scale),
        footer_style(scale),
        palette.text_muted.scaled_alpha(0.8),
    );
}

fn render_overview(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    model: &Model,
    scale: f32,
    x: f32,
    width: f32,
) {
    let y = draw_header(
        surface,
        text,
        palette,
        "Dashboard",
        &model.header,
        x,
        width,
        scale,
    );

    let gap = 14.0 * scale;
    let card_height = 84.0 * scale;
    let card_width = (width - gap) / 2.0;
    for (index, row) in model.rows.iter().enumerate() {
        let column = index % 2;
        let line = index / 2;
        card(
            surface,
            text,
            palette,
            row,
            scale,
            (
                x + column as f32 * (card_width + gap),
                y + line as f32 * (card_height + gap),
                card_width,
                card_height,
            ),
        );
    }

    let lines = model.rows.len().div_ceil(2).max(1);
    let process_y = y + lines as f32 * (card_height + gap) + gap;

    let _ = draw_processes(
        surface,
        text,
        palette,
        &model.processes,
        x,
        width,
        process_y,
        scale,
        8,
        "Top processes",
    );
}

#[allow(clippy::too_many_arguments)]
fn render_module(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    model: &Model,
    page: Page,
    scale: f32,
    x: f32,
    width: f32,
) {
    let mut y = draw_header(
        surface,
        text,
        palette,
        page.label(),
        &model.header,
        x,
        width,
        scale,
    );

    let gap = 14.0 * scale;
    let card_height = 84.0 * scale;
    for (index, row) in model.rows.iter().enumerate() {
        card(
            surface,
            text,
            palette,
            row,
            scale,
            (x, y, width, card_height),
        );
        y += card_height + gap;

        if index != 0 {
            continue;
        }

        if !model.cores.is_empty() {
            core_grid(
                surface,
                text,
                &model.cores,
                (x, y + 4.0 * scale, width),
                4,
                scale,
                palette,
            );
            y += core_grid_height(model.cores.len(), 4) * scale;
        }

        for detail in &model.detail_rows {
            let value_width = 260.0 * scale;
            let _ = text.draw(
                surface,
                &detail.label,
                (x, y, width - value_width - space::S * scale, 16.0 * scale),
                process_style(scale),
                palette.text_primary,
            );
            let _ = text.draw(
                surface,
                &detail.value,
                (x + width - value_width, y, value_width, 16.0 * scale),
                process_value_style(scale),
                palette.text_muted,
            );
            y += 20.0 * scale;
        }

        if !model.detail_rows.is_empty() || !model.cores.is_empty() {
            y += space::S * scale;
        }
    }

    let heading = match page {
        Page::Cpu => "Top processes \u{00b7} CPU",
        Page::Memory => "Top processes \u{00b7} memory",
        Page::Gpu => "Top processes \u{00b7} GPU",
        _ => "Top processes",
    };
    let _ = draw_processes(
        surface,
        text,
        palette,
        &model.processes,
        x,
        width,
        y,
        scale,
        8,
        heading,
    );
}

/// Static information about the build and its privacy posture.
fn render_about(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    scale: f32,
    x: f32,
    width: f32,
) {
    let subtitle = format!("{APP_NAME} {VERSION} \u{00b7} native Rust + Win32 + Direct2D");
    let mut y = draw_header(surface, text, palette, "About", &subtitle, x, width, scale);

    let entries = [
        (
            "Access",
            "Performance counters, DXGI, PDH, storage/device IOCTLs, SMBIOS, WLAN and registry reads. No admin.",
        ),
        (
            "Writes",
            "settings.json, window.json, logs, plus an optional HKCU Run value for startup.",
        ),
        (
            "Startup",
            "Optional, per-user, no elevation; disable here or with --remove-startup.",
        ),
        (
            "Privacy",
            "No network access, no telemetry, no driver, no elevated helper.",
        ),
        ("Settings", "%APPDATA%\\Statline\\settings.json"),
        ("Logs", "%LOCALAPPDATA%\\Statline\\logs"),
        (
            "Licence",
            "MIT. Third-party notices and the Apache/MIT texts ship with the app.",
        ),
        (
            "Sensors",
            "ACPI, drive, battery and SMBIOS readings where Windows exposes them; missing values hide.",
        ),
        (
            "GPU sensors",
            "NVML/ADL where the driver exposes them; CPU package temperature needs a driver.",
        ),
    ];

    for (label, value) in entries {
        let _ = text.draw(
            surface,
            label,
            (x, y, width, 14.0 * scale),
            label_style(scale),
            palette.text_muted,
        );
        y += 16.0 * scale;
        let _ = text.draw(
            surface,
            value,
            (x, y, width, 14.0 * scale),
            detail_style(scale),
            palette.text_primary,
        );
        y += 22.0 * scale;
    }
}

/// Draw the window. `page` selects the content; the nav is always drawn.
pub fn render(
    surface: &mut Surface,
    text: &mut TextEngine,
    palette: &Palette,
    model: &Model,
    scale: f32,
    page: Page,
) -> (f32, f32) {
    let width = width(scale);
    let height = surface.height() as f32;

    surface.begin(palette.surface);
    draw_nav(surface, text, palette, page, scale, height);

    let content_x = (NAV_WIDTH + 28.0) * scale;
    let content_width = width - content_x - 28.0 * scale;

    match page {
        Page::Dashboard => render_overview(
            surface,
            text,
            palette,
            model,
            scale,
            content_x,
            content_width,
        ),
        Page::About => render_about(surface, text, palette, scale, content_x, content_width),
        _ => render_module(
            surface,
            text,
            palette,
            model,
            page,
            scale,
            content_x,
            content_width,
        ),
    }

    draw_footer(
        surface,
        text,
        palette,
        content_x,
        content_width,
        height,
        scale,
    );

    surface.end();
    (width, height)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::flyout::{ProcessRow, Row};

    fn model() -> Model {
        Model {
            header: String::from("Up 1h"),
            rows: vec![Row {
                label: "CPU",
                value: String::from("1%"),
                detail: String::new(),
                series: Vec::new(),
                ..Row::default()
            }],
            processes: vec![ProcessRow::default()],
            ..Model::default()
        }
    }

    #[test]
    fn pages_map_to_nav_order_and_focus() {
        assert_eq!(Page::Dashboard.index(), 0);
        assert_eq!(Page::Cpu.index(), 1);
        assert_eq!(Page::System.index(), 7);
        assert_eq!(Page::About.index(), ABOUT_ITEM);
        assert_eq!(Page::from_index(0), Some(Page::Dashboard));
        assert_eq!(Page::from_index(SETTINGS_ITEM), None);
        assert_eq!(Page::from_index(ABOUT_ITEM), Some(Page::About));
        assert_eq!(Page::Cpu.focus(), Some(Focus::Cpu));
        assert_eq!(Page::System.focus(), Some(Focus::System));
        assert_eq!(Page::Dashboard.focus(), None);
        assert_eq!(Page::About.focus(), None);
    }

    #[test]
    fn nav_hit_tests_every_item_and_rejects_outside() {
        let scale = 1.0;
        for index in 0..NAV_ITEMS.len() {
            let y = NAV_TOP + index as f32 * NAV_ITEM + 1.0;
            assert_eq!(nav_at(20.0, y, scale), Some(index), "item {index}");
        }
        assert_eq!(nav_at(NAV_WIDTH + 1.0, NAV_TOP + 1.0, scale), None);
        assert_eq!(nav_at(20.0, NAV_TOP - 1.0, scale), None);
        assert_eq!(
            nav_at(
                20.0,
                NAV_TOP + NAV_ITEMS.len() as f32 * NAV_ITEM + 1.0,
                scale
            ),
            None
        );
    }

    #[test]
    fn nav_hit_testing_scales_with_dpi() {
        let scale = 1.25;
        assert_eq!(
            nav_at(20.0 * scale, (NAV_TOP + 2.0) * scale, scale),
            Some(0)
        );
        assert_eq!(
            nav_at(20.0 * scale, (NAV_TOP + NAV_ITEM + 1.0) * scale, scale),
            Some(1)
        );
    }

    #[test]
    fn height_is_the_design_height() {
        assert_eq!(height(&model(), 1.0), DESIGN_HEIGHT);
        assert_eq!(height(&Model::default(), 1.0), DESIGN_HEIGHT);
    }

    #[test]
    fn module_pages_carry_only_their_rows() {
        let model = model();
        assert_eq!(model.rows.len(), 1);
        assert_eq!(Page::Cpu.focus(), Some(Focus::Cpu));
    }
}
