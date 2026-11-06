use super::*;
#[test]
fn pointers_accept_root_and_escaped_tokens_and_reject_malformed_escapes() {
    for pointer in ["", "/payload/order_id", "/a~1b/~0", "/", "/中文"] {
        assert!(is_json_pointer(pointer));
    }
    for pointer in ["payload", "#/payload", "/~", "/~2", "/a~x"] {
        assert!(!is_json_pointer(pointer));
    }
}
#[test]
fn consumed_is_a_distinct_dequeue_phase_not_an_ack_alias() {
    assert_eq!(
        serde_json::to_value(EventTriggerPhase::Consumed).unwrap(),
        "consumed"
    );
    assert!(serde_json::from_str::<EventTriggerPhase>("\"acknowledged\"").is_err());
    assert!(serde_json::from_str::<EventTriggerPhase>("\"processed\"").is_err());
}

#[test]
fn source_selection_is_additive_and_keeps_same_uuid_workspace_distinct_from_system() {
    let legacy = serde_json::json!({"enabled":true,"contract_id":"custom.orders","contract_version":"1","trigger_phase":"published","input_mapping":{}});
    let mut input: WorkflowEventTriggerInput = serde_json::from_value(legacy.clone()).unwrap();
    assert!(input.source.is_none());
    assert_eq!(serde_json::to_value(&input).unwrap(), legacy);
    input.source = Some(EventSourceSelection {
        source_id: "native.fact".into(),
        scope: EventSourceScope::System,
    });
    let value = serde_json::to_value(&input).unwrap();
    assert_eq!(
        value["source"],
        serde_json::json!({"source_id":"native.fact","scope":{"kind":"system"}})
    );
    assert_eq!(
        serde_json::from_value::<WorkflowEventTriggerInput>(value).unwrap(),
        input
    );
    assert_ne!(
        input.source.as_ref().unwrap().scope,
        EventSourceScope::Workspace {
            workspace_id: crate::SYSTEM_SCOPE_ID
        }
    );
}
