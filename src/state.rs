use crate::types::State;
#[cfg(test)]
use crate::types::ProcessedEntry;
use std::fs;
use std::path::Path;

pub fn load_state(out: &Path) -> State {
    let path = out.join("state.json");
    if path.exists() {
        let data = fs::read_to_string(&path).unwrap_or_default();
        serde_json::from_str(&data).unwrap_or_default()
    } else {
        State::default()
    }
}

pub fn save_state(out: &Path, state: &State) {
    let path = out.join("state.json");
    let data = serde_json::to_string_pretty(state).expect("failed to serialize state");
    fs::write(&path, format!("{data}\n")).expect("failed to write state");
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn load_state_returns_default_when_missing() {
        let dir = TempDir::new().unwrap();
        let state = load_state(dir.path());
        assert!(state.processed.is_empty());
    }

    #[test]
    fn save_and_load_roundtrip() {
        let dir = TempDir::new().unwrap();
        let mut state = State::default();
        state.processed.insert(
            "abc-123".to_string(),
            ProcessedEntry {
                date: "2024-01-15 10:30".to_string(),
                title: "Test Memo".to_string(),
                method: "tsrp".to_string(),
                words: 42,
                output: Some("2024-01-15-test-memo.md".to_string()),
            },
        );
        save_state(dir.path(), &state);
        let loaded = load_state(dir.path());
        assert_eq!(loaded.processed.len(), 1);
        assert_eq!(loaded.processed["abc-123"].words, 42);
    }

    #[test]
    fn load_state_handles_corrupt_json() {
        let dir = TempDir::new().unwrap();
        fs::write(dir.path().join("state.json"), "not json{{{").unwrap();
        let state = load_state(dir.path());
        assert!(state.processed.is_empty());
    }

    #[test]
    fn save_state_creates_file_with_trailing_newline() {
        let dir = TempDir::new().unwrap();
        save_state(dir.path(), &State::default());
        let content = fs::read_to_string(dir.path().join("state.json")).unwrap();
        assert!(content.ends_with('\n'));
    }
}
