use std::collections::HashMap;
use serde::Deserialize;
use serde_json::json;

const WEATHER_API_BASE_URL = "https://api.open-meteo.com/v1/forecast";

pub struct WeatherData {
    temp: String,
    /* ... */
}

impl WeatherData {
    pub fn new() -> Self {
        let (temp) = get_weather();
        WeatherData {
            temp,
        }
    }

    pub fn draw(&self) {
        println!("Drawing Weather");
        println!("Temperature: {}", self.temp);
    }

    pub fn fetch_data(&mut self) {
        let (temp) = get_weather();
        self.temp = temp;
    }
}

/* 
    Static functions
*/
/* 
Notes:
    cargo add serde --features derive
    cargo add serde_json
*/
fn get_weather() -> (String) {
    let mut url = WEATHER_API_BASE_URL;

    // create a JSON with the request parameters
    let params = json!({
        "daily":
        [
            "weather_code",
            "apparent_temperature_max",
            "apparent_temperature_min",
            "precipitation_sum"
        ],
        "timezone": "Europe/Berlin",
        "forecast_days": 1,
    });

    // iterate through the parameters
    if let Value::Object(map) = params {
        for (key, value) in &map {
            println!("Key: {}, Value: {}", key, value);
        }
    }

    let temp = "25".to_string();
    (temp)
}
