//! The clock state: shows the current time, read from the system clock.
//!
//! The only state that needs no network, so it is also the fallback when every
//! other state is turned off (see `app.rs`).

use chrono::Local;
use embedded_graphics::{
    mono_font::{MonoTextStyle, ascii::FONT_6X10},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::PrimitiveStyle,
    text::{Alignment, Baseline, Text, TextStyle, TextStyleBuilder},
};
use serde::Deserialize;

use super::State;
use crate::matrix::Matrix;

// Drawing styles, built once at compile time
/// 1px red outline used for the panel border
const BORDER_STYLE: PrimitiveStyle<Rgb888> = PrimitiveStyle::with_stroke(Rgb888::RED, 1);
/// White 6x10 mono font for the time (font + color)
const TIME_FONT: MonoTextStyle<'static, Rgb888> = MonoTextStyle::new(&FONT_6X10, Rgb888::WHITE);
/// Text centered horizontally and vertically on its anchor point
const CENTERED: TextStyle = TextStyleBuilder::new()
    .alignment(Alignment::Center)
    .baseline(Baseline::Middle)
    .build();

/// Config of the clock: the `[clock]` section of `config.toml`.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ClockConfig {
    enabled: bool,
}

/// Values used when the section, or one of its fields, is missing: the clock is shown.
impl Default for ClockConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// Shows the current time, centered in a red border.
///
/// Unlike the other states, nothing is fetched in the background: the time is read
/// from the system clock on every frame.
// `Default`: an empty text, formatted on the first frame (`shown_at` 0 is never "now")
#[derive(Default)]
pub struct ClockState {
    /// Time on screen (`HH:MM:SS`), only formatted again when the second changes
    text: String,
    /// Unix timestamp (in seconds) `text` was formatted for
    shown_at: i64,
}

impl ClockState {
    /// Creates the clock, or returns `None` if it is turned off in the config.
    pub fn new(config: ClockConfig) -> Option<Self> {
        config.enabled.then(Self::default)
    }
}

impl State for ClockState {
    /// The clock needs no network: it always has something to display.
    fn is_ready(&self) -> bool {
        true
    }

    /// Draws the border, then the time (`HH:MM:SS`) in the middle of the panel.
    fn draw(&mut self, matrix: &mut Matrix) {
        // Reading the system clock is local and cheap: fine on every frame
        let now = Local::now();

        // Whole timestamp rather than `second()`: stays right if the clock jumps (NTP sync)
        if now.timestamp() != self.shown_at {
            self.text = now.format("%H:%M:%S").to_string();
            self.shown_at = now.timestamp();
        }

        let screen = matrix.bounding_box();

        let Ok(()) = screen.into_styled(BORDER_STYLE).draw(matrix);

        let Ok(_) =
            Text::with_text_style(&self.text, screen.center(), TIME_FONT, CENTERED).draw(matrix);
    }
}
