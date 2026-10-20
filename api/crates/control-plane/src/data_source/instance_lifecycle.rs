use super::*;

impl<R, H> DataSourceService<R, H>
where
    R: AuthRepository + PluginRepository + DataSourceRepository + ModelDefinitionRepository,
    H: DataSourceRuntimePort,
{
    pub async fn create_instance(
        &self,
        command: CreateDataSourceInstanceCommand,
    ) -> Result<DataSourceInstanceView> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_CREATE_OPERATION_ID,
        )
        .await?;

        let installation = self.ready_installation(command.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            command.workspace_id,
            command.installation_id,
        )
        .await?;
        ensure_data_source_installation(&installation, &command.source_code)?;

        let config_schema = load_data_source_config_schema(&installation).await?;

        let instance_id = Uuid::now_v7();
        let secret_ref = domain::data_source_secret_ref(instance_id);
        let (config_json, secret_json) = classify_data_source_config(
            &config_schema,
            &command.config_json,
            &command.secret_json,
            &secret_ref,
            1,
        )?;

        let instance = self
            .repository
            .create_instance(&CreateDataSourceInstanceInput {
                instance_id,
                workspace_id: command.workspace_id,
                installation_id: command.installation_id,
                source_code: normalize_required_text(&command.source_code, "source_code")?,
                display_name: normalize_required_text(&command.display_name, "display_name")?,
                status: domain::DataSourceInstanceStatus::Draft,
                config_json,
                metadata_json: json!({}),
                defaults: domain::DataSourceDefaults::default(),
                created_by: actor.user_id,
            })
            .await?;

        let instance = if secret_json
            .as_object()
            .is_some_and(|secrets| !secrets.is_empty())
        {
            self.repository
                .upsert_secret(&UpsertDataSourceSecretInput {
                    data_source_instance_id: instance.id,
                    secret_ref,
                    plaintext_secret_json: secret_json,
                    master_key: self.secret_master_key.clone(),
                    secret_version: 1,
                })
                .await?;
            self.repository
                .get_instance(command.workspace_id, instance.id)
                .await?
                .unwrap_or(instance)
        } else {
            instance
        };

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.instance_created",
                json!({
                    "installation_id": command.installation_id,
                    "source_code": instance.source_code,
                    "secret_ref": instance.secret_ref.clone(),
                    "secret_version": instance.secret_version,
                }),
            ),
        )
        .await?;

        Ok(DataSourceInstanceView {
            instance,
            catalog: None,
        })
    }

    pub async fn validate_instance(
        &self,
        command: ValidateDataSourceInstanceCommand,
    ) -> Result<ValidateDataSourceInstanceResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_VALIDATE_OPERATION_ID,
        )
        .await?;

        let existing = self
            .repository
            .get_instance(command.workspace_id, command.instance_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("data_source_instance"))?;
        let installation = self.ready_installation(existing.installation_id).await?;
        ensure_installation_assigned(
            &self.repository,
            command.workspace_id,
            existing.installation_id,
        )
        .await?;

        let secret_json = self
            .repository
            .get_secret_json(existing.id, &self.secret_master_key)
            .await?
            .unwrap_or_else(|| json!({}));

        self.ensure_runtime_loaded(&installation).await?;
        let secret_values = collect_secret_strings(&secret_json);
        let output = self
            .runtime
            .validate_config(
                &installation,
                existing.config_json.clone(),
                secret_json.clone(),
            )
            .await?;
        let output = redact_value(&output, &secret_values);
        self.runtime
            .test_connection(
                &installation,
                existing.config_json.clone(),
                secret_json.clone(),
            )
            .await?;

        let instance = self
            .repository
            .update_instance_status(&UpdateDataSourceInstanceStatusInput {
                workspace_id: command.workspace_id,
                instance_id: existing.id,
                status: domain::DataSourceInstanceStatus::Ready,
                metadata_json: existing.metadata_json.clone(),
                updated_by: actor.user_id,
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.instance_validated",
                json!({
                    "status": instance.status.as_str(),
                }),
            ),
        )
        .await?;

        Ok(ValidateDataSourceInstanceResult { instance, output })
    }

    pub async fn update_defaults(
        &self,
        command: UpdateDataSourceDefaultsCommand,
    ) -> Result<domain::DataSourceInstanceRecord> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID,
        )
        .await?;
        ensure_data_source_defaults_compatible(command.defaults)?;

        let instance = self
            .repository
            .update_instance_defaults(&UpdateDataSourceDefaultsInput {
                workspace_id: command.workspace_id,
                instance_id: command.instance_id,
                defaults: command.defaults,
                updated_by: actor.user_id,
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                Some(instance.id),
                "data_source.defaults_updated",
                json!({
                    "default_data_model_status": instance.defaults.data_model_status.as_str(),
                }),
            ),
        )
        .await?;

        Ok(instance)
    }

    pub async fn get_main_source_defaults(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<domain::DataSourceDefaults> {
        let actor = load_actor_context_for_user(&self.repository, actor_user_id).await?;
        ensure_workspace_matches(&actor, workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_LIST_OPERATION_ID,
        )
        .await?;
        DataSourceRepository::get_main_source_defaults(&self.repository, workspace_id).await
    }

    pub async fn update_main_data_source_defaults(
        &self,
        command: UpdateMainDataSourceDefaultsCommand,
    ) -> Result<domain::DataSourceDefaults> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_DEFAULTS_UPDATE_OPERATION_ID,
        )
        .await?;
        ensure_data_source_defaults_compatible(command.defaults)?;

        let defaults = self
            .repository
            .update_main_source_defaults(&UpdateMainSourceDefaultsInput {
                workspace_id: command.workspace_id,
                defaults: command.defaults,
                updated_by: actor.user_id,
            })
            .await?;

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "data_source_instance",
                None,
                "data_source.main_source_defaults_updated",
                json!({
                    "default_data_model_status": defaults.data_model_status.as_str(),
                }),
            ),
        )
        .await?;

        Ok(defaults)
    }
}
