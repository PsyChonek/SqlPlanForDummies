use std::collections::{HashMap, HashSet};
use std::path::Path;

use chrono::{DateTime, Utc};
use tokio::sync::mpsc;

use super::types::*;

/// Determine which PowerShell executable to use (pwsh preferred over powershell)
async fn find_powershell() -> Option<String> {
    for cmd in &["pwsh", "powershell"] {
        let ok = tokio::process::Command::new(cmd)
            .args(&["-NoProfile", "-NonInteractive", "-Command", "echo ok"])
            .stdout(std::process::Stdio::piped())
            .stderr(std::process::Stdio::piped())
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false);
        if ok {
            return Some(cmd.to_string());
        }
    }
    None
}

/// Check if PowerShell and required modules are available
pub async fn check_powershell_availability() -> PowerShellStatus {
    let ps_exe = match find_powershell().await {
        Some(exe) => exe,
        None => {
            return PowerShellStatus {
                available: false,
                sql_server_module: false,
                dbatools_module: false,
                message: "PowerShell is not available on this system".into(),
            };
        }
    };

    // Check for modules and DLL availability in a single PS call
    let check_script = r#"
$result = @{ sqlserver = $false; dbatools = $false; dll = $false; dllPath = '' }
if (Get-Module -ListAvailable SqlServer -ErrorAction SilentlyContinue) { $result.sqlserver = $true }
if (Get-Module -ListAvailable dbatools -ErrorAction SilentlyContinue) { $result.dbatools = $true }
# Check for XEvent DLL in common SSMS/SQL locations
$searchPaths = @(
    "${env:ProgramFiles(x86)}\Microsoft SQL Server Management Studio *\Common7\IDE\Microsoft.SqlServer.XEvent.Linq.dll",
    "${env:ProgramFiles}\Microsoft SQL Server\*\Shared\Microsoft.SqlServer.XEvent.Linq.dll",
    "${env:ProgramFiles(x86)}\Microsoft SQL Server\*\Shared\Microsoft.SqlServer.XEvent.Linq.dll",
    "${env:ProgramFiles}\Microsoft SQL Server\*\Tools\Binn\Microsoft.SqlServer.XEvent.Linq.dll"
)
foreach ($pattern in $searchPaths) {
    $found = Get-ChildItem -Path $pattern -ErrorAction SilentlyContinue | Select-Object -First 1
    if ($found) { $result.dll = $true; $result.dllPath = $found.FullName; break }
}
$result | ConvertTo-Json -Compress
"#;

    let output = tokio::process::Command::new(&ps_exe)
        .args(&["-NoProfile", "-NonInteractive", "-Command", check_script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .output()
        .await;

    let (sql_module, dbatools_module, has_dll, dll_path) = match output {
        Ok(o) if o.status.success() => {
            let stdout = String::from_utf8_lossy(&o.stdout);
            let parsed: Result<serde_json::Value, _> = serde_json::from_str(stdout.trim());
            match parsed {
                Ok(v) => (
                    v.get("sqlserver").and_then(|x| x.as_bool()).unwrap_or(false),
                    v.get("dbatools").and_then(|x| x.as_bool()).unwrap_or(false),
                    v.get("dll").and_then(|x| x.as_bool()).unwrap_or(false),
                    v.get("dllPath").and_then(|x| x.as_str()).unwrap_or("").to_string(),
                ),
                Err(_) => (false, false, false, String::new()),
            }
        }
        _ => (false, false, false, String::new()),
    };

    let message = if sql_module {
        "SqlServer PowerShell module is available".into()
    } else if dbatools_module {
        "dbatools PowerShell module is available".into()
    } else if has_dll {
        format!("Found XEvent DLL at: {}", dll_path)
    } else {
        "SqlServer module will be auto-installed on first XEL load.".into()
    };

    PowerShellStatus {
        available: true,
        sql_server_module: sql_module || has_dll,
        dbatools_module: dbatools_module && !sql_module && !has_dll,
        message,
    }
}

/// Parse XEL files using PowerShell with full fallback chain
pub async fn parse_xel_files(
    file_paths: &[String],
    progress_tx: mpsc::Sender<XelLoadProgress>,
) -> Result<Vec<XelEvent>, String> {
    let ps_exe = find_powershell().await.ok_or(
        "PowerShell not found. Install PowerShell 7 (pwsh) or ensure Windows PowerShell is available."
    )?;

    let paths_joined = file_paths
        .iter()
        .map(|p| format!("'{}'", p.replace('\'', "''")))
        .collect::<Vec<_>>()
        .join(",");

    // Simplified script — no functions, no $ErrorActionPreference='Stop'.
    // Tries each method with try/catch, falls through to auto-install.
    let script = format!(
        r#"
[Console]::OutputEncoding = [System.Text.Encoding]::UTF8
$OutputEncoding = [System.Text.Encoding]::UTF8
$reader = ''

# Try SqlServer module
try {{ Import-Module SqlServer -ErrorAction Stop; $reader = 'sqlserver'; [Console]::Error.WriteLine("INFO:Using SqlServer module") }} catch {{ }}

# Try dbatools
if ($reader -eq '') {{
    try {{ Import-Module dbatools -ErrorAction Stop; $reader = 'dbatools'; [Console]::Error.WriteLine("INFO:Using dbatools module") }} catch {{ }}
}}

# Try SSMS DLL
if ($reader -eq '') {{
    @(
        "${{env:ProgramFiles(x86)}}\Microsoft SQL Server Management Studio *\Common7\IDE\Microsoft.SqlServer.XEvent.Linq.dll",
        "${{env:ProgramFiles}}\Microsoft SQL Server\*\Shared\Microsoft.SqlServer.XEvent.Linq.dll",
        "${{env:ProgramFiles(x86)}}\Microsoft SQL Server\*\Shared\Microsoft.SqlServer.XEvent.Linq.dll"
    ) | ForEach-Object {{
        if ($reader -eq '') {{
            $found = Get-ChildItem -Path $_ -ErrorAction SilentlyContinue | Select-Object -First 1
            if ($found) {{
                try {{ Add-Type -Path $found.FullName -ErrorAction Stop; $reader = 'dll'; [Console]::Error.WriteLine("INFO:Using DLL $($found.FullName)") }} catch {{ }}
            }}
        }}
    }}
}}

# Auto-install SqlServer module
if ($reader -eq '') {{
    [Console]::Error.WriteLine("PROGRESS_MSG:Installing SqlServer module (first time setup)...")
    try {{
        [Net.ServicePointManager]::SecurityProtocol = [Net.SecurityProtocolType]::Tls12
        Install-Module SqlServer -Scope CurrentUser -Force -AllowClobber -ErrorAction Stop
        Import-Module SqlServer -ErrorAction Stop
        $reader = 'sqlserver'
        [Console]::Error.WriteLine("INFO:SqlServer module installed successfully")
    }} catch {{
        [Console]::Error.WriteLine("ERROR:Failed to setup XEL reader: $($_.Exception.Message)")
        exit 1
    }}
}}

if ($reader -eq '') {{
    [Console]::Error.WriteLine("ERROR:No XEL reader could be loaded")
    exit 1
}}

# Stream events as NDJSON
$count = 0
foreach ($filePath in @({paths_joined})) {{
    [Console]::Error.WriteLine("INFO:Reading $filePath")
    try {{
        $events = switch ($reader) {{
            'sqlserver' {{ Read-SqlXEvent -FileName $filePath }}
            'dbatools'  {{ Read-DbaXEFile -Path $filePath }}
            'dll'       {{ New-Object Microsoft.SqlServer.XEvent.Linq.QueryableXEventData(@(,$filePath)) }}
        }}
        foreach ($event in $events) {{
            $obj = @{{
                event_name = $event.Name
                timestamp = $event.Timestamp.ToUniversalTime().ToString("o")
            }}
            $fenum = $event.Fields.GetEnumerator()
            while ($fenum.MoveNext()) {{
                $kv = $fenum.Current
                $k = $kv.Key
                if ([string]::IsNullOrEmpty($k)) {{ continue }}
                $val = $kv.Value
                if ($null -eq $val) {{ continue }}
                if ($val -is [byte[]]) {{ continue }}
                if ($val -is [System.TimeSpan]) {{ $val = [int64]$val.TotalMicroseconds }}
                elseif ($val -is [System.DateTime]) {{ $val = $val.ToUniversalTime().ToString("o") }}
                elseif ($val -is [System.DateTimeOffset]) {{ $val = $val.UtcDateTime.ToString("o") }}
                elseif ($val -is [System.Xml.XmlDocument] -or $val -is [System.Xml.XmlNode]) {{ $val = $val.OuterXml }}
                $obj[$k] = $val
            }}
            $aenum = $event.Actions.GetEnumerator()
            while ($aenum.MoveNext()) {{
                $kv = $aenum.Current
                $k = $kv.Key
                if ([string]::IsNullOrEmpty($k)) {{ continue }}
                $val = $kv.Value
                if ($null -eq $val) {{ continue }}
                if ($val -is [System.TimeSpan]) {{ $val = [int64]$val.TotalMicroseconds }}
                elseif ($val -is [System.DateTime]) {{ $val = $val.ToUniversalTime().ToString("o") }}
                elseif ($val -is [System.DateTimeOffset]) {{ $val = $val.UtcDateTime.ToString("o") }}
                elseif ($val.GetType().Name -eq 'XEActivityId') {{ $val = $val.ToString() }}
                $obj[$k] = $val
            }}
            $obj | ConvertTo-Json -Compress -Depth 3
            $count++
            if ($count % 1000 -eq 0) {{
                [Console]::Error.WriteLine("PROGRESS:$count")
            }}
        }}
    }} catch {{
        [Console]::Error.WriteLine("ERROR:Failed reading $filePath - $($_.Exception.Message)")
        exit 1
    }}
}}
[Console]::Error.WriteLine("COMPLETE:$count")
"#
    );

    let first_file = file_paths.first().map(|s| s.as_str()).unwrap_or("unknown");
    let file_name = Path::new(first_file)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let total_bytes: u64 = file_paths
        .iter()
        .filter_map(|p| std::fs::metadata(p).ok())
        .map(|m| m.len())
        .sum();

    let _ = progress_tx.send(XelLoadProgress {
        file_name: file_name.clone(),
        events_parsed: 0,
        bytes_processed: 0,
        total_bytes,
        phase: LoadPhase::Parsing,
    }).await;

    let mut child = tokio::process::Command::new(&ps_exe)
        .args(&["-NoProfile", "-NonInteractive", "-Command", &script])
        .stdout(std::process::Stdio::piped())
        .stderr(std::process::Stdio::piped())
        .spawn()
        .map_err(|e| format!("Failed to spawn PowerShell: {}", e))?;

    let stdout = child.stdout.take().ok_or("Failed to capture stdout")?;
    let stderr = child.stderr.take().ok_or("Failed to capture stderr")?;

    // Read stderr in background for progress and errors
    let progress_tx_clone = progress_tx.clone();
    let file_name_clone = file_name.clone();
    let stderr_handle = tokio::spawn(async move {
        use tokio::io::{AsyncBufReadExt, BufReader};
        let reader = BufReader::new(stderr);
        let mut lines = reader.lines();
        let mut explicit_error: Option<String> = None;
        let mut last_stderr_lines: Vec<String> = Vec::new();

        while let Ok(Some(line)) = lines.next_line().await {
            if let Some(count_str) = line.strip_prefix("PROGRESS:") {
                if let Ok(count) = count_str.parse::<usize>() {
                    let _ = progress_tx_clone
                        .send(XelLoadProgress {
                            file_name: file_name_clone.clone(),
                            events_parsed: count,
                            bytes_processed: 0,
                            total_bytes,
                            phase: LoadPhase::Parsing,
                        })
                        .await;
                }
            } else if line.starts_with("PROGRESS_MSG:") {
                let _ = progress_tx_clone
                    .send(XelLoadProgress {
                        file_name: file_name_clone.clone(),
                        events_parsed: 0,
                        bytes_processed: 0,
                        total_bytes,
                        phase: LoadPhase::CheckingPowerShell,
                    })
                    .await;
            } else if let Some(err_msg) = line.strip_prefix("ERROR:") {
                let _ = progress_tx_clone
                    .send(XelLoadProgress {
                        file_name: file_name_clone.clone(),
                        events_parsed: 0,
                        bytes_processed: 0,
                        total_bytes,
                        phase: LoadPhase::Error,
                    })
                    .await;
                explicit_error = Some(err_msg.to_string());
            } else if !line.starts_with("INFO:") && !line.starts_with("COMPLETE:") {
                // Capture unexpected stderr (PS errors) for diagnostics
                last_stderr_lines.push(line);
                if last_stderr_lines.len() > 10 {
                    last_stderr_lines.remove(0);
                }
            }
        }

        // Return explicit error first, or fallback to raw stderr
        if let Some(err) = explicit_error {
            Some(err)
        } else if !last_stderr_lines.is_empty() {
            Some(last_stderr_lines.join("\n"))
        } else {
            None
        }
    });

    // Read stdout as raw bytes — handle encoding errors gracefully
    let mut events = Vec::new();
    let mut parse_errors = 0u64;
    let mut first_error: Option<String> = None;
    let mut first_bad_line: Option<String> = None;
    {
        use tokio::io::{AsyncReadExt, BufReader};
        let mut reader = BufReader::new(stdout);
        let mut buf = Vec::with_capacity(64 * 1024);
        let mut leftover = Vec::new();

        loop {
            buf.clear();
            buf.extend_from_slice(&leftover);
            leftover.clear();

            let mut tmp = [0u8; 32 * 1024];
            let n = match reader.read(&mut tmp).await {
                Ok(0) => break,   // EOF
                Ok(n) => n,
                Err(_) => break,
            };
            buf.extend_from_slice(&tmp[..n]);

            // Split on newlines, keep leftover (incomplete last line)
            let mut start = 0;
            for i in 0..buf.len() {
                if buf[i] == b'\n' {
                    let line_bytes = &buf[start..i];
                    start = i + 1;

                    // Convert to string, replacing invalid UTF-8
                    let line = String::from_utf8_lossy(line_bytes).trim().to_string();
                    if line.is_empty() {
                        continue;
                    }

                    match parse_powershell_event(&line, file_paths) {
                        Ok(event) => events.push(event),
                        Err(e) => {
                            parse_errors += 1;
                            if first_error.is_none() {
                                first_error = Some(e);
                                first_bad_line = Some(
                                    if line.len() > 200 {
                                        format!("{}...", &line[..200])
                                    } else {
                                        line
                                    },
                                );
                            }
                        }
                    }
                }
            }
            // Keep remaining bytes for next iteration
            leftover = buf[start..].to_vec();
        }

        // Process any remaining data
        if !leftover.is_empty() {
            let line = String::from_utf8_lossy(&leftover).trim().to_string();
            if !line.is_empty() {
                match parse_powershell_event(&line, file_paths) {
                    Ok(event) => events.push(event),
                    Err(_) => { parse_errors += 1; }
                }
            }
        }
    }

    let status = child
        .wait()
        .await
        .map_err(|e| format!("PowerShell process error: {}", e))?;

    // Check stderr for errors
    let stderr_result = stderr_handle.await.unwrap_or(None);

    if !status.success() && events.is_empty() {
        let detail = stderr_result.unwrap_or_else(|| format!("exit code: {}", status));
        return Err(format!("PowerShell error: {}", detail));
    }

    if let Some(err) = stderr_result {
        if events.is_empty() {
            return Err(format!("PowerShell error: {}", err));
        }
    }

    // Report if many lines failed to parse
    if parse_errors > 0 && events.len() < 10 {
        let detail = format!(
            "Parsed {} events but {} lines failed. First error: {}. Line: {}",
            events.len(),
            parse_errors,
            first_error.unwrap_or_default(),
            first_bad_line.unwrap_or_default()
        );
        return Err(detail);
    }

    let _ = progress_tx.send(XelLoadProgress {
        file_name,
        events_parsed: events.len(),
        bytes_processed: total_bytes,
        total_bytes,
        phase: LoadPhase::Complete,
    }).await;

    Ok(events)
}

fn parse_powershell_event(
    json_line: &str,
    file_paths: &[String],
) -> Result<XelEvent, String> {
    let raw: HashMap<String, serde_json::Value> =
        serde_json::from_str(json_line).map_err(|e| format!("JSON parse error: {}", e))?;

    let event_name = raw
        .get("event_name")
        .and_then(|v| v.as_str())
        .unwrap_or("unknown")
        .to_string();

    let timestamp = raw
        .get("timestamp")
        .and_then(|v| v.as_str())
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|dt| dt.with_timezone(&Utc))
        .unwrap_or_else(Utc::now);

    let source_file = file_paths.first().cloned().unwrap_or_default();

    let get_str = |key: &str| -> Option<String> {
        raw.get(key).and_then(|v| match v {
            serde_json::Value::String(s) => Some(s.clone()),
            serde_json::Value::Number(n) => Some(n.to_string()),
            _ => v.as_str().map(|s| s.to_string()),
        })
    };

    let get_i64 = |key: &str| -> Option<i64> {
        raw.get(key).and_then(|v| match v {
            serde_json::Value::Number(n) => n.as_i64(),
            serde_json::Value::String(s) => s.parse().ok(),
            _ => None,
        })
    };

    // Collect extra fields not in our known set
    let known_keys: HashSet<&str> = [
        "event_name", "timestamp", "session_id", "duration", "cpu_time",
        "logical_reads", "physical_reads", "writes", "result", "statement",
        "sql_text", "object_name", "client_app_name", "username",
        "database_name", "resource_type", "mode", "resource_description",
        "wait_type", "wait_duration", "blocked_process_report", "blocked_process",
        "deadlock_graph", "xml_deadlock_report", "xml_report",
    ]
    .into_iter()
    .collect();

    let extra_fields: HashMap<String, serde_json::Value> = raw
        .iter()
        .filter(|(k, _)| !known_keys.contains(k.as_str()))
        .map(|(k, v)| (k.clone(), v.clone()))
        .collect();

    Ok(XelEvent {
        id: 0, // assigned by store
        source_file,
        event_name,
        timestamp,
        session_id: get_i64("session_id"),
        duration_us: get_i64("duration"),
        cpu_time_us: get_i64("cpu_time"),
        logical_reads: get_i64("logical_reads"),
        physical_reads: get_i64("physical_reads"),
        writes: get_i64("writes"),
        result: get_str("result"),
        statement: get_str("statement"),
        sql_text: get_str("sql_text"),
        object_name: get_str("object_name"),
        client_app_name: get_str("client_app_name"),
        username: get_str("username"),
        database_name: get_str("database_name"),
        resource_type: get_str("resource_type"),
        lock_mode: get_str("mode"),
        resource_description: get_str("resource_description"),
        wait_type: get_str("wait_type"),
        wait_duration_ms: get_i64("wait_duration"),
        blocked_process_report: get_str("blocked_process_report")
            .or_else(|| get_str("blocked_process")),
        deadlock_graph: get_str("deadlock_graph")
            .or_else(|| get_str("xml_deadlock_report"))
            .or_else(|| get_str("xml_report")),
        extra_fields,
    })
}

/// Parse pre-exported XML files (fallback when PowerShell module not available)
pub async fn parse_xml_file(
    file_path: &str,
    progress_tx: mpsc::Sender<XelLoadProgress>,
) -> Result<Vec<XelEvent>, String> {
    use quick_xml::events::Event as XmlEvent;
    use quick_xml::reader::Reader;

    let file_name = Path::new(file_path)
        .file_name()
        .and_then(|n| n.to_str())
        .unwrap_or("unknown")
        .to_string();

    let content = tokio::fs::read_to_string(file_path)
        .await
        .map_err(|e| format!("Failed to read file: {}", e))?;

    let total_bytes = content.len() as u64;

    let _ = progress_tx.send(XelLoadProgress {
        file_name: file_name.clone(),
        events_parsed: 0,
        bytes_processed: 0,
        total_bytes,
        phase: LoadPhase::Parsing,
    }).await;

    let mut reader = Reader::from_str(&content);
    reader.config_mut().trim_text(true);

    let mut events = Vec::new();
    let mut current_event: Option<XelEventBuilder> = None;
    let mut current_data_name: Option<String> = None;
    let mut current_action_name: Option<String> = None;
    let mut in_value = false;

    loop {
        match reader.read_event() {
            Ok(XmlEvent::Start(ref e)) | Ok(XmlEvent::Empty(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();

                match tag_name.as_str() {
                    "event" => {
                        let mut builder = XelEventBuilder::new(file_path.to_string());
                        for attr in e.attributes().flatten() {
                            let key =
                                String::from_utf8_lossy(attr.key.as_ref()).to_string();
                            let val =
                                String::from_utf8_lossy(&attr.value).to_string();
                            match key.as_str() {
                                "name" => builder.event_name = val,
                                "timestamp" => builder.timestamp_str = Some(val),
                                _ => {}
                            }
                        }
                        current_event = Some(builder);
                    }
                    "data" => {
                        if current_event.is_some() {
                            for attr in e.attributes().flatten() {
                                let key =
                                    String::from_utf8_lossy(attr.key.as_ref())
                                        .to_string();
                                if key == "name" {
                                    current_data_name = Some(
                                        String::from_utf8_lossy(&attr.value)
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                    "action" => {
                        if current_event.is_some() {
                            for attr in e.attributes().flatten() {
                                let key =
                                    String::from_utf8_lossy(attr.key.as_ref())
                                        .to_string();
                                if key == "name" {
                                    current_action_name = Some(
                                        String::from_utf8_lossy(&attr.value)
                                            .to_string(),
                                    );
                                }
                            }
                        }
                    }
                    "value" | "text" => {
                        in_value = true;
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Text(ref e)) => {
                if in_value {
                    if let Some(ref mut builder) = current_event {
                        let text = e.unescape().unwrap_or_default().to_string();
                        let field_name = current_data_name
                            .as_deref()
                            .or(current_action_name.as_deref());

                        if let Some(name) = field_name {
                            builder.set_field(name, &text);
                        }
                    }
                }
            }
            Ok(XmlEvent::End(ref e)) => {
                let tag_name = String::from_utf8_lossy(e.name().as_ref()).to_string();
                match tag_name.as_str() {
                    "event" => {
                        if let Some(builder) = current_event.take() {
                            events.push(builder.build());
                            if events.len() % 1000 == 0 {
                                let _ = progress_tx
                                    .send(XelLoadProgress {
                                        file_name: file_name.clone(),
                                        events_parsed: events.len(),
                                        bytes_processed: reader.buffer_position()
                                            as u64,
                                        total_bytes,
                                        phase: LoadPhase::Parsing,
                                    })
                                    .await;
                            }
                        }
                    }
                    "data" => {
                        current_data_name = None;
                    }
                    "action" => {
                        current_action_name = None;
                    }
                    "value" | "text" => {
                        in_value = false;
                    }
                    _ => {}
                }
            }
            Ok(XmlEvent::Eof) => break,
            Err(e) => {
                return Err(format!("XML parse error at position {}: {}", reader.buffer_position(), e));
            }
            _ => {}
        }
    }

    let _ = progress_tx.send(XelLoadProgress {
        file_name,
        events_parsed: events.len(),
        bytes_processed: total_bytes,
        total_bytes,
        phase: LoadPhase::Complete,
    }).await;

    Ok(events)
}

struct XelEventBuilder {
    source_file: String,
    event_name: String,
    timestamp_str: Option<String>,
    fields: HashMap<String, String>,
}

impl XelEventBuilder {
    fn new(source_file: String) -> Self {
        Self {
            source_file,
            event_name: String::new(),
            timestamp_str: None,
            fields: HashMap::new(),
        }
    }

    fn set_field(&mut self, name: &str, value: &str) {
        self.fields.insert(name.to_string(), value.to_string());
    }

    fn get_str(&self, key: &str) -> Option<String> {
        self.fields.get(key).cloned()
    }

    fn get_i64(&self, key: &str) -> Option<i64> {
        self.fields.get(key).and_then(|v| v.parse().ok())
    }

    fn build(self) -> XelEvent {
        let timestamp = self
            .timestamp_str
            .as_deref()
            .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
            .map(|dt| dt.with_timezone(&Utc))
            .or_else(|| {
                self.timestamp_str
                    .as_deref()
                    .and_then(|s| {
                        chrono::NaiveDateTime::parse_from_str(s, "%Y-%m-%dT%H:%M:%S%.f")
                            .ok()
                    })
                    .map(|ndt| ndt.and_utc())
            })
            .unwrap_or_else(Utc::now);

        // Collect extra fields
        let known: HashSet<&str> = [
            "duration", "cpu_time", "logical_reads", "physical_reads", "writes",
            "result", "statement", "sql_text", "object_name", "client_app_name",
            "username", "database_name", "resource_type", "mode",
            "resource_description", "wait_type", "wait_duration",
            "blocked_process_report", "blocked_process", "deadlock_graph",
            "xml_deadlock_report", "xml_report", "session_id",
        ]
        .into_iter()
        .collect();

        // Extract all values before moving self fields
        let session_id = self.get_i64("session_id");
        let duration_us = self.get_i64("duration");
        let cpu_time_us = self.get_i64("cpu_time");
        let logical_reads = self.get_i64("logical_reads");
        let physical_reads = self.get_i64("physical_reads");
        let writes = self.get_i64("writes");
        let result = self.get_str("result");
        let statement = self.get_str("statement");
        let sql_text = self.get_str("sql_text");
        let object_name = self.get_str("object_name");
        let client_app_name = self.get_str("client_app_name");
        let username = self.get_str("username");
        let database_name = self.get_str("database_name");
        let resource_type = self.get_str("resource_type");
        let lock_mode = self.get_str("mode");
        let resource_description = self.get_str("resource_description");
        let wait_type = self.get_str("wait_type");
        let wait_duration_ms = self.get_i64("wait_duration");
        let blocked_process_report = self.get_str("blocked_process_report")
            .or_else(|| self.get_str("blocked_process"));
        let deadlock_graph = self
            .get_str("deadlock_graph")
            .or_else(|| self.get_str("xml_deadlock_report"))
            .or_else(|| self.get_str("xml_report"));

        let extra: HashMap<String, serde_json::Value> = self
            .fields
            .iter()
            .filter(|(k, _)| !known.contains(k.as_str()))
            .map(|(k, v)| (k.clone(), serde_json::Value::String(v.clone())))
            .collect();

        XelEvent {
            id: 0,
            source_file: self.source_file,
            event_name: self.event_name,
            timestamp,
            session_id,
            duration_us,
            cpu_time_us,
            logical_reads,
            physical_reads,
            writes,
            result,
            statement,
            sql_text,
            object_name,
            client_app_name,
            username,
            database_name,
            resource_type,
            lock_mode,
            resource_description,
            wait_type,
            wait_duration_ms,
            blocked_process_report,
            deadlock_graph,
            extra_fields: extra,
        }
    }
}
