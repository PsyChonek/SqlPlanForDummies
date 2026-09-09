use serde::Serialize;
use std::process::{Output, Stdio};
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;
use tokio::process::Command;
use tokio::sync::Mutex;

const PACKAGE_ID: &str = "PsyChonek.SqlPlanForDummies";
const NO_APPLICATIONS_FOUND: u32 = 0x8A150014;
const UPDATE_NOT_APPLICABLE: u32 = 0x8A15002B;
static UPDATE_LOCK: Mutex<()> = Mutex::const_new(());
static UPDATE_COMPLETED: AtomicBool = AtomicBool::new(false);

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateInfo {
    current_version: String,
    available_version: Option<String>,
    installed: bool,
}

fn winget_command() -> Command {
    let mut command = Command::new("winget.exe");
    // Hide the CLI console; --interactive still lets the installer show its UI.
    command.creation_flags(0x08000000);
    command.stdin(Stdio::null());
    command
}

fn clean_output(text: &str) -> String {
    let mut result = String::new();
    let mut chars = text.chars().peekable();
    while let Some(ch) = chars.next() {
        if ch == '\u{1b}' && chars.peek() == Some(&'[') {
            chars.next();
            for escape in chars.by_ref() {
                if ('@'..='~').contains(&escape) {
                    break;
                }
            }
        } else if !ch.is_control() || ch == '\n' || ch == '\t' {
            result.push(ch);
        }
    }
    result
}

fn version_parts(version: &str) -> Option<[u64; 4]> {
    let mut parts = [0; 4];
    let values: Vec<_> = version.split('.').collect();
    if !(2..=4).contains(&values.len()) {
        return None;
    }
    for (index, value) in values.iter().enumerate() {
        if value.is_empty() || !value.bytes().all(|byte| byte.is_ascii_digit()) {
            return None;
        }
        parts[index] = value.parse().ok()?;
    }
    Some(parts)
}

fn parse_update_info(output: &str, current_version: &str) -> Result<UpdateInfo, String> {
    let current = version_parts(current_version)
        .ok_or("Cannot compare this app version with WinGet releases.")?;
    let mut info = UpdateInfo {
        current_version: current_version.to_string(),
        available_version: None,
        installed: false,
    };
    // Match the package ID, not localized headers or fixed column widths.
    for line in clean_output(output).lines() {
        let columns: Vec<_> = line.split_whitespace().collect();
        let Some(index) = columns.iter().position(|column| *column == PACKAGE_ID) else {
            continue;
        };
        if columns.get(index + 1).is_none() {
            return Err("WinGet returned an incomplete package entry. Please try again.".into());
        }
        info.installed = true;
        if let Some(available) = columns.get(index + 2) {
            // A source name may follow the installed version when no update exists.
            if *available == "winget" {
                continue;
            }
            let candidate = version_parts(available).ok_or(
                "WinGet returned an unsupported update version. Update using WinGet manually.",
            )?;
            let best = info
                .available_version
                .as_deref()
                .and_then(version_parts)
                .unwrap_or(current);
            if candidate > best {
                info.available_version = Some((*available).to_string());
            }
        }
    }
    if !info.installed {
        return Err("Could not read the app entry returned by WinGet. Please try again.".into());
    }
    Ok(info)
}

