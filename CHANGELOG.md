# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

## [0.2.1] - 2026-03-10

### Fixed

- Handle Voice Memos databases that use `ZENCRYPTEDNAME` instead of `ZNAME` in the `ZFOLDER` table
- Gracefully fall back when `ZFOLDER` table is missing entirely

## [0.2.0] - 2026-03-10

### Added

- Folder support: recordings now include their Voice Memos folder name in all outputs
- `--folder <name>` filter for `list` and `extract` commands
- iCloud eviction awareness: evicted files are distinguished from missing files
- Human format shows `[FolderName]` after title and `iCloud-only` status for evicted recordings
- `folder` and `evicted` fields in JSON/NDJSON output and schema definitions
- Homebrew tap support via `brew install karbassi/tap/apple-voice-memos-cli`

## [0.1.0] - 2026-03-06

### Added

- Initial release
- Extract transcripts from Apple Voice Memos via embedded tsrp atoms
- Whisply fallback transcription with `--all`
- `list`, `show`, `extract`, `watch`, `schema` commands
- `--output json`, `--output ndjson`, `OUTPUT_FORMAT` env var
- NDJSON auto-detection when stdout is piped
- `--fields` for JSON output filtering
- `--dry-run` for safe extract previews
- `schema` subcommand for runtime introspection
- Input validation and path hardening
- launchd watcher for automatic extraction
