use super::*;

pub(super) fn validate_contract_version(manifest: &PluginManifestV1) -> FrameworkResult<()> {
    let expected = match manifest.consumption_kind {
        PluginConsumptionKind::HostExtension => "1flowbase.host_extension/v1",
        PluginConsumptionKind::RuntimeExtension => {
            if manifest
                .slot_codes
                .iter()
                .any(|slot| slot == "network_egress_provider")
            {
                crate::NETWORK_EGRESS_PROVIDER_CONTRACT
            } else if manifest
                .slot_codes
                .iter()
                .any(|slot| slot == extension_contracts::PROVIDER_DISTRIBUTION_RULE_SLOT)
            {
                extension_contracts::PROVIDER_DISTRIBUTION_RULE_CONTRACT_V1
            } else if manifest
                .slot_codes
                .iter()
                .any(|slot| matches!(slot.as_str(), "data_source" | "data_import_snapshot"))
            {
                "1flowbase.data_source/v1"
            } else {
                CURRENT_PROVIDER_CONTRACT
            }
        }
        PluginConsumptionKind::CapabilityPlugin => "1flowbase.capability/v1",
    };

    if manifest.contract_version == expected {
        return Ok(());
    }

    Err(PluginFrameworkError::invalid_provider_package(format!(
        "contract_version must be {expected} for {}",
        manifest.consumption_kind.as_str()
    )))
}

pub(super) fn validate_contract_version_for_policy(
    manifest: &PluginManifestV1,
    contract_policy: ManifestContractPolicy,
) -> FrameworkResult<()> {
    if matches!(
        contract_policy,
        ManifestContractPolicy::PublisherCutoverLegacyProvider
    ) && manifest.consumption_kind == PluginConsumptionKind::RuntimeExtension
        && manifest
            .slot_codes
            .iter()
            .any(|slot| slot == "model_provider")
        && manifest.contract_version == "1flowbase.provider/v1"
    {
        return Ok(());
    }

    validate_contract_version(manifest)
}

pub(super) fn validate_provider_runtime_capabilities(
    manifest: &PluginManifestV1,
    contract_policy: ManifestContractPolicy,
) -> FrameworkResult<()> {
    let is_model_provider = manifest.consumption_kind == PluginConsumptionKind::RuntimeExtension
        && manifest
            .slot_codes
            .iter()
            .any(|slot| slot == "model_provider");
    let is_distribution_rule = manifest.consumption_kind == PluginConsumptionKind::RuntimeExtension
        && manifest
            .slot_codes
            .iter()
            .any(|slot| slot == extension_contracts::PROVIDER_DISTRIBUTION_RULE_SLOT);
    if !is_model_provider && !is_distribution_rule && !manifest.runtime.capabilities.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "runtime.capabilities is only supported for model_provider runtime extensions",
        ));
    }

    let mut seen = HashSet::new();
    for capability in &manifest.runtime.capabilities {
        if is_distribution_rule {
            validate_allowed(
                capability,
                "runtime.capabilities[]",
                &[extension_contracts::RUNTIME_HOST_CALL_CAPABILITY_V1],
            )?;
            if !seen.insert(capability.as_str()) {
                return Err(PluginFrameworkError::invalid_provider_package(format!(
                    "runtime.capabilities contains duplicate value: {capability}"
                )));
            }
            continue;
        }
        // Publisher-cutover receipts bind these bytes to durable identity; bare
        // `protocol_context` remains rejected by every package intake path.
        let is_legacy_protocol_context = capability == "protocol_context"
            && matches!(
                contract_policy,
                ManifestContractPolicy::PublisherCutoverLegacyProvider
            );
        if !is_legacy_protocol_context {
            validate_allowed(
                capability,
                "runtime.capabilities[]",
                &[
                    "system_prompt_blocks",
                    "system_prompt_cache_control",
                    "end_user_reference",
                    PROVIDER_CONFIGURATION_VALIDATION_CAPABILITY,
                    PROVIDER_MODEL_LISTING_CAPABILITY,
                    PROVIDER_COUNT_TOKENS_CAPABILITY,
                    PROVIDER_USAGE_WINDOWS_CAPABILITY,
                    PROVIDER_RESET_CREDITS_CAPABILITY,
                    PROVIDER_COMPACT_RESPONSES_COMPACT_CAPABILITY,
                    PROVIDER_COMPACT_RESPONSES_COMPACTION_V2_CAPABILITY,
                    PROVIDER_RESPONSES_NATIVE_PASSTHROUGH_CAPABILITY,
                    extension_contracts::provider_contract::ProviderInvocationCapability::ResponsesNativeOutputV1
                        .manifest_capability_name(),
                    PROVIDER_REASONING_OUTPUT_SUPPORTED_CAPABILITY,
                    PROVIDER_REASONING_HISTORY_INPUT_SUPPORTED_CAPABILITY,
                    PROVIDER_NATIVE_CONTINUATION_SUPPORTED_CAPABILITY,
                    PROVIDER_MESSAGE_BLOCKS_REASONING_HISTORY_V1_CAPABILITY,
                    PROVIDER_MESSAGE_BLOCKS_REDACTED_REASONING_HISTORY_V1_CAPABILITY,
                    PROVIDER_PROTOCOL_CONTEXT_CONSUME_ANTHROPIC_MESSAGES_V1_CAPABILITY,
                    PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_CHAT_V1_CAPABILITY,
                    PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_RESPONSES_V1_CAPABILITY,
                    PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V1_CAPABILITY,
                    PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V2_CAPABILITY,
                    PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_CHAT_V1_CAPABILITY,
                    PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_RESPONSES_V1_CAPABILITY,
                    PROVIDER_NETWORK_EGRESS_HANDOFF_V1_CAPABILITY,
                    extension_contracts::RUNTIME_HOST_CALL_CAPABILITY_V1,
                ],
            )?;
        }
        if !seen.insert(capability.as_str()) {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "runtime.capabilities contains duplicate value: {capability}"
            )));
        }
    }
    for (consume, restore) in [
        (
            PROVIDER_PROTOCOL_CONTEXT_CONSUME_ANTHROPIC_MESSAGES_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V1_CAPABILITY,
        ),
        (
            PROVIDER_PROTOCOL_CONTEXT_CONSUME_ANTHROPIC_MESSAGES_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V2_CAPABILITY,
        ),
        (
            PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_CHAT_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_CHAT_V1_CAPABILITY,
        ),
        (
            PROVIDER_PROTOCOL_CONTEXT_CONSUME_OPENAI_RESPONSES_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_OPENAI_RESPONSES_V1_CAPABILITY,
        ),
    ] {
        if seen.contains(consume) && seen.contains(restore) {
            return Err(PluginFrameworkError::invalid_provider_package(format!(
                "runtime.capabilities must not declare conflicting protocol context profiles: {consume}, {restore}"
            )));
        }
    }
    if seen.contains(PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V1_CAPABILITY)
        && seen.contains(PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V2_CAPABILITY)
    {
        return Err(PluginFrameworkError::invalid_provider_package(format!(
            "runtime.capabilities must not declare conflicting protocol context profiles: {}, {}",
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V1_CAPABILITY,
            PROVIDER_PROTOCOL_CONTEXT_RESTORE_ANTHROPIC_MESSAGES_V2_CAPABILITY,
        )));
    }
    Ok(())
}

