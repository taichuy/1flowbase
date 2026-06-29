use anyhow::Result;
use domain::{DataModelScopeKind, ModelFieldKind, SYSTEM_SCOPE_ID};
use uuid::Uuid;

use crate::ports::{
    AddModelFieldInput, CreateModelDefinitionInput, CreateScopeDataModelGrantInput,
    ModelDefinitionRepository, ReconcileSystemModelDefinitionInput, ReconcileSystemModelFieldInput,
};

#[derive(Debug, Clone)]
pub struct SystemMetadataFieldTemplate {
    pub code: &'static str,
    pub title: &'static str,
    pub description: Option<&'static str>,
    pub physical_column_name: Option<&'static str>,
    pub field_kind: ModelFieldKind,
    pub is_system: bool,
    pub is_writable: bool,
    pub apply_physical_schema: bool,
    pub is_required: bool,
    pub is_unique: bool,
}

#[derive(Debug, Clone)]
pub struct SystemMetadataModelTemplate {
    pub code: &'static str,
    pub title: &'static str,
    pub fields: Vec<SystemMetadataFieldTemplate>,
}

fn readonly_system_table_field(
    code: &'static str,
    title: &'static str,
    physical_column_name: &'static str,
    field_kind: ModelFieldKind,
    is_required: bool,
    is_unique: bool,
) -> SystemMetadataFieldTemplate {
    SystemMetadataFieldTemplate {
        code,
        title,
        description: None,
        physical_column_name: Some(physical_column_name),
        field_kind,
        is_system: true,
        is_writable: false,
        apply_physical_schema: false,
        is_required,
        is_unique,
    }
}

pub fn user_metadata_template() -> SystemMetadataModelTemplate {
    SystemMetadataModelTemplate {
        code: "users",
        title: "用户",
        fields: vec![
            readonly_system_table_field("id", "用户 ID", "id", ModelFieldKind::String, true, true),
            readonly_system_table_field(
                "created_by",
                "创建人",
                "created_by",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "updated_by",
                "更新人",
                "updated_by",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "account",
                "账号",
                "account",
                ModelFieldKind::String,
                true,
                true,
            ),
            readonly_system_table_field(
                "email",
                "邮箱",
                "email",
                ModelFieldKind::String,
                true,
                true,
            ),
            readonly_system_table_field(
                "phone",
                "手机号",
                "phone",
                ModelFieldKind::String,
                false,
                true,
            ),
            readonly_system_table_field(
                "name",
                "姓名",
                "name",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "nickname",
                "昵称",
                "nickname",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "avatar_url",
                "头像",
                "avatar_url",
                ModelFieldKind::String,
                false,
                false,
            ),
            readonly_system_table_field(
                "introduction",
                "简介",
                "introduction",
                ModelFieldKind::Text,
                true,
                false,
            ),
            readonly_system_table_field(
                "preferred_locale",
                "偏好语言",
                "preferred_locale",
                ModelFieldKind::String,
                false,
                false,
            ),
            readonly_system_table_field(
                "meta",
                "元数据",
                "meta",
                ModelFieldKind::Json,
                true,
                false,
            ),
            readonly_system_table_field(
                "default_display_role",
                "默认展示角色",
                "default_display_role",
                ModelFieldKind::String,
                false,
                false,
            ),
            readonly_system_table_field(
                "email_login_enabled",
                "邮箱登录",
                "email_login_enabled",
                ModelFieldKind::Boolean,
                true,
                false,
            ),
            readonly_system_table_field(
                "phone_login_enabled",
                "手机登录",
                "phone_login_enabled",
                ModelFieldKind::Boolean,
                true,
                false,
            ),
            readonly_system_table_field(
                "status",
                "状态",
                "status",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "created_at",
                "创建时间",
                "created_at",
                ModelFieldKind::Datetime,
                true,
                false,
            ),
            readonly_system_table_field(
                "updated_at",
                "更新时间",
                "updated_at",
                ModelFieldKind::Datetime,
                true,
                false,
            ),
        ],
    }
}

