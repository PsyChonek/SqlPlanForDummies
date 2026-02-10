use tauri::AppHandle;
use tauri_plugin_store::StoreExt;

use super::types::{ConnectionConfig, PlanHistoryEntry, QueryHistoryEntry};

const CONNECTIONS_STORE: &str = "connections.json";
const HISTORY_STORE: &str = "history.json";

pub fn get_connections(app: &AppHandle) -> Result<Vec<ConnectionConfig>, String> {
    let store = app.store(CONNECTIONS_STORE).map_err(|e| e.to_string())?;
    let connections: Vec<ConnectionConfig> = store
        .get("connections")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(connections)
}

pub fn save_connections(app: &AppHandle, connections: &[ConnectionConfig]) -> Result<(), String> {
    let store = app.store(CONNECTIONS_STORE).map_err(|e| e.to_string())?;
    store.set(
        "connections",
        serde_json::to_value(connections).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_query_history(app: &AppHandle) -> Result<Vec<QueryHistoryEntry>, String> {
    let store = app.store(HISTORY_STORE).map_err(|e| e.to_string())?;
    let history: Vec<QueryHistoryEntry> = store
        .get("queryHistory")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(history)
}

pub fn save_query_history(app: &AppHandle, history: &[QueryHistoryEntry]) -> Result<(), String> {
    let store = app.store(HISTORY_STORE).map_err(|e| e.to_string())?;
    store.set(
        "queryHistory",
        serde_json::to_value(history).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}

pub fn get_plan_history(app: &AppHandle) -> Result<Vec<PlanHistoryEntry>, String> {
    let store = app.store(HISTORY_STORE).map_err(|e| e.to_string())?;
    let history: Vec<PlanHistoryEntry> = store
        .get("planHistory")
        .and_then(|v| serde_json::from_value(v).ok())
        .unwrap_or_default();
    Ok(history)
}

pub fn save_plan_history(app: &AppHandle, history: &[PlanHistoryEntry]) -> Result<(), String> {
    let store = app.store(HISTORY_STORE).map_err(|e| e.to_string())?;
    store.set(
        "planHistory",
        serde_json::to_value(history).map_err(|e| e.to_string())?,
    );
    store.save().map_err(|e| e.to_string())?;
    Ok(())
}
