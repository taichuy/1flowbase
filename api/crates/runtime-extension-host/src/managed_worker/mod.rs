mod binding;
mod capability;
mod capability_multiplex;
mod event;
mod event_stdio;
mod hook;
mod hook_stdio;
mod process;
mod service_workers;
pub(crate) use binding::LoadedManagedBinding;

use std::{collections::BTreeMap, num::NonZeroU64, sync::Arc};

use extension_contracts::extension_bus::{ManagedExecutionHandle, ManagedExecutionIdentity};
use extension_package_runtime::{FrameworkResult, PluginExecutionMode, PluginFrameworkError};
use runtime_core::runtime_backend::RuntimeManagedCapabilityRequest;
use serde_json::{json, Value};

use crate::{
    capability_stdio::{CapabilityStdioMethod, CapabilityStdioRequest},
    plugin_scope::PluginScope,
};

#[derive(Debug)]
struct MountedContribution {
    handle: ManagedExecutionHandle,
    binding: LoadedManagedBinding,
    scope: Arc<PluginScope>,
    service_workers: Option<Arc<service_workers::ServiceWorkers>>,
}

/// Unbound host ports; execute_with_services applies the contribution grants
/// and caller binding before any port reaches a worker callback.
pub(crate) struct ManagedHostServices {
    pub(crate) plugin_data: Arc<dyn extension_contracts::PluginDataPort>,
    pub(crate) plugin_credentials: Option<Arc<dyn extension_contracts::PluginCredentialPort>>,
    pub(crate) file_import: Option<Arc<dyn runtime_core::runtime_backend::FileImportPort>>,
    pub(crate) file_read: Option<Arc<dyn runtime_core::runtime_backend::FileReadPort>>,
    pub(crate) application_logs:
        Option<Arc<dyn runtime_core::runtime_backend::ApplicationLogsPort>>,
    pub(crate) http_egress: Option<Arc<dyn runtime_core::runtime_backend::HttpEgressPort>>,
}

#[derive(Debug)]
pub(crate) struct ManagedWorkers {
    mounted: BTreeMap<ManagedExecutionIdentity, MountedContribution>,
    next_generation: u64,
    budget: Arc<tokio::sync::Semaphore>,
}

impl Default for ManagedWorkers {
    fn default() -> Self {
        Self {
            mounted: BTreeMap::new(),
            next_generation: 0,
            budget: Arc::new(tokio::sync::Semaphore::new(128)),
        }
    }
}

struct ManagedDrain(Vec<crate::plugin_scope::PluginScopeDrain>);
impl runtime_core::runtime_backend::RuntimeManagedDrain for ManagedDrain {
    fn wait_drained(
        &self,
    ) -> std::pin::Pin<
        Box<
            dyn std::future::Future<
                    Output = Result<(), runtime_core::runtime_backend::RuntimeBackendError>,
                > + Send
                + '_,
        >,
    > {
        Box::pin(async move {
            for scope in &self.0 {
                scope.wait_drained().await?;
            }
            Ok(())
        })
    }
}

