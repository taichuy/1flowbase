use super::*;

#[tokio::test]
async fn mcp_instance_discovery_policy_updates_validate_and_isolate_list_behavior() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    for (instance_id, name) in [
        ("workspace_ops", "Workspace Ops"),
        ("workspace_data", "Workspace Data"),
    ] {
        let create_instance_response = app
            .clone()
            .oneshot(
                Request::builder()
                    .method("POST")
                    .uri("/api/console/mcp/instances")
                    .header("cookie", &root_cookie)
                    .header("x-csrf-token", &root_csrf)
                    .header("content-type", "application/json")
                    .body(Body::from(
                        json!({
                            "instance_id": instance_id,
                            "name": name,
                            "description_short": null,
                            "status": "enabled",
                            "default_entry_path": "/"
                        })
                        .to_string(),
                    ))
                    .unwrap(),
            )
            .await
            .unwrap();
        assert_eq!(create_instance_response.status(), StatusCode::CREATED);

        for (path, display_name, sort_order) in [
            ("/system", "System", 1),
            ("/system/runtime", "Runtime", 2),
            ("/ops", "Operations", 3),
        ] {
            let upsert_group_response = app
                .clone()
                .oneshot(
                    Request::builder()
                        .method("POST")
                        .uri(format!("/api/console/mcp/instances/{instance_id}/groups"))
                        .header("cookie", &root_cookie)
                        .header("x-csrf-token", &root_csrf)
                        .header("content-type", "application/json")
                        .body(Body::from(
                            json!({
                                "path": path,
                                "display_name": display_name,
                                "description_short": null,
                                "enabled": true,
                                "sort_order": sort_order
                            })
                            .to_string(),
                        ))
                        .unwrap(),
                )
                .await
                .unwrap();
            assert_eq!(upsert_group_response.status(), StatusCode::OK);
        }
    }

    let invalid_return_fields_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/instances/workspace_ops/discovery-policy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 1,
                        "list_max_depth": 1,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 16,
                        "list_return_fields": ["id", "secret"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(
        invalid_return_fields_response.status(),
        StatusCode::BAD_REQUEST
    );
    let invalid_return_fields_payload = response_json(invalid_return_fields_response).await;
    assert_eq!(
        invalid_return_fields_payload["code"].as_str(),
        Some("list_return_fields")
    );

    let update_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/instances/workspace_ops/discovery-policy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 1,
                        "list_max_depth": 1,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 16,
                        "list_return_fields": ["id", "name"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_response.status(), StatusCode::OK);
    let update_payload = response_json(update_response).await;
    assert_eq!(
        update_payload["data"]["instance_id"],
        json!("workspace_ops")
    );
    assert_eq!(update_payload["data"]["list_default_limit"], json!(1));
    assert!(update_payload["data"]
        .get("get_include_mapping_summary")
        .is_none());
    assert!(update_payload["data"]
        .get("call_default_des_id_policy")
        .is_none());

    let data_policy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/instances/workspace_data/discovery-policy")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(data_policy_response.status(), StatusCode::OK);
    let data_policy_payload = response_json(data_policy_response).await;
    assert_eq!(
        data_policy_payload["data"]["instance_id"],
        json!("workspace_data")
    );
    assert_eq!(data_policy_payload["data"]["list_default_limit"], json!(50));

    let ops_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_ops")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ops_list_response.status(), StatusCode::OK);
    let ops_list_payload = response_json(ops_list_response).await;
    let ops_items = ops_list_payload["data"].as_array().unwrap();
    assert_eq!(ops_items.len(), 1);
    assert!(ops_items[0].get("id").is_some());
    assert!(ops_items[0].get("name").is_some());
    assert!(ops_items[0].get("path").is_none());
    assert!(ops_items[0].get("children_count").is_none());

    let data_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_data")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(data_list_response.status(), StatusCode::OK);
    let data_list_payload = response_json(data_list_response).await;
    let data_items = data_list_payload["data"].as_array().unwrap();
    assert_eq!(data_items.len(), 3);
    assert!(data_items[0].get("path").is_some());
    assert!(data_items[0].get("children_count").is_some());
    let system_group = data_items
        .iter()
        .find(|item| item["path"] == json!("/system"))
        .unwrap();
    assert_eq!(system_group["children_count"], json!(1));
    let runtime_group = data_items
        .iter()
        .find(|item| item["path"] == json!("/system/runtime"))
        .unwrap();
    assert_eq!(runtime_group["children_count"], json!(0));

    let regex_list_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_ops&limit=10&path_regex=%5E%2Fsystem")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(regex_list_response.status(), StatusCode::OK);
    let regex_list_payload = response_json(regex_list_response).await;
    let regex_items = regex_list_payload["data"].as_array().unwrap();
    assert_eq!(regex_items.len(), 1);
    assert_eq!(regex_items[0]["name"].as_str(), Some("System"));

    let long_regex_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/list?instance_id=workspace_ops&path_regex=%5E%2Fsystem%2Fruntime-long")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(long_regex_response.status(), StatusCode::BAD_REQUEST);
    let long_regex_payload = response_json(long_regex_response).await;
    assert_eq!(long_regex_payload["code"].as_str(), Some("path_regex"));
}

