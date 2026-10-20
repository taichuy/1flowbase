use super::*;

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
    assert_eq!(
        exported_package["applications"][0]["template_id"],
        json!(source_id)
    );
    assert_eq!(
        exported_package["applications"][0]["release_version"],
        json!(1)
    );
    assert_eq!(
        exported_package["applications"][0]["exported_from_system_version"],
        json!(env!("CARGO_PKG_VERSION"))
    );
    assert!(exported_package["applications"][0]["exported_at"].is_string());

    let repeated_export = app
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
    let repeated_package: Value = serde_json::from_slice(
        &to_bytes(repeated_export.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        repeated_package["applications"][0]["release_version"],
        json!(1)
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
        preview_body["data"]["applications"][0]["preview"]["application"]["name"],
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
    assert_ne!(
        imported_body["data"]["results"][0]["result"]["application"]["id"],
        source_id
    );
    let imported_id = imported_body["data"]["results"][0]["result"]["application"]["id"]
        .as_str()
        .unwrap()
        .to_string();
    assert_eq!(
        imported_body["data"]["results"][0]["result"]["application"]["name"],
        json!("Imported Portable Agent")
    );
    assert_eq!(
        imported_body["data"]["results"][0]["result"]["application"]["application_type"],
        json!("agent_flow")
    );

    let imported_export = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "application_ids": [&imported_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let imported_package: Value = serde_json::from_slice(
        &to_bytes(imported_export.into_body(), usize::MAX)
            .await
            .unwrap(),
    )
    .unwrap();
    assert_eq!(
        imported_package["applications"][0]["template_id"],
        json!(imported_id)
    );
    assert_ne!(
        imported_package["applications"][0]["template_id"],
        exported_package["applications"][0]["template_id"]
    );
    assert_eq!(
        imported_package["applications"][0]["release_version"],
        json!(1)
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
        legacy_import_body["data"]["results"][0]["result"]["application"]["name"],
        json!("Imported Legacy Agent")
    );
}

#[tokio::test]
async fn ac_001_failed_batch_export_does_not_partially_increment_release_versions() {
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
                        "name": "Atomic archive batch",
                        "description": "batch rollback fixture"
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

    let failed = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "application_ids": [
                            application_id,
                            "ffffffff-ffff-ffff-ffff-ffffffffffff"
                        ]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(failed.status(), StatusCode::NOT_FOUND);

    let export = app
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/applications/archive/export")
                .header("cookie", &cookie)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "application_ids": [application_id] }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    let package: Value =
        serde_json::from_slice(&to_bytes(export.into_body(), usize::MAX).await.unwrap()).unwrap();
    assert_eq!(package["applications"][0]["release_version"], json!(1));
}

#[tokio::test]
async fn delivery_1545_d6_installed_agent_flow_previews_imports_and_reports_workspace_application()
{
    let (state, _) = test_api_state_with_database_url().await;
    let app = crate::app_with_state_and_config(state.clone(), &test_config());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let actor = AuthRepository::find_user_for_password_login(
        &state.store,
        domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID,
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
                expected_checksum: Some(format!(
                    "sha256:{:x}",
                    base16ct::HexDisplay(&Sha256::digest(&archive))
                )),
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
    let preview_status = preview.status();
    let preview_body = to_bytes(preview.into_body(), usize::MAX).await.unwrap();
    assert_eq!(
        preview_status,
        StatusCode::OK,
        "installed extension preview failed: {}",
        String::from_utf8_lossy(&preview_body)
    );
    let preview: Value = serde_json::from_slice(&preview_body).unwrap();
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
    let imported_id = imported_body["data"]["results"][0]["result"]["application"]["id"]
        .as_str()
        .unwrap();
    assert_eq!(
        imported_body["data"]["results"][0]["result"]["application"]["application_type"],
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
    let imported_id = imported_body["data"]["results"][0]["result"]["application"]["id"]
        .as_str()
        .unwrap();
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
