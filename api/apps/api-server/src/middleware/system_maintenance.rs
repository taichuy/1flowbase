use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header::RETRY_AFTER, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use control_plane::system_recovery::SystemWriteOwner;

use crate::{app_state::ApiState, error_response::ErrorBody};

pub async fn fence_mutating_requests(
    State(state): State<Arc<ApiState>>,
    request: Request,
    next: Next,
) -> Response {
    if is_read_only_method(request.method()) {
        return next.run(request).await;
    }

    let permit = match state
        .system_maintenance
        .try_enter_write(SystemWriteOwner::ApiMutation)
    {
        Ok(permit) => permit,
        Err(_) => {
            return (
                StatusCode::SERVICE_UNAVAILABLE,
                [(RETRY_AFTER, "5")],
                Json(ErrorBody {
                    status: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
                    code: "system_maintenance".to_owned(),
                    message: "system writes are temporarily fenced for recovery".to_owned(),
                }),
            )
                .into_response();
        }
    };

    let response = next.run(request).await;
    drop(permit);
    response
}

fn is_read_only_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

#[cfg(test)]
mod tests {
    use super::is_read_only_method;
    use axum::http::Method;

    #[test]
    fn only_protocol_safe_methods_bypass_the_write_fence() {
        for method in [Method::GET, Method::HEAD, Method::OPTIONS, Method::TRACE] {
            assert!(is_read_only_method(&method));
        }
        for method in [
            Method::POST,
            Method::PUT,
            Method::PATCH,
            Method::DELETE,
            Method::CONNECT,
        ] {
            assert!(!is_read_only_method(&method));
        }
    }
}
