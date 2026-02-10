mod db;

use db::connection::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState {
            connection: Arc::new(Mutex::new(None)),
        })
        .invoke_handler(tauri::generate_handler![
            greet,
            db::commands::test_connection,
            db::commands::connect_db,
            db::commands::disconnect_db,
            db::commands::execute_query,
            db::commands::save_connection,
            db::commands::get_connections,
            db::commands::delete_connection,
            db::commands::connect_saved,
            db::commands::get_query_history,
            db::commands::save_query_history_entry,
            db::commands::get_plan_history,
            db::commands::save_plan_history_entry,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
