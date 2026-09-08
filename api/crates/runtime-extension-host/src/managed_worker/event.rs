use super::{event_stdio, invalid, ManagedWorkers};
use extension_contracts::{ManagedEventHostFrame, ManagedEventOutcome, MANAGED_EVENT_PROTOCOL_V1};
use extension_package_runtime::{FrameworkResult, PluginExecutionMode};
use runtime_core::runtime_backend::RuntimeManagedEventRequest;
use std::sync::atomic::{AtomicU64, Ordering};
static NEXT_EVENT_CALL: AtomicU64 = AtomicU64::new(1);

impl ManagedWorkers {
    pub(crate) fn admit_event(
        &self,
        request: RuntimeManagedEventRequest,
    ) -> Result<
        impl std::future::Future<Output = FrameworkResult<ManagedEventOutcome>> + Send + 'static,
        runtime_core::runtime_backend::RuntimeBackendError,
    > {
        let mounted = self.exact_mount(&request.handle)?;
        let binding = &mounted.binding;
        request
            .delivery
            .validate()
            .map_err(|e| invalid(&e.to_string()))?;
        if request.delivery.workspace_id != request.handle.identity().workspace_id().as_str()
            || binding.execution_mode != PluginExecutionMode::ProcessPerCall
            || binding.contribution.point_id.as_str()
                != request
                    .delivery
                    .point_id()
                    .map_err(|e| invalid(&e.to_string()))?
            || binding.contribution.contract_version.as_str()
                != request
                    .delivery
                    .point_contract_version()
                    .map_err(|e| invalid(&e.to_string()))?
            || !binding
                .contribution
                .required_permissions
                .iter()
                .any(|p| p.as_str() == "event.subscribe")
        {
            return Err(
                invalid("managed event binding, workspace or subscription mismatch").into(),
            );
        }
        let now_ms = time::OffsetDateTime::now_utc().unix_timestamp_nanos() / 1_000_000;
        let remaining = u64::try_from(i128::from(request.deadline_unix_ms) - now_ms)
            .ok()
            .filter(|v| *v > 0)
            .ok_or_else(|| invalid("managed event deadline has expired"))?;
        let budget = remaining
            .min(binding.limits.timeout_ms.unwrap_or(10_000))
            .min(10_000);
        let deadline_unix_ms = i64::try_from(now_ms + i128::from(budget))
            .map_err(|_| invalid("managed event deadline overflow"))?;
        let sequence = NEXT_EVENT_CALL
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |v| v.checked_add(1))
            .map_err(|_| invalid("managed event call sequence exhausted"))?;
        let frame = ManagedEventHostFrame {
            protocol: MANAGED_EVENT_PROTOCOL_V1.into(),
            call_id: format!("event-{sequence}"),
            handler: binding.handler.clone(),
            execution_identity: request.handle.identity().clone(),
            generation: request.handle.generation(),
            graph_fingerprint: request.graph_fingerprint,
            authority_revision: request.authority_revision,
            deadline_unix_ms,
            delivery: request.delivery,
        };
        frame.validate().map_err(|e| invalid(&e.to_string()))?;
        let payload =
            serde_json::to_vec(&frame).map_err(|_| invalid("managed event cannot be encoded"))?;
        if payload.len() > extension_contracts::MANAGED_EVENT_MAX_FRAME_BYTES {
            return Err(invalid("managed event request exceeds frame limit").into());
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
                .filter(|v| *v > 0)
                .ok_or_else(|| invalid("managed event deadline has expired"))?;
            let result = tokio::time::timeout(
                std::time::Duration::from_millis(remaining),
                event_stdio::exchange(&binding, &frame, payload, lease),
            )
            .await
            .map_err(|_| invalid("managed event deadline elapsed"))??;
            if matches!(&result, ManagedEventOutcome::Publish { .. })
                && !binding
                    .contribution
                    .required_permissions
                    .iter()
                    .any(|p| p.as_str() == "event.publish")
            {
                return Err(invalid(
                    "managed event contribution has no declared publish permission",
                ));
            }
            if matches!(&result, ManagedEventOutcome::ApplyProcessed { .. })
                && !binding
                    .contribution
                    .required_permissions
                    .iter()
                    .any(|p| p.as_str() == "plugin_data.owned.write")
            {
                return Err(invalid(
                    "managed event contribution has no declared owned write permission",
                ));
            }
            Ok(result)
        })
    }
}
