use super::*;

fn descriptor() -> NotificationTypeDescriptor {
    let mut descriptor = system_notification_descriptor();
    descriptor.type_code = "example/notification/private".into();
    descriptor.authorization = Some(
        serde_json::from_value(json!({
            "actions":[{"code":"qr","label":"Scan","kind":"qr_code"}],
            "managed_secret_keys":["token","user_id"]
        }))
        .unwrap(),
    );
    descriptor
}

fn types(old: &NotificationTypeDescriptor, new: &NotificationTypeDescriptor) -> Vec<String> {
    compatible_notification_migration_types(
        &json!({"notification_types":[old]}),
        &json!({"notification_types":[new]}),
    )
    .unwrap()
}

#[test]
fn migration_ignores_presentation_and_secret_key_order() {
    let old = descriptor();
    let mut new = old.clone();
    new.display_name = "New display name".into();
    new.description = "New description".into();
    let authorization = new.authorization.as_mut().unwrap();
    authorization.actions[0].label = "扫码".into();
    authorization.managed_secret_keys.reverse();
    assert_eq!(types(&old, &new), vec![old.type_code]);
}

#[test]
fn migration_rejects_changed_type_config_payload_or_authority() {
    let old = descriptor();
    let mut candidates = vec![];
    let mut changed = old.clone();
    changed.type_code = "example/notification/other".into();
    candidates.push(changed);
    let mut changed = old.clone();
    changed.config_schema = json!({"type":"object","properties":{"mode":{"type":"string"}}});
    candidates.push(changed);
    let mut changed = old.clone();
    changed.input_schema["additionalProperties"] = json!(true);
    candidates.push(changed);
    let mut changed = old.clone();
    changed
        .authorization
        .as_mut()
        .unwrap()
        .managed_secret_keys
        .push("other".into());
    candidates.push(changed);
    let mut changed = old.clone();
    changed.authorization = None;
    candidates.push(changed);
    for changed in candidates {
        assert!(types(&old, &changed).is_empty());
    }
}

#[test]
fn migration_rejects_ambiguous_metadata_and_removed_types() {
    let old = descriptor();
    let metadata = json!({"notification_types":[old]});
    assert!(
        compatible_notification_migration_types(&metadata, &json!({}))
            .unwrap()
            .is_empty()
    );
    assert!(compatible_notification_migration_types(
        &metadata,
        &json!({"notification_types":[old,old]})
    )
    .is_err());
    assert!(compatible_notification_migration_types(
        &metadata,
        &json!({"notification_types":null})
    )
    .is_err());
}

#[test]
fn migration_preserves_receive_identity_while_allowing_tool_contract_updates() {
    let mut old = descriptor();
    let mcp_tools = ["history_messages", "history_files", "read_file"].map(|name| {
        json!({
            "name":name,"description":name,"operation":name,"input_schema":{"type":"object"}
        })
    });
    old.receive = Some(
        serde_json::from_value(json!({
            "contribution_id":"example.notification.receive",
            "default_system_prompt":"Reply to the current conversation",
            "mcp_tools":mcp_tools
        }))
        .unwrap(),
    );
    let mut new = old.clone();
    new.receive.as_mut().unwrap().default_system_prompt = "Updated default".into();
    new.receive.as_mut().unwrap().mcp_tools[2].input_schema = json!({
        "type":"object","properties":{"message_id":{"type":"string"}},"required":["message_id"]
    });
    assert_eq!(types(&old, &new), vec![old.type_code.clone()]);
    new.receive.as_mut().unwrap().contribution_id =
        ContributionId::new("example.other.receive").unwrap();
    assert!(types(&old, &new).is_empty());
    new.receive = None;
    assert!(types(&old, &new).is_empty());
    assert_eq!(types(&new, &old), vec![old.type_code]);
}

#[test]
fn migration_rejects_secret_visibility_and_send_contribution_changes() {
    let mut old = descriptor();
    old.config_schema = json!({"type":"object","properties":{"password":{"type":"string"}}});
    old.config_form = serde_json::from_value(json!({"schema_version":"1.0.0","fields":[
        {"key":"password","label":"Password","type":"string","control":"password"}
    ]}))
    .unwrap();
    let mut new = old.clone();
    new.config_form = serde_json::from_value(json!({"schema_version":"1.0.0","fields":[
        {"key":"password","label":"Password","type":"string"}
    ]}))
    .unwrap();
    assert!(types(&old, &new).is_empty());
    let mut new = old.clone();
    new.contribution_id = ContributionId::new("example.other.send").unwrap();
    assert!(types(&old, &new).is_empty());
}
