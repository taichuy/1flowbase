use super::*;

impl ProviderInvocationInput {
    pub const NETWORK_EGRESS_CONTEXT_KEY: &'static str = "network_egress";

    pub fn set_recovery_directive(
        &mut self,
        directive: ProviderRecoveryDirective,
    ) -> Result<(), String> {
        directive.validate()?;
        self.run_context.insert(
            PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY.to_string(),
            serde_json::to_value(directive)
                .expect("ProviderRecoveryDirective must always serialize"),
        );
        Ok(())
    }

    pub fn recovery_directive(&self) -> Result<Option<ProviderRecoveryDirective>, String> {
        self.run_context
            .get(PROVIDER_RECOVERY_DIRECTIVE_CONTEXT_KEY)
            .map(|value| {
                let directive: ProviderRecoveryDirective = serde_json::from_value(value.clone())
                    .map_err(|_| "provider recovery directive is invalid".to_string())?;
                directive.validate()?;
                Ok(directive)
            })
            .transpose()
    }

    pub fn set_transport_session_directive(
        &mut self,
        directive: ProviderTransportSessionDirective,
    ) -> Result<(), String> {
        directive.validate()?;
        self.run_context.insert(
            PROVIDER_TRANSPORT_SESSION_CONTEXT_KEY.to_string(),
            serde_json::to_value(directive)
                .expect("ProviderTransportSessionDirective must always serialize"),
        );
        Ok(())
    }

    pub fn transport_session_directive(
        &self,
    ) -> Result<Option<ProviderTransportSessionDirective>, String> {
        self.run_context
            .get(PROVIDER_TRANSPORT_SESSION_CONTEXT_KEY)
            .map(|value| {
                let directive: ProviderTransportSessionDirective =
                    serde_json::from_value(value.clone()).map_err(|_| {
                        "physical transport session directive is invalid".to_string()
                    })?;
                directive.validate()?;
                Ok(directive)
            })
            .transpose()
    }

    pub fn set_network_egress_context(&mut self, context: ProviderNetworkEgressContext) {
        self.required_capabilities
            .insert(ProviderInvocationCapability::NetworkEgressHandoffV1);
        self.run_context.insert(
            Self::NETWORK_EGRESS_CONTEXT_KEY.to_string(),
            serde_json::to_value(context)
                .expect("ProviderNetworkEgressContext must always serialize"),
        );
    }

    pub fn network_egress_context(&self) -> Result<Option<ProviderNetworkEgressContext>, String> {
        self.run_context
            .get(Self::NETWORK_EGRESS_CONTEXT_KEY)
            .map(|value| {
                serde_json::from_value(value.clone())
                    .map_err(|_| "network_egress context is invalid".to_string())
            })
            .transpose()
    }

    pub fn system_text(&self) -> Option<String> {
        (!self.system.is_empty()).then(|| {
            self.system
                .iter()
                .map(NativePromptBlock::text_content)
                .collect::<Vec<_>>()
                .join("\n\n")
        })
    }

    pub fn synchronize_required_capabilities(&mut self) -> Result<(), String> {
        if self.native_transport.is_some()
            || self
                .required_capabilities
                .contains(&ProviderInvocationCapability::ResponsesNativePassthrough)
        {
            self.required_capabilities
                .insert(ProviderInvocationCapability::ResponsesNativeOutputV1);
        }
        self.required_capabilities
            .extend(self.derived_required_capabilities()?);
        Ok(())
    }

    pub fn semantic_required_capabilities(&self) -> BTreeSet<ProviderInvocationCapability> {
        semantic_required_capabilities(&self.system, &self.request_context)
    }

    pub fn compact_profile(&self) -> Result<ProviderCompactProfile, String> {
        if self.operation != ProviderWireOperation::Compact {
            return Err("Compact input must declare operation=compact".to_string());
        }
        self.profile
            .ok_or_else(|| "Compact input must declare a compact profile".to_string())
    }

    pub fn to_current_provider_wire_value(
        &self,
        declared_capabilities: &[String],
    ) -> Result<Value, ExtensionContractError> {
        self.to_current_provider_generate_wire_value(declared_capabilities)
            .map(|(wire_value, _)| wire_value)
    }

    pub fn to_current_provider_generate_wire_value(
        &self,
        declared_capabilities: &[String],
    ) -> Result<(Value, ProviderGenerateTranslationReceipt), ExtensionContractError> {
        let projection = self
            .project_current_provider_generate(declared_capabilities)
            .map_err(|error| {
                ExtensionContractError::invalid_provider_contract(error.to_string())
            })?;
        let invocation = projection.provider_bound_input;
        let receipt = projection.receipt;
        let unsupported = undeclared_provider_capabilities(
            &invocation.required_capabilities,
            declared_capabilities,
        );

        if !unsupported.is_empty() {
            return Err(ExtensionContractError::invalid_provider_contract(format!(
                "current provider contract is missing required capabilities: {}",
                unsupported
                    .iter()
                    .map(provider_invocation_capability_name)
                    .collect::<Vec<_>>()
                    .join(", ")
            )));
        }

        let wire_value = serde_json::to_value(invocation).map_err(|error| {
            ExtensionContractError::invalid_provider_contract(error.to_string())
        })?;
        Ok((wire_value, receipt))
    }

