use super::*;

#[async_trait]
impl OfficialExtensionCatalogSourcePort for ApiOfficialExtensionCatalogSource {
    async fn search(
        &self,
        category: &str,
        query: OfficialExtensionCatalogSearchQuery,
    ) -> Result<OfficialExtensionCatalogSearchResult> {
        if query.limit == 0 || query.limit > 100 {
            bail!("official extension catalog search limit must be between 1 and 100");
        }
        let previous_snapshot = self.cached_search_snapshot(category);
        let mut failures = Vec::new();
        for endpoint in self.endpoints(category)? {
            let (index, snapshot) = match self.load_search_snapshot(category, &endpoint).await {
                Ok(value) => value,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            let selection = match select_search_entries(category, &snapshot, &query) {
                Ok(selection) => selection,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            let entries = match self
                .fetch_selected_search_entries(category, &index, &snapshot, &selection.selected)
                .await
            {
                Ok(entries) => entries,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            self.remember_search_snapshot(category, &snapshot);
            return Ok(OfficialExtensionCatalogSearchResult {
                source_kind: snapshot.source_kind.clone(),
                category: category.to_string(),
                freshness: OfficialExtensionCatalogFreshness::Fresh,
                snapshot_checksum: snapshot.checksum.clone(),
                snapshot_locator: snapshot.locator.clone(),
                total_entries: selection.total_entries,
                next_cursor: selection.next_cursor,
                entries,
            });
        }
        if let Some(snapshot) = previous_snapshot {
            if let Ok(selection) = select_search_entries(category, &snapshot, &query) {
                if let Some(entries) =
                    self.cached_selected_search_entries(category, &selection.selected)
                {
                    return Ok(OfficialExtensionCatalogSearchResult {
                        source_kind: snapshot.source_kind.clone(),
                        category: category.to_string(),
                        freshness: OfficialExtensionCatalogFreshness::Stale,
                        snapshot_checksum: snapshot.checksum.clone(),
                        snapshot_locator: snapshot.locator.clone(),
                        total_entries: selection.total_entries,
                        next_cursor: selection.next_cursor,
                        entries,
                    });
                }
            }
        }
        let source = failures
            .into_iter()
            .next()
            .unwrap_or_else(|| anyhow!("official extension catalog has no configured source"));
        Err(anyhow::Error::new(
            OfficialExtensionCatalogUnavailable::new(source),
        ))
    }

    async fn search_for_workspace(
        &self,
        workspace_id: Uuid,
        category: &str,
        query: OfficialExtensionCatalogSearchQuery,
    ) -> Result<OfficialExtensionCatalogSearchResult> {
        let (source, lease) = self.source_for_workspace(workspace_id).await?;
        Self::finish_workspace_request(source.search(category, query).await, lease).await
    }

    async fn list_page(
        &self,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage> {
        let key = Self::cache_key(category, cursor);
        let mut failures = Vec::new();
        for endpoint in self.endpoints(category)? {
            match self.load_page(category, cursor, &endpoint).await {
                Ok(page) => {
                    self.remember_page(key.clone(), &page);
                    return Ok(page);
                }
                Err(error) => failures.push(error),
            }
        }
        if let Some(page) = self.stale_page(&key) {
            return Ok(page);
        }
        Err(failures
            .into_iter()
            .next()
            .unwrap_or_else(|| anyhow!("official extension catalog has no configured source")))
    }

    async fn list_page_for_workspace(
        &self,
        workspace_id: Uuid,
        category: &str,
        cursor: Option<&str>,
    ) -> Result<OfficialExtensionCatalogPage> {
        let (source, lease) = self.source_for_workspace(workspace_id).await?;
        Self::finish_workspace_request(source.list_page(category, cursor).await, lease).await
    }

    fn cached_verified_entries(
        &self,
        category: &str,
        slot_code: Option<&str>,
    ) -> Option<CachedOfficialExtensionCatalogEntries> {
        let snapshot = self.cached_search_snapshot(category)?;
        let normalized_slot = normalized_filter(slot_code);
        let entries = snapshot
            .document
            .entries
            .iter()
            .filter(|search_entry| {
                normalized_slot.as_ref().is_none_or(|slot| {
                    search_entry
                        .slot_codes
                        .iter()
                        .any(|candidate| candidate == slot)
                })
            })
            .filter_map(|search_entry| {
                self.cached_verified_page(category, &search_entry.catalog_page)?
                    .entries
                    .into_iter()
                    .find(|entry| entry.id == search_entry.id)
            })
            .collect();
        Some(CachedOfficialExtensionCatalogEntries {
            source_kind: snapshot.source_kind,
            snapshot_locator: snapshot.locator,
            entries,
        })
    }

    async fn find_entry(
        &self,
        category: &str,
        catalog_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        if let Ok(cache) = self.last_success.lock() {
            if let Some(page) = cache.values().find(|page| {
                page.category == category && page.entries.iter().any(|entry| entry.id == catalog_id)
            }) {
                if let Some(entry) = page
                    .entries
                    .iter()
                    .find(|entry| entry.id == catalog_id)
                    .cloned()
                {
                    return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                        source_kind: page.source_kind.clone(),
                        entry,
                    }));
                }
            }
        }
        if let Some(snapshot) = self.cached_search_snapshot(category) {
            if let Some(search_entry) = snapshot
                .document
                .entries
                .iter()
                .find(|entry| entry.id == catalog_id)
            {
                let page_reference = snapshot
                    .index
                    .pages
                    .iter()
                    .find(|reference| {
                        reference.page == search_entry.catalog_page.page
                            && reference.cursor == search_entry.catalog_page.cursor
                            && reference.checksum == search_entry.catalog_page.checksum
                            && reference.locator == search_entry.catalog_page.locator
                    })
                    .ok_or_else(|| {
                        anyhow!("catalog search page reference is not in the catalog index")
                    })?;
                let page = if let Some(page) =
                    self.cached_verified_page(category, &search_entry.catalog_page)
                {
                    page
                } else {
                    let page = self
                        .fetch_page(&snapshot.endpoint, &snapshot.index, page_reference)
                        .await?;
                    self.remember_page(
                        Self::cache_key(category, Some(&page.metadata.cursor)),
                        &page,
                    );
                    page
                };
                let entry = page
                    .entries
                    .into_iter()
                    .find(|entry| entry.id == catalog_id)
                    .ok_or_else(|| {
                        anyhow!("catalog search result is missing from its verified source page")
                    })?;
                return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                    source_kind: snapshot.source_kind,
                    entry,
                }));
            }
            return Ok(None);
        }
        let mut failures = Vec::new();
        for endpoint in self.endpoints(category)? {
            let index = match self.fetch_index(category, &endpoint).await {
                Ok(index) => index,
                Err(error) => {
                    failures.push(error);
                    continue;
                }
            };
            let mut failed = None;
            for page_reference in &index.pages {
                match self.fetch_page(&endpoint, &index, page_reference).await {
                    Ok(page) => {
                        self.remember_page(
                            Self::cache_key(category, Some(&page.metadata.cursor)),
                            &page,
                        );
                        if let Some(entry) = page
                            .entries
                            .into_iter()
                            .find(|entry| entry.id == catalog_id)
                        {
                            return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                                source_kind: endpoint.source_kind.clone(),
                                entry,
                            }));
                        }
                    }
                    Err(error) => {
                        failed = Some(error);
                        break;
                    }
                }
            }
            if let Some(error) = failed {
                failures.push(error);
                continue;
            }
            return Ok(None);
        }
        if !failures.is_empty() {
            if let Ok(cache) = self.last_success.lock() {
                if let Some(page) = cache.values().find(|page| {
                    page.category == category
                        && page.entries.iter().any(|entry| entry.id == catalog_id)
                }) {
                    if let Some(entry) = page
                        .entries
                        .iter()
                        .find(|entry| entry.id == catalog_id)
                        .cloned()
                    {
                        return Ok(Some(LocatedOfficialExtensionCatalogEntry {
                            source_kind: page.source_kind.clone(),
                            entry,
                        }));
                    }
                }
            }
            return Err(failures.remove(0));
        }
        Ok(None)
    }

    async fn find_entry_for_workspace(
        &self,
        workspace_id: Uuid,
        category: &str,
        catalog_id: &str,
    ) -> Result<Option<LocatedOfficialExtensionCatalogEntry>> {
        let (source, lease) = self.source_for_workspace(workspace_id).await?;
        Self::finish_workspace_request(source.find_entry(category, catalog_id).await, lease).await
    }

    fn resolve_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<OfficialExtensionArtifactDescriptor> {
        let source = self.source(&entry.category)?;
        resolve_artifact_descriptor(entry, source.github_proxy_url.as_deref())
    }

    async fn download_artifact(
        &self,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact> {
        let descriptor = self.resolve_artifact(entry)?;
        let artifact_bytes = self
            .download_artifact_bytes(&descriptor.locator, artifact_download_budget(entry)?)
            .await?;
        if let Some(expected_checksum) = descriptor.expected_checksum.as_deref() {
            if ensure_sha256(&artifact_bytes, expected_checksum).is_err() {
                return Err(anyhow::Error::new(
                    OfficialExtensionArtifactError::ChecksumMismatch,
                ));
            }
        }
        let file_name = descriptor
            .locator
            .split('?')
            .next()
            .and_then(|url| url.rsplit('/').next())
            .filter(|value| !value.is_empty())
            .unwrap_or("extension-artifact.bin")
            .to_string();
        Ok(DownloadedOfficialExtensionArtifact {
            descriptor,
            file_name,
            artifact_bytes,
        })
    }

    async fn download_artifact_for_workspace(
        &self,
        workspace_id: Uuid,
        entry: &OfficialExtensionCatalogEntry,
    ) -> Result<DownloadedOfficialExtensionArtifact> {
        let descriptor = self.resolve_artifact(entry)?;
        let artifact_bytes = self
            .download_artifact_bytes_for_workspace(
                workspace_id,
                &descriptor.locator,
                artifact_download_budget(entry)?,
            )
            .await?;
        if let Some(expected_checksum) = descriptor.expected_checksum.as_deref() {
            if ensure_sha256(&artifact_bytes, expected_checksum).is_err() {
                return Err(anyhow::Error::new(
                    OfficialExtensionArtifactError::ChecksumMismatch,
                ));
            }
        }
        let file_name = descriptor
            .locator
            .split('?')
            .next()
            .and_then(|url| url.rsplit('/').next())
            .filter(|value| !value.is_empty())
            .unwrap_or("extension-artifact.bin")
            .to_string();
        Ok(DownloadedOfficialExtensionArtifact {
            descriptor,
            file_name,
            artifact_bytes,
        })
    }
}

