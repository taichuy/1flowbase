use std::collections::HashSet;

use access_control::ensure_permission;
use anyhow::Result;
use domain::DataModelScopeKind;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        AddModelFieldInput, CreateModelDefinitionInput, CreateScopeDataModelGrantInput,
        ModelDefinitionRepository, UpdateModelDefinitionInput, UpdateModelDefinitionStatusInput,
        UpdateModelFieldInput, UpdateScopeDataModelGrantInput,
    },
};

use super::{
    advisor::{
        advisor_finding, ensure_unsafe_external_system_all_confirmed, external_source_is_unsafe,
        has_duplicate_or_risky_field_configuration,
    },
    commands::{
        AddModelFieldCommand, BatchDeleteModelDefinitionsCommand, CreateModelDefinitionCommand,
        CreateScopeDataModelGrantCommand, DeleteModelDefinitionCommand, DeleteModelFieldCommand,
        DeleteScopeDataModelGrantCommand, PublishModelCommand, PublishedModel,
        UpdateModelDefinitionCommand, UpdateModelDefinitionStatusCommand, UpdateModelFieldCommand,
        UpdateScopeDataModelGrantCommand,
    },
    external_keys::{
        normalize_external_field_key, normalize_external_resource_key, normalize_external_table_id,
    },
    naming::normalize_api_exposure_for_status,
};

pub struct ModelDefinitionService<R> {
    repository: R,
}

pub fn runtime_scope_grant_from_record(
    grant: &domain::ScopeDataModelGrantRecord,
) -> runtime_core::runtime_acl::RuntimeScopeGrant {
    runtime_core::runtime_acl::RuntimeScopeGrant {
        data_model_id: grant.data_model_id,
        scope_kind: grant.scope_kind,
        scope_id: grant.scope_id,
        enabled: grant.enabled,
        permission_profile: grant.permission_profile,
    }
}

fn ensure_state_model_permission(
    actor: &domain::ActorContext,
    action: &str,
) -> Result<(), ControlPlaneError> {
    if actor.is_root
        || actor.has_permission(&format!("state_model.{action}.all"))
        || actor.has_permission(&format!("state_model.{action}.own"))
    {
        return Ok(());
    }

    Err(ControlPlaneError::PermissionDenied("permission_denied"))
}

fn ensure_scope_grant_lifecycle_authorized(
    actor: &domain::ActorContext,
    scope_kind: DataModelScopeKind,
    scope_id: Uuid,
) -> Result<(), ControlPlaneError> {
    if actor.is_root {
        return Ok(());
    }

    if scope_kind == DataModelScopeKind::Workspace && scope_id == actor.current_workspace_id {
        return Ok(());
    }

    Err(ControlPlaneError::PermissionDenied("permission_denied"))
}

fn ensure_protected_model_override_authorized(
    actor: &domain::ActorContext,
    model: &domain::ModelDefinitionRecord,
) -> Result<(), ControlPlaneError> {
    if model.protection.is_protected && !actor.is_root {
        return Err(ControlPlaneError::PermissionDenied("protected_data_model"));
    }

    Ok(())
}

fn ensure_model_deletable(model: &domain::ModelDefinitionRecord) -> Result<(), ControlPlaneError> {
    if !domain::data_model_capabilities(model).can_delete {
        return Err(ControlPlaneError::InvalidInput("builtin_data_model"));
    }

    Ok(())
}

fn field_changes_physical_metadata(
    field: &domain::ModelFieldRecord,
    command: &UpdateModelFieldCommand,
) -> bool {
    field.is_required != command.is_required
        || field.is_unique != command.is_unique
        || field.default_value != command.default_value
}

fn ensure_field_deletable(
    model: &domain::ModelDefinitionRecord,
    field_id: Uuid,
) -> Result<(), ControlPlaneError> {
    let field = model
        .fields
        .iter()
        .find(|field| field.id == field_id)
        .ok_or(ControlPlaneError::NotFound("model_field"))?;
    let capabilities = domain::data_model_field_capabilities(model, field);
    if !capabilities.can_delete {
        return Err(ControlPlaneError::InvalidInput("model_field"));
    }

    Ok(())
}

