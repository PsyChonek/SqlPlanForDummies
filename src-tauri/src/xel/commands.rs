use std::collections::HashMap;
use std::path::Path;
use std::sync::Arc;

use tauri::Emitter;

use super::parser;
use super::store::XelAppState;
use super::types::*;
use crate::db::connection::AppState;

#[tauri::command]
pub async fn xel_pick_files() -> Result<Vec<String>, String> {
    use rfd::FileDialog;

    let files = FileDialog::new()
        .add_filter("XEL / XML", &["xel", "xml"])
        .set_title("Select Extended Events files")
        .pick_files();

    match files {
        Some(paths) => Ok(paths
            .into_iter()
            .filter_map(|p| p.to_str().map(|s| s.to_string()))
            .collect()),
        None => Ok(Vec::new()), // user cancelled
    }
}

#[tauri::command]
pub async fn xel_check_powershell() -> Result<PowerShellStatus, String> {
    Ok(parser::check_powershell_availability().await)
}

#[tauri::command]
pub async fn xel_load_files(
    request: XelLoadRequest,
    state: tauri::State<'_, XelAppState>,
    app: tauri::AppHandle,
) -> Result<XelSessionStats, String> {
    let (progress_tx, mut progress_rx) = tokio::sync::mpsc::channel::<XelLoadProgress>(100);

    // Forward progress events to frontend
    let app_clone = app.clone();
    tokio::spawn(async move {
        while let Some(progress) = progress_rx.recv().await {
            let _ = app_clone.emit("xel-load-progress", &progress);
        }
    });

    // Parse files in parallel, max 10 concurrent
    let semaphore = Arc::new(tokio::sync::Semaphore::new(10));
    let mut handles = Vec::new();

    for file_path in &request.file_paths {
        let file_path = file_path.clone();
        let ptx = progress_tx.clone();
        let sem = semaphore.clone();

        let handle = tokio::spawn(async move {
            let _permit = sem.acquire().await.map_err(|e| e.to_string())?;

            let ext = Path::new(&file_path)
                .extension()
                .and_then(|e| e.to_str())
                .unwrap_or("")
                .to_lowercase();

            match ext.as_str() {
                "xel" => parser::parse_xel_files(&[file_path], ptx).await,
                "xml" => parser::parse_xml_file(&file_path, ptx).await,
                _ => Err(format!("Unsupported file type: .{}", ext)),
            }
        });

        handles.push(handle);
    }

    let mut all_events = Vec::new();
    for handle in handles {
        let events = handle
            .await
            .map_err(|e| format!("Task error: {}", e))?
            ?;
        all_events.extend(events);
    }

    // Store events
    let mut store = state.store.write().await;

    if !request.append {
        store.clear();
    }

    store.insert_batch(all_events);

    let empty_filter = XelFilter {
        event_names: vec![],
        time_from: None,
        time_to: None,
        session_ids: vec![],
        object_name_contains: None,
        sql_text_contains: None,
        username: None,
        client_app_name: None,
        database_name: None,
        min_duration_us: None,
        max_duration_us: None,
        source_file: None,
        text_search: None,
        result: None,
        errors_only: false,
        deadlocks_only: false,
    };
    let stats = store.get_stats(&empty_filter);

    // Emit final indexing complete
    let _ = app.emit(
        "xel-load-progress",
        &XelLoadProgress {
            file_name: "all".into(),
            events_parsed: stats.total_events,
            bytes_processed: 0,
            total_bytes: 0,
            phase: LoadPhase::Complete,
        },
    );

    Ok(stats)
}

#[tauri::command]
pub async fn xel_query_events(
    request: XelQueryRequest,
    state: tauri::State<'_, XelAppState>,
) -> Result<XelQueryResponse, String> {
    let store = state.store.read().await;
    Ok(store.query(
        &request.filter,
        request.offset,
        request.limit,
        request.sort_by.as_deref(),
        request.sort_desc,
    ))
}

