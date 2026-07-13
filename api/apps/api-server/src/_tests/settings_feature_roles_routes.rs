use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

const ROLES_FEATURE_PERMISSION: &str = "settings_feature.access.system.roles";

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

// AC-003: system.roles alone owns the complete role-settings use case.
#[tokio::test]
async fn roles_feature_only_completes_role_crud_and_permission_configuration() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "roles_feature_only").await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "roles-feature-actor",
        "temp-pass",
    )
    .await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "roles_feature_only",
        &[ROLES_FEATURE_PERMISSION],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["roles_feature_only"],
    )
    .await;
    let (actor_cookie, actor_csrf) =
        login_and_capture_cookie(&app, "roles-feature-actor", "temp-pass").await;

    let list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_response.status(), StatusCode::OK);

    let permission_options_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles/permission-options")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(permission_options_response.status(), StatusCode::OK);
    let permission_options = response_json(permission_options_response).await;
    assert!(permission_options["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|permission| permission["code"] == "application.view.own"));

    let data_model_options_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles/data-model-options")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(data_model_options_response.status(), StatusCode::OK);
    let data_model_options = response_json(data_model_options_response).await;
    assert!(data_model_options["data"].as_array().is_some_and(|models| {
        models.iter().all(|model| {
            model.get("id").is_some() && model.get("code").is_some() && model.get("title").is_some()
        })
    }));

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "settings_managed",
                        "name": "Settings managed",
                        "introduction": "managed by feature-only actor"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);

    let replace_permissions_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/settings_managed/permissions")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "permission_codes": ["application.view.own"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        replace_permissions_response.status(),
        StatusCode::NO_CONTENT
    );

    let get_permissions_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles/settings_managed/permissions")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_permissions_response.status(), StatusCode::OK);
    let permissions = response_json(get_permissions_response).await;
    assert_eq!(
        permissions["data"]["permission_codes"],
        json!(["application.view.own"])
    );

    let replace_policy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/settings_managed/data-policy")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "default_policy": {
                            "can_view": true,
                            "can_create": false,
                            "can_update": false,
                            "can_delete": false,
                            "default_view_scope": "scope_all",
                            "default_update_scope": "own",
                            "default_delete_scope": "own"
                        },
                        "model_policies": []
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace_policy_response.status(), StatusCode::OK);

    let frontstage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/roles/settings_managed/frontstage-routes")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "page_ids": [], "tab_ids": [] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(frontstage_response.status(), StatusCode::NO_CONTENT);

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri("/api/console/settings/roles/settings_managed")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "name": "Settings managed next",
                        "introduction": "updated"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::NO_CONTENT);

    let delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/roles/settings_managed")
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_response.status(), StatusCode::NO_CONTENT);

    let old_route = app
        .oneshot(
            Request::builder()
                .uri("/api/console/roles")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(old_route.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn legacy_role_actions_without_roles_feature_are_forbidden() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "legacy_role_actions").await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy-role-action-actor",
        "temp-pass",
    )
    .await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "legacy_role_actions",
        &["role_permission.view.all", "role_permission.manage.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["legacy_role_actions"],
    )
    .await;
    let (actor_cookie, _) =
        login_and_capture_cookie(&app, "legacy-role-action-actor", "temp-pass").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/roles")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::FORBIDDEN);
}
