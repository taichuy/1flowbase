use super::*;
use control_plane::event_center::EventCenterSubscription;
use domain::event_center::EventDefinitionRecord;

async fn operation(app: &Router, cookie: &str, method: &str, path: &str) -> Value {
    let (status, catalog) = request(
        app,
        "GET",
        "/api/console/events/api-operations",
        cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == method && row["path"] == path)
        .unwrap_or_else(|| panic!("missing actual operation {method} {path}"))
        .clone()
}

struct Observation {
    before: EventDefinitionRecord,
    completion: EventDefinitionRecord,
    input: EventCenterSubscription,
    output: EventCenterSubscription,
    foreign: EventCenterSubscription,
}
impl Observation {
    async fn new(
        app: &Router,
        state: &Arc<crate::app_state::ApiState>,
        cookie: &str,
        csrf: &str,
        method: &str,
        path: &str,
    ) -> Self {
        let selected = operation(app, cookie, method, path).await;
        assert_eq!(selected["before"]["status"], "available", "{selected}");
        assert_eq!(selected["completion"]["status"], "available", "{selected}");
        let id = selected["operation_id"].as_str().unwrap();
        let mut rules = Vec::new();
        for phase in ["before", "completion"] {
            rules.push(
                create_rule(
                    app,
                    state,
                    cookie,
                    csrf,
                    json!({
                "name":format!("Public JSON {method} {path} {phase}"),"description":"Public JSON contract proof",
                        "enabled":true,"interface_event":{"operation_id":id,"phase":phase}
                    }),
                )
                .await,
            );
        }
        let completion = rules.pop().unwrap();
        let before = rules.pop().unwrap();
        Self {
            input: subscribe_rule(state, &before, before.workspace_id),
            output: subscribe_rule(state, &completion, completion.workspace_id),
            foreign: subscribe_rule(state, &completion, Uuid::now_v7()),
            before,
            completion,
        }
    }
    fn assert_call(&self, expected_input: Value, expected_output: &Value) {
        let before = take_payload(&self.input);
        let completion = take_payload(&self.output);
        assert_eq!(before["input"], expected_input, "{before}");
        assert_eq!(completion["output"], *expected_output, "{completion}");
        assert_eq!(completion["terminal"], "completed");
        for (rule, event) in [(&self.before, &before), (&self.completion, &completion)] {
            assert!(
                jsonschema::validator_for(&rule.payload_schema)
                    .unwrap()
                    .is_valid(event),
                "actual public value must satisfy inherited schema: {event}, schema: {}",
                rule.payload_schema
            );
        }
        self.assert_empty();
    }
    fn assert_empty(&self) {
        for queue in [&self.input, &self.output, &self.foreign] {
            assert!(
                queue.try_recv().unwrap().is_none(),
                "exactly once and workspace scoped"
            );
        }
    }
}
impl Drop for Observation {
    fn drop(&mut self) {
        for queue in [&self.input, &self.output, &self.foreign] {
            queue.close();
        }
    }
}

