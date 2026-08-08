use control_plane::model_definition::{
    AddModelFieldCommand, CreateModelDefinitionCommand, CreateScopeDataModelGrantCommand,
    DeleteModelDefinitionCommand, DeleteModelFieldCommand, DeleteScopeDataModelGrantCommand,
    InMemoryModelDefinitionRepository, ModelDefinitionService, PublishModelCommand,
    UpdateModelDefinitionStatusCommand, UpdateModelFieldCommand, UpdateScopeDataModelGrantCommand,
};
use control_plane::ports::{
    AddModelFieldInput, CreateModelDefinitionInput, CreateScopeDataModelGrantInput,
    ModelDefinitionRepository, RoleConsolePolicyReader, UpdateModelDefinitionInput,
    UpdateModelDefinitionStatusInput, UpdateModelFieldInput, UpdateScopeDataModelGrantInput,
};
use domain::{
    ActorContext, AuditLogRecord, DataModelOwnerKind, DataModelProtection, DataModelScopeKind,
    DataModelStatus, DataSourceDefaults, ModelDefinitionRecord, ModelFieldKind, ModelFieldRecord,
    ScopeDataModelGrantRecord, SYSTEM_SCOPE_ID,
};
use serde_json::json;
use std::{
    collections::HashMap,
    sync::{Arc, Mutex},
};
use uuid::Uuid;

type RoleDataPolicyFixture = (
    domain::RoleDataPolicyRecord,
    Option<domain::RoleDataModelPolicyRecord>,
);

#[derive(Clone)]
struct ScopedModelDefinitionRepository {
    actor: ActorContext,
    models: Arc<Mutex<HashMap<Uuid, ModelDefinitionRecord>>>,
    data_source_defaults: Arc<Mutex<HashMap<(Uuid, Uuid), DataSourceDefaults>>>,
    grants: Arc<Mutex<Vec<ScopeDataModelGrantRecord>>>,
    role_data_policies: Arc<Mutex<Vec<RoleDataPolicyFixture>>>,
    audit_logs: Arc<Mutex<Vec<AuditLogRecord>>>,
    console_policies: Arc<Mutex<Vec<domain::RoleConsolePolicy>>>,
}

impl ScopedModelDefinitionRepository {
    fn new(actor: ActorContext) -> Self {
        Self {
            actor,
            models: Arc::default(),
            data_source_defaults: Arc::default(),
            grants: Arc::default(),
            role_data_policies: Arc::default(),
            audit_logs: Arc::default(),
            console_policies: Arc::default(),
        }
    }

    fn with_model(self, model: ModelDefinitionRecord) -> Self {
        self.models
            .lock()
            .expect("model lock poisoned")
            .insert(model.id, model);
        self
    }

    fn with_grant(self, grant: ScopeDataModelGrantRecord) -> Self {
        self.grants.lock().expect("grant lock poisoned").push(grant);
        self
    }

    fn with_role_data_policy(
        self,
        policy: domain::RoleDataPolicyRecord,
        model_policy: Option<domain::RoleDataModelPolicyRecord>,
    ) -> Self {
        self.role_data_policies
            .lock()
            .expect("role data policy lock poisoned")
            .push((policy, model_policy));
        self
    }

    fn with_data_source_defaults(
        self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
        defaults: DataSourceDefaults,
    ) -> Self {
        self.data_source_defaults
            .lock()
            .expect("data source defaults lock poisoned")
            .insert((workspace_id, data_source_instance_id), defaults);
        self
    }

    fn audit_events(&self) -> Vec<String> {
        self.audit_logs
            .lock()
            .expect("audit log lock poisoned")
            .iter()
            .map(|event| event.event_code.clone())
            .collect()
    }

    fn with_console_operation(self, operation_id: &str) -> Self {
        let group = domain::ConsolePolicyGroup::settings_feature("system.data-models")
            .expect("data-model settings group must be valid");
        self.console_policies
            .lock()
            .expect("console policy lock poisoned")
            .push(domain::RoleConsolePolicy::new(
                Uuid::now_v7(),
                vec![domain::RoleConsoleGroupPolicy::custom(
                    group,
                    vec![domain::ConsoleOperationPolicy::simple(
                        domain::ConsoleOperationId::try_from(operation_id)
                            .expect("model definition operation id must be valid"),
                        true,
                    )],
                )],
            ));
        self
    }
}

