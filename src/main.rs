pub mod format;
pub mod output;
pub mod schema;
pub mod state;
pub mod tsrp;
pub mod types;
pub mod validate;

use anyhow::{bail, Context, Result};
use chrono::{Local, TimeZone, Utc};
use clap::{Parser, Subcommand, ValueEnum};
use format::{format_duration, slugify};
use output::{
    build_list_entry, filter_json_fields, format_dry_run_human, format_dry_run_json,
    format_extract_human, format_extract_json, format_list_human, format_list_json,
    format_list_ndjson, format_show_human, format_show_json, format_show_ndjson, DryRunEntry,
    DryRunResult, ExtractResult, ExtractedFile, ShowEntry,
};
use rusqlite::{Connection, OpenFlags};
use state::{load_state, save_state};
use std::fs;
use std::path::PathBuf;
use std::process::Command;
use tsrp::{find_tsrp, parse_tsrp};
use types::{ProcessedEntry, Recording};

const DB_REL: &str =
    "Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings/CloudRecordings.db";
const PLIST_LABEL: &str = "com.karbassi.voice-memos";
/// Core Data epoch: 2001-01-01T00:00:00Z
const CORE_DATA_EPOCH: i64 = 978_307_200;

fn default_out_dir() -> PathBuf {
    dirs::home_dir()
        .expect("no home directory")
        .join("Projects/personal/assistant/transcripts/voice")
}

#[derive(Parser)]
#[command(name = "voice-memos", about = "Extract transcripts from Apple Voice Memos")]
struct Cli {
    /// Output directory
    #[arg(long, default_value_os_t = default_out_dir())]
    dir: PathBuf,

    /// Output format [env: OUTPUT_FORMAT=] [default: ndjson when piped, human otherwise]
    #[arg(long)]
    output: Option<OutputArg>,

    /// Comma-separated list of fields to include in JSON output
    #[arg(long, value_delimiter = ',')]
    fields: Vec<String>,

    #[command(subcommand)]
    command: Commands,
}

#[derive(Clone, ValueEnum)]
enum OutputArg {
    Human,
    Json,
    Ndjson,
}

#[derive(Subcommand)]
enum Commands {
    /// Extract new transcripts
    Extract {
        /// Use whisply for recordings without tsrp
        #[arg(long)]
        all: bool,
        /// Re-process all recordings
        #[arg(long)]
        force: bool,
        /// Preview what would be processed without writing files
        #[arg(long)]
        dry_run: bool,
        /// Only include recordings in this folder
        #[arg(long)]
        folder: Option<String>,
    },
    /// List all recordings and their status
    List {
        /// Only include recordings in this folder
        #[arg(long)]
        folder: Option<String>,
    },
    /// Show recent transcripts
    Show {
        /// Number of transcripts to show
        #[arg(short = 'n', long, default_value_t = 5)]
        limit: usize,
    },
    /// Show output schema for a command (or list all commands)
    Schema {
        /// Command name (list, show, extract). Omit to list all.
        command: Option<String>,
    },
    /// Manage launchd watcher
    Watch {
        #[arg(value_parser = ["install", "uninstall", "status"])]
        action: String,
    },
}

fn print_json(json: &str, fields: &[String]) {
    if fields.is_empty() {
        print!("{json}");
    } else {
        print!("{}", filter_json_fields(json, fields));
    }
}

fn recordings_dir() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("cannot determine home directory")?
        .join("Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings"))
}

fn db_path() -> Result<PathBuf> {
    Ok(dirs::home_dir()
        .context("cannot determine home directory")?
        .join(DB_REL))
}

