//! INDIUM — an archive manager for Linux on Wayland.
//!
//! Copyright © sudo-megas. GPL-3.0-only.
//!
//! CORE §1: "One archive per window. Opening a second archive opens a second window.
//! There are no tabs." So this binary opens exactly one archive: the one named on the
//! command line, if any.

// The window is the product; a console window is not wanted alongside it. On Linux
// this attribute is a no-op, but it documents the intent and costs nothing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use indium::ui::Indium;

const USAGE: &str = "\
INDIUM — an archive manager for Linux on Wayland.

    indium [ARCHIVE]

    ARCHIVE    an archive to open on launch

    -h, --help       this text
    -V, --version    the version

Headless subcommands (extract, list, single-file open) arrive in V1.3.
";

fn main() -> eframe::Result<()> {
    // CORE §2: "argument handling is `std::env::args`". `clap` arrives only if V1.3's
    // headless subcommands justify its sentence.
    let mut open: Option<PathBuf> = None;
    for arg in std::env::args().skip(1) {
        match arg.as_str() {
            "-h" | "--help" => {
                print!("{USAGE}");
                return Ok(());
            }
            "-V" | "--version" => {
                println!("indium {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            _ if arg.starts_with('-') => {
                eprintln!("indium: unknown option {arg}\n");
                print!("{USAGE}");
                std::process::exit(2);
            }
            _ => {
                if open.is_none() {
                    open = Some(PathBuf::from(arg));
                } else {
                    // One archive per window is the rule, so a second path is a
                    // mistake worth naming rather than silently ignoring.
                    eprintln!(
                        "indium: one archive per window — ignoring {arg}\n\
                         indium: open a second window for it instead"
                    );
                }
            }
        }
    }

    if let Some(path) = &open {
        if !path.exists() {
            eprintln!("indium: {} does not exist", path.display());
            std::process::exit(1);
        }
    }

    let options = eframe::NativeOptions {
        viewport: eframe::egui::ViewportBuilder::default()
            .with_title("INDIUM")
            // On Wayland this is what actually resolves a window to its icon: the
            // compositor maps the app id to `org.indium.desktop`, and that names
            // `Icon=indium` in the hicolor tree. `with_icon` below is the belt to this
            // brace, for compositors and tools that ask the window directly.
            .with_app_id("org.indium")
            .with_icon(window_icon())
            .with_inner_size([1180.0, 720.0])
            .with_min_inner_size([840.0, 480.0]),
        ..Default::default()
    };

    eframe::run_native(
        "INDIUM",
        options,
        Box::new(move |cc| Ok(Box::new(Indium::new(cc, open)))),
    )
}

/// The window icon, embedded from the maker's 256px master.
///
/// 256 because eframe's own guidance is a square image of about that size — smaller and
/// a compositor scales up, larger and it scales down for no gain. `from_png_bytes` costs
/// no new dependency: `image` is already linked through eframe's clipboard path with PNG
/// on, which is the correction CORE §2 now records.
///
/// A decode failure yields an empty icon rather than a panic. INDIUM refusing to start
/// because a decoration would not load would be the wrong trade.
fn window_icon() -> eframe::egui::IconData {
    eframe::icon_data::from_png_bytes(include_bytes!("../build/icons/indium-256.png"))
        .unwrap_or_default()
}
