use crate::format::format_duration;
use crate::types::ProcessedEntry;
use serde::Serialize;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputFormat {
    Human,
    Json,
}

impl OutputFormat {
    pub fn from_str_opt(s: Option<&str>) -> Self {
        match s {
            Some("json") => Self::Json,
            _ => Self::Human,
        }
    }
}

#[derive(Serialize)]
pub struct ListEntry {
    pub uuid: String,
    pub date: String,
    pub duration: String,
    pub duration_secs: f64,
    pub title: String,
    pub status: String,
    pub method: Option<String>,
    pub words: Option<usize>,
    pub file: Option<String>,
}

#[derive(Serialize)]
pub struct ShowEntry {
    pub uuid: String,
    pub date: String,
    pub duration: String,
    pub duration_secs: f64,
    pub title: String,
    pub words: usize,
    pub file: String,
    pub transcript: String,
}

#[derive(Serialize)]
pub struct ExtractResult {
    pub extracted: usize,
    pub skipped: usize,
    pub needs_whisply: usize,
    pub files: Vec<ExtractedFile>,
}

#[derive(Serialize)]
pub struct ExtractedFile {
    pub uuid: String,
    pub title: String,
    pub method: String,
    pub words: usize,
    pub file: String,
}

#[derive(Serialize)]
pub struct DryRunResult {
    pub total: usize,
    pub recordings: Vec<DryRunEntry>,
}

#[derive(Serialize)]
pub struct DryRunEntry {
    pub uuid: String,
    pub title: String,
    pub date: String,
    pub duration: String,
    pub has_tsrp: bool,
}

pub fn format_dry_run_human(result: &DryRunResult) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(out, "Dry run: {} recording(s) would be processed\n", result.total).unwrap();
    for e in &result.recordings {
        let tsrp_status = if e.has_tsrp { "tsrp available" } else { "needs whisply" };
        writeln!(out, "  {} {} ({}, {})", e.date, e.title, e.duration, tsrp_status).unwrap();
    }
    out
}

pub fn format_dry_run_json(result: &DryRunResult) -> String {
    serde_json::to_string_pretty(result).unwrap()
}

pub fn build_list_entry(
    uuid: &str,
    date: &str,
    duration_secs: f64,
    title: &str,
    processed: Option<&ProcessedEntry>,
) -> ListEntry {
    let duration = format_duration(duration_secs);
    match processed {
        Some(e) if e.method == "tsrp" || e.method == "whisply" => ListEntry {
            uuid: uuid.to_string(),
            date: date.to_string(),
            duration,
            duration_secs,
            title: title.to_string(),
            status: "done".to_string(),
            method: Some(e.method.clone()),
            words: Some(e.words),
            file: e.output.clone(),
        },
        Some(e) if e.method == "no-transcript" => ListEntry {
            uuid: uuid.to_string(),
            date: date.to_string(),
            duration,
            duration_secs,
            title: title.to_string(),
            status: "needs-whisply".to_string(),
            method: None,
            words: None,
            file: None,
        },
        _ => ListEntry {
            uuid: uuid.to_string(),
            date: date.to_string(),
            duration,
            duration_secs,
            title: title.to_string(),
            status: "pending".to_string(),
            method: None,
            words: None,
            file: None,
        },
    }
}

pub fn format_list_human(entries: &[ListEntry]) -> String {
    use std::fmt::Write;
    let mut out = String::new();
    writeln!(
        out,
        "{:<20} {:>8}   {:<16} {:>5}   Title",
        "Date", "Duration", "Status", "Words"
    )
    .unwrap();
    writeln!(out, "{}", "\u{2500}".repeat(80)).unwrap();

    for e in entries {
        let status_display = match e.status.as_str() {
            "done" => format!("\u{2713} {}", e.method.as_deref().unwrap_or("")),
            "needs-whisply" => "\u{25CB} needs --all".to_string(),
            _ => "\u{25CB} pending".to_string(),
        };
        let words_display = e.words.map_or("\u{2014}".to_string(), |w| w.to_string());
        let title: String = e.title.chars().take(35).collect();
        writeln!(
            out,
            "{:<20} {:>8}   {:<16} {:>5}   {title}",
            e.date, e.duration, status_display, words_display
        )
        .unwrap();
    }
    out
}

pub fn format_list_json(entries: &[ListEntry]) -> String {
    serde_json::to_string_pretty(entries).unwrap()
}

