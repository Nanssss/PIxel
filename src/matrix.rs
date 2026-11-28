//! Everything related to the LED matrix, split in layers:
//! - `ffi`: raw C bindings (private, never leaves this module),
//! - `wrapper`: safe 1:1 Rust abstractions over the C library (`RgbLedMatrix`, `Canvas`...),
//! - `graphics`: `embedded-graphics` support for `Canvas` (trait impls only),
//! - `display`: the app's `Matrix`, double buffering built on the wrapper,
//! - `simulator`: the same `Matrix` shown in a window on the PC, replaces `display`
//!   when built with `--features simulator`.
//!
//! The app only needs `use crate::matrix::Matrix;` (and `PANEL_SIZE` in a `const`), and draws
//! on it with `embedded-graphics` (colors are `Rgb888`). The C library's types never leave
//! this module.

use embedded_graphics::geometry::Size;

mod ffi;
mod graphics;
mod wrapper;

// Exactly one `Matrix` is compiled: the real one, or the simulated one
#[cfg(not(feature = "simulator"))]
mod display;
#[cfg(not(feature = "simulator"))]
pub use display::Matrix;

#[cfg(feature = "simulator")]
mod simulator;
#[cfg(feature = "simulator")]
pub use simulator::Matrix;

/// Width of the panel, in pixels
const PANEL_WIDTH: u32 = 64;
/// Height of the panel, in pixels
const PANEL_HEIGHT: u32 = 32;
/// Size of the panel
pub const PANEL_SIZE: Size = Size::new(PANEL_WIDTH, PANEL_HEIGHT);
