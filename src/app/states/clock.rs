use chrono::Local;
use tracing::info;
use serde::Deserialize;
use crate::app::matrix::matrix::*;
use embedded_graphics::{
    mono_font::{ascii::FONT_6X10, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::PrimitiveStyle,
    text::{Alignment, Baseline, Text, TextStyleBuilder},
};

/* Public struct containing clock config */
#[derive(Debug, Clone, Deserialize)]
pub struct ClockConfig {
    enabled: bool,
}

/* Default trait for config */
impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            enabled: true,
        }
    }
}

impl ClockConfig {
    pub fn set_state(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn get_state(&self) -> bool {
        self.enabled
    }
}

/* Public struct containing clock data */
pub struct ClockData {
    date: String,
    time: String,
}


/* Public struct containing global clock state */
pub struct ClockState {
    pub data: Option<ClockData>,
    pub config: ClockConfig,
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl ClockData {

    /* ClockData constructor */
    pub fn new() -> Self {
        let (date, time) = get_date_time();
        ClockData {
            date,
            time
        }
    }

    /* Method for drawing data to the screen */
    pub fn draw(&self, matrix: &mut Matrix) {
        info!("[CLOCK] - Date: {}, Time: {}", self.date, self.time);

        /* Draw on an offscreen canvas, then swap it to avoid flickering */
        // TODO: a new offscreen canvas is allocated on each draw (memory leak), reuse one instead
        let mut canvas = matrix.create_offscreen_canvas();
        canvas.clear();

        /* 1px red border around the whole panel */
        // Drawing on a Canvas can't fail (Error = Infallible), so the Result is ignored
        let _ = canvas
            .bounding_box()
            .into_styled(PrimitiveStyle::with_stroke(Rgb888::RED, 1))
            .draw(&mut canvas);

        /* Time centered on the panel, with a built-in 6x10 mono font */
        // Character style = font + color, text style = position of the text around its anchor point
        let character_style = MonoTextStyle::new(&FONT_6X10, Rgb888::WHITE);
        let text_style = TextStyleBuilder::new()
            .alignment(Alignment::Center) // horizontally centered on the anchor
            .baseline(Baseline::Middle)   // vertically centered on the anchor
            .build();
        let _ = Text::with_text_style(&self.time, canvas.bounding_box().center(), character_style, text_style)
            .draw(&mut canvas);

        matrix.swap_on_vsync(canvas);
    }

    /* Method for fetching data */
    pub fn fetch_data(&mut self) {
        let (date, time) = get_date_time();
        self.date = date;
        self.time = time;
    }
}

impl ClockState { 
    /* ClockState constructor */
    pub fn new(config: Option<ClockConfig>) -> Self {
        /* 
        * data is initialized only if config is Some(x) and enabled
        * If in the future ClockData needs config, use config.as_ref().map(|cfg| ClockData::new(cfg))
        */
        let config = config.unwrap_or_default();
        let mut data = None;
        
        if config.get_state() {
            data = Some(ClockData::new());
        }

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
    let date = now.format("%Y:%m:%d").to_string();

    (date, time)
}
