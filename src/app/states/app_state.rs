use crate::EXECUTABLE_DIR;
use crate::app::matrix::Matrix;
use crate::app::states::clock::*;
use crate::app::states::weather::*;
use crate::app::states::gtasks::*;
use reqwest::Client;
use std::sync::Arc;
use std::fs;
use tracing::{info, warn};
use anyhow::{Context, Result};
use serde::Deserialize;

/// First state on boot
const START_STATE:StateId = StateId::Clock;

/// Config file path
const CONFIG_FILE_PATH: &str = "in/config.toml";

const NB_STATES: u8 = 3;

// =================================================================
//   Status part                                                   |
// =================================================================

/// Runtime status of a state: tells the app what it may do with it.
///
/// The config says what the user *wants* (`enabled`), the status says what is
/// actually *happening*.
// `Copy`: the enum is as cheap to copy as an integer, so it can be returned by value.
// `PartialEq`/`Eq`: allow comparing two statuses with `==` and `!=`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    /// Turned off in the config, or hit an unrecoverable error:
    /// never fetched nor displayed.
    Disabled,
    /// Enabled, but nothing to display for now: no data yet (e.g. no network at
    /// boot), or data that got too old (e.g. network lost for hours).
    /// Skipped until its background task gets some fresh data.
    Waiting,
    /// Has something to display.
    Ready,
}

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

/// Enum declaring state variants.
///
/// This is only a *tag* telling which state we are talking about (like an index
/// in an array): the states themselves are stored in `AppStates`.
// `Copy`: a tag is as cheap to copy as an integer, so it is passed by value.
#[derive(Clone, Copy)]
enum StateId {
    Clock,
    Weather,
    GTasks,
}

impl StateId {
    /// Return the next state in the application cycle
    pub fn next(self) -> Self {
        match self {
            StateId::Clock   => StateId::Weather,
            StateId::Weather => StateId::GTasks,
            StateId::GTasks  => StateId::Clock,
        }
    }
}

/// Struct storing all states
struct AppStates {
    clock:      ClockState,
    weather:    WeatherState,
    gtasks:     GTasksState,
}

impl AppStates {
    /// Return the runtime status of the given state
    fn status_of(&self, state: StateId) -> Status {
        match state {
            StateId::Clock   => self.clock.status(),
            StateId::Weather => self.weather.status(),
            StateId::GTasks  => self.gtasks.status(),
        }
    }
}

/// App struct
pub struct App {
    states:         AppStates,
    current_state:  StateId,
    matrix:         Matrix,
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
        let context =   AppContext::new();

        /* Build states */
        // Each state takes ownership of its own part of the config (no clone needed).
        // States that need the network start their background fetch task here.
        let clock =     ClockState::new(config.clock);
        let weather =   WeatherState::new(config.weather, &context);
        let gtasks =    GTasksState::new(config.gtasks, &context).await;
        let mut states = AppStates {
            clock,
            weather,
            gtasks,
        };

        /* Get current state */
        let current_state = select_first_state(&mut states);

        /* Build matrix */
        let matrix = Matrix::new();

        /* Return constructed App */
        App { 
            states,
            current_state,
            matrix,
        }
    }

    /// Call draw() method of corresponding state, or blank the screen if that
    /// state has nothing to display.
    ///
    /// The current state isn't `Ready` in two cases:
    /// - its data just got too old: it is skipped at the next state switch;
    /// - no state can be displayed at all (e.g. only the weather is enabled, and
    ///   there is no network): this lasts until some data arrives.
    ///
    /// Blanking the screen avoids leaving outdated data on it in the meantime.
    pub fn draw(&mut self) {
        if self.states.status_of(self.current_state) == Status::Ready {
            match self.current_state {
                StateId::Clock    => self.states.clock.draw(&mut self.matrix),
                StateId::Weather  => self.states.weather.draw(&mut self.matrix),
                StateId::GTasks   => self.states.gtasks.draw(&mut self.matrix),
            }
        }

        // Display the frame that was just drawn. If nothing was drawn, the frame is
        // black: `show` clears the hidden buffer after each frame, so there is no
        // need to fill it here.
        self.matrix.show();
    }

    /// Switch to the next state that is ready to be displayed.
    ///
    /// Never waits: the data is fetched by the background task of each state. A state
    /// that isn't `Ready` is skipped, and gets its chance again on the next rotation.
    /// If no other state can be displayed, we end up back on the current one.
    pub fn next_state(&mut self) {
        /* Iterate through the states until we find one to display */
        for _ in 0..NB_STATES {
            /* Move to next state */
            self.current_state = self.current_state.next();

            /* Stop searching as soon as we found a state to display */
            if self.states.status_of(self.current_state) == Status::Ready {
                break;
            }
        }
    }
}

// =================================================================
//    Static functions                                             |
// =================================================================

/// Returns the first state that isn't disabled, searching from `START_STATE`.
/// If they are all disabled, the clock is enabled and used as a fallback.
fn select_first_state(states: &mut AppStates) -> StateId {
    let mut state = START_STATE;

    for _ in 0..NB_STATES {
        /* Stop searching as soon as we found a state that isn't disabled */
        if states.status_of(state) != Status::Disabled {
            return state;
        }

        /* Move to next state if the current was disabled */
        state = state.next();
    }

    /* No state is enabled: fall back to Clock state */
    warn!("All states were disabled in config! Falling back to Clock.");
    states.clock.enable();
    StateId::Clock
}