pub fn role_metadata_template() -> SystemMetadataModelTemplate {
    SystemMetadataModelTemplate {
        code: "roles",
        title: "角色",
        fields: vec![
            readonly_system_table_field("id", "角色 ID", "id", ModelFieldKind::String, true, true),
            readonly_system_table_field(
                "created_by",
                "创建人",
                "created_by",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "updated_by",
                "更新人",
                "updated_by",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "scope_id",
                "作用域 ID",
                "scope_id",
                ModelFieldKind::ManyToOne,
                true,
                false,
            ),
            readonly_system_table_field(
                "scope_kind",
                "作用域",
                "scope_kind",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "workspace_id",
                "工作区 ID",
                "workspace_id",
                ModelFieldKind::ManyToOne,
                false,
                false,
            ),
            readonly_system_table_field(
                "code",
                "角色标识",
                "code",
                ModelFieldKind::String,
                true,
                true,
            ),
            readonly_system_table_field(
                "name",
                "角色名称",
                "name",
                ModelFieldKind::String,
                true,
                false,
            ),
            readonly_system_table_field(
                "introduction",
                "简介",
                "introduction",
                ModelFieldKind::Text,
                true,
                false,
            ),
            readonly_system_table_field(
                "is_builtin",
                "内置角色",
                "is_builtin",
                ModelFieldKind::Boolean,
                true,
                false,
            ),
            readonly_system_table_field(
                "is_editable",
                "可编辑",
                "is_editable",
                ModelFieldKind::Boolean,
                true,
                false,
            ),
            readonly_system_table_field(
                "auto_grant_new_permissions",
                "自动授予新权限",
                "auto_grant_new_permissions",
                ModelFieldKind::Boolean,
                true,
                false,
            ),
            readonly_system_table_field(
                "is_default_member_role",
                "默认成员角色",
                "is_default_member_role",
                ModelFieldKind::Boolean,
                true,
                false,
            ),
            readonly_system_table_field(
                "system_kind",
                "系统角色类型",
                "system_kind",
                ModelFieldKind::String,
                false,
                false,
            ),
            readonly_system_table_field(
                "created_at",
                "创建时间",
                "created_at",
                ModelFieldKind::Datetime,
                true,
                false,
            ),
            readonly_system_table_field(
                "updated_at",
                "更新时间",
                "updated_at",
                ModelFieldKind::Datetime,
                true,
                false,
            ),
        ],
    }
}

pub fn system_metadata_templates() -> Vec<SystemMetadataModelTemplate> {
    vec![user_metadata_template(), role_metadata_template()]
}

fn registered_system_table_protection() -> domain::DataModelProtection {
    domain::DataModelProtection {
        owner_kind: domain::DataModelOwnerKind::Core,
        owner_id: None,
        is_protected: true,
    }
}

fn seed_string_if_empty(existing: &str, seed: &str) -> String {
    if existing.trim().is_empty() {
        seed.to_string()
    } else {
        existing.to_string()
    }
}

fn seed_description_if_empty(
    existing: &Option<String>,
    seed: Option<&'static str>,
) -> Option<String> {
    match existing {
        Some(value) if !value.trim().is_empty() => Some(value.clone()),
        _ => seed.map(str::to_string),
    }
}

fn seed_json_object_if_empty(existing: &serde_json::Value) -> serde_json::Value {
    match existing {
        serde_json::Value::Null => serde_json::json!({}),
        serde_json::Value::Object(object) if object.is_empty() => serde_json::json!({}),
        _ => existing.clone(),
    }
}

pub struct SystemMetadataBootstrapService<R> {
    repository: R,
}

