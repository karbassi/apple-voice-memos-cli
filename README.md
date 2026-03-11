# apple-voice-memos-cli

> Extract transcripts from Apple Voice Memos.

A Rust CLI that reads the Voice Memos SQLite database on macOS, extracts embedded `tsrp` transcripts from m4a files, and writes them as Markdown with frontmatter. Supports whisply as a fallback transcription engine.

## Install

```sh
cargo install --path .
```

## Usage

```sh
# List all recordings
apple-voice-memos-cli list

# Show recent transcripts
apple-voice-memos-cli show -n 3

# Extract new transcripts (preview first)
apple-voice-memos-cli extract --dry-run
apple-voice-memos-cli extract

# Use whisply for recordings without embedded transcripts
apple-voice-memos-cli extract --all

# JSON output for scripts and agents
apple-voice-memos-cli --output json list
apple-voice-memos-cli --output json --fields title,status,words list

# Inspect output schema
apple-voice-memos-cli schema list

# Auto-watch for new recordings
apple-voice-memos-cli watch install
```

Output format is auto-detected: NDJSON when piped, human-readable in a terminal. Override with `--output` or the `OUTPUT_FORMAT` env var.

## License

MIT © [Ali Karbassi](https://github.com/karbassi)
