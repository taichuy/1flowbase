use super::*;

#[async_trait]
impl CapabilityRuntimePort for RuntimeExtensionHost {
    async fn drain_managed_contributions(
        &self,
        handles: &[extension_contracts::ManagedExecutionHandle],
    ) -> Result<Box<dyn runtime_core::runtime_backend::RuntimeManagedDrain>, RuntimeBackendError>
    {
        self.managed_workers
            .write()
            .await
            .drain(handles)
            .map_err(Into::into)
    }
    async fn activate_managed_contribution(
        &self,
        request: RuntimeManagedActivation,
    ) -> Result<extension_contracts::extension_bus::ManagedExecutionHandle, RuntimeBackendError>
    {
        self.ensure_activating()?;
        if request.artifact.as_str() != request.identity.installation_id().as_str() {
            return Err(RuntimeBackendError::InvalidRequest(
                "managed artifact must belong to the bound installation".into(),
            ));
        }
        let hash_permit = self.admit_managed_hash()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        let identity = request.identity.clone();
        let binding = tokio::task::spawn_blocking(move || {
            let _permit = hash_permit;
            crate::package_loader::PackageLoader::load_managed(package_root, &request)
        })
        .await
        .map_err(|error| RuntimeBackendError::Execution {
            target_id: identity.contribution_id().as_str().to_string(),
            message: error.to_string(),
        })?
        .map_err(RuntimeBackendError::from)?;
        let mut workers = self.managed_workers.write().await;
        let lifecycle = self.lifecycle.read().map_err(|_| {
            RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".into())
        })?;
        if !matches!(
            *lifecycle,
            RuntimeBackendLifecycle::Starting | RuntimeBackendLifecycle::Ready
        ) {
            return Err(RuntimeBackendError::Unavailable(*lifecycle));
        }
        workers
            .mount(identity, binding)
            .map_err(RuntimeBackendError::from)
    }

    async fn deactivate_managed_contribution(
        &self,
        handle: &extension_contracts::extension_bus::ManagedExecutionHandle,
    ) -> Result<(), RuntimeBackendError> {
        let scope = self
            .managed_workers
            .write()
            .await
            .unmount(handle)
            .map_err(RuntimeBackendError::from)?;
        let pool = self
            .managed_workers
            .read()
            .await
            .service_pool_for_handle(handle)?;
        if let Some(pool) = pool {
            pool.retire_all().await?;
        }
        scope.dispose().await.map_err(RuntimeBackendError::from)?;
        self.managed_workers.write().await.finish_unmount(handle);
        Ok(())
    }

    async fn reset_managed_service_workers(
        &self,
        handle: &extension_contracts::ManagedExecutionHandle,
    ) -> Result<(), RuntimeBackendError> {
        let pool = self
            .managed_workers
            .read()
            .await
            .service_pool_for_handle(handle)?;
        if let Some(pool) = pool {
            // Cleanup keeps its owner even if the HTTP deadline drops the awaiting adapter.
            tokio::spawn(async move { pool.retire_all().await })
                .await
                .map_err(|_| {
                    RuntimeBackendError::InvalidRequest("service worker reset task failed".into())
                })??;
        }
        Ok(())
    }

    async fn admit_managed_capability_execute(
        &self,
        request: RuntimeManagedCapabilityRequest,
    ) -> Result<runtime_core::runtime_backend::AdmittedManagedExecution, RuntimeBackendError> {
        self.verify_managed_executable(&request.handle).await?;
        let operation = {
            let workers = self.managed_workers.read().await;
            let lifecycle = self.lifecycle.read().map_err(|_| {
                RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".into())
            })?;
            if *lifecycle != RuntimeBackendLifecycle::Ready {
                return Err(RuntimeBackendError::Unavailable(*lifecycle));
            }
            workers.execute_with_services(
                request,
                crate::managed_worker::ManagedHostServices {
                    plugin_data: Arc::clone(&self.plugin_data),
                    plugin_credentials: self.plugin_credentials.clone(),
                    file_import: self.file_import.clone(),
                    file_read: self.file_read.clone(),
                    application_logs: self.application_logs.clone(),
                    http_egress: self.http_egress.clone(),
                },
            )?
        };
        Ok(Box::pin(async move {
            operation.await.map_err(RuntimeBackendError::from)
        }))
    }

