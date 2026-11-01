use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    frontend_block_catalog::{
        FrontendBlockCatalogService, ListFrontendBlockCatalogQuery,
        FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID, FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION,
        FRONTEND_BLOCK_CONTRIBUTION_POINT_ID, FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
        FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION,
    },
    js_dependency::{JsDependencyService, ListWorkspaceJsDependenciesQuery},
    node_contribution::{
        ApplicationNodeAuthoringStatus, ApplicationNodeCatalogService,
        ApplicationNodeRuntimeStatus, ApplicationNodeSourceKind, ListApplicationNodesQuery,
    },
    ports::{
        AuthRepository, FrontendBlockCatalogDefaultTemplate, FrontendBlockCatalogRepository,
        FrontendBlockCatalogTemplateRevision, JsDependencyRepository, NodeContributionRepository,
        PluginRepository, ReplaceInstallationFrontendBlocksInput,
        ReplaceInstallationJsDependenciesInput, RoleConsolePolicyReader,
    },
};
use domain::{ActorContext, NodeContributionDependencyStatus, NodeContributionRegistryEntry};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::plugin_management::support::actor_with_permissions;

fn frontend_contribution_graph() -> Arc<plugin_framework::extension_bus::EffectiveExtensionGraph> {
    use std::collections::BTreeSet;

    use plugin_framework::extension_bus::{
        compile_extension_graph, Cardinality, ContractDescriptor, DeliverySemantics,
        ExtensionBusVersion, ExtensionPointDescriptor, ExtensionPointId, ExtensionPointKind,
        FailureSemantics, LifecycleSemantics, ModuleActivationDeclaration, ModuleDescriptor,
        ModuleId, ModuleKind, ModuleVersion, OrderingSemantics, OverridePolicy, PermissionCode,
        ScopeSemantics,
    };

    Arc::new(
        compile_extension_graph(vec![ModuleDescriptor {
            bus_version: ExtensionBusVersion::V1,
            module_id: ModuleId::new("1flowbase.boot-core").unwrap(),
            module_version: ModuleVersion::new("1").unwrap(),
            module_kind: ModuleKind::BootCore,
            activation: ModuleActivationDeclaration::Active,
            dependencies: BTreeSet::new(),
            granted_permissions: BTreeSet::new(),
            extension_points: vec![ExtensionPointDescriptor {
                point_id: ExtensionPointId::new(FRONTEND_BLOCK_CONTRIBUTION_POINT_ID).unwrap(),
                owner_module_id: ModuleId::new("1flowbase.boot-core").unwrap(),
                point_kind: ExtensionPointKind::Contribution,
                contract: ContractDescriptor::new(
                    FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID,
                    FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_VERSION,
                )
                .unwrap(),
                scope: ScopeSemantics::Workspace,
                cardinality: Cardinality::Many,
                ordering: OrderingSemantics::Lexicographic,
                failure: FailureSemantics::FailClosed,
                delivery: DeliverySemantics::Synchronous,
                lifecycle: LifecycleSemantics::WorkspaceAssignment,
                allowed_permissions: [
                    FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION,
                    FRONTEND_BLOCK_ISOLATED_UI_MOUNT_PERMISSION,
                ]
                .into_iter()
                .map(|permission| PermissionCode::new(permission).unwrap())
                .collect(),
                override_policy: OverridePolicy::Sealed,
            }],
            contributions: Vec::new(),
        }])
        .unwrap(),
    )
}

#[derive(Clone)]
struct MemoryNodeContributionRepository {
    actor: ActorContext,
    entries: Arc<RwLock<Vec<NodeContributionRegistryEntry>>>,
    console_policies: Vec<domain::RoleConsolePolicy>,
    frontend: Arc<RwLock<FrontendCatalogFixture>>,
}

#[derive(Default)]
struct FrontendCatalogFixture {
    blocks: Vec<domain::FrontendBlockCatalogEntry>,
    defaults: Vec<FrontendBlockCatalogDefaultTemplate>,
    installations: Vec<domain::PluginInstallationRecord>,
    artifacts: Vec<domain::PluginArtifactInstanceRecord>,
    assignments: Vec<domain::PluginAssignmentRecord>,
    reads: Vec<String>,
    fail_on: Option<String>,
}

impl FrontendCatalogFixture {
    fn read(&mut self, operation: String) -> Result<()> {
        self.reads.push(operation.clone());
        if self.fail_on.as_ref() == Some(&operation) {
            anyhow::bail!("fixture read failed: {operation}");
        }
        Ok(())
    }
}

impl MemoryNodeContributionRepository {
    fn new(actor: ActorContext, entries: Vec<NodeContributionRegistryEntry>) -> Self {
        Self {
            actor,
            entries: Arc::new(RwLock::new(entries)),
            console_policies: Vec::new(),
            frontend: Arc::new(RwLock::new(FrontendCatalogFixture::default())),
        }
    }

