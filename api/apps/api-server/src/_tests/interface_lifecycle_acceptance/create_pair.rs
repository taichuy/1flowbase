//! AC-005. Baseline 6824b2c1701eacc5cd06f155b5536b6442b54220:
//! route_docs/docs_routes.rs::create_model (POST/input/201),
//! mcp_protocol_routes.rs::ac_001/ac_002_runtime_mcp_write_* (DTO/403/code),
//! model_definitions.rs::{CreateModelDefinitionBody,ModelDefinitionResponse} (required fields/keys),
//! response.rs::ApiSuccess and error_response.rs::ErrorBody (protocol shape).
//! Real Router + tools/call; no typed handler or test hook injection.
use crate::_tests::{
    mcp_protocol_routes::{
        call_mcp, create_api_key, create_mcp_instance, create_model_probe_tool, response_json,
    },
    support::{create_member, login_and_capture_cookie, test_api_state_with_database_url},
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use std::sync::Arc;
use tower::ServiceExt;

const PATH: &str = "/api/console/settings/data-models/model-definitions";
const CODE: &str = "root_1998_create_pair";
fn input() -> Value {
    json!({"scope_kind":"workspace", "code":CODE, "title":"Delegated model",
        "template_provider":"core", "template_code":"general", "template_version":"v1"})
}
fn mcp_request(body: Value) -> Value {
    json!({"jsonrpc":"2.0", "id":1998, "method":"tools/call", "params":{
        "name":"mcp_call", "arguments":{"tool_id":"create_model_probe", "max_inline_chars":12000,
        "arguments":{"body":body}}}})
}
async fn http(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    body: Value,
) -> axum::response::Response {
    app.clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(PATH)
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(body.to_string()))
                .unwrap(),
        )
        .await
        .unwrap()
}
fn normalized_model(mut model: Value, workspace: uuid::Uuid) -> Value {
    // Only generated model/field identities and the equivalent isolated workspace differ.
    // Preserve every business field, including physical names, namespaces and capabilities.
    let mut keys = model
        .as_object()
        .unwrap()
        .keys()
        .map(String::as_str)
        .collect::<Vec<_>>();
    keys.sort_unstable();
    let mut expected = vec![
        "id",
        "scope_kind",
        "scope_id",
        "code",
        "title",
        "description",
        "status",
        "runtime_availability",
        "data_source_id",
        "source_kind",
        "external_resource_key",
        "external_table_id",
        "template_provider",
        "template_code",
        "template_version",
        "template_summary",
        "physical_table_name",
        "acl_namespace",
        "audit_namespace",
        "builtin_kind",
        "capabilities",
        "fields",
    ];
    expected.sort_unstable();
    assert_eq!(keys, expected);
    uuid::Uuid::parse_str(model["id"].as_str().unwrap()).unwrap();
    assert_eq!(model["scope_id"], workspace.to_string());
    for (key, value) in [
        ("scope_kind", "workspace"),
        ("code", CODE),
        ("title", "Delegated model"),
        ("template_provider", "core"),
        ("template_code", "general"),
        ("template_version", "v1"),
        ("source_kind", "main_source"),
        ("data_source_id", "main"),
        ("physical_table_name", CODE),
        ("status", "published"),
        ("runtime_availability", "available"),
    ] {
        assert_eq!(model[key], value, "baseline field {key}");
    }
    assert_eq!(model["description"], Value::Null);
    assert_eq!(model["external_resource_key"], Value::Null);
    assert_eq!(model["external_table_id"], Value::Null);
    model["id"] = json!("<model-id>");
    model["scope_id"] = json!("<workspace-id>");
    let fields = model["fields"].as_array_mut().unwrap();
    fields.sort_by_key(|field| field["code"].as_str().unwrap().to_string());
    for field in fields {
        uuid::Uuid::parse_str(field["id"].as_str().unwrap()).unwrap();
        field["id"] = json!(format!("<field:{}>", field["code"].as_str().unwrap()));
    }
    model
}
async fn persisted(pool: &sqlx::PgPool) -> Value {
    // Counts detect denial writes. Full target rows retain business columns; only generated
    // IDs, schema-local scope/actor IDs and audit timestamps are normalized away.
    sqlx::query_scalar::<_, Value>("select jsonb_build_object('models', (select count(*) from model_definitions), 'fields', (select count(*) from model_fields), 'grants', (select count(*) from scope_data_model_grants), 'unchanged_rows', (select md5(jsonb_build_array((select jsonb_agg(to_jsonb(m) order by m.id) from model_definitions m), (select jsonb_agg(to_jsonb(f) order by f.id) from model_fields f), (select jsonb_agg(to_jsonb(g) order by g.id) from scope_data_model_grants g))::text)), 'target', (select to_jsonb(m) - array['id','scope_id','created_by','updated_by','created_at','updated_at'] from model_definitions m where code=$1), 'target_fields', (select coalesce(jsonb_agg(to_jsonb(f) - array['id','data_model_id','created_by','updated_by','created_at','updated_at'] order by f.code), '[]'::jsonb) from model_fields f join model_definitions m on m.id=f.data_model_id where m.code=$1))")
        .bind(CODE)
        .fetch_one(pool).await.unwrap()
}

