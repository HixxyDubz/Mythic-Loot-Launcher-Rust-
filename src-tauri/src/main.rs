// Prevents additional console window on Windows in release, DO NOT REMOVE!!
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn main() {
    if let Some(code) = mythic_loot_launcher_lib::try_run_update_helper() {
        std::process::exit(code);
    }
    mythic_loot_launcher_lib::run()
}