    fn with_console_operation(mut self, group_id: &str, operation_id: &str) -> Self {
        let group = domain::ConsolePolicyGroup::other(group_id)
            .expect("plugin catalog policy group must be valid");
        self.console_policies = vec![domain::RoleConsolePolicy::new(
            Uuid::now_v7(),
            vec![domain::RoleConsoleGroupPolicy::custom(
                group,
                vec![domain::ConsoleOperationPolicy::simple(
                    domain::ConsoleOperationId::try_from(operation_id)
                        .expect("node contribution operation id must be valid"),
                    true,
                )],
            )],
        )];
        self
    }
}

#[async_trait]
impl AuthRepository for MemoryNodeContributionRepository {
    async fn find_authentication_connection(
        &self,
        _id: Uuid,
    ) -> Result<Option<domain::AuthenticationConnectionRecord>> {
        anyhow::bail!("authentication connections are not used by contribution tests")
    }

    async fn find_user_for_verified_external_identity(
        &self,
        _identity: &domain::VerifiedExternalIdentity,
    ) -> Result<Option<domain::UserRecord>> {
        anyhow::bail!("external identities are not used by contribution tests")
    }

    async fn bind_verified_external_identity(
        &self,
        _user_id: Uuid,
        _identity: &domain::VerifiedExternalIdentity,
        _audit: &domain::AuditLogRecord,
    ) -> Result<domain::UserAuthIdentity> {
        anyhow::bail!("external identities are not used by contribution tests")
    }

    async fn find_login_entry(&self, _id: Uuid) -> Result<Option<domain::LoginEntryRecord>> {
        Ok(None)
    }

    async fn find_user_for_password_login(
        &self,
        _connection_id: Uuid,
        _identifier: &str,
    ) -> Result<Option<domain::UserRecord>> {
        Ok(None)
    }

    async fn find_user_by_id(&self, _user_id: Uuid) -> Result<Option<domain::UserRecord>> {
        Ok(None)
    }

    async fn default_scope_for_user(&self, _user_id: Uuid) -> Result<domain::ScopeContext> {
        Ok(domain::ScopeContext {
            tenant_id: self.actor.tenant_id,
            workspace_id: self.actor.current_workspace_id,
        })
    }

    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        self.load_actor_context(
            actor_user_id,
            self.actor.tenant_id,
            self.actor.current_workspace_id,
            None,
        )
        .await
    }

    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        _display_role: Option<&str>,
    ) -> Result<ActorContext> {
        let mut actor = self.actor.clone();
        actor.user_id = user_id;
        actor.tenant_id = tenant_id;
        actor.current_workspace_id = workspace_id;
        Ok(actor)
    }

    async fn update_password_hash(
        &self,
        _user_id: Uuid,
        _password_hash: &str,
        _actor_id: Uuid,
    ) -> Result<i64> {
        Ok(1)
    }

    async fn update_profile(
        &self,
        _input: &control_plane::ports::UpdateProfileInput,
    ) -> Result<domain::UserRecord> {
        anyhow::bail!("not implemented")
    }

    async fn update_user_meta(
        &self,
        _input: &control_plane::ports::UpdateUserMetaInput,
    ) -> Result<domain::UserRecord> {
        anyhow::bail!("not implemented")
    }

    async fn bump_session_version(&self, _user_id: Uuid, _actor_id: Uuid) -> Result<i64> {
        Ok(1)
    }

    async fn list_permissions(&self) -> Result<Vec<domain::PermissionDefinition>> {
        Ok(Vec::new())
    }

    async fn append_audit_log(&self, _event: &domain::AuditLogRecord) -> Result<()> {
        Ok(())
    }
}

#[async_trait]
impl NodeContributionRepository for MemoryNodeContributionRepository {
    async fn replace_installation_node_contributions(
        &self,
        _input: &control_plane::ports::ReplaceInstallationNodeContributionsInput,
    ) -> Result<()> {
        Ok(())
    }

    async fn list_node_contributions(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<NodeContributionRegistryEntry>> {
        Ok(self.entries.read().await.clone())
    }
}

#[async_trait]
impl RoleConsolePolicyReader for MemoryNodeContributionRepository {
    async fn load_role_console_policies_for_user(
        &self,
        _actor: &domain::ActorContext,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self.console_policies.clone())
    }
}

#[async_trait]
impl FrontendBlockCatalogRepository for MemoryNodeContributionRepository {
    async fn replace_installation_frontend_blocks(
        &self,
        _input: &ReplaceInstallationFrontendBlocksInput,
    ) -> Result<()> {
        Ok(())
    }

