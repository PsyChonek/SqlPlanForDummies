mod db;
mod xel;

use db::connection::AppState;
use xel::store::XelAppState;
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::sync::RwLock;

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
        .manage(XelAppState {
            store: Arc::new(RwLock::new(xel::store::XelStore::new())),
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
            xel::commands::xel_pick_files,
            xel::commands::xel_check_powershell,
            xel::commands::xel_load_files,
            xel::commands::xel_query_events,
            xel::commands::xel_get_event,
            xel::commands::xel_get_stats,
            xel::commands::xel_get_timeline,
            xel::commands::xel_get_distinct_values,
            xel::commands::xel_get_related_events,
            xel::commands::xel_get_transaction_objects,
            xel::commands::xel_get_problem_stats,
            xel::commands::xel_analyze_blocking,
            xel::commands::xel_enrich_from_db,
            xel::commands::xel_get_columns,
            xel::commands::xel_clear,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