    // Runtime errors intentionally keep the complete upstream diagnostics payload.
    #[allow(clippy::result_large_err)]
    pub fn project_current_provider_generate(
        &self,
        declared_capabilities: &[String],
    ) -> Result<ProviderGenerateProjection, ProviderGenerateProjectionError> {
        self.prepared_current_provider_generate_invocation(declared_capabilities)
    }

    // Compact errors preserve the same typed upstream diagnostics contract.
    #[allow(clippy::result_large_err)]
    pub fn to_current_provider_compact_wire_value(
        &self,
        declared_capabilities: &[String],
    ) -> Result<Value, ProviderCompactError> {
        let profile = self
            .compact_profile()
            .map_err(|message| ProviderCompactError::InvalidContract { message })?;
        let mut invocation = self
            .prepared_current_provider_invocation()
            .map_err(|message| ProviderCompactError::InvalidContract { message })?;
        let declared_protocol_profiles = declared_capabilities
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        project_protocol_context_envelope(
            &mut invocation.client_protocol_envelope,
            &mut invocation.required_capabilities,
            &declared_protocol_profiles,
        );
        let unsupported = undeclared_provider_capabilities(
            &invocation.required_capabilities,
            declared_capabilities,
        );

        if !unsupported.is_empty() {
            return Err(ProviderCompactError::Unsupported {
                profile,
                capabilities: unsupported
                    .iter()
                    .map(provider_invocation_capability_name)
                    .collect(),
            });
        }

        serde_json::to_value(invocation).map_err(|error| ProviderCompactError::InvalidContract {
            message: error.to_string(),
        })
    }

    pub fn wire_audit(&self) -> ProviderWireAudit {
        let lengths = [
            self.messages.len(),
            self.system.len(),
            self.tools.len(),
            self.mcp_bindings.len(),
            self.model_parameters.len(),
            self.trace_context.len(),
            self.run_context.len(),
        ];
        let mut required_capabilities = self.required_capabilities.clone();
        required_capabilities.extend(self.derived_required_capabilities().unwrap_or_default());

        ProviderWireAudit {
            operation: self.operation,
            profile: self.profile,
            contract_version: self.contract_version,
            message_count: bounded_wire_count(self.messages.len()),
            system_block_count: bounded_wire_count(self.system.len()),
            tool_count: bounded_wire_count(self.tools.len()),
            mcp_binding_count: bounded_wire_count(self.mcp_bindings.len()),
            model_parameter_count: bounded_wire_count(self.model_parameters.len()),
            trace_context_entry_count: bounded_wire_count(self.trace_context.len()),
            run_context_entry_count: bounded_wire_count(self.run_context.len()),
            counts_capped: lengths.iter().any(|length| u32::try_from(*length).is_err()),
            has_previous_response_id: self.previous_response_id.is_some(),
            has_request_context: !self.request_context.is_empty(),
            has_response_format: self.response_format.is_some(),
            has_client_protocol_envelope: self.client_protocol_envelope.is_some(),
            has_native_transport: self.native_transport.is_some(),
            required_capabilities,
        }
    }

    pub(super) fn prepared_current_provider_invocation(&self) -> Result<Self, String> {
        self.validate_current_provider_operation()?;
        let mut invocation = self.clone();
        invocation.synchronize_required_capabilities()?;
        Ok(invocation)
    }

