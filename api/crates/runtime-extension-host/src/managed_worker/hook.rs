use std::sync::atomic::{AtomicU64, Ordering};

use extension_contracts::{
    ManagedHookHostContext, ManagedHookHostFrame, ManagedHookOutcome, MANAGED_HOOK_PROTOCOL_V1,
};
use extension_package_runtime::{FrameworkResult, PluginExecutionMode};
use runtime_core::runtime_backend::{RuntimeManagedHookInput, RuntimeManagedHookRequest};

use super::{hook_stdio, invalid, ManagedWorkers};

static NEXT_HOOK_CALL: AtomicU64 = AtomicU64::new(1);

impl ManagedWorkers {
    pub(crate) fn admit_hook(
        &self,
        request: RuntimeManagedHookRequest,
    ) -> Result<
        impl std::future::Future<Output = FrameworkResult<ManagedHookOutcome>> + Send + 'static,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        let mounted = self.exact_mount(&request.handle)?;
        let binding = &mounted.binding;
        if request.principal.workspace_id != request.handle.identity().workspace_id().as_str() {
            return Err(invalid("managed hook workspace mismatch").into());
        }
        let (point_id, phase) = match &request.input {
            RuntimeManagedHookInput::LegacyCreate(input) => {
                (input.point_id().to_owned(), input.phase())
            }
            RuntimeManagedHookInput::Interface {
                interface_id,
                input,
                ..
            } => (
                extension_contracts::managed_interface_hook_point_id(interface_id, input.phase()),
                input.phase(),
            ),
        };
        if binding.execution_mode != PluginExecutionMode::ProcessPerCall
            || binding.contribution.point_id.as_str() != point_id
            || binding.contribution.contract_version.as_str() != "1"
        {
            return Err(
                invalid("managed hook phase is not declared by the bound contribution").into(),
            );
        }
        let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        let remaining = u64::try_from(i128::from(request.principal.deadline_unix_ms) - now_ms)
            .ok()
            .filter(|value| *value > 0)
            .ok_or_else(|| invalid("managed hook deadline has expired"))?;
        // Kernel owns the shared observer budget; this additional cap bounds a standalone port call.
        let phase_limit = if matches!(
            phase,
            extension_contracts::extension_bus::HookPhase::After
                | extension_contracts::extension_bus::HookPhase::Failure
                | extension_contracts::extension_bus::HookPhase::Completion
        ) {
            1_000
        } else {
            30_000
        };
        let budget_ms = remaining
            .min(binding.limits.timeout_ms.unwrap_or(30_000))
            .min(phase_limit);
        let deadline_unix_ms = i64::try_from(now_ms + i128::from(budget_ms))
            .map_err(|_| invalid("managed hook deadline overflow"))?;
        let sequence = NEXT_HOOK_CALL
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |value| {
                value.checked_add(1)
            })
            .map_err(|_| invalid("managed hook call sequence exhausted"))?;
        let context = ManagedHookHostContext {
            invocation: request.invocation,
            execution_identity: request.handle.identity().clone(),
            generation: request.handle.generation(),
            deadline_unix_ms,
            actor_id: request.principal.actor_id,
        };
        let frame = match request.input {
            RuntimeManagedHookInput::LegacyCreate(input) => {
                hook_stdio::HookFrame::LegacyCreate(ManagedHookHostFrame {
                    protocol: MANAGED_HOOK_PROTOCOL_V1.into(),
                    call_id: format!("hook-{sequence}"),
                    handler: binding.handler.clone(),
                    context,
                    input,
                })
            }
            RuntimeManagedHookInput::Interface {
                interface_id,
                interface_version,
                input,
            } => hook_stdio::HookFrame::Interface(extension_contracts::ManagedInterfaceHostFrame {
                protocol: extension_contracts::MANAGED_INTERFACE_PROTOCOL_V1.into(),
                call_id: format!("hook-{sequence}"),
                handler: binding.handler.clone(),
                interface_id,
                interface_version,
                context,
                input,
            }),
        };
        let payload = frame.encode()?;
        let lease = mounted
            .scope
            .admit_generation(request.handle.generation().get())?;
        let binding = binding.clone();
        Ok(async move {
            let lease = std::sync::Arc::new(lease);
            let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
            let remaining = u64::try_from(i128::from(deadline_unix_ms) - now_ms)
                .ok()
                .filter(|value| *value > 0)
                .ok_or_else(|| invalid("managed hook deadline has expired"))?;
            // Includes spawn, stdin, stdout, child exit and decode. Drop also kills the child.
            tokio::time::timeout(
                std::time::Duration::from_millis(remaining),
                hook_stdio::exchange(&binding, &frame, payload, lease),
            )
            .await
            .map_err(|_| invalid("managed hook deadline elapsed"))?
        })
    }
}
