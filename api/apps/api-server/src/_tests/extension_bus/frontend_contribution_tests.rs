use std::{collections::BTreeSet, path::PathBuf, sync::Arc};

use control_plane::frontend_block_catalog::{
    FrontendContributionCandidate, FrontendContributionDisableReason,
    FrontendContributionExecutionKind, FrontendContributionIsolationRequirement,
    FrontendContributionResolution, FrontendContributionResolver, FrontendContributionRuntimeKind,
    FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID, FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION,
    FRONTEND_BLOCK_CONTRIBUTION_POINT_ID, FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
    FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION,
};
use plugin_framework::extension_bus::{
    Cardinality, DeliverySemantics, FailureSemantics, LifecycleSemantics, ModuleKind,
    PermissionCode, ScopeSemantics,
};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::extension_bus::{assemble_extension_graph_input, DEFAULT_PLUGIN_SET_PATH};

fn assembly() -> crate::extension_bus::ExtensionGraphInputAssembly {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    assemble_extension_graph_input(root, DEFAULT_PLUGIN_SET_PATH, Vec::new()).unwrap()
}

fn candidate(workspace_id: Uuid) -> (FrontendContributionCandidate, PathBuf) {
    let installation_id = Uuid::now_v7();
    let actor_id = Uuid::now_v7();
    let root =
        std::env::temp_dir().join(format!("1flowbase-frontend-contribution-{installation_id}"));
    std::fs::create_dir_all(root.join("assets")).unwrap();
    std::fs::write(root.join("assets/block.js"), "export const block = true;\n").unwrap();
    let digest = "65d5a7d33d0bc8d0696e6df1fc36cf4568840243a809f7338d4527c1f8fb1eb6";
    let installation = domain::PluginInstallationRecord {
        id: installation_id,
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::CapabilityPlugins,
        organization: "test".to_string(),
        provider_code: "fixture.frontend".to_string(),
        plugin_id: "fixture.frontend@1.0.0".to_string(),
        plugin_version: "1.0.0".to_string(),
        contract_version: "1flowbase.capability/v1".to_string(),
        protocol: "stdio_json".to_string(),
        display_name: "Fixture frontend".to_string(),
        source_kind: "uploaded".to_string(),
        trust_level: "checksum_only".to_string(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: domain::PluginDesiredState::ActiveRequested,
        expected_checksum: None,
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: serde_json::json!({}),
        is_system_reserved: false,
        created_by: actor_id,
        updated_by: None,
        created_at: OffsetDateTime::now_utc(),
        updated_at: OffsetDateTime::now_utc(),
    };
    let artifact = domain::PluginArtifactInstanceRecord {
        node_id: "test-node".to_string(),
        installation_id,
        local_version: Some("1.0.0".to_string()),
        local_checksum: None,
        local_path: Some(root.display().to_string()),
        package_path: None,
        manifest_fingerprint: Some("fixture-manifest".to_string()),
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        runtime_status: domain::PluginRuntimeStatus::Inactive,
        availability_status: domain::PluginAvailabilityStatus::Available,
        checked_at: OffsetDateTime::now_utc(),
        last_error: None,
        is_current: true,
    };
    let assignment = domain::PluginAssignmentRecord {
        id: Uuid::now_v7(),
        installation_id,
        workspace_id,
        provider_code: installation.provider_code.clone(),
        assigned_by: actor_id,
        created_at: OffsetDateTime::now_utc(),
    };
    let catalog_entry = domain::FrontendBlockCatalogEntry {
        installation_id,
        provider_code: installation.provider_code.clone(),
        plugin_id: installation.plugin_id.clone(),
        plugin_version: installation.plugin_version.clone(),
        contribution_code: "hero".to_string(),
        title: "Hero".to_string(),
        runtime: "native_react".to_string(),
        entry: "blocks/hero.js".to_string(),
        code_template: None,
        code_template_version: None,
        code_template_language: None,
        code_modules: vec![domain::FrontendBlockCodeModule {
            source: "@fixture/block".to_string(),
            version: "1.0.0".to_string(),
            exports: vec!["Hero".to_string()],
            binding: domain::FrontendModuleBinding::Fetched,
            assets: vec![domain::FrontendModuleAsset {
                path: "assets/block.js".to_string(),
                role: domain::FrontendModuleAssetRole::BrowserModule,
                media_type: "text/javascript; charset=utf-8".to_string(),
                sha256: digest.to_string(),
            }],
            type_declarations: "declare module '@fixture/block' {}".to_string(),
        }],
        context_contract: domain::FrontendBlockContextContract {
            primitives: vec!["text".to_string()],
            input_schema: serde_json::json!({"type": "object"}),
        },
        permissions: domain::FrontendBlockPermissions {
            network: "none".to_string(),
            storage: "none".to_string(),
            secrets: "none".to_string(),
        },
        ui_capabilities: vec!["responsive".to_string()],
    };
    (
        FrontendContributionCandidate {
            workspace_id,
            installation,
            artifact,
            assignment: Some(assignment),
            catalog_entry,
        },
        root,
    )
}

fn disabled_reason(
    resolution: FrontendContributionResolution,
) -> FrontendContributionDisableReason {
    match resolution {
        FrontendContributionResolution::Active(_) => panic!("candidate unexpectedly projected"),
        FrontendContributionResolution::Disabled(receipt) => receipt.reason,
    }
}

#[test]
fn boot_point_and_valid_workspace_assignment_produce_one_scoped_typed_binding() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let point = graph
        .points()
        .iter()
        .find(|point| point.descriptor().point_id.as_str() == FRONTEND_BLOCK_CONTRIBUTION_POINT_ID)
        .unwrap();
    assert_eq!(
        point.descriptor().contract.contract_id.as_str(),
        FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID
    );
    assert_eq!(
        point.descriptor().contract.contract_version.as_str(),
        FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION
    );
    assert_eq!(point.descriptor().scope, ScopeSemantics::Workspace);
    assert_eq!(point.descriptor().cardinality, Cardinality::Many);
    assert_eq!(point.descriptor().failure, FailureSemantics::FailClosed);
    assert_eq!(point.descriptor().delivery, DeliverySemantics::Synchronous);
    assert_eq!(
        point.descriptor().lifecycle,
        LifecycleSemantics::WorkspaceAssignment
    );
    assert!(point.contributions().is_empty());

    let resolver = FrontendContributionResolver::compile(Arc::clone(&graph)).unwrap();
    let workspace_id = Uuid::now_v7();
    let (candidate, root) = candidate(workspace_id);
    let binding = match resolver.resolve(candidate) {
        FrontendContributionResolution::Active(binding) => binding,
        FrontendContributionResolution::Disabled(receipt) => {
            panic!("valid candidate disabled: {:?}", receipt.reason)
        }
    };
    assert!(Arc::ptr_eq(resolver.graph_arc(), binding.graph_arc()));
    assert!(Arc::ptr_eq(binding.graph_arc(), &graph));
    assert_eq!(binding.graph_fingerprint, graph.fingerprint().as_str());
    assert_eq!(binding.provenance, *point.provenance());
    assert_eq!(binding.provenance.module_kind(), ModuleKind::BootCore);
    assert_eq!(binding.workspace_id, workspace_id);
    assert!(binding.block_id.ends_with(":hero"));
    assert_eq!(binding.block_version, "1.0.0");
    assert_eq!(
        binding.runtime_kind,
        FrontendContributionRuntimeKind::TrustedNative
    );
    assert_eq!(
        binding.execution_kind,
        FrontendContributionExecutionKind::UiMount
    );
    assert_eq!(
        binding.isolation_requirement,
        FrontendContributionIsolationRequirement::TrustedHostRealm
    );
    assert_eq!(
        FrontendContributionRuntimeKind::Isolated.as_str(),
        "isolated"
    );
    assert_eq!(
        FrontendContributionIsolationRequirement::IndependentRealm.as_str(),
        "independent_realm"
    );
    assert_eq!(binding.lifecycle, LifecycleSemantics::WorkspaceAssignment);
    assert_eq!(
        binding.requested_permissions,
        vec![FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION]
    );
    assert_eq!(binding.requested_permissions, binding.granted_permissions);
    assert!(binding.assets.is_empty());
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn isolated_iframe_projects_independent_realm_and_narrow_permission() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let resolver = FrontendContributionResolver::compile(graph).unwrap();
    let workspace_id = Uuid::now_v7();
    let (mut isolated, root) = candidate(workspace_id);
    isolated.catalog_entry.runtime = "isolated_iframe".to_string();

    let binding = match resolver.resolve(isolated) {
        FrontendContributionResolution::Active(binding) => binding,
        FrontendContributionResolution::Disabled(receipt) => {
            panic!("isolated candidate disabled: {:?}", receipt.reason)
        }
    };

    assert_eq!(
        binding.runtime_kind,
        FrontendContributionRuntimeKind::Isolated
    );
    assert_eq!(
        binding.isolation_requirement,
        FrontendContributionIsolationRequirement::IndependentRealm
    );
    assert_eq!(
        binding.requested_permissions,
        vec![FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION]
    );
    assert_eq!(binding.requested_permissions, binding.granted_permissions);
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn invalid_runtime_facts_and_workspace_scope_fail_closed() {
    let assembly = assembly();
    let graph = Arc::new(assembly.compile_graph().unwrap());
    let resolver = FrontendContributionResolver::compile(Arc::clone(&graph)).unwrap();
    let workspace_id = Uuid::now_v7();
    let (valid, root) = candidate(workspace_id);

    let mut digest_mismatch = valid.clone();
    digest_mismatch.catalog_entry.code_modules[0].assets[0].sha256 = "a".repeat(64);
    assert!(matches!(
        resolver.resolve(digest_mismatch),
        FrontendContributionResolution::Active(_)
    ));

    let mut invalid_media_type = valid.clone();
    invalid_media_type.catalog_entry.code_modules[0].assets[0].media_type = " ".to_string();
    assert!(matches!(
        resolver.resolve(invalid_media_type),
        FrontendContributionResolution::Active(_)
    ));

    let mut stale_assignment = valid.clone();
    stale_assignment.assignment.as_mut().unwrap().provider_code = "stale".to_string();
    assert_eq!(
        disabled_reason(resolver.resolve(stale_assignment)),
        FrontendContributionDisableReason::AssignmentStale
    );

    let mut wrong_workspace = valid.clone();
    wrong_workspace.assignment.as_mut().unwrap().workspace_id = Uuid::now_v7();
    assert_eq!(
        disabled_reason(resolver.resolve(wrong_workspace)),
        FrontendContributionDisableReason::AssignmentWorkspaceMismatch
    );

    let mut invalid_verification = valid.clone();
    invalid_verification.installation.verification_status =
        domain::PluginVerificationStatus::Invalid;
    assert_eq!(
        disabled_reason(resolver.resolve(invalid_verification)),
        FrontendContributionDisableReason::VerificationInvalid
    );

    let mut desired_disabled = valid.clone();
    desired_disabled.installation.desired_state = domain::PluginDesiredState::Disabled;
    assert_eq!(
        disabled_reason(resolver.resolve(desired_disabled)),
        FrontendContributionDisableReason::DesiredStateInactive
    );

    let mut disabled_artifact = valid.clone();
    disabled_artifact.artifact.availability_status = domain::PluginAvailabilityStatus::Disabled;
    assert_eq!(
        disabled_reason(resolver.resolve(disabled_artifact)),
        FrontendContributionDisableReason::ArtifactUnavailable
    );

    let mut modules = assembly.module_descriptors().to_vec();
    let point = modules
        .iter_mut()
        .flat_map(|module| module.extension_points.iter_mut())
        .find(|point| point.point_id.as_str() == FRONTEND_BLOCK_CONTRIBUTION_POINT_ID)
        .unwrap();
    point.allowed_permissions =
        BTreeSet::from([PermissionCode::new(FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION).unwrap()]);
    let denied_graph =
        Arc::new(plugin_framework::extension_bus::compile_extension_graph(modules).unwrap());
    let denied = FrontendContributionResolver::compile(denied_graph).unwrap();
    assert_eq!(
        disabled_reason(denied.resolve(valid)),
        FrontendContributionDisableReason::PermissionDenied
    );
    std::fs::remove_dir_all(root).unwrap();
}

#[test]
fn resolver_source_uses_typed_facts_without_manifest_json_or_shadow_dom_isolation_claims() {
    let source = include_str!(
        "../../../../../crates/control-plane/src/frontend_block_catalog/frontend_contribution.rs"
    );
    assert!(!source.contains("serde_json"));
    assert!(!source.contains("metadata_json"));
    assert!(!source.contains("parse_plugin_manifest"));
    assert!(!source.contains("shadow_dom"));
}
