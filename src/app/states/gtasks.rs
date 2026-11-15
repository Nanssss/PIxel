use crate::app::states::app_state::{AppContext, Status};
use crate::app::dep::oauth2::*;
use crate::app::matrix::{Color, Matrix};
use reqwest::Client;
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

/// Public struct containing gtasks data
#[derive(Default)] // to be able to fill with default values
pub struct GTasksData {
    // `None` until the first successful fetch, `Some(vec![])` when there is no task
    tasks:                  Option<Vec<GTask>>, // last tasks received
    request_url:            String,         // full request URL to send to Google API
    oauth2:                 OAuth2Data,     // OAuth2 authenticator data
}

/// Atomic task
#[derive(Debug)] // to be able to print with {:?}
struct GTask {
    title:          String,         // title of the task
    date:           String,         // date of the task
    description:    String,         // description of the task
}

/// Public struct containing global gtasks state
pub struct GTasksState {
    status: Status,                 // what the app may do with this state
    data:   Option<GTasksData>,     // `None` only when the state is `Disabled`
    config: GTasksConfig,           // what the user asked for: never modified
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl GTasksData {

    /// GTasksData constructor: only prepares the authenticator, nothing is fetched yet
    ///
    /// # Errors
    /// If the OAuth2 authenticator can't be created: this is unrecoverable.
    pub async fn new() -> Result<Self> {
        init()
            .await
            .context("Failed to initialize OAuth2 authenticator")
    }

    /// Method for drawing data to the screen
    pub fn draw(&self, matrix: &mut Matrix) {
        // The app only draws `Ready` states, so this is just a safety net.
        let Some(tasks) = &self.tasks else { return };

        /* Print number of tasks */
        info!("[GOOGLE TASKS] - Found {} Tasks", tasks.len());

        /* Print each task */
        for task in tasks {
            info!("[GOOGLE TASKS] - Task: {} ({})", task.title, task.date);
            
            if !task.description.is_empty() {
                info!("[GOOGLE TASKS] - Task description: {}", task.description);
            }
        }

        let color = Color { r: 0, g: 0, b: 255 };
        matrix.fill(color);
    }

    /// Method for fetching data from public API
    ///
    /// # Errors
    /// If the tasks can't be fetched: the last known tasks (if any) are kept.
    pub async fn fetch_data(&mut self, context: &AppContext) -> Result<()> {
        /* Get data from the GCalendar API and directly fill self.
        The error (if any) is returned to the caller (the state), which decides what to do with it */
        get_gtasks_events(&context.client, self).await
    }
}

impl GTasksState {
    /// GTasksState constructor
    pub async fn new(config: GTasksConfig, context: &AppContext) -> Self {
        /* The state starts disabled, and is only switched on if everything goes well */
        let mut state = GTasksState {
            status: Status::Disabled,
            data: None,
            config,
        };

        /* A state turned off in the config stays disabled: its data is never created */
        if !state.config.is_enabled() {
            return state;
        }

        /* If in the future GTasksData needs config, pass it &state.config */
        match GTasksData::new().await {
            Ok(data) => {
                state.data = Some(data);
                state.status = Status::Waiting;

                /* First fetch. If it fails (e.g. no network at boot), the state
                stays `Waiting` and the fetch is retried on the next rotation. */
                state.refresh(context).await;
            }
            Err(e) => {
                /* Unrecoverable error: the state stays `Disabled` */
                error!("Failed to initialize GTasks, disabling state.\n{e:#}");
            }
        }

        state
    }

    /// Returns the runtime status of the state
    pub fn status(&self) -> Status {
        self.status
    }

    /// Fetches the latest tasks, and updates the status accordingly.
    /// Does nothing if the state is disabled.
    pub async fn refresh(&mut self, context: &AppContext) {
        // `let ... else`: a disabled state has no data, so there is nothing to refresh
        let Some(data) = &mut self.data else { return };

        match data.fetch_data(context).await {
            Ok(()) => self.status = Status::Ready,
            /* The status is kept as it is: a `Waiting` state will be retried,
            a `Ready` state keeps displaying the last known tasks */
            Err(e) => error!("Failed to fetch Google Tasks: {e:#}"),
        }
    }

    /// Method for drawing the state to the screen
    pub fn draw(&self, matrix: &mut Matrix) {
        if let Some(data) = &self.data {
            data.draw(matrix);
        }
    }
}


// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/// Function that initializes the GTasksData struct
async fn init() -> Result<GTasksData> {
    // Perform OAuth2 login to get the authenticator
    let auth = oauth2_get_google_authenticator().await?;

    Ok(GTasksData {
        request_url:            GCALENDAR_API_BASE_URL.to_string(),
        oauth2:                 auth,
        ..Default::default()    // completes other fields with default values
    })
}

/* Function to get data from GTASKS API */
/*
There are 2 possible versions for this function:
    - Using a non-mutable GTasksData as parameter, cloning it and returning a new value. 
    This is a more idiomatic way in Rust, but require more memory allocation.
    - Using a mutable GTasksData as parameter, modifying it directly.
    Here I choose the second option, as it is more efficient in terms of memory usage.
*/
async fn get_gtasks_events(client: &Client, gtasks_object: &mut GTasksData) -> Result<()> {
    /* Get token from oauth2 function */
    let token = oauth2_get_google_token(&gtasks_object.oauth2, GCALENDAR_SCOPES).await
        .context("failed to obtain oauth2 token")?;

    /* Request parameters */
    let query_params = &[
        ("maxResults",      "5"),
        ("orderBy",         "startTime"),
        ("singleEvents",    "true"),
        ("timeMin",         &Utc::now().to_rfc3339()), // get current time in RFC3339 format
    ];

    /* Base url */
    let url = format!("{}/events", gtasks_object.request_url);

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
    gtasks_object.tasks = Some(tasks);

    Ok(())
}


// ================================================================= 
//    Private types                                                |
// ================================================================= 
