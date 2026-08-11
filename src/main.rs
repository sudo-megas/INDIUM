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

/// The help text, and the whole of the argument vocabulary, live in `cli` now — one copy,
/// beside the code that answers to it, and pinned to `cli::SUBCOMMANDS` by a test so the
/// two cannot drift. It used to end with a sentence promising headless subcommands in
/// V1.3; P17 built them, so the sentence is gone rather than merely stale.
use indium::cli::USAGE;

fn main() -> eframe::Result<()> {
    // ---- The terminal half, before anything asks for a window. ----
    //
    // This is above `NativeOptions` and above `window_icon()` deliberately: `indium list`
    // on a machine with no compositor must be an ordinary program reading a file, so no
    // GL context is created and no `ui` item is touched on the way. Proven rather than
    // asserted — `tests/cli_path.rs` runs every subcommand under CI, where there is no
    // WAYLAND_DISPLAY at all.
    let args: Vec<std::ffi::OsString> = std::env::args_os().skip(1).collect();
    if indium::cli::takes_the_terminal(&args) {
        let mut out = std::io::BufWriter::new(std::io::stdout().lock());
        let mut err = std::io::stderr().lock();
        let code = indium::cli::run(&args, &mut out, &mut err);
        // `run` has already flushed `out` and folded any failure into `code`, which is
        // what makes `exit` safe here: it runs no destructors, so a flush left to
        // `BufWriter::drop` would be a truncated file reported as success.
        std::process::exit(code);
    }

    // CORE §2: "argument handling is `std::env::args_os`". **`clap` was refused at P17**,
    // which is the round §2 said would decide it: three subcommands, one string option and
    // two flags is forty lines of `match`, and the derive path would bring a
    // colour-negotiation stack to a program with one palette and no theme setting. The
    // refusal is dated in §2 so it is not reproposed as a discovery.
    let mut open: Option<PathBuf> = None;
    let mut also: Vec<PathBuf> = Vec::new();
    // **`args_os`, and never `args`.** `std::env::args()` panics on an argument that is
    // not valid Unicode, and a path on Linux is bytes — which this program argues at
    // length in `arch::path_to_cstring` and then, until P17, ignored one function later.
    // `indium /tmp/<latin1>.zip` therefore aborted before the window opened, in every
    // binary shipped since P1. Nobody hit it because the corpus INDIUM is tested against
    // is Turkish in UTF-8; that is luck, not a design.
    //
    // Matching on `to_str()` costs nothing and needs no `OsStr` comparisons: every flag
    // this program has is ASCII, so an argument that is not valid UTF-8 cannot be one,
    // and falls through to the path arm exactly where it belongs.
    for arg in std::env::args_os().skip(1) {
        match arg.to_str() {
            Some("-h") | Some("--help") => {
                print!("{USAGE}");
                return Ok(());
            }
            Some("-V") | Some("--version") => {
                println!("indium {}", env!("CARGO_PKG_VERSION"));
                return Ok(());
            }
            Some(text) if text.starts_with('-') => {
                eprintln!("indium: unknown option {text}\n");
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
            // **The floor sits below what the compositor hands out, deliberately.** It read
            // 840×600 while this machine is given a 540-point-tall window; KWin ignores the
            // minimum when restoring a remembered geometry and enforces it the moment an
            // edge is grabbed, so a drag snapped the frame up sixty points before it moved
            // anywhere. `ui::MIN_W`/`MIN_H` carry the reasoning.
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
