use std::sync::Arc;

use axum::{
    extract::{Request, State},
    http::{header::RETRY_AFTER, Method, StatusCode},
    middleware::Next,
    response::{IntoResponse, Response},
    Json,
};
use control_plane::system_recovery::{SystemMaintenancePhase, SystemWriteOwner};

use access_control::ConsoleRouteBinding;

use crate::{app_state::ApiState, error_response::ErrorBody};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum SystemMaintenanceRequestClass {
    CoordinatorControl,
}

#[derive(Clone, Debug)]
pub(crate) struct SystemMaintenanceRequestClassifier {
    coordinator_control_routes: Arc<[ConsoleRouteBinding]>,
}

impl SystemMaintenanceRequestClassifier {
    pub(crate) fn new(coordinator_control_routes: Vec<ConsoleRouteBinding>) -> Self {
        Self {
            coordinator_control_routes: coordinator_control_routes.into(),
        }
    }

    fn classify(&self, method: &Method, path: &str) -> Option<SystemMaintenanceRequestClass> {
        self.coordinator_control_routes
            .iter()
            .any(|route| {
                route.method.eq_ignore_ascii_case(method.as_str())
                    && route_template_matches(&route.path, path)
            })
            .then_some(SystemMaintenanceRequestClass::CoordinatorControl)
    }
}

pub(crate) async fn classify_system_maintenance_request(
    State(classifier): State<SystemMaintenanceRequestClassifier>,
    mut request: Request,
    next: Next,
) -> Response {
    if let Some(request_class) = classifier.classify(request.method(), request.uri().path()) {
        request.extensions_mut().insert(request_class);
    }
    next.run(request).await
}

pub async fn fence_mutating_requests(
    State(state): State<Arc<ApiState>>,
    request: Request,
    next: Next,
) -> Response {
    if is_read_only_method(request.method()) {
        return next.run(request).await;
    }

    if request.extensions().get::<SystemMaintenanceRequestClass>()
        == Some(&SystemMaintenanceRequestClass::CoordinatorControl)
    {
        if state.system_maintenance.snapshot().phase != SystemMaintenancePhase::Online {
            return maintenance_unavailable();
        }
        return next.run(request).await;
    }

    let permit = match state
        .system_maintenance
        .try_enter_write(SystemWriteOwner::ApiMutation)
    {
        Ok(permit) => permit,
        Err(_) => return maintenance_unavailable(),
    };

    let response = next.run(request).await;
    drop(permit);
    response
}

fn maintenance_unavailable() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(RETRY_AFTER, "5")],
        Json(ErrorBody {
            status: StatusCode::SERVICE_UNAVAILABLE.as_u16(),
            code: "system_maintenance".to_owned(),
            message: "system writes are temporarily fenced for recovery".to_owned(),
        }),
    )
        .into_response()
}

fn route_template_matches(template: &str, path: &str) -> bool {
    let template = template.trim_matches('/').split('/').collect::<Vec<_>>();
    let path = path.trim_matches('/').split('/').collect::<Vec<_>>();
    template.len() == path.len()
        && template.iter().zip(path).all(|(template, actual)| {
            template == &actual
                || ((template.starts_with(':')
                    || (template.starts_with('{') && template.ends_with('}')))
                    && !actual.is_empty())
        })
}

fn is_read_only_method(method: &Method) -> bool {
    matches!(
        *method,
        Method::GET | Method::HEAD | Method::OPTIONS | Method::TRACE
    )
}

#[cfg(test)]
mod tests {
    use super::{
        is_read_only_method, SystemMaintenanceRequestClass, SystemMaintenanceRequestClassifier,
    };
    use access_control::ConsoleRouteBinding;
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

    #[test]
    fn only_declared_route_semantics_classify_as_coordinator_control() {
        let classifier = SystemMaintenanceRequestClassifier::new(vec![ConsoleRouteBinding {
            method: "POST".to_owned(),
            path: "/api/console/settings/system-backups/:backup_set_id/recovery/preflight"
                .to_owned(),
        }]);
        assert_eq!(
            classifier.classify(
                &Method::POST,
                "/api/console/settings/system-backups/01989dd9/recovery/preflight"
            ),
            Some(SystemMaintenanceRequestClass::CoordinatorControl)
        );
        for (method, path) in [
            (
                Method::GET,
                "/api/console/settings/system-backups/01989dd9/recovery/preflight",
            ),
            (Method::POST, "/api/console/settings/system-backups/import"),
            (
                Method::POST,
                "/api/console/settings/system-backups/01989dd9/recovery/unknown",
            ),
        ] {
            assert_eq!(classifier.classify(&method, path), None);
        }
    }
}