#[async_trait::async_trait]
impl RoleConsolePolicyReader for ScopedModelDefinitionRepository {
    async fn load_role_console_policies_for_user(
        &self,
        _actor: &domain::ActorContext,
    ) -> anyhow::Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self
            .console_policies
            .lock()
            .expect("console policy lock poisoned")
            .clone())
    }
}

#[tokio::test]
async fn ac_1281_model_definition_policy_only_allows_without_legacy_grant() {
    let actor = ActorContext::scoped(Uuid::now_v7(), Uuid::now_v7(), "member", Vec::new());
    let repository = ScopedModelDefinitionRepository::new(actor.clone())
        .with_console_operation("model_definitions.list");

    ModelDefinitionService::for_console_operation(
        repository.clone(),
        domain::ConsolePolicyGroup::settings_feature("system.data-models").unwrap(),
        "model_definitions.list",
    )
    .list_models(actor.user_id)
    .await
    .expect("the exact model-definition operation must authorize the settings owner");

    let create_error = ModelDefinitionService::for_console_operation(
        repository,
        domain::ConsolePolicyGroup::settings_feature("system.data-models").unwrap(),
        "model_definitions.create",
    )
    .create_model(CreateModelDefinitionCommand {
        actor_user_id: actor.user_id,
        scope_kind: DataModelScopeKind::Workspace,
        data_source_instance_id: None,
        external_resource_key: None,
        external_table_id: None,
        code: "should_not_create".to_string(),
        title: "Should not create".to_string(),
        status: None,
    })
    .await
    .expect_err("list policy must not enable the independent create operation");
    assert!(create_error.to_string().contains("permission_denied"));
}

#[tokio::test]
async fn ac_1281_model_definition_legacy_only_does_not_authorize() {
    let actor = ActorContext::scoped(
        Uuid::now_v7(),
        Uuid::now_v7(),
        "member",
        vec![access_control::SYSTEM_DATA_MODELS_SETTINGS_FEATURE_PERMISSION.to_string()],
    );
    let repository = ScopedModelDefinitionRepository::new(actor.clone());

    assert!(ModelDefinitionService::for_console_operation(
        repository,
        domain::ConsolePolicyGroup::settings_feature("system.data-models").unwrap(),
        "model_definitions.list",
    )
    .list_models(actor.user_id)
    .await
    .is_err());
}

#[async_trait::async_trait]
impl ModelDefinitionRepository for ScopedModelDefinitionRepository {
    async fn load_actor_context_for_user(
        &self,
        _actor_user_id: Uuid,
    ) -> anyhow::Result<ActorContext> {
        Ok(self.actor.clone())
    }

    async fn list_model_definitions(
        &self,
        _workspace_id: Uuid,
    ) -> anyhow::Result<Vec<ModelDefinitionRecord>> {
        Ok(self
            .models
            .lock()
            .expect("model lock poisoned")
            .values()
            .cloned()
            .collect())
    }

    async fn get_model_definition(
        &self,
        workspace_id: Uuid,
        model_id: Uuid,
    ) -> anyhow::Result<Option<ModelDefinitionRecord>> {
        Ok(self
            .models
            .lock()
            .expect("model lock poisoned")
            .get(&model_id)
            .filter(|model| {
                workspace_id.is_nil()
                    || !matches!(model.scope_kind, DataModelScopeKind::Workspace)
                    || model.scope_id == workspace_id
            })
            .cloned())
    }

    async fn get_data_source_defaults(
        &self,
        workspace_id: Uuid,
        data_source_instance_id: Uuid,
    ) -> anyhow::Result<DataSourceDefaults> {
        self.data_source_defaults
            .lock()
            .expect("data source defaults lock poisoned")
            .get(&(workspace_id, data_source_instance_id))
            .copied()
            .ok_or_else(|| {
                control_plane::errors::ControlPlaneError::NotFound("data_source_instance").into()
            })
    }

