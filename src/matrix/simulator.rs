//! The app's screen, simulated in a window on the host PC (no Raspberry Pi needed).
//!
//! Drop-in replacement for the real `Matrix` of `display.rs`: same methods, same
//! `embedded-graphics` traits. `matrix.rs` compiles one or the other, so the rest
//! of the app doesn't know which one it draws on.
//!
//! Usage: `cargo run --features simulator` (needs SDL2: `sudo apt install libsdl2-dev`).

use core::convert::Infallible;
use anyhow::Result;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use embedded_graphics_simulator::{OutputSettingsBuilder, SimulatorDisplay, SimulatorEvent, Window};
use tracing::info;

/// Same size as the real panel (64 columns, 32 rows).
const PANEL_SIZE: Size = Size::new(64, 32);

/// The simulated LED matrix: draw on it, then [`Matrix::show`] the frame.
pub struct Matrix {
    /// Pixel buffer we draw on: plays the role of the real offscreen canvas.
    display: SimulatorDisplay<Rgb888>,
    /// The window showing the buffer (opened by the first `show`).
    window: Window,
}

impl Matrix {
    /// Creates the simulated panel.
    ///
    /// Never fails: it returns a `Result` only to have the same signature as the real
    /// `Matrix::new`, so that the app builds with either.
    pub fn new() -> Result<Self> {
        let settings = OutputSettingsBuilder::new()
            .scale(12)          // each LED is a 12x12 square on the PC screen...
            .pixel_spacing(3)   // ...with a dark gap between LEDs, like on the real panel
            .build();

        Ok(Self {
            display: SimulatorDisplay::new(PANEL_SIZE),
            window: Window::new("PIxel simulator", &settings),
        })
    }

    /// Displays everything drawn on the matrix since the last call.
    ///
    /// Like the real matrix, the buffer is then cleared: each frame starts from a black screen.
    pub fn show(&mut self) {
        self.window.update(&self.display);

        // Closing the window acts like a Ctrl+C: the frame loop (`app::run`) receives
        // the signal and stops the app cleanly, as on the Pi.
        if self.window.events().any(|event| event == SimulatorEvent::Quit) {
            info!("Simulator window closed");
            // Safety: `raise` has no precondition, it only sends a signal to this thread.
            unsafe { libc::raise(libc::SIGINT) };
        }

        // Can't fail (`Error = Infallible`): the pattern needs no `else` (see `State::draw`)
        let Ok(()) = self.display.clear(Rgb888::BLACK);
    }
}

/// Like the real `Matrix`, the simulated one forwards drawing to its buffer.
impl OriginDimensions for Matrix {
    fn size(&self) -> Size {
        self.display.size()
    }
}

impl DrawTarget for Matrix {
    type Color = Rgb888;
    type Error = Infallible;

    fn draw_iter<I>(&mut self, pixels: I) -> Result<(), Self::Error>
    where
        I: IntoIterator<Item = Pixel<Self::Color>>,
    {
        // Pixels outside of the panel are ignored by `SimulatorDisplay`
        self.display.draw_iter(pixels)
    }

    /// Fills the whole panel with one color, like the real `Matrix`.
    fn clear(&mut self, color: Self::Color) -> Result<(), Self::Error> {
        self.display.clear(color)
    }
}
