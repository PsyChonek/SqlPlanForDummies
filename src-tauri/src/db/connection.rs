use std::sync::Arc;
use tiberius::{AuthMethod, Client, Column, Config, Row};
use tokio::net::TcpStream;
use tokio::sync::{Mutex, Notify};
use tokio_util::compat::TokioAsyncWriteCompatExt;

use super::types::{PlanType, QueryResult};

type TiberiusClient = Client<tokio_util::compat::Compat<TcpStream>>;

pub struct DbConnection {
    pub client: Arc<Mutex<TiberiusClient>>,
}

pub struct AppState {
    pub connection: Arc<Mutex<Option<DbConnection>>>,
    pub cancel: Arc<Notify>,
}

impl DbConnection {
    // Helper function to rewrite queries with date columns cast to datetime
    async fn rewrite_query_with_date_cast(
        client: &mut TiberiusClient,
        sql: &str,
    ) -> Result<String, String> {
        // Only rewrite when the select list is exactly a lone star:
        // SELECT [DISTINCT|ALL] [TOP n [PERCENT]] * FROM table ...
        // Anything else (explicit columns, COUNT(*), aliased stars like t.*)
        // is left untouched so multi-statement batches are never mangled.
        let sql_lower = sql.to_ascii_lowercase();

        let select_pos = match sql_lower.find("select") {
            Some(pos) => pos,
            None => return Ok(sql.to_string()),
        };
        let from_pos = match sql_lower[select_pos..].find("from") {
            Some(pos) => select_pos + pos,
            None => return Ok(sql.to_string()),
        };

        let select_list = &sql[select_pos + "select".len()..from_pos];
        let mut tokens: Vec<&str> = select_list.split_whitespace().collect();
        if tokens
            .first()
            .map(|t| t.eq_ignore_ascii_case("distinct") || t.eq_ignore_ascii_case("all"))
            .unwrap_or(false)
        {
            tokens.remove(0);
        }
        if tokens
            .first()
            .map(|t| t.eq_ignore_ascii_case("top"))
            .unwrap_or(false)
        {
            tokens.remove(0);
            if !tokens.is_empty() {
                tokens.remove(0); // the row count
            }
            if tokens
                .first()
                .map(|t| t.eq_ignore_ascii_case("percent"))
                .unwrap_or(false)
            {
                tokens.remove(0);
            }
        }
        if tokens.len() == 1 && tokens[0] == "*" {
            // Get just the table name (before any WHERE, ORDER BY, etc.)
            let table_name = sql[from_pos + "from".len()..]
                .split_whitespace()
                .next()
                .unwrap_or("")
                .trim_end_matches(';');

            if table_name.is_empty() {
                return Ok(sql.to_string());
            }

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

            let mut columns: Vec<String> = Vec::new();
            let mut has_type_casting = false;

            for result_set in &result_sets {
                for row in result_set {
                    let col_name_result = row.try_get::<&str, _>(0);
                    let system_type_id_result = row.try_get::<u8, _>(1);
                    let user_type_id_result = row.try_get::<i32, _>(2);
                    let system_type_name_result = row.try_get::<&str, _>(4);

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

            if has_type_casting && !columns.is_empty() {
                // Replace only the star with the explicit column list so the rest
                // of the batch (BEGIN TRAN, TOP, WHERE, following statements) is
                // preserved exactly as written.
                let star_pos = match sql[select_pos..from_pos].rfind('*') {
                    Some(pos) => select_pos + pos,
                    None => return Ok(sql.to_string()),
                };
                let column_list = columns.join(", ");
                return Ok(format!(
                    "{}{}{}",
                    &sql[..star_pos],
                    column_list,
                    &sql[star_pos + 1..]
                ));
            }
        }

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

                let query_result = match client.simple_query(sql).await {
                    Ok(stream) => stream
                        .into_results()
                        .await
                        .map_err(|e| format_query_error(e.to_string(), true, false)),
                    Err(e) => Err(format_query_error(e.to_string(), true, true)),
                };

                // Always restore the session state, even when the query failed,
                // otherwise later queries on this connection return plans instead of rows
                if let Ok(off) = client.simple_query("SET SHOWPLAN_XML OFF").await {
                    off.into_results().await.ok();
                }

                let result_sets = query_result?;

                let mut plan_xmls: Vec<String> = Vec::new();
                for result_set in &result_sets {
                    for row in result_set {
                        if let Some(xml) = row.try_get::<&str, _>(0).ok().flatten() {
                            plan_xmls.push(xml.to_string());
                        }
                    }
                }
                plan_xml = merge_showplan_xmls(plan_xmls);

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

                let query_result = match client.simple_query(sql).await {
                    Ok(stream) => stream
                        .into_results()
                        .await
                        .map_err(|e| format_query_error(e.to_string(), true, false)),
                    Err(e) => Err(format_query_error(e.to_string(), true, true)),
                };

                // Always restore the session state, even when the query failed
                if let Ok(off) = client.simple_query("SET STATISTICS XML OFF").await {
                    off.into_results().await.ok();
                }

                let result_sets = query_result?;

                let mut plan_xmls: Vec<String> = Vec::new();
                for result_set in &result_sets {
                    if result_set.is_empty() {
                        continue;
                    }

                    let first_row = &result_set[0];
                    if let Some(xml) = first_row.try_get::<&str, _>(0).ok().flatten() {
                        if xml.contains("ShowPlanXML") {
                            plan_xmls.push(xml.to_string());
                            continue;
                        }
                    }

                    rows_affected += result_set.len() as i64;
                }
                plan_xml = merge_showplan_xmls(plan_xmls);

                messages.push(format!(
                    "Query executed. {} row(s) returned with actual execution plan.",
                    rows_affected
                ));
            }
            PlanType::None => {
                let result_sets = match client.simple_query(sql).await {
                    Ok(stream) => stream
                        .into_results()
                        .await
                        .map_err(|e| format_query_error(e.to_string(), false, false)),
                    Err(e) => Err(format_query_error(e.to_string(), false, true)),
                }?;

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

fn format_query_error(err_msg: String, plan_mode: bool, wrap_query_failed: bool) -> String {
    if err_msg.contains("column type") {
        if plan_mode {
            format!(
                "Query contains unsupported column types that cannot be used with execution plans.\n\
                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                \nWorkarounds:\n\
                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                • Exclude these columns from your SELECT statement\n\
                • Use 'No Plan' mode (though unsupported types will still cause errors)\n\
                \nOriginal error: {}",
                err_msg
            )
        } else {
            format!(
                "Query contains unsupported column types that are not supported by the database client.\n\
                Unsupported types include: date, geometry, geography, hierarchyid, and certain CLR types.\n\
                \nWorkarounds:\n\
                • Cast date columns to datetime: SELECT CAST(LicenseValidTo AS datetime) AS LicenseValidTo\n\
                • Exclude these columns from your SELECT statement\n\
                \nOriginal error: {}",
                err_msg
            )
        }
    } else if wrap_query_failed {
        format!("Query failed: {}", err_msg)
    } else {
        err_msg
    }
}

fn merge_showplan_xmls(xmls: Vec<String>) -> Option<String> {
    if xmls.is_empty() {
        return None;
    }
    if xmls.len() == 1 {
        return xmls.into_iter().next();
    }

    let mut iter = xmls.into_iter();
    let mut base = iter.next().unwrap();

    for xml in iter {
        let open_tag = "<Statements>";
        let close_tag = "</Statements>";
        if let Some(open_end) = xml.find(open_tag).map(|i| i + open_tag.len()) {
            if let Some(close_start) = xml.rfind(close_tag) {
                let inner = xml[open_end..close_start].to_string();
                if let Some(insert_pos) = base.rfind(close_tag) {
                    base.insert_str(insert_pos, &inner);
                }
            }
        }
    }

    Some(base)
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
