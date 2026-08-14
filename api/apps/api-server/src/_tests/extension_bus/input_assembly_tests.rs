use std::{fs, path::PathBuf};

use api_server::extension_bus::{
    assemble_extension_graph_input, ModuleActivationFact, DEFAULT_PLUGIN_SET_PATH,
};
use plugin_framework::extension_bus::{
    parse_deployment_plugin_set, Cardinality, CompilationError, DeliverySemantics,
    ExtensionPointKind, FailureSemantics, LifecycleSemantics, ModuleDisableReason,
    ModuleInactivityReason, ModuleResolutionStatus, OrderingSemantics, OverridePolicy,
    ScopeSemantics,
};

#[test]
fn default_plugin_set_parses_and_drives_builtin_host_inventory() {
    let root = api_workspace_root();
    let set_raw = fs::read_to_string(root.join(DEFAULT_PLUGIN_SET_PATH)).unwrap();
    let set = parse_deployment_plugin_set(&set_raw).unwrap();
    let assembly = assemble_extension_graph_input(&root, DEFAULT_PLUGIN_SET_PATH, vec![]).unwrap();

    assert_eq!(set.set_id(), "default");
    assert_eq!(set.host_extension_ids().len(), 6);
    assert_eq!(assembly.host_extension_manifests().len(), 6);
    assert!(assembly
        .module_descriptors()
        .iter()
        .any(|module| { module.module_id.as_str() == "official.runtime-orchestration-host" }));

    let graph = assembly.compile_graph().unwrap();
    let cache_store = graph
        .points()
        .iter()
        .find(|point| {
            point.descriptor().point_id.as_str()
                == api_server::extension_bus::CACHE_STORE_EXTENSION_POINT_ID
        })
        .unwrap();
    assert_eq!(
        cache_store.descriptor().cardinality,
        Cardinality::ExactlyOne
    );
    assert_eq!(
        cache_store.descriptor().point_kind,
        ExtensionPointKind::Slot
    );
    assert_eq!(cache_store.descriptor().scope, ScopeSemantics::System);
    assert_eq!(
        cache_store.descriptor().failure,
        FailureSemantics::FailClosed
    );
    assert_eq!(
        cache_store.descriptor().contract.contract_id.as_str(),
        api_server::extension_bus::CACHE_STORE_CONTRACT_ID
    );
    assert_eq!(
        cache_store.descriptor().delivery,
        DeliverySemantics::Synchronous
    );
    assert_eq!(
        cache_store.descriptor().lifecycle,
        LifecycleSemantics::BootSnapshot
    );
    assert_eq!(cache_store.contributions().len(), 1);
    assert_eq!(
        cache_store.contributions()[0]
            .descriptor()
            .contribution_id
            .as_str(),
        "official.local-infra-host.infrastructure.cache-store.local"
    );

    let model_provider = graph
        .points()
        .iter()
        .find(|point| {
            point.descriptor().point_id.as_str()
                == api_server::extension_bus::MODEL_PROVIDER_EXTENSION_POINT_ID
        })
        .unwrap();
    assert_eq!(
        model_provider.descriptor().point_kind,
        ExtensionPointKind::Slot
    );
    assert_eq!(model_provider.descriptor().cardinality, Cardinality::Many);
    assert_eq!(model_provider.descriptor().scope, ScopeSemantics::Global);
    assert_eq!(
        model_provider.descriptor().contract.contract_id.as_str(),
        api_server::extension_bus::MODEL_PROVIDER_CONTRACT_ID
    );
    assert_eq!(
        model_provider
            .descriptor()
            .contract
            .contract_version
            .as_str(),
        api_server::extension_bus::MODEL_PROVIDER_CONTRACT_VERSION
    );
    assert_eq!(
        model_provider.descriptor().failure,
        FailureSemantics::FailClosed
    );
    assert_eq!(
        model_provider.descriptor().delivery,
        DeliverySemantics::Synchronous
    );
    assert_eq!(
        model_provider.descriptor().lifecycle,
        LifecycleSemantics::RuntimeWorker
    );
    assert!(model_provider.contributions().is_empty());

    let provider_input_pipeline = graph
        .points()
        .iter()
        .find(|point| {
            point.descriptor().point_id.as_str()
                == api_server::extension_bus::PROVIDER_INPUT_PIPELINE_POINT_ID
        })
        .unwrap();
    let descriptor = provider_input_pipeline.descriptor();
    assert_eq!(
        descriptor.owner_module_id.as_str(),
        api_server::extension_bus::BOOT_CORE_MODULE_ID
    );
    assert_eq!(descriptor.point_kind, ExtensionPointKind::Pipeline);
    assert_eq!(descriptor.cardinality, Cardinality::Many);
    assert_eq!(descriptor.scope, ScopeSemantics::Global);
    assert_eq!(descriptor.ordering, OrderingSemantics::Dependency);
    assert_eq!(descriptor.failure, FailureSemantics::FailClosed);
    assert_eq!(descriptor.delivery, DeliverySemantics::Synchronous);
    assert_eq!(descriptor.lifecycle, LifecycleSemantics::Invocation);
    assert_eq!(descriptor.override_policy, OverridePolicy::Sealed);
    assert_eq!(
        descriptor.contract.contract_id.as_str(),
        api_server::extension_bus::PROVIDER_INPUT_PIPELINE_CONTRACT_ID
    );
    assert_eq!(
        descriptor.contract.contract_version.as_str(),
        api_server::extension_bus::PROVIDER_INPUT_PIPELINE_CONTRACT_VERSION
    );
    assert_eq!(
        descriptor
            .allowed_permissions
            .iter()
            .map(|permission| permission.as_str())
            .collect::<Vec<_>>(),
        vec![
            "provider-input.messages.write",
            "provider-input.model-parameters.write",
            "provider-input.response-format.write",
            "provider-input.system.write",
            "provider-input.tools.write",
        ]
    );
    assert!(provider_input_pipeline.contributions().is_empty());
}

