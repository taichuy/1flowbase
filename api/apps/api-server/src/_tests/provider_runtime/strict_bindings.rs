use super::*;
use crate::extension_bus::SnapshotAssembly;

#[tokio::test]
async fn strict_runtime_uses_one_boot_graph_for_model_provider_and_empty_input_pipeline() {
    let graph = model_provider_extension_graph();
    let services = ApiRuntimeServices::new(Arc::clone(&graph)).unwrap();
    let snapshot = crate::extension_bus::ExtensionBootSnapshot::new(Arc::clone(&graph));
    let resolver_graph = services.model_provider_extension_graph().unwrap();

    assert!(Arc::ptr_eq(resolver_graph, snapshot.graph_arc()));
    assert_eq!(
        resolver_graph.fingerprint().as_str(),
        snapshot.fingerprint()
    );
    let pipeline_output = ProviderRuntimePort::pipeline_provider_input(
        &ApiProviderRuntime::new(Arc::new(services.clone())),
        ProviderInvocationInput::default(),
    )
    .await
    .unwrap();
    assert_eq!(pipeline_output.input, ProviderInvocationInput::default());
    assert_eq!(
        pipeline_output.receipt.unwrap().graph_fingerprint,
        snapshot.fingerprint()
    );

    let legacy = ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests();
    assert!(legacy.model_provider_extension_graph().is_none());
}

#[tokio::test]
async fn strict_model_provider_binding_loads_the_stateful_lifecycle_host() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let mut installation = strict_fixture_installation(&package);
    installation.artifact.availability_status = PluginAvailabilityStatus::InstallIncomplete;
    let graph = model_provider_extension_graph();
    let services = Arc::new(ApiRuntimeServices::new(Arc::clone(&graph)).unwrap());
    let runtime = ApiProviderRuntime::new(services);

    let output = ProviderRuntimePort::validate_provider(&runtime, &installation, json!({}))
        .await
        .expect("strict binding should dispatch through the stateful provider host");
    assert_eq!(output["provider"], "fixture");
    assert!(spawn_marker.exists());
    let binding = crate::provider_runtime::resolver_for_current_cfg(
        crate::provider_runtime::ModelProviderSlotResolver::new(graph),
    )
    .resolve(&installation)
    .unwrap();
    assert_eq!(binding.installation_id, installation.id);
    assert_eq!(binding.plugin_id, installation.plugin_id);
    assert_eq!(binding.provider_code, installation.provider_code);
    assert!(matches!(
        binding.provenance,
        crate::provider_runtime::ModelProviderBindingProvenance::BootGraph(_)
    ));
}

