mod db;
#[cfg(target_os = "windows")]
mod xel;

use db::connection::AppState;
use std::sync::Arc;
use tokio::sync::Mutex;

#[cfg(target_os = "windows")]
use xel::store::XelAppState;
#[cfg(target_os = "windows")]
use tokio::sync::RwLock;

#[tauri::command]
fn greet(name: &str) -> String {
    format!("Hello, {}! You've been greeted from Rust!", name)
}

#[tauri::command]
fn get_platform() -> String {
    std::env::consts::OS.to_string()
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let builder = tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_store::Builder::new().build())
        .manage(AppState {
            connection: Arc::new(Mutex::new(None)),
        });

    #[cfg(target_os = "windows")]
    let builder = builder
        .manage(XelAppState {
            store: Arc::new(RwLock::new(xel::store::XelStore::new())),
        });

    builder
        .invoke_handler(tauri::generate_handler![
            greet,
            get_platform,
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
            #[cfg(target_os = "windows")]
            xel::commands::xel_pick_files,
            #[cfg(target_os = "windows")]
            xel::commands::xel_check_powershell,
            #[cfg(target_os = "windows")]
            xel::commands::xel_load_files,
            #[cfg(target_os = "windows")]
            xel::commands::xel_query_events,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_event,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_stats,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_timeline,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_distinct_values,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_related_events,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_transaction_objects,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_problem_stats,
            #[cfg(target_os = "windows")]
            xel::commands::xel_analyze_blocking,
            #[cfg(target_os = "windows")]
            xel::commands::xel_enrich_from_db,
            #[cfg(target_os = "windows")]
            xel::commands::xel_get_columns,
            #[cfg(target_os = "windows")]
            xel::commands::xel_clear,
        ])
        .run(tauri::generate_context!())
        .expect("error while running tauri application");
}
