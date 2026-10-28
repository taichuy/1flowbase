use super::*;
use crate::stdio_runtime::ProviderCallCancellation;
use tokio::sync::{oneshot, watch};

// Same bounded shutdown policy as RuntimeExtensionHost::stop. A serial carrier without
// per-call cancellation may outlive this budget; retain its owner and report failure.
const CANCEL_JOIN_BUDGET: std::time::Duration = std::time::Duration::from_secs(5);

pub(super) struct ActiveRuntimeRequest {
    cancellation: ProviderCallCancellation,
    completed: watch::Sender<bool>,
}

impl ActiveRuntimeRequest {
    fn new() -> Self {
        Self {
            cancellation: ProviderCallCancellation::new(),
            completed: watch::channel(false).0,
        }
    }

    pub(super) fn cancel(&self) {
        self.cancellation.cancel();
    }

    pub(super) async fn join(
        &self,
        request_id: &RuntimeRequestId,
    ) -> Result<(), RuntimeBackendError> {
        let mut completed = self.completed.subscribe();
        tokio::time::timeout(CANCEL_JOIN_BUDGET, async {
            while !*completed.borrow_and_update() {
                if completed.changed().await.is_err() {
                    break;
                }
            }
        })
        .await
        .map_err(|_| RuntimeBackendError::Execution {
            target_id: request_id.to_string(),
            message:
                "runtime cancellation could not join the owned carrier within its shutdown budget"
                    .into(),
        })?;
        if let Some(message) = self.cancellation.cleanup_failure() {
            return Err(RuntimeBackendError::Execution {
                target_id: request_id.to_string(),
                message,
            });
        }
        Ok(())
    }
}

impl RuntimeExtensionHost {
    pub(super) async fn execute_owned(
        &self,
        request: RuntimeExecutionRequest,
        sinks: RuntimeStreamSinks,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        let request_id = request.request_id.clone();
        let target_id = request.target.as_str().to_owned();
        let mut active = self.active_requests.lock().await;
        self.ensure_accepting()?;
        if active.contains_key(&request_id) {
            return Err(RuntimeBackendError::DuplicateRequest(request_id));
        }
        let owner = Arc::new(ActiveRuntimeRequest::new());
        active.insert(request_id.clone(), owner.clone());
        let cancellation = owner.cancellation.clone();
        let providers = self.provider_host.clone();
        let plugin_data = self.plugin_data.clone();
        let task = tokio::spawn(execute_provider(
            providers,
            plugin_data,
            request,
            sinks,
            cancellation,
        ));
        let registry = self.active_requests.clone();
        let (result_tx, result_rx) = oneshot::channel();
        // The registry and this join owner survive an ordinary waiter drop. Only cancel()
        // signals the producer; normal protocol disconnect never becomes business cancel.
        tokio::spawn(async move {
            let result = match task.await {
                Ok(result) => result,
                Err(error) => {
                    owner
                        .cancellation
                        .failed_cleanup(format!("runtime execution owner failed: {error}"));
                    Err(RuntimeBackendError::Execution {
                        target_id,
                        message: error.to_string(),
                    })
                }
            };
            // Failed cleanup remains visible/fenced: a later cancel cannot mistake an
            // unconfirmed physical exit for NotFound and report a false success.
            if owner.cancellation.cleanup_failure().is_none() {
                registry.lock().await.remove(&request_id);
            }
            owner.completed.send_replace(true);
            let _ = result_tx.send(result);
        });
        drop(active);
        result_rx.await.map_err(|_| {
            RuntimeBackendError::InvalidRequest("runtime execution join owner disappeared".into())
        })?
    }
}

async fn execute_provider(
    provider_host: Arc<RwLock<ProviderHost>>,
    plugin_data: Arc<dyn PluginDataPort>,
    request: RuntimeExecutionRequest,
    sinks: RuntimeStreamSinks,
    cancellation: ProviderCallCancellation,
) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
    let target_id = request.target.as_str().to_owned();
    let mut forwarders = Vec::new();
    let mut channel = |sink: Option<Arc<dyn RuntimeStreamEventSink>>| {
        sink.map(|sink| {
            let (sender, receiver) = mpsc::channel(64);
            forwarders.push(tokio::spawn(forward_events(receiver, sink)));
            sender
        })
    };
    let required = channel(sinks.required);
    let diagnostic = channel(sinks.diagnostic);
    let result = async {
        let operation = {
            let host = provider_host.read().await;
            host.invoke_stream_with_host_calls_operation(
                &target_id,
                request.input,
                required,
                diagnostic,
                sinks.protocol_observation,
                (
                    request.principal,
                    Some(plugin_data),
                    Some(cancellation.clone()),
                ),
            )
            .map_err(RuntimeBackendError::from)?
        };
        operation.await.map_err(RuntimeBackendError::from)
    }
    .await;
    let mut forwarding_error = None;
    for mut task in forwarders {
        if result.is_err() || cancellation.is_cancelled() {
            task.abort();
            let _ = task.await;
            continue;
        }
        let joined = tokio::select! {
            biased;
            _ = cancellation.cancelled() => {
                task.abort();
                let _ = task.await;
                continue;
            }
            joined = &mut task => joined,
        };
        let joined = joined
            .map_err(|error| RuntimeBackendError::Execution {
                target_id: target_id.clone(),
                message: error.to_string(),
            })
            .and_then(|result| result);
        if let Err(error) = joined {
            forwarding_error.get_or_insert(error);
        }
    }
    if cancellation.is_cancelled() {
        return Err(RuntimeBackendError::Cancelled(request.request_id));
    }
    let output = result?;
    if let Some(error) = forwarding_error {
        return Err(error);
    }
    Ok(RuntimeExecutionOutcome {
        events: output.events,
        result: output.result,
    })
}
