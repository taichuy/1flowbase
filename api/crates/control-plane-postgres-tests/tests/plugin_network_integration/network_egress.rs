use super::*;

#[tokio::test]
async fn network_egress_installation_commit_keeps_one_current_artifact_and_rolls_back_failed_update(
) {
    let (store, _workspace, actor) = seed_store().await;
    let retained_id = Uuid::now_v7();
    let current_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(retained_id, actor.id, "0.2.2", "native_react"),
    )
    .await
    .unwrap();

    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "invalid"),
    )
    .await
    .unwrap_err();
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("retained artifact must remain")
            .is_current
    );
    assert!(PluginRepository::get_installation(&store, current_id)
        .await
        .unwrap()
        .is_none());

    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "native_react"),
    )
    .await
    .unwrap();

    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("retained artifact must remain")
            .is_current
    );
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", current_id)
            .await
            .unwrap()
            .expect("new artifact must be current")
            .is_current
    );
}

#[tokio::test]
async fn network_egress_current_selection_switches_one_provider_family_atomically() {
    let (store, _workspace, actor) = seed_store().await;
    let retained_id = Uuid::now_v7();
    let current_id = Uuid::now_v7();
    let other_publisher_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(retained_id, actor.id, "0.2.2", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input_for_organization(
            other_publisher_id,
            actor.id,
            "other-publisher",
            "0.3.0",
        ),
    )
    .await
    .unwrap();

    // AC-NCP03: selecting a retained version changes only the family current projection.
    let selected =
        PluginRepository::select_network_egress_current(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("ready retained version must be selectable");

    assert_eq!(selected.installation_id, retained_id);
    assert!(selected.is_current);
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("selected artifact must remain")
            .is_current
    );
    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", current_id)
            .await
            .unwrap()
            .expect("previous current artifact must remain")
            .is_current
    );
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", other_publisher_id)
            .await
            .unwrap()
            .expect("another publisher's same-named artifact must remain")
            .is_current,
        "current selection must be scoped by category, organization, and artifact_id"
    );
}

#[tokio::test]
async fn ac_001_network_egress_provider_persists_stable_family_identity_across_version_switches() {
    let (store, _workspace, actor) = seed_store().await;
    let old_installation_id = Uuid::now_v7();
    let current_installation_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(old_installation_id, actor.id, "0.2.5", "native_react"),
    )
    .await
    .unwrap();

    let provider_id = Uuid::now_v7();
    NetworkEgressRepository::create_network_egress_provider(
        &store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            extension_family: domain::ExtensionCatalogIdentity::new(
                domain::ExtensionCategory::RuntimeExtensions,
                "taichuy",
                "clash-proxy",
            ),
            provider_code: "clash-proxy".into(),
            display_name: "Clash subscription".into(),
            description: String::new(),
            secret_ref: format!("secret://system/network-egress/{provider_id}"),
            lifecycle: domain::NetworkEgressProviderLifecycle::Active,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_installation_id, actor.id, "0.2.8", "native_react"),
    )
    .await
    .unwrap();

    let row = sqlx::query(
        "select extension_category, extension_organization, extension_artifact_id \
         from network_egress_providers where id = $1",
    )
    .bind(provider_id)
    .fetch_one(store.pool())
    .await
    .expect("provider must persist a version-independent extension family");

    assert_eq!(
        row.get::<String, _>("extension_category"),
        "runtime-extensions"
    );
    assert_eq!(row.get::<String, _>("extension_organization"), "taichuy");
    assert_eq!(row.get::<String, _>("extension_artifact_id"), "clash-proxy");

    let provider = NetworkEgressRepository::get_network_egress_provider(&store, provider_id)
        .await
        .unwrap()
        .unwrap();
    let resolved = PluginRepository::get_current_local_installation(
        &store,
        "test-node",
        provider.extension_family.as_ref().unwrap(),
    )
    .await
    .unwrap()
    .expect("provider family must resolve the current artifact on the executing node");
    assert_eq!(resolved.id, current_installation_id);
}

