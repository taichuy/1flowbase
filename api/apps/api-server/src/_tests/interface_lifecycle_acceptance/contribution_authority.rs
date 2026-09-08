//! Root 2007 AUTH-01/02: real route bindings and persistent host authority.
use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_api_state_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::{
    CreatePluginAssignmentInput, PluginRepository, UpsertPluginInstallationInput,
};
use serde_json::{json, Value};
use tower::ServiceExt;
use uuid::Uuid;

async fn request(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    path: &str,
    method: &str,
    body: Value,
) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method(method)
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    (status, serde_json::from_slice(&body).unwrap())
}

#[tokio::test]
async fn root_2007_contribution_authority_routes_are_independent() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let raw = include_str!(
        "../../../../../crates/extension-package-runtime/src/_tests/managed_manifest.yaml"
    )
    .replace(
        "point_id: acme.compute",
        "point_id: 1flowbase.model-definitions.create.before",
    )
    .replace(
        "contract_version: acme.compute/v1",
        "contract_version: 1\n        required_permissions: [hook.model_definitions.create.before]",
    );
    let manifest = plugin_framework::parse_plugin_manifest(&raw).unwrap();
    let installation = state
        .store
        .upsert_installation(&UpsertPluginInstallationInput {
            installation_id: Uuid::now_v7(),
            category: domain::ExtensionCategory::RuntimeExtensions,
            organization: "acme".into(),
            provider_code: "managed_fixture".into(),
            plugin_id: "managed_fixture@0.1.0".into(),
            plugin_version: "0.1.0".into(),
            contract_version: "1flowbase.extension-bus/v1".into(),
            protocol: "stdio_json".into(),
            display_name: "Contribution authority route fixture".into(),
            source_kind: "uploaded".into(),
            trust_level: "unverified".into(),
            verification_status: domain::PluginVerificationStatus::Valid,
            desired_state: domain::PluginDesiredState::Disabled,
            expected_checksum: None,
            signature_status: domain::ExtensionSignatureStatus::Missing,
            signature_algorithm: None,
            signing_key_id: None,
            metadata_json: json!({"managed": manifest.managed}),
            is_system_reserved: false,
            actor_user_id: actor_id,
        })
        .await
        .unwrap();
    state
        .store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: installation.id,
            workspace_id: state.bootstrap_workspace_id,
            provider_code: installation.provider_code.clone(),
            actor_user_id: actor_id,
        })
        .await
        .unwrap();
    let path = format!(
        "/api/console/settings/extension-center/installed/{}/contribution-authorizations",
        installation.id
    );
    let grant = json!({"contribution_id":"managed_fixture.compute", "permission":"hook.model_definitions.create.before",
        "resource_scope":{"kind":"workspace"}, "permission_contract_id":"managed-hook", "permission_contract_version":"1"});
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .clone();
    let snapshot = registry.snapshot();
    for operation in ["grant", "revoke", "view"] {
        let id = format!("extension_center.contribution_authorizations.{operation}");
        let plan = snapshot
            .plan_for_interface(&interface_runtime::InterfaceId::new(id.clone()).unwrap())
            .unwrap();
        assert_eq!(
            plan.definition().handler_reference().as_str(),
            format!("{id}.handler")
        );
        assert_eq!(plan.binding().projection().http_route().unwrap().path(),
            format!("/api/console/settings/extension-center/installed/:installation_id/contribution-authorizations{}", if operation == "revoke" { "/revoke" } else { "" }));
    }
    let (status, initial) = request(&app, &cookie, &csrf, &path, "GET", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(initial["data"]["revision"], 0);
    assert_eq!(initial["data"]["authorizations"], json!([]));
    let (status, granted) = request(&app, &cookie, &csrf, &path, "POST", grant.clone()).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        granted["data"]["workspace_id"],
        state.bootstrap_workspace_id.to_string()
    );
    assert_eq!(granted["data"]["revision"], 1);
    let revoke = json!({"authorization_id":granted["data"]["authorizations"][0]["id"], "expected_revision":1});
    // A real custom member has the legacy installation configure grant only. Existing feature/configure grants
    // must not expand into any of these newly registered operations.
    let role = "root_2007_extension_configure_only";
    create_role(&app, &cookie, &csrf, role).await;
    replace_role_permissions(&app, &cookie, &csrf, role, &["plugin_config.configure.all"]).await;
    let member = create_member(
        &app,
        &cookie,
        &csrf,
        "root-2007-authority-member",
        "temp-pass",
    )
    .await;
    replace_member_roles(&app, &cookie, &csrf, &member, &[role]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "root-2007-authority-member", "temp-pass").await;
    for (method, suffix, body) in [
        ("GET", "", Value::Null),
        ("POST", "", grant.clone()),
        ("POST", "/revoke", revoke.clone()),
    ] {
        assert_eq!(
            request(
                &app,
                &member_cookie,
                &member_csrf,
                &format!("{path}{suffix}"),
                method,
                body
            )
            .await
            .0,
            StatusCode::FORBIDDEN
        );
    }
    let mut self_grant = grant;
    self_grant["workspace_id"] = json!(Uuid::now_v7());
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&path)
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(self_grant.to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let (status, revoked) = request(
        &app,
        &cookie,
        &csrf,
        &format!("{path}/revoke"),
        "POST",
        revoke.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(revoked["data"]["revision"], 2);
    assert_eq!(revoked["data"]["authorizations"][0]["status"], "revoked");
    assert_eq!(
        request(
            &app,
            &cookie,
            &csrf,
            &format!("{path}/revoke"),
            "POST",
            revoke
        )
        .await
        .0,
        StatusCode::CONFLICT
    );
}
