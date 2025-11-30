use serde_json::json;
use serde::{Deserialize, Deserializer};
use reqwest::Client;
use std::fs;
use crate::app::states::app_state::AppContext;
use tracing::{trace, info, error};
use anyhow::{Result, Context};

const WEATHER_API_BASE_URL: &str = "https://api.open-meteo.com/v1/forecast";

/* Public struct containing weather data */
#[derive(Default)] // used to be able to fill with default values
pub struct WeatherData {
    weather_summary:    String,     // concise weather summary
    temp_max:           String,     // maximum daily temperature
    temp_min:           String,     // minimum daily temperature
    precipitation_sum:  String,     // sum of daily precipitations (including rain, snow, hail)
    request_url:        String,     // full request URL to send to weather API
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl WeatherData {

    /* WeatherData constructor */
    pub async fn new(context: &AppContext) -> Self {
        /* Initialize some data in the struct */
        let weather_init = match init() {
            Ok(w) => w,
            Err(e) => {
                /* Handle the error */
                error!("Failed to initialize weather data: {:?}", e);
                return WeatherData::default();
            }
        };

        /* Get data from the weather API */
        let data = match get_weather(&context.client, &weather_init.request_url).await {
            Ok(d) => d,
            Err(e) => {
                /* Handle the error */
                error!("Failed to fetch weather data: {:?}", e);
                return weather_init;
            }
        };

        /* Return WeatherData struct */
        WeatherData {
            weather_summary:    code_to_weather(data.weather_code).unwrap_or_else(|e| {
                error!("Failed to parse weather code: {:?}", e);
                "Unknown".to_string()
            }),
            temp_max:           data.apparent_temperature_max.to_string(),
            temp_min:           data.apparent_temperature_min.to_string(),
            precipitation_sum:  data.precipitation_sum.to_string(),
            ..weather_init // fill other fields from init data
        }
    }

    /* Method for drawing data to the screen */
    pub fn draw(&self) {
        info!("[WEATHER] - Summary: {}, Max Temp: {}, Min Temp: {}, Precipitation Sum: {}",
            self.weather_summary, self.temp_max, self.temp_min, self.precipitation_sum);
    }

    /* Method for fetching data from public API */
    pub async fn fetch_data(&mut self, context: &AppContext) {
        /* Get data from the weather API */
        match get_weather(&context.client, &self.request_url).await {
            Ok(data) => {
                self.weather_summary = code_to_weather(data.weather_code).unwrap_or_else(|e| {
                    error!("Failed to parse weather code: {:?}", e);
                    "Unknown".to_string()
                });
                self.temp_max = data.apparent_temperature_max.to_string();
                self.temp_min = data.apparent_temperature_min.to_string();
                self.precipitation_sum = data.precipitation_sum.to_string();
            }
            Err(e) => {
                /* Handle the error */
                error!("Failed to fetch weather data: {:?}", e);
            }
        }
    }
}


// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/* Function that initializes the WeatherData struct */
fn init() -> Result<WeatherData> {
    /* Create a JSON with the request parameters */
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

    /* Construct query params URL from the json */
    let params_url = json_to_query_string(&params)?;

    /* Concatenate base URL and query params */
    let full_url = format!("{}?{}", WEATHER_API_BASE_URL, params_url);

    /* Return initialized WeatherData struct */
    Ok(WeatherData {
        request_url: full_url,
        ..Default::default()            // use default values for other fields
    })
}


/* Function to get data from public weather API */
async fn get_weather(client: &Client, url: &String) -> Result<DailyData> {
    /* Send HTTP GET request */
    let response = client
        .get(url)
        .header("User-Agent", "reqwest")
        .send()
        .await
        .context("failed to send weather API request")?;
    trace!("Weather response: {response:?}");

    /* Deserialize response JSON */
    let parsed: WeatherResponse = response
        .json::<WeatherResponse>()
        .await
        .context("failed to deserialize weather API response")?;
    trace!("Weather parsed response: {parsed:?}");

    Ok(parsed.daily)
}


/* Transforms json to query parameters URL */
fn json_to_query_string(params_json: &serde_json::Value) -> Result<String> {
    let obj = params_json.as_object()
        .context("expected a json object for weather parameters")?;
    
    let query_string = obj
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
        .join("&");
    
    Ok(query_string)
}


/* Function to take out first element of Vec<> */
fn first_element_or_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Clone + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let vec: Vec<T> = Vec::deserialize(deserializer)?;  // deserialization as a Vec<T>
    Ok(vec.into_iter().next().unwrap_or_default())      // take out first element or send back Default
}


/* Function used to translate weather WMO code to weather String using json code file */
fn code_to_weather(code: i32) -> Result<String> {
    /* Open json file as String */
    let wmo_code_file = fs::read_to_string("./src/app/states/res/wmo_codes.json")
        .context("failed to read weather code JSON file")?;

    /* Convert String as json Value */
    let wmo_code_json: serde_json::Value = serde_json::from_str(&wmo_code_file)
        .context("failed to parse weather code JSON")?;

    /* Get description field ("entry": "day": "description": "xx") */
    let description = wmo_code_json
        .get(code.to_string())
        .and_then(|entry| entry.get("day"))
        .and_then(|d| d.get("description"))
        .and_then(|v| v.as_str())
        .unwrap_or("Unknown")
        .to_string();

    Ok(description)
}


// ================================================================= 
//    Private types                                                |
// ================================================================= 

/* The following types are used for deserialization */

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

#[allow(dead_code)] // necessary to avoid warning because fields only used for deserialization
#[derive(Debug, Deserialize)]
struct DailyUnits {
    time:                       String,
    weather_code:               String,
    apparent_temperature_max:   String,
    apparent_temperature_min:   String,
    precipitation_sum:          String,
}

#[allow(dead_code)] // necessary to avoid warning because fields only used for deserialization
#[derive(Debug, Deserialize)]
struct DailyData {
    #[serde(deserialize_with = "first_element_or_default")] // function used to deserialize, to take out first element of Vec<.>
    time:                       String,
    #[serde(deserialize_with = "first_element_or_default")]
    weather_code:               i32,                        // weather wmo codes: https://gist.github.com/stellasphere/9490c195ed2b53c707087c8c2db4ec0c
    #[serde(deserialize_with = "first_element_or_default")]
    apparent_temperature_max:   f64,
    #[serde(deserialize_with = "first_element_or_default")]
    apparent_temperature_min:   f64,
    #[serde(deserialize_with = "first_element_or_default")]
    precipitation_sum:          f64,
}
