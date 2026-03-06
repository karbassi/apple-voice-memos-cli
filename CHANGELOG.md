# Changelog

All notable changes to this project will be documented in this file.

The format is based on [Keep a Changelog](https://keepachangelog.com/en/1.1.0/),
and this project adheres to [Semantic Versioning](https://semver.org/spec/v2.0.0.html).

## [Unreleased]

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
