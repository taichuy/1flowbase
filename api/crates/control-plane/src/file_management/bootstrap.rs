use anyhow::Result;
use domain::{DataModelScopeKind, FileTableScopeKind, ModelFieldKind, SYSTEM_SCOPE_ID};
use uuid::Uuid;

use crate::{
    file_management::attachments_template_fields,
    ports::{
        AddModelFieldInput, CreateFileTableRegistrationInput, CreateModelDefinitionInput,
        CreateScopeDataModelGrantInput, FileManagementRepository, ModelDefinitionRepository,
    },
};

pub struct CreateWorkspaceFileTableCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub code: String,
    pub title: String,
    pub default_storage_id: Uuid,
}

pub struct FileManagementBootstrapService<R> {
    repository: R,
}

pub struct FileTableProvisioningService<R> {
    repository: R,
}

struct ProvisionFileTableInput {
    actor_user_id: Uuid,
    model_scope_kind: DataModelScopeKind,
    model_scope_id: Uuid,
    grant_scope_kind: DataModelScopeKind,
    grant_scope_id: Uuid,
    code: String,
    title: String,
    file_table_scope_kind: FileTableScopeKind,
    file_table_scope_id: Uuid,
    bound_storage_id: Uuid,
    is_builtin: bool,
    is_default: bool,
    protection: domain::DataModelProtection,
    grant_permission_profile: domain::ScopeDataModelPermissionProfile,
}

fn builtin_attachment_readonly_fields(
) -> [(&'static str, &'static str, ModelFieldKind, bool, bool); 6] {
    [
        ("id", "id", ModelFieldKind::String, true, true),
        (
            "scope_id",
            "scope_id",
            ModelFieldKind::ManyToOne,
            true,
            false,
        ),
        (
            "created_by",
            "created_by",
            ModelFieldKind::String,
            true,
            false,
        ),
        (
            "updated_by",
            "updated_by",
            ModelFieldKind::String,
            true,
            false,
        ),
        (
            "created_at",
            "created_at",
            ModelFieldKind::Datetime,
            true,
            false,
        ),
        (
            "updated_at",
            "updated_at",
            ModelFieldKind::Datetime,
            true,
            false,
        ),
    ]
}

async fn provision_file_table<R>(
    repository: &R,
    input: ProvisionFileTableInput,
) -> Result<domain::FileTableRecord>
where
    R: FileManagementRepository + ModelDefinitionRepository,
{
    let model = repository
        .create_model_definition(&CreateModelDefinitionInput {
            actor_user_id: input.actor_user_id,
            scope_kind: input.model_scope_kind,
            scope_id: input.model_scope_id,
            data_source_instance_id: None,
            source_kind: domain::DataModelSourceKind::MainSource,
            external_resource_key: None,
            external_table_id: None,
            external_capability_snapshot: None,
            status: domain::DataModelStatus::Published,
            api_exposure_status: domain::ApiExposureStatus::PublishedNotExposed,
            protection: input.protection.clone(),
            code: input.code,
            title: input.title,
        })
        .await?;

    if input.is_builtin {
        for (code, title, field_kind, is_required, is_unique) in
            builtin_attachment_readonly_fields()
        {
            repository
                .add_model_field(&AddModelFieldInput {
                    actor_user_id: input.actor_user_id,
                    model_id: model.id,
                    physical_column_name: Some(code.to_string()),
                    external_field_key: None,
                    code: code.to_string(),
                    title: title.to_string(),
                    description: None,
                    field_kind,
                    is_system: true,
                    is_writable: false,
                    apply_physical_schema: false,
                    is_required,
                    api_required: false,
                    is_unique,
                    default_value: None,
                    display_interface: None,
                    display_options: serde_json::json!({}),
                    relation_target_model_id: None,
                    relation_options: serde_json::json!({}),
                })
                .await?;
        }
    }

    for field in attachments_template_fields() {
        repository
            .add_model_field(&AddModelFieldInput {
                actor_user_id: input.actor_user_id,
                model_id: model.id,
                physical_column_name: input.is_builtin.then_some(field.code.clone()),
                external_field_key: None,
                code: field.code,
                title: field.title,
                description: None,
                field_kind: field.field_kind,
                is_system: false,
                is_writable: true,
                apply_physical_schema: !input.is_builtin,
                is_required: field.is_required,
                api_required: field.is_required && !input.is_builtin,
                is_unique: false,
                default_value: None,
                display_interface: None,
                display_options: serde_json::json!({}),
                relation_target_model_id: None,
                relation_options: serde_json::json!({}),
            })
            .await?;
    }

    let published = repository
        .publish_model_definition(input.actor_user_id, model.id)
        .await?;

    repository
        .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
            grant_id: Uuid::now_v7(),
            scope_kind: input.grant_scope_kind,
            scope_id: input.grant_scope_id,
            data_model_id: published.id,
            enabled: true,
            permission_profile: input.grant_permission_profile,
            created_by: Some(input.actor_user_id),
        })
        .await?;

    repository
        .create_file_table_registration(&CreateFileTableRegistrationInput {
            file_table_id: Uuid::now_v7(),
            actor_user_id: input.actor_user_id,
            code: published.code,
            title: published.title,
            scope_kind: input.file_table_scope_kind,
            scope_id: input.file_table_scope_id,
            model_definition_id: published.id,
            bound_storage_id: input.bound_storage_id,
            is_builtin: input.is_builtin,
            is_default: input.is_default,
        })
        .await
}

