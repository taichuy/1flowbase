use super::*;

/// CountTokens owns no second prompt shape. It carries the same complete canonical
/// envelope used by Generate and only changes the requested operation.
#[derive(Debug, Clone, PartialEq)]
pub struct ProviderCountTokensInput(ProviderInvocationInput);

impl Serialize for ProviderCountTokensInput {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.0.serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for ProviderCountTokensInput {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let invocation = ProviderInvocationInput::deserialize(deserializer)?;
        if invocation.operation != ProviderWireOperation::CountTokens
            || invocation.profile.is_some()
        {
            return Err(serde::de::Error::custom(
                "CountTokens input requires operation=count_tokens without a compact profile",
            ));
        }
        Ok(Self(invocation))
    }
}

impl Default for ProviderCountTokensInput {
    fn default() -> Self {
        Self::from_invocation(ProviderInvocationInput::default())
    }
}

impl ProviderCountTokensInput {
    pub fn from_invocation(mut invocation: ProviderInvocationInput) -> Self {
        invocation.operation = ProviderWireOperation::CountTokens;
        invocation.profile = None;
        Self(invocation)
    }

    pub fn as_invocation(&self) -> &ProviderInvocationInput {
        &self.0
    }

    pub fn into_invocation(self) -> ProviderInvocationInput {
        self.0
    }

    pub fn set_provider_config(&mut self, provider_config: Value) {
        self.0.provider_config = provider_config;
    }

    pub fn required_capabilities(&self) -> BTreeSet<ProviderInvocationCapability> {
        let mut capabilities = self.0.required_capabilities.clone();
        capabilities.insert(ProviderInvocationCapability::CountTokens);
        capabilities.extend(semantic_required_capabilities(
            &self.0.system,
            &self.0.request_context,
        ));
        capabilities
    }

    // Runtime errors intentionally keep the complete upstream diagnostics payload.
    #[allow(clippy::result_large_err)]
    pub fn to_current_provider_wire_value(
        &self,
        declared_capabilities: &[String],
    ) -> Result<Value, ProviderCountTokensError> {
        if self.operation != ProviderWireOperation::CountTokens {
            return Err(ProviderCountTokensError::InvalidContract {
                message: "CountTokens input must declare operation=count_tokens".to_string(),
            });
        }
        if let Some(envelope) = &self.client_protocol_envelope {
            validate_protocol_context_envelope(envelope)
                .map_err(|message| ProviderCountTokensError::InvalidContract { message })?;
        }
        let declared_protocol_profiles = declared_capabilities
            .iter()
            .map(String::as_str)
            .collect::<BTreeSet<_>>();
        let mut invocation = self.0.clone();
        project_protocol_context_envelope(
            &mut invocation.client_protocol_envelope,
            &mut invocation.required_capabilities,
            &declared_protocol_profiles,
        );
        let mut required_capabilities = invocation.required_capabilities.clone();
        required_capabilities.insert(ProviderInvocationCapability::CountTokens);
        required_capabilities.extend(semantic_required_capabilities(
            &invocation.system,
            &invocation.request_context,
        ));
        let unsupported =
            undeclared_provider_capabilities(&required_capabilities, declared_capabilities);
        if !unsupported.is_empty() {
            return Err(ProviderCountTokensError::Unsupported {
                capabilities: unsupported
                    .iter()
                    .map(provider_invocation_capability_name)
                    .collect(),
            });
        }

        serde_json::to_value(invocation).map_err(|error| {
            ProviderCountTokensError::InvalidContract {
                message: error.to_string(),
            }
        })
    }
}

impl std::ops::Deref for ProviderCountTokensInput {
    type Target = ProviderInvocationInput;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCountTokensMethod {
    #[default]
    UpstreamApi,
    ModelTokenizer,
    ProviderEstimate,
    GenericEstimate,
    FallbackZero,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCountTokensCoverage {
    #[default]
    Complete,
    Partial,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ProviderCountTokensFallbackReason {
    CapabilityUnavailable,
    PluginUnavailable,
    ProviderRuntimeFailure,
    MalformedProviderResult,
    EstimatorFault,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ProviderCountTokensResult {
    pub operation: ProviderWireOperation,
    pub input_tokens: u64,
    #[serde(default)]
    pub method: ProviderCountTokensMethod,
    #[serde(default)]
    pub coverage: ProviderCountTokensCoverage,
    #[serde(default)]
    pub unknown_block_count: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fallback_reason: Option<ProviderCountTokensFallbackReason>,
}

impl Default for ProviderCountTokensResult {
    fn default() -> Self {
        Self {
            operation: ProviderWireOperation::CountTokens,
            input_tokens: 0,
            method: ProviderCountTokensMethod::UpstreamApi,
            coverage: ProviderCountTokensCoverage::Complete,
            unknown_block_count: 0,
            fallback_reason: None,
        }
    }
}

impl ProviderCountTokensResult {
    pub fn generic_estimate(
        input_tokens: u64,
        coverage: ProviderCountTokensCoverage,
        unknown_block_count: u64,
        fallback_reason: ProviderCountTokensFallbackReason,
    ) -> Self {
        Self {
            operation: ProviderWireOperation::CountTokens,
            input_tokens,
            method: ProviderCountTokensMethod::GenericEstimate,
            coverage,
            unknown_block_count,
            fallback_reason: Some(fallback_reason),
        }
    }

    pub fn fallback_zero() -> Self {
        Self {
            operation: ProviderWireOperation::CountTokens,
            input_tokens: 0,
            method: ProviderCountTokensMethod::FallbackZero,
            coverage: ProviderCountTokensCoverage::Partial,
            unknown_block_count: 0,
            fallback_reason: Some(ProviderCountTokensFallbackReason::EstimatorFault),
        }
    }
}

