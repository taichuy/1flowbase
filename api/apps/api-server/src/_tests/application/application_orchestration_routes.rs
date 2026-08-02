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

#[tokio::test]
async fn application_export_ac_001_single_application_json_previews_and_imports_as_draft() {
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
                        "name": "DeepSeek V4 测试",
                        "description": "archive import fixture"
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
    let source_id = created["data"]["id"].as_str().unwrap();

    let export = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "application_ids": [source_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::OK);
    assert_eq!(
        export.headers().get("content-type").unwrap(),
        "application/json; charset=utf-8"
    );
    let disposition = export
        .headers()
        .get("content-disposition")
        .unwrap()
        .to_str()
        .unwrap();
    assert!(disposition.contains("filename=\"DeepSeek-V4-.1flowbase-application.json\""));
    assert!(disposition
        .contains("filename*=UTF-8''DeepSeek-V4-%E6%B5%8B%E8%AF%95.1flowbase-application.json"));
    let archive = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let exported_package: Value = serde_json::from_slice(&archive).unwrap();
    assert_eq!(
        exported_package["schema_version"],
        json!("1flowbase.application-archive/v1")
    );
    assert_eq!(
        exported_package["applications"].as_array().unwrap().len(),
        1
    );
    assert_eq!(
        exported_package["applications"][0]["application"]["name"],
        json!("DeepSeek V4 测试")
    );

    let preview_boundary = "preview-application-archive";
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/preview")
                .header("cookie", &cookie)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={preview_boundary}"),
                )
                .body(Body::from(application_archive_multipart(
                    preview_boundary,
                    &archive,
                    None,
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let preview_body: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        preview_body["data"]["application"]["name"],
        json!("DeepSeek V4 测试")
    );

    let import_boundary = "import-application-archive";
    let imported = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={import_boundary}"),
                )
                .body(Body::from(application_archive_multipart(
                    import_boundary,
                    &archive,
                    Some("Imported Portable Agent"),
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), StatusCode::CREATED);
    let imported_body: Value =
        serde_json::from_slice(&to_bytes(imported.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_ne!(imported_body["data"]["application"]["id"], source_id);
    assert_eq!(
        imported_body["data"]["application"]["name"],
        json!("Imported Portable Agent")
    );
    assert_eq!(
        imported_body["data"]["application"]["application_type"],
        json!("agent_flow")
    );

    let exported_entry = &exported_package["applications"][0];
    let legacy_template = json!({
        "schema_version": "1flowbase.application-template/v1",
        "application": {
            "application_type": exported_entry["application"]["application_type"],
            "name": exported_entry["application"]["name"],
            "description": exported_entry["application"]["description"],
            "icon": exported_entry["application"]["icon"],
            "icon_type": exported_entry["application"]["icon_type"],
            "icon_background": exported_entry["application"]["icon_background"]
        },
        "flow_document": exported_entry["flow_document"],
        "dependencies": exported_entry["dependencies"]
    });
    let legacy_template = serde_json::to_vec_pretty(&legacy_template).unwrap();
    let legacy_boundary = "import-legacy-application-template";
    let legacy_import = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={legacy_boundary}"),
                )
                .body(Body::from(application_archive_multipart(
                    legacy_boundary,
                    &legacy_template,
                    Some("Imported Legacy Agent"),
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(legacy_import.status(), StatusCode::CREATED);
    let legacy_import_body: Value = serde_json::from_slice(
        &to_bytes(legacy_import.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        legacy_import_body["data"]["application"]["name"],
        json!("Imported Legacy Agent")
    );
}

#[tokio::test]
async fn delivery_1545_d6_installed_agent_flow_previews_imports_and_reports_workspace_application()
{
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let actor = AuthRepository::find_user_for_password_login(
        &state.store,
        domain::PASSWORD_LOCAL_AUTHENTICATOR_ID,
        "root",
    )
    .await
    .unwrap()
    .unwrap();

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
                        "name": "Installed extension source",
                        "description": "extension source fixture"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let created: Value =
        serde_json::from_slice(&to_bytes(create.into_body(), usize::MAX).await.unwrap()).unwrap();
    let source_id = created["data"]["id"].as_str().unwrap();
    let export = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "application_ids": [source_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let archive = to_bytes(export.into_body(), usize::MAX)
        .await
        .unwrap()
        .to_vec();
    let outcome =
        ExtensionInstallationService::new(state.store.clone(), &state.provider_install_root)
            .install_from_bytes(InstallExtensionArtifactCommand {
                actor_user_id: actor.id,
                category: ExtensionCatalogCategory::AgentFlow,
                organization: "taichuy".into(),
                artifact_id: "installed-flow".into(),
                version: "1.0.0".into(),
                node_id: state.api_node_id.clone(),
                artifact_bytes: archive.clone(),
                source: "upload".into(),
                trust: "unknown".into(),
                expected_checksum: Some(format!("sha256:{:x}", Sha256::digest(&archive))),
                signature_status: domain::ExtensionSignatureStatus::Verified,
                signature_algorithm: Some("ed25519".into()),
                signing_key_id: Some("fixture".into()),
                declared_warnings: Vec::new(),
                risk_override: None,
                confirmation_receipt: None,
                application_action: domain::ExtensionApplicationAction::ImportAgentFlow,
            })
            .await
            .unwrap();
    let ExtensionArtifactInstallOutcome::Installed { installation, .. } = outcome else {
        panic!("fixture must install");
    };

    let preview_path = format!(
        "/api/console/applications/archive/installed-extension/{}/preview",
        installation.id
    );
    let preview = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(&preview_path)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(preview.status(), StatusCode::OK);
    let preview: Value =
        serde_json::from_slice(&to_bytes(preview.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(preview["data"]["application_status"], "not_applied");
    assert_eq!(
        preview["data"]["preview"]["application"]["name"],
        "Installed extension source"
    );

    let import = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri(format!(
                    "/api/console/applications/archive/installed-extension/{}/import",
                    installation.id
                ))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "name": "Imported from extension" }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(import.status(), StatusCode::CREATED);

    let applied = app
        .oneshot(
            Request::builder()
                .uri(&preview_path)
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    let applied: Value =
        serde_json::from_slice(&to_bytes(applied.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(applied["data"]["application_status"], "applied");
}

#[tokio::test]
async fn ac_004_workflow_schedule_json_import_preserves_disabled_trigger_config() {
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
                        "application_type": "workflow",
                        "workflow_trigger_type": "schedule",
                        "workflow_trigger_config": {
                            "cron": "15 6 * * *",
                            "timezone": "Asia/Shanghai",
                            "input_payload": { "kind": "morning" }
                        },
                        "name": "Portable Schedule",
                        "description": "schedule import fixture"
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
    let source_id = created["data"]["id"].as_str().unwrap();
    let export = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "application_ids": [source_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::OK);
    let archive = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let boundary = "import-workflow-archive";
    let imported = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(application_archive_multipart(
                    boundary,
                    &archive,
                    Some("Imported Schedule"),
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), StatusCode::CREATED);
    let imported_body: Value =
        serde_json::from_slice(&to_bytes(imported.into_body(), usize::MAX).await.unwrap()).unwrap();
    let imported_id = imported_body["data"]["application"]["id"].as_str().unwrap();
    assert_eq!(
        imported_body["data"]["application"]["application_type"],
        json!("workflow")
    );
    let schedule = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{imported_id}/workflow-schedule-trigger"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(schedule.status(), StatusCode::OK);
    let schedule_body: Value =
        serde_json::from_slice(&to_bytes(schedule.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(schedule_body["data"]["enabled"], json!(false));
    assert_eq!(schedule_body["data"]["cron"], json!("15 6 * * *"));
    assert_eq!(schedule_body["data"]["timezone"], json!("Asia/Shanghai"));
    assert_eq!(
        schedule_body["data"]["input_payload"],
        json!({ "kind": "morning" })
    );
}

#[tokio::test]
async fn ac_004_workflow_extension_json_round_trip_preserves_registration_config() {
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
                        "application_type": "workflow",
                        "workflow_trigger_type": "extension",
                        "workflow_trigger_config": {
                            "subpath": "portable-extension",
                            "http_method": "POST",
                            "response_mode": "async"
                        },
                        "name": "Portable Extension",
                        "description": "extension import fixture"
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
    let source_id = created["data"]["id"].as_str().unwrap();
    let export = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "application_ids": [source_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(export.status(), StatusCode::OK);
    let archive = to_bytes(export.into_body(), usize::MAX).await.unwrap();
    let delete = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/applications/{source_id}"))
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete.status(), StatusCode::NO_CONTENT);

    let boundary = "import-workflow-extension-archive";
    let imported = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/import")
                .header("cookie", &cookie)
                .header("x-csrf-token", &csrf)
                .header(
                    "content-type",
                    format!("multipart/form-data; boundary={boundary}"),
                )
                .body(Body::from(application_archive_multipart(
                    boundary,
                    &archive,
                    Some("Imported Extension"),
                )))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(imported.status(), StatusCode::CREATED);
    let imported_body: Value =
        serde_json::from_slice(&to_bytes(imported.into_body(), usize::MAX).await.unwrap()).unwrap();
    let imported_id = imported_body["data"]["application"]["id"].as_str().unwrap();
    let mapping = app
        .clone()
        .oneshot(
            Request::builder()
                .uri(format!(
                    "/api/console/applications/{imported_id}/api-mapping"
                ))
                .header("cookie", &cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(mapping.status(), StatusCode::OK);
    let mapping_body: Value =
        serde_json::from_slice(&to_bytes(mapping.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(
        mapping_body["data"]["extension"],
        json!({
            "slug": "portable-extension",
            "method": "POST",
            "response_mode": "async"
        })
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
