pub fn slugify(title: &str) -> String {
    let s: String = title
        .to_lowercase()
        .chars()
        .map(|c| {
            if c.is_alphanumeric() || c == ' ' || c == '-' || c == '_' {
                c
            } else {
                ' '
            }
        })
        .collect();
    let slug: String = s.split_whitespace().collect::<Vec<_>>().join("-");
    slug.chars().take(60).collect()
}

pub fn format_duration(seconds: f64) -> String {
    let total = seconds as u64;
    let s = total % 60;
    let m = (total / 60) % 60;
    let h = total / 3600;
    if h > 0 {
        format!("{h}h{m:02}m{s:02}s")
    } else {
        format!("{m}m{s:02}s")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    // --- slugify ---

    #[test]
    fn slugify_simple_title() {
        assert_eq!(slugify("My Voice Memo"), "my-voice-memo");
    }

    #[test]
    fn slugify_with_special_chars() {
        assert_eq!(slugify("Hello, World! (2024)"), "hello-world-2024");
    }

    #[test]
    fn slugify_preserves_hyphens_and_underscores() {
        assert_eq!(slugify("my-memo_draft"), "my-memo_draft");
    }

    #[test]
    fn slugify_collapses_whitespace() {
        assert_eq!(slugify("  lots   of   spaces  "), "lots-of-spaces");
    }

    #[test]
    fn slugify_truncates_at_60_chars() {
        let long_title = "a".repeat(100);
        assert_eq!(slugify(&long_title).len(), 60);
    }

    #[test]
    fn slugify_empty_string() {
        assert_eq!(slugify(""), "");
    }

    #[test]
    fn slugify_only_special_chars() {
        assert_eq!(slugify("!!!@@@###"), "");
    }

    // --- format_duration ---

    #[test]
    fn format_duration_seconds_only() {
        assert_eq!(format_duration(45.0), "0m45s");
    }

    #[test]
    fn format_duration_minutes_and_seconds() {
        assert_eq!(format_duration(125.0), "2m05s");
    }

    #[test]
    fn format_duration_with_hours() {
        assert_eq!(format_duration(3661.0), "1h01m01s");
    }

    #[test]
    fn format_duration_zero() {
        assert_eq!(format_duration(0.0), "0m00s");
    }

    #[test]
    fn format_duration_fractional_seconds() {
        assert_eq!(format_duration(59.9), "0m59s");
    }
}