    async fn create_model_definition(
        &self,
        input: &CreateModelDefinitionInput,
    ) -> anyhow::Result<ModelDefinitionRecord> {
        let model = ModelDefinitionRecord {
            id: Uuid::now_v7(),
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_source_instance_id: input.data_source_instance_id,
            source_kind: input.source_kind,
            external_resource_key: input.external_resource_key.clone(),
            external_table_id: input.external_table_id.clone(),
            external_capability_snapshot: input.external_capability_snapshot.clone(),
            code: input.code.clone(),
            title: input.title.clone(),
            physical_table_name: format!("rtm_workspace_{}", input.code),
            acl_namespace: format!("state_model.{}", input.code),
            audit_namespace: format!("audit.state_model.{}", input.code),
            fields: vec![],
            availability_status: domain::MetadataAvailabilityStatus::Available,
            status: input.status,
            protection: input.protection.clone(),
        };
        self.models
            .lock()
            .expect("model lock poisoned")
            .insert(model.id, model.clone());
        Ok(model)
    }

    async fn update_model_definition(
        &self,
        input: &UpdateModelDefinitionInput,
    ) -> anyhow::Result<ModelDefinitionRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model = models.get_mut(&input.model_id).ok_or(
            control_plane::errors::ControlPlaneError::NotFound("model_definition"),
        )?;
        model.title = input.title.clone();
        Ok(model.clone())
    }

    async fn update_model_definition_status(
        &self,
        input: &UpdateModelDefinitionStatusInput,
    ) -> anyhow::Result<ModelDefinitionRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model = models
            .get_mut(&input.model_id)
            .filter(|model| {
                input.workspace_id.is_nil()
                    || !matches!(model.scope_kind, DataModelScopeKind::Workspace)
                    || model.scope_id == input.workspace_id
            })
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "model_definition",
            ))?;
        model.status = input.status;
        Ok(model.clone())
    }

    async fn add_model_field(
        &self,
        input: &AddModelFieldInput,
    ) -> anyhow::Result<ModelFieldRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model = models.get_mut(&input.model_id).ok_or(
            control_plane::errors::ControlPlaneError::NotFound("model_definition"),
        )?;
        let field = ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: input.model_id,
            code: input.code.clone(),
            title: input.title.clone(),
            description: input.description.clone(),
            physical_column_name: input
                .physical_column_name
                .clone()
                .unwrap_or_else(|| input.code.replace('-', "_")),
            external_field_key: input.external_field_key.clone(),
            field_kind: input.field_kind,
            is_system: input.is_system,
            is_writable: input.is_writable,
            is_required: input.is_required,
            api_required: input.api_required,
            is_unique: input.is_unique,
            default_value: input.default_value.clone(),
            display_interface: input.display_interface.clone(),
            display_options: input.display_options.clone(),
            relation_target_model_id: input.relation_target_model_id,
            relation_options: input.relation_options.clone(),
            sort_order: model.fields.len() as i32,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        };
        model.fields.push(field.clone());
        Ok(field)
    }

    async fn update_model_field(
        &self,
        input: &UpdateModelFieldInput,
    ) -> anyhow::Result<ModelFieldRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model = models.get_mut(&input.model_id).ok_or(
            control_plane::errors::ControlPlaneError::NotFound("model_definition"),
        )?;
        let field = model
            .fields
            .iter_mut()
            .find(|field| field.id == input.field_id)
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "model_field",
            ))?;
        field.title = input.title.clone();
        field.description = input.description.clone();
        field.is_required = input.is_required;
        field.api_required = input.api_required;
        field.is_unique = input.is_unique;
        field.default_value = input.default_value.clone();
        field.display_interface = input.display_interface.clone();
        field.display_options = input.display_options.clone();
        field.relation_options = input.relation_options.clone();
        Ok(field.clone())
    }

    async fn delete_model_definition(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
    ) -> anyhow::Result<()> {
        self.models
            .lock()
            .expect("model lock poisoned")
            .remove(&model_id)
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "model_definition",
            ))?;
        Ok(())
    }

    async fn delete_model_field(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
        field_id: Uuid,
    ) -> anyhow::Result<()> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model =
            models
                .get_mut(&model_id)
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "model_definition",
                ))?;
        let original_len = model.fields.len();
        model.fields.retain(|field| field.id != field_id);
        if model.fields.len() == original_len {
            return Err(control_plane::errors::ControlPlaneError::NotFound("model_field").into());
        }
        Ok(())
    }

    async fn publish_model_definition(
        &self,
        _actor_user_id: Uuid,
        model_id: Uuid,
    ) -> anyhow::Result<ModelDefinitionRecord> {
        let mut models = self.models.lock().expect("model lock poisoned");
        let model =
            models
                .get_mut(&model_id)
                .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                    "model_definition",
                ))?;
        model.status = DataModelStatus::Published;
        Ok(model.clone())
    }

    async fn create_scope_data_model_grant(
        &self,
        input: &CreateScopeDataModelGrantInput,
    ) -> anyhow::Result<ScopeDataModelGrantRecord> {
        let now = time::OffsetDateTime::now_utc();
        let grant = ScopeDataModelGrantRecord {
            id: input.grant_id,
            scope_kind: input.scope_kind,
            scope_id: input.scope_id,
            data_model_id: input.data_model_id,
            enabled: input.enabled,
            permission_profile: input.permission_profile,
            created_by: input.created_by,
            created_at: now,
            updated_at: now,
        };
        self.grants
            .lock()
            .expect("grant lock poisoned")
            .push(grant.clone());
        Ok(grant)
    }

    async fn update_scope_data_model_grant(
        &self,
        input: &UpdateScopeDataModelGrantInput,
    ) -> anyhow::Result<ScopeDataModelGrantRecord> {
        let mut grants = self.grants.lock().expect("grant lock poisoned");
        let grant = grants
            .iter_mut()
            .find(|grant| grant.id == input.grant_id && grant.data_model_id == input.data_model_id)
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "scope_data_model_grant",
            ))?;
        grant.enabled = input.enabled;
        grant.permission_profile = input.permission_profile;
        grant.updated_at = time::OffsetDateTime::now_utc();
        Ok(grant.clone())
    }

    async fn delete_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> anyhow::Result<ScopeDataModelGrantRecord> {
        let mut grants = self.grants.lock().expect("grant lock poisoned");
        let index = grants
            .iter()
            .position(|grant| grant.id == grant_id && grant.data_model_id == data_model_id)
            .ok_or(control_plane::errors::ControlPlaneError::NotFound(
                "scope_data_model_grant",
            ))?;
        Ok(grants.remove(index))
    }

    async fn get_scope_data_model_grant(
        &self,
        data_model_id: Uuid,
        grant_id: Uuid,
    ) -> anyhow::Result<Option<ScopeDataModelGrantRecord>> {
        Ok(self
            .grants
            .lock()
            .expect("grant lock poisoned")
            .iter()
            .find(|grant| grant.id == grant_id && grant.data_model_id == data_model_id)
            .cloned())
    }

    async fn list_scope_data_model_grants(
        &self,
        scope_kind: DataModelScopeKind,
        scope_id: Uuid,
    ) -> anyhow::Result<Vec<ScopeDataModelGrantRecord>> {
        Ok(self
            .grants
            .lock()
            .expect("grant lock poisoned")
            .iter()
            .filter(|grant| grant.scope_kind == scope_kind && grant.scope_id == scope_id)
            .cloned()
            .collect())
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
        Ok(self
            .role_data_policies
            .lock()
            .expect("role data policy lock poisoned")
            .clone())
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> anyhow::Result<()> {
        self.audit_logs
            .lock()
            .expect("audit log lock poisoned")
            .push(event.clone());
        Ok(())
    }
}

