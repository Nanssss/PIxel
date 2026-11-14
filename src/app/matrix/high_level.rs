//! App-oriented helpers built on top of the safe wrapper.
//!
//! Rust allows several `impl` blocks for the same type, in different modules of
//! the same crate: this one extends `Matrix` without touching `wrapper.rs`.

use super::ffi::Color;
use super::wrapper::{Matrix, MatrixOptions};

impl Matrix {
    pub fn new() -> Self {
        let options = MatrixOptions {
            rows: 32,
            cols: 64,
            brightness: 100,
            hardware_mapping: Some("adafruit-hat".to_string()),
            ..Default::default()
        };

        Self::init(options).unwrap()
    }

    /// Displays everything drawn on the matrix since the last call, on the next vertical sync.
    ///
    /// Drawing goes to a hidden buffer (double buffering, no flickering). The swap gives
    /// back the previously displayed buffer, which is cleared and becomes the next hidden
    /// one, so each frame starts from a black screen. Only two buffers ever exist:
    ///
    /// ```text
    ///          frame 1          frame 2          frame 3
    /// shown:   [A]    swap ->   [B]    swap ->   [A]
    /// hidden:  [B] <- draw      [A] <- draw      [B] <- draw
    /// ```
    pub fn show(&mut self) {
        self.swap_offscreen();
        self.offscreen.clear();
    }

    /// Fills the whole panel with a single solid color (visible after `show`).
    pub fn fill(&mut self, color: Color) {
        self.offscreen.fill(color);
    }
}
