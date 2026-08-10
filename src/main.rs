//! INDIUM — an archive manager for Linux on Wayland.
//!
//! Copyright © sudo-megas. GPL-3.0-only.
//!
//! CORE §1: "One archive per window. Opening a second archive opens a second window.
//! There are no tabs." So this binary opens exactly one archive — the first one named on
//! the command line — and every further archive gets a window of its own, which since
//! P8 this program opens rather than asks the user to open.

// The window is the product; a console window is not wanted alongside it. On Linux
// this attribute is a no-op, but it documents the intent and costs nothing.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use std::path::PathBuf;

use indium::platform::window;
use indium::ui::Indium;

const USAGE: &str = "\
INDIUM — an archive manager for Linux on Wayland.

    indium [ARCHIVE]...

    ARCHIVE    an archive to open on launch; each one after the first
               opens in a window of its own

    -h, --help       this text
    -V, --version    the version

Headless subcommands (extract, list, single-file open) arrive in V1.3.
";

fn main() -> eframe::Result<()> {
    // CORE §2: "argument handling is `std::env::args`". `clap` arrives only if V1.3's
    // headless subcommands justify its sentence.
    let mut open: Option<PathBuf> = None;
    let mut also: Vec<PathBuf> = Vec::new();
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
                    // One archive per window is the rule, and until P8 the whole of
                    // INDIUM's answer to a second path was a sentence telling the user
                    // to go and open a window for it by hand. It is the same sentence
                    // this program can now act on, so it does.
                    also.push(PathBuf::from(arg));
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

    // Before the window, so a launch that names five archives puts five windows on
    // screen at once rather than one at a time behind the first one's listing. A child
    // that cannot start is named and does not stop the others — each window reports its
    // own archive, including a missing one, on the terminal they share.
    for path in &also {
        if let Err(e) = window::open_new(path) {
            eprintln!("indium: {e}");
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
            // 480 was below the sidebar's own natural height, so the compositor would
            // happily hand INDIUM a window its first zone could not fit inside. P11 made
            // that survivable — the sidebar reserves its foot and scrolls the rest — but a
            // floor the program can actually be used at is the better half of the answer,
            // and a scrollbar over a wordmark is not a layout anyone chose.
            //
            // **680 is measured, not chosen.** P13's icons made every sidebar row taller,
            // and at 600 the two list sections scrolled out of their own zone. Asked of the
            // running program rather than estimated: the sections lay out to 326.1 and the
            // foot to 146.6, so the panel needs 472.7 inside its frame; the frame costs 40
            // (14+14 inner, 2+2 edge, 4+4 gutter) and the status bar takes `SB_HEIGHT`, so
            // the floor is 472.7 + 40 + 136 = 648.7. 680 is that, rounded up with enough
            // slack that the last row is not flush against the edge it sits on.
            //
            // **880 is measured too, and by the same method.** The three zones have hard
            // floors: the sidebar is fixed at 202 (166 of content + 36 of frame), the
            // Inspector will not go below 272 (`MIN_CONTENT` 236 + 36), and the entry table
            // cannot show its four columns in less than 360 — `Name` is `at_least(120)` and
            // Size, Packed and Method are exact at 84, 84 and 72 — plus its scrollbar. The
            // central zone's own chrome is 20, asked of the running program rather than
            // counted: at a 960 root the sidebar took 202, the Inspector its default 342 and
            // the table 396, and 960 − 940 is that 20. 202 + 272 + 376 + 20 = 870, and 880
            // is that with a little air.
            .with_min_inner_size([indium::ui::MIN_W, indium::ui::MIN_H]),
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
