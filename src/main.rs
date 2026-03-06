pub mod format;
pub mod state;
pub mod tsrp;
pub mod types;

use chrono::{Local, TimeZone, Utc};
use clap::{Parser, Subcommand};
use format::{format_duration, slugify};
use rusqlite::Connection;
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

    #[command(subcommand)]
    command: Commands,
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
    },
    /// List all recordings and their status
    List,
    /// Show recent transcripts
    Show {
        /// Number of transcripts to show
        #[arg(short = 'n', long, default_value_t = 5)]
        limit: usize,
    },
    /// Manage launchd watcher
    Watch {
        #[arg(value_parser = ["install", "uninstall", "status"])]
        action: String,
    },
}

fn recordings_dir() -> PathBuf {
    dirs::home_dir().expect("no home directory").join(
        "Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings",
    )
}

fn db_path() -> PathBuf {
    dirs::home_dir().expect("no home directory").join(DB_REL)
}

fn get_recordings() -> Vec<Recording> {
    let src = db_path();
    let tmp = PathBuf::from("/tmp/vm_extract.db");
    fs::copy(&src, &tmp).expect("failed to copy database");
    let wal = src.with_extension("db-wal");
    if wal.exists() {
        let _ = fs::copy(&wal, tmp.with_extension("db-wal"));
    }
    let shm = src.with_extension("db-shm");
    if shm.exists() {
        let _ = fs::copy(&shm, tmp.with_extension("db-shm"));
    }

    let conn = Connection::open(&tmp).expect("failed to open database");
    let mut stmt = conn
        .prepare(
            "SELECT ZUNIQUEID, ZENCRYPTEDTITLE, ZPATH, ZDURATION, ZDATE, ZCUSTOMLABEL \
             FROM ZCLOUDRECORDING ORDER BY ZDATE DESC",
        )
        .expect("failed to prepare query");

    let rows = stmt
        .query_map([], |row| {
            let uuid: String = row.get(0)?;
            let title: Option<String> = row.get(1)?;
            let path: String = row.get(2)?;
            let duration: f64 = row.get::<_, Option<f64>>(3)?.unwrap_or(0.0);
            let zdate: f64 = row.get::<_, Option<f64>>(4)?.unwrap_or(0.0);
            let custom_label: Option<String> = row.get(5)?;

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
            })
        })
        .expect("failed to query recordings");

    rows.filter_map(|r| r.ok()).collect()
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

fn write_transcript(out: &PathBuf, rec: &Recording, transcript: &str, method: &str) -> PathBuf {
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
    fs::write(&out_path, content).expect("failed to write transcript");
    out_path
}

fn cmd_extract(out: &PathBuf, all: bool, force: bool) {
    fs::create_dir_all(out).expect("failed to create output directory");
    let mut state = load_state(out);
    let recordings = get_recordings();
    let rdir = recordings_dir();

    let to_process: Vec<&Recording> = if force {
        recordings.iter().collect()
    } else {
        recordings
            .iter()
            .filter(|r| !state.processed.contains_key(&r.uuid))
            .collect()
    };

    if to_process.is_empty() {
        println!("All recordings already processed.");
        return;
    }

    println!("Processing {} recording(s)...", to_process.len());

    let mut new_count = 0usize;
    let mut skip_count = 0usize;
    let mut whisply_needed = 0usize;

    for rec in &to_process {
        let m4a = rdir.join(&rec.path);
        if !m4a.exists() {
            let title_short: String = rec.title.chars().take(40).collect();
            println!("  SKIP {title_short} — file not found");
            skip_count += 1;
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
                    whisply_needed += 1;
                } else {
                    let title_short: String = rec.title.chars().take(40).collect();
                    println!("  SKIP {title_short} — no transcript available");
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
                    skip_count += 1;
                }
            }
            Some(ref text) => {
                let out_path = write_transcript(out, rec, text, method);
                let word_count = text.split_whitespace().count();
                let fname = out_path
                    .file_name()
                    .unwrap()
                    .to_string_lossy()
                    .to_string();
                let title_short: String = rec.title.chars().take(40).collect();
                println!("  {method} {title_short} — {word_count} words → {fname}");
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
                new_count += 1;
            }
        }
    }

    save_state(out, &state);
    println!(
        "\nDone: {new_count} extracted, {skip_count} skipped, {whisply_needed} need --all for whisply"
    );
}

fn cmd_list(out: &PathBuf) {
    let recordings = get_recordings();
    let state = load_state(out);

    println!(
        "{:<20} {:>8}   {:<16} {:>5}   Title",
        "Date", "Duration", "Status", "Words"
    );
    println!("{}", "─".repeat(80));

    for rec in &recordings {
        let date_str = rec.date.format("%Y-%m-%d %H:%M").to_string();
        let dur = format_duration(rec.duration);
        let title: String = rec.title.chars().take(35).collect();

        let (status, words) = match state.processed.get(&rec.uuid) {
            Some(e) if e.method == "tsrp" || e.method == "whisply" => {
                (format!("✓ {}", e.method), e.words.to_string())
            }
            Some(e) if e.method == "no-transcript" => {
                ("○ needs --all".to_string(), "—".to_string())
            }
            _ => ("○ pending".to_string(), "—".to_string()),
        };

        println!("{date_str:<20} {dur:>8}   {status:<16} {words:>5}   {title}");
    }
}

fn cmd_show(out: &PathBuf, limit: usize) {
    let recordings = get_recordings();
    let state = load_state(out);
    let mut shown = 0usize;

    for rec in &recordings {
        let Some(entry) = state.processed.get(&rec.uuid) else {
            continue;
        };
        let Some(ref fname) = entry.output else {
            continue;
        };
        let path = out.join(fname);
        let Ok(content) = fs::read_to_string(&path) else {
            continue;
        };

        let transcript = if content.starts_with("---") {
            content[3..]
                .find("---")
                .map_or(content.as_str(), |end| content[end + 6..].trim())
        } else {
            content.as_str()
        };

        let date_str = rec.date.format("%Y-%m-%d %H:%M").to_string();
        let dur = format_duration(rec.duration);
        let word_count = transcript.split_whitespace().count();

        println!("\n{}", "=".repeat(70));
        println!(
            "{}  ({date_str}, {dur}, {word_count} words)",
            rec.title
        );
        println!("{}", "=".repeat(70));

        if transcript.len() > 3000 {
            println!("{}", &transcript[..3000]);
            println!(
                "\n... [{} chars truncated, see {fname}]",
                transcript.len() - 3000
            );
        } else {
            println!("{transcript}");
        }

        shown += 1;
        if shown >= limit {
            break;
        }
    }

    if shown == 0 {
        println!("No transcripts available. Run `voice-memos extract` first.");
    }
}

fn cmd_watch(out: &PathBuf, action: &str) {
    let home = dirs::home_dir().expect("no home directory");
    let plist_path = home.join(format!("Library/LaunchAgents/{PLIST_LABEL}.plist"));
    let rdir = recordings_dir();

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
            fs::write(&plist_path, plist).expect("failed to write plist");
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
                Err(_) => println!("Failed to check launchctl."),
            }
        }
        _ => unreachable!(),
    }
}

fn main() {
    let cli = Cli::parse();
    let out = cli.dir;

    match cli.command {
        Commands::Extract { all, force } => cmd_extract(&out, all, force),
        Commands::List => cmd_list(&out),
        Commands::Show { limit } => cmd_show(&out, limit),
        Commands::Watch { action } => cmd_watch(&out, &action),
    }
}
