use std::path::Path;

pub fn validate_output_dir(path: &Path) -> Result<(), String> {
    let home = dirs::home_dir().ok_or("cannot determine home directory")?;

    // Canonicalize if the path exists, otherwise check the string representation
    let resolved = if path.exists() {
        path.canonicalize()
            .map_err(|e| format!("cannot resolve path: {e}"))?
    } else {
        // For non-existent paths, normalize by resolving what we can
        let normalized = path.to_path_buf();
        // Check for traversal patterns in the raw path
        let path_str = path.to_string_lossy();
        if path_str.contains("..") {
            return Err(format!("path contains traversal: {path_str}"));
        }
        normalized
    };

    if !resolved.starts_with(&home) {
        return Err(format!(
            "output directory must be under home: {}",
            resolved.display()
        ));
    }

    Ok(())
}

pub fn reject_control_chars(input: &str) -> Result<(), String> {
    for (i, c) in input.chars().enumerate() {
        if c.is_control() && c != '\n' && c != '\r' && c != '\t' {
            return Err(format!(
                "control character U+{:04X} at position {i}",
                c as u32
            ));
        }
    }
    Ok(())
}

pub fn validate_resource_name(name: &str) -> Result<(), String> {
    if name.is_empty() {
        return Err("resource name cannot be empty".to_string());
    }
    if name.contains('?') {
        return Err("resource name contains '?' (possible embedded query params)".to_string());
    }
    if name.contains('#') {
        return Err("resource name contains '#' (possible fragment)".to_string());
    }
    if name.contains('%') {
        return Err("resource name contains '%' (possible pre-encoded string)".to_string());
    }
    if name.contains("..") {
        return Err("resource name contains '..' (possible path traversal)".to_string());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    // --- validate_output_dir ---

    #[test]
    fn valid_output_dir() {
        let home = dirs::home_dir().unwrap();
        let dir = TempDir::new_in(&home).unwrap();
        assert!(validate_output_dir(dir.path()).is_ok());
    }

    #[test]
    fn rejects_traversal_in_dir() {
        let path = Path::new("/tmp/../../../etc/passwd");
        assert!(validate_output_dir(path).is_err());
    }

    #[test]
    fn rejects_dir_outside_home() {
        let path = Path::new("/etc/shadow");
        assert!(validate_output_dir(path).is_err());
    }

    #[test]
    fn allows_dir_under_home() {
        let home = dirs::home_dir().unwrap();
        let path = home.join("Projects/test-output");
        assert!(validate_output_dir(&path).is_ok());
    }

    // --- reject_control_chars ---

    #[test]
    fn allows_normal_text() {
        assert!(reject_control_chars("Hello World 123").is_ok());
    }

    #[test]
    fn allows_newlines_and_tabs() {
        assert!(reject_control_chars("line1\nline2\ttab").is_ok());
    }

    #[test]
    fn rejects_null_byte() {
        assert!(reject_control_chars("hello\x00world").is_err());
    }

    #[test]
    fn rejects_bell_char() {
        assert!(reject_control_chars("hello\x07world").is_err());
    }

    #[test]
    fn rejects_escape_char() {
        assert!(reject_control_chars("hello\x1bworld").is_err());
    }

    // --- validate_resource_name ---

    #[test]
    fn valid_uuid() {
        assert!(validate_resource_name("ABC123-DEF456").is_ok());
    }

    #[test]
    fn rejects_question_mark() {
        assert!(validate_resource_name("fileId?fields=name").is_err());
    }

    #[test]
    fn rejects_hash() {
        assert!(validate_resource_name("fileId#section").is_err());
    }

    #[test]
    fn rejects_percent_encoding() {
        assert!(validate_resource_name("file%2e%2e").is_err());
    }

    #[test]
    fn rejects_path_traversal() {
        assert!(validate_resource_name("../../etc/passwd").is_err());
    }

    #[test]
    fn rejects_empty() {
        assert!(validate_resource_name("").is_err());
    }
}
