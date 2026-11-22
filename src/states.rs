//! The states of the app: what it can show on the matrix, one at a time, in rotation.
//!
//! Each state lives in its own module and implements the [`State`] trait below,
//! which is all the app (see `app.rs`) knows about it.

pub mod calendar;
pub mod clock;
pub mod weather;

use anyhow::{Context, Result};
use reqwest::Client;

use crate::matrix::Matrix;

/// Something the app can show on the matrix (the clock, the weather...).
///
/// A state only exists if it is enabled: one that is turned off in the config, or
/// that failed to start, is never built (its constructor returns `None`).
pub trait State {
    /// Whether the state has something to display right now.
    ///
    /// Only used by the rotation: a state that isn't ready (no data yet, or data
    /// that got too old) is skipped until it is ready again.
    fn is_ready(&self) -> bool;

    /// Draws one frame of the state on the matrix.
    ///
    /// Called on every frame of the current state, ready or not: a state with
    /// nothing to display draws nothing (the frame stays black).
    ///
    /// Runs in the frame loop, 30 times per second: no waiting, no I/O, no decoding.
    /// Prepare data in the constructor or in the background `fetch` instead.
    /// `&mut self`: a state can keep what changes between frames (animation, cache...).
    fn draw(&mut self, matrix: &mut Matrix);
}

/// What the app shares with its states while it builds them.
pub struct AppContext {
    /// HTTP client shared by every state that needs the network. Cheap to clone:
    /// the clones share the same connection pool.
    pub client: Client,
}

impl AppContext {
    /// Creates the context: builds the HTTP client the states share.
    ///
    /// # Errors
    /// If the HTTP client can't be built (e.g. the TLS backend fails to start).
    pub fn new() -> Result<Self> {
        let client = Client::builder()
            // Tells the APIs who is calling (e.g. "pixel/0.1.0")
            .user_agent(concat!(
                env!("CARGO_PKG_NAME"),
                "/",
                env!("CARGO_PKG_VERSION")
            ))
            .build()
            .context("failed to build the HTTP client")?;

        Ok(Self { client })
    }
}
