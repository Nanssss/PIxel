//! The app's screen: the LED matrix used with double buffering.
//!
//! This is the policy layer built on the 1:1 wrapper: it owns the matrix and
//! its single offscreen canvas, and contains no FFI call.
//!
//! Usage: draw on the `Matrix` with `use embedded_graphics::prelude::*;` and
//! `drawable.draw(matrix)`, then call `matrix.show()` to display the frame.

use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use super::ffi::Color;
use super::wrapper::{Canvas, MatrixOptions, RgbLedMatrix, RuntimeOptions};

/// The LED matrix as the app sees it: draw on it, then [`Matrix::show`] the frame.
pub struct Matrix {
    /// Hidden buffer we draw on before displaying it (double buffering).
    /// Fields drop in declaration order: the canvas goes before the matrix that owns it.
    offscreen: Canvas,
    rgb_matrix: RgbLedMatrix,
}

impl Matrix {
    /// Creates the matrix for this project's panel (32x64, Adafruit HAT).
    /// Requires root privileges on the Pi.
    ///
    /// # Panics
    /// If the C library fails to create the matrix (usually: not run as root).
    pub fn new() -> Self {
        // This project's panel. Fields not listed keep the library defaults
        // (`..Default::default()` fills the rest with zeros = library defaults).
        let options = MatrixOptions {
            rows: 32,
            cols: 64,
            brightness: 100,
            // `c"..."` is a C-string literal (`&'static CStr`), `to_owned` makes a `CString`
            hardware_mapping: Some(c"adafruit-hat".to_owned()),
            ..Default::default()
        };

        // All library defaults: GPIO slowdown 1, run in the foreground, GPIO init,
        // and drop root privileges to user "daemon" right after init (set
        // `drop_priv_user` here if the app must keep writing root-owned files).
        let rt_options = RuntimeOptions::default();

        // `expect` instead of `unwrap`: same panic, but with a helpful message.
        let mut rgb_matrix = RgbLedMatrix::new(options, &rt_options)
            .expect("failed to init the LED matrix");

        // Safety: the canvas is stored next to `rgb_matrix` and never leaves
        // `Matrix`, so it can't be used after the matrix is dropped.
        let offscreen = unsafe { rgb_matrix.create_offscreen_canvas() };

        Self { offscreen, rgb_matrix }
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
        self.rgb_matrix.swap_on_vsync(&mut self.offscreen);
        self.offscreen.clear();
    }

    /// Fills the whole panel with a single solid color (visible after `show`).
    pub fn fill(&mut self, color: Color) {
        self.offscreen.fill(color);
    }
}

/// The matrix is drawn on like a canvas: everything goes to its hidden buffer,
/// and nothing is visible until `Matrix::show` is called.
impl OriginDimensions for Matrix {
    fn size(&self) -> Size {
        OriginDimensions::size(&self.offscreen)
    }
}

impl DrawTarget for Matrix {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        self.offscreen.draw_iter(pixels)
    }

    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        DrawTarget::clear(&mut self.offscreen, color)
    }
}
