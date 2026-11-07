use serde_json::{json, Value};
pub(super) const CODE: &str = "record_fixture";
pub(super) const BASE: &str = "/api/console/managed-services/record_fixture";
fn object(properties: Value, required: Vec<&str>) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn operation(
    handler: &str,
    method: &str,
    path: &str,
    action: Option<&str>,
    selector: Value,
) -> Value {
    let path_properties = if path.contains("{record_id}") {
        json!({"record_id":{"type":"string"}})
    } else if path.contains("{session_id}") {
        json!({"session_id":{"type":"string"}})
    } else {
        json!({})
    };
    let required = if path.contains("{record_id}") {
        vec!["record_id"]
    } else if path.contains("{session_id}") {
        vec!["session_id"]
    } else {
        vec![]
    };
    let body = match handler {
        "create" | "update" => object(json!({"name":{"type":"string"}}), vec!["name"]),
        "command" => object(json!({"record_id":{"type":"string"}}), vec![]),
        _ => object(json!({}), vec![]),
    };
    let query = if handler == "list" {
        object(
            json!({"limit":{"type":"integer","minimum":1,"maximum":100,"default":100},"offset":{"type":"integer","minimum":0,"default":0}}),
            vec![],
        )
    } else {
        object(json!({}), vec![])
    };
    let output = match handler {
        "open" => object(json!({"session_id":{"type":"string"}}), vec!["session_id"]),
        _ => json!({"type":"object"}),
    };
    let mut op = json!({"interface_id":format!("{CODE}.{handler}"),"contribution_id":format!("{CODE}.{handler}"),"method":method,"path":format!("{BASE}{path}"),"summary":format!("Run fixture {handler}"),"description":format!("Execute the synthetic record permission {handler} fixture through the Host."),"input_schema":object(json!({"path":object(path_properties,required),"query":query,"body":body}),vec!["path","query","body"]),"output_schema":output,"execution_concurrency":"parallel"});
    if let Some(action) = action {
        op["record_permission"] =
            json!({"resource_code":"records","action_code":action,"selector":selector});
    }
    if handler == "open" {
        op["record_permission"]["response_handle_pointer"] = json!("/session_id");
    }
    if handler == "stream" {
        op["stream"] = json!({"input_schema":object(json!({"data":{"type":"string"}}),vec!["data"]),"event_schema":object(json!({"ready":{"type":"boolean"},"record_id":{"type":"string"}}),vec!["ready","record_id"])});
    } else if handler != "unmapped" {
        op["mcp"] = json!({"name":handler,"description":format!("Run fixture {handler}")});
    }
    if handler == "resource" {
        op["mcp"]["output_mapping"] = json!({"mcp_content":[{"type":"resource","uri_path":"/resource/uri","mime_type_path":"/resource/mime_type","text_path":"/resource/text"}],"structured_content_path":"/metadata","resource_read":true});
    }
    op
}
pub(super) fn manifest() -> Value {
    let raw = std::fs::read_to_string(
        crate::api_workspace_root()
            .unwrap()
            .join("crates/extension-package-runtime/src/_tests/managed_manifest.yaml"),
    )
    .unwrap();
    let mut manifest: Value = serde_yaml::from_str(&raw).unwrap();
    manifest["plugin_id"] = json!(CODE);
    manifest["publisher_namespace"] = json!("fixture");
    manifest["display_name"] = json!("Synthetic record permissions");
    manifest["description"] = json!("Public Host ACL integration fixture");
    manifest["binding_targets"] = json!(["system"]);
    manifest["permissions"]["storage"] = json!("host_managed");
    let runtime = json!({"protocol":"stdio_json_multiplex_v1","entry":"bin/record_permission_worker","capabilities":["runtime_duplex/v1","runtime_worker_retention/v1","runtime_host_call/v1"],"limits":{"timeout_ms":30000}});
    manifest["runtime"] = runtime.clone();
    // Package runtime capabilities belong to providers; managed bindings advertise their own.
    manifest["runtime"]["capabilities"] = json!([]);
    let record = json!({"kind":"record","pointer":"/path/record_id"});
    let operations = vec![
        operation(
            "list",
            "GET",
            "/records",
            Some("view"),
            json!({"kind":"list"}),
        ),
        operation(
            "get",
            "GET",
            "/records/{record_id}",
            Some("view"),
            record.clone(),
        ),
        operation(
            "create",
            "POST",
            "/records",
            Some("create"),
            json!({"kind":"create"}),
        ),
        operation(
            "update",
            "PUT",
            "/records/{record_id}",
            Some("update"),
            record.clone(),
        ),
        operation(
            "delete",
            "DELETE",
            "/records/{record_id}",
            Some("delete"),
            record.clone(),
        ),
        operation(
            "open",
            "POST",
            "/records/{record_id}/open",
            Some("open"),
            record.clone(),
        ),
        operation(
            "command",
            "POST",
            "/sessions/{session_id}/command",
            Some("open"),
            json!({"kind":"handle","pointer":"/path/session_id"}),
        ),
        operation(
            "resource",
            "GET",
            "/records/{record_id}/resource",
            Some("open"),
            record.clone(),
        ),
        operation(
            "stream",
            "POST",
            "/records/{record_id}/stream",
            Some("open"),
            record,
        ),
        operation("unmapped", "POST", "/unmapped", None, Value::Null),
    ];
    let actions=[("view",vec!["list","get"]),("update",vec!["update"]),("open",vec!["open","command","resource","stream"]),("delete",vec!["delete"])].into_iter().map(|(action,handlers)|json!({"action_code":action,"label":action,"description":format!("Fixture {action} capability"),"operation_ids":handlers.iter().map(|handler|format!("{CODE}.{handler}")).collect::<Vec<_>>()})).collect::<Vec<_>>();
    manifest["data_models"] = json!([{"contribution_version":"1flowbase.plugin-data-model/v1","storage_binding":"main","owned_collections":[{"collection_code":"records","fields":[{"field_code":"name","field_type":"text","nullable":false}]}]}]);
    manifest["managed_service"] = json!({"scope":"system","worker_lifetime":"actor_workspace_scope","worker_idle_timeout_ms":120000,"record_resources":[{"resource_code":"records","collection_code":"records","id_field":"id","label":"Records","description":"Synthetic records","actions":actions}],"operations":operations});
    let handlers = manifest["managed_service"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|op| {
            op["interface_id"]
                .as_str()
                .unwrap()
                .strip_prefix(&format!("{CODE}."))
                .unwrap()
                .to_owned()
        })
        .collect::<Vec<_>>();
    manifest["managed"] = json!({"module":{"bus_version":"v1","module_id":CODE,"module_version":"0.1.0","module_kind":"runtime","contributions":handlers.iter().map(|handler|json!({"contribution_id":format!("{CODE}.{handler}"),"contributor_module_id":CODE,"point_id":"1flowbase.managed-service.operation","contract_version":"1","mode":"append","required_permissions":["service.execute","plugin_data.owned.write"]})).collect::<Vec<_>>()},"execution_bindings":handlers.iter().map(|handler|json!({"contribution_id":format!("{CODE}.{handler}"),"execution_mode":"process_per_call","runtime":runtime,"handler":handler})).collect::<Vec<_>>()});
    // Reject an invalid synthetic package before any durable fixture side effect.
    plugin_framework::parse_plugin_manifest(&serde_yaml::to_string(&manifest).unwrap())
        .expect("valid synthetic record resource declaration");
    manifest
}
pub(super) fn package() -> Vec<u8> {
    let executable=std::env::var_os("RECORD_PERMISSION_WORKER_FIXTURE").expect("RECORD_PERMISSION_WORKER_FIXTURE must point to the CI-built SDK example; this test must not silently skip");
    assert!(
        std::path::Path::new(&executable).is_file(),
        "finite SDK worker fixture does not exist"
    );
    let manifest = serde_yaml::to_string(&manifest()).unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    let mut header = tar::Header::new_gnu();
    header.set_size(manifest.len() as u64);
    header.set_mode(0o644);
    header.set_cksum();
    archive
        .append_data(&mut header, "manifest.yaml", manifest.as_bytes())
        .unwrap();
    archive
        .append_path_with_name(executable, "bin/record_permission_worker")
        .unwrap();
    archive.into_inner().unwrap().finish().unwrap()
}
