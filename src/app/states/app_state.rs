use crate::app::states::clock::*;
use crate::app::states::weather::*;
use crate::app::states::gtasks::*;
use reqwest::Client;
use std::sync::Arc;
use std::fs;
use tracing::{info, warn};
use serde::Deserialize;

/* First state on boot */
const START_STATE:AppStatesEnum = AppStatesEnum::Weather;

/* Config file path */
const CONFIG_FILE_PATH: &str = "in/";

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
    /* Function that loads the config from the config.toml file */
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
        let config = AppConfig::load();
        info!("config:\n{config:?}");

        let context =   AppContext::new();
        let weather =   WeatherState::new(config.as_ref().map(|cfg| cfg.weather.clone()), &context).await;
        let clock =     ClockState::new(config.as_ref().map(|cfg| cfg.clock.clone())); // todo: pass config here
        let gtasks =    GTasksState::new(config.as_ref().map(|cfg| cfg.gtasks.clone()), &context).await;

        App { 
            context,
            states: AppStates {
                clock,
                weather,
                gtasks,
            },
            current_state: START_STATE,
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
        match &self.current_state {
            AppStatesEnum::Clock    => self.current_state = AppStatesEnum::Weather,
            AppStatesEnum::Weather  => self.current_state = AppStatesEnum::GTasks,
            AppStatesEnum::GTasks   => self.current_state = AppStatesEnum::Clock,
        }
    }
}

