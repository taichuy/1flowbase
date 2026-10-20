use super::*;

impl<R> ModelDefinitionService<R>
where
    R: ModelDefinitionRepository,
{
    pub async fn add_field(
        &self,
        command: AddModelFieldCommand,
    ) -> Result<domain::ModelFieldRecord> {
        let actor = self
            .repository
            .load_actor_context_for_user(command.actor_user_id)
            .await?;
        self.ensure_state_model_action(&actor, "manage").await?;
        let model = self
            .repository
            .get_model_definition(actor.current_workspace_id, command.model_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_definition"))?;
        ensure_field_can_be_added(&model)?;
        if command.field_kind == domain::ModelFieldKind::FilePreviewUrl {
            return Err(
                ControlPlaneError::InvalidInput("native_computed_field_is_system_owned").into(),
            );
        }
        let external_field_key =
            normalize_external_field_key(model.source_kind, command.external_field_key.as_deref())?;
        let api_required = command.api_required.unwrap_or(command.is_required);
        ensure_api_required_allowed(false, true, api_required)?;

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
                api_required,
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
        self.ensure_state_model_action(&actor, "manage").await?;
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
        let existing_field = model
            .fields
            .iter()
            .find(|field| field.id == command.field_id)
            .ok_or(ControlPlaneError::NotFound("model_field"))?;
        let api_required = command.api_required.unwrap_or(existing_field.api_required);
        ensure_api_required_allowed(
            existing_field.is_system,
            existing_field.is_writable,
            api_required,
        )?;

        let field = self
            .repository
            .update_model_field(&UpdateModelFieldInput {
                actor_user_id: command.actor_user_id,
                model_id: command.model_id,
                field_id: command.field_id,
                title: command.title,
                description: command.description,
                is_required: command.is_required,
                api_required,
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
}
