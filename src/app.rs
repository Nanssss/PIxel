//! The app itself: builds the states, then runs the frame loop displaying them
//! one after the other.

use anyhow::{Context, Result};
use tokio::signal::unix::{signal, SignalKind};
use tokio::time::{interval, Duration, Instant, MissedTickBehavior};
use tracing::{info, warn};
use crate::config::AppConfig;
use crate::matrix::Matrix;
use crate::states::{AppContext, State};
use crate::states::clock::ClockState;
use crate::states::gtasks::GTasksState;
use crate::states::weather::WeatherState;

/// Number of frames drawn per second
const FRAME_RATE: u64 = 30;

/// Time between two frames
const FRAME_PERIOD: Duration = Duration::from_millis(1000 / FRAME_RATE);

/// How long each state stays on screen
const DISPLAY_TIME: Duration = Duration::from_millis(2000);

// =================================================================
//    Frame loop                                                   |
// =================================================================

/// Runs the app: builds it, then draws frames until the process is asked to stop.
///
/// Stopping means SIGTERM (sent by `systemctl stop` or `kill`) or SIGINT (Ctrl+C).
/// Without handling them, the process would be killed on the spot, skipping every
/// `Drop`: the panel would keep showing its last frame (or a single stuck row).
///
/// # Errors
/// If the app can't start (e.g. the matrix can't be created). Once started, it only
/// returns `Ok`: runtime problems (network...) are handled by each state.
pub async fn run() -> Result<()> {
    // Listen for the stop signals before anything else: once registered, a signal
    // no longer kills the process, it is only delivered to these streams.
    let mut terminate = signal(SignalKind::terminate()).context("failed to listen for SIGTERM")?;
    let mut interrupt = signal(SignalKind::interrupt()).context("failed to listen for SIGINT")?;

    // States that need the network start their background fetch task while the app is built
    let mut app = App::new().await?;

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
        // Wait for the next frame (the very first tick completes immediately), unless
        // a stop signal arrives first. `select!` waits for all three at once, and runs
        // the branch of the first one that completes.
        tokio::select! {
            _ = frame_clock.tick() => {}
            _ = terminate.recv() => { info!("SIGTERM received"); break }
            _ = interrupt.recv() => { info!("SIGINT received"); break }
        }

        // Time-based rather than counting frames
        if state_start.elapsed() >= DISPLAY_TIME {
            // Never waits for the network: the data is fetched by background tasks
            app.next_state();
            state_start = Instant::now();
        }

        app.draw();
    }

    info!("Shutting down");
    // `app` is dropped at the end of this function, and its `Matrix` with it: the C
    // library then turns the LEDs off. The background tasks are stopped when the
    // tokio runtime shuts down, once `main` returns.
    Ok(())
}

// =================================================================
//    App part                                                     |
// =================================================================

/// App struct
struct App {
    /// States of the rotation, in display order. Never empty (see `build_states`).
    // A `Vec` can only hold values of a single type: `Box<dyn State>` is that type,
    // a pointer to "some state", whatever its concrete type is.
    states:     Vec<Box<dyn State>>,
    /// Index in `states` of the state currently on screen
    current:    usize,
    matrix:     Matrix,
}

/// App methods implementation
impl App {
    /// App constructor
    ///
    /// # Errors
    /// If the matrix can't be created.
    async fn new() -> Result<Self> {
        /* Build config */
        let config = AppConfig::load_or_default();
        info!("config:\n{config:?}");

        /* Build context */
        // Only needed while the states are built: each background task keeps its own handle on it
        let context = AppContext::new();

        /* Build states */
        // States that need the network start their background fetch task here.
        let states = build_states(config, &context).await;

        /* Build matrix */
        let matrix = Matrix::new().context("failed to init the LED matrix")?;

        /* Return constructed App, starting on the first state of the rotation */
        Ok(App {
            states,
            current: 0,
            matrix,
        })
    }

    /// Call draw() method of the current state, or blank the screen if that
    /// state has nothing to display.
    ///
    /// The current state isn't ready in two cases:
    /// - its data just got too old: it is skipped at the next state switch;
    /// - no state can be displayed at all (e.g. only the weather is enabled, and
    ///   there is no network): this lasts until some data arrives.
    ///
    /// Blanking the screen avoids leaving outdated data on it in the meantime.
    fn draw(&mut self) {
        // Borrows `self.states` only: `self.matrix` can still be borrowed mutably
        // below, as the compiler tracks the borrow of each field separately.
        let state = &self.states[self.current];

        if state.is_ready() {
            state.draw(&mut self.matrix);
        }

        // Display the frame that was just drawn. If nothing was drawn, the frame is
        // black: `show` clears the hidden buffer after each frame, so there is no
        // need to fill it here.
        self.matrix.show();
    }

    /// Switch to the next state that is ready to be displayed.
    ///
    /// Never waits: the data is fetched by the background task of each state. A state
    /// that isn't ready is skipped, and gets its chance again on the next rotation.
    /// If no other state can be displayed, we stay on the current one.
    fn next_state(&mut self) {
        let count = self.states.len();

        // Candidates, in order: every state after the current one, wrapping around
        // at the end of the list (`% count`), and finally the current one itself.
        let next = (1..=count)
            .map(|offset| (self.current + offset) % count)
            .find(|&index| self.states[index].is_ready());

        /* No candidate is ready: keep the current state */
        if let Some(index) = next {
            self.current = index;
        }
    }
}

// =================================================================
//    Static functions                                             |
// =================================================================

/// Builds the states of the rotation, in display order: adding a state to the app
/// only takes a new block here.
///
/// A state turned off in the config, or that failed to start, is left out. If none
/// is left, the clock is used as a fallback: the returned list is never empty.
async fn build_states(config: AppConfig, context: &AppContext) -> Vec<Box<dyn State>> {
    // The type annotation matters: it makes each `Box::new(...)` below convert from
    // a `Box<ClockState>` (or any other state) to a `Box<dyn State>`.
    let mut states: Vec<Box<dyn State>> = Vec::new();

    // Each state takes ownership of its own part of the config (no clone needed)
    if let Some(clock) = ClockState::new(config.clock) {
        states.push(Box::new(clock));
    }
    if let Some(weather) = WeatherState::new(config.weather, context) {
        states.push(Box::new(weather));
    }
    if let Some(gtasks) = GTasksState::new(config.gtasks, context).await {
        states.push(Box::new(gtasks));
    }

    /* No state is enabled: fall back to the clock, whatever the config says */
    if states.is_empty() {
        warn!("No state is enabled! Falling back to Clock.");
        states.push(Box::new(ClockState));
    }

    states
}