fn get_recordings() -> Result<Vec<Recording>> {
    let src = db_path()?;
    let conn = Connection::open_with_flags(&src, OpenFlags::SQLITE_OPEN_READ_ONLY)
        .context("failed to open Voice Memos database")?;
    let mut stmt = conn
        .prepare(
            "SELECT r.ZUNIQUEID, r.ZENCRYPTEDTITLE, r.ZPATH, r.ZDURATION, r.ZDATE, r.ZCUSTOMLABEL, \
             f.ZNAME, r.ZEVICTIONDATE \
             FROM ZCLOUDRECORDING r \
             LEFT JOIN ZFOLDER f ON r.ZFOLDER = f.Z_PK \
             ORDER BY r.ZDATE DESC",
        )
        .context("failed to query recordings table")?;

    let rows = stmt
        .query_map([], |row| {
            let uuid: String = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let path: String = row.get(2)?;
            let duration: f64 = row.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
            let zdate: f64 = row.get::<_, Option<f64>>(4)?.unwrap_or(0.0);
            let custom_label: Option<String> = row.get(5)?;
            let folder: Option<String> = row.get(6)?;
            let eviction_date: Option<f64> = row.get(7)?;

            let ts = CORE_DATA_EPOCH + zdate as i64;
            let dt = Utc
                .timestamp_opt(ts, 0)
                .single()
                .unwrap_or_default()
                .with_timezone(&Local);
            let title = title
                .filter(|s| !s.is_empty())
                .or(custom_label)
                .unwrap_or_else(|| "Untitled".to_string());

            Ok(Recording {
                uuid,
                title,
                path,
                duration,
                date: dt,
                folder,
                evicted: eviction_date.is_some(),
            })
        })
        .context("failed to query recordings")?;

    Ok(rows.filter_map(|r| r.ok()).collect())
}

fn extract_transcript_tsrp(m4a_path: &PathBuf) -> Option<String> {
    let data = fs::read(m4a_path).ok()?;
    let payload = find_tsrp(&data)?;
    parse_tsrp(payload)
}