pub fn format_show_human(entries: &[ShowEntry]) -> String {
    use std::fmt::Write;
    let mut out = String::new();

    for e in entries {
        writeln!(out, "\n{}", "=".repeat(70)).unwrap();
        writeln!(
            out,
            "{}  ({}, {}, {} words)",
            e.title, e.date, e.duration, e.words
        )
        .unwrap();
        writeln!(out, "{}", "=".repeat(70)).unwrap();

        if e.transcript.len() > 3000 {
            writeln!(out, "{}", &e.transcript[..3000]).unwrap();
            writeln!(
                out,
                "\n... [{} chars truncated, see {}]",
                e.transcript.len() - 3000,
                e.file
            )
            .unwrap();
        } else {
            writeln!(out, "{}", e.transcript).unwrap();
        }
    }

    if entries.is_empty() {
        writeln!(out, "No transcripts available. Run `voice-memos extract` first.").unwrap();
    }
    out
}

pub fn format_show_json(entries: &[ShowEntry]) -> String {
    serde_json::to_string_pretty(entries).unwrap()
}

pub fn format_extract_human(result: &ExtractResult) -> String {
    format!(
        "Done: {} extracted, {} skipped, {} need --all for whisply",
        result.extracted, result.skipped, result.needs_whisply
    )
}

pub fn format_extract_json(result: &ExtractResult) -> String {
    serde_json::to_string_pretty(result).unwrap()
}

