use std::{thread, time};

use crate::app::states::app_state::App;

pub fn app_main() {
    println!("Hello from app_main!");

    let mut app = App::new();

    loop {
        app.draw();
        app.fetch_data();
        thread::sleep(time::Duration::from_millis(1000));
        app.next_state();
    }
}
