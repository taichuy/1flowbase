use super::*;

impl<R, H> DataSourceService<R, H>
where
    R: AuthRepository + PluginRepository + DataSourceRepository + ModelDefinitionRepository,
    H: DataSourceRuntimePort,
{
    pub async fn map_resource_to_model(
        &self,
        command: MapDataSourceResourceToModelCommand,
    ) -> Result<MapDataSourceResourceToModelResult> {
        let actor = load_actor_context_for_user(&self.repository, command.actor_user_id).await?;
        ensure_workspace_matches(&actor, command.workspace_id)?;
        self.ensure_console_simple_operation(
            &actor,
            &data_models_console_policy_group(),
            access_control::DATA_SOURCES_MAP_TO_MODEL_OPERATION_ID,
        )
        .await?;

        let (instance, installation, descriptor) = self
            .ready_resource_descriptor(
                command.workspace_id,
                command.instance_id,
                command.resource_key,
            )
            .await?;

        let descriptor_resource_key =
            normalize_required_text(&descriptor.resource_key, "external_resource_key")?;
        let defaults = instance.defaults;
        let status = defaults.data_model_status;
        let model_code = normalize_code_identifier(&descriptor_resource_key, "resource_key")?;
        let model_title = descriptor
            .metadata
            .get("display_name")
            .or_else(|| descriptor.metadata.get("title"))
            .and_then(Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty())
            .unwrap_or(&descriptor_resource_key)
            .to_string();
        let template_source = plugin_framework::DataModelTemplateSource {
            kind: plugin_framework::DataModelSourceKind::ExternalSource,
            provider: Some(instance.source_code.clone()),
        };
        let template_identity = plugin_framework::DataModelTemplateIdentity {
            provider: normalize_required_text(&command.template_provider, "template_provider")?,
            code: normalize_required_text(&command.template_code, "template_code")?,
            version: normalize_required_text(&command.template_version, "template_version")?,
        };
        let compatible = self
            .runtime
            .compatible_data_model_templates(
                &installation,
                &template_source,
                &descriptor.capabilities,
            )
            .await?;
        let selected_template = compatible
            .into_iter()
            .find(|template| template.identity == template_identity)
            .ok_or(ControlPlaneError::InvalidInput(
                "data_model_template_incompatible",
            ))?;

        let model = self
            .repository
            .create_model_definition(&CreateModelDefinitionInput {
                bound_storage_id: None,
                actor_user_id: actor.user_id,
                scope_kind: domain::DataModelScopeKind::System,
                scope_id: domain::SYSTEM_SCOPE_ID,
                data_source_instance_id: Some(instance.id),
                source_kind: domain::DataModelSourceKind::ExternalSource,
                external_resource_key: Some(descriptor_resource_key.clone()),
                external_table_id: None,
                external_capability_snapshot: Some(serde_json::to_value(&descriptor.capabilities)?),
                template_provider: template_identity.provider.clone(),
                template_code: template_identity.code.clone(),
                template_version: template_identity.version.clone(),
                code: model_code,
                title: model_title,
                description: None,
                status,
                protection: domain::DataModelProtection::default(),
            })
            .await?;

        self.repository
            .create_scope_data_model_grant(&CreateScopeDataModelGrantInput {
                grant_id: Uuid::now_v7(),
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: command.workspace_id,
                data_model_id: model.id,
                enabled: true,
                permission_profile: domain::ScopeDataModelPermissionProfile::ScopeAll,
                created_by: Some(actor.user_id),
            })
            .await?;

        let mut fields = Vec::new();
        for schema in descriptor.fields {
            let external_field_key = normalize_required_text(&schema.key, "external_field_key")?;
            let field_code = normalize_code_identifier(&external_field_key, "external_field_key")?;
            let schema_required = schema.required.unwrap_or(false);
            let (is_system, is_writable, api_required) = match selected_template
                .system_fields
                .iter()
                .find(|field| field.code == field_code)
                .map(|field| field.write_policy)
            {
                Some(
                    plugin_framework::DataModelSystemFieldWritePolicy::RuntimeGenerated
                    | plugin_framework::DataModelSystemFieldWritePolicy::RuntimeManaged
                    | plugin_framework::DataModelSystemFieldWritePolicy::DatabaseGenerated
                    | plugin_framework::DataModelSystemFieldWritePolicy::DatabaseManaged
                    | plugin_framework::DataModelSystemFieldWritePolicy::ReadOnlyProjection,
                ) => (true, false, false),
                None => (false, true, schema_required),
            };
            let field = self
                .repository
                .add_model_field(&AddModelFieldInput {
                    actor_user_id: actor.user_id,
                    model_id: model.id,
                    physical_column_name: None,
                    code: field_code,
                    title: field_title(&schema),
                    description: schema.description.clone(),
                    external_field_key: Some(external_field_key),
                    field_kind: model_field_kind_from_schema(&schema),
                    is_system,
                    is_writable,
                    apply_physical_schema: false,
                    is_required: schema_required,
                    api_required,
                    is_unique: descriptor
                        .primary_key
                        .as_deref()
                        .map(|primary_key| primary_key == schema.key)
                        .unwrap_or(false),
                    default_value: schema.default_value.clone(),
                    display_interface: schema.control.clone(),
                    display_options: serde_json::to_value(&schema)?,
                    relation_target_model_id: None,
                    relation_options: json!({}),
                })
                .await?;
            fields.push(field);
        }

        AuthRepository::append_audit_log(
            &self.repository,
            &audit_log(
                Some(command.workspace_id),
                Some(actor.user_id),
                "state_model",
                Some(model.id),
                "data_source.resource_mapped_to_model",
                json!({
                    "data_source_instance_id": instance.id,
                    "resource_key": descriptor_resource_key,
                    "field_count": fields.len(),
                }),
            ),
        )
        .await?;

        Ok(MapDataSourceResourceToModelResult { model, fields })
    }
}
