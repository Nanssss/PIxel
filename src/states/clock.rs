//! The clock state: the time and date from the system clock, next to the Earth.
//!
//! The only state that needs no network, so it is also the fallback when every
//! other state is turned off (see `app.rs`).

use chrono::Local;
use embedded_graphics::{
    geometry::{AnchorX, AnchorY},
    image::Image,
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::Rgb888,
    prelude::*,
    text::{Alignment, Baseline, Text, TextStyle, TextStyleBuilder},
};
use serde::Deserialize;
use tinybmp::Bmp;

use super::State;
use crate::matrix::Matrix;

/// The Earth, drawn on the left half
const EARTH: Bmp<'static, Rgb888> =
    match Bmp::from_slice(include_bytes!("../../assets/clock/earth.bmp")) {
        Ok(bmp) => bmp,
        Err(_) => panic!("assets/clock/earth.bmp should be a valid BMP image"),
    };

/// Font and color of the time
const TIME_STYLE: MonoTextStyle<'static, Rgb888> = MonoTextStyle::new(&FONT_5X8, Rgb888::WHITE);
/// Font and color of the date
const DATE_STYLE: MonoTextStyle<'static, Rgb888> =
    MonoTextStyle::new(&FONT_5X8, Rgb888::CSS_ORANGE);
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

/// Shows the Earth on the left half of the panel, the time and date on the right half.
// `Default`: empty texts, formatted on the first frame (`shown_minute` 0 is never "now")
#[derive(Default)]
pub struct ClockState {
    /// `HH:MM`, with a blinking colon
    time: String,
    /// `DD MON`, e.g. `24 NOV`
    date: String,
    /// Minute (since the Unix epoch)
    shown_minute: i64,
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

    /// Draws the Earth on the left half, the time above the date on the right half.
    fn draw(&mut self, matrix: &mut Matrix) {
        // Reading the system clock is local and cheap: fine on every frame
        let now = Local::now();

        // Unix minute rather than `minute()`: right even if the clock jumps
        let minute = now.timestamp() / 60;
        if minute != self.shown_minute {
            self.time = now.format("%H:%M").to_string();
            self.date = now.format("%d %b").to_string().to_uppercase();
            self.shown_minute = minute;
        }

        // Blinking colon: shows the clock is running
        let colon = if now.timestamp_subsec_millis() < 500 {
            ":"
        } else {
            " "
        };
        // Same length swap (the colon of `HH:MM` is at index 2): no allocation
        self.time.replace_range(2..3, colon);

        let screen = matrix.bounding_box();
        let half_width = screen.size.width / 2;
        let left = screen.resized_width(half_width, AnchorX::Left);
        let right = screen.resized_width(half_width, AnchorX::Right);

        let half_height = right.size.height / 2;
        let time_area = right.resized_height(half_height, AnchorY::Top);
        let date_area = right.resized_height(half_height, AnchorY::Bottom);

        let Ok(()) = Image::with_center(&EARTH, left.center()).draw(matrix);

        let Ok(_) = Text::with_text_style(&self.time, time_area.center(), TIME_STYLE, CENTERED)
            .draw(matrix);

        let Ok(_) = Text::with_text_style(&self.date, date_area.center(), DATE_STYLE, CENTERED)
            .draw(matrix);
    }
}
