use crate::_tests::support::{
    login_and_capture_cookie, test_api_state_with_database_url, test_app, test_config,
};
use axum::{
    body::{to_bytes, Body},
    http::{Request, StatusCode},
};
use control_plane::{
    plugin_management::{
        ExtensionArtifactInstallOutcome, ExtensionCatalogCategory, ExtensionInstallationService,
        InstallExtensionArtifactCommand,
    },
    ports::{AuthRepository, I18nCatalogRepository},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::io::{Cursor, Read};
use tower::ServiceExt;
use zip::ZipArchive;

fn application_archive_multipart(boundary: &str, archive: &[u8], name: Option<&str>) -> Vec<u8> {
    let mut body = Vec::new();
    body.extend_from_slice(
        format!(
            "--{boundary}\r\nContent-Disposition: form-data; name=\"file\"; filename=\"application.1flowbase-application.json\"\r\nContent-Type: application/json\r\n\r\n"
        )
        .as_bytes(),
    );
    body.extend_from_slice(archive);
    body.extend_from_slice(b"\r\n");
    if let Some(name) = name {
        body.extend_from_slice(
            format!(
                "--{boundary}\r\nContent-Disposition: form-data; name=\"name\"\r\n\r\n{name}\r\n"
            )
            .as_bytes(),
        );
    }
    body.extend_from_slice(format!("--{boundary}--\r\n").as_bytes());
    body
}

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
        "value": { "key": "Cancel" }
    });
    nodes[1]["bindings"]["localized_title"] = json!({
        "kind": "i18n_text",
        "value": { "key": "Cancel" }
    });
    nodes[1]["bindings"]["localized_missing"] = json!({
        "kind": "i18n_text",
        "value": { "key": "Missing English" }
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
            { "key": "Cancel", "text": "取消" },
            { "key": "Missing English", "text": "Missing English" }
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
async fn ac_002_ac_004_exports_selected_agent_flow_and_workflow_as_zip_archive() {
    let app = test_app().await;
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_agent = app
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
                        "name": "Archive Agent",
                        "description": "agent archive fixture"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_agent.status(), StatusCode::CREATED);
    let agent_body: Value = serde_json::from_slice(
        &to_bytes(create_agent.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let agent_id = agent_body["data"]["id"].as_str().unwrap();

    let create_workflow = app
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
                        "application_type": "workflow",
                        "workflow_trigger_type": "schedule",
                        "workflow_trigger_config": {
                            "cron": "0 9 * * 1-5",
                            "timezone": "Asia/Shanghai",
                            "input_payload": { "report": "daily" }
                        },
                        "name": "Archive Workflow",
                        "description": "workflow archive fixture"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_workflow.status(), StatusCode::CREATED);
    let workflow_body: Value = serde_json::from_slice(
        &to_bytes(create_workflow.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    let workflow_id = workflow_body["data"]["id"].as_str().unwrap();

    let export = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "application_ids": [agent_id, workflow_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();

    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(
        export.headers().get("content-type").unwrap(),
        "application/zip"
    );
    assert!(export
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap()
        .contains(
            "filename=\"applications-2-items.zip\"; filename*=UTF-8''applications-2-items.zip"
        ));

    let archive_bytes = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let mut archive = ZipArchive::new(Cursor::new(archive_bytes)).unwrap();
    let mut manifest_json = String::new();
    archive
        .by_name("manifest.json")
        .unwrap()
        .read_to_string(&mut manifest_json)
        .unwrap();
    let manifest: Value = serde_json::from_str(&manifest_json).unwrap();

    assert_eq!(
        manifest["schema_version"],
        json!("1flowbase.application-archive/v1")
    );
    assert_eq!(manifest["applications"].as_array().unwrap().len(), 2);
    let workflow = manifest["applications"]
        .as_array()
        .unwrap()
        .iter()
        .find(|entry| entry["application"]["application_type"] == "workflow")
        .unwrap();
    assert_eq!(
        workflow["application"]["workflow_trigger_type"],
        json!("schedule")
    );
    assert_eq!(
        workflow["workflow_trigger_config"]["cron"],
        json!("0 9 * * 1-5")
    );
    assert_eq!(
        workflow["workflow_trigger_config"]["timezone"],
        json!("Asia/Shanghai")
    );
    assert!(workflow["workflow_trigger_config"].get("enabled").is_none());
}
mod archive_import;
