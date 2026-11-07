//! Embeds `packaging/statline.ico` into `statline.exe` when the Windows SDK
//! resource compiler is available.
//!
//! The build stays green on machines without the SDK: the icon is skipped with a
//! warning and the tray falls back to the stock application icon. No build-time
//! crate is needed; `rc.exe` ships with the Windows SDK already used for MSIX.

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=../../packaging/statline.ico");
    println!("cargo:rerun-if-changed=build.rs");

    if !cfg!(windows) {
        return;
    }

    let manifest = PathBuf::from(std::env::var("CARGO_MANIFEST_DIR").expect("manifest dir"));
    let icon = manifest.join("../../packaging/statline.ico");
    if !icon.exists() {
        println!("cargo:warning=packaging/statline.ico missing; building without an icon");
        return;
    }

    let out = PathBuf::from(std::env::var("OUT_DIR").expect("out dir"));
    let script = out.join("statline.rc");
    let icon_path = icon.to_string_lossy().replace('\\', "/");
    if let Err(error) = fs::write(&script, format!("1 ICON \"{icon_path}\"\n")) {
        println!("cargo:warning=could not write the icon resource script: {error}");
        return;
    }

    let Some(rc) = find_rc() else {
        println!("cargo:warning=rc.exe not found; building without an icon");
        return;
    };
    let resource = out.join("statline.res");
    let status = Command::new(&rc)
        .arg("/nologo")
        .arg("/fo")
        .arg(&resource)
        .arg(&script)
        .status();
    match status {
        Ok(status) if status.success() && resource.exists() => {
            println!("cargo:rustc-link-arg-bins={}", resource.display());
        }
        Ok(status) => {
            println!("cargo:warning=rc.exe exited with {status}; building without an icon")
        }
        Err(error) => println!("cargo:warning=could not run rc.exe: {error}"),
    }
}

fn find_rc() -> Option<PathBuf> {
    if let Ok(output) = Command::new("where").arg("rc.exe").output() {
        if output.status.success() {
            let text = String::from_utf8_lossy(&output.stdout);
            if let Some(line) = text.lines().find(|line| !line.trim().is_empty()) {
                let path = PathBuf::from(line.trim());
                if path.exists() {
                    return Some(path);
                }
            }
        }
    }

    let kits = Path::new(r"C:\Program Files (x86)\Windows Kits\10\bin");
    let entries = fs::read_dir(kits).ok()?;
    let mut candidates: Vec<PathBuf> = entries
        .flatten()
        .map(|entry| entry.path().join("x64").join("rc.exe"))
        .filter(|path| path.exists())
        .collect();
    candidates.sort();
    candidates.pop()
}
