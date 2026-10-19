use std::sync::{Arc, Mutex};

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use domain::{UiComponentRecord, UiComponentRecordOrigin, SYSTEM_SCOPE_ID};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    ports::{
        OfficialUiComponentCatalogRecord, UiComponentCatalogIndex, UiComponentCatalogPage,
        UiComponentCatalogRepository, UiComponentCatalogSearchResult, UiComponentCatalogSeed,
        UiComponentCatalogSource,
    },
    ui_component_catalog::{UiComponentBootstrapOutcome, UiComponentCatalogService},
};

#[derive(Clone)]
struct FixtureSource {
    seed: Result<UiComponentCatalogSeed, String>,
}

type RecordedSourceGroupReplacements =
    Arc<Mutex<Vec<(String, String, Vec<OfficialUiComponentCatalogRecord>)>>>;

#[async_trait]
impl UiComponentCatalogSource for FixtureSource {
    async fn index(&self) -> Result<UiComponentCatalogIndex> {
        Err(anyhow!("unused fixture method"))
    }

    async fn page(&self, _page: u32) -> Result<UiComponentCatalogPage> {
        let seed = self.seed.clone().map_err(|message| anyhow!(message))?;
        Ok(UiComponentCatalogPage {
            catalog_version: seed.catalog_version,
            total_components: seed.records.len(),
            page_size: 100,
            page: 1,
            cursor: "start".into(),
            next_cursor: None,
            records: seed.records,
        })
    }

    async fn search(
        &self,
        _query: &str,
        _page: u32,
        _page_size: usize,
    ) -> Result<UiComponentCatalogSearchResult> {
        let seed = self.seed.clone().map_err(|message| anyhow!(message))?;
        Ok(UiComponentCatalogSearchResult {
            catalog_version: seed.catalog_version,
            page: 1,
            page_size: 20,
            total_entries: seed.records.len(),
            entries: seed
                .records
                .into_iter()
                .map(|record| crate::ports::UiComponentCatalogSearchEntry {
                    component_code: record.component_code,
                    name: record.name,
                    description: record.description,
                    source: record.source,
                    group: record.group,
                    upstream: record.upstream,
                    version: record.version,
                    keywords: record.keywords,
                    catalog_page: 1,
                })
                .collect(),
        })
    }

    async fn seed(&self) -> Result<UiComponentCatalogSeed> {
        self.seed.clone().map_err(|message| anyhow!(message))
    }
}

#[derive(Clone)]
struct RecordingRepository {
    count: usize,
    official_records: Vec<UiComponentRecord>,
    replacements: RecordedSourceGroupReplacements,
    catalog_replacements: Arc<Mutex<Vec<Vec<OfficialUiComponentCatalogRecord>>>>,
    downloads: Arc<Mutex<Vec<OfficialUiComponentCatalogRecord>>>,
}

impl RecordingRepository {
    fn empty(count: usize) -> Self {
        Self {
            count,
            official_records: Vec::new(),
            replacements: Arc::default(),
            catalog_replacements: Arc::default(),
            downloads: Arc::default(),
        }
    }

    fn with_official_records(records: Vec<UiComponentRecord>) -> Self {
        Self {
            count: records.len(),
            official_records: records,
            replacements: Arc::default(),
            catalog_replacements: Arc::default(),
            downloads: Arc::default(),
        }
    }
}

#[async_trait]
impl UiComponentCatalogRepository for RecordingRepository {
    async fn count_ui_component_records(&self) -> Result<usize> {
        Ok(self.count)
    }

    async fn list_official_ui_component_records(&self) -> Result<Vec<domain::UiComponentRecord>> {
        Ok(self.official_records.clone())
    }

    async fn upsert_official_ui_component_record(
        &self,
        record: &OfficialUiComponentCatalogRecord,
        _actor_user_id: Uuid,
    ) -> Result<()> {
        self.downloads.lock().unwrap().push(record.clone());
        Ok(())
    }

    async fn replace_official_ui_component_source_group(
        &self,
        source: &str,
        group: &str,
        records: &[OfficialUiComponentCatalogRecord],
        _actor_user_id: Uuid,
    ) -> Result<()> {
        self.replacements.lock().unwrap().push((
            source.to_owned(),
            group.to_owned(),
            records.to_vec(),
        ));
        Ok(())
    }

    async fn replace_official_ui_component_catalog_groups(
        &self,
        records: &[OfficialUiComponentCatalogRecord],
        _actor_user_id: Uuid,
    ) -> Result<bool> {
        self.catalog_replacements
            .lock()
            .unwrap()
            .push(records.to_vec());
        Ok(true)
    }
}

