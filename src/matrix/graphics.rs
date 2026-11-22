//! `embedded-graphics` support for the LED matrix `Canvas`: implementing its
//! [`DrawTarget`] trait lets the crate draw anything on it (shapes, text, images).
//!
//! The app draws on `Matrix` (see `display.rs`), which forwards to its offscreen `Canvas`.

use core::convert::Infallible;

use embedded_graphics::{pixelcolor::Rgb888, prelude::*};

use super::ffi::Color;
use super::wrapper::Canvas;

/// Converts an `embedded-graphics` color into the C library `Color` struct.
impl From<Rgb888> for Color {
    fn from(color: Rgb888) -> Self {
        Self {
            r: color.r(),
            g: color.g(),
            b: color.b(),
        }
    }
}

/// Size of the canvas. `Origin`: the drawable area starts at (0, 0).
impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        // The inherent `Canvas::size` (inherent methods win over trait methods), not this one
        let (width, height) = Canvas::size(self);
        Size::new(width as u32, height as u32)
    }
}

impl DrawTarget for Canvas {
    /// The matrix is driven with 8 bits per channel, so `Rgb888` is a perfect fit.
    type Color = Rgb888;

    /// Drawing on the canvas can never fail.
    type Error = Infallible;

    /// Draws every pixel yielded by the iterator.
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // Pixels can be off the screen (e.g. a shape partially off-screen): skip them
        let bounds = self.bounding_box();

        for Pixel(point, color) in pixels {
            if bounds.contains(point) {
                self.set_pixel(point.x, point.y, color.into());
            }
        }

        Ok(())
    }

    /// Fills the whole canvas with one color: a single C call, instead of one per pixel
    /// (the default implementation).
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill(color.into());
        Ok(())
    }
}
