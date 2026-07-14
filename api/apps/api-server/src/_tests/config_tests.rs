use api_server::{
    config::{ApiConfig, ApiEnvironment},
    parse_bind_addr, DEFAULT_API_SERVER_ADDR,
};
use std::path::PathBuf;

fn current_workspace_root() -> PathBuf {
    std::env::current_dir()
        .unwrap()
        .ancestors()
        .find(|path| {
            path.join(".git").exists() && path.join("api").is_dir() && path.join("web").is_dir()
        })
        .unwrap()
        .to_path_buf()
}

fn base_env_without_ephemeral_backend() -> Vec<(&'static str, &'static str)> {
    vec![
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ]
}

#[test]
fn parse_bind_addr_rejects_invalid_candidate() {
    let error = parse_bind_addr(Some("not-a-socket-address"), DEFAULT_API_SERVER_ADDR).unwrap_err();

    assert!(error.to_string().contains("API_SERVER_ADDR"));
}

#[test]
fn parse_bind_addr_accepts_default_when_candidate_is_missing() {
    let addr = parse_bind_addr(None, DEFAULT_API_SERVER_ADDR).unwrap();

    assert_eq!(addr.to_string(), DEFAULT_API_SERVER_ADDR);
}

#[test]
fn api_config_does_not_require_ephemeral_backend_env() {
    let env = base_env_without_ephemeral_backend();
    let config = ApiConfig::from_env_map(&env).unwrap();

    assert_eq!(config.cookie_name, "flowbase_console_session");
}

#[test]
fn api_config_ignores_legacy_ephemeral_redis_env_selection() {
    let mut env = base_env_without_ephemeral_backend();
    env.push(("API_EPHEMERAL_BACKEND", "redis"));
    let config = ApiConfig::from_env_map(&env).unwrap();

    assert_eq!(config.cookie_name, "flowbase_console_session");
}

#[test]
fn api_config_uses_expected_cookie_defaults() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(config.cookie_name, "flowbase_console_session");
    assert_eq!(config.session_ttl_days, 7);
    assert_eq!(config.database_pool_max_connections, 5);
}

#[test]
fn api_config_reads_database_pool_max_connections() {
    let mut env = base_env_without_ephemeral_backend();
    env.push(("API_DATABASE_POOL_MAX_CONNECTIONS", "1"));
    let config = ApiConfig::from_env_map(&env).unwrap();

    assert_eq!(config.database_pool_max_connections, 1);
}

#[test]
fn api_config_rejects_invalid_database_pool_max_connections() {
    let mut env = base_env_without_ephemeral_backend();
    env.push(("API_DATABASE_POOL_MAX_CONNECTIONS", "0"));
    let error = ApiConfig::from_env_map(&env).unwrap_err();

    assert!(error
        .to_string()
        .contains("API_DATABASE_POOL_MAX_CONNECTIONS"));
}

#[test]
fn api_config_defaults_to_development_and_unrestricted_cors() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(config.env, ApiEnvironment::Development);
    assert!(config.cors_allowed_origins.is_none());
}

#[test]
fn api_config_defaults_provider_install_root_to_api_workspace_plugins_directory() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    let expected_root = current_workspace_root().join("api").join("plugins");
    assert_eq!(PathBuf::from(&config.provider_install_root), expected_root);
    assert_eq!(
        PathBuf::from(&config.host_extension_dropin_root),
        expected_root.join("host-extension").join("dropins")
    );
}

#[test]
fn api_config_derives_stable_default_api_node_id_from_provider_install_root() {
    let env = [
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_PROVIDER_INSTALL_ROOT", "/tmp/1flowbase-provider-a"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ];
    let first = ApiConfig::from_env_map(&env).unwrap();
    let second = ApiConfig::from_env_map(&env).unwrap();

    assert_eq!(first.api_node_id, second.api_node_id);
    assert!(first.api_node_id.starts_with("api-node-"));
}

#[test]
fn api_config_uses_explicit_api_node_id() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_PROVIDER_INSTALL_ROOT", "/tmp/1flowbase-provider-a"),
        ("API_NODE_ID", "docker-api-1"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(config.api_node_id, "docker-api-1");
}

#[test]
fn api_config_uses_api_storage_as_default_business_file_root() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("BOOTSTRAP_WORKSPACE_NAME", "System"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "password"),
    ])
    .unwrap();

    assert!(config.business_file_local_root.ends_with("api/storage"));
}

