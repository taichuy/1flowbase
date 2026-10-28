use super::*;

fn command_fixture() -> Value {
    // A command plugin does not need to implement notification receive/send.
    let mut value: Value = serde_yaml::from_str(include_str!("../managed_manifest.yaml")).unwrap();
    value["communication_commands"] = json!([{
        "command_id":"acme/managed_fixture/status","name":"status","description":"Show status",
        "usage":"/status [details]","contribution_id":"managed_fixture.command.status","default_enabled":true
    }]);
    value["managed"]["module"]["contributions"].as_array_mut().unwrap().push(json!({
        "contribution_id":"managed_fixture.command.status","contributor_module_id":"managed_fixture",
        "point_id":extension_contracts::COMMUNICATION_COMMAND_POINT,
        "contract_version":extension_contracts::COMMUNICATION_COMMAND_CONTRACT_VERSION,
        "required_permissions":["event.subscribe"],"mode":"append"
    }));
    value["managed"]["execution_bindings"]
        .as_array_mut()
        .unwrap()
        .push(json!({
            "contribution_id":"managed_fixture.command.status","execution_mode":"process_per_call",
            "runtime":{"protocol":"stdio_json","entry":"bin/fixture"},"handler":"status"
        }));
    value
}

#[test]
fn communication_command_manifest_binds_plugin_event_without_channel_implementation() {
    let manifest = parse(&command_fixture()).unwrap();
    assert!(manifest.notification_types.is_empty());
    assert_eq!(manifest.communication_commands[0].name, "status");
    assert!(
        parse_plugin_manifest(include_str!("../managed_manifest.yaml"))
            .unwrap()
            .communication_commands
            .is_empty()
    );
}

#[test]
fn communication_command_manifest_rejects_unbound_or_wrong_authority_declarations() {
    let original = command_fixture();
    for (pointer, replacement) in [
        (
            "/communication_commands/0/command_id",
            json!("another/managed_fixture/status"),
        ),
        (
            "/communication_commands/0/contribution_id",
            json!("missing.handler"),
        ),
        (
            "/managed/module/contributions/2/point_id",
            json!("1flowbase.notification.send"),
        ),
        (
            "/managed/module/contributions/2/required_permissions",
            json!([]),
        ),
        (
            "/managed/module/contributions/2/contract_version",
            json!("invalid"),
        ),
        ("/binding_targets", json!(["system"])),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(parse(&value).is_err(), "{pointer}");
    }
    let mut duplicate = original.clone();
    duplicate["communication_commands"]
        .as_array_mut()
        .unwrap()
        .push(original["communication_commands"][0].clone());
    assert!(parse(&duplicate).is_err());
    let mut reserved = original;
    reserved["communication_commands"][0]["name"] = json!("new");
    reserved["communication_commands"][0]["usage"] = json!("/new");
    assert!(parse(&reserved).is_err());
}