#[tokio::test]
async fn strict_model_provider_binding_rejects_invalid_dynamic_facts_before_spawn() {
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let graph = model_provider_extension_graph();
    let runtime = ApiProviderRuntime::new(Arc::new(ApiRuntimeServices::new(graph).unwrap()));
    let valid = strict_fixture_installation(&package);
    let mut cases = Vec::new();

    let mut disabled = valid.clone();
    disabled.installation.desired_state = PluginDesiredState::Disabled;
    cases.push(disabled.clone());
    let mut pending = valid.clone();
    pending.installation.desired_state = PluginDesiredState::PendingRestart;
    cases.push(pending);
    let mut artifact_missing = valid.clone();
    artifact_missing.artifact.artifact_status = domain::PluginArtifactInstanceStatus::Missing;
    cases.push(artifact_missing);
    let mut artifact_mismatched = valid.clone();
    artifact_mismatched.artifact.artifact_status = domain::PluginArtifactInstanceStatus::Mismatched;
    cases.push(artifact_mismatched);
    let mut load_failed = valid.clone();
    load_failed.artifact.runtime_status = PluginRuntimeStatus::LoadFailed;
    cases.push(load_failed);
    let mut verification_pending = valid.clone();
    verification_pending.installation.verification_status = PluginVerificationStatus::Pending;
    cases.push(verification_pending);
    let mut wrong_contract = valid.clone();
    wrong_contract.installation.contract_version = "1flowbase.data_source/v1".to_string();
    cases.push(wrong_contract);
    let mut wrong_category = valid.clone();
    wrong_category.installation.category = domain::ExtensionCategory::CapabilityPlugins;
    cases.push(wrong_category);

    for installation in cases {
        assert!(
            ProviderRuntimePort::validate_provider(&runtime, &installation, json!({}))
                .await
                .is_err()
        );
    }
    let canonical_input = ProviderInvocationInput {
        provider_code: disabled.provider_code.clone(),
        ..ProviderInvocationInput::default()
    };
    assert!(ProviderRuntimePort::ensure_loaded(&runtime, &disabled)
        .await
        .is_err());
    assert!(
        ProviderRuntimePort::list_models(&runtime, &disabled, json!({}))
            .await
            .is_err()
    );
    assert!(
        ProviderRuntimePort::get_balance(&runtime, &disabled, json!({}))
            .await
            .is_err()
    );
    assert!(ProviderRuntimePort::count_tokens(
        &runtime,
        &disabled,
        ProviderCountTokensInput::from_invocation(canonical_input.clone())
    )
    .await
    .is_err());
    assert!(
        ProviderRuntimePort::compact(&runtime, &disabled, canonical_input.clone())
            .await
            .is_err()
    );
    assert!(
        ProviderRuntimePort::invoke_stream(&runtime, &disabled, canonical_input.clone())
            .await
            .is_err()
    );
    assert!(ProviderRuntimePort::invoke_stream_with_live_events(
        &runtime,
        &disabled,
        canonical_input,
        None
    )
    .await
    .is_err());
    assert!(!spawn_marker.exists());
}

#[tokio::test]
async fn strict_model_provider_binding_rejects_wrong_provider_code_and_missing_point_before_spawn()
{
    let package = TempProviderPackage::new();
    let spawn_marker = package.path().join("spawned");
    write_stateful_slot_provider_package(&package, &spawn_marker);
    let installation = strict_fixture_installation(&package);
    let runtime = ApiProviderRuntime::new(Arc::new(
        ApiRuntimeServices::new(model_provider_extension_graph()).unwrap(),
    ));
    let wrong_provider = ProviderInvocationInput {
        provider_code: "another_provider".to_string(),
        ..ProviderInvocationInput::default()
    };
    assert!(runtime
        .invoke_stream(&installation, wrong_provider)
        .await
        .is_err());

    let missing_point_error = crate::provider_runtime::resolver_for_current_cfg(
        crate::provider_runtime::ModelProviderSlotResolver::new(
            graph_without_model_provider_point(),
        ),
    )
    .resolve(&installation)
    .unwrap_err();
    assert!(missing_point_error
        .to_string()
        .contains("model_provider_extension_slot_unavailable"));
    assert!(!spawn_marker.exists());
}

