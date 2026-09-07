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
    support::{
        create_member, create_role, login_and_capture_cookie, replace_member_roles,
        replace_role_permissions, test_api_state_with_database_url,
    },
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
    // 6824b2c17: model_definition_repository::platform_runtime_field_records
    // returns these six fields in this order; general adds no ordered-tree fields.
    // The create route maps the returned Vec directly. Never sort the wire array.
    assert_eq!(
        fields
            .iter()
            .map(|field| field["code"].as_str().unwrap())
            .collect::<Vec<_>>(),
        [
            "id",
            "scope_id",
            "created_by",
            "updated_by",
            "created_at",
            "updated_at"
        ]
    );
    for field in fields {
        uuid::Uuid::parse_str(field["id"].as_str().unwrap()).unwrap();
        field["id"] = json!(format!("<field:{}>", field["code"].as_str().unwrap()));
    }
    model
}
async fn persisted(pool: &sqlx::PgPool) -> Value {
    sqlx::query_scalar::<_, Value>(r#"
        select jsonb_build_object(
            'models', (select count(*) from model_definitions),
            'fields', (select count(*) from model_fields),
            'grants', (select count(*) from scope_data_model_grants),
            'unchanged_rows', jsonb_build_array(
                (select jsonb_agg(to_jsonb(m) order by m.id) from model_definitions m where m.code <> $1),
                (select jsonb_agg(to_jsonb(f) order by f.id) from model_fields f where not exists
                    (select 1 from model_definitions m where m.id=f.data_model_id and m.code=$1)),
                (select jsonb_agg(to_jsonb(g) order by g.id) from scope_data_model_grants g where not exists
                    (select 1 from model_definitions m where m.id=g.data_model_id and m.code=$1))),
            'target', (select to_jsonb(m) from model_definitions m where code=$1),
            'target_fields', (select coalesce(jsonb_agg(to_jsonb(f) order by f.sort_order, f.created_at), '[]'::jsonb)
                from model_fields f join model_definitions m on m.id=f.data_model_id where m.code=$1),
            'target_grants', (select coalesce(jsonb_agg(to_jsonb(g) order by g.id), '[]'::jsonb)
                from scope_data_model_grants g join model_definitions m on m.id=g.data_model_id where m.code=$1),
            'target_audits', (select coalesce(jsonb_agg(to_jsonb(a) order by a.event_code), '[]'::jsonb)
                from audit_logs a join model_definitions m on m.id=a.target_id where m.code=$1))
    "#).bind(CODE).fetch_one(pool).await.unwrap()
}

async fn assert_persisted_create(
    pool: &sqlx::PgPool,
    response: &Value,
    persisted: &mut Value,
    workspace: uuid::Uuid,
    started: time::OffsetDateTime,
) {
    let finished: time::OffsetDateTime = sqlx::query_scalar("select clock_timestamp()")
        .fetch_one(pool)
        .await
        .unwrap();
    let actor: uuid::Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(pool)
        .await
        .unwrap();
    let model_id = &response["id"];
    assert_eq!(
        &persisted["target"]["id"], model_id,
        "response model ID must identify the committed row"
    );
    let response_fields = response["fields"].as_array().unwrap();
    let stored_fields = persisted["target_fields"].as_array().unwrap();
    assert_eq!(stored_fields.len(), 6);
    assert_eq!(response_fields.len(), stored_fields.len());
    for (wire, row) in response_fields.iter().zip(stored_fields) {
        assert_eq!(
            wire["id"], row["id"],
            "response field ID must identify its committed row"
        );
        assert_eq!(wire["code"], row["code"]);
        assert_eq!(&row["data_model_id"], model_id);
    }
    let grants = persisted["target_grants"].as_array().unwrap();
    assert_eq!(grants.len(), 1);
    assert_eq!(&grants[0]["data_model_id"], model_id);
    assert_eq!(grants[0]["scope_kind"], "workspace");
    assert_eq!(grants[0]["enabled"], true);
    assert_eq!(grants[0]["permission_profile"], "scope_all");
    let audits = persisted["target_audits"].as_array().unwrap();
    assert_eq!(audits.len(), 2);
    assert_eq!(audits[0]["event_code"], "state_model.created");
    assert_eq!(audits[0]["payload"], json!({"code":CODE}));
    assert_eq!(audits[1]["event_code"], "state_model.scope_grant_created");
    assert_eq!(
        audits[1]["payload"],
        json!({"scope_kind":"workspace", "scope_id":workspace,
        "enabled":true, "permission_profile":"scope_all"})
    );
    for audit in audits {
        assert_eq!(&audit["target_id"], model_id);
        assert_eq!(audit["target_type"], "state_model");
        assert_eq!(audit["workspace_id"], workspace.to_string());
        assert_eq!(audit["actor_user_id"], actor.to_string());
    }
    // Validate every generated identity/actor/time before normalizing isolated-schema values.
    // PostgreSQL parses its own timestamp JSON, avoiding locale/format assumptions in Rust.
    for key in ["target", "target_fields", "target_grants", "target_audits"] {
        let rows: Vec<&mut Value> = if key == "target" {
            vec![&mut persisted[key]]
        } else {
            persisted[key].as_array_mut().unwrap().iter_mut().collect()
        };
        for row in rows {
            uuid::Uuid::parse_str(row["id"].as_str().unwrap()).unwrap();
            assert_eq!(row["scope_id"], workspace.to_string(), "{key} scope");
            assert_eq!(row["created_by"], actor.to_string(), "{key} creator");
            if key == "target_grants" {
                // Baseline grant insertion writes created_by only; updated_by stays null.
                assert_eq!(row["updated_by"], Value::Null);
            } else {
                assert_eq!(row["updated_by"], actor.to_string(), "{key} updater");
            }
            let valid_times: bool = sqlx::query_scalar(
                "select ($1::jsonb->>'created_at')::timestamptz between $2 and $3 and ($1::jsonb->>'updated_at')::timestamptz between ($1::jsonb->>'created_at')::timestamptz and $3")
                .bind(&*row).bind(started).bind(finished).fetch_one(pool).await.unwrap();
            assert!(
                valid_times,
                "{key} audit timestamps must fall within the invocation: {row}"
            );
            let object = row.as_object_mut().unwrap();
            for column in [
                "id",
                "scope_id",
                "created_by",
                "updated_by",
                "created_at",
                "updated_at",
                "data_model_id",
                "workspace_id",
                "actor_user_id",
                "target_id",
            ] {
                if object.get(column).is_some_and(|value| !value.is_null()) {
                    object.insert(column.to_string(), json!(format!("<{column}>")));
                }
            }
            if key == "target_audits" && row["event_code"] == "state_model.scope_grant_created" {
                row["payload"]["scope_id"] = json!("<scope_id>");
            }
        }
    }
    assert_eq!(
        persisted["target_grants"],
        json!([{
            "id":"<id>", "scope_kind":"workspace", "scope_id":"<scope_id>",
            "data_model_id":"<data_model_id>", "enabled":true, "permission_profile":"scope_all",
            "created_by":"<created_by>", "updated_by":null,
            "created_at":"<created_at>", "updated_at":"<updated_at>"
        }])
    );
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
                // Reuse auth_routes' legal member API-key prerequisite. This feature
                // projects only user_api_keys.manage; neither the role nor the token
                // grants model_definitions.create. Both protocol cases get this same role.
                let role = "root_1998_key_only";
                create_role(&app, &root_cookie, &root_csrf, role).await;
                replace_role_permissions(
                    &app,
                    &root_cookie,
                    &root_csrf,
                    role,
                    &["settings_feature.access.system.api-key-authentication"],
                )
                .await;
                let member_id = create_member(
                    &app,
                    &root_cookie,
                    &root_csrf,
                    "root-1998-member",
                    "temp-pass",
                )
                .await;
                replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &[role]).await;
                login_and_capture_cookie(&app, "root-1998-member", "temp-pass").await
            } else {
                (root_cookie, root_csrf)
            };
            let token = create_api_key(&app, &cookie, &csrf).await;
            let pool = sqlx::PgPool::connect(&database_url).await.unwrap();
            let before = persisted(&pool).await;
            let started: time::OffsetDateTime = sqlx::query_scalar("select clock_timestamp()")
                .fetch_one(&pool)
                .await
                .unwrap();
            let output = if use_mcp {
                let response = call_mcp(&app, &token, mcp_request(input())).await;
                assert_eq!(response["jsonrpc"], "2.0");
                assert_eq!(response["id"], 1998);
                if denied {
                    assert_eq!(
                        response,
                        json!({"jsonrpc":"2.0", "id":1998, "error":{
                        "code":-32603, "message":"Tool execution failed", "data":{
                            "category":"target_authorization", "target_code":"console_operation_permission_denied",
                            "http_status":403, "outcome":"failed", "retry_original":false}}})
                    );
                    let error = &response["error"]["data"];
                    json!({"status":error["http_status"], "code":error["target_code"]})
                } else {
                    assert!(response.get("error").is_none(), "{response}");
                    assert_ne!(response["result"]["isError"], true);
                    response["result"]["structuredContent"].clone()
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
                    assert_eq!(
                        response,
                        json!({"status":403,
                        "code":"console_operation_permission_denied",
                        "message":"permission denied: console_operation_permission_denied"})
                    );
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
                    response["data"].clone()
                }
            };
            let mut after = persisted(&pool).await;
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
                assert_eq!(
                    before["unchanged_rows"], after["unchanged_rows"],
                    "successful create must preserve all non-target models, fields and grants"
                );
                assert_persisted_create(&pool, &output, &mut after, workspace, started).await;
            }
            deltas.push(json!({"models":after["models"].as_i64().unwrap()-before["models"].as_i64().unwrap(),
                "fields":after["fields"].as_i64().unwrap()-before["fields"].as_i64().unwrap(),
                "grants":after["grants"].as_i64().unwrap()-before["grants"].as_i64().unwrap()}));
            assert!(
                Arc::ptr_eq(&snapshot, &registry.snapshot()),
                "request must not replace frozen registry"
            );
            results.push(
                json!({"response":if denied { output } else { normalized_model(output, workspace) },
                    "row":after["target"], "fields":after["target_fields"],
                    "grants":after["target_grants"], "audits":after["target_audits"]}),
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
    assert_eq!(
        response.headers()["content-type"],
        "text/plain; charset=utf-8"
    );
    let text = String::from_utf8(
        to_bytes(response.into_body(), 16_384)
            .await
            .unwrap()
            .to_vec(),
    )
    .unwrap();
    assert_eq!(
        text,
        format!(
            "Failed to deserialize the JSON body into the target type: missing field `scope_kind` at line 1 column {}",
            body.to_string().len()
        )
    );
    let response = call_mcp(&app, &token, mcp_request(body)).await;
    // Baseline debug_execute::build_interface_arguments rejects missing required mapping
    // with mcp_arguments; virtual_ui::interface_error preserves that field.
    assert_eq!(
        response,
        json!({"jsonrpc":"2.0", "id":1998, "error":{
        "code":-32602, "message":"Tool execution failed", "data":{
            "category":"invalid_tool_arguments", "field":"mcp_arguments",
            "outcome":"not_started", "retry_original":false}}})
    );
    assert_eq!(before, persisted(&pool).await);
    pool.close().await;
}
