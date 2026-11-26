use super::host::ManagedSource;
use crate::{app_state::ApiState, error_response::ApiError};
use control_plane::errors::ControlPlaneError as Error;
use interface_runtime::UserPrincipal;
use serde_json::Value;
use std::sync::Arc;

/// Rehydrate a cookie principal from its sealed session on every use. A cached
/// ActorContext cannot retain privileges after a role, workspace or session edit.
pub(super) async fn live_principal(
    state: &ApiState,
    principal: &UserPrincipal,
) -> Result<UserPrincipal, ApiError> {
    let Some(session) = principal.authenticated_session() else {
        // Unary API-key invocations are authenticated by their canonical ingress;
        // they may read snapshots, but are never promoted to browser producers.
        return Ok(principal.clone());
    };
    let cookie = format!(
        "{}={}",
        state.cookie_name,
        session.expose_to_trusted_handler()
    );
    let mut headers = axum::http::HeaderMap::new();
    headers.insert(
        axum::http::header::COOKIE,
        cookie.parse().map_err(|_| Error::NotAuthenticated)?,
    );
    let context = crate::middleware::require_session::require_session(state, &headers).await?;
    if context.actor.user_id != principal.actor().user_id
        || context.actor.current_workspace_id != principal.actor().current_workspace_id
    {
        return Err(Error::PermissionDenied("managed_service_session").into());
    }
    Ok(context.interface_principal())
}
pub(super) async fn authorize(
    source: &ManagedSource,
    principal: &UserPrincipal,
    payload: &Value,
) -> Result<UserPrincipal, ApiError> {
    let principal = live_principal(&source.state, principal).await?;
    authorize_current(source, &principal, payload).await?;
    Ok(principal)
}

async fn authorize_current(
    source: &ManagedSource,
    principal: &UserPrincipal,
    payload: &Value,
) -> Result<(), ApiError> {
    if !super::super::record_acl::operation_allowed(
        &source.state,
        principal.actor(),
        &source.operation,
    )
    .await?
    {
        return Err(Error::PermissionDenied("console_operation_permission_denied").into());
    }
    super::super::record_acl::authorize(
        &source.state,
        &source.registration,
        principal.actor(),
        &source.operation,
        payload,
    )
    .await?;
    Ok(())
}

/// Rechecks at the socket projection boundary as well as at broker fanout, so a
/// buffered sample cannot retain authority after a role or row-policy change.
pub(in crate::managed_services) async fn authorize_frame(
    state: Arc<ApiState>,
    headers: &axum::http::HeaderMap,
    session: &str,
    installation: uuid::Uuid,
    operation: &plugin_framework::ManagedServiceOperation,
    payload: &Value,
    event: &Value,
) -> Result<bool, ApiError> {
    let Some(specification) = operation
        .stream
        .as_ref()
        .filter(|s| s.shared_source.is_some() || s.aggregate_source_interface_id.is_some())
    else {
        return Ok(true);
    };
    let context = crate::middleware::require_session::require_session(&state, headers).await?;
    if context.cookie_session()?.session_id != session {
        return Err(Error::NotAuthenticated.into());
    }
    if !super::super::record_acl::operation_allowed(&state, &context.actor, operation).await? {
        return Err(Error::PermissionDenied("console_operation_permission_denied").into());
    }
    let registration = state
        .console_surface_registry
        .managed_services()
        .iter()
        .find(|r| r.installation_id == installation)
        .cloned()
        .ok_or(Error::Conflict("managed_service_inactive"))?;
    let source = super::host::operation(
        &registration,
        specification
            .aggregate_source_interface_id
            .as_deref()
            .unwrap_or(&operation.interface_id),
    )?;
    let source_payload = if specification.aggregate_source_interface_id.is_some() {
        if event.get("type").and_then(Value::as_str) != Some("source") {
            return Ok(true);
        }
        let record = event
            .get("record_id")
            .and_then(Value::as_str)
            .ok_or(Error::InvalidInput("record_id"))?;
        let mut payload = serde_json::json!({"path":{},"query":{},"body":{}});
        super::host::set_pointer(
            &mut payload,
            super::host::record_pointer(&source)?,
            serde_json::json!(record),
        )?;
        payload
    } else {
        payload.clone()
    };
    let source = ManagedSource::new(
        state,
        registration,
        source,
        &context.interface_principal(),
        &source_payload,
    )?;
    Ok(
        authorize_current(&source, &context.interface_principal(), &source_payload)
            .await
            .is_ok(),
    )
}
