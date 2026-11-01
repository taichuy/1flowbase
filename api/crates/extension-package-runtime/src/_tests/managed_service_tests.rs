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
            json!("unsupported_protocol"),
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

#[test]
fn managed_mcp_declarations_are_optional_and_have_unique_local_names() {
    let mut value = fixture();
    assert!(
        parse(&value).unwrap().managed_service.unwrap().operations[0]
            .mcp
            .is_none()
    );
    value["managed_service"]["operations"][0]["mcp"] =
        json!({"name":"hosts.list","description":"List configured hosts."});
    assert_eq!(
        parse(&value).unwrap().managed_service.unwrap().operations[0]
            .mcp
            .as_ref()
            .unwrap()
            .name,
        "hosts.list"
    );
    for invalid in [
        json!({"name":"../host","description":"Bad name"}),
        json!({"name":"hosts.list","description":""}),
        json!({"name":"hosts.list","description":"List", "installation_id":"forged"}),
    ] {
        value["managed_service"]["operations"][0]["mcp"] = invalid;
        assert!(parse(&value).is_err());
    }
}

#[test]
fn managed_mcp_rejects_ambiguous_flat_parameters_and_unsafe_route_identity() {
    let mut valid = fixture();
    valid["managed_service"]["operations"][0]["mcp"] =
        json!({"name":"hosts.list","description":"List hosts"});
    valid["managed_service"]["operations"][0]["input_schema"] = json!({"type":"object","properties":{"path":{"type":"object","properties":{"host_id":{"type":"string"}}},"body":{"type":"object","properties":{"command":{"type":"string"}}}}});
    valid["managed_service"]["operations"][0]["output_schema"] =
        json!({"type":"object","properties":{"stdout":{"type":"string"}}});
    assert!(
        parse(&valid).is_ok(),
        "service schemas must not inherit hook summary string limits"
    );
    let mut duplicate = valid.clone();
    duplicate["managed_service"]["operations"][0]["input_schema"]["properties"]["body"]
        ["properties"]["host_id"] = json!({"type":"string"});
    assert!(parse(&duplicate).is_err());
    for (pointer, value) in [
        (
            "/managed_service/operations/0/interface_id",
            json!("managed_fixture.bad id"),
        ),
        (
            "/managed_service/feature/feature_id",
            json!("managed_fixture."),
        ),
        (
            "/managed_service/operations/0/path",
            json!("/api/console/managed-services/managed_fixture/{}"),
        ),
        (
            "/managed_service/operations/0/path",
            json!("/api/console/managed-services/managed_fixture/:host_id"),
        ),
    ] {
        let mut invalid = valid.clone();
        *invalid.pointer_mut(pointer).unwrap() = value;
        assert!(parse(&invalid).is_err(), "{pointer}");
    }
}

#[test]
fn actor_scope_worker_lifetime_is_explicit_and_bounded_by_idle_policy() {
    let service = parse(&fixture()).unwrap().managed_service.unwrap();
    assert_eq!(
        service.worker_lifetime,
        crate::ManagedServiceWorkerLifetime::PerCall
    );
    let mut value = fixture();
    value["managed_service"]["worker_lifetime"] = json!("actor_scope");
    assert_eq!(
        parse(&value)
            .unwrap()
            .managed_service
            .unwrap()
            .worker_lifetime,
        crate::ManagedServiceWorkerLifetime::ActorScope
    );
    value["managed_service"]["worker_idle_timeout_ms"] = json!(1800000);
    assert!(parse(&value).is_ok());
    value["managed_service"]["worker_idle_timeout_ms"] = json!(0);
    assert!(parse(&value).is_err());
    value["managed_service"]["worker_idle_timeout_ms"] = json!(1000);
    value["managed_service"]["worker_lifetime"] = json!("per_call");
    assert!(parse(&value).is_err());
    value["managed_service"]["worker_lifetime"] = json!("actor_scope");
    let mut op = value["managed_service"]["operations"][0].clone();
    op["interface_id"] = json!("managed_fixture.other");
    op["contribution_id"] = json!("managed_fixture.other");
    op["path"] = json!("/api/console/managed-services/managed_fixture/other");
    value["managed_service"]["operations"]
        .as_array_mut()
        .unwrap()
        .push(op);
    let mut contribution = value["managed"]["module"]["contributions"][0].clone();
    contribution["contribution_id"] = json!("managed_fixture.other");
    value["managed"]["module"]["contributions"]
        .as_array_mut()
        .unwrap()
        .push(contribution);
    let mut binding = value["managed"]["execution_bindings"][0].clone();
    binding["contribution_id"] = json!("managed_fixture.other");
    binding["handler"] = json!("other");
    value["managed"]["execution_bindings"]
        .as_array_mut()
        .unwrap()
        .push(binding);
    assert!(parse(&value).is_ok());
    value["managed"]["execution_bindings"][1]["runtime"]["entry"] = json!("bin/other");
    assert!(
        parse(&value).is_err(),
        "shared operations cannot select separate executables"
    );
}

