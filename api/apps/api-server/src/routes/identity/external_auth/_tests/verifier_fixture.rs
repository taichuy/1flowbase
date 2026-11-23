use crate::{
    app_state::ApiState,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeArtifactResolver, ApiRuntimeServices},
};
use control_plane::{
    plugin_management::{
        EnablePluginCommand, InstallUploadedPluginCommand, PluginManagementService,
    },
    ports::AuthRepository,
};
use domain::LoginEntryRecord;
use serde_json::json;
use std::sync::Arc;
use uuid::Uuid;

pub(crate) const CONTRIBUTION: &str = "acme.authentication-barrier.verify";

pub(crate) fn package_with_second_provider(second_provider: bool) -> Vec<u8> {
    let mut manifest = json!({
        "manifest_version": 2,
        "schema_version": "1flowbase.plugin.manifest/v2",
        "plugin_id": "acme.authentication-barrier",
        "version": "1.0.0",
        "publisher_namespace": "acme",
        "vendor": "Acme",
        "display_name": "Controlled verifier",
        "description": "Real managed OAuth verifier test fixture",
        "source_kind": "uploaded",
        "trust_level": "unverified",
        "consumption_kind": "runtime_extension",
        "execution_mode": "process_per_call",
        "slot_codes": [],
        "binding_targets": ["system"],
        "selection_mode": "assignment_then_select",
        "minimum_host_version": "0.1.0",
        "contract_version": "1flowbase.extension-bus/v1",
        "permissions": {"network":"none", "secrets":"none", "storage":"host_managed", "mcp":"none", "subprocess":"deny"},
        "runtime": {"protocol":"stdio_json_multiplex_v1", "entry":"bin/worker.py", "limits":{"timeout_ms":30000}},
        "data_models": [],
    });
    manifest["authentication_providers"] = json!([{
        "auth_type":"external-fixture", "display_name":"Controlled verifier", "config_schema":[],
        "default_public_ui_block":"external-fixture", "public_variable_keys":[], "contribution_id":CONTRIBUTION
    }]);
    manifest["managed"] = json!({
        "module":{"bus_version":"v1","module_id":"acme.authentication-barrier","module_version":"1.0.0","module_kind":"runtime", "contributions":[{
            "contribution_id":CONTRIBUTION,"contributor_module_id":"acme.authentication-barrier","point_id":"1flowbase.authentication.provider",
            "contract_version":"1","required_permissions":["authentication.verify"],"mode":"append"
        }]},
        "execution_bindings":[{"contribution_id":CONTRIBUTION,"execution_mode":"process_per_call","runtime":{"protocol":"stdio_json_multiplex_v1","entry":"bin/worker.py"},"handler":"authenticate","payload":{"kind":"authentication_provider","auth_type":"external-fixture"}}]
    });
    if second_provider {
        let mut descriptor = manifest["authentication_providers"][0].clone();
        descriptor["auth_type"] = json!("external-second");
        descriptor["display_name"] = json!("Second verifier");
        descriptor["contribution_id"] = json!("acme.authentication-barrier.second");
        manifest["authentication_providers"]
            .as_array_mut()
            .unwrap()
            .push(descriptor);
        let mut contribution = manifest["managed"]["module"]["contributions"][0].clone();
        contribution["contribution_id"] = json!("acme.authentication-barrier.second");
        manifest["managed"]["module"]["contributions"]
            .as_array_mut()
            .unwrap()
            .push(contribution);
        let mut binding = manifest["managed"]["execution_bindings"][0].clone();
        binding["contribution_id"] = json!("acme.authentication-barrier.second");
        binding["payload"]["auth_type"] = json!("external-second");
        manifest["managed"]["execution_bindings"]
            .as_array_mut()
            .unwrap()
            .push(binding);
    }
    // JSON is valid YAML, and this fixture also builds as part of the library.
    let manifest = manifest.to_string();
    plugin_framework::parse_plugin_manifest(&manifest).unwrap();
    let mut archive = tar::Builder::new(flate2::write::GzEncoder::new(
        Vec::new(),
        flate2::Compression::default(),
    ));
    for (path, bytes, mode) in [
        ("manifest.yaml", manifest.as_bytes(), 0o644),
        (
            "bin/worker.py",
            include_bytes!("callback_worker.py").as_slice(),
            0o755,
        ),
    ] {
        let mut header = tar::Header::new_gnu();
        header.set_size(bytes.len() as u64);
        header.set_mode(mode);
        header.set_cksum();
        archive.append_data(&mut header, path, bytes).unwrap();
    }
    archive.into_inner().unwrap().finish().unwrap()
}

