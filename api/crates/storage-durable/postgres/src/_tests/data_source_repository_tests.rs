use control_plane::ports::{
    CreateDataSourceInstanceInput, CreateDataSourcePreviewSessionInput, DataSourceRepository,
    RotateDataSourceSecretInput, UpsertDataSourceCatalogCacheInput, UpsertDataSourceSecretInput,
    UpsertPluginInstallationInput,
};
use domain::{
    DataModelStatus, DataSourceCatalogRefreshStatus, DataSourceDefaults, DataSourceInstanceStatus,
    PluginArtifactStatus, PluginAvailabilityStatus, PluginDesiredState, PluginRuntimeStatus,
    PluginVerificationStatus,
};
use serde_json::json;
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

async fn seed_store() -> (
    PgControlPlaneStore,
    domain::WorkspaceRecord,
    domain::UserRecord,
    Uuid,
) {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);

    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap();
    store
        .upsert_permission_catalog(&access_control::permission_catalog())
        .await
        .unwrap();
    store.upsert_builtin_roles(workspace.id).await.unwrap();
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
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

    let installation_id = Uuid::now_v7();
    control_plane::ports::PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
            provider_code: "acme_hubspot_source".into(),
            plugin_id: "acme_hubspot_source@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.data_source/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Acme HubSpot Source".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            artifact_status: PluginArtifactStatus::Ready,
            runtime_status: PluginRuntimeStatus::Active,
            availability_status: PluginAvailabilityStatus::Available,
            package_path: None,
            installed_path: "/tmp/plugin-installed/acme_hubspot_source/0.1.0".into(),
            checksum: Some("abc123".into()),
            manifest_fingerprint: None,
            signature_status: Some("unsigned".into()),
            signature_algorithm: None,
            signing_key_id: None,
            last_load_error: None,
            metadata_json: json!({}),
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    (store, workspace, actor, installation_id)
}

