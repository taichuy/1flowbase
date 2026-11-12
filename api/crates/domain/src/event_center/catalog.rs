use super::EventCatalogEntry;
use serde::{Deserialize, Serialize};

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventCatalogSortBy {
    CreatedAt,
    #[default]
    UpdatedAt,
}

/// Complete source-owned declarations for the listed scopes. Custom rules have their own
/// transactional projection and must never be overwritten by a declaration reconciliation.
#[derive(Debug, Clone, Default)]
pub struct EventCatalogSnapshot {
    pub scopes: Vec<super::EventSourceScope>,
    pub entries: Vec<EventCatalogEntry>,
}

impl EventCatalogSnapshot {
    pub fn validate(&self) -> Result<(), &'static str> {
        let mut identities = std::collections::BTreeSet::new();
        for entry in &self.entries {
            if entry.origin == super::EventDefinitionOrigin::Custom || entry.id.is_some() {
                return Err("event_catalog_custom_declaration");
            }
            if !self.scopes.contains(&entry.scope)
                || entry
                    .source
                    .as_ref()
                    .is_some_and(|source| source.selection.scope != entry.scope)
            {
                return Err("event_catalog_declaration_scope");
            }
            let identity = serde_json::to_string(&(
                &entry.scope,
                &entry.contract_id,
                entry
                    .source
                    .as_ref()
                    .map(|source| &source.selection.source_id),
                entry
                    .source
                    .as_ref()
                    .map(|source| &source.selection.filters),
            ))
            .map_err(|_| "event_catalog_identity")?;
            if !identities.insert(identity) {
                return Err("event_catalog_duplicate_identity");
            }
        }
        Ok(())
    }
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, utoipa::ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum EventCatalogSortOrder {
    Asc,
    #[default]
    Desc,
}

#[derive(Debug, Default, Clone, Copy, Deserialize, Serialize, utoipa::IntoParams)]
#[into_params(parameter_in = Query)]
pub struct EventCatalogQuery {
    #[serde(default)]
    pub sort_by: EventCatalogSortBy,
    #[serde(default)]
    pub sort_order: EventCatalogSortOrder,
}

impl EventCatalogQuery {
    /// Sort the complete visible catalog, preserving declaration order for equal or absent times.
    pub fn sort(self, entries: &mut [EventCatalogEntry]) {
        entries.sort_by(|left, right| {
            let (left, right) = match self.sort_by {
                EventCatalogSortBy::CreatedAt => (left.created_at, right.created_at),
                EventCatalogSortBy::UpdatedAt => (left.updated_at, right.updated_at),
            };
            match (left, right) {
                (Some(left), Some(right)) => match self.sort_order {
                    EventCatalogSortOrder::Asc => left.cmp(&right),
                    EventCatalogSortOrder::Desc => right.cmp(&left),
                },
                (Some(_), None) => std::cmp::Ordering::Less,
                (None, Some(_)) => std::cmp::Ordering::Greater,
                (None, None) => std::cmp::Ordering::Equal,
            }
        });
    }
}
