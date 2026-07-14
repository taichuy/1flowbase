use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    Router,
    body::{Body, to_bytes},
    http::{Request, StatusCode},
};
use serde_json::{Value, json};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    serde_json::from_slice(&body).unwrap()
}

async fn create_application(app: &Router, cookie: &str, csrf: &str, name: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": name,
                        "description": "application api docs test",
                        "icon": null,
                        "icon_type": null,
                        "icon_background": null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"]["id"]
        .as_str()
        .unwrap()
        .to_string()
}

async fn publish_application(app: &Router, cookie: &str, csrf: &str, application_id: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/api-publications"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "mapping": {
                            "input": {
                                "query_target": "node-start.query",
                                "model_target": null,
                                "inputs_target": "node-start",
                                "history_target": "node-start.history",
                                "attachments_target": "node-start.files"
                            },
                            "output": {
                                "answer_selector": "answer",
                                "usage_selector": "usage",
                                "files_selector": null,
                                "error_selector": "error"
                            }
                        },
                        "api_enabled": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn setup_published_app(app: &Router) -> (String, String) {
    let (cookie, csrf) = login_and_capture_cookie(app, "root", "change-me").await;
    let application_id = create_application(app, &cookie, &csrf, "Application API Docs App").await;
    publish_application(app, &cookie, &csrf, &application_id).await;
    (cookie, application_id)
}

