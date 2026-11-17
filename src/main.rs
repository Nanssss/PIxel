mod app;
mod config;
mod google_auth;
mod matrix;
mod states;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, filter::{EnvFilter, LevelFilter}};
use tracing_appender::{rolling, non_blocking::WorkerGuard};
use std::sync::{OnceLock, LazyLock};
use std::path::PathBuf;
use anyhow::{Context, Result};
use tracing::{error, info};

/* Constants */
const APP_NAME:&str = "PIxel";
const LOG_DIR:&str = "./logs";
static FILE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();
/// Directory of the executable: the files of the app (`in/`...) are found from there,
/// whatever the current directory is when it is started.
///
/// Computed on first use. The `expect`s can't fail on Linux: the path of a running
/// executable is always known, and a file always has a parent directory.
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
    // If this fails, nothing can be logged: the error is only printed by Rust.
    init_logging()?;

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
/// # Errors
/// If the log file can't be created, or a subscriber was already set.
fn init_logging() -> Result<()> {
    /* 1. Create a rotating file appender (daily or hourly) */
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(APP_NAME)
        .filename_suffix("log")
        .build(LOG_DIR)
        .context("failed to create the log file")?;

    let (non_blocking, guard) = tracing_appender::non_blocking(file_appender);
    // NOTE: keep `guard` in a global/static to ensure logs flush on exit
    FILE_GUARD.set(guard).expect("Failed to set FILE_GUARD");

    /* 2. Console layer (stdout) */
    let console_layer = fmt::layer()
        .with_thread_names(true)
        .with_target(true)
        .with_line_number(true)     // include line number
        .with_ansi(true)            // colors for stdout
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
    // to filter PIxel only: RUST_LOG=pixel=debug ./app
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
        .context("failed to set the logging subscriber")
}
