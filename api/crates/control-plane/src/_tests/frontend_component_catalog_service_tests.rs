use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    frontend_block_catalog::{
        FrontendComponentCatalogService, GetFrontendComponentCapabilityQuery,
        ListFrontendComponentCapabilitiesQuery,
    },
    ports::{
        CreateUiCodeTemplateInput, FrontendBlockCatalogRepository,
        ReplaceInstallationFrontendBlocksInput, ReviseUiCodeTemplateInput,
        ReviseUiComponentContractInput, UiManagementRepository,
    },
};
use domain::{
    FrontendBlockCatalogEntry, FrontendBlockCodeModule, FrontendBlockContextContract,
    FrontendBlockPermissions, FrontendComponentContract, FrontendComponentExample,
    FrontendModuleAsset, FrontendModuleAssetRole, FrontendModuleBinding, UiCodeTemplate,
    UiComponentContractRevision, UiComponentLocator, UiComponentOverride, UiComponentOverrideState,
    SYSTEM_SCOPE_ID,
};
use uuid::Uuid;

#[derive(Clone)]
struct MemoryFrontendComponentCatalog {
    entries: Vec<FrontendBlockCatalogEntry>,
    overrides: Vec<UiComponentOverride>,
}

#[async_trait]
impl FrontendBlockCatalogRepository for MemoryFrontendComponentCatalog {
    async fn replace_installation_frontend_blocks(
        &self,
        _input: &ReplaceInstallationFrontendBlocksInput,
    ) -> Result<()> {
        Ok(())
    }

    async fn list_workspace_frontend_blocks(
        &self,
        _node_id: &str,
        _workspace_id: Uuid,
    ) -> Result<Vec<FrontendBlockCatalogEntry>> {
        Ok(self.entries.clone())
    }

    async fn list_system_frontend_blocks(
        &self,
        _node_id: &str,
    ) -> Result<Vec<FrontendBlockCatalogEntry>> {
        Ok(self.entries.clone())
    }

    async fn list_ui_component_overrides_for_catalog(&self) -> Result<Vec<UiComponentOverride>> {
        Ok(self.overrides.clone())
    }
}

#[async_trait]
impl UiManagementRepository for MemoryFrontendComponentCatalog {
    async fn list_ui_code_templates(&self, _include_archived: bool) -> Result<Vec<UiCodeTemplate>> {
        Ok(Vec::new())
    }

    async fn get_ui_code_template(&self, _template_id: Uuid) -> Result<Option<UiCodeTemplate>> {
        Ok(None)
    }

    async fn create_ui_code_template(
        &self,
        _input: &CreateUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }

    async fn revise_ui_code_template(
        &self,
        _input: &ReviseUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }

    async fn publish_ui_code_template_revision(
        &self,
        _template_id: Uuid,
        _revision: i32,
        _actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }

    async fn set_ui_code_template_default(
        &self,
        _template_id: Uuid,
        _actor_user_id: Uuid,
    ) -> Result<()> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }

    async fn reset_ui_code_template_default(
        &self,
        _provider_code: &str,
        _contribution_code: &str,
    ) -> Result<()> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }

    async fn set_ui_code_template_archived(
        &self,
        _template_id: Uuid,
        _archived: bool,
        _actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }

    async fn list_ui_component_overrides(&self) -> Result<Vec<UiComponentOverride>> {
        Ok(self.overrides.clone())
    }

    async fn get_ui_component_override(
        &self,
        locator: &UiComponentLocator,
    ) -> Result<Option<UiComponentOverride>> {
        Ok(self
            .overrides
            .iter()
            .find(|override_record| &override_record.locator == locator)
            .cloned())
    }

    async fn revise_ui_component_contract(
        &self,
        _input: &ReviseUiComponentContractInput,
    ) -> Result<UiComponentOverride> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }

    async fn set_ui_component_state(
        &self,
        _locator: &UiComponentLocator,
        _state: UiComponentOverrideState,
        _actor_user_id: Uuid,
    ) -> Result<UiComponentOverride> {
        Err(anyhow::anyhow!("not used by component catalog tests"))
    }
}

fn sample_block(installation_id: Uuid) -> FrontendBlockCatalogEntry {
    FrontendBlockCatalogEntry {
        installation_id,
        provider_code: "1flowbase".into(),
        plugin_id: "1flowbase@1.0.0".into(),
        plugin_version: "1.0.0".into(),
        contribution_code: "frontstage.js-ui-block".into(),
        title: "代码区块".into(),
        runtime: "native_react".into(),
        entry: "index.js".into(),
        code_template: None,
        code_template_version: None,
        code_template_language: None,
        code_modules: vec![
            FrontendBlockCodeModule {
                source: "@1flowbase/native-components".into(),
                version: "1.0.0".into(),
                exports: vec!["Button".into(), "Alert".into()],
                binding: FrontendModuleBinding::Fetched,
                assets: vec![FrontendModuleAsset {
                    path: "browser-assets/native-components.js".into(),
                    role: FrontendModuleAssetRole::BrowserModule,
                    media_type: "text/javascript; charset=utf-8".into(),
                    sha256: "00c568e229c81c4c18af20961ec14663efa6f7460c0134708391746d7e8ec2e0".into(),
                }],
                type_declarations: "declare module '@1flowbase/native-components' {}".into(),
                components: vec![
                    sample_component("button", "Button"),
                    sample_component("alert", "Alert"),
                ],
            },
            FrontendBlockCodeModule {
                source: "@1flowbase/runtime-utils".into(),
                version: "1.0.0".into(),
                exports: vec!["useRuntimeValue".into()],
                binding: FrontendModuleBinding::Host,
                assets: vec![],
                type_declarations: "declare module '@1flowbase/runtime-utils' { export function useRuntimeValue(): string; }".into(),
                components: vec![],
            },
        ],
        context_contract: FrontendBlockContextContract {
            primitives: vec![],
            input_schema: serde_json::json!({ "type": "object" }),
        },
        permissions: FrontendBlockPermissions {
            network: "none".into(),
            storage: "none".into(),
            secrets: "none".into(),
        },
        ui_capabilities: vec![],
    }
}

