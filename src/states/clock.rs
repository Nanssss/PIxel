use chrono::Local;
use serde::Deserialize;
use crate::matrix::Matrix;
use crate::states::State;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::PrimitiveStyle,
    text::{Alignment, Baseline, Text, TextStyle, TextStyleBuilder},
};

/* Drawing styles, built once at compile time */
/// 1px red outline used for the panel border
const BORDER_STYLE: PrimitiveStyle<Rgb888> = PrimitiveStyle::with_stroke(Rgb888::RED, 1);
/// White 6x10 mono font for the time (font + color)
const TIME_FONT: MonoTextStyle<'static, Rgb888> = MonoTextStyle::new(&FONT_6X10, Rgb888::WHITE);
/// Text centered horizontally and vertically on its anchor point
const CENTERED: TextStyle = TextStyleBuilder::new()
    .alignment(Alignment::Center)
    .baseline(Baseline::Middle)
    .build();

/// Public struct containing clock config
#[derive(Debug, Clone, Deserialize)]
pub struct ClockConfig {
    enabled: bool,
}

/// Default trait for config
impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            enabled: true,
        }
    }
}

impl ClockConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Public struct containing global clock state
///
/// Unlike the other states, the clock stores no data: the time is not fetched
/// in the background, it is read from the system clock on every frame (see `draw`).
// A *unit struct*: a type without any field, only there to carry the `State` implementation.
pub struct ClockState;


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl ClockState {
    /// ClockState constructor: returns `None` if the clock is turned off in the config.
    pub fn new(config: ClockConfig) -> Option<Self> {
        // `then_some(value)` turns a `bool` into an `Option`: `Some(value)` if true, else `None`
        config.is_enabled().then_some(ClockState)
    }
}

impl State for ClockState {
    /// The clock needs no network: it always has something to display.
    fn is_ready(&self) -> bool {
        true
    }

    /// Method for drawing the state to the screen
    fn draw(&self, matrix: &mut Matrix) {
        // The time of day is an input of the frame (like the elapsed time of an
        // animation), not fetched data: reading the system clock is local and cheap.
        let time = Local::now().format("%H:%M:%S").to_string();

        let screen = matrix.bounding_box();

        // Drawing on the matrix can't fail (Error = Infallible), so `unwrap()` never panics
        /* 1px red border around the whole panel */
        screen.into_styled(BORDER_STYLE).draw(matrix).unwrap();

        /* Time centered on the panel */
        Text::with_text_style(&time, screen.center(), TIME_FONT, CENTERED).draw(matrix).unwrap();
    }
}
