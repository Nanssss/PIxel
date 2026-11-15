use tokio::time::{interval, Duration, Instant, MissedTickBehavior};
use crate::app::states::app_state::App;

/// Number of frames drawn per second
const FRAME_RATE: u64 = 30;

/// Time between two frames
const FRAME_PERIOD: Duration = Duration::from_millis(1000 / FRAME_RATE);

/// How long each state stays on screen
const DISPLAY_TIME: Duration = Duration::from_millis(2000);

pub async fn app_main() {
    // States that need the network start their background fetch task while the app is built
    let mut app = App::new().await;

    // Frame clock. Unlike `sleep(FRAME_PERIOD)`, an interval schedules its ticks on
    // a fixed time grid (start + n * period): the time spent drawing a frame is not
    // added to the wait, so the frame rate doesn't drift.
    let mut frame_clock = interval(FRAME_PERIOD);
    // If we are late (e.g. a frame took too long to draw), drop the missed frames
    // instead of drawing them all in a row to catch up (the default, `Burst`).
    frame_clock.set_missed_tick_behavior(MissedTickBehavior::Skip);

    // When the current state came on screen
    let mut state_start = Instant::now();

    loop {
        // Wait for the next frame (the very first tick completes immediately)
        frame_clock.tick().await;

        // Time-based rather than counting frames
        if state_start.elapsed() >= DISPLAY_TIME {
            // Never waits for the network: the data is fetched by background tasks
            app.next_state();
            state_start = Instant::now();
        }

        app.draw();
    }
}
