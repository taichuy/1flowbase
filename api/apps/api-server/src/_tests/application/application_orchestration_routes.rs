use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_app, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::ports::I18nCatalogRepository;
use serde_json::{json, Value};
use tower::ServiceExt;

async fn activate_i18n_seed(state: &crate::app_state::ApiState) {
    let seed = crate::official_i18n_catalog_seed::load_official_i18n_catalog_seed().unwrap();
    let release = seed
        .bind_to_workspace(state.bootstrap_workspace_id)
        .unwrap();
    I18nCatalogRepository::import_verified_release(&state.store, &release)
        .await
        .unwrap();
    let catalog_state = I18nCatalogRepository::bootstrap_workspace_catalog_state(
        &state.store,
        state.bootstrap_workspace_id,
    )
    .await
    .unwrap();
    I18nCatalogRepository::activate_verified_release(
        &state.store,
        state.bootstrap_workspace_id,
        release.id(),
        catalog_state.revision(),
    )
    .await
    .unwrap();
}

#[tokio::test]
async fn ac_006_ac_010_flow_read_projects_only_referenced_i18n_messages() {
    let (state, _) = test_api_state_with_database_url().await;
    activate_i18n_seed(&state).await;
    let app = crate::app_with_state_and_config(state, &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Localized flow",
                        "description": "projection fixture"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let application_id = created["data"]["id"].as_str().unwrap();

    let initial = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let initial: Value =
        serde_json::from_slice(&to_bytes(initial.into_body(), usize::MAX).await.unwrap()).unwrap();
    let mut document = initial["data"]["draft"]["document"].clone();
    let nodes = document["graph"]["nodes"].as_array_mut().unwrap();
    nodes[0]["bindings"]["localized_title"] = json!({
        "kind": "i18n_text",
        "value": { "module": "@taichuy/platform/common", "key": "Cancel" }
    });
    nodes[1]["bindings"]["localized_title"] = json!({
        "kind": "i18n_text",
        "value": { "module": "@taichuy/platform/common", "key": "Cancel" }
    });
    nodes[1]["bindings"]["localized_missing"] = json!({
        "kind": "i18n_text",
        "value": { "module": "@taichuy/platform/common", "key": "Missing English" }
    });

    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/draft"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("x-1flowbase-locale", "zh_Hans")
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "document": document,
                        "change_kind": "logical",
                        "summary": "add localized bindings"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(save.status(), StatusCode::OK);
    let saved: Value =
        serde_json::from_slice(&to_bytes(save.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        saved["data"]["messages"],
        json!([
            { "module": "@taichuy/platform/common", "key": "Cancel", "text": "取消" },
            { "module": "@taichuy/platform/common", "key": "Missing English", "text": "Missing English" }
        ])
    );

    let read = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", &cookie)
                .header("x-1flowbase-locale", "zh_Hans")
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(read.status(), StatusCode::OK);
    let read: Value =
        serde_json::from_slice(&to_bytes(read.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(read["data"]["messages"], saved["data"]["messages"]);
}

#[tokio::test]
async fn application_orchestration_routes_bootstrap_save_and_restore() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Support Agent",
                        "description": "customer support",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(create.status(), StatusCode::CREATED);

    let created_body: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let application_id = created_body["data"]["id"].as_str().unwrap();

    let get_state = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(get_state.status(), StatusCode::OK);

    let get_state_body: Value =
        serde_json::from_slice(&to_bytes(get_state.into_body(), usize::MAX).await.unwrap())
            .unwrap();
    let version_id = get_state_body["data"]["versions"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();
    let mut document = get_state_body["data"]["draft"]["document"].clone();
    let start_node = document["graph"]["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|node| node["type"] == "start")
        .expect("default draft should include a start node");
    assert_eq!(start_node["outputs"], json!([]));
    assert_eq!(start_node["config"]["input_fields"], json!([]));

    let update_version = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PATCH")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/versions/{version_id}"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "summary": "stable baseline",
                        "summary_is_custom": true,
                        "is_user_protected": true
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(update_version.status(), StatusCode::OK);

    let update_version_body: Value = serde_json::from_slice(
        &to_bytes(update_version.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        update_version_body["data"]["versions"][0]["summary"],
        json!("stable baseline")
    );
    assert_eq!(
        update_version_body["data"]["versions"][0]["summary_is_custom"],
        json!(true)
    );
    assert_eq!(
        update_version_body["data"]["versions"][0]["is_user_protected"],
        json!(true)
    );
    assert_eq!(
        update_version_body["data"]["versions"][0]["is_current_publication"],
        json!(false)
    );

    document["graph"]["nodes"][1]["bindings"]["prompt_messages"]["value"][0]["content"]["value"] =
        json!("You are a support agent.");

    let save = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/draft"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "document": document,
                        "change_kind": "logical",
                        "summary": "update llm prompt"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(save.status(), StatusCode::OK);

    let restore = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/versions/{version_id}/restore"
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(restore.status(), StatusCode::OK);
}

#[tokio::test]
async fn application_orchestration_template_routes_export_preview_and_import() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_type": "agent_flow",
                        "name": "Template Source",
                        "description": "source app",
                        "icon": "RobotOutlined",
                        "icon_type": "iconfont",
                        "icon_background": "#E6F7F2"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create.status(), StatusCode::CREATED);
    let created_body: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let application_id = created_body["data"]["id"].as_str().unwrap();

    let state = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(state.status(), StatusCode::OK);
    let state_body: Value =
        serde_json::from_slice(&to_bytes(state.into_body(), usize::MAX).await.unwrap()).unwrap();
    let source_flow_id = state_body["data"]["flow_id"].as_str().unwrap().to_string();
    let source_node_id = state_body["data"]["draft"]["document"]["graph"]["nodes"][0]["id"]
        .as_str()
        .unwrap()
        .to_string();

    let export = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{application_id}/orchestration/template"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::OK);
    let export_body: Value =
        serde_json::from_slice(&to_bytes(export.into_body(), usize::MAX).await.unwrap()).unwrap();
    let template = export_body["data"].clone();
    assert_eq!(
        template["schema_version"],
        json!("1flowbase.application-template/v1")
    );
    assert_eq!(
        template["application"]["application_type"],
        json!("agent_flow")
    );

    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/orchestration/template/preview")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(json!({ "template": template }).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_body: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(preview_body["data"]["unresolved_nodes"], json!([]));

    let import = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/orchestration/template/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "template": preview_body["data"],
                        "name": "Template Imported"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import.status(), StatusCode::BAD_REQUEST);

    let import = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/orchestration/template/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "template": export_body["data"],
                        "name": "Template Imported"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import.status(), StatusCode::CREATED);
    let import_body: Value =
        serde_json::from_slice(&to_bytes(import.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_ne!(
        import_body["data"]["application"]["id"],
        json!(application_id)
    );
    assert_eq!(
        import_body["data"]["application"]["name"],
        json!("Template Imported")
    );
    assert_ne!(
        import_body["data"]["orchestration"]["flow_id"],
        json!(source_flow_id)
    );
    assert_eq!(
        import_body["data"]["orchestration"]["draft"]["document"]["meta"]["flowId"],
        import_body["data"]["orchestration"]["flow_id"]
    );
    assert_eq!(
        import_body["data"]["orchestration"]["draft"]["document"]["graph"]["nodes"][0]["id"],
        json!(source_node_id)
    );
}

#[tokio::test]
async fn application_orchestration_official_template_routes_list_and_download_catalog_entry() {
    let app = test_app().await;
    let (cookie, _) = login_and_capture_cookie(&app, "root", "change-me").await;

    let catalog = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/applications/orchestration/templates/official-catalog")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog.status(), StatusCode::OK);
    let catalog_body: Value =
        serde_json::from_slice(&to_bytes(catalog.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        catalog_body["data"]["source"]["source_label"],
        "Official source"
    );
    assert_eq!(
        catalog_body["data"]["source"]["source_kind"],
        "official_registry"
    );
    assert_eq!(
        catalog_body["data"]["source"]["index_url"],
        "https://raw.githubusercontent.com/taichuy/1flowbase-official-plugins/main/agent-flow/catalog/v1/index.json"
    );
    let entry = &catalog_body["data"]["entries"][0];

    assert_eq!(entry["workflow_id"], json!("multimodal-mount-test"));
    assert_eq!(
        entry["schema_version"],
        json!("1flowbase.application-template/v1")
    );
    assert_eq!(entry["application"]["name"], json!("多模态挂载测试"));
    assert!(entry.get("dependency_summary").is_none());
    assert!(entry.get("tags").is_none());
    assert!(entry.get("author").is_none());
    assert!(entry.get("status").is_none());

    let template = app
        .clone()
        .oneshot(
            Request::builder()
                .uri("/api/console/applications/orchestration/templates/official/multimodal-mount-test")
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(template.status(), StatusCode::OK);
    let template_body: Value =
        serde_json::from_slice(&to_bytes(template.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        template_body["data"]["schema_version"],
        json!("1flowbase.application-template/v1")
    );
    assert_eq!(
        template_body["data"]["application"]["name"],
        json!("多模态挂载测试")
    );
    assert_eq!(template_body["data"]["dependencies"], json!([]));
}
