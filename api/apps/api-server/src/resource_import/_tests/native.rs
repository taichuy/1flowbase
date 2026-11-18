use super::*;
use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url,
};
use axum::{
    body::Body,
    http::{HeaderMap, Request, StatusCode},
};
use serde_json::json;
use tower::ServiceExt;

async fn context(
    state: &ApiState,
    cookie: &str,
) -> crate::middleware::require_session::RequestContext {
    let mut headers = HeaderMap::new();
    headers.insert("cookie", cookie.parse().unwrap());
    crate::middleware::require_session::require_session(state, &headers)
        .await
        .unwrap()
}

#[tokio::test]
async fn native_import_capture_reuses_real_session_and_rejects_recursive_or_stale_callers() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let context = context(&state, &cookie).await;
    let principal = context.interface_principal();
    let caller = capture_caller(&state, &principal, IMPORT_INTERFACE)
        .await
        .unwrap()
        .unwrap();
    assert!(caller.selected_mcp_instance_ids.is_none());
    assert!(current_actor(&state, &caller).await.is_ok());
    let selected =
        with_selected_scope(vec![], capture_caller(&state, &principal, IMPORT_INTERFACE))
            .await
            .unwrap()
            .unwrap();
    assert_eq!(selected.selected_mcp_instance_ids, Some(vec![]));
    let spawned_state = state.clone();
    tokio::spawn(async move {
        assert!(selected_mcp_scope().is_none());
        assert_eq!(selected.selected_mcp_instance_ids, Some(vec![]));
        assert!(current_actor(&spawned_state, &selected).await.is_ok());
    })
    .await
    .unwrap();
    let denied = with_replay(import_for_principal(
        state.clone(),
        &principal,
        "invalid",
        uuid::Uuid::now_v7(),
    ))
    .await
    .unwrap_err();
    assert_eq!(denied.code, "file_import_reentrant_source");
    for case in 0..4 {
        let mut changed = caller.clone();
        match case {
            0 => changed.actor.user_id = uuid::Uuid::now_v7(),
            1 => changed.actor.current_workspace_id = uuid::Uuid::now_v7(),
            2 => changed.actor.effective_display_role = "unbound_role".into(),
            _ => changed.session_version += 1,
        }
        assert!(
            current_actor(&state, &changed).await.is_err(),
            "case {case}"
        );
    }
    let mut expired = context.cookie_session().unwrap().clone();
    expired.expires_at_unix = 1;
    state.session_store.put(expired).await.unwrap();
    assert!(current_actor(&state, &caller).await.is_err());
    state
        .session_store
        .put(context.cookie_session().unwrap().clone())
        .await
        .unwrap();
    assert!(current_actor(&state, &caller).await.is_ok());
    sqlx::query("update users set session_version = session_version + 1 where id = $1")
        .bind(caller.actor.user_id)
        .execute(state.store.pool())
        .await
        .unwrap();
    assert!(current_actor(&state, &caller).await.is_err());
    assert!(capture_caller(&state, &principal, IMPORT_INTERFACE)
        .await
        .is_err());
    sqlx::query("update users set session_version = $2, status = 'disabled' where id = $1")
        .bind(caller.actor.user_id)
        .bind(caller.session_version)
        .execute(state.store.pool())
        .await
        .unwrap();
    assert!(current_actor(&state, &caller).await.is_err());
    assert!(capture_caller(&state, &principal, IMPORT_INTERFACE)
        .await
        .is_err());
}

#[tokio::test]
async fn native_import_checks_current_individual_console_operation_and_selected_scope() {
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member_id = create_member(&app, &cookie, &csrf, "native-import-member", "temp-pass").await;
    create_role(&app, &cookie, &csrf, "native_import_role").await;
    replace_member_roles(&app, &cookie, &csrf, &member_id, &["native_import_role"]).await;
    let (member_cookie, _) =
        login_and_capture_cookie(&app, "native-import-member", "temp-pass").await;
    let principal = context(&state, &member_cookie).await.interface_principal();
    let caller = capture_caller(&state, &principal, IMPORT_INTERFACE)
        .await
        .unwrap()
        .unwrap();
    assert!(current_native_caller(&state, &caller).await.is_err());
    for (enabled, operation, allowed) in [
        (true, IMPORT_INTERFACE, true),
        (true, "local_commands.execute", false),
        (false, IMPORT_INTERFACE, false),
        (true, IMPORT_INTERFACE, true),
    ] {
        let response = app.clone().oneshot(Request::builder().method("PUT")
            .uri("/api/console/settings/roles/native_import_role/console-policy")
            .header("cookie", &cookie).header("x-csrf-token", &csrf).header("content-type", "application/json")
            .body(Body::from(json!({"groups":[{"kind":"other","group_id":"other.local-commands","enabled":enabled,"strategy":"custom","operations":[{"kind":"simple","operation_id":operation,"enabled":true}]}]}).to_string())).unwrap()).await.unwrap();
        assert_eq!(response.status(), StatusCode::NO_CONTENT);
        assert_eq!(
            current_native_caller(&state, &caller).await.is_ok(),
            allowed
        );
    }
    let mut wrong_operation = caller.clone();
    wrong_operation.interface_id = "local_commands.execute".into();
    assert!(current_native_caller(&state, &wrong_operation)
        .await
        .is_err());
    let mut empty_scope = caller.clone();
    empty_scope.selected_mcp_instance_ids = Some(vec![]);
    assert!(
        current_native_caller(&state, &empty_scope).await.is_err(),
        "explicit empty scope cannot become all instances"
    );
    replace_member_roles(&app, &cookie, &csrf, &member_id, &[]).await;
    assert!(
        current_native_caller(&state, &caller).await.is_err(),
        "a captured role cannot outlive its binding"
    );
}