// Root #1688 AC-001/AC-002: set declaration order is not graph resolution order.
#[test]
fn plugin_set_permutation_compiles_the_same_graph() {
    let fixture = FixtureWorkspace::new("permutation");
    fixture.add_cache_host_extension("fixture.alpha", "fixture.alpha");
    fixture.add_host_extension("fixture.beta", "fixture.beta");
    fixture.write_set(
        "forward.yaml",
        "forward",
        &["fixture.alpha", "fixture.beta"],
    );
    fixture.write_set(
        "reverse.yaml",
        "reverse",
        &["fixture.beta", "fixture.alpha"],
    );

    let forward =
        assemble_extension_graph_input(fixture.root(), "plugins/sets/forward.yaml", vec![])
            .unwrap()
            .compile_graph()
            .unwrap();
    let reverse =
        assemble_extension_graph_input(fixture.root(), "plugins/sets/reverse.yaml", vec![])
            .unwrap()
            .compile_graph()
            .unwrap();

    assert_eq!(forward, reverse);
}

#[test]
fn cache_store_slot_fails_closed_without_or_with_disabled_winner() {
    let missing = FixtureWorkspace::new("missing-cache-winner");
    missing.add_host_extension("fixture.host", "fixture.host");
    missing.write_set("default.yaml", "default", &["fixture.host"]);
    let missing_error =
        assemble_extension_graph_input(missing.root(), "plugins/sets/default.yaml", vec![])
            .unwrap()
            .compile_graph()
            .unwrap_err();
    assert!(matches!(
        missing_error.downcast_ref::<CompilationError>(),
        Some(CompilationError::CardinalityConflict {
            cardinality: Cardinality::ExactlyOne,
            actual: 0,
            ..
        })
    ));

    let disabled = assemble_extension_graph_input(
        api_workspace_root(),
        DEFAULT_PLUGIN_SET_PATH,
        vec![ModuleActivationFact::disabled(
            "official.local-infra-host",
            ModuleDisableReason::DesiredState,
        )
        .unwrap()],
    )
    .unwrap()
    .compile_graph()
    .unwrap_err();
    assert!(matches!(
        disabled.downcast_ref::<CompilationError>(),
        Some(CompilationError::CardinalityConflict {
            cardinality: Cardinality::ExactlyOne,
            actual: 0,
            ..
        })
    ));
}

// Root #1688 AC-003: all set/package identity failures stop before a graph can be published.
#[test]
fn missing_package_and_identity_mismatch_fail_input_assembly() {
    let missing = FixtureWorkspace::new("missing");
    missing.write_set("default.yaml", "default", &["fixture.missing"]);
    let error = assemble_extension_graph_input(missing.root(), "plugins/sets/default.yaml", vec![])
        .unwrap_err();
    assert!(format!("{error:#}").contains("fixture.missing"));

    let mismatch = FixtureWorkspace::new("mismatch");
    mismatch.add_host_extension_with_contribution(
        "fixture.listed",
        "fixture.listed",
        "fixture.other",
        false,
    );
    mismatch.write_set("default.yaml", "default", &["fixture.listed"]);
    let error =
        assemble_extension_graph_input(mismatch.root(), "plugins/sets/default.yaml", vec![])
            .unwrap_err();
    assert!(format!("{error:#}").contains("identity mismatch"));
}

// Root #1688 AC-010: deployment, desired-state, and assignment facts remain typed receipts.
#[test]
fn activation_facts_become_typed_inactive_receipts() {
    let assembly = assemble_extension_graph_input(
        api_workspace_root(),
        DEFAULT_PLUGIN_SET_PATH,
        vec![
            ModuleActivationFact::disabled(
                "official.identity-host",
                ModuleDisableReason::DeploymentPolicy,
            )
            .unwrap(),
            ModuleActivationFact::disabled(
                "official.workspace-host",
                ModuleDisableReason::DesiredState,
            )
            .unwrap(),
            ModuleActivationFact::disabled("1flowbase", ModuleDisableReason::WorkspaceAssignment)
                .unwrap(),
        ],
    )
    .unwrap();
    let graph = assembly.compile_graph().unwrap();

    for (module_id, reason) in [
        (
            "official.identity-host",
            ModuleDisableReason::DeploymentPolicy,
        ),
        ("official.workspace-host", ModuleDisableReason::DesiredState),
        ("1flowbase", ModuleDisableReason::WorkspaceAssignment),
    ] {
        let receipt = graph
            .module_receipts()
            .iter()
            .find(|receipt| receipt.provenance().module_id().as_str() == module_id)
            .unwrap();
        assert_eq!(
            receipt.status(),
            &ModuleResolutionStatus::Inactive {
                reason: ModuleInactivityReason::Disabled { reason },
            }
        );
    }
}