#[tauri::command]
pub async fn xel_get_event(
    id: u64,
    state: tauri::State<'_, XelAppState>,
) -> Result<Option<XelEvent>, String> {
    let store = state.store.read().await;
    Ok(store.get_event(id).cloned())
}

#[tauri::command]
pub async fn xel_get_stats(
    filter: XelFilter,
    state: tauri::State<'_, XelAppState>,
) -> Result<XelSessionStats, String> {
    let store = state.store.read().await;
    Ok(store.get_stats(&filter))
}

#[tauri::command]
pub async fn xel_get_timeline(
    request: TimelineRequest,
    state: tauri::State<'_, XelAppState>,
) -> Result<Vec<TimelineBucket>, String> {
    let store = state.store.read().await;
    Ok(store.get_timeline(&request.filter, request.bucket_count))
}

#[tauri::command]
pub async fn xel_get_distinct_values(
    field: String,
    state: tauri::State<'_, XelAppState>,
) -> Result<Vec<String>, String> {
    let store = state.store.read().await;
    Ok(store.get_distinct_values(&field))
}

#[tauri::command]
pub async fn xel_get_related_events(
    event_id: u64,
    time_window_ms: Option<i64>,
    limit: Option<usize>,
    state: tauri::State<'_, XelAppState>,
) -> Result<Vec<XelEvent>, String> {
    let store = state.store.read().await;
    Ok(store.get_related_events(
        event_id,
        time_window_ms.unwrap_or(30_000),
        limit.unwrap_or(2000),
    ))
}

#[tauri::command]
pub async fn xel_get_transaction_objects(
    event_id: u64,
    state: tauri::State<'_, XelAppState>,
) -> Result<Vec<TransactionObject>, String> {
    let store = state.store.read().await;
    Ok(store.get_transaction_objects(event_id))
}

#[tauri::command]
pub async fn xel_get_problem_stats(
    filter: XelFilter,
    state: tauri::State<'_, XelAppState>,
) -> Result<XelProblemStats, String> {
    let store = state.store.read().await;
    Ok(store.get_problem_stats(&filter))
}

#[tauri::command]
pub async fn xel_analyze_blocking(
    event_id: u64,
    time_window_ms: Option<i64>,
    state: tauri::State<'_, XelAppState>,
) -> Result<BlockingAnalysis, String> {
    let store = state.store.read().await;
    Ok(store.analyze_blocking(event_id, time_window_ms.unwrap_or(60_000)))
}

/// Helper: run a simple SQL query and return rows as Vec of (col0, col1) string pairs
async fn run_query_pairs(
    conn: &crate::db::connection::DbConnection,
    sql: &str,
) -> Result<Vec<(String, String)>, String> {
    let mut client = conn.client.lock().await;
    let stream = client
        .simple_query(sql)
        .await
        .map_err(|e| format!("{}", e))?;
    let rows = stream
        .into_first_result()
        .await
        .map_err(|e| format!("{}", e))?;
    let mut result = Vec::new();
    for row in rows {
        // Try to get first column as string (various int types)
        let col0 = row
            .try_get::<i64, _>(0)
            .ok()
            .flatten()
            .map(|v| v.to_string())
            .or_else(|| {
                row.try_get::<i32, _>(0)
                    .ok()
                    .flatten()
                    .map(|v| v.to_string())
            })
            .or_else(|| {
                row.try_get::<&str, _>(0)
                    .ok()
                    .flatten()
                    .map(|v| v.to_string())
            })
            .unwrap_or_default();

        let col1 = row
            .try_get::<&str, _>(1)
            .ok()
            .flatten()
            .map(|v| v.to_string())
            .unwrap_or_default();

        if !col0.is_empty() && !col1.is_empty() {
            result.push((col0, col1));
        }
    }
    Ok(result)
}

