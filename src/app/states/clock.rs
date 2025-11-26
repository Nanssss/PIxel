use chrono::Local;
use tracing::info;

/* Public struct containing clock data */
pub struct ClockData {
    date: String,
    time: String,
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
        info!("\n
        ======================
               [CLOCK]
        | Date  | {}
        | Time  | {}
        ======================\
        ", self.date, self.time);
    }

    /* Method for fetching data */
    pub fn fetch_data(&mut self) {
        let (date, time) = get_date_time();
        self.date = date;
        self.time = time;
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