    async fn list_workspace_frontend_blocks(
        &self,
        _node_id: &str,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
        let mut state = self.frontend.write().await;
        state.read("blocks".into())?;
        Ok(state.blocks.clone())
    }

    async fn list_active_ui_code_templates_for_catalog(
        &self,
    ) -> Result<Vec<FrontendBlockCatalogDefaultTemplate>> {
        let mut state = self.frontend.write().await;
        state.read("defaults".into())?;
        Ok(state.defaults.clone())
    }
}

#[async_trait]
impl PluginRepository for MemoryNodeContributionRepository {
    async fn upsert_installation(
        &self,
        _input: &control_plane::ports::UpsertPluginInstallationInput,
    ) -> Result<domain::PluginInstallationRecord> {
        unimplemented!()
    }

    async fn get_installation(
        &self,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginInstallationRecord>> {
        let mut state = self.frontend.write().await;
        state.read(format!("installation:{installation_id}"))?;
        Ok(state
            .installations
            .iter()
            .find(|installation| installation.id == installation_id)
            .cloned())
    }

    async fn list_installations(&self) -> Result<Vec<domain::PluginInstallationRecord>> {
        Ok(Vec::new())
    }

    async fn upsert_plugin_package_catalog_projection(
        &self,
        _input: &control_plane::ports::UpsertPluginPackageCatalogProjectionInput,
    ) -> Result<domain::PluginPackageCatalogProjectionRecord> {
        unimplemented!()
    }

    async fn get_plugin_package_catalog_projection(
        &self,
        _installation_id: Uuid,
    ) -> Result<Option<domain::PluginPackageCatalogProjectionRecord>> {
        Ok(None)
    }

    async fn list_plugin_package_catalog_projections(
        &self,
    ) -> Result<Vec<domain::PluginPackageCatalogProjectionRecord>> {
        Ok(Vec::new())
    }

    async fn delete_installation(&self, _installation_id: Uuid) -> Result<()> {
        unimplemented!()
    }

    async fn list_pending_restart_host_extensions(
        &self,
    ) -> Result<Vec<domain::PluginInstallationRecord>> {
        Ok(Vec::new())
    }

    async fn update_desired_state(
        &self,
        _input: &control_plane::ports::UpdatePluginDesiredStateInput,
    ) -> Result<domain::PluginInstallationRecord> {
        unimplemented!()
    }

    async fn upsert_artifact_instance(
        &self,
        _input: &control_plane::ports::UpsertPluginArtifactInstanceInput,
    ) -> Result<domain::PluginArtifactInstanceRecord> {
        unimplemented!()
    }

    async fn get_artifact_instance(
        &self,
        node_id: &str,
        installation_id: Uuid,
    ) -> Result<Option<domain::PluginArtifactInstanceRecord>> {
        let mut state = self.frontend.write().await;
        state.read(format!("artifact:{node_id}:{installation_id}"))?;
        Ok(state
            .artifacts
            .iter()
            .find(|artifact| {
                artifact.installation_id == installation_id && artifact.node_id == node_id
            })
            .cloned())
    }

    async fn list_artifact_instances(
        &self,
        _node_id: &str,
    ) -> Result<Vec<domain::PluginArtifactInstanceRecord>> {
        Ok(Vec::new())
    }

    async fn create_assignment(
        &self,
        _input: &control_plane::ports::CreatePluginAssignmentInput,
    ) -> Result<domain::PluginAssignmentRecord> {
        unimplemented!()
    }

    async fn list_assignments(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::PluginAssignmentRecord>> {
        let mut state = self.frontend.write().await;
        state.read("assignments".into())?;
        Ok(state.assignments.clone())
    }

    async fn list_assigned_installation_ids(&self) -> Result<Vec<Uuid>> {
        Ok(Vec::new())
    }

    async fn create_task(
        &self,
        _input: &control_plane::ports::CreatePluginTaskInput,
    ) -> Result<domain::PluginTaskRecord> {
        unimplemented!()
    }

    async fn update_task_status(
        &self,
        _input: &control_plane::ports::UpdatePluginTaskStatusInput,
    ) -> Result<domain::PluginTaskRecord> {
        unimplemented!()
    }

    async fn get_task(&self, _task_id: Uuid) -> Result<Option<domain::PluginTaskRecord>> {
        Ok(None)
    }

    async fn list_tasks(&self) -> Result<Vec<domain::PluginTaskRecord>> {
        Ok(Vec::new())
    }
}

#[async_trait]
impl JsDependencyRepository for MemoryNodeContributionRepository {
    async fn replace_installation_js_dependencies(
        &self,
        _input: &ReplaceInstallationJsDependenciesInput,
    ) -> Result<()> {
        Ok(())
    }

    async fn list_workspace_js_dependencies(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::JsDependencyRegistryEntry>> {
        Ok(Vec::new())
    }
}

fn sample_entry(
    contribution_code: &str,
    status: NodeContributionDependencyStatus,
) -> NodeContributionRegistryEntry {
    NodeContributionRegistryEntry {
        installation_id: Uuid::now_v7(),
        provider_code: "prompt_pack".into(),
        plugin_unique_identifier: "prompt_pack".into(),
        package_id: "prompt_pack@0.1.0".into(),
        plugin_id: "prompt_pack@0.1.0".into(),
        plugin_version: "0.1.0".into(),
        contribution_code: contribution_code.into(),
        node_shell: "action".into(),
        category: "ai".into(),
        title: "OpenAI Prompt".into(),
        description: "Prompt node".into(),
        icon: "spark".into(),
        schema_ui: serde_json::json!({}),
        schema_version: "1flowbase.node-contribution/v2".into(),
        output_schema: serde_json::json!({
            "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
        }),
        contribution_checksum: "sha256:contribution".into(),
        compiled_contribution_hash: "sha256:compiled".into(),
        output_schema_snapshot: serde_json::json!({
            "outputs": [{ "key": "answer", "title": "Answer", "valueType": "string" }]
        }),
        side_effect_policy: "external_read".into(),
        infra_contracts: vec![],
        required_auth: vec!["provider_instance".into()],
        visibility: "public".into(),
        experimental: false,
        dependency_installation_kind: "required".into(),
        dependency_plugin_version_range: ">=0.1.0".into(),
        dependency_status: status,
    }
}

#[tokio::test]
async fn application_node_catalog_service_combines_type_boundaries_and_workspace_entries() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &[]),
        vec![sample_entry(
            "openai_prompt",
            NodeContributionDependencyStatus::Ready,
        )],
    )
    .with_console_operation("other.node-contributions", "node_contributions.view");
    let service = ApplicationNodeCatalogService::new(repository);

