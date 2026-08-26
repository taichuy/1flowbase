use control_plane::{
    network_egress_pool::{
        ensure_global_network_egress_pool, CreateNetworkEgressPoolMemberCommand,
        NetworkEgressPoolService, GLOBAL_NETWORK_EGRESS_POOL_ID,
    },
    network_egress_route::{CreateNetworkEgressRouteCommand, NetworkEgressRouteService},
    ports::{
        CreateNetworkEgressPoolInput, CreateNetworkEgressPoolMemberInput,
        CreateNetworkEgressRouteInput, CreateStaticHttpProxyPoolMemberInput,
        NetworkEgressPoolRepository, NetworkEgressRouteRepository, PluginRepository,
        UpsertPluginInstallationInput,
    },
};
use control_plane_contracts::ports::{
    CreateNetworkEgressProviderInput, NetworkEgressRepository, RecordNetworkEgressSyncFailureInput,
    ReplaceNetworkEgressProjectionInput, UpdateNetworkEgressProviderLifecycleInput,
    UpsertNetworkEgressProviderSecretInput,
};
use domain::{
    ExtensionCategory, ExtensionSignatureStatus, PluginDesiredState, PluginVerificationStatus,
};
use serde_json::json;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use time::OffsetDateTime;
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn store() -> (PgControlPlaneStore, domain::UserRecord) {
    let schema = postgres_test_support::PostgresTestSchema::create(&database_url())
        .await
        .expect("isolated schema should be available");
    let pool = schema.connect().await.expect("schema should connect");
    crate::run_migrations(&pool)
        .await
        .expect("migrations should apply");
    let store = PgControlPlaneStore::new(pool);
    let tenant = store
        .upsert_root_tenant()
        .await
        .expect("tenant should seed");
    let workspace = store
        .upsert_workspace(tenant.id, "network-egress")
        .await
        .expect("workspace should seed");
    store
        .upsert_authenticator(&domain::AuthenticatorRecord {
            id: domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
            auth_type: "password-local".into(),
            title: "Password".into(),
            enabled: true,
            is_builtin: true,
            sort_order: 0,
            public_ui_block: String::new(),
            options: json!({}),
        })
        .await
        .expect("password-local authenticator should seed before root identities");
    let actor = store
        .upsert_root_user(
            workspace.id,
            "network-egress-root",
            "network-egress-root@example.com",
            "$argon2id$v=19$m=19456,t=2,p=1$test$test",
            "Network",
            "Root",
        )
        .await
        .expect("actor should seed");
    (store, actor)
}

