use crate::app::states::app_state::AppContext;
use crate::app::dep::oauth2::*;
use reqwest::Client;
use reqwest::header::{AUTHORIZATION, CONTENT_TYPE};
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

/* Public struct containing gtasks data */
#[derive(Default)] // to be able to fill with default values
pub struct GTasksData {
    tasks:                  Vec<GTask>,     // Vec of tasks
    nb:                     usize,          // b of tasks in tasks
    request_url:            String,         // full request URL to send to weather API
    oauth2:                 OAuth2Data,     // OAuth2 authenticator data
}

/* Atomic task */
#[derive(Debug)] // to be able to print with {:?}
struct GTask {
    title:          String,         // title of the task
    date:           String,         // date of the task
    description:    String,         // description of the task
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl GTasksData {

    /* GTasksData constructor */
    pub async fn new(context: &AppContext) -> Self {
        /* Initialize some data in the struct */
        let mut gtasks_object = init().await;

        /* Get data from the GCalendar API and directly fill gtasks_object */
        if let Err(e) = get_gtasks_events(&context.client, &mut gtasks_object).await {
            error!("Failed to fetch Google Tasks during init: {:?}", e);
        }

        /* Return GTasksData struct */
        gtasks_object
    }

    /* Method for drawing data to the screen */
    pub fn draw(&self) {
        /* Print number of tasks */
        info!("[GOOGLE TASKS] - Found {} Tasks", self.nb);

        /* Print each task */
        for task in &self.tasks {
            info!("[GOOGLE TASKS] - Task: {} ({})", task.title, task.date);
            
            if !task.description.is_empty() {
                info!("[GOOGLE TASKS] - Task description: {}", task.description);
            }
        }
    }

    /* Method for fetching data from public API */
    pub async fn fetch_data(&mut self, context: &AppContext) {
        /* Get data from the GCalendar API and directly fill self */
        if let Err(e) = get_gtasks_events(&context.client, self).await {
            error!("Failed to fetch Google Tasks: {:?}", e);
        }
    }
}


// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/* Function that initializes the GTasksData struct */
async fn init() -> GTasksData {
    // Perform OAuth2 login to get the authenticator
    let auth = match oauth2_get_google_authenticator().await {
        Ok(a) => a,
        Err(e) => {
            error!("Failed to initialize OAuth2 authenticator: {:?}", e);
            OAuth2Data::default()
        }
    };

    GTasksData {
        request_url:            GCALENDAR_API_BASE_URL.to_string(),
        oauth2:                 auth,
        ..Default::default()    // completes other fields with default values
    }
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
    gtasks_object.nb = tasks.len();
    gtasks_object.tasks = tasks;

    Ok(())
}


// ================================================================= 
//    Private types                                                |
// ================================================================= 
