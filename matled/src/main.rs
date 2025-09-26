/* Include ./app.rs */
mod app;
use app::app_main;

/* Notes:
    cargo add serde --features derive
    cargo add serde_json
    cargo add tokio --features full
    cargo add reqwest --features json, blocking
    cargo add yup-oauth2
    cargo add hyper_util
*/

#[tokio::main]
async fn main() {
    println!("Hello from main");
    app_main::app_main().await;
}
