use std::sync::Arc;
use anyhow::{Context, Result};
use tokio::sync::watch;
use tokio::time::{sleep, timeout, Duration};
use tracing::error;
use crate::app::states::app_state::Status;

/// Wait before fetching again after a first failed fetch (e.g. no network at boot).
/// Doubled after each further failure in a row (see `fetch_loop`).
const FIRST_RETRY_DELAY: Duration = Duration::from_secs(30);

// =================================================================
//    Fetcher part (what a state provides)                         |
// =================================================================

/// How to fetch the data of a state.
///
/// A state implements it on a small struct holding everything a fetch needs
/// (HTTP client, URL, credentials...), and hands that struct to `Feed::spawn`.
// `Send + 'static`: the fetcher is moved into a tokio task, which may run on another
// thread (`Send`) and outlive the function that created it (`'static`: owns its data).
pub trait Fetcher: Send + 'static {
    /// The data fetched (`Forecast`, `Vec<GTask>`...)
    // An *associated type*: each implementation picks it once, and the rest of the
    // code can then refer to it as `F::Data`.
    type Data: Send + Sync + 'static;

    /// Name of the data, used in the logs (e.g. "weather data")
    const NAME: &'static str;

    /// How often the data is fetched
    const REFRESH_PERIOD: Duration;

    /// Time after which a fetch that hangs is abandoned
    // An associated const may have a *default value*: an implementation only
    // declares it when it needs another one.
    const TIMEOUT: Duration = Duration::from_secs(30);

    /// Fetches the data once.
    ///
    /// # Errors
    /// If the fetch failed: the task logs the error, and fetches again later.
    // Implementations simply write `async fn fetch(&mut self) -> Result<Self::Data>`.
    // It is spelled out here (an `async fn` is a function returning a `Future`) only
    // to add `+ Send`, which `tokio::spawn` requires and `async fn` can't express.
    //
    // `&mut self` rather than `&self`: the task is the only owner of the fetcher, so
    // exclusive access costs nothing, and lets a fetcher keep some state between two
    // fetches. It also asks less of the fetcher: a future holding a `&mut F` is `Send`
    // as soon as `F` is `Send`, while one holding a `&F` would need `F` to be `Sync` too.
    fn fetch(&mut self) -> impl Future<Output = Result<Self::Data>> + Send;
}

// =================================================================
//    Feed part (what a state gets back)                           |
// =================================================================

/// Latest data fetched by the background task of a state.
///
/// A state never fetches anything itself: the task started by `Feed::spawn` does,
/// and the state only reads the latest value it published.
// Generic over `T`, the type of the published data: the code is written once, and
// the compiler generates a version for each `T` used.
pub enum Feed<T> {
    /// No task is running: the state is turned off in the config, or failed to start.
    Disabled,
    /// A task is running: the channel holds `None` until its first successful fetch.
    // A `watch` channel only keeps the last value sent, which is all a state needs:
    // the task owns the `Sender`, the feed owns the `Receiver`.
    // The value is behind an `Arc` so that `latest` can hand it out without copying it.
    Active(watch::Receiver<Option<Arc<T>>>),
}

impl<T> Feed<T> {
    /// Starts the background task fetching the data, and returns the feed to read it from.
    ///
    /// Returns immediately, nothing is fetched here: the feed is `Waiting` until
    /// the first successful fetch.
    // `Fetcher<Data = T>`: any fetcher, as long as the data it fetches is the `T` of this feed.
    // The bounds on `T` repeat those of `Fetcher::Data`: the compiler doesn't deduce
    // them from `Data = T`, and the task needs them to share the data with the state.
    pub fn spawn<F>(fetcher: F) -> Self
    where
        F: Fetcher<Data = T>,
        T: Send + Sync + 'static,
    {
        /* `None`: nothing has been fetched yet */
        let (tx, rx) = watch::channel(None);

        // `tokio::spawn` starts the task and returns immediately.
        // The task takes ownership of the fetcher and of the sending side of the channel.
        tokio::spawn(fetch_loop(fetcher, tx));

        Feed::Active(rx)
    }

    /// Returns the runtime status of the feed, deduced from what the task published so far
    pub fn status(&self) -> Status {
        match self {
            Feed::Disabled => Status::Disabled,
            // `borrow()` reads the latest value without waiting
            Feed::Active(rx) if rx.borrow().is_some() => Status::Ready,
            Feed::Active(_) => Status::Waiting,
        }
    }

    /// Returns the latest published value, or `None` if there is nothing to display
    /// (feed disabled, or nothing fetched yet).
    // Cloning an `Arc` only increments a counter: the data itself is never copied.
    // The lock taken by `borrow()` is released at the end of the statement, so the
    // caller can keep the value as long as it wants without blocking the task.
    pub fn latest(&self) -> Option<Arc<T>> {
        match self {
            Feed::Disabled => None,
            Feed::Active(rx) => rx.borrow().clone(),
        }
    }
}

// =================================================================
//    Background task                                              |
// =================================================================

/// Fetches the data and publishes each new value to the feed, until the feed is dropped.
///
/// Runs beside the frame loop, so a slow network never freezes the display.
// `mut fetcher`: the task owns the fetcher, and `fetch` borrows it mutably.
async fn fetch_loop<F: Fetcher>(mut fetcher: F, tx: watch::Sender<Option<Arc<F::Data>>>) {
    // Wait before the next retry: grows while the fetches keep failing (see below)
    let mut retry_delay = FIRST_RETRY_DELAY;

    loop {
        // `timeout` gives up on a fetch that hangs (e.g. wifi lost during the request).
        // It wraps the result in a second `Result`, whose `Err` means "timed out":
        // `context` turns that into an anyhow error like those of a fetch, and `flatten`
        // merges the two levels (`Result<Result<T>>` -> `Result<T>`), so both failures
        // are handled alike.
        let result = timeout(F::TIMEOUT, fetcher.fetch())
            .await
            .context("timed out")
            .flatten();

        let delay = match result {
            Ok(data) => {
                // `send_replace` (unlike `send`) never fails, even if the feed is gone
                tx.send_replace(Some(Arc::new(data)));
                retry_delay = FIRST_RETRY_DELAY;
                F::REFRESH_PERIOD
            }
            /* On a failure the last known data is kept, and the fetch is retried sooner */
            Err(e) => {
                // Exponential backoff: 30 s, 1 min, 2 min... A long outage (network down
                // for hours) then costs a few retries and log lines instead of thousands.
                // Capped: retrying less often than a normal refresh would make no sense.
                let delay = retry_delay.min(F::REFRESH_PERIOD);
                retry_delay = delay * 2;

                error!("Failed to fetch {}, retrying in {delay:?}: {e:#}", F::NAME);
                delay
            }
        };

        // Wait for the next fetch, but stop the task if the feed is dropped meanwhile.
        // `select!` runs both futures at once, and executes the branch of the first
        // one that completes (the other future is simply dropped).
        tokio::select! {
            _ = sleep(delay) => {}
            /* `closed()` completes once every `Receiver` is dropped: nobody reads anymore */
            _ = tx.closed() => return,
        }
    }
}
