// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::local_commands::execute_local_command,
    crate::routes::terminal::info,
    crate::routes::terminal::files_list,
    crate::routes::terminal::files_stat,
    crate::routes::terminal::files_read_text,
    crate::routes::terminal::files_read_range,
    crate::routes::terminal::files_search,
    crate::routes::terminal::files_read_image,
    crate::routes::terminal::files_read_audio,
    crate::routes::terminal::files_read_resource,
    crate::routes::terminal::files_import_resource,
    crate::routes::terminal::files_download_file,
    crate::routes::terminal::jobs_start,
    crate::routes::terminal::jobs_get,
    crate::routes::terminal::jobs_read,
    crate::routes::terminal::jobs_cancel,
))]
struct Paths0;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::files::upload_file,
    crate::routes::files::import_remote_file,
    crate::routes::files::read_file_content,
    crate::routes::files::create_file_share_link,
    crate::routes::files::public::read_temporary_file,
))]
struct Paths145;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::network_center::list_network_egress_providers,
    crate::routes::network_center::create_network_egress_provider,
    crate::routes::network_center::update_network_egress_provider_lifecycle,
    crate::routes::network_center::sync_network_egress_provider,
    crate::routes::network_center::plugins::list_official_catalog,
    crate::routes::network_center::plugins::list_plugin_families,
    crate::routes::network_center::plugins::switch_plugin_version,
    crate::routes::network_center::plugins::uninstall_plugin_version,
    crate::routes::network_center::plugins::uninstall_plugin_family,
    crate::routes::network_center::plugins::install_official_plugin,
    crate::routes::network_center::plugins::install_uploaded_plugin,
    crate::routes::network_center::pools::list_network_egress_pools,
    crate::routes::network_center::pools::list_http_node_proxy_options,
    crate::routes::network_center::pools::create_network_egress_proxy,
    crate::routes::network_center::pools::get_network_egress_proxy,
    crate::routes::network_center::pools::update_network_egress_proxy,
    crate::routes::network_center::pools::test_network_egress_pool_member_connection,
    crate::routes::network_center::pools::create_network_egress_pool,
    crate::routes::network_center::pools::update_network_egress_pool,
    crate::routes::network_center::pools::delete_network_egress_pool,
    crate::routes::network_center::pools::create_network_egress_pool_member,
    crate::routes::network_center::pools::update_network_egress_pool_member,
    crate::routes::network_center::pools::delete_network_egress_pool_member,
    crate::routes::network_center::pools::delete_network_egress_pool_members,
    crate::routes::network_center::list_network_egress_routes,
    crate::routes::network_center::create_network_egress_route,
    crate::routes::network_center::update_network_egress_route,
    crate::routes::network_center::delete_network_egress_route,
))]
struct Paths168;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::local_terminal::dto::PathInput,
    crate::local_terminal::dto::ListInput,
    crate::local_terminal::dto::TextInput,
    crate::local_terminal::dto::RangeInput,
    crate::local_terminal::dto::SearchInput,
    crate::local_terminal::dto::MediaInput,
    crate::local_terminal::dto::ResourceInput,
    crate::local_terminal::dto::ImportInput,
    crate::local_terminal::dto::DownloadInput,
    crate::local_terminal::dto::FileMetadata,
    crate::local_terminal::dto::ListOutput,
    crate::local_terminal::dto::TextOutput,
    crate::local_terminal::dto::RangeMetadata,
    crate::local_terminal::dto::RangeOutput,
    crate::local_terminal::dto::MediaOutput,
    crate::local_terminal::dto::ResourceOutput,
    crate::local_terminal::dto::SearchMatch,
    crate::local_terminal::dto::SearchOutput,
    crate::local_terminal::dto::DownloadOutput,
    crate::local_terminal::dto::InfoOutput,
    crate::routes::terminal::InfoInput,
    crate::routes::terminal::JobInput,
    crate::routes::terminal::JobReadInput,
    crate::routes::terminal::ImportOutput,
    crate::local_command::jobs::StartInput,
    crate::local_command::jobs::ReadInput,
    crate::local_command::jobs::ReadOutput,
    crate::local_command::jobs::JobSnapshot,
    crate::local_command::jobs::JobState,
    crate::local_command::jobs::OutputStream,
    crate::routes::local_commands::LocalCommandBody,
    crate::routes::local_commands::LocalCommandResponse,
)))]
struct Schemas0;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::files::UploadedFileResponse,
    crate::routes::files::RemoteFileReference,
    crate::routes::files::ImportRemoteFileBody,
    crate::routes::files::CreateFileShareLinkInput,
    crate::routes::files::FileShareLinkResponse,
)))]
struct Schemas196;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::network_center::CreateNetworkEgressProviderBody,
    crate::routes::network_center::NetworkEgressProjectionResponse,
    crate::routes::network_center::NetworkEgressProviderResponse,
    crate::routes::network_center::UpdateNetworkEgressProviderLifecycleBody,
    crate::routes::network_center::pools::CreateNetworkEgressPoolBody,
    crate::routes::network_center::pools::UpdateNetworkEgressPoolBody,
    crate::routes::network_center::pools::CreateNetworkEgressPoolMemberBody,
    crate::routes::network_center::pools::UpdateNetworkEgressPoolMemberBody,
    crate::routes::network_center::pools::NetworkEgressPoolResponse,
    crate::routes::network_center::pools::HttpNodeProxyOptionResponse,
    crate::routes::network_center::pools::NetworkEgressPoolMemberResponse,
    crate::routes::network_center::CreateNetworkEgressRouteBody,
    crate::routes::network_center::UpdateNetworkEgressRouteBody,
    crate::routes::network_center::NetworkEgressRouteResponse,
)))]
struct Schemas220;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![
        (0, Paths0::openapi()),
        (145, Paths145::openapi()),
        (168, Paths168::openapi()),
    ]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![
        (0, Schemas0::openapi()),
        (196, Schemas196::openapi()),
        (220, Schemas220::openapi()),
    ]
}
