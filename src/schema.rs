use serde::Serialize;

#[derive(Serialize)]
pub struct FieldDef {
    pub name: &'static str,
    pub r#type: &'static str,
    pub nullable: bool,
    pub description: &'static str,
}

#[derive(Serialize)]
pub struct CommandSchema {
    pub command: &'static str,
    pub description: &'static str,
    pub output_fields: Vec<FieldDef>,
}

pub fn schema_for(command: &str) -> Option<CommandSchema> {
    match command {
        "list" => Some(CommandSchema {
            command: "list",
            description: "List all recordings and their processing status",
            output_fields: vec![
                FieldDef { name: "uuid", r#type: "string", nullable: false, description: "Unique recording identifier" },
                FieldDef { name: "date", r#type: "string", nullable: false, description: "Recording date (YYYY-MM-DD HH:MM)" },
                FieldDef { name: "duration", r#type: "string", nullable: false, description: "Human-readable duration (e.g. 2m05s)" },
                FieldDef { name: "duration_secs", r#type: "number", nullable: false, description: "Duration in seconds" },
                FieldDef { name: "title", r#type: "string", nullable: false, description: "Recording title or location" },
                FieldDef { name: "status", r#type: "string", nullable: false, description: "Processing status: pending, done, needs-whisply" },
                FieldDef { name: "method", r#type: "string", nullable: true, description: "Transcription method used: tsrp or whisply" },
                FieldDef { name: "words", r#type: "integer", nullable: true, description: "Word count of transcript" },
                FieldDef { name: "file", r#type: "string", nullable: true, description: "Output filename" },
            ],
        }),
        "show" => Some(CommandSchema {
            command: "show",
            description: "Show recent transcripts with full text",
            output_fields: vec![
                FieldDef { name: "uuid", r#type: "string", nullable: false, description: "Unique recording identifier" },
                FieldDef { name: "date", r#type: "string", nullable: false, description: "Recording date (YYYY-MM-DD HH:MM)" },
                FieldDef { name: "duration", r#type: "string", nullable: false, description: "Human-readable duration" },
                FieldDef { name: "duration_secs", r#type: "number", nullable: false, description: "Duration in seconds" },
                FieldDef { name: "title", r#type: "string", nullable: false, description: "Recording title" },
                FieldDef { name: "words", r#type: "integer", nullable: false, description: "Word count" },
                FieldDef { name: "file", r#type: "string", nullable: false, description: "Output filename" },
                FieldDef { name: "transcript", r#type: "string", nullable: false, description: "Full transcript text" },
            ],
        }),
        "extract" => Some(CommandSchema {
            command: "extract",
            description: "Extract new transcripts from Voice Memos",
            output_fields: vec![
                FieldDef { name: "extracted", r#type: "integer", nullable: false, description: "Number of recordings successfully transcribed" },
                FieldDef { name: "skipped", r#type: "integer", nullable: false, description: "Number of recordings skipped" },
                FieldDef { name: "needs_whisply", r#type: "integer", nullable: false, description: "Number needing --all for whisply fallback" },
                FieldDef { name: "files", r#type: "array", nullable: false, description: "List of extracted files with uuid, title, method, words, file" },
            ],
        }),
        _ => None,
    }
}

pub fn available_commands() -> Vec<&'static str> {
    vec!["list", "show", "extract"]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn schema_list_has_expected_fields() {
        let s = schema_for("list").unwrap();
        assert_eq!(s.command, "list");
        let names: Vec<&str> = s.output_fields.iter().map(|f| f.name).collect();
        assert!(names.contains(&"uuid"));
        assert!(names.contains(&"title"));
        assert!(names.contains(&"status"));
        assert!(names.contains(&"words"));
        assert!(names.contains(&"duration_secs"));
    }

    #[test]
    fn schema_show_has_transcript() {
        let s = schema_for("show").unwrap();
        let names: Vec<&str> = s.output_fields.iter().map(|f| f.name).collect();
        assert!(names.contains(&"transcript"));
        assert!(names.contains(&"words"));
    }

    #[test]
    fn schema_extract_has_files() {
        let s = schema_for("extract").unwrap();
        let names: Vec<&str> = s.output_fields.iter().map(|f| f.name).collect();
        assert!(names.contains(&"extracted"));
        assert!(names.contains(&"files"));
    }

    #[test]
    fn schema_unknown_returns_none() {
        assert!(schema_for("nonexistent").is_none());
    }

    #[test]
    fn available_commands_includes_all() {
        let cmds = available_commands();
        assert!(cmds.contains(&"list"));
        assert!(cmds.contains(&"show"));
        assert!(cmds.contains(&"extract"));
    }

    #[test]
    fn schema_serializes_to_valid_json() {
        let s = schema_for("list").unwrap();
        let json = serde_json::to_string_pretty(&s).unwrap();
        let parsed: serde_json::Value = serde_json::from_str(&json).unwrap();
        assert_eq!(parsed["command"], "list");
        assert!(parsed["output_fields"].is_array());
        let first = &parsed["output_fields"][0];
        assert!(first.get("name").is_some());
        assert!(first.get("type").is_some());
        assert!(first.get("nullable").is_some());
        assert!(first.get("description").is_some());
    }

    #[test]
    fn schema_list_nullable_fields_marked_correctly() {
        let s = schema_for("list").unwrap();
        let method = s.output_fields.iter().find(|f| f.name == "method").unwrap();
        assert!(method.nullable);
        let uuid = s.output_fields.iter().find(|f| f.name == "uuid").unwrap();
        assert!(!uuid.nullable);
    }
}