    let view = service
        .list_application_nodes(ListApplicationNodesQuery {
            actor_user_id: Uuid::now_v7(),
            application_type: domain::ApplicationType::AgentFlow,
        })
        .await
        .unwrap();

    assert!(view.nodes.iter().any(|node| node.node_type == "start"));
    assert!(view.nodes.iter().any(|node| node.node_type == "answer"));
    assert!(!view
        .nodes
        .iter()
        .any(|node| node.node_type == "workflow_start"));
    let plugin = view
        .nodes
        .iter()
        .find(|node| {
            node.source_kind == control_plane::node_contribution::ApplicationNodeSourceKind::Plugin
        })
        .unwrap();
    assert_eq!(
        plugin.plugin.as_ref().unwrap().contribution_code,
        "openai_prompt"
    );
    assert_eq!(plugin.dependency_status.as_str(), "ready");
}

#[tokio::test]
async fn ac_001_application_node_catalog_distinguishes_published_and_hidden_builtins() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &[]),
        Vec::new(),
    )
    .with_console_operation("other.node-contributions", "node_contributions.view");

    let view = ApplicationNodeCatalogService::new(repository)
        .list_application_nodes(ListApplicationNodesQuery {
            actor_user_id: Uuid::now_v7(),
            application_type: domain::ApplicationType::AgentFlow,
        })
        .await
        .unwrap();

    let hidden_types = view
        .nodes
        .iter()
        .filter(|node| node.authoring_status == ApplicationNodeAuthoringStatus::Hidden)
        .map(|node| node.node_type.as_str())
        .collect::<Vec<_>>();
    assert_eq!(
        hidden_types,
        vec![
            "knowledge_retrieval",
            "question_classifier",
            "tool",
            "parameter_extractor",
            "human_input",
        ]
    );

    for published_type in [
        "iteration",
        "loop",
        "start",
        "answer",
        "llm",
        "if_else",
        "code",
        "template_transform",
        "http_request",
        "tool_result",
        "data_model_list",
        "data_model_get",
        "data_model_create",
        "data_model_update",
        "data_model_delete",
        "sql",
        "variable_assigner",
    ] {
        assert_eq!(
            view.nodes
                .iter()
                .find(|node| node.node_type == published_type)
                .unwrap()
                .authoring_status,
            ApplicationNodeAuthoringStatus::Published,
            "{published_type} must remain published",
        );
    }
}

