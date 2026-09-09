use api_server::host_extension_boot::builtin_host_extension_ids;

#[test]
fn builtin_host_extensions_include_plan_f_official_hosts() {
    let ids = builtin_host_extension_ids();

    assert_eq!(
        ids,
        vec![
            "official.identity-host",
            "official.workspace-host",
            "official.plugin-host",
            "official.local-infra-host",
            "official.file-management-host",
            "official.runtime-orchestration-host",
        ]
    );
}

use control_plane::ports::{
    AuthRepository, PluginRepository, ReviseUiCodeTemplateInput, UiManagementRepository,
};
use domain::{NativePluginTarget, PluginDesiredState, PluginRuntimeStatus, UiCodeTemplateLanguage};
use std::{path::PathBuf, sync::Arc};
use uuid::Uuid;

pub(crate) struct NativeSettingsFixture {
    pub state: Arc<crate::app_state::ApiState>,
    pub actor: Uuid,
    source_root: PathBuf,
}
impl Drop for NativeSettingsFixture {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.source_root);
    }
}
impl NativeSettingsFixture {
    pub async fn new() -> Self {
        let (state, _) = crate::_tests::support::test_api_state_with_database_url().await;
        let actor = state
            .store
            .find_user_for_password_login(domain::BUILTIN_PASSWORD_LOGIN_ENTRY_ID, "root")
            .await
            .unwrap()
            .unwrap()
            .id;
        Self {
            state,
            actor,
            source_root: std::env::temp_dir()
                .join(format!("native-settings-fixture-{}", Uuid::now_v7())),
        }
    }
    pub async fn select(&self, version: &str) -> NativePluginTarget {
        let root = self.source_root.join(version);
        std::fs::create_dir_all(&root).unwrap();
        let source = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../plugins/fixtures/northwind.settings-page");
        for file in ["manifest.yaml", "host-extension.yaml", "settings.tsx"] {
            std::fs::write(
                root.join(file),
                std::fs::read_to_string(source.join(file))
                    .unwrap()
                    .replace("1.0.0", version),
            )
            .unwrap();
        }
        let service = control_plane::plugin_management::PluginManagementService::new(
            self.state.store.clone(),
            crate::provider_runtime::ApiProviderRuntime::new(self.state.provider_runtime.clone()),
            self.state.official_plugin_source.clone(),
            self.state.provider_install_root.clone(),
        )
        .with_node_id(self.state.api_node_id.clone());
        let installation = service
            .install_plugin(control_plane::plugin_management::InstallPluginCommand {
                actor_user_id: self.actor,
                package_root: root.display().to_string(),
            })
            .await
            .unwrap()
            .installation;
        service
            .enable_plugin(control_plane::plugin_management::EnablePluginCommand {
                actor_user_id: self.actor,
                installation_id: installation.id,
            })
            .await
            .unwrap();
        self.state
            .store
            .list_native_plugin_targets()
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.installation_id == installation.id)
            .unwrap()
    }
    pub fn contribution(
        &self,
        version: &str,
    ) -> plugin_framework::HostExtensionContributionManifest {
        plugin_framework::parse_host_extension_contribution_manifest(
            &std::fs::read_to_string(self.source_root.join(version).join("host-extension.yaml"))
                .unwrap(),
        )
        .unwrap()
    }
    pub fn process_state(
        &self,
        version: &str,
        target: NativePluginTarget,
    ) -> Arc<crate::app_state::ApiState> {
        let contribution = self.contribution(version);
        let surfaces = crate::console_surface_registry::ConsoleSurfaceRegistry::from_host_extension_contributions([&contribution]).unwrap().with_native_targets(vec![target]);
        let assembly = crate::extension_bus::assemble_extension_graph_input(
            crate::api_workspace_root().unwrap(),
            crate::extension_bus::DEFAULT_PLUGIN_SET_PATH,
            Vec::new(),
        )
        .unwrap();
        let snapshot = Arc::new(
            crate::extension_bus::ExtensionBootSnapshot::compile(
                Arc::new(assembly.compile_graph().unwrap()),
                assembly.interface_operations(),
                assembly.host_extension_manifests(),
                Arc::new(
                    crate::extension_bus::DurableHostInfrastructureProvidersViewQuery::new(
                        self.state.store.clone(),
                        self.state.api_node_id.clone(),
                    ),
                ),
                Vec::new(),
            )
            .unwrap(),
        );
        Arc::new(crate::app_state::ApiState {
            console_surface_registry: Arc::new(surfaces),
            extension_boot_snapshot: Some(snapshot),
            ..(*self.state).clone()
        })
    }
    pub async fn prepare(&self) -> crate::host_extension_loader::PreparedHostExtensionsAtStartup {
        crate::host_extension_loader::prepare_host_extensions_at_startup(
            &self.state.store,
            &self.state.api_node_id,
            &self.state.provider_install_root,
            &self.state.host_extension_dropin_root,
            self.state.allow_unverified_filesystem_dropins,
        )
        .await
        .unwrap()
    }
    pub async fn edit(&self, source: &str) -> Uuid {
        let template = self
            .state
            .store
            .list_ui_code_templates(false)
            .await
            .unwrap()
            .into_iter()
            .find(|t| t.owner_plugin_code.as_deref() == Some("northwind.settings-page"))
            .unwrap();
        self.state
            .store
            .revise_ui_code_template(&ReviseUiCodeTemplateInput {
                template_id: template.id,
                name: "User settings".into(),
                source: source.into(),
                language: UiCodeTemplateLanguage::Tsx,
                actor_user_id: self.actor,
            })
            .await
            .unwrap();
        template.id
    }
}

