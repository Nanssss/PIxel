//! Where the app finds its files (config, credentials, logs): next to the executable.

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
