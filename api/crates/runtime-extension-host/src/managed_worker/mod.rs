mod binding;
mod capability;
mod capability_multiplex;
mod event;
mod event_stdio;
mod hook;
mod hook_stdio;
mod process;
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
        self.mounted.insert(
            identity,
            MountedContribution {
                handle: handle.clone(),
                binding,
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
        plugin_data: Arc<dyn extension_contracts::PluginDataPort>,
        plugin_credentials: Option<Arc<dyn extension_contracts::PluginCredentialPort>>,
    ) -> Result<
        impl std::future::Future<Output = FrameworkResult<Value>> + Send + 'static,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
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
            plugin_data,
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
        let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        let remaining_ms = i128::from(request.principal.deadline_unix_ms) - now_ms;
        let remaining_ms = u64::try_from(remaining_ms)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid("managed execution deadline has expired"))?;
        let lease = mounted
            .scope
            .admit_generation(request.handle.generation().get())?;
        let mut binding = mounted.binding.clone();
        binding.limits.timeout_ms = Some(
            binding
                .limits
                .timeout_ms
                .unwrap_or(30_000)
                .min(remaining_ms),
        );
        Ok(async move {
            let lease = Arc::new(lease);
            let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            let remaining_ms =
                u64::try_from(i128::from(request.principal.deadline_unix_ms) - now_ms)
                    .ok()
                    .filter(|value| *value > 0)
                    .ok_or_else(|| invalid("managed execution deadline has expired"))?;
            let request = CapabilityStdioRequest {
                method: CapabilityStdioMethod::Execute,
                input: json!({
                    "plugin_id": binding.plugin_id,
                    "contribution_code": binding.handler,
                    "handler": binding.handler,
                    "config_payload": request.config_payload,
                    "input_payload": request.input_payload,
                }),
            };
            // The request deadline also bounds process start and stdin writes, not only output.
            tokio::time::timeout(
                std::time::Duration::from_millis(
                    remaining_ms.min(binding.limits.timeout_ms.unwrap_or(30_000)),
                ),
                async {
                    if binding.protocol == extension_contracts::STDIO_JSON_MULTIPLEX_V1 {
                        capability_multiplex::exchange(&binding, &request, lease, host_calls).await
                    } else {
                        capability::exchange(&binding, &request, lease).await
                    }
                },
            )
            .await
            .map_err(|_| invalid("managed execution deadline elapsed"))?
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
