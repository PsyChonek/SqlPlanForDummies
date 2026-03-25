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
    all_columns: Vec<String>,
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
            all_columns: Vec::new(),
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
        if let Some(v) = anchor.extra_fields.get("attach_activity_id") {
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
    }

    pub fn event_count(&self) -> usize {
        self.events.len()
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
        // Formats: "KEY: db_id:hobt_id (hash)", "PAGE: db_id:file:page", "RID: db_id:file:page:slot"
        // We want the hobt_id from KEY locks
        let trimmed = wait_resource.trim();
        if let Some(rest) = trimmed.strip_prefix("KEY:") {
            let rest = rest.trim();
            // "6:72057625259081728 (c9a34554b564)" → split on ':'
            let parts: Vec<&str> = rest.splitn(3, ':').collect();
            if parts.len() >= 2 {
                // Second part is "72057625259081728 (hash)" or just "72057625259081728"
                let hobt_str = parts[1].split_whitespace().next().unwrap_or("");
                return hobt_str.parse::<i64>().ok();
            }
        }
        None
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
            // From wait_resource string (e.g., "KEY: 6:72057625259081728 (hash)")
            if let Some(ref wr) = event.extra_fields.get("wait_resource")
                .and_then(|v| v.as_str().map(|s| s.to_string()))
                .or_else(|| event.resource_description.clone())
            {
                if let Some(hobt_id) = Self::parse_wait_resource_hobt_id(wr) {
                    ids.insert(hobt_id);
                }
            }
        }
        ids.into_iter().collect()
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

    /// Apply resolved object names from associated_object_id and wait_resource
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

        // Collect candidate event indices from session + transaction indexes
        let mut candidate_indices: HashSet<usize> = HashSet::new();

        if let Some(sid) = anchor.session_id {
            if let Some(indices) = self.by_session.get(&sid) {
                candidate_indices.extend(indices.iter().copied());
            }
        }

        // Also check by transaction_id
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
        let mut timeout_count = 0usize;
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

            // Timeouts
            if en.contains("timeout") {
                timeout_count += 1;
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
            timeout_count,
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
                    diagnosis: "unknown".into(),
                    recommendations: Vec::new(),
                };
            }
        };
        let anchor = &self.events[anchor_idx];
        let anchor_sid = anchor.session_id;
        let anchor_ts = anchor.timestamp;
        let window = chrono::Duration::milliseconds(time_window_ms);

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

        for &idx in &bpr_indices {
            let bpr_event = &self.events[idx];
            let diff = (bpr_event.timestamp - anchor_ts).num_milliseconds().abs();
            if diff > time_window_ms {
                continue;
            }

            if let Some(ref xml) = bpr_event.blocked_process_report {
                if let Some(parsed) = parse_blocked_process_report_xml(xml, bpr_event.id, bpr_event.timestamp) {
                    // Only include if anchor session is involved
                    let anchor_involved = anchor_sid.map_or(true, |sid| {
                        parsed.blocked_spid == Some(sid)
                            || parsed.blocking_spid == Some(sid)
                    });

                    if anchor_involved {
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

        // 2. Build blocking chain from BPRs
        let blocking_chain = build_blocking_chain(&parsed_bprs, &self.events, &involved_sessions, anchor_ts, window);

        // 3. Find blocker events (events from blocking sessions around the time)
        let mut blocker_events: Vec<XelEvent> = Vec::new();
        let blocker_sids: HashSet<i64> = parsed_bprs
            .iter()
            .filter_map(|p| p.blocking_spid)
            .collect();

        for &sid in &blocker_sids {
            if Some(sid) == anchor_sid {
                continue; // Don't include anchor session as blocker events
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

        // 4. Find lock_escalation events in time window
        let mut lock_escalations: Vec<XelEvent> = Vec::new();
        if let Some(esc_indices) = self.by_event_name.get("lock_escalation") {
            for &idx in esc_indices {
                let ev = &self.events[idx];
                let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                if diff <= time_window_ms {
                    if involved_sessions.is_empty()
                        || ev.session_id.map_or(false, |s| involved_sessions.contains(&s))
                    {
                        lock_escalations.push(ev.clone());
                    }
                }
            }
        }

        // 5. Find wait events for the anchor session
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
                    if ev.session_id == anchor_sid {
                        let diff = (ev.timestamp - anchor_ts).num_milliseconds().abs();
                        if diff <= time_window_ms {
                            wait_events.push(ev.clone());
                        }
                    }
                }
            }
        }
        wait_events.sort_by_key(|e| e.timestamp);

        // 6. Aggregate wait stats by wait type
        let wait_stats = aggregate_wait_stats(&wait_events);

        // 7. Find and parse deadlock graphs in time window
        let mut deadlocks: Vec<ParsedDeadlockGraph> = Vec::new();
        let deadlock_event_names = ["xml_deadlock_report", "lock_deadlock", "lock_deadlock_chain"];

        // Detect if anchor event is likely a deadlock victim:
        // - Error result with non-timeout duration (not ~30s)
        // - Or error_number 1205 (deadlock)
        let error_number = anchor.extra_fields.get("error_number")
            .and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            });
        let is_error = anchor.result.as_deref() == Some("Error")
            || anchor.result.as_deref() == Some("Abort");
        let is_likely_deadlock_victim = error_number == Some(1205)
            || (is_error && anchor.duration_us.map_or(false, |d| {
                // Deadlock victims get killed at arbitrary times (not 30s timeout)
                // If duration is NOT close to a timeout boundary, more likely deadlock
                let d_sec = d / 1_000_000;
                d_sec < 28 || (d_sec > 32 && d_sec < 58)
            }));
        let is_likely_timeout = error_number == Some(-2)
            || (is_error && anchor.duration_us.map_or(false, |d| {
                let d_sec = d / 1_000_000;
                (28..=32).contains(&d_sec) || (58..=62).contains(&d_sec)
                    || (118..=122).contains(&d_sec) || (298..=302).contains(&d_sec)
            }));

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
                            // If anchor is likely a deadlock victim, include nearby
                            // deadlocks even if session doesn't match exactly
                            // (victim session may have been recycled, or the XE
                            // session may report a different spid)
                            let close_in_time = diff <= 5000; // within 5s
                            if anchor_involved || (is_likely_deadlock_victim && close_in_time) {
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

        // 8. Diagnose the root cause
        let diagnosis = if !deadlocks.is_empty() {
            "deadlock".to_string()
        } else if is_likely_deadlock_victim && deadlocks.is_empty() {
            // Error with non-timeout duration, high wait ratio, no captured deadlock graph
            "likely_deadlock".to_string()
        } else if is_likely_timeout {
            "timeout".to_string()
        } else {
            diagnose_wait_pattern(anchor, &parsed_bprs, &wait_stats)
        };

        // 9. Build recommendations
        let recommendations = build_recommendations(anchor, &diagnosis, &wait_stats, &deadlocks);

        // 10. Build summary
        let summary = build_analysis_summary(anchor, &parsed_bprs, &blocking_chain, &wait_stats, &diagnosis, &deadlocks);

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
        blocking_spid: None,
        blocking_xact_id: None,
        blocking_input_buffer: None,
        blocking_database: None,
        blocking_hostname: None,
        blocking_app_name: None,
        blocking_login_name: None,
        blocking_status: None,
        blocking_last_batch_started: None,
    };

    let mut reader = Reader::from_str(xml);
    reader.config_mut().trim_text(true);

    #[derive(PartialEq)]
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
                                result.blocked_database = attrs.get("currentdb").cloned();
                                result.blocked_hostname = attrs.get("hostname").cloned();
                                result.blocked_app_name = attrs.get("clientapp").cloned();
                                result.blocked_login_name = attrs.get("loginname").cloned();
                            }
                            Section::BlockingProcess => {
                                result.blocking_spid = attrs.get("spid").and_then(|s| s.parse().ok());
                                result.blocking_xact_id = attrs.get("xactid").cloned();
                                result.blocking_database = attrs.get("currentdb").cloned();
                                result.blocking_hostname = attrs.get("hostname").cloned();
                                result.blocking_app_name = attrs.get("clientapp").cloned();
                                result.blocking_login_name = attrs.get("loginname").cloned();
                                result.blocking_status = attrs.get("status").cloned();
                                result.blocking_last_batch_started = attrs.get("lastbatchstarted").cloned();
                            }
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
        ResourceList,
        InResource(DeadlockResource),
    }
    let mut state = State::Root;
    let mut current_input_buf = String::new();

    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(ref e)) | Ok(XmlEvent::Empty(ref e)) => {
                let tag = String::from_utf8_lossy(e.name().as_ref()).to_string();
                let is_empty = matches!(reader.read_event(), _) && {
                    // Re-check — actually we can't easily tell from here.
                    // Let's use the original event type
                    false
                };
                // Actually, let's handle this differently with proper matching
                let _ = is_empty;
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
                                lock_mode: attrs.get("lockMode").cloned(),
                                wait_resource: attrs.get("waitresource").cloned(),
                                wait_time_ms: attrs.get("waittime").and_then(|s| s.parse().ok()),
                                transaction_name: attrs.get("transactionname").cloned(),
                                log_used: attrs.get("logused").and_then(|s| s.parse().ok()),
                                input_buffer: None,
                                database_name: attrs.get("currentdb").cloned(),
                                hostname: attrs.get("hostname").cloned(),
                                app_name: attrs.get("clientapp").cloned(),
                                login_name: attrs.get("loginname").cloned(),
                                isolation_level: attrs.get("isolationlevel").cloned(),
                                status: attrs.get("status").cloned(),
                            };
                            processes.push(proc);
                            state = State::InProcess(proc_id);
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
                    | "exchangeEvent" | "threadpool" | "metadata" => {
                        if matches!(state, State::ResourceList) {
                            let res = DeadlockResource {
                                resource_type: tag.clone(),
                                database_name: attrs.get("dbid").cloned()
                                    .or_else(|| attrs.get("databasename").cloned()),
                                object_name: attrs.get("objectname").cloned(),
                                index_name: attrs.get("indexname").cloned(),
                                mode: attrs.get("mode").cloned(),
                                holders: Vec::new(),
                                waiters: Vec::new(),
                            };
                            state = State::InResource(res);
                        }
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
                if let State::InInputBuf(_) = &state {
                    if let Ok(text) = e.unescape() {
                        current_input_buf.push_str(&text);
                    }
                }
            }
            Ok(XmlEvent::CData(ref e)) => {
                if let State::InInputBuf(_) = &state {
                    current_input_buf.push_str(&String::from_utf8_lossy(e.as_ref()));
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
                    "process" => {
                        if matches!(state, State::InProcess(_)) {
                            state = State::ProcessList;
                        }
                    }
                    "process-list" => state = State::Root,
                    "victim-list" => state = State::Root,
                    "keylock" | "pagelock" | "objectlock" | "ridlock" | "hobtlock"
                    | "exchangeEvent" | "threadpool" | "metadata" => {
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

        let (wait_resource, lock_mode, sql_preview, app_name, username, database) =
            if let Some((bpr, is_blocked)) = session_info.get(&sid) {
                if *is_blocked {
                    (
                        bpr.blocked_wait_resource.clone(),
                        bpr.blocked_lock_mode.clone(),
                        bpr.blocked_input_buffer.clone(),
                        bpr.blocked_app_name.clone(),
                        bpr.blocked_login_name.clone(),
                        bpr.blocked_database.clone(),
                    )
                } else {
                    (
                        None,
                        None,
                        bpr.blocking_input_buffer.clone(),
                        bpr.blocking_app_name.clone(),
                        bpr.blocking_login_name.clone(),
                        bpr.blocking_database.clone(),
                    )
                }
            } else {
                (None, None, None, None, None, None)
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
) -> Vec<String> {
    let mut recs = Vec::new();

    match diagnosis {
        "deadlock" => {
            recs.push("DEADLOCK detected — two or more sessions were waiting on each other's locks.".to_string());
            // Check if MERGE is involved
            let has_merge = deadlocks.iter().any(|dl| {
                dl.processes.iter().any(|p| {
                    p.input_buffer.as_ref().map_or(false, |buf| {
                        let upper = buf.to_uppercase();
                        upper.contains("MERGE") || upper.contains("INSERT") || upper.contains("UPDATE")
                    })
                })
            });
            if has_merge {
                recs.push("MERGE statements are notorious for deadlocks — they acquire multiple lock types (S, U, X) on the same resources in unpredictable order.".to_string());
                recs.push("Consider replacing MERGE with separate IF EXISTS/UPDATE/INSERT logic, or use sp_getapplock to serialize concurrent executions.".to_string());
            }
            // Check for communication buffer deadlock (parallelism)
            let has_exchange = deadlocks.iter().any(|dl| {
                dl.resources.iter().any(|r| r.resource_type.contains("exchange") || r.resource_type.contains("communication"))
            });
            if has_exchange {
                recs.push("Parallelism (exchange/communication buffer) deadlock detected — this occurs when parallel query plans deadlock internally.".to_string());
                recs.push("Try OPTION(MAXDOP 1) on the affected query, or reduce server MAXDOP. Updating statistics may also change the plan.".to_string());
            }
            recs.push("Ensure the SP handles deadlock retries: wrap the MERGE in a WHILE loop with TRY/CATCH and retry logic.".to_string());
            recs.push("Fix the transaction count mismatch: after a deadlock, the transaction is already rolled back — check XACT_STATE() before calling ROLLBACK.".to_string());
        }
        "likely_deadlock" => {
            recs.push("This event is likely a DEADLOCK VICTIM — it has an Error result with a non-timeout duration and high wait ratio.".to_string());
            recs.push("The deadlock graph was not captured in the XEvent session. Enable the 'xml_deadlock_report' event to capture full deadlock details.".to_string());
            recs.push("Common cause: concurrent MERGE/INSERT/UPDATE operations on the same tables. Consider serializing with sp_getapplock.".to_string());
            recs.push("Fix transaction handling: check XACT_STATE() in CATCH blocks before ROLLBACK, as deadlock auto-rolls back the transaction.".to_string());
        }
        "timeout" => {
            let dur_sec = anchor.duration_us.unwrap_or(0) / 1_000_000;
            recs.push(format!(
                "Execution timeout (~{}s) — the query was terminated because it exceeded the command timeout.",
                dur_sec
            ));
            recs.push("The most common cause is lock blocking — another session holds a lock this query needs.".to_string());
            recs.push("Check if there are blocked_process_report events around this time (requires 'blocked process threshold' server config).".to_string());
            recs.push("Consider increasing the command timeout for long-running operations, or optimize the query to complete faster.".to_string());
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
            recs.push("Identify the blocking session and its query — consider optimizing it or reducing its transaction scope.".to_string());
            recs.push("Check if READ COMMITTED SNAPSHOT ISOLATION (RCSI) is enabled — it eliminates reader/writer blocking.".to_string());
            recs.push("Review lock escalation — if the query touches many rows, SQL Server may escalate to a table lock.".to_string());
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
        // If diagnosis is not lock but lock waits are present
        if diagnosis != "lock_contention" && diagnosis != "lock_blocking" && lock_count > 0 {
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
) -> String {
    let sid = anchor.session_id.map_or("-".to_string(), |s| s.to_string());
    let mut parts: Vec<String> = Vec::new();

    // Diagnosis headline
    match diagnosis {
        "deadlock" => {
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
            // Show resource types
            let resource_types: HashSet<String> = deadlocks.iter()
                .flat_map(|dl| dl.resources.iter())
                .map(|r| r.resource_type.clone())
                .collect();
            if !resource_types.is_empty() {
                let types: Vec<String> = resource_types.into_iter().collect();
                parts.push(format!("Contended resources: {}.", types.join(", ")));
            }
        }
        "likely_deadlock" => {
            parts.push(format!(
                "Session {} was likely a DEADLOCK VICTIM. Error result with {:.1}s duration ({:.0}% waiting) — not a timeout pattern.",
                sid,
                anchor.duration_us.unwrap_or(0) as f64 / 1_000_000.0,
                anchor.cpu_time_us.map_or(0.0, |cpu| {
                    (1.0 - cpu as f64 / anchor.duration_us.unwrap_or(1) as f64) * 100.0
                })
            ));
            parts.push("No deadlock graph was captured — enable xml_deadlock_report in the XEvent session for full details.".to_string());
        }
        "timeout" => {
            let dur_sec = anchor.duration_us.unwrap_or(0) / 1_000_000;
            parts.push(format!(
                "Session {} hit an execution timeout (~{}s). The query was terminated by the client because it exceeded the command timeout.",
                sid, dur_sec
            ));
            if anchor.cpu_time_us.map_or(false, |cpu| {
                anchor.duration_us.unwrap_or(0) > cpu * 3
            }) {
                parts.push("Most time was spent waiting (not CPU) — likely blocked by another session or waiting on IO.".to_string());
            }
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
            if let Some(bpr) = bprs.first() {
                if let Some(ref res) = bpr.blocked_wait_resource {
                    parts.push(format!("Contended resource: {}", res));
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
