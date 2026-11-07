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

#[test]
fn backend_service_can_register_without_a_settings_feature_or_page() {
    let mut value = fixture();
    value["managed_service"]
        .as_object_mut()
        .unwrap()
        .remove("feature");
    value.as_object_mut().unwrap().remove("settings_pages");
    let parsed = parse(&value).unwrap();
    let service = parsed.managed_service.unwrap();
    assert!(service.feature.is_none());
    assert_eq!(service.operations[0].interface_id, "managed_fixture.list");
    assert!(parsed.settings_pages.is_empty());
}

#[test]
fn settings_page_cannot_register_without_its_feature() {
    let mut value = fixture();
    value["managed_service"]
        .as_object_mut()
        .unwrap()
        .remove("feature");
    assert!(parse(&value).is_err());
}

#[test]
fn resource_link_only_replay_and_discovery_manifest_contract() {
    use crate::ManagedMcpMediaMapping;
    let value = json!({
        "structured_content_path":"/metadata", "resource_read":true,
        "resource_read_arguments":{"link_only":false},
        "resources":[{"name":"Report","uri":"urn:report:current","arguments":{"id":"current"},"mime_type":"application/pdf"}],
        "mcp_content":[{"type":"resource_link","uri_path":"/uri","name_path":"/name"},
            {"type":"resource","uri_path":"/uri","blob_path":"/blob","skip_if_null":true}]
    });
    let parsed = ManagedMcpMediaMapping::parse(&value).unwrap();
    assert_eq!(parsed.resource_read_arguments.unwrap()["link_only"], false);
    for (field, invalid) in [
        ("resource_read_arguments", json!([])),
        (
            "resources",
            json!([{"name":"x","uri":"urn:x","arguments":[]}]),
        ),
        (
            "resources",
            json!([{"name":"","uri":"urn:x","arguments":{}}]),
        ),
        (
            "resources",
            json!([{"name":"x","uri":"urn:x","arguments":{},"secret":"no"}]),
        ),
        ("resource_read", json!(false)),
    ] {
        let mut invalid_value = value.clone();
        invalid_value[field] = invalid;
        assert!(
            ManagedMcpMediaMapping::parse(&invalid_value).is_err(),
            "{field}"
        );
    }
    let mut invalid = value;
    invalid["mcp_content"][1]["skip_if_null"] = json!("true");
    assert!(ManagedMcpMediaMapping::parse(&invalid).is_err());
}

#[test]
fn packaged_mcp_template_is_hash_bound_optional_and_reset_is_explicit() {
    let mut value = fixture();
    value["managed_service"]["mcp_instance"] = json!({
        "instance_id":"managed_fixture_tools", "name":"Fixture", "description":"Native tools",
        "template":{"path":"_assets/mcp/manifest.json", "sha256":format!("sha256:{}", "a".repeat(64))}
    });
    let parsed = parse(&value).unwrap();
    let instance = parsed.managed_service.unwrap().mcp_instance.unwrap();
    assert!(!instance.reset_on_activation);
    assert_eq!(instance.template.unwrap().path, "_assets/mcp/manifest.json");
    assert!(
        value["managed_service"]["operations"][0]
            .get("mcp")
            .is_none(),
        "native templates may explicitly wrap operations without a flat MCP declaration"
    );
    value["managed_service"]["mcp_instance"]["reset_on_activation"] = json!(true);
    assert!(
        parse(&value)
            .unwrap()
            .managed_service
            .unwrap()
            .mcp_instance
            .unwrap()
            .reset_on_activation
    );
    for path in [
        "",
        "/manifest.json",
        "../manifest.json",
        "_assets/../manifest.json",
        "_assets//manifest.json",
        "./manifest.json",
        "_assets\\manifest.json",
        "C:/manifest.json",
    ] {
        let mut invalid = value.clone();
        invalid["managed_service"]["mcp_instance"]["template"]["path"] = json!(path);
        assert!(parse(&invalid).is_err(), "unsafe template path {path}");
    }
    for hash in [
        "a".repeat(64),
        "sha256:bad".into(),
        format!("sha256:{}", "A".repeat(64)),
    ] {
        let mut invalid = value.clone();
        invalid["managed_service"]["mcp_instance"]["template"]["sha256"] = json!(hash);
        assert!(parse(&invalid).is_err(), "invalid digest {hash}");
    }
    value["managed_service"]["operations"][0]["mcp"] = json!({"name":"list", "description":"List"});
    value["managed_service"]["mcp_instance"]
        .as_object_mut()
        .unwrap()
        .remove("template");
    assert!(parse(&value).is_err(), "reset requires a packaged default");
    value["managed_service"]["mcp_instance"]["reset_on_activation"] = json!(false);
    let legacy = parse(&value)
        .unwrap()
        .managed_service
        .unwrap()
        .mcp_instance
        .unwrap();
    assert!(legacy.template.is_none());
    assert!(!legacy.reset_on_activation);
}

// AC-006/007: declaration ownership, schema selectors and operation mapping are Host-validated.
fn record_fixture() -> Value {
    let mut value = fixture();
    value["permissions"]["storage"] = json!("host_managed");
    value["data_models"] = json!([{"contribution_version":"1flowbase.plugin-data-model/v1", "storage_binding":"main",
        "owned_collections":[{"collection_code":"hosts", "fields":[{"field_code":"host_id", "field_type":"string", "nullable":false}]}]}]);
    value["managed"]["module"]["contributions"].as_array_mut().unwrap().push(json!({
        "contribution_id":"managed_fixture.data", "contributor_module_id":"managed_fixture", "point_id":"acme.data", "contract_version":"1", "mode":"append"
    }));
    value["managed"]["execution_bindings"].as_array_mut().unwrap().push(json!({
        "contribution_id":"managed_fixture.data", "execution_mode":"declarative_only",
        "runtime":{"protocol":"stdio_json", "entry":"ui/data.json"}, "handler":"data", "payload":{"kind":"data_models"}
    }));
    value["managed_service"]["record_resources"] = json!([{
        "resource_code":"hosts", "collection_code":"hosts", "id_field":"host_id", "label":"Hosts", "description":"Host row permissions",
        "actions":[{"action_code":"view", "label":"View", "description":"View a host", "operation_ids":["managed_fixture.list"]}]
    }]);
    value["managed_service"]["operations"][0]["record_permission"] =
        json!({"resource_code":"hosts", "action_code":"view", "selector":{"kind":"list"}});
    value
}

