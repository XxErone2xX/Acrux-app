#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = acrux_lib::session_helper() {
        std::process::exit(code);
    }
    acrux_lib::run();
}