#[tokio::test]
async fn root_2014_ac_014_template_commit_failure_keeps_target_pending() {
    let f = NativeSettingsFixture::new().await;
    let target = f.select("1.0.0").await;
    // Two pages exercise rollback after an earlier revision was already inserted.
    sqlx::query("insert into plugin_settings_template_defaults(installation_id,contribution_code,feature_id,source,language) values($1,'zz','northwind.settings-page.zz','reject','tsx')").bind(target.installation_id).execute(f.state.store.pool()).await.unwrap();
    sqlx::query("alter table ui_code_template_revisions add constraint controlled_apply_failure check(source <> 'reject')").execute(f.state.store.pool()).await.unwrap();
    let prepared = f.prepare().await;
    assert!(prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .is_err());
    assert!(f
        .state
        .store
        .list_ui_code_templates(false)
        .await
        .unwrap()
        .is_empty());
    assert!(!f
        .state
        .store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
    assert_eq!(
        f.state
            .store
            .get_installation(target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .desired_state,
        PluginDesiredState::PendingRestart
    );
    assert_eq!(
        f.state
            .store
            .get_artifact_instance(&f.state.api_node_id, target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::LoadFailed
    );
    sqlx::query("alter table ui_code_template_revisions drop constraint controlled_apply_failure")
        .execute(f.state.store.pool())
        .await
        .unwrap();
    let prepared = f.prepare().await;
    prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .unwrap();
    crate::host_extension_loader::activate_prepared_host_extensions(
        &f.state.store,
        &f.state.api_node_id,
        prepared,
    )
    .await
    .unwrap();
    assert!(f
        .state
        .store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
}

#[tokio::test]
async fn root_2014_ac_014_post_apply_boot_failure_retry_preserves_user_edit() {
    let f = NativeSettingsFixture::new().await;
    let target = f.select("1.0.0").await;
    let prepared = f.prepare().await;
    prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .unwrap();
    crate::host_extension_loader::fail_after_template_apply_for(target.installation_id);
    assert!(
        crate::host_extension_loader::activate_prepared_host_extensions(
            &f.state.store,
            &f.state.api_node_id,
            prepared
        )
        .await
        .is_err()
    );
    assert!(f
        .state
        .store
        .native_plugin_target_is_applied(&target)
        .await
        .unwrap());
    assert_eq!(
        f.state
            .store
            .get_artifact_instance(&f.state.api_node_id, target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::LoadFailed
    );
    let id = f
        .edit("export default () => <p>after committed application</p>")
        .await;
    let prepared = f.prepare().await;
    prepared
        .apply_templates(&f.state.store, &f.state.api_node_id)
        .await
        .unwrap();
    crate::host_extension_loader::activate_prepared_host_extensions(
        &f.state.store,
        &f.state.api_node_id,
        prepared,
    )
    .await
    .unwrap();
    assert_eq!(
        f.state
            .store
            .get_ui_code_template(id)
            .await
            .unwrap()
            .unwrap()
            .latest_revision
            .source,
        "export default () => <p>after committed application</p>"
    );
    assert_eq!(
        f.state
            .store
            .get_artifact_instance(&f.state.api_node_id, target.installation_id)
            .await
            .unwrap()
            .unwrap()
            .runtime_status,
        PluginRuntimeStatus::Active
    );
}
