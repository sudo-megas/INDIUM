//! INDIUM — an archive manager for Linux on Wayland.
//!
//! The library half exists so the integration tests in `tests/` can drive the reader
//! and the store without going through the window. `main.rs` is the binary.
//!
//! Copyright © sudo-megas. GPL-3.0-only.

pub mod arch;
pub mod model;
pub mod platform;
pub mod secret;
pub mod tasks;
pub mod theme;
pub mod ui;
pub mod util;
