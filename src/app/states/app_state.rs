use crate::EXECUTABLE_DIR;
use crate::app::matrix::Matrix;
use crate::app::states::State;
use crate::app::states::clock::*;
use crate::app::states::weather::*;
use crate::app::states::gtasks::*;
use reqwest::Client;
use std::sync::Arc;
use std::fs;
use tracing::{info, warn};
use anyhow::{Context, Result};
use serde::Deserialize;

/// Config file path
const CONFIG_FILE_PATH: &str = "in/config.toml";

// =================================================================
//   AppContext part                                               |
// =================================================================

/// Struct used to store context used by different states
pub struct AppContext {
    pub client: Arc<Client>,
}

/// Method implementations for AppContext
impl AppContext {
    /// AppContext constructor
    pub fn new() -> Self {
        AppContext {
            client: Arc::new(Client::new()),
        }
    }
}

// ================================================================= 
//    AppConfig part                                                |
// ================================================================= 

/// Struct used to match the TOML file
// `Default` can be derived because every field type implements `Default`
#[derive(Debug, Clone, Default, Deserialize)]
pub struct AppConfig {
    pub clock: ClockConfig,
    pub weather: WeatherConfig,
    pub gtasks: GTasksConfig,
}

impl AppConfig {
    /// Loads the config from the config.toml file, or falls back to the default
    /// config (and logs why) if it can't be loaded.
    pub fn load_or_default() -> Self {
        // `{e:#}` prints the whole anyhow context chain on one line
        Self::load()
            .inspect_err(|e| warn!("No config loaded, using defaults: {e:#}"))
            .unwrap_or_default()
    }

    /// Reads and parses the config.toml file: only propagates errors,
    /// `load_or_default` decides what to do with them.
    ///
    /// # Errors
    /// If the file can't be read, or isn't a valid config.
    fn load() -> Result<Self> {
        let config_file_path = EXECUTABLE_DIR.join(CONFIG_FILE_PATH);

        // `with_context` takes a closure: the message is only built if there is an error
        let content = fs::read_to_string(&config_file_path)
            .with_context(|| format!("failed to read {config_file_path:?}"))?;

        toml::from_str(&content)
            .with_context(|| format!("failed to deserialize {config_file_path:?}"))
    }
}

// ================================================================= 
//    App part                                                     |
// ================================================================= 

/// App struct
pub struct App {
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
    pub async fn new() -> Self {
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
        let matrix = Matrix::new();

        /* Return constructed App, starting on the first state of the rotation */
        App {
            states,
            current: 0,
            matrix,
        }
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
    pub fn draw(&mut self) {
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
    pub fn next_state(&mut self) {
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
