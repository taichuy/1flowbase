use super::*;

pub(super) fn to_config_field_response(
    field: ProviderConfigField,
) -> ModelProviderConfigFieldResponse {
    ModelProviderConfigFieldResponse {
        key: field.key,
        field_type: field.field_type,
        label: field.label,
        control: field.control,
        required: field.required,
        advanced: field.advanced,
        description: field.description,
        placeholder: field.placeholder,
        default_value: field.default_value,
        options: field
            .options
            .into_iter()
            .map(to_plugin_form_option_response)
            .collect(),
    }
}

pub(super) fn to_model_descriptor_response(
    model: LocalizedProviderModelDescriptor,
) -> ProviderModelDescriptorResponse {
    let descriptor = model.descriptor;
    ProviderModelDescriptorResponse {
        model_id: descriptor.model_id,
        display_name: descriptor.display_name.clone(),
        namespace: model.namespace,
        label_key: model.label_key,
        description_key: model.description_key,
        display_name_fallback: model
            .display_name_fallback
            .or_else(|| Some(descriptor.display_name.clone())),
        source: format!("{:?}", descriptor.source).to_ascii_lowercase(),
        supports_streaming: descriptor.supports_streaming,
        supports_tool_call: descriptor.supports_tool_call,
        supports_multimodal: descriptor.supports_multimodal,
        context_window: descriptor.context_window,
        max_output_tokens: descriptor.max_output_tokens,
        provider_metadata: descriptor.provider_metadata,
    }
}

pub(super) fn to_plugin_form_option_response(option: PluginFormOption) -> PluginFormOptionResponse {
    PluginFormOptionResponse {
        label: option.label,
        value: option.value,
        description: option.description,
        disabled: option.disabled,
    }
}

pub(super) fn to_plugin_form_condition_response(
    condition: PluginFormCondition,
) -> PluginFormConditionResponse {
    PluginFormConditionResponse {
        field: condition.field,
        operator: condition.operator,
        value: condition.value,
        values: condition.values,
    }
}

pub(super) fn to_plugin_form_field_schema_response(
    field: PluginFormFieldSchema,
) -> PluginFormFieldSchemaResponse {
    PluginFormFieldSchemaResponse {
        key: field.key,
        label: field.label,
        field_type: field.field_type,
        control: field.control,
        group: field.group,
        order: field.order,
        advanced: field.advanced,
        required: field.required,
        send_mode: field.send_mode,
        enabled_by_default: field.enabled_by_default,
        description: field.description,
        placeholder: field.placeholder,
        default_value: field.default_value,
        min: field.min,
        max: field.max,
        step: field.step,
        precision: field.precision,
        unit: field.unit,
        options: field
            .options
            .into_iter()
            .map(to_plugin_form_option_response)
            .collect(),
        visible_when: field
            .visible_when
            .into_iter()
            .map(to_plugin_form_condition_response)
            .collect(),
        disabled_when: field
            .disabled_when
            .into_iter()
            .map(to_plugin_form_condition_response)
            .collect(),
    }
}

pub(super) fn to_plugin_form_schema_response(schema: PluginFormSchema) -> PluginFormSchemaResponse {
    PluginFormSchemaResponse {
        schema_version: schema.schema_version,
        title: schema.title,
        description: schema.description,
        fields: schema
            .fields
            .into_iter()
            .map(to_plugin_form_field_schema_response)
            .collect(),
    }
}

