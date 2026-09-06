use super::*;

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
                .body(if body.is_null() {
                    Body::empty()
                } else {
                    Body::from(body.to_string())
                })
                .unwrap(),
        )
        .await
        .unwrap();
    (response.status(), response_json(response).await)
}

async fn assert_catalog_contract(app: &Router, cookie: &str, interface_id: &str, output_key: &str) {
    let (status, detail) = request(
        app,
        "GET",
        &format!("/api/console/frontstage/interface-capabilities/{interface_id}"),
        cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(
        status,
        StatusCode::OK,
        "published workflow must be discoverable: {detail}"
    );
    let (status, catalog) = request(
        app,
        "GET",
        "/api/console/mcp/interface-capabilities",
        cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{catalog}");
    let matches: Vec<_> = catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|entry| entry["interface_id"] == interface_id)
        .collect();
    assert_eq!(
        matches.len(),
        1,
        "a publication must have exactly one MCP projection"
    );
    for key in [
        "interface_id",
        "method",
        "path",
        "parameter_schema",
        "result_schema",
        "bindable",
    ] {
        assert_eq!(
            detail["data"][key], matches[0][key],
            "contract drift at {key}"
        );
    }
    assert_eq!(
        detail["data"]["result_schema"]["required"],
        json!([output_key])
    );
    assert_eq!(detail["data"]["host_injected_parameters"], json!([]));
    let (status, listed) = request(
        app,
        "GET",
        "/api/console/frontstage/interface-capabilities?path_query=callable-ticket",
        cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{listed}");
    assert_eq!(listed["data"]["total"], 1);
    assert_eq!(
        listed["data"]["items"][0]["path"],
        "/api/ex/callable-ticket"
    );
}

async fn invoke_mcp_workflow(app: &Router, token: &str, des_id: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/mcp/callable-test")
                .header("authorization", format!("Bearer {token}"))
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"jsonrpc":"2.0","id":1,"method":"tools/call",
            "params":{"name":"mcp_call","arguments":{"tool_id":"workflow_test",
                "des_id":des_id,"arguments":{"customer_id":"C-42"}}}})
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    response_json(response).await
}

