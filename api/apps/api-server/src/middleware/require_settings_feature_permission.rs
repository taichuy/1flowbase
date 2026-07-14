use std::sync::Arc;

use access_control::AccessRule;
use axum::{body::Body, extract::State, http::Request, middleware::Next, response::Response};
use control_plane::errors::ControlPlaneError;

use crate::{
    app_state::ApiState, error_response::ApiError, middleware::require_session::require_session,
};

pub async fn require_settings_feature_permission(
    State(state): State<Arc<ApiState>>,
    request: Request<Body>,
    next: Next,
) -> Result<Response, ApiError> {
    let method = request.method().as_str();
    let path = request.uri().path();

    let Some(AccessRule::SettingsFeature(feature_id)) =
        state.settings_feature_registry.access_rule(method, path)
    else {
        if path.starts_with("/api/console/settings/") {
            return Err(
                ControlPlaneError::PermissionDenied("settings_feature_route_unregistered").into(),
            );
        }
        return Ok(next.run(request).await);
    };

    let context = require_session(&state, request.headers()).await?;
    let permission_code = format!("settings_feature.access.{feature_id}");
    if context.actor.has_permission(&permission_code) {
        return Ok(next.run(request).await);
    }

    Err(ControlPlaneError::PermissionDenied("settings_feature_permission_denied").into())
}