impl<R> SystemMetadataBootstrapService<R>
where
    R: ModelDefinitionRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn ensure_builtin_user_and_role_models(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<domain::ModelDefinitionRecord>> {
        let mut ensured = Vec::new();
        for template in system_metadata_templates() {
            ensured.push(self.ensure_template(actor_user_id, template).await?);
        }
        Ok(ensured)
    }

    pub async fn ensure_builtin_runtime_read_model_grants(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::ScopeDataModelGrantRecord>> {
        let models = self
            .repository
            .list_model_definitions(SYSTEM_SCOPE_ID)
            .await?;
        let existing_grants = self
            .repository
            .list_scope_data_model_grants(DataModelScopeKind::Workspace, workspace_id)
            .await?;
        let mut ensured = Vec::new();

        for model in models.into_iter().filter(|model| {
            domain::builtin_contract_for_model(model)
                .is_some_and(|contract| contract.kind == domain::BuiltinDataModelKind::RuntimeRead)
        }) {
            if let Some(existing) = existing_grants
                .iter()
                .find(|grant| grant.data_model_id == model.id)
            {
                ensured.push(existing.clone());
                continue;
            }

            ensured.push(
                self.repository
                    .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
                        grant_id: Uuid::now_v7(),
                        scope_kind: DataModelScopeKind::Workspace,
                        scope_id: workspace_id,
                        data_model_id: model.id,
                        enabled: true,
                        permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
                        created_by: Some(actor_user_id),
                    })
                    .await?,
            );
        }

        Ok(ensured)
    }

    async fn ensure_template(
        &self,
        actor_user_id: Uuid,
        template: SystemMetadataModelTemplate,
    ) -> Result<domain::ModelDefinitionRecord> {
        if let Some(existing) = self
            .repository
            .list_model_definitions(SYSTEM_SCOPE_ID)
            .await?
            .into_iter()
            .find(|model| {
                model.scope_kind == DataModelScopeKind::System
                    && model.scope_id == SYSTEM_SCOPE_ID
                    && model.source_kind == domain::DataModelSourceKind::MainSource
                    && model.code == template.code
            })
        {
            return self
                .ensure_existing_template(actor_user_id, existing, template)
                .await;
        }

        let model = self
            .repository
            .create_model_definition(&CreateModelDefinitionInput {
                actor_user_id,
                scope_kind: DataModelScopeKind::System,
                scope_id: SYSTEM_SCOPE_ID,
                data_source_instance_id: None,
                source_kind: domain::DataModelSourceKind::MainSource,
                external_resource_key: None,
                external_table_id: None,
                external_capability_snapshot: None,
                status: domain::DataModelStatus::Published,
                api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
                protection: registered_system_table_protection(),
                code: template.code.to_string(),
                title: template.title.to_string(),
            })
            .await?;

        self.ensure_template_fields(actor_user_id, model.id, &model.fields, &template)
            .await?;

        let published = self
            .repository
            .publish_model_definition(actor_user_id, model.id)
            .await?;

        self.ensure_system_scope_grant(actor_user_id, published.id)
            .await?;

        Ok(published)
    }

    async fn ensure_existing_template(
        &self,
        actor_user_id: Uuid,
        existing: domain::ModelDefinitionRecord,
        template: SystemMetadataModelTemplate,
    ) -> Result<domain::ModelDefinitionRecord> {
        let reconciled = self
            .repository
            .reconcile_system_model_definition(&ReconcileSystemModelDefinitionInput {
                actor_user_id,
                model_id: existing.id,
                title: seed_string_if_empty(&existing.title, template.title),
                physical_table_name: template.code.to_string(),
                status: domain::DataModelStatus::Published,
                api_exposure_status: existing.api_exposure_status,
                protection: registered_system_table_protection(),
            })
            .await?;

        self.ensure_template_fields(actor_user_id, reconciled.id, &reconciled.fields, &template)
            .await?;

        self.ensure_system_scope_grant(actor_user_id, reconciled.id)
            .await?;

        Ok(self
            .repository
            .get_model_definition(SYSTEM_SCOPE_ID, reconciled.id)
            .await?
            .unwrap_or(reconciled))
    }

    async fn ensure_template_fields(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
        existing_fields: &[domain::ModelFieldRecord],
        template: &SystemMetadataModelTemplate,
    ) -> Result<()> {
        for (sort_order, field) in template.fields.iter().enumerate() {
            let physical_column_name = field.physical_column_name.map(str::to_string);
            let Some(physical_column_name) = physical_column_name else {
                continue;
            };
            if let Some(existing) = existing_fields
                .iter()
                .find(|existing| existing.code == field.code)
            {
                self.repository
                    .reconcile_system_model_field(&ReconcileSystemModelFieldInput {
                        actor_user_id,
                        model_id,
                        field_id: existing.id,
                        title: seed_string_if_empty(&existing.title, field.title),
                        description: seed_description_if_empty(
                            &existing.description,
                            field.description,
                        ),
                        physical_column_name,
                        external_field_key: None,
                        field_kind: field.field_kind,
                        is_system: field.is_system,
                        is_writable: field.is_writable,
                        is_required: field.is_required,
                        is_unique: field.is_unique,
                        default_value: None,
                        display_interface: existing.display_interface.clone(),
                        display_options: seed_json_object_if_empty(&existing.display_options),
                        relation_target_model_id: None,
                        relation_options: serde_json::json!({}),
                        sort_order: sort_order as i32,
                        availability_status: domain::MetadataAvailabilityStatus::Available,
                    })
                    .await?;
            } else {
                self.repository
                    .add_model_field(&AddModelFieldInput {
                        actor_user_id,
                        model_id,
                        physical_column_name: Some(physical_column_name),
                        external_field_key: None,
                        code: field.code.to_string(),
                        title: field.title.to_string(),
                        description: field.description.map(str::to_string),
                        field_kind: field.field_kind,
                        is_system: field.is_system,
                        is_writable: field.is_writable,
                        apply_physical_schema: field.apply_physical_schema,
                        is_required: field.is_required,
                        is_unique: field.is_unique,
                        default_value: None,
                        display_interface: None,
                        display_options: serde_json::json!({}),
                        relation_target_model_id: None,
                        relation_options: serde_json::json!({}),
                    })
                    .await?;
            }
        }

        Ok(())
    }

    async fn ensure_system_scope_grant(&self, actor_user_id: Uuid, model_id: Uuid) -> Result<()> {
        let grants = self
            .repository
            .list_scope_data_model_grants(DataModelScopeKind::System, SYSTEM_SCOPE_ID)
            .await?;
        if grants.iter().any(|grant| grant.data_model_id == model_id) {
            return Ok(());
        }

        self.repository
            .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
                grant_id: Uuid::now_v7(),
                scope_kind: DataModelScopeKind::System,
                scope_id: SYSTEM_SCOPE_ID,
                data_model_id: model_id,
                enabled: true,
                permission_profile: domain::ScopeDataModelPermissionProfile::SystemAll,
                created_by: Some(actor_user_id),
            })
            .await?;

        Ok(())
    }
}
