use serde::Deserialize;
use reqwest::Client;
use tokio::time::Duration;
use crate::states::{AppContext, State};
use crate::feed::{Feed, Fetcher};
use crate::matrix::Matrix;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use tracing::{trace, info};
use anyhow::{Result, Context};

/// Weather API (Open-Meteo: free, no key needed). Doc: https://open-meteo.com/en/docs
const WEATHER_API_URL: &str = "https://api.open-meteo.com/v1/forecast";

/// Daily values asked to the weather API: must match the fields of `DailyData`
const DAILY_VARIABLES: &str = "weather_code,apparent_temperature_max,apparent_temperature_min,precipitation_sum";

/// Public struct containing weather config
// `#[serde(default)]`: a field missing from the config file takes its value from
// the `Default` implementation below, instead of making the whole file invalid.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct WeatherConfig {
    enabled: bool,
    /// Place the forecast is fetched for, in decimal degrees (north is positive)
    pub latitude: f64,
    /// Place the forecast is fetched for, in decimal degrees (east is positive)
    pub longitude: f64,
}

/// Default values for config
impl Default for WeatherConfig {
    fn default() -> Self {
        WeatherConfig{
            enabled: true,
            latitude: 43.57,
            longitude: 1.46,
        }
    }
}

/// Forecast of the day
#[derive(Debug)]
struct Forecast {
    condition:          Condition,  // weather of the day: tells which icon to draw
    temp_max:           f32,        // maximum daily temperature, in °C
    temp_min:           f32,        // minimum daily temperature, in °C
    precipitation_sum:  f32,        // sum of daily precipitations (rain, snow, hail), in mm
}

/// Weather of the day
///
/// The weather API describes it with a *WMO code*, which is far more detailed (light,
/// moderate or heavy rain, freezing rain, rain showers...): see `Condition::from_wmo`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Condition {
    Clear,
    PartlyCloudy,
    Overcast,
    Fog,
    Drizzle,
    Rain,
    Snow,
    Thunderstorm,
    /// A WMO code that isn't in the table
    Unknown,
}

impl Condition {
    /// Returns the condition matching a WMO weather code.
    ///
    /// Codes used by the weather API: https://open-meteo.com/en/docs#weathervariables
    fn from_wmo(code: u8) -> Self {
        match code {
            0 | 1               => Condition::Clear,        // clear, mainly clear
            2                   => Condition::PartlyCloudy,
            3                   => Condition::Overcast,
            45 | 48             => Condition::Fog,          // fog, rime fog
            51..=57             => Condition::Drizzle,      // light to dense, freezing
            61..=67 | 80..=82   => Condition::Rain,         // slight to heavy, freezing | showers
            71..=77 | 85 | 86   => Condition::Snow,         // slight to heavy, grains | showers
            95..=99             => Condition::Thunderstorm, // with or without hail
            _                   => Condition::Unknown,
        }
    }
}


/// Public struct containing global weather state
///
/// The state never fetches anything itself: a background task (see `Feed::spawn`) does,
/// and the state only reads the latest forecast it published.
pub struct WeatherState {
    forecast:   Feed<Forecast>,     // latest forecast published by the background task
}


// =================================================================
//    Methods implementation                                       |
// =================================================================

impl WeatherState {
    /// WeatherState constructor: starts the background task fetching the forecast.
    ///
    /// Returns `None` if the state is turned off in the config.
    /// Nothing is fetched here: the state isn't ready until the first forecast arrives.
    pub fn new(config: WeatherConfig, context: &AppContext) -> Option<Self> {
        /* A state turned off in the config is never built: its task is never started */
        if !config.enabled {
            return None;
        }

        // The background task must own everything it uses: cloning the client
        // only copies a handle, the connections stay shared (see `AppContext`).
        let fetcher = ForecastFetcher {
            client:     context.client.clone(),
            latitude:   config.latitude,
            longitude:  config.longitude,
        };

        /* Start the background task: returns immediately */
        Some(WeatherState {
            forecast: Feed::spawn(fetcher),
        })
    }
}

impl State for WeatherState {
    /// Ready as soon as the background task published a forecast that is still fresh
    fn is_ready(&self) -> bool {
        self.forecast.latest().is_some()
    }

    /// Method for drawing the state to the screen
    fn draw(&self, matrix: &mut Matrix) {
        // The app only draws a state that is ready, so the `else` is a safety net
        // (the forecast may expire right between the two calls)
        let Some(_forecast) = self.forecast.latest() else { return };

        let Ok(()) = matrix.clear(Rgb888::GREEN);
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
    client:     Client,     // HTTP client shared with the other states
    latitude:   f64,        // place the forecast is fetched for (see `WeatherConfig`)
    longitude:  f64,
}

impl Fetcher for ForecastFetcher {
    type Data = Forecast;

    const NAME: &'static str = "weather data";

    /// The forecast only changes a few times a day
    const REFRESH_PERIOD: Duration = Duration::from_secs(15 * 60);