fn ensure_field_can_be_added(
    model: &domain::ModelDefinitionRecord,
) -> Result<(), ControlPlaneError> {
    if !domain::data_model_capabilities(model).can_add_user_field {
        return Err(ControlPlaneError::InvalidInput(
            "builtin_data_model_fields_readonly",
        ));
    }

    Ok(())
}

fn ensure_field_update_allowed(
    model: &domain::ModelDefinitionRecord,
    command: &UpdateModelFieldCommand,
) -> Result<(), ControlPlaneError> {
    let field = model
        .fields
        .iter()
        .find(|field| field.id == command.field_id)
        .ok_or(ControlPlaneError::NotFound("model_field"))?;
    let capabilities = domain::data_model_field_capabilities(model, field);

    if field_changes_physical_metadata(field, command) {
        if !capabilities.can_update_physical_metadata {
            return Err(ControlPlaneError::InvalidInput(
                "builtin_data_model_physical_fields_readonly",
            ));
        }
    }

    if !capabilities.can_update_presentation_metadata
        && (field.title != command.title
            || field.description != command.description
            || field.display_interface != command.display_interface
            || field.display_options != command.display_options
            || field.relation_options != command.relation_options)
    {
        return Err(ControlPlaneError::InvalidInput("model_field"));
    }

    Ok(())
}

