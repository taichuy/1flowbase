use crate::_tests::support::{login_and_capture_cookie, test_app_with_database_url};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use sqlx::PgPool;
use tower::ServiceExt;
use uuid::Uuid;

async fn create_application(app: &axum::Router, cookie: &str, csrf: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Agent Support",
                        "description": "",
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let payload: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn seed_js_dependency_pack(
    database_url: &str,
    workspace_assigned: bool,
    version: &str,
) -> Uuid {
    let pool = PgPool::connect(database_url).await.unwrap();
    let workspace_id: Uuid =
        sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
            .fetch_one(&pool)
            .await
            .unwrap();
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account = 'root' limit 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();

    sqlx::query(
        r#"
        insert into plugin_installations (
            id, provider_code, plugin_id, plugin_version, contract_version, protocol,
            display_name, source_kind, trust_level, verification_status, desired_state,
            artifact_status, runtime_status, availability_status, package_path, installed_path,
            checksum, manifest_fingerprint, signature_status, signature_algorithm, signing_key_id,
            last_load_error, metadata_json, created_by
        ) values (
            $1, $2, $3, $4, '1flowbase.capability/v1', 'stdio_json',
            'Fixture JS Dependency Pack', 'uploaded', 'checksum_only', 'valid', 'active_requested',
            'ready', 'inactive', 'available', null, $5, null, null, 'unsigned', null, null,
            null, $6, $7
        )
        "#,
    )
    .bind(installation_id)
    .bind(format!("fixture_js_dependency_pack_{version}"))
    .bind(format!("fixture_js_dependency_pack@{version}"))
    .bind(version)
    .bind(format!("/tmp/plugins/fixture_js_dependency_pack/{version}"))
    .bind(json!({}))
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into js_dependency_registry (
            id, installation_id, provider_code, plugin_id, plugin_version, alias, package,
            version, target, artifact_path, integrity, permission_network,
            permission_filesystem, permission_env
        ) values ($1, $2, $3, $4, $5, 'zod', 'zod', $6, 'backend_code', $7, $8,
            'outbound_only', 'deny', 'deny')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(format!("fixture_js_dependency_pack_{version}"))
    .bind(format!("fixture_js_dependency_pack@{version}"))
    .bind(version)
    .bind(version)
    .bind(format!("artifacts/zod-{version}.backend.mjs"))
    .bind(format!("sha256-zod-{version}"))
    .execute(&pool)
    .await
    .unwrap();

    if workspace_assigned {
        sqlx::query(
            r#"
            insert into plugin_assignments (
                id, installation_id, workspace_id, provider_code, assigned_by
            ) values ($1, $2, $3, $4, $5)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(installation_id)
        .bind(workspace_id)
        .bind(format!("fixture_js_dependency_pack_{version}"))
        .bind(actor_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    installation_id
}

#[tokio::test]
async fn application_js_dependency_routes_replace_and_list_selection() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_application(&app, &cookie, &csrf).await;
    let zod_v3 = seed_js_dependency_pack(&database_url, true, "3.24.0").await;
    let zod_v4 = seed_js_dependency_pack(&database_url, true, "4.0.0").await;
    let hidden = seed_js_dependency_pack(&database_url, false, "5.0.0").await;

    let blocked = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/js-dependencies"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "installation_id": hidden.to_string(),
                        "alias": "zod",
                        "target": "backend_code"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked.status(), StatusCode::NOT_FOUND);

    for installation_id in [zod_v3, zod_v4] {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("PUT")
                    .uri(format!(
                        "/api/console/applications/{application_id}/js-dependencies"
                    ))
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "installation_id": installation_id.to_string(),
                            "alias": "zod",
                            "target": "backend_code"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
    }

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/js-dependencies"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(list.into_body(), usize::MAX).await.unwrap()).unwrap();
    let selections = payload["data"].as_array().unwrap();

    assert_eq!(selections.len(), 1);
    assert_eq!(selections[0]["package"].as_str(), Some("zod"));
    assert_eq!(selections[0]["version"].as_str(), Some("4.0.0"));
    assert_eq!(
        selections[0]["artifact_hash"].as_str(),
        Some("sha256-zod-4.0.0")
    );
    assert_eq!(
        selections[0]["permissions"]["network"].as_str(),
        Some("outbound_only")
    );
}
