//! Authority supplied by the communication execution owner, never by model arguments.
use crate::error_response::ApiError;
use domain::CommunicationHistoryScope;
use std::future::Future;
use uuid::Uuid;
tokio::task_local! { static COMMUNICATION_SCOPE: CommunicationHistoryScope; }
pub(crate) async fn with_scope<T>(
    scope: CommunicationHistoryScope,
    future: impl Future<Output = T>,
) -> T {
    COMMUNICATION_SCOPE.scope(scope, future).await
}
pub(crate) fn for_channel(
    actor: &domain::ActorContext,
    instance_id: Uuid,
) -> Result<Option<CommunicationHistoryScope>, ApiError> {
    let scope = COMMUNICATION_SCOPE.try_with(Clone::clone).ok();
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

#[cfg(test)]
#[path = "_tests/scope.rs"]
mod tests;
