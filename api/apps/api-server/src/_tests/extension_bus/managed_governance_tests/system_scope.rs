use super::*;

#[tokio::test]
async fn system_governance_uses_installation_scope_and_existing_operation_authorization() {
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
    .with_node_id(&state.api_node_id);
    let mut declaration = manifest("system");
    let module = "acme.composition-system";
    declaration["binding_targets"] = json!(["system"]);
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
    PluginContributionAuthorityService::new(
        runtime.store.clone(),
        HostContributionGrantPolicy::root_composition(),
    )
    .with_node_id(&state.api_node_id)
    .grant(
        &actor,
        id,
        GrantContributionPermission {
            contribution_id: "acme.composition-system.list".into(),
            permission: "service.execute".into(),
            resource_scope: domain::ContributionResourceScope::System,
            permission_contract_id: "managed-service".into(),
            permission_contract_version: "1".into(),
        },
    )
    .await
    .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor_id,
            installation_id: id,
        })
        .await
        .unwrap();
    let base = format!("/api/console/settings/extension-center/installed/{id}");
    let view = format!("{base}/managed-execution");
    let (status, body) = request(&app, &cookie, &csrf, "GET", &view, Value::Null).await;
    assert_eq!(status, StatusCode::OK, "{body}");
    assert_eq!(
        body["data"]["workspace_id"],
        domain::SYSTEM_SCOPE_ID.to_string()
    );
    for (scope, expected) in [
        (domain::SYSTEM_SCOPE_ID, StatusCode::OK),
        (actor.current_workspace_id, StatusCode::BAD_REQUEST),
    ] {
        let cursor = ManagedDeliveryCursor {
            installation_id: id,
            workspace_id: scope,
            event_id: Uuid::now_v7(),
            subscriber_id: "cursor-probe".into(),
        }
        .encode();
        let (status, result) = request(
            &app,
            &cookie,
            &csrf,
            "GET",
            &format!("{view}?cursor={cursor}"),
            Value::Null,
        )
        .await;
        assert_eq!(status, expected, "{result}");
    }
    let target = body["data"]["executions"][0]["target"].clone();
    assert!(target.is_object(), "system execution must be visible");
    let role = "system_governance_role";
    create_role(&app, &cookie, &csrf, role).await;
    let member = create_member(
        &app,
        &cookie,
        &csrf,
        "system-governance-member",
        "temp-pass",
    )
    .await;
    replace_member_roles(&app, &cookie, &csrf, &member, &[role]).await;
    let (member_cookie, member_csrf) =
        login_and_capture_cookie(&app, "system-governance-member", "temp-pass").await;
    assert_eq!(
        request(
            &app,
            &member_cookie,
            &member_csrf,
            "GET",
            &view,
            Value::Null
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    allow(
        &app,
        &cookie,
        &csrf,
        role,
        "extension_center.managed_execution.view",
    )
    .await;
    let (status, member_body) = request(
        &app,
        &member_cookie,
        &member_csrf,
        "GET",
        &view,
        Value::Null,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "{member_body}");
    assert_eq!(
        member_body["data"]["workspace_id"],
        domain::SYSTEM_SCOPE_ID.to_string()
    );
    // Resume resolves the same system history; a missing delivery is a target conflict, not scope denial.
    let (status, missing) = request(
        &app,
        &cookie,
        &csrf,
        "POST",
        &format!("{base}/lifecycle-deliveries/resume"),
        json!({"event_id":Uuid::now_v7(),"subscriber_id":"missing","expected":target}),
    )
    .await;
    assert_eq!(status, StatusCode::CONFLICT, "{missing}");
    assert_eq!(missing["code"], "managed_paused_target_missing");
    management
        .disable_plugin(DisablePluginCommand {
            actor_user_id: actor_id,
            installation_id: id,
        })
        .await
        .unwrap();
    let retire = format!("{base}/managed-executions/retire");
    assert_eq!(
        request(
            &app,
            &member_cookie,
            &member_csrf,
            "POST",
            &retire,
            target.clone()
        )
        .await
        .0,
        StatusCode::FORBIDDEN
    );
    allow(
        &app,
        &cookie,
        &csrf,
        role,
        "extension_center.managed_executions.retire",
    )
    .await;
    let (status, retired) =
        request(&app, &member_cookie, &member_csrf, "POST", &retire, target).await;
    assert_eq!(status, StatusCode::OK, "{retired}");
    assert_eq!(
        retired["data"]["workspace_id"],
        domain::SYSTEM_SCOPE_ID.to_string()
    );
    assert_eq!(retired["data"]["executions"], json!([]));
    runtime.host.stop().await.unwrap();
}