impl ManagedWorkers {
    pub(crate) fn drain(
        &self,
        handles: &[ManagedExecutionHandle],
    ) -> FrameworkResult<Box<dyn runtime_core::runtime_backend::RuntimeManagedDrain>> {
        let scopes = handles
            .iter()
            .map(|handle| {
                self.exact_mount(handle)
                    .map(|mounted| mounted.scope.clone())
            })
            .collect::<FrameworkResult<Vec<_>>>()?;
        let drains = scopes
            .iter()
            .map(|scope| scope.begin_drain())
            .collect::<FrameworkResult<Vec<_>>>()?;
        Ok(Box::new(ManagedDrain(drains)))
    }
    pub(crate) fn mount(
        &mut self,
        identity: ManagedExecutionIdentity,
        binding: LoadedManagedBinding,
    ) -> FrameworkResult<ManagedExecutionHandle> {
        if let Some(mounted) = self.mounted.get(&identity) {
            if mounted.binding.executable_fingerprint != binding.executable_fingerprint {
                return Err(invalid("frozen managed executable bytes changed"));
            }
            return Ok(mounted.handle.clone());
        }
        let generation = self
            .next_generation
            .checked_add(1)
            .and_then(NonZeroU64::new)
            .ok_or_else(|| invalid("managed worker generation exhausted"))?;
        self.next_generation = generation.get();
        let handle = ManagedExecutionHandle::new(identity.clone(), generation);
        let service_workers = binding.worker_idle_timeout_ms.map(|idle_ms| {
            self.mounted
                .iter()
                .find_map(|(existing, mounted)| {
                    (existing.installation_id() == identity.installation_id()
                        && existing.workspace_id() == identity.workspace_id()
                        && existing.artifact_fingerprint() == identity.artifact_fingerprint()
                        && mounted.binding.executable_fingerprint == binding.executable_fingerprint
                        && mounted.binding.runtime_executable == binding.runtime_executable
                        && mounted.binding.actor_workspace_scope == binding.actor_workspace_scope)
                        .then(|| mounted.service_workers.clone())
                        .flatten()
                })
                .unwrap_or_else(|| {
                    service_workers::ServiceWorkers::new(idle_ms, self.budget.clone())
                })
        });
        self.mounted.insert(
            identity,
            MountedContribution {
                handle: handle.clone(),
                binding,
                service_workers,
                scope: PluginScope::managed(generation.get(), self.budget.clone()),
            },
        );
        Ok(handle)
    }

    pub(crate) fn unmount(
        &mut self,
        handle: &ManagedExecutionHandle,
    ) -> FrameworkResult<Arc<PluginScope>> {
        let mounted = self.exact_mount(handle)?;
        mounted.scope.close_admission()?;
        Ok(mounted.scope.clone())
    }

    pub(crate) fn service_pools(&self) -> Vec<Arc<service_workers::ServiceWorkers>> {
        let mut pools = Vec::new();
        for mounted in self.mounted.values() {
            if let Some(pool) = &mounted.service_workers {
                if !pools.iter().any(|old| Arc::ptr_eq(old, pool)) {
                    pools.push(pool.clone());
                }
            }
        }
        pools
    }

    pub(crate) fn service_pool_for_handle(
        &self,
        handle: &ManagedExecutionHandle,
    ) -> FrameworkResult<Option<Arc<service_workers::ServiceWorkers>>> {
        Ok(self.exact_mount(handle)?.service_workers.clone())
    }

    pub(crate) fn finish_unmount(&mut self, handle: &ManagedExecutionHandle) {
        if self
            .mounted
            .get(handle.identity())
            .is_some_and(|m| &m.handle == handle)
        {
            self.mounted.remove(handle.identity());
        }
    }

    pub(crate) fn close_admission(&self) -> FrameworkResult<()> {
        for mounted in self.mounted.values() {
            mounted.scope.close_admission()?;
        }
        Ok(())
    }

    pub(crate) fn scopes(&self) -> Vec<Arc<PluginScope>> {
        self.mounted
            .values()
            .map(|mounted| mounted.scope.clone())
            .collect()
    }
    pub(crate) fn clear_disposed(&mut self) {
        self.mounted.clear();
    }

    pub(crate) fn loaded_count(&self) -> usize {
        self.mounted.len()
    }