fn actor_in_workspace(actor_user_id: Uuid, workspace_id: Uuid) -> ActorContext {
    ActorContext::root(actor_user_id, workspace_id, "root")
}

fn scoped_manager_in_workspace(actor_user_id: Uuid, workspace_id: Uuid) -> ActorContext {
    ActorContext::scoped(
        actor_user_id,
        workspace_id,
        "member",
        [
            "state_model.view.all".into(),
            "state_model.manage.all".into(),
        ],
    )
}

fn system_model(model_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        id: model_id,
        scope_kind: DataModelScopeKind::System,
        scope_id: SYSTEM_SCOPE_ID,
        code: "system_orders".into(),
        title: "System Orders".into(),
        physical_table_name: "rtm_system_orders".into(),
        acl_namespace: "state_model.system_orders".into(),
        audit_namespace: "audit.state_model.system_orders".into(),
        fields: vec![],
        availability_status: domain::MetadataAvailabilityStatus::Available,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        status: DataModelStatus::Published,
        protection: DataModelProtection::default(),
    }
}

fn protected_extension_model(model_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        protection: DataModelProtection {
            owner_kind: DataModelOwnerKind::RuntimeExtension,
            owner_id: Some("ext.crm".into()),
            is_protected: true,
        },
        fields: vec![ModelFieldRecord {
            id: Uuid::now_v7(),
            data_model_id: model_id,
            code: "email".into(),
            title: "Email".into(),
            description: None,
            physical_column_name: "email".into(),
            external_field_key: Some("email".into()),
            field_kind: ModelFieldKind::String,
            is_system: false,
            is_writable: true,
            is_required: false,
            api_required: false,
            is_unique: false,
            default_value: None,
            display_interface: None,
            display_options: json!({}),
            relation_target_model_id: None,
            relation_options: json!({}),
            sort_order: 0,
            availability_status: domain::MetadataAvailabilityStatus::Available,
        }],
        ..system_model(model_id)
    }
}

