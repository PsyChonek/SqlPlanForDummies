use super::types::{XelEvent, XelFilter};

pub fn matches_filter(event: &XelEvent, filter: &XelFilter) -> bool {
    if !filter.event_names.is_empty()
        && !filter.event_names.contains(&event.event_name)
    {
        return false;
    }

    if let Some(ref from) = filter.time_from {
        if event.timestamp < *from {
            return false;
        }
    }

    if let Some(ref to) = filter.time_to {
        if event.timestamp > *to {
            return false;
        }
    }

    if !filter.session_ids.is_empty() {
        match event.session_id {
            Some(sid) if filter.session_ids.contains(&sid) => {}
            _ => return false,
        }
    }

    if let Some(ref needle) = filter.object_name_contains {
        let needle_lower = needle.to_lowercase();
        match &event.object_name {
            Some(name) if name.to_lowercase().contains(&needle_lower) => {}
            _ => return false,
        }
    }

    if let Some(ref needle) = filter.sql_text_contains {
        let needle_lower = needle.to_lowercase();
        let found = event
            .sql_text
            .as_ref()
            .map(|s| s.to_lowercase().contains(&needle_lower))
            .unwrap_or(false)
            || event
                .statement
                .as_ref()
                .map(|s| s.to_lowercase().contains(&needle_lower))
                .unwrap_or(false);
        if !found {
            return false;
        }
    }

    if let Some(ref u) = filter.username {
        match &event.username {
            Some(eu) if eu.eq_ignore_ascii_case(u) => {}
            _ => return false,
        }
    }

    if let Some(ref app) = filter.client_app_name {
        match &event.client_app_name {
            Some(ea) if ea.eq_ignore_ascii_case(app) => {}
            _ => return false,
        }
    }

    if let Some(ref db) = filter.database_name {
        match &event.database_name {
            Some(ed) if ed.eq_ignore_ascii_case(db) => {}
            _ => return false,
        }
    }

    if let Some(min) = filter.min_duration_us {
        match event.duration_us {
            Some(d) if d >= min => {}
            _ => return false,
        }
    }

    if let Some(max) = filter.max_duration_us {
        match event.duration_us {
            Some(d) if d <= max => {}
            _ => return false,
        }
    }

    if let Some(ref sf) = filter.source_file {
        if event.source_file != *sf {
            return false;
        }
    }

    if let Some(ref r) = filter.result {
        match &event.result {
            Some(er) if er.eq_ignore_ascii_case(r) => {}
            _ => return false,
        }
    }

    if filter.errors_only {
        match &event.result {
            Some(r) if r != "OK" => {}
            _ => return false,
        }
    }

    if filter.deadlocks_only {
        let has_deadlock_id = event.extra_fields.get("deadlock_id")
            .and_then(|v| match v {
                serde_json::Value::Number(n) => n.as_i64(),
                serde_json::Value::String(s) => s.parse().ok(),
                _ => None,
            })
            .map_or(false, |id| id != 0);
        let is_deadlock = event.event_name.contains("deadlock")
            || event.deadlock_graph.is_some()
            || has_deadlock_id;
        if !is_deadlock {
            return false;
        }
    }

    if let Some(ref search) = filter.text_search {
        // Support "column:value" syntax for targeted column search
        // e.g. "transaction_id:1934357206" or "deadlock_id:381319"
        if let Some(colon_pos) = search.find(':') {
            let col = &search[..colon_pos];
            let val = search[colon_pos + 1..].to_lowercase();
            if !col.is_empty() && !val.is_empty() {
                // Try matching against known fixed fields first
                let field_value: Option<String> = match col {
                    "event_name" | "eventName" => Some(event.event_name.clone()),
                    "session_id" | "sessionId" => event.session_id.map(|s| s.to_string()),
                    "object_name" | "objectName" => event.object_name.clone(),
                    "result" => event.result.clone(),
                    "resource_type" | "resourceType" => event.resource_type.clone(),
                    "lock_mode" | "lockMode" => event.lock_mode.clone(),
                    "wait_type" | "waitType" => event.wait_type.clone(),
                    "username" => event.username.clone(),
                    "database_name" | "databaseName" => event.database_name.clone(),
                    "client_app_name" | "clientAppName" => event.client_app_name.clone(),
                    _ => None,
                };

                let found = if let Some(ref fv) = field_value {
                    fv.to_lowercase().contains(&val)
                } else {
                    // Search in extra_fields by column name
                    let direct = event.extra_fields.get(col).map_or(false, |v| {
                        match v {
                            serde_json::Value::String(s) => s.to_lowercase().contains(&val),
                            serde_json::Value::Number(n) => n.to_string().contains(&val),
                            _ => false,
                        }
                    });
                    // For attach_activity_id, also check _xfer — child events
                    // (waits) share _xfer with their parent request
                    if direct {
                        true
                    } else if col == "attach_activity_id" {
                        event.extra_fields.get("attach_activity_id_xfer").map_or(false, |v| {
                            match v {
                                serde_json::Value::String(s) => s.to_lowercase().contains(&val),
                                _ => false,
                            }
                        })
                    } else {
                        false
                    }
                };
                if !found {
                    return false;
                }
            }
        } else {
            // Regular full-text search across all fields
            let needle = search.to_lowercase();
            let found = event.event_name.to_lowercase().contains(&needle)
                || event
                    .object_name
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .sql_text
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .statement
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .result
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .resource_type
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .lock_mode
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .wait_type
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .username
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event
                    .client_app_name
                    .as_ref()
                    .map(|s| s.to_lowercase().contains(&needle))
                    .unwrap_or(false)
                || event.extra_fields.values().any(|v| {
                    match v {
                        serde_json::Value::String(s) => s.to_lowercase().contains(&needle),
                        serde_json::Value::Number(n) => n.to_string().contains(&needle),
                        _ => false,
                    }
                });
            if !found {
                return false;
            }
        }
    }

    true
}
