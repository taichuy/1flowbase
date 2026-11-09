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
mod download_acl;
mod managed_projection;
mod mcp_media;
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
    assert_eq!(
        input.project_for_managed_hook().unwrap(),
        json!({"action":"info"})
    );
    let output = TerminalOutput::Info(local_terminal::info());
    assert_eq!(
        output.project_for_managed_hook().unwrap(),
        json!({"kind":"info","capability_count":3,"pty":false})
    );
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
    assert_eq!(value["data"]["text"], "a");
    assert_eq!(value["data"]["next_offset"], 1);
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
    assert_eq!(
        first["data"]["execution_id"],
        replay["data"]["execution_id"]
    );
    let (status, _) = request(
        &app,
        "POST",
        "/api/console/terminal/jobs/get",
        &root,
        &csrf,
        json!({"execution_id":first["data"]["execution_id"]}),
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
        json!({"execution_id":first["data"]["execution_id"]}),
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
        json!({"execution_id":first["data"]["execution_id"]}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn terminal_mcp_bindable_catalog_and_tool_creation_preserve_canonical_ids() {
    use std::collections::BTreeSet;
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let (status, catalog) = request(
        &app,
        "GET",
        "/api/console/mcp/interface-capabilities",
        &cookie,
        &csrf,
        json!({}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let entries = catalog["data"].as_array().unwrap();
    // Pin the user-facing IDs independently of handler function names and OpenAPI generation.
    let expected = [
        ("terminal.info", "info"),
        ("terminal.files.list", "files/list"),
        ("terminal.files.stat", "files/stat"),
        ("terminal.files.read_text", "files/read-text"),
        ("terminal.files.read_range", "files/read-range"),
        ("terminal.files.search", "files/search"),
        ("terminal.files.read_image", "files/read-image"),
        ("terminal.files.read_audio", "files/read-audio"),
        ("terminal.files.read_resource", "files/read-resource"),
        ("terminal.files.import_resource", "files/import-resource"),
        ("terminal.files.download_file", "files/download-file"),
        ("terminal.jobs.start", "jobs/start"),
        ("terminal.jobs.get", "jobs/get"),
        ("terminal.jobs.read", "jobs/read"),
        ("terminal.jobs.cancel", "jobs/cancel"),
    ];
    let terminal_entries = entries
        .iter()
        .filter(|entry| {
            entry["path"]
                .as_str()
                .is_some_and(|path| path.starts_with("/api/console/terminal/"))
        })
        .collect::<Vec<_>>();
    assert_eq!(terminal_entries.len(), expected.len());
    let actual = terminal_entries
        .iter()
        .map(|entry| {
            (
                entry["interface_id"].as_str().unwrap().to_owned(),
                entry["method"].as_str().unwrap().to_owned(),
                entry["path"].as_str().unwrap().to_owned(),
            )
        })
        .collect::<BTreeSet<_>>();
    let identities = expected
        .iter()
        .map(|(id, path)| {
            (
                (*id).to_owned(),
                "POST".to_owned(),
                format!("/api/console/terminal/{path}"),
            )
        })
        .collect::<BTreeSet<_>>();
    assert_eq!(
        actual, identities,
        "bindable discovery must use canonical terminal IDs, not handler names"
    );
    let legacy = entries
        .iter()
        .find(|entry| entry["path"] == "/api/console/local-commands/execute")
        .unwrap();
    assert_eq!(
        legacy["interface_id"], "execute_local_command",
        "preserve the existing execute OpenAPI ID"
    );

    for (index, (id, _)) in expected.into_iter().enumerate() {
        let entry = terminal_entries
            .iter()
            .find(|entry| entry["interface_id"] == id)
            .unwrap();
        assert_eq!(entry["bindable"], true, "{id}: {entry}");
        assert_eq!(entry["disabled_reason"], Value::Null, "{id}");
        assert_eq!(entry["parameter_schema"]["type"], "object", "{id}");
        assert_ne!(entry["result_schema"], json!(false), "{id}");
        let (status, created) = request(
            &app, "POST", "/api/console/mcp/tools", &cookie, &csrf,
            json!({
                "tool_id":format!("terminal_contract_{index}"),
                "des_id":"terminalcontractv1",
                "name":format!("Terminal contract {index}"),
                "short_description":"Validate a terminal interface binding.",
                "full_description":"Bind a canonical terminal interface through the real MCP catalog.",
                "execution_target":{"kind":"interface_wrapper","interface_id":id},
                "parameter_schema":{}, "result_schema":{},
                "input_mapping":{"mappings":[]}, "output_mapping":{},
                "permission_code":null, "risk_level":"low", "status":"enabled"
            }),
        ).await;
        assert_eq!(status, StatusCode::CREATED, "{id}: {created}");
        assert_eq!(created["data"]["execution_target"]["interface_id"], id);
        assert_eq!(
            created["data"]["parameter_schema"], entry["parameter_schema"],
            "{id}"
        );
        assert_eq!(
            created["data"]["result_schema"], entry["result_schema"],
            "{id}"
        );
    }
}
