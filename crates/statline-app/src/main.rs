// Console in debug builds (for `--probe`, `--bench`), no console in release.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use log::{LevelFilter, error, info};
use statline_core::{APP_NAME, VERSION};
use statline_win::window::MessageWindow;
use statline_win::{
    HWND, LPARAM, LRESULT, WPARAM, command_id, destroy_window, dpi, is_destroy, logger, menus,
    paths, quit_message_loop, shell, tray, window,
};

const WINDOW_CLASS: &str = "StatlineMessageWindow";
const TRAY_ICON_ID: u32 = 1;

fn main() {
    if let Err(err) = run() {
        eprintln!("{APP_NAME} failed: {err}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args: Vec<String> = std::env::args().skip(1).collect();
    if args.iter().any(|a| a == "--version" || a == "-V") {
        println!("{APP_NAME} {VERSION}");
        return Ok(());
    }
    if args.iter().any(|a| a == "--help" || a == "-h") {
        println!("{APP_NAME} {VERSION}");
        println!();
        println!("Usage: statline [options]");
        println!();
        println!("  --help, -h       Show this help");
        println!("  --version, -V    Show the version");
        println!();
        println!("With no options, Statline starts in the notification area.");
        return Ok(());
    }

    dpi::enable_per_monitor_v2();

    let log_dir = paths::log_dir();
    if let Err(err) = logger::init(&log_dir, LevelFilter::Info) {
        eprintln!("could not open log file in {}: {err}", log_dir.display());
    }
    info!("{APP_NAME} {VERSION} starting");

    let window = MessageWindow::create(WINDOW_CLASS, APP_NAME, Some(wndproc))?;
    if let Err(err) = tray::add(window.hwnd(), TRAY_ICON_ID, APP_NAME) {
        error!("failed to add tray icon: {err}");
        return Err(err.into());
    }

    info!("tray icon ready");
    let exit_code = MessageWindow::run_message_loop()?;
    info!("{APP_NAME} exiting with {exit_code}");
    Ok(())
}

unsafe extern "system" fn wndproc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == tray::WM_TRAY {
        match tray::TrayEvent::from_callback(lparam) {
            Some(tray::TrayEvent::Activate) => open_data_dir(),
            Some(tray::TrayEvent::ContextMenu) => {
                if let Err(err) = menus::show_tray_menu(hwnd) {
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
