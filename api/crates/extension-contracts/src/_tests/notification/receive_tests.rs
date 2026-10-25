use crate::*;
use serde_json::{json, Value};

fn descriptor() -> NotificationTypeDescriptor {
    serde_json::from_value(json!({
        "type_code":"acme/fixture/chat","display_name":"Chat","description":"Communications fixture",
        "config_schema":{"type":"object"},"config_form":{"schema_version":"1.0.0","fields":[]},
        "input_schema":{"type":"object"},"input_form":{"schema_version":"1.0.0","fields":[]},
        "output_schema":{"type":"object"},"contribution_id":"fixture.send",
        "authorization":{"actions":[{"code":"qr","label":"Scan","kind":"qr_code"}],"managed_secret_keys":["context_token"]},
        "receive":{"contribution_id":"fixture.receive","default_system_prompt":"Use channel tools to read history and files.",
            "mcp_tools":[
                {"name":"messages","description":"Read scoped messages","operation":"history_messages","input_schema":{"type":"object"}},
                {"name":"files","description":"Read scoped files","operation":"history_files","input_schema":{"type":"object"}},
                {"name":"read_file","description":"Read scoped file content","operation":"read_file","input_schema":{"type":"object"}}
            ]}
    })).unwrap()
}

fn input() -> NotificationRuntimeInput {
    serde_json::from_value(json!({"operation":{"type":"receive","cursor":"opaque-provider-cursor","max_messages":10,"wait_seconds":20}})).unwrap()
}

fn result() -> Value {
    json!({"type":"receive","outcome":{"status":"succeeded","batch":{
        "messages":[{"provider_message_id":"m1","conversation_id":"private-peer","sender":{"id":"peer"},"received_at_ms":1,"text":"Hello","attachments":[{
            "provider_attachment_id":"a1","file_name":"photo.jpg","media_type":"image/jpeg","size_bytes":100_000_000,
            "reference":{"provider_message_id":"m1","provider_attachment_id":"a1","download_token":{"cdn_token":"provider-private"}}
        }]}],"candidate_cursor":"next-opaque-cursor","has_more":false
    }},"managed_secret_patch":{"context_token":"provider-context"}})
}

#[test]
fn receive_keeps_candidate_cursor_and_download_locator_separate_from_secrets() {
    let descriptor = descriptor();
    descriptor.validate().unwrap();
    let input = input();
    input.validate(&descriptor).unwrap();
    let wire = result();
    let result: NotificationRuntimeResult = serde_json::from_value(wire.clone()).unwrap();
    result.validate_for(&input, &descriptor).unwrap();
    assert_eq!(serde_json::to_value(result).unwrap(), wire);
    assert!(!wire["managed_secret_patch"]
        .as_object()
        .unwrap()
        .contains_key("cursor"));
    // Repeated provider delivery is valid across batches: deduplication belongs to durable Host inbox.
    let repeated: NotificationRuntimeResult = serde_json::from_value(wire).unwrap();
    repeated.validate_for(&input, &descriptor).unwrap();
}

#[test]
fn receive_rejects_bad_identities_duplicates_secret_cursor_and_mismatched_operation() {
    let descriptor = descriptor();
    let input = input();
    for (pointer, replacement) in [
        ("/outcome/batch/messages/0/provider_message_id", json!("")),
        ("/outcome/batch/messages/0/sender/id", json!("")),
        ("/outcome/batch/messages/0/received_at_ms", json!(0)),
        (
            "/outcome/batch/messages/0/attachments/0/reference/provider_message_id",
            json!("other"),
        ),
        (
            "/managed_secret_patch",
            json!({"cursor":"lost-before-inbox"}),
        ),
        ("/managed_secret_patch", json!({"undeclared":"private"})),
    ] {
        let mut wire = result();
        *wire.pointer_mut(pointer).unwrap() = replacement;
        let result: NotificationRuntimeResult = serde_json::from_value(wire).unwrap();
        assert!(
            result.validate_for(&input, &descriptor).is_err(),
            "{pointer}"
        );
    }
    let mut duplicate = result();
    let message = duplicate["outcome"]["batch"]["messages"][0].clone();
    duplicate["outcome"]["batch"]["messages"]
        .as_array_mut()
        .unwrap()
        .push(message);
    let duplicate: NotificationRuntimeResult = serde_json::from_value(duplicate).unwrap();
    assert!(duplicate.validate_for(&input, &descriptor).is_err());
    let result: NotificationRuntimeResult = serde_json::from_value(result()).unwrap();
    let mut send = input.clone();
    send.operation = NotificationOperation::Send {
        target: None,
        input_payload: json!({}),
    };
    assert!(result.validate_for(&send, &descriptor).is_err());
    let mut send_only = descriptor.clone();
    send_only.receive = None;
    assert!(input.validate(&send_only).is_err());
    assert!(send.validate(&send_only).is_ok());
}

