use crate::app::states;

pub fn hello_from_appmain() {
    println!("Hello from app_main!");

    states::clock::hello_from_clock();
}
