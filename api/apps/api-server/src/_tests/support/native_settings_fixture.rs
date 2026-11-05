use control_plane::ports::PluginRepository;
use domain::NativePluginTarget;

#[cfg_attr(test, api_test_selection::group(contracts))]
use control_plane::ports::{ReviseUiCodeTemplateInput, UiManagementRepository};
#[cfg_attr(test, api_test_selection::group(contracts))]
use domain::UiCodeTemplateLanguage;

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

    #[cfg_attr(test, api_test_selection::group(contracts))]
    pub async fn select(&self, version: &str) -> NativePluginTarget {
        self.select_with_modules(version, false).await
    }

    pub async fn select_with_modules(&self, version: &str, modules: bool) -> NativePluginTarget {
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
        if modules {
            use sha2::{Digest, Sha256};
            let bytes = format!("export const version = '{version}';");
            std::fs::write(root.join("module.js"), &bytes).unwrap();
            let module = format!("    code_modules:\n      - source: '@fixture/settings'\n        version: '{version}'\n        exports: [version]\n        binding: fetched\n        assets:\n          - path: module.js\n            role: browser_module\n            media_type: 'text/javascript; charset=utf-8'\n            sha256: '{:x}'\n        type_declarations: \"declare module '@fixture/settings' {{ export const version: string; }}\"\n", base16ct::HexDisplay(&Sha256::digest(bytes.as_bytes())));
            for file in ["manifest.yaml", "host-extension.yaml"] {
                let path = root.join(file);
                let mut raw = std::fs::read_to_string(&path).unwrap();
                raw.push_str(&module);
                std::fs::write(path, raw).unwrap();
            }
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

    #[cfg_attr(test, api_test_selection::group(contracts))]
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

    #[cfg_attr(test, api_test_selection::group(contracts))]
    pub async fn edit(&self, source: &str) -> Uuid {
        let template = self
            .state
            .store
            .list_ui_code_templates()
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
