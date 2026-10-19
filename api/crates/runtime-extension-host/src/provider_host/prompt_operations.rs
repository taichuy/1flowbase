use super::*;

impl ProviderHost {
    // Preserve the typed provider diagnostic at this public runtime boundary.
    #[allow(clippy::result_large_err)]
    #[cfg(test)]
    pub async fn count_tokens(
        &self,
        plugin_id: &str,
        input: ProviderCountTokensInput,
    ) -> Result<ProviderCountTokensOutput, ProviderCountTokensError> {
        self.count_tokens_operation(plugin_id, input)?.await
    }

    // Provider errors intentionally preserve complete typed upstream diagnostics.
    #[allow(clippy::result_large_err)]
    pub fn count_tokens_operation(
        &self,
        plugin_id: &str,
        input: ProviderCountTokensInput,
    ) -> Result<
        impl std::future::Future<
                Output = Result<ProviderCountTokensOutput, ProviderCountTokensError>,
            > + Send
            + 'static,
        ProviderCountTokensError,
    > {
        let loaded = self.loaded_package(plugin_id).ok().cloned();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let Some(loaded) = loaded else {
                return Ok(ProviderCountTokensOutput {
                    result: generic_count_tokens_fallback(
                        &input,
                        ProviderCountTokensFallbackReason::PluginUnavailable,
                    ),
                });
            };
            if loaded.package.manifest.contract_version != CURRENT_PROVIDER_CONTRACT {
                return Ok(ProviderCountTokensOutput {
                    result: generic_count_tokens_fallback(
                        &input,
                        ProviderCountTokensFallbackReason::PluginUnavailable,
                    ),
                });
            }
            let wire_input = match current_provider_count_tokens_wire_input(&loaded, &input) {
                Ok(wire_input) => wire_input,
                Err(ProviderCountTokensError::Unsupported { .. }) => {
                    return Ok(ProviderCountTokensOutput {
                        result: generic_count_tokens_fallback(
                            &input,
                            ProviderCountTokensFallbackReason::CapabilityUnavailable,
                        ),
                    });
                }
                Err(error) => return Err(error),
            };
            tracing::info!(
                provider_code = %input.provider_code,
                model = %input.model,
                "provider count tokens wire prepared"
            );
            let output = match Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Invoke,
                wire_input,
            )
            .await
            {
                Ok(output) => output,
                Err(_) => {
                    return Ok(ProviderCountTokensOutput {
                        result: generic_count_tokens_fallback(
                            &input,
                            ProviderCountTokensFallbackReason::ProviderRuntimeFailure,
                        ),
                    });
                }
            };
            let result = match serde_json::from_value::<ProviderCountTokensResult>(output) {
                Ok(result) => result,
                Err(_) => {
                    return Ok(ProviderCountTokensOutput {
                        result: generic_count_tokens_fallback(
                            &input,
                            ProviderCountTokensFallbackReason::MalformedProviderResult,
                        ),
                    });
                }
            };
            if result.operation != ProviderWireOperation::CountTokens {
                return Ok(ProviderCountTokensOutput {
                    result: generic_count_tokens_fallback(
                        &input,
                        ProviderCountTokensFallbackReason::MalformedProviderResult,
                    ),
                });
            }
            Ok(ProviderCountTokensOutput { result })
        })
    }

    // Preserve the typed provider diagnostic at this public runtime boundary.
    #[allow(clippy::result_large_err)]
    #[cfg(test)]
    pub async fn compact(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
    ) -> Result<ProviderCompactOutput, ProviderCompactError> {
        self.compact_operation(plugin_id, input)?.await
    }

    // Provider errors intentionally preserve complete typed upstream diagnostics.
    #[allow(clippy::result_large_err)]
    pub fn compact_operation(
        &self,
        plugin_id: &str,
        input: ProviderInvocationInput,
    ) -> Result<
        impl std::future::Future<Output = Result<ProviderCompactOutput, ProviderCompactError>>
            + Send
            + 'static,
        ProviderCompactError,
    > {
        let expected_profile = input
            .compact_profile()
            .map_err(|message| ProviderCompactError::InvalidContract { message })?;
        let loaded = self
            .loaded_package(plugin_id)
            .map_err(compact_framework_error)?
            .clone();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let wire_input = current_provider_compact_wire_input(&loaded, &input)?;
            tracing::info!(
                provider_code = %input.provider_code,
                model = %input.model,
                profile = %expected_profile.as_str(),
                "provider compact wire prepared"
            );
            let output = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Invoke,
                wire_input,
            )
            .await
            .map_err(compact_framework_error)?;
            let result =
                serde_json::from_value::<ProviderCompactResult>(output).map_err(|error| {
                    ProviderCompactError::Runtime {
                        error: ProviderRuntimeError::new(
                            ProviderRuntimeErrorKind::ProviderInvalidResponse,
                            format!("provider Compact result is malformed: {error}"),
                        ),
                    }
                })?;
            if !result.satisfies_profile(expected_profile) {
                return Err(ProviderCompactError::Runtime {
                    error: ProviderRuntimeError::new(
                        ProviderRuntimeErrorKind::ProviderInvalidResponse,
                        format!(
                            "provider Compact result must declare operation=compact with profile={}",
                            expected_profile.as_str()
                        ),
                    ),
                });
            }
            Ok(ProviderCompactOutput { result })
        })
    }
}
