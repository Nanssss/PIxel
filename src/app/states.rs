//! The states of the app: what it can show on the matrix, one at a time, in rotation.
//!
//! Each state lives in its own module and implements the [`State`] trait below,
//! which is all the app (see `app_state.rs`) knows about it.

pub mod app_state;
pub mod clock;
pub mod feed;
pub mod weather;
pub mod gtasks;

use crate::app::matrix::Matrix;

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
    /// Only called when the state is ready, and must never wait (no network, no
    /// file): it runs in the frame loop.
    fn draw(&self, matrix: &mut Matrix);
}
