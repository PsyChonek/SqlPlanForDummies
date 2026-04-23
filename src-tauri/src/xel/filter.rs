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

    // Errors / Deadlocks quick filters — treat as OR when both active
    if filter.errors_only || filter.deadlocks_only {
        let is_deadlock = event.event_name.contains("deadlock")
            || event.deadlock_graph.is_some()
            || event.extra_fields.get("deadlock_id")
                .and_then(|v| match v {
                    serde_json::Value::Number(n) => n.as_i64(),
                    serde_json::Value::String(s) => s.parse().ok(),
                    _ => None,
                })
                .map_or(false, |id| id != 0);

        let pass_errors = filter.errors_only && !is_deadlock && {
            let is_error_result = matches!(&event.result, Some(r) if r != "OK");
            let has_error_number = event.extra_fields.get("error_number")
                .and_then(|v| match v {
                    serde_json::Value::Number(n) => n.as_i64(),
                    serde_json::Value::String(s) => s.parse().ok(),
                    _ => None,
                })
                .map_or(false, |n| n > 0 && n != 1205);
            is_error_result || has_error_number
        };

        let pass_deadlocks = filter.deadlocks_only && is_deadlock;

        if !pass_errors && !pass_deadlocks {
            return false;
        }
    }

    if let Some(ref search) = filter.text_search {
        // Parse search into expression tree supporting:
        //   space = AND, || = OR, () = grouping, "quoted phrases"
        // Example: (session_id:64 rpc) || (session_id:72 "my phrase")
        let expr = parse_search_expr(search);
        if !eval_expr(event, &expr) {
            return false;
        }
    }

    true
}

/// Search expression tree: OR of AND groups, each containing atomic terms.
enum SearchExpr {
    Or(Vec<SearchExpr>),
    And(Vec<SearchExpr>),
    Column(String, String), // (column_name, lowercase_value)
    FreeText(String),       // lowercase needle
}

fn eval_expr(event: &XelEvent, expr: &SearchExpr) -> bool {
    match expr {
        SearchExpr::Or(children) => children.iter().any(|c| eval_expr(event, c)),
        SearchExpr::And(children) => children.iter().all(|c| eval_expr(event, c)),
        SearchExpr::Column(col, val) => match_column(event, col, val),
        SearchExpr::FreeText(needle) => match_free_text(event, needle),
    }
}

// Tokenizer layer — raw tokens before building the tree
#[derive(Debug)]
enum RawToken {
    Column(String, String),
    FreeText(String),
    Or,         // ||
    OpenParen,  // (
    CloseParen, // )
}

fn tokenize_search(search: &str) -> Vec<RawToken> {
    let mut tokens = Vec::new();
    let mut chars = search.chars().peekable();

    while chars.peek().is_some() {
        // Skip whitespace
        while chars.peek().map_or(false, |c| c.is_whitespace()) {
            chars.next();
        }
        if chars.peek().is_none() {
            break;
        }

        // ||
        if chars.peek() == Some(&'|') {
            chars.next();
            if chars.peek() == Some(&'|') {
                chars.next();
            }
            tokens.push(RawToken::Or);
            continue;
        }

        // Parentheses
        if chars.peek() == Some(&'(') {
            chars.next();
            tokens.push(RawToken::OpenParen);
            continue;
        }
        if chars.peek() == Some(&')') {
            chars.next();
            tokens.push(RawToken::CloseParen);
            continue;
        }

        // Quoted string: "my two words"
        if chars.peek() == Some(&'"') {
            chars.next();
            let mut quoted = String::new();
            while let Some(&c) = chars.peek() {
                if c == '"' {
                    chars.next();
                    break;
                }
                quoted.push(c);
                chars.next();
            }
            if !quoted.is_empty() {
                tokens.push(RawToken::FreeText(quoted.to_lowercase()));
            }
            continue;
        }

        // Unquoted word (possibly col:value, and col:"quoted value")
        let mut word = String::new();
        while let Some(&c) = chars.peek() {
            if c.is_whitespace() || c == '(' || c == ')' {
                break;
            }
            // || at word boundary
            if c == '|' {
                break;
            }
            // col:"quoted value"
            if c == '"' && word.ends_with(':') {
                chars.next();
                while let Some(&qc) = chars.peek() {
                    if qc == '"' {
                        chars.next();
                        break;
                    }
                    word.push(qc);
                    chars.next();
                }
                break;
            }
            word.push(c);
            chars.next();
        }

        if word.is_empty() {
            continue;
        }

        if let Some(colon_pos) = word.find(':') {
            let col = &word[..colon_pos];
            let val = &word[colon_pos + 1..];
            if !col.is_empty() && !val.is_empty() {
                tokens.push(RawToken::Column(col.to_string(), val.to_lowercase()));
                continue;
            }
        }
        tokens.push(RawToken::FreeText(word.to_lowercase()));
    }
    tokens
}