#[test]
fn duplex_operations_require_authorized_multiplex_marker_and_self_contained_schemas() {
    let mut value = fixture();
    value["managed_service"]["operations"][0]["method"] = json!("POST");
    value["managed_service"]["operations"][0]["stream"] =
        json!({"input_schema":{"type":"object"},"event_schema":{"type":"object"}});
    assert!(parse(&value).is_err());
    value["managed"]["execution_bindings"][0]["runtime"]["capabilities"] =
        json!(["runtime_duplex/v1"]);
    let parsed = parse(&value).unwrap();
    assert!(!parsed.managed_service.unwrap().operations[0].invalidates_streams);
    value["managed_service"]["operations"][0]["stream"]["event_schema"] =
        json!({"$ref":"https://attacker.invalid/schema"});
    assert!(parse(&value).is_err());
    value["managed_service"]["operations"][0]["stream"]["event_schema"] = json!({"type":"object"});
    value["managed_service"]["operations"][0]["method"] = json!("GET");
    assert!(parse(&value).is_err());
}

#[test]
fn operation_concurrency_defaults_to_serialized_and_parallel_is_explicit() {
    use crate::ManagedServiceExecutionConcurrency;
    let mut value = fixture();
    value["managed_service"]["operations"][0]["method"] = json!("POST");
    let default = parse(&value)
        .unwrap()
        .managed_service
        .unwrap()
        .operations
        .remove(0);
    assert_eq!(
        default.execution_concurrency,
        ManagedServiceExecutionConcurrency::Serialized
    );
    assert!(default.requires_mutation_lock());
    assert!(!default.invalidates_workers);
    value["managed_service"]["operations"][0]["execution_concurrency"] = json!("parallel");
    value["managed_service"]["operations"][0]["invalidates_workers"] = json!(true);
    let parallel = parse(&value)
        .unwrap()
        .managed_service
        .unwrap()
        .operations
        .remove(0);
    assert!(!parallel.requires_mutation_lock());
    assert!(parallel.invalidates_workers);
    assert_eq!(parallel.method, "POST");
    value["managed_service"]["operations"][0]["execution_concurrency"] = json!("unrestricted");
    assert!(
        parse(&value).is_err(),
        "unknown concurrency must fail closed"
    );
    value["managed_service"]["operations"][0]["execution_concurrency"] = json!("serialized");
    value["managed_service"]["operations"][0]["method"] = json!("GET");
    assert!(
        !parse(&value).unwrap().managed_service.unwrap().operations[0].requires_mutation_lock()
    );
}

#[test]
fn worker_retention_requires_supported_multiplex_actor_scope_capability() {
    let mut value = fixture();
    value["managed_service"]["worker_lifetime"] = json!("actor_scope");
    value["managed"]["execution_bindings"][0]["runtime"]["capabilities"] =
        json!([extension_contracts::RUNTIME_WORKER_RETENTION_V1]);
    assert!(parse(&value).is_ok());
    value["managed"]["execution_bindings"][0]["runtime"]["capabilities"] =
        json!(["runtime_worker_retention/v2"]);
    assert!(parse(&value).is_err());
    value["managed"]["execution_bindings"][0]["runtime"]["capabilities"] =
        json!([extension_contracts::RUNTIME_WORKER_RETENTION_V1]);
    value["managed_service"]["worker_lifetime"] = json!("per_call");
    assert!(parse(&value).is_err());
}