#[tokio::test]
async fn ac_005_ac_006_ac_007_variable_aggregator_is_shared_with_the_exact_public_contract() {
    for application_type in [
        domain::ApplicationType::AgentFlow,
        domain::ApplicationType::Workflow,
    ] {
        let workspace_id = Uuid::now_v7();
        let repository = MemoryNodeContributionRepository::new(
            actor_with_permissions(workspace_id, &[]),
            Vec::new(),
        )
        .with_console_operation("other.node-contributions", "node_contributions.view");

        let view = ApplicationNodeCatalogService::new(repository)
            .list_application_nodes(ListApplicationNodesQuery {
                actor_user_id: Uuid::now_v7(),
                application_type,
            })
            .await
            .unwrap();
        let aggregator = view
            .nodes
            .iter()
            .find(|node| node.node_type == "variable_aggregator")
            .expect("variable aggregator must be shared by both application catalogs");

        assert_eq!(aggregator.source_kind, ApplicationNodeSourceKind::Builtin);
        assert_eq!(aggregator.category, "control");
        assert_eq!(
            aggregator.authoring_status,
            ApplicationNodeAuthoringStatus::Published
        );
        assert_eq!(
            aggregator.runtime_status,
            ApplicationNodeRuntimeStatus::Ready
        );
        assert!(aggregator.field_contract.config_fields.is_empty());
        assert_eq!(aggregator.field_contract.input_fields.len(), 1);
        assert_eq!(
            aggregator.field_contract.input_fields[0].key,
            "bindings.groups"
        );
        assert!(aggregator.field_contract.input_fields[0].required);
        assert_eq!(
            aggregator.field_contract.input_fields[0].value_types,
            vec!["variable_groups"]
        );
        assert_eq!(aggregator.field_contract.output_fields.len(), 1);
        assert_eq!(aggregator.field_contract.output_fields[0].key, "outputs[]");
        assert!(aggregator.field_contract.output_fields[0].required);
        assert_eq!(
            aggregator.field_contract.output_fields[0].value_types,
            vec!["array"]
        );
    }
}

#[tokio::test]
async fn node_contribution_service_defaults_to_deny_without_console_operation() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &[]),
        vec![sample_entry(
            "openai_prompt",
            NodeContributionDependencyStatus::Ready,
        )],
    );
    let service = ApplicationNodeCatalogService::new(repository);

    let error = service
        .list_application_nodes(ListApplicationNodesQuery {
            actor_user_id: Uuid::now_v7(),
            application_type: domain::ApplicationType::Workflow,
        })
        .await
        .unwrap_err();

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied(_))
    ));
}

#[tokio::test]
async fn ac_1281_node_contributions_policy_only_allows_without_legacy_grant() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &[]),
        vec![sample_entry(
            "policy_only",
            NodeContributionDependencyStatus::Ready,
        )],
    )
    .with_console_operation("other.node-contributions", "node_contributions.view");

    let view = ApplicationNodeCatalogService::new(repository)
        .list_application_nodes(ListApplicationNodesQuery {
            actor_user_id: Uuid::now_v7(),
            application_type: domain::ApplicationType::AgentFlow,
        })
        .await
        .expect("the exact console operation must authorize the catalog owner");

    assert!(view.nodes.iter().any(|node| {
        node.plugin
            .as_ref()
            .is_some_and(|plugin| plugin.contribution_code == "policy_only")
    }));
}

#[tokio::test]
async fn ac_1281_node_contributions_legacy_only_does_not_authorize() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &["plugin_config.view.all"]),
        vec![sample_entry(
            "legacy_only",
            NodeContributionDependencyStatus::Ready,
        )],
    );

    let error = ApplicationNodeCatalogService::new(repository)
        .list_application_nodes(ListApplicationNodesQuery {
            actor_user_id: Uuid::now_v7(),
            application_type: domain::ApplicationType::AgentFlow,
        })
        .await
        .expect_err("legacy plugin_config grants must not authorize the compiled operation");

    assert!(matches!(
        error.downcast_ref::<ControlPlaneError>(),
        Some(ControlPlaneError::PermissionDenied(_))
    ));
}

#[tokio::test]
async fn ac_1281_frontend_blocks_policy_only_allows_without_legacy_grant() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &[]),
        Vec::new(),
    )
    .with_console_operation("other.frontend-blocks", "frontend_blocks.view");

    FrontendBlockCatalogService::new(repository, "test-node", frontend_contribution_graph())
        .unwrap()
        .list_frontend_blocks(ListFrontendBlockCatalogQuery {
            actor_user_id: Uuid::now_v7(),
        })
        .await
        .expect("the frontend block operation must authorize the catalog owner");
}

#[tokio::test]
async fn ac_1281_frontend_blocks_legacy_only_does_not_authorize() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &["plugin_config.view.all"]),
        Vec::new(),
    );

    assert!(FrontendBlockCatalogService::new(
        repository,
        "test-node",
        frontend_contribution_graph()
    )
    .unwrap()
    .list_frontend_blocks(ListFrontendBlockCatalogQuery {
        actor_user_id: Uuid::now_v7(),
    })
    .await
    .is_err());
}

