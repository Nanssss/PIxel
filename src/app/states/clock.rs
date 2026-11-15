use chrono::Local;
use tracing::info;
use serde::Deserialize;
use crate::app::matrix::Matrix;
use crate::app::states::app_state::Status;
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
/// on a state change, it is read from the system clock on every frame (see `draw`).
pub struct ClockState {
    status: Status,         // what the app may do with this state
    config: ClockConfig,    // what the user asked for: never modified
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl ClockState { 
    /// ClockState constructor
    pub fn new(config: ClockConfig) -> Self {
        let mut state = ClockState {
            status: Status::Disabled,
            config,
        };

        if state.config.is_enabled() {
            state.enable();
        }

        state
    }

    /// Enables the state
    /// The clock needs no network: it is ready as soon as it is enabled.
    pub fn enable(&mut self) {
        self.status = Status::Ready;
    }

    /// Returns the runtime status of the state
    pub fn status(&self) -> Status {
        self.status
    }

    /// Called when the app switches to this state, like for the other states.
    /// The clock has nothing to fetch: it only logs that it is displayed.
    pub fn refresh(&self) {
        if self.status != Status::Disabled {
            info!("[CLOCK] - {}", Local::now().format("%Y-%m-%d %H:%M:%S"));
        }
    }

    /// Method for drawing the state to the screen
    pub fn draw(&self, matrix: &mut Matrix) {
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