    async fn admit_managed_event(
        &self,
        request: runtime_core::runtime_backend::RuntimeManagedEventRequest,
    ) -> Result<runtime_core::runtime_backend::AdmittedManagedEvent, RuntimeBackendError> {
        self.verify_managed_executable(&request.handle).await?;
        let operation = {
            let workers = self.managed_workers.read().await;
            let lifecycle = self.lifecycle.read().map_err(|_| {
                RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".into())
            })?;
            if *lifecycle != RuntimeBackendLifecycle::Ready {
                return Err(RuntimeBackendError::Unavailable(*lifecycle));
            }
            workers.admit_event(request)?
        };
        Ok(Box::pin(async move {
            operation.await.map_err(RuntimeBackendError::from)
        }))
    }

    async fn admit_managed_hook(
        &self,
        request: runtime_core::runtime_backend::RuntimeManagedHookRequest,
    ) -> Result<runtime_core::runtime_backend::AdmittedManagedHook, RuntimeBackendError> {
        self.verify_managed_executable(&request.handle).await?;
        let operation = {
            let workers = self.managed_workers.read().await;
            let lifecycle = self.lifecycle.read().map_err(|_| {
                RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".into())
            })?;
            if *lifecycle != RuntimeBackendLifecycle::Ready {
                return Err(RuntimeBackendError::Unavailable(*lifecycle));
            }
            workers.admit_hook(request)?
        };
        Ok(Box::pin(async move {
            operation.await.map_err(RuntimeBackendError::from)
        }))
    }

    async fn managed_capability_execute(
        &self,
        request: RuntimeManagedCapabilityRequest,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.admit_managed_capability_execute(request).await?.await
    }

    async fn activate_capability(
        &self,
        request: RuntimePackageActivation,
    ) -> Result<(), RuntimeBackendError> {
        self.ensure_activating()?;
        let package_root = self.artifact_resolver.resolve(&request.artifact).await?;
        let mut host = self.capability_host.write().await;
        if host.is_loaded(&request.plugin_id) {
            return Ok(());
        }
        match request.legacy_eligibility {
            Some(eligibility) => {
                host.load_legacy_installed(&package_root, &legacy_eligibility(eligibility))
                    .await
            }
            None => host.load(&package_root).await,
        }
        .map(|_| ())
        .map_err(RuntimeBackendError::from)
    }

    async fn deactivate_capability(&self, plugin_id: &str) -> Result<(), RuntimeBackendError> {
        self.capability_host
            .write()
            .await
            .unload(plugin_id)
            .await
            .map_err(RuntimeBackendError::from)
    }

    async fn capability_validate(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .validate_config_operation(target_id, contribution_code, config_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn capability_resolve_dynamic_options(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .resolve_dynamic_options_operation(target_id, contribution_code, config_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn capability_resolve_output_schema(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
    ) -> Result<serde_json::Value, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .resolve_output_schema_operation(target_id, contribution_code, config_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| value.output)
            .map_err(Into::into)
    }

    async fn capability_execute(
        &self,
        target_id: &str,
        contribution_code: &str,
        config_payload: serde_json::Value,
        input_payload: serde_json::Value,
    ) -> Result<RuntimeCapabilityExecutionOutcome, RuntimeBackendError> {
        self.ensure_accepting()?;
        let operation = self
            .capability_host
            .read()
            .await
            .execute_operation(target_id, contribution_code, config_payload, input_payload)
            .map_err(RuntimeBackendError::from)?;
        operation
            .await
            .map(|value| RuntimeCapabilityExecutionOutcome {
                output_payload: value.output_payload,
                granted_credit_permissions: value.granted_credit_permissions,
            })
            .map_err(Into::into)
    }
}
