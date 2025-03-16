use serde_json::json;
use serde::{Deserialize, Deserializer};
use reqwest::blocking::Client;
use std::fs;

const WEATHER_API_BASE_URL: &str = "https://api.open-meteo.com/v1/forecast";

pub struct WeatherData {
    weather_code:       String,
    temp_max:           String,
    temp_min:           String,
    precipitation_sum:  String,
}

impl WeatherData {
    pub fn new() -> Self {
        let data= get_weather();
        WeatherData {
            weather_code:       code_to_weather(data.weather_code),
            temp_max:           data.apparent_temperature_max.to_string(),
            temp_min:           data.apparent_temperature_min.to_string(),
            precipitation_sum:  data.precipitation_sum.to_string(),
        }
    }

    pub fn draw(&self) {
        println!("Drawing Weather");
    }

    pub fn fetch_data(&mut self) {
        let data= get_weather();
        self.weather_code =         code_to_weather(data.weather_code);
        self.temp_max =             data.apparent_temperature_max.to_string();
        self.temp_min =             data.apparent_temperature_min.to_string();
        self.precipitation_sum =    data.precipitation_sum.to_string();
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
fn get_weather() -> DailyData {
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
    // println!("{response:?}");

    // let body = response.unwrap().text().unwrap();
    let parsed: WeatherResponse = response
        .unwrap()
        .json::<WeatherResponse>()
        .unwrap();
    // println!("{parsed:?}");

    parsed.daily
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

#[allow(dead_code)] // necessary to avoid warning because fields only used for deserialization
#[derive(Debug, Deserialize)]
struct WeatherResponse {
    latitude:               f64,
    longitude:              f64,
    generationtime_ms:      f64,
    utc_offset_seconds:     i32,
    timezone:               String,
    timezone_abbreviation:  String,
    elevation:              f64,
    daily_units:            DailyUnits,
    daily:                  DailyData,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct DailyUnits {
    time:                       String,
    weather_code:               String,
    apparent_temperature_max:   String,
    apparent_temperature_min:   String,
    precipitation_sum:          String,
}

#[allow(dead_code)]
#[derive(Debug, Deserialize)]
struct DailyData {
    #[serde(deserialize_with = "first_element_or_default")] // function used when deserializing, to take 1rst element of Vec<.>
    time:                       String,
    #[serde(deserialize_with = "first_element_or_default")]
    weather_code:               i32,
    #[serde(deserialize_with = "first_element_or_default")]
    apparent_temperature_max:   f64,
    #[serde(deserialize_with = "first_element_or_default")]
    apparent_temperature_min:   f64,
    #[serde(deserialize_with = "first_element_or_default")]
    precipitation_sum:          f64,
}

fn first_element_or_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Clone + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let vec: Vec<T> = Vec::deserialize(deserializer)?; // Désérialisation en Vec<T>
    Ok(vec.into_iter().next().unwrap_or_default()) // Prend le premier élément ou renvoie le défaut
}

fn code_to_weather(code: i32) -> String {
    let mut description: String = "Invalid weather code".to_string();

    let wmo_code_file =
        fs::read_to_string("./src/app/states/res/wmo_codes.json")
        .expect("Failed to read weather code JSON file");
    let wmo_code_json: serde_json::Value =
        serde_json::from_str(&wmo_code_file)
        .expect("Invalid weather code JSON");

    if let Some(entry) = wmo_code_json.get(code.to_string()) {
        description = entry
            .get("day")
            .and_then(|d| d.get("description"))
            .unwrap().
            to_string();
    }
    description
}
