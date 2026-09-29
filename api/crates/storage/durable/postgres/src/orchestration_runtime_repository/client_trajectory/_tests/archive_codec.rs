use super::*;
use serde_json::json;

fn originals() -> Vec<ClientTrajectoryArchiveFrame> {
    vec![
        ClientTrajectoryArchiveFrame {
            sequence: 1,
            kind: ClientTrajectoryFrameKind::Request,
            observed_at: " arbitrary timestamp \0 北京 🌍 \n".into(),
            bytes: [vec![0, 255, 128], vec![b'x'; 192 * 1024]].concat(),
        },
        ClientTrajectoryArchiveFrame {
            sequence: 4,
            kind: ClientTrajectoryFrameKind::ResponseJson,
            observed_at: String::new(),
            bytes: b"literal\0NUL \xff".to_vec(),
        },
        ClientTrajectoryArchiveFrame {
            sequence: i64::MAX,
            kind: ClientTrajectoryFrameKind::ResponseSse,
            observed_at: "0000 not-normalized +08:00".into(),
            bytes: Vec::new(),
        },
    ]
}

fn read(encoded: &EncodedPart) -> Result<Vec<DecodedFrame>> {
    decode(Part {
        version: VERSION,
        frames: &Value::Null,
        directory: Some(&encoded.directory),
        bytes: &encoded.bytes,
        raw_byte_length: Some(encoded.raw_byte_length),
        checksum: Some(&encoded.checksum),
        first_sequence: 1,
        last_sequence: i64::MAX,
    })
}

fn assert_error(encoded: &EncodedPart, expected: &str) {
    let error = match read(encoded) {
        Err(error) => error,
        Ok(_) => panic!("corruption was accepted: {expected}"),
    };
    assert!(format!("{error:#}").contains(expected), "{error:#}");
}

#[test]
fn exact_binary_frames_arbitrary_timestamp_strings_empty_frame_and_sequence_gaps() {
    let expected = originals();
    let encoded = encode(&expected).unwrap();
    assert!(encoded.bytes.len() < expected[0].bytes.len());
    let actual = read(&encoded).unwrap();
    assert_eq!(actual.len(), expected.len());
    for (actual, expected) in actual.into_iter().zip(expected) {
        assert_eq!(actual.frame.sequence, expected.sequence);
        assert_eq!(actual.frame.kind, expected.kind);
        assert_eq!(actual.frame.observed_at, expected.observed_at);
        assert_eq!(actual.frame.bytes, expected.bytes);
        assert!(actual.format == Format::Wire);
    }
}

#[test]
fn rejects_checksum_length_raw_length_unknown_version_and_compressed_truncation() {
    let mut encoded = encode(&originals()).unwrap();
    encoded.checksum[0] ^= 1;
    assert_error(&encoded, "checksum mismatch");
    let mut encoded = encode(&originals()).unwrap();
    encoded.checksum.pop();
    assert_error(&encoded, "checksum length invalid");
    for difference in [-1, 1] {
        let mut encoded = encode(&originals()).unwrap();
        encoded.raw_byte_length += difference;
        assert_error(&encoded, "raw length mismatch");
    }
    let mut encoded = encode(&originals()).unwrap();
    encoded.raw_byte_length = -1;
    assert_error(&encoded, "raw length invalid");
    let mut encoded = encode(&originals()).unwrap();
    encoded.bytes[0] = 0;
    assert_error(&encoded, "compression decode failed");
    let encoded = encode(&originals()).unwrap();
    let result = decode(Part {
        version: 99,
        frames: &Value::Null,
        directory: Some(&encoded.directory),
        bytes: &encoded.bytes,
        raw_byte_length: Some(encoded.raw_byte_length),
        checksum: Some(&encoded.checksum),
        first_sequence: 1,
        last_sequence: i64::MAX,
    });
    assert!(result
        .err()
        .unwrap()
        .to_string()
        .contains("unknown client archive codec version"));
    // In particular a missing final zlib trailer byte must fail even though all
    // uncompressed bytes may already have been produced.
    for removed in [1, 4, 20] {
        let mut encoded = encode(&originals()).unwrap();
        encoded.bytes.truncate(encoded.bytes.len() - removed);
        assert_error(&encoded, "truncated");
    }
    let mut encoded = encode(&originals()).unwrap();
    encoded.bytes.push(0);
    assert_error(&encoded, "trailing");
}

