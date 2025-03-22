use crate::app::states::clock::*;
use crate::app::states::weather::*;
use std::sync::Arc;

enum AppState {
    Clock(ClockData),
    Weather(WeatherData),
}

enum AppContext {
    client: Arc<Client>,
}

impl AppContext {
    pub fn new() -> Self {
        AppContext {
            client: Arc<Client::new()>,
        }
    }
}

pub struct App {
    context: AppContext,
    state: AppState,
}

impl App {
    pub fn new() -> Self {
        App { 
            context: AppContext::new(),
            // state: AppState::Clock(ClockData::new()),
            state: AppState::Weather(WeatherData::new(&context)),
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
