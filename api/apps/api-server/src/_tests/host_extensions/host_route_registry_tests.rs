use api_server::host_route_registry::{HostRouteDefinition, HostRouteRegistry};

#[test]
fn registry_rejects_duplicate_route_id() {
    let mut registry = HostRouteRegistry::default();
    registry
        .register(test_route("file-security.scan-report"))
        .unwrap();

    let err = registry
        .register(test_route("file-security.scan-report"))
        .unwrap_err();

    assert!(err.to_string().contains("duplicate route"));
}

#[test]
fn registry_rejects_duplicate_method_and_path() {
    let mut registry = HostRouteRegistry::default();
    registry
        .register(test_route("file-security.scan-report"))
        .unwrap();

    let err = registry
        .register(HostRouteDefinition {
            route_id: "file-security.scan-report-v2".into(),
            ..test_route("file-security.scan-report")
        })
        .unwrap_err();

    assert!(err.to_string().contains("duplicate route path"));
}

#[test]
fn registry_rejects_uncontrolled_path() {
    let mut registry = HostRouteRegistry::default();

    let err = registry.register(test_route_with_path("/raw")).unwrap_err();

    assert!(err.to_string().contains("controlled route"));
}

#[test]
fn registry_accepts_manifest_validated_public_auth_path() {
    let mut registry = HostRouteRegistry::default();

    registry
        .register(test_route_with_path(
            "/api/public/auth/file-security/qr/start",
        ))
        .unwrap();
}

#[test]
fn registry_rejects_empty_action_target() {
    let mut registry = HostRouteRegistry::default();

    let err = registry
        .register(HostRouteDefinition {
            resource_code: String::new(),
            ..test_route("file-security.scan-report")
        })
        .unwrap_err();

    assert!(err.to_string().contains("resource_code"));
}

fn test_route(route_id: &str) -> HostRouteDefinition {
    HostRouteDefinition {
        extension_id: "file-security".into(),
        route_id: route_id.into(),
        method: "GET".into(),
        path: "/api/system/file-security/files/{file_id}/scan-report".into(),
        resource_code: "file_scan_reports".into(),
        action_code: "get".into(),
    }
}

fn test_route_with_path(path: &str) -> HostRouteDefinition {
    HostRouteDefinition {
        path: path.into(),
        ..test_route("file-security.scan-report")
    }
}
