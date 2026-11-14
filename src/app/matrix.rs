//! Everything related to the LED matrix, split in layers:
//! - `ffi`: raw C bindings (private, never leaves this module),
//! - `wrapper`: safe 1:1 Rust abstractions over the C library (`RgbLedMatrix`, `Canvas`...),
//! - `graphics`: `embedded-graphics` support for `Canvas` (trait impls only),
//! - `display`: the app's `Matrix`, double buffering built on the wrapper,
//! - `simulator`: the same `Matrix` shown in a window on the PC, replaces `display`
//!   when built with `--features simulator`.
//!
//! The public API is re-exported below, so the app only needs
//! `use crate::app::matrix::{Matrix, Color};`.

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

// Only re-export what the app uses: in a binary crate, an unused `pub use`
// triggers a warning. Add `Canvas`, `Font`, `MatrixOptions`... here when needed.
pub use ffi::Color;
