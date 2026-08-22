use crate::app::states::clock::*;
use crate::app::states::weather::*;
use crate::app::states::gtasks::*;
use reqwest::Client;
use std::sync::Arc;
use std::fs;
use tracing::{info, warn};
use serde::Deserialize;

/* First state on boot */
const START_STATE:AppStatesEnum = AppStatesEnum::Clock;

/* Config file path */
const CONFIG_FILE_PATH: &str = "in/";

const NB_STATES: u8 = 3;

// ================================================================= 
//   AppContext part                                               |
// ================================================================= 

/* Struct used to store context used by different states */
pub struct AppContext {
    pub client: Arc<Client>,
}

/* Method implementations for AppContext */
impl AppContext {
    /* AppContext constructor */
    pub fn new() -> Self {
        AppContext {
            client: Arc::new(Client::new()),
        }
    }
}

// ================================================================= 
//    AppConfig part                                                |
// ================================================================= 

/* Struct used to match the TOML file */
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub clock: ClockConfig,
    pub weather: WeatherConfig,
    pub gtasks: GTasksConfig,
}

impl AppConfig {
    /* Loads the config from the config.toml file */
    pub fn load() -> Option<Self> {
        let file_path = format!("{}config.toml", CONFIG_FILE_PATH);

        let content = fs::read_to_string(&file_path).inspect_err(|e| {
            warn!("Error while reading {file_path}:\n{e}");
        }).ok()?;

        let config: AppConfig= toml::from_str(&content).inspect_err(|e| {
            warn!("failed to deserialize {file_path}:\n{e}");
        }).ok()?;   

        Some(config)
    }
}

// ================================================================= 
//    App part                                                     |
// ================================================================= 

/* Enum declaring state variants */
enum AppStatesEnum {
    Clock,
    Weather,
    GTasks,
}

impl AppStatesEnum {
    /* Return the next state in the application cycle */
    pub fn next(&self) -> Self {
        match self {
            AppStatesEnum::Clock   => AppStatesEnum::Weather,
            AppStatesEnum::Weather => AppStatesEnum::GTasks,
            AppStatesEnum::GTasks  => AppStatesEnum::Clock,
        }
    }

    /* Return if state is enabled */
    pub fn is_enabled(&self, states: &AppStates) -> bool {
        match self {
            AppStatesEnum::Clock   => states.clock.config.get_state(),
            AppStatesEnum::Weather => states.weather.config.enabled,
            AppStatesEnum::GTasks  => states.gtasks.config.enabled,
        }
    }
}

/* Struct storing all states */
struct AppStates {
    clock:      ClockState,
    weather:    WeatherState,
    gtasks:     GTasksState,
}

/* App struct */
pub struct App {
    context:        AppContext,
    states:         AppStates,
    current_state:  AppStatesEnum,
}

/* App methods implementation */
impl App {
    /* App constructor */
    pub async fn new() -> Self {
        /* Build config */
        let config = AppConfig::load();
        info!("config:\n{config:?}");

        /* Build context */
        let context =   AppContext::new();

        /* Build states */
        let clock =     ClockState::new(config.as_ref().map(|cfg| cfg.clock.clone()));
        let weather =   WeatherState::new(config.as_ref().map(|cfg| cfg.weather.clone()), &context).await;
        let gtasks =    GTasksState::new(config.as_ref().map(|cfg| cfg.gtasks.clone()), &context).await;
        let mut states = AppStates {
            clock,
            weather,
            gtasks,
        };

        /* Get current state */
        let current_state = select_first_state(&mut states);

        /* Return constructed App */
        App { 
            context,
            states,
            current_state,
        }
    }

    /* Call draw() method of corresponding state */
    pub fn draw(&self) {
        match &self.current_state {
            AppStatesEnum::Clock    => self.states.clock.data.as_ref().unwrap().draw(),
            AppStatesEnum::Weather  => self.states.weather.data.as_ref().unwrap().draw(),
            AppStatesEnum::GTasks   => self.states.gtasks.data.as_ref().unwrap().draw(),
        }
    }

    /* Call fetch_data() method of corresponding state */
    pub async fn fetch_data(&mut self) {
        match &mut self.current_state{
            AppStatesEnum::Clock    => self.states.clock.data.as_mut().unwrap().fetch_data(),
            AppStatesEnum::Weather  => self.states.weather.data.as_mut().unwrap().fetch_data(&self.context).await,
            AppStatesEnum::GTasks   => self.states.gtasks.data.as_mut().unwrap().fetch_data(&self.context).await,
        }
    }

    /* Switch to next state */
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
        states.clock.config.set_state(true);
    }

    AppStatesEnum::Clock
}