impl<R> FileManagementBootstrapService<R>
where
    R: FileManagementRepository + ModelDefinitionRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn ensure_builtin_attachments(
        &self,
        actor_user_id: Uuid,
        default_storage_id: Uuid,
        default_code: &str,
    ) -> Result<domain::FileTableRecord> {
        if let Some(existing) = self
            .repository
            .find_file_table_by_code(default_code)
            .await?
        {
            return Ok(existing);
        }

        if let Some(existing_model) = self
            .repository
            .list_model_definitions(SYSTEM_SCOPE_ID)
            .await?
            .into_iter()
            .find(|model| {
                model.code == default_code
                    && domain::builtin_contract_for_model(model)
                        .is_some_and(|contract| contract.code == "attachments")
            })
        {
            let grants = self
                .repository
                .list_scope_data_model_grants(DataModelScopeKind::System, SYSTEM_SCOPE_ID)
                .await?;
            if !grants
                .iter()
                .any(|grant| grant.data_model_id == existing_model.id)
            {
                self.repository
                    .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
                        grant_id: Uuid::now_v7(),
                        scope_kind: DataModelScopeKind::System,
                        scope_id: SYSTEM_SCOPE_ID,
                        data_model_id: existing_model.id,
                        enabled: true,
                        permission_profile: domain::ScopeDataModelPermissionProfile::SystemAll,
                        created_by: Some(actor_user_id),
                    })
                    .await?;
            }

            return self
                .repository
                .create_file_table_registration(&CreateFileTableRegistrationInput {
                    file_table_id: Uuid::now_v7(),
                    actor_user_id,
                    code: existing_model.code,
                    title: existing_model.title,
                    scope_kind: FileTableScopeKind::System,
                    scope_id: SYSTEM_SCOPE_ID,
                    model_definition_id: existing_model.id,
                    bound_storage_id: default_storage_id,
                    is_builtin: true,
                    is_default: true,
                })
                .await;
        }

        provision_file_table(
            &self.repository,
            ProvisionFileTableInput {
                actor_user_id,
                model_scope_kind: DataModelScopeKind::System,
                model_scope_id: SYSTEM_SCOPE_ID,
                grant_scope_kind: DataModelScopeKind::System,
                grant_scope_id: SYSTEM_SCOPE_ID,
                code: default_code.to_string(),
                title: "Attachments".into(),
                file_table_scope_kind: FileTableScopeKind::System,
                file_table_scope_id: SYSTEM_SCOPE_ID,
                bound_storage_id: default_storage_id,
                is_builtin: true,
                is_default: true,
                protection: domain::DataModelProtection {
                    owner_kind: domain::DataModelOwnerKind::Core,
                    owner_id: None,
                    is_protected: true,
                },
                grant_permission_profile: domain::ScopeDataModelPermissionProfile::SystemAll,
            },
        )
        .await
    }
}

impl<R> FileTableProvisioningService<R>
where
    R: FileManagementRepository + ModelDefinitionRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn create_workspace_file_table(
        &self,
        command: CreateWorkspaceFileTableCommand,
    ) -> Result<domain::FileTableRecord> {
        provision_file_table(
            &self.repository,
            ProvisionFileTableInput {
                actor_user_id: command.actor_user_id,
                model_scope_kind: DataModelScopeKind::System,
                model_scope_id: SYSTEM_SCOPE_ID,
                grant_scope_kind: DataModelScopeKind::Workspace,
                grant_scope_id: command.workspace_id,
                code: command.code,
                title: command.title,
                file_table_scope_kind: FileTableScopeKind::Workspace,
                file_table_scope_id: command.workspace_id,
                bound_storage_id: command.default_storage_id,
                is_builtin: false,
                is_default: false,
                protection: domain::DataModelProtection::default(),
                grant_permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
            },
        )
        .await
    }
}
