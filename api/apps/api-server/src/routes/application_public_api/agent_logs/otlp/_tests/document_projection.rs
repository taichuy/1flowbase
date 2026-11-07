use super::*;
use crate::event_center::api_projection::{
    ApiEventInput, ApiEventOperationContext, ApiEventOutput,
};
fn operation(id: &str) -> ApiEventOperationContext {
    ApiEventOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/logs/otlp/v1/traces".into(),
    }
}
#[test]
fn otlp_document_uses_decoded_spans_for_json_protobuf_and_gzip_without_attributes() {
    let mut batch = super::super::super::event_projection_tests::batch_without_task_end();
    batch.events = (0..45)
        .map(|index| {
            let mut event = batch.events[0].clone();
            event.event_id = format!("event-{index}");
            event.sequence = index;
            event
        })
        .collect();
    let request = crate::agent_logs_protocol::batches_to_traces(&[batch]).unwrap();
    for (format, bytes) in [
        (
            WireFormat::Json,
            crate::agent_logs_protocol::encode_json(&request).unwrap(),
        ),
        (
            WireFormat::Protobuf,
            crate::agent_logs_protocol::encode_protobuf(&request),
        ),
    ] {
        for encoding in [ContentEncoding::Identity, ContentEncoding::Gzip] {
            let encoded = if matches!(encoding, ContentEncoding::Gzip) {
                crate::agent_logs_protocol::encode_gzip(&bytes).unwrap()
            } else {
                bytes.clone()
            };
            let input = Input::new(format, encoding, axum::body::Bytes::from(encoded));
            let value = input.project_document_input(&operation(ID)).unwrap();
            let count = value["body"]["resourceSpans"]
                .as_array()
                .unwrap()
                .iter()
                .flat_map(|resource| resource["scopeSpans"].as_array().unwrap())
                .map(|scope| scope["spans"].as_array().unwrap().len())
                .sum::<usize>();
            assert_eq!(count, 45);
            assert!(!value.to_string().contains("attributes"));
            assert!(!value.to_string().contains("RAW_SECRET"));
            assert!(jsonschema::validator_for(
                &Input::document_input_schema(&operation(ID)).unwrap()
            )
            .unwrap()
            .is_valid(&value));
        }
    }
}
