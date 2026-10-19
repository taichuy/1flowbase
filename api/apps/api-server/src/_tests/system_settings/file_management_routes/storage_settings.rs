use super::*;

#[tokio::test]
async fn file_management_settings_routes_enforce_root_only_storage_and_binding_rules() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_files_feature_permission(&database_url).await;
    create_role(&app, &root_cookie, &root_csrf, "file_admin").await;
    replace_role_permissions(
        &app,
        &root_cookie,
        &root_csrf,
        "file_admin",
        &[FILES_FEATURE_PERMISSION],
    )
    .await;
    let member_id = create_member(&app, &root_cookie, &root_csrf, "file-admin", "change-me").await;
    replace_member_roles(&app, &root_cookie, &root_csrf, &member_id, &["file_admin"]).await;
    let (admin_cookie, admin_csrf) =
        login_and_capture_cookie(&app, "file-admin", "change-me").await;

    let create_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "workspace_docs",
                        "title": "Workspace Docs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_table_response.status(), StatusCode::CREATED);
    let table_payload = response_json(create_table_response).await;
    let file_table_id = table_payload["data"]["id"].as_str().unwrap().to_string();

    let root_storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(root_storages_response.status(), StatusCode::OK);
    let root_storages_payload = response_json(root_storages_response).await;
    let default_storage_title = root_storages_payload["data"][0]["title"]
        .as_str()
        .unwrap()
        .to_string();

    let list_tables_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_tables_response.status(), StatusCode::OK);
    let list_tables_payload = response_json(list_tables_response).await;
    assert_eq!(
        list_tables_payload["data"][0]["bound_storage_title"].as_str(),
        Some(default_storage_title.as_str())
    );

    let list_storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &admin_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_storages_response.status(), StatusCode::FORBIDDEN);

    let create_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "admin_local",
                        "title": "Admin Local",
                        "driver_type": "local",
                        "enabled": true,
                        "is_default": false,
                        "config_json": {
                            "root_path": std::env::temp_dir().join(format!("file-management-admin-{}", Uuid::now_v7())).display().to_string(),
                            "public_base_url": null
                        },
                        "rule_json": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_storage_response.status(), StatusCode::FORBIDDEN);

    let bind_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!(
                    "/api/console/settings/files/tables/{file_table_id}/binding"
                ))
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({ "bound_storage_id": Uuid::now_v7() }).to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(bind_response.status(), StatusCode::FORBIDDEN);

    let update_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/settings/files/storages/00000000-0000-0000-0000-000000000001")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .header("content-type", "application/json")
                .body(Body::from(json!({"title":"Admin Local Updated","enabled":true,"is_default":false,"config_json":{"root_path":"/tmp/admin-local"},"rule_json":{}}).to_string()))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_storage_response.status(), StatusCode::FORBIDDEN);

    let delete_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri("/api/console/settings/files/storages/00000000-0000-0000-0000-000000000001")
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_storage_response.status(), StatusCode::FORBIDDEN);

    let delete_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/files/tables/{file_table_id}"
                ))
                .header("cookie", &admin_cookie)
                .header("x-csrf-token", &admin_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_table_response.status(), StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn file_management_settings_routes_allow_root_to_update_and_delete_storage_and_delete_table()
{
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let storages_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storages_response.status(), StatusCode::OK);
    let storages_payload = response_json(storages_response).await;
    let default_storage_id = storages_payload["data"][0]["id"].as_str().unwrap();

    let create_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "root_cleanup_docs",
                        "title": "Root Cleanup Docs"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_table_response.status(), StatusCode::CREATED);
    let table_payload = response_json(create_table_response).await;
    let file_table_id = table_payload["data"]["id"].as_str().unwrap().to_string();

    let storage_root =
        std::env::temp_dir().join(format!("file-management-update-{}", Uuid::now_v7()));
    let create_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "code": "cleanup_local",
                        "title": "Cleanup Local",
                        "driver_type": "local",
                        "enabled": true,
                        "is_default": false,
                        "config_json": {
                            "root_path": storage_root.display().to_string()
                        },
                        "rule_json": {}
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_storage_response.status(), StatusCode::CREATED);
    let storage_payload = response_json(create_storage_response).await;
    let storage_id = storage_payload["data"]["id"].as_str().unwrap().to_string();

    let update_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri(format!("/api/console/settings/files/storages/{storage_id}"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "title": "Cleanup Archive",
                        "enabled": false,
                        "is_default": false,
                        "config_json": {
                            "root_path": storage_root.display().to_string(),
                            "public_base_url": "https://files.example.com"
                        },
                        "rule_json": {
                            "description": "archive"
                        }
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_storage_response.status(), StatusCode::OK);
    let updated_storage_payload = response_json(update_storage_response).await;
    assert_eq!(
        updated_storage_payload["data"]["title"].as_str(),
        Some("Cleanup Archive")
    );
    assert_eq!(
        updated_storage_payload["data"]["enabled"].as_bool(),
        Some(false)
    );
    assert_eq!(
        updated_storage_payload["data"]["config_json"]["public_base_url"].as_str(),
        Some("https://files.example.com")
    );

    let delete_table_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!(
                    "/api/console/settings/files/tables/{file_table_id}"
                ))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_table_response.status(), StatusCode::NO_CONTENT);

    let list_tables_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/tables")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(list_tables_response.status(), StatusCode::OK);
    let list_tables_payload = response_json(list_tables_response).await;
    let table_records = list_tables_payload["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(table_records
        .iter()
        .all(|record| record["id"].as_str() != Some(file_table_id.as_str())));

    let delete_storage_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("DELETE")
                .uri(format!("/api/console/settings/files/storages/{storage_id}"))
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(delete_storage_response.status(), StatusCode::NO_CONTENT);

    let storages_after_delete_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/settings/files/storages")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(storages_after_delete_response.status(), StatusCode::OK);
    let storages_after_delete_payload = response_json(storages_after_delete_response).await;
    let storage_records = storages_after_delete_payload["data"]
        .as_array()
        .cloned()
        .unwrap_or_default();
    assert!(storage_records
        .iter()
        .any(|record| record["id"].as_str() == Some(default_storage_id)));
    assert!(storage_records
        .iter()
        .all(|record| record["id"].as_str() != Some(storage_id.as_str())));

    let _ = std::fs::remove_dir_all(storage_root);
}