/// Load cached enrichment data from local store
fn load_enrich_cache(app: &tauri::AppHandle) -> (HashMap<i64, String>, HashMap<i64, String>, HashMap<i64, String>) {
    use tauri_plugin_store::StoreExt;
    let store = match app.store("xel_enrich_cache.json") {
        Ok(s) => s,
        Err(_) => return (HashMap::new(), HashMap::new(), HashMap::new()),
    };

    let parse_map = |key: &str| -> HashMap<i64, String> {
        store
            .get(key)
            .and_then(|v| serde_json::from_value::<HashMap<String, String>>(v).ok())
            .map(|m| {
                m.into_iter()
                    .filter_map(|(k, v)| k.parse::<i64>().ok().map(|id| (id, v)))
                    .collect()
            })
            .unwrap_or_default()
    };

    (parse_map("databases"), parse_map("objects"), parse_map("queries"))
}

/// Save enrichment cache to local store
fn save_enrich_cache(
    app: &tauri::AppHandle,
    dbs: &HashMap<i64, String>,
    objs: &HashMap<i64, String>,
    queries: &HashMap<i64, String>,
) {
    use tauri_plugin_store::StoreExt;
    let store = match app.store("xel_enrich_cache.json") {
        Ok(s) => s,
        Err(_) => return,
    };

    let to_json = |map: &HashMap<i64, String>| -> serde_json::Value {
        let m: HashMap<String, String> = map.iter().map(|(k, v)| (k.to_string(), v.clone())).collect();
        serde_json::to_value(m).unwrap_or_default()
    };

    store.set("databases", to_json(dbs));
    store.set("objects", to_json(objs));
    store.set("queries", to_json(queries));
    let _ = store.save();
}