impl<R> ModelDefinitionService<R>
where
    R: ModelDefinitionRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn load_runtime_scope_grant(
        &self,
        actor: &domain::ActorContext,
        data_model_id: Uuid,
    ) -> Result<Option<runtime_core::runtime_acl::RuntimeScopeGrant>> {
        let workspace_grants = self
            .repository
            .list_scope_data_model_grants(DataModelScopeKind::Workspace, actor.current_workspace_id)
            .await?;
        if let Some(grant) = workspace_grants
            .iter()
            .find(|grant| grant.data_model_id == data_model_id)
        {
            return Ok(Some(runtime_scope_grant_from_record(grant)));
        }

        if !actor.is_root {
            return Ok(None);
        }

        let system_grants = self
            .repository
            .list_scope_data_model_grants(DataModelScopeKind::System, domain::SYSTEM_SCOPE_ID)
            .await?;
        Ok(system_grants
            .iter()
            .find(|grant| grant.data_model_id == data_model_id)
            .map(runtime_scope_grant_from_record))
    }

    pub async fn load_runtime_scope_grant_for_scope(
        &self,
        scope_kind: DataModelScopeKind,
        scope_id: Uuid,
        data_model_id: Uuid,
    ) -> Result<Option<runtime_core::runtime_acl::RuntimeScopeGrant>> {
        let grants = self
            .repository
            .list_scope_data_model_grants(scope_kind, scope_id)
            .await?;
        Ok(grants
            .iter()
            .find(|grant| grant.data_model_id == data_model_id)
            .map(runtime_scope_grant_from_record))
    }

    pub async fn list_models(
        &self,
        actor_user_id: Uuid,
    ) -> Result<Vec<domain::ModelDefinitionRecord>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "view")?;
        let models = self
            .repository
            .list_model_definitions(actor.current_workspace_id)
            .await?;
        self.with_effective_exposures(models).await
    }

    pub async fn create_model(
        &self,
        command: CreateModelDefinitionCommand,
    ) -> Result<domain::ModelDefinitionRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, "state_model.create.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
        let grant_scope_id = match command.scope_kind {
            DataModelScopeKind::Workspace => actor.current_workspace_id,
            DataModelScopeKind::System => domain::SYSTEM_SCOPE_ID,
        };
        let source_kind = if command.data_source_instance_id.is_some() {
            domain::DataModelSourceKind::ExternalSource
        } else {
            domain::DataModelSourceKind::MainSource
        };
        let external_resource_key =
            normalize_external_resource_key(source_kind, command.external_resource_key.as_deref())?;
        let external_table_id =
            normalize_external_table_id(source_kind, command.external_table_id.as_deref())?;
        let defaults = match command.data_source_instance_id {
            Some(data_source_instance_id) => {
                self.repository
                    .get_data_source_defaults(actor.current_workspace_id, data_source_instance_id)
                    .await?
            }
            None => {
                self.repository
                    .get_main_source_defaults(actor.current_workspace_id)
                    .await?
            }
        };
        let status = command.status.unwrap_or(defaults.data_model_status);
        let api_exposure_status =
            normalize_api_exposure_for_status(status, defaults.api_exposure_status)?;

        let model = self
            .repository
            .create_model_definition(&CreateModelDefinitionInput {
                actor_user_id: command.actor_user_id,
                scope_kind: DataModelScopeKind::System,
                scope_id: domain::SYSTEM_SCOPE_ID,
                data_source_instance_id: command.data_source_instance_id,
                source_kind,
                external_resource_key,
                external_table_id,
                external_capability_snapshot: None,
                code: command.code,
                title: command.title,
                status,
                api_exposure_status,
                protection: domain::DataModelProtection::default(),
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(model.id),
                "state_model.created",
                serde_json::json!({ "code": model.code }),
            ))
            .await?;
        let grant = self
            .repository
            .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
                grant_id: Uuid::now_v7(),
                scope_kind: command.scope_kind,
                scope_id: grant_scope_id,
                data_model_id: model.id,
                enabled: true,
                permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
                created_by: Some(command.actor_user_id),
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(model.id),
                "state_model.scope_grant_created",
                serde_json::json!({
                    "scope_kind": grant.scope_kind.as_str(),
                    "scope_id": grant.scope_id,
                    "enabled": grant.enabled,
                    "permission_profile": grant.permission_profile.as_str(),
                }),
            ))
            .await?;

        self.with_effective_exposure(model).await
    }

    pub async fn update_model_status(
        &self,
        command: UpdateModelDefinitionStatusCommand,
    ) -> Result<domain::ModelDefinitionRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let previous_model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        if !domain::data_model_capabilities(&previous_model).can_update_lifecycle_status
            && previous_model.status != command.status
        {
            return Err(ControlPlaneError::InvalidInput(
                "builtin_data_model_lifecycle_status_readonly",
            )
            .into());
        }
        if domain::builtin_contract_for_model(&previous_model).is_none() {
            ensure_protected_model_override_authorized(&actor, &previous_model)?;
        }
        let previous_effective = self.effective_api_exposure_status(&previous_model);

        let candidate = domain::ModelDefinitionRecord {
            status: command.status,
            api_exposure_status: command.api_exposure_status,
            ..previous_model
        };
        let api_exposure_status = self.normalized_api_exposure_for_status(&candidate);
        let model = self
            .repository
            .update_model_definition_status(&UpdateModelDefinitionStatusInput {
                actor_user_id: command.actor_user_id,
                workspace_id: actor.current_workspace_id,
                model_id: command.model_id,
                status: command.status,
                api_exposure_status,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.model_id),
                "state_model.status_updated",
                serde_json::json!({
                    "status": model.status.as_str(),
                    "api_exposure_status": model.api_exposure_status.as_str(),
                }),
            ))
            .await?;
        let model = self.with_effective_exposure(model).await?;
        if previous_effective != model.api_exposure_status {
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "state_model",
                    Some(command.model_id),
                    "state_model.api_exposure_status_changed",
                    serde_json::json!({
                        "from": previous_effective.as_str(),
                        "to": model.api_exposure_status.as_str(),
                        "status": model.status.as_str(),
                    }),
                ))
                .await?;
        }

        Ok(model)
    }

    pub async fn get_model(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
    ) -> Result<domain::ModelDefinitionRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "view")?;

        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        self.with_effective_exposure(model).await
    }

    pub async fn list_scope_grants(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
    ) -> Result<Vec<domain::ScopeDataModelGrantRecord>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "view")?;
        self.repository
            .get_model_definition(actor.current_workspace_id, model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;

        let mut grants = self
            .repository
            .list_scope_data_model_grants(
                domain::DataModelScopeKind::Workspace,
                actor.current_workspace_id,
            )
            .await?;
        grants.extend(
            self.repository
                .list_scope_data_model_grants(
                    domain::DataModelScopeKind::System,
                    domain::SYSTEM_SCOPE_ID,
                )
                .await?,
        );
        grants.retain(|grant| grant.data_model_id == model_id);
        grants.sort_by(|left, right| {
            left.scope_kind
                .as_str()
                .cmp(right.scope_kind.as_str())
                .then(
                    left.permission_profile
                        .as_str()
                        .cmp(right.permission_profile.as_str()),
                )
                .then(left.id.cmp(&right.id))
        });
        Ok(grants)
    }

    pub async fn update_model(
        &self,
        command: UpdateModelDefinitionCommand,
    ) -> Result<domain::ModelDefinitionRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let previous_model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let external_table_id = normalize_external_table_id(
            previous_model.source_kind,
            command.external_table_id.as_deref(),
        )?;

        let model = self
            .repository
            .update_model_definition(&UpdateModelDefinitionInput {
                actor_user_id: command.actor_user_id,
                model_id: command.model_id,
                title: command.title,
                external_table_id,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.model_id),
                "state_model.updated",
                serde_json::json!({ "title": model.title }),
            ))
            .await?;

        self.with_effective_exposure(model).await
    }

    pub async fn add_field(
        &self,
        command: AddModelFieldCommand,
    ) -> Result<domain::ModelFieldRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        ensure_field_can_be_added(&model)?;
        let external_field_key =
            normalize_external_field_key(model.source_kind, command.external_field_key.as_deref())?;

        let field = self
            .repository
            .add_model_field(&AddModelFieldInput {
                actor_user_id: command.actor_user_id,
                model_id: command.model_id,
                code: command.code,
                title: command.title,
                description: command.description,
                physical_column_name: None,
                external_field_key,
                field_kind: command.field_kind,
                is_system: false,
                is_writable: true,
                apply_physical_schema: true,
                is_required: command.is_required,
                is_unique: command.is_unique,
                default_value: command.default_value,
                display_interface: command.display_interface,
                display_options: command.display_options,
                relation_target_model_id: command.relation_target_model_id,
                relation_options: command.relation_options,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.model_id),
                "state_model.field_created",
                serde_json::json!({ "field_code": field.code }),
            ))
            .await?;

        Ok(field)
    }

    pub async fn update_field(
        &self,
        command: UpdateModelFieldCommand,
    ) -> Result<domain::ModelFieldRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let is_builtin_data_model = domain::builtin_contract_for_model(&model).is_some();
        ensure_field_update_allowed(&model, &command)?;
        if !is_builtin_data_model {
            ensure_protected_model_override_authorized(&actor, &model)?;
        }

        let field = self
            .repository
            .update_model_field(&UpdateModelFieldInput {
                actor_user_id: command.actor_user_id,
                model_id: command.model_id,
                field_id: command.field_id,
                title: command.title,
                description: command.description,
                is_required: command.is_required,
                is_unique: command.is_unique,
                default_value: command.default_value,
                display_interface: command.display_interface,
                display_options: command.display_options,
                relation_options: command.relation_options,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.model_id),
                "state_model.field_updated",
                serde_json::json!({ "field_id": command.field_id }),
            ))
            .await?;

        Ok(field)
    }

    pub async fn delete_model(&self, command: DeleteModelDefinitionCommand) -> Result<()> {
        if !command.confirmed {
            return Err(ControlPlaneError::InvalidInput("confirmation").into());
        }

        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        ensure_model_deletable(&model)?;
        ensure_protected_model_override_authorized(&actor, &model)?;

        self.repository
            .delete_model_definition(command.actor_user_id, command.model_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.model_id),
                "state_model.deleted",
                serde_json::json!({}),
            ))
            .await?;

        Ok(())
    }

    pub async fn batch_delete_models(
        &self,
        command: BatchDeleteModelDefinitionsCommand,
    ) -> Result<Vec<Uuid>> {
        if !command.confirmed {
            return Err(ControlPlaneError::InvalidInput("confirmation").into());
        }
        if command.model_ids.is_empty() {
            return Err(ControlPlaneError::InvalidInput("model_ids").into());
        }

        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;

        let mut seen_model_ids = HashSet::new();
        let mut model_ids = Vec::with_capacity(command.model_ids.len());
        for model_id in command.model_ids {
            if seen_model_ids.insert(model_id) {
                model_ids.push(model_id);
            }
        }

        let mut models = Vec::with_capacity(model_ids.len());
        for model_id in &model_ids {
            let model = self
                .repository
                .get_model_definition(actor.current_workspace_id, *model_id)
                .await?
                .ok_or(ControlPlaneError::NotFound("model_definition"))?;
            ensure_model_deletable(&model)?;
            ensure_protected_model_override_authorized(&actor, &model)?;
            models.push(model);
        }

        for model in &models {
            self.repository
                .delete_model_definition(command.actor_user_id, model.id)
                .await?;
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(command.actor_user_id),
                    "state_model",
                    Some(model.id),
                    "state_model.deleted",
                    serde_json::json!({ "batch": true }),
                ))
                .await?;
        }

        Ok(model_ids)
    }

    pub async fn delete_field(&self, command: DeleteModelFieldCommand) -> Result<()> {
        if !command.confirmed {
            return Err(ControlPlaneError::InvalidInput("confirmation").into());
        }

        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        if domain::builtin_contract_for_model(&model).is_none() {
            ensure_protected_model_override_authorized(&actor, &model)?;
        }
        ensure_field_deletable(&model, command.field_id)?;

        self.repository
            .delete_model_field(command.actor_user_id, command.model_id, command.field_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.model_id),
                "state_model.field_deleted",
                serde_json::json!({ "field_id": command.field_id }),
            ))
            .await?;

        Ok(())
    }

    pub async fn publish_model(&self, command: PublishModelCommand) -> Result<PublishedModel> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_permission(&actor, "state_model.manage.all")
            .map_err(ControlPlaneError::PermissionDenied)?;
        let existing = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        ensure_protected_model_override_authorized(&actor, &existing)?;

        let model = self
            .repository
            .publish_model_definition(command.actor_user_id, command.model_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.model_id),
                "state_model.published",
                serde_json::json!({}),
            ))
            .await?;

        Ok(PublishedModel {
            resource: runtime_core::resource_descriptor::ResourceDescriptor::runtime_model(
                &model.code,
                model.scope_kind,
            ),
            model,
        })
    }

    pub async fn create_scope_grant(
        &self,
        command: CreateScopeDataModelGrantCommand,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let permission_profile =
            domain::ScopeDataModelPermissionProfile::parse(&command.permission_profile)
                .ok_or(ControlPlaneError::InvalidInput("permission_profile"))?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.data_model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        ensure_scope_grant_lifecycle_authorized(&actor, command.scope_kind, command.scope_id)?;
        ensure_unsafe_external_system_all_confirmed(
            &model,
            permission_profile,
            command.confirm_unsafe_external_source_system_all,
        )?;

        let grant = self
            .repository
            .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
                grant_id: Uuid::now_v7(),
                scope_kind: command.scope_kind,
                scope_id: command.scope_id,
                data_model_id: command.data_model_id,
                enabled: command.enabled,
                permission_profile,
                created_by: Some(command.actor_user_id),
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.data_model_id),
                "state_model.scope_grant_created",
                serde_json::json!({
                    "scope_kind": grant.scope_kind.as_str(),
                    "scope_id": grant.scope_id,
                    "enabled": grant.enabled,
                    "permission_profile": grant.permission_profile.as_str(),
                }),
            ))
            .await?;

        Ok(grant)
    }

    pub async fn update_scope_grant(
        &self,
        command: UpdateScopeDataModelGrantCommand,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.data_model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;

        let existing = self
            .repository
            .get_scope_data_model_grant(command.data_model_id, command.grant_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;
        ensure_scope_grant_lifecycle_authorized(&actor, existing.scope_kind, existing.scope_id)?;
        let permission_profile = match command.permission_profile {
            Some(permission_profile) => {
                domain::ScopeDataModelPermissionProfile::parse(&permission_profile)
                    .ok_or(ControlPlaneError::InvalidInput("permission_profile"))?
            }
            None => existing.permission_profile,
        };
        let enabled = command.enabled.unwrap_or(existing.enabled);
        ensure_unsafe_external_system_all_confirmed(
            &model,
            permission_profile,
            command.confirm_unsafe_external_source_system_all,
        )?;

        let grant = self
            .repository
            .update_scope_data_model_grant(&UpdateScopeDataModelGrantInput {
                data_model_id: command.data_model_id,
                grant_id: command.grant_id,
                enabled,
                permission_profile,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.data_model_id),
                "state_model.scope_grant_updated",
                serde_json::json!({
                    "scope_kind": grant.scope_kind.as_str(),
                    "scope_id": grant.scope_id,
                    "enabled": grant.enabled,
                    "permission_profile": grant.permission_profile.as_str(),
                }),
            ))
            .await?;

        Ok(grant)
    }

    pub async fn delete_scope_grant(
        &self,
        command: DeleteScopeDataModelGrantCommand,
    ) -> Result<domain::ScopeDataModelGrantRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "manage")?;
        self.repository
            .get_model_definition(actor.current_workspace_id, command.data_model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let existing = self
            .repository
            .get_scope_data_model_grant(command.data_model_id, command.grant_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("scope_data_model_grant"))?;
        ensure_scope_grant_lifecycle_authorized(&actor, existing.scope_kind, existing.scope_id)?;

        let grant = self
            .repository
            .delete_scope_data_model_grant(command.data_model_id, command.grant_id)
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "state_model",
                Some(command.data_model_id),
                "state_model.scope_grant_deleted",
                serde_json::json!({
                    "grant_id": grant.id,
                    "scope_kind": grant.scope_kind.as_str(),
                    "scope_id": grant.scope_id,
                    "enabled": grant.enabled,
                    "permission_profile": grant.permission_profile.as_str(),
                }),
            ))
            .await?;

        Ok(grant)
    }

    pub async fn advisor_findings(
        &self,
        actor_user_id: Uuid,
        model_id: Uuid,
    ) -> Result<Vec<domain::DataModelAdvisorFinding>> {
        let actor = self
            .repository
            .load_actor_context_for_user(actor_user_id)
            .await?;
        ensure_state_model_permission(&actor, "view")?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        let effective = self.with_effective_exposure(model.clone()).await?;
        let mut findings = Vec::new();

        if external_source_is_unsafe(&model) {
            findings.push(advisor_finding(
                model.id,
                domain::DataModelAdvisorSeverity::Blocking,
                "unsafe_external_source",
                "The external source lacks required scope filtering safety guarantees.",
                "Enable scope filtering in the data source capability before exposing this Data Model.",
                false,
            ));
        }

        if model.protection.is_protected && effective.status == domain::DataModelStatus::Published {
            findings.push(advisor_finding(
                model.id,
                domain::DataModelAdvisorSeverity::Blocking,
                "protected_model_exposure_attempt",
                "Protected Data Models cannot be exposed by normal admin API configuration.",
                "Use root emergency override only for audited operational recovery.",
                false,
            ));
        }

        if has_duplicate_or_risky_field_configuration(&model.fields) {
            findings.push(advisor_finding(
                model.id,
                domain::DataModelAdvisorSeverity::Medium,
                "duplicate_risky_field_configuration",
                "Fields contain duplicate external identifiers or risky uniqueness settings.",
                "Review duplicate field codes, duplicate external keys, and unique JSON fields.",
                true,
            ));
        }

        Ok(findings)
    }

    async fn with_effective_exposures(
        &self,
        models: Vec<domain::ModelDefinitionRecord>,
    ) -> Result<Vec<domain::ModelDefinitionRecord>> {
        let mut effective_models = Vec::with_capacity(models.len());
        for model in models {
            effective_models.push(self.with_effective_exposure(model).await?);
        }
        Ok(effective_models)
    }

    async fn with_effective_exposure(
        &self,
        mut model: domain::ModelDefinitionRecord,
    ) -> Result<domain::ModelDefinitionRecord> {
        model.api_exposure_status = self.effective_api_exposure_status(&model);
        Ok(model)
    }

    fn normalized_api_exposure_for_status(
        &self,
        model: &domain::ModelDefinitionRecord,
    ) -> domain::ApiExposureStatus {
        self.effective_api_exposure_status(model)
    }

    fn effective_api_exposure_status(
        &self,
        model: &domain::ModelDefinitionRecord,
    ) -> domain::ApiExposureStatus {
        match model.status {
            domain::DataModelStatus::Draft => domain::ApiExposureStatus::Draft,
            domain::DataModelStatus::Published => domain::ApiExposureStatus::ApiExposedReady,
            domain::DataModelStatus::Disabled | domain::DataModelStatus::Broken => {
                domain::ApiExposureStatus::PublishedNotExposed
            }
        }
    }
}
