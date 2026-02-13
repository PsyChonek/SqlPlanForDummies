use std::sync::Arc;
use tiberius::{AuthMethod, Client, Column, Config, Row};
use tokio::net::TcpStream;
use tokio::sync::Mutex;
use tokio_util::compat::TokioAsyncWriteCompatExt;

use super::types::{PlanType, QueryResult};

type TiberiusClient = Client<tokio_util::compat::Compat<TcpStream>>;

pub struct DbConnection {
    pub client: Arc<Mutex<TiberiusClient>>,
}

pub struct AppState {
    pub connection: Arc<Mutex<Option<DbConnection>>>,
}

impl DbConnection {
    // Helper function to rewrite queries with date columns cast to datetime
    async fn rewrite_query_with_date_cast(
        client: &mut TiberiusClient,
        sql: &str,
    ) -> Result<String, String> {
        // Try to extract table name from simple queries like "SELECT * FROM table"
        let sql_trimmed = sql.trim();
        let sql_lower = sql_trimmed.to_lowercase();

        // Handle simple SELECT * FROM table_name patterns
        // Normalize whitespace to handle various spacing patterns
        let normalized = sql_lower.split_whitespace().collect::<Vec<_>>().join(" ");

        // Also check for patterns like "select*from" (no spaces around *)
        let has_select_star_from = normalized.starts_with("select * from")
            || normalized.starts_with("select*from")
            || (sql_lower.contains("select") && sql_lower.contains("*") && sql_lower.contains("from"));

        eprintln!("DEBUG: Original query: {}", sql);
        eprintln!("DEBUG: Normalized: {}", normalized);
        eprintln!("DEBUG: Pattern matched: {}", has_select_star_from);

        if has_select_star_from {
            // Extract table name (simple pattern matching)
            let after_from = if let Some(pos) = sql_lower.find("from") {
                sql_trimmed[pos + 4..].trim()
            } else {
                return Ok(sql.to_string());
            };

            // Get just the table name (before any WHERE, ORDER BY, etc.)
            let table_name = after_from
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(';');

            if table_name.is_empty() {
                return Ok(sql.to_string());
            }

            eprintln!("DEBUG: Extracted table name: '{}'", table_name);

            // Query for columns with their actual system type
            // This resolves user-defined alias types to their base types
            // Check both tables and views using sys.objects
            let metadata_query = format!(
                "SELECT c.name, c.system_type_id, c.user_type_id, t.name as type_name, st.name as system_type_name \
                FROM sys.columns c \
                INNER JOIN sys.objects o ON c.object_id = o.object_id \
                INNER JOIN sys.types t ON c.user_type_id = t.user_type_id \
                INNER JOIN sys.types st ON c.system_type_id = st.user_type_id \
                WHERE LOWER(o.name) = LOWER('{}') \
                AND o.type IN ('U', 'V') \
                ORDER BY c.column_id",
                table_name.replace("'", "''")
            );

            eprintln!("DEBUG: Metadata query: {}", metadata_query);

            // Debug: List tables/views that match the pattern
            let debug_query = format!(
                "SELECT name, type_desc FROM sys.objects \
                WHERE LOWER(name) LIKE LOWER('%{}%') AND type IN ('U', 'V')",
                table_name.replace("'", "''")
            );
            if let Ok(stream) = client.simple_query(&debug_query).await {
                if let Ok(results) = stream.into_results().await {
                    eprintln!("DEBUG: Found {} matching objects:", results.iter().map(|r| r.len()).sum::<usize>());
                    for result_set in &results {
                        for row in result_set {
                            if let (Some(name), Some(type_desc)) = (
                                row.try_get::<&str, _>(0).ok().flatten(),
                                row.try_get::<&str, _>(1).ok().flatten()
                            ) {
                                eprintln!("  - {} ({})", name, type_desc);
                            }
                        }
                    }
                }
            }

            let stream = match client.simple_query(&metadata_query).await {
                Ok(s) => s,
                Err(e) => {
                    eprintln!("Warning: Failed to query column metadata: {}. Date casting will not be applied.", e);
                    return Ok(sql.to_string());
                }
            };

            let result_sets = match stream.into_results().await {
                Ok(r) => r,
                Err(e) => {
                    eprintln!("Warning: Failed to retrieve column metadata results: {}. Date casting will not be applied.", e);
                    return Ok(sql.to_string());
                }
            };

            eprintln!("DEBUG: Got {} result sets from metadata query", result_sets.len());
            for (i, rs) in result_sets.iter().enumerate() {
                eprintln!("DEBUG: Result set {} has {} rows", i, rs.len());
            }

            let mut columns: Vec<String> = Vec::new();
            let mut has_type_casting = false;

            for result_set in &result_sets {
                for row in result_set {
                    let col_name_result = row.try_get::<&str, _>(0);
                    let system_type_id_result = row.try_get::<u8, _>(1);
                    let user_type_id_result = row.try_get::<i32, _>(2);
                    let system_type_name_result = row.try_get::<&str, _>(4);

                    eprintln!("DEBUG: Row - col_name: {:?}, system_type_id: {:?}, user_type_id: {:?}, system_type_name: {:?}",
                        col_name_result, system_type_id_result, user_type_id_result, system_type_name_result);

                    if let Some(col_name) = col_name_result.ok().flatten() {
                        // Extract values once to avoid move issues
                        let system_type_id = system_type_id_result.ok().flatten();
                        let user_type_id = user_type_id_result.ok().flatten();
                        let system_type_name = system_type_name_result.ok().flatten();

                        let mut needs_cast = false;
                        let mut cast_type = String::new();

                        // Check if it's a date type (needs casting to datetime for Tiberius compatibility)
                        if let Some(sys_type_id) = system_type_id {
                            if sys_type_id == 40 {
                                needs_cast = true;
                                cast_type = "datetime".to_string();
                            }
                        }

                        // Check if it's an alias type (user_type_id != system_type_id)
                        // If so, cast to the base system type
                        if !needs_cast {
                            if let (Some(sys_type_id), Some(usr_type_id)) = (system_type_id, user_type_id) {
                                // If user_type_id differs from system_type_id, it's an alias type
                                if sys_type_id as i32 != usr_type_id {
                                    if let Some(sys_type_name) = system_type_name {
                                        needs_cast = true;
                                        cast_type = sys_type_name.to_string();
                                    }
                                }
                            }
                        }

                        if needs_cast && !cast_type.is_empty() {
                            columns.push(format!("CAST([{}] AS {}) AS [{}]", col_name, cast_type, col_name));
                            has_type_casting = true;
                        } else {
                            columns.push(format!("[{}]", col_name));
                        }
                    }
                }
            }

            eprintln!("DEBUG: Found {} columns, {} require type casting", columns.len(), if has_type_casting { "some" } else { "none" });

            if has_type_casting && !columns.is_empty() {
                // Rebuild the query with explicit column list
                let column_list = columns.join(", ");

                // Get everything after "FROM table_name" (WHERE, ORDER BY, etc.)
                let from_pos = match sql_lower.find("from") {
                    Some(pos) => pos,
                    None => {
                        eprintln!("DEBUG: Failed to find 'from' in query, returning original");
                        return Ok(sql.to_string());
                    }
                };
                let after_from = &sql_trimmed[from_pos + 4..].trim_start();
                let table_end_pos = after_from.find(table_name).map(|p| p + table_name.len()).unwrap_or(0);
                let rest_of_query = &after_from[table_end_pos..];

                let new_query = format!("SELECT {} FROM {}{}", column_list, table_name, rest_of_query);
                eprintln!("DEBUG: Rewritten query: {}", new_query);
                return Ok(new_query);
            }
        }

        eprintln!("DEBUG: No date casting applied, returning original query");
        Ok(sql.to_string())
    }

