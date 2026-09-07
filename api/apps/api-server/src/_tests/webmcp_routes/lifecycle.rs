use super::*;
use interface_runtime::{
    InterfaceInvocationReceipt, InterfaceInvocationStage, InterfaceInvocationTerminal,
};

fn outer_receipt(
    response: &axum::response::Response,
    binding: &str,
    executed: bool,
) -> InterfaceInvocationReceipt {
    let receipt = response
        .extensions()
        .get::<InterfaceInvocationReceipt>()
        .expect("WebMCP must expose its own frozen outer invocation receipt to server observers")
        .clone();
    assert_eq!(receipt.resolved().unwrap().binding_id().as_str(), binding);
    assert_eq!(
        receipt
            .stages()
            .filter(|stage| *stage == InterfaceInvocationStage::Executing)
            .count(),
        usize::from(executed)
    );
    assert_eq!(
        receipt
            .stages()
            .filter(|stage| *stage == InterfaceInvocationStage::Projected)
            .count(),
        1
    );
    assert_eq!(
        receipt
            .stages()
            .filter(|stage| *stage == InterfaceInvocationStage::PrincipalEstablished)
            .count(),
        1
    );
    receipt
}

async fn post(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    path: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(path)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}

#[tokio::test]
async fn root_1998_webmcp_outer_lifecycle_covers_all_operations_and_scope_rejections() {
    let (state, database_url) = crate::_tests::support::test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let created = post(
        &app,
        &cookie,
        &csrf,
        "/api/console/mcp/instances",
        json!({
            "instance_id":"outer_lifecycle", "name":"Outer lifecycle", "description_short":null,
            "status":"enabled", "default_entry_path":"/", "webmcp_exposure":"authenticated_session"
        }),
    )
    .await;
    assert_eq!(created.status(), StatusCode::CREATED);
    let tool = post(&app, &cookie, &csrf, "/api/console/mcp/tools", json!({
        "tool_id":"outer_runtime_profile", "des_id":"outer_runtime_profile_description", "name":"Runtime profile",
        "short_description":"Read runtime profile.", "full_description":"Read system runtime topology and locale profile.",
        "execution_target":{"kind":"interface_wrapper","interface_id":"get_runtime_profile"},
        "parameter_schema":{}, "result_schema":{}, "input_mapping":{}, "output_mapping":{},
        "permission_code":null, "risk_level":"low", "status":"enabled"
    })).await;
    assert_eq!(
        tool.status(),
        StatusCode::CREATED,
        "{}",
        response_json(tool).await
    );
    let binding = post(&app, &cookie, &csrf, "/api/console/mcp/instances/outer_lifecycle/tool-bindings", json!({
        "group_path":"/", "tool_id":"outer_runtime_profile", "display_alias":null, "visible":true, "sort_order":0
    })).await;
    assert_eq!(binding.status(), StatusCode::CREATED);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/webmcp/registrations")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let registration_receipt = outer_receipt(&response, "http.webmcp.registrations.v1", true);
    assert_eq!(
        registration_receipt.terminal(),
        InterfaceInvocationTerminal::Completed
    );

    let mut ids = std::collections::BTreeSet::new();
    ids.insert(registration_receipt.invocation_id().value());
    for (operation, arguments, is_error) in [
        ("list", json!({"path":"/"}), false),
        ("get", json!({"tool_id":"outer_runtime_profile"}), false),
        (
            "result",
            json!({"result_ref":"00000000-0000-0000-0000-000000000001"}),
            true,
        ),
        (
            "call",
            json!({"tool_id":"outer_runtime_profile","arguments":{}}),
            false,
        ),
    ] {
        let response = post(
            &app,
            &cookie,
            &csrf,
            &format!("/api/webmcp/outer_lifecycle/tools/{operation}"),
            json!({"arguments":arguments}),
        )
        .await;
        assert_eq!(response.status(), StatusCode::OK, "{operation}");
        let receipt = outer_receipt(&response, "http.webmcp.tools.invoke.v1", true);
        assert_eq!(
            receipt.interface_id().unwrap().as_str(),
            "webmcp.tools.invoke"
        );
        assert_eq!(receipt.terminal(), InterfaceInvocationTerminal::Completed);
        assert!(ids.insert(receipt.invocation_id().value()));
        let body = response_json(response).await;
        assert_eq!(
            body["data"]["is_error"],
            json!(is_error),
            "{operation}: {body}"
        );
    }
    let invalid = post(
        &app,
        &cookie,
        &csrf,
        "/api/webmcp/outer_lifecycle/tools/unknown",
        json!({"arguments":{}}),
    )
    .await;
    assert_eq!(invalid.status(), StatusCode::BAD_REQUEST);
    assert_eq!(
        outer_receipt(&invalid, "http.webmcp.tools.invoke.v1", true).terminal(),
        InterfaceInvocationTerminal::Failed
    );
    assert_eq!(
        response_json(invalid).await["code"],
        json!("webmcp_operation")
    );

    // The actor's selected workspace is retained through both the outer invocation and delegation.
    let workspace =
        crate::_tests::support::seed_workspace(&database_url, "WebMCP other workspace").await;
    let switched = post(
        &app,
        &cookie,
        &csrf,
        "/api/console/session/actions/switch-workspace",
        json!({"workspace_id":workspace}),
    )
    .await;
    assert_eq!(switched.status(), StatusCode::OK);
    let other_csrf = response_json(switched).await["data"]["csrf_token"]
        .as_str()
        .unwrap()
        .to_string();
    let scoped = post(
        &app,
        &cookie,
        &other_csrf,
        "/api/webmcp/outer_lifecycle/tools/list",
        json!({"arguments":{}}),
    )
    .await;
    assert_eq!(scoped.status(), StatusCode::NOT_FOUND);
    assert_eq!(
        outer_receipt(&scoped, "http.webmcp.tools.invoke.v1", true).terminal(),
        InterfaceInvocationTerminal::Failed
    );
    assert_eq!(
        response_json(scoped).await["code"],
        json!("webmcp_registration")
    );
}
