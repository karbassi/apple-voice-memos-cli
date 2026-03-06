# voice-memos

> Extract transcripts from Apple Voice Memos.

A Rust CLI that reads the Voice Memos SQLite database on macOS, extracts embedded `tsrp` transcripts from m4a files, and writes them as Markdown with frontmatter. Supports whisply as a fallback transcription engine.

## Install

```sh
cargo install --path .
```

## Usage

```sh
# List all recordings
voice-memos list

# Show recent transcripts
voice-memos show -n 3

# Extract new transcripts (preview first)
voice-memos extract --dry-run
voice-memos extract

# Use whisply for recordings without embedded transcripts
voice-memos extract --all

# JSON output for scripts and agents
voice-memos --output json list
voice-memos --output json --fields title,status,words list

# Inspect output schema
voice-memos schema list

# Auto-watch for new recordings
voice-memos watch install
```

Output format is auto-detected: NDJSON when piped, human-readable in a terminal. Override with `--output` or the `OUTPUT_FORMAT` env var.

## License

MIT © [Ali Karbassi](https://github.com/karbassi)
