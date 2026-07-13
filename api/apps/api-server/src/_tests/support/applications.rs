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
        captured_at: OffsetDateTime::from_unix_timestamp(1_700_000_120).unwrap(),
        service: service.to_string(),
        service_version: "0.1.0".to_string(),
        service_status: "ok".to_string(),
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
