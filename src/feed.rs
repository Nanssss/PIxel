//! Background fetching, shared by the states that get their data from the network.
//!
//! A state provides a [`Fetcher`] (how to fetch its data), and gets back a [`Feed`]
//! (the latest data fetched), updated by a background task.

use std::sync::Arc;

use anyhow::{Context, Result};
use tokio::sync::watch;
use tokio::time::{Duration, Instant, sleep, timeout};
use tracing::warn;

/// Wait before fetching again after a failed fetch (e.g. no network at boot),
/// doubled after each further failure in a row
const FIRST_RETRY_DELAY: Duration = Duration::from_secs(30);

// =================================================================
//    Fetcher part (what a state provides)                         |
// =================================================================

/// How to fetch the data of a state.
///
/// A state implements it on a small struct owning everything a fetch needs (HTTP
/// client, URL, credentials...), and hands it to [`Feed::spawn`], which moves it
/// into the background task.
pub trait Fetcher: Send + 'static {
    /// The data fetched (`Forecast`, `Vec<Event>`...)
    type Data: Send + Sync + 'static;

    /// Name of the data, used in the logs (e.g. "weather forecast")
    const NAME: &'static str;

    /// How often the data is fetched
    const REFRESH_PERIOD: Duration;

    /// Age past which the data is too old to be displayed (e.g. after a long network
    /// outage). Must be longer than `REFRESH_PERIOD`: checked at compile time.
    const MAX_AGE: Duration;

    /// Time after which a fetch that hangs is abandoned
    const TIMEOUT: Duration = Duration::from_secs(30);

    /// Fetches the data once.
    ///
    /// # Errors
    /// If the fetch failed: the task logs the error, and fetches again later.
    // Implemented as `async fn fetch(&mut self) -> Result<Self::Data>`: spelled out
    // here only to add `+ Send`, required by `tokio::spawn`
    fn fetch(&mut self) -> impl Future<Output = Result<Self::Data>> + Send;
}

// =================================================================
//    Feed part (what a state gets back)                           |
// =================================================================

/// Latest data fetched by the background task of a state.
///
/// Built by [`Feed::spawn`], read with [`Feed::latest`]: a state never fetches
/// anything itself.
pub struct Feed<T> {
    /// What the task published: `None` until its first successful fetch
    rx: watch::Receiver<Option<Fetched<T>>>,
    /// Age past which the published data is no longer displayed (`Fetcher::MAX_AGE`)
    max_age: Duration,
}

/// A value published by the background task, with the time it was fetched at.
struct Fetched<T> {
    /// In an `Arc`: `Feed::latest` hands it out without copying it
    data: Arc<T>,
    /// When the fetch succeeded. Monotonic: unaffected by clock changes (NTP sync at boot)
    fetched_at: Instant,
}

impl<T> Feed<T> {
    /// Starts the background task fetching the data, and returns the feed to read it from.
    ///
    /// Returns immediately: the feed has nothing to give until the first successful fetch.
    // `T` repeats the bounds of `Fetcher::Data`: they aren't deduced from `Data = T`
    pub fn spawn<F>(fetcher: F) -> Self
    where
        F: Fetcher<Data = T>,
        T: Send + Sync + 'static,
    {
        // Checked at compile time, for each fetcher type.
        // `as_nanos`: `Duration`s can't be compared in a `const` context yet.
        const {
            assert!(
                F::MAX_AGE.as_nanos() > F::REFRESH_PERIOD.as_nanos(),
                "Fetcher::MAX_AGE must be longer than Fetcher::REFRESH_PERIOD"
            );
        }

        let (tx, rx) = watch::channel(None);
        tokio::spawn(fetch_loop(fetcher, tx));

        Feed {
            rx,
            max_age: F::MAX_AGE,
        }
    }

    /// Returns the latest published value, or `None` if there is nothing to display
    /// (nothing fetched yet, or data older than the maximum age).
    ///
    /// The age is checked here rather than by the task: the data then expires even if
    /// the task is stuck or dead.
    pub fn latest(&self) -> Option<Arc<T>> {
        // The lock taken by `borrow` ends with the statement: only an `Arc` clone leaves it
        self.rx
            .borrow()
            .as_ref()
            .filter(|fetched| fetched.fetched_at.elapsed() <= self.max_age)
            .map(|fetched| Arc::clone(&fetched.data))
    }
}

// =================================================================
//    Background task                                              |
// =================================================================

/// Fetches the data and publishes each new value to the feed, until the feed is dropped.
///
/// Runs beside the frame loop: a slow network never freezes the display.
async fn fetch_loop<F: Fetcher>(mut fetcher: F, tx: watch::Sender<Option<Fetched<F::Data>>>) {
    let mut retry_delay = FIRST_RETRY_DELAY;

    loop {
        // A fetch that hangs (e.g. wifi lost) counts as a failed one
        let result = timeout(F::TIMEOUT, fetcher.fetch())
            .await
            .context("timed out")
            .flatten();

        let delay = match result {
            Ok(data) => {
                tx.send_replace(Some(Fetched {
                    data: Arc::new(data),
                    fetched_at: Instant::now(),
                }));
                retry_delay = FIRST_RETRY_DELAY;
                F::REFRESH_PERIOD
            }
            // The last data stays on screen until it expires
            Err(e) => {
                // Exponential backoff, capped at the refresh period
                let delay = retry_delay.min(F::REFRESH_PERIOD);
                retry_delay = delay * 2;

                warn!("Failed to fetch {}, retrying in {delay:?}: {e:#}", F::NAME);
                delay
            }
        };

        // Wait for the next fetch, or stop if the feed was dropped
        tokio::select! {
            _ = sleep(delay) => {}
            _ = tx.closed() => return,
        }
    }
}