#[tokio::test]
async fn root_1998_ac_005_http_mcp_create_success_and_core_deny_preserve_baseline() {
    for denied in [false, true] {
        let mut results = Vec::new();
        let mut pins = Vec::new();
        let mut deltas = Vec::new();
        for use_mcp in [false, true] {
            // Separate schemas use the same seed/configuration and intent. Root/member accounts
            // represent the same actor/role; their generated database IDs are schema-local.
            let (state, database_url) = test_api_state_with_database_url().await;
            let app = crate::app_with_state(state.clone());
            let registry = state
                .extension_boot_snapshot
                .as_ref()
                .unwrap()
                .interface_registry()
                .unwrap()
                .clone();
            let snapshot = registry.snapshot();
            let plan = snapshot
                .plan_for_interface(
                    &interface_runtime::InterfaceId::new("model_definitions.create").unwrap(),
                )
                .unwrap();
            assert_eq!(
                plan.definition().handler_reference().as_str(),
                "model_definitions.create.handler"
            );
            assert_eq!(
                plan.binding().projection().http_route().unwrap().path(),
                PATH
            );
            assert!(!plan.has_executable_extensions());
            pins.push((
                snapshot.graph_fingerprint().clone(),
                snapshot.fingerprint().clone(),
                plan.fingerprint().clone(),
            ));
            let workspace = state.bootstrap_workspace_id;
            let (root_cookie, root_csrf) =
                login_and_capture_cookie(&app, "root", "change-me").await;
            create_mcp_instance(&app, &root_cookie, &root_csrf).await;
            create_model_probe_tool(&app, &root_cookie, &root_csrf).await;
            let (cookie, csrf) = if denied {
                create_member(
                    &app,
                    &root_cookie,
                    &root_csrf,
                    "root-1998-member",
                    "temp-pass",
                )
                .await;
                login_and_capture_cookie(&app, "root-1998-member", "temp-pass").await
            } else {
                (root_cookie, root_csrf)
            };
            let token = create_api_key(&app, &cookie, &csrf).await;
            let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
            let before = persisted(&pool).await;
            let output = if use_mcp {
                let response = call_mcp(&app, &token, mcp_request(input())).await;
                assert_eq!(response["jsonrpc"], "2.0");
                assert_eq!(response["id"], 1998);
                if denied {
                    assert_eq!(response["error"]["code"], -32603, "{response}");
                    let error = &response["error"]["data"];
                    assert_eq!(error["category"], "target_authorization");
                    assert_eq!(error["target_code"], "console_operation_permission_denied");
                    assert_eq!(error["http_status"], 403);
                    assert_eq!(error["outcome"], "failed");
                    assert_eq!(error["retry_original"], false);
                    json!({"status":error["http_status"], "code":error["target_code"]})
                } else {
                    assert!(response.get("error").is_none(), "{response}");
                    assert_ne!(response["result"]["isError"], true);
                    normalized_model(response["result"]["structuredContent"].clone(), workspace)
                }
            } else {
                let response = http(&app, &cookie, &csrf, input()).await;
                assert_eq!(
                    response.status(),
                    if denied {
                        StatusCode::FORBIDDEN
                    } else {
                        StatusCode::CREATED
                    }
                );
                let response = response_json(response).await;
                if denied {
                    assert_eq!(response["status"], 403);
                    assert_eq!(response["code"], "console_operation_permission_denied");
                    assert!(response["message"].is_string());
                    json!({"status":response["status"], "code":response["code"]})
                } else {
                    assert_eq!(
                        response
                            .as_object()
                            .unwrap()
                            .keys()
                            .map(String::as_str)
                            .collect::<Vec<_>>(),
                        ["data", "meta"]
                    );
                    assert_eq!(response["meta"], Value::Null);
                    normalized_model(response["data"].clone(), workspace)
                }
            };
            let after = persisted(&pool).await;
            if denied {
                assert_eq!(
                    before, after,
                    "denied invocation must not write models, fields or grants"
                );
            } else {
                assert_eq!(
                    after["models"].as_i64().unwrap(),
                    before["models"].as_i64().unwrap() + 1
                );
                let row: (uuid::Uuid, String, String, String) = sqlx::query_as("select scope_id, code, title, template_code from model_definitions where code = $1")
                    .bind(CODE).fetch_one(&pool).await.unwrap();
                assert_eq!(
                    row,
                    (
                        workspace,
                        CODE.into(),
                        "Delegated model".into(),
                        "general".into()
                    )
                );
                let actor_matches: bool = sqlx::query_scalar("select m.created_by = u.id and m.updated_by = u.id from model_definitions m cross join users u where m.code=$1 and u.account='root'")
                    .bind(CODE).fetch_one(&pool).await.unwrap();
                assert!(actor_matches);
            }
            deltas.push(json!({"models":after["models"].as_i64().unwrap()-before["models"].as_i64().unwrap(),
                "fields":after["fields"].as_i64().unwrap()-before["fields"].as_i64().unwrap(),
                "grants":after["grants"].as_i64().unwrap()-before["grants"].as_i64().unwrap()}));
            assert!(
                Arc::ptr_eq(&snapshot, &registry.snapshot()),
                "request must not replace frozen registry"
            );
            results.push(
                json!({"response":output, "row":after["target"], "fields":after["target_fields"]}),
            );
            pool.close().await;
        }
        assert_eq!(results[0], results[1]);
        assert_eq!(pins[0], pins[1]);
        assert_eq!(deltas[0], deltas[1]);
    }
}

