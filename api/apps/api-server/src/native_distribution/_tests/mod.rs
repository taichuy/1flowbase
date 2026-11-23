use super::static_web;
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    routing::get,
    Router,
};
use std::{fs, path::PathBuf};
use tower::ServiceExt;

mod bootstrap;

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
        ("/healthz", StatusCode::NOT_FOUND, ""),
        ("/readyz", StatusCode::NOT_FOUND, ""),
        ("/metrics", StatusCode::NOT_FOUND, ""),
        ("/assets/missing.js", StatusCode::NOT_FOUND, ""),
        ("/external-npm/missing", StatusCode::NOT_FOUND, ""),
        ("/%2e%2e/settings.json", StatusCode::NOT_FOUND, ""),
        ("/C:%5csettings.json", StatusCode::NOT_FOUND, ""),
        ("/%01probe", StatusCode::NOT_FOUND, ""),
        ("/%7fprobe", StatusCode::NOT_FOUND, ""),
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

#[tokio::test]
async fn serves_precompressed_sidecars_and_immutable_hashed_asset_headers_without_touching_backend()
{
    use std::sync::{
        atomic::{AtomicUsize, Ordering},
        Arc,
    };

    let directory = Directory::new();
    let assets = directory.0.join("assets");
    fs::create_dir_all(&assets).unwrap();
    fs::write(directory.0.join("index.html"), "<html>spa</html>").unwrap();
    fs::write(directory.0.join("icon.svg"), "<svg/>").unwrap();
    fs::write(
        directory.0.join("authenticated-prewarm.json"),
        "{\"files\":[]}",
    )
    .unwrap();
    let hashed_js = assets.join("shell-core-Ab12_-9Z.js");
    fs::write(&hashed_js, "console.log('uncompressed');").unwrap();
    fs::write(
        assets.join("shell-core-Ab12_-9Z.js.br"),
        b"br-compressed-bytes",
    )
    .unwrap();
    fs::write(
        assets.join("shell-core-Ab12_-9Z.js.gz"),
        b"gz-compressed-bytes",
    )
    .unwrap();

    let backend_hits = Arc::new(AtomicUsize::new(0));
    let counter = Arc::clone(&backend_hits);
    let backend = Router::new().fallback(move || {
        let counter = Arc::clone(&counter);
        async move {
            counter.fetch_add(1, Ordering::SeqCst);
            (StatusCode::NOT_FOUND, "backend fallback")
        }
    });
    let router = static_web::mount(backend, &directory.0).unwrap();

    // 1. Brotli negotiation serves .br sidecar + immutable Cache-Control without invoking backend
    let br_res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/shell-core-Ab12_-9Z.js")
                .header("accept-encoding", "gzip, deflate, br")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(br_res.status(), StatusCode::OK);
    assert_eq!(br_res.headers().get("content-encoding").unwrap(), "br");
    assert_eq!(
        br_res.headers().get("cache-control").unwrap(),
        "public, max-age=31536000, immutable"
    );
    assert_eq!(
        to_bytes(br_res.into_body(), 1024).await.unwrap().as_ref(),
        b"br-compressed-bytes"
    );
    assert_eq!(
        backend_hits.load(Ordering::SeqCst),
        0,
        "fast-path static asset hit must not traverse backend router"
    );

    // 2. Gzip negotiation serves .gz sidecar
    let gz_res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/assets/shell-core-Ab12_-9Z.js")
                .header("accept-encoding", "gzip")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(gz_res.status(), StatusCode::OK);
    assert_eq!(gz_res.headers().get("content-encoding").unwrap(), "gzip");
    assert_eq!(
        to_bytes(gz_res.into_body(), 1024).await.unwrap().as_ref(),
        b"gz-compressed-bytes"
    );
    assert_eq!(backend_hits.load(Ordering::SeqCst), 0);

    // 3. /icon.svg gets 1-day cache and /authenticated-prewarm.json gets no-cache without hitting backend
    let icon_res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/icon.svg")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(icon_res.status(), StatusCode::OK);
    assert_eq!(
        icon_res.headers().get("cache-control").unwrap(),
        "public, max-age=86400"
    );
    let prewarm_res = router
        .clone()
        .oneshot(
            Request::builder()
                .uri("/authenticated-prewarm.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(prewarm_res.status(), StatusCode::OK);
    assert_eq!(
        prewarm_res.headers().get("cache-control").unwrap(),
        "no-cache"
    );
    assert_eq!(backend_hits.load(Ordering::SeqCst), 0);

    // 4. Missing /assets/ file returns fast 404 directly without traversing the backend router
    let missing_res = router
        .oneshot(
            Request::builder()
                .uri("/assets/does-not-exist-12345678.js")
                .header("accept", "text/html")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_res.status(), StatusCode::NOT_FOUND);
    assert!(to_bytes(missing_res.into_body(), 1024)
        .await
        .unwrap()
        .is_empty());
    assert_eq!(
        backend_hits.load(Ordering::SeqCst),
        0,
        "missing hashed static asset must fail fast without traversing API router"
    );

    // 5. apply_default_cache_control preserves any pre-existing explicit Cache-Control header
    let mut explicit_response = axum::response::IntoResponse::into_response((
        [(axum::http::header::CACHE_CONTROL, "private, max-age=60")],
        "custom",
    ));
    static_web::apply_default_cache_control(
        &mut explicit_response,
        "public, max-age=31536000, immutable",
    );
    assert_eq!(
        explicit_response.headers().get("cache-control").unwrap(),
        "private, max-age=60"
    );
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

#[test]
#[ignore = "requires FLOWBASE_TEST_POSTGRES_ROOT pointing to a PostgreSQL 18 bundle"]
fn bundled_postgres_uses_tcp_only() {
    use super::{postgres::Postgres, settings::private_dir};
    use std::{net::TcpListener, process::Command};

    let root = PathBuf::from(
        std::env::var_os("FLOWBASE_TEST_POSTGRES_ROOT")
            .expect("FLOWBASE_TEST_POSTGRES_ROOT is required for the real PostgreSQL test"),
    );
    let directory = Directory::new();
    let data = directory.0.join("database with spaces");
    private_dir(&data).unwrap();
    let postgres_password = uuid::Uuid::now_v7().to_string();
    let reservation = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let port = reservation.local_addr().unwrap().port();
    drop(reservation);

    // Cover both a fresh cluster and the already initialized cluster users retain
    // after an earlier startup failure. No application/developer database is used.
    for _ in 0..2 {
        let mut postgres = Postgres::new(&root, &data);
        postgres.validate().unwrap();
        postgres
            .start(port, &postgres_password)
            .unwrap_or_else(|error| {
                panic!(
                    "{error:#}\n{}",
                    fs::read_to_string(data.join("postgres.log")).unwrap_or_default()
                )
            });
        let result = Command::new(root.join("bin").join(format!("psql{}", std::env::consts::EXE_SUFFIX)))
            .args(["-X", "-w", "-h", "127.0.0.1", "-p", &port.to_string(), "-U", "postgres", "-d", "1flowbase", "-Atc"])
            .arg(format!("SELECT current_setting('unix_socket_directories') = '', current_setting('listen_addresses') = '127.0.0.1', current_setting('port') = '{port}'"))
            .env("PGPASSWORD", &postgres_password)
            .env_remove("PGSERVICE")
            .env_remove("PGOPTIONS")
            .output().unwrap();
        assert!(
            result.status.success(),
            "{}",
            String::from_utf8_lossy(&result.stderr)
        );
        assert_eq!(String::from_utf8_lossy(&result.stdout).trim(), "t|t|t");
        drop(postgres);
        assert!(
            !data.join("postgres/postmaster.pid").exists(),
            "owned PostgreSQL must stop"
        );
    }
}
