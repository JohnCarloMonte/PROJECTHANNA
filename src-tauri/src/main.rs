// Prevents an extra console window on Windows in release builds.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

mod commands;

fn main() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_http::init())
        .invoke_handler(tauri::generate_handler![
            commands::audio::start_recording,
            commands::audio::stop_recording,
            commands::models::transcribe,
            commands::models::ask_llm,
            commands::models::embed_text,
            commands::models::speak,
            commands::system::open_app,
            commands::system::list_local_tracks,
            commands::system::play_track,
            commands::memory::init_memory,
            commands::memory::save_message,
            commands::memory::save_fact,
            commands::memory::recall_similar,
            commands::memory::get_personality,
            commands::memory::update_personality,
        ])
        .run(tauri::generate_context!())
        .expect("error while running Hanna");
}