#[test]
fn provider_runtime_consumer_has_no_provider_specific_branch_and_production_wires_strict_graph() {
    let consumer = concat!(
        include_str!("../../../../../crates/api/kernel/src/host/provider_runtime/mod.rs"),
        "\n",
        include_str!("../../../../../crates/api/kernel/src/host/provider_runtime/provider_port.rs"),
        "\n",
        include_str!(
            "../../../../../crates/api/kernel/src/host/provider_runtime/capability_port.rs"
        ),
        "\n",
        include_str!(
            "../../../../../crates/api/kernel/src/host/provider_runtime/data_source_ports.rs"
        ),
    );
    let transport_lifecycle = concat!(
        include_str!("../../../../../crates/api/kernel/src/host/provider_runtime/transport_session_lifecycle.rs"),
        "\n",
        include_str!("../../../../../crates/api/kernel/src/host/provider_runtime/transport_session_lifecycle/admission.rs"),
        "\n",
        include_str!("../../../../../crates/api/kernel/src/host/provider_runtime/transport_session_lifecycle/termination_dispatch.rs"),
    );
    let boot = concat!(
        include_str!("../../lib.rs"),
        "\n",
        include_str!("../../internal/startup_assembly.rs"),
    );

    assert!(!consumer.to_ascii_lowercase().contains("openai"));
    assert!(!consumer.to_ascii_lowercase().contains("anthropic"));
    assert_eq!(
        consumer
            .matches("self.resolve_model_provider_binding(installation)?")
            .count(),
        13,
        "every ProviderRuntimePort operation must resolve the typed slot binding"
    );
    assert!(consumer.contains(".orchestration_backend"));
    assert!(consumer.contains(".provider_count_tokens("));
    assert!(consumer.contains(".data_source_execute_sql("));
    assert!(consumer.contains(".capability_execute("));
    assert!(consumer.contains(".network_egress_sync("));
    assert!(!consumer.contains(".provider_registry()"));
    assert!(!consumer.contains(".data_source_registry()"));
    assert!(!consumer.contains(".capability_registry()"));
    assert!(!consumer.contains(".network_egress_registry()"));
    assert_eq!(
        consumer.matches("binding.require_provider_code(").count(),
        5,
        "every canonical provider input must fail closed on a crossed provider code"
    );
    assert!(boot.contains(
        "RuntimeExtensionHost::new_with_artifact_resolver_plugin_data_and_profile_source("
    ));
    assert!(boot.contains("ApiRuntimeArtifactResolver::new("));
    assert!(boot.contains("RuntimeBackendSlot::default()"));
    assert!(boot.contains("runtime_backend_slot.bind(runtime_extension_host.clone())"));
    assert!(boot.contains("ApiRuntimeServices::new_with_runtime_backend_and_transport_config("));
    assert!(boot.contains("provider_runtime.start_transport_session_scheduler()"));
    assert!(boot.contains("shutdown_transport_sessions("));
    assert!(transport_lifecycle.contains("maintain_and_dispatch"));
    assert!(transport_lifecycle.contains("ProviderTransportSessionAction::Drain"));
    assert!(transport_lifecycle.contains("ProviderTransportSessionAction::Close"));
    assert!(transport_lifecycle.contains("generation: fence.generation.get()"));
    assert!(transport_lifecycle.contains("InvocationRequest"));
    assert!(transport_lifecycle.contains(".rotate_generation_to(&fence, target.clone())"));
    assert!(transport_lifecycle.contains("drain_disposition(&snapshot, &task.fence)"));
    assert!(!transport_lifecycle.contains("task_deadline"));
    assert!(boot.contains("Arc::clone(&extension_graph)"));
    assert!(!boot.contains("new_without_model_provider_extension_graph_for_tests"));
}

#[tokio::test]
async fn ac_003_api_runtime_dispatches_network_egress_lifecycle_without_model_provider_v2_wire() {
    let package = TempProviderPackage::new();
    write_network_egress_package(&package);
    let installation = network_egress_fixture_installation(&package);
    let services =
        Arc::new(ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests());
    let runtime = ApiProviderRuntime::new(services);

    ProviderRuntimePort::activate_plugin(&runtime, &installation)
        .await
        .expect("network egress activation must dispatch its stateful worker");
    ProviderRuntimePort::deactivate_plugin(&runtime, &installation.installation)
        .await
        .expect("network egress deactivation must revoke its active lease");
}