pub(super) fn to_catalog_response(
    entry: ModelProviderCatalogEntry,
) -> Result<ModelProviderCatalogEntryResponse, ApiError> {
    Ok(ModelProviderCatalogEntryResponse {
        installation_id: entry.installation_id.to_string(),
        provider_code: entry.provider_code,
        plugin_id: entry.plugin_id,
        plugin_version: entry.plugin_version,
        plugin_type: entry.plugin_type,
        namespace: entry.namespace,
        label_key: entry.label_key,
        description_key: entry.description_key,
        display_name: entry.display_name,
        protocol: entry.protocol,
        help_url: entry.help_url,
        default_base_url: entry.default_base_url,
        model_discovery_mode: entry.model_discovery_mode,
        supports_model_fetch_without_credentials: entry.supports_model_fetch_without_credentials,
        desired_state: entry.desired_state,
        availability_status: entry.availability_status,
        form_schema: entry
            .form_schema
            .into_iter()
            .map(to_config_field_response)
            .collect(),
        auth: entry.auth.map(to_auth_projection_response),
        operational_capabilities: entry
            .operational_capabilities
            .into_iter()
            .map(provider_operational_capability)
            .collect(),
        predefined_models: entry
            .predefined_models
            .into_iter()
            .map(to_model_descriptor_response)
            .collect(),
        catalog_refresh_status: entry.catalog_refresh_status,
        catalog_last_error_message: entry.catalog_last_error_message,
        catalog_refreshed_at: format_optional_time(entry.catalog_refreshed_at)?,
    })
}

pub(super) fn provider_operational_capability(value: ProviderOperationalCapability) -> String {
    match value {
        ProviderOperationalCapability::ConfigurationValidation => "validate_config",
        ProviderOperationalCapability::ModelListing => "list_models",
        ProviderOperationalCapability::UsageWindows => "usage_windows",
        ProviderOperationalCapability::ResetCredits => "reset_credits",
    }
    .to_string()
}

pub(super) fn provider_auth_user_action_kind(value: ProviderAuthUserActionKind) -> String {
    match value {
        ProviderAuthUserActionKind::DeviceCode => "device_code",
        ProviderAuthUserActionKind::PasteCallbackUrl => "paste_callback_url",
    }
    .to_string()
}

pub(super) fn provider_auth_status(value: ProviderAuthStatus) -> String {
    match value {
        ProviderAuthStatus::Pending => "pending",
        ProviderAuthStatus::Authorized => "authorized",
        ProviderAuthStatus::Failed => "failed",
        ProviderAuthStatus::Cancelled => "cancelled",
    }
    .to_string()
}

pub(super) fn to_auth_projection_response(
    auth: plugin_framework::provider_contract::ProviderAuthManifest,
) -> ModelProviderAuthProjectionResponse {
    ModelProviderAuthProjectionResponse {
        actions: auth
            .actions
            .into_iter()
            .map(|action| ModelProviderAuthActionResponse {
                code: action.code,
                label: action.label,
                user_action_kinds: action
                    .user_action_kinds
                    .into_iter()
                    .map(provider_auth_user_action_kind)
                    .collect(),
            })
            .collect(),
    }
}

pub(super) fn to_authenticate_response(
    result: AuthenticateModelProviderInstanceResult,
) -> Result<AuthenticateModelProviderInstanceResponse, ApiError> {
    Ok(AuthenticateModelProviderInstanceResponse {
        instance: to_instance_response(ModelProviderInstanceView {
            instance: result.instance,
            cache: None,
        })?,
        status: provider_auth_status(result.result.status),
        message: result.result.message,
        user_action: result
            .result
            .user_action
            .map(|action| ModelProviderAuthUserActionResponse {
                kind: provider_auth_user_action_kind(action.kind),
                open_url: action.open_url,
                user_code: action.user_code,
                expires_at: action.expires_at,
                poll_interval_seconds: action.poll_interval_seconds,
                prompt: action.prompt,
            }),
    })
}

pub(super) fn to_catalog_view_response(
    locale_meta: LocaleMetaResponse,
    catalog: ModelProviderCatalogView,
) -> Result<ModelProviderCatalogResponse, ApiError> {
    Ok(ModelProviderCatalogResponse {
        locale_meta,
        i18n_catalog: serde_json::to_value(catalog.i18n_catalog).unwrap(),
        entries: catalog
            .entries
            .into_iter()
            .map(to_catalog_response)
            .collect::<Result<Vec<_>, _>>()?,
    })
}

