//! Everything related to the LED matrix, split in layers:
//! - `ffi`: raw C bindings (private, never leaves this module),
//! - `wrapper`: safe 1:1 Rust abstractions over the C library (`RgbLedMatrix`, `Canvas`...),
//! - `graphics`: `embedded-graphics` support for `Canvas` (trait impls only),
//! - `display`: the app's `Matrix`, double buffering built on the wrapper,
//! - `simulator`: the same `Matrix` shown in a window on the PC, replaces `display`
//!   when built with `--features simulator`.
//!
//! The app only needs `use crate::matrix::Matrix;`, and draws on it with
//! `embedded-graphics` (colors are `Rgb888`). The C library's types never leave this module.

mod ffi;
mod graphics;
mod wrapper;

// The app's `Matrix` is chosen at compile time: `#[cfg(...)]` only compiles the
// item below it when its condition is true, so exactly one `Matrix` exists.
#[cfg(not(feature = "simulator"))]
mod display;
#[cfg(not(feature = "simulator"))]
pub use display::Matrix;

#[cfg(feature = "simulator")]
mod simulator;
#[cfg(feature = "simulator")]
pub use simulator::Matrix;

/// Size of the panel, in pixels (shared by `display` and `simulator`).
// `u32`: the type `embedded-graphics` uses for sizes
const PANEL_WIDTH: u32 = 64;
const PANEL_HEIGHT: u32 = 32;
