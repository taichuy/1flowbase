//! Managed operations share the host carrier and its callback/cancellation ownership.
use super::{invalid, LoadedManagedBinding};
use crate::{
    capability_stdio::{CapabilityStdioRequest, CapabilityStdioResponse},
    plugin_scope::PluginScopeLease,
    stdio_runtime::{
        MultiplexProviderWorker, ProviderHostCallContext, ProviderWorkerProcessControl,
        StreamingCallContext,
    },
};
use extension_package_runtime::FrameworkResult;
use serde_json::Value;
use std::sync::Arc;

struct ProcessOwner {
    control: ProviderWorkerProcessControl,
    lease: Arc<PluginScopeLease>,
    finished: bool,
}
impl Drop for ProcessOwner {
    fn drop(&mut self) {
        if !self.finished {
            let control = self.control.clone();
            let lease = self.lease.clone();
            // Keep the generation admitted until its worker has actually been reaped.
            tokio::spawn(async move {
                control.terminate().await;
                drop(lease);
            });
        }
    }
}

pub(super) async fn exchange(
    binding: &LoadedManagedBinding,
    request: &CapabilityStdioRequest,
    lease: Arc<PluginScopeLease>,
    host_calls: Option<ProviderHostCallContext>,
) -> FrameworkResult<Value> {
    let worker = MultiplexProviderWorker::activate(
        binding.runtime_executable.clone(),
        binding.limits.clone(),
        1,
    )?;
    let mut owner = ProcessOwner {
        control: worker.process_control(),
        lease: lease.clone(),
        finished: false,
    };
    let context = StreamingCallContext {
        required_live_events: None,
        diagnostic_live_events: None,
        protocol_observation: None,
        event_observer: None,
        host_calls,
    };
    let result = worker
        .call_json(
            serde_json::to_value(request)
                .map_err(|_| invalid("managed operation request encoding failed"))?,
            &binding.limits,
            Some(context),
            Some(Box::new(lease)),
        )
        .await;
    owner.control.terminate().await;
    owner.finished = true;
    let (response, _) = result?;
    let response: CapabilityStdioResponse = serde_json::from_value(response)
        .map_err(|_| invalid("managed operation response is malformed"))?;
    if !response.ok {
        // Only protocol classifications may cross the boundary, never arbitrary diagnostics.
        let code = response
            .error
            .as_ref()
            .map(|error| error.message.as_str())
            .filter(|value| {
                !value.is_empty()
                    && value.len() <= 128
                    && value.bytes().all(|byte| {
                        byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'_'
                    })
            })
            .unwrap_or("managed_operation_failed");
        return Err(invalid(code));
    }
    Ok(response.result)
}
