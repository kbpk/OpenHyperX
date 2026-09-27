#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if std::env::args().any(|arg| arg == "--smoke-test") {
        if let Err(error) = hyperx_gui::smoke_test() {
            eprintln!("{error:#}");
            std::process::exit(1);
        }
        return;
    }
    hyperx_gui::desktop::run(std::env::args().any(|arg| arg == "--demo"));
}
