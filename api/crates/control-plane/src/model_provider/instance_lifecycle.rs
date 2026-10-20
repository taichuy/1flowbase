use super::*;

impl<R, H> ModelProviderService<R, H>
where
    R: AuthRepository + PluginRepository + ModelProviderRepository,
    H: ProviderRuntimePort,
{
    pub async fn create_instance(
        &self,
        command: CreateModelProviderInstanceCommand,
    ) -> Result<ModelProviderInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", &self.use_case).await?;
        let installation = self.ready_installation(command.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            actor.current_workspace_id,
            command.installation_id,
        )
        .await?;
        if matches!(
            installation.desired_state,
            domain::PluginDesiredState::Disabled
        ) {
            return Err(ControlPlaneError::Conflict("plugin_installation_disabled").into());
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
        let configured_models =
            normalize_configured_models(command.configured_models, command.enabled_model_ids)?;
        let enabled_model_ids = configured_models_to_enabled_model_ids(&configured_models);
        let preview_state = self
            .resolve_preview_state(
                &actor,
                PreviewStateRequest {
                    installation_id: Some(installation.id),
                    instance_id: None,
                    provider_config: &provider_config,
                    preview_token: command.preview_token,
                },
            )
            .await?;
        let instance = self
            .repository
            .create_instance(&CreateModelProviderInstanceInput {
                instance_id: Uuid::now_v7(),
                workspace_id: actor.current_workspace_id,
                installation_id: installation.id,
                provider_code: installation.provider_code.clone(),
                protocol: installation.protocol.clone(),
                display_name: normalize_required_text(&command.display_name, "display_name")?,
                status: derive_instance_status(false, &enabled_model_ids),
                config_json: public_config.clone(),
                configured_models: configured_models.clone(),
                enabled_model_ids,
                included_in_main: command.included_in_main,
                created_by: command.actor_user_id,
            })
            .await?;
        if !is_empty_object(&secret_config) {
            self.repository
                .upsert_secret(&UpsertModelProviderSecretInput {
                    provider_instance_id: instance.id,
                    plaintext_secret_json: secret_config,
                    secret_version: 1,
                    master_key: self.provider_secret_master_key.clone(),
                })
                .await?;
        }
        if let Some(models_json) = preview_state.models_json.as_ref() {
            self.repository
                .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                    provider_instance_id: instance.id,
                    model_discovery_mode: map_model_discovery_mode(
                        package.provider.model_discovery_mode,
                    ),
                    refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                    source: map_catalog_source(package.provider.model_discovery_mode),
                    models_json: models_json.clone(),
                    last_error_message: None,
                    refreshed_at: preview_state.refreshed_at,
                })
                .await?;
        }
        if let Some(preview_token) = preview_state.preview_token {
            self.repository
                .delete_preview_session(actor.current_workspace_id, preview_token)
                .await?;
        }
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider_instance",
                Some(instance.id),
                "model_provider.created",
                json!({
                    "provider_code": instance.provider_code,
                    "display_name": instance.display_name,
                }),
            ))
            .await?;

        self.invalidate_routing_catalog(actor.current_workspace_id)
            .await;

        self.hydrate_instance_view(
            instance.clone(),
            self.repository.get_catalog_cache(instance.id).await?,
            &package.provider.form_schema,
        )
        .await
    }

    pub async fn update_instance(
        &self,
        command: UpdateModelProviderInstanceCommand,
    ) -> Result<ModelProviderInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", &self.use_case).await?;
        let existing = self
            .repository
            .get_instance(actor.current_workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        let installation = self.ready_installation(existing.installation_id).await?;
        let package = load_installed_provider_package(&installation)?;

        let (patch_public_config, patch_secret_config) =
            split_provider_config(&package.provider.form_schema, &command.config_json)?;
        let current_secret_json = self
            .repository
            .get_secret_json(existing.id, &self.provider_secret_master_key)
            .await?
            .unwrap_or_else(empty_object);
        let merged_public_config = merge_json_object(&existing.config_json, &patch_public_config)?;
        let merged_secret_config = merge_json_object(&current_secret_json, &patch_secret_config)?;
        validate_required_fields(
            &package.provider.form_schema,
            &merged_public_config,
            &merged_secret_config,
        )?;
        let provider_config = merge_json_object(&merged_public_config, &merged_secret_config)?;
        let configured_models =
            normalize_configured_models(command.configured_models, command.enabled_model_ids)?;
        let enabled_model_ids = configured_models_to_enabled_model_ids(&configured_models);

        if !is_empty_object(&patch_secret_config) {
            let version = self
                .repository
                .get_secret_record(existing.id)
                .await?
                .map(|record| record.secret_version + 1)
                .unwrap_or(1);
            self.repository
                .upsert_secret(&UpsertModelProviderSecretInput {
                    provider_instance_id: existing.id,
                    plaintext_secret_json: merged_secret_config.clone(),
                    secret_version: version,
                    master_key: self.provider_secret_master_key.clone(),
                })
                .await?;
        }

        let preview_state = self
            .resolve_preview_state(
                &actor,
                PreviewStateRequest {
                    installation_id: Some(installation.id),
                    instance_id: Some(existing.id),
                    provider_config: &provider_config,
                    preview_token: command.preview_token,
                },
            )
            .await?;
        let next_status = derive_instance_status(
            matches!(
                existing.status,
                domain::ModelProviderInstanceStatus::Disabled
            ),
            &enabled_model_ids,
        );
        ensure_model_provider_instance_transition(existing.status, next_status, "update_instance")?;

        let updated = self
            .repository
            .update_instance(&UpdateModelProviderInstanceInput {
                instance_id: existing.id,
                workspace_id: actor.current_workspace_id,
                display_name: normalize_required_text(&command.display_name, "display_name")?,
                status: next_status,
                config_json: merged_public_config,
                configured_models: configured_models.clone(),
                enabled_model_ids,
                included_in_main: command.included_in_main,
                updated_by: command.actor_user_id,
            })
            .await?;
        if let Some(models_json) = preview_state.models_json.as_ref() {
            self.repository
                .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                    provider_instance_id: existing.id,
                    model_discovery_mode: map_model_discovery_mode(
                        package.provider.model_discovery_mode,
                    ),
                    refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                    source: map_catalog_source(package.provider.model_discovery_mode),
                    models_json: models_json.clone(),
                    last_error_message: None,
                    refreshed_at: preview_state.refreshed_at,
                })
                .await?;
        }
        if let Some(preview_token) = preview_state.preview_token {
            self.repository
                .delete_preview_session(actor.current_workspace_id, preview_token)
                .await?;
        }
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(command.actor_user_id),
                "model_provider_instance",
                Some(updated.id),
                "model_provider.updated",
                json!({
                    "provider_code": updated.provider_code,
                    "display_name": updated.display_name,
                }),
            ))
            .await?;

        self.invalidate_routing_catalog(actor.current_workspace_id)
            .await;

        self.hydrate_instance_view(
            updated,
            self.repository.get_catalog_cache(existing.id).await?,
            &package.provider.form_schema,
        )
        .await
    }

    pub(super) async fn resolve_preview_state(
        &self,
        actor: &domain::ActorContext,
        request: PreviewStateRequest<'_>,
    ) -> Result<ResolvedPreviewState> {
        let Some(preview_token) = request.preview_token else {
            return Ok(ResolvedPreviewState {
                models_json: None,
                refreshed_at: None,
                preview_token: None,
            });
        };

        let session = self
            .repository
            .get_preview_session(actor.current_workspace_id, preview_token)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("preview_token"))?;
        if session.actor_user_id != actor.user_id || session.expires_at < OffsetDateTime::now_utc()
        {
            return Err(ControlPlaneError::InvalidInput("preview_token").into());
        }
        if session.installation_id != request.installation_id
            || session.instance_id != request.instance_id
        {
            return Err(ControlPlaneError::InvalidInput("preview_token").into());
        }
        if session.config_fingerprint != fingerprint_provider_config(request.provider_config)? {
            return Err(ControlPlaneError::InvalidInput("preview_token").into());
        }

        let models_json = session.models_json;
        deserialize_models_json(&models_json)?;

        Ok(ResolvedPreviewState {
            models_json: Some(models_json),
            refreshed_at: Some(OffsetDateTime::now_utc()),
            preview_token: Some(preview_token),
        })
    }

    pub async fn validate_instance(
        &self,
        actor_user_id: Uuid,
        instance_id: Uuid,
    ) -> Result<ValidateModelProviderResult> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_model_provider_permission(&actor, "manage", &self.use_case).await?;
        let instance = self
            .repository
            .get_instance(actor.current_workspace_id, instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("model_provider_instance"))?;
        let installation = self.ready_installation(instance.installation_id).await?;
        if installation.availability_status() != domain::PluginAvailabilityStatus::Available {
            return Err(ControlPlaneError::PluginUnavailable.into());
        }
        let package = load_installed_provider_package(&installation)?;
        ensure_model_provider_instance_transition(
            instance.status,
            domain::ModelProviderInstanceStatus::Ready,
            "validate_instance_success",
        )?;

        let validation_result = async {
            self.ensure_runtime_loaded(&installation).await?;
            let provider_config = self
                .maintain_provider_runtime_config(&package, &installation, &instance)
                .await?;
            let output = self
                .runtime
                .validate_provider(&installation, provider_config.clone())
                .await?;
            let models = self
                .runtime
                .list_models(&installation, provider_config)
                .await?;
            let next_status = derive_instance_status(false, &instance.enabled_model_ids);
            let now = OffsetDateTime::now_utc();
            let cache = self
                .repository
                .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                    provider_instance_id: instance.id,
                    model_discovery_mode: map_model_discovery_mode(
                        package.provider.model_discovery_mode,
                    ),
                    refresh_status: domain::ModelProviderCatalogRefreshStatus::Ready,
                    source: map_catalog_source(package.provider.model_discovery_mode),
                    models_json: serde_json::to_value(&models)?,
                    last_error_message: None,
                    refreshed_at: Some(now),
                })
                .await?;
            ensure_model_provider_instance_transition(
                instance.status,
                next_status,
                "validate_instance_success",
            )?;
            let updated_instance = self
                .repository
                .update_instance(&UpdateModelProviderInstanceInput {
                    instance_id: instance.id,
                    workspace_id: actor.current_workspace_id,
                    display_name: instance.display_name.clone(),
                    status: next_status,
                    config_json: instance.config_json.clone(),
                    configured_models: instance.configured_models.clone(),
                    enabled_model_ids: instance.enabled_model_ids.clone(),
                    included_in_main: instance.included_in_main,
                    updated_by: actor_user_id,
                })
                .await?;
            self.repository
                .append_audit_log(&audit_log(
                    Some(actor.current_workspace_id),
                    Some(actor_user_id),
                    "model_provider_instance",
                    Some(instance.id),
                    "model_provider.validated",
                    json!({
                        "provider_code": instance.provider_code,
                        "model_count": models.len(),
                    }),
                ))
                .await?;
            let masked_view = self
                .hydrate_instance_view(
                    updated_instance,
                    Some(cache.clone()),
                    &package.provider.form_schema,
                )
                .await?;
            Ok::<ValidateModelProviderResult, anyhow::Error>(ValidateModelProviderResult {
                instance: masked_view.instance,
                cache,
                output,
            })
        }
        .await;

        let result = match validation_result {
            Ok(result) => Ok(result),
            Err(error) => {
                let existing_cache = self.repository.get_catalog_cache(instance.id).await?;
                let invalid_transition_allowed = ensure_model_provider_instance_transition(
                    instance.status,
                    domain::ModelProviderInstanceStatus::Invalid,
                    "validate_instance_failure",
                )
                .is_ok();
                let _ = self
                    .repository
                    .upsert_catalog_cache(&UpsertModelProviderCatalogCacheInput {
                        provider_instance_id: instance.id,
                        model_discovery_mode: map_model_discovery_mode(
                            package.provider.model_discovery_mode,
                        ),
                        refresh_status: domain::ModelProviderCatalogRefreshStatus::Failed,
                        source: map_catalog_source(package.provider.model_discovery_mode),
                        models_json: existing_cache
                            .map(|cache| cache.models_json)
                            .unwrap_or_else(|| json!([])),
                        last_error_message: Some(error.to_string()),
                        refreshed_at: None,
                    })
                    .await;
                if invalid_transition_allowed {
                    let _ = self
                        .repository
                        .update_instance(&UpdateModelProviderInstanceInput {
                            instance_id: instance.id,
                            workspace_id: actor.current_workspace_id,
                            display_name: instance.display_name.clone(),
                            status: domain::ModelProviderInstanceStatus::Invalid,
                            config_json: instance.config_json.clone(),
                            configured_models: instance.configured_models.clone(),
                            enabled_model_ids: instance.enabled_model_ids.clone(),
                            included_in_main: instance.included_in_main,
                            updated_by: actor_user_id,
                        })
                        .await;
                }
                let _ = self
                    .repository
                    .append_audit_log(&audit_log(
                        Some(actor.current_workspace_id),
                        Some(actor_user_id),
                        "model_provider_instance",
                        Some(instance.id),
                        "model_provider.validate_failed",
                        json!({
                            "provider_code": instance.provider_code,
                            "message": error.to_string(),
                        }),
                    ))
                    .await;
                Err(error)
            }
        };
        self.invalidate_routing_catalog(actor.current_workspace_id)
            .await;
        result
    }
}
