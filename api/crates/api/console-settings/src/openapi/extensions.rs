// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::file_storages::list_file_storages,
    crate::routes::file_storages::create_file_storage,
    crate::routes::file_tables::list_file_tables,
    crate::routes::file_tables::create_file_table,
    crate::routes::file_tables::bind_file_table_storage,
    crate::routes::ui_management::plugin_settings_page,
    crate::routes::ui_management::plugin_settings_module_asset,
    crate::routes::ui_management::custom_blocks::list_blocks,
    crate::routes::ui_management::custom_blocks::create_block,
    crate::routes::ui_management::custom_blocks::update_block,
    crate::routes::ui_management::custom_blocks::delete_block,
    crate::routes::ui_management::list_templates,
    crate::routes::ui_management::create_template,
    crate::routes::ui_management::update_template,
    crate::routes::ui_management::publish_template,
    crate::routes::ui_management::set_default_template,
    crate::routes::ui_management::reset_default_template,
    crate::routes::ui_management::delete_template,
    crate::routes::ui_management::list_components,
    crate::routes::ui_management::get_component,
    crate::routes::ui_management::create_component,
    crate::routes::ui_management::update_component,
    crate::routes::ui_management::delete_component,
    crate::routes::ui_management::catalog_index,
    crate::routes::ui_management::catalog_page,
    crate::routes::ui_management::catalog_search,
    crate::routes::ui_management::catalog_update_status,
    crate::routes::ui_management::catalog_download,
    crate::routes::ui_management::catalog_sync_group,
))]
struct Paths63;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::host_infrastructure::get_host_infrastructure_memory_overview,
    crate::routes::host_infrastructure::get_host_infrastructure_memory_stats_overview,
    crate::routes::host_infrastructure::get_host_infrastructure_memory_stats,
    crate::routes::host_infrastructure::list_host_infrastructure_memory_entries,
    crate::routes::host_infrastructure::list_host_infrastructure_memory_tree,
    crate::routes::host_infrastructure::search_host_infrastructure_memory_entries,
    crate::routes::host_infrastructure::reveal_host_infrastructure_memory_entry,
    crate::routes::host_infrastructure::get_host_infrastructure_cache_overview,
    crate::routes::host_infrastructure::list_host_infrastructure_cache_entries,
    crate::routes::host_infrastructure::reveal_host_infrastructure_cache_entry,
    crate::routes::host_infrastructure::clear_host_infrastructure_cache_entry,
    crate::routes::host_infrastructure::clear_host_infrastructure_cache_domain,
))]
struct Paths133;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::file_storages::CreateFileStorageBody,
    crate::routes::file_storages::FileStorageResponse,
    crate::routes::file_tables::BindFileTableStorageBody,
    crate::routes::file_tables::CreateFileTableBody,
    crate::routes::file_tables::FileTableResponse,
    crate::routes::ui_management::custom_blocks::CreateBlockBody,
    crate::routes::ui_management::custom_blocks::UpdateBlockBody,
    crate::routes::ui_management::custom_blocks::CustomBlockResponse,
    crate::routes::ui_management::TemplateBody,
    crate::routes::ui_management::UpdateTemplateBody,
    crate::routes::ui_management::PublishTemplateBody,
    crate::routes::ui_management::ResetDefaultTemplateBody,
    crate::routes::ui_management::ComponentUpstreamBody,
    crate::routes::ui_management::CreateComponentBody,
    crate::routes::ui_management::UpdateComponentBody,
    crate::routes::ui_management::TemplateRevisionResponse,
    crate::routes::ui_management::ManagedTemplateResponse,
    crate::routes::ui_management::OfficialTemplateResponse,
    crate::routes::ui_management::TemplateListResponse,
    crate::routes::ui_management::ComponentRecordResponse,
    crate::routes::ui_management::CatalogIndexResponse,
    crate::routes::ui_management::CatalogComponentResponse,
    crate::routes::ui_management::CatalogPageResponse,
    crate::routes::ui_management::CatalogSearchEntryResponse,
    crate::routes::ui_management::CatalogSearchResponse,
    crate::routes::ui_management::CatalogGroupUpdateResponse,
    crate::routes::ui_management::CatalogUpdateStatusResponse,
    crate::routes::ui_management::CatalogSyncResponse,
)))]
struct Schemas94;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::host_infrastructure::MemoryContractSummaryResponse,
    crate::routes::host_infrastructure::MemoryEntriesResponse,
    crate::routes::host_infrastructure::MemoryEntryRevealBody,
    crate::routes::host_infrastructure::MemoryEntryMetadataResponse,
    crate::routes::host_infrastructure::MemoryEntryValueResponse,
    crate::routes::host_infrastructure::MemoryInspectionCapabilitiesResponse,
    crate::routes::host_infrastructure::MemoryOverviewResponse,
    crate::routes::host_infrastructure::MemoryPageQuery,
    crate::routes::host_infrastructure::MemorySearchQuery,
    crate::routes::host_infrastructure::MemoryTreeNodeResponse,
    crate::routes::host_infrastructure::MemoryTreeResponse,
    crate::routes::host_infrastructure::CacheEntriesResponse,
    crate::routes::host_infrastructure::CacheEntryKeyBody,
    crate::routes::host_infrastructure::CacheEntryMetadataResponse,
    crate::routes::host_infrastructure::CacheEntryValueResponse,
    crate::routes::host_infrastructure::CacheDomainResponse,
    crate::routes::host_infrastructure::CacheInspectionCapabilitiesResponse,
    crate::routes::host_infrastructure::CacheOverviewResponse,
    crate::routes::host_infrastructure::ClearCacheDomainResponse,
    crate::routes::host_infrastructure::ClearCacheEntryResponse,
)))]
struct Schemas176;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(63, Paths63::openapi()), (133, Paths133::openapi())]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(94, Schemas94::openapi()), (176, Schemas176::openapi())]
}