#[test]
fn directory_corruption_is_authenticated_and_structural_bounds_remain_enforced() {
    let frames = originals();
    let raw: Vec<_> = frames
        .iter()
        .flat_map(|f| f.bytes.iter().copied())
        .collect();
    let mut encoded = encode(&frames).unwrap();
    encoded.directory[0] ^= 1;
    assert_error(&encoded, "checksum mismatch");
    // Re-sign controlled structural negatives so checks also exercise the
    // parser, rather than passing solely because of checksum authentication.
    encoded.checksum = checksum(&encoded.directory, &raw);
    assert_error(&encoded, "header invalid");
    let mut encoded = encode(&frames).unwrap();
    encoded.directory.pop();
    encoded.checksum = checksum(&encoded.directory, &raw);
    assert_error(&encoded, "truncated");
    let mut encoded = encode(&frames).unwrap();
    encoded.directory.push(0);
    encoded.checksum = checksum(&encoded.directory, &raw);
    assert_error(&encoded, "directory trailing bytes");
    let mut encoded = encode(&frames).unwrap();
    encoded.directory = MAGIC.to_vec();
    encoded.directory.extend_from_slice(&[255; 10]);
    encoded.checksum = checksum(&encoded.directory, &raw);
    assert_error(&encoded, "integer overflow");
    let mut encoded = encode(&frames).unwrap();
    encoded.directory = MAGIC.to_vec();
    put_uint(&mut encoded.directory, 1);
    put_uint(&mut encoded.directory, 1);
    encoded.directory.push(0);
    put_uint(&mut encoded.directory, 0);
    put_uint(
        &mut encoded.directory,
        u64::try_from(raw.len()).unwrap() + 1,
    );
    put_uint(&mut encoded.directory, 0);
    encoded.checksum = checksum(&encoded.directory, &raw);
    assert_error(&encoded, "out of bounds");
}

#[test]
fn rejects_frame_order_invalid_kind_timestamp_and_part_bounds() {
    let mut frames = originals();
    frames[1].sequence = 1;
    assert!(encode(&frames)
        .err()
        .unwrap()
        .to_string()
        .contains("sequence order"));
    let encoded = encode(&originals()).unwrap();
    assert!(decode(Part {
        version: VERSION,
        frames: &Value::Null,
        directory: Some(&encoded.directory),
        bytes: &encoded.bytes,
        raw_byte_length: Some(encoded.raw_byte_length),
        checksum: Some(&encoded.checksum),
        first_sequence: 2,
        last_sequence: i64::MAX,
    })
    .err()
    .unwrap()
    .to_string()
    .contains("sequence bounds"));
    let frames = vec![ClientTrajectoryArchiveFrame {
        sequence: 1,
        kind: ClientTrajectoryFrameKind::Request,
        observed_at: "a".into(),
        bytes: vec![0],
    }];
    for (index, value, expected) in [
        (6, 99, "frame kind invalid"),
        (10, 255, "timestamp encoding invalid"),
    ] {
        let mut encoded = encode(&frames).unwrap();
        encoded.directory[index] = value;
        encoded.checksum = checksum(&encoded.directory, &[0]);
        let result = decode(Part {
            version: VERSION,
            frames: &Value::Null,
            directory: Some(&encoded.directory),
            bytes: &encoded.bytes,
            raw_byte_length: Some(encoded.raw_byte_length),
            checksum: Some(&encoded.checksum),
            first_sequence: 1,
            last_sequence: 1,
        });
        assert!(format!("{:#}", result.err().unwrap()).contains(expected));
    }
}

#[test]
fn original_version_retains_wire_legacy_formats_and_rejects_truncated_spans() {
    let raw = b"\0\xff{}";
    let directory = json!([
        {"sequence":90,"kind":"request","observed_at":" odd \n","offset":0,"length":2,"format":"wire"},
        {"sequence":91,"kind":"response_json","observed_at":"","offset":2,"length":2,"format":"legacy_json"}
    ]);
    let frames = decode(Part {
        version: 0,
        frames: &directory,
        directory: None,
        bytes: raw,
        raw_byte_length: None,
        checksum: None,
        first_sequence: 90,
        last_sequence: 91,
    })
    .unwrap();
    assert_eq!(frames[0].frame.bytes, &[0, 255]);
    assert_eq!(frames[0].frame.observed_at, " odd \n");
    assert!(frames[1].format == Format::LegacyJson);
    assert!(decode(Part {
        version: 0,
        frames: &directory,
        directory: None,
        bytes: &raw[..3],
        raw_byte_length: None,
        checksum: None,
        first_sequence: 90,
        last_sequence: 91,
    })
    .err()
    .unwrap()
    .to_string()
    .contains("out of bounds"));
}