fn sample_component(component_code: &str, export_name: &str) -> FrontendComponentContract {
    FrontendComponentContract {
        component_code: component_code.into(),
        export_name: export_name.into(),
        upstream: None,
        description: format!("{export_name} API contract"),
        props: vec![],
        limitations: vec!["仅支持已声明参数".into()],
        examples: vec![FrontendComponentExample {
            title: "基础用法".into(),
            code: format!("<{export_name}></{export_name}>"),
        }],
        insert_snippet: format!("<{export_name}></{export_name}>"),
    }
}

#[tokio::test]
async fn d2_ac_001_lists_filters_and_pages_registered_native_components() {
    let installation_id = Uuid::now_v7();
    let service = FrontendComponentCatalogService::new(
        MemoryFrontendComponentCatalog {
            entries: vec![sample_block(installation_id)],
            overrides: vec![],
        },
        "test-node",
    );

    let page = service
        .list_component_capabilities(ListFrontendComponentCapabilitiesQuery {
            workspace_id: Uuid::now_v7(),
            installation_id: Some(installation_id),
            contribution_code: Some("frontstage.js-ui-block".into()),
            query: Some("button".into()),
            module_source: Some("@1flowbase/native-components".into()),
            offset: 0,
            limit: 1,
        })
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].export_name, "Button");
    assert_eq!(page.items[0].contract.export_name, "Button");
    assert_eq!(page.items[0].exports, vec!["Button", "Alert"]);
    assert!(!page.has_more);
    assert_eq!(page.next_offset, None);
    assert_eq!(page.module_sources, vec!["@1flowbase/native-components"]);

    let detail = service
        .get_component_capability(GetFrontendComponentCapabilityQuery {
            workspace_id: Uuid::now_v7(),
            component_id: page.items[0].component_id.clone(),
        })
        .await
        .unwrap()
        .expect("the paged component must be addressable by id");
    assert_eq!(detail.contract.insert_snippet, "<Button></Button>");
}

#[tokio::test]
async fn ac_005_hidden_and_published_overlays_change_discovery_without_modules() {
    let installation_id = Uuid::now_v7();
    let now = time::OffsetDateTime::now_utc();
    let actor = Uuid::now_v7();
    let hidden_id = Uuid::now_v7();
    let published_id = Uuid::now_v7();
    let published_contract = sample_component("button-managed", "Button");
    let service = FrontendComponentCatalogService::new(
        MemoryFrontendComponentCatalog {
            entries: vec![sample_block(installation_id)],
            overrides: vec![
                UiComponentOverride {
                    id: hidden_id,
                    scope_id: SYSTEM_SCOPE_ID,
                    locator: UiComponentLocator {
                        provider_code: "1flowbase".into(),
                        contribution_code: "frontstage.js-ui-block".into(),
                        module_source: "@1flowbase/native-components".into(),
                        export_name: "Alert".into(),
                    },
                    state: UiComponentOverrideState::Hidden,
                    latest_revision: None,
                    published_revision: None,
                    created_by: actor,
                    updated_by: actor,
                    created_at: now,
                    updated_at: now,
                },
                UiComponentOverride {
                    id: published_id,
                    scope_id: SYSTEM_SCOPE_ID,
                    locator: UiComponentLocator {
                        provider_code: "1flowbase".into(),
                        contribution_code: "frontstage.js-ui-block".into(),
                        module_source: "@1flowbase/native-components".into(),
                        export_name: "Button".into(),
                    },
                    state: UiComponentOverrideState::Published,
                    latest_revision: None,
                    published_revision: Some(UiComponentContractRevision {
                        id: Uuid::now_v7(),
                        component_override_id: published_id,
                        revision: 2,
                        contract: published_contract,
                        is_latest: false,
                        is_published: true,
                        created_by: actor,
                        created_at: now,
                    }),
                    created_by: actor,
                    updated_by: actor,
                    created_at: now,
                    updated_at: now,
                },
            ],
        },
        "test-node",
    );

    let page = service
        .list_component_capabilities(ListFrontendComponentCapabilitiesQuery {
            workspace_id: Uuid::now_v7(),
            installation_id: None,
            contribution_code: None,
            query: None,
            module_source: None,
            offset: 0,
            limit: 20,
        })
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].export_name, "Button");
    assert_eq!(page.items[0].contract.component_code, "button-managed");
    assert_eq!(page.items[0].exports, vec!["Button", "Alert"]);
}