#[test]
fn api_config_rejects_production_without_allowed_origins() {
    let error = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("API_ENV", "production"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .expect_err("production config should require explicit API_ALLOWED_ORIGINS");

    assert!(error.to_string().contains("API_ALLOWED_ORIGINS"));
}

#[test]
fn api_config_accepts_production_with_explicit_allowed_origins() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("API_ENV", "production"),
        (
            "API_ALLOWED_ORIGINS",
            "https://console.example.com,https://ops.example.com",
        ),
        ("API_PROVIDER_SECRET_MASTER_KEY", "provider-secret-key"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(config.env, ApiEnvironment::Production);
    let origins = config
        .cors_allowed_origins
        .expect("production should keep explicit cors origins");
    let values = origins
        .iter()
        .map(|value| value.to_str().unwrap().to_string())
        .collect::<Vec<_>>();

    assert_eq!(
        values,
        vec![
            "https://console.example.com".to_string(),
            "https://ops.example.com".to_string()
        ]
    );
}

#[test]
fn api_config_rejects_production_placeholder_provider_secret_master_key() {
    let error = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_ENV", "production"),
        ("API_ALLOWED_ORIGINS", "https://console.example.com"),
        (
            "API_PROVIDER_SECRET_MASTER_KEY",
            "change-me-provider-secret-master-key",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .expect_err("production config should reject placeholder provider secret key");

    assert!(error.to_string().contains("API_PROVIDER_SECRET_MASTER_KEY"));
}

#[test]
fn api_config_marks_session_cookie_secure_in_production() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_ENV", "production"),
        ("API_ALLOWED_ORIGINS", "https://console.example.com"),
        (
            "API_PROVIDER_SECRET_MASTER_KEY",
            "strong-provider-secret-master-key",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert!(config.cookie_secure);
}

#[test]
fn api_config_allows_disabling_secure_session_cookie_for_plain_http_deployments() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_ENV", "production"),
        ("API_ALLOWED_ORIGINS", "http://192.168.31.25:3200"),
        ("API_COOKIE_SECURE", "false"),
        (
            "API_PROVIDER_SECRET_MASTER_KEY",
            "strong-provider-secret-master-key",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert!(!config.cookie_secure);
}

#[test]
fn api_config_leaves_session_cookie_insecure_by_default_for_development() {
    let config = ApiConfig::from_env_map(&base_env_without_ephemeral_backend()).unwrap();

    assert!(!config.cookie_secure);
}

#[test]
fn api_config_reads_bootstrap_workspace_name() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(config.bootstrap_workspace_name, "1flowbase");
}

#[test]
fn api_config_defaults_host_extension_settings() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("API_PROVIDER_INSTALL_ROOT", "/srv/1flowbase/plugins"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(
        config.host_extension_dropin_root,
        "/srv/1flowbase/plugins/host-extension/dropins"
    );
    assert!(config.allow_unverified_filesystem_dropins);
    assert!(!config.allow_uploaded_host_extensions);
}

#[test]
fn api_config_reads_host_extension_overrides() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("API_HOST_EXTENSION_DROPIN_ROOT", "/opt/host-dropins"),
        ("API_PLUGIN_ALLOW_UNVERIFIED_FILESYSTEM_DROPINS", "false"),
        ("API_PLUGIN_ALLOW_UPLOADED_HOST_EXTENSIONS", "true"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(config.host_extension_dropin_root, "/opt/host-dropins");
    assert!(!config.allow_unverified_filesystem_dropins);
    assert!(config.allow_uploaded_host_extensions);
}

#[test]
fn api_config_reads_provider_secret_master_key() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("API_PROVIDER_SECRET_MASTER_KEY", "provider-secret-key"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(config.provider_secret_master_key, "provider-secret-key");
}

#[test]
fn api_config_defaults_plugin_runner_internal_base_url() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(
        config.plugin_runner_internal_base_url,
        "http://127.0.0.1:7801"
    );
}

#[test]
fn api_config_reads_plugin_runner_internal_base_url() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        (
            "API_PLUGIN_RUNNER_INTERNAL_BASE_URL",
            "http://plugin-runner.internal:7801",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(
        config.plugin_runner_internal_base_url,
        "http://plugin-runner.internal:7801"
    );
}

