use super::*;
use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    test_api_state_with_database_url,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
    Router,
};
use serde_json::{json, Value};
use tower::ServiceExt;
mod transfers;
struct Temp(std::path::PathBuf);
impl Temp {
    fn new() -> Self {
        let path = std::env::temp_dir().join(format!("terminal-route-{}", Uuid::now_v7()));
        std::fs::create_dir_all(&path).unwrap();
        Self(path)
    }
}
impl Drop for Temp {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}
async fn request(
    app: &Router,
    method: &str,
    path: &str,
    cookie: &str,
    csrf: &str,
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
    let bytes = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or_else(|_| json!(String::from_utf8_lossy(&bytes))),
    )
}
async fn grant(app: &Router, cookie: &str, csrf: &str, role: &str, operations: &[&str]) {
    let (status, value) = request(app,"PUT",&format!("/api/console/settings/roles/{role}/console-policy"),cookie,csrf,json!({"groups":[{"kind":"other","group_id":"other.local-commands","enabled":true,"strategy":"custom","operations":operations.iter().map(|op|json!({"kind":"simple","operation_id":op,"enabled":true})).collect::<Vec<_>>()}]})).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{value}");
}
#[test]
fn terminal_contract_requires_explicit_budgets_and_keeps_contents_out_of_hooks() {
    assert!(
        serde_json::from_value::<ListInput>(json!({"path":"/","timeout_ms":1,"limit":1})).is_err()
    );
    assert!(
        serde_json::from_value::<TextInput>(json!({"path":"/x","timeout_ms":1,"offset":0}))
            .is_err()
    );
    assert!(serde_json::from_value::<JobInput>(
        json!({"execution_id":Uuid::now_v7(),"user_id":Uuid::now_v7()})
    )
    .is_err());
    let input = TerminalInput::Info(InfoInput {});
    assert!(input.project_for_managed_hook().is_none());
    let output = TerminalOutput::Info(local_terminal::info());
    assert!(output.project_for_managed_hook().is_none());
    let registry = crate::app_state::compile_core_console_operation_registry(
        &crate::app_state::compile_core_settings_feature_registry().unwrap(),
    )
    .unwrap();
    for id in INTERFACE_IDS {
        let entry = registry
            .inventory()
            .interfaces
            .iter()
            .find(|entry| &entry.interface_id == id)
            .unwrap();
        assert_eq!(entry.authorization_operation_id.as_deref(), Some(*id));
        assert!(entry.summary.is_ascii());
        assert!(entry.description.is_ascii());
    }
}
#[tokio::test]
async fn terminal_routes_enforce_independent_operations_and_shared_job_registry() {
    let (state, _) = test_api_state_with_database_url().await;
    let clone = state.as_ref().clone();
    assert!(Arc::ptr_eq(&state.terminal_jobs, &clone.terminal_jobs));
    let app = crate::app_with_state(state);
    let (root, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let temp = Temp::new();
    let path = temp.0.join("text.txt");
    std::fs::write(&path, "a中b").unwrap();
    let body = json!({"path":path,"timeout_ms":1000,"offset":0,"max_bytes":3});
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/files/read-text",
        "",
        "",
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    let member = create_member(&app, &root, &csrf, "terminal-member", "temp-pass").await;
    create_role(&app, &root, &csrf, "terminal_role").await;
    replace_member_roles(&app, &root, &csrf, &member, &["terminal_role"]).await;
    let (cookie, token) = login_and_capture_cookie(&app, "terminal-member", "temp-pass").await;
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/files/read-text",
        &cookie,
        &token,
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    grant(
        &app,
        &root,
        &csrf,
        "terminal_role",
        &["terminal.files.read_text"],
    )
    .await;
    let (status, value) = request(
        &app,
        "POST",
        "/api/console/terminal/files/read-text",
        &cookie,
        &token,
        body.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{value}");
    assert_eq!(value["text"], "a");
    assert_eq!(value["next_offset"], 1);
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/files/stat",
        &cookie,
        &token,
        json!({"path":path,"timeout_ms":1000}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/local-commands/execute",
        &cookie,
        &token,
        json!({"command":"echo forbidden"}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/files/read-text",
        &cookie,
        &token,
        json!({"path":path,"timeout_ms":0,"offset":0,"max_bytes":3}),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    grant(
        &app,
        &root,
        &csrf,
        "terminal_role",
        &[
            "terminal.jobs.start",
            "terminal.jobs.get",
            "terminal.jobs.read",
            "terminal.jobs.cancel",
        ],
    )
    .await;
    let job = json!({"request_id":Uuid::now_v7(),"command":"echo terminal-job","timeout_ms":5000,"retention_ms":60000,"max_output_bytes":1000});
    let (status, first) = request(
        &app,
        "POST",
        "/api/console/terminal/jobs/start",
        &cookie,
        &token,
        job.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{first}");
    let (status, replay) = request(
        &app,
        "POST",
        "/api/console/terminal/jobs/start",
        &cookie,
        &token,
        job,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{replay}");
    assert_eq!(first["execution_id"], replay["execution_id"]);
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/jobs/get",
        &root,
        &csrf,
        json!({"execution_id":first["execution_id"]}),
    )
    .await;
    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "root cannot impersonate job owner"
    );
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/jobs/cancel",
        &cookie,
        &token,
        json!({"execution_id":first["execution_id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    grant(&app, &root, &csrf, "terminal_role", &[]).await;
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/jobs/get",
        &cookie,
        &token,
        json!({"execution_id":first["execution_id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}
