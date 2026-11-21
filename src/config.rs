//! The app configuration, read from the `config.toml` file.
//!
//! Each state declares its own part of the config in its module (`ClockConfig`...):
//! this file only gathers them, and knows how to load the whole.

use anyhow::{Context, Result};
use serde::Deserialize;
use std::fs;
use tracing::warn;

use crate::EXECUTABLE_DIR;
use crate::states::clock::ClockConfig;
use crate::states::gtasks::GTasksConfig;
use crate::states::weather::WeatherConfig;

/// Config file path, relative to the executable directory
const CONFIG_FILE_PATH: &str = "in/config.toml";

/// The whole config: one section per state (`[clock]`, `[weather]`...).
// `Default` can be derived because every field type implements `Default`.
// `#[serde(default)]`: a section missing from the file takes its default config,
// instead of making the whole file invalid.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(default)]
pub struct AppConfig {
    pub clock: ClockConfig,
    pub weather: WeatherConfig,
    pub gtasks: GTasksConfig,
}

impl AppConfig {
    /// Loads the config from the config.toml file, or falls back to the default
    /// config (and logs why) if it can't be loaded.
    pub fn load_or_default() -> Self {
        // `{e:#}` prints the whole anyhow context chain on one line
        Self::load()
            .inspect_err(|e| warn!("No config loaded, using defaults: {e:#}"))
            .unwrap_or_default()
    }

    /// Reads and parses the config.toml file: only propagates errors,
    /// `load_or_default` decides what to do with them.
    ///
    /// # Errors
    /// If the file can't be read, or isn't a valid config.
    fn load() -> Result<Self> {
        let config_file_path = EXECUTABLE_DIR.join(CONFIG_FILE_PATH);

        // `with_context` takes a closure: the message is only built if there is an error
        let content = fs::read_to_string(&config_file_path)
            .with_context(|| format!("failed to read {config_file_path:?}"))?;

        toml::from_str(&content)
            .with_context(|| format!("failed to deserialize {config_file_path:?}"))
    }
}