#[tokio::test]
async fn root_1998_ac_005_required_input_preserves_http_and_mcp_ingress_contract() {
    let (state, database_url) = test_api_state_with_database_url().await;
    let app = crate::app_with_state(state);
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_mcp_instance(&app, &cookie, &csrf).await;
    create_model_probe_tool(&app, &cookie, &csrf).await;
    let token = create_api_key(&app, &cookie, &csrf).await;
    let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
    let before = persisted(&pool).await;
    let mut body = input();
    body.as_object_mut().unwrap().remove("scope_kind");
    let response = http(&app, &cookie, &csrf, body.clone()).await;
    // Baseline required String DTO + Axum Json rejection (not a business error envelope).
    assert_eq!(response.status(), StatusCode::UNPROCESSABLE_ENTITY);
    let text = String::from_utf8(
        to_bytes(response.into_body(), 16_384)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert!(text.contains("missing field `scope_kind`"), "{text}");
    let response = call_mcp(&app, &token, mcp_request(body)).await;
    // Baseline debug_execute::build_interface_arguments rejects missing required mapping
    // with mcp_arguments; virtual_ui::interface_error preserves that field.
    assert_eq!(response["error"]["code"], -32602, "{response}");
    assert_eq!(
        response["error"]["data"],
        json!({"category":"invalid_tool_arguments", "field":"mcp_arguments", "outcome":"not_started", "retry_original":false})
    );
    assert_eq!(before, persisted(&pool).await);
    pool.close().await;
}