fn unsafe_external_system_model(model_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        data_source_instance_id: Some(Uuid::now_v7()),
        source_kind: domain::DataModelSourceKind::ExternalSource,
        external_resource_key: Some("unsafe.contacts".into()),
        external_table_id: None,
        external_capability_snapshot: Some(json!({
            "supports_list": true,
            "supports_scope_filter": false
        })),
        ..system_model(model_id)
    }
}

fn safe_external_system_model(model_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        data_source_instance_id: Some(Uuid::now_v7()),
        source_kind: domain::DataModelSourceKind::ExternalSource,
        external_resource_key: Some("safe.contacts".into()),
        external_table_id: None,
        external_capability_snapshot: Some(json!({
            "supports_list": true,
            "supports_scope_filter": true
        })),
        ..system_model(model_id)
    }
}

fn model_in_workspace(model_id: Uuid, workspace_id: Uuid) -> ModelDefinitionRecord {
    ModelDefinitionRecord {
        id: model_id,
        scope_kind: DataModelScopeKind::Workspace,
        scope_id: workspace_id,
        code: "foreign_orders".into(),
        title: "Foreign Orders".into(),
        physical_table_name: "rtm_workspace_foreign_orders".into(),
        acl_namespace: "state_model.foreign_orders".into(),
        audit_namespace: "audit.state_model.foreign_orders".into(),
        fields: vec![],
        availability_status: domain::MetadataAvailabilityStatus::Available,
        data_source_instance_id: None,
        source_kind: domain::DataModelSourceKind::MainSource,
        external_resource_key: None,
        external_table_id: None,
        external_capability_snapshot: None,
        status: DataModelStatus::Published,
        protection: DataModelProtection::default(),
    }
}

fn scope_grant(
    grant_id: Uuid,
    model_id: Uuid,
    scope_kind: DataModelScopeKind,
    scope_id: Uuid,
) -> ScopeDataModelGrantRecord {
    let now = time::OffsetDateTime::now_utc();
    ScopeDataModelGrantRecord {
        id: grant_id,
        scope_kind,
        scope_id,
        data_model_id: model_id,
        enabled: true,
        permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
        created_by: None,
        created_at: now,
        updated_at: now,
    }
}

fn role_data_policy(
    can_view: bool,
    can_create: bool,
    can_update: bool,
    can_delete: bool,
    scope: domain::RoleDataPolicyScope,
) -> domain::RoleDataPolicyRecord {
    let now = time::OffsetDateTime::now_utc();
    domain::RoleDataPolicyRecord {
        id: Uuid::now_v7(),
        role_id: Uuid::now_v7(),
        role_code: "member".into(),
        can_view,
        can_create,
        can_update,
        can_delete,
        default_view_scope: scope,
        default_update_scope: scope,
        default_delete_scope: scope,
        created_at: now,
        updated_at: now,
    }
}

fn role_data_model_policy(
    role_id: Uuid,
    model_id: Uuid,
    view_scope_override: Option<domain::RoleDataPolicyScope>,
    update_scope_override: Option<domain::RoleDataPolicyScope>,
    delete_scope_override: Option<domain::RoleDataPolicyScope>,
) -> domain::RoleDataModelPolicyRecord {
    let now = time::OffsetDateTime::now_utc();
    domain::RoleDataModelPolicyRecord {
        id: Uuid::now_v7(),
        role_id,
        data_model_id: model_id,
        can_create_override: None,
        view_scope_override,
        update_scope_override,
        delete_scope_override,
        created_at: now,
        updated_at: now,
    }
}

mod basic_lifecycle;
mod external_source;
mod protection_advisor;
mod registered_system_table;
mod scope_grants;
