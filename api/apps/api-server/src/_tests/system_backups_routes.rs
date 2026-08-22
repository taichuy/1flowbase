use std::convert::Infallible;

use axum::{
    body::{to_bytes, Body, Bytes},
    http::{header, Request, StatusCode},
};
use futures_util::{stream, StreamExt};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url, test_config,
};

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_backup(app: &axum::Router, cookie: &str, csrf: &str) -> Uuid {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::ACCEPTED, "{payload}");
    let backup_job_id =
        Uuid::parse_str(payload["data"]["backup_job_id"].as_str().unwrap()).unwrap();
    let backup_set_id =
        Uuid::parse_str(payload["data"]["backup_set_id"].as_str().unwrap()).unwrap();

    let status_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/system-backups/jobs/{backup_job_id}"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(status_response.status(), StatusCode::OK);
    let status_payload = response_json(status_response).await;
    assert_eq!(
        status_payload["data"]["backup_job_id"],
        backup_job_id.to_string()
    );
    assert_eq!(
        status_payload["data"]["backup_set_id"],
        backup_set_id.to_string()
    );
    assert!(status_payload["data"]["status"].is_string());
    assert!(status_payload["data"]["sealed_components"].is_u64());
    assert!(status_payload["data"].get("failure_code").is_some());

    for _ in 0..300 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/console/settings/system-backups/jobs/{backup_job_id}"
                    ))
                    .header("cookie", cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        let status_payload = response_json(response).await;
        match status_payload["data"]["status"].as_str() {
            Some("succeeded") => return backup_set_id,
            Some("failed") => panic!("queued backup failed: {status_payload}"),
            _ => tokio::time::sleep(std::time::Duration::from_millis(20)).await,
        }
    }
    panic!("queued backup did not complete before the route test timeout")
}

async fn preflight(app: &axum::Router, cookie: &str, csrf: &str, backup_set_id: Uuid) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/system-backups/{backup_set_id}/recovery/preflight"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await["data"].clone()
}

async fn issue_challenge(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    backup_set_id: Uuid,
    plan_digest: &str,
) -> Uuid {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups/recovery/reauth")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "backup_set_id": backup_set_id,
                        "exact_backup_name": backup_set_id.to_string(),
                        "plan_digest": plan_digest,
                        "password": "change-me"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    Uuid::parse_str(
        response_json(response).await["data"]["challenge_token"]
            .as_str()
            .unwrap(),
    )
    .unwrap()
}

#[tokio::test]
async fn system_backup_routes_return_unavailable_when_postgresql_tools_are_missing() {
    let (state, _) = test_api_state_with_database_url().await;
    let mut unavailable_state = (*state).clone();
    unavailable_state.system_backup = None;
    let app =
        crate::app_with_state_and_config(std::sync::Arc::new(unavailable_state), &test_config());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/system-backups")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let payload = response_json(response).await;

    assert_eq!(status, StatusCode::SERVICE_UNAVAILABLE, "{payload}");
    assert_eq!(payload["code"], "system_backup_unavailable");
}

#[tokio::test]
async fn system_backup_queue_rejects_a_second_maintenance_owner() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let first = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first.status(), StatusCode::ACCEPTED);
    let second = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second.status(), StatusCode::CONFLICT);
}

