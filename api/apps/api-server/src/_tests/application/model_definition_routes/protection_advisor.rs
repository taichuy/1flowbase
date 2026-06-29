use super::*;

#[tokio::test]
async fn protected_model_routes_reject_non_root_admin_mutations() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    create_role(&app, &root_cookie, &root_csrf, "model_admin").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "model_admin",
        &[
            "state_model.view.all",
            "state_model.manage.all",
            "api_reference.view.all",
        ],
    )
    .await;
    let member_id = create_member(
        &app,
        &root_cookie,
        &root_csrf,
        "protected-admin",
        "temp-pass",
    )
    .await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &["model_admin"]).await;
    let (admin_cookie, admin_csrf) =
        login_and_capture_cookie(&app, "protected-admin", "temp-pass").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "protected_route_orders",
                        "title": "Protected Route Orders"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created["data"]["id"].as_str().unwrap().to_string();

    let field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/fields"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "email",
                        "title": "Email",
                        "field_kind": "string"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(field_response.status(), StatusCode::CREATED);
    let created_field: serde_json::Value = serde_json::from_slice(
        &to_bytes(field_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let field_id = created_field["data"]["id"].as_str().unwrap().to_string();
    protect_model(&database_url, &model_id).await;

    for request in [
        Request::builder()
            .method("PATCH")
            .uri(format!("/api/console/models/{model_id}"))
            .header("cookie", &admin_cookie)
            .header("x-csrf-token", &admin_csrf)
            .header("content-type", "application/json")
            .body(Body::from(json!({ "status": "disabled" }).to_string()))
            .unwrap(),
        Request::builder()
            .method("PATCH")
            .uri(format!("/api/console/models/{model_id}/fields/{field_id}"))
            .header("cookie", &admin_cookie)
            .header("x-csrf-token", &admin_csrf)
            .header("content-type", "application/json")
            .body(Body::from(
                json!({
                    "title": "Work Email",
                    "is_required": true,
                    "is_unique": false,
                    "relation_options": {}
                })
                .to_string(),
            ))
            .unwrap(),
        Request::builder()
            .method("DELETE")
            .uri(format!(
                "/api/console/models/{model_id}/fields/{field_id}?confirmed=true"
            ))
            .header("cookie", &admin_cookie)
            .header("x-csrf-token", &admin_csrf)
            .body(Body::empty())
            .unwrap(),
        Request::builder()
            .method("DELETE")
            .uri(format!("/api/console/models/{model_id}?confirmed=true"))
            .header("cookie", &admin_cookie)
            .header("x-csrf-token", &admin_csrf)
            .body(Body::empty())
            .unwrap(),
    ] {
        let response = app.clone().oneshot(request).await.unwrap();
        assert_eq!(response.status(), StatusCode::FORBIDDEN);
        let payload: serde_json::Value =
            serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap())
                .unwrap();
        assert_eq!(payload["code"], json!("protected_data_model"));
    }

    let root_delete = app
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/models/{model_id}?confirmed=true"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_delete.status(), StatusCode::OK);
}

#[tokio::test]
async fn model_definition_routes_expose_advisor_findings_and_dynamic_openapi_docs() {
    let (app, database_url) = test_app_with_database_url().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/models")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "scope_kind": "workspace",
                        "code": "advisor_doc_orders",
                        "title": "Advisor Doc Orders"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_response.status(), StatusCode::CREATED);
    let created: serde_json::Value = serde_json::from_slice(
        &to_bytes(create_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let model_id = created["data"]["id"].as_str().unwrap().to_string();

    let field_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!("/api/console/models/{model_id}/fields"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "status",
                        "title": "Status",
                        "field_kind": "enum",
                        "is_required": true,
                        "display_options": { "options": ["draft", "paid"] }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(field_response.status(), StatusCode::CREATED);
    protect_model(&database_url, &model_id).await;

    let advisor_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!("/api/console/models/{model_id}/advisor-findings"))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(advisor_response.status(), StatusCode::OK);
    let advisor_payload: serde_json::Value = serde_json::from_slice(
        &to_bytes(advisor_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let finding_codes = advisor_payload["data"]
        .as_array()
        .unwrap()
        .iter()
        .map(|finding| finding["code"].as_str().unwrap())
        .collect::<Vec<_>>();
    assert!(finding_codes.contains(&"protected_model_exposure_attempt"));

    let docs_response = app
        .oneshot(
            Request::builder()
                .method("GET")
                .uri(format!(
                    "/api/console/docs/data-models/{model_id}/openapi.json"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(docs_response.status(), StatusCode::OK);
    let docs: serde_json::Value = serde_json::from_slice(
        &to_bytes(docs_response.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();

    assert_eq!(docs["openapi"], json!("3.1.0"));
    assert!(docs["paths"]["/api/runtime/models/advisor_doc_orders/list"]["get"].is_object());
    assert!(docs["paths"]["/api/runtime/models/advisor_doc_orders/create"]["post"].is_object());
    assert!(docs["paths"]["/api/runtime/models/advisor_doc_orders/get/{id}"]["get"].is_object());
    assert!(
        docs["paths"]["/api/runtime/models/advisor_doc_orders/update/{id}"]["patch"].is_object()
    );
    assert!(
        docs["paths"]["/api/runtime/models/advisor_doc_orders/delete/{id}"]["delete"].is_object()
    );
    assert_eq!(
        docs["components"]["schemas"]["AdvisorDocOrdersRecord"]["properties"]["status"]["type"],
        json!("string")
    );
    assert!(docs["components"]["securitySchemes"]["apiKeyBearer"].is_null());
    assert_eq!(
        docs["components"]["securitySchemes"]["patBearer"]["scheme"],
        json!("bearer")
    );
    assert_eq!(
        docs["x-data-model"]["api_exposure_status"],
        json!("api_exposed_ready")
    );
    assert!(docs["x-scope-permission-note"]
        .as_str()
        .unwrap()
        .contains("scope grant"));
    assert!(docs["x-external-source-safety-limits"]
        .as_str()
        .unwrap()
        .contains("scope filter"));
    assert_eq!(
        docs["paths"]["/api/runtime/models/advisor_doc_orders/list"]["get"]["parameters"][0]
            ["name"],
        json!("filter")
    );
    assert!(
        docs["paths"]["/api/runtime/models/advisor_doc_orders/list"]["get"]["responses"]["403"]
            .is_object()
    );
}