fn command_error(action: &str, output: &Output) -> String {
    let details = clean_output(&format!(
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    let details: String = details.trim().chars().take(3000).collect();
    format!(
        "WinGet could not {action} (code 0x{:08X}). {}",
        output.status.code().unwrap_or(-1) as u32,
        details
    )
}

#[tauri::command]
pub async fn check_app_update(app: tauri::AppHandle) -> Result<UpdateInfo, String> {
    let mut command = winget_command();
    command.kill_on_drop(true).args([
        "list",
        "--id",
        PACKAGE_ID,
        "--exact",
        "--source",
        "winget",
        "--accept-source-agreements",
        "--disable-interactivity",
    ]);
    let output = tokio::time::timeout(Duration::from_secs(60), command.output())
        .await
        .map_err(|_| "The WinGet update check timed out. Check your connection and try again.".to_string())?
        .map_err(|_| "WinGet could not be started. Install or update App Installer from the Microsoft Store, then try again.".to_string())?;
    let current_version = app.package_info().version.to_string();
    if output.status.code().map(|code| code as u32) == Some(NO_APPLICATIONS_FOUND) {
        return Ok(UpdateInfo {
            current_version,
            available_version: None,
            installed: false,
        });
    }
    if !output.status.success() {
        return Err(command_error("check for updates", &output));
    }
    parse_update_info(&String::from_utf8_lossy(&output.stdout), &current_version)
}

#[tauri::command]
pub async fn install_app_update(app: tauri::AppHandle) -> Result<(), String> {
    let _guard = UPDATE_LOCK
        .try_lock()
        .map_err(|_| "An update is already running.")?;
    let info = check_app_update(app).await?;
    if info.available_version.is_none() {
        return Err("No newer version is available for this app. Check for updates again.".into());
    }
    let mut command = winget_command();
    // Leave the installer alive if it needs to close this app to replace its files.
    command.kill_on_drop(false).args([
        "upgrade",
        "--id",
        PACKAGE_ID,
        "--exact",
        "--source",
        "winget",
        "--interactive",
        "--accept-source-agreements",
        "--accept-package-agreements",
        "--disable-interactivity",
    ]);
    let output = command
        .output()
        .await
        .map_err(|_| "Could not start the WinGet installer. Please try again.".to_string())?;
    if output.status.code().map(|code| code as u32) == Some(UPDATE_NOT_APPLICABLE) {
        return Err("WinGet did not find an applicable update. Check for updates again.".into());
    }
    if !output.status.success() {
        return Err(command_error("install the update", &output));
    }
    UPDATE_COMPLETED.store(true, Ordering::SeqCst);
    Ok(())
}

#[tauri::command]
pub fn restart_after_update(app: tauri::AppHandle) -> Result<(), String> {
    if !UPDATE_COMPLETED.load(Ordering::SeqCst) {
        return Err("Install an update before restarting.".into());
    }
    app.restart();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn finds_update_in_localized_output_with_ansi_and_duplicate_installs() {
        let output = "Název Id Verze Dostupné\n\u{1b}[32mSQL Plan For Dummies PsyChonek.SqlPlanForDummies 2.5.4 2.10.0\u{1b}[0m\nSQL Plan For Dummies PsyChonek.SqlPlanForDummies 2.5.2\n";
        let info = parse_update_info(output, "2.7.0").unwrap();
        assert!(info.installed);
        assert_eq!(info.available_version.as_deref(), Some("2.10.0"));
    }

    #[test]
    fn compares_to_running_app_not_an_older_installed_copy() {
        let output = "SQL Plan For Dummies PsyChonek.SqlPlanForDummies 2.5.4 2.7.0";
        assert!(parse_update_info(output, "2.7.0")
            .unwrap()
            .available_version
            .is_none());
        assert!(parse_update_info(output, "2.8.0")
            .unwrap()
            .available_version
            .is_none());
    }

    #[test]
    fn accepts_current_install_with_or_without_source_column() {
        for suffix in ["", " winget"] {
            let output = format!("SQL Plan For Dummies {PACKAGE_ID} 2.7.0{suffix}");
            assert!(parse_update_info(&output, "2.7.0")
                .unwrap()
                .available_version
                .is_none());
        }
    }

    #[test]
    fn rejects_unrecognized_output_instead_of_claiming_up_to_date() {
        assert!(parse_update_info("Source unavailable", "2.7.0").is_err());
        assert!(
            parse_update_info("App PsyChonek.SqlPlanForDummies.Other 2.7.0 3.0.0", "2.7.0")
                .is_err()
        );
        assert!(
            parse_update_info("App PsyChonek.SqlPlanForDummies 2.7.0 unknown", "2.7.0").is_err()
        );
    }

    #[test]
    fn compares_numeric_versions_with_zero_padding() {
        assert!(version_parts("2.10.0") > version_parts("2.9.9"));
        assert_eq!(version_parts("2.7.0.0"), version_parts("2.7.0"));
        assert!(version_parts("2.8.0-preview").is_none());
    }
}
