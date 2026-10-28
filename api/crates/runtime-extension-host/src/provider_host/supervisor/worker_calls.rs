use super::*;
use crate::stdio_runtime::ProviderCallCancellation;

struct CallReleased(Option<tokio::sync::oneshot::Sender<()>>);
impl Drop for CallReleased {
    fn drop(&mut self) {
        if let Some(released) = self.0.take() {
            let _ = released.send(());
        }
    }
}

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
        self.call_streaming_inner(request, timeout_limits, context, None, None)
            .await
    }

    #[cfg(test)]
    pub(crate) async fn call_streaming_admitted(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: crate::stdio_runtime::StreamingCallContext,
        resource: Box<dyn Send + Sync>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        self.call_streaming_inner(request, timeout_limits, context, Some(resource), None)
            .await
    }

    pub(crate) async fn call_streaming_cancellable(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: crate::stdio_runtime::StreamingCallContext,
        resource: Box<dyn Send + Sync>,
        cancellation: Option<ProviderCallCancellation>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        self.call_streaming_inner(
            request,
            timeout_limits,
            context,
            Some(resource),
            cancellation,
        )
        .await
    }

    pub(super) async fn call_streaming_inner(
        self: &Arc<Self>,
        request: &ProviderStdioRequest,
        timeout_limits: &PluginRuntimeLimits,
        context: crate::stdio_runtime::StreamingCallContext,
        resource: Option<Box<dyn Send + Sync>>,
        cancellation: Option<ProviderCallCancellation>,
    ) -> FrameworkResult<StreamingProviderOutput> {
        let lease = self.admit()?;
        if let Some(multiplex) = &self.multiplex {
            if multiplex.is_failed() {
                self.fail_multiplex().await?;
            }
            self.ensure_multiplex_can_dispatch(multiplex)?;
            let (released_tx, released_rx) = tokio::sync::oneshot::channel();
            let result = multiplex
                .call_cancellable(
                    request,
                    timeout_limits,
                    Some(context),
                    Some(Box::new((lease, resource, CallReleased(Some(released_tx))))),
                    cancellation
                        .as_ref()
                        .map(ProviderCallCancellation::subscribe),
                )
                .await
                .and_then(|(response, events)| {
                    let result = serde_json::from_value(response).map_err(|error| {
                        PluginFrameworkError::serialization(None, error.to_string())
                    })?;
                    Ok(StreamingProviderOutput { events, result })
                });
            // The actor owns callbacks and admission until the result/cancel delimiter or
            // confirmed process retirement. Its resource release is the join boundary.
            let _ = released_rx.await;
            if multiplex.is_failed() {
                if let Err(error) = self.fail_multiplex().await {
                    tracing::warn!(%error, "secondary multiplex cleanup failure");
                }
            }
            if let Some(cancellation) = &cancellation {
                if multiplex.is_failed() && !self.process_control.confirmed_exit().await {
                    cancellation
                        .failed_cleanup("multiplex provider process exit was not confirmed");
                }
            }
            return result;
        }
        let mut worker = ProviderCallCancellation::run(cancellation.as_ref(), async {
            Ok(Arc::clone(&self.worker).lock_owned().await)
        })
        .await?;
        self.ensure_lease_can_dispatch(&worker)?;
        let supervisor = Arc::clone(self);
        let request = request.clone();
        let timeout_limits = timeout_limits.clone();
        // Serial stdio has no call-specific cancellation frame. Once dispatched, keep
        // the original carrier owner until its result/deadline; never kill peers or
        // retained sessions. Runtime cancel reports a bounded join failure if necessary.
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