#[test]
fn managed_mcp_media_mapping_roundtrips_and_rejects_invalid_declarations() {
    let mapping = json!({"mcp_content":[{"type":"image","data_path":"/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"});
    let mut value = fixture();
    value["managed_service"]["operations"][0]["mcp"] =
        json!({"name":"image.read","description":"Read image","output_mapping":mapping});
    let parsed = parse(&value).unwrap();
    let mcp = parsed.managed_service.unwrap().operations[0]
        .mcp
        .clone()
        .unwrap();
    assert_eq!(mcp.output_mapping, Some(mapping.clone()));
    assert_eq!(
        serde_json::to_value(&mcp).unwrap()["output_mapping"],
        mapping
    );
    for invalid in [
        json!({}),
        json!({"mcp_content":[],"structured_content_path":"/metadata"}),
        json!({"mcp_content":[{"type":"unknown","data_path":"/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"}),
        json!({"mcp_content":[{"type":"image","data_path":"data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"}),
        json!({"mcp_content":[{"type":"image","data_path":"/bad~2","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"}),
        json!({"mcp_content":[{"type":"image","data_path":"/metadata/data","mime_type_path":"/mime_type"}],"structured_content_path":"/metadata"}),
    ] {
        value["managed_service"]["operations"][0]["mcp"]["output_mapping"] = invalid;
        assert!(parse(&value).is_err());
    }
    value["managed_service"]["operations"][0]["mcp"]
        .as_object_mut()
        .unwrap()
        .remove("output_mapping");
    let parsed = parse(&value).unwrap();
    let mcp = parsed.managed_service.unwrap().operations[0]
        .mcp
        .clone()
        .unwrap();
    assert!(mcp.output_mapping.is_none());
    assert!(serde_json::to_value(mcp)
        .unwrap()
        .get("output_mapping")
        .is_none());
}

#[test]
fn standard_content_declarations_validate_paths_and_resource_read() {
    use crate::ManagedMcpMediaMapping;
    let blocks = json!([
        {"type":"text","text_path":"/text","annotations_path":"/annotations"},
        {"type":"image","data_path":"/image","mime_type_path":"/image_mime"},
        {"type":"audio","data_path":"/audio","mime_type_path":"/audio_mime"},
        {"type":"resource_link","uri_path":"/uri","name_path":"/name","description_path":"/description"},
        {"type":"resource","uri_path":"/uri","text_path":"/text"},
        {"type":"resource","uri_path":"/uri","blob_path":"/blob"}
    ]);
    for block in blocks.as_array().unwrap() {
        let value = json!({"mcp_content":[block],"structured_content_path":"/metadata"});
        let parsed = ManagedMcpMediaMapping::parse(&value).unwrap();
        assert!(!parsed.resource_read);
        assert!(ManagedMcpMediaMapping::parse(&serde_json::to_value(parsed).unwrap()).is_ok());
        let resource_read = json!({"mcp_content":[block],"structured_content_path":"/metadata","resource_read":true});
        assert_eq!(
            ManagedMcpMediaMapping::parse(&resource_read).is_ok(),
            block["type"] == "resource"
        );
    }
    let readable = json!({"structured_content_path":"/metadata","resource_read":true,"mcp_content":[
        {"type":"resource_link","uri_path":"/uri","name_path":"/name"},
        {"type":"resource","uri_path":"/uri","blob_path":"/blob"}
    ]});
    assert!(ManagedMcpMediaMapping::parse(&readable).is_ok());
    let mut mixed = readable.clone();
    mixed["mcp_content"]
        .as_array_mut()
        .unwrap()
        .push(json!({"type":"text","text_path":"/text"}));
    assert!(ManagedMcpMediaMapping::parse(&mixed).is_err());
    for block in [
        json!({"type":"resource","uri_path":"/uri"}),
        json!({"type":"resource","uri_path":"/uri","text_path":"/text","blob_path":"/blob"}),
        json!({"type":"text","text_path":"/metadata/text"}),
        json!({"type":"audio","data_path":"/data","mime_type_path":"/data/mime"}),
        json!({"type":"resource_link","uri_path":"relative","name_path":"/name"}),
        json!({"type":"text","text_path":"/text","annotations_path":"/text"}),
    ] {
        assert!(ManagedMcpMediaMapping::parse(
            &json!({"mcp_content":[block],"structured_content_path":"/metadata"})
        )
        .is_err());
    }
}

#[test]
fn actor_workspace_scope_is_opt_in_and_accepts_shared_retention() {
    let mut value = fixture();
    value["managed_service"]["worker_lifetime"] = json!("actor_workspace_scope");
    value["managed"]["execution_bindings"][0]["runtime"]["capabilities"] =
        json!([extension_contracts::RUNTIME_WORKER_RETENTION_V1]);
    let service = parse(&value).unwrap().managed_service.unwrap();
    assert_eq!(
        service.worker_lifetime,
        crate::ManagedServiceWorkerLifetime::ActorWorkspaceScope
    );
    assert!(service.worker_lifetime.is_shared());
    assert_eq!(
        serde_json::to_value(service).unwrap()["worker_lifetime"],
        "actor_workspace_scope"
    );
    value["managed_service"]["worker_idle_timeout_ms"] = json!(0);
    assert!(parse(&value).is_err());
}

#[test]
fn managed_mcp_instance_is_optional_owned_and_requires_mcp_operation() {
    let mut value = fixture();
    assert!(parse(&value)
        .unwrap()
        .managed_service
        .unwrap()
        .mcp_instance
        .is_none());
    value["managed_service"]["mcp_instance"] = json!({"instance_id":"managed_fixture_tools", "name":"Fixture", "description":"Fixture tools"});
    assert!(
        parse(&value).is_err(),
        "an instance without tools is invalid"
    );
    value["managed_service"]["operations"][0]["mcp"] =
        json!({"name":"hosts.list", "description":"List hosts"});
    let service = parse(&value).unwrap().managed_service.unwrap();
    assert_eq!(
        service.mcp_instance.unwrap().instance_id,
        "managed_fixture_tools"
    );
    assert_eq!(
        service.operations[0].mcp.as_ref().unwrap().name,
        "hosts.list"
    );
    for (field, invalid) in [
        ("instance_id", "other_tools"),
        ("instance_id", "managed_fixture.tools"),
        ("instance_id", "managed_fixture/tools"),
        ("name", "  "),
        ("description", ""),
    ] {
        let mut invalid_value = value.clone();
        invalid_value["managed_service"]["mcp_instance"][field] = json!(invalid);
        assert!(parse(&invalid_value).is_err(), "{field}={invalid}");
    }
}

#[test]
fn managed_mcp_instance_normalizes_hyphenated_plugin_owner() {
    let wire = serde_json::to_string(&fixture())
        .unwrap()
        .replace("managed_fixture", "managed-fixture");
    let mut value: Value = serde_json::from_str(&wire).unwrap();
    value["managed_service"]["mcp_instance"] = json!({"instance_id":"managed_fixture_tools", "name":"Fixture", "description":"Fixture tools"});
    value["managed_service"]["operations"][0]["mcp"] =
        json!({"name":"hosts.list", "description":"List hosts"});
    assert!(parse(&value).is_ok());
    value["managed_service"]["mcp_instance"]["instance_id"] = json!("managed-fixture_tools");
    assert!(parse(&value).is_err());
}

#[test]
fn unary_managed_service_uses_existing_per_call_carrier_only() {
    let mut value = fixture();
    value["runtime"]["protocol"] = json!("stdio_json");
    value["managed"]["execution_bindings"][0]["runtime"]["protocol"] = json!("stdio_json");
    assert!(
        parse(&value).is_ok(),
        "synchronous unary operations use the existing carrier"
    );
    value["managed_service"]["worker_lifetime"] = json!("actor_workspace_scope");
    assert!(
        parse(&value).is_err(),
        "unary carrier must not imply a resident worker"
    );
    value["managed_service"]
        .as_object_mut()
        .unwrap()
        .remove("worker_lifetime");
    value["managed_service"]["operations"][0]["stream"] =
        json!({"input_schema":{"type":"object"},"event_schema":{"type":"object"}});
    assert!(
        parse(&value).is_err(),
        "stream transport still requires multiplex"
    );
}
