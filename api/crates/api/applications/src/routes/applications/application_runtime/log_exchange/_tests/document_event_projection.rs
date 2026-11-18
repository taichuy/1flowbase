use super::*;

#[test]
fn application_document_otlp_keeps_actual_spans_and_omits_opaque_attributes() {
    let input = json!({"resourceSpans":[{"resource":{"attributes":[{"key":"http.request.header.authorization","value":{"stringValue":"OTLP_DOCUMENT_SENTINEL_SECRET"}}]},"scopeSpans":[{"scope":{"name":"actual scope","attributes":[{"key":"api_key","value":{"stringValue":"OTLP_DOCUMENT_SENTINEL_SECRET"}}]},"spans":(0..40).map(|index|json!({"traceId":"01010101010101010101010101010101","spanId":"0101010101010101","name":format!("actual span {index}"),"startTimeUnixNano":"1000000000000000000","endTimeUnixNano":"1000000000000000001","attributes":[{"key":"authorization","value":{"stringValue":"OTLP_DOCUMENT_SENTINEL_SECRET"}}],"events":[{"timeUnixNano":"1000000000000000000","name":"actual event","attributes":[{"key":"token","value":{"stringValue":"OTLP_DOCUMENT_SENTINEL_SECRET"}}]}]})).collect::<Vec<_>>()}]}]});
    let value = ep::to_value(&document(&input).unwrap()).unwrap();
    assert_eq!(
        value["resourceSpans"][0]["scopeSpans"][0]["spans"]
            .as_array()
            .unwrap()
            .len(),
        40
    );
    assert_eq!(
        value["resourceSpans"][0]["scopeSpans"][0]["spans"][39]["name"],
        "actual span 39"
    );
    assert_eq!(
        value["resourceSpans"][0]["scopeSpans"][0]["spans"][0]["events"][0]["name"],
        "actual event"
    );
    assert!(!value.to_string().contains("OTLP_DOCUMENT_SENTINEL_SECRET"));
    let schema = ep::schema::<OtlpDocument>();
    assert!(!schema.to_string().contains("\"attributes\""));
    assert!(jsonschema::validator_for(&schema).unwrap().is_valid(&value));
}
