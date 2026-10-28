use crate::{
    CommunicationCommandDescriptor, CommunicationCommandReply, CommunicationCommandRequest,
};
use serde_json::json;

fn command() -> CommunicationCommandDescriptor {
    serde_json::from_value(json!({
        "command_id":"acme/fixture/status","name":"status",
        "description":"Show plugin status","usage":"/status",
        "contribution_id":"fixture.command.status"
    }))
    .unwrap()
}

#[test]
fn communication_command_declaration_preserves_identity_and_defaults_disabled() {
    let command = command();
    command
        .validate_plugin_namespace("acme", "fixture")
        .unwrap();
    assert!(!command.default_enabled);
    assert_eq!(
        serde_json::from_value::<CommunicationCommandDescriptor>(
            serde_json::to_value(&command).unwrap()
        )
        .unwrap(),
        command
    );
}

#[test]
fn communication_command_declaration_rejects_namespace_reserved_and_ambiguous_spelling() {
    for name in [
        "new",
        "help",
        "model",
        "model-user",
        "model-user-gpt",
        "/status",
        "Status",
        "a b",
        "a/b",
        "",
    ] {
        let mut value = command();
        value.name = name.into();
        value.usage = format!("/{name}");
        assert!(
            value.validate_plugin_namespace("acme", "fixture").is_err(),
            "{name}"
        );
    }
    let mut value = command();
    value.command_id = "another/fixture/status".into();
    assert!(value.validate_plugin_namespace("acme", "fixture").is_err());
    let mut value = command();
    value.usage = "/status-other".into();
    assert!(value.validate().is_err());
}

#[test]
fn communication_command_event_separates_arguments_from_trusted_execution_context() {
    let value = json!({"command_id":"acme/fixture/status","arguments":"details",
        "channel_id":"channel-a","conversation_id":"history-a","session_id":"session-a","input_id":"input-a"});
    let request: CommunicationCommandRequest = serde_json::from_value(value.clone()).unwrap();
    request.validate().unwrap();
    let validator =
        jsonschema::validator_for(&CommunicationCommandRequest::event_schema()).unwrap();
    assert!(validator.is_valid(&value));
    let mut forged = value.clone();
    forged["workspace_id"] = json!("foreign");
    assert!(!validator.is_valid(&forged));
    assert!(serde_json::from_value::<CommunicationCommandRequest>(forged).is_err());
    let mut oversized = request;
    oversized.arguments = "x".repeat(16_385);
    assert!(oversized.validate().is_err());
    assert!(CommunicationCommandReply { text: " ".into() }
        .validate()
        .is_err());
    CommunicationCommandReply {
        text: "Plugin is ready.".into(),
    }
    .validate()
    .unwrap();
    assert!(serde_json::from_value::<CommunicationCommandReply>(
        json!({"text":"ready","sender_id":"other"})
    )
    .is_err());
}
