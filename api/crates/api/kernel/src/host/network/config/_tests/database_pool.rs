use super::{default_database_pool_max_connections, ApiConfig};

fn base_env() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "API_DATABASE_URL",
            "postgres://postgres:fixture@localhost/fixture",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "fixture"),
        ("BOOTSTRAP_WORKSPACE_NAME", "fixture"),
    ]
}

#[test]
fn api_config_uses_adaptive_database_pool_max_connections_when_unset() {
    let config = ApiConfig::from_env_map(&base_env()).unwrap();
    let expected = default_database_pool_max_connections();
    assert!((10..=32).contains(&expected));
    assert_eq!(config.database_pool_max_connections, expected);
}

#[test]
fn api_config_honors_explicit_and_rejects_zero_database_pool_max_connections() {
    let mut explicit = base_env();
    explicit.push(("API_DATABASE_POOL_MAX_CONNECTIONS", "3"));
    assert_eq!(
        ApiConfig::from_env_map(&explicit)
            .unwrap()
            .database_pool_max_connections,
        3
    );

    let mut invalid = base_env();
    invalid.push(("API_DATABASE_POOL_MAX_CONNECTIONS", "0"));
    assert!(ApiConfig::from_env_map(&invalid)
        .unwrap_err()
        .to_string()
        .contains("API_DATABASE_POOL_MAX_CONNECTIONS"));
}
