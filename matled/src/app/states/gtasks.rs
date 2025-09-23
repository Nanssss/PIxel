use reqwest::Client;
use crate::app::states::app_state::AppContext;
use crate::app::dep::oauth2::oauth2_login_google;

const GCALENDAR_API_BASE_URL: &str = "...";
/* 
Add the GCALENDAR scopes you need here.
(find them at https://developers.google.com/workspace/calendar/api/auth?hl=fr)
*/
const GCALENDAR_SCOPES: &[&str] = &["https://www.googleapis.com/auth/calendar.events.readonly"];

/* Public struct containing gtasks data */
#[derive(Default)] // to be able to fill with default values
pub struct GTasksData {
    tasks:          Vec<GTask>,     // Vec of tasks
    nb:             usize,          // nb of tasks in tasks
    request_url:    String,         // full request URL to send to weather API
}

/* Atomic task */
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
        let gtasks_init = init().await;

        /* Get data from the GCalendar API */
        // let data = get_gtasks(&context.client, &gtasks_init.request_url).await;

        /* Return GTasksData struct */
        GTasksData {
            // tasks:      Vec::new(),     // to complete from GCalendar API call,-> data.xx
            // nb:         4,              // same -> data.nb
            ..gtasks_init // completes other fields from init data
        }
    }

    /* Method for drawing data to the screen */
    pub fn draw(&self) {
        println!("\
        TASKS:
            ==========================================
            | Title             | {}
            | Date              | {}
            ==========================================\
        ", self.nb, self.nb);
    }

    /* Method for fetching data from public API */
    pub async fn fetch_data(&mut self, context: &AppContext) {
        /* Get data from the GCalendar API */
        // let data = get_gtasks(&context.client, &self.request_url).await;

        /* Update self fields */
        // self.tasks = ..;
        self.nb = 5;            // data.nb
    }
}


// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/* Function that initializes the GTasksData struct */
async fn init() -> GTasksData {
    // @todo: To correctly use this function, I'll have to use async functions and refactor the whole app
    oauth2_login_google(GCALENDAR_SCOPES).await;

    GTasksData {
        request_url: GCALENDAR_API_BASE_URL.to_string(),
        ..Default::default()        // completes other fields with default values
    }
}

/* Function to get data from public weather API */
// fn get_weather(client: &Client, url: &String) -> DailyData {
// }


// ================================================================= 
//    Private types                                                |
// ================================================================= 
