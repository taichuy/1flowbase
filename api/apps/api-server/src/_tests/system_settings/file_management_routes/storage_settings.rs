use super::*;

#[tokio::test]
async fn file_management_settings_routes_enforce_root_only_storage_and_binding_rules() {
    let (app, database_url) = test_app_with_database_url().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    register_files_feature_permission(&database_url).await;
    create_role(&app, &root_cookie, &root_csrf, "file_admin").await;
    grant_file_operations(
        &app,
        &root_cookie,
        &root_csrf,
        "file_admin",
        &[
            "file_tables.create",
            "file_tables.list",
            "file_tables.delete",
            "file_tables.storage.bind",
            "file_storages.create",
            "file_storages.update",
            "file_storages.delete",
        ],
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

#[tokio::test]
async fn customer_sessions_manage_workspace_tables_but_cannot_access_host_storage() {
    use control_plane_contracts::ports::{
        FileManagementRepository, GrantWorkspaceMembershipInput, ProvisionWorkspaceInput,
        ProvisionWorkspaceTenant, UpdateFileStorageBindingInput, WorkspaceGovernanceRepository,
    };

    async fn request(
        app: &axum::Router,
        (cookie, csrf): (&str, &str),
        method: &str,
        path: &str,
        body: Value,
    ) -> (StatusCode, Value) {
        let response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method(method)
                    .uri(path)
                    .header("cookie", cookie)
                    .header("x-csrf-token", csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(body.to_string()))
                    .unwrap(),
            )
            .await
            .unwrap();
        (response.status(), response_json(response).await)
    }

    const TABLES: &str = "/api/console/settings/files/tables";
    const STORAGES: &str = "/api/console/settings/files/storages";
    let (state, _) = test_api_state_with_database_url().await;
    let store = state.store.clone();
    let app = crate::app_with_state_and_config(state, &test_config());
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let root = (root_cookie.as_str(), root_csrf.as_str());
    let (status, me) = request(&app, root, "GET", "/api/console/me", Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{me}");
    assert!(
        me["data"]["permissions"]
            .as_array()
            .unwrap()
            .contains(&json!("file_storages.list"))
    );
    let (status, storages) = request(&app, root, "GET", STORAGES, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{storages}");
    let storage = storages["data"]
        .as_array()
        .unwrap()
        .iter()
        .find(|storage| storage["is_default"] == true)
        .unwrap();
    let storage_id = storage["id"].as_str().unwrap();
    let storage_path = format!("{STORAGES}/{storage_id}");
    let (status, bootstrap_table) = request(
        &app,
        root,
        "POST",
        TABLES,
        json!({"code":"bootstrap_docs", "title":"Bootstrap Docs"}),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{bootstrap_table}");

    let alice = create_member(&app, &root_cookie, &root_csrf, "alice", "change-me").await;
    let acme = store
        .provision_workspace(&ProvisionWorkspaceInput {
            tenant: ProvisionWorkspaceTenant::New {
                code: "acme".into(),
                name: "Acme".into(),
            },
            workspace_name: "Acme".into(),
            logo_url: None,
            introduction: String::new(),
            actor_user_id: None,
            role_templates: access_control::bootstrap_role_templates(),
            initial_member: None,
        })
        .await
        .unwrap()
        .workspace
        .id;
    store
        .grant_or_activate_workspace_membership(&GrantWorkspaceMembershipInput {
            workspace_id: acme,
            user_id: alice.parse().unwrap(),
            role_codes: vec!["admin".into()],
            actor_user_id: None,
        })
        .await
        .unwrap();
    let (alice_cookie, alice_csrf) = login_and_capture_cookie(&app, "alice", "change-me").await;
    let alice = (alice_cookie.as_str(), alice_csrf.as_str());

    let mut acme_table_id = None;
    for (account, (cookie, csrf)) in [("alice", alice), ("root", root)] {
        let (status, switched) = request(
            &app,
            (cookie, csrf),
            "POST",
            "/api/console/session/actions/switch-workspace",
            json!({"workspace_id":acme}),
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{account}: {switched}");
        let acme_session = (cookie, switched["data"]["csrf_token"].as_str().unwrap());
        if account == "alice" {
            let (status, created) = request(
                &app,
                acme_session,
                "POST",
                TABLES,
                json!({"code":"acme_docs", "title":"Acme Docs"}),
            )
            .await;
            assert_eq!(status, StatusCode::CREATED, "{created}");
            assert_eq!(created["data"]["scope_id"], acme.to_string());
            let id = created["data"]["id"].as_str().unwrap();
            // An existing authorized binding remains readable via its table, not via storage settings.
            store
                .update_file_table_binding(&UpdateFileStorageBindingInput {
                    actor_user_id: me["data"]["id"].as_str().unwrap().parse().unwrap(),
                    file_table_id: id.parse().unwrap(),
                    bound_storage_id: Some(storage_id.parse().unwrap()),
                })
                .await
                .unwrap();
            acme_table_id = Some(id.to_owned());
        }
        let acme_table_id = acme_table_id.as_deref().unwrap();
        let (status, listed) = request(&app, acme_session, "GET", TABLES, Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{account}: {listed}");
        let rows = listed["data"].as_array().unwrap();
        let acme_table = rows.iter().find(|row| row["id"] == acme_table_id).unwrap();
        assert_eq!(acme_table["bound_storage_title"], storage["title"]);
        assert!(
            rows.iter()
                .all(|row| row["id"] != bootstrap_table["data"]["id"])
        );
        assert!(
            rows.iter().all(|row| {
                row["scope_kind"] == "system" || row["scope_id"] == acme.to_string()
            })
        );
        let attachments = rows
            .iter()
            .find(|row| row["code"] == "attachments")
            .unwrap();
        assert_eq!(attachments["bound_storage_title"], storage["title"]);

        let binding_path = format!("{TABLES}/{acme_table_id}/binding");
        for (method, path, body) in [
            ("GET", STORAGES, Value::Null),
            (
                "POST",
                STORAGES,
                json!({"code":"denied_storage", "title":"Denied", "driver_type":"local", "enabled":true, "is_default":false, "config_json":storage["config_json"], "rule_json":{}}),
            ),
            (
                "PUT",
                storage_path.as_str(),
                json!({"title":"Denied update", "enabled":true, "is_default":true, "config_json":storage["config_json"], "rule_json":{}}),
            ),
            ("DELETE", storage_path.as_str(), Value::Null),
            (
                "PUT",
                binding_path.as_str(),
                json!({"bound_storage_id":storage_id}),
            ),
            (
                "POST",
                TABLES,
                json!({"code":"denied_binding", "title":"Denied", "bound_storage_id":storage_id}),
            ),
        ] {
            let (status, error) = request(&app, acme_session, method, path, body).await;
            assert_eq!(
                status,
                StatusCode::FORBIDDEN,
                "{account} {method} {path}: {error}"
            );
            assert_eq!(
                error["code"], "permission_denied",
                "{account} {method} {path}"
            );
        }
        let (status, me) = request(&app, acme_session, "GET", "/api/console/me", Value::Null).await;
        assert_eq!(status, StatusCode::OK, "{me}");
        let permissions = me["data"]["permissions"].as_array().unwrap();
        if account == "alice" {
            assert!(permissions.contains(&json!("file_tables.list")), "{me}");
        }
        assert!(!permissions.contains(&json!("file_storages.list")));
    }
}