// This integration target does not mount mcp_protocol_routes. Keep the same real
// HTTP setup here, rather than replacing tools/call with debug_execute or a port.
async fn mcp_client(app: &Router, cookie: &str, csrf: &str) -> String {
    let (status, instance) = request(app, "POST", "/api/console/mcp/instances", cookie, csrf,
        Some(json!({"instance_id":"public-json-proof","name":"Public JSON proof","description_short":null,"status":"enabled","default_entry_path":"/"}))).await;
    assert_eq!(status, StatusCode::CREATED, "{instance}");
    let (status, key) = request(
        app,
        "POST",
        "/api/console/user-api-keys",
        cookie,
        csrf,
        Some(json!({"name":"Public JSON proof","expiration_policy":"never"})),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{key}");
    key["data"]["token"].as_str().unwrap().to_owned()
}

async fn register_tool(
    app: &Router,
    cookie: &str,
    csrf: &str,
    tool: &str,
    method: &str,
    path: &str,
) {
    let (status, catalog) = request(
        app,
        "GET",
        "/api/console/mcp/interface-capabilities?bindable_only=true",
        cookie,
        "",
        None,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let capability = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["method"] == method && row["path"] == path)
        .unwrap_or_else(|| panic!("missing bindable capability {method} {path}: {catalog}"));
    let mappings: Vec<_> = capability["parameter_descriptors"].as_array().unwrap().iter()
        .map(|field| json!({"interface_param":field["name"],"mcp_param":field["name"],"required":field["required"]})).collect();
    let (status, created) = request(app, "POST", "/api/console/mcp/tools", cookie, csrf, Some(json!({
        "tool_id":tool,"des_id":format!("{tool}_description"),"name":tool,
        "short_description":format!("Execute {tool}."),"full_description":"",
        "execution_target":{"kind":"interface_wrapper","interface_id":capability["interface_id"]},
        "parameter_schema":{},"result_schema":{},"input_mapping":{"mappings":mappings},"output_mapping":{},
        "permission_code":null,"risk_level":"low","status":"enabled"
    }))).await;
    assert_eq!(status, StatusCode::CREATED, "{created}");
    let (status, bound) = request(app, "POST", "/api/console/mcp/instances/public-json-proof/tool-bindings", cookie, csrf,
        Some(json!({"group_path":"/proof","tool_id":tool,"display_alias":null,"visible":true,"sort_order":0}))).await;
    assert_eq!(status, StatusCode::CREATED, "{bound}");
}

async fn call_tool(app: &Router, token: &str, tool: &str, arguments: Value) -> Value {
    let response = app.clone().oneshot(Request::builder()
        .method("POST").uri("/api/mcp/public-json-proof")
        .header("authorization", format!("Bearer {token}"))
        .header("content-type", "application/json")
        .header("mcp-protocol-version", "2025-11-25")
        .body(Body::from(json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"mcp_call","arguments":{"tool_id":tool,"arguments":arguments}}
        }).to_string())).unwrap()).await.unwrap();
    let status = response.status();
    let wire: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(status, StatusCode::OK, "{wire}");
    assert_eq!(wire["result"]["isError"], false, "{wire}");
    wire["result"]["structuredContent"].clone()
}