    /// A forecast of the day stays useful for a while, but not until the next day
    const MAX_AGE: Duration = Duration::from_secs(2 * 60 * 60);

    /// Fetches the forecast of the day from the weather API
    async fn fetch(&mut self) -> Result<Forecast> {
        /* Request parameters */
        let latitude = self.latitude.to_string();
        let longitude = self.longitude.to_string();
        let query_params = &[
            ("latitude",        latitude.as_str()),
            ("longitude",       longitude.as_str()),
            ("daily",           DAILY_VARIABLES),
            ("timezone",        "auto"),    // the day starts at midnight, local time of the place
            ("forecast_days",   "1"),       // today only
        ];

        /* Send HTTP GET request, and deserialize the JSON of the response */
        let response: WeatherResponse = self.client
            .get(WEATHER_API_URL)
            .query(query_params)
            .send()
            .await
            .context("failed to send weather API request")?
            // Without this, a "400 Bad Request" would be read as if it was a forecast
            .error_for_status()
            .context("weather API returned an error")?
            .json()
            .await
            .context("failed to deserialize weather API response")?;
        trace!("Weather parsed response: {response:?}");

        let forecast = Forecast::from(response.daily);

        // No "[WEATHER]" prefix needed: the log line already shows the module
        info!("Forecast: {:?}, {}°C max, {}°C min, {} mm of precipitation",
            forecast.condition, forecast.temp_max, forecast.temp_min, forecast.precipitation_sum);

        Ok(forecast)
    }
}

// =================================================================
//    Private types                                                |
// =================================================================

/* The following types are used for deserialization */

/// Response of the weather API.
// Only the fields we use are declared: serde ignores the other ones of the JSON.
#[derive(Debug, Deserialize)]
struct WeatherResponse {
    daily: DailyData,
}

/// Daily values of the response. The API returns a list for each of them, with one
/// element per forecast day: we ask for a single day, so one element.
#[derive(Debug, Deserialize)]
struct DailyData {
    weather_code:               [u8; 1],    // WMO code, see `Condition::from_wmo`
    apparent_temperature_max:   [f32; 1],   // °C
    apparent_temperature_min:   [f32; 1],   // °C
    precipitation_sum:          [f32; 1],   // mm
}

/// Turns what the API sent into what the state displays.
///
/// Implementing `From` gives us `Into` for free.
impl From<DailyData> for Forecast {
    fn from(daily: DailyData) -> Self {
        Forecast {
            condition:          Condition::from_wmo(daily.weather_code[0]),
            temp_max:           daily.apparent_temperature_max[0],
            temp_min:           daily.apparent_temperature_min[0],
            precipitation_sum:  daily.precipitation_sum[0],
        }
    }
}

// =================================================================
//    Tests                                                        |
// =================================================================

// `#[cfg(test)]`: this module is only compiled by `cargo test`, never in the app.
#[cfg(test)]
mod tests {
    // Gives access to everything of the parent module, private items included
    use super::*;

    /// A real response of the weather API (shortened)
    const RESPONSE: &str = r#"{
        "latitude": 52.512604,
        "timezone": "Europe/Berlin",
        "daily_units": { "time": "iso8601", "weather_code": "wmo code" },
        "daily": {
            "time": ["2026-11-15"],
            "weather_code": [61],
            "apparent_temperature_max": [4.5],
            "apparent_temperature_min": [-3.6],
            "precipitation_sum": [0.00]
        }
    }"#;

    #[test]
    fn forecast_is_read_from_the_api_response() {
        let response: WeatherResponse = serde_json::from_str(RESPONSE).unwrap();
        let forecast = Forecast::from(response.daily);

        assert_eq!(forecast.condition, Condition::Rain);
        assert_eq!(forecast.temp_max, 4.5);
        assert_eq!(forecast.temp_min, -3.6);
        assert_eq!(forecast.precipitation_sum, 0.0);
    }

    #[test]
    fn response_without_any_day_is_refused() {
        let response = r#"{ "daily": {
            "weather_code": [],
            "apparent_temperature_max": [],
            "apparent_temperature_min": [],
            "precipitation_sum": []
        } }"#;

        assert!(serde_json::from_str::<WeatherResponse>(response).is_err());
    }

    #[test]
    fn wmo_codes_are_grouped_by_condition() {
        assert_eq!(Condition::from_wmo(0), Condition::Clear);
        assert_eq!(Condition::from_wmo(48), Condition::Fog);
        assert_eq!(Condition::from_wmo(57), Condition::Drizzle);
        assert_eq!(Condition::from_wmo(67), Condition::Rain);
        assert_eq!(Condition::from_wmo(82), Condition::Rain);     // rain showers
        assert_eq!(Condition::from_wmo(86), Condition::Snow);     // snow showers
        assert_eq!(Condition::from_wmo(99), Condition::Thunderstorm);
        assert_eq!(Condition::from_wmo(42), Condition::Unknown);  // not a code of the API
    }
}
