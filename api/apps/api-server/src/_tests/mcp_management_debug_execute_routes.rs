use crate::_tests::support::{
    create_member, create_role, login_and_capture_cookie, replace_member_roles,
    replace_role_permissions, test_app,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

async fn create_exposed_published_model(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    code: &str,
) -> String {
    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": code,
                        "title": code,
                        "status": "published"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let create_payload = response_json(create_response).await;
    let model_id = create_payload["data"]["id"].as_str().unwrap().to_string();

    let field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/fields"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "order_title",
                        "title": "order_title",
                        "field_kind": "string",
                        "is_required": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(field_response.status(), StatusCode::CREATED);

    let grant_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/scope-grants"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "system",
                        "scope_id": domain::SYSTEM_SCOPE_ID,
                        "enabled": true,
                        "permission_profile": "scope_all"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(grant_response.status(), StatusCode::CREATED);

    let expose_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!("/api/console/models/{model_id}"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "status": "published" }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(expose_response.status(), StatusCode::OK);

    model_id
}

async fn create_bindable_create_interface(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    code: &str,
) -> String {
    create_exposed_published_model(app, cookie, csrf, code).await;
    bindable_interface_id_for_path(
        app,
        cookie,
        "POST",
        &format!("/api/runtime/models/{code}/create"),
    )
    .await
}

async fn bindable_interface_id_for_path(
    app: &axum::Router,
    cookie: &str,
    method: &str,
    path: &str,
) -> String {
    let interface_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/interface-capabilities?bindable_only=true")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(interface_response.status(), StatusCode::OK);
    let interface_payload = response_json(interface_response).await;
    interface_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["method"] == json!(method) && entry["path"] == json!(path))
        .unwrap_or_else(|| {
            panic!("MCP debug test should expose bindable interface {method} {path}")
        })["interface_id"]
        .as_str()
        .unwrap()
        .to_string()
}

#[tokio::test]
async fn mcp_debug_execute_returns_tool_result_by_default() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create_interface_id =
        create_bindable_create_interface(&app, &root_cookie, &root_csrf, "mcp_debug_orders").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": create_interface_id,
                        "mcp_arguments": {
                            "title": "Debug order"
                        },
                        "input_mapping": {
                            "interface_parameters": [
                                {
                                    "name": "order_title",
                                    "field_type": "string",
                                    "parameter_type": "json_body",
                                    "description": "Order title",
                                    "required": true
                                }
                            ],
                            "mappings": [
                                {
                                    "interface_param": "order_title",
                                    "mcp_param": "title",
                                    "description": "Order title",
                                    "required": true
                                }
                            ]
                        },
                        "output_mapping": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "order_title": { "type": "string" }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["data"]["order_title"], json!("Debug order"));
    assert!(payload["data"]["id"].is_string());
    assert!(payload["data"]["mcp_arguments"].is_null());
    assert!(payload["data"]["interface_arguments"].is_null());
    assert!(payload["data"]["interface_response"].is_null());
    assert!(payload["data"]["tool_result"].is_null());
}

#[tokio::test]
async fn mcp_debug_execute_returns_debug_details_when_requested() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create_interface_id = create_bindable_create_interface(
        &app,
        &root_cookie,
        &root_csrf,
        "mcp_debug_details_orders",
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": create_interface_id,
                        "debug_response_mode": "debug_details",
                        "mcp_arguments": {
                            "title": "Debug order"
                        },
                        "input_mapping": {
                            "mappings": [
                                {
                                    "interface_param": "order_title",
                                    "mcp_param": "title",
                                    "required": true
                                }
                            ]
                        },
                        "output_mapping": {
                            "type": "object",
                            "properties": {
                                "id": { "type": "string" },
                                "order_title": { "type": "string" }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(
        payload["data"]["mcp_arguments"],
        json!({
            "title": "Debug order"
        })
    );
    assert_eq!(
        payload["data"]["interface_arguments"]["body"],
        json!({
            "order_title": "Debug order"
        })
    );
    assert_eq!(
        payload["data"]["interface_response"]["data"]["order_title"],
        json!("Debug order")
    );
    assert!(payload["data"]["interface_response"]["data"]["id"].is_string());
    assert_eq!(
        payload["data"]["tool_result"]["order_title"],
        json!("Debug order")
    );
    assert_eq!(
        payload["data"]["tool_result"]["id"],
        payload["data"]["interface_response"]["data"]["id"]
    );
}

#[tokio::test]
async fn mcp_debug_execute_filters_array_item_fields_from_output_mapping() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let list_users_interface_id =
        bindable_interface_id_for_path(&app, &root_cookie, "GET", "/api/runtime/models/users/list")
            .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": list_users_interface_id,
                        "mcp_arguments": {
                            "page": 1,
                            "page_size": 1
                        },
                        "input_mapping": {
                            "mappings": [
                                {
                                    "interface_param": "page",
                                    "mcp_param": "page",
                                    "required": true
                                },
                                {
                                    "interface_param": "page_size",
                                    "mcp_param": "page_size",
                                    "required": true
                                }
                            ]
                        },
                        "output_mapping": {
                            "type": "object",
                            "properties": {
                                "items": {
                                    "type": "array",
                                    "items": {
                                        "type": "object",
                                        "properties": {
                                            "id": { "type": "string" },
                                            "email_login_enabled": { "type": "boolean" }
                                        }
                                    }
                                },
                                "total": { "type": "integer" }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["data"]["total"], json!(1));
    assert!(payload["data"]["items"][0]["id"].is_string());
    assert_eq!(
        payload["data"]["items"][0]["email_login_enabled"],
        json!(true)
    );
    assert!(payload["data"]["items"][0]["account"].is_null());
    assert!(payload["data"]["items"][0]["meta"].is_null());
}

#[tokio::test]
async fn mcp_debug_execute_requires_csrf() {
    let app = test_app().await;
    let (root_cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &root_cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": "missing_interface",
                        "mcp_arguments": {},
                        "input_mapping": { "mappings": [] },
                        "output_mapping": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::UNAUTHORIZED);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("not_authenticated"));
}

#[tokio::test]
async fn mcp_debug_execute_requires_mcp_manage_permission() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create_interface_id = create_bindable_create_interface(
        &app,
        &root_cookie,
        &root_csrf,
        "mcp_debug_permission_orders",
    )
    .await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "mcp-debug-no-manage",
        "temp-pass",
    )
    .await;
    create_role(&app, &root_cookie, &root_csrf, "mcp_debug_no_manage").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "mcp_debug_no_manage",
        &["mcp_management.view.all"],
    )
    .await;
    replace_member_roles(
        &app,
        &root_cookie,
        &root_csrf,
        &member_id,
        &["mcp_debug_no_manage"],
    )
    .await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "mcp-debug-no-manage", "temp-pass").await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &member_cookie)
                .header("x-csrf-token", &member_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": create_interface_id,
                        "mcp_arguments": {
                            "title": "Debug order"
                        },
                        "input_mapping": {
                            "mappings": [
                                {
                                    "interface_param": "order_title",
                                    "mcp_param": "title",
                                    "required": true
                                }
                            ]
                        },
                        "output_mapping": {
                            "type": "object",
                            "properties": {
                                "order_title": { "type": "string" }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::FORBIDDEN);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("permission_denied"));
}

