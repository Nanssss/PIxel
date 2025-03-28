use reqwest::blocking::Client;
use crate::app::states::app_state::AppContext;

const GCALENDAR_API_BASE_URL: &str = "...";

/* Public struct containing gtasks data */
pub struct GTasksData {
    tasks:      Vec<GTask>,     // Vec of tasks
    nb:         usize,          // Nb of tasks in tasks
}

/* Atomic task */
struct GTask {
    title:      String,         // Title of the task
    date:       String,         // Date of the task
}


// ================================================================= 
//    Methods implementation                                       |
// ================================================================= 

impl GTasksData {

    /* GTasksData constructor */
    pub fn new(context: &AppContext) -> Self {
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
    }
}


// ================================================================= 
//    Static functions                                             |
// ================================================================= 

/* Function that initializes the GTasksData struct */
fn init() -> GTasksData {
}


// ================================================================= 
//    Private types                                                |
// ================================================================= 

