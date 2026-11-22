//! Where the app finds its files: next to the executable.
//!
//! ```text
//! <executable dir>         (made by `make deploy`)
//! ├─ PIxel                 the executable
//! ├─ in/config.toml        config of the states
//! ├─ in/credentials.json   Google OAuth2 application secret
//! ├─ in/tokencache.json    Google OAuth2 tokens, cached
//! └─ logs/                 daily log files
//! ```

use std::path::PathBuf;
use std::sync::LazyLock;

/// Directory of the executable, whatever the current directory is when it is started
/// (e.g. by systemd). Computed on first use.
pub static EXECUTABLE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf()
});

/// Config of the states (see `config.rs`).
pub fn config_file() -> PathBuf {
    EXECUTABLE_DIR.join("in/config.toml")
}

/// Google OAuth2 application secret. Generate this file from the Google Cloud
/// console: <https://console.cloud.google.com/> (see the README).
pub fn credentials_file() -> PathBuf {
    EXECUTABLE_DIR.join("in/credentials.json")
}

/// Google OAuth2 tokens, cached by `yup-oauth2`: the user only grants the access once.
pub fn token_file() -> PathBuf {
    EXECUTABLE_DIR.join("in/tokencache.json")
}

/// Daily log files.
pub fn log_dir() -> PathBuf {
    EXECUTABLE_DIR.join("logs")
}
