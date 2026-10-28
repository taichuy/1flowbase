use crate::{parse_plugin_manifest, ManagedContributionPayload};
use serde_json::{json, Value};

#[path = "notification/command_manifest_tests.rs"]
mod command_manifest_tests;

fn fixture() -> Value {
    let mut value: Value = serde_yaml::from_str(include_str!("managed_manifest.yaml")).unwrap();
    value["notification_types"] = json!([{
        "type_code":"acme/managed_fixture/webhook", "display_name":"Webhook", "description":"Test notification provider",
        "contribution_id":"managed_fixture.send",
        "config_schema":{"type":"object","properties":{"token":{"type":"string"}},"required":["token"],"additionalProperties":false},
        "config_form":{"schema_version":"1.0.0","fields":[{"key":"token","label":"Token","type":"string","control":"password"}]},
        "input_schema":{"type":"object","properties":{"text":{"type":"string"}},"required":["text"],"additionalProperties":false},
        "input_form":{"schema_version":"1.0.0","fields":[{"key":"text","label":"Text","type":"string"}]},
        "output_schema":{"type":"object","properties":{"accepted":{"type":"boolean"}},"required":["accepted"],"additionalProperties":false}
    }]);
    value["managed"]["module"]["contributions"].as_array_mut().unwrap().push(json!({
        "contribution_id":"managed_fixture.send", "contributor_module_id":"managed_fixture", "point_id":"1flowbase.notification.send",
        "contract_version":"1flowbase.notification/v1", "required_permissions":["notification.send"], "mode":"append"
    }));
    value["managed"]["execution_bindings"].as_array_mut().unwrap().push(json!({
        "contribution_id":"managed_fixture.send", "execution_mode":"process_per_call", "runtime":{"protocol":"stdio_json","entry":"bin/fixture"},
        "handler":"send", "payload":{"kind":"notification_type","type_code":"acme/managed_fixture/webhook"}
    }));
    value
}

fn parse(value: &Value) -> crate::FrameworkResult<crate::PluginManifestV1> {
    parse_plugin_manifest(&serde_yaml::to_string(value).unwrap())
}

#[test]
fn notification_sdk_package_fixture_is_a_valid_managed_manifest() {
    let manifest = parse_plugin_manifest(include_str!(
        "../../../../plugins/fixtures/acme.notification/manifest.yaml"
    ))
    .expect("the documented SDK fixture must parse without rewriting its identity");
    assert_eq!(manifest.plugin_code().unwrap(), "notification-fixture");
    assert_eq!(
        manifest.notification_types[0].type_code,
        "acme/notification-fixture/receipt"
    );
    let module = &manifest.managed.as_ref().unwrap().module;
    assert_eq!(module.module_id.as_str(), manifest.plugin_code().unwrap());
    assert_eq!(
        module.contributions[0].contributor_module_id,
        module.module_id
    );
}

#[test]
fn notification_manifest_registers_explicit_config_and_send_contract() {
    let manifest = parse(&fixture()).unwrap();
    assert_eq!(manifest.notification_types.len(), 1);
    let descriptor = &manifest.notification_types[0];
    assert_eq!(descriptor.config_form.fields[0].key, "token");
    assert_eq!(descriptor.input_form.fields[0].key, "text");
    assert_eq!(descriptor.contribution_id.as_str(), "managed_fixture.send");
    assert!(matches!(
        manifest.managed.unwrap().execution_bindings[2].payload,
        Some(ManagedContributionPayload::NotificationType { .. })
    ));
    // The old fixture needs neither notification metadata nor a new binding.
    assert!(parse_plugin_manifest(include_str!("managed_manifest.yaml"))
        .unwrap()
        .notification_types
        .is_empty());
}

#[test]
fn notification_manifest_rejects_missing_duplicate_and_incompatible_bindings() {
    let original = fixture();
    let mut invalid = Vec::new();
    for (pointer, replacement) in [
        ("/notification_types/0/type_code", json!("system")),
        (
            "/notification_types/0/contribution_id",
            json!("managed_fixture.compute"),
        ),
        (
            "/notification_types/0/config_schema",
            json!({"type":"invalid"}),
        ),
        (
            "/notification_types/0/input_schema",
            json!({"$ref":"https://example.com/schema"}),
        ),
        (
            "/notification_types/0/output_schema",
            json!({"type":"integer","minimum":"zero"}),
        ),
        (
            "/managed/execution_bindings/2/payload/type_code",
            json!("acme/managed_fixture/missing"),
        ),
        ("/managed/execution_bindings/2/payload", Value::Null),
        (
            "/managed/execution_bindings/2/execution_mode",
            json!("declarative_only"),
        ),
        (
            "/managed/module/contributions/2/point_id",
            json!("acme.other"),
        ),
        (
            "/managed/module/contributions/2/contract_version",
            json!("2"),
        ),
        (
            "/managed/module/contributions/2/required_permissions",
            json!([]),
        ),
        ("/binding_targets", json!(["system"])),
    ] {
        let mut value = original.clone();
        *value.pointer_mut(pointer).unwrap() = replacement;
        invalid.push((pointer.to_owned(), value));
    }
    let mut duplicate = original.clone();
    duplicate["notification_types"]
        .as_array_mut()
        .unwrap()
        .push(original["notification_types"][0].clone());
    invalid.push(("duplicate type".into(), duplicate));
    let mut missing = original.clone();
    missing["notification_types"][0]
        .as_object_mut()
        .unwrap()
        .remove("type_code");
    invalid.push(("missing type".into(), missing));
    let mut missing_form = original.clone();
    missing_form["notification_types"][0]
        .as_object_mut()
        .unwrap()
        .remove("config_form");
    invalid.push(("missing config form".into(), missing_form));
    let mut unbound = original;
    unbound["managed"]["execution_bindings"]
        .as_array_mut()
        .unwrap()
        .pop();
    invalid.push(("unbound type".into(), unbound));
    for (name, value) in invalid {
        assert!(parse(&value).is_err(), "{name} must fail closed");
    }
}

