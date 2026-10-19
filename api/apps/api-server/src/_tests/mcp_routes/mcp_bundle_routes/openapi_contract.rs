use crate::_tests::support::{login_and_capture_cookie, test_app};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

fn assert_closed_schema(openapi: &Value, schema: &Value) {
    assert_ne!(schema, &json!({}), "schema must not be unconstrained");
    assert_ne!(
        schema,
        &json!(true),
        "schema must not accept arbitrary JSON"
    );
    if let Some(reference) = schema.get("$ref").and_then(Value::as_str) {
        let pointer = reference
            .strip_prefix('#')
            .expect("fixture only accepts document-local schema references");
        let resolved = openapi
            .pointer(pointer)
            .unwrap_or_else(|| panic!("unresolved OpenAPI schema reference {reference}"));
        assert_ne!(resolved, &json!({}), "resolved schema must be constrained");
    }
}

#[tokio::test]
async fn ac_001_ac_011_bundle_library_openapi_and_interface_catalog_are_truthful() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let openapi_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(openapi_response.status(), StatusCode::OK);
    let openapi = response_json(openapi_response).await;

    let contracts = [
        (
            "get",
            "/api/console/mcp/bundles/library",
            "list_mcp_bundle_library",
            None,
            "McpBundleLibraryCatalog",
        ),
        (
            "post",
            "/api/console/mcp/bundles/library/{organization}/{bundle_id}/sync",
            "sync_mcp_bundle_library_release",
            Some("McpBundleLibraryVersionBody"),
            "LocalMcpBundleReceipt",
        ),
        (
            "post",
            "/api/console/mcp/bundles/library/{organization}/{bundle_id}/preview",
            "preview_mcp_bundle_library_release",
            Some("McpBundleLibraryVersionBody"),
            "McpBundlePreview",
        ),
        (
            "post",
            "/api/console/mcp/bundles/library/{organization}/{bundle_id}/import",
            "import_mcp_bundle_library_release",
            Some("McpBundleLibraryVersionBody"),
            "McpBundleImportReport",
        ),
    ];

    for (method, path, operation_id, request_schema, response_schema) in contracts {
        let operation = &openapi["paths"][path][method];
        assert_eq!(operation["operationId"], json!(operation_id));
        assert!(operation["summary"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert!(operation["description"]
            .as_str()
            .is_some_and(|value| !value.is_empty()));
        assert_eq!(
            operation["responses"]["200"]["content"]["application/json"]["schema"]["$ref"],
            json!(format!("#/components/schemas/{response_schema}"))
        );
        assert_closed_schema(
            &openapi,
            &operation["responses"]["200"]["content"]["application/json"]["schema"],
        );
        match request_schema {
            Some(request_schema) => {
                let schema = &operation["requestBody"]["content"]["application/json"]["schema"];
                assert_eq!(
                    schema["$ref"],
                    json!(format!("#/components/schemas/{request_schema}"))
                );
                assert_closed_schema(&openapi, schema);
                for parameter_name in ["organization", "bundle_id"] {
                    assert!(operation["parameters"]
                        .as_array()
                        .unwrap()
                        .iter()
                        .any(|parameter| parameter["name"] == parameter_name
                            && parameter["in"] == "path"
                            && parameter["required"] == true));
                }
            }
            None => {
                assert!(operation.get("requestBody").is_none());
                assert!(operation["parameters"].as_array().unwrap().iter().any(
                    |parameter| parameter["name"] == "refresh_remote"
                        && parameter["in"] == "query"
                        && parameter["required"] == false
                        && parameter["schema"]["type"] == "boolean"
                ));
            }
        }
    }

    let interface_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/interface-capabilities?bindable_only=true")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(interface_response.status(), StatusCode::OK);
    let interface_payload = response_json(interface_response).await;
    let interfaces = interface_payload["data"].as_array().unwrap();
    for (method, path, operation_id, _, _) in contracts {
        let interface = interfaces
            .iter()
            .find(|entry| entry["interface_id"] == operation_id)
            .unwrap_or_else(|| panic!("missing bundle interface {operation_id}"));
        assert_eq!(interface["method"], json!(method.to_ascii_uppercase()));
        assert_eq!(interface["path"], json!(path));
        assert_eq!(interface["bindable"], json!(true));
        assert_eq!(
            interface["permission_code"],
            json!(access_control::SYSTEM_MCP_MANAGEMENT_SETTINGS_FEATURE_PERMISSION)
        );
        assert_ne!(interface["parameter_schema"], json!({}));
        assert_ne!(interface["result_schema"], json!({}));
        let expected_security = if method == "get" {
            json!([
                {"sessionCookie": []},
                {"patBearer": []}
            ])
        } else {
            json!([
                {"sessionCookie": [], "csrfHeader": []},
                {"patBearer": []}
            ])
        };
        assert_eq!(interface["security"], expected_security);
    }
}
