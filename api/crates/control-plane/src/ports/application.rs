use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationVisibility {
    Own,
    All,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationManagementSortField {
    UpdatedAt,
    CreatedAt,
    Name,
    ApplicationType,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApplicationManagementSortDirection {
    Asc,
    Desc,
}

#[derive(Debug, Clone)]
pub struct ApplicationManagementQuery {
    pub filter: domain::ResourceFilterExpr,
    pub sort_field: ApplicationManagementSortField,
    pub sort_direction: ApplicationManagementSortDirection,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub struct ApplicationManagementRecord {
    pub id: Uuid,
    pub application_type: domain::ApplicationType,
    pub workflow_trigger_type: Option<domain::WorkflowTriggerType>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
    pub created_by: Uuid,
    pub created_by_display_name: String,
    pub created_at: OffsetDateTime,
    pub updated_at: OffsetDateTime,
    pub tags: Vec<domain::ApplicationTag>,
    pub publication_status: domain::ApplicationPublicationStatus,
}

#[derive(Debug, Clone)]
pub struct ApplicationManagementPage {
    pub items: Vec<ApplicationManagementRecord>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
}

#[derive(Debug, Clone)]
pub enum CreateWorkflowTriggerConfig {
    Schedule {
        cron: String,
        timezone: String,
        input_payload: serde_json::Value,
    },
    Extension {
        subpath: String,
        http_method: String,
        access_policy: String,
        response_mode: String,
    },
}

#[derive(Debug, Clone)]
pub struct CreateApplicationInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub application_type: domain::ApplicationType,
    pub workflow_trigger_type: Option<domain::WorkflowTriggerType>,
    pub workflow_trigger_config: Option<CreateWorkflowTriggerConfig>,
    pub name: String,
    pub description: String,
    pub icon: Option<String>,
    pub icon_type: Option<String>,
    pub icon_background: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateApplicationInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub name: String,
    pub description: String,
    pub tag_ids: Vec<Uuid>,
    pub icon: Option<Option<String>>,
    pub icon_type: Option<Option<String>>,
    pub icon_background: Option<Option<String>>,
}

#[derive(Debug, Clone)]
pub struct DeleteApplicationInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
}

#[derive(Debug, Clone)]
pub struct CreateApplicationTagInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct ApplicationEnvironmentVariableInput {
    pub name: String,
    pub value_type: String,
    pub value: serde_json::Value,
    pub description: String,
}

#[derive(Debug, Clone)]
pub struct ReplaceApplicationEnvironmentVariablesInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub variables: Vec<ApplicationEnvironmentVariableInput>,
}

#[derive(Debug, Clone)]
pub struct ReplaceApplicationJsDependencySelectionInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub application_id: Uuid,
    pub installation_id: Uuid,
    pub provider_code: String,
    pub plugin_id: String,
    pub plugin_version: String,
    pub alias: String,
    pub package: String,
    pub version: String,
    pub target: String,
    pub artifact_path: String,
    pub artifact_hash: String,
    pub integrity: String,
    pub permissions: domain::JsDependencyPermissions,
}

impl ReplaceApplicationJsDependencySelectionInput {
    pub fn from_catalog_entry(
        actor_user_id: Uuid,
        workspace_id: Uuid,
        application_id: Uuid,
        entry: domain::JsDependencyRegistryEntry,
    ) -> Self {
        Self {
            actor_user_id,
            workspace_id,
            application_id,
            installation_id: entry.installation_id,
            provider_code: entry.provider_code,
            plugin_id: entry.plugin_id,
            plugin_version: entry.plugin_version,
            alias: entry.alias,
            package: entry.package,
            version: entry.version,
            target: entry.target,
            artifact_path: entry.artifact_path,
            artifact_hash: entry.integrity.clone(),
            integrity: entry.integrity,
            permissions: entry.permissions,
        }
    }
}

#[async_trait]
pub trait ApplicationRepository: Send + Sync {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> anyhow::Result<domain::ActorContext>;
    async fn load_role_console_policies_for_user(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<domain::RoleConsolePolicy>>;
    async fn list_applications(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> anyhow::Result<Vec<domain::ApplicationRecord>>;
    async fn create_application(
        &self,
        input: &CreateApplicationInput,
    ) -> anyhow::Result<domain::ApplicationRecord>;
    async fn update_application(
        &self,
        input: &UpdateApplicationInput,
    ) -> anyhow::Result<domain::ApplicationRecord>;
    async fn delete_application(&self, input: &DeleteApplicationInput) -> anyhow::Result<()>;
    async fn get_application(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> anyhow::Result<Option<domain::ApplicationRecord>>;
    async fn get_application_for_visibility(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> anyhow::Result<Option<domain::ApplicationRecord>>;
    async fn list_application_tags(
        &self,
        workspace_id: Uuid,
        actor_user_id: Uuid,
        visibility: ApplicationVisibility,
    ) -> anyhow::Result<Vec<domain::ApplicationTagCatalogEntry>>;
    async fn create_application_tag(
        &self,
        input: &CreateApplicationTagInput,
    ) -> anyhow::Result<domain::ApplicationTagCatalogEntry>;
    async fn list_application_environment_variables(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let _ = (workspace_id, application_id);
        anyhow::bail!("list_application_environment_variables not implemented")
    }
    async fn replace_application_environment_variables(
        &self,
        input: &ReplaceApplicationEnvironmentVariablesInput,
    ) -> anyhow::Result<Vec<domain::ApplicationEnvironmentVariable>> {
        let _ = input;
        anyhow::bail!("replace_application_environment_variables not implemented")
    }
    async fn append_audit_log(&self, event: &domain::AuditLogRecord) -> anyhow::Result<()>;
}

#[async_trait]
pub trait ApplicationManagementRepository: Send + Sync {
    async fn list_application_management(
        &self,
        workspace_id: Uuid,
        query: &ApplicationManagementQuery,
    ) -> anyhow::Result<ApplicationManagementPage>;
}

#[async_trait]
pub trait ApplicationJsDependencySelectionRepository: Send + Sync {
    async fn list_application_js_dependency_selections(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ApplicationJsDependencySelection>>;

    async fn replace_application_js_dependency_selection(
        &self,
        input: &ReplaceApplicationJsDependencySelectionInput,
    ) -> anyhow::Result<domain::ApplicationJsDependencySelection>;
}