/// Shared real installed verifier fixture for the personal authentication API.
/// Every repository, grant and worker call uses the production implementations.
pub async fn installed_verifier_fixture() -> (
    Arc<ApiState>,
    Arc<runtime_extension_host::RuntimeExtensionHost>,
    LoginEntryRecord,
    domain::ActorContext,
) {
    installed_verifier_fixture_with_second_provider(false).await
}

pub async fn installed_verifier_fixture_with_second_provider(
    second_provider: bool,
) -> (
    Arc<ApiState>,
    Arc<runtime_extension_host::RuntimeExtensionHost>,
    LoginEntryRecord,
    domain::ActorContext,
) {
    let (initial, database_url) =
        crate::test_support::support::test_api_state_with_database_url().await;
    let mut owned = (*initial).clone();
    let pool = sqlx::postgres::PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .unwrap();
    owned.store = storage_durable_postgres::MainDurableStore::new(pool);
    let assembly = crate::extension_bus::assemble_extension_graph_input(
        crate::api_workspace_root().unwrap(),
        crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
        vec![],
    )
    .unwrap();
    let host = Arc::new(
        runtime_extension_host::RuntimeExtensionHost::new_with_artifact_resolver(
            time::OffsetDateTime::now_utc(),
            Arc::new(ApiRuntimeArtifactResolver::new(
                owned.store.clone(),
                &owned.api_node_id,
                &owned.provider_install_root,
            )),
        )
        .unwrap(),
    );
    host.mark_ready().unwrap();
    owned.provider_runtime = Arc::new(
        crate::managed_runtime_assembly::bind_managed_runtime(
            crate::provider_runtime::services_for_current_cfg(
                ApiRuntimeServices::new_with_runtime_backend(
                    host.clone(),
                    Arc::new(assembly.compile_graph().unwrap()),
                )
                .unwrap(),
            ),
            owned.store.clone(),
            owned.api_node_id.clone(),
            assembly.module_descriptors().to_vec(),
        )
        .0,
    );
    let state = Arc::new(owned);
    let root_id: Uuid = sqlx::query_scalar("select id from users where account='root'")
        .fetch_one(state.store.pool())
        .await
        .unwrap();
    let actor = state
        .store
        .load_actor_context_for_user(root_id)
        .await
        .unwrap();
    let management = PluginManagementService::new(
        state.store.clone(),
        ApiProviderRuntime::new(state.provider_runtime.clone()),
        state.official_plugin_source.clone(),
        &state.provider_install_root,
    )
    .with_node_id(&state.api_node_id)
    .with_managed_activation();
    let installed = management
        .install_uploaded_plugin(InstallUploadedPluginCommand {
            actor_user_id: root_id,
            file_name: "authentication-methods.1flowbasepkg".into(),
            package_bytes: package_with_second_provider(second_provider),
        })
        .await
        .unwrap();
    management
        .enable_plugin(EnablePluginCommand {
            actor_user_id: root_id,
            installation_id: installed.installation.id,
        })
        .await
        .unwrap();
    let entry = LoginEntryRecord {
        id: Uuid::now_v7(),
        connection_id: Uuid::now_v7(),
        auth_type: "external-fixture".into(),
        title: "Controlled verifier".into(),
        enabled: true,
        is_builtin: false,
        sort_order: 1,
        public_ui_block: "external-fixture".into(),
        options: json!({"extension_config":{"self_registration_enabled":true}}),
    };
    state.store.create_login_entry(&entry).await.unwrap();
    if second_provider {
        let mut second = entry.clone();
        second.id = Uuid::now_v7();
        second.connection_id = Uuid::now_v7();
        second.auth_type = "external-second".into();
        second.title = "Second verifier".into();
        state.store.create_login_entry(&second).await.unwrap();
    }
    (state, host, entry, actor)
}
