mod app;
mod config;
mod feed;
mod google_auth;
mod matrix;
mod states;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, filter::{EnvFilter, LevelFilter}};
use tracing_appender::{rolling, non_blocking::WorkerGuard};
use std::io::IsTerminal;
use std::sync::LazyLock;
use std::path::PathBuf;
use anyhow::{Context, Result};
use tracing::{error, info};

/* Constants */
const APP_NAME:&str = "PIxel";
/// Log directory, relative to the executable directory
const LOG_DIR: &str = "logs";
/// Number of daily log files kept: older ones are deleted, so the SD card never fills up
const LOG_FILES_KEPT: usize = 7;
/// Directory of the executable, whatever the current directory is when it is started.
pub static EXECUTABLE_DIR: LazyLock<PathBuf> = LazyLock::new(|| {
    std::env::current_exe()
        .expect("Failed to get current executable path")
        .parent()
        .expect("Executable has no parent directory")
        .to_path_buf()
});

// ================================================================= 
//    main function                                                |
// ================================================================= 

/// Returning a `Result` from `main`: on `Err`, Rust prints the error and the process
/// exits with code 1 (so a service manager knows it failed), instead of a panic.
#[tokio::main]
async fn main() -> Result<()> {
    /* Create tracing subscriber */
    // The guard must live until the end of `main`: dropping it writes the last
    // buffered lines to the file. `let _log_guard` keeps it alive until then, while
    // `let _ = ...` would drop it right away.
    let _log_guard = init_logging()?;
    log_panics();

    info!("Starting PIxel...");
    // The error is also logged, so that it ends up in the log file
    app::run()
        .await
        .inspect_err(|e| error!("Fatal error: {e:#}"))
}

// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/// Function that initializes tracing subscriber for logging
///
/// Returns the guard of the file writer: the caller keeps it until the end of the
/// program, as dropping it flushes the last lines to the file.
// The guard can't be kept in a `static` instead: Rust never drops statics, so the
// last lines would never be written.
///
/// # Errors
/// If the log file can't be created, or a subscriber was already set.
fn init_logging() -> Result<WorkerGuard> {
    /* 1. Create a rotating file appender (daily or hourly) */
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(APP_NAME)
        .filename_suffix("log")
        .max_log_files(LOG_FILES_KEPT)
        .build(EXECUTABLE_DIR.join(LOG_DIR))
        .context("failed to create the log file")?;

    // Lines are written to the file by a dedicated thread: logging never waits for the SD card
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    /* 2. Console layer (stdout) */
    let console_layer = fmt::layer()
        .with_thread_names(true)
        .with_target(true)
        .with_line_number(true)     // include line number
        // Colors only in a terminal: not in a file (`nohup.out`) or the systemd journal
        .with_ansi(std::io::stdout().is_terminal())
        .compact();                 // shorter logs

    /* 3. File layer */
    let file_layer = fmt::layer()
        .with_writer(non_blocking)  // async, non-blocking writes
        .with_ansi(false)           // no ANSI codes in file
        .event_format(
            fmt::format()
            .with_thread_names(true)
            .with_target(true)
            .with_line_number(true) // include line number
            .compact());            // either nothing (full), json, pretty, or compact

    /* 4. Filtering */
    // default: info ; overridable by RUST_LOG=debug ./app
    // to filter PIxel only: RUST_LOG=PIxel=debug ./app (the target is the binary name)
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    /* 5. Combine the layers into a single subscriber */
    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        // `try_init` returns an error where `init` would panic
        .try_init()
        .context("failed to set the logging subscriber")?;

    Ok(guard)
}

/// Logs every panic, so that it also ends up in the log file.
///
/// Rust only prints panics to stderr. A panic in a background task is even quieter:
/// tokio catches it and ends the task, and its state just disappears from the rotation.
fn log_panics() {
    // Keep the default hook, which prints the panic (and the backtrace, if
    // `RUST_BACKTRACE=1`), and log the panic before calling it.
    let default_hook = std::panic::take_hook();

    // `move`: the closure takes ownership of `default_hook`, as it outlives this function
    std::panic::set_hook(Box::new(move |info| {
        // `info` displays as "panicked at src/file.rs:line:col:\n<message>"
        error!("{info}");
        default_hook(info);
    }));
}
