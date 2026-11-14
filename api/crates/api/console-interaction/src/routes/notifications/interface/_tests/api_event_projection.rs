use super::*;
fn operation(id: &str) -> ApiOperationContext {
    ApiOperationContext {
        interface_id: id.into(),
        method: "POST".into(),
        path: "/api/console/communications/instances".into(),
    }
}
#[test]
fn notification_before_omits_mixed_secret_config_but_send_business_input_matches_real_body() {
    let op = operation("notifications.instances.create");
    let input = NotificationInput::Create(CreateNotificationInstance {
        type_code: "type-a".into(),
        installation_id: None,
        display_name: "Sender".into(),
        config_json: json!({"token":"NOTIFICATION-KEY-SENTINEL"}),
        status: Some(domain::NotificationInstanceStatus::Enabled),
    });
    let value = input.project_document_input(&op).unwrap();
    assert_eq!(value["body"]["display_name"], "Sender");
    assert_eq!(value["body"]["status"], "enabled");
    assert!(value["body"].get("config_json").is_none());
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(!NotificationInput::document_input_schema(&op)
        .unwrap()
        .to_string()
        .contains("config_json"));
    let payload = json!({"text":"Message".repeat(300),"business_metadata":{"secret_label":"Public field title","values":(0..90).collect::<Vec<_>>()}});
    let input = NotificationInput::Send(Uuid::nil(), payload.clone());
    let op = operation("notifications.instances.send");
    assert_eq!(
        input.project_document_input(&op).unwrap(),
        json!({"path":{"instance_id":Uuid::nil()},"body":{"input":payload}})
    );
    // The old persisted-rule shape remains distinct and unchanged.
    assert_eq!(
        input.project_for_event().unwrap(),
        json!({"instance_id":Uuid::nil(),"input_payload":payload})
    );
}
#[test]
fn notification_authorization_challenge_is_omitted_from_schema_and_value() {
    let op = operation("notifications.instances.authorization");
    let original = json!({"status":"pending","delivery_window":{"status":"waiting_for_message"},"user_action":{"kind":"qr_code","qr_code_url":"https://provider.test/qr?proof=QR-CHALLENGE-SENTINEL","poll_interval_seconds":5,"expires_at":"2026-11-07"}});
    let output = NotificationOutput(original.clone());
    let value = output.project_document_output(&op).unwrap();
    assert_eq!(value["status"], "pending");
    assert_eq!(value["user_action"]["poll_interval_seconds"], 5);
    assert!(value["user_action"].get("qr_code_url").is_none());
    assert!(!value.to_string().contains("SENTINEL"));
    assert!(!NotificationOutput::document_output_schema(&op)
        .unwrap()
        .to_string()
        .contains("qr_code_url"));
    assert_eq!(output.0, original);
}
#[test]
fn notification_instance_public_config_is_complete_while_nested_challenge_is_safe() {
    let op = operation("notifications.instances.get");
    let public = json!({"business_metadata":{"secret_label":"Public metadata","entries":(0..100).collect::<Vec<_>>()}});
    let output = NotificationOutput(
        json!({"id":Uuid::nil(),"workspace_id":Uuid::nil(),"type_code":"type-a","installation_id":null,"display_name":"Sender","status":"enabled","config_json":public,"configured_secret_fields":["token"],"created_at":"2026-11-07","updated_at":"2026-11-07","available":true,"unavailable_reason":null,"authorization":{"status":"pending","user_action":{"kind":"qr_code","qr_code_url":"QR-SECRET-SENTINEL","poll_interval_seconds":5}}}),
    );
    let value = output.project_document_output(&op).unwrap();
    assert_eq!(value["config_json"], public);
    assert_eq!(value["configured_secret_fields"], json!(["token"]));
    assert!(!value.to_string().contains("SENTINEL"));
    let output = NotificationOutput(
        json!({"instance_id":Uuid::nil(),"type_code":"type-a","result":{"text":"Result".repeat(300),"values":(0..100).collect::<Vec<_>>()}}),
    );
    let expected = output.0.clone();
    assert_eq!(
        output
            .project_document_output(&operation("notifications.instances.send"))
            .unwrap(),
        expected
    );
    assert_eq!(output.project_for_event().unwrap(), expected);
}