#[tokio::test]
async fn ac_005_failed_provider_preflight_preserves_the_previous_current_artifact() {
    let (store, _workspace, actor) = seed_store().await;
    let old_installation_id = Uuid::now_v7();
    let target_installation_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(old_installation_id, actor.id, "0.2.5", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(target_installation_id, actor.id, "0.2.8", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::select_network_egress_current(&store, "test-node", old_installation_id)
        .await
        .unwrap()
        .unwrap();
    let provider_id = Uuid::now_v7();
    NetworkEgressRepository::create_network_egress_provider(
        &store,
        &CreateNetworkEgressProviderInput {
            provider_id,
            extension_family: domain::ExtensionCatalogIdentity::new(
                domain::ExtensionCategory::RuntimeExtensions,
                "taichuy",
                "clash-proxy",
            ),
            provider_code: "clash-proxy".into(),
            display_name: "Clash subscription".into(),
            description: String::new(),
            secret_ref: format!("secret://system/network-egress/{provider_id}"),
            lifecycle: domain::NetworkEgressProviderLifecycle::Active,
            actor_user_id: actor.id,
        },
    )
    .await
    .unwrap();

    let service = control_plane::network_egress::NetworkEgressProviderService::new(
        store.clone(),
        RejectNetworkEgressPreflight,
        StaticNetworkEgressSecret,
        "unused-test-key".into(),
        "test-node".into(),
    );
    let error = service
        .activate_version(target_installation_id)
        .await
        .expect_err("a rejected provider config must abort activation");
    assert!(error
        .to_string()
        .contains("network_egress_plugin_version_preflight_failed"));
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", old_installation_id)
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", target_installation_id)
            .await
            .unwrap()
            .unwrap()
            .is_current
    );
}

#[tokio::test]
async fn network_egress_current_repair_selects_latest_ready_version_from_legacy_artifacts() {
    let (store, _workspace, actor) = seed_store().await;
    let retained_id = Uuid::now_v7();
    let current_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(retained_id, actor.id, "0.2.2", "native_react"),
    )
    .await
    .unwrap();
    PluginRepository::commit_plugin_installation(
        &store,
        &network_egress_commit_input(current_id, actor.id, "0.2.3", "native_react"),
    )
    .await
    .unwrap();
    sqlx::query(
        "update extension_artifact_instances set is_current = false where installation_id in ($1, $2)",
    )
    .bind(retained_id)
    .bind(current_id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::raw_sql(REPAIR_NETWORK_EGRESS_CURRENT_ARTIFACTS_SQL)
        .execute(store.pool())
        .await
        .unwrap();

    assert!(
        !PluginRepository::get_artifact_instance(&store, "test-node", retained_id)
            .await
            .unwrap()
            .expect("retained artifact must remain")
            .is_current
    );
    assert!(
        PluginRepository::get_artifact_instance(&store, "test-node", current_id)
            .await
            .unwrap()
            .expect("latest artifact must become current")
            .is_current
    );
}

#[tokio::test]
async fn plugin_installation_commit_rolls_back_new_installation_when_frontend_catalog_violates_constraint(
) {
    let (store, _workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    let error = PluginRepository::commit_plugin_installation(
        &store,
        &installation_commit_input(installation_id, actor.id, "Invalid", "invalid"),
    )
    .await
    .unwrap_err();

    assert!(error
        .to_string()
        .contains("frontend_block_catalog_runtime_check"));
    assert!(PluginRepository::get_installation(&store, installation_id)
        .await
        .unwrap()
        .is_none());
}

#[tokio::test]
async fn plugin_installation_commit_preserves_previous_installation_and_catalog_on_reinstall_failure(
) {
    let (store, _workspace, actor) = seed_store().await;
    let installation_id = Uuid::now_v7();
    PluginRepository::commit_plugin_installation(
        &store,
        &installation_commit_input(installation_id, actor.id, "Original", "native_react"),
    )
    .await
    .unwrap();

    PluginRepository::commit_plugin_installation(
        &store,
        &installation_commit_input(installation_id, actor.id, "Changed", "invalid"),
    )
    .await
    .unwrap_err();

    let installation = PluginRepository::get_installation(&store, installation_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(installation.display_name, "Original");
    let catalog: (String, String) = sqlx::query_as(
        "select title, runtime from frontend_block_catalog where installation_id = $1",
    )
    .bind(installation_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(catalog, ("Original".into(), "native_react".into()));
}
