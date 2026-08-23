use anyhow::Result;
use async_trait::async_trait;
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ports::{
        CreateUiCodeTemplateInput, CreateUiComponentRecordInput, FrontendBlockCatalogRepository,
        ReplaceInstallationFrontendBlocksInput, ReviseUiCodeTemplateInput, UiComponentRecordPatch,
        UiManagementRepository,
    },
    ui_management::{ListUiComponentRecordsQuery, UiManagementService},
};
use domain::{
    UiCodeTemplate, UiComponentRecord, UiComponentRecordOrigin, UiComponentRecordUpstream,
    SYSTEM_SCOPE_ID,
};

#[derive(Clone)]
struct MemoryPersistedComponentCatalog {
    records: Vec<UiComponentRecord>,
}

#[async_trait]
impl FrontendBlockCatalogRepository for MemoryPersistedComponentCatalog {
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
    ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
        panic!("persisted component catalog must not query workspace frontend blocks")
    }

    async fn list_system_frontend_blocks(
        &self,
        _node_id: &str,
    ) -> Result<Vec<domain::FrontendBlockCatalogEntry>> {
        panic!("persisted component catalog must not query system frontend blocks")
    }
}

#[async_trait]
impl UiManagementRepository for MemoryPersistedComponentCatalog {
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
        panic!("unused template method")
    }

    async fn revise_ui_code_template(
        &self,
        _input: &ReviseUiCodeTemplateInput,
    ) -> Result<UiCodeTemplate> {
        panic!("unused template method")
    }

    async fn publish_ui_code_template_revision(
        &self,
        _template_id: Uuid,
        _revision: i32,
        _actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        panic!("unused template method")
    }

    async fn set_ui_code_template_default(
        &self,
        _template_id: Uuid,
        _actor_user_id: Uuid,
    ) -> Result<()> {
        panic!("unused template method")
    }

    async fn reset_ui_code_template_default(
        &self,
        _provider_code: &str,
        _contribution_code: &str,
    ) -> Result<()> {
        panic!("unused template method")
    }

    async fn set_ui_code_template_archived(
        &self,
        _template_id: Uuid,
        _archived: bool,
        _actor_user_id: Uuid,
    ) -> Result<UiCodeTemplate> {
        panic!("unused template method")
    }

    async fn list_ui_component_records(&self) -> Result<Vec<UiComponentRecord>> {
        Ok(self.records.clone())
    }

    async fn get_ui_component_record(&self, id: Uuid) -> Result<Option<UiComponentRecord>> {
        Ok(self.records.iter().find(|record| record.id == id).cloned())
    }

    async fn create_ui_component_record(
        &self,
        _input: &CreateUiComponentRecordInput,
    ) -> Result<UiComponentRecord> {
        panic!("unused component write method")
    }

    async fn update_ui_component_record(
        &self,
        _id: Uuid,
        _patch: &UiComponentRecordPatch,
    ) -> Result<UiComponentRecord> {
        panic!("unused component write method")
    }

    async fn delete_ui_component_record(&self, _id: Uuid) -> Result<bool> {
        panic!("unused component write method")
    }
}

fn opaque_record() -> UiComponentRecord {
    let timestamp = OffsetDateTime::UNIX_EPOCH;
    UiComponentRecord {
        id: Uuid::now_v7(),
        scope_id: SYSTEM_SCOPE_ID,
        component_code: "taichuy.opaque.widget".into(),
        name: "Opaque Widget".into(),
        description: "Persisted without dependency availability".into(),
        import_code: "import Widget from '@definitely/not-installed';".into(),
        source_code: "<Widget impossible={{ syntax: true }} />".into(),
        origin: UiComponentRecordOrigin::Official,
        source: "taichuy".into(),
        group: "opaque".into(),
        upstream: UiComponentRecordUpstream {
            identity: "@definitely/not-installed".into(),
            version: "99.0.0".into(),
        },
        version: "1.0.0".into(),
        keywords: vec!["opaque".into()],
        catalog_updated_at: Some(timestamp),
        source_locator: Some("ui_components/opaque.json".into()),
        source_checksum: Some(
            "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa".into(),
        ),
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
        created_at: timestamp,
        updated_at: timestamp,
    }
}

#[tokio::test]
async fn wp_d4_persisted_record_is_listed_and_gettable_without_module_or_export_availability() {
    let record = opaque_record();
    let service = UiManagementService::new(
        MemoryPersistedComponentCatalog {
            records: vec![record.clone()],
        },
        "test-node",
    );

    let page = service
        .list_component_records_page(ListUiComponentRecordsQuery {
            query: Some("not-installed".into()),
            offset: 0,
            limit: 20,
        })
        .await
        .unwrap();

    assert_eq!(page.total, 1);
    assert_eq!(page.items[0].import_code, record.import_code);
    assert_eq!(page.items[0].source_code, record.source_code);
    assert_eq!(
        service.get_component_record(record.id).await.unwrap(),
        record
    );
}

#[tokio::test]
async fn wp_d4_empty_persistence_yields_empty_catalog_without_inferred_exports() {
    let service = UiManagementService::new(
        MemoryPersistedComponentCatalog { records: vec![] },
        "test-node",
    );

    let page = service
        .list_component_records_page(ListUiComponentRecordsQuery {
            query: Some("hooks message theme version icons".into()),
            offset: 0,
            limit: 20,
        })
        .await
        .unwrap();

    assert_eq!(page.total, 0);
    assert!(page.items.is_empty());
    assert!(!page.has_more);
    assert_eq!(page.next_offset, None);
}
