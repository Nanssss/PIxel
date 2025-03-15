use crate::app::states::clock::*;
use crate::app::states::weather::*;

enum AppState {
    Clock(ClockData),
    Weather(WeatherData),
}

pub struct App {
    state: AppState,
}

impl App {
    pub fn new() -> Self {
        App { 
            // state: AppState::Clock(ClockData::new()),
            state: AppState::Weather(WeatherData::new()),
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
            AppState::Weather(data) => data.fetch_data(),
        }
    }
}
