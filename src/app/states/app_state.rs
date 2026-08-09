use crate::app::states::clock::*;
use crate::app::states::weather::*;
use crate::app::states::gtasks::*;
use anyhow::Context;
use reqwest::Client;
use std::sync::Arc;
use std::fs;
use tracing::{info, warn};
use serde::Deserialize;
use anyhow::Result;

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
    weather:    WeatherData,
    gtasks:     GTasksData,
}

/* App struct */
pub struct App {
    context:        AppContext,
    states:         AppStates,
    current_state:  AppStatesEnum,
}

/* Struct used to match the TOML file */
#[derive(Debug, Clone, Deserialize)]
pub struct AppConfig {
    pub clock: ClockConfig,
}

/* App methods implementation */
impl App {

    /* App constructor */
    pub async fn new() -> Self {
        // Todo: Add config retrieving here
        let config = load_config_file();
        info!("config:\n{config:?}");

        let context =   AppContext::new();
        let weather =   WeatherData::new(&context).await;
        let clock =     ClockState::new(); // todo: pass config here
        let gtasks =    GTasksData::new(&context).await;

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
            AppStatesEnum::Clock    => self.states.clock.data.draw(),
            AppStatesEnum::Weather  => self.states.weather.draw(),
            AppStatesEnum::GTasks   => self.states.gtasks.draw(),
        }
    }

    /* Call fetch_data() method of corresponding state */
    pub async fn fetch_data(&mut self) {
        match &mut self.current_state{
            AppStatesEnum::Clock    => self.states.clock.data.fetch_data(),
            AppStatesEnum::Weather  => self.states.weather.fetch_data(&self.context).await,
            AppStatesEnum::GTasks   => self.states.gtasks.fetch_data(&self.context).await,
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

fn load_config_file() -> Option<AppConfig> {
    let file_path = format!("{}config.toml", CONFIG_FILE_PATH);
    // let content = fs::read_to_string(file_path).context("failed to read {file_path}")?;
    let content = fs::read_to_string(&file_path).inspect_err(|e| {
        warn!("Error while reading {file_path}:\n{e}");
    }).ok()?;

    let config: AppConfig= toml::from_str(&content).inspect_err(|e| {
        warn!("failed to deserialize {file_path}:\n{e}");
    }).ok()?;   

    Some(config)
}
