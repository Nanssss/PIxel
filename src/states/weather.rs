//! The weather state: shows the forecast of the day for the place set in the config.
//!
//! The forecast is fetched in the background (see `feed.rs`) from the Open-Meteo API.

use anyhow::{Context, Result};
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::Duration;
use tracing::{info, trace};

use crate::feed::{Feed, Fetcher};
use crate::matrix::Matrix;
use crate::states::{AppContext, State};

/// Weather API (Open-Meteo: free, no key needed). Doc: <https://open-meteo.com/en/docs>
const WEATHER_API_URL: &str = "https://api.open-meteo.com/v1/forecast";

/// Daily values asked to the weather API: must match the fields of `DailyData`
const DAILY_VARIABLES: &str =
    "weather_code,apparent_temperature_max,apparent_temperature_min,precipitation_sum";

/// Config of the weather: the `[weather]` section of `config.toml`.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct WeatherConfig {
    enabled: bool,
    /// Place the forecast is fetched for, in decimal degrees (north is positive)
    latitude: f64,
    /// Place the forecast is fetched for, in decimal degrees (east is positive)
    longitude: f64,
}

/// Values used when the section, or one of its fields, is missing: the weather at
/// the coordinates below.
impl Default for WeatherConfig {
    fn default() -> Self {
        Self {
            enabled: true,
            latitude: 43.57,
            longitude: 1.46,
        }
    }
}

/// Forecast of the day, as shown on the panel.
#[derive(Debug)]
struct Forecast {
    /// Weather of the day: tells which icon to draw
    condition: Condition,
    /// Maximum apparent ("feels like") temperature of the day, in °C
    apparent_temperature_max: f32,
    /// Minimum apparent ("feels like") temperature of the day, in °C
    apparent_temperature_min: f32,
    /// Sum of the day's precipitation (rain, snow, hail), in mm
    precipitation_sum: f32,
}

/// Weather of the day, grouped by the icon that shows it.
///
/// The weather API gives a far more detailed *WMO code* (light, moderate or heavy
/// rain, freezing rain, showers...): see `Condition::from_wmo`.
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
    /// Codes used by the weather API: <https://open-meteo.com/en/docs#weathervariables>
    // Keeps the table aligned
    #[rustfmt::skip]
    fn from_wmo(code: u8) -> Self {
        match code {
            0 | 1               => Self::Clear,        // clear, mainly clear
            2                   => Self::PartlyCloudy,
            3                   => Self::Overcast,
            45 | 48             => Self::Fog,          // fog, rime fog
            51..=57             => Self::Drizzle,      // light to dense, freezing
            61..=67 | 80..=82   => Self::Rain,         // slight to heavy, freezing | showers
            71..=77 | 85 | 86   => Self::Snow,         // slight to heavy, grains | showers
            95..=99             => Self::Thunderstorm, // with or without hail
            _                   => Self::Unknown,
        }
    }
}

/// Shows the forecast of the day, fetched by a background task (see `Feed`).
pub struct WeatherState {
    /// Latest forecast published by the background task
    forecast: Feed<Forecast>,
}

// =================================================================
//    Methods implementation                                       |
// =================================================================

impl WeatherState {
    /// Creates the state, and starts the background task fetching the forecast.
    ///
    /// Returns `None` if the state is turned off in the config.
    /// Nothing is fetched here: the state isn't ready until the first forecast arrives.
    pub fn new(config: WeatherConfig, context: &AppContext) -> Option<Self> {
        if !config.enabled {
            return None;
        }

        let fetcher = ForecastFetcher {
            client: context.client.clone(),
            latitude: config.latitude,
            longitude: config.longitude,
        };

        Some(Self {
            forecast: Feed::spawn(fetcher),
        })
    }
}

impl State for WeatherState {
    /// Ready as soon as the background task published a forecast that is still fresh
    fn is_ready(&self) -> bool {
        self.forecast.latest().is_some()
    }

