use super::*;

#[tokio::test]
async fn draft_analysis_authenticates_without_csrf_and_does_not_write_editor_state() {
    let (state, _) = test_api_state_with_database_url().await;
    let pool = state.store.pool().clone();
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let create = app.clone().oneshot(Request::builder()
        .method("POST").uri("/api/console/applications")
        .header("cookie", &cookie).header("x-csrf-token", &csrf)
        .header("content-type", "application/json")
        .body(Body::from(json!({"application_type":"workflow", "name":"Read only analysis", "description":""}).to_string()))
        .unwrap()).await.unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let application_id = created["data"]["id"].as_str().unwrap();
    let id = uuid::Uuid::parse_str(application_id).unwrap();
    let before: Value = sqlx::query_scalar(
        "select jsonb_build_object('flows', (select count(*) from flows where application_id=$1), \
         'drafts', (select count(*) from flow_drafts d join flows f on f.id=d.flow_id where f.application_id=$1), \
         'versions', (select count(*) from flow_versions v join flows f on f.id=v.flow_id where f.application_id=$1))",
    ).bind(id).fetch_one(&pool).await.unwrap();
    let uri = format!("/api/console/applications/{application_id}/orchestration/analysis");
    let mut document = domain::default_flow_document_for_application(
        domain::ApplicationType::Workflow,
        uuid::Uuid::nil(),
    );
    document["graph"]["nodes"][1]["bindings"]["result"] =
        json!({"kind":"selector", "value":["missing-source", "result"]});
    let unauthenticated = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header("content-type", "application/json")
                .body(Body::from(json!({"document":document}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(unauthenticated.status(), StatusCode::UNAUTHORIZED);
    let response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(json!({"document":document}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let analyzed: Value =
        serde_json::from_slice(&to_bytes(response.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert!(analyzed["data"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["level"] == "error"));
    assert!(analyzed["data"]["node_outputs"].is_array());
    assert_eq!(analyzed["data"]["variable_catalogs"], json!([]));
    let selected = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({"document":document, "node_id":"node-workflow-end"}).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(selected.status(), StatusCode::OK);
    let selected: Value =
        serde_json::from_slice(&to_bytes(selected.into_body(), usize::MAX).await.unwrap()).unwrap();
    let catalogs = selected["data"]["variable_catalogs"].as_array().unwrap();
    assert!(!catalogs.is_empty());
    assert!(catalogs
        .iter()
        .all(|catalog| catalog["node_id"] == "node-workflow-end"));
    assert_eq!(
        selected["data"]["diagnostics"],
        analyzed["data"]["diagnostics"]
    );
    assert_eq!(
        selected["data"]["node_outputs"],
        analyzed["data"]["node_outputs"]
    );
    let after: Value = sqlx::query_scalar(
        "select jsonb_build_object('flows', (select count(*) from flows where application_id=$1), \
         'drafts', (select count(*) from flow_drafts d join flows f on f.id=d.flow_id where f.application_id=$1), \
         'versions', (select count(*) from flow_versions v join flows f on f.id=v.flow_id where f.application_id=$1))",
    ).bind(id).fetch_one(&pool).await.unwrap();
    assert_eq!(after, before);
    let malformed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(&uri)
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(json!({"document":{}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(malformed.status(), StatusCode::BAD_REQUEST);
    let saved = app.clone().oneshot(Request::builder()
        .method("PUT").uri(format!("/api/console/applications/{application_id}/orchestration/draft"))
        .header("cookie", &cookie).header("x-csrf-token", &csrf).header("content-type", "application/json")
        .body(Body::from(json!({"document":document,"change_kind":"logical","summary":"Save editable semantic error"}).to_string())).unwrap()).await.unwrap();
    assert_eq!(saved.status(), StatusCode::OK);
    let saved: Value =
        serde_json::from_slice(&to_bytes(saved.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(saved["data"]["draft"]["document"], document);
    assert!(saved["data"]["analysis"]["diagnostics"]
        .as_array()
        .unwrap()
        .iter()
        .any(|diagnostic| diagnostic["level"] == "error"));
}
