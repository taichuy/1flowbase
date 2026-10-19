use crate::{parse_plugin_manifest, ManagedContributionPayload};
use serde_json::{json, Value};

fn fixture() -> Value {
    let mut value: Value = serde_yaml::from_str(include_str!("managed_manifest.yaml")).unwrap();
    value["binding_targets"] = json!(["system"]);
    value["runtime"]["protocol"] = json!(extension_contracts::STDIO_JSON_MULTIPLEX_V1);
    value["authentication_providers"] = json!([{
        "auth_type":"github", "display_name":"GitHub", "config_schema":[{"key":"client_id","label":"Client ID","type":"string"},{"key":"client_secret","label":"Client secret","type":"string","control":"password"}],
        "default_public_ui_block":"github-login", "public_variable_keys":["client_id"], "contribution_id":"managed_fixture.auth"
    }]);
    value["managed"]["module"]["contributions"] = json!([{
        "contribution_id":"managed_fixture.auth", "contributor_module_id":"managed_fixture", "point_id":"1flowbase.authentication.provider",
        "contract_version":"1", "required_permissions":["authentication.verify", "network.http.request"], "mode":"append"
    }]);
    value["managed"]["execution_bindings"] = json!([{
        "contribution_id":"managed_fixture.auth", "execution_mode":"process_per_call", "runtime":{"protocol":"stdio_json_multiplex_v1","entry":"bin/fixture"},
        "handler":"authenticate", "payload":{"kind":"authentication_provider","auth_type":"github"}
    }]);
    value
}

fn parse(value: &Value) -> crate::FrameworkResult<crate::PluginManifestV1> {
    parse_plugin_manifest(&serde_yaml::to_string(value).unwrap())
}

#[test]
fn authentication_is_a_system_managed_contribution_without_managed_service() {
    let manifest = parse(&fixture()).unwrap();
    assert!(manifest.managed_service.is_none());
    assert_eq!(manifest.authentication_providers[0].auth_type, "github");
    assert!(
        matches!(&manifest.managed.unwrap().execution_bindings[0].payload,
        Some(ManagedContributionPayload::AuthenticationProvider { auth_type }) if auth_type == "github")
    );
    assert!(parse_plugin_manifest(include_str!("managed_manifest.yaml"))
        .unwrap()
        .authentication_providers
        .is_empty());
}

#[test]
fn authentication_rejects_descriptor_binding_scope_and_host_authority_forgery() {
    for (pointer, replacement) in [
        ("/manifest_version", json!(1)),
        (
            "/authentication_providers/0/auth_type",
            json!("password-local"),
        ),
        (
            "/authentication_providers/0/public_variable_keys",
            json!(["client_secret"]),
        ),
        (
            "/authentication_providers/0/public_variable_keys",
            json!(["password"]),
        ),
        (
            "/authentication_providers/0/contribution_id",
            json!("missing.auth"),
        ),
        (
            "/managed/execution_bindings/0/payload/auth_type",
            json!("other"),
        ),
        ("/managed/execution_bindings/0/payload", Value::Null),
        (
            "/managed/execution_bindings/0/execution_mode",
            json!("stateful_runtime_worker"),
        ),
        (
            "/managed/execution_bindings/0/runtime/protocol",
            json!("stdio_json"),
        ),
        (
            "/managed/module/contributions/0/point_id",
            json!("acme.authentication"),
        ),
        (
            "/managed/module/contributions/0/contract_version",
            json!("2"),
        ),
        (
            "/managed/module/contributions/0/required_permissions",
            json!(["network.http.request"]),
        ),
        ("/managed/module/contributions/0/mode", json!("override")),
        ("/binding_targets", json!(["workspace"])),
        ("/binding_targets", json!(["system", "workspace"])),
        ("/runtime/protocol", json!("stdio_json")),
    ] {
        let mut value = fixture();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(parse(&value).is_err(), "{pointer} must fail closed");
    }
    let mut duplicate = fixture();
    let descriptor = duplicate["authentication_providers"][0].clone();
    duplicate["authentication_providers"]
        .as_array_mut()
        .unwrap()
        .push(descriptor);
    assert!(parse(&duplicate).is_err());
    let mut forged_point = fixture();
    forged_point["managed"]["module"]["extension_points"] = json!([{
        "point_id":"1flowbase.authentication.provider", "owner_module_id":"managed_fixture", "point_kind":"resource_action",
        "contract":{"contract_id":"authentication","contract_version":"1"}, "scope":"system", "cardinality":"many", "ordering":"lexicographic",
        "failure":"fail_closed", "delivery":"synchronous", "lifecycle":"invocation", "allowed_permissions":["authentication.verify"], "override_policy":"sealed"
    }]);
    serde_json::from_value::<extension_contracts::ExtensionPointDescriptor>(
        forged_point["managed"]["module"]["extension_points"][0].clone(),
    )
    .expect("forged point must be a structurally valid descriptor so authority rejects it");
    assert!(parse(&forged_point).is_err());
}
