//! Exercise the real protocol, catalog mapping, authentication and typed role handler.
//! No direct store grants, dispatcher substitution or weakened ACL fixture.
use super::*;
use crate::_tests::support::{create_role, replace_member_roles};

const TARGET_ROLE: &str = "mcp_policy_target";
const READER_ROLE: &str = "mcp_policy_reader";
const GET_TOOL: &str = "role_policy_get";
const PUT_TOOL: &str = "role_policy_put";

struct PolicyFixture {
    app: axum::Router,
    cookie: String,
    csrf: String,
    token: String,
}

impl PolicyFixture {
    async fn new() -> Self {
        let app = test_app().await;
        let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
        create_mcp_instance(&app, &cookie, &csrf).await;
        let token = create_api_key(&app, &cookie, &csrf).await;
        for (tool, interface, mappings) in [
            (
                GET_TOOL,
                "roles.console_policy.view",
                json!([
                    {"interface_param":"id","mcp_param":"role_code","required":true}
                ]),
            ),
            (
                PUT_TOOL,
                "roles.console_policy.replace",
                json!([
                    {"interface_param":"id","mcp_param":"role_code","required":true},
                    {"interface_param":"groups","mcp_param":"groups","required":true}
                ]),
            ),
        ] {
            create_interface_tool_and_binding(
                &app,
                &cookie,
                &csrf,
                tool,
                interface,
                json!({"mappings":mappings}),
            )
            .await;
        }
        create_role(&app, &cookie, &csrf, TARGET_ROLE).await;
        Self {
            app,
            cookie,
            csrf,
            token,
        }
    }

    async fn policy(&self, role: &str) -> Value {
        let (status, payload) = policy_snapshot(&self.app, &self.cookie, role).await;
        assert_eq!(status, StatusCode::OK, "{role}: {payload}");
        payload["data"].clone()
    }

    async fn replace(&self, role: &str, groups: Value) -> Value {
        policy_call(&self.app, &self.token, PUT_TOOL, role, Some(groups)).await
    }
}

fn role_group(operations: &[&str]) -> Value {
    json!({
        "kind":"settings_feature", "group_id":"system.roles", "enabled":true,
        "strategy":"custom", "operations":operations.iter().map(|id| json!({
            "kind":"simple", "operation_id":id, "enabled":true
        })).collect::<Vec<_>>()
    })
}

fn roles_group(policy: &Value) -> &Value {
    policy["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["kind"] == "settings_feature" && group["group_id"] == "system.roles")
        .expect("role policy must contain its compiled roles group")
}

async fn policy_snapshot(app: &axum::Router, cookie: &str, role: &str) -> (StatusCode, Value) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!("/api/console/settings/roles/{role}/console-policy"))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), response_json(response).await)
}