fn opaque_record() -> OfficialUiComponentCatalogRecord {
    OfficialUiComponentCatalogRecord {
        component_code: "taichuy.missing-package.widget".into(),
        name: "Missing package widget".into(),
        description: "Opaque source fixture".into(),
        import_code: "import Widget from '@definitely/not-installed';".into(),
        source_code: "<Widget impossible={{ syntax: true }} />".into(),
        source: "taichuy".into(),
        group: "missing-package".into(),
        upstream: domain::UiComponentRecordUpstream {
            identity: "@definitely/not-installed".into(),
            version: "99.0.0".into(),
        },
        version: "1.0.0".into(),
        keywords: vec!["opaque".into()],
        catalog_updated_at: OffsetDateTime::parse(
            "2026-08-23T00:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap(),
        source_locator: "ui_components/@taichuy/missing-package/widget.json".into(),
        source_checksum: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .into(),
    }
}

fn seed() -> UiComponentCatalogSeed {
    UiComponentCatalogSeed {
        catalog_version: "1.0.0".into(),
        source_fingerprint:
            "sha256:bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb".into(),
        records: vec![opaque_record()],
    }
}

fn local_record(version: &str) -> UiComponentRecord {
    let timestamp = OffsetDateTime::UNIX_EPOCH;
    UiComponentRecord {
        id: Uuid::now_v7(),
        scope_id: SYSTEM_SCOPE_ID,
        component_code: "taichuy.missing-package.widget".into(),
        name: "Missing package widget".into(),
        description: "Local catalog record".into(),
        import_code: "opaque import".into(),
        source_code: "opaque source".into(),
        origin: UiComponentRecordOrigin::Official,
        source: "taichuy".into(),
        group: "missing-package".into(),
        upstream: domain::UiComponentRecordUpstream {
            identity: "@definitely/not-installed".into(),
            version: "99.0.0".into(),
        },
        version: version.into(),
        keywords: Vec::new(),
        catalog_updated_at: Some(timestamp),
        source_locator: Some("ui_components/local.json".into()),
        source_checksum: Some("sha256:local".into()),
        created_by: Uuid::nil(),
        updated_by: Uuid::nil(),
        created_at: timestamp,
        updated_at: timestamp,
    }
}

#[tokio::test]
async fn catalog_page_and_search_project_local_versions_by_component_code() {
    let service = UiComponentCatalogService::new(
        RecordingRepository::with_official_records(vec![local_record("0.9.0")]),
        FixtureSource { seed: Ok(seed()) },
    );

    let page = service.page(1).await.unwrap();
    let search = service.search("widget", 1, 20).await.unwrap();

    assert_eq!(page.records[0].local_version.as_deref(), Some("0.9.0"));
    assert_eq!(search.entries[0].local_version.as_deref(), Some("0.9.0"));

    let missing_service = UiComponentCatalogService::new(
        RecordingRepository::empty(0),
        FixtureSource { seed: Ok(seed()) },
    );
    assert_eq!(
        missing_service.page(1).await.unwrap().records[0].local_version,
        None
    );
}

#[tokio::test]
async fn wp_d3_manual_source_failure_is_observable_and_performs_no_writes() {
    let repository = RecordingRepository::empty(0);
    let service = UiComponentCatalogService::new(
        repository.clone(),
        FixtureSource {
            seed: Err("catalog checksum mismatch".into()),
        },
    );

    let error = service
        .sync_source_group("taichuy", "missing-package", Uuid::now_v7())
        .await
        .unwrap_err();

    assert!(error.to_string().contains("catalog checksum mismatch"));
    assert!(repository.replacements.lock().unwrap().is_empty());
}

#[tokio::test]
async fn wp_d3_nonexistent_package_import_and_source_remain_opaque_during_download_and_sync() {
    let repository = RecordingRepository::empty(0);
    let service =
        UiComponentCatalogService::new(repository.clone(), FixtureSource { seed: Ok(seed()) });

    service
        .download_component("taichuy.missing-package.widget", Uuid::now_v7())
        .await
        .unwrap();
    service
        .sync_source_group("taichuy", "missing-package", Uuid::now_v7())
        .await
        .unwrap();

    let downloaded = repository.downloads.lock().unwrap();
    assert_eq!(
        downloaded[0].import_code,
        "import Widget from '@definitely/not-installed';"
    );
    assert_eq!(
        downloaded[0].source_code,
        "<Widget impossible={{ syntax: true }} />"
    );
    assert_eq!(
        repository.replacements.lock().unwrap()[0].2,
        vec![opaque_record()]
    );
}

#[tokio::test]
async fn wp_d3_empty_system_bootstraps_once_while_non_empty_system_skips_source_access() {
    let empty_repository = RecordingRepository::empty(0);
    let empty_service = UiComponentCatalogService::new(
        empty_repository.clone(),
        FixtureSource { seed: Ok(seed()) },
    );
    assert_eq!(
        empty_service
            .bootstrap_empty_system(Uuid::now_v7())
            .await
            .unwrap(),
        UiComponentBootstrapOutcome::Imported { records: 1 }
    );
    assert_eq!(
        empty_repository.catalog_replacements.lock().unwrap().len(),
        1
    );

    let non_empty_repository = RecordingRepository::empty(1);
    let non_empty_service = UiComponentCatalogService::new(
        non_empty_repository.clone(),
        FixtureSource {
            seed: Err("source must not be accessed".into()),
        },
    );
    assert_eq!(
        non_empty_service
            .bootstrap_empty_system(Uuid::now_v7())
            .await
            .unwrap(),
        UiComponentBootstrapOutcome::SkippedNonEmpty
    );
    assert!(non_empty_repository
        .catalog_replacements
        .lock()
        .unwrap()
        .is_empty());
}

#[tokio::test]
async fn wp_d3_empty_system_bootstrap_failure_leaves_repository_untouched() {
    let repository = RecordingRepository::empty(0);
    let service = UiComponentCatalogService::new(
        repository.clone(),
        FixtureSource {
            seed: Err("network unavailable".into()),
        },
    );

    assert!(service
        .bootstrap_empty_system(Uuid::now_v7())
        .await
        .is_err());
    assert!(repository.catalog_replacements.lock().unwrap().is_empty());
}
