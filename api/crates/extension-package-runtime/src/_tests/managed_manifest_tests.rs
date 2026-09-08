use crate::{parse_plugin_manifest, PluginExecutionMode};

const MANAGED: &str = include_str!("managed_manifest.yaml");

// Root #2007 AC-001/AC-002: finite schema positives and controlled negatives.
#[test]
fn root_2007_ac_001_002_manifest_routes() {
    let manifest = parse_plugin_manifest(MANAGED).expect("explicit mixed contributions");
    let managed = manifest.managed.expect("managed declaration");
    assert_eq!(managed.module.contributions.len(), 2);
    assert_eq!(managed.execution_bindings.len(), 2);
    assert_eq!(
        managed.execution_bindings[1].execution_mode,
        PluginExecutionMode::DeclarativeOnly
    );
    assert_eq!(managed.execution_bindings[0].handler, "compute");
    assert_eq!(managed.execution_bindings[1].handler, "render");

    for (name, raw) in [
        ("v1 cannot carry managed", MANAGED.replace("manifest_version: 2", "manifest_version: 1").replace("manifest/v2", "manifest/v1")),
        ("version mismatch", MANAGED.replace("manifest/v2", "manifest/v1")),
        ("unknown field", format!("{MANAGED}\nunknown_field: true\n")),
        ("legacy slot conflicts", MANAGED.replace("slot_codes: []", "slot_codes: [model_provider]")),
        ("forged identity", MANAGED.replace("module_id: managed_fixture\n", "module_id: other\n")),
        ("host impersonation", MANAGED.replace("module_kind: runtime", "module_kind: trusted_host")),
        ("self grant", MANAGED.replace("    contributions:", "    granted_permissions: [admin]\n    contributions:")),
        ("duplicate binding", MANAGED.replace("    - contribution_id: managed_fixture.render", "    - contribution_id: managed_fixture.compute")),
        ("missing binding", MANAGED.split("    - contribution_id: managed_fixture.render").next().unwrap().to_owned()),
        ("unknown binding", MANAGED.replace("    - contribution_id: managed_fixture.render", "    - contribution_id: unknown")),
        ("invalid path", MANAGED.replace("entry: ui/render.json", "entry: ../render.json")),
        ("invalid execution pair", MANAGED.replace("execution_mode: declarative_only", "execution_mode: stateful_runtime_worker")),
        ("missing payload", MANAGED.replace("      handler: render", "      handler: render\n      payload: {kind: frontend_block, contribution_code: missing}")),
    ] {
        assert!(parse_plugin_manifest(&raw).is_err(), "{name} must fail closed");
    }
}

#[test]
fn root_2007_ac_001_002_manifest_routes_preserve_typed_data_model_payload() {
    let raw = format!("{}\ndata_models:\n  - contribution_version: 1flowbase.plugin-data-model/v1\n    storage_binding: main\n    owned_collections:\n      - collection_code: processed_models\n        fields:\n          - field_code: model_code\n            field_type: string\n            nullable: false\n", MANAGED.replace("storage: none", "storage: host_managed").replace("      handler: render", "      handler: render\n      payload: {kind: data_models}"));
    let manifest =
        parse_plugin_manifest(&raw).expect("typed payload retains its canonical contract");
    assert_eq!(
        manifest.data_models[0].owned_collections[0].collection_code,
        "processed_models"
    );
    assert!(
        parse_plugin_manifest(&raw.replace("      payload: {kind: data_models}\n", "")).is_err()
    );
    assert!(parse_plugin_manifest(&raw.replace("storage: host_managed", "storage: none")).is_err());
}
