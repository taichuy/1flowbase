use crate::_tests::support::{login_and_capture_cookie, test_app, test_app_with_database_url};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use serde_json::{json, Value};
use tower::ServiceExt;

async fn create_member(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    account: &str,
    password: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/members")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "account": account,
                        "email": format!("{account}@example.com"),
                        "phone": null,
                        "password": password,
                        "name": account,
                        "nickname": account,
                        "introduction": "",
                        "email_login_enabled": true,
                        "phone_login_enabled": false
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);

    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn create_role(app: &axum::Router, cookie: &str, csrf: &str, code: &str) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/roles")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": code,
                        "name": code,
                        "introduction": "docs test role"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

async fn replace_role_permissions(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    role_code: &str,
    permission_codes: &[&str],
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/roles/{role_code}/permissions"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "permission_codes": permission_codes,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

async fn replace_member_roles(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    member_id: &str,
    role_codes: &[&str],
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/members/{member_id}/roles"))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "role_codes": role_codes,
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NO_CONTENT);
}

async fn create_model(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    code: &str,
    status: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/data-models/model-definitions")
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": code,
                        "title": code,
                        "status": status
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn create_model_field(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
    input: TestModelFieldInput,
) {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/fields"
                ))
                .header("cookie", cookie)
                .header("x-csrf-token", csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": input.code,
                        "title": input.code,
                        "field_kind": input.field_kind,
                        "is_required": input.is_required,
                        "api_required": input.api_required
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::CREATED);
}

struct TestModelFieldInput {
    code: &'static str,
    field_kind: &'static str,
    is_required: bool,
    api_required: bool,
}

async fn create_scope_grant(
    app: &axum::Router,
    cookie: &str,
    csrf: &str,
    model_id: &str,
) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{model_id}/scope-grants"
                ))
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

    assert_eq!(response.status(), StatusCode::CREATED);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]["id"].as_str().unwrap().to_string()
}

async fn find_model_id_by_code(app: &axum::Router, cookie: &str, code: &str) -> String {
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions?filter=%7B%22code%22%3A%7B%22%24eq%22%3A%22{code}%22%7D%7D"
                ))
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::OK);
    let body = to_bytes(response.into_body(), usize::MAX).await.unwrap();
    let payload: Value = serde_json::from_slice(&body).unwrap();
    payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|model| model["code"] == json!(code))
        .and_then(|model| model["id"].as_str())
        .expect("model should exist")
        .to_string()
}

