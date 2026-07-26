use anyhow::Result;
use async_trait::async_trait;
use control_plane::{
    frontend_block_catalog::{
        FrontendComponentCatalogService, GetFrontendComponentCapabilityQuery,
        ListFrontendComponentCapabilitiesQuery,
    },
    ports::{FrontendBlockCatalogRepository, ReplaceInstallationFrontendBlocksInput},
};
use domain::{
    FrontendBlockCatalogEntry, FrontendBlockCodeModule, FrontendBlockContextContract,
    FrontendBlockPermissions, FrontendComponentContract, FrontendComponentExample,
    FrontendModuleBrowserAsset,
};
use uuid::Uuid;

#[derive(Clone)]
struct MemoryFrontendComponentCatalog {
    entries: Vec<FrontendBlockCatalogEntry>,
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
        _workspace_id: Uuid,
    ) -> Result<Vec<FrontendBlockCatalogEntry>> {
        Ok(self.entries.clone())
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
        code_modules: vec![FrontendBlockCodeModule {
            source: "@1flowbase/native-components".into(),
            version: "1.0.0".into(),
            browser_asset: FrontendModuleBrowserAsset {
                path: "browser-assets/native-components.js".into(),
                sha256: "00c568e229c81c4c18af20961ec14663efa6f7460c0134708391746d7e8ec2e0".into(),
            },
            type_declarations: "declare module '@1flowbase/native-components' {}".into(),
            components: vec![
                sample_component("button", "Button"),
                sample_component("alert", "Alert"),
            ],
        }],
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
    let service = FrontendComponentCatalogService::new(MemoryFrontendComponentCatalog {
        entries: vec![sample_block(installation_id)],
    });

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
    assert_eq!(page.items[0].contract.export_name, "Button");
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
