//! The states of the app: what it can show on the matrix, one at a time, in rotation.
//!
//! Each state lives in its own module and implements the [`State`] trait below,
//! which is all the app (see `app.rs`) knows about it.

pub mod clock;
pub mod gtasks;
pub mod weather;

use reqwest::Client;
use anyhow::{Context, Result};
use crate::matrix::Matrix;

/// Something the app can show on the matrix (the clock, the weather...).
///
/// A state only exists if it is enabled: one that is turned off in the config, or
/// that failed to start, is never built (its constructor returns `None`).
// The app stores its states as `Box<dyn State>`: "any type implementing `State`".
// The method to run is then picked at runtime (*dynamic dispatch*), which is what
// lets the app loop over states of different types without knowing them.
pub trait State {
    /// Whether the state has something to display right now.
    ///
    /// A state that isn't ready (no data yet, or data that got too old) is skipped
    /// by the rotation until it is ready again.
    fn is_ready(&self) -> bool;

    /// Draws one frame of the state on the matrix.
    ///
    /// Only called when the state is ready. Runs in the frame loop, 30 times per
    /// second: no waiting, no I/O, no decoding. Prepare data in the constructor or
    /// in the background `fetch` instead.
    ///
    /// A color, font or style used by several states belongs in a shared module
    /// (e.g. `src/ui.rs`, to create with the first one), not copied in each state.
    fn draw(&self, matrix: &mut Matrix);
}

/// What the app shares with its states while it builds them.
pub struct AppContext {
    /// HTTP client shared by every state that needs the network
    // A reqwest `Client` is already a handle on a shared connection pool (an `Arc`
    // inside): cloning it is cheap, and every clone reuses the same connections.
    // Wrapping it in another `Arc` would add nothing.
    pub client: Client,
}

/// Method implementations for AppContext
impl AppContext {
    /// AppContext constructor
    ///
    /// # Errors
    /// If the HTTP client can't be built (e.g. the TLS backend fails to start).
    // `Client::new()` would panic in that case: the builder returns an error instead.
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            // Sent with every request, to tell the APIs who is calling (e.g. "pixel/0.1.0").
            .user_agent(concat!(env!("CARGO_PKG_NAME"), "/", env!("CARGO_PKG_VERSION")))
            .build()
            .context("failed to build the HTTP client")?;

        Ok(AppContext {
            client,
        })
    }
}
