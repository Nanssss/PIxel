use chrono::Local;
use tracing::info;
use serde::Deserialize;

/* Public struct containing clock data */
pub struct ClockData {
    date: String,
    time: String,
}

/* Public struct containing clock config */
#[derive(Debug, Clone, Deserialize)]
pub struct ClockConfig {
    enabled: bool,
}


/* Public struct containing global clock state */
pub struct ClockState {
    pub data: ClockData,
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

    /* Method fro drawing data to the screen */
    pub fn draw(&self) {
        info!("[CLOCK] - Date: {}, Time: {}", self.date, self.time);
    }

    /* Method for fetching data */
    pub fn fetch_data(&mut self) {
        let (date, time) = get_date_time();
        self.date = date;
        self.time = time;
    }
}

impl ClockConfig {
    /* ClockConfig constructor */
    pub fn new() -> Self {
        ClockConfig {
            enabled: true,
        }
    }

    /* ClockConfig setter */
    pub fn set(&mut self, state: bool) {
        self.enabled = state;
    }
}

impl ClockState { 
    /* ClockState constructor */
    pub fn new() -> Self {
        ClockState {
            data:   ClockData::new(),
            config: ClockConfig::new(),
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
