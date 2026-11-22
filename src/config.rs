//! The app configuration, read from the `config.toml` file.
//!
//! Each state declares its own part of the config in its module (`ClockConfig`...):
//! this file only gathers them, and knows how to load the whole.
//!
//! Every config struct refuses unknown keys (`deny_unknown_fields`): a typo such as
//! `enabeld` is reported, instead of being silently ignored.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use tracing::warn;

use crate::paths;
use crate::states::calendar::CalendarConfig;
use crate::states::clock::ClockConfig;
use crate::states::weather::WeatherConfig;

/// The whole config: one section per state (`[clock]`, `[weather]`...).
/// A section missing from the file takes its default values.
#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct AppConfig {
    pub clock: ClockConfig,
    pub weather: WeatherConfig,
    pub calendar: CalendarConfig,
}

impl AppConfig {
    /// Loads the config from the config.toml file, or falls back to the default
    /// config (and logs why) if it can't be loaded.
    pub fn load_or_default() -> Self {
        Self::load()
            .inspect_err(|e| warn!("No config loaded, using defaults: {e:#}"))
            .unwrap_or_default()
    }

    /// Reads and parses the config.toml file.
    ///
    /// # Errors
    /// If the file can't be read, or isn't a valid config.
    fn load() -> Result<Self> {
        let config_file_path = paths::config_file();

        let content = fs::read_to_string(&config_file_path)
            .with_context(|| format!("failed to read {}", config_file_path.display()))?;

        toml::from_str(&content)
            .with_context(|| format!("failed to deserialize {}", config_file_path.display()))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn partial_config_is_valid() {
        // `enabled` and `longitude`: from `WeatherConfig::default()`, other sections too
        let config = r#"
            [weather]
            latitude = 48.85
        "#;
        assert!(toml::from_str::<AppConfig>(config).is_ok());
    }

    #[test]
    fn misspelled_field_is_refused() {
        let config = r#"
            [weather]
            enabeld = false
        "#;
        assert!(toml::from_str::<AppConfig>(config).is_err());
    }

    #[test]
    fn unknown_section_is_refused() {
        let config = r#"
            [wether]
            enabled = false
        "#;
        assert!(toml::from_str::<AppConfig>(config).is_err());
    }
}
