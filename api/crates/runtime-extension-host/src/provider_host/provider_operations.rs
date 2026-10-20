use super::*;

impl ProviderHost {
    #[cfg(test)]
    pub async fn validate(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<ProviderValidationOutput> {
        self.validate_operation(plugin_id, provider_config)?.await
    }

    pub fn validate_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderValidationOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        if !loaded
            .package
            .provider
            .supports_operational_capability(ProviderOperationalCapability::ConfigurationValidation)
        {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "provider package does not declare configuration validation support",
            ));
        }
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let output = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Validate,
                provider_config,
            )
            .await?;
            Ok(ProviderValidationOutput { output })
        })
    }

    #[cfg(test)]
    pub async fn authenticate(
        &self,
        plugin_id: &str,
        provider_config: Value,
        operation: ProviderAuthOperation,
    ) -> FrameworkResult<ProviderAuthOutput> {
        self.authenticate_operation(plugin_id, provider_config, operation)?
            .await
    }

    pub fn authenticate_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
        operation: ProviderAuthOperation,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderAuthOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        let auth = loaded.package.provider.auth.clone().ok_or_else(|| {
            PluginFrameworkError::invalid_provider_contract(
                "provider package does not declare authentication support",
            )
        })?;
        if let ProviderAuthOperation::Begin { action } = &operation {
            if !auth
                .actions
                .iter()
                .any(|candidate| candidate.code == *action)
            {
                return Err(PluginFrameworkError::invalid_provider_contract(
                    "provider auth action is not declared by the package",
                ));
            }
        }
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let input = serde_json::to_value(ProviderAuthRuntimeInput {
                provider_config,
                operation: operation.clone(),
            })
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
            let raw = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Auth,
                input,
            )
            .await?;
            let result: ProviderAuthResult = serde_json::from_value(raw).map_err(|error| {
                PluginFrameworkError::invalid_provider_contract(error.to_string())
            })?;
            for key in result.managed_secret_patch.keys() {
                if !auth
                    .managed_secret_keys
                    .iter()
                    .any(|allowed| allowed == key)
                {
                    return Err(PluginFrameworkError::invalid_provider_contract(
                        "provider auth result includes an undeclared managed secret key",
                    ));
                }
            }
            if let Some(user_action) = result.user_action.as_ref() {
                let allowed = auth.actions.iter().any(|action| {
                    action
                        .user_action_kinds
                        .iter()
                        .any(|kind| kind == &user_action.kind)
                });
                if !allowed {
                    return Err(PluginFrameworkError::invalid_provider_contract(
                        "provider auth result includes an undeclared user action kind",
                    ));
                }
            }
            Ok(ProviderAuthOutput { result })
        })
    }

    #[cfg(test)]
    pub async fn list_models(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<ProviderModelsOutput> {
        self.list_models_operation(plugin_id, provider_config)?
            .await
    }

    pub fn list_models_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderModelsOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        if !loaded
            .package
            .provider
            .supports_operational_capability(ProviderOperationalCapability::ModelListing)
        {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "provider package does not declare model listing support",
            ));
        }
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let models = match loaded.package.provider.model_discovery_mode {
                ModelDiscoveryMode::Static => loaded.package.predefined_models.clone(),
                ModelDiscoveryMode::Dynamic => {
                    let dynamic = Self::call_runtime_loaded(
                        loaded,
                        provider_workers,
                        ProviderStdioMethod::ListModels,
                        provider_config,
                    )
                    .await?;
                    normalize_models(dynamic)?
                }
                ModelDiscoveryMode::Hybrid => {
                    let predefined_models = loaded.package.predefined_models.clone();
                    let dynamic = Self::call_runtime_loaded(
                        loaded,
                        provider_workers,
                        ProviderStdioMethod::ListModels,
                        provider_config,
                    )
                    .await?;
                    merge_models(&predefined_models, normalize_models(dynamic)?)
                }
            };
            Ok(ProviderModelsOutput { models })
        })
    }

    #[cfg(test)]
    #[expect(
        dead_code,
        reason = "internal direct-call fixture; production facade uses the admitted operation future"
    )]
    pub async fn get_balance(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<ProviderBalanceOutput> {
        self.get_balance_operation(plugin_id, provider_config)?
            .await
    }

    pub fn get_balance_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderBalanceOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let raw_balance = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Balance,
                provider_config,
            )
            .await?;
            Ok(ProviderBalanceOutput {
                balance: normalize_balance(raw_balance)?,
            })
        })
    }

    #[cfg(test)]
    pub async fn get_usage_windows(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<ProviderUsageWindowsOutput> {
        self.get_usage_windows_operation(plugin_id, provider_config)?
            .await
    }

    pub fn get_usage_windows_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderUsageWindowsOutput>> + Send + 'static,
    > {
        let loaded = self.loaded_package(plugin_id)?.clone();
        if !loaded
            .package
            .provider
            .supports_operational_capability(ProviderOperationalCapability::UsageWindows)
        {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "provider package does not declare usage windows support",
            ));
        }
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let raw_usage = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::Usage,
                provider_config,
            )
            .await?;
            Ok(ProviderUsageWindowsOutput {
                usage: normalize_usage_windows(raw_usage)?,
            })
        })
    }

    #[cfg(test)]
    pub async fn reset_credit(
        &self,
        plugin_id: &str,
        provider_config: Value,
        operation: ProviderResetCreditOperation,
    ) -> FrameworkResult<ProviderResetCreditOutput> {
        self.reset_credit_operation(plugin_id, provider_config, operation)?
            .await
    }

    pub fn reset_credit_operation(
        &self,
        plugin_id: &str,
        provider_config: Value,
        operation: ProviderResetCreditOperation,
    ) -> FrameworkResult<
        impl std::future::Future<Output = FrameworkResult<ProviderResetCreditOutput>> + Send + 'static,
    > {
        if matches!(
            &operation,
            ProviderResetCreditOperation::Consume { idempotency_key }
                if idempotency_key.trim().is_empty()
        ) {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "reset credit consume requires a non-empty idempotency key",
            ));
        }
        let loaded = self.loaded_package(plugin_id)?.clone();
        if !loaded
            .package
            .provider
            .supports_operational_capability(ProviderOperationalCapability::ResetCredits)
        {
            return Err(PluginFrameworkError::invalid_provider_contract(
                "provider package does not declare reset credits support",
            ));
        }
        let provider_workers = Arc::clone(&self.provider_workers);
        Ok(async move {
            let input = serde_json::to_value(ProviderResetCreditRuntimeInput {
                provider_config,
                operation: operation.clone(),
            })
            .map_err(|error| PluginFrameworkError::serialization(None, error.to_string()))?;
            let raw_result = Self::call_runtime_loaded(
                loaded,
                provider_workers,
                ProviderStdioMethod::ResetCredit,
                input,
            )
            .await?;
            let result = normalize_reset_credit_result(raw_result)?;
            if !reset_credit_result_matches_operation(&result, &operation) {
                return Err(PluginFrameworkError::invalid_provider_contract(
                    "provider reset credit result does not match requested operation",
                ));
            }
            Ok(ProviderResetCreditOutput { result })
        })
    }
}