#[test]
fn record_resources_validate_owned_collection_and_leave_legacy_packages_unchanged() {
    assert!(parse(&fixture())
        .unwrap()
        .managed_service
        .unwrap()
        .record_resources
        .is_empty());
    let parsed = parse(&record_fixture()).unwrap();
    assert_eq!(
        parsed.managed_service.unwrap().record_resources[0].id_field,
        "host_id"
    );
    let mut builtin = record_fixture();
    builtin["managed_service"]["record_resources"][0]["id_field"] = json!("id");
    assert!(
        parse(&builtin).is_ok(),
        "Host-owned UUID id is also a valid owned collection identity"
    );
}

#[test]
fn record_resources_reject_ownership_action_reference_and_selector_schema_forgery() {
    for (pointer, replacement) in [
        (
            "/managed_service/record_resources/0/collection_code",
            json!("other_plugin_hosts"),
        ),
        (
            "/managed_service/record_resources/0/id_field",
            json!("missing_id"),
        ),
        (
            "/data_models/0/owned_collections/0/fields/0/nullable",
            json!(true),
        ),
        (
            "/data_models/0/owned_collections/0/fields/0/field_type",
            json!("json"),
        ),
        (
            "/managed_service/record_resources/0/actions/0/operation_ids",
            json!(["other_plugin.view"]),
        ),
        (
            "/managed_service/operations/0/record_permission/resource_code",
            json!("other_resource"),
        ),
        (
            "/managed_service/operations/0/record_permission/action_code",
            json!("delete"),
        ),
        (
            "/managed_service/operations/0/record_permission/selector",
            json!({"kind":"record", "pointer":"/body/host_id"}),
        ),
        (
            "/managed_service/operations/0/record_permission/selector",
            json!({"kind":"handle", "pointer":"/path/session_id"}),
        ),
    ] {
        let mut value = record_fixture();
        *value.pointer_mut(pointer).unwrap() = replacement;
        assert!(parse(&value).is_err(), "{pointer} must fail closed");
    }
    let mut duplicate = record_fixture();
    let resource = duplicate["managed_service"]["record_resources"][0].clone();
    duplicate["managed_service"]["record_resources"]
        .as_array_mut()
        .unwrap()
        .push(resource);
    assert!(parse(&duplicate).is_err());
}

fn open_record_fixture() -> Value {
    let mut value = record_fixture();
    value["managed_service"]["record_resources"][0]["actions"][0]["action_code"] = json!("open");
    value["managed_service"]["operations"][0]["method"] = json!("POST");
    value["managed_service"]["operations"][0]["input_schema"] = json!({
        "type":"object", "required":["body"], "properties":{"body":{"type":"object", "required":["host_id"], "properties":{"host_id":{"type":"string"}}}}
    });
    value["managed_service"]["operations"][0]["output_schema"] = json!({
        "type":"object", "required":["session"], "properties":{"session":{"type":"object", "required":["session_id"], "properties":{"session_id":{"type":"string"}}}}
    });
    value["managed_service"]["operations"][0]["record_permission"] = json!({"resource_code":"hosts", "action_code":"open", "selector":{"kind":"record", "pointer":"/body/host_id"}, "response_handle_pointer":"/session/session_id"});
    value
}

#[test]
fn record_and_host_handle_selectors_validate_requiredness_and_open_response_binding() {
    assert!(parse(&open_record_fixture()).is_ok());
    for (pointer, replacement) in [
        ("/managed_service/operations/0/input_schema/properties/body/required", json!([])),
        ("/managed_service/operations/0/record_permission/selector/pointer", json!("/body/host_id~2")),
        ("/managed_service/operations/0/output_schema/properties/session/properties/session_id/type", json!("number")),
        ("/managed_service/operations/0/record_permission/response_handle_pointer", json!("/missing/session_id")),
        ("/managed_service/operations/0/record_permission/selector/allow_missing", json!(true)),
    ] {
        let mut value = open_record_fixture();
        if let Some(target) = value.pointer_mut(pointer) { *target = replacement; }
        else { value["managed_service"]["operations"][0]["record_permission"]["selector"]["allow_missing"] = replacement; }
        assert!(parse(&value).is_err(), "{pointer} must fail closed");
    }
    let mut handle = open_record_fixture();
    handle["managed_service"]["operations"][0]["record_permission"] = json!({"resource_code":"hosts", "action_code":"open", "selector":{"kind":"handle", "pointer":"/body/host_id"}});
    assert!(parse(&handle).is_ok());
    let mut draft = open_record_fixture();
    draft["managed_service"]["operations"][0]["input_schema"]["properties"]["body"]["required"] =
        json!([]);
    draft["managed_service"]["operations"][0]["record_permission"] = json!({"resource_code":"hosts", "action_code":"open", "selector":{"kind":"record", "pointer":"/body/host_id", "allow_missing":true}});
    assert!(
        parse(&draft).is_ok(),
        "explicit optional record permits draft without weakening normal records"
    );
}
