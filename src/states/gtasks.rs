use crate::states::{AppContext, State};
use crate::states::feed::{Feed, Fetcher};
use crate::google_auth::GoogleAuth;
use crate::matrix::Matrix;
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use reqwest::Client;
use tokio::time::Duration;
use serde::Deserialize;
use tracing::{trace, info, error};
use anyhow::{Result, Context};
use chrono::{DateTime, Local, NaiveDate, Utc};

/// Events of the user's main calendar.
/// Doc: https://developers.google.com/workspace/calendar/api/v3/reference/events/list
const GCALENDAR_EVENTS_URL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
/*
    Add the GCALENDAR scopes you need here.
    (find them at https://developers.google.com/workspace/calendar/api/auth?hl=fr)
*/
const GCALENDAR_SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar.events.readonly"];

/// Public struct containing gtasks config
// `#[serde(default)]`: a field missing from the config file takes its value from
// the `Default` implementation below, instead of making the whole file invalid.
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct GTasksConfig {
    pub enabled: bool,
}

/// Default values for config
impl Default for GTasksConfig {
    fn default() -> Self {
        GTasksConfig {
            enabled: true,
        }
    }
}

impl GTasksConfig {
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }
}

/// Atomic task
///
/// Read as is from the JSON of the API: each field is taken from the event field
/// of the same name, unless `rename` gives another one.
#[derive(Debug, Deserialize)]
struct GTask {
    #[serde(rename = "summary", default)]
    title:          String,
    start:          TaskStart,
    description:    Option<String>,
}

/// When a task starts.
///
/// The API gives it in one of two shapes: `{ "dateTime": "2026-11-15T18:30:00+01:00" }`,
/// or `{ "date": "2026-11-15" }` for a task lasting the whole day.
// `untagged`: serde tries each variant in turn, and keeps the first one whose
// fields are all found in the JSON. Other fields (e.g. "timeZone") are ignored.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(untagged)]
enum TaskStart {
    /// At a given time
    At {
        #[serde(rename = "dateTime")]
        time: DateTime<Local>,
    },
    AllDay {
        date: NaiveDate,
    },
}

/// Public struct containing global gtasks state
///
/// The state never fetches anything itself: a background task (see `Feed::spawn`) does,
/// and the state only reads the latest tasks it published.
pub struct GTasksState {
    /// Latest tasks published by the background task (an empty `Vec` if there is no task)
    tasks:  Feed<Vec<GTask>>,
}


// =================================================================
//    Methods implementation                                       |
// =================================================================

impl GTasksState {
    /// GTasksState constructor: starts the background task fetching the tasks.
    ///
    /// Returns `None` if the state is turned off in the config, or failed to start.
    /// Only the OAuth2 authenticator is prepared here (it reads local files). Nothing
    /// is fetched: the state isn't ready until the first tasks arrive.
    pub async fn new(config: GTasksConfig, context: &AppContext) -> Option<Self> {
        /* A state turned off in the config is never built: its task is never started */
        if !config.is_enabled() {
            return None;
        }

        /* If in the future the request needs config, pass it &config */
        // `inspect_err` logs the error without consuming it, `ok()` turns the `Result`
        // into an `Option`, and `?` returns `None` right away if there is no value.
        let auth = GoogleAuth::new().await
            .inspect_err(|e| error!("Failed to initialize Google authentication, disabling GTasks state: {e:#}"))
            .ok()?;

        // The background task must own everything it uses: cloning the client
        // only copies a handle, the connections stay shared (see `AppContext`).
        let fetcher = TasksFetcher {
            client: context.client.clone(),
            auth,
        };

        /* Start the background task: returns immediately */
        Some(GTasksState {
            tasks: Feed::spawn(fetcher),
        })
    }
}

impl State for GTasksState {
    /// Ready as soon as the background task published a list of tasks that is still fresh
    fn is_ready(&self) -> bool {
        self.tasks.latest().is_some()
    }

    /// Method for drawing the state to the screen
    fn draw(&self, matrix: &mut Matrix) {
        // The app only draws a state that is ready, so the `else` is a safety net
        // (the tasks may expire right between the two calls)
        let Some(_tasks) = self.tasks.latest() else { return };

        let Ok(()) = matrix.clear(Rgb888::BLUE);
    }
}


// =================================================================
//    Background task                                              |
// =================================================================

/// Fetches the tasks for the background task of the state.
///
/// Owns everything a fetch needs: the task that runs it (see `Feed::spawn`)
/// lives longer than the function that started it, so it can't borrow anything.
struct TasksFetcher {
    client: Client,         // HTTP client shared with the other states
    auth:   GoogleAuth,     // Google authentication, prepared once at startup
}

impl Fetcher for TasksFetcher {
    type Data = Vec<GTask>;

    const NAME: &'static str = "Google Tasks";