pub(super) fn to_runtime_model_descriptor_response(
    model: ProviderModelDescriptor,
) -> ProviderModelDescriptorResponse {
    let display_name = model.display_name.clone();
    ProviderModelDescriptorResponse {
        model_id: model.model_id,
        display_name: display_name.clone(),
        namespace: None,
        label_key: None,
        description_key: None,
        display_name_fallback: Some(display_name),
        source: format!("{:?}", model.source).to_ascii_lowercase(),
        supports_streaming: model.supports_streaming,
        supports_tool_call: model.supports_tool_call,
        supports_multimodal: model.supports_multimodal,
        context_window: model.context_window,
        max_output_tokens: model.max_output_tokens,
        provider_metadata: model.provider_metadata,
    }
}

pub(super) fn to_instance_response(
    view: ModelProviderInstanceView,
) -> Result<ModelProviderInstanceResponse, ApiError> {
    let model_count = view
        .cache
        .as_ref()
        .and_then(|cache| cache.models_json.as_array().map(|items| items.len()))
        .unwrap_or(0);
    Ok(ModelProviderInstanceResponse {
        id: view.instance.id.to_string(),
        installation_id: view.instance.installation_id.to_string(),
        provider_code: view.instance.provider_code,
        protocol: view.instance.protocol,
        display_name: view.instance.display_name,
        status: view.instance.status.as_str().to_string(),
        included_in_main: view.instance.included_in_main,
        config_json: view.instance.config_json,
        configured_models: view
            .instance
            .configured_models
            .into_iter()
            .map(|model| ConfiguredModelResponse {
                model_id: model.model_id,
                enabled: model.enabled,
                context_window_override_tokens: model.context_window_override_tokens,
                supports_multimodal: model.supports_multimodal,
                pricing_provider_code: model.pricing_provider_code,
                pricing_model_id: model.pricing_model_id,
            })
            .collect(),
        enabled_model_ids: view.instance.enabled_model_ids,
        catalog_refresh_status: view
            .cache
            .as_ref()
            .map(|cache| cache.refresh_status.as_str().to_string()),
        catalog_last_error_message: view
            .cache
            .as_ref()
            .and_then(|cache| cache.last_error_message.clone()),
        catalog_refreshed_at: format_optional_time(
            view.cache.as_ref().and_then(|cache| cache.refreshed_at),
        )?,
        model_count,
    })
}

pub(super) fn to_validate_response(
    result: ValidateModelProviderResult,
) -> Result<ValidateModelProviderResponse, ApiError> {
    Ok(ValidateModelProviderResponse {
        instance: to_instance_response(ModelProviderInstanceView {
            instance: result.instance,
            cache: Some(result.cache),
        })?,
        output: result.output,
    })
}

pub(super) fn to_balance_response(
    result: ModelProviderBalanceResult,
) -> ModelProviderBalanceResponse {
    ModelProviderBalanceResponse {
        is_available: result.is_available,
        balance_infos: result
            .balance_infos
            .into_iter()
            .map(|info| ModelProviderBalanceInfoResponse {
                currency: info.currency,
                total_balance: info.total_balance,
                granted_balance: info.granted_balance,
                topped_up_balance: info.topped_up_balance,
            })
            .collect(),
        provider_metadata: result.provider_metadata,
    }
}

pub(super) fn to_usage_windows_response(
    result: ModelProviderUsageWindowsResult,
) -> ModelProviderUsageWindowsResponse {
    ModelProviderUsageWindowsResponse {
        windows: result
            .windows
            .into_iter()
            .map(|window| ModelProviderUsageWindowResponse {
                limit_window_seconds: window.limit_window_seconds,
                used_percent: window.used_percent,
                reset_at: window.reset_at,
            })
            .collect(),
        queried_at: result.queried_at,
    }
}

