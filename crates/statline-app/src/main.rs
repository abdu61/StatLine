// Console in debug builds (for `--probe`, `--bench`), no console in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod cli;
mod profile;
mod strip_host;

use log::{LevelFilter, error, info};
use statline_core::{APP_NAME, VERSION};
use statline_win::window::MessageWindow;
use statline_win::{
    HWND, LPARAM, LRESULT, WPARAM, command_id, destroy_window, dpi, is_destroy, logger, menus,
    paths, quit_message_loop, shell, tray, window,
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
        cli::print_line(if statline_win::autostart::is_enabled() {
            "startup: enabled"
        } else {
            "startup: disabled"
        });
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

    let window = MessageWindow::create(WINDOW_CLASS, APP_NAME, Some(wndproc))?;
    if let Err(err) = tray::add(window.hwnd(), TRAY_ICON_ID, APP_NAME) {
        error!("failed to add tray icon: {err}");
        return Err(err.into());
    }
    info!("tray icon ready");

    // The strip is the product; `--tray-only` is the escape hatch when a shell
    // build will not cooperate.
    let mut _strip = None;
    if !args.iter().any(|a| a == "--tray-only") {
        match strip_host::Host::start(strip_host::default_palette(true)) {
            Ok(host) => {
                info!("taskbar strip attached");
                _strip = Some(host);
            }
            Err(err) => error!("taskbar strip unavailable: {err}"),
        }
    }

    let exit_code = MessageWindow::run_message_loop()?;
    info!("{APP_NAME} exiting with {exit_code}");
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == tray::WM_TRAY {
        match tray::TrayEvent::from_callback(lparam) {
            Some(tray::TrayEvent::Activate) => open_data_dir(),
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
                info!("dashboard requested (M6)");
            }
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
