use chrono::Utc;
use uuid::Uuid;

use super::connection::{AppState, DbConnection};
use super::encryption;
use super::store;
use super::types::*;

#[tauri::command]
pub async fn test_connection(request: ConnectionRequest) -> Result<String, String> {
    let conn = DbConnection::connect(
        &request.host,
        request.port,
        &request.database,
        &request.username,
        &request.password,
    )
    .await?;

    // Verify with a simple query
    let result = conn
        .execute_query("SELECT 1 AS test", &PlanType::None)
        .await?;

    if result.rows.is_empty() {
        return Err("Connection test failed: no response from server".into());
    }

    Ok("Connection successful".into())
}

#[tauri::command]
pub async fn connect_db(
    request: ConnectionRequest,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let conn = DbConnection::connect(
        &request.host,
        request.port,
        &request.database,
        &request.username,
        &request.password,
    )
    .await?;

    *state.connection.lock().await = Some(conn);
    Ok(format!(
        "Connected to {}:{}/{}",
        request.host, request.port, request.database
    ))
}

#[tauri::command]
pub async fn disconnect_db(state: tauri::State<'_, AppState>) -> Result<(), String> {
    *state.connection.lock().await = None;
    Ok(())
}

#[tauri::command]
pub async fn execute_query(
    request: QueryRequest,
    state: tauri::State<'_, AppState>,
) -> Result<QueryResult, String> {
    let lock = state.connection.lock().await;
    let conn = lock.as_ref().ok_or("Not connected to database")?;
    conn.execute_query(&request.sql, &request.plan_type).await
}

#[tauri::command]
pub async fn save_connection(
    request: SaveConnectionRequest,
    app: tauri::AppHandle,
) -> Result<ConnectionConfig, String> {
    let encrypted_password = encryption::encrypt_password(&request.password)?;

    let config = ConnectionConfig {
        id: Uuid::new_v4().to_string(),
        name: request.name,
        host: request.host,
        port: request.port,
        database: request.database,
        username: request.username,
        encrypted_password,
        last_used: Some(Utc::now()),
        created_at: Utc::now(),
    };

    let mut connections = store::get_connections(&app)?;
    connections.push(config.clone());
    store::save_connections(&app, &connections)?;

    Ok(config)
}

#[tauri::command]
pub async fn get_connections(app: tauri::AppHandle) -> Result<Vec<ConnectionConfig>, String> {
    store::get_connections(&app)
}

#[tauri::command]
pub async fn delete_connection(id: String, app: tauri::AppHandle) -> Result<(), String> {
    let mut connections = store::get_connections(&app)?;
    connections.retain(|c| c.id != id);
    store::save_connections(&app, &connections)?;
    Ok(())
}

#[tauri::command]
pub async fn connect_saved(
    id: String,
    app: tauri::AppHandle,
    state: tauri::State<'_, AppState>,
) -> Result<String, String> {
    let mut connections = store::get_connections(&app)?;
    let conn_config = connections
        .iter_mut()
        .find(|c| c.id == id)
        .ok_or("Connection not found")?;

    let password = encryption::decrypt_password(&conn_config.encrypted_password)?;

    let conn = DbConnection::connect(
        &conn_config.host,
        conn_config.port,
        &conn_config.database,
        &conn_config.username,
        &password,
    )
    .await?;

    let display = format!(
        "Connected to {}:{}/{}",
        conn_config.host, conn_config.port, conn_config.database
    );

    conn_config.last_used = Some(Utc::now());
    store::save_connections(&app, &connections)?;

    *state.connection.lock().await = Some(conn);
    Ok(display)
}

#[tauri::command]
pub async fn get_query_history(app: tauri::AppHandle) -> Result<Vec<QueryHistoryEntry>, String> {
    store::get_query_history(&app)
}

#[tauri::command]
pub async fn save_query_history_entry(
    entry: QueryHistoryEntry,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut history = store::get_query_history(&app)?;
    history.insert(0, entry);
    if history.len() > 100 {
        history.truncate(100);
    }
    store::save_query_history(&app, &history)?;
    Ok(())
}

#[tauri::command]
pub async fn get_plan_history(app: tauri::AppHandle) -> Result<Vec<PlanHistoryEntry>, String> {
    store::get_plan_history(&app)
}

#[tauri::command]
pub async fn save_plan_history_entry(
    entry: PlanHistoryEntry,
    app: tauri::AppHandle,
) -> Result<(), String> {
    let mut history = store::get_plan_history(&app)?;
    history.insert(0, entry);
    if history.len() > 50 {
        history.truncate(50);
    }
    store::save_plan_history(&app, &history)?;
    Ok(())
}