#[tauri::command]
pub async fn xel_enrich_from_db(
    db_state: tauri::State<'_, AppState>,
    xel_state: tauri::State<'_, XelAppState>,
    app: tauri::AppHandle,
) -> Result<XelEnrichResult, String> {
    let db_conn = db_state.connection.lock().await;
    let conn = db_conn
        .as_ref()
        .ok_or("No database connection. Connect to the source database first.")?;

    let mut errors: Vec<String> = Vec::new();
    let mut databases_resolved = 0usize;
    let mut objects_resolved = 0usize;
    let mut query_texts_resolved = 0usize;

    // Load existing cache
    let (mut cached_dbs, mut cached_objs, mut cached_queries) = load_enrich_cache(&app);
    let mut cache_dirty = false;

    // 1. Resolve database_id → name
    {
        let db_ids = {
            let store = xel_state.store.read().await;
            store.collect_database_ids()
        };
        // Filter out already-cached IDs
        let uncached_db_ids: Vec<i64> = db_ids.iter().filter(|id| !cached_dbs.contains_key(id)).cloned().collect();

        if !uncached_db_ids.is_empty() {
            // Try sys.databases first (works fully on-prem, limited on Azure SQL)
            match run_query_pairs(conn, "SELECT database_id, name FROM sys.databases WITH (NOLOCK)").await {
                Ok(pairs) => {
                    for (id_str, name) in pairs {
                        if let Ok(id) = id_str.parse::<i64>() {
                            cached_dbs.insert(id, name);
                            cache_dirty = true;
                        }
                    }
                }
                Err(e) => errors.push(format!("Database names: {}", e)),
            }
            // Fallback: if any IDs still unresolved, get current DB's id+name
            // (Azure SQL only returns current DB from sys.databases)
            let still_missing: Vec<i64> = uncached_db_ids.iter().filter(|id| !cached_dbs.contains_key(id)).cloned().collect();
            if !still_missing.is_empty() {
                if let Ok(pairs) = run_query_pairs(conn, "SELECT DB_ID(), DB_NAME()").await {
                    for (id_str, name) in pairs {
                        if let Ok(id) = id_str.parse::<i64>() {
                            cached_dbs.insert(id, name);
                            cache_dirty = true;
                        }
                    }
                }
            }
        }
        // Apply all cached (including freshly fetched) to events
        if !cached_dbs.is_empty() {
            let mut store = xel_state.store.write().await;
            databases_resolved = store.apply_database_names(&cached_dbs);
        }
    }

    // 1b. Fetch database isolation settings (RCSI, snapshot isolation)
    {
        let sql = "SELECT name, \
            CAST(is_read_committed_snapshot_on AS CHAR(1)) + ',' + \
            CAST(CASE WHEN snapshot_isolation_state IN (1,3) THEN 1 ELSE 0 END AS CHAR(1)) \
            FROM sys.databases WITH (NOLOCK)";
        match run_query_pairs(conn, sql).await {
            Ok(pairs) => {
                let mut settings: std::collections::HashMap<String, super::types::DbSettings> = std::collections::HashMap::new();
                for (db_name, flags) in pairs {
                    let parts: Vec<&str> = flags.split(',').collect();
                    if parts.len() == 2 {
                        settings.insert(db_name, super::types::DbSettings {
                            is_read_committed_snapshot_on: parts[0] == "1",
                            snapshot_isolation_on: parts[1] == "1",
                        });
                    }
                }
                if !settings.is_empty() {
                    let mut store = xel_state.store.write().await;
                    store.set_db_settings(settings);
                }
            }
            Err(e) => errors.push(format!("DB settings: {}", e)),
        }
    }

    // 2. Resolve associated_object_id → schema.table.index
    {
        let obj_ids = {
            let store = xel_state.store.read().await;
            store.collect_object_ids()
        };
        let uncached: Vec<i64> = obj_ids.iter().filter(|id| !cached_objs.contains_key(id)).cloned().collect();

        if !uncached.is_empty() {
            for chunk in uncached.chunks(100) {
                let id_list: String = chunk.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                // NOLOCK to avoid any blocking on prod; hobt_id filter on sys.partitions
                // Resolve hobt_id → user-visible table.index name.
                // If the object is an internal table (fulltext, service broker, etc.),
                // follow parent_id to the actual user table.
                let sql = format!(
                    "SELECT p.hobt_id, \
                     CASE \
                       WHEN o.type = 'IT' AND it.parent_id IS NOT NULL THEN \
                         SCHEMA_NAME(parent_o.schema_id) + '.' + parent_o.name + \
                         ' (' + o.name + \
                         CASE WHEN i.name IS NOT NULL THEN '.' + i.name ELSE '' END + ')' \
                       ELSE \
                         SCHEMA_NAME(o.schema_id) + '.' + o.name + \
                         CASE WHEN i.name IS NOT NULL THEN '.' + i.name ELSE '' END \
                     END \
                     FROM sys.partitions p WITH (NOLOCK) \
                     JOIN sys.objects o WITH (NOLOCK) ON p.object_id = o.object_id \
                     LEFT JOIN sys.indexes i WITH (NOLOCK) ON p.object_id = i.object_id AND p.index_id = i.index_id \
                     LEFT JOIN sys.internal_tables it WITH (NOLOCK) ON o.object_id = it.object_id AND o.type = 'IT' \
                     LEFT JOIN sys.objects parent_o WITH (NOLOCK) ON it.parent_id = parent_o.object_id \
                     WHERE p.hobt_id IN ({})",
                    id_list
                );
                match run_query_pairs(conn, &sql).await {
                    Ok(pairs) => {
                        for (id_str, name) in pairs {
                            if let Ok(id) = id_str.parse::<i64>() {
                                cached_objs.insert(id, name);
                                cache_dirty = true;
                            }
                        }
                    }
                    Err(e) => { errors.push(format!("Object names: {}", e)); break; }
                }
            }
        }
        if !cached_objs.is_empty() {
            let mut store = xel_state.store.write().await;
            objects_resolved = store.apply_object_names(&cached_objs);
        }
    }

    // 2b. Resolve direct object_ids (from OBJECT: wait_resource, BPR) via sys.objects
    {
        let direct_obj_ids = {
            let store = xel_state.store.read().await;
            store.collect_direct_object_ids()
        };
        // Filter to IDs not already resolved by hobt_id lookup
        let uncached: Vec<i64> = direct_obj_ids.iter()
            .filter(|id| !cached_objs.contains_key(id))
            .cloned()
            .collect();

        if !uncached.is_empty() {
            for chunk in uncached.chunks(100) {
                let id_list: String = chunk.iter().map(|id| id.to_string()).collect::<Vec<_>>().join(",");
                let sql = format!(
                    "SELECT o.object_id, SCHEMA_NAME(o.schema_id) + '.' + o.name \
                     FROM sys.objects o WITH (NOLOCK) \
                     WHERE o.object_id IN ({})",
                    id_list
                );
                match run_query_pairs(conn, &sql).await {
                    Ok(pairs) => {
                        for (id_str, name) in pairs {
                            if let Ok(id) = id_str.parse::<i64>() {
                                cached_objs.insert(id, name);
                                cache_dirty = true;
                            }
                        }
                    }
                    Err(e) => { errors.push(format!("Direct object names: {}", e)); break; }
                }
            }
        }
        if !cached_objs.is_empty() {
            let mut store = xel_state.store.write().await;
            objects_resolved += store.apply_direct_object_names(&cached_objs);
        }
    }

    // 3. Resolve query_hash → query text from Query Store
    {
        let hashes = {
            let store = xel_state.store.read().await;
            store.collect_query_hashes()
        };
        let uncached: Vec<i64> = hashes.iter().filter(|h| !cached_queries.contains_key(h)).cloned().collect();

        if !uncached.is_empty() {
            // Convert i64 hashes to binary(8) hex literals for index-seekable WHERE clause.
            // query_hash in sys.query_store_query is binary(8), so we must match natively.
            for chunk in uncached.chunks(50) {
                let hash_values: String = chunk
                    .iter()
                    .map(|h| {
                        // i64 → unsigned bytes → 0x hex literal
                        let bytes = (*h as u64).to_be_bytes();
                        format!("0x{}", bytes.iter().map(|b| format!("{:02X}", b)).collect::<String>())
                    })
                    .collect::<Vec<_>>()
                    .join(",");
                let sql = format!(
                    "SELECT DISTINCT \
                     CAST(q.query_hash AS BIGINT), \
                     CAST(qt.query_sql_text AS NVARCHAR(MAX)) \
                     FROM sys.query_store_query q WITH (NOLOCK) \
                     JOIN sys.query_store_query_text qt WITH (NOLOCK) ON q.query_text_id = qt.query_text_id \
                     WHERE q.query_hash IN ({})",
                    hash_values
                );
                match run_query_pairs(conn, &sql).await {
                    Ok(pairs) => {
                        for (hash_str, text) in pairs {
                            if let Ok(h) = hash_str.parse::<i64>() {
                                cached_queries.insert(h, text);
                                cache_dirty = true;
                            }
                        }
                    }
                    Err(e) => { errors.push(format!("Query Store: {}", e)); break; }
                }
            }
        }
        if !cached_queries.is_empty() {
            let mut store = xel_state.store.write().await;
            query_texts_resolved = store.apply_query_texts(&cached_queries);
        }
    }

    // Save cache if anything new was fetched
    if cache_dirty {
        save_enrich_cache(&app, &cached_dbs, &cached_objs, &cached_queries);
    }

    Ok(XelEnrichResult {
        databases_resolved,
        objects_resolved,
        query_texts_resolved,
        unique_databases: cached_dbs.len(),
        unique_objects: cached_objs.len(),
        unique_queries: cached_queries.len(),
        errors,
    })
}

#[tauri::command]
pub async fn xel_get_columns(
    state: tauri::State<'_, XelAppState>,
) -> Result<Vec<String>, String> {
    let store = state.store.read().await;
    Ok(store.get_columns())
}

#[tauri::command]
pub async fn xel_clear(
    state: tauri::State<'_, XelAppState>,
) -> Result<(), String> {
    let mut store = state.store.write().await;
    store.clear();
    Ok(())
}