#[tokio::test]
async fn mcp_debug_execute_returns_full_payload_when_output_mapping_matches_nothing() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create_interface_id = create_bindable_create_interface(
        &app,
        &root_cookie,
        &root_csrf,
        "mcp_debug_mapping_orders",
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": create_interface_id,
                        "mcp_arguments": {
                            "title": "Debug order"
                        },
                        "input_mapping": {
                            "mappings": [
                                {
                                    "interface_param": "order_title",
                                    "mcp_param": "title",
                                    "required": true
                                }
                            ]
                        },
                        "output_mapping": {
                            "type": "object",
                            "properties": {
                                "missing_field": { "type": "string" }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    let status = response.status();
    let payload = response_json(response).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    assert_eq!(payload["data"]["order_title"], json!("Debug order"));
    assert!(payload["data"]["id"].is_string());
    assert!(payload["data"]["missing_field"].is_null());
    assert!(payload["data"]["tool_result"].is_null());
}

#[tokio::test]
async fn mcp_debug_execute_reports_missing_required_mcp_argument() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create_interface_id = create_bindable_create_interface(
        &app,
        &root_cookie,
        &root_csrf,
        "mcp_debug_missing_args_orders",
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": create_interface_id,
                        "mcp_arguments": {},
                        "input_mapping": {
                            "mappings": [
                                {
                                    "interface_param": "order_title",
                                    "mcp_param": "title",
                                    "required": true
                                }
                            ]
                        },
                        "output_mapping": {
                            "type": "object",
                            "properties": {
                                "order_title": { "type": "string" }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::BAD_REQUEST);
    let payload = response_json(response).await;
    assert_eq!(payload["code"], json!("mcp_arguments"));
}

#[tokio::test]
async fn mcp_debug_execute_forwards_target_interface_failure() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_exposed_published_model(
        &app,
        &root_cookie,
        &root_csrf,
        "mcp_debug_target_failure_orders",
    )
    .await;
    let get_interface_id = bindable_interface_id_for_path(
        &app,
        &root_cookie,
        "GET",
        "/api/runtime/models/mcp_debug_target_failure_orders/get/{id}",
    )
    .await;

    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/debug/execute")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "interface_id": get_interface_id,
                        "mcp_arguments": {
                            "record_id": "00000000-0000-0000-0000-000000000000"
                        },
                        "input_mapping": {
                            "mappings": [
                                {
                                    "interface_param": "id",
                                    "mcp_param": "record_id",
                                    "required": true
                                }
                            ]
                        },
                        "output_mapping": {
                            "type": "object",
                            "properties": {
                                "order_title": { "type": "string" }
                            }
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
    let payload = response_json(response).await;
    assert!(payload["code"].is_string(), "{payload}");
    assert_ne!(payload["code"], json!("output_mapping"));
}

#[tokio::test]
async fn mcp_debug_execute_is_not_a_bindable_interface_capability() {
    let app = test_app().await;
    let (root_cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let full_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/interface-capabilities")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(full_response.status(), StatusCode::OK);
    let full_payload = response_json(full_response).await;
    let debug_entry = full_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["path"] == json!("/api/console/mcp/debug/execute"))
        .expect("debug execute should be documented as a console capability");
    assert_eq!(debug_entry["bindable"], json!(false));
    assert_eq!(
        debug_entry["disabled_reason"],
        json!("unsupported_mcp_interface_scope")
    );

    let bindable_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/interface-capabilities?bindable_only=true")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bindable_response.status(), StatusCode::OK);
    let bindable_payload = response_json(bindable_response).await;
    assert!(!bindable_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .any(|entry| entry["path"] == json!("/api/console/mcp/debug/execute")));
}