#[tokio::test]
async fn data_source_runtime_intakes_and_dispatches_checked_external_template() {
    let services =
        Arc::new(ApiRuntimeServices::new_without_model_provider_extension_graph_for_tests());
    let runtime = ApiProviderRuntime::new(services.clone());
    let installation = checked_data_source_fixture_installation();

    DataSourceRuntimePort::ensure_loaded(&runtime, &installation)
        .await
        .expect("checked data source fixture should load");
    let identity = DataModelTemplateIdentity {
        provider: "data_source_http_fixture".to_string(),
        code: "contact_archive".to_string(),
        version: "v1".to_string(),
    };
    let template = services
        .data_model_template_catalog()
        .resolve(&identity)
        .expect("checked external template should enter the shared catalog");
    assert_eq!(template.descriptor().operations[0].code, "archive_contact");

    let output = DataSourceRuntimePort::execute_model_operation(
        &runtime,
        &installation,
        checked_data_source_model_operation_input(json!({ "reason": "duplicate" })),
    )
    .await
    .expect("checked external operation should dispatch through the host");
    assert_eq!(output["archived"], true);
    assert_eq!(output["contact_id"], "contact-1");

    let malformed_error = DataSourceRuntimePort::execute_model_operation(
        &runtime,
        &installation,
        checked_data_source_model_operation_input(json!({ "malformed_response": true })),
    )
    .await
    .expect_err("malformed external operation output must fail closed");
    assert!(matches!(
        malformed_error.downcast_ref::<control_plane::errors::ControlPlaneError>(),
        Some(control_plane::errors::ControlPlaneError::InvalidInput(
            "data_source_runtime"
        ))
    ));
}

