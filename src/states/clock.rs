//! The clock state: the time and date from the system clock, over an animation of the
//! Earth, the Moon and the Sun.
//!
//! The only state that needs no network, so it is also the fallback when every
//! other state is turned off (see `app.rs`).

use std::time::{Duration, Instant};

use chrono::Local;
use embedded_graphics::{
    geometry::{AnchorX, AnchorY},
    image::GetPixel,
    mono_font::{MonoTextStyle, ascii::FONT_5X8},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Alignment, Baseline, Text, TextStyle, TextStyleBuilder},
};
use serde::Deserialize;
use tinybmp::Bmp;

use super::State;
use crate::matrix::{Matrix, PANEL_SIZE};

/// The animation behind the texts
/// Frames the size of the panel, stacked vertically: the BMP image is W:64 x H:7680
const EARTH: Bmp<'static, Rgb888> =
    match Bmp::from_slice(include_bytes!("../../assets/clock/earth.bmp")) {
        Ok(bmp) => bmp,
        Err(_) => panic!("assets/clock/earth.bmp should be a valid BMP image"),
    };
/// Number of frames in `EARTH`
const NB_EARTH_FRAMES: u32 = EARTH.as_raw().header().image_size.height / PANEL_SIZE.height;
// A wrong image size fails the build, instead of drawing garbage
const _: () = assert!(
    EARTH.as_raw().header().image_size.width == PANEL_SIZE.width
        && EARTH.as_raw().header().image_size.height % PANEL_SIZE.height == 0,
    "earth.bmp should be a strip of panel-sized frames"
);
/// Time for one loop of `EARTH`
const EARTH_LOOP_PERIOD: Duration = Duration::from_secs(20);

/// Font and color of the time
const TIME_STYLE: MonoTextStyle<'static, Rgb888> = MonoTextStyle::new(&FONT_5X8, Rgb888::WHITE);
/// Font and color of the date
const DATE_STYLE: MonoTextStyle<'static, Rgb888> =
    MonoTextStyle::new(&FONT_5X8, Rgb888::CSS_DARK_MAGENTA);
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

pub struct ClockState {
    /// `HH:MM`, with a blinking colon
    time: String,
    /// `DD mon`
    date: String,
    /// Minute (since the Unix epoch)
    shown_minute: i64,
    /// Start of the animation
    started: Instant,
}

/// Written by hand: `Instant` has no `Default`.
impl Default for ClockState {
    fn default() -> Self {
        Self {
            // Empty texts, formatted on the first frame (`shown_minute` 0 is never "now")
            time: String::new(),
            date: String::new(),
            shown_minute: 0,
            started: Instant::now(),
        }
    }
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

    /// Draws animation + text
    fn draw(&mut self, matrix: &mut Matrix) {
        // Reading the system clock is local and cheap: fine on every frame
        let now = Local::now();

        // Unix minute rather than `minute()`: right even if the clock jumps
        let minute = now.timestamp() / 60;
        if minute != self.shown_minute {
            self.time = now.format("%H:%M").to_string();
            self.date = now.format("%d %b").to_string().to_lowercase();
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
        let right = screen.resized_width(screen.size.width / 2, AnchorX::Right);
        let half_height = right.size.height / 2;
        let time_area = right.resized_height(half_height, AnchorY::Top);
        let date_area = right.resized_height(half_height, AnchorY::Bottom);

        // Elapsed time (monotonic) rather than the time of day: no jump on an NTP sync
        let progress = self
            .started
            .elapsed()
            .div_duration_f64(EARTH_LOOP_PERIOD)
            .fract();
        let frame = (progress * f64::from(NB_EARTH_FRAMES)) as u32;
        let source = Rectangle::new(
            Point::new(0, (frame * PANEL_SIZE.height) as i32),
            PANEL_SIZE,
        );
        // Reads only this frame: `sub_image` would decode the whole strip
        let colors = source
            .points()
            .map(|point| EARTH.pixel(point).unwrap_or(Rgb888::BLACK));
        let Ok(()) = matrix.fill_contiguous(&screen, colors);

        let time = Text::with_text_style(&self.time, time_area.center(), TIME_STYLE, CENTERED);
        let date = Text::with_text_style(&self.date, date_area.center(), DATE_STYLE, CENTERED);
        for text in [&time, &date] {
            let Ok(_) = text.draw(matrix);
        }
    }
}
