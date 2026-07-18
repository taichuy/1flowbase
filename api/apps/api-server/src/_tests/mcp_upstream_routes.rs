use crate::_tests::support::{
    login_and_capture_cookie, test_app, test_app_with_database_url,
    test_app_with_runtime_profile_error,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use time::{format_description::well_known::Rfc3339, OffsetDateTime};
use tower::ServiceExt;
use uuid::Uuid;

async fn response_json(response: axum::response::Response) -> Value {
    serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap()
}

#[tokio::test]
async fn issue_1246_ac_016_openapi_exposes_typed_tool_availability_enum() {
    let response = crate::app()
        .oneshot(
            Request::builder()
                .uri("/openapi.json")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let openapi = response_json(response).await;
    assert_eq!(
        openapi["components"]["schemas"]["McpToolAvailabilityStatusDto"]["enum"],
        json!([
            "available",
            "interface_missing",
            "upstream_disabled",
            "credentials_missing",
            "upstream_tool_missing",
            "mapping_invalid"
        ])
    );
    assert_eq!(
        openapi["components"]["schemas"]["McpToolResponse"]["properties"]["availability_status"]
            ["$ref"],
        json!("#/components/schemas/McpToolAvailabilityStatusDto")
    );
}

fn connection_body() -> Value {
    json!({
        "name":"Weather MCP",
        "endpoint":"https://mcp.example.com/rpc",
        "transport":"streamable_http",
        "auth_type":"bearer",
        "custom_header_name":null,
        "status":"enabled"
    })
}

async fn create_connection(app: &axum::Router, cookie: &str, csrf: &str) -> Value {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/upstream-connections")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(connection_body().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::CREATED);
    response_json(response).await["data"].clone()
}

#[tokio::test]
async fn issue_1246_upstream_connection_api_timestamps_are_rfc3339() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let connection = create_connection(&app, &cookie, &csrf).await;

    for field in ["created_at", "updated_at"] {
        let timestamp = connection[field]
            .as_str()
            .unwrap_or_else(|| panic!("{field} must be a string"));
        OffsetDateTime::parse(timestamp, &Rfc3339)
            .unwrap_or_else(|error| panic!("{field} must be RFC 3339: {error}"));
    }
}

#[tokio::test]
async fn form_connection_test_uses_request_config_without_persisting_connection() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let tested = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/upstream-connections/test")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "connection_id":null,
                        "endpoint":"http://127.0.0.1/mcp",
                        "transport":"streamable_http",
                        "auth_type":"none",
                        "custom_header_name":null,
                        "credential":null
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(tested.status(), StatusCode::OK);
    let tested = response_json(tested).await;
    assert_eq!(tested["data"]["ok"], json!(false));
    assert_eq!(tested["data"]["error"], json!("invalid upstream endpoint"));
    OffsetDateTime::parse(tested["data"]["tested_at"].as_str().unwrap(), &Rfc3339).unwrap();

    let listed = app
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/upstream-connections")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(listed.status(), StatusCode::OK);
    assert_eq!(response_json(listed).await["data"], json!([]));
}

