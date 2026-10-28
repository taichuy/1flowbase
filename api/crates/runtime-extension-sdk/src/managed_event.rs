//! One finite Event exchange over the worker's stdin/stdout. The SDK never accepts caller identity.
use std::io::{Read, Write};

use extension_contracts::{
    ManagedEventHostFrame, ManagedEventOutcome, ManagedEventWorkerFrame,
    MANAGED_EVENT_MAX_FRAME_BYTES, MANAGED_EVENT_PROTOCOL_V2,
};

use crate::RuntimeExtensionSdkError;

/// Reads exactly one bounded host request and emits a correlated, contract-checked response.
/// Authors acknowledge, publish, or request the finite owned-collection effect. The host checks
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
        protocol: MANAGED_EVENT_PROTOCOL_V2.into(),
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

/// A finite typed command worker. Destination/actor authority stays in the Host request receipt.
pub fn serve_communication_command<R: Read, W: Write>(
    reader: R,
    writer: W,
    command: impl FnOnce(
        &extension_contracts::CommunicationCommandRequest,
    ) -> extension_contracts::CommunicationCommandReply,
) -> Result<(), RuntimeExtensionSdkError> {
    serve_managed_event(reader, writer, |frame| {
        let parsed = serde_json::from_value::<extension_contracts::CommunicationCommandRequest>(
            frame.delivery.payload.clone(),
        );
        match parsed {
            Ok(request)
                if frame.delivery.point_id == extension_contracts::COMMUNICATION_COMMAND_POINT
                    && request.validate().is_ok() =>
            {
                ManagedEventOutcome::CommandReply {
                    reply: command(&request),
                }
            }
            _ => ManagedEventOutcome::Failed {
                classification: "invalid_command_request".into(),
            },
        }
    })
}
