use crate::app::states;
use crate::app::data;

pub fn hello_from_appmain() {
    println!("Hello from app_main!");

    data::fetch_data::hello_from_fetch_data();
    states::clock::hello_from_clock();
}
