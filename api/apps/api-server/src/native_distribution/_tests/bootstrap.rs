use super::*;
use crate::native_distribution::{
    env_file,
    postgres::Postgres,
    settings::{private_dir, Settings},
};
use argon2::{
    password_hash::{phc::PasswordHash, PasswordVerifier},
    Argon2,
};
use control_plane_contracts::ports::AuthRepository;
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
    let data = root.join("data");
    assert_eq!(data, root.join("data"));
    assert!(
        !data.exists(),
        "bootstrap test requires a fresh staged package"
    );
    let _directory = Directory(data.clone());
    private_dir(&data).unwrap();
    let http = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let pg = TcpListener::bind(("127.0.0.1", 0)).unwrap();
    let http_port = http.local_addr().unwrap().port();
    let pg_port = pg.local_addr().unwrap().port();
    drop((http, pg));
    let mut overrides = env_file::Values::new();
    overrides.insert("API_SERVER_ADDR".into(), format!("127.0.0.1:{http_port}"));
    overrides.insert(
        "API_DATABASE_URL".into(),
        format!("postgres://postgres:__GENERATE__@127.0.0.1:{pg_port}/1flowbase"),
    );
    overrides.insert("FLOWBASE_OPEN_BROWSER".into(), "false".into());
    overrides.insert(
        "BOOTSTRAP_ROOT_ACCOUNT".into(),
        "native-config-probe".into(),
    );
    overrides.insert(
        "BOOTSTRAP_ROOT_EMAIL".into(),
        "native-config-probe@example.com".into(),
    );
    overrides.insert(
        "BOOTSTRAP_ROOT_PASSWORD".into(),
        "Native-env-probe123".into(),
    );
    env_file::persist_generated(&root, &overrides).unwrap();
    let settings = Settings::load(&root).unwrap();
    settings.apply(&root).unwrap();
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
                // Resolve the same local-login identity and verify the same hash
                // algorithm as password authentication, against the initialized DB.
                let pool = sqlx::PgPool::connect(&std::env::var("API_DATABASE_URL").unwrap())
                    .await
                    .unwrap();
                let store = storage_durable_postgres::PgControlPlaneStore::new(pool.clone());
                let user = store
                    .find_user_for_password_login(
                        domain::PASSWORD_LOCAL_CONNECTION_ID,
                        "native-config-probe",
                    )
                    .await
                    .unwrap()
                    .expect("default root login identity must exist");
                let hash = PasswordHash::new(&user.password_hash).unwrap();
                assert!(Argon2::default()
                    .verify_password(b"Native-env-probe123", &hash)
                    .is_ok());
                assert!(Argon2::default()
                    .verify_password(b"wrong-password", &hash)
                    .is_err());
                drop(store);
                pool.close().await;
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
