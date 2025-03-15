use serde_json::json;
use reqwest::blocking::Client;

const WEATHER_API_BASE_URL: &str = "https://api.open-meteo.com/v1/forecast";

pub struct WeatherData {
    temp: String,
    /* ... */
}

impl WeatherData {
    pub fn new() -> Self {
        let temp= get_weather();
        WeatherData {
            temp,
        }
    }

    pub fn draw(&self) {
        println!("Drawing Weather");
        println!("Temperature: {}", self.temp);
    }

    pub fn fetch_data(&mut self) {
        let temp = get_weather();
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
    cargo add tokio --features full
    cargo add reqwest --features json, blocking
*/
fn get_weather() -> String {
    /*
        URL part
    */
    // let correct_url = "https://api.open-meteo.com/v1/forecast?latitude=52.52&longitude=13.41&daily=weather_code,temperature_2m_max,temperature_2m_min,precipitation_sum&timezone=Europe%2FBerlin&forecast_days=1";

    // create a JSON with the request parameters
    let params = json!({
        "latitude": "43.57",
        "longitude": "1.46",
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

    let params_url = json_to_query_string(&params);
    let full_url = format!("{}?{}", WEATHER_API_BASE_URL, params_url);
    // println!("full_url: {full_url}");

    /*
        Request part
    */

    let client = Client::new();
    let response = client
        .get(full_url)
        .header("User-Agent", "reqwest")
        .send();
    
    println!("{response:?}");

    let body = response.unwrap().text();

    println!("{body:?}");

    let temp = "25".to_string();
    temp
}

fn json_to_query_string(params_json: &serde_json::Value) -> String {
    params_json.as_object()
        .expect("Expected a json object")
        .iter()
        .map(|(key, value)| {
            let clean_value = value
                .to_string()
                .replace(" ", "")
                .replace("\"", "")
                .replace("[", "")
                .replace("]", "");
            format!("{}={}", key, clean_value)
        })
        .collect::<Vec<String>>()
        .join("&")
}