#[tokio::test]
async fn mcp_instance_copy_creates_a_draft_with_groups_and_discovery_policy_atomically() {
    let app = test_app().await;
    let (root_cookie, root_csrf) = login_and_capture_cookie(&app, "root", "change-me").await;

    let create_source = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "source_ops",
                        "name": "Source Operations",
                        "description_short": "Copy this configuration",
                        "status": "enabled",
                        "default_entry_path": "/ops"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_source.status(), StatusCode::CREATED);

    let create_group = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/source_ops/groups")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "path": "/ops",
                        "display_name": "Operations",
                        "description_short": "Operational tools",
                        "enabled": true,
                        "sort_order": 7
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(create_group.status(), StatusCode::OK);

    let update_policy = app
        .clone()
        .oneshot(
            Request::builder()
                .method("PUT")
                .uri("/api/console/mcp/instances/source_ops/discovery-policy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "list_default_limit": 17,
                        "list_max_depth": 5,
                        "list_regex_enabled": true,
                        "list_regex_max_length": 64,
                        "list_return_fields": ["id", "name", "path"]
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(update_policy.status(), StatusCode::OK);

    let copy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/source_ops/copy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "copied_ops",
                        "name": "Copied Operations"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(copy_response.status(), StatusCode::CREATED);
    let copied_payload = response_json(copy_response).await;
    assert_eq!(copied_payload["data"]["instance_id"], "copied_ops");
    assert_eq!(copied_payload["data"]["name"], "Copied Operations");
    assert_eq!(copied_payload["data"]["status"], "draft");
    assert_eq!(
        copied_payload["data"]["description_short"],
        "Copy this configuration"
    );
    assert_eq!(copied_payload["data"]["default_entry_path"], "/ops");

    let duplicate_copy_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("POST")
                .uri("/api/console/mcp/instances/source_ops/copy")
                .header("cookie", &root_cookie)
                .header("x-csrf-token", &root_csrf)
                .header("content-type", "application/json")
                .body(Body::from(
                    json!({
                        "instance_id": "copied_ops",
                        "name": "Duplicate Copy"
                    })
                    .to_string(),
                ))
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(duplicate_copy_response.status(), StatusCode::CONFLICT);

    let catalog_response = app
        .clone()
        .oneshot(
            Request::builder()
                .method("GET")
                .uri("/api/console/mcp/catalog")
                .header("cookie", &root_cookie)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(catalog_response.status(), StatusCode::OK);
    let catalog = response_json(catalog_response).await;
    let copied_instance = catalog["data"]["instances"]
        .as_array()
        .unwrap()
        .iter()
        .find(|instance| instance["instance_id"] == "copied_ops")
        .unwrap();
    let copied_record_id = copied_instance["id"].as_str().unwrap();
    assert!(catalog["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .any(|group| group["instance_record_id"] == copied_record_id && group["path"] == "/ops"));
    let copied_policy = catalog["data"]["discovery_policies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|policy| policy["instance_record_id"] == copied_record_id)
        .unwrap();
    assert_eq!(copied_policy["list_default_limit"], 17);
    assert_eq!(copied_policy["list_max_depth"], 5);
    assert_eq!(copied_policy["list_regex_enabled"], true);
}
