use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    frontend_block_catalog::{FrontendBlockCatalogService, ListFrontendBlockCatalogQuery},
    js_dependency::{JsDependencyService, ListWorkspaceJsDependenciesQuery},
    node_contribution::{ListNodeContributionsQuery, NodeContributionService},
    ports::{
        AuthRepository, FrontendBlockCatalogRepository, JsDependencyRepository,
        NodeContributionRepository, ReplaceInstallationFrontendBlocksInput,
        ReplaceInstallationJsDependenciesInput, RoleConsolePolicyReader,
    },
};
use domain::{ActorContext, NodeContributionDependencyStatus, NodeContributionRegistryEntry};
use tokio::sync::RwLock;
use uuid::Uuid;

use super::plugin_management::support::actor_with_permissions;

#[derive(Clone)]
struct MemoryNodeContributionRepository {
    actor: ActorContext,
    entries: Arc<RwLock<Vec<NodeContributionRegistryEntry>>>,
    console_policies: Vec<domain::RoleConsolePolicy>,
}

impl MemoryNodeContributionRepository {
    fn new(actor: ActorContext, entries: Vec<NodeContributionRegistryEntry>) -> Self {
        Self {
            actor,
            entries: Arc::new(RwLock::new(entries)),
            console_policies: Vec::new(),
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
    async fn find_authenticator(&self, _id: Uuid) -> Result<Option<domain::AuthenticatorRecord>> {
        Ok(None)
    }

    async fn find_user_for_password_login(
        &self,
        _authenticator_id: Uuid,
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
        _user_id: Uuid,
        _workspace_id: Uuid,
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
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
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
async fn node_contribution_service_lists_workspace_entries() {
    let workspace_id = Uuid::now_v7();
    let repository = MemoryNodeContributionRepository::new(
        actor_with_permissions(workspace_id, &[]),
        vec![sample_entry(
            "openai_prompt",
            NodeContributionDependencyStatus::Ready,
        )],
    )
    .with_console_operation("other.node-contributions", "node_contributions.view");
    let service = NodeContributionService::new(repository);

    let view = service
        .list_node_contributions(ListNodeContributionsQuery {
            actor_user_id: Uuid::now_v7(),
        })
        .await
        .unwrap();

    assert_eq!(view.entries.len(), 1);
    assert_eq!(view.entries[0].contribution_code, "openai_prompt");
    assert_eq!(view.entries[0].dependency_status.as_str(), "ready");
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
    let service = NodeContributionService::new(repository);

    let error = service
        .list_node_contributions(ListNodeContributionsQuery {
            actor_user_id: Uuid::now_v7(),
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

    let view = NodeContributionService::new(repository)
        .list_node_contributions(ListNodeContributionsQuery {
            actor_user_id: Uuid::now_v7(),
        })
        .await
        .expect("the exact console operation must authorize the catalog owner");

    assert_eq!(view.entries[0].contribution_code, "policy_only");
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

    let error = NodeContributionService::new(repository)
        .list_node_contributions(ListNodeContributionsQuery {
            actor_user_id: Uuid::now_v7(),
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

    FrontendBlockCatalogService::new(repository)
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

    assert!(FrontendBlockCatalogService::new(repository)
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
