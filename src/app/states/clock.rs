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

/* Default values for config */
impl Default for ClockConfig {
    fn default() -> Self {
        ClockConfig {
            enabled: true,
        }
    }
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

impl ClockState { 
    /* ClockState constructor */
    pub fn new(config: Option<ClockConfig>) -> Self {
        /* 
        * data is initialized only if config is Some(x)
        * If in the future ClockData needs config, use config.as_ref().map(|cfg| ClockData::new(cfg))
        */
        let data = config.is_some().then(ClockData::new);

        ClockState {
            data,
            config: config.unwrap_or_default(),
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