    pub async fn connect(
        host: &str,
        port: u16,
        database: &str,
        username: &str,
        password: &str,
    ) -> Result<Self, String> {
        let mut config = Config::new();
        config.host(host);
        config.port(port);
        config.database(database);
        config.authentication(AuthMethod::sql_server(username, password));
        config.trust_cert();

        let tcp = TcpStream::connect(config.get_addr())
            .await
            .map_err(|e| format!("TCP connection failed: {}", e))?;
        tcp.set_nodelay(true).ok();

        let client = Client::connect(config, tcp.compat_write())
            .await
            .map_err(|e| format!("SQL Server connection failed: {}", e))?;

        Ok(Self {
            client: Arc::new(Mutex::new(client)),
        })
    }

    pub async fn execute_query(
        &self,
        sql: &str,
        plan_type: &PlanType,
    ) -> Result<QueryResult, String> {
        let mut client = self.client.lock().await;

        // Automatically rewrite queries with date columns
        let original_sql = sql;
        let sql = Self::rewrite_query_with_date_cast(&mut client, sql).await?;
        let date_cast_applied = sql != original_sql;

        let start = std::time::Instant::now();
        let mut messages: Vec<String> = Vec::new();
        let mut plan_xml: Option<String> = None;
        let mut columns: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut rows_affected: i64 = 0;

        if date_cast_applied {
            messages.push("Note: Alias types and date columns automatically cast to their base types for compatibility.".to_string());
        }

        match plan_type {
            PlanType::Estimated => {
                // SHOWPLAN_XML returns the plan without executing
                client
                    .simple_query("SET SHOWPLAN_XML ON")
                    .await
                    .map_err(|e| format!("Failed to enable SHOWPLAN_XML: {}", e))?
                    .into_results()
                    .await
                    .map_err(|e| e.to_string())?;

                let stream = client
                    .simple_query(sql)
                    .await
                    .map_err(|e| {
                        let err_msg = e.to_string();
                        if err_msg.contains("column type") {
                            format!(
                                "Query contains unsupported column types that cannot be used with execution plans.\n\
                                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                                \nWorkarounds:\n\
                                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                                • Exclude these columns from your SELECT statement\n\
                                • Use 'No Plan' mode (though unsupported types will still cause errors)\n\
                                \nOriginal error: {}", err_msg
                            )
                        } else {
                            format!("Query failed: {}", err_msg)
                        }
                    })?;

                let result_sets = stream
                    .into_results()
                    .await
                    .map_err(|e| {
                        let err_msg = e.to_string();
                        if err_msg.contains("column type") {
                            format!(
                                "Query contains unsupported column types that cannot be used with execution plans.\n\
                                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                                \nWorkarounds:\n\
                                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                                • Exclude these columns from your SELECT statement\n\
                                • Use 'No Plan' mode (though unsupported types will still cause errors)\n\
                                \nOriginal error: {}", err_msg
                            )
                        } else {
                            err_msg
                        }
                    })?;

                // The plan XML is in the first result set, first row, first column
                for result_set in &result_sets {
                    for row in result_set {
                        if let Some(xml) = row.try_get::<&str, _>(0).ok().flatten() {
                            plan_xml = Some(xml.to_string());
                        }
                    }
                }

                client
                    .simple_query("SET SHOWPLAN_XML OFF")
                    .await
                    .map_err(|e| format!("Failed to disable SHOWPLAN_XML: {}", e))?
                    .into_results()
                    .await
                    .map_err(|e| e.to_string())?;

                messages.push("Estimated execution plan generated.".to_string());
            }
            PlanType::Actual => {
                // STATISTICS XML returns results + plan
                client
                    .simple_query("SET STATISTICS XML ON")
                    .await
                    .map_err(|e| format!("Failed to enable STATISTICS XML: {}", e))?
                    .into_results()
                    .await
                    .map_err(|e| e.to_string())?;

                let stream = client
                    .simple_query(sql)
                    .await
                    .map_err(|e| {
                        let err_msg = e.to_string();
                        if err_msg.contains("column type") {
                            format!(
                                "Query contains unsupported column types that cannot be used with execution plans.\n\
                                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                                \nWorkarounds:\n\
                                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                                • Exclude these columns from your SELECT statement\n\
                                • Use 'No Plan' mode (though unsupported types will still cause errors)\n\
                                \nOriginal error: {}", err_msg
                            )
                        } else {
                            format!("Query failed: {}", err_msg)
                        }
                    })?;

                let result_sets = stream
                    .into_results()
                    .await
                    .map_err(|e| {
                        let err_msg = e.to_string();
                        if err_msg.contains("column type") {
                            format!(
                                "Query contains unsupported column types that cannot be used with execution plans.\n\
                                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                                \nWorkarounds:\n\
                                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                                • Exclude these columns from your SELECT statement\n\
                                • Use 'No Plan' mode (though unsupported types will still cause errors)\n\
                                \nOriginal error: {}", err_msg
                            )
                        } else {
                            err_msg
                        }
                    })?;

                for result_set in &result_sets {
                    if result_set.is_empty() {
                        continue;
                    }

                    // Check each result set for the XML plan
                    let first_row = &result_set[0];
                    if let Some(xml) = first_row.try_get::<&str, _>(0).ok().flatten() {
                        if xml.contains("ShowPlanXML") {
                            plan_xml = Some(xml.to_string());
                            continue;
                        }
                    }

                    // Count rows affected but don't collect result data
                    rows_affected += result_set.len() as i64;
                }

                client
                    .simple_query("SET STATISTICS XML OFF")
                    .await
                    .map_err(|e| format!("Failed to disable STATISTICS XML: {}", e))?
                    .into_results()
                    .await
                    .map_err(|e| e.to_string())?;

                messages.push(format!(
                    "Query executed. {} row(s) returned with actual execution plan.",
                    rows_affected
                ));
            }
            PlanType::None => {
                let stream = client
                    .simple_query(sql)
                    .await
                    .map_err(|e| {
                        let err_msg = e.to_string();
                        if err_msg.contains("column type") {
                            format!(
                                "Query contains unsupported column types that are not supported by the database client.\n\
                                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                                \nWorkarounds:\n\
                                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                                • Exclude these columns from your SELECT statement\n\
                                \nOriginal error: {}", err_msg
                            )
                        } else {
                            format!("Query failed: {}", err_msg)
                        }
                    })?;

                let result_sets = stream
                    .into_results()
                    .await
                    .map_err(|e| {
                        let err_msg = e.to_string();
                        if err_msg.contains("column type") {
                            format!(
                                "Query contains unsupported column types that are not supported by the database client.\n\
                                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                                \nWorkarounds:\n\
                                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                                • Exclude these columns from your SELECT statement\n\
                                \nOriginal error: {}", err_msg
                            )
                        } else {
                            err_msg
                        }
                    })?;

                for result_set in &result_sets {
                    if result_set.is_empty() {
                        continue;
                    }

                    if columns.is_empty() {
                        columns = result_set[0]
                            .columns()
                            .iter()
                            .map(|c| c.name().to_string())
                            .collect();
                    }

                    for row in result_set {
                        let row_data = extract_row_values(row);
                        rows.push(row_data);
                        rows_affected += 1;
                    }
                }

                messages.push(format!("Query executed. {} row(s) returned.", rows_affected));
            }
        }

        let duration = start.elapsed();
        messages.push(format!("Execution time: {:.2}ms", duration.as_secs_f64() * 1000.0));

        Ok(QueryResult {
            columns,
            rows,
            messages,
            plan_xml,
            duration_ms: duration.as_millis() as u64,
            rows_affected,
        })
    }
}

