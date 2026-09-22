use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{header, Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;
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
async fn portable_template_routes_enforce_independent_grants_and_reject_invalid_package() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root, &csrf, "template_reader").await;
    let member = create_member(&app, &root, &csrf, "template-reader", "temp-pass").await;
    replace_member_roles(&app, &root, &csrf, &member, &["template_reader"]).await;
    let (cookie, member_csrf) =
        login_and_capture_cookie(&app, "template-reader", "temp-pass").await;
    for granted in [false, true, false] {
        replace_backup_policy(
            &app,
            &root,
            &csrf,
            "template_reader",
            &[("system_templates.catalog", granted)],
        )
        .await;
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .uri("/api/console/settings/system-templates/catalog")
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(
            response.status(),
            if granted {
                StatusCode::OK
            } else {
                StatusCode::FORBIDDEN
            }
        );
        if granted {
            let payload: Value =
                serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                    .unwrap();
            assert!(payload["data"]["pages"].is_array());
            assert!(payload["data"]["applications"].is_array());
            assert!(payload["data"]["data_models"].is_array());
        }
    }
    let package = json!({"schema_version":"invalid","pages":[],"applications":[],"data_models":[],"plugins":[]});
    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-templates/install")
                .header("cookie", &cookie)
                .header("x-csrf-token", &member_csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-templates/preview")
                .header("cookie", &root)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let payload: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(payload["data"]["valid"], false);
    assert!(payload["data"]["failures"]
        .as_array()
        .unwrap()
        .iter()
        .any(|v| v == "portable_template_schema_version"));
    let rejected = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/system-templates/install")
                .header("cookie", &root)
                .header("x-csrf-token", &csrf)
                .header(header::CONTENT_TYPE, "application/json")
                .body(Body::from(package.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(rejected.status(), StatusCode::BAD_REQUEST);
}
