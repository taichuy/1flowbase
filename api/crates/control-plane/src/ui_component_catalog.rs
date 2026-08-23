use std::collections::{BTreeMap, BTreeSet};

use anyhow::{bail, Result};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::ports::{
    OfficialUiComponentCatalogRecord, UiComponentCatalogIndex, UiComponentCatalogPage,
    UiComponentCatalogRepository, UiComponentCatalogSearchResult, UiComponentCatalogSource,
};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentCatalogGroupUpdate {
    pub source: String,
    pub group: String,
    pub remote_records: usize,
    pub new_or_updated_records: usize,
    pub removed_records: usize,
}

impl UiComponentCatalogGroupUpdate {
    pub fn update_available(&self) -> bool {
        self.new_or_updated_records > 0 || self.removed_records > 0
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct UiComponentCatalogUpdateStatus {
    pub catalog_version: String,
    pub source_fingerprint: String,
    pub update_available: bool,
    pub groups: Vec<UiComponentCatalogGroupUpdate>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiComponentBootstrapOutcome {
    SkippedNonEmpty,
    Imported { records: usize },
}

pub struct UiComponentCatalogService<R, S> {
    repository: R,
    source: S,
}

impl<R, S> UiComponentCatalogService<R, S>
where
    R: UiComponentCatalogRepository,
    S: UiComponentCatalogSource,
{
    pub fn new(repository: R, source: S) -> Self {
        Self { repository, source }
    }

    pub async fn index(&self) -> Result<UiComponentCatalogIndex> {
        self.source.index().await
    }

    pub async fn page(&self, page: u32) -> Result<UiComponentCatalogPage> {
        self.source.page(page).await
    }

    pub async fn search(
        &self,
        query: &str,
        page: u32,
        page_size: usize,
    ) -> Result<UiComponentCatalogSearchResult> {
        if page == 0 || !(1..=100).contains(&page_size) {
            bail!("catalog search page and page_size are invalid");
        }
        self.source.search(query, page, page_size).await
    }

    pub async fn update_status(&self) -> Result<UiComponentCatalogUpdateStatus> {
        let seed = self.source.seed().await?;
        let local = self.repository.list_official_ui_component_records().await?;
        let mut remote_groups = BTreeMap::<(String, String), Vec<_>>::new();
        for record in &seed.records {
            remote_groups
                .entry((record.source.clone(), record.group.clone()))
                .or_default()
                .push(record);
        }
        let mut local_groups = BTreeMap::<(String, String), Vec<_>>::new();
        for record in &local {
            local_groups
                .entry((record.source.clone(), record.group.clone()))
                .or_default()
                .push(record);
        }
        let keys = remote_groups
            .keys()
            .chain(local_groups.keys())
            .cloned()
            .collect::<BTreeSet<_>>();
        let groups = keys
            .into_iter()
            .map(|(source, group)| {
                let remote = remote_groups
                    .get(&(source.clone(), group.clone()))
                    .cloned()
                    .unwrap_or_default();
                let local = local_groups
                    .get(&(source.clone(), group.clone()))
                    .cloned()
                    .unwrap_or_default();
                let local_by_code = local
                    .iter()
                    .map(|record| {
                        (
                            record.component_code.as_str(),
                            record.source_checksum.as_deref(),
                        )
                    })
                    .collect::<BTreeMap<_, _>>();
                let remote_codes = remote
                    .iter()
                    .map(|record| record.component_code.as_str())
                    .collect::<BTreeSet<_>>();
                UiComponentCatalogGroupUpdate {
                    source,
                    group,
                    remote_records: remote.len(),
                    new_or_updated_records: remote
                        .iter()
                        .filter(|record| {
                            local_by_code
                                .get(record.component_code.as_str())
                                .copied()
                                .flatten()
                                != Some(record.source_checksum.as_str())
                        })
                        .count(),
                    removed_records: local
                        .iter()
                        .filter(|record| !remote_codes.contains(record.component_code.as_str()))
                        .count(),
                }
            })
            .collect::<Vec<_>>();
        Ok(UiComponentCatalogUpdateStatus {
            catalog_version: seed.catalog_version,
            source_fingerprint: seed.source_fingerprint,
            update_available: groups
                .iter()
                .any(UiComponentCatalogGroupUpdate::update_available),
            groups,
        })
    }

    pub async fn download_component(
        &self,
        component_code: &str,
        actor_user_id: Uuid,
    ) -> Result<OfficialUiComponentCatalogRecord> {
        let seed = self.source.seed().await?;
        let record = seed
            .records
            .into_iter()
            .find(|record| record.component_code == component_code)
            .ok_or_else(|| anyhow::anyhow!("catalog component not found"))?;
        self.repository
            .upsert_official_ui_component_record(&record, actor_user_id)
            .await?;
        Ok(record)
    }

    pub async fn sync_source_group(
        &self,
        source: &str,
        group: &str,
        actor_user_id: Uuid,
    ) -> Result<usize> {
        let seed = self.source.seed().await?;
        let records = seed
            .records
            .into_iter()
            .filter(|record| record.source == source && record.group == group)
            .collect::<Vec<_>>();
        if records.is_empty() {
            bail!("catalog source/group not found");
        }
        self.repository
            .replace_official_ui_component_source_group(source, group, &records, actor_user_id)
            .await?;
        Ok(records.len())
    }

    pub async fn bootstrap_empty_system(
        &self,
        actor_user_id: Uuid,
    ) -> Result<UiComponentBootstrapOutcome> {
        if self.repository.count_ui_component_records().await? != 0 {
            return Ok(UiComponentBootstrapOutcome::SkippedNonEmpty);
        }
        let seed = self.source.seed().await?;
        let count = seed.records.len();
        let imported = self
            .repository
            .replace_official_ui_component_catalog_groups(&seed.records, actor_user_id)
            .await?;
        if imported {
            Ok(UiComponentBootstrapOutcome::Imported { records: count })
        } else {
            Ok(UiComponentBootstrapOutcome::SkippedNonEmpty)
        }
    }
}
