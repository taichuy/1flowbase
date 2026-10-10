use crate::parse_plugin_manifest;
use serde_json::{json, Value};
fn fixture() -> Value {
    let mut value: Value = serde_yaml::from_str(include_str!("managed_manifest.yaml")).unwrap();
    value["binding_targets"] = json!(["system"]);
    value["runtime"]["protocol"] = json!("stdio_json_multiplex_v1");
    value["managed"]["module"]["contributions"] = json!([{
        "contribution_id":"managed_fixture.list", "contributor_module_id":"managed_fixture",
        "point_id":"1flowbase.managed-service.operation", "contract_version":"1", "mode":"append",
        "required_permissions":["service.execute"]
    }]);
    value["managed"]["execution_bindings"] = json!([{
        "contribution_id":"managed_fixture.list", "execution_mode":"process_per_call",
        "runtime":{"protocol":"stdio_json_multiplex_v1","entry":"bin/fixture"}, "handler":"list"
    }]);
    value["managed_service"] = json!({"scope":"system", "feature":{
        "feature_id":"managed_fixture.settings", "label":"Fixture", "description":"Manage fixture",
        "route_id":"fixture", "path":"/settings/fixture"
    }, "operations":[{
        "interface_id":"managed_fixture.list", "contribution_id":"managed_fixture.list",
        "method":"GET", "path":"/api/console/managed-services/managed_fixture/items",
        "summary":"List items", "description":"List shared items.",
        "input_schema":{"type":"object","additionalProperties":false,"properties":{}},
        "output_schema":{"type":"object","additionalProperties":false,"properties":{}}
    }]});
    value["settings_pages"] = json!([{"feature_id":"managed_fixture.settings", "contribution_code":"page", "source_file":"ui/page.tsx", "language":"tsx"}]);
    value
}
fn parse(value: &Value) -> crate::FrameworkResult<crate::PluginManifestV1> {
    parse_plugin_manifest(&serde_yaml::to_string(value).unwrap())
}
#[test]
fn managed_system_service_registers_owned_operation_and_tsx() {
    let parsed = parse(&fixture()).unwrap();
    assert_eq!(
        parsed.managed_service.unwrap().operations[0].interface_id,
        "managed_fixture.list"
    );
    assert_eq!(parsed.settings_pages[0].source_file, "ui/page.tsx");
}
#[test]
fn managed_service_rejects_scope_route_identity_authority_and_protocol_forgery() {
    for (pointer, replacement) in [
        ("/binding_targets", json!(["workspace"])),
        ("/managed_service/scope", json!("workspace")),
        (
            "/managed_service/operations/0/path",
            json!("/api/console/settings/roles"),
        ),
        (
            "/managed_service/operations/0/interface_id",
            json!("core.roles.list"),
        ),
        (
            "/managed/module/contributions/0/required_permissions",
            json!([]),
        ),
        (
            "/managed/execution_bindings/0/runtime/protocol",
            json!("stdio_json"),
        ),
        (
            "/managed_service/operations/0/input_schema",
            json!({"$ref":"https://attacker/schema"}),
        ),
        ("/settings_pages/0/source_file", json!("../secret.tsx")),
    ] {
        let mut value = fixture();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(parse(&value).is_err(), "{pointer} must fail closed");
    }
}
