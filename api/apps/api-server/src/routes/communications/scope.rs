use crate::error_response::ApiError;
pub(crate) use api_host_kernel::communication_scope::{current_scope, has_scope, with_scope};
use domain::CommunicationHistoryScope;
use uuid::Uuid;
pub(crate) fn require_management_scope() -> Result<(), ApiError> {
    if has_scope() {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "communication_management_scope_denied",
        )
        .into());
    }
    Ok(())
}
pub(crate) fn for_channel(
    actor: &domain::ActorContext,
    instance_id: Uuid,
) -> Result<Option<CommunicationHistoryScope>, ApiError> {
    let scope = current_scope();
    if scope.as_ref().is_some_and(|s| {
        s.workspace_id != actor.current_workspace_id || s.instance_id != instance_id
    }) {
        return Err(control_plane::errors::ControlPlaneError::PermissionDenied(
            "communication_scope_mismatch",
        )
        .into());
    }
    Ok(scope)
}

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/scope.rs"]
mod tests;
