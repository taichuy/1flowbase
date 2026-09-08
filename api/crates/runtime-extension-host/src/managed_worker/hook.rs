use std::sync::atomic::{AtomicU64, Ordering};

use extension_contracts::{
    ManagedHookHostContext, ManagedHookHostFrame, ManagedHookOutcome, MANAGED_HOOK_PROTOCOL_V1,
};
use extension_package_runtime::{FrameworkResult, PluginExecutionMode};
use runtime_core::runtime_backend::RuntimeManagedHookRequest;

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
        if binding.execution_mode != PluginExecutionMode::ProcessPerCall
            || binding.contribution.point_id.as_str() != request.input.point_id()
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
        let phase_limit = if request.input.is_observer() {
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
        let frame = ManagedHookHostFrame {
            protocol: MANAGED_HOOK_PROTOCOL_V1.into(),
            call_id: format!("hook-{sequence}"),
            handler: binding.handler.clone(),
            context: ManagedHookHostContext {
                invocation: request.invocation,
                execution_identity: request.handle.identity().clone(),
                generation: request.handle.generation(),
                deadline_unix_ms,
                actor_id: request.principal.actor_id,
            },
            input: request.input,
        };
        frame
            .validate()
            .map_err(|error| invalid(&error.to_string()))?;
        let payload = serde_json::to_vec(&frame)
            .map_err(|_| invalid("managed hook frame cannot be encoded"))?;
        if payload.len() > extension_contracts::MANAGED_HOOK_MAX_FRAME_BYTES {
            return Err(invalid("managed hook request exceeds frame limit").into());
        }
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
