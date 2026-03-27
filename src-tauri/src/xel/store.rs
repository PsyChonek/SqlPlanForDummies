use std::collections::{HashMap, HashSet};
use std::sync::Arc;
use tokio::sync::RwLock;

use super::filter::matches_filter;
use super::types::*;

use quick_xml::events::Event as XmlEvent;
use quick_xml::reader::Reader;

pub struct XelStore {
    events: Vec<XelEvent>,
    next_id: u64,
    by_event_name: HashMap<String, Vec<usize>>,
    by_session: HashMap<i64, Vec<usize>>,
    by_file: HashMap<String, Vec<usize>>,
    by_transaction: HashMap<String, Vec<usize>>,
    by_resource: HashMap<String, Vec<usize>>,
    by_activity: HashMap<String, Vec<usize>>, // activity_id GUID (without sequence) → events
    by_deadlock: HashMap<String, Vec<usize>>, // deadlock_id → events
    all_columns: Vec<String>,
    db_settings: HashMap<String, DbSettings>, // database_name → settings from enrich
}

pub struct XelAppState {
    pub store: Arc<RwLock<XelStore>>,
}

impl XelStore {
    pub fn new() -> Self {
        Self {
            events: Vec::new(),
            next_id: 1,
            by_event_name: HashMap::new(),
            by_session: HashMap::new(),
            by_file: HashMap::new(),
            by_transaction: HashMap::new(),
            by_resource: HashMap::new(),
            by_activity: HashMap::new(),
            by_deadlock: HashMap::new(),
            all_columns: Vec::new(),
            db_settings: HashMap::new(),
        }
    }

