use chrono::Local;
use tracing::info;
use serde::Deserialize;
use crate::app::matrix::Matrix;
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
    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Public struct containing clock data
pub struct ClockData {
    date: String,
    time: String,
}


/// Public struct containing global clock state
pub struct ClockState {
    pub data: Option<ClockData>,
    pub config: ClockConfig,
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl ClockData {

    /// ClockData constructor
    pub fn new() -> Self {
        let (date, time) = get_date_time();
        ClockData {
            date,
            time
        }
    }

    /// Method for drawing data to the screen
    pub fn draw(&self, matrix: &mut Matrix) {
        info!("[CLOCK] - Date: {}, Time: {}", self.date, self.time);

        let screen = matrix.bounding_box();

        // Drawing on the matrix can't fail (Error = Infallible), so `unwrap()` never panics
        /* 1px red border around the whole panel */
        screen.into_styled(BORDER_STYLE).draw(matrix).unwrap();

        /* Time centered on the panel */
        Text::with_text_style(&self.time, screen.center(), TIME_FONT, CENTERED).draw(matrix).unwrap();
    }

    /// Method for fetching data
    pub fn fetch_data(&mut self) {
        let (date, time) = get_date_time();
        self.date = date;
        self.time = time;
    }
}

impl ClockState { 
    /// ClockState constructor
    pub fn new(config: ClockConfig) -> Self {
        /* 
        * data is initialized only if the state is enabled
        * If in the future ClockData needs config, use ClockData::new(&config)
        */
        // `bool::then` returns Some(ClockData::new()) if enabled, None otherwise
        let data = config.is_enabled().then(ClockData::new);

        ClockState {
            data,
            config
        }
    }
}

// ================================================================= 
//    Static functions                                             |
// ================================================================= 

fn get_date_time() -> (String, String) {
    let now = Local::now();
    
    let time = now.format("%H:%M:%S").to_string();
    let date = now.format("%Y-%m-%d").to_string();

    (date, time)
}