#[tokio::test]
async fn ac_1281_js_dependencies_policy_only_allows_without_legacy_grant() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &[]),
        Vec::new(),
    )
    .with_console_operation("other.js-dependencies", "js_dependencies.view");

    JsDependencyService::new(repository)
        .list_workspace_js_dependencies(ListWorkspaceJsDependenciesQuery {
            actor_user_id: Uuid::now_v7(),
        })
        .await
        .expect("the JavaScript dependency operation must authorize the catalog owner");
}

#[tokio::test]
async fn ac_1281_js_dependencies_legacy_only_does_not_authorize() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &["plugin_config.view.all"]),
        Vec::new(),
    );

    assert!(JsDependencyService::new(repository)
        .list_workspace_js_dependencies(ListWorkspaceJsDependenciesQuery {
            actor_user_id: Uuid::now_v7(),
        })
        .await
        .is_err());
}

fn frontend_candidate(
    workspace_id: Uuid,
    version: &str,
) -> control_plane::frontend_block_catalog::FrontendContributionCandidate {
    let now = time::OffsetDateTime::UNIX_EPOCH;
    let id = Uuid::now_v7();
    let installation = domain::PluginInstallationRecord {
        id,
        scope_id: domain::SYSTEM_SCOPE_ID,
        category: domain::ExtensionCategory::CapabilityPlugins,
        organization: "fixture".into(),
        provider_code: "fixture.frontend".into(),
        plugin_id: format!("fixture.frontend@{version}"),
        plugin_version: version.into(),
        contract_version: "1flowbase.capability/v1".into(),
        protocol: "stdio_json".into(),
        display_name: "Frontend fixture".into(),
        source_kind: "uploaded".into(),
        trust_level: "checksum_only".into(),
        verification_status: domain::PluginVerificationStatus::Valid,
        desired_state: domain::PluginDesiredState::ActiveRequested,
        expected_checksum: Some("fixture-checksum".into()),
        signature_status: domain::ExtensionSignatureStatus::Missing,
        signature_algorithm: None,
        signing_key_id: None,
        legacy_manifest_compatibility: None,
        metadata_json: serde_json::json!({}),
        is_system_reserved: false,
        created_by: Uuid::nil(),
        updated_by: None,
        created_at: now,
        updated_at: now,
    };
    let artifact = domain::PluginArtifactInstanceRecord {
        node_id: "test-node".into(),
        installation_id: id,
        local_version: Some(version.into()),
        local_checksum: installation.expected_checksum.clone(),
        local_path: None,
        package_path: None,
        manifest_fingerprint: None,
        artifact_status: domain::PluginArtifactInstanceStatus::Ready,
        runtime_status: domain::PluginRuntimeStatus::Inactive,
        availability_status: domain::PluginAvailabilityStatus::Available,
        checked_at: now,
        last_error: None,
        is_current: true,
    };
    let assignment = domain::PluginAssignmentRecord {
        id: Uuid::now_v7(),
        installation_id: id,
        workspace_id,
        provider_code: installation.provider_code.clone(),
        assigned_by: Uuid::nil(),
        created_at: now,
    };
    let catalog_entry = domain::FrontendBlockCatalogEntry {
        installation_id: id,
        provider_code: installation.provider_code.clone(),
        plugin_id: installation.plugin_id.clone(),
        plugin_version: version.into(),
        contribution_code: "hero".into(),
        title: "Hero".into(),
        runtime: "native_react".into(),
        entry: "blocks/hero.js".into(),
        ui_registration: "code_template".into(),
        code_template: Some("official".into()),
        code_template_version: Some("official-v1".into()),
        code_template_language: Some("jsx".into()),
        code_modules: vec![],
        context_contract: domain::FrontendBlockContextContract {
            primitives: vec![],
            input_schema: serde_json::json!({}),
        },
        permissions: domain::FrontendBlockPermissions {
            network: "none".into(),
            storage: "none".into(),
            secrets: "none".into(),
        },
        ui_capabilities: vec![],
    };
    control_plane::frontend_block_catalog::FrontendContributionCandidate {
        workspace_id,
        installation,
        artifact,
        assignment: Some(assignment),
        catalog_entry,
    }
}