    pub fn insert_batch(&mut self, mut events: Vec<XelEvent>) {
        // Track all column names from extra_fields
        let mut col_set: HashSet<String> = self.all_columns.iter().cloned().collect();
        for event in &events {
            for key in event.extra_fields.keys() {
                col_set.insert(key.clone());
            }
        }
        self.all_columns = col_set.into_iter().collect();
        self.all_columns.sort();

        for event in &mut events {
            event.id = self.next_id;
            self.next_id += 1;
        }

        self.events.extend(events);

        // Re-sort by timestamp
        self.events.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));

        // Rebuild indexes
        self.rebuild_indexes();
    }

    pub fn get_columns(&self) -> Vec<String> {
        // Fixed columns first, then dynamic extras
        let mut cols = vec![
            "id".into(),
            "timestamp".into(),
            "eventName".into(),
            "sessionId".into(),
            "durationUs".into(),
            "cpuTimeUs".into(),
            "logicalReads".into(),
            "physicalReads".into(),
            "writes".into(),
            "result".into(),
            "objectName".into(),
            "statement".into(),
            "sqlText".into(),
            "resourceType".into(),
            "lockMode".into(),
            "resourceDescription".into(),
            "waitType".into(),
            "waitDurationMs".into(),
            "username".into(),
            "clientAppName".into(),
            "databaseName".into(),
            "sourceFile".into(),
        ];
        // Add dynamic extra columns
        for col in &self.all_columns {
            if !cols.contains(col) {
                cols.push(col.clone());
            }
        }
        cols
    }

    fn rebuild_indexes(&mut self) {
        self.by_event_name.clear();
        self.by_session.clear();
        self.by_file.clear();
        self.by_transaction.clear();
        self.by_resource.clear();
        self.by_activity.clear();
        self.by_deadlock.clear();

        for (idx, event) in self.events.iter().enumerate() {
            self.by_event_name
                .entry(event.event_name.clone())
                .or_default()
                .push(idx);

            if let Some(sid) = event.session_id {
                self.by_session.entry(sid).or_default().push(idx);
            }

            self.by_file
                .entry(event.source_file.clone())
                .or_default()
                .push(idx);

            // Index by transaction_id (handles both Number and String)
            if let Some(v) = event.extra_fields.get("transaction_id") {
                let tid_str = match v {
                    serde_json::Value::Number(n) => n.as_i64().map(|n| n.to_string()),
                    serde_json::Value::String(s) if !s.is_empty() && s != "0" => Some(s.clone()),
                    _ => None,
                };
                if let Some(tid) = tid_str {
                    self.by_transaction.entry(tid).or_default().push(idx);
                }
            }

            // Index by resource (associated_object_id — handles both Number and String)
            if let Some(v) = event.extra_fields.get("associated_object_id") {
                if let Some(oid) = Self::json_value_as_i64(v) {
                    if oid != 0 {
                        self.by_resource.entry(oid.to_string()).or_default().push(idx);
                    }
                }
            }

            // Index by activity_id GUID (strip sequence number after ':')
            // Format: "cd50265e-218c-425e-a952-e1d8f7812300:8" → key is GUID part
            if let Some(v) = event.extra_fields.get("attach_activity_id") {
                if let Some(s) = v.as_str() {
                    if let Some(guid) = s.split(':').next() {
                        if guid.len() >= 36 {
                            self.by_activity.entry(guid.to_string()).or_default().push(idx);
                        }
                    }
                }
            }

            // Also index by attach_activity_id_xfer — the transferred activity ID
            // links child events (waits) to their parent request even when each
            // child has its own unique attach_activity_id
            if let Some(v) = event.extra_fields.get("attach_activity_id_xfer") {
                if let Some(s) = v.as_str() {
                    if let Some(guid) = s.split(':').next() {
                        if guid.len() >= 36 {
                            self.by_activity.entry(guid.to_string()).or_default().push(idx);
                        }
                    }
                }
            }

            // Index by deadlock_id (from lock_deadlock events) and
            // deadlock_cycle_id (from database_xml_deadlock_report events)
            for key in &["deadlock_id", "deadlock_cycle_id"] {
                if let Some(v) = event.extra_fields.get(*key) {
                    if let Some(did) = Self::json_value_as_i64(v) {
                        if did != 0 {
                            self.by_deadlock.entry(did.to_string()).or_default().push(idx);
                        }
                    }
                }
            }
        }
    }

    /// Find events related to a given event by multiple correlation strategies
    pub fn get_related_events(
        &self,
        event_id: u64,
        time_window_ms: i64,
        limit: usize,
    ) -> Vec<XelEvent> {
        let anchor_idx = match self.events.iter().position(|e| e.id == event_id) {
            Some(idx) => idx,
            None => return Vec::new(),
        };
        let anchor = &self.events[anchor_idx];

        // Compute the search range: [anchor_start - window, anchor_end + window]
        let anchor_dur_ms = anchor.duration_us.unwrap_or(0) / 1000;
        let range_start = anchor.timestamp
            - chrono::Duration::milliseconds(time_window_ms);
        let range_end = anchor.timestamp
            + chrono::Duration::milliseconds(anchor_dur_ms + time_window_ms);

        let in_range = |idx: usize| -> bool {
            let ts = self.events[idx].timestamp;
            ts >= range_start && ts <= range_end
        };

        let mut related_indices: HashSet<usize> = HashSet::new();

        // Always include the anchor event itself
        related_indices.insert(anchor_idx);

        // 1. Same session — all events from same session in time window
        if let Some(sid) = anchor.session_id {
            if let Some(indices) = self.by_session.get(&sid) {
                for &idx in indices {
                    if in_range(idx) {
                        related_indices.insert(idx);
                    }
                }
            }
        }

        // 2. Same activity_id GUID — events in the same request within time window
        //    Check both attach_activity_id and attach_activity_id_xfer since
        //    child events (waits) may each have unique activity IDs but share _xfer
        for field in &["attach_activity_id", "attach_activity_id_xfer"] {
            if let Some(v) = anchor.extra_fields.get(*field) {
                if let Some(s) = v.as_str() {
                    if let Some(guid) = s.split(':').next() {
                        if guid.len() >= 36 {
                            if let Some(indices) = self.by_activity.get(guid) {
                                for &idx in indices {
                                    if in_range(idx) {
                                        related_indices.insert(idx);
                                    }
                                }
                            }
                        }
                    }
                }
            }
        }

        // 3. Same transaction_id — events in the same transaction within time window
        if let Some(v) = anchor.extra_fields.get("transaction_id") {
            let tid_str = match v {
                serde_json::Value::Number(n) => n.as_i64().filter(|&n| n != 0).map(|n| n.to_string()),
                serde_json::Value::String(s) if !s.is_empty() && s != "0" => Some(s.clone()),
                _ => None,
            };
            if let Some(tid) = tid_str {
                if let Some(indices) = self.by_transaction.get(&tid) {
                    for &idx in indices {
                        if in_range(idx) {
                            related_indices.insert(idx);
                        }
                    }
                }
            }
        }

        // 4. Same resource — events competing for the same lock object
        if let Some(v) = anchor.extra_fields.get("associated_object_id") {
            if let Some(oid) = Self::json_value_as_i64(v) {
                if oid != 0 {
                    if let Some(indices) = self.by_resource.get(&oid.to_string()) {
                        for &idx in indices {
                            if in_range(idx) {
                                related_indices.insert(idx);
                            }
                        }
                    }
                }
            }
        }

        // Collect, sort, limit
        let mut result: Vec<XelEvent> = related_indices
            .into_iter()
            .map(|idx| self.events[idx].clone())
            .collect();
        result.sort_by(|a, b| a.timestamp.cmp(&b.timestamp));
        result.truncate(limit);
        result
    }

    pub fn query(
        &self,
        filter: &XelFilter,
        offset: usize,
        limit: usize,
        sort_by: Option<&str>,
        sort_desc: bool,
    ) -> XelQueryResponse {
        // Collect candidate indices
        let candidates = self.get_candidate_indices(filter);

        // Apply full filter
        let mut filtered: Vec<usize> = candidates
            .into_iter()
            .filter(|&idx| matches_filter(&self.events[idx], filter))
            .collect();

        // Sort
        if let Some(field) = sort_by {
            self.sort_indices(&mut filtered, field, sort_desc);
        }

        let total_count = filtered.len();

        // Paginate
        let page: Vec<XelEvent> = filtered
            .into_iter()
            .skip(offset)
            .take(limit)
            .map(|idx| self.events[idx].clone())
            .collect();

        XelQueryResponse {
            events: page,
            total_count,
            offset,
            limit,
        }
    }

    fn get_candidate_indices(&self, filter: &XelFilter) -> Vec<usize> {
        // Use indexes when possible for faster lookup
        if filter.event_names.len() == 1 {
            if let Some(indices) = self.by_event_name.get(&filter.event_names[0]) {
                return indices.clone();
            }
        }
        if filter.session_ids.len() == 1 {
            if let Some(indices) = self.by_session.get(&filter.session_ids[0]) {
                return indices.clone();
            }
        }
        if let Some(ref sf) = filter.source_file {
            if let Some(indices) = self.by_file.get(sf) {
                return indices.clone();
            }
        }
        // Full scan
        (0..self.events.len()).collect()
    }

    fn sort_indices(&self, indices: &mut [usize], field: &str, desc: bool) {
        indices.sort_by(|&a, &b| {
            let ea = &self.events[a];
            let eb = &self.events[b];
            let ord = match field {
                "timestamp" => ea.timestamp.cmp(&eb.timestamp),
                "eventName" => ea.event_name.cmp(&eb.event_name),
                "sessionId" => ea.session_id.cmp(&eb.session_id),
                "durationUs" => ea.duration_us.cmp(&eb.duration_us),
                "cpuTimeUs" => ea.cpu_time_us.cmp(&eb.cpu_time_us),
                "logicalReads" => ea.logical_reads.cmp(&eb.logical_reads),
                "objectName" => ea.object_name.cmp(&eb.object_name),
                "result" => ea.result.cmp(&eb.result),
                "resourceType" => ea.resource_type.cmp(&eb.resource_type),
                "lockMode" => ea.lock_mode.cmp(&eb.lock_mode),
                "waitType" => ea.wait_type.cmp(&eb.wait_type),
                "username" => ea.username.cmp(&eb.username),
                "clientAppName" => ea.client_app_name.cmp(&eb.client_app_name),
                _ => ea.timestamp.cmp(&eb.timestamp),
            };
            if desc {
                ord.reverse()
            } else {
                ord
            }
        });
    }

    pub fn get_event(&self, id: u64) -> Option<&XelEvent> {
        self.events.iter().find(|e| e.id == id)
    }

    pub fn get_stats(&self, filter: &XelFilter) -> XelSessionStats {
        let mut event_type_counts: HashMap<String, usize> = HashMap::new();
        let mut sessions: HashSet<i64> = HashSet::new();
        let mut databases: HashSet<String> = HashSet::new();
        let mut users: HashSet<String> = HashSet::new();
        let mut apps: HashSet<String> = HashSet::new();
        let mut files: HashSet<String> = HashSet::new();

        for event in self.events.iter().filter(|e| matches_filter(e, filter)) {
            *event_type_counts.entry(event.event_name.clone()).or_default() += 1;
            if let Some(sid) = event.session_id {
                sessions.insert(sid);
            }
            if let Some(ref db) = event.database_name {
                databases.insert(db.clone());
            }
            if let Some(ref u) = event.username {
                users.insert(u.clone());
            }
            if let Some(ref a) = event.client_app_name {
                apps.insert(a.clone());
            }
            files.insert(event.source_file.clone());
        }

        let filtered: Vec<&XelEvent> = self.events.iter().filter(|e| matches_filter(e, filter)).collect();

        let time_range_start = filtered.first().map(|e| e.timestamp);
        let time_range_end = filtered.last().map(|e| e.timestamp);

        // Top by duration
        let mut by_dur: Vec<&XelEvent> = filtered.iter()
            .filter(|e| e.duration_us.is_some())
            .copied()
            .collect();
        by_dur.sort_by(|a, b| b.duration_us.cmp(&a.duration_us));
        let top_by_duration: Vec<XelEventSummary> = by_dur
            .iter()
            .take(20)
            .map(|e| self.event_to_summary(e))
            .collect();

        // Top by reads
        let mut by_reads: Vec<&XelEvent> = filtered.iter()
            .filter(|e| e.logical_reads.is_some())
            .copied()
            .collect();
        by_reads.sort_by(|a, b| b.logical_reads.cmp(&a.logical_reads));
        let top_by_reads: Vec<XelEventSummary> = by_reads
            .iter()
            .take(20)
            .map(|e| self.event_to_summary(e))
            .collect();

        let mut unique_sessions: Vec<i64> = sessions.into_iter().collect();
        unique_sessions.sort();

        XelSessionStats {
            total_events: filtered.len(),
            event_type_counts,
            time_range_start,
            time_range_end,
            unique_sessions,
            unique_databases: databases.into_iter().collect(),
            unique_users: users.into_iter().collect(),
            unique_apps: apps.into_iter().collect(),
            files_loaded: files.into_iter().collect(),
            top_by_duration,
            top_by_reads,
        }
    }

    fn event_to_summary(&self, event: &XelEvent) -> XelEventSummary {
        let preview = event
            .statement
            .as_ref()
            .or(event.sql_text.as_ref())
            .or(event.object_name.as_ref())
            .map(|s| {
                if s.chars().count() > 120 {
                    let truncated: String = s.chars().take(120).collect();
                    format!("{}...", truncated)
                } else {
                    s.clone()
                }
            });

        XelEventSummary {
            id: event.id,
            event_name: event.event_name.clone(),
            timestamp: event.timestamp,
            duration_us: event.duration_us,
            logical_reads: event.logical_reads,
            statement_preview: preview,
            session_id: event.session_id,
        }
    }

    pub fn get_timeline(
        &self,
        filter: &XelFilter,
        bucket_count: usize,
    ) -> Vec<TimelineBucket> {
        let filtered: Vec<&XelEvent> = self
            .events
            .iter()
            .filter(|e| matches_filter(e, filter))
            .collect();

        if filtered.is_empty() || bucket_count == 0 {
            return Vec::new();
        }

        let start = filtered.first().unwrap().timestamp;
        let end = filtered.last().unwrap().timestamp;
        let total_duration = (end - start).num_milliseconds().max(1) as f64;
        let bucket_ms = total_duration / bucket_count as f64;

        let mut buckets: Vec<TimelineBucket> = (0..bucket_count)
            .map(|i| {
                let b_start =
                    start + chrono::Duration::milliseconds((i as f64 * bucket_ms) as i64);
                let b_end = start
                    + chrono::Duration::milliseconds(((i + 1) as f64 * bucket_ms) as i64);
                TimelineBucket {
                    bucket_start: b_start,
                    bucket_end: b_end,
                    event_count: 0,
                    avg_duration_us: None,
                    max_duration_us: None,
                    total_logical_reads: 0,
                    event_type_counts: HashMap::new(),
                }
            })
            .collect();

        for event in &filtered {
            let offset_ms = (event.timestamp - start).num_milliseconds() as f64;
            let bucket_idx = ((offset_ms / bucket_ms) as usize).min(bucket_count - 1);
            let bucket = &mut buckets[bucket_idx];

            bucket.event_count += 1;
            *bucket
                .event_type_counts
                .entry(event.event_name.clone())
                .or_default() += 1;

            if let Some(reads) = event.logical_reads {
                bucket.total_logical_reads += reads;
            }

            if let Some(dur) = event.duration_us {
                bucket.max_duration_us = Some(
                    bucket.max_duration_us.map_or(dur, |m: i64| m.max(dur)),
                );
            }
        }

        // Calculate averages
        for bucket in &mut buckets {
            if bucket.event_count > 0 {
                // avg_duration is calculated from events in this bucket
                // We need a second pass or accumulate sum; let's use a simpler approach
                // For now, set avg to max (we can refine later with sum tracking)
            }
        }

        buckets
    }

    pub fn get_distinct_values(&self, field: &str) -> Vec<String> {
        let mut values: HashSet<String> = HashSet::new();

        for event in &self.events {
            let val = match field {
                "eventName" => Some(event.event_name.clone()),
                "sessionId" => event.session_id.map(|s| s.to_string()),
                "objectName" => event.object_name.clone(),
                "result" => event.result.clone(),
                "resourceType" => event.resource_type.clone(),
                "lockMode" => event.lock_mode.clone(),
                "waitType" => event.wait_type.clone(),
                "username" => event.username.clone(),
                "clientAppName" => event.client_app_name.clone(),
                "databaseName" => event.database_name.clone(),
                "sourceFile" => Some(event.source_file.clone()),
                _ => None,
            };
            if let Some(v) = val {
                values.insert(v);
            }
        }

        let mut result: Vec<String> = values.into_iter().collect();
        result.sort();
        result
    }

    pub fn clear(&mut self) {
        self.events.clear();
        self.next_id = 1;
        self.by_event_name.clear();
        self.by_session.clear();
        self.by_file.clear();
        self.by_transaction.clear();
        self.by_resource.clear();
        self.by_activity.clear();
        self.all_columns.clear();
        self.db_settings.clear();
    }

    pub fn set_db_settings(&mut self, settings: HashMap<String, DbSettings>) {
        self.db_settings = settings;
    }

    /// Check if a database has RCSI or snapshot isolation already enabled
    pub fn has_snapshot_isolation(&self, db_name: Option<&str>) -> bool {
        if let Some(name) = db_name {
            if let Some(settings) = self.db_settings.get(name) {
                return settings.is_read_committed_snapshot_on || settings.snapshot_isolation_on;
            }
        }
        false
    }
    /// Helper: extract i64 from a serde_json::Value that may be Number or String
    fn json_value_as_i64(v: &serde_json::Value) -> Option<i64> {
        match v {
            serde_json::Value::Number(n) => n.as_i64(),
            serde_json::Value::String(s) => s.parse::<i64>().ok(),
            _ => None,
        }
    }

    /// Parse wait_resource strings like "KEY: 6:72057625259081728 (hash)" to extract hobt_id
    fn parse_wait_resource_hobt_id(wait_resource: &str) -> Option<i64> {
        let trimmed = wait_resource.trim();
        if let Some(rest) = trimmed.strip_prefix("KEY:") {
            let rest = rest.trim();
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() >= 2 {
                let hobt_str = parts[1].split_whitespace().next().unwrap_or("");
                return hobt_str.parse::<i64>().ok();
            }
        }
        None
    }

    /// Parse wait_resource "OBJECT: db_id:object_id:partition" to extract object_id
    fn parse_wait_resource_object_id(wait_resource: &str) -> Option<i64> {
        let trimmed = wait_resource.trim();
        if let Some(rest) = trimmed.strip_prefix("OBJECT:") {
            let rest = rest.trim();
            let parts: Vec<&str> = rest.splitn(4, ':').collect();
            if parts.len() >= 2 {
                return parts[1].trim().parse::<i64>().ok();
            }
        }
        if let Some(rest) = trimmed.strip_prefix("ALLOCATION_UNIT:") {
            let rest = rest.trim();
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() >= 2 {
                return parts[1].trim().parse::<i64>().ok();
            }
        }
        None
    }

    /// Extract Object Id values from inputbuf strings like "Proc [Database Id = 6 Object Id = 123]"
    fn extract_object_ids_from_inputbufs(xml: &str, ids: &mut HashSet<i64>) {
        for cap_start in xml.match_indices("Object Id = ").map(|(i, _)| i) {
            let rest = &xml[cap_start + 12..];
            if let Some(end) = rest.find(']') {
                if let Ok(id) = rest[..end].trim().parse::<i64>() {
                    if id > 0 { ids.insert(id); }
                }
            }
        }
    }

    /// Collect unique database_ids from extra_fields (handles both Number and String)
    pub fn collect_database_ids(&self) -> Vec<i64> {
        let mut ids: HashSet<i64> = HashSet::new();
        for event in &self.events {
            if let Some(v) = event.extra_fields.get("database_id") {
                if let Some(id) = Self::json_value_as_i64(v) {
                    if id > 0 { ids.insert(id); }
                }
            }
        }
        ids.into_iter().collect()
    }

    /// Collect unique hobt_ids from associated_object_id AND wait_resource strings
    pub fn collect_object_ids(&self) -> Vec<i64> {
        let mut ids: HashSet<i64> = HashSet::new();
        for event in &self.events {
            // From associated_object_id field
            if let Some(v) = event.extra_fields.get("associated_object_id") {
                if let Some(id) = Self::json_value_as_i64(v) {
                    if id > 0 { ids.insert(id); }
                }
            }
            // From object_id field (lock events with resource_type=OBJECT)
            if let Some(v) = event.extra_fields.get("object_id") {
                if let Some(id) = Self::json_value_as_i64(v) {
                    if id > 0 { ids.insert(id); }
                }
            }
            // From wait_resource string (e.g., "KEY: 6:72057625259081728 (hash)")
            if let Some(ref wr) = event.extra_fields.get("wait_resource")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or_else(|| event.resource_description.clone())
            {
                if let Some(hobt_id) = Self::parse_wait_resource_hobt_id(wr) {
                    ids.insert(hobt_id);
                }
            }
            // From BPR wait_resource (OBJECT: db_id:object_id:partition)
            if let Some(ref xml) = event.blocked_process_report {
                // Quick extract of waitresource from BPR XML
                if let Some(start) = xml.find("waitresource=\"") {
                    let rest = &xml[start + 14..];
                    if let Some(end) = rest.find('"') {
                        let wr = &rest[..end];
                        if let Some(obj_id) = Self::parse_wait_resource_object_id(wr) {
                            ids.insert(obj_id);
                        }
                        if let Some(hobt_id) = Self::parse_wait_resource_hobt_id(wr) {
                            ids.insert(hobt_id);
                        }
                    }
                }
            }
        }
        ids.into_iter().collect()
    }

    /// Collect unique direct object_ids from OBJECT: wait_resource and BPR blocked_process XML
    pub fn collect_direct_object_ids(&self) -> Vec<i64> {
        let mut ids: HashSet<i64> = HashSet::new();
        for event in &self.events {
            // From object_id extra field
            if let Some(v) = event.extra_fields.get("object_id") {
                if let Some(id) = Self::json_value_as_i64(v) {
                    if id > 0 { ids.insert(id); }
                }
            }
            // From OBJECT: wait_resource
            if let Some(ref wr) = event.extra_fields.get("wait_resource")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
            {
                if let Some(obj_id) = Self::parse_wait_resource_object_id(wr) {
                    ids.insert(obj_id);
                }
            }
            // From BPR XML
            if let Some(ref xml) = event.blocked_process_report {
                if let Some(start) = xml.find("waitresource=\"") {
                    let rest = &xml[start + 14..];
                    if let Some(end) = rest.find('"') {
                        let wr = &rest[..end];
                        if let Some(obj_id) = Self::parse_wait_resource_object_id(wr) {
                            ids.insert(obj_id);
                        }
                    }
                }
                // Object Id from inputbuf "Proc [Database Id = X Object Id = Y]"
                Self::extract_object_ids_from_inputbufs(xml, &mut ids);
            }
            // Same for deadlock graph XML
            if let Some(ref xml) = event.deadlock_graph {
                if let Some(start) = xml.find("waitresource=\"") {
                    let rest = &xml[start + 14..];
                    if let Some(end) = rest.find('"') {
                        let wr = &rest[..end];
                        if let Some(obj_id) = Self::parse_wait_resource_object_id(wr) {
                            ids.insert(obj_id);
                        }
                    }
                }
                Self::extract_object_ids_from_inputbufs(xml, &mut ids);
            }
        }
        ids.into_iter().collect()
    }

    /// Apply resolved direct object names (from sys.objects) to events and BPR data
    pub fn apply_direct_object_names(&mut self, obj_map: &HashMap<i64, String>) -> usize {
        let mut count = 0;
        for event in &mut self.events {
            // Resolve object_id extra field
            if let Some(v) = event.extra_fields.get("object_id") {
                if let Some(id) = Self::json_value_as_i64(v) {
                    if let Some(name) = obj_map.get(&id) {
                        if !event.extra_fields.contains_key("resolved_object") {
                            event.extra_fields.insert(
                                "resolved_object".to_string(),
                                serde_json::Value::String(name.clone()),
                            );
                            if event.object_name.is_none() {
                                event.object_name = Some(name.clone());
                            }
                            count += 1;
                        }
                    }
                }
            }
            // Resolve BPR wait_resource and inputbuf Object Ids
            if let Some(ref xml) = event.blocked_process_report {
                let mut resolved_wr: Option<String> = None;
                if let Some(start) = xml.find("waitresource=\"") {
                    let rest = &xml[start + 14..];
                    if let Some(end) = rest.find('"') {
                        let wr = &rest[..end];
                        if let Some(obj_id) = Self::parse_wait_resource_object_id(wr) {
                            if let Some(name) = obj_map.get(&obj_id) {
                                resolved_wr = Some(format!("{} [{}]", wr.trim(), name));
                            }
                        }
                    }
                }
                if let Some(ref resolved) = resolved_wr {
                    event.extra_fields.insert(
                        "resolved_wait_resource".to_string(),
                        serde_json::Value::String(resolved.clone()),
                    );
                    count += 1;
                }
                // Resolve "Proc [Database Id = X Object Id = Y]" references
                let mut xml_clone = xml.clone();
                for cap_start in xml.match_indices("Object Id = ").map(|(i, _)| i).collect::<Vec<_>>() {
                    let rest = &xml[cap_start + 12..];
                    if let Some(end) = rest.find(']') {
                        if let Ok(id) = rest[..end].trim().parse::<i64>() {
                            if let Some(name) = obj_map.get(&id) {
                                let old = format!("Object Id = {}]", id);
                                let new = format!("Object Id = {} ({})]", id, name);
                                xml_clone = xml_clone.replacen(&old, &new, 1);
                            }
                        }
                    }
                }
                if xml_clone != *xml {
                    event.blocked_process_report = Some(xml_clone);
                }
            }
            // Resolve deadlock graph inputbufs the same way
            if let Some(ref xml) = event.deadlock_graph {
                let mut xml_clone = xml.clone();
                for cap_start in xml.match_indices("Object Id = ").map(|(i, _)| i).collect::<Vec<_>>() {
                    let rest = &xml[cap_start + 12..];
                    if let Some(end) = rest.find(']') {
                        if let Ok(id) = rest[..end].trim().parse::<i64>() {
                            if let Some(name) = obj_map.get(&id) {
                                let old = format!("Object Id = {}]", id);
                                let new = format!("Object Id = {} ({})]", id, name);
                                xml_clone = xml_clone.replacen(&old, &new, 1);
                            }
                        }
                    }
                }
                if xml_clone != *xml {
                    event.deadlock_graph = Some(xml_clone);
                    count += 1;
                }
            }
        }
        count
    }

    /// Collect unique non-zero query_hash values (handles both Number and String)
    pub fn collect_query_hashes(&self) -> Vec<i64> {
        let mut hashes: HashSet<i64> = HashSet::new();
        for event in &self.events {
            if let Some(v) = event.extra_fields.get("query_hash") {
                if let Some(h) = Self::json_value_as_i64(v) {
                    if h != 0 { hashes.insert(h); }
                }
            }
        }
        hashes.into_iter().collect()
    }

    /// Apply resolved database names to events
    pub fn apply_database_names(&mut self, db_map: &HashMap<i64, String>) -> usize {
        let mut count = 0;
        for event in &mut self.events {
            // Skip if already has a non-numeric database name
            if let Some(ref name) = event.database_name {
                if name.parse::<i64>().is_err() { continue; }
            }
            if let Some(v) = event.extra_fields.get("database_id") {
                if let Some(id) = Self::json_value_as_i64(v) {
                    if let Some(name) = db_map.get(&id) {
                        event.database_name = Some(name.clone());
                        count += 1;
                    }
                }
            }
        }
        count
    }

    /// Apply resolved object names from associated_object_id, object_id, and wait_resource
    pub fn apply_object_names(&mut self, obj_map: &HashMap<i64, String>) -> usize {
        let mut count = 0;
        for event in &mut self.events {
            let mut resolved = false;

            // From associated_object_id
            if let Some(v) = event.extra_fields.get("associated_object_id") {
                if let Some(id) = Self::json_value_as_i64(v) {
                    if let Some(name) = obj_map.get(&id) {
                        event.extra_fields.insert(
                            "resolved_object".to_string(),
                            serde_json::Value::String(name.clone()),
                        );
                        if event.object_name.is_none() {
                            event.object_name = Some(name.clone());
                        }
                        resolved = true;
                    }
                }
            }

            // From object_id (lock events with resource_type=OBJECT)
            if !resolved {
                if let Some(v) = event.extra_fields.get("object_id") {
                    if let Some(id) = Self::json_value_as_i64(v) {
                        if let Some(name) = obj_map.get(&id) {
                            event.extra_fields.insert(
                                "resolved_object".to_string(),
                                serde_json::Value::String(name.clone()),
                            );
                            if event.object_name.is_none() {
                                event.object_name = Some(name.clone());
                            }
                            resolved = true;
                        }
                    }
                }
            }

            // From wait_resource (KEY: db_id:hobt_id)
            if !resolved {
                let wr = event.extra_fields.get("wait_resource")
                    .and_then(|v| v.as_str().map(|s| s.to_string()))
                    .or_else(|| event.resource_description.clone());
                if let Some(ref wr_str) = wr {
                    if let Some(hobt_id) = Self::parse_wait_resource_hobt_id(wr_str) {
                        if let Some(name) = obj_map.get(&hobt_id) {
                            event.extra_fields.insert(
                                "resolved_wait_object".to_string(),
                                serde_json::Value::String(name.clone()),
                            );
                            resolved = true;
                        }
                    }
                }
            }

            if resolved { count += 1; }
        }
        count
    }

    /// Get resolved object names from events sharing the same session/transaction as the given event.
    /// Used for XACT and other resource types that don't directly map to objects.
    pub fn get_transaction_objects(&self, event_id: u64) -> Vec<TransactionObject> {
        let anchor = match self.events.iter().find(|e| e.id == event_id) {
            Some(e) => e,
            None => return Vec::new(),
        };

        // Collect candidate event indices using precise correlators first,
        // then fall back to session_id with a tight time window
        let mut candidate_indices: HashSet<usize> = HashSet::new();

        // 1. activity_id — same logical request
        if let Some(v) = anchor.extra_fields.get("attach_activity_id")
            .or_else(|| anchor.extra_fields.get("activity_id"))
        {
            if let Some(s) = v.as_str() {
                if let Some(guid) = s.split(':').next() {
                    if guid.len() >= 36 {
                        if let Some(indices) = self.by_activity.get(guid) {
                            candidate_indices.extend(indices.iter().copied());
                        }
                    }
                }
            }
        }

        // 2. transaction_id — same transaction
        if let Some(v) = anchor.extra_fields.get("transaction_id") {
            let tid_str = match v {
                serde_json::Value::Number(n) => n.as_i64().filter(|&n| n != 0).map(|n| n.to_string()),
                serde_json::Value::String(s) if !s.is_empty() && s != "0" => Some(s.clone()),
                _ => None,
            };
            if let Some(tid) = tid_str {
                if let Some(indices) = self.by_transaction.get(&tid) {
                    candidate_indices.extend(indices.iter().copied());
                }
            }
        }

        // 3. session_id — only with a tight time window (±30s) as fallback
        if let Some(sid) = anchor.session_id {
            if let Some(indices) = self.by_session.get(&sid) {
                let window = chrono::Duration::seconds(30);
                let range_start = anchor.timestamp - window;
                let range_end = anchor.timestamp + window;
                for &idx in indices {
                    let ts = self.events[idx].timestamp;
                    if ts >= range_start && ts <= range_end {
                        candidate_indices.insert(idx);
                    }
                }
            }
        }

        // Collect unique resolved objects from these events
        let mut seen: HashMap<String, TransactionObject> = HashMap::new();

        for &idx in &candidate_indices {
            let event = &self.events[idx];
            if event.id == event_id { continue; }

            // Check resolved_object (from associated_object_id)
            if let Some(name) = event.extra_fields.get("resolved_object").and_then(|v| v.as_str()) {
                let entry = seen.entry(name.to_string()).or_insert_with(|| TransactionObject {
                    object_name: name.to_string(),
                    resource_type: event.resource_type.clone(),
                    lock_modes: Vec::new(),
                    event_count: 0,
                    sample_event_id: event.id,
                });
                entry.event_count += 1;
                if let Some(ref lm) = event.lock_mode {
                    if !entry.lock_modes.contains(lm) {
                        entry.lock_modes.push(lm.clone());
                    }
                }
            }

            // Check resolved_wait_object (from wait_resource KEY:)
            if let Some(name) = event.extra_fields.get("resolved_wait_object").and_then(|v| v.as_str()) {
                let entry = seen.entry(name.to_string()).or_insert_with(|| TransactionObject {
                    object_name: name.to_string(),
                    resource_type: event.resource_type.clone(),
                    lock_modes: Vec::new(),
                    event_count: 0,
                    sample_event_id: event.id,
                });
                entry.event_count += 1;
                if let Some(ref lm) = event.lock_mode {
                    if !entry.lock_modes.contains(lm) {
                        entry.lock_modes.push(lm.clone());
                    }
                }
            }

            // Check object_name directly
            if let Some(ref obj) = event.object_name {
                if !seen.contains_key(obj) {
                    let entry = seen.entry(obj.clone()).or_insert_with(|| TransactionObject {
                        object_name: obj.clone(),
                        resource_type: event.resource_type.clone(),
                        lock_modes: Vec::new(),
                        event_count: 0,
                        sample_event_id: event.id,
                    });
                    entry.event_count += 1;
                    if let Some(ref lm) = event.lock_mode {
                        if !entry.lock_modes.contains(lm) {
                            entry.lock_modes.push(lm.clone());
                        }
                    }
                }
            }
        }

        let mut result: Vec<TransactionObject> = seen.into_values().collect();
        result.sort_by(|a, b| b.event_count.cmp(&a.event_count));
        result
    }

    /// Apply resolved query texts from Query Store
    pub fn apply_query_texts(&mut self, text_map: &HashMap<i64, String>) -> usize {
        let mut count = 0;
        for event in &mut self.events {
            if let Some(v) = event.extra_fields.get("query_hash") {
                if let Some(h) = Self::json_value_as_i64(v) {
                    if let Some(text) = text_map.get(&h) {
                        event.extra_fields.insert(
                            "query_store_text".to_string(),
                            serde_json::Value::String(text.clone()),
                        );
                        if event.sql_text.is_none() && event.statement.is_none() {
                            event.sql_text = Some(text.clone());
                        }
                        count += 1;
                    }
                }
            }
        }
        count
    }

    pub fn get_problem_stats(&self, filter: &XelFilter) -> XelProblemStats {
        let mut deadlock_count = 0usize;
        let mut error_count = 0usize;
        let mut blocked_process_count = 0usize;
        let mut lock_wait_count = 0usize;

        // Wait type aggregation
        let mut wait_agg: HashMap<String, (usize, i64, i64)> = HashMap::new(); // (count, total_dur, max_dur)
        // Error sessions
        let mut error_sessions: HashMap<i64, (usize, i64, String, Option<String>)> = HashMap::new();
        // Wait sessions
        let mut wait_sessions: HashMap<i64, (usize, i64, String, Option<String>)> = HashMap::new();

        for event in self.events.iter().filter(|e| matches_filter(e, filter)) {
            let en = &event.event_name;

            // Deadlocks
            if en.contains("deadlock") {
                deadlock_count += 1;
            }

            // Errors
            if event.result.as_deref() == Some("Error") || event.result.as_deref() == Some("Abort") {
                error_count += 1;
                if let Some(sid) = event.session_id {
                    let entry = error_sessions.entry(sid).or_insert((0, 0, en.clone(), event.object_name.clone()));
                    entry.0 += 1;
                    entry.1 += event.duration_us.unwrap_or(0);
                }
            }

            // Blocked process reports
            if en == "blocked_process_report" {
                blocked_process_count += 1;
            }

            // Lock waits
            if en == "locks_lock_waits" || en.starts_with("lock_timeout")
                || (en == "lock_acquired" && event.duration_us.unwrap_or(0) > 0)
            {
                lock_wait_count += 1;
            }

            // Wait type aggregation (from wait_completed and lock_acquired events)
            if en == "wait_completed" {
                if let Some(ref wt) = event.wait_type {
                    let dur = event.duration_us.unwrap_or(0);
                    let entry = wait_agg.entry(wt.clone()).or_insert((0, 0, 0));
                    entry.0 += 1;
                    entry.1 += dur;
                    entry.2 = entry.2.max(dur);

                    if let Some(sid) = event.session_id {
                        let ws_entry = wait_sessions.entry(sid).or_insert((0, 0, wt.clone(), event.object_name.clone()));
                        ws_entry.0 += 1;
                        ws_entry.1 += dur;
                    }
                }
            }

            // lock_acquired with duration > 0 means the session waited to get the lock
            if en == "lock_acquired" {
                let dur = event.duration_us.unwrap_or(0);
                if dur > 0 {
                    let wt = if let Some(ref mode) = event.lock_mode {
                        format!("LCK_M_{}", mode)
                    } else {
                        "LCK_M_UNKNOWN".to_string()
                    };
                    let entry = wait_agg.entry(wt.clone()).or_insert((0, 0, 0));
                    entry.0 += 1;
                    entry.1 += dur;
                    entry.2 = entry.2.max(dur);

                    if let Some(sid) = event.session_id {
                        let ws_entry = wait_sessions.entry(sid).or_insert((0, 0, wt, event.object_name.clone()));
                        ws_entry.0 += 1;
                        ws_entry.1 += dur;
                    }
                }
            }
        }

        // Build top wait types
        let mut top_wait_types: Vec<WaitTypeStat> = wait_agg
            .into_iter()
            .map(|(wt, (count, total, max))| {
                let avg = if count > 0 { total / count as i64 } else { 0 };
                let category = categorize_wait_type(&wt).to_string();
                WaitTypeStat {
                    wait_type: wt,
                    count,
                    total_duration_us: total,
                    max_duration_us: max,
                    avg_duration_us: avg,
                    category,
                }
            })
            .collect();
        top_wait_types.sort_by(|a, b| b.total_duration_us.cmp(&a.total_duration_us));
        top_wait_types.truncate(15);

        // Build error sessions
        let mut err_sessions: Vec<SessionProblemStat> = error_sessions
            .into_iter()
            .map(|(sid, (count, dur, name, obj))| SessionProblemStat {
                session_id: sid,
                count,
                total_duration_us: dur,
                sample_event_name: name,
                sample_object_name: obj,
            })
            .collect();
        err_sessions.sort_by(|a, b| b.count.cmp(&a.count));
        err_sessions.truncate(10);

        // Build wait sessions
        let mut w_sessions: Vec<SessionProblemStat> = wait_sessions
            .into_iter()
            .map(|(sid, (count, dur, name, obj))| SessionProblemStat {
                session_id: sid,
                count,
                total_duration_us: dur,
                sample_event_name: name,
                sample_object_name: obj,
            })
            .collect();
        w_sessions.sort_by(|a, b| b.total_duration_us.cmp(&a.total_duration_us));
        w_sessions.truncate(10);

        XelProblemStats {
            deadlock_count,
            error_count,
            blocked_process_count,
            lock_wait_count,
            top_wait_types,
            error_sessions: err_sessions,
            wait_sessions: w_sessions,
        }
    }

    /// Analyze blocking relationships for a given event.
    /// Parses blocked_process_report XML, finds blocker sessions, builds chain.
    pub fn analyze_blocking(
        &self,
        event_id: u64,
        time_window_ms: i64,
    ) -> BlockingAnalysis {
        let anchor_idx = match self.events.iter().position(|e| e.id == event_id) {
            Some(idx) => idx,
            None => {
                return BlockingAnalysis {
                    anchor_event_id: event_id,
                    summary: "Event not found".into(),
                    blocked_process_reports: Vec::new(),
                    blocking_chain: Vec::new(),
                    blocker_events: Vec::new(),
                    lock_escalations: Vec::new(),
                    wait_events: Vec::new(),
                    wait_stats: Vec::new(),
                    deadlocks: Vec::new(),
                    deadlock_id: None,
                    deadlock_lock_events: Vec::new(),
                    diagnosis: "unknown".into(),
                    recommendations: Vec::new(),
                };
            }
        };
        let anchor = &self.events[anchor_idx];
        let anchor_sid = anchor.session_id;
        let anchor_ts = anchor.timestamp;
        let window = chrono::Duration::milliseconds(time_window_ms);

        // Compute execution window: [start, end] for the anchor event.
        // For completed events (rpc_completed, sql_batch_completed), the timestamp is
        // the completion time, so start = timestamp - duration.
        // This is used to filter wait events to only those that occurred DURING execution.
        let anchor_dur_us = anchor.duration_us.unwrap_or(0);
        let anchor_exec_start = if anchor_dur_us > 0 {
            anchor_ts - chrono::Duration::microseconds(anchor_dur_us)
        } else {
            anchor_ts - window
        };
        let anchor_exec_end = anchor_ts;

        // 1. Find all blocked_process_report events in time window
        let bpr_indices = self
            .by_event_name
            .get("blocked_process_report")
            .cloned()
            .unwrap_or_default();

        let mut parsed_bprs: Vec<ParsedBlockedProcessReport> = Vec::new();
        let mut involved_sessions: HashSet<i64> = HashSet::new();

        if let Some(sid) = anchor_sid {
            involved_sessions.insert(sid);
        }

        // If the anchor IS a blocked_process_report, pre-seed involved_sessions
        // from its own XML. The event's session_id is the XE listener session,
        // not the blocked/blocking SPID, so we need the real SPIDs to match.
        if anchor.event_name == "blocked_process_report" {
            if let Some(ref xml) = anchor.blocked_process_report {
                if let Some(parsed) = parse_blocked_process_report_xml(xml, anchor.id, anchor.timestamp) {
                    if let Some(s) = parsed.blocked_spid {
                        involved_sessions.insert(s);
                    }
                    if let Some(s) = parsed.blocking_spid {
                        involved_sessions.insert(s);
                    }
                    parsed_bprs.push(parsed);
                }
            }
        }

        for &idx in &bpr_indices {
            let bpr_event = &self.events[idx];
            let diff = (bpr_event.timestamp - anchor_ts).num_milliseconds().abs();
            if diff > time_window_ms {
                continue;
            }

            if let Some(ref xml) = bpr_event.blocked_process_report {
                if let Some(parsed) = parse_blocked_process_report_xml(xml, bpr_event.id, bpr_event.timestamp) {
                    // Skip if already added (e.g. anchor's own BPR)
                    if parsed_bprs.iter().any(|p| p.event_id == parsed.event_id) {
                        continue;
                    }
                    // Include if any known involved session is referenced
                    let session_involved = anchor_sid.map_or(true, |sid| {
                        parsed.blocked_spid == Some(sid)
                            || parsed.blocking_spid == Some(sid)
                    }) || parsed.blocked_spid.map_or(false, |s| involved_sessions.contains(&s))
                       || parsed.blocking_spid.map_or(false, |s| involved_sessions.contains(&s));

                    if session_involved {
                        if let Some(s) = parsed.blocked_spid {
                            involved_sessions.insert(s);
                        }
                        if let Some(s) = parsed.blocking_spid {
                            involved_sessions.insert(s);
                        }
                        parsed_bprs.push(parsed);
                    }
                }
            }
        }

        // Also search BPRs where involved sessions appear (transitive blocking)
        // Second pass: find BPRs involving any session we've already identified
        for &idx in &bpr_indices {
            let bpr_event = &self.events[idx];
            let diff = (bpr_event.timestamp - anchor_ts).num_milliseconds().abs();
            if diff > time_window_ms {
                continue;
            }
            if let Some(ref xml) = bpr_event.blocked_process_report {
                if let Some(parsed) = parse_blocked_process_report_xml(xml, bpr_event.id, bpr_event.timestamp) {
                    let already_have = parsed_bprs.iter().any(|p| p.event_id == parsed.event_id);
                    if already_have {
                        continue;
                    }
                    let transitive = parsed.blocked_spid.map_or(false, |s| involved_sessions.contains(&s))
                        || parsed.blocking_spid.map_or(false, |s| involved_sessions.contains(&s));
                    if transitive {
                        if let Some(s) = parsed.blocked_spid {
                            involved_sessions.insert(s);
                        }
                        if let Some(s) = parsed.blocking_spid {
                            involved_sessions.insert(s);
                        }
                        parsed_bprs.push(parsed);
                    }
                }
            }
        }

        parsed_bprs.sort_by_key(|p| p.timestamp);

        // Enrich BPR wait_resource with resolved object names from extra_fields
        for bpr in &mut parsed_bprs {
            if let Some(event) = self.events.iter().find(|e| e.id == bpr.event_id) {
                if let Some(resolved) = event.extra_fields.get("resolved_wait_resource").and_then(|v| v.as_str()) {
                    bpr.blocked_wait_resource = Some(resolved.to_string());
                }
            }
        }

        // 2. Build blocking chain from BPRs
        let blocking_chain = build_blocking_chain(&parsed_bprs, &self.events, &involved_sessions, anchor_ts, window);

        // 3. Find events from blocking AND blocked sessions around the time.
        // Both sides are important for understanding the contention scenario.
        let mut blocker_events: Vec<XelEvent> = Vec::new();
        let mut bpr_sids: HashSet<i64> = HashSet::new();
        for bpr in &parsed_bprs {
            if let Some(s) = bpr.blocking_spid { bpr_sids.insert(s); }
            if let Some(s) = bpr.blocked_spid { bpr_sids.insert(s); }
        }

        for &sid in &bpr_sids {
            if Some(sid) == anchor_sid {
                continue; // Don't include anchor session's own events
            }
            if let Some(indices) = self.by_session.get(&sid) {
                for &idx in indices {
                    let ev = &self.events[idx];
                    let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                    if diff <= time_window_ms {
                        blocker_events.push(ev.clone());
                    }
                }
            }
        }
        blocker_events.sort_by_key(|e| e.timestamp);
        blocker_events.truncate(100);

        // 3b. When no BPRs found, try to find blocking sessions by correlating lock events.
        // Look at other sessions that held locks on the same object around the same time.
        if parsed_bprs.is_empty() && blocker_events.is_empty() {
            // Collect the anchor session's object(s) — from object_name, extra_fields
            let mut contended_objects: HashSet<String> = HashSet::new();
            if let Some(ref obj) = anchor.object_name {
                contended_objects.insert(obj.to_lowercase());
            }
            for key in ["resolved_object", "resolved_wait_object"] {
                if let Some(name) = anchor.extra_fields.get(key).and_then(|v| v.as_str()) {
                    contended_objects.insert(name.to_lowercase());
                }
            }
            // Collect associated_object_id from the anchor session's lock events in window
            let mut contended_hobt_ids: HashSet<i64> = HashSet::new();
            if let Some(sid) = anchor_sid {
                let lock_scan_names = ["wait_completed", "lock_acquired", "locks_lock_waits"];
                for name in &lock_scan_names {
                    if let Some(indices) = self.by_event_name.get(*name) {
                        for &idx in indices {
                            let ev = &self.events[idx];
                            if ev.session_id != Some(sid) { continue; }
                            // Only consider lock events during execution window
                            if ev.timestamp < anchor_exec_start || ev.timestamp > anchor_exec_end {
                                continue;
                            }
                            let is_lock_wait = ev.wait_type.as_deref().map_or(false, |w| w.starts_with("LCK_M_"))
                                || ev.event_name != "wait_completed";
                            if !is_lock_wait { continue; }
                            if let Some(hobt) = ev.extra_fields.get("associated_object_id").and_then(|v| Self::json_value_as_i64(v)) {
                                if hobt != 0 { contended_hobt_ids.insert(hobt); }
                            }
                            if let Some(ref obj) = ev.object_name {
                                contended_objects.insert(obj.to_lowercase());
                            }
                            for key in ["resolved_object", "resolved_wait_object"] {
                                if let Some(name) = ev.extra_fields.get(key).and_then(|v| v.as_str()) {
                                    contended_objects.insert(name.to_lowercase());
                                }
                            }
                        }
                    }
                }
            }

            if !contended_objects.is_empty() || !contended_hobt_ids.is_empty() {
                // Search lock-related and completed events from OTHER sessions in the time window
                let search_names = ["lock_acquired", "lock_released", "lock_escalation",
                    "locks_lock_waits", "lock_timeout", "lock_timeout_greater_than_0",
                    "wait_completed", "rpc_completed", "sql_batch_completed"];
                let mut candidate_sids: HashMap<i64, Vec<XelEvent>> = HashMap::new();

                for name in &search_names {
                    if let Some(indices) = self.by_event_name.get(*name) {
                        for &idx in indices {
                            let ev = &self.events[idx];
                            if ev.session_id == anchor_sid || ev.session_id.is_none() {
                                continue;
                            }
                            // Only consider events that overlap with the anchor's execution window
                            if ev.timestamp < anchor_exec_start || ev.timestamp > anchor_exec_end {
                                continue;
                            }
                            let mut matches = false;
                            if let Some(ref obj) = ev.object_name {
                                if contended_objects.contains(&obj.to_lowercase()) {
                                    matches = true;
                                }
                            }
                            for key in ["resolved_object", "resolved_wait_object"] {
                                if let Some(n) = ev.extra_fields.get(key).and_then(|v| v.as_str()) {
                                    if contended_objects.contains(&n.to_lowercase()) {
                                        matches = true;
                                    }
                                }
                            }
                            if !matches {
                                if let Some(hobt) = ev.extra_fields.get("associated_object_id").and_then(|v| Self::json_value_as_i64(v)) {
                                    if contended_hobt_ids.contains(&hobt) {
                                        matches = true;
                                    }
                                }
                            }
                            if matches {
                                let sid = ev.session_id.unwrap();
                                candidate_sids.entry(sid).or_default().push(ev.clone());
                            }
                        }
                    }
                }

                for (sid, mut events) in candidate_sids {
                    involved_sessions.insert(sid);
                    events.sort_by_key(|e| e.timestamp);
                    events.truncate(20);
                    blocker_events.extend(events);
                }
                blocker_events.sort_by_key(|e| e.timestamp);
                blocker_events.truncate(100);
            }
        }

        // 4. Find lock_escalation events in time window
        let mut lock_escalations: Vec<XelEvent> = Vec::new();
        if let Some(esc_indices) = self.by_event_name.get("lock_escalation") {
            for &idx in esc_indices {
                let ev = &self.events[idx];
                // For escalations from the anchor session, restrict to execution window.
                // For other involved sessions, use the broader time window.
                let in_window = if ev.session_id == anchor_sid {
                    ev.timestamp >= anchor_exec_start && ev.timestamp <= anchor_exec_end
                } else {
                    let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                    diff <= time_window_ms
                };
                if in_window {
                    if involved_sessions.is_empty()
                        || ev.session_id.map_or(false, |s| involved_sessions.contains(&s))
                    {
                        lock_escalations.push(ev.clone());
                    }
                }
            }
        }

        // 5. Find wait events for involved sessions (anchor + blocked/blocking SPIDs)
        // For blocked_process_report events, the anchor's session_id is the XE listener,
        // so we need to search by the real involved SPIDs from the parsed BPRs.
        let wait_search_sids: HashSet<i64> = if anchor.event_name == "blocked_process_report" {
            // Use all involved sessions (blocked + blocking SPIDs)
            involved_sessions.clone()
        } else {
            anchor_sid.into_iter().collect()
        };
        let mut wait_events: Vec<XelEvent> = Vec::new();
        let wait_event_names = ["locks_lock_waits", "lock_timeout", "lock_timeout_greater_than_0", "wait_completed", "lock_acquired"];
        for name in &wait_event_names {
            if let Some(indices) = self.by_event_name.get(*name) {
                for &idx in indices {
                    let ev = &self.events[idx];
                    // For lock_acquired, only include if there was actual wait time
                    if ev.event_name == "lock_acquired" && ev.duration_us.unwrap_or(0) == 0 {
                        continue;
                    }
                    if ev.session_id.map_or(false, |s| wait_search_sids.contains(&s)) {
                        // For the anchor's own session, only include waits that occurred
                        // DURING the anchor's execution window (not after it completed).
                        // For other sessions (blocking SPIDs), use the broader time window.
                        let in_window = if ev.session_id == anchor_sid {
                            ev.timestamp >= anchor_exec_start && ev.timestamp <= anchor_exec_end
                        } else {
                            let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                            diff <= time_window_ms
                        };
                        if in_window {
                            wait_events.push(ev.clone());
                        }
                    }
                }
            }
        }
        wait_events.sort_by_key(|e| e.timestamp);

        // 6. Aggregate wait stats by wait type
        let mut wait_stats = aggregate_wait_stats(&wait_events);

        // 7. Find and parse deadlock graphs in time window
        let mut deadlocks: Vec<ParsedDeadlockGraph> = Vec::new();
        let deadlock_event_names = ["xml_deadlock_report", "database_xml_deadlock_report", "lock_deadlock", "lock_deadlock_chain"];

        // Detect if anchor event is likely a deadlock victim (error_number 1205)
        let error_number = anchor.extra_fields.get("error_number")
            .and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            });
        let is_likely_deadlock_victim = error_number == Some(1205);
        for name in &deadlock_event_names {
            if let Some(indices) = self.by_event_name.get(*name) {
                for &idx in indices {
                    let ev = &self.events[idx];
                    let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                    if diff > time_window_ms {
                        continue;
                    }
                    if let Some(ref xml) = ev.deadlock_graph {
                        if let Some(parsed) = parse_deadlock_graph_xml(xml, ev.id, ev.timestamp) {
                            let anchor_involved = anchor_sid.map_or(true, |sid| {
                                parsed.processes.iter().any(|p| p.spid == Some(sid))
                            });
                            // The anchor IS the deadlock report event itself
                            let anchor_is_this_event = ev.id == event_id;
                            // If anchor is likely a deadlock victim, include nearby
                            // deadlocks even if session doesn't match exactly
                            let close_in_time = diff <= 5000; // within 5s
                            if anchor_involved || anchor_is_this_event
                                || (is_likely_deadlock_victim && close_in_time)
                            {
                                deadlocks.push(parsed);
                            }
                        }
                    }
                }
            }
        }
        deadlocks.sort_by_key(|d| d.timestamp);
        // Deduplicate by event_id
        deadlocks.dedup_by_key(|d| d.event_id);

        // 7b. Search for deadlock_id on lock events in the same session/transaction/time window.
        // This confirms a deadlock even when xml_deadlock_report was not captured.
        let mut deadlock_id: Option<i64> = None;
        let mut deadlock_lock_events: Vec<XelEvent> = Vec::new();

        // First check the anchor event itself (deadlock_id from lock_deadlock,
        // deadlock_cycle_id from database_xml_deadlock_report)
        for key in &["deadlock_id", "deadlock_cycle_id"] {
            if deadlock_id.is_some() { break; }
            if let Some(did) = anchor.extra_fields.get(*key).and_then(|v| Self::json_value_as_i64(v)) {
                if did != 0 {
                    deadlock_id = Some(did);
                }
            }
        }

        // If anchor doesn't have deadlock_id, search related events in same session/transaction
        let deadlock_id_keys = ["deadlock_id", "deadlock_cycle_id"];
        if deadlock_id.is_none() {
            // Check events in same session within time window
            if let Some(sid) = anchor_sid {
                if let Some(indices) = self.by_session.get(&sid) {
                    'session_search: for &idx in indices {
                        let ev = &self.events[idx];
                        let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                        if diff <= time_window_ms {
                            for key in &deadlock_id_keys {
                                if let Some(did) = ev.extra_fields.get(*key).and_then(|v| Self::json_value_as_i64(v)) {
                                    if did != 0 {
                                        deadlock_id = Some(did);
                                        break 'session_search;
                                    }
                                }
                            }
                        }
                    }
                }
            }
            // Also check by transaction_id
            if deadlock_id.is_none() {
                if let Some(v) = anchor.extra_fields.get("transaction_id") {
                    let tid_str = match v {
                        serde_json::Value::Number(n) => n.as_i64().filter(|&n| n != 0).map(|n| n.to_string()),
                        serde_json::Value::String(s) if !s.is_empty() && s != "0" => Some(s.clone()),
                        _ => None,
                    };
                    if let Some(tid) = tid_str {
                        if let Some(indices) = self.by_transaction.get(&tid) {
                            'txn_search: for &idx in indices {
                                let ev = &self.events[idx];
                                let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                                if diff <= time_window_ms {
                                    for key in &deadlock_id_keys {
                                        if let Some(did) = ev.extra_fields.get(*key).and_then(|v| Self::json_value_as_i64(v)) {
                                            if did != 0 {
                                                deadlock_id = Some(did);
                                                break 'txn_search;
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

        // Collect all events with the same deadlock_id
        if let Some(did) = deadlock_id {
            if let Some(indices) = self.by_deadlock.get(&did.to_string()) {
                for &idx in indices {
                    deadlock_lock_events.push(self.events[idx].clone());
                }
            }
            deadlock_lock_events.sort_by_key(|e| e.timestamp);

            // Find other sessions involved in this deadlock and fetch their SQL
            let other_sids: HashSet<i64> = deadlock_lock_events.iter()
                .filter_map(|e| e.session_id)
                .filter(|s| Some(*s) != anchor_sid)
                .collect();

            for &sid in &other_sids {
                if let Some(indices) = self.by_session.get(&sid) {
                    for &idx in indices {
                        let ev = &self.events[idx];
                        let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                        if diff <= time_window_ms {
                            if !blocker_events.iter().any(|e| e.id == ev.id) {
                                blocker_events.push(ev.clone());
                            }
                        }
                    }
                }
            }
            blocker_events.sort_by_key(|e| e.timestamp);
            blocker_events.truncate(100);
        }

        let has_deadlock_id = deadlock_id.is_some() && !deadlock_lock_events.is_empty();

        // 7c. If we have parsed deadlocks, fetch events from participant sessions
        // (even if deadlock_lock_events is empty — the parsed graph has the SPIDs)
        // Include ALL participants — including the anchor session, because for
        // deadlock events (database_xml_deadlock_report, lock_deadlock) the anchor
        // IS a participant and we need its rpc_completed/sql_batch_completed.
        if !deadlocks.is_empty() {
            let mut participant_sids: HashSet<i64> = HashSet::new();
            for dl in &deadlocks {
                for proc in &dl.processes {
                    if let Some(spid) = proc.spid {
                        participant_sids.insert(spid);
                    }
                }
            }
            for &sid in &participant_sids {
                if let Some(indices) = self.by_session.get(&sid) {
                    for &idx in indices {
                        let ev = &self.events[idx];
                        let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                        if diff <= time_window_ms {
                            if !blocker_events.iter().any(|e| e.id == ev.id) {
                                blocker_events.push(ev.clone());
                            }
                        }
                    }
                }
            }
            blocker_events.sort_by_key(|e| e.timestamp);
            blocker_events.truncate(100);
        }

        // 7d. Correlate by attach_activity_id — find all events (locks, waits, rpc)
        // sharing an activity_id with any deadlock participant event.
        // This catches rpc_completed events that share the same request as lock events.
        // Parallel worker threads have unique activity_ids but share attach_activity_id_xfer
        // with their parent request (the rpc_completed). Collecting _xfer GUIDs from
        // session-correlated events (blocker_events from step 7c) links back to the RPC.
        // Only run for deadlock scenarios — for lock_blocking with BPRs, the blocker/blocked
        // sessions are already identified and activity_id fan-out pulls in unrelated sessions.
        if !deadlocks.is_empty() || has_deadlock_id || is_likely_deadlock_victim {
            let mut activity_guids: HashSet<String> = HashSet::new();

            let collect_guids = |ev: &XelEvent, guids: &mut HashSet<String>| {
                for field in &["attach_activity_id", "attach_activity_id_xfer"] {
                    if let Some(s) = ev.extra_fields.get(*field).and_then(|v| v.as_str()) {
                        if let Some(guid) = s.split(':').next() {
                            if guid.len() >= 36 {
                                guids.insert(guid.to_string());
                            }
                        }
                    }
                }
            };

            // Collect from anchor, deadlock lock events, and blocker events (from step 7c)
            collect_guids(anchor, &mut activity_guids);
            for ev in &deadlock_lock_events {
                collect_guids(ev, &mut activity_guids);
            }
            for ev in &blocker_events {
                collect_guids(ev, &mut activity_guids);
            }

            let existing_ids: HashSet<u64> = blocker_events.iter().map(|e| e.id)
                .chain(std::iter::once(event_id))
                .collect();
            for guid in &activity_guids {
                if let Some(indices) = self.by_activity.get(guid) {
                    for &idx in indices {
                        let ev = &self.events[idx];
                        if existing_ids.contains(&ev.id) { continue; }
                        let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                        if diff <= time_window_ms {
                            blocker_events.push(ev.clone());
                        }
                    }
                }
            }
            blocker_events.sort_by_key(|e| e.timestamp);
            blocker_events.truncate(200);
        }

        // 8. Diagnose the root cause
        let diagnosis = if !deadlocks.is_empty() {
            "deadlock".to_string()
        } else if has_deadlock_id {
            // Confirmed deadlock via lock events (deadlock_id present) even without xml graph
            "deadlock".to_string()
        } else if is_likely_deadlock_victim && deadlocks.is_empty() {
            // Error number 1205 but no captured deadlock graph
            "likely_deadlock".to_string()
        } else {
            diagnose_wait_pattern(anchor, &parsed_bprs, &wait_stats)
        };

        // 8b. If the query is CPU-bound (no_waits), discard fallback-correlated blocker
        // events and wait events — they touched the same objects but didn't actually
        // block this session. The query spent its time on CPU, not waiting.
        // Only clear when blockers came from the fallback path (no BPRs, no deadlocks).
        if diagnosis == "no_waits" && parsed_bprs.is_empty() && deadlocks.is_empty() && !has_deadlock_id {
            blocker_events.clear();
            wait_events.clear();
            wait_stats.clear();
        }

        // 9. Build recommendations
        let has_snapshot = self.has_snapshot_isolation(anchor.database_name.as_deref());
        let recommendations = build_recommendations(anchor, &diagnosis, &wait_stats, &deadlocks, deadlock_id, &deadlock_lock_events, &parsed_bprs, &blocker_events, has_snapshot);

        // 10. Build summary
        let summary = build_analysis_summary(anchor, &parsed_bprs, &blocking_chain, &wait_stats, &diagnosis, &deadlocks, deadlock_id, &deadlock_lock_events);

        BlockingAnalysis {
            anchor_event_id: event_id,
            summary,
            blocked_process_reports: parsed_bprs,
            blocking_chain,
            blocker_events,
            lock_escalations,
            wait_events,
            wait_stats,
            deadlocks,
            deadlock_id,
            deadlock_lock_events,
            diagnosis,
            recommendations,
        }
    }
}

/// Parse the XML content of a blocked_process_report
fn parse_blocked_process_report_xml(
    xml: &str,
    event_id: u64,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Option<ParsedBlockedProcessReport> {
    let mut result = ParsedBlockedProcessReport {
        event_id,
        timestamp,
        blocked_spid: None,
        blocked_xact_id: None,
        blocked_wait_resource: None,
        blocked_wait_time_ms: None,
        blocked_lock_mode: None,
        blocked_input_buffer: None,
        blocked_database: None,
        blocked_hostname: None,
        blocked_app_name: None,
        blocked_login_name: None,
        blocked_status: None,
        blocked_last_batch_started: None,
        blocking_spid: None,
        blocking_xact_id: None,
        blocking_input_buffer: None,
        blocking_database: None,
        blocking_hostname: None,
        blocking_app_name: None,
        blocking_login_name: None,
        blocking_status: None,
        blocking_last_batch_started: None,
        blocked_isolation_level: None,
        blocked_tran_count: None,
        blocking_isolation_level: None,
        blocking_tran_count: None,
        blocked_execution_stack: Vec::new(),
        blocking_execution_stack: Vec::new(),
    };

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    #[derive(PartialEq, Clone)]
    enum Section {
        None,
        BlockedProcess,
        BlockingProcess,
    }
    let mut section = Section::None;
    let mut in_input_buf = false;
    let mut current_input_buf = String::new();
    let mut input_buf_section = Section::None;

    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(ref e)) | Ok(XmlEvent::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "blocked-process" => section = Section::BlockedProcess,
                    "blocking-process" => section = Section::BlockingProcess,
                    "process" => {
                        let attrs = extract_xml_attrs(e);
                        match section {
                            Section::BlockedProcess => {
                                result.blocked_spid = attrs.get("spid").and_then(|s| s.parse().ok());
                                result.blocked_xact_id = attrs.get("xactid").cloned();
                                result.blocked_wait_resource = attrs.get("waitresource").cloned();
                                result.blocked_wait_time_ms = attrs.get("waittime").and_then(|s| s.parse().ok());
                                result.blocked_lock_mode = attrs.get("lockMode").cloned();
                                result.blocked_database = attrs.get("currentdb").cloned()
                                    .or_else(|| attrs.get("currentdbname").cloned());
                                result.blocked_hostname = attrs.get("hostname").cloned();
                                result.blocked_app_name = attrs.get("clientapp").cloned();
                                result.blocked_login_name = attrs.get("loginname").cloned();
                                result.blocked_isolation_level = attrs.get("isolationlevel").cloned();
                                result.blocked_tran_count = attrs.get("trancount").and_then(|s| s.parse().ok());
                                result.blocked_status = attrs.get("status").cloned();
                                result.blocked_last_batch_started = attrs.get("lastbatchstarted").cloned();
                            }
                            Section::BlockingProcess => {
                                result.blocking_spid = attrs.get("spid").and_then(|s| s.parse().ok());
                                result.blocking_xact_id = attrs.get("xactid").cloned();
                                result.blocking_database = attrs.get("currentdb").cloned()
                                    .or_else(|| attrs.get("currentdbname").cloned());
                                result.blocking_hostname = attrs.get("hostname").cloned();
                                result.blocking_app_name = attrs.get("clientapp").cloned();
                                result.blocking_login_name = attrs.get("loginname").cloned();
                                result.blocking_status = attrs.get("status").cloned();
                                result.blocking_last_batch_started = attrs.get("lastbatchstarted").cloned();
                                result.blocking_isolation_level = attrs.get("isolationlevel").cloned();
                                result.blocking_tran_count = attrs.get("trancount").and_then(|s| s.parse().ok());
                            }
                            Section::None => {}
                        }
                    }
                    "frame" => {
                        let attrs = extract_xml_attrs(e);
                        let frame = ExecutionFrame {
                            query_hash: attrs.get("queryhash").cloned().filter(|s| s != "0x0000000000000000"),
                            query_plan_hash: attrs.get("queryplanhash").cloned().filter(|s| s != "0x0000000000000000"),
                            line: attrs.get("line").and_then(|s| s.parse().ok()),
                            sql_handle: attrs.get("sqlhandle").cloned(),
                        };
                        match section {
                            Section::BlockedProcess => result.blocked_execution_stack.push(frame),
                            Section::BlockingProcess => result.blocking_execution_stack.push(frame),
                            Section::None => {}
                        }
                    }
                    "inputbuf" => {
                        in_input_buf = true;
                        current_input_buf.clear();
                        input_buf_section = match section {
                            Section::BlockedProcess => Section::BlockedProcess,
                            Section::BlockingProcess => Section::BlockingProcess,
                            Section::None => Section::None,
                        };
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Text(ref e)) => {
                if in_input_buf {
                    if let Ok(text) = e.unescape() {
                        current_input_buf.push_str(&text);
                    }
                }
            }
            Ok(XmlEvent::CData(ref e)) => {
                if in_input_buf {
                    current_input_buf.push_str(&String::from_utf8_lossy(e.as_ref()));
                }
            }
            Ok(XmlEvent::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "blocked-process" | "blocking-process" => {
                        section = Section::None;
                    }
                    "inputbuf" => {
                        let buf = current_input_buf.trim().to_string();
                        if !buf.is_empty() {
                            match input_buf_section {
                                Section::BlockedProcess => {
                                    result.blocked_input_buffer = Some(buf);
                                }
                                Section::BlockingProcess => {
                                    result.blocking_input_buffer = Some(buf);
                                }
                                Section::None => {}
                            }
                        }
                        in_input_buf = false;
                        current_input_buf.clear();
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // If we got at least one spid, consider it valid
    if result.blocked_spid.is_some() || result.blocking_spid.is_some() {
        Some(result)
    } else {
        None
    }
}

/// Helper: extract attributes from an XML element as HashMap
fn extract_xml_attrs(e: &quick_xml::events::BytesStart) -> HashMap<String, String> {
    let mut map = HashMap::new();
    for attr in e.attributes().flatten() {
        let key = String::from_utf8_lossy(attr.key.as_ref()).to_string();
        let val = String::from_utf8_lossy(&attr.value).to_string();
        map.insert(key, val);
    }
    map
}

/// Parse a deadlock graph XML (from xml_deadlock_report or deadlock_graph field)
fn parse_deadlock_graph_xml(
    xml: &str,
    event_id: u64,
    timestamp: chrono::DateTime<chrono::Utc>,
) -> Option<ParsedDeadlockGraph> {
    let mut processes: Vec<DeadlockProcess> = Vec::new();
    let mut resources: Vec<DeadlockResource> = Vec::new();
    let mut victim_ids: HashSet<String> = HashSet::new();

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    // State tracking
    enum State {
        Root,
        VictimList,
        ProcessList,
        InProcess(String), // process id
        InInputBuf(String), // owning process id
        InExecStack(String), // owning process id
        InExecFrame(String, DeadlockExecutionFrame), // owning process id + frame being built
        ResourceList,
        InResource(DeadlockResource),
    }
    let mut state = State::Root;
    let mut current_input_buf = String::new();
    let mut current_frame_text = String::new();

    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(ref e)) | Ok(XmlEvent::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let attrs = extract_xml_attrs(e);

                match tag.as_str() {
                    "victim-list" => state = State::VictimList,
                    "victimProcess" => {
                        if let State::VictimList = &state {
                            if let Some(id) = attrs.get("id") {
                                victim_ids.insert(id.clone());
                            }
                        }
                    }
                    "process-list" => {
                        if matches!(state, State::Root | State::VictimList) {
                            state = State::ProcessList;
                        }
                    }
                    "process" => {
                        if matches!(state, State::ProcessList) {
                            let proc_id = attrs.get("id").cloned().unwrap_or_default();
                            let proc = DeadlockProcess {
                                id: proc_id.clone(),
                                spid: attrs.get("spid").and_then(|s| s.parse().ok()),
                                is_victim: false, // set later from victim_ids
                                xact_id: attrs.get("xactid").cloned(),
                                lock_mode: attrs.get("lockMode").cloned(),
                                wait_resource: attrs.get("waitresource").cloned(),
                                wait_time_ms: attrs.get("waittime").and_then(|s| s.parse().ok()),
                                transaction_name: attrs.get("transactionname").cloned(),
                                log_used: attrs.get("logused").and_then(|s| s.parse().ok()),
                                input_buffer: None,
                                database_name: attrs.get("currentdbname").cloned()
                                    .or_else(|| attrs.get("currentdb").cloned()),
                                hostname: attrs.get("hostname").cloned(),
                                app_name: attrs.get("clientapp").cloned(),
                                login_name: attrs.get("loginname").cloned(),
                                isolation_level: attrs.get("isolationlevel").cloned(),
                                status: attrs.get("status").cloned(),
                                tran_count: attrs.get("trancount").and_then(|s| s.parse().ok()),
                                last_batch_started: attrs.get("lastbatchstarted").cloned(),
                                last_batch_completed: attrs.get("lastbatchcompleted").cloned(),
                                ecid: attrs.get("ecid").and_then(|s| s.parse().ok()),
                                execution_stack: Vec::new(),
                            };
                            processes.push(proc);
                            state = State::InProcess(proc_id);
                        }
                    }
                    "executionStack" => {
                        if let State::InProcess(ref pid) = state {
                            state = State::InExecStack(pid.clone());
                        }
                    }
                    "frame" => {
                        if let State::InExecStack(ref pid) = state {
                            let frame = DeadlockExecutionFrame {
                                proc_name: attrs.get("procname").cloned()
                                    .filter(|s| s != "unknown" && s != "adhoc"),
                                query_hash: attrs.get("queryhash").cloned()
                                    .filter(|s| s != "0x0000000000000000"),
                                query_plan_hash: attrs.get("queryplanhash").cloned()
                                    .filter(|s| s != "0x0000000000000000"),
                                line: attrs.get("line").and_then(|s| s.parse().ok()),
                                sql_handle: attrs.get("sqlhandle").cloned(),
                                sql_text: None,
                            };
                            current_frame_text.clear();
                            state = State::InExecFrame(pid.clone(), frame);
                        }
                    }
                    "inputbuf" => {
                        if let State::InProcess(ref pid) = state {
                            current_input_buf.clear();
                            state = State::InInputBuf(pid.clone());
                        }
                    }
                    "resource-list" => state = State::ResourceList,
                    "keylock" | "pagelock" | "objectlock" | "ridlock" | "hobtlock"
                    | "xactlock" | "exchangeEvent" | "threadpool" | "metadata" => {
                        if matches!(state, State::ResourceList) {
                            let res = DeadlockResource {
                                resource_type: tag.clone(),
                                database_name: attrs.get("dbid").cloned()
                                    .or_else(|| attrs.get("databasename").cloned()),
                                object_name: attrs.get("objectname").cloned(),
                                index_name: attrs.get("indexname").cloned(),
                                mode: attrs.get("mode").cloned(),
                                hobt_id: attrs.get("hobtid").cloned()
                                    .or_else(|| attrs.get("associatedObjectId").cloned()),
                                file_id: attrs.get("fileid").cloned(),
                                page_id: attrs.get("pageid").cloned(),
                                holders: Vec::new(),
                                waiters: Vec::new(),
                            };
                            state = State::InResource(res);
                        } else if let State::InResource(ref mut res) = state {
                            // Nested resource (e.g. keylock inside xactlock/UnderlyingResource)
                            // — fill in details on the parent resource
                            if res.object_name.is_none() {
                                res.object_name = attrs.get("objectname").cloned();
                            }
                            if res.index_name.is_none() {
                                res.index_name = attrs.get("indexname").cloned();
                            }
                            if res.hobt_id.is_none() {
                                res.hobt_id = attrs.get("hobtid").cloned()
                                    .or_else(|| attrs.get("associatedObjectId").cloned());
                            }
                            if res.database_name.is_none() {
                                res.database_name = attrs.get("dbid").cloned();
                            }
                        }
                    }
                    // xactlock wraps underlying resource info (e.g. keylock inside <UnderlyingResource>)
                    "UnderlyingResource" => {
                        // Keep current resource state — child elements fill in details
                    }
                    "owner-list" | "waiter-list" => {
                        // Keep current resource state
                    }
                    "owner" => {
                        if let State::InResource(ref mut res) = state {
                            let owner = DeadlockResourceOwner {
                                process_id: attrs.get("id").cloned().unwrap_or_default(),
                                mode: attrs.get("mode").cloned(),
                            };
                            res.holders.push(owner);
                        }
                    }
                    "waiter" => {
                        if let State::InResource(ref mut res) = state {
                            let waiter = DeadlockResourceOwner {
                                process_id: attrs.get("id").cloned().unwrap_or_default(),
                                mode: attrs.get("mode").cloned(),
                            };
                            res.waiters.push(waiter);
                        }
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Text(ref e)) => {
                match &state {
                    State::InInputBuf(_) => {
                        if let Ok(text) = e.unescape() {
                            current_input_buf.push_str(&text);
                        }
                    }
                    State::InExecFrame(_, _) => {
                        if let Ok(text) = e.unescape() {
                            current_frame_text.push_str(&text);
                        }
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::CData(ref e)) => {
                match &state {
                    State::InInputBuf(_) => {
                        current_input_buf.push_str(&String::from_utf8_lossy(e.as_ref()));
                    }
                    State::InExecFrame(_, _) => {
                        current_frame_text.push_str(&String::from_utf8_lossy(e.as_ref()));
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::End(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag.as_str() {
                    "inputbuf" => {
                        if let State::InInputBuf(ref pid) = state {
                            let buf = current_input_buf.trim().to_string();
                            if let Some(proc) = processes.iter_mut().find(|p| &p.id == pid) {
                                if !buf.is_empty() {
                                    proc.input_buffer = Some(buf);
                                }
                            }
                            let pid_clone = pid.clone();
                            state = State::InProcess(pid_clone);
                            current_input_buf.clear();
                        }
                    }
                    "frame" => {
                        if let State::InExecFrame(pid, mut frame) = std::mem::replace(&mut state, State::Root) {
                            let text = current_frame_text.trim().to_string();
                            if !text.is_empty() {
                                frame.sql_text = Some(text);
                            }
                            current_frame_text.clear();
                            // Add frame to the process
                            if let Some(proc) = processes.iter_mut().find(|p| p.id == pid) {
                                proc.execution_stack.push(frame);
                            }
                            state = State::InExecStack(pid);
                        }
                    }
                    "executionStack" => {
                        if let State::InExecStack(pid) = std::mem::replace(&mut state, State::Root) {
                            state = State::InProcess(pid);
                        }
                    }
                    "process" => {
                        if matches!(state, State::InProcess(_)) {
                            state = State::ProcessList;
                        }
                    }
                    "process-list" => state = State::Root,
                    "victim-list" => state = State::Root,
                    "keylock" | "pagelock" | "objectlock" | "ridlock" | "hobtlock"
                    | "xactlock" | "exchangeEvent" | "threadpool" | "metadata" => {
                        if let State::InResource(res) = std::mem::replace(&mut state, State::ResourceList) {
                            resources.push(res);
                        }
                    }
                    "resource-list" => state = State::Root,
                    _ => {}
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(_) => break,
            _ => {}
        }
    }

    // Mark victims
    for proc in &mut processes {
        if victim_ids.contains(&proc.id) {
            proc.is_victim = true;
        }
    }

    if processes.is_empty() {
        return None;
    }

    // Clean GUID-prefixed names (e.g. "13de662b-...-5a5005f15f11.dbo.SP_Name" → "dbo.SP_Name")
    fn strip_guid_prefix(name: &mut String) {
        if name.len() > 37 && name.as_bytes()[8] == b'-' && name.as_bytes()[36] == b'.' {
            *name = name[37..].to_string();
        }
    }
    for proc in &mut processes {
        for frame in &mut proc.execution_stack {
            if let Some(ref mut name) = frame.proc_name {
                strip_guid_prefix(name);
            }
        }
    }
    for res in &mut resources {
        if let Some(ref mut name) = res.object_name {
            strip_guid_prefix(name);
        }
    }

    // Collapse parallel worker threads (ecid > 0) into the coordinator (ecid == 0).
    // SQL Server parallel queries create many process entries for the same session —
    // showing 80+ identical entries is noise. Keep the coordinator and summarize workers.
    let has_parallel = processes.iter().any(|p| p.ecid.unwrap_or(0) > 0);
    let processes = if has_parallel {
        let mut coordinator_map: HashMap<i64, usize> = HashMap::new(); // spid → index in collapsed
        let mut collapsed: Vec<DeadlockProcess> = Vec::new();

        // First pass: collect coordinators (ecid == 0)
        for proc in &processes {
            let ecid = proc.ecid.unwrap_or(0);
            if ecid == 0 {
                if let Some(spid) = proc.spid {
                    coordinator_map.insert(spid, collapsed.len());
                }
                collapsed.push(proc.clone());
            }
        }

        // Second pass: merge workers into coordinators
        for proc in &processes {
            let ecid = proc.ecid.unwrap_or(0);
            if ecid == 0 { continue; }
            let spid = proc.spid.unwrap_or(-1);
            if let Some(&coord_idx) = coordinator_map.get(&spid) {
                let coord = &mut collapsed[coord_idx];
                // Take wait_resource from worker if coordinator doesn't have one
                if coord.wait_resource.is_none() && proc.wait_resource.is_some() {
                    coord.wait_resource = proc.wait_resource.clone();
                    coord.wait_time_ms = proc.wait_time_ms;
                    coord.lock_mode = proc.lock_mode.clone();
                }
                // Track max ecid as parallel thread count
                let current = coord.ecid.unwrap_or(0);
                coord.ecid = Some(current.max(ecid));
            } else {
                // Orphan worker without coordinator — keep it
                collapsed.push(proc.clone());
            }
        }

        // Also update victim flags — a worker thread may be the victim
        for proc in &processes {
            if proc.is_victim {
                if let Some(spid) = proc.spid {
                    if let Some(&coord_idx) = coordinator_map.get(&spid) {
                        collapsed[coord_idx].is_victim = true;
                    }
                }
            }
        }

        collapsed
    } else {
        processes
    };

    Some(ParsedDeadlockGraph {
        event_id,
        timestamp,
        processes,
        resources,
    })
}

/// Build the blocking chain from parsed BPRs
fn build_blocking_chain(
    bprs: &[ParsedBlockedProcessReport],
    events: &[XelEvent],
    involved_sessions: &HashSet<i64>,
    anchor_ts: chrono::DateTime<chrono::Utc>,
    window: chrono::Duration,
) -> Vec<BlockingChainLink> {
    // Build a map: session_id -> who blocks it
    let mut blocked_by: HashMap<i64, i64> = HashMap::new();
    let mut session_info: HashMap<i64, (&ParsedBlockedProcessReport, bool)> = HashMap::new(); // (bpr, is_blocked)

    for bpr in bprs {
        if let (Some(victim), Some(blocker)) = (bpr.blocked_spid, bpr.blocking_spid) {
            blocked_by.insert(victim, blocker);
            session_info.entry(victim).or_insert((bpr, true));
            session_info.entry(blocker).or_insert((bpr, false));
        }
    }

    // Find root blockers (sessions that block others but are not blocked themselves)
    let all_blockers: HashSet<i64> = bprs.iter().filter_map(|b| b.blocking_spid).collect();
    let all_victims: HashSet<i64> = bprs.iter().filter_map(|b| b.blocked_spid).collect();

    let mut chain: Vec<BlockingChainLink> = Vec::new();

    // Process each session
    for &sid in involved_sessions {
        let role = if all_blockers.contains(&sid) && !all_victims.contains(&sid) {
            "root_blocker"
        } else if all_blockers.contains(&sid) && all_victims.contains(&sid) {
            "intermediate"
        } else if all_victims.contains(&sid) {
            "victim"
        } else {
            continue;
        };

        let (wait_resource, lock_mode, sql_preview, app_name, username, database,
             hostname, status, isolation_level, tran_count, last_batch_started, wait_time_ms, xact_id, execution_stack) =
            if let Some((bpr, is_blocked)) = session_info.get(&sid) {
                if *is_blocked {
                    (
                        bpr.blocked_wait_resource.clone(),
                        bpr.blocked_lock_mode.clone(),
                        bpr.blocked_input_buffer.clone(),
                        bpr.blocked_app_name.clone(),
                        bpr.blocked_login_name.clone(),
                        bpr.blocked_database.clone(),
                        bpr.blocked_hostname.clone(),
                        bpr.blocked_status.clone(),
                        bpr.blocked_isolation_level.clone(),
                        bpr.blocked_tran_count,
                        bpr.blocked_last_batch_started.clone(),
                        bpr.blocked_wait_time_ms,
                        bpr.blocked_xact_id.clone(),
                        bpr.blocked_execution_stack.clone(),
                    )
                } else {
                    (
                        None,
                        None,
                        bpr.blocking_input_buffer.clone(),
                        bpr.blocking_app_name.clone(),
                        bpr.blocking_login_name.clone(),
                        bpr.blocking_database.clone(),
                        bpr.blocking_hostname.clone(),
                        bpr.blocking_status.clone(),
                        bpr.blocking_isolation_level.clone(),
                        bpr.blocking_tran_count,
                        bpr.blocking_last_batch_started.clone(),
                        None, // blocker has no wait_time
                        bpr.blocking_xact_id.clone(),
                        bpr.blocking_execution_stack.clone(),
                    )
                }
            } else {
                (None, None, None, None, None, None, None, None, None, None, None, None, None, Vec::new())
            };

        // Find event IDs from this session in time window
        let event_ids: Vec<u64> = events
            .iter()
            .filter(|e| {
                e.session_id == Some(sid)
                    && (e.timestamp - anchor_ts).num_milliseconds().abs()
                        <= window.num_milliseconds()
            })
            .take(20)
            .map(|e| e.id)
            .collect();

        chain.push(BlockingChainLink {
            session_id: sid,
            role: role.to_string(),
            wait_resource,
            lock_mode,
            sql_preview,
            app_name,
            username,
            database,
            event_ids,
            blocked_by_session: blocked_by.get(&sid).copied(),
            hostname,
            status,
            isolation_level,
            tran_count,
            last_batch_started,
            wait_time_ms,
            xact_id,
            execution_stack,
        });
    }

    // Sort: root blockers first, then intermediates, then victims
    chain.sort_by_key(|link| match link.role.as_str() {
        "root_blocker" => 0,
        "intermediate" => 1,
        "victim" => 2,
        _ => 3,
    });

    chain
}

/// Categorize a wait type into a high-level category
fn categorize_wait_type(wait_type: &str) -> &'static str {
    let wt = wait_type.to_uppercase();
    if wt.starts_with("PAGEIOLATCH") || wt.starts_with("WRITELOG") || wt.starts_with("IO_COMPLETION")
        || wt.starts_with("ASYNC_IO") || wt == "LOGBUFFER"
    {
        "io"
    } else if wt.starts_with("LCK_M_") || wt.starts_with("LOCK") {
        "lock"
    } else if wt.starts_with("PAGELATCH") || wt.starts_with("LATCH_") {
        "latch"
    } else if wt.starts_with("ASYNC_NETWORK") || wt.starts_with("NET_") || wt == "OLEDB" {
        "network"
    } else if wt.starts_with("CXPACKET") || wt.starts_with("CXCONSUMER") || wt == "SOS_SCHEDULER_YIELD" || wt == "THREADPOOL" {
        "cpu"
    } else if wt.starts_with("RESOURCE_SEMAPHORE") || wt.starts_with("CMEMTHREAD") {
        "memory"
    } else if wt == "SLEEP_TASK" || wt.starts_with("WAITFOR") || wt.starts_with("BROKER") {
        "idle"
    } else {
        "other"
    }
}

/// Aggregate wait events into per-type statistics
fn aggregate_wait_stats(wait_events: &[XelEvent]) -> Vec<WaitTypeStat> {
    let mut stats: HashMap<String, (usize, i64, i64)> = HashMap::new(); // (count, total_dur, max_dur)

    for ev in wait_events {
        // For lock_acquired events, synthesize a wait type from lock_mode (e.g. "LCK_M_IX")
        // to match the SQL Server wait type convention
        let wt = if ev.event_name == "lock_acquired" {
            if let Some(ref mode) = ev.lock_mode {
                format!("LCK_M_{}", mode)
            } else {
                "LCK_M_UNKNOWN".to_string()
            }
        } else {
            ev.wait_type.as_deref().unwrap_or("UNKNOWN").to_string()
        };
        let dur = ev.duration_us.unwrap_or(0);
        let entry = stats.entry(wt).or_insert((0, 0, 0));
        entry.0 += 1;
        entry.1 += dur;
        entry.2 = entry.2.max(dur);
    }

    let mut result: Vec<WaitTypeStat> = stats
        .into_iter()
        .map(|(wt, (count, total, max))| {
            let avg = if count > 0 { total / count as i64 } else { 0 };
            let category = categorize_wait_type(&wt).to_string();
            WaitTypeStat {
                wait_type: wt,
                count,
                total_duration_us: total,
                max_duration_us: max,
                avg_duration_us: avg,
                category,
            }
        })
        .collect();

    // Sort by total duration descending
    result.sort_by(|a, b| b.total_duration_us.cmp(&a.total_duration_us));
    result
}

/// Diagnose the root cause from wait patterns
fn diagnose_wait_pattern(
    anchor: &XelEvent,
    bprs: &[ParsedBlockedProcessReport],
    wait_stats: &[WaitTypeStat],
) -> String {
    // If we have BPRs, it's lock blocking
    if !bprs.is_empty() {
        return "lock_blocking".to_string();
    }

    let total_wait_dur: i64 = wait_stats.iter().map(|w| w.total_duration_us).sum();
    let total_wait_count: usize = wait_stats.iter().map(|w| w.count).sum();

    if total_wait_dur == 0 && total_wait_count == 0 {
        if anchor.duration_us.unwrap_or(0) > anchor.cpu_time_us.unwrap_or(0) * 2 {
            return "unknown_wait".to_string();
        }
        return "no_waits".to_string();
    }

    // If the query is CPU-bound (CPU ≈ duration), waits aren't the bottleneck.
    // lock_acquired durations may overlap with parallel CPU execution — the query
    // was actively working the whole time, so these aren't true blocking waits.
    let anchor_dur = anchor.duration_us.unwrap_or(0);
    let anchor_cpu = anchor.cpu_time_us.unwrap_or(0);
    if anchor_dur > 0 && anchor_cpu > 0 {
        let gap = anchor_dur.saturating_sub(anchor_cpu);
        let gap_pct = gap as f64 / anchor_dur as f64;
        // Less than 15% idle time means the query was CPU-bound.
        // Also check that total wait time exceeds the gap — if waits fit within
        // the gap they could still be the bottleneck for that portion.
        if gap_pct < 0.15 && total_wait_dur > gap {
            return "no_waits".to_string();
        }
    }

    // Compute per-category scores using BOTH duration and count.
    // XEvent sessions often sample wait_completed events, so captured durations
    // may represent only a fraction of actual wait time. Count is a better
    // indicator of the dominant pattern when total captured duration is small
    // relative to the wall-clock wait gap.
    let duration_gap = anchor.duration_us.unwrap_or(0) - anchor.cpu_time_us.unwrap_or(0);
    let captured_fraction = if duration_gap > 0 {
        total_wait_dur as f64 / duration_gap as f64
    } else {
        1.0
    };
    // If captured waits explain < 10% of the gap, count-based scoring dominates
    let count_weight = if captured_fraction < 0.1 { 0.8 } else { 0.3 };
    let dur_weight = 1.0 - count_weight;

    struct CatScore {
        dur: i64,
        count: usize,
    }
    let mut cats: HashMap<&str, CatScore> = HashMap::new();

    for ws in wait_stats {
        let cat = ws.category.as_str();
        // Skip idle waits (SLEEP_TASK etc) from scoring
        if cat == "idle" {
            continue;
        }
        let entry = cats.entry(match cat {
            "io" => "io",
            "lock" => "lock",
            "latch" => "latch",
            "network" => "network",
            "memory" => "memory",
            "cpu" => "cpu",
            _ => "other",
        }).or_insert(CatScore { dur: 0, count: 0 });
        entry.dur += ws.total_duration_us;
        entry.count += ws.count;
    }

    let total_count_no_idle: usize = cats.values().map(|c| c.count).sum();
    let total_dur_no_idle: i64 = cats.values().map(|c| c.dur).sum();

    if total_count_no_idle == 0 {
        return "no_waits".to_string();
    }

    // Score each category: weighted combination of duration% and count%
    let mut best_score = 0.0_f64;
    let mut best_cat = "mixed";

    for (&cat_name, score) in &cats {
        let dur_pct = if total_dur_no_idle > 0 {
            score.dur as f64 / total_dur_no_idle as f64
        } else {
            0.0
        };
        let count_pct = score.count as f64 / total_count_no_idle as f64;
        let combined = dur_pct * dur_weight + count_pct * count_weight;

        if combined > best_score {
            best_score = combined;
            best_cat = cat_name;
        }
    }

    match best_cat {
        "io" => "io_starvation",
        "lock" => "lock_contention",
        "latch" => "latch_contention",
        "network" => "network_bottleneck",
        "memory" => "memory_pressure",
        "cpu" => "cpu_pressure",
        _ => "mixed",
    }.to_string()
}

/// Build actionable recommendations based on diagnosis
fn build_recommendations(
    anchor: &XelEvent,
    diagnosis: &str,
    wait_stats: &[WaitTypeStat],
    deadlocks: &[ParsedDeadlockGraph],
    deadlock_id: Option<i64>,
    deadlock_lock_events: &[XelEvent],
    bprs: &[ParsedBlockedProcessReport],
    blocker_events: &[XelEvent],
    has_snapshot_isolation: bool,
) -> Vec<String> {
    let mut recs = Vec::new();
    let has_xml_graph = !deadlocks.is_empty();
    let has_lock_events = deadlock_id.is_some() && !deadlock_lock_events.is_empty();

    match diagnosis {
        "deadlock" => {
            // Find other sessions involved in the deadlock and show their details
            let anchor_sid = anchor.session_id;
            let mut other_sids: Vec<i64> = Vec::new();

            if has_xml_graph {
                for dl in deadlocks {
                    for p in &dl.processes {
                        if let Some(spid) = p.spid {
                            if Some(spid) != anchor_sid && !other_sids.contains(&spid) {
                                other_sids.push(spid);
                            }
                        }
                    }
                }
            }
            if has_lock_events {
                for ev in deadlock_lock_events {
                    if let Some(sid) = ev.session_id {
                        if Some(sid) != anchor_sid && !other_sids.contains(&sid) {
                            other_sids.push(sid);
                        }
                    }
                }
            }

            // Show other participants with SQL and lock details
            for &sid in &other_sids {
                let mut sql_preview: Option<String> = None;
                let mut obj_name: Option<String> = None;
                let mut lock_info: Vec<String> = Vec::new();
                let mut app_name: Option<String> = None;
                let mut user_name: Option<String> = None;

                for ev in deadlock_lock_events.iter() {
                    if ev.session_id == Some(sid) {
                        if sql_preview.is_none() {
                            sql_preview = ev.statement.clone().or_else(|| ev.sql_text.clone());
                        }
                        if obj_name.is_none() {
                            obj_name = ev.object_name.clone()
                                .or_else(|| ev.extra_fields.get("resolved_object").and_then(|v| v.as_str()).map(String::from))
                                .or_else(|| ev.extra_fields.get("resolved_wait_object").and_then(|v| v.as_str()).map(String::from));
                        }
                        if app_name.is_none() { app_name = ev.client_app_name.clone(); }
                        if user_name.is_none() { user_name = ev.username.clone(); }
                        if let (Some(ref rt), Some(ref lm)) = (&ev.resource_type, &ev.lock_mode) {
                            let info = format!("{} {}", lm, rt);
                            if !lock_info.contains(&info) { lock_info.push(info); }
                        }
                    }
                }
                // Check XML graph for input buffers
                if sql_preview.is_none() {
                    for dl in deadlocks {
                        for p in &dl.processes {
                            if p.spid == Some(sid) {
                                if sql_preview.is_none() { sql_preview = p.input_buffer.clone(); }
                                if app_name.is_none() { app_name = p.app_name.clone(); }
                                if user_name.is_none() { user_name = p.login_name.clone(); }
                            }
                        }
                    }
                }

                let mut desc = format!("Other participant: Session {}", sid);
                if let Some(ref u) = user_name { desc.push_str(&format!(" ({})", u)); }
                if let Some(ref a) = app_name { desc.push_str(&format!(" [{}]", a)); }
                if let Some(ref obj) = obj_name { desc.push_str(&format!(" on [{}]", obj)); }
                if !lock_info.is_empty() { desc.push_str(&format!(" — locks: {}", lock_info.join(", "))); }
                if let Some(ref sql) = sql_preview {
                    // Clean up raw "Proc [Database Id = X Object Id = Y (schema.name)]" to just "Proc [schema.name]"
                    let cleaned = if let (Some(start), Some(end)) = (sql.find('('), sql.rfind(')')) {
                        if sql.contains("Database Id") && sql.contains("Object Id") && start < end {
                            let proc_name = &sql[start + 1..end];
                            format!("Proc [{}]", proc_name)
                        } else {
                            sql.chars().take(200).collect()
                        }
                    } else {
                        sql.chars().take(200).collect()
                    };
                    desc.push_str(&format!("\nSQL: {}", cleaned));
                }
                recs.push(desc);
            }

            // Show contended resources from lock events
            let mut resource_info: Vec<String> = Vec::new();
            for ev in deadlock_lock_events {
                if let (Some(ref rt), Some(ref lm)) = (&ev.resource_type, &ev.lock_mode) {
                    let obj = ev.object_name.as_deref().or_else(||
                        ev.extra_fields.get("resolved_object").and_then(|v| v.as_str())
                            .or_else(|| ev.extra_fields.get("resolved_wait_object").and_then(|v| v.as_str()))
                    );
                    let desc = if let Some(o) = obj {
                        format!("S{}: {} {} on [{}]", ev.session_id.unwrap_or(0), lm, rt, o)
                    } else {
                        format!("S{}: {} {}", ev.session_id.unwrap_or(0), lm, rt)
                    };
                    if !resource_info.contains(&desc) {
                        resource_info.push(desc);
                    }
                }
            }
            if !resource_info.is_empty() {
                recs.push(format!("Lock requests at deadlock: {}", resource_info.join(" | ")));
            }

            // Show contended resources from XML graph — deduplicated and aggregated
            if has_xml_graph {
                use std::collections::HashMap;
                // Key: (resource_type, object_name, index_name) → count + unique lock mode combos
                let mut res_agg: HashMap<(String, String, String), (usize, Vec<String>)> = HashMap::new();
                for dl in deadlocks {
                    for res in &dl.resources {
                        if res.resource_type == "exchangeEvent" { continue; }
                        let obj = res.object_name.as_deref().unwrap_or("").to_string();
                        let idx = res.index_name.as_deref().unwrap_or("").to_string();
                        let key = (res.resource_type.clone(), obj, idx);

                        let mut modes = String::new();
                        let holders: Vec<String> = res.holders.iter().map(|h| format!("held {}", h.mode.as_deref().unwrap_or("?"))).collect();
                        let waiters: Vec<String> = res.waiters.iter().map(|w| format!("waited {}", w.mode.as_deref().unwrap_or("?"))).collect();
                        if !holders.is_empty() || !waiters.is_empty() {
                            modes = format!("{} / {}", holders.join(", "), waiters.join(", "));
                        }

                        let entry = res_agg.entry(key).or_insert_with(|| (0, Vec::new()));
                        entry.0 += 1;
                        if !modes.is_empty() && !entry.1.contains(&modes) {
                            entry.1.push(modes);
                        }
                    }
                }

                for ((rtype, obj, idx), (count, mode_combos)) in &res_agg {
                    let mut desc = rtype.clone();
                    if !obj.is_empty() { desc.push_str(&format!(" on [{}]", obj)); }
                    if !idx.is_empty() { desc.push_str(&format!(" ({})", idx)); }
                    if !mode_combos.is_empty() {
                        desc.push_str(&format!(" — {}", mode_combos.join("; ")));
                    }
                    if *count > 1 {
                        desc.push_str(&format!(" ({}x)", count));
                    }
                    recs.push(desc);
                }
            }

            // Check if MERGE is involved
            let anchor_sql = anchor.statement.as_deref()
                .or(anchor.sql_text.as_deref())
                .unwrap_or("");
            let has_merge_anchor = anchor_sql.to_uppercase().contains("MERGE");
            let has_merge_xml = deadlocks.iter().any(|dl| {
                dl.processes.iter().any(|p| {
                    p.input_buffer.as_ref().map_or(false, |buf| buf.to_uppercase().contains("MERGE"))
                })
            });
            if has_merge_anchor || has_merge_xml {
                recs.push("MERGE acquires multiple lock types (S, U, X) in unpredictable order — common deadlock source. Consider replacing with separate IF EXISTS/UPDATE/INSERT.".to_string());
            }

            // Check for parallelism deadlock
            let has_exchange = deadlocks.iter().any(|dl| {
                dl.resources.iter().any(|r| r.resource_type.contains("exchange") || r.resource_type.contains("communication"))
            });
            if has_exchange {
                recs.push("Parallelism deadlock — parallel query plans deadlocked internally. Try OPTION(MAXDOP 1).".to_string());
            }
        }
        "likely_deadlock" => {
            recs.push("Likely DEADLOCK VICTIM — Error result with non-timeout duration pattern.".to_string());
        }
        "io_starvation" => {
            let pageio_count: usize = wait_stats.iter()
                .filter(|w| w.wait_type.starts_with("PAGEIOLATCH"))
                .map(|w| w.count)
                .sum();

            if let Some(reads) = anchor.logical_reads {
                if reads > 100_000 {
                    recs.push(format!(
                        "Very high logical reads ({}) — check execution plan for table/index scans. Missing indexes are the most common cause.",
                        reads
                    ));
                }
            }
            if pageio_count > 50 {
                recs.push(format!(
                    "{} PAGEIOLATCH waits — data is being read from disk instead of memory. Server may need more RAM, or the query accesses too much data.",
                    pageio_count
                ));
            }
            recs.push("Check sys.dm_io_virtual_file_stats for disk latency. SSD storage dramatically reduces PAGEIOLATCH waits.".to_string());
            recs.push("Run the query's execution plan through SQL Plan For Dummies to find expensive operators.".to_string());
        }
        "lock_blocking" | "lock_contention" => {
            // Show BPR-derived blocking details
            for bpr in bprs {
                let blocked = bpr.blocked_spid.map_or("?".to_string(), |s| s.to_string());
                let blocker = bpr.blocking_spid.map_or("?".to_string(), |s| s.to_string());
                let mut desc = format!("Session {} blocked by Session {}", blocked, blocker);
                if let Some(ref res) = bpr.blocked_wait_resource {
                    desc.push_str(&format!(" on {}", res));
                }
                if let Some(ref lm) = bpr.blocked_lock_mode {
                    desc.push_str(&format!(" (lock: {})", lm));
                }
                if let Some(wt) = bpr.blocked_wait_time_ms {
                    desc.push_str(&format!(" — waiting {}ms", wt));
                }
                recs.push(desc);

                if let Some(ref sql) = bpr.blocking_input_buffer {
                    let preview: String = sql.chars().take(200).collect();
                    recs.push(format!("Blocker (S{}) SQL: {}", blocker, preview));
                }
                if let Some(ref sql) = bpr.blocked_input_buffer {
                    let preview: String = sql.chars().take(200).collect();
                    recs.push(format!("Victim (S{}) SQL: {}", blocked, preview));
                }
                // Show execution stack if available
                for frame in &bpr.blocked_execution_stack {
                    if let Some(ref qh) = frame.query_hash {
                        let mut stack_info = format!("Victim query hash: {}", qh);
                        if let Some(ref ph) = frame.query_plan_hash {
                            stack_info.push_str(&format!(", plan hash: {}", ph));
                        }
                        if let Some(line) = frame.line {
                            stack_info.push_str(&format!(", line {}", line));
                        }
                        recs.push(stack_info);
                    }
                }
                for frame in &bpr.blocking_execution_stack {
                    if let Some(ref qh) = frame.query_hash {
                        let mut stack_info = format!("Blocker query hash: {}", qh);
                        if let Some(ref ph) = frame.query_plan_hash {
                            stack_info.push_str(&format!(", plan hash: {}", ph));
                        }
                        if let Some(line) = frame.line {
                            stack_info.push_str(&format!(", line {}", line));
                        }
                        recs.push(stack_info);
                    }
                }
            }
            // When no BPRs but lock waits detected (lock_contention without blocked_process_report)
            if bprs.is_empty() {
                let lock_waits: Vec<&WaitTypeStat> = wait_stats.iter().filter(|w| w.category == "lock").collect();
                let total_lock_dur: i64 = lock_waits.iter().map(|w| w.total_duration_us).sum();
                let total_lock_count: usize = lock_waits.iter().map(|w| w.count).sum();
                recs.push(format!(
                    "{} lock wait(s) totaling {:.1}s detected. The query is competing for locks with other sessions.",
                    total_lock_count, total_lock_dur as f64 / 1_000_000.0
                ));
                for ws in &lock_waits {
                    recs.push(format!(
                        "Wait type {} — {} occurrence(s), total {:.1}s, max {:.1}ms.",
                        ws.wait_type, ws.count, ws.total_duration_us as f64 / 1_000_000.0, ws.max_duration_us as f64 / 1000.0
                    ));
                }
                if let Some(reads) = anchor.logical_reads {
                    if reads > 100_000 {
                        recs.push(format!(
                            "High logical reads ({}) amplify lock hold time — the query scans too much data, holding locks longer. Optimize with better indexes.",
                            reads
                        ));
                    }
                }
                // Show correlated sessions that touched the same object
                if !blocker_events.is_empty() {
                    let anchor_sid = anchor.session_id;
                    // Gather per-session info
                    struct SessionInfo {
                        lock_wait_count: usize,
                        max_lock_wait_us: i64,
                        total_lock_wait_us: i64,
                        proc_name: Option<String>,
                        sql_preview: Option<String>,
                        app_name: Option<String>,
                        username: Option<String>,
                    }
                    let mut session_map: HashMap<i64, SessionInfo> = HashMap::new();
                    for ev in blocker_events {
                        if let Some(sid) = ev.session_id {
                            if Some(sid) == anchor_sid { continue; }
                            let info = session_map.entry(sid).or_insert(SessionInfo {
                                lock_wait_count: 0,
                                max_lock_wait_us: 0,
                                total_lock_wait_us: 0,
                                proc_name: None,
                                sql_preview: None,
                                app_name: None,
                                username: None,
                            });
                            // Count lock_acquired events (these mean the session was waiting for locks too)
                            if ev.event_name == "lock_acquired" || ev.event_name == "locks_lock_waits" {
                                if let Some(dur) = ev.duration_us {
                                    if dur > 0 {
                                        info.lock_wait_count += 1;
                                        info.total_lock_wait_us += dur;
                                        if dur > info.max_lock_wait_us {
                                            info.max_lock_wait_us = dur;
                                        }
                                    }
                                }
                            }
                            if info.proc_name.is_none() {
                                if ev.event_name == "rpc_completed" || ev.event_name == "sql_batch_completed" {
                                    info.proc_name = ev.object_name.clone();
                                    info.sql_preview = ev.statement.clone().or_else(|| ev.sql_text.clone());
                                }
                            }
                            if info.app_name.is_none() { info.app_name = ev.client_app_name.clone(); }
                            if info.username.is_none() { info.username = ev.username.clone(); }
                        }
                    }

                    let session_count = session_map.len();
                    // Check if multiple sessions are all experiencing lock waits (hot table pattern)
                    let sessions_also_waiting: usize = session_map.values()
                        .filter(|i| i.lock_wait_count > 0)
                        .count();

                    if sessions_also_waiting > 1 {
                        recs.push(format!(
                            "{} other sessions were also waiting for locks on the same object — this is a hot table contention pattern where many concurrent operations compete for the same resource.",
                            sessions_also_waiting
                        ));
                    } else if session_count > 0 {
                        recs.push(format!(
                            "{} other session(s) were accessing the same object concurrently.",
                            session_count
                        ));
                    }

                    // Show per-session details (sorted by total lock wait descending)
                    let mut sorted_sessions: Vec<(i64, &SessionInfo)> = session_map.iter().map(|(&k, v)| (k, v)).collect();
                    sorted_sessions.sort_by(|a, b| b.1.total_lock_wait_us.cmp(&a.1.total_lock_wait_us));

                    for (sid, info) in sorted_sessions.iter().take(5) {
                        let mut desc = format!("Session {}", sid);
                        if let Some(ref user) = info.username {
                            desc.push_str(&format!(" ({})", user));
                        }
                        if let Some(ref app) = info.app_name {
                            desc.push_str(&format!(" [{}]", app));
                        }
                        if info.lock_wait_count > 0 {
                            desc.push_str(&format!(
                                " — also waiting for locks: {} wait(s), {:.1}s total, max {:.1}s",
                                info.lock_wait_count,
                                info.total_lock_wait_us as f64 / 1_000_000.0,
                                info.max_lock_wait_us as f64 / 1_000_000.0
                            ));
                        }
                        if let Some(ref proc_name) = info.proc_name {
                            desc.push_str(&format!("\n  Proc: {}", proc_name));
                        }
                        if let Some(ref sql) = info.sql_preview {
                            let preview: String = sql.chars().take(120).collect();
                            desc.push_str(&format!("\n  SQL: {}", preview));
                        }
                        recs.push(desc);
                    }
                    if sorted_sessions.len() > 5 {
                        recs.push(format!("...and {} more sessions.", sorted_sessions.len() - 5));
                    }
                }
                if !has_snapshot_isolation {
                    recs.push("Consider enabling Read Committed Snapshot Isolation (RCSI) to reduce reader-writer lock conflicts.".to_string());
                }
                recs.push("Check for long-running transactions that may be holding locks. Use sp_whoisactive or sys.dm_tran_active_transactions.".to_string());
            }
        }
        "latch_contention" => {
            recs.push("PAGELATCH waits indicate in-memory contention on hot pages (last page insert, tempdb allocation).".to_string());
            recs.push("Consider using OPTIMIZE_FOR_SEQUENTIAL_KEY on clustered indexes with identity columns (SQL Server 2019+).".to_string());
        }
        "network_bottleneck" => {
            recs.push("ASYNC_NETWORK_IO waits mean the client is not consuming results fast enough.".to_string());
            recs.push("Check if the query returns too many rows/columns, or if the application processes results slowly.".to_string());
        }
        "memory_pressure" => {
            recs.push("RESOURCE_SEMAPHORE waits mean queries are queuing for memory grants.".to_string());
            recs.push("Check for large sorts/hash joins in the execution plan. Consider adding indexes to avoid sorts.".to_string());
        }
        "cpu_pressure" => {
            recs.push("CXPACKET/SOS_SCHEDULER_YIELD waits indicate CPU bottleneck or excessive parallelism.".to_string());
            recs.push("Review MAXDOP settings and consider query-level OPTION(MAXDOP N) hints.".to_string());
        }
        _ => {
            if let (Some(dur), Some(cpu)) = (anchor.duration_us, anchor.cpu_time_us) {
                if dur > cpu * 3 {
                    recs.push(format!(
                        "Duration ({}ms) is {}x CPU time ({}ms) — the query spent most time waiting, not computing.",
                        dur / 1000, dur / cpu.max(1), cpu / 1000
                    ));
                }
            }
        }
    }

    // Add secondary recommendations if multiple wait categories are significant
    let total_count: usize = wait_stats.iter().map(|w| w.count).sum();
    if total_count > 0 {
        let io_count: usize = wait_stats.iter().filter(|w| w.category == "io").map(|w| w.count).sum();
        let lock_count: usize = wait_stats.iter().filter(|w| w.category == "lock").map(|w| w.count).sum();

        // If diagnosis is not IO but IO waits are significant by count
        if diagnosis != "io_starvation" && io_count as f64 / total_count as f64 > 0.3 {
            if let Some(reads) = anchor.logical_reads {
                if reads > 50_000 {
                    recs.push(format!(
                        "Also: {} IO waits with {} logical reads — disk IO is a contributing factor. Check for missing indexes.",
                        io_count, reads
                    ));
                }
            }
        }
        // If diagnosis is not lock but lock waits are present.
        // Skip when "no_waits" — the lock durations overlapped with CPU execution
        // and didn't actually block the query.
        if diagnosis != "lock_contention" && diagnosis != "lock_blocking" && diagnosis != "no_waits" && lock_count > 0 {
            let lock_dur: i64 = wait_stats.iter().filter(|w| w.category == "lock").map(|w| w.total_duration_us).sum();
            if lock_dur > 10_000 { // > 10ms
                recs.push(format!(
                    "Also: {} lock wait(s) totaling {:.1}ms — minor lock contention detected.",
                    lock_count, lock_dur as f64 / 1000.0
                ));
            }
        }
    }

    // Always add high logical reads recommendation regardless of diagnosis
    if recs.iter().all(|r| !r.contains("logical reads")) {
        if let Some(reads) = anchor.logical_reads {
            if reads > 100_000 {
                recs.push(format!(
                    "High logical reads ({}) — review the execution plan for scans that could be converted to seeks with better indexes.",
                    reads
                ));
            }
        }
    }

    recs
}

/// Build comprehensive analysis summary
fn build_analysis_summary(
    anchor: &XelEvent,
    bprs: &[ParsedBlockedProcessReport],
    chain: &[BlockingChainLink],
    wait_stats: &[WaitTypeStat],
    diagnosis: &str,
    deadlocks: &[ParsedDeadlockGraph],
    deadlock_id: Option<i64>,
    deadlock_lock_events: &[XelEvent],
) -> String {
    let sid = anchor.session_id.map_or("-".to_string(), |s| s.to_string());
    let mut parts: Vec<String> = Vec::new();
    let has_xml_graph = !deadlocks.is_empty();
    let has_lock_events = deadlock_id.is_some() && !deadlock_lock_events.is_empty();

    // Diagnosis headline
    match diagnosis {
        "deadlock" => {
            if has_xml_graph {
                let dl_count = deadlocks.len();
                let victim_count: usize = deadlocks.iter()
                    .flat_map(|dl| dl.processes.iter())
                    .filter(|p| p.is_victim)
                    .count();
                let process_count: usize = deadlocks.iter().map(|dl| dl.processes.len()).sum();
                parts.push(format!(
                    "DEADLOCK: {} deadlock(s) detected involving {} process(es), {} victim(s). Session {} was involved.",
                    dl_count, process_count, victim_count, sid
                ));
                let resource_types: HashSet<String> = deadlocks.iter()
                    .flat_map(|dl| dl.resources.iter())
                    .filter(|r| r.resource_type != "exchangeEvent")
                    .map(|r| r.resource_type.clone())
                    .collect();
                let exchange_count: usize = deadlocks.iter()
                    .flat_map(|dl| dl.resources.iter())
                    .filter(|r| r.resource_type == "exchangeEvent")
                    .count();
                if !resource_types.is_empty() {
                    let types: Vec<String> = resource_types.into_iter().collect();
                    let suffix = if exchange_count > 0 {
                        format!(" (+ {} parallel exchange waits)", exchange_count)
                    } else { String::new() };
                    parts.push(format!("Contended resources: {}.{}", types.join(", "), suffix));
                }
                // Show contended object/table names from resources
                let contended_objects: HashSet<String> = deadlocks.iter()
                    .flat_map(|dl| dl.resources.iter())
                    .filter(|r| r.resource_type != "exchangeEvent")
                    .filter_map(|r| r.object_name.clone())
                    .collect();
                if !contended_objects.is_empty() {
                    let objs: Vec<String> = contended_objects.into_iter().collect();
                    parts.push(format!("Tables involved: {}.", objs.join(", ")));
                }
                // Show SP names from execution stacks
                let proc_names: HashSet<String> = deadlocks.iter()
                    .flat_map(|dl| dl.processes.iter())
                    .flat_map(|p| p.execution_stack.iter())
                    .filter_map(|f| f.proc_name.clone())
                    .collect();
                if !proc_names.is_empty() {
                    let procs: Vec<String> = proc_names.into_iter().collect();
                    parts.push(format!("Stored procedures: {}.", procs.join(", ")));
                }
            } else if has_lock_events {
                let did = deadlock_id.unwrap();
                // Find other sessions in the deadlock
                let other_sids: Vec<i64> = deadlock_lock_events.iter()
                    .filter_map(|e| e.session_id)
                    .filter(|s| Some(*s) != anchor.session_id)
                    .collect::<HashSet<_>>()
                    .into_iter()
                    .collect();

                if other_sids.is_empty() {
                    parts.push(format!(
                        "DEADLOCK (deadlock_id: {}). Session {} was the victim.",
                        did, sid
                    ));
                } else {
                    parts.push(format!(
                        "DEADLOCK (deadlock_id: {}). Session {} (victim) vs Session {} (other participant).",
                        did, sid, other_sids.iter().map(|s| s.to_string()).collect::<Vec<_>>().join(", ")
                    ));
                }

                // Show lock details
                let lock_modes: HashSet<String> = deadlock_lock_events.iter()
                    .filter_map(|e| e.lock_mode.clone())
                    .collect();
                let resource_types: HashSet<String> = deadlock_lock_events.iter()
                    .filter_map(|e| e.resource_type.clone())
                    .collect();
                let objects: HashSet<String> = deadlock_lock_events.iter()
                    .filter_map(|e| e.object_name.clone()
                        .or_else(|| e.extra_fields.get("resolved_object").and_then(|v| v.as_str()).map(String::from)))
                    .collect();
                let mut details = Vec::new();
                if !objects.is_empty() {
                    details.push(format!("objects: {}", objects.into_iter().collect::<Vec<_>>().join(", ")));
                }
                if !resource_types.is_empty() {
                    details.push(format!("resource: {}", resource_types.into_iter().collect::<Vec<_>>().join(", ")));
                }
                if !lock_modes.is_empty() {
                    details.push(format!("lock mode: {}", lock_modes.into_iter().collect::<Vec<_>>().join(", ")));
                }
                if !details.is_empty() {
                    parts.push(format!("Deadlock on {}.", details.join(", ")));
                }
            }
        }
        "likely_deadlock" => {
            parts.push(format!(
                "Session {} was likely a DEADLOCK VICTIM. Error result with {:.1}s duration ({:.0}% waiting).",
                sid,
                anchor.duration_us.unwrap_or(0) as f64 / 1_000_000.0,
                anchor.cpu_time_us.map_or(0.0, |cpu| {
                    (1.0 - cpu as f64 / anchor.duration_us.unwrap_or(1) as f64) * 100.0
                })
            ));
        }
        "io_starvation" => {
            let total_io_us: i64 = wait_stats.iter()
                .filter(|w| w.category == "io")
                .map(|w| w.total_duration_us)
                .sum();
            let io_count: usize = wait_stats.iter()
                .filter(|w| w.category == "io")
                .map(|w| w.count)
                .sum();
            parts.push(format!(
                "Session {} is IO-bound: {} IO waits totaling {:.1}s. Data is being read from disk because it's not cached in the buffer pool.",
                sid, io_count, total_io_us as f64 / 1_000_000.0
            ));
        }
        "lock_blocking" => {
            // Use BPR data for summary (more accurate than chain for BPR anchor events)
            if let Some(bpr) = bprs.first() {
                let blocked = bpr.blocked_spid.map_or("-".to_string(), |s| s.to_string());
                let blocker = bpr.blocking_spid.map_or("-".to_string(), |s| s.to_string());
                parts.push(format!(
                    "Session {} is blocked by Session {}.",
                    blocked, blocker
                ));
                if let Some(ref res) = bpr.blocked_wait_resource {
                    parts.push(format!("Contended resource: {}", res));
                }
                if let Some(ref lm) = bpr.blocked_lock_mode {
                    parts.push(format!("Lock mode requested: {}", lm));
                }
                if let Some(wt) = bpr.blocked_wait_time_ms {
                    parts.push(format!("Wait time: {}ms.", wt));
                }
                if let Some(ref sql) = bpr.blocking_input_buffer {
                    let preview: String = sql.chars().take(150).collect();
                    parts.push(format!("Blocker SQL: {}", preview));
                }
                if let Some(ref sql) = bpr.blocked_input_buffer {
                    let preview: String = sql.chars().take(150).collect();
                    parts.push(format!("Victim SQL: {}", preview));
                }
            } else {
                let root_blockers: Vec<&BlockingChainLink> = chain.iter().filter(|l| l.role == "root_blocker").collect();
                if let Some(root) = root_blockers.first() {
                    parts.push(format!(
                        "Session {} is blocked by Session {} (root blocker).",
                        sid, root.session_id
                    ));
                    if let Some(ref sql) = root.sql_preview {
                        let preview: String = sql.chars().take(100).collect();
                        parts.push(format!("Blocker is running: {}", preview));
                    }
                }
            }
        }
        "lock_contention" => {
            let lock_waits: Vec<&WaitTypeStat> = wait_stats.iter().filter(|w| w.category == "lock").collect();
            if let Some(top) = lock_waits.first() {
                parts.push(format!(
                    "Session {} has lock contention: {} {} waits totaling {:.1}s.",
                    sid, top.count, top.wait_type, top.total_duration_us as f64 / 1_000_000.0
                ));
            }
        }
        "latch_contention" => {
            parts.push(format!(
                "Session {} has latch contention — in-memory page contention (hot pages).",
                sid
            ));
        }
        "network_bottleneck" => {
            parts.push(format!(
                "Session {} is waiting on the network — the client application is consuming results too slowly.",
                sid
            ));
        }
        "memory_pressure" => {
            parts.push(format!(
                "Session {} is waiting for memory grants — the server is under memory pressure.",
                sid
            ));
        }
        "cpu_pressure" => {
            parts.push(format!(
                "Session {} shows CPU pressure — parallelism overhead or scheduler contention.",
                sid
            ));
        }
        _ => {
            if let (Some(dur), Some(cpu)) = (anchor.duration_us, anchor.cpu_time_us) {
                if dur > cpu * 3 {
                    parts.push(format!(
                        "Session {} duration ({:.1}s) is much higher than CPU time ({:.1}s) — {}% of time was spent waiting.",
                        sid, dur as f64 / 1_000_000.0, cpu as f64 / 1_000_000.0,
                        ((1.0 - cpu as f64 / dur as f64) * 100.0) as i64
                    ));
                    if wait_stats.is_empty() {
                        parts.push("No wait events captured — ensure the XEvent session captures 'wait_completed' events for this analysis.".to_string());
                    }
                } else {
                    parts.push(format!("Session {} — no significant blocking or wait patterns detected.", sid));
                }
            }
        }
    }

    // Add duration context
    if let Some(dur) = anchor.duration_us {
        if let Some(cpu) = anchor.cpu_time_us {
            let wait_pct = ((1.0 - cpu as f64 / dur.max(1) as f64) * 100.0).max(0.0) as i64;
            if wait_pct > 50 && !parts.iter().any(|p| p.contains("waiting")) {
                parts.push(format!("{}% of elapsed time was waiting (not CPU).", wait_pct));
            }
        }
    }

    parts.join(" ")
}
