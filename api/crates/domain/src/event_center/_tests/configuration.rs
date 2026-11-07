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
    let legacy = serde_json::json!({"enabled":true,"contract_id":"custom.orders","trigger_phase":"published","input_mapping":{}});
    let mut input: WorkflowEventTriggerInput = serde_json::from_value(legacy.clone()).unwrap();
    assert!(input.source.is_none());
    assert_eq!(serde_json::to_value(&input).unwrap(), legacy);
    input.source = Some(EventSourceSelection {
        filters: Default::default(),

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

#[test]
fn event_center_inputs_reject_removed_version() {
    let current = serde_json::json!({"enabled":true,"name":"Orders","description":"","interface_event":{"interface_id":"orders.create","phase":"before"}});
    let input: CreateEventDefinitionInput = serde_json::from_value(current.clone()).unwrap();
    assert_eq!(serde_json::to_value(input).unwrap(), current);
    let mut old = current;
    old["contract_version"] = serde_json::json!("1");
    assert!(serde_json::from_value::<CreateEventDefinitionInput>(old).is_err());
}

#[test]
fn workflow_input_rejects_removed_version() {
    let mut old = serde_json::json!({"enabled":true,"contract_id":"custom.orders","trigger_phase":"published","input_mapping":{}});
    assert!(serde_json::from_value::<WorkflowEventTriggerInput>(old.clone()).is_ok());
    old["contract_version"] = serde_json::json!("1");
    assert!(serde_json::from_value::<WorkflowEventTriggerInput>(old).is_err());
}

#[test]
fn interface_rules_reject_free_schema_and_filter_by_terminal() {
    let input = serde_json::json!({"name":"Orders","description":"","enabled":true,"interface_event":{"interface_id":"orders.create","phase":"completion","results":["failed"]}});
    let rule: CreateEventDefinitionInput = serde_json::from_value(input.clone()).unwrap();
    assert!(rule.interface_event.matches(
        "orders.create",
        InterfaceEventPhase::Completion,
        Some(InterfaceEventTerminal::Failed)
    ));
    assert!(!rule.interface_event.matches(
        "orders.create",
        InterfaceEventPhase::Completion,
        Some(InterfaceEventTerminal::Completed)
    ));
    let mut free = input;
    free["payload_schema"] = serde_json::json!({});
    assert!(serde_json::from_value::<CreateEventDefinitionInput>(free).is_err());
}