pub fn format_list_ndjson(entries: &[ListEntry]) -> String {
    entries
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn format_show_ndjson(entries: &[ShowEntry]) -> String {
    entries
        .iter()
        .map(|e| serde_json::to_string(e).unwrap())
        .collect::<Vec<_>>()
        .join("\n")
        + "\n"
}

pub fn filter_json_fields(json: &str, fields: &[String]) -> String {
    if fields.is_empty() {
        return json.to_string();
    }

    let val: serde_json::Value = serde_json::from_str(json).unwrap();

    let filter_obj = |obj: &serde_json::Map<String, serde_json::Value>| -> serde_json::Value {
        let filtered: serde_json::Map<String, serde_json::Value> = obj
            .iter()
            .filter(|(k, _)| fields.iter().any(|f| f == *k))
            .map(|(k, v)| (k.clone(), v.clone()))
            .collect();
        serde_json::Value::Object(filtered)
    };

    let result = match val {
        serde_json::Value::Array(arr) => {
            let filtered: Vec<serde_json::Value> = arr
                .iter()
                .map(|item| {
                    if let Some(obj) = item.as_object() {
                        filter_obj(obj)
                    } else {
                        item.clone()
                    }
                })
                .collect();
            serde_json::Value::Array(filtered)
        }
        serde_json::Value::Object(ref obj) => filter_obj(obj),
        other => other,
    };

    serde_json::to_string_pretty(&result).unwrap()
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- OutputFormat ---

    #[test]
    fn output_format_from_none() {
        assert_eq!(OutputFormat::from_str_opt(None), OutputFormat::Human);
    }

    #[test]
    fn output_format_from_json() {
        assert_eq!(OutputFormat::from_str_opt(Some("json")), OutputFormat::Json);
    }

    #[test]
    fn output_format_from_unknown() {
        assert_eq!(OutputFormat::from_str_opt(Some("xml")), OutputFormat::Human);
    }

    // --- build_list_entry ---

    #[test]
    fn build_list_entry_pending() {
        let entry = build_list_entry("uuid-1", "2024-01-15 10:30", 125.0, "My Memo", None);
        assert_eq!(entry.uuid, "uuid-1");
        assert_eq!(entry.status, "pending");
        assert_eq!(entry.duration, "2m05s");
        assert!(entry.method.is_none());
        assert!(entry.words.is_none());
    }

    #[test]
    fn build_list_entry_processed_tsrp() {
        let pe = ProcessedEntry {
            date: "2024-01-15 10:30".to_string(),
            title: "My Memo".to_string(),
            method: "tsrp".to_string(),
            words: 42,
            output: Some("2024-01-15-my-memo.md".to_string()),
        };
        let entry = build_list_entry("uuid-1", "2024-01-15 10:30", 125.0, "My Memo", Some(&pe));
        assert_eq!(entry.status, "done");
        assert_eq!(entry.method, Some("tsrp".to_string()));
        assert_eq!(entry.words, Some(42));
        assert_eq!(entry.file, Some("2024-01-15-my-memo.md".to_string()));
    }

    #[test]
    fn build_list_entry_no_transcript() {
        let pe = ProcessedEntry {
            date: "2024-01-15 10:30".to_string(),
            title: "My Memo".to_string(),
            method: "no-transcript".to_string(),
            words: 0,
            output: None,
        };
        let entry = build_list_entry("uuid-1", "2024-01-15 10:30", 125.0, "My Memo", Some(&pe));
        assert_eq!(entry.status, "needs-whisply");
    }

    // --- format_list_json ---

    #[test]
    fn format_list_json_valid() {
        let entries = vec![ListEntry {
            uuid: "uuid-1".to_string(),
            date: "2024-01-15 10:30".to_string(),
            duration: "2m05s".to_string(),
            duration_secs: 125.0,
            title: "My Memo".to_string(),
            status: "pending".to_string(),
            method: None,
            words: None,
            file: None,
        }];
        let json = format_list_json(&entries);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert!(parsed.is_array());
        assert_eq!(parsed[0]["uuid"], "uuid-1");
    }

    // --- format_list_human ---

    #[test]
    fn format_list_human_contains_header() {
        let entries = vec![ListEntry {
            uuid: "uuid-1".to_string(),
            date: "2024-01-15 10:30".to_string(),
            duration: "2m05s".to_string(),
            duration_secs: 125.0,
            title: "My Memo".to_string(),
            status: "pending".to_string(),
            method: None,
            words: None,
            file: None,
        }];
        let output = format_list_human(&entries);
        assert!(output.contains("Date"));
        assert!(output.contains("My Memo"));
    }

    // --- format_show_json ---

    #[test]
    fn format_show_json_valid() {
        let entries = vec![ShowEntry {
            uuid: "uuid-1".to_string(),
            date: "2024-01-15 10:30".to_string(),
            duration: "2m05s".to_string(),
            duration_secs: 125.0,
            title: "My Memo".to_string(),
            words: 42,
            file: "2024-01-15-my-memo.md".to_string(),
            transcript: "Hello world".to_string(),
        }];
        let json = format_show_json(&entries);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed[0]["transcript"], "Hello world");
    }

    // --- format_show_human ---

    #[test]
    fn format_show_human_truncates_long_transcripts() {
        let long_text = "word ".repeat(1000);
        let entries = vec![ShowEntry {
            uuid: "uuid-1".to_string(),
            date: "2024-01-15 10:30".to_string(),
            duration: "2m05s".to_string(),
            duration_secs: 125.0,
            title: "My Memo".to_string(),
            words: 1000,
            file: "test.md".to_string(),
            transcript: long_text,
        }];
        let output = format_show_human(&entries);
        assert!(output.contains("truncated"));
    }

    // --- format_extract_json ---

    #[test]
    fn format_extract_json_valid() {
        let result = ExtractResult {
            extracted: 2,
            skipped: 1,
            needs_whisply: 0,
            files: vec![ExtractedFile {
                uuid: "uuid-1".to_string(),
                title: "Memo".to_string(),
                method: "tsrp".to_string(),
                words: 50,
                file: "2024-01-15-memo.md".to_string(),
            }],
        };
        let json = format_extract_json(&result);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["extracted"], 2);
        assert_eq!(parsed["files"][0]["words"], 50);
    }

    // --- format_extract_human ---

    #[test]
    fn format_extract_human_summary() {
        let result = ExtractResult {
            extracted: 2,
            skipped: 1,
            needs_whisply: 3,
            files: vec![],
        };
        let output = format_extract_human(&result);
        assert!(output.contains("2 extracted"));
        assert!(output.contains("1 skipped"));
        assert!(output.contains("3 need --all"));
    }

    // --- dry run ---

    #[test]
    fn format_dry_run_json_valid() {
        let result = DryRunResult {
            total: 2,
            recordings: vec![
                DryRunEntry {
                    uuid: "uuid-1".to_string(),
                    title: "Memo A".to_string(),
                    date: "2024-01-15 10:30".to_string(),
                    duration: "2m05s".to_string(),
                    has_tsrp: true,
                },
                DryRunEntry {
                    uuid: "uuid-2".to_string(),
                    title: "Memo B".to_string(),
                    date: "2024-01-16 11:00".to_string(),
                    duration: "5m30s".to_string(),
                    has_tsrp: false,
                },
            ],
        };
        let json = format_dry_run_json(&result);
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["total"], 2);
        assert_eq!(parsed["recordings"][0]["has_tsrp"], true);
        assert_eq!(parsed["recordings"][1]["has_tsrp"], false);
    }

    #[test]
    fn format_dry_run_human_shows_count() {
        let result = DryRunResult {
            total: 1,
            recordings: vec![DryRunEntry {
                uuid: "uuid-1".to_string(),
                title: "Test Memo".to_string(),
                date: "2024-01-15 10:30".to_string(),
                duration: "1m00s".to_string(),
                has_tsrp: true,
            }],
        };
        let output = format_dry_run_human(&result);
        assert!(output.contains("1 recording(s) would be processed"));
        assert!(output.contains("Test Memo"));
        assert!(output.contains("tsrp available"));
    }

    #[test]
    fn format_dry_run_human_shows_needs_whisply() {
        let result = DryRunResult {
            total: 1,
            recordings: vec![DryRunEntry {
                uuid: "uuid-1".to_string(),
                title: "No Tsrp".to_string(),
                date: "2024-01-15 10:30".to_string(),
                duration: "3m00s".to_string(),
                has_tsrp: false,
            }],
        };
        let output = format_dry_run_human(&result);
        assert!(output.contains("needs whisply"));
    }

    // --- filter_json_fields ---

    #[test]
    fn filter_json_fields_array() {
        let json = r#"[{"uuid":"1","title":"A","date":"2024-01-15","words":42}]"#;
        let filtered = filter_json_fields(json, &["title".to_string(), "words".to_string()]);
        let parsed: serde_json::Value = serde_json::from_str(&filtered).unwrap();
        assert_eq!(parsed[0]["title"], "A");
        assert_eq!(parsed[0]["words"], 42);
        assert!(parsed[0].get("uuid").is_none());
        assert!(parsed[0].get("date").is_none());
    }

    #[test]
    fn filter_json_fields_object() {
        let json = r#"{"extracted":2,"skipped":1,"files":[]}"#;
        let filtered = filter_json_fields(json, &["extracted".to_string()]);
        let parsed: serde_json::Value = serde_json::from_str(&filtered).unwrap();
        assert_eq!(parsed["extracted"], 2);
        assert!(parsed.get("skipped").is_none());
    }

    #[test]
    fn filter_json_fields_empty_fields_returns_all() {
        let json = r#"{"a":1,"b":2}"#;
        let filtered = filter_json_fields(json, &[]);
        let parsed: serde_json::Value = serde_json::from_str(&filtered).unwrap();
        assert_eq!(parsed["a"], 1);
        assert_eq!(parsed["b"], 2);
    }

    // --- NDJSON ---

    #[test]
    fn format_list_ndjson_one_per_line() {
        let entries = vec![
            ListEntry {
                uuid: "uuid-1".to_string(),
                date: "2024-01-15".to_string(),
                duration: "1m00s".to_string(),
                duration_secs: 60.0,
                title: "A".to_string(),
                status: "pending".to_string(),
                method: None,
                words: None,
                file: None,
            },
            ListEntry {
                uuid: "uuid-2".to_string(),
                date: "2024-01-16".to_string(),
                duration: "2m00s".to_string(),
                duration_secs: 120.0,
                title: "B".to_string(),
                status: "done".to_string(),
                method: Some("tsrp".to_string()),
                words: Some(50),
                file: Some("b.md".to_string()),
            },
        ];
        let output = format_list_ndjson(&entries);
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 2);
        let a: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        let b: serde_json::Value = serde_json::from_str(lines[1]).unwrap();
        assert_eq!(a["uuid"], "uuid-1");
        assert_eq!(b["uuid"], "uuid-2");
    }

    #[test]
    fn format_show_ndjson_one_per_line() {
        let entries = vec![ShowEntry {
            uuid: "uuid-1".to_string(),
            date: "2024-01-15".to_string(),
            duration: "1m00s".to_string(),
            duration_secs: 60.0,
            title: "A".to_string(),
            words: 10,
            file: "a.md".to_string(),
            transcript: "Hello".to_string(),
        }];
        let output = format_show_ndjson(&entries);
        let lines: Vec<&str> = output.trim().lines().collect();
        assert_eq!(lines.len(), 1);
        let parsed: serde_json::Value = serde_json::from_str(lines[0]).unwrap();
        assert_eq!(parsed["transcript"], "Hello");
    }
}
