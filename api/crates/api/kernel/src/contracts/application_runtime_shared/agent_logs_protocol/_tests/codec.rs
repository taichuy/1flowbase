use super::*;

#[test]
fn official_json_protobuf_gzip_typed_any_values_and_nanoseconds_agree() {
    let request = traces_fixture();
    let json = encode_json(&request).unwrap();
    let protobuf = encode_protobuf(&request);
    for (mime, body) in [
        ("application/json; charset=utf-8", json),
        ("application/x-protobuf", protobuf),
    ] {
        assert_eq!(decode(mime, None, &body).unwrap(), request);
        assert_eq!(
            decode(mime, Some("gzip"), &encode_gzip(&body).unwrap()).unwrap(),
            request
        );
    }
    assert_eq!(
        request.resource_spans[0].scope_spans[0].spans[1].start_time_unix_nano,
        1791350400123456790
    );
    let serialized: Value = serde_json::from_slice(&encode_json(&request).unwrap()).unwrap();
    assert_eq!(
        serialized["resourceSpans"][0]["scopeSpans"][0]["spans"][1]["traceId"],
        "112233445566778899aabbccddeeff00"
    );
    assert_eq!(
        serialized["resourceSpans"][0]["scopeSpans"][0]["spans"][1]["startTimeUnixNano"],
        "1791350400123456790"
    );
}

#[test]
fn wire_validation_rejects_unsupported_mime_encoding_and_invalid_gzip() {
    assert!(decode("application/grpc", None, b"").is_err());
    assert!(decode("application/json", Some("br"), b"{}").is_err());
    assert!(decode("application/json", Some("gzip"), b"not gzip").is_err());
}

#[test]
fn optional_proto_fields_default_but_missing_source_identity_or_time_is_rejected() {
    assert!(
        traces_to_batches(&decode("application/json", None, b"{}").unwrap())
            .unwrap()
            .is_empty()
    );
    let bytes = br#"{"resourceSpans":[{"scopeSpans":[{"spans":[{"name":"missing-ids"}]}]}]}"#;
    assert_eq!(
        traces_to_batches(&decode("application/json", None, bytes).unwrap())
            .unwrap_err()
            .to_string(),
        "agent_logs.otlp_span_identity"
    );
    let mut request = traces_fixture();
    first_span(&mut request).start_time_unix_nano = 0;
    first_span(&mut request).end_time_unix_nano = 0;
    assert_eq!(
        traces_to_batches(&request).unwrap_err().to_string(),
        "agent_logs.otlp_timestamp_missing"
    );
}
