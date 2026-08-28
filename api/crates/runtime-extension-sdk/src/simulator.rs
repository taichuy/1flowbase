use std::collections::BTreeSet;

use extension_contracts::{
    PluginDataError, PluginDataRequest, PluginDataResponse, RuntimeHostFrame,
    RuntimeHostWorkerFrame, PLUGIN_DATA_SERVICE_V1, RUNTIME_HOST_CALL_PROTOCOL_V1,
};

use crate::RuntimeExtensionSdkError;

pub struct PluginDataHostSimulator<F> {
    handler: F,
    active_call_ids: BTreeSet<String>,
}

impl<F> PluginDataHostSimulator<F>
where
    F: FnMut(&PluginDataRequest) -> Result<PluginDataResponse, PluginDataError>,
{
    pub fn new(handler: F) -> Self {
        Self {
            handler,
            active_call_ids: BTreeSet::new(),
        }
    }

    pub fn accept_worker_line(&mut self, line: &str) -> Result<String, RuntimeExtensionSdkError> {
        let frame: RuntimeHostWorkerFrame = serde_json::from_str(line)?;
        let RuntimeHostWorkerFrame::HostCall {
            protocol,
            call_id,
            service,
            request,
        } = frame
        else {
            return Err(RuntimeExtensionSdkError::UncorrelatedFrame);
        };
        if protocol != RUNTIME_HOST_CALL_PROTOCOL_V1 || service != PLUGIN_DATA_SERVICE_V1 {
            return Err(RuntimeExtensionSdkError::UncorrelatedFrame);
        }
        if !self.active_call_ids.insert(call_id.clone()) {
            return Err(RuntimeExtensionSdkError::UncorrelatedFrame);
        }
        request
            .validate()
            .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.code))?;
        let outcome = (self.handler)(&request);
        self.active_call_ids.remove(&call_id);
        let (result, error) = match outcome {
            Ok(result) => (Some(result), None),
            Err(error) => (None, Some(error)),
        };
        serde_json::to_string(&RuntimeHostFrame::HostResult {
            protocol: RUNTIME_HOST_CALL_PROTOCOL_V1.to_string(),
            call_id,
            result,
            error,
        })
        .map_err(RuntimeExtensionSdkError::from)
    }
}
