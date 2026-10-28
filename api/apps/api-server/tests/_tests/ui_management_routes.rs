use super::*;

const ROLE: &str = "ui_menu_limited";
const GROUP: &str = "system.ui-management";

async fn ui_policy(
    app: &Router,
    cookie: &str,
    csrf: &str,
    enabled: bool,
    strategy: &str,
    operations: Value,
) {
    let (status, payload) = json_request(app, "PUT", &format!("/api/console/settings/roles/{ROLE}/console-policy"), cookie, csrf, json!({
        "groups": [{ "kind": "settings_feature", "group_id": GROUP, "enabled": enabled, "strategy": strategy, "operations": operations }]
    })).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{payload}");
}

async fn design_permission(app: &Router, cookie: &str) -> bool {
    let (status, payload) = get(app, "/api/console/me", cookie).await;
    assert_eq!(status, StatusCode::OK, "{payload}");
    payload["data"]["permissions"]
        .as_array()
        .unwrap()
        .contains(&json!("frontstage.page.design"))
}

#[tokio::test]
async fn ui_management_controls_menu_operations_and_design_visibility() {
    let fixture = fixture().await;
    let app = &fixture.app;
    let (root, root_csrf) = login(app, "root", "change-me").await;
    create_member(app, &root, &root_csrf, "ui-menu-member", "temp-pass").await;
    let (cookie, _) = login(app, "ui-menu-member", "temp-pass").await;
    let (user_id, workspace_id) = session_identity(app, &cookie).await;
    let (status, payload) = json_request(
        app,
        "POST",
        "/api/console/settings/roles",
        &root,
        &root_csrf,
        json!({ "code": ROLE, "name": ROLE, "introduction": "UI configuration test" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    assert_eq!(
        json_request(
            app,
            "PUT",
            &format!("/api/console/settings/members/{user_id}/roles"),
            &root,
            &root_csrf,
            json!({ "role_codes": [ROLE] })
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    // Existing credentials cannot retain the previously selected role after rebinding.
    let (cookie, csrf) = login(app, "ui-menu-member", "temp-pass").await;
    assert_eq!(
        json_request(
            app,
            "PUT",
            &format!("/api/console/settings/roles/{ROLE}/permissions"),
            &root,
            &root_csrf,
            json!({ "permission_codes": ["frontstage.page.design"] })
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(!design_permission(app, &cookie).await);
    let groups_path = "/api/console/frontstage/pages/groups";
    let pages_path = "/api/console/frontstage/pages";
    assert_eq!(
        json_request(
            app,
            "POST",
            groups_path,
            &cookie,
            &csrf,
            json!({ "title": "Denied" })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );

    // Root can configure UI without any role policy; it supplies a page for the denied-operation checks.
    let (page_id, tab_id, _) = create_page(app, &root, &root_csrf, workspace_id).await;
    let page_path = format!("{pages_path}/{page_id}");
    let tab_path = format!("{page_path}/tabs/{tab_id}");
    let operations =
        json!([{ "kind": "simple", "operation_id": "frontstage.groups.create", "enabled": true }]);
    ui_policy(app, &root, &root_csrf, true, "custom", operations.clone()).await;
    // No legacy permission is needed once the UI-management operation is granted.
    assert_eq!(
        json_request(
            app,
            "PUT",
            &format!("/api/console/settings/roles/{ROLE}/permissions"),
            &root,
            &root_csrf,
            json!({ "permission_codes": [] })
        )
        .await
        .0,
        StatusCode::NO_CONTENT
    );
    assert!(design_permission(app, &cookie).await);
    assert_eq!(
        json_request(
            app,
            "POST",
            groups_path,
            &cookie,
            &csrf,
            json!({ "title": "Allowed" })
        )
        .await
        .0,
        StatusCode::CREATED
    );
    for (method, path, body) in [
        (
            "POST",
            pages_path.to_string(),
            json!({ "title": "Denied page" }),
        ),
        (
            "PATCH",
            page_path.clone(),
            json!({ "title": "Denied rename" }),
        ),
        (
            "POST",
            format!("{page_path}/move"),
            json!({ "parent_id": null, "rank": "a" }),
        ),
        ("DELETE", page_path.clone(), json!({})),
        (
            "POST",
            format!("{page_path}/tabs"),
            json!({ "title": "Denied tab" }),
        ),
        (
            "PATCH",
            tab_path.clone(),
            json!({ "title": "Denied tab rename" }),
        ),
        ("DELETE", tab_path.clone(), json!({})),
        (
            "PUT",
            format!("{tab_path}/document"),
            json!({ "payload": { "version": 1 } }),
        ),
        // The action dispatcher must not bypass the document-save operation grant.
        (
            "POST",
            format!("{tab_path}/actions/dispatch"),
            json!({ "action_id": "frontstage.page_tab.document.save", "params": { "payload": { "version": 1 } } }),
        ),
    ] {
        let (status, payload) = json_request(app, method, &path, &cookie, &csrf, body).await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{method} {path}: {payload}");
    }

    ui_policy(app, &root, &root_csrf, false, "custom", operations).await;
    assert!(!design_permission(app, &cookie).await);
    assert_eq!(
        json_request(
            app,
            "POST",
            groups_path,
            &cookie,
            &csrf,
            json!({ "title": "Revoked" })
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    assert_eq!(get(app, pages_path, &cookie).await.0, StatusCode::OK);
    let (status, saved) = get(
        app,
        &format!("/api/console/settings/roles/{ROLE}/console-policy"),
        &root,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{saved}");
    let saved_group = saved["data"]["groups"]
        .as_array()
        .unwrap()
        .iter()
        .find(|group| group["group_id"] == GROUP)
        .expect("saved UI-management policy");
    assert_eq!(saved_group["enabled"], false);
    assert_eq!(
        saved_group["operations"][0]["operation_id"],
        "frontstage.groups.create"
    );

    ui_policy(app, &root, &root_csrf, true, "full", json!([])).await;
    assert!(design_permission(app, &cookie).await);
    let (status, payload) = json_request(
        app,
        "POST",
        pages_path,
        &cookie,
        &csrf,
        json!({ "title": "Full access" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    assert_eq!(payload["data"]["page"]["show_title"], false);
    let page_id = payload["data"]["page"]["id"].as_str().unwrap();
    let tab_id = payload["data"]["default_tab"]["id"].as_str().unwrap();
    let page_path = format!("{pages_path}/{page_id}");
    assert_eq!(
        json_request(
            app,
            "PATCH",
            &page_path,
            &cookie,
            &csrf,
            json!({ "title": "Updated", "content_presentation": "tabs", "show_title": true })
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        json_request(
            app,
            "POST",
            &format!("{page_path}/move"),
            &cookie,
            &csrf,
            json!({ "parent_id": null, "rank": "a" })
        )
        .await
        .0,
        StatusCode::OK
    );
    let (status, payload) = json_request(
        app,
        "POST",
        &format!("{page_path}/tabs"),
        &cookie,
        &csrf,
        json!({ "title": "Extra tab", "route_segment": "extra" }),
    )
    .await;
    assert_eq!(status, StatusCode::CREATED, "{payload}");
    let extra_tab_id = payload["data"]["id"].as_str().unwrap();
    let extra_tab_path = format!("{page_path}/tabs/{extra_tab_id}");
    assert_eq!(
        json_request(
            app,
            "PATCH",
            &extra_tab_path,
            &cookie,
            &csrf,
            json!({ "title": "Updated tab" })
        )
        .await
        .0,
        StatusCode::OK
    );
    let document_path = format!("{page_path}/tabs/{tab_id}/document");
    assert_eq!(
        json_request(
            app,
            "PUT",
            &document_path,
            &cookie,
            &csrf,
            json!({ "payload": { "version": 1 } })
        )
        .await
        .0,
        StatusCode::OK
    );
    assert_eq!(
        json_request(app, "DELETE", &extra_tab_path, &cookie, &csrf, json!({}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
    assert_eq!(
        json_request(app, "DELETE", &page_path, &cookie, &csrf, json!({}))
            .await
            .0,
        StatusCode::NO_CONTENT
    );
}

#[test]
fn ui_management_owns_individual_menu_configuration_operations() {
    let snapshot =
        api_server::app_state::compile_core_console_operation_inventory_snapshot().unwrap();
    for id in [
        "frontstage.groups.create",
        "frontstage.pages.create",
        "frontstage.pages.update",
        "frontstage.pages.move",
        "frontstage.pages.delete",
        "frontstage.tabs.create",
        "frontstage.tabs.update",
        "frontstage.tabs.delete",
        "frontstage.tabs.document.save",
    ] {
        let entry = snapshot
            .compiled_inventory
            .operations
            .iter()
            .find(|entry| entry.operation_id == id)
            .unwrap();
        assert_eq!(
            entry.policy_group,
            access_control::ConsolePolicyGroup::SettingsFeature(GROUP.into())
        );
        assert_eq!(
            entry.authorization,
            access_control::ConsoleAuthorization::Simple
        );
        assert_eq!(entry.routes.len(), 1);
    }
}
