use std::sync::Arc;
use tokio::sync::watch;
use crate::app::states::app_state::Status;

// =================================================================
//    Feed part (reading side, owned by a state)                   |
// =================================================================

/// Latest data published by the background fetch task of a state.
///
/// A state never fetches anything itself: its task does, and publishes each new
/// value with a `FeedPublisher`. The state only reads the latest one through its `Feed`.
// Generic over `T`, the type of the published data (`Forecast`, `Vec<GTask>`...):
// the code is written once, and the compiler generates a version for each `T` used.
pub enum Feed<T> {
    /// No task is running: the state is turned off in the config, or failed to start.
    Disabled,
    /// A task is running: the channel holds `None` until its first successful fetch.
    // A `watch` channel only keeps the last value sent, which is all a state needs.
    // The value is behind an `Arc` so that `latest` can hand it out without copying it.
    Active(watch::Receiver<Option<Arc<T>>>),
}

impl<T> Feed<T> {
    /// Creates an active feed, and the publisher to give to the background task.
    // Returns the pair as a tuple, like `watch::channel` (and `mpsc::channel`) do.
    pub fn channel() -> (FeedPublisher<T>, Self) {
        /* `None`: nothing has been fetched yet */
        let (tx, rx) = watch::channel(None);

        (FeedPublisher { tx }, Feed::Active(rx))
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
//    FeedPublisher part (writing side, owned by a background task) |
// =================================================================

/// Writing side of a `Feed`: owned by the background task, which publishes
/// each value it fetched.
pub struct FeedPublisher<T> {
    tx: watch::Sender<Option<Arc<T>>>,
}

impl<T> FeedPublisher<T> {
    /// Publishes a new value, replacing the previous one
    pub fn publish(&self, value: T) {
        // `send_replace` (unlike `send`) never fails, even if the state is gone
        self.tx.send_replace(Some(Arc::new(value)));
    }
}
