//! Template projection through the native, root-authorized catalog management owner.
use std::collections::BTreeSet;

use anyhow::Result;
use domain::WorkspaceCatalogRevision;

use crate::{
    errors::ControlPlaneError,
    i18n_catalog::management::{
        CatalogManagementAccess, I18nCatalogManagementService, ListCatalogEntriesCommand,
    },
    ports::{CatalogManagementEntry, I18nCatalogManagementRepository},
};
use control_plane_contracts::portable_template::{PortableI18nCatalogItem, PortableI18nEntry};

pub struct I18nTemplateSnapshot {
    pub revision: WorkspaceCatalogRevision,
    pub entries: Vec<CatalogManagementEntry>,
}

/// Reject a changing catalog rather than mixing revisions across paged reads.
pub async fn snapshot<R: I18nCatalogManagementRepository>(
    service: &I18nCatalogManagementService<R>,
    access: &CatalogManagementAccess,
) -> Result<I18nTemplateSnapshot> {
    let mut entries = Vec::new();
    let mut revision = None;
    let mut expected_total = None;
    loop {
        let offset = u32::try_from(entries.len())?;
        let page = service
            .list(ListCatalogEntriesCommand {
                access: access.clone(),
                key: None,
                locale: None,
                search: None,
                origin: None,
                offset,
                limit: 200,
            })
            .await?;
        if revision.is_some_and(|value| value != page.revision)
            || expected_total.is_some_and(|total| total != page.total)
            || page
                .entries
                .iter()
                .any(|entry| entry.revision != page.revision)
        {
            return Err(ControlPlaneError::Conflict("template_i18n_catalog_revision").into());
        }
        revision = Some(page.revision);
        expected_total = Some(page.total);
        let empty = page.entries.is_empty();
        entries.extend(page.entries);
        if entries.len() as u64 >= page.total {
            return Ok(I18nTemplateSnapshot {
                revision: page.revision,
                entries,
            });
        }
        if empty {
            return Err(ControlPlaneError::Conflict("template_i18n_catalog_pagination").into());
        }
    }
}

/// Never use effective_value: it can be a fallback rather than a stored translation.
pub fn current_translation(entry: &CatalogManagementEntry) -> Option<&str> {
    entry
        .override_translation
        .as_deref()
        .or(entry.custom_translation.as_deref())
        .or(entry.official_translation.as_deref())
}

pub fn catalog(snapshot: &I18nTemplateSnapshot) -> Vec<PortableI18nCatalogItem> {
    snapshot
        .entries
        .iter()
        .filter(|entry| current_translation(entry).is_some())
        .map(|entry| entry.key.clone())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .map(|key| PortableI18nCatalogItem { key })
        .collect()
}

pub fn export_selected(
    snapshot: &I18nTemplateSnapshot,
    keys: &[String],
) -> Result<Vec<PortableI18nEntry>> {
    let selected: BTreeSet<_> = keys.iter().map(String::as_str).collect();
    let mut exported = std::collections::BTreeMap::new();
    for entry in &snapshot.entries {
        if !selected.contains(entry.key.as_str()) {
            continue;
        }
        if let Some(translation) = current_translation(entry) {
            let value = PortableI18nEntry {
                key: entry.key.clone(),
                locale: entry.locale.as_str().to_owned(),
                translation: translation.to_owned(),
            };
            let id = (value.key.clone(), value.locale.clone());
            if exported.get(&id).is_some_and(|previous| previous != &value) {
                return Err(
                    ControlPlaneError::Conflict("template_i18n_duplicate_translation").into(),
                );
            }
            exported.insert(id, value);
        }
    }
    let found: BTreeSet<_> = exported.keys().map(|(key, _)| key.as_str()).collect();
    if selected.iter().any(|key| !found.contains(key)) {
        return Err(ControlPlaneError::NotFound("template_i18n_selected_key").into());
    }
    Ok(exported.into_values().collect())
}

#[cfg(test)]
#[path = "_tests/i18n.rs"]
mod tests;