#[tokio::test]
async fn docs_catalog_requires_session_and_permission() {
    let app = test_app().await;

    let unauthenticated_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/catalog")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(unauthenticated_response.status(), StatusCode::UNAUTHORIZED);

    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    create_member(&app, &root_cookie, &root_csrf, "docs-blocked", "temp-pass").await;
    let (member_cookie, _) = login_and_capture_cookie(&app, "docs-blocked", "temp-pass").await;

    let forbidden_response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/catalog")
                .header("cookie", member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(forbidden_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn docs_routes_allow_root_and_granted_members() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/catalog")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog_body = to_bytes(catalog_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let catalog_payload: Value = serde_json::from_slice(&catalog_body).unwrap();
    assert!(!catalog_payload["data"]["categories"]
        .as_array()
        .unwrap()
        .is_empty());

    let category_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/categories/console/openapi.json")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(category_response.status(), StatusCode::OK);
    let category_body = to_bytes(category_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let category_payload: Value = serde_json::from_slice(&category_body).unwrap();
    assert_eq!(category_payload["info"]["title"], "1flowbase API");
    assert!(category_payload["paths"]["/api/console/me"]["patch"].is_object());
    assert!(category_payload["paths"]["/api/console/settings/members"]["get"].is_object());

    let operation_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/operations/patch_me/openapi.json")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operation_response.status(), StatusCode::OK);
    let operation_body = to_bytes(operation_response.into_body(), usize::MAX)
        .await
        .unwrap();
    let operation_payload: Value = serde_json::from_slice(&operation_body).unwrap();
    assert_eq!(operation_payload["servers"][0]["url"], "/");
    assert_eq!(
        operation_payload["security"],
        json!([
            { "sessionCookie": [], "csrfHeader": [] },
            { "patBearer": [] }
        ])
    );
    assert_eq!(
        operation_payload["components"]["securitySchemes"]["sessionCookie"]["in"],
        "cookie"
    );
    assert_eq!(
        operation_payload["components"]["securitySchemes"]["csrfHeader"]["name"],
        "x-csrf-token"
    );
    assert_eq!(
        operation_payload["components"]["securitySchemes"]["patBearer"]["scheme"],
        "bearer"
    );

    let member_id = create_member(&app, &root_cookie, &root_csrf, "docs-viewer", "temp-pass").await;
    create_role(&app, &root_cookie, &root_csrf, "docs_viewer").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "docs_viewer",
        &["settings_feature.access.system.docs"],
    )
    .await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &["docs_viewer"]).await;
    let (member_cookie, _) = login_and_capture_cookie(&app, "docs-viewer", "temp-pass").await;

    let member_catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/catalog")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(member_catalog_response.status(), StatusCode::OK);

    let member_category_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/categories/console/openapi.json")
                .header("cookie", &member_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(member_category_response.status(), StatusCode::OK);
}

#[tokio::test]
async fn docs_operation_route_returns_404_for_unknown_operation() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/operations/unknown_operation/openapi.json")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn docs_category_operations_route_paginates_twenty_at_a_time() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let first_page_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/categories/console/operations")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(first_page_response.status(), StatusCode::OK);
    let first_page_payload: Value = serde_json::from_slice(
        &to_bytes(first_page_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let first_page_operations = first_page_payload["data"]["operations"].as_array().unwrap();

    assert_eq!(first_page_operations.len(), 20);
    assert!(first_page_payload["data"]["total"].as_u64().unwrap() > 20);
    assert_eq!(first_page_payload["data"]["offset"], json!(0));
    assert_eq!(first_page_payload["data"]["limit"], json!(20));
    assert_eq!(first_page_payload["data"]["has_more"], json!(true));
    assert_eq!(first_page_payload["data"]["next_offset"], json!(20));

    let first_operation_id = first_page_operations[0]["id"].clone();
    let second_page_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/categories/console/operations?offset=20&limit=20")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(second_page_response.status(), StatusCode::OK);
    let second_page_payload: Value = serde_json::from_slice(
        &to_bytes(second_page_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let second_page_operations = second_page_payload["data"]["operations"]
        .as_array()
        .unwrap();

    assert_eq!(second_page_operations.len(), 20);
    assert_eq!(second_page_payload["data"]["offset"], json!(20));
    assert_eq!(second_page_payload["data"]["limit"], json!(20));
    assert_ne!(second_page_operations[0]["id"], first_operation_id);
}

#[tokio::test]
async fn docs_category_route_returns_404_for_unknown_category() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let response = app
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/categories/missing/openapi.json")
                .header("cookie", cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(response.status(), StatusCode::NOT_FOUND);
}

#[tokio::test]
async fn docs_routes_append_dynamic_data_model_api_category_and_specs() {
    let (app, _database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let ready_model_id = create_model(&app, &cookie, &csrf, "docs_ready_orders", "published").await;
    create_model_field(
        &app,
        &cookie,
        &csrf,
        &ready_model_id,
        TestModelFieldInput {
            code: "order_title",
            field_kind: "string",
            is_required: true,
            api_required: false,
        },
    )
    .await;
    create_model_field(
        &app,
        &cookie,
        &csrf,
        &ready_model_id,
        TestModelFieldInput {
            code: "paid_at",
            field_kind: "datetime",
            is_required: false,
            api_required: true,
        },
    )
    .await;
    create_scope_grant(&app, &cookie, &csrf, &ready_model_id).await;
    let hidden_model_id = create_model(&app, &cookie, &csrf, "docs_hidden_orders", "draft").await;
    assert_ne!(ready_model_id, hidden_model_id);

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog_payload: Value = serde_json::from_slice(
        &to_bytes(catalog_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let dynamic_category = catalog_payload["data"]["categories"]
        .as_array()
        .unwrap()
        .iter()
        .find(|category| category["id"] == json!("data-model-apis"))
        .cloned()
        .expect("dynamic data model api category should exist");
    assert_eq!(dynamic_category["label"], json!("Data Model APIs"));
    assert!(dynamic_category["operation_count"]
        .as_u64()
        .is_some_and(|count| count >= 5));

    let operations_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/categories/data-model-apis/operations?q=docs_ready_orders")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operations_response.status(), StatusCode::OK);
    let operations_payload: Value = serde_json::from_slice(
        &to_bytes(operations_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let operations = operations_payload["data"]["operations"]
        .as_array()
        .unwrap()
        .to_vec();
    assert_eq!(operations.len(), 5);
    for (method, path) in [
        ("GET", "/api/runtime/models/docs_ready_orders/list"),
        ("GET", "/api/runtime/models/docs_ready_orders/get/{id}"),
        ("POST", "/api/runtime/models/docs_ready_orders/create"),
        ("PATCH", "/api/runtime/models/docs_ready_orders/update/{id}"),
        (
            "DELETE",
            "/api/runtime/models/docs_ready_orders/delete/{id}",
        ),
    ] {
        assert!(
            operations
                .iter()
                .any(|operation| operation["method"] == json!(method)
                    && operation["path"] == json!(path)),
            "missing dynamic data model operation {method} {path}"
        );
    }
    assert!(!operations.iter().any(|operation| {
        operation["path"]
            .as_str()
            .is_some_and(|path| path.contains("docs_hidden_orders"))
    }));

    let category_spec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/docs/categories/data-model-apis/openapi.json")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(category_spec_response.status(), StatusCode::OK);
    let category_spec: Value = serde_json::from_slice(
        &to_bytes(category_spec_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        category_spec["paths"]["/api/runtime/models/docs_ready_orders/list"]["get"].is_object()
    );
    assert!(
        category_spec["paths"]["/api/runtime/models/docs_ready_orders/create"]["post"].is_object()
    );
    assert!(
        category_spec["paths"]["/api/runtime/models/docs_ready_orders/get/{id}"]["get"].is_object()
    );
    assert!(
        category_spec["paths"]["/api/runtime/models/docs_ready_orders/update/{id}"]["patch"]
            .is_object()
    );
    assert!(
        category_spec["paths"]["/api/runtime/models/docs_ready_orders/delete/{id}"]["delete"]
            .is_object()
    );
    assert!(category_spec["paths"]
        .get("/api/runtime/models/docs_hidden_orders/list")
        .is_none());

    let hidden_model_spec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{hidden_model_id}/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(hidden_model_spec_response.status(), StatusCode::NOT_FOUND);

    let list_operation = operations
        .iter()
        .find(|operation| {
            operation["method"] == json!("GET")
                && operation["path"] == json!("/api/runtime/models/docs_ready_orders/list")
        })
        .expect("list operation should exist");
    let list_operation_id = list_operation["id"].as_str().unwrap();

    let operation_spec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/docs/operations/{list_operation_id}/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(operation_spec_response.status(), StatusCode::OK);
    let operation_spec: Value = serde_json::from_slice(
        &to_bytes(operation_spec_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert!(
        operation_spec["paths"]["/api/runtime/models/docs_ready_orders/list"]["get"].is_object()
    );
    assert!(operation_spec["x-data-model"]
        .get("api_exposure_status")
        .is_none());
    assert!(operation_spec["paths"]
        .get("/api/runtime/models/{model_code}/list")
        .is_none());
    assert!(operation_spec["paths"]
        .get("/api/runtime/models/docs_ready_orders/create")
        .is_none());
    let create_operation = operations
        .iter()
        .find(|operation| {
            operation["method"] == json!("POST")
                && operation["path"] == json!("/api/runtime/models/docs_ready_orders/create")
        })
        .expect("create operation should exist");
    let create_operation_id = create_operation["id"].as_str().unwrap();

    let create_spec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/docs/operations/{create_operation_id}/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_spec_response.status(), StatusCode::OK);
    let create_spec: Value = serde_json::from_slice(
        &to_bytes(create_spec_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let create_body_schema = &create_spec["paths"]["/api/runtime/models/docs_ready_orders/create"]
        ["post"]["requestBody"]["content"]["application/json"]["schema"];
    assert_eq!(
        create_body_schema["$ref"],
        json!("#/components/schemas/DocsReadyOrdersRecordCreateInput")
    );
    assert_eq!(
        create_spec["components"]["schemas"]["DocsReadyOrdersRecordCreateInput"]["required"],
        json!(["paid_at"])
    );
    let create_input_properties =
        &create_spec["components"]["schemas"]["DocsReadyOrdersRecordCreateInput"]["properties"];
    for readonly_field in [
        "id",
        "scope_id",
        "created_by",
        "updated_by",
        "created_at",
        "updated_at",
    ] {
        assert!(
            create_input_properties.get(readonly_field).is_none(),
            "create input schema should not expose readonly field {readonly_field}"
        );
    }
    assert_eq!(
        create_input_properties["order_title"]["type"],
        json!("string")
    );
    assert_eq!(
        create_input_properties["paid_at"]["format"],
        json!("date-time")
    );
    assert_eq!(
        create_spec["components"]["schemas"]["DocsReadyOrdersRecord"]["properties"]["created_at"]
            ["format"],
        json!("date-time")
    );

    let update_operation = operations
        .iter()
        .find(|operation| {
            operation["method"] == json!("PATCH")
                && operation["path"] == json!("/api/runtime/models/docs_ready_orders/update/{id}")
        })
        .expect("update operation should exist");
    let update_operation_id = update_operation["id"].as_str().unwrap();
    let update_spec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/docs/operations/{update_operation_id}/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_spec_response.status(), StatusCode::OK);
    let update_spec: Value = serde_json::from_slice(
        &to_bytes(update_spec_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let update_body_schema = &update_spec["paths"]
        ["/api/runtime/models/docs_ready_orders/update/{id}"]["patch"]["requestBody"]["content"]
        ["application/json"]["schema"];
    assert_eq!(
        update_body_schema["$ref"],
        json!("#/components/schemas/DocsReadyOrdersRecordUpdateInput")
    );
    assert_eq!(
        update_spec["components"]["schemas"]["DocsReadyOrdersRecordUpdateInput"]["required"],
        json!([])
    );
    let update_input_properties =
        &update_spec["components"]["schemas"]["DocsReadyOrdersRecordUpdateInput"]["properties"];
    assert_eq!(
        update_input_properties["order_title"]["type"],
        json!("string")
    );
    assert_eq!(
        update_input_properties["paid_at"]["format"],
        json!("date-time")
    );
    for readonly_field in [
        "id",
        "scope_id",
        "created_by",
        "updated_by",
        "created_at",
        "updated_at",
    ] {
        assert!(
            update_input_properties.get(readonly_field).is_none(),
            "update input schema should not expose readonly field {readonly_field}"
        );
    }

    assert!(operation_spec["components"]["securitySchemes"]["apiKeyBearer"].is_null());
    assert_eq!(
        operation_spec["components"]["securitySchemes"]["patBearer"]["scheme"],
        json!("bearer")
    );
}

#[tokio::test]
async fn docs_routes_data_model_write_schema_omits_readonly_system_fields() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;
    let roles_model_id = find_model_id_by_code(&app, &cookie, "roles").await;

    let create_spec_response = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/settings/data-models/model-definitions/{roles_model_id}/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_spec_response.status(), StatusCode::OK);
    let create_spec: Value = serde_json::from_slice(
        &to_bytes(create_spec_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();

    let properties = &create_spec["components"]["schemas"]["RolesRecordCreateInput"]["properties"];
    for readonly_field in ["id", "created_by", "updated_by", "created_at", "updated_at"] {
        assert!(
            properties.get(readonly_field).is_none(),
            "roles create input schema should not expose readonly field {readonly_field}"
        );
    }
    let required = create_spec["components"]["schemas"]["RolesRecordCreateInput"]["required"]
        .as_array()
        .expect("roles create input required should be an array");
    for readonly_field in ["id", "created_by", "updated_by", "created_at", "updated_at"] {
        assert!(
            !required.contains(&json!(readonly_field)),
            "roles create input required should not include readonly field {readonly_field}"
        );
    }
}