#[test]
fn notification_manifest_reuses_multiplex_dispatcher_without_service_actor_pool() {
    let mut value = fixture();
    value["runtime"]["protocol"] = json!(extension_contracts::STDIO_JSON_MULTIPLEX_V1);
    value["managed"]["execution_bindings"][2]["runtime"]["protocol"] =
        json!(extension_contracts::STDIO_JSON_MULTIPLEX_V1);
    assert!(parse(&value).is_ok());
    value["managed"]["execution_bindings"][2]["runtime"]["capabilities"] =
        json!([extension_contracts::RUNTIME_WORKER_RETENTION_V1]);
    assert!(
        parse(&value).is_err(),
        "notification binding must not self-award service worker retention"
    );
}

fn receive_fixture() -> Value {
    let mut value = fixture();
    value["notification_types"][0]["receive"] = json!({
        "contribution_id":"managed_fixture.receive", "default_system_prompt":"Use tools for channel history and files.",
        "mcp_tools":[
            {"name":"messages","description":"Channel messages","operation":"history_messages","input_schema":{"type":"object"}},
            {"name":"files","description":"Channel files","operation":"history_files","input_schema":{"type":"object"}},
            {"name":"read_file","description":"File contents","operation":"read_file","input_schema":{"type":"object"}}
        ]
    });
    value["managed"]["module"]["contributions"].as_array_mut().unwrap().push(json!({
        "contribution_id":"managed_fixture.receive", "contributor_module_id":"managed_fixture", "point_id":"1flowbase.notification.receive",
        "contract_version":"1flowbase.notification/v1", "required_permissions":["notification.receive"], "mode":"append"
    }));
    value["managed"]["execution_bindings"].as_array_mut().unwrap().push(json!({
        "contribution_id":"managed_fixture.receive", "execution_mode":"process_per_call", "runtime":{"protocol":"stdio_json","entry":"bin/fixture"},
        "handler":"receive", "payload":{"kind":"notification_receive","type_code":"acme/managed_fixture/webhook"}
    }));
    value
}

#[test]
fn communications_manifest_binds_receive_independently_of_send() {
    let value = receive_fixture();
    let manifest = parse(&value).unwrap();
    let receive = manifest.notification_types[0].receive.as_ref().unwrap();
    assert_eq!(receive.contribution_id.as_str(), "managed_fixture.receive");
    assert_eq!(receive.mcp_tools.len(), 3);
    assert!(matches!(
        manifest.managed.unwrap().execution_bindings[3].payload,
        Some(ManagedContributionPayload::NotificationReceive { .. })
    ));
    for (pointer, replacement) in [
        (
            "/notification_types/0/receive/contribution_id",
            json!("managed_fixture.send"),
        ),
        (
            "/notification_types/0/receive/default_system_prompt",
            json!(""),
        ),
        ("/notification_types/0/receive/mcp_tools", json!([])),
        (
            "/managed/module/contributions/3/point_id",
            json!("1flowbase.notification.send"),
        ),
        (
            "/managed/module/contributions/3/required_permissions",
            json!(["notification.send"]),
        ),
        (
            "/managed/execution_bindings/3/payload/kind",
            json!("notification_type"),
        ),
        (
            "/managed/execution_bindings/3/runtime/capabilities",
            json!([extension_contracts::RUNTIME_WORKER_RETENTION_V1]),
        ),
    ] {
        let mut invalid = value.clone();
        if let Some(slot) = invalid.pointer_mut(pointer) {
            *slot = replacement;
        } else {
            invalid["managed"]["execution_bindings"][3]["runtime"]["capabilities"] = replacement;
        }
        assert!(parse(&invalid).is_err(), "{pointer}");
    }
    let mut unbound = value;
    unbound["managed"]["execution_bindings"]
        .as_array_mut()
        .unwrap()
        .pop();
    assert!(parse(&unbound).is_err());
}
