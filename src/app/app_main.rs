use tokio::time::{sleep, Duration}; // <-- use tokio's async sleep
use crate::app::states::app_state::App;

/// How long each state stays on screen
const DISPLAY_TIME: Duration = Duration::from_millis(2000);

pub async fn app_main() {
    // Each state fetches its data once while the app is built
    let mut app = App::new().await;

    loop {
        app.draw();
        sleep(DISPLAY_TIME).await; // use tokio async sleep

        // Refreshes the data of the next state, then switches to it
        app.next_state().await;
    }
}
