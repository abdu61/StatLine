// Console in debug builds (for `--probe`, `--bench`), no console in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod dashboard_host;
mod flyout_host;
mod profile;
mod settings_host;
mod snapshots;
mod strip_host;

use log::{LevelFilter, error, info};
use statline_config::Settings;
use statline_core::{APP_NAME, VERSION};
use statline_win::window::MessageWindow;
use statline_win::{
    HWND, LPARAM, LRESULT, WPARAM, command_id, destroy_window, dpi, is_destroy, logger, menus,
    paths, quit_message_loop, shell, theme, tray, window,
};
use std::time::Duration;

const WINDOW_CLASS: &str = "StatlineMessageWindow";
const TRAY_ICON_ID: u32 = 1;
const DEFAULT_INTERVAL_MS: u64 = 1000;

fn main() {
    if let Err(err) = run() {
        eprintln!("{APP_NAME} failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();

    if args.iter().any(|a| a == "--help" || a == "-h") {
        cli::print_help();
        return Ok(());
    }
    if args.iter().any(|a| a == "--version" || a == "-V") {
        cli::print_version();
        return Ok(());
    }
    if args.iter().any(|a| a == "--probe") {
        cli::probe(5, Duration::from_millis(DEFAULT_INTERVAL_MS));
        return Ok(());
    }
    if let Some(index) = args.iter().position(|a| a == "--render-snapshots") {
        let dir = args
            .get(index + 1)
            .cloned()
            .unwrap_or_else(|| String::from("snapshots-out"));
        snapshots::run(std::path::Path::new(&dir))?;
        return Ok(());
    }
    if let Some(index) = args.iter().position(|a| a == "--profile") {
        let count = args
            .get(index + 1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(20);
        profile::run(count);
        return Ok(());
    }
    if let Some(index) = args.iter().position(|a| a == "--bench") {
        let count = args
            .get(index + 1)
            .and_then(|value| value.parse::<u32>().ok())
            .unwrap_or(60);
        let interval_ms = args
            .get(index + 2)
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(DEFAULT_INTERVAL_MS);
        cli::bench(count, Duration::from_millis(interval_ms));
        return Ok(());
    }

    if args.iter().any(|a| a == "--install-startup") {
        match statline_win::autostart::set_enabled(true) {
            Ok(()) => cli::print_line("startup entry installed"),
            Err(error) => cli::print_line(&format!("could not install startup entry: {error}")),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--remove-startup") {
        match statline_win::autostart::set_enabled(false) {
            Ok(()) => cli::print_line("startup entry removed"),
            Err(error) => cli::print_line(&format!("could not remove startup entry: {error}")),
        }
        return Ok(());
    }
    if args.iter().any(|a| a == "--startup-status") {
        match statline_win::autostart::stored_command() {
            Some(command) => {
                let expected = std::env::current_exe().ok();
                let matches = expected
                    .as_deref()
                    .map(statline_win::autostart::points_at)
                    .unwrap_or(false);
                cli::print_line(&format!(
                    "startup: enabled {} — {command}",
                    if matches {
                        "(matches this build)"
                    } else {
                        "(stale)"
                    }
                ));
            }
            None => cli::print_line("startup: disabled"),
        }
        return Ok(());
    }

    if args.iter().any(|a| a == "--reset-settings") {
        let path = paths::settings_file();
        let window = paths::window_state_file();
        for file in [&path, &window] {
            match std::fs::remove_file(file) {
                Ok(()) => cli::print_line(&format!("removed {}", file.display())),
                Err(err) if err.kind() == std::io::ErrorKind::NotFound => {}
                Err(err) => cli::print_line(&format!("could not remove {}: {err}", file.display())),
            }
        }
        cli::print_line("settings reset to defaults");
        return Ok(());
    }

    dpi::enable_per_monitor_v2();

    let log_dir = paths::log_dir();
    if let Err(err) = logger::init(&log_dir, LevelFilter::Info) {
        eprintln!("could not open log file in {}: {err}", log_dir.display());
    }
    logger::install_panic_hook();
    info!("{APP_NAME} {VERSION} starting");

    // A second instance must not stack trays or strips.
    let Some(_instance) =
        statline_win::single_instance::InstanceGuard::acquire("Statline.SingleInstance")
    else {
        info!("another instance is already running; exiting");
        return Ok(());
    };

    let settings_path = paths::settings_file();
    let first_run = !settings_path.exists();
    let mut settings = match Settings::load_or_default(&settings_path) {
        Ok(settings) => settings,
        Err(err) => {
            error!("could not read settings: {err}");
            Settings::default()
        }
    };
    settings.normalize();
    // The Run key is the truth for startup; a settings file can outlive a manual
    // change in Task Manager or a moved executable.
    settings.general.run_at_startup = statline_win::autostart::is_enabled();
    let dark = settings
        .appearance
        .theme
        .resolve_dark(theme::is_dark_mode());

    // Heal a stale Run value (older build, moved folder, or a debug/test path).
    // This keeps login from launching a console test binary.
    if settings.general.run_at_startup {
        match statline_win::autostart::repair(&std::env::current_exe()?) {
            Ok(true) => info!("repaired the startup entry"),
            Ok(false) => {}
            Err(err) => error!("could not repair the startup entry: {err}"),
        }
    }

    let window = MessageWindow::create(WINDOW_CLASS, APP_NAME, Some(wndproc))?;
    if let Err(err) = tray::add(window.hwnd(), TRAY_ICON_ID, APP_NAME) {
        error!("failed to add tray icon: {err}");
        return Err(err.into());
    }
    info!("tray icon ready");
    if first_run {
        // One-time, unobtrusive statement of what the app touches. Focus Assist
        // can suppress it; the same facts live in the About page and docs/ACCESS.md.
        let _ = tray::notify(
            window.hwnd(),
            TRAY_ICON_ID,
            "Statline is running",
            "Reads performance counters and sensors. No admin, no network, no telemetry. See About > Access.",
        );
        info!("first run: access summary shown");
    }

    // The strip is the product; `--tray-only` and `--safe-mode` are escape
    // hatches for a shell build or a provider that misbehaves.
    let safe_mode = args.iter().any(|a| a == "--safe-mode");
    let mut _strip = None;
    if !args.iter().any(|a| a == "--tray-only") {
        match strip_host::Host::start(strip_host::palette(dark, settings.appearance.accent)) {
            Ok(host) => {
                info!("taskbar strip attached");
                _strip = Some(host);
                strip_host::apply_settings(&settings, dark);
                if safe_mode {
                    strip_host::set_safe_mode(true);
                    info!("safe mode: documented counters only");
                }
            }
            Err(err) => error!("taskbar strip unavailable: {err}"),
        }
    }

    let exit_code = MessageWindow::run_message_loop()?;
    info!("{APP_NAME} exiting with {exit_code}");
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    // Broadcast to top-level windows only; a parented strip is a child, so the
    // restart is handled here for both placement tiers.
    if msg == window::taskbar_created_message() {
        strip_host::recreate();
        return LRESULT(0);
    }

    if msg == tray::WM_TRAY {
        match tray::TrayEvent::from_callback(lparam) {
            Some(tray::TrayEvent::Activate) => {
                // Primary click opens the dashboard; the data folder moved to
                // the context menu because clicking the icon should do the
                // obvious thing.
                strip_host::toggle_dashboard();
            }
            Some(tray::TrayEvent::ContextMenu) => {
                if let Err(err) = menus::show_tray_menu(hwnd, strip_host::menu_state()) {
                    error!("context menu failed: {err}");
                }
            }
            None => {}
        }
        return LRESULT(0);
    }

    if let Some(id) = command_id(msg, wparam) {
        match id {
            menus::MENU_OPEN_DATA => open_data_dir(),
            menus::MENU_EXIT => destroy_window(hwnd),
            menus::MENU_PAUSE => {
                if let Some(paused) = strip_host::toggle_paused() {
                    info!("monitoring paused: {paused}");
                }
            }
            menus::MENU_TOGGLE_STRIP => {
                if let Some(visible) = strip_host::toggle_strip() {
                    info!("strip visible: {visible}");
                }
            }
            menus::MENU_DASHBOARD => {
                strip_host::toggle_dashboard();
            }
            menus::MENU_SETTINGS => settings_host::toggle(),
            menus::MENU_ACCESS => show_access_summary(),
            _ => {}
        }
        return LRESULT(0);
    }

    if is_destroy(msg) {
        tray::remove(hwnd, TRAY_ICON_ID);
        quit_message_loop();
        return LRESULT(0);
    }

    window::default_proc(hwnd, msg, wparam, lparam)
}

fn show_access_summary() {
    window::message_box(
        "Statline \u{2014} access & privacy",
        "Reads performance counters and sensors only.\n\n\
         No administrator/UAC. No network. No telemetry. No driver.\n\n\
         Writes:\n\
           %APPDATA%\\Statline (settings, window)\n\
           %LOCALAPPDATA%\\Statline (logs)\n\
           HKCU\\...\\Run (only when Start with Windows is on)\n\n\
         Full details: docs\\ACCESS.md and PRIVACY.md.",
    );
}

fn open_data_dir() {
    let dir = paths::data_dir();
    if let Err(err) = std::fs::create_dir_all(&dir) {
        error!("could not create {}: {err}", dir.display());
        return;
    }
    if let Err(err) = shell::open_path(&dir) {
        error!("could not open {}: {err}", dir.display());
    }
}
