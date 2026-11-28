//! The clock state: the time and date from the system clock, over an animation of the
//! Earth, the Moon and the Sun.
//!
//! The only state that needs no network, so it is also the fallback when every
//! other state is turned off (see `app.rs`).

use std::time::{Duration, Instant};

use chrono::Local;
use embedded_graphics::{
    image::GetPixel, pixelcolor::Rgb888, prelude::*, primitives::Rectangle, text::Text,
};
use embedded_layout::prelude::*;
use serde::Deserialize;
use tinybmp::Bmp;
use u8g2_fonts::{U8g2TextStyle, fonts::u8g2_font_tinyface_tr};

use super::State;
use crate::matrix::{Matrix, PANEL_SIZE};

/// The animation behind the texts, made by `tools/earth.py`: frames the size of the
/// panel, stacked vertically in one image
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
        && EARTH.as_raw().header().image_size.height % PANEL_SIZE.height == 0
        && NB_EARTH_FRAMES > 0,
    "earth.bmp should be a strip of panel-sized frames"
);
/// Speed of the `EARTH` animation. A loop (one Moon orbit) lasts
/// `NB_EARTH_FRAMES / EARTH_FPS` seconds
const EARTH_FPS: f64 = 12.0;

/// Size of a text area: a quarter of the panel
const AREA_SIZE: Size = Size::new(PANEL_SIZE.width / 2, PANEL_SIZE.height / 2);
/// Where the time goes: the upper right quarter (the Earth is on the left half).
/// `tools/earth.py` keeps the stars out of the texts (`TEXT_BOXES`): update it too
const TIME_AREA: Rectangle = Rectangle::new(Point::new(AREA_SIZE.width as i32, 0), AREA_SIZE);
/// Where the date goes: the lower right quarter
const DATE_AREA: Rectangle = Rectangle::new(
    Point::new(AREA_SIZE.width as i32, AREA_SIZE.height as i32),
    AREA_SIZE,
);

/// Font and color of the time
const TIME_STYLE: U8g2TextStyle<Rgb888> = U8g2TextStyle::new(u8g2_font_tinyface_tr, Rgb888::WHITE);
/// Font and color of the date
const DATE_STYLE: U8g2TextStyle<Rgb888> =
    U8g2TextStyle::new(u8g2_font_tinyface_tr, Rgb888::CSS_DARK_MAGENTA);

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
    /// Start of the animation
    started: Instant,
}

/// Written by hand: `Instant` has no `Default`
impl Default for ClockState {
    fn default() -> Self {
        Self {
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

    /// Draws the animation, then the texts over it
    fn draw(&mut self, matrix: &mut Matrix) {
        draw_earth(matrix, self.started.elapsed());

        // Cheap (a few µs): no need to keep the texts between frames
        let now = Local::now();
        // Blinking colon: shows the clock is running
        let time_format = if now.timestamp_subsec_millis() < 500 {
            "%H:%M"
        } else {
            "%H %M"
        };
        let time = now.format(time_format).to_string();
        let date = now.format("%d %b").to_string().to_lowercase();

        // Each text centered in its area, on its own pixels
        let Ok(_) = Text::new(&time, Point::zero(), TIME_STYLE)
            .align_to(&TIME_AREA, horizontal::Center, vertical::Center)
            // 1 px down: the same margin above the time as below the date (6 rows)
            .translate(Point::new(0, 1))
            .draw(matrix);
        let Ok(_) = Text::new(&date, Point::zero(), DATE_STYLE)
            .align_to(&DATE_AREA, horizontal::Center, vertical::Center)
            .draw(matrix);
    }
}

/// Fills the panel with the frame of `EARTH` due `elapsed` after the start.
///
/// Elapsed time (monotonic) rather than the time of day: no jump on an NTP sync.
fn draw_earth(matrix: &mut Matrix, elapsed: Duration) {
    // Frames shown since the start, wrapped to the loop (u64: no overflow, even after years)
    let frame = (elapsed.as_secs_f64() * EARTH_FPS) as u64 % u64::from(NB_EARTH_FRAMES);
    let source = Rectangle::new(
        Point::new(0, frame as i32 * PANEL_SIZE.height as i32),
        PANEL_SIZE,
    );
    // Reads only this frame: `sub_image` would decode the whole strip
    let colors = source
        .points()
        .map(|point| EARTH.pixel(point).unwrap_or(Rgb888::BLACK));
    let Ok(()) = matrix.fill_contiguous(&matrix.bounding_box(), colors);
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every time, with or without its colon, covers the same pixels as `00:00`: the time
    /// never moves. Relies on the font (all digits as wide, a space as wide as the colon):
    /// fails if a new font breaks it.
    #[test]
    fn time_never_moves() {
        let bounds = |text: &str| Text::new(text, Point::zero(), TIME_STYLE).bounding_box();
        let reference = bounds("00:00");
        for (h, m) in (0..24).flat_map(|h| (0..60).map(move |m| (h, m))) {
            for time in [format!("{h:02}:{m:02}"), format!("{h:02} {m:02}")] {
                assert_eq!(bounds(&time), reference, "{time}");
            }
        }
    }
}
