use super::*;

async fn query_request(
    app: &axum::Router,
    uri: &str,
    cookie: Option<&str>,
    csrf: Option<&str>,
    body: Value,
) -> axum::response::Response {
    let mut request = Request::builder()
        .method("POST")
        .uri(uri)
        .header("content-type", "application/json");
    if let Some(cookie) = cookie {
        request = request.header("cookie", cookie);
    }
    if let Some(csrf) = csrf {
        request = request.header("x-csrf-token", csrf);
    }
    app.clone()
        .oneshot(request.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap()
}
async fn response_json(response: axum::response::Response) -> Value {
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1024 * 1024).await.unwrap();
    assert_eq!(
        status,
        StatusCode::OK,
        "{}",
        String::from_utf8_lossy(&bytes)
    );
    serde_json::from_slice(&bytes).unwrap()
}

async fn replace_log_read_policy(app: &axum::Router, cookie: &str, csrf: &str, scope: &str) {
    let operations = [
        "get_query_fields",
        "query_log_records",
        "query_log_conversations",
        "get_record_trace_tree",
        "list_record_trace_children",
        "get_log_record",
        "list_record_conversation_messages",
        "query_record_trajectory",
        "query_conversation_trajectory",
        "get_conversation_trajectory_section",
        "list_record_client_trajectory",
        "get_record_client_trajectory_section",
    ]
    .map(|operation_id| json!({"kind":"row", "operation_id":operation_id, "scope":scope}));
    let response = app.clone().oneshot(Request::builder().method("PUT")
        .uri("/api/console/settings/roles/log_query_viewer/console-policy")
        .header("cookie", cookie).header("x-csrf-token", csrf)
        .header("content-type", "application/json")
        .body(Body::from(json!({"groups":[{"kind":"settings_feature", "group_id":"system.applications", "enabled":true, "strategy":"custom", "operations":operations}]}).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn agent_logs_unified_query_console_auth_readonly_discovery_and_validation() {
    let (app, _) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let uri = "/api/console/applications/logs/records/query";
    assert_eq!(
        query_request(&app, uri, None, None, json!({}))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    assert_eq!(
        query_request(
            &app,
            uri,
            Some(&cookie),
            None,
            json!({"application_ids":[]})
        )
        .await
        .status(),
        StatusCode::OK,
        "read-only POST follows the existing query/preview credential contract"
    );
    assert_eq!(
        query_request(
            &app,
            uri,
            Some(&cookie),
            Some("wrong"),
            json!({"application_ids":[]})
        )
        .await
        .status(),
        StatusCode::OK,
        "CSRF protects mutation; an unused header cannot turn a read into a mutation"
    );
    let fields = get_console_json(
        &app,
        &cookie,
        "/api/console/applications/logs/query-fields".into(),
    )
    .await;
    assert!(fields["data"]["conversation_trajectory_fields"]
        .as_array()
        .unwrap()
        .iter()
        .any(|field| field["field"] == "is_subagent" && field["value_type"] == "boolean"));
    for (field, kind, sortable) in [
        ("application_id", "uuid", false),
        ("total_cost", "number", false),
        ("started_at", "datetime", true),
        ("requested_model_id", "string", false),
    ] {
        let actual = fields["data"]["record_fields"]
            .as_array()
            .unwrap()
            .iter()
            .find(|v| v["field"] == field)
            .unwrap();
        assert_eq!(actual["value_type"], kind);
        assert_eq!(actual["sortable"], sortable);
        assert!(actual["operators"]
            .as_array()
            .unwrap()
            .contains(&json!("$eq")));
    }
    assert_eq!(
        fields["data"]["trajectory_search_sections"],
        json!(["overview", "parameters", "result"])
    );
    assert_eq!(
        fields["data"]["record_keyword_fields"],
        json!(["title", "user_input", "final_output"])
    );
    for body in [
        json!({"application_ids":[],"filter":{"total_tokens":{"$includes":"12"}}}),
        json!({"application_ids":[],"filter":{"application_id":{"$eq":"not-a-uuid"}}}),
        json!({"application_ids":[],"sort_field":"title"}),
        json!({"application_ids":[],"cursor":"bogus"}),
    ] {
        let response = query_request(&app, uri, Some(&cookie), Some(&csrf), body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert!(error["code"].is_string());
    }
    let empty = response_json(
        query_request(
            &app,
            uri,
            Some(&cookie),
            Some(&csrf),
            json!({"application_ids":[]}),
        )
        .await,
    )
    .await;
    assert_eq!(empty["data"]["items"], json!([]));
    assert!(empty["data"]["next_cursor"].is_null());
    let conversations_uri = "/api/console/applications/logs/conversations/query";
    assert_eq!(
        query_request(&app, conversations_uri, None, None, json!({}))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let conversations = response_json(
        query_request(
            &app,
            conversations_uri,
            Some(&cookie),
            None,
            json!({"application_ids":[],"page":2,"page_size":3}),
        )
        .await,
    )
    .await;
    assert_eq!(
        conversations["data"],
        json!({"items":[],"total":0,"page":2,"page_size":3})
    );
    let larger_page = response_json(
        query_request(
            &app,
            conversations_uri,
            Some(&cookie),
            None,
            json!({"application_ids":[],"page_size":201}),
        )
        .await,
    )
    .await;
    assert_eq!(
        larger_page["data"]["page_size"], 201,
        "positive sizes have no arbitrary capacity cap"
    );
    for body in [
        json!({"application_ids":[],"page":0}),
        json!({"application_ids":[],"page_size":0}),
        json!({"application_ids":[],"page":3,"page_size":i64::MAX}),
        json!({"application_ids":[],"sort_field":"title"}),
        json!({"application_ids":[],"sort_field":"started_at; DROP TABLE applications"}),
        json!({"application_ids":[],"filter":{"application_id":{"$eq":"not-a-uuid"}}}),
        json!({"application_ids":[],"filter":{"total_tokens":{"$includes":"12"}}}),
    ] {
        let response = query_request(&app, conversations_uri, Some(&cookie), None, body).await;
        assert_eq!(response.status(), StatusCode::BAD_REQUEST);
        let error: Value =
            serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
                .unwrap();
        assert!(error["code"].is_string());
    }
}



#[tokio::test]
async fn agent_logs_unified_query_console_ingested_record_locator_wrong_application_and_legacy_readers(
) {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let response=app.clone().oneshot(Request::builder().method("POST").uri("/api/console/applications").header("cookie",&cookie).header("x-csrf-token",&csrf).header("content-type","application/json").body(Body::from(json!({"application_type":"agent_logs","name":"query fixture","description":""}).to_string())).unwrap()).await.unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    let value: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), 1024 * 1024).await.unwrap())
            .unwrap();
    let id = value["data"]["id"].as_str().unwrap();
    let application_id = Uuid::parse_str(id).unwrap();
    let pool = storage_durable_postgres::connect(&database_url)
        .await
        .unwrap();
    let scope: Uuid = sqlx::query_scalar("select workspace_id from applications where id=$1")
        .bind(application_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    let store = MainDurableStore::new(pool);
    let event=serde_json::from_value(json!({"event_id":"route-final","source_session_id":"session","source_task_id":"turn","sequence":1,"occurred_at":"2026-10-08T00:00:00Z","kind":"assistant","phase":"final_answer","content":"route needle","model_id":"gpt-route","reasoning_effort":"high","raw":{}})).unwrap();
    enable_agent_logs_fixture(&store, scope, application_id).await;
    let record = control_plane::agent_logs::AgentLogsService::new(store.clone())
        .ingest(
            application_id,
            scope,
            Uuid::now_v7(),
            control_plane_contracts::ports::AgentLogsBatch {
                schema_version: control_plane_contracts::ports::AGENT_LOGS_SCHEMA_VERSION.into(),
                source_id: "actual-route-source".into(),
                source_client: "codex".into(),
                events: vec![event],
            },
        )
        .await
        .unwrap()
        .record_ids[0];
    let second_response=app.clone().oneshot(Request::builder().method("POST").uri("/api/console/applications").header("cookie",&cookie).header("x-csrf-token",&csrf).header("content-type","application/json").body(Body::from(json!({"application_type":"agent_logs","name":"other query fixture","description":""}).to_string())).unwrap()).await.unwrap();
    assert_eq!(second_response.status(), StatusCode::CREATED);
    let second: Value = serde_json::from_slice(
        &to_bytes(second_response.into_body(), 1024 * 1024)
            .await
            .unwrap(),
    )
    .unwrap();
    let other_id = second["data"]["id"].as_str().unwrap();
    let other_application = Uuid::parse_str(other_id).unwrap();
    let other_event=serde_json::from_value(json!({"event_id":"other-final","source_session_id":"session","source_task_id":"turn","sequence":1,"occurred_at":"2026-10-08T00:00:00Z","kind":"assistant","phase":"final_answer","content":"other application secret","raw":{}})).unwrap();
    enable_agent_logs_fixture(&store, scope, other_application).await;
    let other_record = control_plane::agent_logs::AgentLogsService::new(store.clone())
        .ingest(
            other_application,
            scope,
            Uuid::now_v7(),
            control_plane_contracts::ports::AgentLogsBatch {
                schema_version: control_plane_contracts::ports::AGENT_LOGS_SCHEMA_VERSION.into(),
                source_id: "other-source".into(),
                source_client: "codex".into(),
                events: vec![other_event],
            },
        )
        .await
        .unwrap()
        .record_ids[0];
    let bounded=response_json(query_request(&app,"/api/console/applications/logs/records/query",Some(&cookie),Some(&csrf),json!({"application_ids":[id],"filter":{"$or":[{"application_id":{"$eq":other_id}},{"id":{"$eq":other_record}}]}})).await).await;
    assert_eq!(
        bounded["data"]["items"],
        json!([]),
        "OR predicates must remain inside the authorized application selection"
    );
    let result=response_json(query_request(&app,"/api/console/applications/logs/records/query",Some(&cookie),Some(&csrf),json!({"application_ids":[id],"keyword":"ROUTE%NEEDLE","filter":{"$and":[{"application_id":{"$eq":id}},{"$or":[{"requested_model_id":{"$eq":"gpt-route"}},{"source_kind":{"$eq":"native"}}]}]}})).await).await;
    assert_eq!(result["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(result["data"]["items"][0]["record_id"], record.to_string());
    assert_eq!(
        result["data"]["items"][0]["source_id"],
        "actual-route-source"
    );
    let uri =
        format!("/api/console/applications/{id}/logs/records/{record}/client-trajectory/query");
    let trajectory = response_json(
        query_request(
            &app,
            &uri,
            Some(&cookie),
            Some(&csrf),
            json!({"keyword":"route needle"}),
        )
        .await,
    )
    .await;
    let step = trajectory["data"]["items"][0]["id"].as_str().unwrap();
    assert_eq!(trajectory["data"]["matches"][0]["step_id"], step);
    assert_eq!(trajectory["data"]["matches"][0]["section"], "result");
    let overview = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{id}/logs/records/{record}"),
    )
    .await;
    assert_eq!(overview["data"]["parent_task_run_id"], Value::Null);
    assert!(overview["data"]["log_conversation_id"].is_string());
    let conversations = response_json(
        query_request(
            &app,
            "/api/console/applications/logs/conversations/query",
            Some(&cookie),
            None,
            json!({"application_ids":[id],"keyword":"route needle","page_size":1}),
        )
        .await,
    )
    .await;
    assert_eq!(conversations["data"]["total"], 1);
    assert_eq!(
        conversations["data"]["items"][0]["record_id"],
        record.to_string()
    );
    assert_eq!(
        conversations["data"]["items"][0]["log_conversation_id"],
        overview["data"]["log_conversation_id"]
    );
    let trace_uri = format!("/api/console/applications/{id}/logs/records/{record}/trace-tree");
    let tree = get_console_json(&app, &cookie, trace_uri.clone()).await;
    assert_eq!(tree["data"]["nodes"].as_array().unwrap().len(), 1);
    let node = &tree["data"]["nodes"][0];
    assert_eq!(node["record_id"], record.to_string());
    assert_eq!(node["trace_node_id"], record.to_string());
    assert!(node.get("flow_run_id").is_none());
    assert!(node.get("messages").is_none());
    assert_eq!(node["has_content"], true);
    let children_uri = format!("{trace_uri}/{record}/children");
    let children = get_console_json(&app, &cookie, children_uri.clone()).await;
    assert_eq!(children["data"]["items"], json!([]));
    assert_eq!(children["data"]["page_info"]["has_more"], false);
    for (url, expected) in [
        (trace_uri.clone(), StatusCode::UNAUTHORIZED),
        (
            format!("{children_uri}?page_size=0"),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("{children_uri}?cursor=forged"),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("{trace_uri}/{other_record}/children"),
            StatusCode::NOT_FOUND,
        ),
        (
            format!("/api/console/applications/{other_id}/logs/records/{record}/trace-tree"),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let mut req = Request::builder().uri(&url);
        if url != trace_uri {
            req = req.header("cookie", &cookie);
        }
        assert_eq!(
            app.clone()
                .oneshot(req.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            expected
        );
    }
    assert_eq!(overview["data"]["child_task_count"], 0);
    let conversation_uri =
        format!("/api/console/applications/{id}/logs/records/{record}/conversation/messages");
    let conversation = get_console_json(&app, &cookie, conversation_uri.clone()).await;
    assert_eq!(conversation["data"]["items"][0], overview["data"]);
    assert_eq!(
        conversation["data"]["items"][0]["requested_model_id"],
        "gpt-route"
    );
    assert_eq!(conversation["data"]["items"][0]["reasoning_effort"], "high");
    assert!(conversation["data"]["items"][0]["started_at"].is_string());
    assert_eq!(
        conversation["data"]["page"]["newest_cursor"],
        record.to_string()
    );
    for (url, cookie_value, status) in [
        (conversation_uri.clone(), None, StatusCode::UNAUTHORIZED),
        (
            format!("{conversation_uri}?scope=children"),
            Some(cookie.as_str()),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("{conversation_uri}?scope=invalid"),
            Some(cookie.as_str()),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("{conversation_uri}?scope=children&before={other_record}"),
            Some(cookie.as_str()),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("{conversation_uri}?before={other_record}"),
            Some(cookie.as_str()),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!("{conversation_uri}?before={record}&after={record}"),
            Some(cookie.as_str()),
            StatusCode::BAD_REQUEST,
        ),
        (
            format!(
                "/api/console/applications/{other_id}/logs/records/{record}/conversation/messages"
            ),
            Some(cookie.as_str()),
            StatusCode::NOT_FOUND,
        ),
    ] {
        let mut request = Request::builder().uri(url);
        if let Some(cookie) = cookie_value {
            request = request.header("cookie", cookie);
        }
        assert_eq!(
            app.clone()
                .oneshot(request.body(Body::empty()).unwrap())
                .await
                .unwrap()
                .status(),
            status
        );
    }
    assert_eq!(overview["data"]["source_id"], "actual-route-source");
    assert_eq!(overview["data"]["messages"][0]["content"], "route needle");
    let old = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{id}/logs/records/{record}/client-trajectory"),
    )
    .await;
    assert_eq!(old["data"]["items"][0]["id"], step);
    let section=get_console_json(&app,&cookie,format!("/api/console/applications/{id}/logs/records/{record}/client-trajectory/{step}?section=result")).await;
    assert_eq!(section["data"]["items"][0]["value"], "route needle");
    let wrong = format!(
        "/api/console/applications/{}/logs/records/{record}/client-trajectory/query",
        other_application
    );
    assert_eq!(
        query_request(&app, &wrong, Some(&cookie), Some(&csrf), json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    assert_eq!(
        query_request(
            &app,
            &uri,
            Some(&cookie),
            Some(&csrf),
            json!({"keyword":"route needle","search_sections":["raw"]})
        )
        .await
        .status(),
        StatusCode::BAD_REQUEST
    );
}

#[tokio::test]
async fn agent_logs_unified_query_view_own_all_without_list_keeps_workspace_and_record_acl() {
    use crate::_tests::support::{
        create_member, create_role, replace_member_roles, replace_role_permissions, seed_workspace,
    };
    use control_plane_contracts::ports::CreateApplicationInput;
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let member = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "log-query-viewer",
        "temp-pass",
    )
    .await;
    let member_id = Uuid::parse_str(&member).unwrap();
    create_role(&app, &root_cookie, &root_csrf, "log_query_viewer").await;
    replace_log_read_policy(&app, &root_cookie, &root_csrf, "own").await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member,
        &["log_query_viewer"],
    )
    .await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "log-query-viewer", "temp-pass").await;
    let root_session = get_console_json(&app, &root_cookie, "/api/console/session".into()).await;
    let root_id = Uuid::parse_str(root_session["data"]["actor"]["id"].as_str().unwrap()).unwrap();
    let session = get_console_json(&app, &cookie, "/api/console/session".into()).await;
    let workspace = Uuid::parse_str(
        session["data"]["session"]["current_workspace_id"]
            .as_str()
            .unwrap(),
    )
    .unwrap();
    let foreign_workspace = seed_workspace(&database_url, "Foreign log query workspace").await;
    let store = MainDurableStore::new(
        storage_durable_postgres::connect(&database_url)
            .await
            .unwrap(),
    );
    let mut fixtures = vec![];
    for (owner, scope, name) in [
        (member_id, workspace, "owned"),
        (root_id, workspace, "other-owner"),
        (member_id, foreign_workspace, "foreign-workspace"),
    ] {
        // Setup uses the real repository writer in this app's exclusive test DB.
        // HTTP authorization is exercised by the member's official session below.
        let application = store
            .create_application(&CreateApplicationInput {
                actor_user_id: owner,
                workspace_id: scope,
                application_type: domain::ApplicationType::AgentLogs,
                workflow_trigger_type: None,
                workflow_trigger_config: None,
                name: name.into(),
                description: "ACL fixture".into(),
                icon: None,
                icon_type: None,
                icon_background: None,
            })
            .await
            .unwrap();
        let event=serde_json::from_value(json!({"event_id":name,"source_session_id":"acl-session","source_task_id":"acl-turn","sequence":1,"occurred_at":"2026-10-08T00:00:00Z","kind":"assistant","phase":"final_answer","content":format!("{name} secret"),"raw":{}})).unwrap();
        enable_agent_logs_fixture(&store, scope, application.id).await;
        let record = control_plane::agent_logs::AgentLogsService::new(store.clone())
            .ingest(
                application.id,
                scope,
                Uuid::now_v7(),
                control_plane_contracts::ports::AgentLogsBatch {
                    schema_version: control_plane_contracts::ports::AGENT_LOGS_SCHEMA_VERSION
                        .into(),
                    source_id: name.into(),
                    source_client: "codex".into(),
                    events: vec![event],
                },
            )
            .await
            .unwrap()
            .record_ids[0];
        fixtures.push((application.id, record));
    }
    let conversations_uri = "/api/console/applications/logs/conversations/query";
    let owned_conversations =
        response_json(query_request(&app, conversations_uri, Some(&cookie), None, json!({})).await)
            .await;
    assert_eq!(owned_conversations["data"]["total"], 1);
    assert_eq!(
        owned_conversations["data"]["items"][0]["record_id"],
        fixtures[0].1.to_string()
    );
    let own_tree = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{}/logs/records/{}/trace-tree",
            fixtures[0].0, fixtures[0].1
        ),
    )
    .await;
    assert_eq!(
        own_tree["data"]["nodes"][0]["record_id"],
        fixtures[0].1.to_string()
    );
    for index in [1, 2] {
        assert_eq!(
            query_request(
                &app,
                conversations_uri,
                Some(&cookie),
                None,
                json!({"application_ids":[fixtures[index].0],"filter":{"id":{"$eq":fixtures[0].1}}})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
        for suffix in [
            "trace-tree".to_owned(),
            format!("trace-tree/{}/children", fixtures[index].1),
        ] {
            let url = format!(
                "/api/console/applications/{}/logs/records/{}/{}",
                fixtures[index].0, fixtures[index].1, suffix
            );
            assert_eq!(
                app.clone()
                    .oneshot(
                        Request::builder()
                            .uri(url)
                            .header("cookie", &cookie)
                            .body(Body::empty())
                            .unwrap()
                    )
                    .await
                    .unwrap()
                    .status(),
                StatusCode::NOT_FOUND
            );
        }
    }
    let uri = "/api/console/applications/logs/records/query";
    let implicit =
        response_json(query_request(&app, uri, Some(&cookie), Some(&csrf), json!({})).await).await;
    assert_eq!(implicit["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        implicit["data"]["items"][0]["record_id"],
        fixtures[0].1.to_string()
    );
    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        list.status(),
        StatusCode::FORBIDDEN,
        "VIEW must not silently grant the independent LIST operation"
    );
    let own_page = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{}/logs/records/{}/conversation/messages",
            fixtures[0].0, fixtures[0].1
        ),
    )
    .await;
    assert_eq!(
        own_page["data"]["items"][0]["record_id"],
        fixtures[0].1.to_string()
    );
    let own_cursor_page = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{}/logs/records/{}/conversation/messages?after={}",
            fixtures[0].0, fixtures[0].1, fixtures[0].1
        ),
    )
    .await;
    assert_eq!(own_cursor_page["data"]["items"], json!([]));
    for index in [1, 2] {
        let cursor_denied = format!(
            "/api/console/applications/{}/logs/records/{}/conversation/messages?after={}",
            fixtures[index].0, fixtures[index].1, fixtures[0].1
        );
        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri(cursor_denied)
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        let denied = format!(
            "/api/console/applications/{}/logs/records/{}/conversation/messages",
            fixtures[index].0, fixtures[index].1
        );
        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri(denied)
                        .header("cookie", &cookie)
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            StatusCode::NOT_FOUND
        );
        let explicit = query_request(
            &app,
            uri,
            Some(&cookie),
            Some(&csrf),
            json!({"application_ids":[fixtures[index].0],"filter":{"id":{"$eq":fixtures[0].1}}}),
        )
        .await;
        assert_eq!(
            explicit.status(),
            StatusCode::NOT_FOUND,
            "every explicit application must be authorized even with an impossible filter"
        );
        let locator = format!(
            "/api/console/applications/{}/logs/records/{}/client-trajectory/query",
            fixtures[index].0, fixtures[index].1
        );
        assert_eq!(
            query_request(
                &app,
                &locator,
                Some(&cookie),
                Some(&csrf),
                json!({"keyword":"secret"})
            )
            .await
            .status(),
            StatusCode::NOT_FOUND
        );
    }
    let or=response_json(query_request(&app,uri,Some(&cookie),Some(&csrf),json!({"filter":{"$or":[{"application_id":{"$eq":fixtures[1].0}},{"application_id":{"$eq":fixtures[2].0}},{"id":{"$eq":fixtures[0].1}}]}})).await).await;
    assert_eq!(or["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        or["data"]["items"][0]["record_id"],
        fixtures[0].1.to_string()
    );
    let owned_locator = format!(
        "/api/console/applications/{}/logs/records/{}/client-trajectory/query",
        fixtures[0].0, fixtures[0].1
    );
    let own_trajectory = response_json(
        query_request(
            &app,
            &owned_locator,
            Some(&cookie),
            Some(&csrf),
            json!({"keyword":"owned secret"}),
        )
        .await,
    )
    .await;
    assert_eq!(own_trajectory["data"]["items"].as_array().unwrap().len(), 1);
    let own_overview = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{}/logs/records/{}",
            fixtures[0].0, fixtures[0].1
        ),
    )
    .await;
    assert_eq!(
        own_overview["data"]["messages"][0]["content"],
        "owned secret"
    );
    let canonical = own_overview["data"]["log_conversation_id"]
        .as_str()
        .unwrap();
    let timeline_uri = format!(
        "/api/console/applications/{}/logs/conversations/{canonical}/client-trajectory/query",
        fixtures[0].0
    );
    assert_eq!(
        query_request(&app, &timeline_uri, None, None, json!({}))
            .await
            .status(),
        StatusCode::UNAUTHORIZED
    );
    let timeline = response_json(
        query_request(
            &app,
            &timeline_uri,
            Some(&cookie),
            None,
            json!({"keyword":"owned secret","search_sections":["result"],"limit":1,
            "focus_record_id":fixtures[0].1}),
        )
        .await,
    )
    .await;
    assert_eq!(timeline["data"]["log_conversation_id"], canonical);
    assert_eq!(timeline["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        timeline["data"]["items"][0]["record_id"],
        fixtures[0].1.to_string()
    );
    assert_eq!(timeline["data"]["items"][0]["is_subagent"], false);
    for other in [1, 2] {
        let denied = format!(
            "/api/console/applications/{}/logs/conversations/{canonical}/client-trajectory/query",
            fixtures[other].0
        );
        assert_eq!(
            query_request(&app, &denied, Some(&cookie), None, json!({}))
                .await
                .status(),
            StatusCode::NOT_FOUND
        );
        assert_eq!(
            query_request(
                &app,
                &timeline_uri,
                Some(&cookie),
                None,
                json!({"focus_record_id":fixtures[other].1})
            )
            .await
            .status(),
            StatusCode::BAD_REQUEST
        );
    }
    let own_steps = get_console_json(
        &app,
        &cookie,
        format!(
            "/api/console/applications/{}/logs/records/{}/client-trajectory",
            fixtures[0].0, fixtures[0].1
        ),
    )
    .await;
    let own_step = own_steps["data"]["items"][0]["id"].as_str().unwrap();
    let own_section = get_console_json(
        &app,
        &cookie,
        format!("/api/console/applications/{}/logs/records/{}/client-trajectory/{own_step}?section=result", fixtures[0].0, fixtures[0].1),
    ).await;
    assert_eq!(own_section["data"]["items"][0]["value"], "owned secret");
    let canonical_section_uri = format!(
        "/api/console/applications/{}/logs/conversations/{canonical}/client-trajectory/{own_step}?section=result",
        fixtures[0].0
    );
    let canonical_section = get_console_json(&app, &cookie, canonical_section_uri).await;
    assert_eq!(canonical_section["data"], own_section["data"]);
    let foreign_section_uri = format!(
        "/api/console/applications/{}/logs/conversations/{}/client-trajectory/{own_step}?section=result",
        fixtures[0].0, Uuid::new_v4()
    );
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(foreign_section_uri)
                    .header("cookie", &cookie)
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::NOT_FOUND
    );

    let wrong_record = format!(
        "/api/console/applications/{}/logs/records/{}/client-trajectory/query",
        fixtures[0].0, fixtures[1].1
    );
    assert_eq!(
        query_request(&app, &wrong_record, Some(&cookie), Some(&csrf), json!({}))
            .await
            .status(),
        StatusCode::NOT_FOUND
    );
    replace_log_read_policy(&app, &root_cookie, &root_csrf, "scope_all").await;
    let (all_cookie, all_csrf) =
        login_and_capture_cookie(&app, "log-query-viewer", "temp-pass").await;
    let all = response_json(
        query_request(&app, uri, Some(&all_cookie), Some(&all_csrf), json!({})).await,
    )
    .await;
    let mut actual = all["data"]["items"]
        .as_array()
        .unwrap()
        .iter()
        .map(|r| r["record_id"].as_str().unwrap().to_owned())
        .collect::<Vec<_>>();
    actual.sort();
    let mut expected = vec![fixtures[0].1.to_string(), fixtures[1].1.to_string()];
    expected.sort();
    assert_eq!(
        actual, expected,
        "VIEW all still means current workspace only"
    );
    let other_locator = format!(
        "/api/console/applications/{}/logs/records/{}/client-trajectory/query",
        fixtures[1].0, fixtures[1].1
    );
    let other = response_json(
        query_request(
            &app,
            &other_locator,
            Some(&all_cookie),
            Some(&all_csrf),
            json!({"keyword":"other-owner secret"}),
        )
        .await,
    )
    .await;
    assert_eq!(other["data"]["items"].as_array().unwrap().len(), 1);
    assert_eq!(
        query_request(
            &app,
            uri,
            Some(&all_cookie),
            Some(&all_csrf),
            json!({"application_ids":[fixtures[2].0]})
        )
        .await
        .status(),
        StatusCode::NOT_FOUND
    );
    let all_page = get_console_json(
        &app,
        &all_cookie,
        format!(
            "/api/console/applications/{}/logs/records/{}/conversation/messages",
            fixtures[1].0, fixtures[1].1
        ),
    )
    .await;
    assert_eq!(
        all_page["data"]["items"][0]["record_id"],
        fixtures[1].1.to_string()
    );
    let all_cursor_page = get_console_json(
        &app,
        &all_cookie,
        format!(
            "/api/console/applications/{}/logs/records/{}/conversation/messages?after={}",
            fixtures[1].0, fixtures[1].1, fixtures[1].1
        ),
    )
    .await;
    assert_eq!(all_cursor_page["data"]["items"], json!([]));
    for (anchor_index, cursor_index, status) in [
        (1, 0, StatusCode::BAD_REQUEST),
        (2, 1, StatusCode::NOT_FOUND),
    ] {
        let url = format!(
            "/api/console/applications/{}/logs/records/{}/conversation/messages?after={}",
            fixtures[anchor_index].0, fixtures[anchor_index].1, fixtures[cursor_index].1
        );
        assert_eq!(
            app.clone()
                .oneshot(
                    Request::builder()
                        .uri(url)
                        .header("cookie", &all_cookie)
                        .body(Body::empty())
                        .unwrap()
                )
                .await
                .unwrap()
                .status(),
            status
        );
    }
    replace_role_permissions(&app, &root_cookie, &root_csrf, "log_query_viewer", &[]).await;
    let (denied_cookie, denied_csrf) =
        login_and_capture_cookie(&app, "log-query-viewer", "temp-pass").await;
    assert_eq!(
        app.clone()
            .oneshot(
                Request::builder()
                    .uri(format!(
                        "/api/console/applications/{}/logs/records/{}/conversation/messages",
                        fixtures[0].0, fixtures[0].1
                    ))
                    .header("cookie", &denied_cookie)
                    .body(Body::empty())
                    .unwrap()
            )
            .await
            .unwrap()
            .status(),
        StatusCode::FORBIDDEN
    );
    assert_eq!(
        query_request(
            &app,
            uri,
            Some(&denied_cookie),
            Some(&denied_csrf),
            json!({})
        )
        .await
        .status(),
        StatusCode::FORBIDDEN
    );
}
