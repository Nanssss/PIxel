use std::sync::Arc;
use crate::app::states::app_state::{AppContext, Status, FETCH_RETRY_DELAY, FETCH_TIMEOUT};
use crate::app::states::feed::{Feed, FeedPublisher};
use crate::app::dep::oauth2::*;
use crate::app::matrix::{Color, Matrix};
use reqwest::Client;
use tokio::time::{sleep, timeout, Duration};
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
use serde::Deserialize;
use serde_json::Value;
use tracing::{trace, info, error};
use anyhow::{Result, Context};
use chrono::Utc;

const GCALENDAR_API_BASE_URL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary";
/* 
    Add the GCALENDAR scopes you need here.
    (find them at https://developers.google.com/workspace/calendar/api/auth?hl=fr)
*/
const GCALENDAR_SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar.events.readonly"];
/// How often the tasks are fetched
const REFRESH_PERIOD: Duration = Duration::from_secs(5 * 60);

/// Public struct containing gtasks config
#[derive(Debug, Clone, Deserialize)]
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
#[derive(Debug)] // to be able to print with {:?}
struct GTask {
    title:          String,         // title of the task
    date:           String,         // date of the task
    description:    String,         // description of the task
}

/// Public struct containing global gtasks state
///
/// The state never fetches anything itself: a background task (`fetch_loop`) does,
/// and the state only reads the latest tasks it published.
pub struct GTasksState {
    /// Latest tasks published by the background task (an empty `Vec` if there is no task)
    tasks:  Feed<Vec<GTask>>,
    config: GTasksConfig,           // what the user asked for: never modified
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl GTasksState {
    /// GTasksState constructor: starts the background task fetching the tasks.
    ///
    /// Only the OAuth2 authenticator is prepared here (it reads local files). Nothing
    /// is fetched: the state is `Waiting` until the first tasks arrive.
    pub async fn new(config: GTasksConfig, context: &AppContext) -> Self {
        /* The state starts disabled, and is only switched on if everything goes well */
        let mut state = GTasksState {
            tasks: Feed::Disabled,
            config,
        };

        /* A state turned off in the config stays disabled: its task is never started */
        if !state.config.is_enabled() {
            return state;
        }

        /* If in the future the request needs config, pass it &state.config */
        match oauth2_get_google_authenticator().await {
            Ok(oauth2) => {
                // The task gets the writing side of the feed, the state keeps the reading side
                let (publisher, feed) = Feed::channel();

                // `tokio::spawn` starts the task and returns immediately. The task must own
                // everything it uses: `Arc::clone` only copies a pointer to the shared client.
                tokio::spawn(fetch_loop(Arc::clone(&context.client), oauth2, publisher));

                state.tasks = feed;
            }
            Err(e) => {
                /* Unrecoverable error: the state stays `Disabled` */
                error!("Failed to initialize OAuth2 authenticator, disabling GTasks state.\n{e:#}");
            }
        }

        state
    }

    /// Returns the runtime status of the state, deduced from what the task published so far
    pub fn status(&self) -> Status {
        self.tasks.status()
    }

    /// Method for drawing the state to the screen
    pub fn draw(&self, matrix: &mut Matrix) {
        // The app only draws `Ready` states, so the `else` is a safety net
        let Some(_tasks) = self.tasks.latest() else { return };

        let color = Color { r: 0, g: 0, b: 255 };
        matrix.fill(color);
    }
}


// ================================================================= 
//    Background task                                              |
// ================================================================= 

/// Fetches the tasks forever, and publishes each new list to the state.
///
/// Runs beside the frame loop, so a slow network never freezes the display.
async fn fetch_loop(client: Arc<Client>, oauth2: OAuth2Data, publisher: FeedPublisher<Vec<GTask>>) {
    loop {
        // `timeout` gives up on a fetch that hangs (e.g. wifi lost during the request).
        // It wraps the result in a second `Result`: the outer `Err` means "timed out".
        let delay = match timeout(FETCH_TIMEOUT, fetch_tasks(&client, &oauth2)).await {
            Ok(Ok(tasks)) => {
                publisher.publish(tasks);
                REFRESH_PERIOD
            }
            /* On a failure the last known tasks are kept, and the fetch is retried sooner */
            Ok(Err(e)) => {
                error!("Failed to fetch Google Tasks: {e:#}");
                FETCH_RETRY_DELAY
            }
            Err(_) => {
                error!("Failed to fetch Google Tasks: timed out");
                FETCH_RETRY_DELAY
            }
        };

        sleep(delay).await;
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
        info!("[GOOGLE TASKS] - Task: {} ({})", task.title, task.date);

        if !task.description.is_empty() {
            info!("[GOOGLE TASKS] - Task description: {}", task.description);
        }
    }
}

/// Fetches the next tasks from the GCalendar API.
///
/// The tasks are returned as a new value (rather than written into the state):
/// the background task that calls this then publishes them to the state.
async fn fetch_tasks(client: &Client, oauth2: &OAuth2Data) -> Result<Vec<GTask>> {
    /* Get token from oauth2 function */
    let token = oauth2_get_google_token(oauth2, GCALENDAR_SCOPES).await
        .context("failed to obtain oauth2 token")?;

    /* Request parameters */
    let query_params = &[
        ("maxResults",      "5"),
        ("orderBy",         "startTime"),
        ("singleEvents",    "true"),
        ("timeMin",         &Utc::now().to_rfc3339()), // get current time in RFC3339 format
    ];

    /* Base url */
    let url = format!("{GCALENDAR_API_BASE_URL}/events");

    /* Send GET request to GTASKS API */
    let res = client
        .get(&url)
        .header(AUTHORIZATION, format!("Bearer {}", token))
        .header(CONTENT_TYPE, "application/json")
        .query(query_params)
        .send()
        .await
        .context("failed to send GTASKS API request")?;

    let body = res.text().await.context("failed to read response body")?;
    trace!("GTASKS API Response Body: {}", body);
    let json: Value = serde_json::from_str(&body).context("failed to parse GTASKS JSON")?;

    // Parse events
    let mut tasks = Vec::new();
    if let Some(items) = json.get("items").and_then(|v| v.as_array()) {
        for item in items {
            let title = item
                .get("summary")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let date = item
                .get("start")
                .and_then(|v| v.get("dateTime").or_else(|| v.get("date")))
                .and_then(|v| v.as_str()).unwrap_or("").to_string();
            let description = item
                .get("description")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            tasks.push(GTask { title, date, description });
        }
    }
    trace!("Parsed Tasks: {:#?}", tasks);
    log_tasks(&tasks);

    Ok(tasks)
}


// ================================================================= 
//    Private types                                                |
// ================================================================= 
