use super::*;
use storage_durable::MainDurableStore;

async fn main_durable_store(database_url: &str) -> MainDurableStore {
    storage_durable::build_main_durable_postgres(database_url)
        .await
        .unwrap()
        .store
}

pub async fn seed_workspace(database_url: &str, workspace_name: &str) -> Uuid {
    let store = main_durable_store(database_url).await;
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();

    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, $2, $3, null, null)",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .bind(workspace_name)
    .execute(store.pool())
    .await
    .unwrap();

    workspace_id
}

pub(super) fn sample_runtime_profile(service: &str, host_fingerprint: &str) -> RuntimeProfile {
    let captured_at = OffsetDateTime::from_unix_timestamp(1_700_000_120).unwrap();
    let (related_process_bytes, related_process_count) = match service {
        "api-server" => (320 * 1024 * 1024, 2),
        "plugin-runner" => (448 * 1024 * 1024, 3),
        _ => (256 * 1024 * 1024, 1),
    };
    RuntimeProfile {
        host_fingerprint: host_fingerprint.to_string(),
        platform: RuntimePlatform {
            os: "linux".to_string(),
            arch: "amd64".to_string(),
            libc: Some("musl".to_string()),
            rust_target: "x86_64-unknown-linux-musl".to_string(),
        },
        cpu: RuntimeCpu { logical_count: 8 },
        memory: RuntimeMemory::from_bytes(
            16 * 1024 * 1024 * 1024,
            8 * 1024 * 1024 * 1024,
            256 * 1024 * 1024,
        ),
        uptime_seconds: 42,
        started_at: OffsetDateTime::from_unix_timestamp(1_700_000_000).unwrap(),
        captured_at,
        service: service.to_string(),
        service_version: "0.1.0".to_string(),
        service_status: "ok".to_string(),
        metrics: RuntimeMetricsSnapshot {
            captured_at,
            sample_interval_milliseconds: Some(2_000),
            cpu: RuntimeCpuMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::Host,
                usage_percent: Some(12.5),
                logical_count: 8,
                limit_cores: 8.0,
            },
            memory: RuntimeMemoryMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::Host,
                total_bytes: 16 * 1024 * 1024 * 1024,
                available_bytes: 8 * 1024 * 1024 * 1024,
                used_bytes: 8 * 1024 * 1024 * 1024,
                process_bytes: 256 * 1024 * 1024,
                related_process_bytes,
                related_process_count,
                cgroup_composition: None,
            },
            storage: RuntimeStorageMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
                mount_point: Some("/".to_string()),
                file_system: Some("overlay".to_string()),
                total_bytes: Some(64 * 1024 * 1024 * 1024),
                available_bytes: Some(48 * 1024 * 1024 * 1024),
                used_bytes: Some(16 * 1024 * 1024 * 1024),
            },
            network: RuntimeNetworkMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
                received_bytes_per_second: Some(2_048.0),
                transmitted_bytes_per_second: Some(1_024.0),
            },
            disk_io: RuntimeDiskIoMetrics {
                availability: RuntimeMetricAvailability::Available,
                scope_kind: RuntimeMetricScopeKind::RuntimeVisible,
                read_bytes_per_second: Some(4_096.0),
                written_bytes_per_second: Some(8_192.0),
            },
        },
    }
}

pub(crate) async fn create_member(
    app: &Router,
    cookie: &str,
    csrf: &str,
    account: &str,
    password: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/members")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "account": account,
                        "email": format!("{account}@example.com"),
                        "phone": null,
                        "password": password,
                        "name": account,
                        "nickname": account,
                        "introduction": "",
                        "email_login_enabled": true,
                        "phone_login_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: serde_json::Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

pub(crate) async fn create_role(app: &Router, cookie: &str, csrf: &str, code: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": code,
                        "name": code,
                        "introduction": "system runtime test role"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

pub(crate) async fn replace_role_permissions(
    app: &Router,
    cookie: &str,
    csrf: &str,
    role_code: &str,
    permission_codes: &[&str],
) {
    replace_role_legacy_permissions_only(app, cookie, csrf, role_code, permission_codes).await;
    project_legacy_permissions_to_console_policy(app, cookie, csrf, role_code, permission_codes)
        .await;
}

pub(crate) async fn replace_role_legacy_permissions_only(
    app: &Router,
    cookie: &str,
    csrf: &str,
    role_code: &str,
    permission_codes: &[&str],
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/roles/{role_code}/permissions"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "permission_codes": permission_codes,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

pub(crate) async fn project_legacy_permissions_to_console_policy(
    app: &Router,
    cookie: &str,
    csrf: &str,
    role_code: &str,
    permission_codes: &[&str],
) {
    let settings_feature_registry = crate::app_state::compile_core_settings_feature_registry()
        .expect("test settings feature registry should compile");
    let console_operation_registry =
        crate::app_state::compile_core_console_operation_registry(&settings_feature_registry)
            .expect("test console operation registry should compile");
    let migration = crate::console_policy_migration::compile_core_console_policy_migration_plan(
        console_operation_registry.inventory(),
    )
    .expect("test console policy migration should compile");
    let known_legacy_grants = migration
        .legacy_mappings()
        .iter()
        .map(|mapping| mapping.legacy_grant.as_str())
        .collect::<std::collections::BTreeSet<_>>();
    let source_grants = permission_codes
        .iter()
        .filter(|permission| known_legacy_grants.contains(**permission))
        .map(|permission| (*permission).to_string())
        .collect::<Vec<_>>();
    let projection = migration
        .plan()
        .project_legacy_role(Uuid::nil(), &source_grants)
        .expect("test legacy permissions should project into console policy");
    let groups = projection
        .policy
        .groups()
        .iter()
        .map(|policy| {
            json!({
                "kind": policy.group().kind().as_str(),
                "group_id": policy.group().group_id().as_str(),
                "mode": policy.mode().as_str(),
                "operations": policy.operations(),
            })
        })
        .collect::<Vec<_>>();
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/roles/{role_code}/console-policy"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "groups": groups }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

pub(crate) async fn replace_member_roles(
    app: &Router,
    cookie: &str,
    csrf: &str,
    member_id: &str,
    role_codes: &[&str],
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{member_id}/roles"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "role_codes": role_codes,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

pub(super) async fn set_user_preferred_locale(
    database_url: &str,
    account: &str,
    locale: Option<&str>,
) {
    let store = main_durable_store(database_url).await;
    sqlx::query("update users set preferred_locale = $1 where account = $2")
        .bind(locale)
        .bind(account)
        .execute(store.pool())
        .await
        .unwrap();
}