    #[cfg(test)]
    pub(crate) fn execute(
        &self,
        request: RuntimeManagedCapabilityRequest,
    ) -> Result<
        impl std::future::Future<Output = FrameworkResult<Value>> + Send + 'static,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        self.execute_bound(request, None)
    }

    pub(crate) fn execute_with_services(
        &self,
        request: RuntimeManagedCapabilityRequest,
        services: ManagedHostServices,
    ) -> Result<
        impl std::future::Future<Output = FrameworkResult<Value>> + Send + 'static,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        let ManagedHostServices {
            plugin_data,
            plugin_credentials,
            file_import,
            file_read,
            application_logs,
            http_egress,
        } = services;
        let binding = &self.exact_mount(&request.handle)?.binding;
        let permissions = binding
            .contribution
            .required_permissions
            .iter()
            .map(|p| p.as_str())
            .collect::<std::collections::BTreeSet<_>>();
        let mut data_permissions = std::collections::BTreeSet::new();
        if permissions.contains("plugin_data.owned.read")
            || permissions.contains("plugin_data.owned.write")
        {
            data_permissions.insert(extension_contracts::PluginDataPermission::Read);
        }
        if permissions.contains("plugin_data.owned.write") {
            data_permissions.insert(extension_contracts::PluginDataPermission::Write);
        }
        let host_calls = crate::stdio_runtime::ProviderHostCallContext {
            binding: extension_contracts::PluginDataBinding {
                managed_subject: Some(request.handle.identity().subject().clone()),
                publisher_namespace: binding.publisher_namespace.clone(),
                plugin_code: binding.plugin_code.clone(),
                plugin_version: binding.plugin_version.clone(),
                storage_binding: "main".into(),
                workspace_id: request.principal.workspace_id.clone(),
                actor_id: request.principal.actor_id.clone(),
                provider_instance_id: request.handle.identity().installation_id().as_str().into(),
                permissions: data_permissions,
                deadline_unix_ms: request.principal.deadline_unix_ms,
            },
            application_logs: if permissions.contains("application.logs.read")
                || permissions.contains("application.logs.import")
            {
                application_logs
                    .zip(request.principal.caller.clone())
                    .map(|(port, caller)| {
                        (
                            runtime_core::runtime_backend::FileImportBinding {
                                subject: request.handle.identity().subject().clone(),
                                publisher_namespace: binding.publisher_namespace.clone(),
                                plugin_code: binding.plugin_code.clone(),
                                plugin_version: binding.plugin_version.clone(),
                                caller,
                                deadline_unix_ms: request.principal.deadline_unix_ms,
                            },
                            port,
                        )
                    })
            } else {
                None
            },
            http_egress: if permissions.contains("network.http.request") {
                http_egress
                    .filter(|_| {
                        request.principal.caller.is_some()
                            || binding.contribution.point_id.as_str()
                                == extension_contracts::AUTHENTICATION_POINT
                    })
                    .map(|port| {
                        (
                            runtime_core::runtime_backend::HttpEgressBinding {
                                subject: request.handle.identity().subject().clone(),
                                publisher_namespace: binding.publisher_namespace.clone(),
                                plugin_code: binding.plugin_code.clone(),
                                plugin_version: binding.plugin_version.clone(),
                                caller: request.principal.caller.clone(),
                                deadline_unix_ms: request.principal.deadline_unix_ms,
                            },
                            port,
                        )
                    })
            } else {
                None
            },
            plugin_data,
            file_read: if permissions.contains("file.read.content") {
                file_read
                    .zip(request.principal.caller.clone())
                    .map(|(port, caller)| {
                        crate::file_read_host::FileReadCall::new(
                            runtime_core::runtime_backend::FileReadBinding {
                                subject: request.handle.identity().subject().clone(),
                                publisher_namespace: binding.publisher_namespace.clone(),
                                plugin_code: binding.plugin_code.clone(),
                                plugin_version: binding.plugin_version.clone(),
                                caller,
                                deadline_unix_ms: request.principal.deadline_unix_ms,
                            },
                            port,
                        )
                    })
            } else {
                None
            },
            file_import: if permissions.contains("file.import.resource") {
                file_import
                    .zip(request.principal.caller.clone())
                    .map(|(port, caller)| {
                        (
                            runtime_core::runtime_backend::FileImportBinding {
                                subject: request.handle.identity().subject().clone(),
                                publisher_namespace: binding.publisher_namespace.clone(),
                                plugin_code: binding.plugin_code.clone(),
                                plugin_version: binding.plugin_version.clone(),
                                caller,
                                deadline_unix_ms: request.principal.deadline_unix_ms,
                            },
                            port,
                        )
                    })
            } else {
                None
            },
            plugin_credentials: if permissions.contains("credential.manage") {
                plugin_credentials.map(|port| {
                    (
                        extension_contracts::PluginCredentialBinding {
                            installation_id: request
                                .handle
                                .identity()
                                .installation_id()
                                .as_str()
                                .into(),
                            contribution_id: request
                                .handle
                                .identity()
                                .contribution_id()
                                .as_str()
                                .into(),
                            publisher_namespace: binding.publisher_namespace.clone(),
                            plugin_code: binding.plugin_code.clone(),
                            plugin_version: binding.plugin_version.clone(),
                            scope_id: request.principal.workspace_id.clone(),
                            deadline_unix_ms: request.principal.deadline_unix_ms,
                        },
                        port,
                    )
                })
            } else {
                None
            },
        };
        self.execute_bound(request, Some(host_calls))
    }

    fn execute_bound(
        &self,
        request: RuntimeManagedCapabilityRequest,
        host_calls: Option<crate::stdio_runtime::ProviderHostCallContext>,
    ) -> Result<
        impl std::future::Future<Output = FrameworkResult<Value>> + Send + 'static,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        let mounted = self.exact_mount(&request.handle)?;
        // Hook bindings must use their finite typed transport, never opaque capability JSON.
        if mounted
            .binding
            .contribution
            .required_permissions
            .iter()
            .any(|p| matches!(p.as_str(), "event.subscribe" | "event.publish"))
        {
            return Err(invalid("managed event binding requires typed event admission").into());
        }
        if mounted
            .binding
            .contribution
            .point_id
            .as_str()
            .starts_with("1flowbase.model-definitions.create.")
        {
            return Err(invalid("managed Hook binding requires typed Hook admission").into());
        }
        if request.principal.workspace_id != request.handle.identity().workspace_id().as_str() {
            return Err(
                invalid("managed execution workspace does not match the mounted identity").into(),
            );
        }
        if mounted.binding.execution_mode != PluginExecutionMode::ProcessPerCall {
            return Err(invalid(
                "managed contribution is not a process_per_call capability binding",
            )
            .into());
        }
        let call_context = mounted.binding.call_context(&request.principal)?;
        let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        let remaining_ms = i128::from(request.principal.deadline_unix_ms) - now_ms;
        let remaining_ms = u64::try_from(remaining_ms)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid("managed execution deadline has expired"))?;
        // Subscribe before admission so a drain racing with this call cannot be missed.
        let scope_cancel = request
            .duplex
            .as_ref()
            .map(|_| mounted.scope.duplex_cancellation());
        let lease = mounted
            .scope
            .admit_generation(request.handle.generation().get())?;
        let shared = if let Some(pool) = &mounted.service_workers {
            let actor = request
                .principal
                .actor_id
                .as_ref()
                .filter(|actor| !actor.is_empty())
                .ok_or_else(|| invalid("actor_scope service requires a trusted actor"))?
                .clone();
            // JSON tuple encoding is unambiguous even if identity formats change.
            let key = if let Some(context) = &call_context {
                serde_json::to_string(&(&context.actor_id, &context.workspace_id))
                    .map_err(|_| invalid("managed service worker identity unavailable"))?
            } else {
                actor
            };
            Some((pool.clone(), pool.epoch()?, key))
        } else {
            None
        };
        let mut binding = mounted.binding.clone();
        // Duplex lifetimes belong to the admitted principal and protocol/plugin cancellation.
        // Provider/unary runtime limits do not impose a total terminal session duration.
        let duplex_call = request.duplex.is_some();
        if !duplex_call {
            binding.limits.timeout_ms = Some(
                binding
                    .limits
                    .timeout_ms
                    .unwrap_or(
                        if binding.protocol == extension_contracts::STDIO_JSON_MULTIPLEX_V1 {
                            remaining_ms
                        } else {
                            30_000
                        },
                    )
                    .min(remaining_ms),
            );
        }
        if request.duplex.is_some()
            && binding.protocol != extension_contracts::STDIO_JSON_MULTIPLEX_V1
        {
            return Err(invalid("duplex requires multiplex transport").into());
        }
        Ok(async move {
            let (cancel_tx, cancel_rx) = tokio::sync::watch::channel(false);
            let cancel_task = scope_cancel.map(|mut scope| {
                tokio::spawn(async move {
                    if !scope.has_changed().unwrap_or(true) {
                        let _ = scope.changed().await;
                    }
                    cancel_tx.send_replace(true);
                })
            });
            struct CancelTask(Option<tokio::task::JoinHandle<()>>);
            impl Drop for CancelTask {
                fn drop(&mut self) {
                    if let Some(task) = &self.0 {
                        task.abort();
                    }
                }
            }
            let _cancel_task = CancelTask(cancel_task);
            let indefinite = duplex_call && request.principal.deadline_unix_ms == i64::MAX;
            let duplex = request.duplex;
            let cancel = duplex.as_ref().map(|_| cancel_rx);
            let lease = Arc::new(lease);
            let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            let remaining_ms =
                u64::try_from(i128::from(request.principal.deadline_unix_ms) - now_ms)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| invalid("managed execution deadline has expired"))?;
            let mut input = json!({
                "plugin_id": binding.plugin_id,
                "contribution_code": binding.handler,
                "handler": binding.handler,
                "config_payload": request.config_payload,
                "input_payload": request.input_payload,
                // Project the selected host policy, not a worker/private capacity guess.
                "execution_budget": extension_contracts::CapabilityExecutionBudget {
                    timeout_ms: remaining_ms.min(binding.limits.timeout_ms.unwrap_or(remaining_ms)),
                    memory_bytes: binding.limits.memory_bytes,
                    max_output_bytes: extension_contracts::CAPABILITY_STDIO_FRAME_BYTES,
                },
            });
            if let Some(context) = call_context {
                input["call_context"] = serde_json::to_value(context)
                    .map_err(|_| invalid("managed service caller context unavailable"))?;
            }
            let request = CapabilityStdioRequest {
                method: CapabilityStdioMethod::Execute,
                input,
            };
            let duration_ms = if duplex_call {
                remaining_ms
            } else {
                remaining_ms.min(binding.limits.timeout_ms.unwrap_or(remaining_ms))
            };
            let operation = async {
                if binding.protocol == extension_contracts::STDIO_JSON_MULTIPLEX_V1 {
                    if let Some((pool, epoch, actor)) = shared {
                        let (worker, activity) = pool.acquire(epoch, &actor, &binding)?;
                        capability_multiplex::exchange_shared(
                            &worker,
                            &binding,
                            &request,
                            Box::new((lease, activity)),
                            host_calls,
                            duplex,
                            cancel,
                        )
                        .await
                    } else {
                        capability_multiplex::exchange(
                            &binding, &request, lease, host_calls, duplex, cancel,
                        )
                        .await
                    }
                } else {
                    capability::exchange(&binding, &request, lease).await
                }
            };
            if indefinite {
                // Do not create an enormous timer: i64::MAX is the trusted stream sentinel.
                operation.await
            } else {
                tokio::time::timeout(std::time::Duration::from_millis(duration_ms), operation)
                    .await
                    .map_err(|_| invalid("managed execution deadline elapsed"))?
            }
        })
    }

    pub(crate) fn executable_for_handle(
        &self,
        handle: &ManagedExecutionHandle,
    ) -> FrameworkResult<LoadedManagedBinding> {
        Ok(self.exact_mount(handle)?.binding.clone())
    }

    fn exact_mount(
        &self,
        handle: &ManagedExecutionHandle,
    ) -> FrameworkResult<&MountedContribution> {
        let mounted = self
            .mounted
            .get(handle.identity())
            .ok_or_else(|| invalid("managed contribution identity is not mounted"))?;
        if &mounted.handle != handle {
            return Err(invalid(
                "managed execution handle is forged or belongs to an old generation",
            ));
        }
        Ok(mounted)
    }
}

fn invalid(message: &str) -> PluginFrameworkError {
    PluginFrameworkError::invalid_provider_package(message)
}
