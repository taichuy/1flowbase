use storage_durable_postgres::{run_migrations, PgControlPlaneStore};

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

pub async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "Control Plane PostgreSQL Tests")
        .await
        .unwrap();
    control_plane_test_support::upsert_permission_catalog(&store)
        .await
        .unwrap();
    control_plane_test_support::upsert_builtin_roles(&store, workspace.id)
        .await
        .unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: serde_json::json!({}),
        })
        .await
        .unwrap();
    let actor = store
        .upsert_root_user(
            workspace.id,
            "root",
            "root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    (store, workspace, actor)
}

pub fn runtime_profile_interface() -> domain::McpInterfaceCatalogEntry {
    domain::McpInterfaceCatalogEntry {
        interface_id: "get_runtime_profile".into(),
        source: domain::McpInterfaceCatalogSource::StaticApi,
        method: "GET".into(),
        path: "/api/console/system/runtime-profile".into(),
        name: "Get runtime profile".into(),
        short_description: "Read system runtime profile.".into(),
        parameter_descriptors: vec![domain::mcp_management::McpParameterDescriptor {
            name: "locale".into(),
            field_type: "string".into(),
            parameter_type: domain::mcp_management::McpParameterType::Url,
            description: None,
            required: false,
            schema: serde_json::json!({"type":"string"}),
        }],
        parameter_schema: serde_json::json!({"type":"object"}),
        result_schema: serde_json::json!({"type":"object"}),
        permission_code: None,
        security: serde_json::json!([]),
        risk_level: domain::McpRiskLevel::Low,
        bindable: true,
        disabled_reason: None,
    }
}
