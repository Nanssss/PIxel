//! Everything related to the LED matrix, split in layers:
//! - `ffi`: raw C bindings (private, never leaves this module),
//! - `wrapper`: safe 1:1 Rust abstractions over the C library (`RgbLedMatrix`, `Canvas`...),
//! - `graphics`: `embedded-graphics` support for `Canvas` (trait impls only),
//! - `display`: the app's `Matrix`, double buffering built on the wrapper.
//!
//! The public API is re-exported below, so the app only needs
//! `use crate::app::matrix::{Matrix, Color};`.

mod display;
mod ffi;
mod graphics;
mod wrapper;

// Only re-export what the app uses: in a binary crate, an unused `pub use`
// triggers a warning. Add `Canvas`, `Font`, `MatrixOptions`... here when needed.
pub use display::Matrix;
pub use ffi::Color;
