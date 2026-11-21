mod app;
mod config;
mod feed;
mod google_auth;
mod matrix;
mod paths;
mod states;

use std::io::IsTerminal;

use anyhow::{Context, Result};
use tracing::{error, info};
use tracing_appender::{non_blocking::WorkerGuard, rolling};
use tracing_subscriber::{
    filter::{EnvFilter, LevelFilter},
    fmt,
    layer::SubscriberExt,
    util::SubscriberInitExt,
};

use crate::paths::EXECUTABLE_DIR;

/// Name of the app, used as the prefix of the log files
const APP_NAME: &str = "PIxel";
/// Log directory, relative to the executable directory
const LOG_DIR: &str = "logs";
/// Number of daily log files kept: older ones are deleted, so the SD card never fills up
const LOG_FILES_KEPT: usize = 7;

// =================================================================
//    main function                                                |
// =================================================================

/// Sets up logging, then runs the app until it is asked to stop.
///
/// Returning a `Result` from `main`: on `Err`, Rust prints the error and the process
/// exits with code 1 (so a service manager knows it failed), instead of a panic.
#[tokio::main]
async fn main() -> Result<()> {
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

/// Sets up logging, to the console and to a daily log file.
///
/// Returns the guard of the file writer: the caller keeps it until the end of the
/// program, as dropping it flushes the last lines to the file.
// The guard can't be kept in a `static` instead: Rust never drops statics, so the
// last lines would never be written.
///
/// # Errors
/// If the log file can't be created, or a subscriber was already set.
fn init_logging() -> Result<WorkerGuard> {
    // 1. File appender: a new file each day, the oldest ones deleted
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(APP_NAME)
        .filename_suffix("log")
        .max_log_files(LOG_FILES_KEPT)
        .build(EXECUTABLE_DIR.join(LOG_DIR))
        .context("failed to create the log file")?;

    // Lines are written to the file by a dedicated thread: logging never waits for the SD card
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 2. Console layer (stdout)
    let console_layer = fmt::layer()
        .with_thread_names(true)
        .with_target(true)
        .with_line_number(true) // include line number
        // Colors only in a terminal: not in a file (`nohup.out`) or the systemd journal
        .with_ansi(std::io::stdout().is_terminal())
        .compact(); // shorter logs

    // 3. File layer
    let file_layer = fmt::layer()
        .with_writer(non_blocking) // async, non-blocking writes
        .with_ansi(false) // no ANSI codes in file
        .event_format(
            fmt::format()
                .with_thread_names(true)
                .with_target(true)
                .with_line_number(true) // include line number
                // Line format: either nothing (full), json, pretty, or compact
                .compact(),
        );

    // 4. Filtering
    // default: info ; overridable by RUST_LOG=debug ./app
    // to filter PIxel only: RUST_LOG=PIxel=debug ./app (the target is the binary name)
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    // 5. Combine the layers into a single subscriber
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
