use crate::app::states::clock::*;

enum AppState {
    Clock(ClockData),
}

pub struct App {
    state: AppState,
}

impl App {
    pub fn new() -> Self {
        App { 
            state: AppState::Clock(ClockData::new()),
        }
    }

    pub fn draw(&self) {
        match &self.state {
            AppState::Clock(data) => data.draw(),
        }
    }

    pub fn fetch_data(&mut self) {
        match &mut self.state {
            AppState::Clock(data) => data.fetch_data(),
        }
    }
}
