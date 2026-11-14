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
const START_STATE:AppStatesEnum = AppStatesEnum::Clock;

/// Config file path
const CONFIG_FILE_PATH: &str = "in/config.toml";

const NB_STATES: u8 = 3;

/// Panic message if the current state has no data. This can't happen: only enabled
/// states are ever selected, and a state is only enabled if its data was created.
const DATA: &str = "the current state is enabled, so it has data";

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

/// Enum declaring state variants
enum AppStatesEnum {
    Clock,
    Weather,
    GTasks,
}

impl AppStatesEnum {
    /// Return the next state in the application cycle
    pub fn next(&self) -> Self {
        match self {
            AppStatesEnum::Clock   => AppStatesEnum::Weather,
            AppStatesEnum::Weather => AppStatesEnum::GTasks,
            AppStatesEnum::GTasks  => AppStatesEnum::Clock,
        }
    }

    /// Return if state is enabled
    pub fn is_enabled(&self, states: &AppStates) -> bool {
        match self {
            AppStatesEnum::Clock   => states.clock.config.is_enabled(),
            AppStatesEnum::Weather => states.weather.config.is_enabled(),
            AppStatesEnum::GTasks  => states.gtasks.config.is_enabled(),
        }
    }
}

/// Struct storing all states
struct AppStates {
    clock:      ClockState,
    weather:    WeatherState,
    gtasks:     GTasksState,
}

/// App struct
pub struct App {
    context:        AppContext,
    states:         AppStates,
    current_state:  AppStatesEnum,
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
        let context =   AppContext::new();

        /* Build states */
        // Each state takes ownership of its own part of the config (no clone needed)
        let clock =     ClockState::new(config.clock);
        let weather =   WeatherState::new(config.weather, &context).await;
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
            context,
            states,
            current_state,
            matrix,
        }
    }

    /// Call draw() method of corresponding state
    pub fn draw(&mut self) {
        match &self.current_state {
            AppStatesEnum::Clock    => self.states.clock.data.as_ref().expect(DATA).draw(&mut self.matrix),
            AppStatesEnum::Weather  => self.states.weather.data.as_ref().expect(DATA).draw(&mut self.matrix),
            AppStatesEnum::GTasks   => self.states.gtasks.data.as_ref().expect(DATA).draw(&mut self.matrix),
        }

        /* Display the frame the state just drew */
        self.matrix.show();
    }

    /// Call fetch_data() method of corresponding state
    pub async fn fetch_data(&mut self) {
        match &mut self.current_state{
            AppStatesEnum::Clock    => self.states.clock.data.as_mut().expect(DATA).fetch_data(),
            AppStatesEnum::Weather  => self.states.weather.data.as_mut().expect(DATA).fetch_data(&self.context).await,
            AppStatesEnum::GTasks   => self.states.gtasks.data.as_mut().expect(DATA).fetch_data(&self.context).await,
        }
    }

    /// Switch to next state
    pub fn next_state(&mut self) {
        /* Iterate through the states until we find an enabled one */
        for _ in 0..NB_STATES {
            /* Move to next state */
            self.current_state = self.current_state.next();

            /* Stop searching as soon as we found an enabled state */
            if self.current_state.is_enabled(&self.states) {
                break;
            }
        }
    }
}

// ================================================================= 
//    Static functions                                             |
// ================================================================= 
fn select_first_state(states: &mut AppStates) -> AppStatesEnum {
    let mut current_state = START_STATE;
    let mut found_enabled = false;

    for _ in 0..NB_STATES {
        /* Stop searching as soon as we found an enabled state */
        if current_state.is_enabled(states) {
            found_enabled = true;
            break;
        }

        /* Move to next state if the current was disabled */
        current_state = current_state.next();
    }

    /* If no state is enabled, fall back to Clock state */
    if !found_enabled {
        warn!("All states were disabled in config! Falling back to Clock.");
        states.clock.config.set_enabled(true);
        // The clock was disabled at startup, so its data was never created
        states.clock.data = Some(ClockData::new());
        return AppStatesEnum::Clock;
    }

    current_state
}
