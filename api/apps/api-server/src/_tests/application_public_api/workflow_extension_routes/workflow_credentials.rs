use super::super::workflow_credentials_fixture::save_document;
use super::*;
use crate::_tests::support::{create_member, create_role, replace_member_roles};
use control_plane_contracts::ports::{
    ApplicationPublicationRepository, WorkflowRunIdentityRepository,
};

#[tokio::test]
async fn workflow_credentials_extension_formal_run_preserves_current_nondefault_role() {
    let (app, state) = test_app_with_state().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id =
        create_workflow_application(&app, &root_cookie, &root_csrf, "Extension credentials").await;
    save_document(
        &app,
        &root_cookie,
        &root_csrf,
        &application_id,
        true,
        "executor",
    )
    .await;
    publish_workflow_extension(
        &app,
        &root_cookie,
        &root_csrf,
        &application_id,
        "workflow-credentials-role",
        "sync",
    )
    .await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "workflow-credential-extension",
        "temp-pass",
    )
    .await;
    for role in [
        "workflow_credential_default",
        "workflow_credential_selected",
    ] {
        create_role(&app, &root_cookie, &root_csrf, role).await;
    }
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &[
            "workflow_credential_default",
            "workflow_credential_selected",
        ],
    )
    .await;
    // This consumer verifies the host identity, using the published full application
    // profile rather than the unrelated legacy custom-operation crosswalk.
    set_application_group(&app, &root_cookie, &root_csrf, true).await;
    let (cookie, csrf) =
        login_and_capture_cookie(&app, "workflow-credential-extension", "temp-pass").await;
    let before = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(before.status(), StatusCode::OK);
    assert_eq!(
        response_json(before).await["data"]["effective_display_role"],
        "workflow_credential_default"
    );
    let switched = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/session/actions/switch-role")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"role_code":"workflow_credential_selected"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(switched.status(), StatusCode::OK);
    let session = response_json(switched).await;
    let csrf = session["data"]["csrf_token"].as_str().unwrap();
    assert_eq!(session["data"]["actor"]["id"], member_id);
    assert_eq!(
        session["data"]["actor"]["effective_display_role"],
        "workflow_credential_selected"
    );
    let publication = state
        .store
        .load_active_application_publication(application_id.parse().unwrap())
        .await
        .unwrap()
        .expect("fixture must publish before extension admission");
    assert_eq!(
        publication.workspace_id.to_string(),
        session["data"]["session"]["current_workspace_id"]
            .as_str()
            .unwrap(),
        "session and published workflow must share a workspace"
    );
    let publication_tenant: Uuid =
        sqlx::query_scalar("select tenant_id from workspaces where id=$1")
            .bind(publication.workspace_id)
            .fetch_one(state.store.pool())
            .await
            .unwrap();
    assert_eq!(
        publication_tenant.to_string(),
        session["data"]["session"]["tenant_id"].as_str().unwrap()
    );
    let actual_me = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/me")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(actual_me.status(), StatusCode::OK);
    let actual_me = response_json(actual_me).await;
    assert_eq!(actual_me["data"]["id"], member_id);
    assert_eq!(
        actual_me["data"]["effective_display_role"],
        "workflow_credential_selected"
    );
    let actual_session = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/session")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(actual_session.status(), StatusCode::OK);
    let actual_session = response_json(actual_session).await;
    assert_eq!(
        actual_session["data"]["session"], session["data"]["session"],
        "switch response must match the next authenticated request"
    );
    let admission = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/applications/{application_id}"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let admission_status = admission.status();
    let admission_body = response_json(admission).await;
    assert_eq!(
        admission_status,
        StatusCode::OK,
        "published application admission before extension: {admission_body}"
    );
    let call = || {
        app.clone().oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/ex/workflow-credentials-role")
                .header("cookie", &cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from("{}"))
                .unwrap(),
        )
    };
    let response = call().await.unwrap();
    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["status_code"], 200, "{payload}");
    let identity: Value = serde_json::from_str(payload["identity"].as_str().unwrap()).unwrap();
    assert_eq!(identity["data"]["id"], member_id);
    assert_eq!(
        identity["data"]["effective_display_role"],
        "workflow_credential_selected"
    );
    let run_id: Uuid = sqlx::query_scalar(
        "select id from flow_runs where application_id=$1 order by started_at desc limit 1",
    )
    .bind(application_id.parse::<Uuid>().unwrap())
    .fetch_one(state.store.pool())
    .await
    .unwrap();
    let snapshot = state
        .store
        .get_workflow_run_identity(run_id)
        .await
        .unwrap()
        .unwrap();
    let executor = snapshot.executor.unwrap();
    assert_eq!(executor.active_role_code, "workflow_credential_selected");
    assert_eq!(
        executor.workspace_id.to_string(),
        session["data"]["session"]["current_workspace_id"]
            .as_str()
            .unwrap()
    );
    assert_eq!(
        executor.tenant_id.to_string(),
        session["data"]["session"]["tenant_id"].as_str().unwrap()
    );
    assert_ne!(snapshot.creator.unwrap().user_id, executor.user_id);
    set_application_group(&app, &root_cookie, &root_csrf, false).await;
    assert_eq!(
        call().await.unwrap().status(),
        StatusCode::FORBIDDEN,
        "revoked application access must remain authoritative"
    );
}

async fn set_application_group(app: &Router, cookie: &str, csrf: &str, enabled: bool) {
    let path = "/api/console/settings/roles/workflow_credential_selected/console-policy";
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"groups":[{"kind":"settings_feature","group_id":"system.applications",
            "enabled":enabled,"strategy":"full","operations":[]}]})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let status = response.status();
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        status,
        StatusCode::NO_CONTENT,
        "{}",
        String::from_utf8_lossy(&body)
    );
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(path)
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let policy = response_json(response).await;
    let group = policy["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| {
            group["kind"] == "settings_feature" && group["group_id"] == "system.applications"
        })
        .expect("the application group must be retained in the official read model");
    assert_eq!(group["enabled"], enabled, "{policy}");
    assert_eq!(group["strategy"], "full", "{policy}");
}
