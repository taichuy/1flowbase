use super::*;

impl<R, H> ModelProviderService<R, H>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
    H: ProviderRuntimePort,
{
    pub async fn preview_models(
        &self,
        command: PreviewModelProviderModelsCommand,
    ) -> Result<PreviewModelProviderModelsResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", &self.use_case).await?;

        let (installation, _package, provider_config, provider_code, audit_resource_id) =
            match command.instance_id {
                Some(instance_id) => {
                    let instance = self
                        .repository
                        .get_instance(actor.current_workspace_id, instance_id)
                        .await?
                        .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
                    let installation = self.ready_installation(instance.installation_id).await?;
                    if installation.availability_status()
                        != domain::PluginAvailabilityStatus::Available
                    {
                        return Err(ControlPlaneError::PluginUnavailable.into());
                    }
                    let package = load_installed_provider_package(&installation)?;
                    let (patch_public_config, patch_secret_config) =
                        split_provider_config(&package.provider.form_schema, &command.config_json)?;
                    let current_secret_json = self
                        .repository
                        .get_secret_json(instance.id, &self.provider_secret_master_key)
                        .await?
                        .unwrap_or_else(empty_object);
                    let merged_public_config =
                        merge_json_object(&instance.config_json, &patch_public_config)?;
                    let merged_secret_config =
                        merge_json_object(&current_secret_json, &patch_secret_config)?;
                    validate_required_fields(
                        &package.provider.form_schema,
                        &merged_public_config,
                        &merged_secret_config,
                    )?;
                    let provider_config =
                        merge_json_object(&merged_public_config, &merged_secret_config)?;
                    (
                        installation,
                        package,
                        provider_config,
                        instance.provider_code,
                        Some(instance.id),
                    )
                }
                None => {
                    let installation_id = command
                        .installation_id
                        .ok_or(ControlPlaneError::InvalidInput("installation_id"))?;
                    let installation = self.ready_installation(installation_id).await?;
                    ensure_installation_assigned(
                        &self.repository,
                        actor.current_workspace_id,
                        installation_id,
                    )
                    .await?;
                    if matches!(
                        installation.desired_state,
                        domain::PluginDesiredState::Disabled
                    ) || installation.availability_status()
                        != domain::PluginAvailabilityStatus::Available
                    {
                        return Err(ControlPlaneError::PluginUnavailable.into());
                    }
                    let package = load_installed_provider_package(&installation)?;
                    let (public_config, secret_config) =
                        split_provider_config(&package.provider.form_schema, &command.config_json)?;
                    validate_required_fields(
                        &package.provider.form_schema,
                        &public_config,
                        &secret_config,
                    )?;
                    let provider_config = merge_json_object(&public_config, &secret_config)?;
                    (
                        installation.clone(),
                        package,
                        provider_config,
                        installation.provider_code.clone(),
                        None,
                    )
                }
            };

        self.ensure_runtime_loaded(&installation).await?;
        let models = self
            .runtime
            .list_models(&installation, provider_config.clone())
            .await?;
        let expires_at = OffsetDateTime::now_utc() + time::Duration::minutes(10);
        let preview_token = Uuid::now_v7();
        self.repository
            .create_preview_session(&CreateModelProviderPreviewSessionInput {
                session_id: preview_token,
                workspace_id: actor.current_workspace_id,
                actor_user_id: command.actor_user_id,
                installation_id: Some(installation.id),
                instance_id: audit_resource_id,
                config_fingerprint: fingerprint_provider_config(&provider_config)?,
                models_json: serde_json::to_value(&models)?,
                expires_at,
            })
            .await?;
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider_instance",
                audit_resource_id,
                "model_provider.models_previewed",
                json!({
                    "provider_code": provider_code,
                    "model_count": models.len(),
                }),
            ))
            .await?;
        Ok(PreviewModelProviderModelsResult {
            models,
            preview_token,
            expires_at,
        })
    }

}