#[tokio::test]
async fn application_api_docs_catalog_lists_public_api_categories() {
    let app = test_app().await;
    let (cookie, application_id) = setup_published_app(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/catalog"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let payload = response_json(response).await;
    let labels = payload["data"]["categories"]
        .as_array()
        .unwrap()
        .iter()
        .map(|category| category["label"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(labels.contains(&"Application Native API"));
    assert!(labels.contains(&"OpenAI Compatible API"));
    assert!(labels.contains(&"Anthropic Compatible API"));
}

#[tokio::test]
async fn application_api_docs_category_and_operation_specs_use_public_paths_only() {
    let app = test_app().await;
    let (cookie, application_id) = setup_published_app(&app).await;

    let operations = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/categories/openai-compatible-api/operations"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operations.status(), StatusCode::OK);
    let operations_payload = response_json(operations).await;
    assert_eq!(
        operations_payload["data"]["operations"][0]["path"],
        json!("/v1/chat/completions")
    );

    let spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationOpenAiCreateChatCompletion/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spec.status(), StatusCode::OK);
    let spec_payload = response_json(spec).await;
    assert!(spec_payload["paths"].get("/v1/chat/completions").is_some());
    assert!(
        spec_payload["paths"]
            .as_object()
            .unwrap()
            .keys()
            .all(|path| !path.contains("application_id"))
    );
    assert_eq!(
        spec_payload["x-1flowbase-application"]["api_enabled"],
        json!(true)
    );
    assert_eq!(
        spec_payload["x-1flowbase-application"]["mapping"]["model_target"],
        Value::Null
    );
    let description = spec_payload["paths"]["/v1/chat/completions"]["post"]["description"]
        .as_str()
        .unwrap();
    assert!(description.contains("forwards tools, tool_choice, and function_call"));
}

#[tokio::test]
async fn application_api_docs_list_models_and_streaming_contracts() {
    let app = test_app().await;
    let (cookie, application_id) = setup_published_app(&app).await;

    let operations = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/categories/openai-compatible-api/operations"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operations.status(), StatusCode::OK);
    let operations_payload = response_json(operations).await;
    let operation_paths = operations_payload["data"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|operation| {
            (
                operation["method"].as_str().unwrap(),
                operation["path"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert!(operation_paths.contains(&("GET", "/v1/models")));
    assert!(operation_paths.contains(&("POST", "/v1/chat/completions")));
    assert!(!operation_paths.contains(&("POST", "/openai/v1/chat/completions")));
    assert!(operation_paths.contains(&("POST", "/v1/responses")));

    let models_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationOpenAiListModels/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(models_spec.status(), StatusCode::OK);
    let models_payload = response_json(models_spec).await;
    let model_list_response = &models_payload["paths"]["/v1/models"]["get"]["responses"]["200"]["content"]
        ["application/json"]["schema"];
    assert_eq!(model_list_response["required"], json!(["object", "data"]));
    assert_eq!(
        model_list_response["properties"]["data"]["items"]["properties"]["id"]["type"],
        json!("string")
    );

    let native_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationNativeCreateRun/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(native_spec.status(), StatusCode::OK);
    let native_payload = response_json(native_spec).await;
    let native_streaming_schema = &native_payload["paths"]["/api/agent/v1/runs"]["post"]["responses"]
        ["201"]["content"]["text/event-stream"]["schema"];
    assert_eq!(
        native_streaming_schema["x-1flowbase-heartbeat"],
        json!(true)
    );
    assert_eq!(
        native_streaming_schema["x-1flowbase-events"],
        json!([
            "run.started",
            "reasoning.delta",
            "message.delta",
            "workflow.event",
            "required_action",
            "run.completed",
            "run.failed",
            "run.cancelled"
        ])
    );

    let openai_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationOpenAiCreateChatCompletion/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(openai_spec.status(), StatusCode::OK);
    let openai_payload = response_json(openai_spec).await;
    let openai_streaming_schema = &openai_payload["paths"]["/v1/chat/completions"]["post"]["responses"]
        ["200"]["content"]["text/event-stream"]["schema"];
    assert_eq!(
        openai_streaming_schema["x-1flowbase-heartbeat"],
        json!({"interval_seconds": 10, "text": "heartbeat"})
    );
    assert_eq!(
        openai_streaming_schema["x-1flowbase-reasoning-delta"],
        json!("choices[0].delta.reasoning_content")
    );

    let responses_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationOpenAiCreateResponse/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(responses_spec.status(), StatusCode::OK);
    let responses_payload = response_json(responses_spec).await;
    assert!(responses_payload["paths"].get("/v1/responses").is_some());
    let responses_streaming_schema = &responses_payload["paths"]["/v1/responses"]["post"]["responses"]
        ["200"]["content"]["text/event-stream"]["schema"];
    assert_eq!(
        responses_streaming_schema["x-1flowbase-message-delta"],
        json!("response.output_text.delta")
    );
}

#[tokio::test]
async fn application_api_docs_anthropic_operation_advertises_x_api_key_auth() {
    let app = test_app().await;
    let (cookie, application_id) = setup_published_app(&app).await;

    let spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationAnthropicCreateMessage/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spec.status(), StatusCode::OK);
    let spec_payload = response_json(spec).await;

    assert_eq!(
        spec_payload["paths"]["/v1/messages"]["post"]["security"],
        json!([
            {"applicationApiKey": []},
            {"anthropicApplicationApiKey": []}
        ])
    );
    assert_eq!(
        spec_payload["components"]["securitySchemes"]["anthropicApplicationApiKey"],
        json!({
            "type": "apiKey",
            "in": "header",
            "name": "x-api-key",
            "description": "Use an application API key created from this application API tab."
        })
    );
}

#[tokio::test]
async fn application_api_docs_registers_anthropic_count_tokens_operation() {
    let app = test_app().await;
    let (cookie, application_id) = setup_published_app(&app).await;

    let operations = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/categories/anthropic-compatible-api/operations"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operations.status(), StatusCode::OK);
    let operations_payload = response_json(operations).await;
    let operation_paths = operations_payload["data"]["operations"]
        .as_array()
        .unwrap()
        .iter()
        .map(|operation| {
            (
                operation["id"].as_str().unwrap(),
                operation["method"].as_str().unwrap(),
                operation["path"].as_str().unwrap(),
            )
        })
        .collect::<Vec<_>>();
    assert!(operation_paths.contains(&(
        "applicationAnthropicCountMessageTokens",
        "POST",
        "/v1/messages/count_tokens"
    )));

    let spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationAnthropicCountMessageTokens/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spec.status(), StatusCode::OK);
    let spec_payload = response_json(spec).await;
    let operation = &spec_payload["paths"]["/v1/messages/count_tokens"]["post"];
    assert_eq!(
        operation["requestBody"]["content"]["application/json"]["schema"]["required"],
        json!(["model", "messages"])
    );
    assert_eq!(
        operation["responses"]["200"]["content"]["application/json"]["schema"]["required"],
        json!(["input_tokens"])
    );
    assert_eq!(
        operation["security"],
        json!([
            {"applicationApiKey": []},
            {"anthropicApplicationApiKey": []}
        ])
    );
}

#[tokio::test]
async fn application_api_docs_operation_specs_include_request_parameters() {
    let app = test_app().await;
    let (cookie, application_id) = setup_published_app(&app).await;

    let create_run_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationNativeCreateRun/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_run_spec.status(), StatusCode::OK);
    let create_run_payload = response_json(create_run_spec).await;
    let create_run_body = &create_run_payload["paths"]["/api/agent/v1/runs"]["post"]["requestBody"]
        ["content"]["application/json"]["schema"];
    assert_eq!(create_run_body["required"], json!(["query"]));
    assert_eq!(
        create_run_body["properties"]["query"]["type"],
        json!("string")
    );
    assert_eq!(
        create_run_body["properties"]["response_mode"]["enum"],
        json!(["blocking", "streaming"])
    );
    assert_eq!(
        create_run_body["properties"]["expand_id"]["type"],
        json!("string")
    );
    assert!(create_run_body["properties"].get("user_id").is_none());
    assert_eq!(
        create_run_body["properties"]["title"]["maxLength"],
        json!(255)
    );
    assert_eq!(
        create_run_body["properties"]["attachments"]["items"]["properties"]["value"]["type"],
        json!("string")
    );

    let openai_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationOpenAiCreateChatCompletion/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(openai_spec.status(), StatusCode::OK);
    let openai_payload = response_json(openai_spec).await;
    let openai_body = &openai_payload["paths"]["/v1/chat/completions"]["post"]["requestBody"]["content"]
        ["application/json"]["schema"];
    assert_eq!(openai_body["required"], json!(["model", "messages"]));
    assert_eq!(
        openai_body["properties"]["messages"]["items"]["required"],
        json!(["role", "content"])
    );
    assert_eq!(
        openai_body["properties"]["messages"]["items"]["properties"]["role"]["enum"],
        json!(["system", "user", "assistant", "tool"])
    );
    assert!(openai_body["properties"].get("tools").is_some());
    assert!(openai_body["properties"].get("tool_choice").is_some());
    assert!(openai_body["properties"].get("function_call").is_some());
    assert!(
        openai_body["properties"]["messages"]["items"]["properties"]
            .get("tool_calls")
            .is_some()
    );
    assert!(
        openai_body["properties"]["messages"]["items"]["properties"]
            .get("tool_call_id")
            .is_some()
    );

    let anthropic_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationAnthropicCreateMessage/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(anthropic_spec.status(), StatusCode::OK);
    let anthropic_payload = response_json(anthropic_spec).await;
    let anthropic_body = &anthropic_payload["paths"]["/v1/messages"]["post"]["requestBody"]["content"]
        ["application/json"]["schema"];
    assert_eq!(anthropic_body["required"], json!(["model", "messages"]));
    assert_eq!(
        anthropic_body["properties"]["max_tokens"]["type"],
        json!("integer")
    );
    assert!(anthropic_body["properties"].get("tools").is_some());
    assert!(anthropic_body["properties"].get("tool_choice").is_some());
    assert_eq!(
        anthropic_body["properties"]["messages"]["items"]["properties"]["content"]["oneOf"][1]["items"]
            ["properties"]["type"]["enum"],
        json!(["text", "tool_use", "tool_result"])
    );
    assert_eq!(
        anthropic_body["properties"]["metadata"]["properties"]["expand_id"]["type"],
        json!("string")
    );
    assert!(
        anthropic_body["properties"]["metadata"]["properties"]
            .get("user_id")
            .is_none()
    );

    let get_run_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationNativeGetRun/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(get_run_spec.status(), StatusCode::OK);
    let get_run_payload = response_json(get_run_spec).await;
    let get_run_operation = &get_run_payload["paths"]["/api/agent/v1/runs/{run_id}"]["get"];
    assert_eq!(get_run_operation["requestBody"], Value::Null);
    assert_eq!(get_run_operation["parameters"][0]["name"], json!("run_id"));
    assert_eq!(get_run_operation["parameters"][0]["in"], json!("path"));
    assert_eq!(
        get_run_operation["parameters"][0]["schema"]["format"],
        json!("uuid")
    );

    let upload_file_spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationNativeUploadFile/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(upload_file_spec.status(), StatusCode::OK);
    let upload_file_payload = response_json(upload_file_spec).await;
    let upload_body = &upload_file_payload["paths"]["/api/agent/v1/files"]["post"]["requestBody"]["content"]
        ["multipart/form-data"]["schema"];
    assert_eq!(upload_body["required"], json!(["file_table_id", "file"]));
    assert_eq!(
        upload_body["properties"]["file_table_id"]["format"],
        json!("uuid")
    );
    assert_eq!(upload_body["properties"]["file"]["format"], json!("binary"));

    let create_run_post = &create_run_payload["paths"]["/api/agent/v1/runs"]["post"];
    assert_eq!(
        create_run_post["responses"]["201"]["content"]["application/json"]["schema"]["properties"]
            ["data"]["properties"]["status"]["type"],
        json!("string")
    );
    assert!(
        create_run_post["responses"]["201"]["content"]["application/json"]["schema"]["properties"]
            ["data"]["properties"]
            .get("tool_calls")
            .is_some()
    );
    assert_eq!(
        create_run_post["responses"]["400"]["content"]["application/json"]["schema"]["properties"]
            ["code"]["type"],
        json!("string")
    );

    let openai_post = &openai_payload["paths"]["/v1/chat/completions"]["post"];
    assert_eq!(
        openai_post["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["choices"]
            ["items"]["properties"]["message"]["properties"]["content"]["oneOf"][1]["type"],
        json!("null")
    );
    assert_eq!(
        openai_post["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["choices"]
            ["items"]["properties"]["finish_reason"]["enum"],
        json!(["stop", "tool_calls"])
    );
    assert!(
        openai_post["responses"]["200"]["content"]["application/json"]["schema"]["properties"]
            ["choices"]["items"]["properties"]["message"]["properties"]
            .get("tool_calls")
            .is_some()
    );
    assert_eq!(
        openai_post["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["choices"]
            ["items"]["properties"]["message"]["properties"]["tool_calls"]["items"]["properties"]["type"]
            ["enum"],
        json!(["function"])
    );
    assert_eq!(
        openai_post["responses"]["400"]["content"]["application/json"]["schema"]["properties"]["error"]
            ["properties"]["message"]["type"],
        json!("string")
    );

    let anthropic_post = &anthropic_payload["paths"]["/v1/messages"]["post"];
    assert_eq!(
        anthropic_post["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["content"]
            ["items"]["properties"]["type"]["enum"],
        json!(["text", "tool_use"])
    );
    assert_eq!(
        anthropic_post["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["stop_reason"]
            ["enum"],
        json!(["end_turn", "tool_use"])
    );
    assert_eq!(
        anthropic_post["responses"]["200"]["content"]["application/json"]["schema"]["properties"]["content"]
            ["items"]["properties"]["input"]["type"],
        json!("object")
    );
    assert_eq!(
        anthropic_post["responses"]["400"]["content"]["application/json"]["schema"]["properties"]["error"]
            ["properties"]["message"]["type"],
        json!("string")
    );
}

#[tokio::test]
async fn application_api_docs_specs_follow_requested_locale() {
    let app = test_app().await;
    let (cookie, application_id) = setup_published_app(&app).await;

    let spec = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/operations/applicationNativeCreateRun/openapi.json"
                ))
                .header("cookie", &cookie)
                .header("x-1flowbase-locale", "zh_Hans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(spec.status(), StatusCode::OK);
    let spec_payload = response_json(spec).await;

    assert_eq!(
        spec_payload["info"]["title"],
        json!("Application API Docs App 公开 API")
    );
    assert_eq!(
        spec_payload["info"]["description"],
        json!(
            "Application API Docs App 的应用级公开 API 文档。当前启用的是发布版本 v1。公开路径由应用 API 密钥选择，不通过 application_id 选择。"
        )
    );
    assert_eq!(
        spec_payload["paths"]["/api/agent/v1/runs"]["post"]["summary"],
        json!("创建原生公开运行")
    );
    assert_eq!(
        spec_payload["paths"]["/api/agent/v1/runs"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"]["properties"]["query"]["description"],
        json!("用户输入，会映射到当前应用发布配置中的 query target。")
    );
    assert_eq!(
        spec_payload["paths"]["/api/agent/v1/runs"]["post"]["requestBody"]["content"]["application/json"]
            ["schema"]["properties"]["title"]["description"],
        json!("运行标题。未传时默认使用用户输入，并截断到 255 个字符。")
    );
    assert_eq!(
        spec_payload["paths"]["/api/agent/v1/runs"]["post"]["responses"]["201"]["description"],
        json!("原生运行已创建")
    );
    assert_eq!(
        spec_payload["components"]["securitySchemes"]["applicationApiKey"]["description"],
        json!("使用在当前应用 API 页签中创建的应用 API 密钥。")
    );
}

#[tokio::test]
async fn application_api_docs_routes_require_session_access() {
    let app = test_app().await;
    let (_, application_id) = setup_published_app(&app).await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/api-docs/catalog"
                ))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
}
