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
const APP_NAME: &str = env!("CARGO_BIN_NAME");
/// Log directory, relative to the executable directory
const LOG_DIR: &str = "logs";
/// Number of daily log files kept, so the SD card never fills up
const LOG_FILES_KEPT: usize = 7;

// =================================================================
//    main function                                                |
// =================================================================

/// Sets up logging, then runs the app until it is asked to stop.
// A 2nd worker keeps running the timers while the 1st one computes
#[tokio::main(worker_threads = 2)]
async fn main() -> Result<()> {
    // Not `let _`: the guard must live until the end of `main`, to flush the last logs
    let _log_guard = init_logging()?;
    log_panics();

    info!("Starting {APP_NAME}...");
    // Also logged, so that the error ends up in the log file
    app::run()
        .await
        .inspect_err(|e| error!("Fatal error: {e:#}"))
}

// =================================================================
//    Helpers                                                      |
// =================================================================

/// Sets up logging, to the console and to a daily log file.
///
/// Returns the guard of the file writer: keep it until the end of the program,
/// as dropping it flushes the last lines to the file.
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

    // Written by a dedicated thread: logging never waits for the SD card
    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);

    // 2. Console layer (stdout)
    let console_layer = fmt::layer()
        .with_thread_names(true)
        .with_target(true)
        .with_line_number(true)
        // No colors in a file (`nohup.out`) or in the systemd journal
        .with_ansi(std::io::stdout().is_terminal())
        .compact();

    // 3. File layer
    let file_layer = fmt::layer()
        .with_writer(non_blocking)
        .with_thread_names(true)
        .with_line_number(true)
        .with_target(true)
        .with_ansi(false)
        .compact();

    // 4. Filtering: `info` by default, overridden by `RUST_LOG` (e.g. `RUST_LOG=PIxel=debug`)
    let filter = EnvFilter::builder()
        .with_default_directive(LevelFilter::INFO.into())
        .from_env_lossy();

    // 5. Combine the layers into a single subscriber
    tracing_subscriber::registry()
        .with(filter)
        .with(console_layer)
        .with(file_layer)
        .try_init()
        .context("failed to set the logging subscriber")?;

    Ok(guard)
}

/// Logs every panic, so that it also ends up in the log file.
///
/// Rust only prints panics to stderr. A panicking background task is even quieter:
/// tokio ends it, and the app keeps running without it.
fn log_panics() {
    // Kept: it prints the panic (and the backtrace, with `RUST_BACKTRACE=1`)
    let default_hook = std::panic::take_hook();

    std::panic::set_hook(Box::new(move |info| {
        error!("{info}");
        default_hook(info);
    }));
}