#[test]
fn receive_descriptor_requires_separate_binding_prompt_and_all_scoped_mcp_operations() {
    let original = descriptor();
    let mut value = original.clone();
    value.receive.as_mut().unwrap().contribution_id = value.contribution_id.clone();
    assert!(value.validate().is_err());
    let mut value = original.clone();
    value
        .receive
        .as_mut()
        .unwrap()
        .default_system_prompt
        .clear();
    assert!(value.validate().is_err());
    let mut value = original.clone();
    value.receive.as_mut().unwrap().mcp_tools.pop();
    assert!(value.validate().is_err());
    let mut value = original.clone();
    value.receive.as_mut().unwrap().mcp_tools[0].input_schema =
        json!({"$ref":"https://invalid/schema"});
    assert!(value.validate().is_err());
    let mut value = original;
    value
        .authorization
        .as_mut()
        .unwrap()
        .managed_secret_keys
        .push("cursor".into());
    assert!(value.validate().is_err());
}

#[test]
fn attachment_download_is_chunked_and_rejects_offset_size_and_progress_errors() {
    let descriptor = descriptor();
    let attachment = serde_json::from_value(
        result()["outcome"]["batch"]["messages"][0]["attachments"][0]["reference"].clone(),
    )
    .unwrap();
    let input = NotificationRuntimeInput {
        delivery_window: None,
        managed_secrets: Default::default(),
        operation: NotificationOperation::DownloadAttachment {
            attachment,
            offset: 100,
            max_bytes: 3,
        },
    };
    for (outcome, valid) in [
        (
            json!({"status":"succeeded","offset":100,"data_base64":"YWJj","eof":false}),
            true,
        ),
        (
            json!({"status":"succeeded","offset":100,"data_base64":"","eof":true}),
            true,
        ),
        (
            json!({"status":"succeeded","offset":99,"data_base64":"YWJj","eof":true}),
            false,
        ),
        (
            json!({"status":"succeeded","offset":100,"data_base64":"YWJjZA==","eof":true}),
            false,
        ),
        (
            json!({"status":"succeeded","offset":100,"data_base64":"","eof":false}),
            false,
        ),
        (
            json!({"status":"succeeded","offset":100,"data_base64":"!!","eof":true}),
            false,
        ),
    ] {
        let result: NotificationRuntimeResult =
            serde_json::from_value(json!({"type":"download_attachment","outcome":outcome}))
                .unwrap();
        assert_eq!(result.validate_for(&input, &descriptor).is_ok(), valid);
    }
}

#[test]
fn receive_poll_and_batch_budgets_are_enforced_before_provider_io() {
    let descriptor = descriptor();
    for operation in [
        json!({"type":"receive","max_messages":0,"wait_seconds":1}),
        json!({"type":"receive","max_messages":101,"wait_seconds":1}),
        json!({"type":"receive","max_messages":1,"wait_seconds":31}),
        json!({"type":"receive","max_messages":1,"wait_seconds":1,"cursor":"x".repeat(16385)}),
    ] {
        let input: NotificationRuntimeInput =
            serde_json::from_value(json!({"operation":operation})).unwrap();
        assert!(input.validate(&descriptor).is_err());
    }
    let mut wire = result();
    wire["outcome"]["batch"]["messages"][0]["text"] =
        json!("x".repeat(NOTIFICATION_MAX_RECEIVE_BATCH_BYTES));
    let result: NotificationRuntimeResult = serde_json::from_value(wire).unwrap();
    assert!(result.validate_for(&input(), &descriptor).is_err());
    assert!(serde_json::from_value::<NotificationRuntimeInput>(
        json!({"operation":{"type":"receive","max_messages":1,"wait_seconds":1,"ack":true}})
    )
    .is_err());
}

#[test]
fn receive_cursor_roundtrip_uses_the_same_budget_as_committed_batch() {
    let descriptor = descriptor();
    let cursor = json!({"pending":"x".repeat(32*1024)});
    let mut wire = result();
    wire["outcome"]["batch"]["candidate_cursor"] = cursor.clone();
    let result: NotificationRuntimeResult = serde_json::from_value(wire).unwrap();
    result.validate_for(&input(), &descriptor).unwrap();
    let mut next = input();
    next.operation = NotificationOperation::Receive {
        cursor: Some(cursor),
        max_messages: 10,
        wait_seconds: 20,
    };
    next.validate(&descriptor).unwrap();
    next.operation = NotificationOperation::Receive {
        cursor: Some(json!("x".repeat(NOTIFICATION_MAX_RECEIVE_BATCH_BYTES))),
        max_messages: 10,
        wait_seconds: 20,
    };
    assert!(next.validate(&descriptor).is_err());
}
#[test]
fn reply_target_is_explicit_and_rejects_missing_identity() {
    let descriptor = descriptor();
    let mut request = input();
    request.operation = NotificationOperation::Send {
        input_payload: json!({}),
        target: Some(NotificationSendTarget {
            sender_id: "peer".into(),
            conversation_id: "private-peer".into(),
            provider_message_id: "m1".into(),
        }),
    };
    request.validate(&descriptor).unwrap();
    if let NotificationOperation::Send {
        target: Some(target),
        ..
    } = &mut request.operation
    {
        target.sender_id.clear();
    }
    assert!(request.validate(&descriptor).is_err());
}
