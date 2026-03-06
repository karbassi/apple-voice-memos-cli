# voice-memos CLI

Extracts transcripts from Apple Voice Memos on macOS.

## Quick Reference

```bash
# List all recordings with status
voice-memos list
voice-memos --output json list

# Show recent transcripts
voice-memos show -n 3
voice-memos --output json show --fields uuid,title,words

# Extract new transcripts
voice-memos extract --dry-run          # preview first
voice-memos extract                     # tsrp only
voice-memos extract --all              # tsrp + whisply fallback
voice-memos extract --force            # re-process everything

# JSON output (for agents)
voice-memos --output json list
voice-memos --output ndjson list       # streaming, one object per line
voice-memos --output json list --fields uuid,title,status

# Manage launchd watcher
voice-memos watch install
voice-memos watch status
voice-memos watch uninstall
```

## Important for Agents

- **Always use `--dry-run` before `extract`** to preview what will be processed
- **Always use `--output json`** for machine-readable output
- **Always use `--fields`** to limit response size and protect your context window
- **Never run `extract --force` without user confirmation** - it re-processes everything
- The `--dir` flag is validated to be under the user's home directory
- Input validation rejects path traversals, control characters, and percent-encoded strings

## Data Sources

- Voice Memos database: `~/Library/Group Containers/group.com.apple.VoiceMemos.shared/Recordings/CloudRecordings.db`
- Recordings directory: same parent as the database
- Transcripts: embedded `tsrp` atoms in m4a files, or whisply transcription as fallback
- State file: `state.json` in the output directory

## Output Formats

| Format | Flag | Use Case |
|--------|------|----------|
| human | `--output human` (default) | Interactive terminal use |
| json | `--output json` | Structured agent consumption |
| ndjson | `--output ndjson` | Streaming/incremental processing |
