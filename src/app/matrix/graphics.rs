//! Glue between the `embedded-graphics` crate and the LED matrix `Canvas`.
//!
//! `embedded-graphics` draws anything (shapes, text, images) on any type that
//! implements its [`DrawTarget`] trait. A target only has to provide two things:
//! - its size, through [`OriginDimensions`],
//! - a way to set pixels, through [`DrawTarget::draw_iter`].
//!
//! Every other drawing method has a default implementation built on these two.
//! We still override [`DrawTarget::clear`] because the C library has a much
//! faster "fill the whole canvas" call than setting pixels one by one.
//!
//! Usage: once this module is compiled, any `Canvas` can be drawn on with
//! `use embedded_graphics::prelude::*;` and `drawable.draw(&mut canvas)`.

use core::convert::Infallible;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use super::matrix::{Canvas, Color};

/// Converts an `embedded-graphics` color into the C library `Color` struct.
///
/// Implementing `From` gives us `Into` for free, so we can write `color.into()`.
impl From<Rgb888> for Color {
    fn from(color: Rgb888) -> Self {
        Color { r: color.r(), g: color.g(), b: color.b() }
    }
}

/// Tells `embedded-graphics` how big the canvas is.
///
/// `OriginDimensions` (instead of `Dimensions`) means the drawable area
/// starts at (0, 0), which is the case for the LED matrix.
impl OriginDimensions for Canvas {
    fn size(&self) -> Size {
        // `Canvas::size` is the inherent method returning `(i32, i32)`.
        // Inherent methods take priority over trait methods with the same name.
        let (width, height) = Canvas::size(self);
        Size::new(width as u32, height as u32)
    }
}

impl DrawTarget for Canvas {
    /// The matrix is driven with 8 bits per channel, so `Rgb888` is a perfect fit.
    type Color = Rgb888;

    /// Drawing on the canvas can never fail: `Infallible` is a type with no value,
    /// so the compiler knows the `Err` case can't happen.
    type Error = Infallible;

    /// Draws every pixel yielded by the iterator.
    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // embedded-graphics may give coordinates outside of the screen
        // (e.g. a shape partially off-screen): skip them.
        let bounds = self.bounding_box();

        for Pixel(point, color) in pixels {
            if bounds.contains(point) {
                self.set_pixel(point.x, point.y, color.into());
            }
        }

        Ok(())
    }

    /// Fills the whole canvas with one color, using the fast C library call.
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.fill(color.into());
        Ok(())
    }
}
