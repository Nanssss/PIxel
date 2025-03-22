use crate::app::states::clock::*;
use crate::app::states::weather::*;
use reqwest::blocking::Client;
use std::sync::Arc;

enum AppState {
    Clock(ClockData),
    Weather(WeatherData),
}

pub struct AppContext {
    pub client: Arc<Client>,
}

impl AppContext {
    pub fn new() -> Self {
        AppContext {
            client: Arc::new(Client::new()),
        }
    }
}

pub struct App {
    context: AppContext,
    state: AppState,
}

impl App {
    pub fn new() -> Self {
        let context = AppContext::new();
        let state = AppState::Weather(WeatherData::new(&context));
        // state: AppState::Clock(ClockData::new()),
        App { 
            context,
            state,
        }
    }

    pub fn draw(&self) {
        match &self.state {
            AppState::Clock(data) => data.draw(),
            AppState::Weather(data) => data.draw(),
        }
    }

    pub fn fetch_data(&mut self) {
        match &mut self.state {
            AppState::Clock(data) => data.fetch_data(),
            AppState::Weather(data) => data.fetch_data(&self.context),
        }
    }
}