pub(super) fn to_reset_credit_count_response(
    result: ModelProviderResetCreditCount,
) -> ModelProviderResetCreditCountResponse {
    ModelProviderResetCreditCountResponse {
        available_count: result.available_count,
    }
}

pub(super) fn to_consume_reset_credit_response(
    result: ConsumeModelProviderResetCreditResult,
) -> ConsumeModelProviderResetCreditResponse {
    ConsumeModelProviderResetCreditResponse {
        consumed: result.consumed,
    }
}

pub(super) fn to_main_instance_response(
    view: ModelProviderMainInstanceView,
    distribution_rules: Vec<
        plugin_framework::provider_distribution_registry::ProviderDistributionRuleDefinition,
    >,
) -> ModelProviderMainInstanceResponse {
    ModelProviderMainInstanceResponse {
        provider_code: view.provider_code,
        auto_include_new_instances: view.auto_include_new_instances,
        revision: view.revision,
        model_routing_policies: view
            .model_routing_policies
            .into_iter()
            .map(to_main_model_routing_policy_response)
            .collect(),
        distribution_rules: distribution_rules
            .into_iter()
            .map(to_distribution_rule_definition_response)
            .collect(),
    }
}

pub(super) fn to_distribution_rule_definition_response(
    definition: plugin_framework::provider_distribution_registry::ProviderDistributionRuleDefinition,
) -> ModelProviderDistributionRuleDefinitionResponse {
    let value = match &definition.handler {
        plugin_framework::provider_distribution_registry::ProviderDistributionHandlerRef::Builtin { code } => code.clone(),
        plugin_framework::provider_distribution_registry::ProviderDistributionHandlerRef::RuntimeExtension { .. } => definition.rule_id.clone(),
    };
    ModelProviderDistributionRuleDefinitionResponse {
        value,
        rule_id: definition.rule_id,
        rule_version: definition.rule_version,
        contract_version: definition.contract_version,
        display_name: definition.display_name,
        config_fields: definition
            .config_fields
            .into_iter()
            .map(
                |(key, field)| ModelProviderDistributionConfigFieldResponse {
                    key,
                    value_type: match field.value_type {
                        extension_contracts::ProviderDistributionConfigValueType::String => {
                            "string"
                        }
                        extension_contracts::ProviderDistributionConfigValueType::Integer => {
                            "integer"
                        }
                        extension_contracts::ProviderDistributionConfigValueType::Boolean => {
                            "boolean"
                        }
                    }
                    .to_string(),
                    required: field.required,
                },
            )
            .collect(),
    }
}

pub(super) fn to_main_model_routing_policy_response(
    policy: domain::ModelProviderMainModelRoutingPolicy,
) -> ModelProviderMainModelRoutingPolicyResponse {
    let distribution_rule_config = policy
        .distribution_rule
        .config()
        .iter()
        .map(|(key, value)| {
            let value = match value {
                domain::ModelProviderDistributionConfigValue::String(value) => {
                    ModelProviderDistributionConfigValueBody::String(value.clone())
                }
                domain::ModelProviderDistributionConfigValue::Integer(value) => {
                    ModelProviderDistributionConfigValueBody::Integer(*value)
                }
                domain::ModelProviderDistributionConfigValue::Boolean(value) => {
                    ModelProviderDistributionConfigValueBody::Boolean(*value)
                }
            };
            (key.clone(), value)
        })
        .collect();
    ModelProviderMainModelRoutingPolicyResponse {
        model_id: policy.model_id,
        distribution_rule: policy.distribution_rule.as_str().to_string(),
        distribution_rule_id: policy.distribution_rule.rule_id().to_string(),
        distribution_rule_contract_version: policy.distribution_rule.contract_version().to_string(),
        distribution_rule_config,
        provider_instance_ids: policy.provider_instance_ids,
        excluded_provider_instance_ids: policy.excluded_provider_instance_ids,
    }
}