    const REFRESH_PERIOD: Duration = Duration::from_secs(5 * 60);

    /// The tasks are the next ones to come: an old list may show tasks already over
    const MAX_AGE: Duration = Duration::from_secs(60 * 60);

    /// Longer than the default: on the first run, the fetch waits for the Google
    /// authorization to be completed in the browser.
    const TIMEOUT: Duration = Duration::from_secs(120);

    /// Fetches the next tasks from the GCalendar API
    async fn fetch(&mut self) -> Result<Vec<GTask>> {
        /* Get the access token (from the cache, unless a new one is needed) */
        let token = self.auth.token(GCALENDAR_SCOPES).await?;

        /* Request parameters */
        let query_params = &[
            ("maxResults",      "5"),
            ("orderBy",         "startTime"),
            ("singleEvents",    "true"),
            ("timeMin",         &Utc::now().to_rfc3339()), // get current time in RFC3339 format
        ];

        /* Send GET request to GTASKS API, and deserialize the JSON of the response */
        let response: TasksResponse = self.client
            .get(GCALENDAR_EVENTS_URL)
            .bearer_auth(token)     // adds the header `Authorization: Bearer <token>`
            .query(query_params)
            .send()
            .await
            .context("failed to send GTASKS API request")?
            // Without this, an error (e.g. "401 Unauthorized") would be read as if it
            // was a list of events: it has no "items", so it would look like "no task".
            .error_for_status()
            .context("GTASKS API returned an error")?
            .json()
            .await
            .context("failed to deserialize GTASKS API response")?;

        let tasks = response.items;
        trace!("Parsed Tasks: {:#?}", tasks);
        log_tasks(&tasks);

        Ok(tasks)
    }
}


// =================================================================
//    Static functions                                             |
// =================================================================

/// Logs the given tasks
fn log_tasks(tasks: &[GTask]) {
    /* Print number of tasks */
    info!("[GOOGLE TASKS] - Found {} Tasks", tasks.len());

    /* Print each task */
    for task in tasks {
        info!("[GOOGLE TASKS] - Task: {} ({:?})", task.title, task.start);

        if let Some(description) = &task.description {
            info!("[GOOGLE TASKS] - Task description: {}", description);
        }
    }
}


// =================================================================
//    Private types                                                |
// =================================================================

/* The following type is used for deserialization */

/// Response of the GCalendar API.
// Only the field we use is declared: serde ignores the other ones of the JSON.
#[derive(Debug, Deserialize)]
struct TasksResponse {
    /// The events, already sorted by start time. `default`: an empty list if the
    /// field is missing.
    #[serde(default)]
    items: Vec<GTask>,
}


// =================================================================
//    Tests                                                        |
// =================================================================

// `#[cfg(test)]`: this module is only compiled by `cargo test`, never in the app.
#[cfg(test)]
mod tests {
    // Gives access to everything of the parent module, private items included
    use super::*;

    /// A response of the GCalendar API (shortened): a task at a given time, then an
    /// all-day task without title nor description.
    const RESPONSE: &str = r#"{
        "kind": "calendar#events",
        "items": [
            {
                "summary": "Dentist",
                "description": "Bring the X-ray",
                "start": { "dateTime": "2026-11-16T18:30:00+01:00", "timeZone": "Europe/Paris" },
                "end":   { "dateTime": "2026-11-16T19:00:00+01:00", "timeZone": "Europe/Paris" }
            },
            {
                "start": { "date": "2026-11-17" },
                "end":   { "date": "2026-11-18" }
            }
        ]
    }"#;

    #[test]
    fn tasks_are_read_from_the_api_response() {
        let tasks = serde_json::from_str::<TasksResponse>(RESPONSE).unwrap().items;
        assert_eq!(tasks.len(), 2);

        assert_eq!(tasks[0].title, "Dentist");
        assert_eq!(tasks[0].description.as_deref(), Some("Bring the X-ray"));
        // Compared as an instant: the test doesn't depend on the time zone of the machine
        let expected = DateTime::parse_from_rfc3339("2026-11-16T17:30:00Z").unwrap();
        assert!(matches!(tasks[0].start, TaskStart::At { time } if time == expected));

        assert_eq!(tasks[1].title, "");
        assert_eq!(tasks[1].description, None);
        let expected = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        assert_eq!(tasks[1].start, TaskStart::AllDay { date: expected });
    }

    #[test]
    fn response_without_items_means_no_task() {
        let tasks = serde_json::from_str::<TasksResponse>(r#"{ "kind": "calendar#events" }"#).unwrap().items;
        assert!(tasks.is_empty());
    }

    #[test]
    fn task_without_start_is_refused() {
        let response = r#"{ "items": [ { "summary": "Dentist", "start": {} } ] }"#;
        assert!(serde_json::from_str::<TasksResponse>(response).is_err());
    }
}
