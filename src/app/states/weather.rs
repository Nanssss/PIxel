use std::sync::Arc;
use serde_json::json;
use serde::{Deserialize, Deserializer};
use reqwest::Client;
use tokio::time::Duration;
use crate::app::states::app_state::{AppContext, Status};
use crate::app::states::feed::{Feed, Fetcher};
use crate::app::matrix::{Color, Matrix};
use tracing::{trace, info, error};
use anyhow::{Result, Context};

const WEATHER_API_BASE_URL: &str = "https://api.open-meteo.com/v1/forecast";
// Include the file in the final executable
const WMO_CODES_JSON: &str = include_str!("./res/wmo_codes.json");

/// Public struct containing weather config
#[derive(Debug, Clone, Deserialize)]
pub struct WeatherConfig {
    pub enabled: bool,
}

/// Default values for config
impl Default for WeatherConfig {
    fn default() -> Self {
        WeatherConfig{
            enabled: true,
        }
    }
}

impl WeatherConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Forecast of the day, as received from the weather API
struct Forecast {
    summary:            String,     // concise weather summary
    temp_max:           String,     // maximum daily temperature
    temp_min:           String,     // minimum daily temperature
    precipitation_sum:  String,     // sum of daily precipitations (including rain, snow, hail)
}


/// Public struct containing global weather state
///
/// The state never fetches anything itself: a background task (see `Feed::spawn`) does,
/// and the state only reads the latest forecast it published.
pub struct WeatherState {
    forecast:   Feed<Forecast>,     // latest forecast published by the background task
    config:     WeatherConfig,      // what the user asked for: never modified
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl WeatherState { 
    /// WeatherState constructor: starts the background task fetching the forecast.
    ///
    /// Nothing is fetched here: the state is `Waiting` until the first forecast arrives.
    pub fn new(config: WeatherConfig, context: &AppContext) -> Self {
        /* The state starts disabled, and is only switched on if everything goes well */
        let mut state = WeatherState {
            forecast: Feed::Disabled,
            config,
        };

        /* A state turned off in the config stays disabled: its task is never started */
        if !state.config.is_enabled() {
            return state;
        }

        /* If in the future the request needs config, pass it &state.config */
        match build_request_url() {
            Ok(url) => {
                // The background task must own everything it uses:
                // `Arc::clone` only copies a pointer to the shared client.
                let fetcher = ForecastFetcher {
                    client: Arc::clone(&context.client),
                    url,
                };

                /* Start the background task: returns immediately */
                state.forecast = Feed::spawn(fetcher);
            }
            Err(e) => {
                /* Unrecoverable error: the state stays `Disabled` */
                error!("Failed to initialize Weather, disabling state.\n{e:#}");
            }
        }

        state
    }

    /// Returns the runtime status of the state, deduced from what the task published so far
    pub fn status(&self) -> Status {
        self.forecast.status()
    }

    /// Method for drawing the state to the screen
    pub fn draw(&self, matrix: &mut Matrix) {
        // The app only draws `Ready` states, so the `else` is a safety net
        let Some(_forecast) = self.forecast.latest() else { return };

        let color = Color { r: 0, g: 255, b: 0 };
        matrix.fill(color);
    }
}

// ================================================================= 
//    Background task                                              |
// ================================================================= 

/// Fetches the forecast for the background task of the state.
///
/// Owns everything a fetch needs: the task that runs it (see `Feed::spawn`)
/// lives longer than the function that started it, so it can't borrow anything.
struct ForecastFetcher {
    client: Arc<Client>,    // HTTP client shared with the other states
    url:    String,         // full request URL, built once at startup
}

impl Fetcher for ForecastFetcher {
    type Data = Forecast;

    const NAME: &'static str = "weather data";

    /// The forecast only changes a few times a day
    const REFRESH_PERIOD: Duration = Duration::from_secs(15 * 60);

    /// Fetches the forecast of the day from the weather API
    async fn fetch(&mut self) -> Result<Forecast> {
        // `?` returns the error to the caller (the task), which decides what to do with it
        let data = get_weather(&self.client, &self.url).await?;

        let summary = code_to_weather(data.weather_code).unwrap_or_else(|e| {
            error!("Failed to parse weather code: {e:#}");
            "Unknown".to_string()
        });
        let forecast = Forecast {
            summary,
            temp_max:           data.apparent_temperature_max.to_string(),
            temp_min:           data.apparent_temperature_min.to_string(),
            precipitation_sum:  data.precipitation_sum.to_string(),
        };

        info!("[WEATHER] - Summary: {}, Max Temp: {}, Min Temp: {}, Precipitation Sum: {}",
            forecast.summary, forecast.temp_max, forecast.temp_min, forecast.precipitation_sum);

        Ok(forecast)
    }
}

// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/// Builds the full request URL sent to the weather API
fn build_request_url() -> Result<String> {
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
    let params_url = json_to_query_string(&params)
        .context("failed to build the weather request")?;

    /* Concatenate base URL and query params */
    Ok(format!("{}?{}", WEATHER_API_BASE_URL, params_url))
}


/// Function to get data from public weather API
async fn get_weather(client: &Client, url: &str) -> Result<DailyData> {
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


/// Transforms json to query parameters URL
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


/// Function to take out first element of `Vec<T>`
fn first_element_or_default<'de, T, D>(deserializer: D) -> Result<T, D::Error>
where
    T: Default + Clone + Deserialize<'de>,
    D: Deserializer<'de>,
{
    let vec: Vec<T> = Vec::deserialize(deserializer)?;  // deserialization as a Vec<T>
    Ok(vec.into_iter().next().unwrap_or_default())      // take out first element or send back Default
}


/// Function used to translate weather WMO code to weather String using json code file
fn code_to_weather(code: i32) -> Result<String> {
    /* Convert String as json Value */
    let wmo_code_json: serde_json::Value = serde_json::from_str(WMO_CODES_JSON)
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
