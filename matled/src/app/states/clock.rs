use chrono::Local;

pub struct ClockData {
    time: String,
    date: String,
}

pub fn hello_from_clock() {
    println!("Hello from clock!");

    let now = Local::now();
    
    // ClockData {
    //     time: now.format("%H:%M:%S").to_string(),
    //     time: now.format("%Y:%m:%d").to_string(),
    // }
    let time = now.format("%H:%M:%S").to_string();
    let date = now.format("%Y:%m:%d").to_string();

    println!("[Clock] - It is now {} | {}", date, time);
}
