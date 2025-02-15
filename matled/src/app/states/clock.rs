use chrono::Local;

pub struct ClockData {
    date: String,
    time: String,
}

impl ClockData {
    pub fn new() -> Self {
        let (date, time) = get_date_time();
        ClockData {
            date,
            time
        }
    }

    pub fn draw(&self) {
        println!("Drawing Clock");
        println!("Date: {} | Time: {}", self.date, self.time);
    }

    pub fn fetch_data(&mut self) {
        let (date, time) = get_date_time();
        self.date = date;
        self.time = time;
    }
}

/* 
    Static functions
*/
fn get_date_time() -> (String, String) {
    let now = Local::now();
    
    let time = now.format("%H:%M:%S").to_string();
    let date = now.format("%Y:%m:%d").to_string();

    (date, time)
}
