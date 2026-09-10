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

    // Anchor the complete YAML line: declarations are indented six spaces, while
    // execution bindings use four. A substring replacement would rename both sides.
    let render_binding = "\n    - contribution_id: managed_fixture.render";
    assert_eq!(MANAGED.matches(render_binding).count(), 1);

    for (name, raw) in [
        ("v1 cannot carry managed", MANAGED.replace("manifest_version: 2", "manifest_version: 1").replace("manifest/v2", "manifest/v1")),
        ("version mismatch", MANAGED.replace("manifest/v2", "manifest/v1")),
        ("unknown field", format!("{MANAGED}\nunknown_field: true\n")),
        ("legacy slot conflicts", MANAGED.replace("slot_codes: []", "slot_codes: [model_provider]")),
        ("forged identity", MANAGED.replace("module_id: managed_fixture\n", "module_id: other\n")),
        ("host impersonation", MANAGED.replace("module_kind: runtime", "module_kind: trusted_host")),
        ("self grant", MANAGED.replace("    contributions:", "    granted_permissions: [admin]\n    contributions:")),
        ("duplicate binding", MANAGED.replace(render_binding, "\n    - contribution_id: managed_fixture.compute")),
        ("missing binding", MANAGED.split(render_binding).next().unwrap().to_owned()),
        ("unknown binding", MANAGED.replace(render_binding, "\n    - contribution_id: unknown")),
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

#[test]
fn root_2014_ac_005_006_manifest_event_namespace_and_schema() {
    let raw = include_str!("../../../../plugins/fixtures/acme.composition-a/event-manifest.yaml")
        .replace("acme.composition-a", "orion.shipments")
        .replace("publisher_namespace: acme", "publisher_namespace: orion");
    let parsed = parse_plugin_manifest(&raw).expect("new installed module namespace");
    let managed = parsed.managed.unwrap();
    let point = &managed.module.extension_points[0];
    assert!(point.is_managed_composition_event(&managed.module.module_id));
    let contract =
        extension_contracts::ManagedEventSchema::from_descriptor(&point.contract).unwrap();
    assert_eq!(contract.contract_id, "orion.shipments.processed");
    assert!(parse_plugin_manifest(&raw.replace(
        "owner_module_id: orion.shipments",
        "owner_module_id: foreign.module"
    ))
    .is_err());
    assert!(parse_plugin_manifest(
        &raw.replace("additionalProperties: false", "additionalProperties: true")
    )
    .is_err());
    assert!(parse_plugin_manifest(&raw.replace("maxLength: 128", "maxLength: 999999")).is_err());
}

// Root #2014 AC-015/017/018: omitted selection preserves the existing signed binding bytes.
#[test]
fn root_2014_r3_probe_explicit_protocol_selection() {
    use extension_contracts::{ManagedBindingFingerprint, ManagedInterfaceProtocol};
    let raw = MANAGED.replace(
        "point_id: acme.compute",
        "point_id: 1flowbase.interface.shipments.create.before",
    );
    let baseline = parse_plugin_manifest(&raw).unwrap().managed.unwrap();
    let contribution = &baseline.module.contributions[0];
    let binding = &baseline.execution_bindings[0];
    assert_eq!(binding.interface_protocol, None);
    let mut historical = serde_json::to_value(binding).unwrap();
    assert!(historical
        .as_object_mut()
        .unwrap()
        .remove("interface_protocol")
        .is_none());
    #[derive(serde::Serialize)]
    struct LegacyBinding<'a> {
        contribution_id: &'a extension_contracts::ContributionId,
        execution_mode: &'a PluginExecutionMode,
        runtime: &'a crate::PluginRuntimeManifest,
        handler: &'a str,
        payload: &'a Option<crate::ManagedContributionPayload>,
    }
    let historical = LegacyBinding {
        contribution_id: &binding.contribution_id,
        execution_mode: &binding.execution_mode,
        runtime: &binding.runtime,
        handler: &binding.handler,
        payload: &binding.payload,
    };
    let expected = ManagedBindingFingerprint::from_bytes(
        &serde_json::to_vec(&(contribution, historical)).unwrap(),
    );
    assert_eq!(
        baseline
            .execution_binding_fingerprint(&contribution.contribution_id)
            .unwrap(),
        expected
    );
    for (name, selected) in [
        ("interface-v1", ManagedInterfaceProtocol::InterfaceV1),
        ("reference-v2", ManagedInterfaceProtocol::ReferenceV2),
    ] {
        let explicit = raw.replace(
            "      handler: compute",
            &format!("      handler: compute\n      interface_protocol: {name}"),
        );
        let parsed = parse_plugin_manifest(&explicit).unwrap().managed.unwrap();
        assert_eq!(
            parsed.execution_bindings[0].interface_protocol,
            Some(selected)
        );
        assert_ne!(
            parsed
                .execution_binding_fingerprint(&contribution.contribution_id)
                .unwrap(),
            expected
        );
        assert!(parse_plugin_manifest(&explicit.replace(
            "point_id: 1flowbase.interface.shipments.create.before",
            "point_id: acme.compute"
        ))
        .is_err());
        assert!(parse_plugin_manifest(&explicit.replace(
            "execution_mode: process_per_call",
            "execution_mode: declarative_only"
        ))
        .is_err());
        assert!(parse_plugin_manifest(
            &explicit.replace("interface_protocol: ", "unknown_protocol: ")
        )
        .is_err());
    }
    assert!(parse_plugin_manifest(&raw.replace(
        "      handler: compute",
        "      handler: compute\n      interface_protocol: reference-v99"
    ))
    .is_err());
}