// AC: publication, republish and disable use one contract in MCP and Frontstage;
// the Frontstage dispatch executes the existing Workflow endpoint with session + CSRF.
#[tokio::test]
async fn workflow_callable_catalog_tracks_publication_and_dispatch_lifecycle() {
    let (app, state) = test_app_with_state().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let application_id = create_workflow_application(&app, &cookie, &csrf, "callable-ticket").await;
    save_workflow_document(&app, &cookie, &csrf, &application_id).await;
    let publication = publish_workflow_extension(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "callable-ticket",
        "sync",
    )
    .await;
    let interface_id = publication["data"]["operation"]["interface_id"]
        .as_str()
        .unwrap();
    assert_catalog_contract(&app, &cookie, interface_id, "ticket_id").await;

    // Boot also consumes the dynamic OpenAPI document when publications already exist.
    let (status, document) = request(&app, "GET", "/openapi.json", &cookie, "", Value::Null).await;
    assert_eq!(status, StatusCode::OK);
    let registry = state
        .extension_boot_snapshot
        .as_ref()
        .unwrap()
        .interface_registry()
        .unwrap()
        .snapshot();
    let assembly = crate::routes::console_route_assembly::migrated_core_console_route_assembly_with_interface_operations(
        Some(registry.as_ref()),
    );
    let _restarted = crate::app_with_state_and_config_and_console_route_assembly(
        Arc::clone(&state),
        &test_config(),
        assembly,
        &document,
    );
    // The existing state's boot snapshot is immutable; inspect a fresh compilation.
    let mut compiler = crate::external_endpoint_catalog::ExternalEndpointCatalogCompiler::default();
    compiler
        .contribute_openapi_document("published-workflow-openapi", &document)
        .unwrap();
    compiler
        .absorb_registry("registry", registry.as_ref())
        .unwrap();
    compiler.contribute_approved_controls(true).unwrap();
    let catalog = compiler.compile_complete(registry.as_ref()).unwrap();
    let row = catalog
        .row(
            &crate::external_endpoint_catalog::ExternalEndpointIdentity::http(
                "POST",
                "/api/ex/callable-ticket",
            ),
        )
        .unwrap();
    assert_eq!(
        row.binding_id(),
        Some(crate::routes::application_public_api::workflow_extension_interface::BINDING_ID)
    );

    for (path, body) in [
        (
            "/api/console/mcp/instances",
            json!({"instance_id":"callable-test",
            "name":"Workflow test","status":"enabled","default_entry_path":"/"}),
        ),
        (
            "/api/console/mcp/instances/callable-test/groups",
            json!({"path":"/runtime",
            "enabled":true,"sort_order":0}),
        ),
        (
            "/api/console/mcp/tools",
            json!({"tool_id":"workflow_test","des_id":"workflow_test_description",
            "name":"Workflow test","short_description":"Invoke the published Workflow",
            "full_description":"","execution_target":{"kind":"interface_wrapper","interface_id":interface_id},
            "parameter_schema":{},"result_schema":{},"input_mapping":{
                "interface_parameters":[{"name":"customer_id","field_type":"string","parameter_type":"json_body","required":true}],
                "mappings":[{"interface_param":"customer_id","mcp_param":"customer_id","required":true}]},
            "output_mapping":{},"permission_code":null,"risk_level":"low","status":"enabled"}),
        ),
        (
            "/api/console/mcp/instances/callable-test/tool-bindings",
            json!({"group_path":"/runtime",
            "tool_id":"workflow_test","visible":true,"sort_order":0}),
        ),
    ] {
        let (status, payload) = request(&app, "POST", path, &cookie, &csrf, body).await;
        assert!(status.is_success(), "{path}: {status} {payload}");
    }
    let token = create_user_api_key(&app, &cookie, &csrf).await;
    let mcp_output = invoke_mcp_workflow(&app, &token, "workflow_test_description").await;
    assert_eq!(mcp_output["result"]["isError"], false, "{mcp_output}");
    assert_eq!(
        mcp_output["result"]["structuredContent"],
        json!({"ticket_id":"ticket-C-42"})
    );

    let other_workspace =
        crate::openapi_interface::build_openapi_capability_catalog(&state, Uuid::now_v7())
            .await
            .unwrap();
    assert!(!other_workspace
        .iter()
        .any(|entry| entry.interface.operation_id == interface_id));

    let (status, page) = request(
        &app,
        "POST",
        "/api/console/frontstage/pages",
        &cookie,
        &csrf,
        json!({"title":"Workflow consumer", "rank":"a", "placement":"topbar","slug":"workflow-consumer"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{page}");
    let page_id = page["data"]["page"]["id"].as_str().unwrap();
    let tab_id = page["data"]["default_tab"]["id"].as_str().unwrap();
    let (status, block) = request(
        &app,
        "POST",
        &format!("/api/console/frontstage/pages/{page_id}/blocks"),
        &cookie,
        &csrf,
        json!({"tab_id":tab_id,"title":"Workflow report","presentation":"inline",
            "source_code":"export default function Report() { return null; }"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{block}");
    let dispatch_path = format!(
        "/api/console/frontstage/pages/{page_id}/tabs/{tab_id}/callable-interfaces/dispatch"
    );
    let invocation = json!({"block_id":block["data"]["block_id"], "method":"POST",
        "path":"/api/ex/callable-ticket", "request":{"body":{"customer_id":"C-42"}}});
    let (status, output) = request(
        &app,
        "POST",
        &dispatch_path,
        &cookie,
        &csrf,
        invocation.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{output}");
    assert_eq!(output["data"], json!({"ticket_id":"ticket-C-42"}));
    let (status, _) = request(
        &app,
        "POST",
        &dispatch_path,
        &cookie,
        "",
        invocation.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let mut invalid = invocation.clone();
    invalid["request"]["body"] = json!({});
    let (status, _) = request(&app, "POST", &dispatch_path, &cookie, &csrf, invalid).await;
    assert_eq!(status, StatusCode::BAD_REQUEST);

    save_workflow_document_with_builder(&app, &cookie, &csrf, &application_id, |flow_id| {
        let mut document = workflow_document(flow_id);
        document["graph"]["nodes"][2]["bindings"] = json!({
            "reference": {"kind":"selector","value":["node-transform","ticket_id"]}
        });
        document["graph"]["nodes"][2]["outputs"] = json!([
            {"key":"reference","title":"Reference","valueType":"string"}
        ]);
        document
    })
    .await;
    publish_workflow_extension(
        &app,
        &cookie,
        &csrf,
        &application_id,
        "callable-ticket",
        "sync",
    )
    .await;
    assert_catalog_contract(&app, &cookie, interface_id, "reference").await;
    let (status, output) = request(
        &app,
        "POST",
        &dispatch_path,
        &cookie,
        &csrf,
        invocation.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{output}");
    assert_eq!(output["data"], json!({"reference":"ticket-C-42"}));

    publish_workflow_extension_with_enabled(
        &app,
        &cookie,
        &csrf,
        &application_id,
        WorkflowExtensionPublishOptions::new("callable-ticket", "sync", false),
    )
    .await;
    let (status, _) = request(
        &app,
        "GET",
        &format!("/api/console/frontstage/interface-capabilities/{interface_id}"),
        &cookie,
        "",
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    let (_, catalog) = request(
        &app,
        "GET",
        "/api/console/mcp/interface-capabilities",
        &cookie,
        "",
        Value::Null,
    )
    .await;
    assert!(!catalog["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["interface_id"] == interface_id));
    let (status, _) = request(&app, "POST", &dispatch_path, &cookie, &csrf, invocation).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
}
