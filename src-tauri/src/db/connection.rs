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
        let start = std::time::Instant::now();
        let mut messages: Vec<String> = Vec::new();
        let mut plan_xml: Option<String> = None;
        let mut columns: Vec<String> = Vec::new();
        let mut rows: Vec<Vec<serde_json::Value>> = Vec::new();
        let mut rows_affected: i64 = 0;

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
                    .map_err(|e| format!("Query failed: {}", e))?;

                let result_sets = stream
                    .into_results()
                    .await
                    .map_err(|e| e.to_string())?;

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
                    .map_err(|e| format!("Query failed: {}", e))?;

                let result_sets = stream
                    .into_results()
                    .await
                    .map_err(|e| e.to_string())?;

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
                    .map_err(|e| format!("Query failed: {}", e))?;

                let result_sets = stream
                    .into_results()
                    .await
                    .map_err(|e| e.to_string())?;

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
        let val = if let Some(v) = row.try_get::<&str, _>(i).ok().flatten() {
            serde_json::Value::String(v.to_string())
        } else if let Some(v) = row.try_get::<i32, _>(i).ok().flatten() {
            serde_json::json!(v)
        } else if let Some(v) = row.try_get::<i64, _>(i).ok().flatten() {
            serde_json::json!(v)
        } else if let Some(v) = row.try_get::<i16, _>(i).ok().flatten() {
            serde_json::json!(v)
        } else if let Some(v) = row.try_get::<f32, _>(i).ok().flatten() {
            serde_json::json!(v)
        } else if let Some(v) = row.try_get::<f64, _>(i).ok().flatten() {
            serde_json::json!(v)
        } else if let Some(v) = row.try_get::<u8, _>(i).ok().flatten() {
            serde_json::json!(v)
        } else if let Some(v) = row.try_get::<bool, _>(i).ok().flatten() {
            serde_json::json!(v)
        } else if let Some(v) = row.try_get::<chrono::NaiveDateTime, _>(i).ok().flatten() {
            serde_json::Value::String(v.to_string())
        } else if let Some(v) = row.try_get::<uuid::Uuid, _>(i).ok().flatten() {
            serde_json::Value::String(v.to_string())
        } else {
            serde_json::Value::Null
        };
        values.push(val);
    }

    values
}
