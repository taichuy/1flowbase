//! One finite Event exchange over the worker's stdin/stdout. The SDK never accepts caller identity.
use std::io::{Read, Write};

use extension_contracts::{
    ManagedEventHostFrame, ManagedEventOutcome, ManagedEventWorkerFrame,
    MANAGED_EVENT_MAX_FRAME_BYTES, MANAGED_EVENT_PROTOCOL_V1,
};

use crate::RuntimeExtensionSdkError;

/// Reads exactly one bounded host request and emits a correlated, contract-checked response.
/// Authors acknowledge, publish, or request the finite processed-model effect. The host checks
/// current contribution authority and commits effects before acknowledging delivery.
pub fn serve_managed_event<R: Read, W: Write>(
    reader: R,
    mut writer: W,
    event: impl FnOnce(&ManagedEventHostFrame) -> ManagedEventOutcome,
) -> Result<(), RuntimeExtensionSdkError> {
    let mut bytes = Vec::new();
    reader
        .take((MANAGED_EVENT_MAX_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MANAGED_EVENT_MAX_FRAME_BYTES {
        return Err(RuntimeExtensionSdkError::InvalidRequest(
            "managed event frame exceeds limit".into(),
        ));
    }
    let request: ManagedEventHostFrame = serde_json::from_slice(&bytes)?;
    request
        .validate()
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    let response = ManagedEventWorkerFrame {
        protocol: MANAGED_EVENT_PROTOCOL_V1.into(),
        call_id: request.call_id.clone(),
        result: event(&request),
    };
    response
        .validate_for(&request)
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    serde_json::to_writer(&mut writer, &response)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}
