//! One finite Hook exchange over the worker's stdin/stdout. The SDK never accepts caller identity.
use std::io::{Read, Write};

use extension_contracts::{
    ManagedHookHostFrame, ManagedHookOutcome, ManagedHookWorkerFrame, MANAGED_HOOK_MAX_FRAME_BYTES,
    MANAGED_HOOK_PROTOCOL_V1,
};

use crate::RuntimeExtensionSdkError;

/// Reads exactly one bounded host request and emits a correlated, phase-checked response.
/// Authors match on the typed Create input and may only veto or observe it.
pub fn serve_managed_hook<R: Read, W: Write>(
    reader: R,
    mut writer: W,
    hook: impl FnOnce(&ManagedHookHostFrame) -> ManagedHookOutcome,
) -> Result<(), RuntimeExtensionSdkError> {
    let mut bytes = Vec::new();
    reader
        .take((MANAGED_HOOK_MAX_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MANAGED_HOOK_MAX_FRAME_BYTES {
        return Err(RuntimeExtensionSdkError::InvalidRequest(
            "managed hook frame exceeds limit".into(),
        ));
    }
    let request: ManagedHookHostFrame = serde_json::from_slice(&bytes)?;
    request
        .validate()
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    let response = ManagedHookWorkerFrame {
        protocol: MANAGED_HOOK_PROTOCOL_V1.into(),
        call_id: request.call_id.clone(),
        phase: request.input.phase(),
        outcome: hook(&request),
    };
    response
        .validate_for(&request)
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    serde_json::to_writer(&mut writer, &response)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Author entry for every canonical interface. The host has already applied its explicit
/// safe-view codec; the SDK still checks the bounded schema and response phase at the wire edge.
pub fn serve_managed_interface_hook<R: Read, W: Write>(
    reader: R,
    mut writer: W,
    hook: impl FnOnce(&extension_contracts::ManagedInterfaceHostFrame) -> ManagedHookOutcome,
) -> Result<(), RuntimeExtensionSdkError> {
    use extension_contracts::{
        ManagedInterfaceHostFrame, ManagedInterfaceWorkerFrame, MANAGED_INTERFACE_MAX_FRAME_BYTES,
        MANAGED_INTERFACE_PROTOCOL_V1,
    };
    let mut bytes = Vec::new();
    reader
        .take((MANAGED_INTERFACE_MAX_FRAME_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MANAGED_INTERFACE_MAX_FRAME_BYTES {
        return Err(RuntimeExtensionSdkError::InvalidRequest(
            "managed interface frame exceeds limit".into(),
        ));
    }
    let request: ManagedInterfaceHostFrame = serde_json::from_slice(&bytes)?;
    request
        .validate()
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    let response = ManagedInterfaceWorkerFrame {
        protocol: MANAGED_INTERFACE_PROTOCOL_V1.into(),
        call_id: request.call_id.clone(),
        phase: request.input.phase(),
        outcome: hook(&request),
    };
    response
        .validate_for(&request)
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    serde_json::to_writer(&mut writer, &response)?;
    writer.write_all(b"\n")?;
    writer.flush()?;
    Ok(())
}

/// Explicit schema-free author entry. Shape, budgets and response correlation are checked locally;
/// only the installed host owns the compiled schema and authority. No schema lookup or cache exists.
pub fn serve_managed_interface_reference_hook<R: Read, W: Write>(
    reader: R,
    mut writer: W,
    hook: impl FnOnce(&extension_contracts::ManagedInterfaceReferenceHostFrame) -> ManagedHookOutcome,
) -> Result<(), RuntimeExtensionSdkError> {
    use extension_contracts::{
        ManagedInterfaceReferenceHostFrame, ManagedInterfaceReferenceWorkerFrame,
        MANAGED_INTERFACE_MAX_REQUEST_BYTES, MANAGED_INTERFACE_PROTOCOL_V2,
    };
    let mut bytes = Vec::new();
    reader
        .take((MANAGED_INTERFACE_MAX_REQUEST_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MANAGED_INTERFACE_MAX_REQUEST_BYTES {
        return Err(RuntimeExtensionSdkError::InvalidRequest(
            "managed interface frame exceeds limit".into(),
        ));
    }
    let request: ManagedInterfaceReferenceHostFrame = serde_json::from_slice(&bytes)?;
    request
        .validate()
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    let response = ManagedInterfaceReferenceWorkerFrame {
        protocol: MANAGED_INTERFACE_PROTOCOL_V2.into(),
        call_id: request.call_id.clone(),
        phase: request.input.phase(),
        outcome: hook(&request),
    };
    response
        .validate_for(&request)
        .map_err(|error| RuntimeExtensionSdkError::InvalidRequest(error.to_string()))?;
    let bytes = serde_json::to_vec(&response)?;
    if bytes.len() > extension_contracts::MANAGED_INTERFACE_MAX_REPLY_BYTES {
        return Err(RuntimeExtensionSdkError::InvalidRequest(
            "managed reference reply exceeds limit".into(),
        ));
    }
    writer.write_all(&bytes)?;
    writer.flush()?;
    Ok(())
}
