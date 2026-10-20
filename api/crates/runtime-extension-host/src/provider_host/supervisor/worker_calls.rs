use super::*;

impl ProviderWorkerSupervisor {
    #[cfg(test)]
    pub(crate) async fn call(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
    ) -> FrameworkResult<Value> {
        self.call_inner(request, None).await
    }

    pub(crate) async fn call_admitted(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        resource: Box<dyn Send + Sync>,
    ) -> FrameworkResult<Value> {
        self.call_inner(request, Some(resource)).await
    }

    pub(super) async fn call_inner(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        resource: Option<Box<dyn Send + Sync>>,
    ) -> FrameworkResult<Value> {
        let lease = self.admit()?;
        if let Some(multiplex) = &self.multiplex {
            if multiplex.is_failed() {
                self.fail_multiplex().await?;
            }
            self.ensure_multiplex_can_dispatch(multiplex)?;
            let result = multiplex
                .call(
                    request,
                    multiplex.limits(),
                    None,
                    Some(Box::new((lease, resource))),
                )
                .await
                .and_then(|(response, _)| self.multiplex_response(response));
            if multiplex.is_failed() {
                if let Err(error) = self.fail_multiplex().await {
                    tracing::warn!(%error, "secondary multiplex cleanup failure");
                }
            }
            return result;
        }
        // The caller owns the queue wait: cancellation here releases admission
        // without dispatch. Only transfer a ready carrier to its cleanup owner.
        let mut worker = Arc::clone(&self.worker).lock_owned().await;
        self.ensure_lease_can_dispatch(&worker)?;
        let supervisor = Arc::clone(self);
        let request = request.clone();
        tokio::spawn(async move {
            let _resource = resource;
            let result = worker.call(&request).await;
            if result.is_err() && worker.last_cleanup_receipt().is_some() {
                // The carrier, not a provider error kind, determines synchronization.
                // A complete unary rejection leaves the same worker and cursors alive.
                if let Err(cleanup_error) = supervisor.fail_active_worker(&mut worker).await {
                    tracing::warn!(error = %cleanup_error, "secondary provider worker cleanup failure");
                }
            }
            drop(worker);
            drop(lease);
            result
        })
        .await
        .map_err(|_| lifecycle_lock_error())?
    }

    /// Queue expiry never writes to stdio. Once dispatched, the carrier owns
    /// timeout retirement and bounded cleanup; callers must await this future.
    pub(crate) async fn call_with_deadline(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        limits: &PluginRuntimeLimits,
        deadline_unix_ms: i64,
    ) -> FrameworkResult<Value> {
        let lease = self.admit()?;
        if let Some(multiplex) = &self.multiplex {
            if multiplex.is_failed() {
                self.fail_multiplex().await?;
            }
            self.ensure_multiplex_can_dispatch(multiplex)?;
            let remaining = control_remaining(deadline_unix_ms)?;
            let mut limits = limits.clone();
            limits.timeout_ms = Some(remaining.as_millis().min(u64::MAX as u128) as u64);
            let result = multiplex
                .call(request, &limits, None, Some(Box::new(lease)))
                .await
                .and_then(|(response, _)| self.multiplex_response(response));
            if multiplex.is_failed() {
                if let Err(error) = self.fail_multiplex().await {
                    tracing::warn!(%error, "secondary multiplex cleanup failure");
                }
            }
            return result;
        }
        let mut worker = tokio::time::timeout(
            control_remaining(deadline_unix_ms)?,
            Arc::clone(&self.worker).lock_owned(),
        )
        .await
        .map_err(|_| control_deadline_error())?;
        self.ensure_lease_can_dispatch(&worker)?;
        let supervisor = Arc::clone(self);
        let request = request.clone();
        let limits = limits.clone();
        tokio::spawn(async move {
            let remaining = control_remaining(deadline_unix_ms)?;
            let mut limits = limits.clone();
            limits.timeout_ms = Some(remaining.as_millis().min(u64::MAX as u128) as u64);
            let result = worker.call_with_limits(&request, &limits).await;
            if result.is_err() && worker.last_cleanup_receipt().is_some() {
                if let Err(error) = supervisor.fail_active_worker(&mut worker).await {
                    tracing::warn!(error = %error, "secondary provider control cleanup failure");
                }
            }
            drop(worker);
            drop(lease);
            result
        })
        .await
        .map_err(|_| lifecycle_lock_error())?
    }

    #[cfg(test)]
    pub(crate) async fn call_streaming_with_limits_and_host_calls(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: crate::stdio_runtime::StreamingCallContext,
    ) -> FrameworkResult<StreamingProviderOutput> {
        self.call_streaming_inner(request, timeout_limits, context, None)
            .await
    }

    pub(crate) async fn call_streaming_admitted(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: crate::stdio_runtime::StreamingCallContext,
        resource: Box<dyn Send + Sync>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        self.call_streaming_inner(request, timeout_limits, context, Some(resource))
            .await
    }

    pub(super) async fn call_streaming_inner(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: crate::stdio_runtime::StreamingCallContext,
        resource: Option<Box<dyn Send + Sync>>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let lease = self.admit()?;
        if let Some(multiplex) = &self.multiplex {
            if multiplex.is_failed() {
                self.fail_multiplex().await?;
            }
            self.ensure_multiplex_can_dispatch(multiplex)?;
            let result = multiplex
                .call(
                    request,
                    timeout_limits,
                    Some(context),
                    Some(Box::new((lease, resource))),
                )
                .await
                .and_then(|(response, events)| {
                    let result = serde_json::from_value(response).map_err(|error| {
                        PluginFrameworkError::serialization(None, error.to_string())
                    })?;
                    Ok(StreamingProviderOutput { events, result })
                });
            if multiplex.is_failed() {
                if let Err(error) = self.fail_multiplex().await {
                    tracing::warn!(%error, "secondary multiplex cleanup failure");
                }
            }
            return result;
        }
        let mut worker = Arc::clone(&self.worker).lock_owned().await;
        self.ensure_lease_can_dispatch(&worker)?;
        let supervisor = Arc::clone(self);
        let request = request.clone();
        let timeout_limits = timeout_limits.clone();
        tokio::spawn(async move {
            let _resource = resource;
            let result = worker
                .call_streaming_with_limits_and_host_calls(&request, &timeout_limits, context)
                .await;
            if result.is_err() && worker.last_cleanup_receipt().is_some() {
                // Stdio retires only broken streams. A provider error drained through
                // its result delimiter leaves the worker usable for the next call.
                // Lifecycle cleanup is secondary and cannot replace the primary error.
                if let Err(cleanup_error) = supervisor.fail_active_worker(&mut worker).await {
                    tracing::warn!(error = %cleanup_error, "secondary provider worker cleanup failure");
                }
            }
            drop(worker);
            drop(lease);
            result
        })
        .await
        .map_err(|_| lifecycle_lock_error())?
    }
}
