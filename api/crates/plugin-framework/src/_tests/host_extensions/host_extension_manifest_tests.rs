use plugin_framework::{
    parse_host_extension_manifest, HostExtensionActivationPhase, HostExtensionSourceKind,
};

#[test]
fn host_extension_manifest_parses_storage_host() {
    let raw = r#"
manifest_version: 1
extension_id: official.storage-host
version: 0.1.0
display_name: Storage Host
source_kind: builtin
trust_level: trusted_host
activation_phase: boot
provides_contracts:
  - storage-durable
  - storage-ephemeral
  - storage-object
overrides_contracts: []
registers_slots: []
registers_interfaces:
  - code: storage.health
    kind: internal_service
registers_storage:
  - kind: storage-durable
    implementation: postgres
  - kind: storage-ephemeral
    implementation: memory
dependencies: []
load_order:
  after: []
  before:
    - official.data-access-host
"#;

    let manifest = parse_host_extension_manifest(raw).expect("manifest should parse");
    assert_eq!(manifest.extension_id, "official.storage-host");
    assert_eq!(manifest.source_kind, HostExtensionSourceKind::Builtin);
    assert_eq!(
        manifest.activation_phase,
        HostExtensionActivationPhase::Boot
    );
    assert_eq!(
        manifest.provides_contracts,
        vec!["storage-durable", "storage-ephemeral", "storage-object"]
    );
    assert_eq!(manifest.registers_storage[0].implementation, "postgres");
}

#[test]
fn host_extension_manifest_rejects_runtime_activation() {
    let raw = r#"
manifest_version: 1
extension_id: official.bad-host
version: 0.1.0
display_name: Bad Host
source_kind: builtin
trust_level: trusted_host
activation_phase: runtime
provides_contracts: []
overrides_contracts: []
registers_slots: []
registers_interfaces: []
registers_storage: []
dependencies: []
load_order:
  after: []
  before: []
"#;

    let error = parse_host_extension_manifest(raw).expect_err("runtime activation is invalid");
    assert!(error.to_string().contains("activation_phase"));
}