#[tokio::test]
async fn frontend_catalog_reuses_request_facts_with_first_defaults_assignments_and_entry_validation(
) {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryNodeContributionRepository::new(actor_with_permissions(workspace_id, &[]), vec![])
            .with_console_operation("other.frontend-blocks", "frontend_blocks.view");
    let first = frontend_candidate(workspace_id, "1.0.0");
    let second = frontend_candidate(workspace_id, "2.0.0");
    let mut secondary = first.catalog_entry.clone();
    secondary.contribution_code = "secondary".into();
    let mut stale_version = first.catalog_entry.clone();
    stale_version.plugin_version = "old".into();
    let mut invalid_runtime = first.catalog_entry.clone();
    invalid_runtime.runtime = "unsupported".into();
    let mut invalid_assignment = second.assignment.clone().unwrap();
    invalid_assignment.workspace_id = Uuid::now_v7();
    let mut later_invalid = first.assignment.clone().unwrap();
    later_invalid.provider_code = "wrong-provider".into();
    let template = |code: &str, revision: Option<i32>| FrontendBlockCatalogDefaultTemplate {
        provider_code: first.installation.provider_code.clone(),
        contribution_code: code.into(),
        published_revision: revision.map(|revision| FrontendBlockCatalogTemplateRevision {
            revision,
            source: format!("managed {revision}"),
            language: domain::UiCodeTemplateLanguage::Tsx,
        }),
    };
    {
        let mut state = repository.frontend.write().await;
        state.installations = vec![first.installation.clone(), second.installation.clone()];
        state.artifacts = vec![first.artifact.clone(), second.artifact.clone()];
        state.assignments = vec![
            first.assignment.clone().unwrap(),
            later_invalid,
            invalid_assignment,
            second.assignment.clone().unwrap(),
        ];
        state.defaults = vec![
            template("hero", Some(2)),
            template("hero", Some(99)),
            template("secondary", None),
            template("secondary", Some(99)),
        ];
        state.blocks = vec![
            first.catalog_entry.clone(),
            second.catalog_entry.clone(),
            stale_version,
            invalid_runtime,
            secondary,
        ];
    }
    let service = FrontendBlockCatalogService::new(
        repository.clone(),
        "test-node",
        frontend_contribution_graph(),
    )
    .unwrap();
    let view = service
        .list_frontend_blocks(ListFrontendBlockCatalogQuery {
            actor_user_id: Uuid::now_v7(),
        })
        .await
        .unwrap();
    assert_eq!(
        view.entries
            .iter()
            .map(|entry| entry.catalog_entry.contribution_code.as_str())
            .collect::<Vec<_>>(),
        ["hero", "secondary"]
    );
    assert_eq!(
        view.entries[0].catalog_entry.code_template.as_deref(),
        Some("managed 2")
    );
    assert_eq!(
        view.entries[0]
            .catalog_entry
            .code_template_version
            .as_deref(),
        Some("managed-r2")
    );
    assert_eq!(
        view.entries[0]
            .catalog_entry
            .code_template_language
            .as_deref(),
        Some("tsx")
    );
    assert_eq!(
        view.entries[1].catalog_entry.code_template.as_deref(),
        Some("official")
    );
    let expected_reads = vec![
        "blocks".into(),
        "defaults".into(),
        "assignments".into(),
        format!("installation:{}", first.installation.id),
        format!("artifact:test-node:{}", first.installation.id),
        format!("installation:{}", second.installation.id),
        format!("artifact:test-node:{}", second.installation.id),
    ];
    {
        let mut state = repository.frontend.write().await;
        assert_eq!(state.reads, expected_reads);
        state.reads.clear();
        state.installations[0].plugin_version = "changed-between-requests".into();
    }
    assert!(service
        .list_frontend_blocks(ListFrontendBlockCatalogQuery {
            actor_user_id: Uuid::now_v7()
        })
        .await
        .unwrap()
        .entries
        .is_empty());
    assert_eq!(repository.frontend.read().await.reads, expected_reads);
    let other_node =
        FrontendBlockCatalogService::new(repository, "other-node", frontend_contribution_graph())
            .unwrap();
    assert!(other_node
        .list_frontend_blocks(ListFrontendBlockCatalogQuery {
            actor_user_id: Uuid::now_v7()
        })
        .await
        .unwrap()
        .entries
        .is_empty());
}

#[tokio::test]
async fn frontend_catalog_missing_facts_are_reused_and_read_errors_keep_lazy_order() {
    let workspace_id = Uuid::now_v7();
    let repository =
        MemoryNodeContributionRepository::new(actor_with_permissions(workspace_id, &[]), vec![])
            .with_console_operation("other.frontend-blocks", "frontend_blocks.view");
    let missing = frontend_candidate(workspace_id, "1");
    let no_artifact = frontend_candidate(workspace_id, "1");
    let failing = frontend_candidate(workspace_id, "1");
    let unread = frontend_candidate(workspace_id, "1");
    let failure = format!("artifact:test-node:{}", failing.installation.id);
    {
        let mut state = repository.frontend.write().await;
        state.installations = vec![
            no_artifact.installation.clone(),
            failing.installation.clone(),
            unread.installation,
        ];
        state.blocks = vec![
            missing.catalog_entry.clone(),
            missing.catalog_entry,
            no_artifact.catalog_entry.clone(),
            no_artifact.catalog_entry,
            failing.catalog_entry,
            unread.catalog_entry,
        ];
        state.fail_on = Some(failure.clone());
    }
    let service = FrontendBlockCatalogService::new(
        repository.clone(),
        "test-node",
        frontend_contribution_graph(),
    )
    .unwrap();
    let error = service
        .list_frontend_blocks(ListFrontendBlockCatalogQuery {
            actor_user_id: Uuid::now_v7(),
        })
        .await
        .unwrap_err();
    assert_eq!(error.to_string(), format!("fixture read failed: {failure}"));
    {
        let mut state = repository.frontend.write().await;
        assert_eq!(
            state.reads,
            vec![
                "blocks".into(),
                "defaults".into(),
                "assignments".into(),
                format!("installation:{}", missing.installation.id),
                format!("installation:{}", no_artifact.installation.id),
                format!("artifact:test-node:{}", no_artifact.installation.id),
                format!("installation:{}", failing.installation.id),
                failure
            ]
        );
        state.blocks.clear();
        state.reads.clear();
    }
    assert!(service
        .list_frontend_blocks(ListFrontendBlockCatalogQuery {
            actor_user_id: Uuid::now_v7()
        })
        .await
        .unwrap()
        .entries
        .is_empty());
    assert_eq!(
        repository.frontend.read().await.reads,
        ["blocks", "defaults", "assignments"]
    );
}