async fn replace_backup_policy(
    app: &axum::Router,
    root_cookie: &str,
    root_csrf: &str,
    role_code: &str,
    operations: &[(&str, bool)],
) {
    let operations = operations
        .iter()
        .map(|(operation_id, enabled)| {
            json!({
                "kind": "simple",
                "operation_id": operation_id,
                "enabled": enabled
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
                .header("cookie", root_cookie)
                .header("x-csrf-token", root_csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "groups": [{
                            "kind": "settings_feature",
                            "group_id": "system.backups",
                            "enabled": true,
                            "strategy": "custom",
                            "operations": operations
                        }]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn system_backups_route_enforces_cookie_csrf_detail_projection_and_chunked_streams() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());

    let anonymous = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/system-backups")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anonymous.status(), StatusCode::UNAUTHORIZED);

    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let backup_set_id = create_backup(&app, &cookie, &csrf).await;
    let detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/system-backups/{backup_set_id}"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let detail_status = detail.status();
    let detail = response_json(detail).await;
    assert_eq!(detail_status, StatusCode::OK, "{detail}");
    let detail = &detail["data"];
    assert!(detail.get("sealed_manifest").is_none());
    assert_eq!(detail["compatibility"]["compatible"], true);
    assert_eq!(detail["verification"]["verified"], true);
    assert!(detail["verification"]["checked_at"].is_string());
    assert!(detail["content"]["component_count"].as_u64().unwrap() >= 1);
    assert_eq!(detail["content"]["postgresql_count"], 1);
    assert!(detail["components"]
        .as_array()
        .unwrap()
        .iter()
        .any(|component| {
            component["kind"] == "postgre_sql"
                && component["component_id"] == "postgresql"
                && component["content_digest"].is_string()
        }));
    assert!(detail["creation_journal"]
        .as_array()
        .unwrap()
        .iter()
        .any(|event| { event["state"] == "succeeded" }));
    assert!(detail["recovery_history"].as_array().unwrap().is_empty());

    let download = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/system-backups/{backup_set_id}/download"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(download.status(), StatusCode::OK);
    assert_eq!(
        download.headers().get(header::CONTENT_TYPE).unwrap(),
        "application/octet-stream"
    );
    assert!(download
        .headers()
        .get(header::CONTENT_DISPOSITION)
        .unwrap()
        .to_str()
        .unwrap()
        .contains(&backup_set_id.to_string()));
    let mut archive = Vec::new();
    let mut chunk_count = 0;
    let mut download_stream = download.into_body().into_data_stream();
    while let Some(chunk) = download_stream.next().await {
        let chunk = chunk.unwrap();
        assert!(chunk.len() <= 64 * 1024);
        archive.extend_from_slice(&chunk);
        chunk_count += 1;
    }
    assert!(archive.len() > 1024 * 1024);
    assert!(chunk_count > 1);

    let wrong_content_type = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_content_type.status(), StatusCode::BAD_REQUEST);

    let upload_chunks = archive
        .chunks(8192)
        .map(|chunk| Ok::<Bytes, Infallible>(Bytes::copy_from_slice(chunk)))
        .collect::<Vec<_>>();
    let chunked_import = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from_stream(stream::iter(upload_chunks)))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(chunked_import.status(), StatusCode::OK);
    assert_eq!(
        response_json(chunked_import).await["data"]["backup_set_id"],
        backup_set_id.to_string()
    );

    let system_backup = state
        .system_backup
        .as_ref()
        .expect("test system backup runtime should be available");
    let before = system_backup.list().await.unwrap().len();
    let truncated_import = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/octet-stream")
                .body(Body::from(archive[..7].to_vec()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(truncated_import.status(), StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(system_backup.list().await.unwrap().len(), before);

    let missing_backup = Uuid::now_v7();
    let preflight = preflight(&app, &cookie, &csrf, missing_backup).await;
    assert_eq!(preflight["compatible"], false);
    assert!(!preflight["failures"].as_array().unwrap().is_empty());
    assert_eq!(system_backup.list().await.unwrap().len(), before);
    assert_eq!(
        system_backup.maintenance_status().phase,
        control_plane::system_recovery::SystemMaintenancePhase::Online
    );
}

#[tokio::test]
async fn system_backup_create_requires_backup_job_status_access_before_queuing() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "backup_creator").await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "backup-creator",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["backup_creator"],
    )
    .await;
    replace_backup_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "backup_creator",
        &[("system_backups.create", true)],
    )
    .await;
    let (creator_cookie, creator_csrf) =
        login_and_capture_cookie(&app, "backup-creator", "temp-pass").await;

    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups")
                .header("cookie", &creator_cookie)
                .header("x-csrf-token", &creator_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    replace_backup_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "backup_creator",
        &[
            ("system_backups.create", true),
            ("system_backups.status", true),
        ],
    )
    .await;
    let allowed = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups")
                .header("cookie", creator_cookie)
                .header("x-csrf-token", creator_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::ACCEPTED);
}

#[tokio::test]
async fn system_backups_operation_grants_revoke_live_and_recovery_remains_root_only() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "backup_reader").await;
    let member_id =
        create_member(&app, &root_cookie, &root_csrf, "backup-reader", "temp-pass").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["backup_reader"],
    )
    .await;
    replace_backup_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "backup_reader",
        &[("system_backups.list", true)],
    )
    .await;
    let (reader_cookie, reader_csrf) =
        login_and_capture_cookie(&app, "backup-reader", "temp-pass").await;

    let allowed = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/system-backups")
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(allowed.status(), StatusCode::OK);

    let denied_detail = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/system-backups/{}",
                    Uuid::now_v7()
                ))
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied_detail.status(), StatusCode::FORBIDDEN);

    replace_backup_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "backup_reader",
        &[("system_backups.list", false)],
    )
    .await;
    let revoked = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/system-backups")
                .header("cookie", &reader_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(revoked.status(), StatusCode::FORBIDDEN);

    replace_backup_policy(
        &app,
        &root_cookie,
        &root_csrf,
        "backup_reader",
        &[("system_backups.recovery.preflight", true)],
    )
    .await;
    let non_root_recovery = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/system-backups/{}/recovery/preflight",
                    Uuid::now_v7()
                ))
                .header("cookie", &reader_cookie)
                .header("x-csrf-token", &reader_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(non_root_recovery.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn system_backups_reauth_binds_session_backup_plan_name_expiry_and_single_use() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let first_backup = create_backup(&app, &cookie, &csrf).await;
    let second_backup = create_backup(&app, &cookie, &csrf).await;
    let first_plan = preflight(&app, &cookie, &csrf, first_backup).await;
    let second_plan = preflight(&app, &cookie, &csrf, second_backup).await;
    assert_eq!(first_plan["compatible"], true);
    assert_eq!(second_plan["compatible"], true);
    let first_digest = first_plan["plan_digest"].as_str().unwrap();
    let second_digest = second_plan["plan_digest"].as_str().unwrap();

    let wrong_name = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups/recovery/reauth")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "backup_set_id": first_backup,
                        "exact_backup_name": "wrong-name",
                        "plan_digest": first_digest,
                        "password": "change-me"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_name.status(), StatusCode::BAD_REQUEST);

    let wrong_plan = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups/recovery/reauth")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "backup_set_id": first_backup,
                        "exact_backup_name": first_backup.to_string(),
                        "plan_digest": "0".repeat(64),
                        "password": "change-me"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_plan.status(), StatusCode::CONFLICT);

    let wrong_password = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-backups/recovery/reauth")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "backup_set_id": first_backup,
                        "exact_backup_name": first_backup.to_string(),
                        "plan_digest": first_digest,
                        "password": "wrong-password"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(wrong_password.status(), StatusCode::FORBIDDEN);

    let session_bound = issue_challenge(&app, &cookie, &csrf, first_backup, first_digest).await;
    let (second_cookie, second_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let wrong_session = recovery_intent_request(
        &app,
        &second_cookie,
        &second_csrf,
        first_backup,
        first_digest,
        session_bound,
    )
    .await;
    assert_eq!(wrong_session.status(), StatusCode::FORBIDDEN);

    let backup_bound = issue_challenge(&app, &cookie, &csrf, first_backup, first_digest).await;
    let wrong_backup = recovery_intent_request(
        &app,
        &cookie,
        &csrf,
        second_backup,
        second_digest,
        backup_bound,
    )
    .await;
    assert_eq!(wrong_backup.status(), StatusCode::FORBIDDEN);

    let expired = issue_challenge(&app, &cookie, &csrf, first_backup, first_digest).await;
    crate::recovery_authorization::expire_reauth_challenge(expired);
    let expired_response =
        recovery_intent_request(&app, &cookie, &csrf, first_backup, first_digest, expired).await;
    assert_eq!(expired_response.status(), StatusCode::FORBIDDEN);

    let replayed = issue_challenge(&app, &cookie, &csrf, first_backup, first_digest).await;
    crate::recovery_authorization::mark_reauth_challenge_consumed(replayed);
    let replay_response =
        recovery_intent_request(&app, &cookie, &csrf, first_backup, first_digest, replayed).await;
    assert_eq!(replay_response.status(), StatusCode::FORBIDDEN);

    let accepted = issue_challenge(&app, &cookie, &csrf, first_backup, first_digest).await;
    let accepted_response =
        recovery_intent_request(&app, &cookie, &csrf, first_backup, first_digest, accepted).await;
    assert_eq!(accepted_response.status(), StatusCode::ACCEPTED);
    let accepted_payload = response_json(accepted_response).await;
    assert_eq!(
        accepted_payload["data"]["backup_set_id"],
        first_backup.to_string()
    );
    assert_eq!(accepted_payload["data"]["status"], "preparing");
}

async fn recovery_intent_request(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    backup_set_id: Uuid,
    plan_digest: &str,
    challenge_token: Uuid,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/system-backups/{backup_set_id}/recovery/intents"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(
                    json!({
                        "challenge_token": challenge_token,
                        "exact_backup_name": backup_set_id.to_string(),
                        "plan_digest": plan_digest
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap()
}