/// Build expression tree from raw tokens.
/// Grammar: expr = and_group (|| and_group)*
///          and_group = atom+
///          atom = term | '(' expr ')'
fn parse_search_expr(search: &str) -> SearchExpr {
    let tokens = tokenize_search(search);
    let (expr, _) = parse_or(&tokens, 0);
    expr
}

fn parse_or(tokens: &[RawToken], mut pos: usize) -> (SearchExpr, usize) {
    let (first, new_pos) = parse_and(tokens, pos);
    pos = new_pos;
    let mut branches = vec![first];

    while pos < tokens.len() {
        if matches!(tokens[pos], RawToken::Or) {
            pos += 1; // consume ||
            let (branch, new_pos) = parse_and(tokens, pos);
            pos = new_pos;
            branches.push(branch);
        } else {
            break;
        }
    }

    if branches.len() == 1 {
        (branches.remove(0), pos)
    } else {
        (SearchExpr::Or(branches), pos)
    }
}

fn parse_and(tokens: &[RawToken], mut pos: usize) -> (SearchExpr, usize) {
    let mut terms = Vec::new();

    while pos < tokens.len() {
        match &tokens[pos] {
            RawToken::Or | RawToken::CloseParen => break,
            RawToken::OpenParen => {
                pos += 1; // consume (
                let (inner, new_pos) = parse_or(tokens, pos);
                pos = new_pos;
                // consume ) if present
                if pos < tokens.len() && matches!(tokens[pos], RawToken::CloseParen) {
                    pos += 1;
                }
                terms.push(inner);
            }
            RawToken::Column(col, val) => {
                terms.push(SearchExpr::Column(col.clone(), val.clone()));
                pos += 1;
            }
            RawToken::FreeText(text) => {
                terms.push(SearchExpr::FreeText(text.clone()));
                pos += 1;
            }
        }
    }

    if terms.len() == 1 {
        (terms.remove(0), pos)
    } else if terms.is_empty() {
        // Edge case: empty group — match everything
        (SearchExpr::And(vec![]), pos)
    } else {
        (SearchExpr::And(terms), pos)
    }
}

fn match_column(event: &XelEvent, col: &str, val: &str) -> bool {
    let field_value: Option<String> = match col {
        "id" | "event_id" | "eventId" => Some(event.id.to_string()),
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

    if let Some(ref fv) = field_value {
        return fv.to_lowercase().contains(val);
    }

    // Search in extra_fields by column name
    let direct = event.extra_fields.get(col).map_or(false, |v| match v {
        serde_json::Value::String(s) => s.to_lowercase().contains(val),
        serde_json::Value::Number(n) => n.to_string().contains(val),
        _ => false,
    });
    if direct {
        return true;
    }
    // For attach_activity_id, also check _xfer — child events
    // (waits) share _xfer with their parent request
    if col == "attach_activity_id" {
        return event
            .extra_fields
            .get("attach_activity_id_xfer")
            .map_or(false, |v| match v {
                serde_json::Value::String(s) => s.to_lowercase().contains(val),
                _ => false,
            });
    }
    false
}

fn match_free_text(event: &XelEvent, needle: &str) -> bool {
    event.id.to_string().contains(needle)
        || event.event_name.to_lowercase().contains(needle)
        || event
            .object_name
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .sql_text
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .statement
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .result
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .resource_type
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .lock_mode
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .wait_type
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .username
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .client_app_name
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event.extra_fields.values().any(|v| match v {
            serde_json::Value::String(s) => s.to_lowercase().contains(needle),
            serde_json::Value::Number(n) => n.to_string().contains(needle),
            _ => false,
        })
        || event
            .deadlock_graph
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
        || event
            .blocked_process_report
            .as_ref()
            .map(|s| s.to_lowercase().contains(needle))
            .unwrap_or(false)
}
