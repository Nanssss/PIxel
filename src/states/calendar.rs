//! The calendar state: shows the next events of the user's Google calendar.
//!
//! The events are fetched in the background (see `feed.rs`) from the Google
//! Calendar API, with the OAuth2 tokens of `google_auth.rs`.

use std::time::Duration;

use anyhow::{Context, Result};
use chrono::{DateTime, Local, NaiveDate, Utc};
use embedded_graphics::{pixelcolor::Rgb888, prelude::*};
use reqwest::Client;
use serde::Deserialize;
use tracing::{debug, error, info};

use super::{AppContext, State};
use crate::feed::{Feed, Fetcher};
use crate::google_auth::GoogleAuth;
use crate::matrix::Matrix;

/// Events of the user's main calendar.
/// Doc: <https://developers.google.com/workspace/calendar/api/v3/reference/events/list>
const CALENDAR_API_URL: &str = "https://www.googleapis.com/calendar/v3/calendars/primary/events";
/// Access requested from the user: read only the calendar events.
/// Other scopes: <https://developers.google.com/workspace/calendar/api/auth>
const CALENDAR_SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar.events.readonly"];

/// Config of the calendar: the `[calendar]` section of `config.toml`.
#[derive(Debug, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct CalendarConfig {
    enabled: bool,
}

/// Values used when the section, or one of its fields, is missing: the calendar is shown.
impl Default for CalendarConfig {
    fn default() -> Self {
        Self { enabled: true }
    }
}

/// One event of the calendar, read as is from the JSON of the API.
#[derive(Debug, Deserialize)]
struct Event {
    #[serde(rename = "summary", default)]
    title: String,
    start: EventStart,
    description: Option<String>,
}

/// When an event starts.
///
/// The API gives it in one of two shapes: `{ "dateTime": "2026-11-15T18:30:00+01:00" }`,
/// or `{ "date": "2026-11-15" }` for an event lasting the whole day.
#[derive(Debug, Clone, Copy, PartialEq, Deserialize)]
#[serde(untagged)]
enum EventStart {
    At {
        #[serde(rename = "dateTime")]
        time: DateTime<Local>,
    },
    AllDay {
        date: NaiveDate,
    },
}

/// Shows the next events of the user's calendar, fetched by a background task (see `Feed`).
pub struct CalendarState {
    /// Latest events published by the background task (an empty `Vec` if there is none)
    events: Feed<Vec<Event>>,
}

// ===== impl CalendarState =====

impl CalendarState {
    /// Creates the state, and starts the background task fetching the events.
    ///
    /// Returns `None` if the state is turned off in the config, or failed to start.
    /// Only the OAuth2 authenticator is prepared here (it reads local files). Nothing
    /// is fetched: the state isn't ready until the first events arrive.
    pub async fn new(config: CalendarConfig, context: &AppContext) -> Option<Self> {
        if !config.enabled {
            return None;
        }

        let auth = GoogleAuth::new()
            .await
            .inspect_err(|e| {
                error!("Failed to initialize Google authentication, disabling the calendar: {e:#}")
            })
            .ok()?;

        let fetcher = EventsFetcher {
            client: context.client.clone(),
            auth,
        };

        Some(Self {
            events: Feed::spawn(fetcher),
        })
    }
}

impl State for CalendarState {
    /// Ready as soon as the background task published a list of events that is still fresh
    fn is_ready(&self) -> bool {
        self.events.latest().is_some()
    }

    /// Placeholder until the events are drawn: fills the panel in blue.
    fn draw(&mut self, matrix: &mut Matrix) {
        // No fresh list of events: draw nothing, the frame stays black
        let Some(_events) = self.events.latest() else {
            return;
        };

        let Ok(()) = matrix.clear(Rgb888::BLUE);
    }
}

// ===== Background task =====

/// Fetches the events for the background task of the state.
struct EventsFetcher {
    /// HTTP client shared with the other states
    client: Client,
    /// Google authentication, prepared once at startup
    auth: GoogleAuth,
}

impl Fetcher for EventsFetcher {
    type Data = Vec<Event>;

    const NAME: &'static str = "calendar events";

    /// A new event, or one just over, shows on the panel within 5 minutes
    const REFRESH_PERIOD: Duration = Duration::from_mins(5);

    /// The events are the next ones to come: an old list may show events already over
    const MAX_AGE: Duration = Duration::from_hours(1);

    /// Longer than the default: on the first run, the fetch waits for the Google
    /// authorization to be completed in the browser.
    const TIMEOUT: Duration = Duration::from_mins(2);

    /// Fetches the next events from the calendar API
    async fn fetch(&mut self) -> Result<Vec<Event>> {
        let token = self.auth.token(CALENDAR_SCOPES).await?;

        let query_params = &[
            ("maxResults", "5"),
            ("orderBy", "startTime"),
            ("singleEvents", "true"),
            ("timeMin", &Utc::now().to_rfc3339()), // events not over yet
        ];

        let response: CalendarResponse = self
            .client
            .get(CALENDAR_API_URL)
            .bearer_auth(token)
            .query(query_params)
            .send()
            .await
            .context("failed to send calendar API request")?
            // Without this, an error (e.g. "401 Unauthorized") has no "items": it would
            // look like "no event"
            .error_for_status()
            .context("calendar API returned an error")?
            .json()
            .await
            .context("failed to deserialize calendar API response")?;

        let events = response.items;
        log_events(&events);

        Ok(events)
    }
}

// ===== Helpers =====

/// Logs how many events were fetched, and each of them at the `debug` level.
fn log_events(events: &[Event]) {
    info!("Found {} events", events.len());

    for event in events {
        debug!("Event: {} ({:?})", event.title, event.start);

        if let Some(description) = &event.description {
            debug!("Event description: {description}");
        }
    }
}

// ===== API response =====

/// Response of the calendar API (only the field used: the others are ignored).
#[derive(Debug, Deserialize)]
struct CalendarResponse {
    /// The events, already sorted by start time (empty if the field is missing)
    #[serde(default)]
    items: Vec<Event>,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A response of the calendar API (shortened): an event at a given time, then an
    /// all-day event without title nor description.
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
    fn events_are_read_from_the_api_response() {
        let events = serde_json::from_str::<CalendarResponse>(RESPONSE)
            .unwrap()
            .items;
        assert_eq!(events.len(), 2);

        assert_eq!(events[0].title, "Dentist");
        assert_eq!(events[0].description.as_deref(), Some("Bring the X-ray"));
        // Compared as an instant: doesn't depend on the time zone of the machine
        let expected = DateTime::parse_from_rfc3339("2026-11-16T17:30:00Z").unwrap();
        assert!(matches!(events[0].start, EventStart::At { time } if time == expected));

        assert_eq!(events[1].title, "");
        assert_eq!(events[1].description, None);
        let expected = NaiveDate::from_ymd_opt(2026, 11, 17).unwrap();
        assert_eq!(events[1].start, EventStart::AllDay { date: expected });
    }

    #[test]
    fn response_without_items_means_no_event() {
        let events = serde_json::from_str::<CalendarResponse>(r#"{ "kind": "calendar#events" }"#)
            .unwrap()
            .items;
        assert!(events.is_empty());
    }

    #[test]
    fn event_without_start_is_refused() {
        let response = r#"{ "items": [ { "summary": "Dentist", "start": {} } ] }"#;
        assert!(serde_json::from_str::<CalendarResponse>(response).is_err());
    }
}