fn extract_row_values(row: &Row) -> Vec<serde_json::Value> {
    let columns: &[Column] = row.columns();
    let mut values = Vec::with_capacity(columns.len());

    for i in 0..columns.len() {
        // Try each type and distinguish between NULL and type mismatch
        let val =
            // String types
            match row.try_get::<&str, _>(i) {
                Ok(Some(v)) => serde_json::Value::String(v.to_string()),
                Ok(None) => serde_json::Value::Null,
                Err(_) => {
                    // Not a string, try numeric types
                    match row.try_get::<i32, _>(i) {
                        Ok(Some(v)) => serde_json::json!(v),
                        Ok(None) => serde_json::Value::Null,
                        Err(_) => {
                            match row.try_get::<i64, _>(i) {
                                Ok(Some(v)) => serde_json::json!(v),
                                Ok(None) => serde_json::Value::Null,
                                Err(_) => {
                                    match row.try_get::<i16, _>(i) {
                                        Ok(Some(v)) => serde_json::json!(v),
                                        Ok(None) => serde_json::Value::Null,
                                        Err(_) => {
                                            match row.try_get::<f32, _>(i) {
                                                Ok(Some(v)) => serde_json::json!(v),
                                                Ok(None) => serde_json::Value::Null,
                                                Err(_) => {
                                                    match row.try_get::<f64, _>(i) {
                                                        Ok(Some(v)) => serde_json::json!(v),
                                                        Ok(None) => serde_json::Value::Null,
                                                        Err(_) => {
                                                            match row.try_get::<u8, _>(i) {
                                                                Ok(Some(v)) => serde_json::json!(v),
                                                                Ok(None) => serde_json::Value::Null,
                                                                Err(_) => {
                                                                    match row.try_get::<bool, _>(i) {
                                                                        Ok(Some(v)) => serde_json::json!(v),
                                                                        Ok(None) => serde_json::Value::Null,
                                                                        Err(_) => {
                                                                            match row.try_get::<chrono::NaiveDateTime, _>(i) {
                                                                                Ok(Some(v)) => serde_json::Value::String(v.to_string()),
                                                                                Ok(None) => serde_json::Value::Null,
                                                                                Err(_) => {
                                                                                    match row.try_get::<uuid::Uuid, _>(i) {
                                                                                        Ok(Some(v)) => serde_json::Value::String(v.to_string()),
                                                                                        Ok(None) => serde_json::Value::Null,
                                                                                        Err(_) => {
                                                                                            // For truly unsupported types (geometry, geography, etc.)
                                                                                            serde_json::Value::String(format!("[Unsupported type: {}]", columns[i].name()))
                                                                                        }
                                                                                    }
                                                                                }
                                                                            }
                                                                        }
                                                                    }
                                                                }
                                                            }
                                                        }
                                                    }
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
            };
        values.push(val);
    }

    values
}