#[test]
fn mixed_system_service_frontend_retains_explicit_assignment_and_durable_authority() {
    use control_plane::frontend_block_catalog::{
        FrontendContributionResolution, FrontendContributionResolver,
    };
    use domain::{ContributionAuthorizationStatus, ContributionResourceScope};
    let workspace = Uuid::now_v7();
    let mut candidate = frontend_candidate(workspace, "1.0.0");
    candidate.installation.contract_version = "1flowbase.extension-bus/v1".into();
    let owner = candidate.installation.provider_code.clone();
    let contribution = format!("{owner}.hero");
    candidate.installation.metadata_json = serde_json::json!({
        "managed_service": {"scope":"system"},
        "managed": {
            "module": {"bus_version":"v1", "module_id":owner, "module_version":"1.0.0", "module_kind":"capability",
                "contributions":[{"contribution_id":contribution,"contributor_module_id":owner,"point_id":FRONTEND_BLOCK_CONTRIBUTION_POINT_ID,
                    "contract_version":"1","required_permissions":[FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION],"mode":"append"}]},
            "execution_bindings":[{"contribution_id":contribution,"execution_mode":"declarative_only",
                "runtime":{"protocol":"stdio_json","entry":"bin/fixture"},"handler":"render.hero","payload":{"kind":"frontend_block","contribution_code":"hero"}}]
        }
    });
    candidate.authority = Some(domain::PluginContributionAuthoritySnapshot {
        installation_id: candidate.installation.id,
        workspace_id: domain::SYSTEM_SCOPE_ID,
        revision: 1,
        authorizations: vec![domain::PluginContributionAuthorization {
            id: Uuid::now_v7(),
            installation_id: candidate.installation.id,
            workspace_id: domain::SYSTEM_SCOPE_ID,
            contribution_id: contribution,
            point_id: FRONTEND_BLOCK_CONTRIBUTION_POINT_ID.into(),
            permission: FRONTEND_BLOCK_TRUSTED_UI_MOUNT_PERMISSION.into(),
            resource_scope: ContributionResourceScope::Workspace,
            permission_contract_id: FRONTEND_BLOCK_CONTRIBUTION_CONTRACT_ID.into(),
            permission_contract_version: "1".into(),
            status: ContributionAuthorizationStatus::Active,
            granted_by: Uuid::nil(),
            revoked_by: None,
            granted_at: time::OffsetDateTime::UNIX_EPOCH,
            revoked_at: None,
            revision: 1,
        }],
    });
    let resolver = FrontendContributionResolver::compile(frontend_contribution_graph()).unwrap();
    assert!(matches!(
        resolver.resolve(candidate.clone()),
        FrontendContributionResolution::Active(_)
    ));
    let mut missing_assignment = candidate.clone();
    missing_assignment.assignment = None;
    assert!(matches!(
        resolver.resolve(missing_assignment),
        FrontendContributionResolution::Disabled(_)
    ));
    let mut wrong_workspace = candidate.clone();
    wrong_workspace.workspace_id = Uuid::now_v7();
    assert!(matches!(
        resolver.resolve(wrong_workspace),
        FrontendContributionResolution::Disabled(_)
    ));
    let mut revoked = candidate.clone();
    revoked.authority.as_mut().unwrap().authorizations[0].status =
        ContributionAuthorizationStatus::Revoked;
    assert!(matches!(
        resolver.resolve(revoked),
        FrontendContributionResolution::Disabled(_)
    ));
    let mut wrong_scope = candidate;
    wrong_scope.authority.as_mut().unwrap().workspace_id = workspace;
    assert!(matches!(
        resolver.resolve(wrong_scope),
        FrontendContributionResolution::Disabled(_)
    ));
}
