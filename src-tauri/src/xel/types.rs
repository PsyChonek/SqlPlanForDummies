use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelEvent {
    pub id: u64,
    pub source_file: String,
    pub event_name: String,
    pub timestamp: DateTime<Utc>,
    pub session_id: Option<i64>,
    pub duration_us: Option<i64>,
    pub cpu_time_us: Option<i64>,
    pub logical_reads: Option<i64>,
    pub physical_reads: Option<i64>,
    pub writes: Option<i64>,
    pub result: Option<String>,
    pub statement: Option<String>,
    pub sql_text: Option<String>,
    pub object_name: Option<String>,
    pub client_app_name: Option<String>,
    pub username: Option<String>,
    pub database_name: Option<String>,
    pub resource_type: Option<String>,
    pub lock_mode: Option<String>,
    pub resource_description: Option<String>,
    pub wait_type: Option<String>,
    pub wait_duration_ms: Option<i64>,
    pub blocked_process_report: Option<String>,
    pub deadlock_graph: Option<String>,
    #[serde(default)]
    pub extra_fields: HashMap<String, serde_json::Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct XelFilter {
    #[serde(default)]
    pub event_names: Vec<String>,
    pub time_from: Option<DateTime<Utc>>,
    pub time_to: Option<DateTime<Utc>>,
    #[serde(default)]
    pub session_ids: Vec<i64>,
    pub object_name_contains: Option<String>,
    pub sql_text_contains: Option<String>,
    pub username: Option<String>,
    pub client_app_name: Option<String>,
    pub database_name: Option<String>,
    pub min_duration_us: Option<i64>,
    pub max_duration_us: Option<i64>,
    pub source_file: Option<String>,
    pub text_search: Option<String>,
    pub result: Option<String>,
    #[serde(default)]
    pub errors_only: bool,
    #[serde(default)]
    pub deadlocks_only: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelQueryRequest {
    pub filter: XelFilter,
    pub offset: usize,
    pub limit: usize,
    pub sort_by: Option<String>,
    #[serde(default)]
    pub sort_desc: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelQueryResponse {
    pub events: Vec<XelEvent>,
    pub total_count: usize,
    pub offset: usize,
    pub limit: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelSessionStats {
    pub total_events: usize,
    pub event_type_counts: HashMap<String, usize>,
    pub time_range_start: Option<DateTime<Utc>>,
    pub time_range_end: Option<DateTime<Utc>>,
    pub unique_sessions: Vec<i64>,
    pub unique_databases: Vec<String>,
    pub unique_users: Vec<String>,
    pub unique_apps: Vec<String>,
    pub files_loaded: Vec<String>,
    pub top_by_duration: Vec<XelEventSummary>,
    pub top_by_reads: Vec<XelEventSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelEventSummary {
    pub id: u64,
    pub event_name: String,
    pub timestamp: DateTime<Utc>,
    pub duration_us: Option<i64>,
    pub logical_reads: Option<i64>,
    pub statement_preview: Option<String>,
    pub session_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelLoadProgress {
    pub file_name: String,
    pub events_parsed: usize,
    pub bytes_processed: u64,
    pub total_bytes: u64,
    pub phase: LoadPhase,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum LoadPhase {
    Starting,
    CheckingPowerShell,
    Parsing,
    Indexing,
    Complete,
    Error,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PowerShellStatus {
    pub available: bool,
    pub sql_server_module: bool,
    pub dbatools_module: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelLoadRequest {
    pub file_paths: Vec<String>,
    #[serde(default)]
    pub append: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineBucket {
    pub bucket_start: DateTime<Utc>,
    pub bucket_end: DateTime<Utc>,
    pub event_count: usize,
    pub avg_duration_us: Option<f64>,
    pub max_duration_us: Option<i64>,
    pub total_logical_reads: i64,
    pub event_type_counts: HashMap<String, usize>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TimelineRequest {
    pub filter: XelFilter,
    pub bucket_count: usize,
}

/// Structured result from analyzing blocking relationships for a given event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockingAnalysis {
    /// The anchor event that was analyzed
    pub anchor_event_id: u64,
    /// Summary explanation of what happened
    pub summary: String,
    /// Parsed blocked process reports involving this event's session
    pub blocked_process_reports: Vec<ParsedBlockedProcessReport>,
    /// The blocking chain: ordered from root blocker -> intermediate -> victim
    pub blocking_chain: Vec<BlockingChainLink>,
    /// Events from the blocking session(s) around the same time
    pub blocker_events: Vec<XelEvent>,
    /// Lock escalation events that may explain why a broad lock exists
    pub lock_escalations: Vec<XelEvent>,
    /// Wait statistics for the victim session
    pub wait_events: Vec<XelEvent>,
    /// Aggregated wait stats by wait type
    pub wait_stats: Vec<WaitTypeStat>,
    /// Parsed deadlock graphs involving this event's session
    pub deadlocks: Vec<ParsedDeadlockGraph>,
    /// Deadlock ID from lock events (confirms deadlock without xml_deadlock_report)
    pub deadlock_id: Option<i64>,
    /// Lock events that share the same deadlock_id (deadlock participants)
    pub deadlock_lock_events: Vec<XelEvent>,
    /// Categorized diagnosis: "lock_blocking", "io_starvation", "memory_pressure", "unknown"
    pub diagnosis: String,
    /// Actionable recommendations
    pub recommendations: Vec<String>,
}

/// Parsed deadlock graph showing all processes and resources involved
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedDeadlockGraph {
    pub event_id: u64,
    pub timestamp: DateTime<Utc>,
    pub processes: Vec<DeadlockProcess>,
    pub resources: Vec<DeadlockResource>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlockProcess {
    pub id: String,        // e.g. "process28abc123"
    pub spid: Option<i64>,
    pub is_victim: bool,
    pub xact_id: Option<String>,   // transaction id from deadlock XML (xactid attr)
    pub lock_mode: Option<String>,
    pub wait_resource: Option<String>,
    pub wait_time_ms: Option<i64>,
    pub transaction_name: Option<String>,
    pub log_used: Option<i64>,
    pub input_buffer: Option<String>,
    pub database_name: Option<String>,
    pub hostname: Option<String>,
    pub app_name: Option<String>,
    pub login_name: Option<String>,
    pub isolation_level: Option<String>,
    pub status: Option<String>,
    pub tran_count: Option<i64>,
    pub last_batch_started: Option<String>,
    pub last_batch_completed: Option<String>,
    pub ecid: Option<i64>,
    pub execution_stack: Vec<DeadlockExecutionFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlockResource {
    pub resource_type: String,  // "keylock", "pagelock", "objectlock", "xactlock", etc.
    pub database_name: Option<String>,
    pub object_name: Option<String>,
    pub index_name: Option<String>,
    pub mode: Option<String>,
    pub hobt_id: Option<String>,
    pub file_id: Option<String>,
    pub page_id: Option<String>,
    /// Processes holding this resource
    pub holders: Vec<DeadlockResourceOwner>,
    /// Processes waiting on this resource
    pub waiters: Vec<DeadlockResourceOwner>,
}

/// Execution stack frame from a deadlock process
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlockExecutionFrame {
    pub proc_name: Option<String>,
    pub query_hash: Option<String>,
    pub query_plan_hash: Option<String>,
    pub line: Option<i64>,
    pub sql_handle: Option<String>,
    /// SQL text from frame body
    pub sql_text: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DeadlockResourceOwner {
    pub process_id: String,
    pub mode: Option<String>,
}

/// Problem-centric statistics for the dashboard
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelProblemStats {
    pub deadlock_count: usize,
    pub error_count: usize,
    pub blocked_process_count: usize,
    pub lock_wait_count: usize,
    /// Top wait types aggregated across all sessions
    pub top_wait_types: Vec<WaitTypeStat>,
    /// Sessions with the most errors
    pub error_sessions: Vec<SessionProblemStat>,
    /// Sessions with the most waits
    pub wait_sessions: Vec<SessionProblemStat>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SessionProblemStat {
    pub session_id: i64,
    pub count: usize,
    pub total_duration_us: i64,
    pub sample_event_name: String,
    pub sample_object_name: Option<String>,
}

/// Result of enriching XEL data from a connected database
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct XelEnrichResult {
    pub databases_resolved: usize,
    pub objects_resolved: usize,
    pub query_texts_resolved: usize,
    /// How many unique values were looked up from DB
    pub unique_databases: usize,
    pub unique_objects: usize,
    pub unique_queries: usize,
    pub errors: Vec<String>,
}

/// Database-level settings fetched during enrichment
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DbSettings {
    pub is_read_committed_snapshot_on: bool,
    pub snapshot_isolation_on: bool,
}

/// Object found in the same session/transaction as an XACT or unresolvable lock event
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TransactionObject {
    pub object_name: String,
    pub resource_type: Option<String>,
    pub lock_modes: Vec<String>,
    pub event_count: usize,
    pub sample_event_id: u64,
}

/// Aggregated wait statistics per wait type
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WaitTypeStat {
    pub wait_type: String,
    pub count: usize,
    pub total_duration_us: i64,
    pub max_duration_us: i64,
    pub avg_duration_us: i64,
    pub category: String, // "io", "lock", "latch", "network", "cpu", "memory", "other"
}

/// Parsed content from a blocked_process_report XML
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ParsedBlockedProcessReport {
    /// Event ID of the BPR event
    pub event_id: u64,
    pub timestamp: DateTime<Utc>,
    /// Blocked (victim) session info
    pub blocked_spid: Option<i64>,
    pub blocked_xact_id: Option<String>,
    pub blocked_wait_resource: Option<String>,
    pub blocked_wait_time_ms: Option<i64>,
    pub blocked_lock_mode: Option<String>,
    pub blocked_input_buffer: Option<String>,
    pub blocked_database: Option<String>,
    pub blocked_hostname: Option<String>,
    pub blocked_app_name: Option<String>,
    pub blocked_login_name: Option<String>,
    pub blocked_status: Option<String>,
    pub blocked_last_batch_started: Option<String>,
    /// Blocking (holder) session info
    pub blocking_spid: Option<i64>,
    pub blocking_xact_id: Option<String>,
    pub blocking_input_buffer: Option<String>,
    pub blocking_database: Option<String>,
    pub blocking_hostname: Option<String>,
    pub blocking_app_name: Option<String>,
    pub blocking_login_name: Option<String>,
    pub blocking_status: Option<String>,
    pub blocking_last_batch_started: Option<String>,
    /// Additional context
    pub blocked_isolation_level: Option<String>,
    pub blocked_tran_count: Option<i64>,
    pub blocking_isolation_level: Option<String>,
    pub blocking_tran_count: Option<i64>,
    /// Execution stack frames (query hash + plan hash)
    #[serde(default)]
    pub blocked_execution_stack: Vec<ExecutionFrame>,
    #[serde(default)]
    pub blocking_execution_stack: Vec<ExecutionFrame>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExecutionFrame {
    pub query_hash: Option<String>,
    pub query_plan_hash: Option<String>,
    pub line: Option<i64>,
    pub sql_handle: Option<String>,
}

/// One link in a blocking chain
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BlockingChainLink {
    pub session_id: i64,
    pub role: String, // "root_blocker", "intermediate", "victim"
    pub wait_resource: Option<String>,
    pub lock_mode: Option<String>,
    pub sql_preview: Option<String>,
    pub app_name: Option<String>,
    pub username: Option<String>,
    pub database: Option<String>,
    /// Event IDs from this session relevant to the chain
    pub event_ids: Vec<u64>,
    /// Who this session is blocked by (if any)
    pub blocked_by_session: Option<i64>,
    /// Additional context from BPR
    pub hostname: Option<String>,
    pub status: Option<String>,
    pub isolation_level: Option<String>,
    pub tran_count: Option<i64>,
    pub last_batch_started: Option<String>,
    pub wait_time_ms: Option<i64>,
    /// Transaction ID (xactid from BPR XML) for filtering related events
    pub xact_id: Option<String>,
    #[serde(default)]
    pub execution_stack: Vec<ExecutionFrame>,
}