// Root #1688 AC-001: discovery follows set data and package convention, not compiler edits.
#[test]
fn added_set_module_is_discovered_without_a_central_path_entry() {
    let fixture = FixtureWorkspace::new("added-module");
    fixture.add_host_extension("fixture.added", "fixture.added");
    fixture.write_set("default.yaml", "default", &["fixture.added"]);

    let assembly =
        assemble_extension_graph_input(fixture.root(), "plugins/sets/default.yaml", vec![])
            .unwrap();

    assert!(assembly
        .module_descriptors()
        .iter()
        .any(|module| module.module_id.as_str() == "fixture.added"));
    assert_eq!(assembly.host_extension_manifests().len(), 1);
}

struct FixtureWorkspace {
    root: PathBuf,
}

impl FixtureWorkspace {
    fn new(name: &str) -> Self {
        let root = std::env::temp_dir().join(format!(
            "1flowbase-extension-bus-{name}-{}",
            uuid::Uuid::now_v7()
        ));
        fs::create_dir_all(root.join("plugins/sets")).unwrap();
        Self { root }
    }

    fn root(&self) -> &PathBuf {
        &self.root
    }

    fn add_host_extension(&self, directory_id: &str, manifest_id: &str) {
        self.add_host_extension_with_contribution(directory_id, manifest_id, manifest_id, false);
    }

    fn add_cache_host_extension(&self, directory_id: &str, manifest_id: &str) {
        self.add_host_extension_with_contribution(directory_id, manifest_id, manifest_id, true);
    }

    fn add_host_extension_with_contribution(
        &self,
        directory_id: &str,
        manifest_id: &str,
        contribution_id: &str,
        provides_cache_store: bool,
    ) {
        let package = self.root.join("plugins/host-extensions").join(directory_id);
        fs::create_dir_all(&package).unwrap();
        fs::write(
            package.join("manifest.yaml"),
            host_manifest_yaml(manifest_id),
        )
        .unwrap();
        fs::write(
            package.join("host-extension.yaml"),
            host_contribution_yaml(contribution_id, provides_cache_store),
        )
        .unwrap();
    }

    fn write_set(&self, file_name: &str, set_id: &str, host_extensions: &[&str]) {
        fs::write(
            self.root.join("plugins/sets").join(file_name),
            plugin_set_yaml(set_id, host_extensions),
        )
        .unwrap();
    }
}

impl Drop for FixtureWorkspace {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.root);
    }
}

fn plugin_set_yaml(set_id: &str, host_extensions: &[&str]) -> String {
    let entries = host_extensions
        .iter()
        .map(|id| format!("  - {id}"))
        .collect::<Vec<_>>()
        .join("\n");
    format!(
        "schema_version: 1flowbase.plugin-set/v1\nset_id: {set_id}\nhost_extensions:\n{entries}\nruntime_extensions: []\ncapability_plugins: []\n"
    )
}

fn host_manifest_yaml(module_id: &str) -> String {
    format!(
        r#"schema_version: 1flowbase.plugin.manifest/v1
manifest_version: 1
plugin_id: {module_id}@0.1.0
version: 0.1.0
publisher_namespace: fixture
vendor: fixture
display_name: Fixture Host
description: Fixture host extension.
source_kind: official_registry
trust_level: verified_official
consumption_kind: host_extension
execution_mode: in_process
slot_codes: [host_bootstrap]
binding_targets: []
selection_mode: auto_activate
minimum_host_version: 0.1.0
contract_version: 1flowbase.host_extension/v1
permissions:
  network: none
  secrets: host_managed
  storage: host_managed
  mcp: none
  subprocess: deny
runtime:
  protocol: native_host
  entry: host-extension.yaml
"#
    )
}

fn host_contribution_yaml(module_id: &str, provides_cache_store: bool) -> String {
    let infrastructure_providers = if provides_cache_store {
        r#"infrastructure_providers:
  - contract: cache-store
    provider_code: local
    display_name: Local
    config_ref: secret://system/fixture/config
    config_schema: []"#
    } else {
        "infrastructure_providers: []"
    };
    format!(
        r#"schema_version: 1flowbase.host-extension/v1
extension_id: {module_id}
version: 0.1.0
bootstrap_phase: boot
native:
  abi_version: 1flowbase.host.native/v1
  library: builtin://{module_id}
  entry_symbol: fixture_host
owned_resources: []
extends_resources: []
{infrastructure_providers}
routes: []
workers: []
migrations: []
"#
    )
}

fn api_workspace_root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..")
}
