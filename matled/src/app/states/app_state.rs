use crate::app::states::clock::*;

enum AppState {
    Clock(ClockData),
}

pub struct App {
    state: AppState,
}
