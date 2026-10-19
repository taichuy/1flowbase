use super::*;

#[tokio::test]
async fn activation_permissions_are_shared_by_generic_and_extension_center_routes() {
    let (initial, _) = test_api_state_with_database_url().await;
    let mut owned = (*initial).clone();
    let runtime = RuntimeFixture::new(&owned);
    owned.store = runtime.store.clone();
    owned.provider_runtime = runtime.services.clone();
    let state = Arc::new(owned);
    let app = crate::app_with_state(state.clone());
    let (cookie, csrf) = login_and_capture_cookie(&app, "root", "change-me").await;
    let actor_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(runtime.store.pool())
        .await
        .unwrap();
    let actor = AuthRepository::load_actor_context_for_user(&runtime.store, actor_id)
        .await
        .unwrap();
    assert_ne!(actor.current_workspace_id, domain::SYSTEM_SCOPE_ID);
    let management = PluginManagementService::new(
        runtime.store.clone(),
        crate::provider_runtime::ApiProviderRuntime::new(runtime.services.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let mut declaration = manifest("system");
    let module = "acme.composition-system";
    declaration["binding_targets"] = json!(["system"]);
    declaration["runtime"]["protocol"] = "stdio_json_multiplex_v1".into();
    declaration["managed"]["execution_bindings"][0]["runtime"]["protocol"] =
        "stdio_json_multiplex_v1".into();
    declaration["managed"]["module"]["contributions"] = json!([{
        "contribution_id":"acme.composition-system.list", "contributor_module_id":module,
        "point_id":"1flowbase.managed-service.operation", "contract_version":"1",
        "required_permissions":["service.execute"], "mode":"append"
    }]);
    declaration["managed"]["execution_bindings"][0]["contribution_id"] =
        "acme.composition-system.list".into();
    declaration["managed_service"] = json!({
        "scope":"system",
        "feature":{"feature_id":"acme.composition-system.settings","label":"Fixture","description":"System governance fixture","route_id":"system-fixture","path":"/settings/system-fixture"},
        "operations":[{"interface_id":"acme.composition-system.list","contribution_id":"acme.composition-system.list","method":"GET",
            "path":"/api/console/managed-services/acme.composition-system/items","summary":"List items","description":"List system fixture items.",
            "input_schema":{"type":"object"},"output_schema":{"type":"object"}}]
    });
    let id = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "system-governance.1flowbasepkg".into(),
            package_bytes: package(&declaration),
        })
        .await
        .unwrap()
        .installation
        .id;
    let base = format!("/api/console/settings/extension-center/installed/{id}");
    let authority_url = format!("{base}/contribution-authorizations");
    let role = "activation_operator";
    create_role(&app, &cookie, &csrf, role).await;
    let member = create_member(&app, &cookie, &csrf, "activation_operator", "temp-pass").await;
    replace_member_roles(&app, &cookie, &csrf, &member, &[role]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "activation_operator", "temp-pass").await;
    let (status, _) = request(
        &app,
        &member_cookie,
        &member_csrf,
        "POST",
        &format!("{base}/enable"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    let ops = [
        "extension_center.installed.enable",
        "extension_center.installed.disable",
    ];
    let (status, policy) = request(&app, &cookie, &csrf, "PUT", &format!("/api/console/settings/roles/{role}/console-policy"), json!({"groups":[
        {"kind":"settings_feature","group_id":"system.extension-center","enabled":true,"strategy":"custom",
         "operations":ops.into_iter().map(|op| json!({"kind":"simple","operation_id":op,"enabled":true})).collect::<Vec<_>>()},
        {"kind":"other","group_id":"other.plugins","enabled":true,"strategy":"custom",
         "operations":[{"kind":"simple","operation_id":"plugins.families.switch","enabled":true},
                        {"kind":"simple","operation_id":"plugins.enable","enabled":true}]}
    ]})).await;
    assert_eq!(status, StatusCode::NO_CONTENT, "{policy}");
    let authority = PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id(&state.api_node_id);
    for url in [
        format!("{base}/enable"),
        format!("/api/console/plugins/{id}/enable"),
    ] {
        let (status, rejected) = request(
            &app,
            &member_cookie,
            &member_csrf,
            "POST",
            &url,
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::FORBIDDEN, "{url}: {rejected}");
        assert_eq!(
            rejected["code"],
            "managed_contribution_authorization_required"
        );
    }
    assert!(authority
        .query(&actor, id)
        .await
        .unwrap()
        .authorizations
        .is_empty());
    assert_eq!(
        runtime
            .store
            .get_installation(id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        domain::PluginDesiredState::Disabled
    );
    let (status, enabled) = request(
        &app,
        &cookie,
        &csrf,
        "POST",
        &format!("/api/console/plugins/{id}/enable"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{enabled}");
    let approved = authority.query(&actor, id).await.unwrap();
    assert_eq!(approved.authorizations.len(), 1);
    assert_eq!(
        request(
            &app,
            &member_cookie,
            &member_csrf,
            "GET",
            &authority_url,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    for action in ["disable", "enable"] {
        let (status, result) = request(
            &app,
            &member_cookie,
            &member_csrf,
            "POST",
            &format!("{base}/{action}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{result}");
    }
    assert_eq!(authority.query(&actor, id).await.unwrap(), approved);
    declaration["version"] = json!("1.1.0");
    declaration["managed"]["module"]["module_version"] = json!("1.1.0");
    let target = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: actor_id,
            file_name: "system-governance-new.1flowbasepkg".into(),
            package_bytes: package(&declaration),
        })
        .await
        .unwrap()
        .installation
        .id;
    let switch_url = "/api/console/plugins/families/acme.composition-system/switch-version";
    let (status, rejected) = request(
        &app,
        &member_cookie,
        &member_csrf,
        "POST",
        switch_url,
        json!({"installation_id":target}),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{rejected}");
    assert_eq!(
        rejected["code"],
        "managed_contribution_authorization_required"
    );
    assert!(authority
        .query(&actor, target)
        .await
        .unwrap()
        .authorizations
        .is_empty());
    let (status, switched) = request(
        &app,
        &cookie,
        &csrf,
        "POST",
        switch_url,
        json!({"installation_id":target}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{switched}");
    let target_approved = authority.query(&actor, target).await.unwrap();
    let (status, switched_back) = request(
        &app,
        &member_cookie,
        &member_csrf,
        "POST",
        switch_url,
        json!({"installation_id":id}),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{switched_back}");
    assert_eq!(
        authority.query(&actor, target).await.unwrap(),
        target_approved
    );
    let revoked = authority
        .revoke(
            &actor,
            id,
            RevokeContributionPermission {
                authorization_id: approved.authorizations[0].id,
                expected_revision: approved.revision,
            },
        )
        .await
        .unwrap();
    let (status, rejected) = request(
        &app,
        &member_cookie,
        &member_csrf,
        "POST",
        &format!("{base}/enable"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN, "{rejected}");
    assert_eq!(
        rejected["code"],
        "managed_contribution_authorization_required"
    );
    assert_eq!(authority.query(&actor, id).await.unwrap(), revoked);
    let (status, enabled) = request(
        &app,
        &cookie,
        &csrf,
        "POST",
        &format!("{base}/enable"),
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{enabled}");
    assert!(authority.query(&actor, id).await.unwrap().revision > revoked.revision);
    runtime.host.stop().await.unwrap();
}
