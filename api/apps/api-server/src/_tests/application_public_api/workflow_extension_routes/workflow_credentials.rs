use super::super::workflow_credentials_fixture::save_document;
use super::*;
use crate::_tests::support::{
    create_member, create_role, replace_member_roles, replace_role_permissions,
};
use control_plane_contracts::ports::WorkflowRunIdentityRepository;

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
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "workflow_credential_selected",
        &["application.view.all"],
    )
    .await;
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
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "workflow_credential_selected",
        &[],
    )
    .await;
    assert_eq!(
        call().await.unwrap().status(),
        StatusCode::FORBIDDEN,
        "revoked application access must remain authoritative"
    );
}
