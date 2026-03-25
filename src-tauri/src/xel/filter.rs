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

    if let Some(ref search) = filter.text_search {
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
                .unwrap_or(false);
        if !found {
            return false;
        }
    }

    true
}
