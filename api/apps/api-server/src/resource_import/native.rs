//! Native terminal import uses the real user operation and upload ACL, never a plugin grant.
use super::*;
use control_plane::file_management::{FileUploadService, FileUploadTarget, UploadFileCommand};
use extension_contracts::FileImportResponse;

const IMPORT_INTERFACE: &str = "terminal.files.import_resource";

/// Invoke inside the canonical handler before spawning work so selected MCP scope is captured.
/// No task is spawned here; the generation and captured caller survive both authorization checks.
pub(crate) async fn import_for_principal(
    state: Arc<ApiState>,
    principal: &interface_runtime::UserPrincipal,
    resource_uri: &str,
    file_table_id: uuid::Uuid,
) -> Result<FileImportResponse, FileImportError> {
    let caller = capture_caller(&state, principal, IMPORT_INTERFACE)
        .await
        .map_err(|_| FileImportError::new("file_import_caller_invalid"))?
        .ok_or_else(|| FileImportError::new("file_import_reentrant_source"))?;
    FileImportRequest {
        resource_uri: resource_uri.to_owned(),
        file_table_id: file_table_id.to_string(),
    }
    .validate()?;
    let owner = crate::generation_dispatch::GenerationDispatch::acquire(&state)
        .map_err(|_| FileImportError::new("file_import_host_unavailable"))?;
    let generation = crate::generation_dispatch::GenerationLease { state, owner };
    let (actor, user) = current_native_caller(&generation, &caller).await?;
    let source = crate::routes::mcp_protocol::resources::replay_for_actor(
        &generation,
        &actor,
        user,
        resource_uri,
        caller.selected_mcp_instance_ids.as_deref(),
        None,
    )
    .await?;
    // A slow source read cannot carry a revoked session, role or operation into target effects.
    let (actor, _) = current_native_caller(&generation, &caller).await?;
    let uploaded = FileUploadService::new(
        generation.store.clone(),
        generation.file_storage_registry.clone(),
        generation.runtime_engine.clone(),
    )
    .upload(UploadFileCommand {
        actor,
        target: FileUploadTarget::Table(file_table_id),
        original_filename: source.filename,
        content_type: source.mime_type,
        bytes: source.bytes,
    })
    .await
    .map_err(|error| {
        if error.is::<runtime_core::runtime_acl::RuntimeAclError>()
            || matches!(
                error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
                Some(control_plane::errors::ControlPlaneError::PermissionDenied(
                    _
                ))
            )
        {
            FileImportError::new("file_import_target_permission_denied")
        } else {
            FileImportError::new("file_import_upload_failed")
        }
    })?;
    Ok(FileImportResponse {
        file_table_id: uploaded.file_table_id.to_string(),
        storage_id: uploaded.storage_id.to_string(),
        record: uploaded.record,
    })
}

async fn current_native_caller(
    state: &ApiState,
    caller: &RuntimeCallerContext,
) -> Result<(domain::ActorContext, domain::UserRecord), FileImportError> {
    let denied = || FileImportError::new("file_import_caller_invalid");
    if caller.interface_id != IMPORT_INTERFACE {
        return Err(denied());
    }
    let (actor, user) = current_actor(state, caller).await?;
    let operation = state
        .console_operation_registry
        .inventory()
        .operations
        .iter()
        .find(|operation| operation.operation_id == IMPORT_INTERFACE)
        .ok_or_else(denied)?;
    // Fail closed if the operation is absent or has been reclassified by a generation.
    if operation.lifecycle != access_control::SettingsFeatureLifecycle::Active
        || !matches!(&operation.policy_group, access_control::ConsolePolicyGroup::Other(group) if group == "other.local-commands")
        || !matches!(
            operation.authorization,
            access_control::ConsoleAuthorization::Simple
        )
    {
        return Err(denied());
    }
    let policies = state
        .console_policy_reader
        .load_role_console_policies_for_user(&actor)
        .await
        .map_err(|_| denied())?;
    let group = domain::ConsolePolicyGroup::other("other.local-commands").map_err(|_| denied())?;
    let operation_id =
        domain::ConsoleOperationId::try_from(IMPORT_INTERFACE).map_err(|_| denied())?;
    if !actor.is_root
        && !domain::effective_console_simple_operation(&policies, &group, &operation_id)
    {
        return Err(denied());
    }
    current_selected_operation(
        state,
        &actor,
        IMPORT_INTERFACE,
        &caller.selected_mcp_instance_ids,
    )
    .await?;
    Ok((actor, user))
}

#[cfg_attr(test, api_test_selection::group(core))]
#[cfg(test)]
#[path = "_tests/native.rs"]
mod tests;
