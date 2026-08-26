use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{Arc, Mutex},
};

use async_trait::async_trait;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    mcp_bundle::{
        compare_system_versions, official_builtin_interface_disposition,
        retain_official_builtin_tools, OfficialBuiltinInterfaceDisposition,
        SeedBuiltinMcpBundleCommand,
    },
    mcp_management::McpManagementService,
    ports::*,
};

fn interface(
    interface_id: &str,
    source: domain::McpInterfaceCatalogSource,
) -> domain::McpInterfaceCatalogEntry {
    domain::McpInterfaceCatalogEntry {
        interface_id: interface_id.into(),
        source,
        method: "GET".into(),
        path: format!("/api/{interface_id}"),
        name: interface_id.into(),
        short_description: interface_id.into(),
        parameter_descriptors: Vec::new(),
        parameter_schema: serde_json::json!({}),
        result_schema: serde_json::json!({}),
        permission_code: None,
        security: serde_json::json!([]),
        risk_level: domain::McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    }
}

fn tool(
    id: u128,
    tool_id: &str,
    execution_target: domain::McpToolExecutionTarget,
) -> domain::McpToolRecord {
    domain::McpToolRecord {
        id: Uuid::from_u128(id),
        workspace_id: Uuid::nil(),
        tool_id: tool_id.into(),
        name: tool_id.into(),
        short_description: tool_id.into(),
        full_description: tool_id.into(),
        execution_target,
        parameter_schema: serde_json::json!({}),
        result_schema: serde_json::json!({}),
        input_mapping: serde_json::json!({}),
        output_mapping: serde_json::json!({}),
        permission_code: None,
        risk_level: domain::McpRiskLevel::Low,
        des_id: tool_id.into(),
        des_id_required: false,
        status: domain::McpToolStatus::Enabled,
        revision: 1,
        managed_by: None,
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

fn binding(id: u128, tool_id: &str) -> domain::McpToolBindingRecord {
    domain::McpToolBindingRecord {
        id: Uuid::from_u128(id + 100),
        instance_record_id: Uuid::nil(),
        tool_record_id: Uuid::from_u128(id),
        group_path: "/".into(),
        tool_id: tool_id.into(),
        display_alias: None,
        visible: true,
        sort_order: id as i32,
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    }
}

#[derive(Debug, Clone, PartialEq)]
struct SeedGraphSnapshot {
    applied_versions: Vec<String>,
    tool_sources: Vec<domain::McpManagedBundleSource>,
    instance_sources: Vec<domain::McpManagedBundleSource>,
    binding_count: usize,
}

#[derive(Default)]
struct SeedGraphState {
    receipts: BTreeSet<(String, String, String)>,
    applied_versions: Vec<String>,
    tools: BTreeMap<String, domain::McpManagedBundleSource>,
    instances: BTreeMap<String, (domain::McpManagedBundleSource, usize)>,
}

#[derive(Clone, Default)]
struct SeedOnlyMcpRepository {
    state: Arc<Mutex<SeedGraphState>>,
}

impl SeedOnlyMcpRepository {
    fn remove_managed_graph(&self) {
        let mut state = self
            .state
            .lock()
            .expect("seed graph recording lock must not be poisoned");
        state.tools.clear();
        state.instances.clear();
    }

    fn snapshot(&self) -> SeedGraphSnapshot {
        let state = self
            .state
            .lock()
            .expect("seed graph recording lock must not be poisoned");
        SeedGraphSnapshot {
            applied_versions: state.applied_versions.clone(),
            tool_sources: state.tools.values().cloned().collect(),
            instance_sources: state
                .instances
                .values()
                .map(|(source, _)| source.clone())
                .collect(),
            binding_count: state
                .instances
                .values()
                .map(|(_, binding_count)| *binding_count)
                .sum(),
        }
    }
}

#[async_trait]
impl McpManagementRepository for SeedOnlyMcpRepository {
    async fn record_mcp_extension_bundle_import(
        &self,
        _workspace_id: Uuid,
        _extension_installation_id: Uuid,
        _actor_user_id: Uuid,
        _result_status: &str,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not record extension imports")
    }

    async fn has_mcp_extension_bundle_import(
        &self,
        _workspace_id: Uuid,
        _extension_installation_id: Uuid,
    ) -> anyhow::Result<bool> {
        unreachable!("seed-only fixture does not read extension imports")
    }

    async fn load_actor_context_for_user(
        &self,
        _actor_user_id: Uuid,
    ) -> anyhow::Result<domain::ActorContext> {
        unreachable!("builtin seed does not authorize through actor context")
    }

    async fn list_mcp_instances(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpInstanceRecord>> {
        unreachable!("seed-only fixture does not list instances")
    }

    async fn get_mcp_instance(
        &self,
        _workspace_id: Uuid,
        _instance_id: &str,
    ) -> anyhow::Result<Option<domain::McpInstanceRecord>> {
        unreachable!("seed-only fixture does not read instances")
    }

    async fn create_mcp_instance(
        &self,
        _input: &CreateMcpInstanceInput,
    ) -> anyhow::Result<domain::McpInstanceRecord> {
        unreachable!("seed-only fixture does not create instances")
    }

    async fn create_mcp_instance_graph_atomically(
        &self,
        _input: &CreateMcpInstanceGraphInput,
    ) -> anyhow::Result<domain::McpInstanceRecord> {
        unreachable!("seed-only fixture does not create unmanaged graphs")
    }

    async fn replace_mcp_bundle_graph_atomically(
        &self,
        _input: &ReplaceMcpBundleGraphInput,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not import bundles")
    }

    async fn seed_mcp_bundle_graph_once_atomically(
        &self,
        input: &SeedMcpBundleGraphInput,
    ) -> anyhow::Result<()> {
        let key = (
            input.source.organization.clone(),
            input.source.bundle_id.clone(),
            input.source.bundle_version.clone(),
        );
        let mut state = self
            .state
            .lock()
            .expect("seed graph recording lock must not be poisoned");
        if !state.receipts.insert(key) {
            return Ok(());
        }
        state
            .applied_versions
            .push(input.source.bundle_version.clone());
        for tool in &input.tools {
            state
                .tools
                .insert(tool.tool_id.clone(), input.source.clone());
        }
        for instance in &input.instances {
            state.instances.insert(
                instance.instance_id.clone(),
                (input.source.clone(), instance.bindings.len()),
            );
        }
        Ok(())
    }

    async fn update_mcp_instance(
        &self,
        _input: &UpdateMcpInstanceInput,
    ) -> anyhow::Result<domain::McpInstanceRecord> {
        unreachable!("seed-only fixture does not update instances")
    }

    async fn delete_mcp_instance(
        &self,
        _workspace_id: Uuid,
        _instance_id: &str,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture uses direct graph removal")
    }

    async fn get_mcp_client_credential(
        &self,
        _user_id: Uuid,
        _workspace_id: Uuid,
        _instance_record_id: Uuid,
        _master_key: &str,
    ) -> anyhow::Result<Option<String>> {
        unreachable!("seed-only fixture does not read credentials")
    }

    async fn upsert_mcp_client_credential(
        &self,
        _input: &UpsertMcpClientCredentialInput,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not write credentials")
    }

    async fn delete_mcp_client_credential(
        &self,
        _user_id: Uuid,
        _workspace_id: Uuid,
        _instance_record_id: Uuid,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not delete credentials")
    }

    async fn list_mcp_upstream_connections(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpUpstreamConnectionRecord>> {
        unreachable!("seed-only fixture does not list upstream connections")
    }

    async fn get_mcp_upstream_connection(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
    ) -> anyhow::Result<Option<domain::McpUpstreamConnectionRecord>> {
        unreachable!("seed-only fixture does not read upstream connections")
    }

    async fn create_mcp_upstream_connection(
        &self,
        _input: &CreateMcpUpstreamConnectionInput,
    ) -> anyhow::Result<domain::McpUpstreamConnectionRecord> {
        unreachable!("seed-only fixture does not create upstream connections")
    }

    async fn update_mcp_upstream_connection(
        &self,
        _input: &UpdateMcpUpstreamConnectionInput,
    ) -> anyhow::Result<domain::McpUpstreamConnectionRecord> {
        unreachable!("seed-only fixture does not update upstream connections")
    }

    async fn delete_mcp_upstream_connection(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not delete upstream connections")
    }

    async fn get_mcp_upstream_secret(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
        _master_key: &str,
    ) -> anyhow::Result<Option<serde_json::Value>> {
        unreachable!("seed-only fixture does not read upstream secrets")
    }

    async fn upsert_mcp_upstream_secret(
        &self,
        _input: &UpsertMcpUpstreamSecretInput,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not write upstream secrets")
    }

    async fn delete_mcp_upstream_secret(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not delete upstream secrets")
    }

    async fn record_mcp_upstream_connection_result(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
        _connected_at: Option<OffsetDateTime>,
        _discovered_at: Option<OffsetDateTime>,
        _last_error: Option<&str>,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not record upstream results")
    }

    async fn list_mcp_upstream_tool_sources(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpUpstreamToolSourceRecord>> {
        unreachable!("seed-only fixture does not list upstream tools")
    }

    async fn upsert_mcp_upstream_tool_source(
        &self,
        _input: &UpsertMcpUpstreamToolSourceInput,
    ) -> anyhow::Result<domain::McpUpstreamToolSourceRecord> {
        unreachable!("seed-only fixture does not write upstream tools")
    }

    async fn mark_mcp_upstream_tool_sources_missing(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
        _discovered_remote_tool_names: &[String],
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not reconcile upstream tools")
    }

    async fn link_mcp_upstream_tool_source(
        &self,
        _workspace_id: Uuid,
        _connection_id: Uuid,
        _remote_tool_name: &str,
        _tool_record_id: Uuid,
    ) -> anyhow::Result<domain::McpUpstreamToolSourceRecord> {
        unreachable!("seed-only fixture does not link upstream tools")
    }

    async fn list_mcp_groups(
        &self,
        _instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpGroupRecord>> {
        unreachable!("seed-only fixture does not list groups")
    }

    async fn upsert_mcp_group(
        &self,
        _input: &UpsertMcpGroupInput,
    ) -> anyhow::Result<domain::McpGroupRecord> {
        unreachable!("seed-only fixture does not write groups")
    }

    async fn update_mcp_group(
        &self,
        _input: &UpsertMcpGroupInput,
    ) -> anyhow::Result<domain::McpGroupRecord> {
        unreachable!("seed-only fixture does not update groups")
    }

    async fn move_mcp_group(
        &self,
        _actor_user_id: Uuid,
        _instance_record_id: Uuid,
        _source_path: &str,
        _target_path: &str,
        _sort_order: i32,
    ) -> anyhow::Result<domain::McpGroupRecord> {
        unreachable!("seed-only fixture does not move groups")
    }

    async fn delete_mcp_group_subtree(
        &self,
        _instance_record_id: Uuid,
        _path: &str,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not delete groups")
    }

    async fn list_mcp_tools(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpToolRecord>> {
        unreachable!("seed-only fixture does not list tools")
    }

    async fn get_mcp_tool(
        &self,
        _workspace_id: Uuid,
        _tool_id: &str,
    ) -> anyhow::Result<Option<domain::McpToolRecord>> {
        unreachable!("seed-only fixture does not read tools")
    }

    async fn create_mcp_tool(
        &self,
        _input: &CreateMcpToolInput,
    ) -> anyhow::Result<domain::McpToolRecord> {
        unreachable!("seed-only fixture does not create tools")
    }

    async fn update_mcp_tool(
        &self,
        _input: &UpdateMcpToolInput,
    ) -> anyhow::Result<domain::McpToolRecord> {
        unreachable!("seed-only fixture does not update tools")
    }

    async fn refresh_mcp_tool_des_id(
        &self,
        _workspace_id: Uuid,
        _actor_user_id: Uuid,
        _tool_id: &str,
        _des_id: &str,
    ) -> anyhow::Result<domain::McpToolRecord> {
        unreachable!("seed-only fixture does not refresh tool descriptions")
    }

    async fn delete_mcp_tool(&self, _workspace_id: Uuid, _tool_id: &str) -> anyhow::Result<()> {
        unreachable!("seed-only fixture uses direct graph removal")
    }

    async fn list_mcp_tool_bindings(
        &self,
        _instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpToolBindingRecord>> {
        unreachable!("seed-only fixture does not list bindings")
    }

    async fn create_mcp_tool_binding(
        &self,
        _input: &CreateMcpToolBindingInput,
    ) -> anyhow::Result<domain::McpToolBindingRecord> {
        unreachable!("seed-only fixture does not create bindings")
    }

    async fn update_mcp_tool_binding(
        &self,
        _input: &UpdateMcpToolBindingInput,
    ) -> anyhow::Result<domain::McpToolBindingRecord> {
        unreachable!("seed-only fixture does not update bindings")
    }

    async fn delete_mcp_tool_binding(
        &self,
        _workspace_id: Uuid,
        _binding_id: Uuid,
    ) -> anyhow::Result<()> {
        unreachable!("seed-only fixture does not delete bindings")
    }

    async fn list_mcp_instance_discovery_policies(
        &self,
        _instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpInstanceDiscoveryPolicyRecord>> {
        unreachable!("seed-only fixture does not list policies")
    }

    async fn get_mcp_instance_discovery_policy(
        &self,
        _instance_record_id: Uuid,
    ) -> anyhow::Result<Option<domain::McpInstanceDiscoveryPolicyRecord>> {
        unreachable!("seed-only fixture does not read policies")
    }

    async fn update_mcp_instance_discovery_policy(
        &self,
        _input: &UpdateMcpInstanceDiscoveryPolicyInput,
    ) -> anyhow::Result<domain::McpInstanceDiscoveryPolicyRecord> {
        unreachable!("seed-only fixture does not update policies")
    }
}

fn managed_frontstage_package(version: &str) -> domain::McpBundlePackage {
    let tool = |capability_code: &str| domain::McpBundleTool {
        tool_id: format!("frontstage_{capability_code}"),
        name: capability_code.into(),
        short_description: capability_code.into(),
        full_description: capability_code.into(),
        execution_target: domain::McpToolExecutionTarget::AssistantClient {
            capability_code: capability_code.into(),
        },
        parameter_schema_snapshot: serde_json::json!({"type":"object"}),
        result_schema_snapshot: serde_json::json!({"type":"object"}),
        input_mapping: serde_json::json!({}),
        output_mapping: serde_json::json!({}),
        permission_code_snapshot: None,
        risk_level_snapshot: domain::McpRiskLevel::Low,
        status: domain::McpToolStatus::Enabled,
    };
    let tools = vec![tool("list_page_blocks"), tool("inspect_block_render")];
    domain::McpBundlePackage {
        manifest: domain::McpBundleManifest {
            schema_version: domain::MCP_BUNDLE_SCHEMA_VERSION.into(),
            organization: "1flowbase".into(),
            bundle_id: "frontstage_assistant".into(),
            bundle_version: version.into(),
            locale: "zh_Hans".into(),
            minimum_host_version: "0.3.6".into(),
            exported_from_system_version: "0.3.6".into(),
            exported_at: "2026-08-17T02:00:00Z".into(),
            files: Vec::new(),
        },
        instances: vec![domain::McpBundleInstance {
            instance_id: "frontstage_browser".into(),
            name: "Frontstage Browser".into(),
            description_short: Some("Managed".into()),
            status: domain::McpInstanceStatus::Enabled,
            default_entry_path: "/frontstage".into(),
            groups: vec![domain::McpBundleGroup {
                path: "/frontstage".into(),
                display_name: "Frontstage".into(),
                description_short: None,
                enabled: true,
                sort_order: 0,
            }],
            bindings: tools
                .iter()
                .enumerate()
                .map(|(index, tool)| domain::McpBundleToolBinding {
                    group_path: "/frontstage".into(),
                    tool_id: tool.tool_id.clone(),
                    display_alias: None,
                    visible: true,
                    sort_order: index as i32,
                })
                .collect(),
            discovery_policy: domain::McpBundleInstanceDiscoveryPolicy {
                list_default_limit: 20,
                list_max_depth: 3,
                list_regex_enabled: false,
                list_regex_max_length: 64,
                list_return_fields: serde_json::json!(["id", "name"]),
            },
        }],
        tools,
        connections: Vec::new(),
    }
}

#[tokio::test]
async fn builtin_seed_respects_version_receipt_after_managed_graph_is_deleted() {
    let repository = SeedOnlyMcpRepository::default();
    let service = McpManagementService::new(repository.clone());
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let package = managed_frontstage_package("1.0.2");
    let command = || SeedBuiltinMcpBundleCommand {
        actor_user_id,
        workspace_id,
        package: package.clone(),
        interface_catalog: Vec::new(),
    };

    service.seed_builtin_bundle_once(command()).await.unwrap();
    let seeded = repository.snapshot();
    assert_eq!(seeded.tool_sources.len(), 2);
    assert_eq!(seeded.instance_sources.len(), 1);
    assert_eq!(seeded.binding_count, 2);
    assert!(seeded.tool_sources.iter().all(|source| {
        source.organization == "1flowbase"
            && source.bundle_id == "frontstage_assistant"
            && source.bundle_version == "1.0.2"
    }));
    assert_eq!(seeded.instance_sources, seeded.tool_sources[..1]);

    repository.remove_managed_graph();
    service.seed_builtin_bundle_once(command()).await.unwrap();
    let after_restart = repository.snapshot();
    assert!(after_restart.tool_sources.is_empty());
    assert!(after_restart.instance_sources.is_empty());
    assert_eq!(after_restart.binding_count, 0);
    assert_eq!(after_restart.applied_versions, vec!["1.0.2"]);
}

#[tokio::test]
async fn new_builtin_version_adds_managed_tools_once() {
    let repository = SeedOnlyMcpRepository::default();
    let service = McpManagementService::new(repository.clone());
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    service
        .seed_builtin_bundle_once(SeedBuiltinMcpBundleCommand {
            actor_user_id,
            workspace_id,
            package: managed_frontstage_package("1.0.2"),
            interface_catalog: Vec::new(),
        })
        .await
        .unwrap();

    let mut upgraded = managed_frontstage_package("1.1.0");
    let mut added_tool = upgraded.tools[0].clone();
    added_tool.tool_id = "frontstage_read_block_source_fragment".into();
    added_tool.name = "read_block_source_fragment".into();
    upgraded.tools.push(added_tool);
    upgraded.instances[0]
        .bindings
        .push(domain::McpBundleToolBinding {
            group_path: "/frontstage".into(),
            tool_id: "frontstage_read_block_source_fragment".into(),
            display_alias: None,
            visible: true,
            sort_order: 2,
        });
    let command = || SeedBuiltinMcpBundleCommand {
        actor_user_id,
        workspace_id,
        package: upgraded.clone(),
        interface_catalog: Vec::new(),
    };
    service.seed_builtin_bundle_once(command()).await.unwrap();
    service.seed_builtin_bundle_once(command()).await.unwrap();

    let snapshot = repository.snapshot();
    assert_eq!(snapshot.tool_sources.len(), 3);
    assert_eq!(snapshot.binding_count, 3);
    assert!(snapshot
        .tool_sources
        .iter()
        .all(|source| source.bundle_version == "1.1.0"));
    assert!(snapshot
        .instance_sources
        .iter()
        .all(|source| source.bundle_version == "1.1.0"));
    assert_eq!(snapshot.applied_versions, vec!["1.0.2", "1.1.0"]);
}

#[test]
fn compares_export_source_and_current_system_versions() {
    assert_eq!(
        compare_system_versions("0.2.5", "0.2.6"),
        domain::McpBundleVersionStatus::ExportedFromOlderSystem
    );
    assert_eq!(
        compare_system_versions("0.3.0", "0.2.6"),
        domain::McpBundleVersionStatus::ExportedFromNewerSystem
    );
    assert_eq!(
        compare_system_versions("latest", "0.2.6"),
        domain::McpBundleVersionStatus::UnknownSystemVersion
    );
}

#[test]
fn ac_002_ac_004_official_builtin_export_filters_workspace_capabilities_and_fails_unknown() {
    let sources = BTreeMap::from([
        (
            "static_api".to_string(),
            domain::McpInterfaceCatalogSource::StaticApi,
        ),
        (
            "builtin_crud".to_string(),
            domain::McpInterfaceCatalogSource::BuiltinDataModelCrud,
        ),
        (
            "workflow".to_string(),
            domain::McpInterfaceCatalogSource::PublishedWorkflow,
        ),
        (
            "workspace_crud".to_string(),
            domain::McpInterfaceCatalogSource::WorkspaceDataModelCrud,
        ),
    ]);

    for interface_id in ["static_api", "builtin_crud"] {
        assert_eq!(
            official_builtin_interface_disposition(interface_id, &sources).unwrap(),
            OfficialBuiltinInterfaceDisposition::Include
        );
    }
    assert_eq!(
        official_builtin_interface_disposition("workflow", &sources).unwrap(),
        OfficialBuiltinInterfaceDisposition::Exclude("published_workflow")
    );
    assert_eq!(
        official_builtin_interface_disposition("workspace_crud", &sources).unwrap(),
        OfficialBuiltinInterfaceDisposition::Exclude("workspace_data_model_crud")
    );
    assert!(official_builtin_interface_disposition("missing", &sources).is_err());
}

#[test]
fn ac_002_official_builtin_export_removes_excluded_tools_and_their_bindings() {
    let mut tools = vec![
        tool(
            1,
            "static_api",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "static_api".into(),
            },
        ),
        tool(
            2,
            "builtin_crud",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "builtin_crud".into(),
            },
        ),
        tool(
            3,
            "workflow",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "workflow".into(),
            },
        ),
        tool(
            4,
            "workspace_crud",
            domain::McpToolExecutionTarget::InterfaceWrapper {
                interface_id: "workspace_crud".into(),
            },
        ),
        tool(
            5,
            "proxy",
            domain::McpToolExecutionTarget::McpProxy {
                upstream_connection_id: Uuid::from_u128(99),
                remote_tool_name: "remote".into(),
                source_schema_hash: "hash".into(),
            },
        ),
    ];
    let mut bindings = vec![
        binding(1, "static_api"),
        binding(2, "builtin_crud"),
        binding(3, "workflow"),
        binding(4, "workspace_crud"),
        binding(5, "proxy"),
    ];
    let catalog = vec![
        interface("static_api", domain::McpInterfaceCatalogSource::StaticApi),
        interface(
            "builtin_crud",
            domain::McpInterfaceCatalogSource::BuiltinDataModelCrud,
        ),
        interface(
            "workflow",
            domain::McpInterfaceCatalogSource::PublishedWorkflow,
        ),
        interface(
            "workspace_crud",
            domain::McpInterfaceCatalogSource::WorkspaceDataModelCrud,
        ),
    ];

    let report = retain_official_builtin_tools(&mut tools, &mut bindings, &catalog).unwrap();

    assert_eq!(
        tools
            .iter()
            .map(|tool| tool.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["static_api", "builtin_crud", "proxy"]
    );
    assert_eq!(
        bindings
            .iter()
            .map(|binding| binding.tool_id.as_str())
            .collect::<Vec<_>>(),
        vec!["static_api", "builtin_crud", "proxy"]
    );
    assert_eq!(report.excluded_tool_count, 2);
    assert_eq!(
        report.exclusion_reasons,
        vec!["published_workflow", "workspace_data_model_crud"]
    );
}
