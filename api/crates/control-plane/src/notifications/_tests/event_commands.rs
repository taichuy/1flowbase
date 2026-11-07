use super::*;
use domain::event_center::EventDefinitionRecord;

fn binding() -> CommunicationEventCommandBinding {
    CommunicationEventCommandBinding {
        command_id: "user/notify".into(),
        name: "notify".into(),
        description: "Publish request".into(),
        enabled: true,
        contract_id: "custom.notify".into(),
        input_mapping: std::collections::BTreeMap::from([
            ("message".into(), "/arguments".into()),
            ("input".into(), "/input_id".into()),
        ]),
    }
}
fn event(workspace_id: Uuid) -> EventDefinitionRecord {
    EventDefinitionRecord {
        interface_event: None,

        id: Uuid::now_v7(),
        workspace_id,
        contract_id: "custom.notify".into(),
        name: "Notify".into(),
        description: String::new(),
        enabled: true,
        payload_schema: json!({"type":"object","required":["message","input"],"additionalProperties":false,"properties":{"message":{"type":"string","minLength":1},"input":{"type":"string"}}}),
        created_by: Uuid::now_v7(),
        created_at: time::OffsetDateTime::now_utc(),
        updated_at: time::OffsetDateTime::now_utc(),
    }
}
#[test]
fn custom_command_identity_names_and_pointers_are_owned_and_unambiguous() {
    let good = binding();
    assert!(validate_declarations(&[good.clone()], &[]).is_ok());
    for (id, name) in [
        ("host/notify", "notify"),
        ("plugin/bot/notify", "notify"),
        ("user/notify", "new"),
        ("user/notify", "model-user-a"),
        ("user/notify", "0bad"),
    ] {
        let mut bad = good.clone();
        bad.command_id = id.into();
        bad.name = name.into();
        assert!(validate_declarations(&[bad], &[]).is_err());
    }
    assert!(validate_declarations(&[good.clone(), good.clone()], &[]).is_err());
    let plugin = CommunicationCommandView {
        command_id: "acme/bot/notify".into(),
        name: "notify".into(),
        description: String::new(),
        usage: "/notify".into(),
        source_kind: CommunicationCommandSourceKind::Plugin,
        source_name: "bot".into(),
        installation_id: Some(Uuid::now_v7()),
        enabled: false,
        available: false,
        unavailable_reason: None,
    };
    assert!(
        validate_declarations(&[good.clone()], &[plugin]).is_err(),
        "disabled plugins still own their names"
    );
    for pointer in [
        "arguments",
        "/arguments/~2",
        "/actor_user_id",
        "/workspace_id",
        "",
    ] {
        let mut bad = good.clone();
        bad.input_mapping.insert("message".into(), pointer.into());
        assert!(validate_declarations(&[bad], &[]).is_err());
    }
}
#[test]
fn workspace_and_identity_lookup_never_crosses_scopes_or_uses_disabled_definitions() {
    let workspace = Uuid::now_v7();
    let mut target = event(workspace);
    let command = binding();
    assert!(definition(&command, &[target.clone()], workspace).is_some());
    assert!(definition(&command, &[target.clone()], Uuid::now_v7()).is_none());
    target.enabled = false;
    assert!(definition(&command, &[target.clone()], workspace).is_none());
    target.enabled = true;
    target.contract_id = "custom.other".into();
    assert!(definition(&command, &[target], workspace).is_none());
}
#[test]
fn payload_uses_host_context_and_validates_actual_arguments() {
    let command = binding();
    let target = event(Uuid::now_v7());
    let context =
        json!({"arguments":"notify me", "input_id":"host-receipt", "workspace_id":"caller-forged"});
    assert_eq!(
        communication_event_payload(&command, &context, &target).unwrap(),
        json!({"message":"notify me", "input":"host-receipt"})
    );
    assert!(communication_event_payload(
        &command,
        &json!({"arguments":"", "input_id":"i"}),
        &target
    )
    .is_err());
    assert!(communication_event_payload(&command, &json!({"arguments":"hello"}), &target).is_err());
    let mut unsafe_mapping = command.clone();
    unsafe_mapping
        .input_mapping
        .insert("message".into(), "/workspace_id".into());
    assert!(communication_event_payload(&unsafe_mapping, &context, &target).is_err());
    let mut disabled = command;
    disabled.enabled = false;
    assert!(communication_event_payload(&disabled, &context, &target).is_err());
}

#[test]
fn mapping_shape_validation_rejects_missing_required_fields_and_nonstring_properties() {
    let command = binding();
    let mut target = event(Uuid::now_v7());
    assert!(validate_target_mapping(&command, &target).is_ok());
    target.payload_schema["required"] = json!(["unmapped"]);
    assert!(validate_target_mapping(&command, &target).is_err());
    target.payload_schema["required"] = json!(["message"]);
    target.payload_schema["properties"]["message"]["type"] = json!("integer");
    assert!(validate_target_mapping(&command, &target).is_err());
    target.payload_schema["properties"]["message"]["type"] = json!("string");
    target.payload_schema["properties"]["message"]["pattern"] = json!("^specific-input$");
    assert!(
        validate_target_mapping(&command, &target).is_ok(),
        "saving must not invent caller arguments for enum/pattern validation"
    );
}

#[test]
fn event_command_current_target_has_no_version_and_rejects_removed_input() {
    let encoded = serde_json::to_value(binding()).unwrap();
    assert!(encoded.get("contract_version").is_none());
    let mut old = encoded;
    old["contract_version"] = json!("1");
    assert!(serde_json::from_value::<CommunicationEventCommandBinding>(old).is_err());
}
