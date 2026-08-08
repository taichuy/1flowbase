use super::*;

#[derive(Debug, Clone)]
pub struct CreateModelDefinitionInput {
    pub actor_user_id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub scope_id: Uuid,
    pub data_source_instance_id: Option<Uuid>,
    pub source_kind: domain::DataModelSourceKind,
    pub external_resource_key: Option<String>,
    pub external_table_id: Option<String>,
    pub external_capability_snapshot: Option<serde_json::Value>,
    pub code: String,
    pub title: String,
    pub status: domain::DataModelStatus,
    pub protection: domain::DataModelProtection,
}

#[derive(Debug, Clone)]
pub struct UpdateModelDefinitionInput {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub title: String,
    pub external_table_id: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ReconcileSystemModelDefinitionInput {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub title: String,
    pub physical_table_name: String,
    pub status: domain::DataModelStatus,
    pub protection: domain::DataModelProtection,
}

#[derive(Debug, Clone)]
pub struct UpdateModelDefinitionStatusInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub model_id: Uuid,
    pub status: domain::DataModelStatus,
}

#[derive(Debug, Clone)]
pub struct CreateScopeDataModelGrantInput {
    pub grant_id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub scope_id: Uuid,
    pub data_model_id: Uuid,
    pub enabled: bool,
    pub permission_profile: domain::ScopeDataModelPermissionProfile,
    pub created_by: Option<Uuid>,
}

#[derive(Debug, Clone)]
pub struct UpdateScopeDataModelGrantInput {
    pub data_model_id: Uuid,
    pub grant_id: Uuid,
    pub enabled: bool,
    pub permission_profile: domain::ScopeDataModelPermissionProfile,
}

#[derive(Debug, Clone)]
pub struct AddModelFieldInput {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub code: String,
    pub title: String,
    pub description: Option<String>,
    pub physical_column_name: Option<String>,
    pub external_field_key: Option<String>,
    pub field_kind: ModelFieldKind,
    pub is_system: bool,
    pub is_writable: bool,
    pub apply_physical_schema: bool,
    pub is_required: bool,
    pub api_required: bool,
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    pub display_options: serde_json::Value,
    pub relation_target_model_id: Option<Uuid>,
    pub relation_options: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct UpdateModelFieldInput {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub field_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub is_required: bool,
    pub api_required: bool,
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    pub display_options: serde_json::Value,
    pub relation_options: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct ReconcileSystemModelFieldInput {
    pub actor_user_id: Uuid,
    pub model_id: Uuid,
    pub field_id: Uuid,
    pub title: String,
    pub description: Option<String>,
    pub physical_column_name: String,
    pub external_field_key: Option<String>,
    pub field_kind: ModelFieldKind,
    pub is_system: bool,
    pub is_writable: bool,
    pub is_required: bool,
    pub api_required: bool,
    pub is_unique: bool,
    pub default_value: Option<serde_json::Value>,
    pub display_interface: Option<String>,
    pub display_options: serde_json::Value,
    pub relation_target_model_id: Option<Uuid>,
    pub relation_options: serde_json::Value,
    pub sort_order: i32,
    pub availability_status: domain::MetadataAvailabilityStatus,
}

#[async_trait]
pub trait ModelDefinitionRepository: Send + Sync {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> anyhow::Result<ActorContext>;
    async fn list_model_definitions(
        &self,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<ModelDefinitionRecord>>;
    async fn get_model_definition(
        &self,
        workspace_id: Uuid,
        model_id: Uuid,
    ) -> anyhow::Result<Option<ModelDefinitionRecord>>;
    async fn get_data_source_defaults(
        &self,
        _workspace_id: Uuid,
        _data_source_instance_id: Uuid,
    ) -> anyhow::Result<domain::DataSourceDefaults> {
        anyhow::bail!("get_data_source_defaults is not implemented")
    }
    async fn get_main_source_defaults(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<domain::DataSourceDefaults> {
        Ok(domain::DataSourceDefaults::default())
    }
    async fn create_model_definition(
        &self,
        input: &CreateModelDefinitionInput,
    ) -> anyhow::Result<ModelDefinitionRecord>;
    async fn update_model_definition(
        &self,
        input: &UpdateModelDefinitionInput,
    ) -> anyhow::Result<ModelDefinitionRecord>;
    async fn reconcile_system_model_definition(
        &self,
        _input: &ReconcileSystemModelDefinitionInput,
    ) -> anyhow::Result<ModelDefinitionRecord> {
        anyhow::bail!("reconcile_system_model_definition is not implemented")
    }
    async fn update_model_definition_status(
        &self,
        _input: &UpdateModelDefinitionStatusInput,
    ) -> anyhow::Result<ModelDefinitionRecord> {
        anyhow::bail!("update_model_definition_status is not implemented")
    }
    async fn add_model_field(&self, input: &AddModelFieldInput)
        -> anyhow::Result<ModelFieldRecord>;
    async fn update_model_field(
        &self,
        input: &UpdateModelFieldInput,
    ) -> anyhow::Result<ModelFieldRecord>;
    async fn reconcile_system_model_field(
        &self,
        _input: &ReconcileSystemModelFieldInput,
    ) -> anyhow::Result<ModelFieldRecord> {
        anyhow::bail!("reconcile_system_model_field is not implemented")
    }
    async fn delete_model_definition(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn delete_model_field(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
        field_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn publish_model_definition(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
    ) -> anyhow::Result<ModelDefinitionRecord>;
    async fn create_scope_data_model_grant(
        &self,
        _input: &CreateScopeDataModelGrantInput,
    ) -> anyhow::Result<domain::ScopeDataModelGrantRecord> {
        anyhow::bail!("create_scope_data_model_grant is not implemented")
    }
    async fn update_scope_data_model_grant(
        &self,
        _input: &UpdateScopeDataModelGrantInput,
    ) -> anyhow::Result<domain::ScopeDataModelGrantRecord> {
        anyhow::bail!("update_scope_data_model_grant is not implemented")
    }
    async fn get_scope_data_model_grant(
        &self,
        _data_model_id: Uuid,
        _grant_id: Uuid,
    ) -> anyhow::Result<Option<domain::ScopeDataModelGrantRecord>> {
        anyhow::bail!("get_scope_data_model_grant is not implemented")
    }
    async fn delete_scope_data_model_grant(
        &self,
        _data_model_id: Uuid,
        _grant_id: Uuid,
    ) -> anyhow::Result<domain::ScopeDataModelGrantRecord> {
        anyhow::bail!("delete_scope_data_model_grant is not implemented")
    }
    async fn list_scope_data_model_grants(
        &self,
        _scope_kind: DataModelScopeKind,
        _scope_id: Uuid,
    ) -> anyhow::Result<Vec<domain::ScopeDataModelGrantRecord>> {
        anyhow::bail!("list_scope_data_model_grants is not implemented")
    }
    async fn list_actor_role_data_policies(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
        _role_code: &str,
        _data_model_id: Uuid,
    ) -> anyhow::Result<
        Vec<(
            domain::RoleDataPolicyRecord,
            Option<domain::RoleDataModelPolicyRecord>,
        )>,
    > {
        anyhow::bail!("list_actor_role_data_policies is not implemented")
    }
    async fn append_audit_log(&self, event: &AuditLogRecord) -> anyhow::Result<()>;
}
