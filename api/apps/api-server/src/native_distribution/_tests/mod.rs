use super::{settings::Settings, static_web};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use std::{fs, path::PathBuf};
use tower::ServiceExt;

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("1flowbase-native-{}", uuid::Uuid::now_v7()));
        fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[tokio::test]
async fn serves_spa_and_files_without_hiding_api_errors() {
    let directory = Directory::new();
    fs::write(directory.0.join("index.html"), "<html>1flowbase</html>").unwrap();
    fs::write(directory.0.join("app.js"), "export default 1").unwrap();
    let router = static_web::mount(
        Router::new().route("/api/probe", get(|| async { "api" })),
        &directory.0,
    )
    .unwrap();
    for (path, status, body) in [
        ("/", StatusCode::OK, "<html>1flowbase</html>"),
        (
            "/applications/123",
            StatusCode::OK,
            "<html>1flowbase</html>",
        ),
        ("/app.js", StatusCode::OK, "export default 1"),
        ("/api/probe", StatusCode::OK, "api"),
        ("/api/missing", StatusCode::NOT_FOUND, ""),
        ("/api", StatusCode::NOT_FOUND, ""),
        ("/v1/missing", StatusCode::NOT_FOUND, ""),
        ("/.well-known/missing", StatusCode::NOT_FOUND, ""),
        ("/assets/missing.js", StatusCode::NOT_FOUND, ""),
        ("/external-npm/missing", StatusCode::NOT_FOUND, ""),
        ("/%2e%2e/settings.json", StatusCode::NOT_FOUND, ""),
        ("/C:%5csettings.json", StatusCode::NOT_FOUND, ""),
    ] {
        let response = router
            .clone()
            .oneshot(
                Request::builder()
                    .uri(path)
                    .header("accept", "text/html")
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), status, "{path}");
        assert_eq!(
            to_bytes(response.into_body(), 1024).await.unwrap(),
            body,
            "{path}"
        );
    }
    let response = router
        .clone()
        .oneshot(
            Request::builder()
                .method("HEAD")
                .uri("/app.js")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    assert!(to_bytes(response.into_body(), 1024)
        .await
        .unwrap()
        .is_empty());
    let response = router
        .oneshot(
            Request::builder()
                .uri("/not-an-api")
                .header("accept", "application/json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[cfg(unix)]
#[tokio::test]
async fn symlinks_cannot_expose_secrets_outside_web() {
    let directory = Directory::new();
    let outside = Directory::new();
    fs::write(directory.0.join("index.html"), "index").unwrap();
    fs::write(outside.0.join("secret.txt"), "secret").unwrap();
    std::os::unix::fs::symlink(outside.0.join("secret.txt"), directory.0.join("leak.txt")).unwrap();
    let router = static_web::mount(Router::new(), &directory.0).unwrap();
    let response = router
        .oneshot(
            Request::builder()
                .uri("/leak.txt")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[test]
fn secrets_persist_and_missing_or_corrupt_configuration_is_not_replaced() {
    let directory = Directory::new();
    let first = Settings::load_or_create(&directory.0).unwrap();
    let original = fs::read(directory.0.join("settings.json")).unwrap();
    let second = Settings::load_or_create(&directory.0).unwrap();
    assert_eq!(first.postgres_password, second.postgres_password);
    assert_eq!(
        first.provider_secret_master_key,
        second.provider_secret_master_key
    );
    assert_ne!(first.postgres_password, first.root_password);
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        assert_eq!(
            fs::metadata(directory.0.join("settings.json"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o600
        );
    }
    fs::write(directory.0.join("settings.json"), "broken").unwrap();
    assert!(Settings::load_or_create(&directory.0).is_err());
    assert_eq!(
        fs::read_to_string(directory.0.join("settings.json")).unwrap(),
        "broken"
    );
    fs::remove_file(directory.0.join("settings.json")).unwrap();
    fs::create_dir(directory.0.join("postgres")).unwrap();
    assert!(Settings::load_or_create(&directory.0).is_err());
    assert!(!original.is_empty());
}

#[tokio::test]
async fn preserves_canonical_dispatch_fallback_and_backend_error_body() {
    let directory = Directory::new();
    fs::write(directory.0.join("index.html"), "index").unwrap();
    let backend = Router::new().fallback(|request: Request<Body>| async move {
        if request.uri().path() == "/health" {
            (StatusCode::OK, "healthy")
        } else {
            (StatusCode::NOT_FOUND, "original backend error")
        }
    });
    let router = static_web::mount(backend, &directory.0).unwrap();
    for (path, status, body) in [
        ("/health", StatusCode::OK, "healthy"),
        (
            "/api/missing",
            StatusCode::NOT_FOUND,
            "original backend error",
        ),
        ("/", StatusCode::OK, "index"),
    ] {
        let response = router
            .clone()
            .oneshot(Request::builder().uri(path).body(Body::empty()).unwrap())
            .await
            .unwrap();
        assert_eq!(response.status(), status);
        assert_eq!(to_bytes(response.into_body(), 1024).await.unwrap(), body);
    }
}

#[test]
fn native_file_storage_uses_explicit_persistent_directory() {
    let config = api_server::config::ApiConfig::from_env_map(&[
        ("API_DATABASE_URL", "postgres://localhost/fixture"),
        ("BOOTSTRAP_WORKSPACE_NAME", "fixture"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "test-only"),
        (
            "API_BUSINESS_FILE_LOCAL_ROOT",
            "/persistent-data/api/storage",
        ),
    ])
    .unwrap();
    assert_eq!(
        config.business_file_local_root,
        "/persistent-data/api/storage"
    );
}
