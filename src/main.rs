mod app; // Include ./app.rs
use app::app_main;

use tracing_subscriber::{fmt, layer::SubscriberExt, util::SubscriberInitExt, filter::{EnvFilter, LevelFilter}};
use tracing_appender::{rolling, non_blocking::WorkerGuard};
use std::sync::{OnceLock, LazyLock};
use std::path::PathBuf;
use tracing::info;

/* Notes:
    cargo add serde --features derive
    cargo add serde_json
    cargo add tokio --features full
    cargo add reqwest --features json, blocking
    cargo add yup-oauth2
    cargo add hyper_util
    cargo add tracing
    cargo add tracing-subscriber --features env-filter json
    cargo add tracing-appender
    cargo add anyhow
    cargo add toml
    cargo add bindgen
    cargo add cc
*/

/* Constants */
const APP_NAME:&str = "PIxel";
const LOG_DIR:&str = "./logs";
static FILE_GUARD: OnceLock<WorkerGuard> = OnceLock::new();
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

#[tokio::main]
async fn main() {
    /* Create tracing subscriber */
    init_logging();

    info!("Starting PIxel...");
    app_main::app_main().await;
}

// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/* Function that initializes tracing subscriber for logging */
fn init_logging() {
    /* 1. Create a rotating file appender (daily or hourly) */
    let file_appender = rolling::RollingFileAppender::builder()
        .rotation(rolling::Rotation::DAILY)
        .filename_prefix(APP_NAME)
        .filename_suffix("log")
        .build(LOG_DIR)
        .expect("Failed to initialize rolling file appender");

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
        .init();
}