fn transcribe_whisply(m4a_path: &PathBuf) -> Option<String> {
    let token_output = Command::new("op")
        .args(["read", "op://homelab/AI Assistant/HuggingFace/token"])
        .output()
        .ok()?;
    if !token_output.status.success() {
        return None;
    }
    let hf_token = String::from_utf8_lossy(&token_output.stdout)
        .trim()
        .to_string();
    if hf_token.is_empty() {
        return None;
    }

    let tmp_out = PathBuf::from("/tmp/whisply_out");
    fs::create_dir_all(&tmp_out).ok()?;

    Command::new("whisply")
        .args([
            "run",
            "--file",
            &m4a_path.to_string_lossy(),
            "--output_dir",
            &tmp_out.to_string_lossy(),
            "--output_format",
            "txt",
            "--hf_token",
            &hf_token,
        ])
        .output()
        .ok()?;

    let txt = fs::read_dir(&tmp_out)
        .ok()?
        .filter_map(|e| e.ok())
        .find(|e| e.path().extension().is_some_and(|ext| ext == "txt"))?;
    let content = fs::read_to_string(txt.path()).ok()?;
    let trimmed = content.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

fn write_transcript(
    out: &PathBuf,
    rec: &Recording,
    transcript: &str,
    method: &str,
) -> Result<PathBuf> {
    let date_str = rec.date.format("%Y-%m-%d").to_string();
    let slug = slugify(&rec.title);
    let base = if slug.is_empty() {
        format!("{date_str}-untitled")
    } else {
        format!("{date_str}-{slug}")
    };

    let mut filename = format!("{base}.md");
    let mut out_path = out.join(&filename);
    let mut counter = 1u32;
    while out_path.exists() {
        counter += 1;
        filename = format!("{base}-{counter}.md");
        out_path = out.join(&filename);
    }

    let word_count = transcript.split_whitespace().count();
    let content = format!(
        "---\ndate: {}\nduration: {}\nlocation: {}\nsource: {}\nwords: {}\nfile: {}\n---\n\n{}\n",
        rec.date.format("%Y-%m-%d %H:%M"),
        format_duration(rec.duration),
        rec.title,
        method,
        word_count,
        rec.path,
        transcript
    );
    fs::write(&out_path, content)
        .with_context(|| format!("failed to write transcript: {}", out_path.display()))?;
    Ok(out_path)
}

fn cmd_extract_dry_run(
    out: &PathBuf,
    force: bool,
    json: bool,
    fields: &[String],
    folder_filter: Option<&str>,
) -> Result<()> {
    let state = load_state(out);
    let recordings = get_recordings()?;
    let rdir = recordings_dir()?;

    let to_process: Vec<&Recording> = recordings
        .iter()
        .filter(|r| {
            if let Some(f) = folder_filter {
                r.folder.as_deref() == Some(f)
            } else {
                true
            }
        })
        .filter(|r| force || !state.processed.contains_key(&r.uuid))
        .collect();

    let mut evicted_count = 0usize;
    let entries: Vec<DryRunEntry> = to_process
        .iter()
        .filter_map(|rec| {
            if rec.evicted {
                evicted_count += 1;
                return Some(DryRunEntry {
                    uuid: rec.uuid.clone(),
                    title: rec.title.clone(),
                    date: rec.date.format("%Y-%m-%d %H:%M").to_string(),
                    duration: format_duration(rec.duration),
                    has_tsrp: false,
                    folder: rec.folder.clone(),
                    evicted: true,
                });
            }
            let m4a = rdir.join(&rec.path);
            if !m4a.exists() {
                return None;
            }
            let data = fs::read(&m4a).ok()?;
            let has_tsrp = find_tsrp(&data).is_some();
            Some(DryRunEntry {
                uuid: rec.uuid.clone(),
                title: rec.title.clone(),
                date: rec.date.format("%Y-%m-%d %H:%M").to_string(),
                duration: format_duration(rec.duration),
                has_tsrp,
                folder: rec.folder.clone(),
                evicted: false,
            })
        })
        .collect();

    let result = DryRunResult {
        total: entries.len(),
        recordings: entries,
    };

    if json {
        print_json(&format_dry_run_json(&result), fields);
    } else {
        print!("{}", format_dry_run_human(&result));
    }
    Ok(())
}

fn cmd_extract(
    out: &PathBuf,
    all: bool,
    force: bool,
    json: bool,
    fields: &[String],
    folder_filter: Option<&str>,
) -> Result<()> {
    fs::create_dir_all(out).context("failed to create output directory")?;
    let mut state = load_state(out);
    let recordings = get_recordings()?;
    let rdir = recordings_dir()?;

    let to_process: Vec<&Recording> = recordings
        .iter()
        .filter(|r| {
            if let Some(f) = folder_filter {
                r.folder.as_deref() == Some(f)
            } else {
                true
            }
        })
        .filter(|r| force || !state.processed.contains_key(&r.uuid))
        .collect();

    if to_process.is_empty() {
        if json {
            print_json(
                &format_extract_json(&ExtractResult {
                    extracted: 0,
                    skipped: 0,
                    evicted: 0,
                    needs_whisply: 0,
                    files: vec![],
                }),
                fields,
            );
        } else {
            println!("All recordings already processed.");
        }
        return Ok(());
    }

    if !json {
        println!("Processing {} recording(s)...", to_process.len());
    }

    let mut result = ExtractResult {
        extracted: 0,
        skipped: 0,
        evicted: 0,
        needs_whisply: 0,
        files: vec![],
    };

    for rec in &to_process {
        if rec.evicted {
            if !json {
                let title_short: String = rec.title.chars().take(40).collect();
                println!("  SKIP {title_short} \u{2014} iCloud-only (evicted)");
            }
            result.evicted += 1;
            continue;
        }
        let m4a = rdir.join(&rec.path);
        if !m4a.exists() {
            if !json {
                let title_short: String = rec.title.chars().take(40).collect();
                println!("  SKIP {title_short} \u{2014} file not found");
            }
            result.skipped += 1;
            continue;
        }

        let mut transcript = extract_transcript_tsrp(&m4a);
        let mut method = "tsrp";

        if transcript.is_none() && all {
            transcript = transcribe_whisply(&m4a);
            method = "whisply";
        }

        match transcript {
            None => {
                if !all {
                    state.processed.insert(
                        rec.uuid.clone(),
                        ProcessedEntry {
                            date: rec.date.format("%Y-%m-%d %H:%M").to_string(),
                            title: rec.title.clone(),
                            method: "no-transcript".to_string(),
                            words: 0,
                            output: None,
                        },
                    );
                    result.needs_whisply += 1;
                } else {
                    if !json {
                        let title_short: String = rec.title.chars().take(40).collect();
                        println!("  SKIP {title_short} \u{2014} no transcript available");
                    }
                    state.processed.insert(
                        rec.uuid.clone(),
                        ProcessedEntry {
                            date: rec.date.format("%Y-%m-%d %H:%M").to_string(),
                            title: rec.title.clone(),
                            method: "failed".to_string(),
                            words: 0,
                            output: None,
                        },
                    );
                    result.skipped += 1;
                }
            }
            Some(ref text) => {
                let out_path = write_transcript(out, rec, text, method)?;
                let word_count = text.split_whitespace().count();
                let fname = out_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                if !json {
                    let title_short: String = rec.title.chars().take(40).collect();
                    println!(
                        "  {method} {title_short} \u{2014} {word_count} words \u{2192} {fname}"
                    );
                }
                result.files.push(ExtractedFile {
                    uuid: rec.uuid.clone(),
                    title: rec.title.clone(),
                    method: method.to_string(),
                    words: word_count,
                    file: fname.clone(),
                    folder: rec.folder.clone(),
                });
                state.processed.insert(
                    rec.uuid.clone(),
                    ProcessedEntry {
                        date: rec.date.format("%Y-%m-%d %H:%M").to_string(),
                        title: rec.title.clone(),
                        method: method.to_string(),
                        words: word_count,
                        output: Some(fname),
                    },
                );
                result.extracted += 1;
            }
        }
    }

    save_state(out, &state)?;
    if json {
        print_json(&format_extract_json(&result), fields);
    } else {
        println!("\n{}", format_extract_human(&result));
    }
    Ok(())
}

fn cmd_list(
    out: &PathBuf,
    json: bool,
    ndjson: bool,
    fields: &[String],
    folder_filter: Option<&str>,
) -> Result<()> {
    let recordings = get_recordings()?;
    let state = load_state(out);

    let entries: Vec<_> = recordings
        .iter()
        .filter(|r| {
            if let Some(f) = folder_filter {
                r.folder.as_deref() == Some(f)
            } else {
                true
            }
        })
        .map(|rec| {
            let date_str = rec.date.format("%Y-%m-%d %H:%M").to_string();
            build_list_entry(
                &rec.uuid,
                &date_str,
                rec.duration,
                &rec.title,
                state.processed.get(&rec.uuid),
                rec.folder.as_deref(),
                rec.evicted,
            )
        })
        .collect();

    if ndjson {
        print_json(&format_list_ndjson(&entries), fields);
    } else if json {
        print_json(&format_list_json(&entries), fields);
    } else {
        print!("{}", format_list_human(&entries));
    }
    Ok(())
}

fn cmd_show(out: &PathBuf, limit: usize, json: bool, ndjson: bool, fields: &[String]) -> Result<()> {
    let recordings = get_recordings()?;
    let state = load_state(out);

    let entries: Vec<ShowEntry> = recordings
        .iter()
        .filter_map(|rec| {
            let entry = state.processed.get(&rec.uuid)?;
            let fname = entry.output.as_ref()?;
            let path = out.join(fname);
            let content = fs::read_to_string(&path).ok()?;

            let transcript = if content.starts_with("---") {
                content[3..]
                    .find("---")
                    .map_or(content.clone(), |end| content[end + 6..].trim().to_string())
            } else {
                content
            };

            let word_count = transcript.split_whitespace().count();
            Some(ShowEntry {
                uuid: rec.uuid.clone(),
                date: rec.date.format("%Y-%m-%d %H:%M").to_string(),
                duration: format_duration(rec.duration),
                duration_secs: rec.duration,
                title: rec.title.clone(),
                words: word_count,
                file: fname.clone(),
                transcript,
                folder: rec.folder.clone(),
            })
        })
        .take(limit)
        .collect();

    if ndjson {
        print_json(&format_show_ndjson(&entries), fields);
    } else if json {
        print_json(&format_show_json(&entries), fields);
    } else {
        print!("{}", format_show_human(&entries));
    }
    Ok(())
}

fn cmd_watch(out: &PathBuf, action: &str) -> Result<()> {
    let home = dirs::home_dir().context("cannot determine home directory")?;
    let plist_path = home.join(format!("Library/LaunchAgents/{PLIST_LABEL}.plist"));
    let rdir = recordings_dir()?;

    match action {
        "install" => {
            let exe =
                std::env::current_exe().unwrap_or_else(|_| PathBuf::from("voice-memos"));
            let log_path = out.join("launchd.log");
            let plist = format!(
                r#"<?xml version="1.0" encoding="UTF-8"?>
<!DOCTYPE plist PUBLIC "-//Apple//DTD PLIST 1.0//EN" "http://www.apple.com/DTDs/PropertyList-1.0.dtd">
<plist version="1.0">
<dict>
    <key>Label</key>
    <string>{PLIST_LABEL}</string>
    <key>ProgramArguments</key>
    <array>
        <string>{exe}</string>
        <string>extract</string>
    </array>
    <key>WatchPaths</key>
    <array>
        <string>{rdir}</string>
    </array>
    <key>StandardOutPath</key>
    <string>{log}</string>
    <key>StandardErrorPath</key>
    <string>{log}</string>
    <key>RunAtLoad</key>
    <false/>
</dict>
</plist>"#,
                exe = exe.display(),
                rdir = rdir.display(),
                log = log_path.display(),
            );
            fs::write(&plist_path, &plist)
                .with_context(|| format!("failed to write plist: {}", plist_path.display()))?;
            Command::new("launchctl")
                .args(["load", &plist_path.to_string_lossy()])
                .status()
                .ok();
            println!("Installed and loaded {}", plist_path.display());
        }
        "uninstall" => {
            if plist_path.exists() {
                Command::new("launchctl")
                    .args(["unload", &plist_path.to_string_lossy()])
                    .status()
                    .ok();
                fs::remove_file(&plist_path).ok();
                println!("Unloaded and removed {}", plist_path.display());
            } else {
                println!("Watcher not installed.");
            }
        }
        "status" => {
            let output = Command::new("launchctl").args(["list"]).output();
            match output {
                Ok(o) => {
                    let stdout = String::from_utf8_lossy(&o.stdout);
                    if let Some(line) = stdout.lines().find(|l| l.contains(PLIST_LABEL)) {
                        println!("Running: {line}");
                    } else {
                        println!("Watcher not running.");
                    }
                }
                Err(e) => bail!("failed to check launchctl: {e}"),
            }
        }
        _ => unreachable!(),
    }
    Ok(())
}

fn cmd_schema(command: Option<&str>) -> Result<()> {
    match command {
        Some(cmd) => match schema::schema_for(cmd) {
            Some(s) => {
                println!("{}", serde_json::to_string_pretty(&s).unwrap());
            }
            None => {
                let cmds = schema::available_commands();
                bail!("unknown command: {cmd}. Available: {}", cmds.join(", "));
            }
        },
        None => {
            let cmds = schema::available_commands();
            let schemas: Vec<_> = cmds.iter().filter_map(|c| schema::schema_for(c)).collect();
            println!("{}", serde_json::to_string_pretty(&schemas).unwrap());
        }
    }
    Ok(())
}

fn resolve_output_format(explicit: Option<OutputArg>) -> OutputArg {
    use std::io::IsTerminal;

    // 1. Explicit --output flag wins
    if let Some(arg) = explicit {
        return arg;
    }

    // 2. OUTPUT_FORMAT env var
    if let Ok(val) = std::env::var("OUTPUT_FORMAT") {
        return match val.to_lowercase().as_str() {
            "json" => OutputArg::Json,
            "ndjson" => OutputArg::Ndjson,
            "human" => OutputArg::Human,
            _ => OutputArg::Human,
        };
    }

    // 3. NDJSON when stdout is not a TTY (piped)
    if !std::io::stdout().is_terminal() {
        return OutputArg::Ndjson;
    }

    OutputArg::Human
}

fn main() {
    let cli = Cli::parse();
    let out = cli.dir;
    let output = resolve_output_format(cli.output);
    let json = matches!(output, OutputArg::Json | OutputArg::Ndjson);
    let ndjson = matches!(output, OutputArg::Ndjson);

    if let Err(e) = validate::validate_output_dir(&out) {
        eprintln!("error: {e}");
        std::process::exit(1);
    }

    let fields = &cli.fields;

    let result = match cli.command {
        Commands::Extract {
            all,
            force,
            dry_run,
            folder,
        } => {
            if dry_run {
                cmd_extract_dry_run(&out, force, json, fields, folder.as_deref())
            } else {
                cmd_extract(&out, all, force, json, fields, folder.as_deref())
            }
        }
        Commands::Schema { command } => cmd_schema(command.as_deref()),
        Commands::List { folder } => cmd_list(&out, json, ndjson, fields, folder.as_deref()),
        Commands::Show { limit } => cmd_show(&out, limit, json, ndjson, fields),
        Commands::Watch { action } => cmd_watch(&out, &action),
    };

    if let Err(e) = result {
        if json {
            let err = serde_json::json!({"error": format!("{e:#}")});
            eprintln!("{}", serde_json::to_string_pretty(&err).unwrap());
        } else {
            eprintln!("error: {e:#}");
        }
        std::process::exit(1);
    }
}