#[tokio::test]
async fn postgres_activation_rejects_manifest_replaced_after_scan_even_with_updated_marker() {
    use control_plane::plugin_management::{
        try_mark_current_node_plugin_runtime_active, InstallPluginCommand, PluginManagementService,
    };
    use control_plane_contracts::ports::PluginRepository;

    let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
    let actor = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let package = TempProviderPackage::new();
    write_stateful_slot_provider_package(&package, &package.path().join("spawned"));
    let services = Arc::new(ApiRuntimeServices::new(model_provider_extension_graph()).unwrap());
    let runtime = ApiProviderRuntime::new(services.clone());
    let management = PluginManagementService::new(
        state.store.clone(),
        runtime.clone(),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id);
    let installed = management
        .install_plugin(InstallPluginCommand {
            actor_user_id: actor,
            package_root: package.path().display().to_string(),
        })
        .await
        .unwrap();
    let id = installed.installation.id;
    let artifact = installed.local_artifact;
    assert!(artifact.artifact_status.is_ready());
    assert!(artifact.last_error.is_none());
    assert_eq!(artifact.runtime_status, PluginRuntimeStatus::Inactive);
    let path = Path::new(artifact.local_path.as_deref().unwrap());
    let manifest = path.join("manifest.yaml");
    let original = fs::read_to_string(&manifest).unwrap();
    let mut raw = original.clone();
    raw.push_str("\n# replaced after the inventory scan\n");
    fs::write(&manifest, &raw).unwrap();
    let actual = plugin_framework::compute_manifest_fingerprint(&manifest)
        .await
        .unwrap();
    assert_ne!(
        artifact.manifest_fingerprint.as_deref(),
        Some(actual.as_str())
    );
    let marker = path.join(".1flowbase-artifact.json");
    let original_marker = fs::read(&marker).unwrap();
    let mut marker_json: serde_json::Value = serde_json::from_slice(&original_marker).unwrap();
    marker_json["manifest_fingerprint"] = actual.into();
    fs::write(&marker, serde_json::to_vec(&marker_json).unwrap()).unwrap();

    // Request activation without an inventory refresh, just as an on-demand provider load does.
    sqlx::query(
        "update extension_installations set desired_state = 'active_requested' where id = $1",
    )
    .bind(id)
    .execute(state.store.pool())
    .await
    .unwrap();
    let local = state
        .store
        .get_local_installation(&state.api_node_id, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(
        local.artifact.manifest_fingerprint,
        artifact.manifest_fingerprint
    );
    if let Ok(loaded) = ProviderRuntimePort::ensure_loaded(&runtime, &local).await {
        let outcome = try_mark_current_node_plugin_runtime_active(
            &state.store,
            &state.api_node_id,
            &local,
            loaded.as_ref(),
        )
        .await
        .unwrap();
        panic!("host loaded unverified manifest; activation outcome: {outcome:?}");
    }
    let stored = state
        .store
        .get_artifact_instance(&state.api_node_id, id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(stored.manifest_fingerprint, artifact.manifest_fingerprint);
    assert_ne!(
        stored.runtime_status,
        PluginRuntimeStatus::Active,
        "unexpected active runtime"
    );
    let host = services.runtime_backend().snapshot().await.unwrap();
    assert_eq!(
        host.registries.providers, 0,
        "host retained an unverified manifest"
    );

    fs::write(&manifest, original).unwrap();
    fs::write(&marker, original_marker).unwrap();
    let loaded = ProviderRuntimePort::ensure_loaded(&runtime, &local)
        .await
        .unwrap()
        .expect("real host returns the parsed artifact identity");
    assert_eq!(
        Some(loaded.manifest_fingerprint.as_str()),
        artifact.manifest_fingerprint.as_deref()
    );
    assert_eq!(loaded.plugin_version, installed.installation.plugin_version);
    assert_eq!(
        loaded.marker.as_ref().unwrap().checksum,
        artifact.local_checksum
    );
    assert!(matches!(
        try_mark_current_node_plugin_runtime_active(
            &state.store,
            &state.api_node_id,
            &local,
            Some(&loaded)
        )
        .await
        .unwrap(),
        control_plane_contracts::ports::ActivatePluginArtifactOutcome::Activated(_)
    ));

    fs::write(&manifest, raw).unwrap();
    assert!(
        ProviderRuntimePort::ensure_loaded(&runtime, &local)
            .await
            .is_err(),
        "a matching source cache key must not hide manifest changes"
    );
    ProviderRuntimePort::deactivate_plugin(&runtime, &installed.installation)
        .await
        .unwrap();
}

#[tokio::test]
async fn symmetric_f3_switch_revalidates_cached_active_target_after_final_scan() {
    use control_plane::plugin_management::{
        AssignPluginCommand, EnablePluginCommand, InstallPluginCommand, PluginManagementService,
        SwitchPluginVersionCommand,
    };
    use control_plane_contracts::ports::{
        AuthRepository, CreatePluginAssignmentInput, PluginRepository,
    };

    let (state, database_url) = crate::_tests::support::test_api_state_with_database_pool_capacity(
        std::num::NonZeroU32::new(3).unwrap(),
    )
    .await;
    let actor = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let workspace = state
        .store
        .load_actor_context_for_user(actor)
        .await
        .unwrap()
        .current_workspace_id;
    let other = crate::_tests::support::seed_workspace(&database_url, "already on v2").await;
    let services = Arc::new(ApiRuntimeServices::new(model_provider_extension_graph()).unwrap());
    let runtime = ApiProviderRuntime::new(services);
    let management = PluginManagementService::new(
        state.store.clone(),
        runtime.clone(),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .for_plugin_console_operation(
        domain::ConsolePolicyGroup::other("other.plugins").unwrap(),
        "plugins.families.switch",
    );
    let install = |version: &'static str| {
        let management = &management;
        async move {
            let package = TempProviderPackage::new();
            write_stateful_slot_provider_package(&package, &package.path().join("spawned"));
            let manifest = package.path().join("manifest.yaml");
            let raw = tokio::fs::read_to_string(&manifest)
                .await
                .unwrap()
                .replace(
                    "fixture_provider@0.1.0",
                    &format!("fixture_provider@{version}"),
                )
                .replace("\nversion: 0.1.0\n", &format!("\nversion: {version}\n"));
            tokio::fs::write(&manifest, raw).await.unwrap();
            management
                .install_plugin(InstallPluginCommand {
                    actor_user_id: actor,
                    package_root: package.path().display().to_string(),
                })
                .await
                .unwrap()
        }
    };
    let first = install("0.1.0").await.installation;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor,
            installation_id: first.id,
        })
        .await
        .unwrap();
    management
        .assign_plugin(AssignPluginCommand {
            actor_user_id: actor,
            installation_id: first.id,
        })
        .await
        .unwrap();
    let second = install("0.2.0").await.installation;
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: actor,
            installation_id: second.id,
        })
        .await
        .unwrap();
    state
        .store
        .create_assignment(&CreatePluginAssignmentInput {
            installation_id: second.id,
            workspace_id: other,
            provider_code: second.provider_code.clone(),
            actor_user_id: actor,
        })
        .await
        .unwrap();
    let local = state
        .store
        .get_local_installation(&state.api_node_id, second.id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(local.desired_state, PluginDesiredState::ActiveRequested);
    assert_eq!(local.artifact.runtime_status, PluginRuntimeStatus::Active);
    let marker =
        Path::new(local.artifact.local_path.as_deref().unwrap()).join(".1flowbase-artifact.json");
    let original_marker = tokio::fs::read(&marker).await.unwrap();
    let mut value: serde_json::Value = serde_json::from_slice(&original_marker).unwrap();
    value["checksum"] = "sha256:tampered".into();

    // Hold the second refresh after its disk scan, so even the ready-target scan sees the
    // original marker. A DB-only assertion at version-switch commit would now pass.
    sqlx::raw_sql(r#"
        create table switch_scan_count (installation_id uuid primary key, n integer not null);
        create function pause_after_target_scan() returns trigger language plpgsql as $$
        declare scans integer;
        begin
            update switch_scan_count set n = n + 1 where installation_id = old.installation_id returning n into scans;
            if scans = 2 then perform pg_advisory_xact_lock(224311); end if;
            return new;
        end $$;
        create trigger pause_after_target_scan before update of checked_at on extension_artifact_instances
        for each row execute function pause_after_target_scan();
    "#).execute(state.store.pool()).await.unwrap();
    sqlx::query("insert into switch_scan_count values ($1, 0)")
        .bind(second.id)
        .execute(state.store.pool())
        .await
        .unwrap();
    let mut blocker = state.store.pool().begin().await.unwrap();
    let blocker_pid: i32 = sqlx::query_scalar("select pg_backend_pid()")
        .fetch_one(&mut *blocker)
        .await
        .unwrap();
    sqlx::query("select pg_advisory_xact_lock(224311)")
        .execute(&mut *blocker)
        .await
        .unwrap();
    let switched = management.switch_version(SwitchPluginVersionCommand {
        actor_user_id: actor,
        provider_code: first.provider_code.clone(),
        target_installation_id: second.id,
    });
    let tamper = async {
        let waited = tokio::time::timeout(Duration::from_secs(10), async {
            loop {
                let blocked: bool = sqlx::query_scalar("select exists(select 1 from pg_locks where not granted and locktype = 'advisory' and $1 = any(pg_blocking_pids(pid)))")
                    .bind(blocker_pid).fetch_one(state.store.pool()).await.unwrap();
                if blocked { break; }
                tokio::time::sleep(Duration::from_millis(10)).await;
            }
        }).await;
        if waited.is_ok() {
            tokio::fs::write(&marker, serde_json::to_vec(&value).unwrap())
                .await
                .unwrap();
        }
        blocker.rollback().await.unwrap();
        waited.unwrap();
    };
    let (result, ()) = tokio::join!(switched, tamper);
    tokio::fs::write(marker, original_marker).await.unwrap();
    result.expect_err("switch reused the active row without checking the host's cached identity");
    assert_eq!(
        state.store.list_assignments(workspace).await.unwrap()[0].installation_id,
        first.id
    );
    assert_eq!(
        state.store.list_assignments(other).await.unwrap()[0].installation_id,
        second.id
    );
    ProviderRuntimePort::deactivate_plugin(&runtime, &first)
        .await
        .unwrap();
    ProviderRuntimePort::deactivate_plugin(&runtime, &second)
        .await
        .unwrap();
}