#[tokio::test]
async fn issue_1246_ac_004_ac_018_upstream_connection_writes_require_session_and_csrf() {
    let app = test_app().await;
    let unauthenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/upstream-connections")
                .header("content-type", "application/json")
                .body(Body::from(connection_body().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);

    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let missing_csrf = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/upstream-connections")
                .header("cookie", cookie)
                .header("content-type", "application/json")
                .body(Body::from(connection_body().to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(missing_csrf.status(), StatusCode::UNAUTHORIZED);

    let (restricted_app, restricted_cookie) =
        test_app_with_runtime_profile_error(&["system_runtime.view.all"]).await;
    let forbidden = restricted_app
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/upstream-connections")
                .header("cookie", restricted_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(forbidden.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn issue_1246_ac_005_connection_read_returns_status_but_never_secret() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let connection = create_connection(&app, &cookie, &csrf).await;
    let connection_id = connection["connection_id"].as_str().unwrap();
    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/mcp/upstream-connections/{connection_id}/credentials"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"kind":"bearer","token":"route-secret-token"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::NO_CONTENT);

    let list = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/mcp/upstream-connections")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list.status(), StatusCode::OK);
    let payload = response_json(list).await;
    assert_eq!(
        payload["data"][0]["credentials_status"],
        json!("configured")
    );
    let serialized = payload.to_string();
    assert!(!serialized.contains("route-secret-token"));
    assert!(!serialized.contains("token\""));
    assert!(payload["data"][0].get("custom_header_name").is_some());
}

#[tokio::test]
async fn issue_1246_ac_016_discovered_but_unimported_source_does_not_block_connection_delete() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let connection = create_connection(&app, &cookie, &csrf).await;
    let connection_id = Uuid::parse_str(connection["connection_id"].as_str().unwrap()).unwrap();
    let workspace_id = Uuid::parse_str(connection["workspace_id"].as_str().unwrap()).unwrap();
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        r#"insert into mcp_upstream_tool_sources (
            id,workspace_id,upstream_connection_id,remote_tool_name,input_schema,
            output_schema,schema_hash,source_status,discovered_at
        ) values ($1,$2,$3,'unimported.tool','{}','{}','v1','not_imported',now())"#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(connection_id)
    .execute(&pool)
    .await
    .unwrap();
    let deleted = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/mcp/upstream-connections/{connection_id}"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(deleted.status(), StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn issue_1246_ac_008_ac_009_import_route_is_idempotent_and_returns_tagged_tool() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let connection = create_connection(&app, &cookie, &csrf).await;
    let connection_id = Uuid::parse_str(connection["connection_id"].as_str().unwrap()).unwrap();
    let workspace_id = Uuid::parse_str(connection["workspace_id"].as_str().unwrap()).unwrap();
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    sqlx::query(
        r#"insert into mcp_upstream_tool_sources (
            id,workspace_id,upstream_connection_id,remote_tool_name,description,input_schema,
            output_schema,schema_hash,source_status,discovered_at
        ) values ($1,$2,$3,'weather.lookup','Weather',$4,$5,'schema-v1','not_imported',now())"#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(connection_id)
    .bind(json!({"type":"object","properties":{"city":{"type":"string"}},"required":["city"]}))
    .bind(json!({"type":"object","properties":{"temperature":{"type":"number"}}}))
    .execute(&pool)
    .await
    .unwrap();

    let mut tool_ids = Vec::new();
    let mut imported_tool = None;
    for _ in 0..2 {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri(format!(
                        "/api/console/mcp/upstream-connections/{connection_id}/imports"
                    ))
                    .header("cookie", &cookie)
                    .header("x-csrf-token", &csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({"remote_tool_names":["weather.lookup"]}).to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(response.status(), StatusCode::OK);
        let payload = response_json(response).await;
        let tool = &payload["data"][0];
        assert_eq!(tool["execution_target"]["kind"], json!("mcp_proxy"));
        assert_eq!(
            tool["execution_target"]["upstream_connection_id"],
            json!(connection_id)
        );
        assert_eq!(tool["status"], json!("draft"));
        assert_eq!(tool["risk_level"], json!("high"));
        assert_eq!(tool["availability_status"], json!("credentials_missing"));
        assert_eq!(
            tool["input_mapping"]["mappings"][0],
            json!({
                "local_path":"city","remote_path":"city","required":true
            })
        );
        tool_ids.push(tool["id"].as_str().unwrap().to_string());
        imported_tool.get_or_insert_with(|| tool.clone());
    }
    assert_eq!(tool_ids[0], tool_ids[1]);

    let tool = imported_tool.unwrap();
    let update = app.clone().oneshot(
        Request::builder().method("PUT")
            .uri(format!("/api/console/mcp/tools/{}", tool["tool_id"].as_str().unwrap()))
            .header("cookie", &cookie).header("x-csrf-token", &csrf)
            .header("content-type", "application/json")
            .body(Body::from(json!({
                "name":"Local weather","des_id":tool["des_id"],
                "short_description":"Local weather","full_description":"Local contract",
                "execution_target":tool["execution_target"],
                "parameter_schema":{"type":"object","properties":{"city_name":{"type":"string"}}},
                "result_schema":{"type":"object","properties":{"temperature_celsius":{"type":"number"}}},
                "input_mapping":{"mappings":[{"local_path":"city_name","remote_path":"city","required":true}]},
                "output_mapping":{"mappings":[{"remote_path":"temperature","local_path":"temperature_celsius","required":true}]},
                "permission_code":null,"risk_level":"medium","status":"enabled"
            }).to_string())).unwrap()
    ).await.unwrap();
    assert_eq!(update.status(), StatusCode::OK);
    let updated = response_json(update).await;
    assert_eq!(
        updated["data"]["execution_target"],
        tool["execution_target"]
    );
    assert_eq!(
        updated["data"]["input_mapping"]["mappings"][0]["local_path"],
        json!("city_name")
    );
    assert_eq!(
        updated["data"]["availability_status"],
        json!("credentials_missing")
    );
    let blocked_delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/mcp/upstream-connections/{connection_id}"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(blocked_delete.status(), StatusCode::CONFLICT);
}
