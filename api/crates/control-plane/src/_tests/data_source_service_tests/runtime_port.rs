use super::*;

#[async_trait]
impl DataSourceRuntimePort for StubDataSourceRuntime {
    async fn ensure_loaded(&self, _installation: &LocalPluginInstallationRecord) -> Result<()> {
        Ok(())
    }

    async fn compatible_data_model_templates(
        &self,
        _installation: &LocalPluginInstallationRecord,
        source: &plugin_framework::DataModelTemplateSource,
        capabilities: &plugin_framework::DataSourceCrudCapabilities,
    ) -> Result<Vec<plugin_framework::DataModelTemplateDescriptor>> {
        let template = stub_external_template();
        let live_capabilities = runtime_core::general_data_model_template::source_capabilities(
            source,
            Some(capabilities),
        );
        let mut compatible =
            runtime_core::data_model_template_registry::DataModelTemplateCatalog::core()
                .compatible_templates(source, live_capabilities.iter().map(String::as_str))
                .into_iter()
                .map(|template| template.descriptor().clone())
                .collect::<Vec<_>>();
        if template.source_selector.matches(source)
            && template
                .required_capabilities
                .iter()
                .all(|required| live_capabilities.contains(&required.code))
        {
            compatible.push(template);
        }
        Ok(compatible)
    }

    async fn validate_config(
        &self,
        _installation: &LocalPluginInstallationRecord,
        _config_json: Value,
        secret_json: Value,
    ) -> Result<Value> {
        if self.echo_secret_output {
            let secret = secret_json["client_secret"].as_str().unwrap_or_default();
            return Ok(json!({
                "ok": true,
                "echoed": secret_json["client_secret"].clone(),
                "authorization": format!("Bearer {secret}"),
                "nested": {
                    "token": secret_json["client_secret"].clone(),
                    "authorization": format!("Token {secret}"),
                }
            }));
        }
        Ok(json!({ "ok": true }))
    }

    async fn test_connection(
        &self,
        _installation: &LocalPluginInstallationRecord,
        _config_json: Value,
        _secret_json: Value,
    ) -> Result<Value> {
        Ok(json!({ "status": "ok" }))
    }

    async fn discover_catalog(
        &self,
        _installation: &LocalPluginInstallationRecord,
        _config_json: Value,
        secret_json: Value,
    ) -> Result<Value> {
        if self.echo_secret_output {
            let secret = secret_json["client_secret"].as_str().unwrap_or_default();
            return Ok(serde_json::to_value(vec![DataSourceCatalogEntry {
                resource_key: "contacts".to_string(),
                display_name: "Contacts".to_string(),
                resource_kind: "object".to_string(),
                capabilities: Default::default(),
                metadata: json!({
                    "authorization": format!("Bearer {secret}"),
                    "nested": {
                        "token": secret_json["client_secret"].clone(),
                    },
                }),
            }])?);
        }
        Ok(serde_json::to_value(vec![DataSourceCatalogEntry {
            resource_key: "contacts".to_string(),
            display_name: "Contacts".to_string(),
            resource_kind: "object".to_string(),
            capabilities: Default::default(),
            metadata: json!({}),
        }])?)
    }