pub(super) fn to_main_model_routing_policy(
    policy: ModelProviderMainModelRoutingPolicyBody,
    rule_version: Option<String>,
) -> Result<domain::ModelProviderMainModelRoutingPolicy, ApiError> {
    Ok(domain::ModelProviderMainModelRoutingPolicy {
        model_id: policy.model_id,
        distribution_rule: parse_distribution_rule(
            &policy.distribution_rule,
            rule_version,
            policy.distribution_rule_contract_version,
            policy.distribution_rule_config,
        )?,
        provider_instance_ids: policy.provider_instance_ids,
        excluded_provider_instance_ids: policy.excluded_provider_instance_ids,
    })
}

pub(super) fn to_model_catalog_response(
    catalog: ModelProviderModelCatalog,
) -> Result<ModelProviderModelCatalogResponse, ApiError> {
    Ok(ModelProviderModelCatalogResponse {
        provider_instance_id: catalog.provider_instance_id.to_string(),
        refresh_status: catalog.refresh_status.as_str().to_string(),
        source: catalog.source.as_str().to_string(),
        last_error_message: catalog.last_error_message,
        refreshed_at: format_optional_time(catalog.refreshed_at)?,
        models: catalog
            .models
            .into_iter()
            .map(to_runtime_model_descriptor_response)
            .collect(),
    })
}

pub(super) fn to_option_response(
    option: ModelProviderOptionEntry,
    priced_models: &HashSet<(String, String)>,
) -> ModelProviderOptionResponse {
    let provider_code = option.provider_code.clone();
    ModelProviderOptionResponse {
        icon: normalize_provider_icon(&option.provider_code, option.icon),
        provider_code: option.provider_code,
        plugin_type: option.plugin_type,
        namespace: option.namespace,
        label_key: option.label_key,
        description_key: option.description_key,
        protocol: option.protocol,
        display_name: option.display_name,
        parameter_form: option.parameter_form.map(to_plugin_form_schema_response),
        main_instance: ModelProviderMainInstanceSummaryResponse {
            provider_code: option.main_instance.provider_code,
            auto_include_new_instances: option.main_instance.auto_include_new_instances,
            group_count: option.main_instance.group_count,
            model_count: option.main_instance.model_count,
        },
        model_groups: option
            .model_groups
            .into_iter()
            .map(|group| ModelProviderOptionGroupResponse {
                pricing_configured: priced_models
                    .contains(&(provider_code.clone(), group.model_id.clone())),
                model_id: group.model_id,
                distribution_rule: group.distribution_rule.as_str().to_string(),
                model: to_model_descriptor_response(group.model),
                targets: group
                    .targets
                    .into_iter()
                    .map(|target| ModelProviderOptionTargetResponse {
                        source_instance_id: target.source_instance_id.to_string(),
                        source_instance_display_name: target.source_instance_display_name,
                        routing_enabled: target.routing_enabled,
                        model: to_model_descriptor_response(target.model),
                    })
                    .collect(),
            })
            .collect(),
    }
}

pub(super) fn to_options_view_response(
    locale_meta: LocaleMetaResponse,
    options: ModelProviderOptionsView,
    pricing_targets: Vec<ModelProviderPricingTargetResponse>,
) -> ModelProviderOptionsResponse {
    let priced_models = pricing_targets
        .iter()
        .map(|target| {
            (
                target.provider_code.clone(),
                target.upstream_model_id.clone(),
            )
        })
        .collect::<HashSet<_>>();
    ModelProviderOptionsResponse {
        locale_meta,
        i18n_catalog: serde_json::to_value(options.i18n_catalog).unwrap(),
        providers: options
            .providers
            .into_iter()
            .map(|option| to_option_response(option, &priced_models))
            .collect(),
        pricing_targets,
    }
}
