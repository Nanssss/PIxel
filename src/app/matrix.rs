//! Everything related to the LED matrix, split in layers:
//! - `ffi`: raw C bindings (private, never leaves this module),
//! - `wrapper`: safe 1:1 Rust abstractions over the C library,
//! - `high_level`: app-oriented helpers built on the wrapper,
//! - `graphics`: `embedded-graphics` support (trait impls only).
//!
//! The public API is re-exported below, so the app only needs
//! `use crate::app::matrix::{Matrix, Color};`.

mod ffi;
mod graphics;
mod high_level;
mod wrapper;

// Only re-export what the app uses: in a binary crate, an unused `pub use`
// triggers a warning. Add `Canvas`, `Font`, `MatrixOptions`... here when needed.
pub use ffi::Color;
pub use wrapper::Matrix;