async fn http_replace(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    role: &str,
    groups: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/roles/{role}/console-policy"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"groups":groups}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

async fn policy_call(
    app: &axum::Router,
    token: &str,
    tool: &str,
    role: &str,
    groups: Option<Value>,
) -> Value {
    let mut arguments = json!({"role_code":role});
    if let Some(groups) = groups {
        arguments["groups"] = groups;
    }
    call_mcp(
        app,
        token,
        json!({
            "jsonrpc":"2.0", "id":1, "method":"tools/call", "params":{
                "name":"mcp_call", "arguments":{
                    "tool_id":tool, "arguments":arguments, "max_inline_chars":16000
                }
            }
        }),
    )
    .await
}

fn assert_no_content(payload: &Value) {
    assert!(payload.get("error").is_none(), "{payload}");
    assert_eq!(payload["result"]["isError"], false, "{payload}");
    // MCP structuredContent must be an object; preserve the HTTP 204 as null,
    // not a fabricated policy response or an empty object.
    assert_eq!(
        payload["result"]["structuredContent"],
        json!({"result":null})
    );
    let text: Value =
        serde_json::from_str(payload["result"]["content"][0]["text"].as_str().unwrap()).unwrap();
    assert_eq!(text, json!({"result":null}));
}

fn assert_target_error(payload: &Value, status: u16, code: &str) {
    assert!(payload.get("result").is_none(), "{payload}");
    assert_eq!(payload["error"]["code"], -32603, "{payload}");
    let data = &payload["error"]["data"];
    assert_eq!(data["http_status"], status, "{payload}");
    assert_eq!(data["target_code"], code, "{payload}");
    assert_eq!(data["outcome"], "failed", "{payload}");
    assert_eq!(data["retry_original"], false, "{payload}");
}

#[tokio::test]
async fn mcp_role_permission_policy_get_put_roundtrips_through_real_typed_handler() {
    let fixture = PolicyFixture::new().await;
    let before = fixture.policy(TARGET_ROLE).await;
    let read = policy_call(&fixture.app, &fixture.token, GET_TOOL, TARGET_ROLE, None).await;
    assert!(read.get("error").is_none(), "{read}");
    assert_eq!(read["result"]["isError"], false);
    assert_eq!(read["result"]["structuredContent"], before);

    let group = role_group(&["roles.console_policy.view"]);
    let write = fixture.replace(TARGET_ROLE, json!([group])).await;
    assert_no_content(&write);
    let stored = fixture.policy(TARGET_ROLE).await;
    assert_ne!(
        stored, before,
        "successful MCP PUT must persist the requested policy"
    );
    assert_eq!(stored["role_code"], TARGET_ROLE);
    assert_eq!(roles_group(&stored), &group);
    let read = policy_call(&fixture.app, &fixture.token, GET_TOOL, TARGET_ROLE, None).await;
    assert_eq!(read["result"]["structuredContent"], stored, "{read}");

    // The HTTP projection of the same handler still owns its 204/empty-body contract.
    let cleared = http_replace(
        &fixture.app,
        &fixture.cookie,
        &fixture.csrf,
        TARGET_ROLE,
        json!([]),
    )
    .await;
    assert_eq!(cleared.status(), StatusCode::NO_CONTENT);
    assert!(to_bytes(cleared.into_body(), usize::MAX)
        .await
        .unwrap()
        .is_empty());
    let restored = fixture.policy(TARGET_ROLE).await;
    assert_eq!(roles_group(&restored)["enabled"], false);
    let read = policy_call(&fixture.app, &fixture.token, GET_TOOL, TARGET_ROLE, None).await;
    assert_eq!(read["result"]["structuredContent"], restored, "{read}");
}

#[tokio::test]
async fn mcp_role_permission_readonly_user_can_get_but_cannot_put_or_escalate() {
    let fixture = PolicyFixture::new().await;
    create_role(&fixture.app, &fixture.cookie, &fixture.csrf, READER_ROLE).await;
    let reader_group = role_group(&["roles.console_policy.view"]);
    // Legal prerequisite for issuing the user's own role-bound key. It grants
    // no role mutation operation and is not a direct database/auth bypass.
    let groups = json!([reader_group, {
        "kind":"settings_feature", "group_id":"system.api-key-authentication",
        "enabled":true, "strategy":"custom", "operations":[{
            "kind":"simple", "operation_id":"user_api_keys.manage", "enabled":true
        }]
    }]);
    assert_no_content(&fixture.replace(READER_ROLE, groups).await);
    let member = create_member(
        &fixture.app,
        &fixture.cookie,
        &fixture.csrf,
        "mcp-policy-reader",
        "temp-pass",
    )
    .await;
    replace_member_roles(
        &fixture.app,
        &fixture.cookie,
        &fixture.csrf,
        &member,
        &[READER_ROLE],
    )
    .await;
    let (cookie, csrf) =
        login_and_capture_cookie(&fixture.app, "mcp-policy-reader", "temp-pass").await;
    let token = create_api_key(&fixture.app, &cookie, &csrf).await;
    let target_before = fixture.policy(TARGET_ROLE).await;
    let reader_before = fixture.policy(READER_ROLE).await;
    let (status, http_read) = policy_snapshot(&fixture.app, &cookie, TARGET_ROLE).await;
    assert_eq!(status, StatusCode::OK, "{http_read}");
    assert_eq!(http_read["data"], target_before);
    let read = policy_call(&fixture.app, &token, GET_TOOL, TARGET_ROLE, None).await;
    assert_eq!(read["result"]["isError"], false, "{read}");
    assert_eq!(read["result"]["structuredContent"], target_before);

    let escalation = json!([role_group(&[
        "roles.console_policy.view",
        "roles.console_policy.replace"
    ])]);
    for role in [TARGET_ROLE, READER_ROLE] {
        let denied = policy_call(
            &fixture.app,
            &token,
            PUT_TOOL,
            role,
            Some(escalation.clone()),
        )
        .await;
        assert_target_error(&denied, 403, "console_operation_permission_denied");
        let denied = http_replace(&fixture.app, &cookie, &csrf, role, escalation.clone()).await;
        assert_eq!(denied.status(), StatusCode::FORBIDDEN);
        assert_eq!(
            response_json(denied).await["code"],
            "console_operation_permission_denied"
        );
        assert_eq!(fixture.policy(TARGET_ROLE).await, target_before);
        assert_eq!(fixture.policy(READER_ROLE).await, reader_before);
    }
    let read = policy_call(&fixture.app, &token, GET_TOOL, TARGET_ROLE, None).await;
    assert_eq!(
        read["result"]["structuredContent"], target_before,
        "denied writes must not revoke valid reads: {read}"
    );
}

#[tokio::test]
async fn mcp_role_permission_invalid_policy_and_root_mutation_are_rejected_without_side_effects() {
    let fixture = PolicyFixture::new().await;
    let valid_group = role_group(&["roles.console_policy.view"]);
    assert_no_content(&fixture.replace(TARGET_ROLE, json!([valid_group])).await);
    let target_before = fixture.policy(TARGET_ROLE).await;
    let admin_before = fixture.policy("admin").await;
    let member_before = fixture.policy("member").await;
    let root_before = policy_snapshot(&fixture.app, &fixture.cookie, "root").await;
    let mut unknown_group = valid_group.clone();
    unknown_group["group_id"] = json!("system.not-a-real-feature");
    let unknown_operation = role_group(&["roles.not_a_real_operation"]);
    let duplicate_operation =
        role_group(&["roles.console_policy.view", "roles.console_policy.view"]);
    let mut wrong_operation_type = valid_group.clone();
    wrong_operation_type["operations"] = json!([{
        "kind":"row", "operation_id":"roles.console_policy.view", "scope":"scope_all"
    }]);
    for (groups, code) in [
        (json!([unknown_group]), "console_policy_group"),
        (json!([unknown_operation]), "console_policy_operation"),
        (
            json!([duplicate_operation]),
            "console_policy_operation_duplicate",
        ),
        (
            json!([valid_group, valid_group]),
            "console_policy_group_duplicate",
        ),
        (
            json!([wrong_operation_type]),
            "console_policy_operation_type",
        ),
    ] {
        let denied = fixture.replace(TARGET_ROLE, groups).await;
        assert_target_error(&denied, 400, code);
        assert_eq!(fixture.policy(TARGET_ROLE).await, target_before, "{code}");
    }
    let denied = fixture.replace("root", json!([])).await;
    assert_target_error(&denied, 403, "root_role_immutable");
    assert_eq!(
        policy_snapshot(&fixture.app, &fixture.cookie, "root").await,
        root_before
    );
    assert_eq!(fixture.policy(TARGET_ROLE).await, target_before);
    assert_eq!(fixture.policy("admin").await, admin_before);
    assert_eq!(fixture.policy("member").await, member_before);
}
