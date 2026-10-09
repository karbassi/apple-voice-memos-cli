const QTA_TSRP_KEY: &[u8] = b"com.apple.VoiceMemos.tsrp";

/// Locate the Apple Intelligence transcript JSON in either container:
/// a `tsrp` atom (.m4a) or a QuickTime metadata item (.qta).
pub fn find_transcript(data: &[u8]) -> Option<&[u8]> {
    find_tsrp(data).or_else(|| find_qta_tsrp(data))
}

fn be32(data: &[u8], at: usize) -> Option<usize> {
    Some(u32::from_be_bytes(data.get(at..at.checked_add(4)?)?.try_into().ok()?) as usize)
}

/// Every offset where `marker` appears.
fn positions<'a>(data: &'a [u8], marker: &'a [u8]) -> impl Iterator<Item = usize> + 'a {
    data.windows(marker.len())
        .enumerate()
        .filter(move |(_, w)| *w == marker)
        .map(|(i, _)| i)
}

/// Box (atom) whose type tag sits at `tag_at`: returns (start, end) if its
/// size field is sane. Guards against matching the tag inside other bytes.
fn box_at(data: &[u8], tag_at: usize, min_size: usize) -> Option<(usize, usize)> {
    let start = tag_at.checked_sub(4)?;
    let size = be32(data, start)?;
    let end = start.checked_add(size)?;
    (size >= min_size && end <= data.len()).then_some((start, end))
}

pub fn find_tsrp(data: &[u8]) -> Option<&[u8]> {
    positions(data, b"tsrp").find_map(|idx| {
        let (_, end) = box_at(data, idx, 8)?;
        Some(&data[idx + 4..end])
    })
}

/// .qta (QuickTime Audio, iOS 26+ Voice Memos): the transcript is an `mdta`
/// metadata item keyed `com.apple.VoiceMemos.tsrp`. Find the key's 1-based
/// index in `keys`, then the `ilst` item with that index, then its `data`
/// payload (after the 8-byte type + locale header).
pub fn find_qta_tsrp(data: &[u8]) -> Option<&[u8]> {
    positions(data, b"keys").find_map(|idx| {
        let (start, end) = box_at(data, idx, 16)?;
        let key_index = qta_key_index(data, start, end)?;
        let ilst_at = positions(&data[end..], b"ilst").next()? + end;
        qta_ilst_payload(data, ilst_at, key_index)
    })
}

fn qta_key_index(data: &[u8], start: usize, end: usize) -> Option<usize> {
    let count = be32(data, start + 12)?;
    let mut pos = start + 16;
    for i in 1..=count {
        let size = be32(data, pos)?;
        if size < 8 || pos + size > end {
            return None;
        }
        if &data[pos + 4..pos + 8] == b"mdta" && &data[pos + 8..pos + size] == QTA_TSRP_KEY {
            return Some(i);
        }
        pos += size;
    }
    None
}

fn qta_ilst_payload(data: &[u8], ilst_at: usize, key_index: usize) -> Option<&[u8]> {
    let (start, end) = box_at(data, ilst_at, 8)?;
    let mut pos = start + 8;
    while pos + 8 <= end {
        let size = be32(data, pos)?;
        if size < 8 || pos + size > end {
            return None;
        }
        if be32(data, pos + 4)? == key_index {
            let (dstart, dend) = box_at(data, pos + 12, 16)?;
            if &data[dstart + 4..dstart + 8] != b"data" || dend > pos + size {
                return None;
            }
            return Some(&data[dstart + 16..dend]);
        }
        pos += size;
    }
    None
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

    // --- find_qta_tsrp ---

    fn make_box(tag: &[u8], body: &[u8]) -> Vec<u8> {
        let mut b = ((8 + body.len()) as u32).to_be_bytes().to_vec();
        b.extend_from_slice(tag);
        b.extend_from_slice(body);
        b
    }

    fn make_qta_meta(keys: &[&[u8]], tsrp_index: u32, json: &[u8]) -> Vec<u8> {
        make_qta_meta_ns(b"mdta", keys, tsrp_index, json)
    }

    fn make_qta_meta_ns(ns: &[u8], keys: &[&[u8]], tsrp_index: u32, json: &[u8]) -> Vec<u8> {
        let mut kbody = vec![0u8; 4];
        kbody.extend_from_slice(&(keys.len() as u32).to_be_bytes());
        for k in keys {
            kbody.extend_from_slice(&make_box(ns, k));
        }
        let mut dbody = vec![0, 0, 0, 1, 0, 0, 0, 0]; // type (UTF-8), locale
        dbody.extend_from_slice(json);
        let item = make_box(&tsrp_index.to_be_bytes(), &make_box(b"data", &dbody));
        let other = make_box(
            &(tsrp_index + 1).to_be_bytes(),
            &make_box(b"data", &[0u8; 9]),
        );
        let mut ilst = other;
        ilst.extend_from_slice(&item);
        let mut meta = make_box(b"keys", &kbody);
        meta.extend_from_slice(&make_box(b"ilst", &ilst));
        meta
    }

    #[test]
    fn find_qta_tsrp_extracts_payload_by_key_index() {
        let json = br#"{"attributedString":{"runs":["hi"]}}"#;
        let meta = make_qta_meta(&[b"com.apple.other", QTA_TSRP_KEY], 2, json);
        let mut data = vec![0u8; 40];
        data.extend_from_slice(&meta);
        assert_eq!(find_qta_tsrp(&data).unwrap(), json);
        assert_eq!(find_transcript(&data).unwrap(), json);
    }

    #[test]
    fn find_qta_tsrp_ignores_tsrp_suffix_in_key_string() {
        // The bare b"tsrp" match is the tail of the key name; find_tsrp must
        // reject it (size field reads as garbage) rather than return junk.
        let json = br#"{"attributedString":{"runs":["hi"]}}"#;
        let meta = make_qta_meta(&[QTA_TSRP_KEY], 1, json);
        assert!(find_tsrp(&meta).is_none());
        assert_eq!(find_transcript(&meta).unwrap(), json);
    }

    #[test]
    fn find_qta_tsrp_requires_mdta_namespace() {
        let meta = make_qta_meta_ns(b"udta", &[QTA_TSRP_KEY], 1, br#"{"x":1}"#);
        assert!(find_qta_tsrp(&meta).is_none());
    }

    #[test]
    fn find_qta_tsrp_none_without_key() {
        let meta = make_qta_meta(&[b"com.apple.other"], 1, b"{}");
        assert!(find_qta_tsrp(&meta).is_none());
    }

    #[test]
    fn find_tsrp_skips_false_match_then_finds_real_atom() {
        let json = br#"{"attributedString":{"runs":["x"]}}"#;
        let mut data = b"\xff\xff\xff\xfftsrp".to_vec();
        data.extend_from_slice(&make_tsrp_atom(json));
        assert_eq!(find_tsrp(&data).unwrap(), json);
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
