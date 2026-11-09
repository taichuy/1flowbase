use super::*;
use extension_contracts::ManagedProjectionContract;
use serde_json::json;

#[test]
fn otlp_managed_contracts_expose_only_counts_and_static_wire_tags() {
    let input = Input::new(
        WireFormat::Json,
        ContentEncoding::Gzip,
        Bytes::from_static(b"private body with raw source facts and bearer-like text"),
    );
    let projection = input.project_for_managed_hook().unwrap();
    assert_eq!(
        projection,
        json!({"kind":Input::CONTRACT_ID,"byte_count":input.byte_count,"format":"json","encoding":"gzip"})
    );
    let output = Output {
        response: ExportTraceServiceResponse {
            partial_success: Some(ExportTracePartialSuccess {
                rejected_spans: 3,
                error_message: "not projected".into(),
            }),
        },
        accepted_spans: 2,
        receipt: AgentLogsReceipt {
            accepted_events: 2,
            duplicate_events: 0,
            record_ids: vec![],
        },
    };
    let error = TargetError(ControlPlaneError::NotAuthenticated.into());
    for (id, schema, value) in [
        (
            Input::CONTRACT_ID,
            Input::managed_projection_schema().unwrap(),
            projection,
        ),
        (
            Output::CONTRACT_ID,
            Output::managed_projection_schema().unwrap(),
            output.project_for_managed_hook().unwrap(),
        ),
        (
            TargetError::CONTRACT_ID,
            TargetError::managed_projection_schema().unwrap(),
            error.project_for_managed_hook().unwrap(),
        ),
    ] {
        let contract = ManagedProjectionContract {
            contract_id: id.into(),
            contract_version: "1".into(),
            schema,
        }
        .compile_for_registration()
        .unwrap();
        contract.validate(&value).unwrap();
        let mut leaked = value.clone();
        leaked["body"] = json!("private source");
        assert!(contract.validate(&leaked).is_err());
        assert!(
            !value.to_string().contains("private") && !value.to_string().contains("not projected")
        );
    }
}

#[test]
fn otlp_event_projection_uses_decoded_json_protobuf_and_gzip_facts_and_true_receipts() {
    let batch = super::super::event_projection_tests::batch_without_task_end();
    let request = agent_logs_protocol::batches_to_traces(&[batch]).unwrap();
    for (format, bytes) in [
        (
            WireFormat::Json,
            agent_logs_protocol::encode_json(&request).unwrap(),
        ),
        (
            WireFormat::Protobuf,
            agent_logs_protocol::encode_protobuf(&request),
        ),
    ] {
        for encoding in [ContentEncoding::Identity, ContentEncoding::Gzip] {
            let bytes = if matches!(encoding, ContentEncoding::Gzip) {
                agent_logs_protocol::encode_gzip(&bytes).unwrap()
            } else {
                bytes.clone()
            };
            let input = Input::new(format, encoding, Bytes::from(bytes));
            let value = input.project_for_event().unwrap();
            assert_eq!(
                value["batches"][0]["events"][0]["content"],
                "business answer"
            );
            assert_eq!(value["batches"][0]["events"][0]["kind"], "assistant");
            assert!(!value.to_string().contains("RAW_SECRET"));
            assert!(
                jsonschema::validator_for(&Input::event_projection_schema().unwrap())
                    .unwrap()
                    .is_valid(&value)
            );
        }
    }
    let record_id = uuid::Uuid::now_v7();
    let output = Output {
        accepted_spans: 3,
        receipt: AgentLogsReceipt {
            accepted_events: 2,
            duplicate_events: 1,
            record_ids: vec![record_id],
        },
        response: ExportTraceServiceResponse {
            partial_success: Some(ExportTracePartialSuccess {
                rejected_spans: 4,
                error_message: "safe partial receipt".into(),
            }),
        },
    };
    let value = output.project_for_event().unwrap();
    assert_eq!(
        value,
        json!({"accepted_spans":3,"rejected_spans":4,"accepted_events":2,"duplicate_events":1,"record_ids":[record_id]})
    );
    assert!(
        jsonschema::validator_for(&Output::event_projection_schema().unwrap())
            .unwrap()
            .is_valid(&value)
    );
}

#[test]
fn opaque_or_invalid_otlp_wire_has_no_business_event_projection() {
    for (format, encoding, bytes) in [
        (
            WireFormat::Json,
            ContentEncoding::Identity,
            b"not json".as_slice(),
        ),
        (
            WireFormat::Protobuf,
            ContentEncoding::Identity,
            b"\xff".as_slice(),
        ),
        (
            WireFormat::Json,
            ContentEncoding::Gzip,
            b"not gzip".as_slice(),
        ),
        (
            WireFormat::Unsupported,
            ContentEncoding::Identity,
            b"{}".as_slice(),
        ),
        (
            WireFormat::Json,
            ContentEncoding::Unsupported,
            b"{}".as_slice(),
        ),
    ] {
        let input = Input::new(format, encoding, Bytes::copy_from_slice(bytes));
        assert!(input.prepared.is_err());
        assert!(input.project_for_event().is_none());
    }
}

#[test]
fn partial_attribution_counts_input_spans_not_events_or_deduplicated_spans() {
    let mut request = agent_logs_protocol::decode(
        "application/json",
        None,
        include_bytes!("../../../../../agent_logs_protocol/_tests/fixtures/openlit.json"),
    )
    .unwrap();
    let span = request.resource_spans[0].scope_spans[0].spans[1].clone();
    request.resource_spans[0].scope_spans[0].spans.push(span);
    let (batches, counts) = validated_batches(&request).unwrap();
    assert_eq!(counts.values().sum::<usize>(), 5);
    assert!(
        batches
            .iter()
            .map(|batch| batch.events.len())
            .sum::<usize>()
            > 5
    );
    assert_eq!(counts.values().copied().collect::<Vec<_>>(), [4, 1]);
}
