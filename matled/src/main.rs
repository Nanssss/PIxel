/* Include ./app.rs */
mod app;
use app::app_main;

fn main() {
    println!("Hello from main");
    app_main::hello_from_appmain();
}
