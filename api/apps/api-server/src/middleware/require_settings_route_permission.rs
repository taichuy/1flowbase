use std::sync::Arc;

use access_control::{settings_route_permissions_for_console_request, AccessRule};
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use control_plane::errors::ControlPlaneError;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
};

pub async fn require_settings_route_permission(
    State(state): State<Arc<ApiState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let method = request.method().as_str().to_string();
    let path = request.uri().path().to_string();

    if path.starts_with("/api/console/settings/") {
        let feature_id = match state.settings_feature_registry.access_rule(&method, &path) {
            Some(AccessRule::SettingsFeature(feature_id)) => feature_id,
            _ => {
                return Err(ControlPlaneError::PermissionDenied(
                    "settings_feature_route_unregistered",
                )
                .into())
            }
        };
        let context = require_session(&state, request.headers()).await?;
        let permission_code = format!("settings_feature.access.{feature_id}");
        if context.actor.has_permission(&permission_code) {
            return Ok(next.run(request).await);
        }

        return Err(
            ControlPlaneError::PermissionDenied("settings_feature_permission_denied").into(),
        );
    }

    let required_permissions = settings_route_permissions_for_console_request(&method, &path);

    if required_permissions.is_empty() {
        return Ok(next.run(request).await);
    }

    let context = require_session(&state, request.headers()).await?;
    if required_permissions
        .iter()
        .any(|permission_code| context.actor.has_permission(permission_code))
    {
        return Ok(next.run(request).await);
    }

    Err(ControlPlaneError::PermissionDenied("settings_route_permission_denied").into())
}
