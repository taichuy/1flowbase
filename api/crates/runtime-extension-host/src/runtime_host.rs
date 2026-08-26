use std::{
    collections::HashMap,
    sync::{Arc, RwLock as StdRwLock},
};

use async_trait::async_trait;
use runtime_core::runtime_backend::{
    RuntimeBackendError, RuntimeBackendLifecycle, RuntimeBackendSnapshot, RuntimeCancelOutcome,
    RuntimeExecutionOutcome, RuntimeExecutionPort, RuntimeExecutionRequest, RuntimeObservationPort,
    RuntimeRegistrySnapshot, RuntimeRequestId, RuntimeStreamEventSink, RuntimeStreamSinks,
};
use runtime_profile::{RuntimeProfile, RuntimeProfileCollector};
use time::OffsetDateTime;
use tokio::{
    sync::{mpsc, Mutex, RwLock},
    task::AbortHandle,
};

use crate::{
    capability_host::CapabilityHost, data_source_host::DataSourceHost,
    network_egress_host::NetworkEgressHost, provider_host::ProviderHost,
};

#[derive(Clone)]
pub struct RuntimeExtensionHost {
    provider_host: Arc<RwLock<ProviderHost>>,
    capability_host: Arc<RwLock<CapabilityHost>>,
    data_source_host: Arc<RwLock<DataSourceHost>>,
    network_egress_host: Arc<RwLock<NetworkEgressHost>>,
    profile: Arc<RuntimeProfileCollector>,
    lifecycle: Arc<StdRwLock<RuntimeBackendLifecycle>>,
    active_requests: Arc<Mutex<HashMap<RuntimeRequestId, AbortHandle>>>,
}

impl std::fmt::Debug for RuntimeExtensionHost {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter
            .debug_struct("RuntimeExtensionHost")
            .field("lifecycle", &self.lifecycle())
            .finish_non_exhaustive()
    }
}

impl RuntimeExtensionHost {
    pub fn new(process_started_at: OffsetDateTime) -> Result<Self, RuntimeBackendError> {
        Self::from_registries(
            process_started_at,
            ProviderHost::default(),
            CapabilityHost::default(),
            DataSourceHost::default(),
        )
    }

    pub fn from_registries(
        process_started_at: OffsetDateTime,
        provider_host: ProviderHost,
        capability_host: CapabilityHost,
        data_source_host: DataSourceHost,
    ) -> Result<Self, RuntimeBackendError> {
        Self::from_shared_registries(
            process_started_at,
            Arc::new(RwLock::new(provider_host)),
            Arc::new(RwLock::new(capability_host)),
            Arc::new(RwLock::new(data_source_host)),
        )
    }

