use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, seed_workspace, test_app_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use domain::PermissionDefinition;
use serde_json::{json, Value};
use tower::ServiceExt;

const MEMBERS_FEATURE_PERMISSION: &str = "settings_feature.access.system.members";

async fn register_members_feature_permission(database_url: &str) {
    let store = storage_durable::build_main_durable_postgres(database_url)
        .await
        .expect("test database should be available")
        .store;

    store
        .upsert_permission_catalog(&[PermissionDefinition {
            code: MEMBERS_FEATURE_PERMISSION.to_string(),
            resource: "settings_feature".to_string(),
            action: "access".to_string(),
            scope: "system.members".to_string(),
            name: "settings_feature:access:system.members".to_string(),
        }])
        .await
        .expect("members feature permission should be seeded");
}

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("response body should be readable");
    serde_json::from_slice(&body).expect("response should be JSON")
}

// AC-003: a role with only system.members may use the complete member-role use case.
#[tokio::test]
async fn members_feature_only_lists_role_options_and_replaces_member_roles_within_scope() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "assignable").await;
    create_role(&app, &root_cookie, &root_csrf, "members_feature_only").await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "members-feature-actor",
        "temp-pass",
    )
    .await;
    let target_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "members-feature-target",
        "temp-pass",
    )
    .await;
    register_members_feature_permission(&database_url).await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "members_feature_only",
        &[MEMBERS_FEATURE_PERMISSION],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["members_feature_only"],
    )
    .await;
    let (actor_cookie, actor_csrf) =
        login_and_capture_cookie(&app, "members-feature-actor", "temp-pass").await;

    let options_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/members/role-options")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(options_response.status(), StatusCode::OK);
    let options = response_json(options_response).await;
    let assignable = options["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|role| role["code"] == "assignable")
        .expect("current-scope role should be assignable");
    assert_eq!(assignable["name"], "assignable");
    assert_eq!(assignable.as_object().unwrap().len(), 2);
    assert!(!options["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|role| role["code"] == "root"));

    let replace_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{target_id}/roles"))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "role_codes": ["assignable"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(replace_response.status(), StatusCode::NO_CONTENT);

    let members_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/members")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(members_response.status(), StatusCode::OK);
    let members = response_json(members_response).await;
    let target = members["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|member| member["id"] == target_id)
        .expect("target member should stay visible in current scope");
    assert_eq!(target["role_codes"], json!(["assignable"]));

    let root_role_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{target_id}/roles"))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "role_codes": ["root"] }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_role_response.status(), StatusCode::BAD_REQUEST);

    let outside_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "outside-member",
        "temp-pass",
    )
    .await;
    let outside_workspace_id = seed_workspace(&database_url, "Outside workspace").await;
    let store = storage_durable::build_main_durable_postgres(&database_url)
        .await
        .unwrap()
        .store;
    sqlx::query("update workspace_memberships set workspace_id = $1 where user_id = $2")
        .bind(outside_workspace_id)
        .bind(uuid::Uuid::parse_str(&outside_id).unwrap())
        .execute(store.pool())
        .await
        .unwrap();

    let outside_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{outside_id}/roles"))
                .header("cookie", &actor_cookie)
                .header("x-csrf-token", &actor_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "role_codes": ["assignable"] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(outside_response.status(), StatusCode::BAD_REQUEST);
}

// AC-002/AC-003: missing feature grants and unregistered settings routes fail closed.
#[tokio::test]
async fn members_settings_routes_without_feature_or_registration_are_forbidden() {
    let (app, _) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_role(&app, &root_cookie, &root_csrf, "no_members_feature").await;
    let actor_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "no-members-feature",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &actor_id,
        &["no_members_feature"],
    )
    .await;
    let (actor_cookie, _) = login_and_capture_cookie(&app, "no-members-feature", "temp-pass").await;

    let denied = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/members")
                .header("cookie", &actor_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(denied.status(), StatusCode::FORBIDDEN);

    let unregistered = app
        .oneshot(
            Request::builder()
                .uri("/api/console/settings/members/unregistered")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unregistered.status(), StatusCode::FORBIDDEN);
}