#[tokio::test]
async fn creates_instance_secret_and_catalog_cache_rows() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let instance_id = Uuid::now_v7();

    let created = <PgControlPlaneStore as DataSourceRepository>::create_instance(
        &store,
        &CreateDataSourceInstanceInput {
            instance_id,
            workspace_id: workspace.id,
            installation_id,
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            status: domain::DataSourceInstanceStatus::Draft,
            config_json: json!({ "client_id": "abc" }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults::default(),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(created.source_code, "acme_hubspot_source");
    assert_eq!(
        created.defaults.data_model_status,
        DataModelStatus::Published
    );

    let secret = <PgControlPlaneStore as DataSourceRepository>::upsert_secret(
        &store,
        &UpsertDataSourceSecretInput {
            data_source_instance_id: created.id,
            secret_ref: domain::data_source_secret_ref(created.id),
            plaintext_secret_json: json!({ "client_secret": "secret" }),
            master_key: "test-master-key".into(),
            secret_version: 1,
        },
    )
    .await
    .unwrap();
    assert_eq!(secret.data_source_instance_id, created.id);
    assert_eq!(
        secret.secret_ref,
        domain::data_source_secret_ref(created.id)
    );
    let secret_scope_id: Uuid = sqlx::query_scalar(
        "select scope_id from data_source_secrets where data_source_instance_id = $1",
    )
    .bind(created.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let secret_platform_id: Uuid =
        sqlx::query_scalar("select id from data_source_secrets where data_source_instance_id = $1")
            .bind(created.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(secret_scope_id, workspace.id);
    assert_ne!(secret_platform_id, created.id);
    let stored_secret: serde_json::Value = sqlx::query_scalar(
        "select encrypted_secret_json from data_source_secrets where data_source_instance_id = $1",
    )
    .bind(created.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(
        stored_secret["algorithm"],
        json!("aead_xchacha20poly1305_v1")
    );
    assert!(!stored_secret.to_string().contains("secret"));

    let cache = <PgControlPlaneStore as DataSourceRepository>::upsert_catalog_cache(
        &store,
        &UpsertDataSourceCatalogCacheInput {
            data_source_instance_id: created.id,
            refresh_status: DataSourceCatalogRefreshStatus::Ready,
            catalog_json: json!([
                {
                    "resource_key": "contacts",
                    "display_name": "Contacts",
                    "resource_kind": "object",
                    "metadata": {}
                }
            ]),
            last_error_message: None,
            refreshed_at: None,
        },
    )
    .await
    .unwrap();
    assert_eq!(cache.refresh_status, DataSourceCatalogRefreshStatus::Ready);
    let cache_scope_id: Uuid = sqlx::query_scalar(
        "select scope_id from data_source_catalog_caches where data_source_instance_id = $1",
    )
    .bind(created.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let cache_platform_id: Uuid = sqlx::query_scalar(
        "select id from data_source_catalog_caches where data_source_instance_id = $1",
    )
    .bind(created.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(cache_scope_id, workspace.id);
    assert_ne!(cache_platform_id, created.id);

    let loaded_secret = <PgControlPlaneStore as DataSourceRepository>::get_secret_json(
        &store,
        created.id,
        "test-master-key",
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(loaded_secret, json!({ "client_secret": "secret" }));
}

#[tokio::test]
async fn instance_record_returns_secret_reference_and_version_without_secret_value() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let plaintext = "plain-config-token";
    let instance_id = Uuid::now_v7();
    let created = <PgControlPlaneStore as DataSourceRepository>::create_instance(
        &store,
        &CreateDataSourceInstanceInput {
            instance_id,
            workspace_id: workspace.id,
            installation_id,
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            status: DataSourceInstanceStatus::Draft,
            config_json: json!({
                "base_url": "https://api.example.test",
                "access_token": {
                    "secret_ref": domain::data_source_secret_ref(instance_id),
                    "secret_version": 2
                }
            }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults::default(),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as DataSourceRepository>::upsert_secret(
        &store,
        &UpsertDataSourceSecretInput {
            data_source_instance_id: created.id,
            secret_ref: domain::data_source_secret_ref(created.id),
            plaintext_secret_json: json!({ "access_token": plaintext }),
            master_key: "test-master-key".into(),
            secret_version: 2,
        },
    )
    .await
    .unwrap();

    let loaded = <PgControlPlaneStore as DataSourceRepository>::get_instance(
        &store,
        workspace.id,
        created.id,
    )
    .await
    .unwrap()
    .unwrap();

    assert_eq!(
        loaded.secret_ref,
        Some(domain::data_source_secret_ref(created.id))
    );
    assert_eq!(loaded.secret_version, Some(2));
    assert!(!loaded.config_json.to_string().contains(plaintext));
}

#[tokio::test]
async fn rotate_secret_increments_version_inside_repository_update() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let instance_id = Uuid::now_v7();
    let created = <PgControlPlaneStore as DataSourceRepository>::create_instance(
        &store,
        &CreateDataSourceInstanceInput {
            instance_id,
            workspace_id: workspace.id,
            installation_id,
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            status: DataSourceInstanceStatus::Draft,
            config_json: json!({
                "access_token": {
                    "secret_ref": domain::data_source_secret_ref(instance_id),
                    "secret_version": 1
                }
            }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults::default(),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as DataSourceRepository>::upsert_secret(
        &store,
        &UpsertDataSourceSecretInput {
            data_source_instance_id: created.id,
            secret_ref: domain::data_source_secret_ref(created.id),
            plaintext_secret_json: json!({ "client_secret": "initial" }),
            master_key: "test-master-key".into(),
            secret_version: 1,
        },
    )
    .await
    .unwrap();

    let rotated_once = <PgControlPlaneStore as DataSourceRepository>::rotate_secret(
        &store,
        &RotateDataSourceSecretInput {
            workspace_id: workspace.id,
            data_source_instance_id: created.id,
            secret_ref: domain::data_source_secret_ref(created.id),
            plaintext_secret_json: json!({ "client_secret": "rotated-once" }),
            master_key: "test-master-key".into(),
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    let rotated_twice = <PgControlPlaneStore as DataSourceRepository>::rotate_secret(
        &store,
        &RotateDataSourceSecretInput {
            workspace_id: workspace.id,
            data_source_instance_id: created.id,
            secret_ref: domain::data_source_secret_ref(created.id),
            plaintext_secret_json: json!({ "client_secret": "rotated-twice" }),
            master_key: "test-master-key".into(),
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(rotated_once.secret.secret_version, 2);
    assert_eq!(rotated_once.instance.secret_version, Some(2));
    assert_eq!(
        rotated_once.instance.config_json["access_token"]["secret_version"],
        json!(2)
    );
    assert_eq!(rotated_twice.secret.secret_version, 3);
    assert_eq!(rotated_twice.instance.secret_version, Some(3));
    assert_eq!(
        rotated_twice.instance.config_json["access_token"]["secret_version"],
        json!(3)
    );
    assert_eq!(
        <PgControlPlaneStore as DataSourceRepository>::get_secret_json(
            &store,
            created.id,
            "test-master-key",
        )
        .await
        .unwrap()
        .unwrap()["client_secret"],
        "rotated-twice"
    );
}

#[tokio::test]
async fn rotate_secret_preserves_existing_config_marker_values_when_payload_is_partial() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let instance_id = Uuid::now_v7();
    let secret_ref = domain::data_source_secret_ref(instance_id);
    let created = <PgControlPlaneStore as DataSourceRepository>::create_instance(
        &store,
        &CreateDataSourceInstanceInput {
            instance_id,
            workspace_id: workspace.id,
            installation_id,
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            status: DataSourceInstanceStatus::Draft,
            config_json: json!({
                "access_token": {
                    "secret_ref": secret_ref,
                    "secret_version": 1
                },
                "headers": [
                    {
                        "name": "Authorization",
                        "value": {
                            "secret_ref": secret_ref,
                            "secret_version": 1
                        }
                    },
                    { "name": "X-Trace", "value": "not-secret" }
                ]
            }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults::default(),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    <PgControlPlaneStore as DataSourceRepository>::upsert_secret(
        &store,
        &UpsertDataSourceSecretInput {
            data_source_instance_id: created.id,
            secret_ref: domain::data_source_secret_ref(created.id),
            plaintext_secret_json: json!({
                "client_secret": "initial-client-secret",
                "__config_secret_values": {
                    "/access_token": "config-token-secret",
                    "/headers/0/value": "authorization-secret"
                }
            }),
            master_key: "test-master-key".into(),
            secret_version: 1,
        },
    )
    .await
    .unwrap();

    let rotated = <PgControlPlaneStore as DataSourceRepository>::rotate_secret(
        &store,
        &RotateDataSourceSecretInput {
            workspace_id: workspace.id,
            data_source_instance_id: created.id,
            secret_ref: domain::data_source_secret_ref(created.id),
            plaintext_secret_json: json!({ "client_secret": "rotated-client-secret" }),
            master_key: "test-master-key".into(),
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();

    let stored_secret = <PgControlPlaneStore as DataSourceRepository>::get_secret_json(
        &store,
        created.id,
        "test-master-key",
    )
    .await
    .unwrap()
    .unwrap();
    assert_eq!(stored_secret["client_secret"], "rotated-client-secret");
    assert_eq!(
        stored_secret["__config_secret_values"]["/access_token"],
        "config-token-secret"
    );
    assert_eq!(
        stored_secret["__config_secret_values"]["/headers/0/value"],
        "authorization-secret"
    );
    assert_eq!(rotated.secret.secret_version, 2);
    assert_eq!(
        rotated.instance.config_json["access_token"]["secret_version"],
        json!(2)
    );
    assert_eq!(
        rotated.instance.config_json["headers"][0]["value"]["secret_version"],
        json!(2)
    );
    assert_eq!(
        rotated.instance.config_json["headers"][1]["value"],
        json!("not-secret")
    );
}

#[tokio::test]
async fn creates_preview_session_rows() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let instance_id = Uuid::now_v7();
    let created = <PgControlPlaneStore as DataSourceRepository>::create_instance(
        &store,
        &CreateDataSourceInstanceInput {
            instance_id,
            workspace_id: workspace.id,
            installation_id,
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            status: DataSourceInstanceStatus::Draft,
            config_json: json!({ "client_id": "abc" }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults {
                data_model_status: DataModelStatus::Draft,
            },
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    let preview_session = <PgControlPlaneStore as DataSourceRepository>::create_preview_session(
        &store,
        &CreateDataSourcePreviewSessionInput {
            session_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            actor_user_id: actor.id,
            data_source_instance_id: Some(created.id),
            config_fingerprint: "preview:contacts".into(),
            preview_json: json!({
                "rows": [{ "id": "1" }],
                "next_cursor": null
            }),
            expires_at: time::OffsetDateTime::now_utc() + time::Duration::minutes(10),
        },
    )
    .await
    .unwrap();

    assert_eq!(preview_session.data_source_instance_id, Some(created.id));
}

#[tokio::test]
async fn updates_data_source_default_status() {
    let (store, workspace, actor, installation_id) = seed_store().await;
    let created = <PgControlPlaneStore as DataSourceRepository>::create_instance(
        &store,
        &CreateDataSourceInstanceInput {
            instance_id: Uuid::now_v7(),
            workspace_id: workspace.id,
            installation_id,
            source_code: "acme_hubspot_source".into(),
            display_name: "HubSpot".into(),
            status: DataSourceInstanceStatus::Draft,
            config_json: json!({ "client_id": "abc" }),
            metadata_json: json!({}),
            defaults: DataSourceDefaults::default(),
            created_by: actor.id,
        },
    )
    .await
    .unwrap();

    let updated = <PgControlPlaneStore as DataSourceRepository>::update_instance_defaults(
        &store,
        &control_plane::ports::UpdateDataSourceDefaultsInput {
            workspace_id: workspace.id,
            instance_id: created.id,
            defaults: DataSourceDefaults {
                data_model_status: DataModelStatus::Draft,
            },
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();

    assert_eq!(updated.defaults.data_model_status, DataModelStatus::Draft);
}

#[tokio::test]
async fn updates_main_source_defaults_with_workspace_scoped_status() {
    let (store, workspace, actor, _) = seed_store().await;

    let updated = <PgControlPlaneStore as DataSourceRepository>::update_main_source_defaults(
        &store,
        &control_plane::ports::UpdateMainSourceDefaultsInput {
            workspace_id: workspace.id,
            defaults: DataSourceDefaults {
                data_model_status: DataModelStatus::Draft,
            },
            updated_by: actor.id,
        },
    )
    .await
    .unwrap();
    assert_eq!(updated.data_model_status, DataModelStatus::Draft);

    let (row_id, scope_id, row_count): (Uuid, Uuid, i64) = sqlx::query_as(
        r#"
        select id, scope_id, count(*) over ()::bigint as row_count
        from main_source_defaults
        where workspace_id = $1
        "#,
    )
    .bind(workspace.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_ne!(row_id, Uuid::nil());
    assert_eq!(scope_id, workspace.id);
    assert_eq!(row_count, 1);
}