#[tokio::test]
async fn event_center_public_json_file_tables_http_mcp_events_preserve_body_path_null_and_204() {
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let token = mcp_client(&app, &cookie, &csrf).await;
    let base = "/api/console/settings/files/tables";
    register_tool(&app, &cookie, &csrf, "proof_file_create", "POST", base).await;
    register_tool(
        &app,
        &cookie,
        &csrf,
        "proof_file_bind",
        "PUT",
        &format!("{base}/{{id}}/binding"),
    )
    .await;
    let created_events = Observation::new(&app, &state, &cookie, &csrf, "POST", base).await;
    let http_body =
        json!({"code":"public_json_http","title":"HTTP full table title","bound_storage_id":null});
    let (status, http) = request(&app, "POST", base, &cookie, &csrf, Some(http_body.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{http}");
    assert_eq!(http["data"]["title"], http_body["title"]);
    assert_eq!(http["data"]["bound_storage_id"], Value::Null);
    assert!(
        http["data"]["record_permissions"].is_object(),
        "full DTO: {http}"
    );
    created_events.assert_call(json!({"body":http_body}), &http["data"]);
    let mcp_body =
        json!({"code":"public_json_mcp","title":"MCP full table title","bound_storage_id":null});
    let mcp = call_tool(&app, &token, "proof_file_create", mcp_body.clone()).await;
    assert_eq!(mcp["title"], mcp_body["title"]);
    assert!(mcp["record_permissions"].is_object(), "full MCP DTO: {mcp}");
    created_events.assert_call(json!({"body":mcp_body}), &mcp);
    let (_, listed) = request(&app, "GET", base, &cookie, "", None).await;
    let rows = listed["data"].as_array().unwrap();
    for item in [&http["data"], &mcp] {
        assert_eq!(
            rows.iter().find(|row| row["id"] == item["id"]).unwrap(),
            item,
            "writes must be persisted and read back through the real owner"
        );
    }
    let bound_events = Observation::new(
        &app,
        &state,
        &cookie,
        &csrf,
        "PUT",
        &format!("{base}/{{id}}/binding"),
    )
    .await;
    let id = mcp["id"].as_str().unwrap();
    let bound = call_tool(
        &app,
        &token,
        "proof_file_bind",
        json!({"id":id,"bound_storage_id":null}),
    )
    .await;
    bound_events.assert_call(
        json!({"path":{"id":id},"body":{"bound_storage_id":null}}),
        &bound,
    );
    assert_eq!(bound["id"], id);
    let deleted_events = Observation::new(
        &app,
        &state,
        &cookie,
        &csrf,
        "DELETE",
        &format!("{base}/{{id}}"),
    )
    .await;
    let (status, deleted) = request(
        &app,
        "DELETE",
        &format!("{base}/{id}"),
        &cookie,
        &csrf,
        None,
    )
    .await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{deleted}");
    assert_eq!(deleted, Value::Null, "204 has no HTTP body");
    deleted_events.assert_call(json!({"path":{"id":id}}), &Value::Null);
    let (_, after) = request(&app, "GET", base, &cookie, "", None).await;
    assert!(!after["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|row| row["id"] == id));
    let (status, _) = request(
        &app,
        "POST",
        base,
        "",
        "",
        Some(json!({"code":"public_json_denied","title":"Denied"})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    created_events.assert_empty();
    let (_, after_denied) = request(&app, "GET", base, &cookie, "", None).await;
    assert_eq!(
        after_denied["data"], after["data"],
        "unauthorized request has no side effect"
    );
}

#[tokio::test]
async fn event_center_public_json_organization_http_mcp_events_share_dto_and_normalized_query() {
    let state = event_state().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let token = mcp_client(&app, &cookie, &csrf).await;
    let base = "/api/console/settings/departments";
    register_tool(
        &app,
        &cookie,
        &csrf,
        "proof_department_update",
        "PATCH",
        &format!("{base}/{{id}}"),
    )
    .await;
    register_tool(&app, &cookie, &csrf, "proof_department_list", "GET", base).await;
    let created_events = Observation::new(&app, &state, &cookie, &csrf, "POST", base).await;
    let body = json!({"name":"HTTP department","parent_id":null,"role_codes":[]});
    let (status, http) = request(&app, "POST", base, &cookie, &csrf, Some(body.clone())).await;
    assert_eq!(status, StatusCode::CREATED, "{http}");
    created_events.assert_call(json!({"body":body}), &http["data"]);
    let id = http["data"]["id"].as_str().unwrap();
    let update_events = Observation::new(
        &app,
        &state,
        &cookie,
        &csrf,
        "PATCH",
        &format!("{base}/{{id}}"),
    )
    .await;
    let update = json!({"name":"MCP updated department","parent_id":null,"role_codes":[]});
    let mut arguments = update.clone();
    arguments["id"] = json!(id);
    let mcp = call_tool(&app, &token, "proof_department_update", arguments).await;
    assert_eq!(mcp["name"], update["name"]);
    update_events.assert_call(json!({"path":{"id":id},"body":update}), &mcp);
    let list_events = Observation::new(&app, &state, &cookie, &csrf, "GET", base).await;
    // Two IDs make the CSV contract distinguishable from a Vec<Uuid> projection.
    let ids = format!("{id},{}", Uuid::now_v7());
    let (status, listed) =
        request(&app, "GET", &format!("{base}?ids={ids}"), &cookie, "", None).await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    let expected_query = json!({"query":{"ids":ids,"limit":100}});
    list_events.assert_call(expected_query.clone(), &listed["data"]);
    let item = listed["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .find(|row| row["id"] == id)
        .unwrap();
    for (key, value) in mcp.as_object().unwrap() {
        assert_eq!(
            &item[key], value,
            "MCP update must persist the complete department DTO"
        );
    }
    let listed_mcp = call_tool(&app, &token, "proof_department_list", json!({"ids":ids})).await;
    assert_eq!(
        listed_mcp, listed["data"],
        "HTTP and actual tools/call return the same complete page"
    );
    list_events.assert_call(expected_query, &listed_mcp);
    let (status, _) = request(
        &app,
        "PATCH",
        &format!("{base}/{id}"),
        "",
        "",
        Some(json!({"name":"Denied mutation","parent_id":null,"role_codes":[]})),
    )
    .await;
    assert_eq!(status, StatusCode::UNAUTHORIZED);
    update_events.assert_empty();
    let (_, after_denied) =
        request(&app, "GET", &format!("{base}?ids={ids}"), &cookie, "", None).await;
    assert_eq!(
        after_denied["data"], listed_mcp,
        "denied mutation cannot reach target"
    );
    list_events.assert_call(
        json!({"query":{"ids":ids,"limit":100}}),
        &after_denied["data"],
    );
}
