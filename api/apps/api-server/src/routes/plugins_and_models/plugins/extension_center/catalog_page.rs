use control_plane::plugin_management::ExtensionCatalogCategory;
use uuid::Uuid;

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    official_extension_catalog::{
        OfficialExtensionCatalogFreshness, OfficialExtensionCatalogSearchQuery,
    },
};

use super::{
    builtin_mcp::{
        builtin_frontstage_catalog_entry, builtin_frontstage_matches_query,
        BUILTIN_FRONTSTAGE_CATALOG_ID, BUILTIN_FRONTSTAGE_CURSOR,
    },
    installed_catalog_joins, project_catalog_entry, ExtensionCatalogGatewayPageResponse,
    ExtensionCatalogGatewayQuery,
};

pub(super) async fn load_catalog_page(
    state: &ApiState,
    workspace_id: Uuid,
    category: ExtensionCatalogCategory,
    query: ExtensionCatalogGatewayQuery,
) -> Result<ExtensionCatalogGatewayPageResponse, ApiError> {
    let limit = query.limit.unwrap_or(50).clamp(1, 100);
    let builtin_cursor = query.cursor.as_deref() == Some(BUILTIN_FRONTSTAGE_CURSOR);
    let builtin_package = (category == ExtensionCatalogCategory::Mcp && !builtin_cursor)
        .then(crate::official_mcp_bundles::ApiOfficialMcpBundleRegistry::bundled_frontstage_assistant_package)
        .transpose()?;
    let builtin_matches = builtin_package.as_ref().is_some_and(|package| {
        builtin_frontstage_matches_query(package, query.q.as_deref(), query.slot_code.as_deref())
    });
    let remote_has_builtin = if builtin_matches {
        state
            .official_extension_catalog_source
            .find_entry(category.as_str(), BUILTIN_FRONTSTAGE_CATALOG_ID)
            .await?
            .is_some()
    } else {
        false
    };
    let remote_limit = if builtin_matches && limit > 1 {
        limit - 1
    } else {
        limit
    };
    let page = state
        .official_extension_catalog_source
        .search(
            category.as_str(),
            OfficialExtensionCatalogSearchQuery {
                slot_code: query.slot_code,
                q: query.q,
                limit: remote_limit,
                cursor: if builtin_cursor { None } else { query.cursor },
            },
        )
        .await?;
    let installed = installed_catalog_joins(state, category).await?;
    let trusted_key_ids = state
        .official_plugin_source
        .trusted_public_keys()
        .iter()
        .map(|key| key.key_id.clone())
        .collect::<Vec<_>>();
    let catalog_source = match page.source_kind.as_str() {
        "official_repository" => "official",
        _ => "mirror",
    };
    let mut entries = page
        .entries
        .into_iter()
        .map(|entry| project_catalog_entry(entry, catalog_source, &installed, &trusted_key_ids))
        .collect::<Vec<_>>();
    if category == ExtensionCatalogCategory::Mcp {
        entries.retain(|entry| entry.id != BUILTIN_FRONTSTAGE_CATALOG_ID);
    }
    let mut next_cursor = page.next_cursor;
    if builtin_matches {
        let builtin = builtin_frontstage_catalog_entry(state, workspace_id, &installed).await?;
        if limit == 1 {
            entries.clear();
            if page.total_entries > 0 {
                next_cursor = Some(BUILTIN_FRONTSTAGE_CURSOR.to_string());
            }
        }
        entries.insert(0, builtin);
    }
    Ok(ExtensionCatalogGatewayPageResponse {
        category: page.category,
        freshness: match page.freshness {
            OfficialExtensionCatalogFreshness::Fresh => "fresh",
            OfficialExtensionCatalogFreshness::Stale => "stale",
        }
        .to_string(),
        catalog_page: page.snapshot_checksum.clone(),
        catalog_page_number: 0,
        catalog_page_checksum: page.snapshot_checksum,
        catalog_page_locator: page.snapshot_locator,
        limit,
        next_cursor,
        total_entries: page.total_entries + usize::from(builtin_matches && !remote_has_builtin),
        entries,
    })
}