    // Runtime errors intentionally keep the complete upstream diagnostics payload.
    #[allow(clippy::result_large_err)]
    pub(super) fn prepared_current_provider_generate_invocation(
        &self,
        declared_capabilities: &[String],
    ) -> Result<ProviderGenerateProjection, ProviderGenerateProjectionError> {
        self.validate_current_provider_operation()
            .map_err(|message| ProviderGenerateProjectionError::InvalidContract { message })?;
        if self.operation != ProviderWireOperation::Generate {
            return Err(ProviderGenerateProjectionError::InvalidContract {
                message: "Generate translation requires operation=generate".to_string(),
            });
        }
        let declared_capabilities = declared_capabilities
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut invocation = self.clone();
        let mut receipt = ProviderGenerateTranslationReceipt::default();

        message_block_required_capabilities(&invocation.messages)
            .map_err(|message| ProviderGenerateProjectionError::InvalidContract { message })?;
        project_generate_reasoning_history(&mut invocation, &declared_capabilities, &mut receipt)?;
        let projected_message_capabilities =
            message_block_required_capabilities(&invocation.messages)
                .map_err(|message| ProviderGenerateProjectionError::InvalidContract { message })?;
        if projected_message_capabilities
            .contains(&ProviderInvocationCapability::MessageBlocksReasoningHistoryV1)
        {
            receipt
                .decisions
                .insert(ProviderGenerateTranslationDecision::DelegatedReasoningHistoryToProvider);
        }
        if projected_message_capabilities
            .contains(&ProviderInvocationCapability::MessageBlocksRedactedReasoningHistoryV1)
        {
            receipt.decisions.insert(
                ProviderGenerateTranslationDecision::DelegatedRedactedReasoningHistoryToProvider,
            );
        }

        if invocation
            .system
            .iter()
            .any(NativePromptBlock::has_cache_control)
            && !declared_capabilities.contains(
                ProviderInvocationCapability::SystemPromptCacheControl.manifest_capability_name(),
            )
        {
            for block in &mut invocation.system {
                let NativePromptBlock::Text { cache_control, .. } = block;
                *cache_control = None;
            }
            for capability in [
                ProviderInvocationCapability::SystemPromptBlocks,
                ProviderInvocationCapability::SystemPromptCacheControl,
            ] {
                invocation.required_capabilities.remove(&capability);
            }
            receipt
                .decisions
                .insert(ProviderGenerateTranslationDecision::OmittedSystemPromptCacheControl);
        }

        if invocation.request_context.end_user_reference.is_some()
            && !declared_capabilities
                .contains(ProviderInvocationCapability::EndUserReference.manifest_capability_name())
        {
            invocation.request_context.end_user_reference = None;
            invocation
                .required_capabilities
                .remove(&ProviderInvocationCapability::EndUserReference);
            receipt
                .decisions
                .insert(ProviderGenerateTranslationDecision::OmittedEndUserReference);
        }

        if project_protocol_context_envelope(
            &mut invocation.client_protocol_envelope,
            &mut invocation.required_capabilities,
            &declared_capabilities,
        ) {
            receipt
                .decisions
                .insert(ProviderGenerateTranslationDecision::OmittedProtocolContextProfileMismatch);
        }

        if invocation.native_transport.is_some() {
            invocation
                .required_capabilities
                .insert(ProviderInvocationCapability::ResponsesNativePassthrough);
        }

        invocation
            .synchronize_required_capabilities()
            .map_err(|message| ProviderGenerateProjectionError::InvalidContract { message })?;
        if declared_capabilities.contains(PROVIDER_REASONING_HISTORY_INPUT_SUPPORTED_CAPABILITY)
            && projected_message_capabilities
                .contains(&ProviderInvocationCapability::MessageBlocksReasoningHistoryV1)
        {
            invocation
                .required_capabilities
                .remove(&ProviderInvocationCapability::MessageBlocksReasoningHistoryV1);
            invocation
                .required_capabilities
                .insert(ProviderInvocationCapability::ReasoningHistoryInputSupported);
        }
        Ok(ProviderGenerateProjection {
            provider_bound_input: invocation,
            receipt,
        })
    }

    pub(super) fn derived_required_capabilities(
        &self,
    ) -> Result<BTreeSet<ProviderInvocationCapability>, String> {
        let mut capabilities = self.semantic_required_capabilities();
        capabilities.extend(message_block_required_capabilities(&self.messages)?);
        if self.operation == ProviderWireOperation::Compact {
            if let Some(profile) = self.profile {
                capabilities.insert(profile.required_capability());
            }
        }
        Ok(capabilities)
    }

    pub(super) fn validate_current_provider_operation(&self) -> Result<(), String> {
        self.transport_session_directive()?;
        self.recovery_directive()?;
        if let Some(envelope) = &self.client_protocol_envelope {
            validate_protocol_context_envelope(envelope)?;
        }
        let claimed_compact_capability = self.required_capabilities.iter().find(|capability| {
            matches!(
                capability,
                ProviderInvocationCapability::CompactResponsesCompact
                    | ProviderInvocationCapability::CompactResponsesCompactionV2
            )
        });

        match (self.operation, self.profile) {
            (ProviderWireOperation::Generate, None) => {
                if let Some(capability) = claimed_compact_capability {
                    return Err(format!(
                        "Generate input must not claim Compact capability {}",
                        provider_invocation_capability_name(capability)
                    ));
                }
                Ok(())
            }
            (ProviderWireOperation::Generate, Some(_)) => {
                Err("Generate input must not declare a compact profile".to_string())
            }
            (ProviderWireOperation::CountTokens, _) => Err(
                "CountTokens input must use ProviderCountTokensInput instead of ProviderInvocationInput"
                    .to_string(),
            ),
            (ProviderWireOperation::Compact, Some(profile)) => {
                if let Some(capability) = self.required_capabilities.iter().find(|capability| {
                    matches!(
                        capability,
                        ProviderInvocationCapability::CompactResponsesCompact
                            | ProviderInvocationCapability::CompactResponsesCompactionV2
                    ) && **capability != profile.required_capability()
                })
                {
                    return Err(format!(
                        "Compact input profile {} must not claim capability {}",
                        profile.as_str(),
                        provider_invocation_capability_name(capability)
                    ));
                }
                Ok(())
            }
            (ProviderWireOperation::Compact, None) => {
                Err("Compact input must declare a compact profile".to_string())
            }
        }
    }
}
