//! The Google tasks state: shows the next events of the user's Google calendar.
//!
//! The events are fetched in the background (see `feed.rs`) from the Google
//! Calendar API, with the OAuth2 tokens of `google_auth.rs`.

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, Utc};
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use reqwest::Client;
use serde::Deserialize;
use tokio::time::Duration;
use tracing::{debug, error, info};

use crate::feed::{Feed, Fetcher};
use crate::google_auth::GoogleAuth;
use crate::matrix::Matrix;
use crate::states::{AppContext, State};

/// Events of the user's main calendar.
/// Doc: <https://developers.google.com/workspace/calendar/api/v3/reference/events/list>
const GCALENDAR_EVENTS_URL: &str =
    "https://www.googleapis.com/calendar/v3/calendars/primary/events";
/// Access asked to the user: read only the calendar events.
/// Other scopes: <https://developers.google.com/workspace/calendar/api/auth>
const GCALENDAR_SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar.events.readonly"];

/// Config of the Google tasks: the `[gtasks]` section of `config.toml`.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct GTasksConfig {
    enabled: bool,
}

/// Values used when the section, or one of its fields, is missing: the tasks are shown.
impl Default for GTasksConfig {
    fn default() -> Self {
        GTasksConfig { enabled: true }
    }
}

/// One task to show: an event of the calendar, read as is from the JSON of the API.
#[derive(Debug, Deserialize)]
struct GTask {
    #[serde(rename = "summary", default)]
    title: String,
    start: TaskStart,
    description: Option<String>,
}

/// When a task starts.
///
/// The API gives it in one of two shapes: `{ "dateTime": "2026-11-15T18:30:00+01:00" }`,
/// or `{ "date": "2026-11-15" }` for a task lasting the whole day.
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

/// Shows the next tasks of the user's calendar, fetched by a background task (see `Feed`).
pub struct GTasksState {
    /// Latest tasks published by the background task (an empty `Vec` if there is no task)
    tasks: Feed<Vec<GTask>>,
}

// =================================================================
//    Methods implementation                                       |
// =================================================================

impl GTasksState {
    /// Creates the state, and starts the background task fetching the tasks.
    ///
    /// Returns `None` if the state is turned off in the config, or failed to start.
    /// Only the OAuth2 authenticator is prepared here (it reads local files). Nothing
    /// is fetched: the state isn't ready until the first tasks arrive.
    pub async fn new(config: GTasksConfig, context: &AppContext) -> Option<Self> {
        if !config.enabled {
            return None;
        }

        let auth = GoogleAuth::new()
            .await
            .inspect_err(|e| {
                error!("Failed to initialize Google authentication, disabling GTasks state: {e:#}")
            })
            .ok()?;

        let fetcher = TasksFetcher {
            client: context.client.clone(),
            auth,
        };

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

    /// Placeholder until the tasks are drawn: fills the panel in blue.
    fn draw(&mut self, matrix: &mut Matrix) {
        // No fresh list of tasks: draw nothing, the frame stays black
        let Some(_tasks) = self.tasks.latest() else {
            return;
        };

        let Ok(()) = matrix.clear(Rgb888::BLUE);
    }
}

// =================================================================
//    Background task                                              |
// =================================================================

/// Fetches the tasks for the background task of the state.
///
/// Owns everything a fetch needs: the task outlives the function that starts it.
struct TasksFetcher {
    /// HTTP client shared with the other states
    client: Client,
    /// Google authentication, prepared once at startup
    auth: GoogleAuth,
}

impl Fetcher for TasksFetcher {
    type Data = Vec<GTask>;

    const NAME: &'static str = "Google Tasks";

    const REFRESH_PERIOD: Duration = Duration::from_mins(5);

    /// The tasks are the next ones to come: an old list may show tasks already over
    const MAX_AGE: Duration = Duration::from_hours(1);

    /// Longer than the default: on the first run, the fetch waits for the Google
    /// authorization to be completed in the browser.
    const TIMEOUT: Duration = Duration::from_mins(2);

    /// Fetches the next tasks from the GCalendar API
    async fn fetch(&mut self) -> Result<Vec<GTask>> {
        // From the cache, unless a new one is needed
        let token = self.auth.token(GCALENDAR_SCOPES).await?;

        let query_params = &[
            ("maxResults", "5"),
            ("orderBy", "startTime"),
            ("singleEvents", "true"),
            ("timeMin", &Utc::now().to_rfc3339()), // tasks not over yet
        ];

        let response: TasksResponse = self
            .client
            .get(GCALENDAR_EVENTS_URL)
            .bearer_auth(token)
            .query(query_params)
            .send()
            .await
            .context("failed to send GTASKS API request")?
            // Without this, an error (e.g. "401 Unauthorized") has no "items": it would
            // look like "no task"
            .error_for_status()
            .context("GTASKS API returned an error")?
            .json()
            .await
            .context("failed to deserialize GTASKS API response")?;

        let tasks = response.items;
        log_tasks(&tasks);

        Ok(tasks)
    }
}

// =================================================================
//    Helpers                                                      |
// =================================================================

/// Logs how many tasks were fetched, and each of them at the `debug` level.
fn log_tasks(tasks: &[GTask]) {
    info!("Found {} tasks", tasks.len());

    for task in tasks {
        debug!("Task: {} ({:?})", task.title, task.start);

        if let Some(description) = &task.description {
            debug!("Task description: {description}");
        }
    }
}

// =================================================================
//    Private types                                                |
// =================================================================

/// Response of the GCalendar API (only the field used: the others are ignored).
#[derive(Debug, Deserialize)]
struct TasksResponse {
    /// The events, already sorted by start time (empty if the field is missing)
    #[serde(default)]
    items: Vec<GTask>,
}

// =================================================================
//    Tests                                                        |
// =================================================================

#[cfg(test)]
mod tests {
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
        let tasks = serde_json::from_str::<TasksResponse>(RESPONSE)
            .unwrap()
            .items;
        assert_eq!(tasks.len(), 2);

        assert_eq!(tasks[0].title, "Dentist");
        assert_eq!(tasks[0].description.as_deref(), Some("Bring the X-ray"));
        // Compared as an instant: doesn't depend on the time zone of the machine
        let expected = DateTime::parse_from_rfc3339("2026-11-16T17:30:00Z").unwrap();
        assert!(matches!(tasks[0].start, TaskStart::At { time } if time == expected));

        assert_eq!(tasks[1].title, "");
        assert_eq!(tasks[1].description, None);
        let expected = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        assert_eq!(tasks[1].start, TaskStart::AllDay { date: expected });
    }

    #[test]
    fn response_without_items_means_no_task() {
        let tasks = serde_json::from_str::<TasksResponse>(r#"{ "kind": "calendar#events" }"#)
            .unwrap()
            .items;
        assert!(tasks.is_empty());
    }

    #[test]
    fn task_without_start_is_refused() {
        let response = r#"{ "items": [ { "summary": "Dentist", "start": {} } ] }"#;
        assert!(serde_json::from_str::<TasksResponse>(response).is_err());
    }
}