#[test]
fn api_config_reads_official_plugin_repository_settings() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        (
            "API_OFFICIAL_PLUGIN_REPOSITORY",
            "taichuy/1flowbase-official-plugins",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(
        config.official_plugin_repository,
        "taichuy/1flowbase-official-plugins"
    );
}

#[test]
fn api_config_prefers_mirror_registry_when_present() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        (
            "API_OFFICIAL_PLUGIN_DEFAULT_REGISTRY_URL",
            "https://official.example.com/official-registry.json",
        ),
        (
            "API_OFFICIAL_PLUGIN_MIRROR_REGISTRY_URL",
            "https://mirror.example.com/official-registry.json",
        ),
        (
            "API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON",
            r#"[{"key_id":"official-key-2026-04","algorithm":"ed25519","public_key_pem":"-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEA7n50M0Xkq4n3aQm7x0Whv14jArlTc95xJ3Adxpv8uKk=\n-----END PUBLIC KEY-----"}]"#,
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    let resolved = config.resolve_official_plugin_source();
    assert_eq!(resolved.source_kind, "mirror_registry");
    assert_eq!(
        resolved.registry_url,
        "https://mirror.example.com/official-registry.json"
    );
}

#[test]
fn api_config_defaults_official_plugin_signature_required() {
    let config = ApiConfig::from_env_map(&base_env_without_ephemeral_backend()).unwrap();

    assert!(config.official_plugin_signature_required);

    let resolved = config.resolve_official_plugin_source();
    assert_eq!(resolved.trust_mode, "signature_required");
}

#[test]
fn api_config_reads_official_plugin_signature_required_override() {
    let mut env = base_env_without_ephemeral_backend();
    env.push(("API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED", "false"));
    let config = ApiConfig::from_env_map(&env).unwrap();

    assert!(!config.official_plugin_signature_required);

    let resolved = config.resolve_official_plugin_source();
    assert_eq!(resolved.trust_mode, "allow_unsigned");
}

#[test]
fn api_config_rejects_invalid_official_plugin_signature_required_override() {
    let mut env = base_env_without_ephemeral_backend();
    env.push(("API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED", "sometimes"));
    let error = ApiConfig::from_env_map(&env).unwrap_err();

    assert!(error
        .to_string()
        .contains("API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED"));
}

#[test]
fn api_config_reads_official_plugin_github_proxy_url() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        (
            "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL",
            "https://gh-proxy.com/",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(
        config.official_plugin_github_proxy_url.as_deref(),
        Some("https://gh-proxy.com/")
    );

    let resolved = config.resolve_official_plugin_source();
    assert_eq!(
        resolved.github_proxy_url.as_deref(),
        Some("https://gh-proxy.com/")
    );
}

#[test]
fn api_config_resolves_official_agent_flow_template_catalog_source() {
    let config = ApiConfig::from_env_map(&[
        (
            "API_DATABASE_URL",
            "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase",
        ),
        ("API_EPHEMERAL_BACKEND", "memory"),
        (
            "API_OFFICIAL_PLUGIN_REPOSITORY",
            "taichuy/1flowbase-official-plugins",
        ),
        (
            "API_OFFICIAL_AGENT_FLOW_TEMPLATE_MIRROR_INDEX_URL",
            "https://mirror.example.com/agent-flow/catalog/v1/index.json",
        ),
        (
            "API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL",
            "https://gh-proxy.com/",
        ),
        ("BOOTSTRAP_ROOT_ACCOUNT", "root"),
        ("BOOTSTRAP_ROOT_EMAIL", "root@example.com"),
        ("BOOTSTRAP_ROOT_PASSWORD", "secret"),
        ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase"),
    ])
    .unwrap();

    assert_eq!(
        config.official_agent_flow_template_default_index_url,
        "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/agent-flow/catalog/v1/index.json"
    );

    let resolved = config.resolve_official_agent_flow_template_source();
    assert_eq!(resolved.source_kind, "mirror_registry");
    assert_eq!(
        resolved.index_url,
        "https://mirror.example.com/agent-flow/catalog/v1/index.json"
    );
    assert_eq!(
        resolved.github_proxy_url.as_deref(),
        Some("https://gh-proxy.com/")
    );
}
