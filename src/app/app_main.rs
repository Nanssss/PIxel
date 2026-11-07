use tokio::time::{sleep, Duration}; // <-- use tokio's async sleep
use crate::app::states::app_state::App;

pub async fn app_main() {
    println!("Hello from app_main!");

    let mut app = App::new().await;

    loop {
        app.draw();
        app.fetch_data().await;
        sleep(Duration::from_millis(1000)).await; // use tokio async sleep

        app.next_state();
    }
}
