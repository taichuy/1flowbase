use super::ApiConfig;

#[test]
fn plugin_resources_do_not_require_or_parse_a_deployment_participant_registry() {
    let standard_env = vec![
        (
            "API_DATABASE_URL",
            "postgres://postgres:fixture@localhost/fixture",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "fixture"),
        ("BOOTSTRAP_WORKSPACE_NAME", "fixture"),
    ];
    assert!(standard_env
        .iter()
        .all(|(key, _)| *key != "PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON"));
    ApiConfig::from_env_map(&standard_env)
        .expect("standard deployment needs no resource allowlist");

    // Even a stale deployment value is inert; it cannot grant access or break startup.
    let mut stale_env = standard_env;
    stale_env.push(("PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON", "not-a-registry"));
    ApiConfig::from_env_map(&stale_env).expect("retired allowlist configuration is not consulted");
}
