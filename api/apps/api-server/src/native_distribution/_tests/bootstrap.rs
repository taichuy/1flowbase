use super::*;
use crate::native_distribution::{postgres::Postgres, settings::private_dir};
use std::net::TcpListener;

// Run alone: Settings::apply configures the process environment before Tokio starts.
// The actual packaged resources must be present, including application templates.
#[test]
#[ignore = "requires FLOWBASE_TEST_PACKAGE_ROOT; run with --ignored --exact"]
fn packaged_application_bootstraps_and_reopens() {
    let root = PathBuf::from(
        std::env::var_os("FLOWBASE_TEST_PACKAGE_ROOT")
            .expect("FLOWBASE_TEST_PACKAGE_ROOT must point to the staged distribution"),
    );
    std::env::remove_var("FLOWBASE_DATA_DIR");
    let data = crate::native_distribution::paths::data_root(&root).unwrap();
    assert_eq!(data, root.join("data"));
    assert!(
        !data.exists(),
        "bootstrap test requires a fresh staged package"
    );
    let _directory = Directory(data.clone());
    private_dir(&data).unwrap();
    let settings = Settings::load_or_create(&data).unwrap();
    let http = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let pg = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let http_port = http.local_addr().unwrap().port();
    let pg_port = pg.local_addr().unwrap().port();
    drop((http, pg));
    settings.apply(&root, &data, http_port, pg_port).unwrap();
    let previous_directory = std::env::current_dir().unwrap();
    std::env::set_current_dir(&data).unwrap();

    for _ in 0..2 {
        let mut postgres = Postgres::new(&root.join("postgres"), &data);
        postgres.validate().unwrap();
        postgres
            .start(pg_port, &settings.postgres_password)
            .unwrap();
        {
            let runtime = tokio::runtime::Builder::new_multi_thread()
                .enable_all()
                .build()
                .unwrap();
            runtime.block_on(async {
                // Exercise real migrations, bootstrap/template document writes and
                // the complete router assembly, not just PostgreSQL readiness.
                let (app, host) = api_server::app_and_runtime_host_from_env()
                    .await
                    .expect("packaged application bootstrap must succeed");
                let app = static_web::mount(app, &root.join("web")).unwrap();
                for uri in ["/health", "/", "/native-smoke/deep-link"] {
                    let response = app
                        .clone()
                        .oneshot(
                            Request::builder()
                                .uri(uri)
                                .header("accept", "text/html")
                                .body(Body::empty())
                                .unwrap(),
                        )
                        .await
                        .unwrap();
                    assert_eq!(response.status(), StatusCode::OK, "{uri}");
                }
                host.stop().await.unwrap();
            });
        }
        drop(postgres);
        assert!(!data.join("postgres/postmaster.pid").exists());
    }
    std::env::set_current_dir(previous_directory).unwrap();
}
