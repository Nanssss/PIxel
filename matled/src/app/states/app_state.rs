use crate::app::states::clock::*;
use crate::app::states::weather::*;
use reqwest::blocking::Client;
use std::sync::Arc;

/* First state on boot */
const START_STATE:AppStatesEnum = AppStatesEnum::Weather;


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
}

/* Struct storing all states */
struct AppStates {
    clock: ClockData,
    weather: WeatherData,
}

/* App struct */
pub struct App {
    context: AppContext,
    states: AppStates,
    current_state: AppStatesEnum,
}

/* App methods implementation */
impl App {

    /* App constructor */
    pub fn new() -> Self {
        let context = AppContext::new();
        let weather = WeatherData::new(&context);
        let clock = ClockData::new();

        App { 
            context,
            states: AppStates {
                clock,
                weather,
            },
            current_state: START_STATE,
        }
    }

    /* Call draw() method of corresponding state */
    pub fn draw(&self) {
        match &self.current_state {
            AppStatesEnum::Clock => self.states.clock.draw(),
            AppStatesEnum::Weather => self.states.weather.draw(),
        }
    }

    /* Call fetch_data() method of corresponding state */
    pub fn fetch_data(&mut self) {
        match &mut self.current_state{
            AppStatesEnum::Clock => self.states.clock.fetch_data(),
            AppStatesEnum::Weather => self.states.weather.fetch_data(&self.context),
        }
    }

    /* Switch to next state */
    pub fn next_state(&mut self) {
        match &self.current_state {
            AppStatesEnum::Clock => self.current_state = AppStatesEnum::Weather,
            AppStatesEnum::Weather => self.current_state = AppStatesEnum::Clock,
        }
    }
}