    pub fn from_shared_registries(
        process_started_at: OffsetDateTime,
        provider_host: Arc<RwLock<ProviderHost>>,
        capability_host: Arc<RwLock<CapabilityHost>>,
        data_source_host: Arc<RwLock<DataSourceHost>>,
    ) -> Result<Self, RuntimeBackendError> {
        let profile = RuntimeProfileCollector::new(
            "plugin-runner",
            env!("CARGO_PKG_VERSION"),
            process_started_at,
            "ok",
        )
        .map_err(|error| RuntimeBackendError::Execution {
            target_id: "runtime-extension-host".to_string(),
            message: error.to_string(),
        })?;
        Ok(Self {
            provider_host,
            capability_host,
            data_source_host,
            network_egress_host: Arc::new(RwLock::new(NetworkEgressHost::default())),
            profile: Arc::new(profile),
            lifecycle: Arc::new(StdRwLock::new(RuntimeBackendLifecycle::Starting)),
            active_requests: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub fn mark_ready(&self) -> Result<(), RuntimeBackendError> {
        let mut lifecycle = self.lifecycle.write().map_err(|_| {
            RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".to_string())
        })?;
        match *lifecycle {
            RuntimeBackendLifecycle::Starting | RuntimeBackendLifecycle::Ready => {
                *lifecycle = RuntimeBackendLifecycle::Ready;
                Ok(())
            }
            state => Err(RuntimeBackendError::Unavailable(state)),
        }
    }

    pub fn lifecycle(&self) -> RuntimeBackendLifecycle {
        self.lifecycle
            .read()
            .map(|state| *state)
            .unwrap_or(RuntimeBackendLifecycle::Failed)
    }

    pub fn collect_runtime_profile(&self) -> Result<RuntimeProfile, RuntimeBackendError> {
        self.profile
            .collect()
            .map_err(|error| RuntimeBackendError::Execution {
                target_id: "runtime-extension-host".to_string(),
                message: error.to_string(),
            })
    }

    pub async fn drain(&self) -> Result<(), RuntimeBackendError> {
        {
            let mut lifecycle = self.lifecycle.write().map_err(|_| {
                RuntimeBackendError::InvalidRequest(
                    "runtime lifecycle lock is poisoned".to_string(),
                )
            })?;
            match *lifecycle {
                RuntimeBackendLifecycle::Starting | RuntimeBackendLifecycle::Ready => {
                    *lifecycle = RuntimeBackendLifecycle::Draining;
                }
                RuntimeBackendLifecycle::Draining | RuntimeBackendLifecycle::Stopped => {}
                state => return Err(RuntimeBackendError::Unavailable(state)),
            }
        }

        let handles = self
            .active_requests
            .lock()
            .await
            .drain()
            .map(|(_, handle)| handle)
            .collect::<Vec<_>>();
        for handle in handles {
            handle.abort();
        }
        Ok(())
    }

    pub async fn stop(&self) -> Result<(), RuntimeBackendError> {
        self.drain().await?;
        let mut first_error = None;
        if let Err(error) = self.provider_host.write().await.stop_all().await {
            first_error = Some(RuntimeBackendError::from(error));
        }
        if let Err(error) = self.data_source_host.write().await.stop_all().await {
            first_error.get_or_insert_with(|| RuntimeBackendError::from(error));
        }
        if let Err(error) = self.capability_host.write().await.stop_all().await {
            first_error.get_or_insert_with(|| RuntimeBackendError::from(error));
        }
        if let Err(error) = self.network_egress_host.write().await.stop_all().await {
            first_error.get_or_insert_with(|| RuntimeBackendError::from(error));
        }
        let mut lifecycle = self.lifecycle.write().map_err(|_| {
            RuntimeBackendError::InvalidRequest("runtime lifecycle lock is poisoned".to_string())
        })?;
        if let Some(error) = first_error {
            *lifecycle = RuntimeBackendLifecycle::Failed;
            Err(error)
        } else {
            *lifecycle = RuntimeBackendLifecycle::Stopped;
            Ok(())
        }
    }

    pub fn provider_registry(&self) -> &Arc<RwLock<ProviderHost>> {
        &self.provider_host
    }

    pub fn capability_registry(&self) -> &Arc<RwLock<CapabilityHost>> {
        &self.capability_host
    }

    pub fn data_source_registry(&self) -> &Arc<RwLock<DataSourceHost>> {
        &self.data_source_host
    }

    pub fn network_egress_registry(&self) -> &Arc<RwLock<NetworkEgressHost>> {
        &self.network_egress_host
    }

    fn ensure_accepting(&self) -> Result<(), RuntimeBackendError> {
        match self.lifecycle() {
            RuntimeBackendLifecycle::Ready => Ok(()),
            state => Err(RuntimeBackendError::Unavailable(state)),
        }
    }
}

async fn forward_events(
    mut receiver: mpsc::Receiver<extension_contracts::provider_contract::ProviderStreamEvent>,
    sink: Arc<dyn RuntimeStreamEventSink>,
) -> Result<(), RuntimeBackendError> {
    while let Some(event) = receiver.recv().await {
        sink.emit(event).await?;
    }
    Ok(())
}

#[async_trait]
impl RuntimeExecutionPort for RuntimeExtensionHost {
    async fn execute(
        &self,
        request: RuntimeExecutionRequest,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        self.execute_stream(request, RuntimeStreamSinks::default())
            .await
    }

    async fn execute_stream(
        &self,
        request: RuntimeExecutionRequest,
        sinks: RuntimeStreamSinks,
    ) -> Result<RuntimeExecutionOutcome, RuntimeBackendError> {
        self.ensure_accepting()?;
        let request_id = request.request_id.clone();
        let target_id = request.target.as_str().to_string();
        let provider_host = Arc::clone(&self.provider_host);

        let mut active = self.active_requests.lock().await;
        if active.contains_key(&request_id) {
            return Err(RuntimeBackendError::DuplicateRequest(request_id));
        }

        let task = tokio::spawn(async move {
            let (required_sender, required_forwarder) = match sinks.required {
                Some(sink) => {
                    let (sender, receiver) = mpsc::channel(64);
                    (
                        Some(sender),
                        Some(tokio::spawn(forward_events(receiver, sink))),
                    )
                }
                None => (None, None),
            };
            let (diagnostic_sender, diagnostic_forwarder) = match sinks.diagnostic {
                Some(sink) => {
                    let (sender, receiver) = mpsc::channel(64);
                    (
                        Some(sender),
                        Some(tokio::spawn(forward_events(receiver, sink))),
                    )
                }
                None => (None, None),
            };
            let operation = {
                let host = provider_host.read().await;
                host.invoke_stream_with_live_events_operation(
                    &target_id,
                    request.input,
                    required_sender,
                    diagnostic_sender,
                )
                .map_err(RuntimeBackendError::from)?
            };
            let output = operation.await.map_err(RuntimeBackendError::from)?;
            if let Some(forwarder) = required_forwarder {
                forwarder
                    .await
                    .map_err(|error| RuntimeBackendError::Execution {
                        target_id: target_id.clone(),
                        message: error.to_string(),
                    })??;
            }
            if let Some(forwarder) = diagnostic_forwarder {
                forwarder
                    .await
                    .map_err(|error| RuntimeBackendError::Execution {
                        target_id: target_id.clone(),
                        message: error.to_string(),
                    })??;
            }
            Ok(RuntimeExecutionOutcome {
                events: output.events,
                result: output.result,
            })
        });

        active.insert(request_id.clone(), task.abort_handle());
        drop(active);
        let result = task.await;
        self.active_requests.lock().await.remove(&request_id);
        match result {
            Ok(result) => result,
            Err(error) if error.is_cancelled() => Err(RuntimeBackendError::Cancelled(request_id)),
            Err(error) => Err(RuntimeBackendError::Execution {
                target_id: request.target.as_str().to_string(),
                message: error.to_string(),
            }),
        }
    }

    async fn cancel(
        &self,
        request_id: &RuntimeRequestId,
    ) -> Result<RuntimeCancelOutcome, RuntimeBackendError> {
        let Some(handle) = self.active_requests.lock().await.remove(request_id) else {
            return Ok(RuntimeCancelOutcome::NotFound);
        };
        handle.abort();
        Ok(RuntimeCancelOutcome::Cancelled)
    }
}

#[async_trait]
impl RuntimeObservationPort for RuntimeExtensionHost {
    async fn snapshot(&self) -> Result<RuntimeBackendSnapshot, RuntimeBackendError> {
        let providers = self.provider_host.read().await.loaded_count();
        let capabilities = self.capability_host.read().await.loaded_count();
        let data_sources = self.data_source_host.read().await.loaded_count();
        let network_egress_providers = self.network_egress_host.read().await.loaded_count();
        let mut active_request_ids = self
            .active_requests
            .lock()
            .await
            .keys()
            .map(ToString::to_string)
            .collect::<Vec<_>>();
        active_request_ids.sort();
        Ok(RuntimeBackendSnapshot {
            backend_kind: "in_process".to_string(),
            lifecycle: self.lifecycle(),
            registries: RuntimeRegistrySnapshot {
                providers,
                data_sources,
                capabilities,
                network_egress_providers,
            },
            active_request_ids,
        })
    }
}
