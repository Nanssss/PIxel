use reqwest::blocking::Client;
use crate::app::states::app_state::AppContext;

const GCALENDAR_API_BASE_URL: &str = "...";

/* Public struct containing gtasks data */
pub struct GTasksData {
    tasks:          Vec<GTask>,     // Vec of tasks
    nb:             usize,          // nb of tasks in tasks
    request_url:    String,         // full request URL to send to weather API
}

/* Atomic task */
struct GTask {
    title:          String,         // title of the task
    date:           String,         // date of the task
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl GTasksData {

    /* GTasksData constructor */
    pub fn new(context: &AppContext) -> Self {
        /* Initialize some data in the struct */
        // let gtasks_init = init();

        /* Get data from the GCalendar API */
        // let data = get_gtasks(&context.client, &gtasks_init.request_url);

        /* Return GTasksData struct */
        GTasksData {
            tasks:      Vec::new(),     // to complete from GCalendar API call,-> data.xx
            nb:         4,              // same -> data.nb
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
    pub fn fetch_data(&mut self, context: &AppContext) {
        /* Get data from the GCalendar API */
        // let data = get_gtasks(&context.client, &gtasks_init.request_url);

        /* Update self fields */
        // self.tasks = ..;
        self.nb = 5;            // data.nb
    }
}


// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/* Function that initializes the GTasksData struct */
// fn init() -> GTasksData {
// }

/* Function to get data from public weather API */
// fn get_weather(client: &Client, url: &String) -> DailyData {
// }


// ================================================================= 
//    Private types                                                |
// ================================================================= 