    /// Placeholder until the forecast is drawn: fills the panel in green.
    fn draw(&mut self, matrix: &mut Matrix) {
        // No fresh forecast: draw nothing, the frame stays black
        let Some(_forecast) = self.forecast.latest() else {
            return;
        };

        let Ok(()) = matrix.clear(Rgb888::GREEN);
    }
}

// =================================================================
//    Background task                                              |
// =================================================================

/// Fetches the forecast for the background task of the state.
///
/// Owns everything a fetch needs: the task outlives the function that starts it.
struct ForecastFetcher {
    /// HTTP client shared with the other states
    client: Client,
    /// Latitude of the place, from `WeatherConfig`
    latitude: f64,
    /// Longitude of the place, from `WeatherConfig`
    longitude: f64,
}

impl Fetcher for ForecastFetcher {
    type Data = Forecast;

    const NAME: &'static str = "weather forecast";

    /// The forecast only changes a few times a day
    const REFRESH_PERIOD: Duration = Duration::from_mins(15);

    /// A forecast of the day stays useful for a while, but not until the next day
    const MAX_AGE: Duration = Duration::from_hours(2);

    /// Fetches the forecast of the day from the weather API
    async fn fetch(&mut self) -> Result<Forecast> {
        let response: WeatherResponse = self
            .client
            .get(WEATHER_API_URL)
            // Two calls, each one appends its parameters: the items of an array must
            // all have the same type (numbers here, text below)
            .query(&[("latitude", self.latitude), ("longitude", self.longitude)])
            .query(&[
                ("daily", DAILY_VARIABLES),
                ("timezone", "auto"), // the day starts at midnight, local time of the place
                ("forecast_days", "1"), // today only
            ])
            .send()
            .await
            .context("failed to send weather API request")?
            // Without this, a "400 Bad Request" would be parsed as if it was a forecast
            .error_for_status()
            .context("weather API returned an error")?
            .json()
            .await
            .context("failed to deserialize weather API response")?;
        trace!("Weather parsed response: {response:?}");

        let forecast = Forecast::from(response.daily);

        info!(
            "Forecast: {:?}, apparent temperature {}°C max, {}°C min, {} mm of precipitation",
            forecast.condition,
            forecast.apparent_temperature_max,
            forecast.apparent_temperature_min,
            forecast.precipitation_sum
        );

        Ok(forecast)
    }
}

// =================================================================
//    Private types                                                |
// =================================================================

/// Response of the weather API (only the fields used: the others are ignored).
#[derive(Debug, Deserialize)]
struct WeatherResponse {
    daily: DailyData,
}

/// Daily values of the response: a list for each, with one element per forecast day.
/// We ask for a single day: `[_; 1]` makes serde refuse any other length.
#[derive(Debug, Deserialize)]
struct DailyData {
    /// WMO code, see `Condition::from_wmo`
    weather_code: [u8; 1],
    /// In °C
    apparent_temperature_max: [f32; 1],
    /// In °C
    apparent_temperature_min: [f32; 1],
    /// In mm
    precipitation_sum: [f32; 1],
}

/// Turns what the API sent into what the state displays.
impl From<DailyData> for Forecast {
    fn from(daily: DailyData) -> Self {
        Self {
            condition: Condition::from_wmo(daily.weather_code[0]),
            apparent_temperature_max: daily.apparent_temperature_max[0],
            apparent_temperature_min: daily.apparent_temperature_min[0],
            precipitation_sum: daily.precipitation_sum[0],
        }
    }
}

// =================================================================
//    Tests                                                        |
// =================================================================

#[cfg(test)]
mod tests {
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
        assert_eq!(forecast.apparent_temperature_max, 4.5);
        assert_eq!(forecast.apparent_temperature_min, -3.6);
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
        assert_eq!(Condition::from_wmo(82), Condition::Rain); // rain showers
        assert_eq!(Condition::from_wmo(86), Condition::Snow); // snow showers
        assert_eq!(Condition::from_wmo(99), Condition::Thunderstorm);
        assert_eq!(Condition::from_wmo(42), Condition::Unknown); // not a code of the API
    }
}
