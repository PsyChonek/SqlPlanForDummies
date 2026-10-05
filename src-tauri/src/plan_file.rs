use rfd::AsyncFileDialog;

const UTF8_DECLARATION: &str = r#"<?xml version="1.0" encoding="utf-8"?>"#;

/// Rewrites the XML declaration as UTF-8, since the file is always written as UTF-8.
/// Plans copied out of SSMS often declare utf-16, which would make SSMS misread the file.
fn normalize_plan_xml(xml: &str) -> String {
    let body = xml.trim_start_matches('\u{feff}').trim_start();
    let body = match body.strip_prefix("<?xml") {
        Some(rest) => rest.find("?>").map_or(body, |end| rest[end + 2..].trim_start()),
        None => body,
    };
    format!("{UTF8_DECLARATION}\r\n{body}")
}

/// Shows a save dialog and writes the plan as a standalone .sqlplan file.
/// Returns the saved path, or None when the user cancels.
#[tauri::command]
pub async fn save_plan_file(xml: String, suggested_name: String) -> Result<Option<String>, String> {
    let file = AsyncFileDialog::new()
        .add_filter("SQL Server Execution Plan", &["sqlplan"])
        .add_filter("XML", &["xml"])
        .set_file_name(&suggested_name)
        .set_title("Export execution plan")
        .save_file()
        .await;

    let Some(file) = file else {
        return Ok(None);
    };

    let path = file.path().to_path_buf();
    tokio::fs::write(&path, normalize_plan_xml(&xml))
        .await
        .map_err(|e| format!("Could not write {}: {}", path.display(), e))?;
    Ok(Some(path.to_string_lossy().into_owned()))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn adds_declaration_when_missing() {
        let out = normalize_plan_xml("<ShowPlanXML/>");
        assert_eq!(out, format!("{UTF8_DECLARATION}\r\n<ShowPlanXML/>"));
    }

    #[test]
    fn replaces_utf16_declaration() {
        let out = normalize_plan_xml("<?xml version=\"1.0\" encoding=\"utf-16\"?>\n<ShowPlanXML/>");
        assert_eq!(out, format!("{UTF8_DECLARATION}\r\n<ShowPlanXML/>"));
    }

    #[test]
    fn strips_bom_and_leading_whitespace() {
        let out = normalize_plan_xml("\u{feff}  <ShowPlanXML/>");
        assert_eq!(out, format!("{UTF8_DECLARATION}\r\n<ShowPlanXML/>"));
    }
}