#[tokio::test]
async fn ac_015_failed_sync_preserves_stably_ordered_egress_projection() {
    let (store, actor) = store().await;
    let installation_id = Uuid::now_v7();
    PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
            provider_code: "fixture_egress".to_string(),
            plugin_id: "fixture_egress@0.1.0".to_string(),
            plugin_version: "0.1.0".to_string(),
            contract_version: extension_contracts::NETWORK_EGRESS_PROVIDER_CONTRACT.to_string(),
            protocol: "stdio_json_worker".to_string(),
            display_name: "Fixture egress".to_string(),
            source_kind: "uploaded".to_string(),
            trust_level: "unverified".to_string(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("installation should persist");
    let provider_id = Uuid::now_v7();
    NetworkEgressRepository::create_network_egress_provider(
        &store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            extension_family: domain::ExtensionCatalogIdentity::new(
                ExtensionCategory::RuntimeExtensions,
                "test",
                "fixture_egress",
            ),
            provider_code: "fixture_egress".to_string(),
            display_name: "Fixture egress".to_string(),
            description: String::new(),
            secret_ref: "secret://system/network-egress/fixture".to_string(),
            lifecycle: domain::NetworkEgressProviderLifecycle::Active,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("provider should persist");
    let plaintext = json!({ "token": "registry-secret-must-not-persist-in-cleartext" });
    let secret = NetworkEgressRepository::upsert_network_egress_provider_secret(
        &store,
        &UpsertNetworkEgressProviderSecretInput {
            provider_id,
            secret_ref: "secret://system/network-egress/fixture".to_string(),
            plaintext_secret_json: plaintext.clone(),
            master_key: "network-egress-test-master-key".to_string(),
            secret_version: 1,
        },
    )
    .await
    .expect("registry-owned secret should persist encrypted");
    assert!(!secret
        .encrypted_secret_json
        .to_string()
        .contains("registry-secret-must-not-persist-in-cleartext"));
    let resolved = NetworkEgressRepository::resolve_network_egress_provider_secret_json(
        &store,
        provider_id,
        "secret://system/network-egress/fixture",
        "network-egress-test-master-key",
    )
    .await
    .expect("secret should resolve with the provisioning key");
    assert_eq!(resolved, Some(plaintext));
    let mismatched_ref = NetworkEgressRepository::resolve_network_egress_provider_secret_json(
        &store,
        provider_id,
        "secret://system/network-egress/wrong-ref",
        "network-egress-test-master-key",
    )
    .await
    .expect("mismatched reference should not decrypt any material");
    assert!(mismatched_ref.is_none());
    let synchronized_at = OffsetDateTime::now_utc();
    NetworkEgressRepository::replace_network_egress_projection(
        &store,
        &ReplaceNetworkEgressProjectionInput {
            provider_id,
            health_status: domain::NetworkEgressHealthStatus::Healthy,
            last_sync_error: None,
            synchronized_at,
            actor_user_id: actor.id,
            egresses: vec![
                domain::NetworkEgressProjectionRecord {
                    provider_id,
                    provider_egress_key: "z-last".to_string(),
                    display_name: "Last".to_string(),
                    region: None,
                    tags: Vec::new(),
                    availability: "available".to_string(),
                    synced_at: synchronized_at,
                },
                domain::NetworkEgressProjectionRecord {
                    provider_id,
                    provider_egress_key: "a-first".to_string(),
                    display_name: "First".to_string(),
                    region: None,
                    tags: Vec::new(),
                    availability: "available".to_string(),
                    synced_at: synchronized_at,
                },
            ],
        },
    )
    .await
    .expect("projection should persist");

    let initial = NetworkEgressRepository::list_network_egress_projections(&store, provider_id)
        .await
        .expect("projection should list");
    assert_eq!(
        initial
            .iter()
            .map(|egress| egress.provider_egress_key.as_str())
            .collect::<Vec<_>>(),
        ["a-first", "z-last"]
    );

    let failed = NetworkEgressRepository::record_network_egress_sync_failure(
        &store,
        &RecordNetworkEgressSyncFailureInput {
            provider_id,
            last_sync_error: "network_egress_sync_failed".to_string(),
            synchronized_at: OffsetDateTime::now_utc(),
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("failure should be projected");
    assert_eq!(
        failed.health_status,
        domain::NetworkEgressHealthStatus::Unhealthy
    );
    let retained = NetworkEgressRepository::list_network_egress_projections(&store, provider_id)
        .await
        .expect("prior projection should remain readable");
    assert_eq!(retained, initial);

    NetworkEgressRepository::delete_network_egress_provider(&store, provider_id)
        .await
        .expect("an initial failed provider must be removable with its private state");
    assert!(
        NetworkEgressRepository::get_network_egress_provider(&store, provider_id)
            .await
            .expect("deleted provider lookup should succeed")
            .is_none()
    );
    assert!(
        NetworkEgressRepository::resolve_network_egress_provider_secret_json(
            &store,
            provider_id,
            "secret://system/network-egress/fixture",
            "network-egress-test-master-key",
        )
        .await
        .expect("deleted provider secret lookup should succeed")
        .is_none()
    );
    assert!(
        NetworkEgressRepository::list_network_egress_projections(&store, provider_id)
            .await
            .expect("deleted provider projection lookup should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn ac_007_pool_member_preserves_missing_provider_reference_without_runtime_lease_fields() {
    let (store, actor) = store().await;
    let pool_id = Uuid::now_v7();
    NetworkEgressPoolRepository::create_network_egress_pool(
        &store,
        &CreateNetworkEgressPoolInput {
            pool_id,
            display_name: "Stable references".to_string(),
            owner_provider_id: None,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("pool should persist");
    let provider_id = Uuid::now_v7();
    let member_id = Uuid::now_v7();
    NetworkEgressPoolRepository::create_network_egress_pool_member(
        &store,
        &CreateNetworkEgressPoolMemberInput {
            member_id,
            pool_id,
            provider_id,
            provider_egress_key: "gone-egress".to_string(),
            enabled: true,
            sequence: 10,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("durable reference should persist even when a provider later disappears");

    let duplicate = NetworkEgressPoolRepository::create_network_egress_pool_member(
        &store,
        &CreateNetworkEgressPoolMemberInput {
            member_id: Uuid::now_v7(),
            pool_id,
            provider_id,
            provider_egress_key: "gone-egress".to_string(),
            enabled: true,
            sequence: 20,
            actor_user_id: actor.id,
        },
    )
    .await;
    assert!(
        duplicate.is_err(),
        "stable pool references must be unique per pool"
    );

    let view = NetworkEgressPoolService::new(store.clone())
        .list()
        .await
        .expect("missing provider should project instead of discarding the member");
    assert_eq!(view[0].members[0].member.id, member_id);
    assert_eq!(
        view[0].members[0].health,
        domain::NetworkEgressPoolMemberHealth::Invalid
    );

    let columns = sqlx::query_scalar::<_, String>(
        r#"
                select column_name from information_schema.columns
                where table_schema = current_schema()
                    and table_name = 'network_egress_pool_members'
                order by column_name
            "#,
    )
    .fetch_all(store.pool())
    .await
    .expect("pool member columns should be readable");
    for forbidden in ["http_proxy_url", "port", "lease_id", "cleanup_token"] {
        assert!(
            !columns.iter().any(|column| column == forbidden),
            "pool members must not persist runtime lease field {forbidden}"
        );
    }
}

/// AC-006/007: a pool stores only durable provider/key references. Runtime proxy material is
/// acquired after this selection, so an unavailable member is skipped and a stale descriptor
/// cannot be selected after a later provider synchronization replaces the projection.
#[tokio::test]
async fn ac_006_ac_007_pool_selection_uses_current_healthy_projection_and_never_persists_lease() {
    let (store, actor) = store().await;
    let installation_id = Uuid::now_v7();
    PluginRepository::upsert_installation(
        &store,
        &UpsertPluginInstallationInput {
            installation_id,
            category: ExtensionCategory::RuntimeExtensions,
            organization: "test".to_string(),
            provider_code: "selection_fixture".to_string(),
            plugin_id: "selection_fixture@0.1.0".to_string(),
            plugin_version: "0.1.0".to_string(),
            contract_version: extension_contracts::NETWORK_EGRESS_PROVIDER_CONTRACT.to_string(),
            protocol: "stdio_json_worker".to_string(),
            display_name: "Selection fixture".to_string(),
            source_kind: "uploaded".to_string(),
            trust_level: "unverified".to_string(),
            verification_status: PluginVerificationStatus::Valid,
            desired_state: PluginDesiredState::ActiveRequested,
            expected_checksum: None,
            signature_status: ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({}),
            is_system_reserved: false,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("fixture installation should persist");
    let provider_id = Uuid::now_v7();
    let created = NetworkEgressRepository::create_network_egress_provider(
        &store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            extension_family: domain::ExtensionCatalogIdentity::new(
                ExtensionCategory::RuntimeExtensions,
                "test",
                "selection_fixture",
            ),
            provider_code: "selection_fixture".to_string(),
            display_name: "Selection fixture".to_string(),
            description: String::new(),
            secret_ref: "secret://system/network-egress/selection".to_string(),
            lifecycle: domain::NetworkEgressProviderLifecycle::Draft,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("provider configuration should persist as draft");
    assert_eq!(
        created.lifecycle,
        domain::NetworkEgressProviderLifecycle::Draft
    );
    let started = NetworkEgressRepository::update_network_egress_provider_lifecycle(
        &store,
        &UpdateNetworkEgressProviderLifecycleInput {
            provider_id,
            lifecycle: domain::NetworkEgressProviderLifecycle::Active,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("provider start should persist its active lifecycle");
    assert_eq!(
        started.lifecycle,
        domain::NetworkEgressProviderLifecycle::Active
    );

    let synced_at = OffsetDateTime::now_utc();
    NetworkEgressRepository::replace_network_egress_projection(
        &store,
        &ReplaceNetworkEgressProjectionInput {
            provider_id,
            health_status: domain::NetworkEgressHealthStatus::Healthy,
            last_sync_error: None,
            synchronized_at: synced_at,
            actor_user_id: actor.id,
            egresses: vec![
                domain::NetworkEgressProjectionRecord {
                    provider_id,
                    provider_egress_key: "unavailable-first".to_string(),
                    display_name: "Unavailable first".to_string(),
                    region: Some("test-a".to_string()),
                    tags: vec!["fixture".to_string()],
                    availability: "unavailable".to_string(),
                    synced_at,
                },
                domain::NetworkEgressProjectionRecord {
                    provider_id,
                    provider_egress_key: "available-second".to_string(),
                    display_name: "Available second".to_string(),
                    region: Some("test-b".to_string()),
                    tags: vec!["fixture".to_string()],
                    availability: "available".to_string(),
                    synced_at,
                },
            ],
        },
    )
    .await
    .expect("provider synchronization should persist the stable projection");

    let pool = ensure_global_network_egress_pool(&store, actor.id)
        .await
        .expect("pool should persist");
    assert_eq!(pool.id, GLOBAL_NETWORK_EGRESS_POOL_ID);
    assert_eq!(pool.selection_strategy.as_str(), "healthy_first");
    let pool_id = pool.id;
    let unavailable = NetworkEgressPoolService::new(store.clone())
        .add_member(CreateNetworkEgressPoolMemberCommand {
            actor_user_id: actor.id,
            pool_id,
            provider_id,
            provider_egress_key: "unavailable-first".to_string(),
            enabled: true,
            sequence: 0,
        })
        .await
        .expect("current but unavailable descriptor may be retained as a durable reference");
    let available = NetworkEgressPoolService::new(store.clone())
        .add_member(CreateNetworkEgressPoolMemberCommand {
            actor_user_id: actor.id,
            pool_id,
            provider_id,
            provider_egress_key: "available-second".to_string(),
            enabled: true,
            sequence: 10,
        })
        .await
        .expect("current available descriptor should join the pool");
    assert_eq!(unavailable.health.as_str(), "unhealthy");
    assert_eq!(available.health.as_str(), "not_tested");

    let selection = NetworkEgressPoolService::new(store.clone())
        .select_healthy_first(pool_id)
        .await
        .expect("an enabled untested member should remain selectable after unhealthy members");
    assert_eq!(selection.member_id, available.member.id);
    assert_eq!(selection.provider_id, provider_id);
    assert_eq!(selection.provider_egress_key, "available-second");

    let route_selection = NetworkEgressPoolService::new(store.clone())
        .select_healthy_first_from(pool_id, &[available.member.id])
        .await
        .expect("route selection must stay inside its explicit proxy mapping");
    assert_eq!(route_selection.member_id, available.member.id);
    let unavailable_route = NetworkEgressPoolService::new(store.clone())
        .select_healthy_first_from(pool_id, &[unavailable.member.id])
        .await
        .expect_err("a route must not fall through to an unbound healthy proxy");
    assert!(unavailable_route
        .to_string()
        .contains("network_egress_pool_unavailable"));

    let workspace_id =
        sqlx::query_scalar::<_, Uuid>("select id from workspaces where name = 'network-egress'")
            .fetch_one(store.pool())
            .await
            .expect("fixture workspace should exist");
    let invalid_mapping = NetworkEgressRouteService::new(store.clone())
        .create(CreateNetworkEgressRouteCommand {
            actor_user_id: actor.id,
            workspace_id,
            selector: domain::NetworkEgressConsumerSelector::ModelProviderDefault,
            pool_member_ids: vec![available.member.id, available.member.id],
            enabled: true,
        })
        .await
        .expect_err("route mappings must be non-empty and unique");
    assert!(invalid_mapping.to_string().contains("pool_member_ids"));
    let route = NetworkEgressRouteService::new(store.clone())
        .create(CreateNetworkEgressRouteCommand {
            actor_user_id: actor.id,
            workspace_id,
            selector: domain::NetworkEgressConsumerSelector::HttpNodeDefault,
            pool_member_ids: vec![available.member.id, unavailable.member.id],
            enabled: true,
        })
        .await
        .expect("ordered route proxy mapping should persist");
    assert_eq!(
        route.pool_member_ids,
        vec![available.member.id, unavailable.member.id]
    );
    assert!(
        NetworkEgressRouteRepository::is_network_egress_pool_member_referenced(
            &store,
            available.member.id
        )
        .await
        .expect("route reference lookup should succeed")
    );
    let delete_referenced = NetworkEgressPoolService::new(store.clone())
        .delete_member(actor.id, pool_id, available.member.id)
        .await
        .expect_err("a proxy referenced by a route must not be deleted");
    assert!(delete_referenced
        .to_string()
        .contains("network_egress_pool_member_in_use"));

    NetworkEgressRepository::replace_network_egress_projection(
        &store,
        &ReplaceNetworkEgressProjectionInput {
            provider_id,
            health_status: domain::NetworkEgressHealthStatus::Healthy,
            last_sync_error: None,
            synchronized_at: OffsetDateTime::now_utc(),
            actor_user_id: actor.id,
            egresses: vec![],
        },
    )
    .await
    .expect("a later sync may invalidate a formerly current descriptor");
    let views = NetworkEgressPoolService::new(store.clone())
        .list()
        .await
        .expect("stale references should remain visible for correction");
    assert!(views[0]
        .members
        .iter()
        .all(|member| member.health == domain::NetworkEgressPoolMemberHealth::Invalid));
    let unavailable_error = NetworkEgressPoolService::new(store.clone())
        .select_healthy_first(pool_id)
        .await
        .expect_err("a stale projection must fail closed before any runtime lease is acquired");
    assert!(unavailable_error
        .to_string()
        .contains("network_egress_pool_unavailable"));

    let stopped = NetworkEgressRepository::update_network_egress_provider_lifecycle(
        &store,
        &UpdateNetworkEgressProviderLifecycleInput {
            provider_id,
            lifecycle: domain::NetworkEgressProviderLifecycle::Disabled,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("provider stop should persist its disabled lifecycle");
    assert_eq!(
        stopped.lifecycle,
        domain::NetworkEgressProviderLifecycle::Disabled
    );

    let columns = sqlx::query_scalar::<_, String>(
        r#"
                select column_name from information_schema.columns
                where table_schema = current_schema()
                    and table_name in ('network_egress_pool_members', 'network_egress_projections')
                order by column_name
            "#,
    )
    .fetch_all(store.pool())
    .await
    .expect("network center storage columns should be readable");
    for forbidden in ["http_proxy_url", "port", "lease_id", "cleanup_token"] {
        assert!(
            !columns.iter().any(|column| column == forbidden),
            "durable control-plane records must not persist runtime lease field {forbidden}"
        );
    }
}

/// AC-NC16: direct pool additions create the static provider, encrypted secret, projection,
/// and pool member as one durable unit without putting proxy material in the pool table.
#[tokio::test]
async fn ac_nc16_static_http_proxy_member_is_atomic_and_keeps_credentials_out_of_pool() {
    let (store, actor) = store().await;
    let pool_id = Uuid::now_v7();
    NetworkEgressPoolRepository::create_network_egress_pool(
        &store,
        &CreateNetworkEgressPoolInput {
            pool_id,
            display_name: "Manual exits".to_string(),
            owner_provider_id: None,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("target pool should persist");
    let rolled_back_provider_id = Uuid::now_v7();
    let failed = NetworkEgressRepository::create_static_http_proxy_pool_member(
        &store,
        &CreateStaticHttpProxyPoolMemberInput {
            provider_id: rolled_back_provider_id,
            member_id: Uuid::now_v7(),
            pool_id: Uuid::now_v7(),
            display_name: "Should roll back".to_string(),
            description: String::new(),
            secret_ref: format!("secret://system/network-egress/{rolled_back_provider_id}"),
            plaintext_secret_json: json!({"host": "198.65.36.212", "port": 37867}),
            master_key: "test-master-key".to_string(),
            enabled: true,
            sequence: 0,
            synchronized_at: OffsetDateTime::now_utc(),
            actor_user_id: actor.id,
        },
    )
    .await;
    assert!(
        failed.is_err(),
        "a nonexistent pool must reject the complete write"
    );
    assert!(
        NetworkEgressRepository::get_network_egress_provider(&store, rolled_back_provider_id)
            .await
            .expect("provider lookup should succeed")
            .is_none(),
        "a failed member write must roll back its newly-created provider and secret"
    );
    let provider_id = Uuid::now_v7();
    let member = NetworkEgressRepository::create_static_http_proxy_pool_member(
        &store,
        &CreateStaticHttpProxyPoolMemberInput {
            provider_id,
            member_id: Uuid::now_v7(),
            pool_id,
            display_name: "US proxy".to_string(),
            description: "Manual proxy for US traffic".to_string(),
            secret_ref: format!("secret://system/network-egress/{provider_id}"),
            plaintext_secret_json: json!({
                "host": "198.65.36.212",
                "port": 37867,
                "username": "proxy-user",
                "password": "proxy-password",
            }),
            master_key: "test-master-key".to_string(),
            enabled: true,
            sequence: 0,
            synchronized_at: OffsetDateTime::now_utc(),
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("static proxy should join its target pool atomically");
    assert_eq!(member.provider_id, provider_id);
    assert_eq!(member.provider_egress_key, "static-http");
    let provider = NetworkEgressRepository::get_network_egress_provider(&store, provider_id)
        .await
        .expect("provider lookup should succeed")
        .expect("static provider should persist");
    assert_eq!(provider.extension_family, None);
    assert_eq!(provider.provider_code, "builtin_static_http");
    assert_eq!(provider.description, "Manual proxy for US traffic");
    let secret = NetworkEgressRepository::resolve_network_egress_provider_secret_json(
        &store,
        provider_id,
        &provider.secret_ref,
        "test-master-key",
    )
    .await
    .expect("secret should decrypt")
    .expect("secret should exist");
    assert_eq!(secret["password"], "proxy-password");
    let columns = sqlx::query_scalar::<_, String>(
        r#"
                select column_name from information_schema.columns
                where table_schema = current_schema()
                  and table_name = 'network_egress_pool_members'
                order by column_name
            "#,
    )
    .fetch_all(store.pool())
    .await
    .expect("pool member columns should be readable");
    assert!(!columns.iter().any(|column| column == "password"));
}

#[tokio::test]
async fn nc_09_route_storage_keeps_closed_selector_identity_and_workspace_instance_boundary() {
    let (store, actor) = store().await;
    let workspace_id =
        sqlx::query_scalar::<_, Uuid>("select id from workspaces where name = 'network-egress'")
            .fetch_one(store.pool())
            .await
            .expect("fixture workspace should exist");
    let pool_id = Uuid::now_v7();
    NetworkEgressPoolRepository::create_network_egress_pool(
        &store,
        &CreateNetworkEgressPoolInput {
            pool_id,
            display_name: "Route target".to_string(),
            owner_provider_id: None,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("route target pool should persist");

    let route = NetworkEgressRouteRepository::create_network_egress_route(
        &store,
        &CreateNetworkEgressRouteInput {
            route_id: Uuid::now_v7(),
            workspace_id,
            selector: domain::NetworkEgressConsumerSelector::GithubOfficialSources,
            pool_id,
            pool_member_ids: vec![],
            enabled: true,
            actor_user_id: actor.id,
        },
    )
    .await
    .expect("github selector should persist without a free reference");
    assert_eq!(
        route.selector,
        domain::NetworkEgressConsumerSelector::GithubOfficialSources
    );

    let duplicate = NetworkEgressRouteRepository::create_network_egress_route(
        &store,
        &CreateNetworkEgressRouteInput {
            route_id: Uuid::now_v7(),
            workspace_id,
            selector: domain::NetworkEgressConsumerSelector::GithubOfficialSources,
            pool_id,
            pool_member_ids: vec![],
            enabled: false,
            actor_user_id: actor.id,
        },
    )
    .await;
    assert!(
        duplicate.is_err(),
        "one workspace can have only one github selector"
    );

    let unknown_instance = sqlx::query(
        r#"
                insert into network_egress_routes (
                    id, workspace_id, consumer_kind, consumer_reference, pool_id, enabled,
                    failure_policy, created_by, updated_by
                ) values ($1, $2, 'model_provider', $3, $4, true, 'block', $5, $5)
            "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(Uuid::now_v7())
    .bind(pool_id)
    .bind(actor.id)
    .execute(store.pool())
    .await;
    assert!(
        unknown_instance.is_err(),
        "an exact model provider selector must belong to the route workspace"
    );
}