pub(crate) fn validate_node_contributions(
    contributions: &[NodeContributionManifest],
) -> FrameworkResult<()> {
    if contributions.is_empty() {
        return Err(PluginFrameworkError::invalid_provider_package(
            "capability_plugin must declare node_contributions",
        ));
    }

    for node_contribution in contributions {
        validate_non_empty(
            &node_contribution.contribution_code,
            "node_contributions[].contribution_code",
        )?;
        validate_non_empty(
            &node_contribution.node_shell,
            "node_contributions[].node_shell",
        )?;
        validate_non_empty(&node_contribution.category, "node_contributions[].category")?;
        validate_non_empty(&node_contribution.title, "node_contributions[].title")?;
        validate_non_empty(
            &node_contribution.description,
            "node_contributions[].description",
        )?;
        validate_non_empty(&node_contribution.icon, "node_contributions[].icon")?;
        validate_non_empty(
            &node_contribution.schema_version,
            "node_contributions[].schema_version",
        )?;
        validate_allowed(
            &node_contribution.node_shell,
            "node_contributions[].node_shell",
            &["action"],
        )?;
        validate_allowed(
            &node_contribution.schema_version,
            "node_contributions[].schema_version",
            &["1flowbase.node-contribution/v2"],
        )?;
        validate_allowed(
            &node_contribution.side_effect_policy,
            "node_contributions[].side_effect_policy",
            &["none", "external_read", "external_write", "durable_write"],
        )?;
        validate_node_contribution_schema_ui(&node_contribution.schema_ui)?;
        validate_node_contribution_output_schema(&node_contribution.output_schema)?;
        validate_node_contribution_infra_contracts(&node_contribution.infra_contracts)?;
        validate_required_auth(&node_contribution.required_auth)?;
        validate_allowed(
            &node_contribution.visibility,
            "node_contributions[].visibility",
            &["public"],
        )?;
        validate_allowed(
            &node_contribution.dependency.installation_kind,
            "node_contributions[].dependency.installation_kind",
            &["optional", "required"],
        )?;
        validate_non_empty(
            &node_contribution.dependency.installation_kind,
            "node_contributions[].dependency.installation_kind",
        )?;
        validate_non_empty(
            &node_contribution.dependency.plugin_version_range,
            "node_contributions[].dependency.plugin_version_range",
        )?;
        if node_contribution.schema_ui.is_null() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "node_contributions[].schema_ui cannot be null",
            ));
        }
        if node_contribution.output_schema.is_null() {
            return Err(PluginFrameworkError::invalid_provider_package(
                "node_contributions[].output_schema cannot be null",
            ));
        }
    }
    Ok(())
}
