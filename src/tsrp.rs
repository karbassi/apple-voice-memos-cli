pub fn find_tsrp(data: &[u8]) -> Option<&[u8]> {
    let marker = b"tsrp";
    let idx = data.windows(4).position(|w| w == marker)?;
    if idx < 4 {
        return None;
    }
    let atom_start = idx - 4;
    let size = u32::from_be_bytes(data[atom_start..atom_start + 4].try_into().ok()?) as usize;
    if size < 8 || atom_start + size > data.len() {
        return None;
    }
    Some(&data[idx + 4..atom_start + size])
}

pub fn parse_tsrp(payload: &[u8]) -> Option<String> {
    let val: serde_json::Value = serde_json::from_slice(payload).ok()?;
    let obj = val.as_object()?;
    let astr = obj.get("attributedString")?;

    let runs = match astr {
        serde_json::Value::Object(map) => map.get("runs")?.as_array()?,
        serde_json::Value::Array(arr) => arr,
        _ => return None,
    };

    let text: String = runs.iter().filter_map(|r| r.as_str()).collect();

    let trimmed = text.trim().to_string();
    if trimmed.is_empty() {
        None
    } else {
        Some(trimmed)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_tsrp_atom(json: &[u8]) -> Vec<u8> {
        let atom_size = (8 + json.len()) as u32;
        let mut buf = Vec::new();
        buf.extend_from_slice(&atom_size.to_be_bytes());
        buf.extend_from_slice(b"tsrp");
        buf.extend_from_slice(json);
        buf
    }

    // --- find_tsrp ---

    #[test]
    fn find_tsrp_extracts_payload() {
        let json = br#"{"attributedString":{"runs":["hello"]}}"#;
        let atom = make_tsrp_atom(json);
        let payload = find_tsrp(&atom).unwrap();
        assert_eq!(payload, json);
    }

    #[test]
    fn find_tsrp_with_prefix_data() {
        let json = br#"{"attributedString":{"runs":["hello"]}}"#;
        let mut data = vec![0u8; 64]; // junk before
        let atom = make_tsrp_atom(json);
        data.extend_from_slice(&atom);
        data.extend_from_slice(&[0u8; 32]); // junk after
        let payload = find_tsrp(&data).unwrap();
        assert_eq!(payload, json);
    }

    #[test]
    fn find_tsrp_returns_none_when_missing() {
        let data = vec![0u8; 128];
        assert!(find_tsrp(&data).is_none());
    }

    #[test]
    fn find_tsrp_returns_none_when_too_short() {
        // marker at position 0 means atom_start would be negative
        let data = b"tsrpsome data";
        assert!(find_tsrp(data).is_none());
    }

    #[test]
    fn find_tsrp_returns_none_on_bad_size() {
        // size claims 1000 bytes but data is short
        let mut buf = Vec::new();
        buf.extend_from_slice(&1000u32.to_be_bytes());
        buf.extend_from_slice(b"tsrp");
        buf.extend_from_slice(b"tiny");
        assert!(find_tsrp(&buf).is_none());
    }

    // --- parse_tsrp ---

    #[test]
    fn parse_tsrp_object_format() {
        let json = br#"{"attributedString":{"runs":["Hello ","world"]}}"#;
        assert_eq!(parse_tsrp(json).unwrap(), "Hello world");
    }

    #[test]
    fn parse_tsrp_array_format() {
        let json = br#"{"attributedString":["segment one","segment two"]}"#;
        assert_eq!(parse_tsrp(json).unwrap(), "segment onesegment two");
    }

    #[test]
    fn parse_tsrp_empty_runs() {
        let json = br#"{"attributedString":{"runs":[]}}"#;
        assert!(parse_tsrp(json).is_none());
    }

    #[test]
    fn parse_tsrp_whitespace_only() {
        let json = br#"{"attributedString":{"runs":["  \n  "]}}"#;
        assert!(parse_tsrp(json).is_none());
    }

    #[test]
    fn parse_tsrp_invalid_json() {
        assert!(parse_tsrp(b"not json").is_none());
    }

    #[test]
    fn parse_tsrp_missing_attributed_string() {
        let json = br#"{"other":"field"}"#;
        assert!(parse_tsrp(json).is_none());
    }
}
