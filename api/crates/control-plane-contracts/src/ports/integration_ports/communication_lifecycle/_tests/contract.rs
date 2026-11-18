use super::*;
use std::collections::BTreeSet;

fn assert_fields_are_declared(payload: Value, schema: Value) {
    let actual = payload
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let declared = schema["properties"]
        .as_object()
        .unwrap()
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    let required = schema["required"]
        .as_array()
        .unwrap()
        .iter()
        .map(|field| field.as_str().unwrap().to_owned())
        .collect::<BTreeSet<_>>();
    assert_eq!(actual, declared);
    assert_eq!(actual, required);
    assert_eq!(schema["additionalProperties"], false);
}

#[test]
fn communication_fact_contract_schemas_match_serialized_business_fields() {
    let workspace_id = Uuid::now_v7();
    let instance_id = Uuid::now_v7();
    let input_id = Uuid::now_v7();
    let received = CommunicationReceivedFact {
        workspace_id,
        instance_id,
        conversation_id: Uuid::now_v7(),
        input_id,
        provider_message_id: "provider-key".into(),
        sender_id: "sender".into(),
        received_at_ms: 42,
        contents: vec![
            CommunicationFactContent::Text {
                message_id: Some(input_id),
                content: "hello".into(),
            },
            CommunicationFactContent::File {
                message_id: Uuid::now_v7(),
                file_name: "photo.png".into(),
                media_type: "image/png".into(),
                size_bytes: Some(25),
            },
        ],
        workflow_ancestor_ids: vec![],
    };
    assert_fields_are_declared(
        serde_json::to_value(&received).unwrap(),
        CommunicationReceivedFact::event_source_schema(),
    );
    assert_eq!(
        CommunicationLifecycleFact::Received(received.clone()).contract_id(),
        COMMUNICATION_RECEIVED_CONTRACT_ID
    );
    let payload = serde_json::to_value(&received).unwrap();
    assert_eq!(payload["contents"][0]["type"], "text");
    assert_eq!(payload["contents"][1]["type"], "file");
    assert_eq!(payload["contents"][1]["media_type"], "image/png");
    let result = CommunicationSendResultFact {
        workspace_id,
        instance_id,
        send_log_id: Some(Uuid::now_v7()),
        input_id: None,
        turn_id: None,
        conversation_id: None,
        type_code: Some("fixture.notification".into()),
        source: Some(domain::NotificationSource::Api),
        result_kind: CommunicationSendResultKind::Accepted,
        failure_code: None,
        target: None,
        content: Some(CommunicationFactContent::Text {
            message_id: None,
            content: "sent".into(),
        }),
        occurred_at_ms: 42,
        workflow_ancestor_ids: vec![],
    };
    assert_fields_are_declared(
        serde_json::to_value(&result).unwrap(),
        CommunicationSendResultFact::event_source_schema(),
    );
    assert_eq!(
        CommunicationLifecycleFact::SendResult(result).contract_id(),
        COMMUNICATION_SEND_RESULT_CONTRACT_ID
    );
    let schema = CommunicationSendResultFact::event_source_schema();
    assert_eq!(
        schema["properties"]["result_kind"]["enum"],
        json!(["accepted", "failed", "unknown"])
    );
    assert_eq!(
        schema["properties"]["content"]["oneOf"][1]["properties"]["type"]["const"],
        "text"
    );
    for private in [
        "managed_secrets",
        "encrypted_reference",
        "candidate_cursor",
        "config_json",
        "delivered",
        "read",
    ] {
        assert!(!schema.to_string().contains(private), "{private}");
    }
}