    async fn describe_resource(
        &self,
        _installation: &LocalPluginInstallationRecord,
        input: DataSourceDescribeResourceInput,
    ) -> Result<DataSourceResourceDescriptor> {
        let echoed_secret = input.connection.secret_json["client_secret"].clone();
        self.describe_inputs.write().await.push(input.clone());
        if self.echo_secret_output {
            let secret = echoed_secret.as_str().unwrap_or_default();
            return Ok(DataSourceResourceDescriptor {
                resource_key: format!("{}-{secret}", input.resource_key),
                primary_key: Some("id".to_string()),
                fields: vec![
                    PluginFormFieldSchema {
                        key: "id".to_string(),
                        label: format!("Record {secret} ID"),
                        field_type: "string".to_string(),
                        control: None,
                        group: None,
                        order: Some(0),
                        advanced: None,
                        required: Some(true),
                        send_mode: None,
                        enabled_by_default: None,
                        description: Some(format!("Primary key {secret}")),
                        placeholder: None,
                        default_value: None,
                        min: None,
                        max: None,
                        step: None,
                        precision: None,
                        unit: None,
                        options: vec![],
                        visible_when: vec![],
                        disabled_when: vec![],
                    },
                    PluginFormFieldSchema {
                        key: format!("properties.email.{secret}"),
                        label: format!("Email {secret}"),
                        field_type: "email".to_string(),
                        control: None,
                        group: None,
                        order: Some(1),
                        advanced: None,
                        required: Some(false),
                        send_mode: None,
                        enabled_by_default: None,
                        description: None,
                        placeholder: None,
                        default_value: Some(json!({ "echoed": secret })),
                        min: None,
                        max: None,
                        step: None,
                        precision: None,
                        unit: None,
                        options: vec![],
                        visible_when: vec![],
                        disabled_when: vec![],
                    },
                ],
                supports_preview_read: true,
                supports_import_snapshot: false,
                capabilities: plugin_framework::data_source_contract::DataSourceCrudCapabilities {
                    supports_list: true,
                    supports_get: true,
                    supports_filter: true,
                    supports_scope_filter: true,
                    ..Default::default()
                },
                metadata: json!({
                    "display_name": format!("Contacts {secret}"),
                    "authorization": format!("Bearer {secret}"),
                    "nested": {
                        "token": echoed_secret,
                    },
                }),
            });
        }
        Ok(DataSourceResourceDescriptor {
            resource_key: input.resource_key,
            primary_key: Some("id".to_string()),
            fields: vec![
                PluginFormFieldSchema {
                    key: "id".to_string(),
                    label: "Record ID".to_string(),
                    field_type: "string".to_string(),
                    control: None,
                    group: None,
                    order: Some(0),
                    advanced: None,
                    required: Some(true),
                    send_mode: None,
                    enabled_by_default: None,
                    description: None,
                    placeholder: None,
                    default_value: None,
                    min: None,
                    max: None,
                    step: None,
                    precision: None,
                    unit: None,
                    options: vec![],
                    visible_when: vec![],
                    disabled_when: vec![],
                },
                PluginFormFieldSchema {
                    key: "properties.email".to_string(),
                    label: "Email".to_string(),
                    field_type: "email".to_string(),
                    control: None,
                    group: None,
                    order: Some(1),
                    advanced: None,
                    required: Some(true),
                    send_mode: None,
                    enabled_by_default: None,
                    description: None,
                    placeholder: None,
                    default_value: None,
                    min: None,
                    max: None,
                    step: None,
                    precision: None,
                    unit: None,
                    options: vec![],
                    visible_when: vec![],
                    disabled_when: vec![],
                },
            ],
            supports_preview_read: true,
            supports_import_snapshot: false,
            capabilities: plugin_framework::data_source_contract::DataSourceCrudCapabilities {
                supports_list: true,
                supports_get: true,
                supports_filter: true,
                supports_scope_filter: true,
                ..Default::default()
            },
            metadata: json!({ "display_name": "Contacts" }),
        })
    }

    async fn preview_read(
        &self,
        _installation: &LocalPluginInstallationRecord,
        input: DataSourcePreviewReadInput,
    ) -> Result<DataSourcePreviewReadOutput> {
        let echoed_secret = input.connection.secret_json["client_secret"].clone();
        self.preview_inputs.write().await.push(input);
        if self.echo_secret_output {
            let secret = echoed_secret.as_str().unwrap_or_default();
            return Ok(DataSourcePreviewReadOutput {
                rows: vec![json!({
                    "id": "1",
                    "token": echoed_secret,
                    "authorization": format!("Bearer {secret}"),
                    "nested": { "secret": echoed_secret },
                    "items": [echoed_secret]
                })],
                next_cursor: None,
            });
        }
        Ok(DataSourcePreviewReadOutput {
            rows: vec![json!({ "id": "1", "email": "person@example.com" })],
            next_cursor: None,
        })
    }

    async fn execute_model_operation(
        &self,
        _installation: &LocalPluginInstallationRecord,
        _input: plugin_framework::DataSourceExecuteModelOperationInput,
    ) -> Result<Value> {
        Ok(json!({ "archived": true, "contact_id": "contact-1" }))
    }
}
