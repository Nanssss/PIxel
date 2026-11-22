//! The app itself: builds the states, then runs the frame loop displaying them
//! one after the other.

use anyhow::{Context, Result};
use tokio::signal::unix::{SignalKind, signal};
use tokio::time::{Duration, Instant, MissedTickBehavior, interval};
use tracing::{info, warn};

use crate::config::AppConfig;
use crate::matrix::Matrix;
use crate::states::calendar::CalendarState;
use crate::states::clock::ClockState;
use crate::states::weather::WeatherState;
use crate::states::{AppContext, State};

/// Number of frames drawn per second
const FRAME_RATE: u64 = 30;

/// Time between two frames
const FRAME_PERIOD: Duration = Duration::from_nanos(1_000_000_000 / FRAME_RATE);

/// How long each state stays on screen
const DISPLAY_TIME: Duration = Duration::from_secs(2);

// ===== Frame loop =====

/// Runs the app: builds it, then draws frames until the process is asked to stop.
///
/// # Errors
/// If the app can't start (e.g. the matrix can't be created). Once started, it only
/// returns `Ok`: runtime problems (network...) are handled by each state.
pub async fn run() -> Result<()> {
    // First thing: from now on, a stop signal ends the loop cleanly instead of killing the process
    let mut terminate = signal(SignalKind::terminate()).context("failed to listen for SIGTERM")?;
    let mut interrupt = signal(SignalKind::interrupt()).context("failed to listen for SIGINT")?;

    let mut app = App::new().await?;

    // Ticks on a fixed time grid: unlike `sleep`, the frame rate doesn't drift
    let mut frame_clock = interval(FRAME_PERIOD);
    // Frames missed while late are dropped, not drawn in a burst to catch up
    frame_clock.set_missed_tick_behavior(MissedTickBehavior::Skip);

    loop {
        // Wait for the next frame, unless a stop signal arrives first
        tokio::select! {
            biased; // polled in order: a stop signal always wins over the next frame
            _ = terminate.recv() => { info!("SIGTERM received"); break }
            _ = interrupt.recv() => { info!("SIGINT received"); break }
            _ = frame_clock.tick() => {}
        }

        app.rotate_if_due();
        app.draw();
    }

    info!("Shutting down");
    Ok(())
}

// ===== App =====

/// The running app: the states of the rotation, and the matrix they are drawn on.
struct App {
    /// States of the rotation, in display order. Never empty (see `build_states`).
    states: Vec<Box<dyn State>>,
    /// Index in `states` of the state currently on screen
    current: usize,
    /// When the current state came on screen
    last_shown: Instant,
    matrix: Matrix,
}

impl App {
    /// Builds the app: creates the matrix, loads the config, then builds the states.
    /// Starts on the first state of the rotation.
    ///
    /// # Errors
    /// If the HTTP client or the matrix can't be created.
    async fn new() -> Result<Self> {
        let matrix = Matrix::new().context("failed to init the LED matrix")?;

        let config = AppConfig::load_or_default();
        info!("config:\n{config:#?}");

        // Only needed while the states are built: each task keeps its own client handle
        let context = AppContext::new()?;

        // The network states start their background fetch task here
        let states = build_states(config, &context).await;

        Ok(Self {
            states,
            current: 0,
            last_shown: Instant::now(),
            matrix,
        })
    }

    /// Draws one frame: the current state, or a black screen if it has nothing to
    /// display (its data just expired, or nothing is ready yet, e.g. no network).
    fn draw(&mut self) {
        // No `is_ready` check: a state with nothing to display draws nothing
        self.states[self.current].draw(&mut self.matrix);

        // `show` clears the hidden buffer after each frame: each frame starts black
        self.matrix.show();
    }

    /// Moves on to the next state once the current one has been on screen for
    /// `DISPLAY_TIME`.
    fn rotate_if_due(&mut self) {
        if self.last_shown.elapsed() >= DISPLAY_TIME {
            self.next_state();
        }
    }

    /// Switches to the next state that is ready to be displayed.
    ///
    /// Never waits: a state that isn't ready is skipped until the next rotation.
    /// If no other state is ready, the current one stays for another `DISPLAY_TIME`.
    fn next_state(&mut self) {
        let count = self.states.len();

        // Every state after the current one, wrapping around, then the current one
        let next = (1..=count)
            .map(|offset| (self.current + offset) % count)
            .find(|&index| self.states[index].is_ready());

        if let Some(index) = next {
            self.current = index;
        }
        self.last_shown = Instant::now();
    }
}

// ===== Helpers =====

/// Builds the states of the rotation, in display order: adding a state to the app
/// only takes a new block here.
///
/// A state turned off in the config, or that failed to start, is left out. If none
/// is left, the clock is used as a fallback: the returned list is never empty.
async fn build_states(config: AppConfig, context: &AppContext) -> Vec<Box<dyn State>> {
    let mut states: Vec<Box<dyn State>> = Vec::new();

    if let Some(clock) = ClockState::new(config.clock) {
        states.push(Box::new(clock));
    }
    if let Some(weather) = WeatherState::new(config.weather, context) {
        states.push(Box::new(weather));
    }
    if let Some(calendar) = CalendarState::new(config.calendar, context).await {
        states.push(Box::new(calendar));
    }

    if states.is_empty() {
        warn!("No state is enabled! Falling back to Clock.");
        states.push(Box::new(ClockState::default()));
    }

    states
}
