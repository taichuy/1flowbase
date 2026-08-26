use std::{
    collections::{BTreeMap, BTreeSet},
    sync::{
        atomic::{AtomicBool, Ordering},
        Arc, Mutex,
    },
};

use async_trait::async_trait;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    mcp_bundle::{
        compare_system_versions, official_builtin_interface_disposition,
        retain_official_builtin_tools, ExportMcpInstanceBundleCommand, McpInstanceBundleExportKind,
        OfficialBuiltinInterfaceDisposition, SeedBuiltinMcpBundleCommand,
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

struct McpBundleExportSnapshot {
    actor: domain::ActorContext,
    instances: Vec<domain::McpInstanceRecord>,
    groups: Vec<domain::McpGroupRecord>,
    bindings: Vec<domain::McpToolBindingRecord>,
    policies: Vec<domain::McpInstanceDiscoveryPolicyRecord>,
    tools: Vec<domain::McpToolRecord>,
    connections: Vec<domain::McpUpstreamConnectionRecord>,
}

#[derive(Clone, Default)]
struct McpBundleFixtureRepository {
    state: Arc<Mutex<SeedGraphState>>,
    export_snapshot: Option<Arc<McpBundleExportSnapshot>>,
    replace_inputs: Arc<Mutex<Vec<ReplaceMcpBundleGraphInput>>>,
    fail_replace: Arc<AtomicBool>,
}

impl McpBundleFixtureRepository {
    fn with_export_snapshot(snapshot: McpBundleExportSnapshot) -> Self {
        Self {
            state: Arc::default(),
            export_snapshot: Some(Arc::new(snapshot)),
            replace_inputs: Arc::default(),
            fail_replace: Arc::default(),
        }
    }

    fn export_snapshot(&self) -> &McpBundleExportSnapshot {
        self.export_snapshot
            .as_deref()
            .expect("export fixture must provide a workspace snapshot")
    }

    fn replace_inputs(&self) -> Vec<ReplaceMcpBundleGraphInput> {
        self.replace_inputs
            .lock()
            .expect("replace input recording lock must not be poisoned")
            .clone()
    }

    fn reject_replace(&self) {
        self.fail_replace.store(true, Ordering::SeqCst);
    }

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
impl McpManagementRepository for McpBundleFixtureRepository {
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
        actor_user_id: Uuid,
    ) -> anyhow::Result<domain::ActorContext> {
        let actor = &self.export_snapshot().actor;
        assert_eq!(actor.user_id, actor_user_id);
        Ok(actor.clone())
    }

    async fn list_mcp_instances(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpInstanceRecord>> {
        Ok(self
            .export_snapshot()
            .instances
            .iter()
            .filter(|instance| instance.workspace_id == workspace_id)
            .cloned()
            .collect())
    }

    async fn get_mcp_instance(
        &self,
        workspace_id: Uuid,
        instance_id: &str,
    ) -> anyhow::Result<Option<domain::McpInstanceRecord>> {
        Ok(self
            .export_snapshot()
            .instances
            .iter()
            .find(|instance| {
                instance.workspace_id == workspace_id && instance.instance_id == instance_id
            })
            .cloned())
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
        input: &ReplaceMcpBundleGraphInput,
    ) -> anyhow::Result<()> {
        if self.fail_replace.load(Ordering::SeqCst) {
            anyhow::bail!("replace rejected by fixture")
        }
        self.replace_inputs
            .lock()
            .expect("replace input recording lock must not be poisoned")
            .push(input.clone());
        Ok(())
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
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpUpstreamConnectionRecord>> {
        Ok(self
            .export_snapshot()
            .connections
            .iter()
            .filter(|connection| connection.workspace_id == workspace_id)
            .cloned()
            .collect())
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
        instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpGroupRecord>> {
        Ok(self
            .export_snapshot()
            .groups
            .iter()
            .filter(|group| instance_record_ids.contains(&group.instance_record_id))
            .cloned()
            .collect())
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
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::McpToolRecord>> {
        Ok(self
            .export_snapshot()
            .tools
            .iter()
            .filter(|tool| tool.workspace_id == workspace_id)
            .cloned()
            .collect())
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
        instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpToolBindingRecord>> {
        Ok(self
            .export_snapshot()
            .bindings
            .iter()
            .filter(|binding| instance_record_ids.contains(&binding.instance_record_id))
            .cloned()
            .collect())
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
        instance_record_ids: &[Uuid],
    ) -> anyhow::Result<Vec<domain::McpInstanceDiscoveryPolicyRecord>> {
        Ok(self
            .export_snapshot()
            .policies
            .iter()
            .filter(|policy| instance_record_ids.contains(&policy.instance_record_id))
            .cloned()
            .collect())
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
    let repository = McpBundleFixtureRepository::default();
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
    let repository = McpBundleFixtureRepository::default();
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

#[tokio::test]
async fn instance_export_includes_only_the_bound_tools_connection_dependencies() {
    let actor_user_id = Uuid::from_u128(10);
    let workspace_id = Uuid::from_u128(20);
    let selected_instance_record_id = Uuid::from_u128(30);
    let unrelated_instance_record_id = Uuid::from_u128(31);
    let selected_connection_id = Uuid::from_u128(40);
    let unrelated_connection_id = Uuid::from_u128(41);

    let instance = |id, instance_id: &str| domain::McpInstanceRecord {
        id,
        workspace_id,
        instance_id: instance_id.into(),
        name: instance_id.into(),
        description_short: None,
        status: domain::McpInstanceStatus::Enabled,
        default_entry_path: "/".into(),
        managed_by: None,
        created_by: actor_user_id,
        updated_by: actor_user_id,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let policy = |id, instance_record_id| domain::McpInstanceDiscoveryPolicyRecord {
        id,
        workspace_id,
        instance_record_id,
        list_default_limit: 100,
        list_max_depth: 8,
        list_regex_enabled: true,
        list_regex_max_length: 256,
        list_return_fields: serde_json::json!(["name", "description"]),
        created_by: actor_user_id,
        updated_by: actor_user_id,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let connection = |id, name: &str| domain::McpUpstreamConnectionRecord {
        id,
        workspace_id,
        name: name.into(),
        endpoint: format!("https://{name}.example.com/rpc"),
        transport: domain::McpUpstreamTransport::StreamableHttp,
        auth_type: domain::McpUpstreamAuthType::None,
        custom_header_name: None,
        status: domain::McpUpstreamConnectionStatus::Enabled,
        credentials_configured: false,
        last_connected_at: None,
        last_discovered_at: None,
        last_error: None,
        created_by: actor_user_id,
        updated_by: actor_user_id,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let proxy_tool = |id, tool_id: &str, upstream_connection_id| {
        let mut record = tool(
            id,
            tool_id,
            domain::McpToolExecutionTarget::McpProxy {
                upstream_connection_id,
                remote_tool_name: format!("{tool_id}.lookup"),
                source_schema_hash: format!("{tool_id}-schema"),
            },
        );
        record.workspace_id = workspace_id;
        record.created_by = actor_user_id;
        record.updated_by = actor_user_id;
        record
    };
    let scoped_binding = |id, instance_record_id, tool_id: &str| {
        let mut record = binding(id, tool_id);
        record.instance_record_id = instance_record_id;
        record.created_by = actor_user_id;
        record.updated_by = actor_user_id;
        record
    };

    let repository = McpBundleFixtureRepository::with_export_snapshot(McpBundleExportSnapshot {
        actor: domain::ActorContext::root(actor_user_id, workspace_id, "root"),
        instances: vec![
            instance(selected_instance_record_id, "selected_instance"),
            instance(unrelated_instance_record_id, "unrelated_instance"),
        ],
        groups: Vec::new(),
        bindings: vec![
            scoped_binding(1, selected_instance_record_id, "selected_tool"),
            scoped_binding(2, unrelated_instance_record_id, "unrelated_tool"),
        ],
        policies: vec![
            policy(Uuid::from_u128(50), selected_instance_record_id),
            policy(Uuid::from_u128(51), unrelated_instance_record_id),
        ],
        tools: vec![
            proxy_tool(1, "selected_tool", selected_connection_id),
            proxy_tool(2, "unrelated_tool", unrelated_connection_id),
        ],
        connections: vec![
            connection(selected_connection_id, "selected"),
            connection(unrelated_connection_id, "unrelated"),
        ],
    });
    let bundle = McpManagementService::new(repository)
        .export_instance_bundle(ExportMcpInstanceBundleCommand {
            actor_user_id,
            instance_id: "selected_instance".into(),
            organization: "taichuy".into(),
            bundle_id: "selected_instance".into(),
            bundle_version: "1.0.0".into(),
            locale: "zh_Hans".into(),
            current_system_version: "0.2.6".into(),
            kind: McpInstanceBundleExportKind::Portable,
        })
        .await
        .unwrap()
        .package;

    assert_eq!(bundle.instances.len(), 1);
    assert_eq!(bundle.instances[0].instance_id, "selected_instance");
    assert_eq!(bundle.tools.len(), 1);
    assert_eq!(bundle.tools[0].tool_id, "selected_tool");
    assert_eq!(bundle.connections.len(), 1);
    assert_eq!(bundle.connections[0].connection_id, selected_connection_id);
}

#[tokio::test]
async fn bundle_preview_and_import_preserve_shared_tool_impact_and_atomic_replace_input() {
    let actor_user_id = Uuid::from_u128(110);
    let workspace_id = Uuid::from_u128(120);
    let selected_instance_record_id = Uuid::from_u128(130);
    let other_instance_record_id = Uuid::from_u128(131);
    let mut package = managed_frontstage_package("1.0.2");
    package.instances[0].name = "New name".into();
    package.tools[0].name = "New tool".into();

    let instance = |id, instance_id: &str, name: &str| domain::McpInstanceRecord {
        id,
        workspace_id,
        instance_id: instance_id.into(),
        name: name.into(),
        description_short: Some("Managed".into()),
        status: domain::McpInstanceStatus::Enabled,
        default_entry_path: "/frontstage".into(),
        managed_by: None,
        created_by: actor_user_id,
        updated_by: actor_user_id,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let policy = |id, instance_record_id| domain::McpInstanceDiscoveryPolicyRecord {
        id,
        workspace_id,
        instance_record_id,
        list_default_limit: 20,
        list_max_depth: 3,
        list_regex_enabled: false,
        list_regex_max_length: 64,
        list_return_fields: serde_json::json!(["id", "name"]),
        created_by: actor_user_id,
        updated_by: actor_user_id,
        created_at: OffsetDateTime::UNIX_EPOCH,
        updated_at: OffsetDateTime::UNIX_EPOCH,
    };
    let record_from_bundle = |id: u128, bundle: &domain::McpBundleTool| {
        let mut record = tool(id, &bundle.tool_id, bundle.execution_target.clone());
        record.workspace_id = workspace_id;
        record.name = bundle.name.clone();
        record.short_description = bundle.short_description.clone();
        record.full_description = bundle.full_description.clone();
        record.parameter_schema = bundle.parameter_schema_snapshot.clone();
        record.result_schema = bundle.result_schema_snapshot.clone();
        record.input_mapping = bundle.input_mapping.clone();
        record.output_mapping = bundle.output_mapping.clone();
        record.permission_code = bundle.permission_code_snapshot.clone();
        record.risk_level = bundle.risk_level_snapshot;
        record.created_by = actor_user_id;
        record.updated_by = actor_user_id;
        record
    };
    let mut first_tool = record_from_bundle(1, &package.tools[0]);
    first_tool.name = "Old tool".into();
    let second_tool = record_from_bundle(2, &package.tools[1]);
    let scoped_binding = |id, instance_record_id, tool_id: &str| {
        let mut record = binding(id, tool_id);
        record.instance_record_id = instance_record_id;
        record.created_by = actor_user_id;
        record.updated_by = actor_user_id;
        record
    };
    let repository = McpBundleFixtureRepository::with_export_snapshot(McpBundleExportSnapshot {
        actor: domain::ActorContext::root(actor_user_id, workspace_id, "root"),
        instances: vec![
            instance(
                selected_instance_record_id,
                "frontstage_browser",
                "Old name",
            ),
            instance(other_instance_record_id, "keep_me", "Keep me"),
        ],
        groups: vec![domain::McpGroupRecord {
            id: Uuid::from_u128(140),
            instance_record_id: selected_instance_record_id,
            path: "/frontstage".into(),
            display_name: "Frontstage".into(),
            description_short: None,
            enabled: true,
            sort_order: 0,
            created_by: actor_user_id,
            updated_by: actor_user_id,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }],
        bindings: vec![
            scoped_binding(
                1,
                selected_instance_record_id,
                "frontstage_list_page_blocks",
            ),
            scoped_binding(
                2,
                selected_instance_record_id,
                "frontstage_inspect_block_render",
            ),
            scoped_binding(1, other_instance_record_id, "frontstage_list_page_blocks"),
        ],
        policies: vec![
            policy(Uuid::from_u128(150), selected_instance_record_id),
            policy(Uuid::from_u128(151), other_instance_record_id),
        ],
        tools: vec![first_tool, second_tool],
        connections: Vec::new(),
    });
    let service = McpManagementService::new(repository.clone());

    let preview = service
        .preview_bundle(crate::mcp_bundle::PreviewMcpBundleCommand {
            actor_user_id,
            package: package.clone(),
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .unwrap();
    assert_eq!(
        preview.instances[0].effect,
        domain::McpBundleItemEffect::Update
    );
    assert_eq!(preview.tools[0].effect, domain::McpBundleItemEffect::Update);
    assert_eq!(
        preview.shared_tool_impacts,
        vec![domain::McpBundleSharedToolImpact {
            tool_id: "frontstage_list_page_blocks".into(),
            instance_ids: vec!["keep_me".into()],
        }]
    );

    service
        .import_bundle(crate::mcp_bundle::ImportMcpBundleCommand {
            actor_user_id,
            package: package.clone(),
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .unwrap();
    let inputs = repository.replace_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].actor_user_id, actor_user_id);
    assert_eq!(inputs[0].workspace_id, workspace_id);
    assert_eq!(inputs[0].instances[0].name, "New name");
    assert_eq!(inputs[0].tools[0].name, "New tool");

    let mut invalid_package = package;
    invalid_package.instances[0].bindings[0].tool_id = "missing_tool".into();
    repository.reject_replace();
    assert!(service
        .import_bundle(crate::mcp_bundle::ImportMcpBundleCommand {
            actor_user_id,
            package: invalid_package,
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .is_err());
    assert_eq!(repository.replace_inputs().len(), 1);
}

#[tokio::test]
async fn explicit_bundle_import_restores_a_missing_managed_graph_through_atomic_replace() {
    let actor_user_id = Uuid::from_u128(210);
    let workspace_id = Uuid::from_u128(220);
    let repository = McpBundleFixtureRepository::with_export_snapshot(McpBundleExportSnapshot {
        actor: domain::ActorContext::root(actor_user_id, workspace_id, "root"),
        instances: Vec::new(),
        groups: Vec::new(),
        bindings: Vec::new(),
        policies: Vec::new(),
        tools: Vec::new(),
        connections: Vec::new(),
    });

    McpManagementService::new(repository.clone())
        .import_bundle(crate::mcp_bundle::ImportMcpBundleCommand {
            actor_user_id,
            package: managed_frontstage_package("1.0.2"),
            interface_catalog: Vec::new(),
            current_system_version: "0.3.6".into(),
        })
        .await
        .unwrap();

    let inputs = repository.replace_inputs();
    assert_eq!(inputs.len(), 1);
    assert_eq!(inputs[0].source.organization, "1flowbase");
    assert_eq!(inputs[0].source.bundle_id, "frontstage_assistant");
    assert_eq!(inputs[0].instances[0].instance_id, "frontstage_browser");
    assert_eq!(inputs[0].tools.len(), 2);
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
