// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::notifications::types,
    crate::routes::notifications::list,
    crate::routes::notifications::get,
    crate::routes::notifications::create,
    crate::routes::notifications::patch,
    crate::routes::notifications::delete,
    crate::routes::notifications::authorize,
    crate::routes::notifications::send,
    crate::routes::notifications::logs,
    crate::routes::notifications::log,
    crate::routes::notifications::messages,
    crate::routes::notifications::unread,
    crate::routes::notifications::read,
    crate::routes::notifications::read_all,
    crate::routes::notifications::mark_unread,
    crate::routes::notifications::channels,
    crate::routes::notifications::read_channel,
    crate::routes::notifications::events::events,
))]
struct Paths36;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::frontstage::dispatch_frontstage_query,
    crate::routes::frontstage::dispatch_frontstage_action,
))]
struct Paths72;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::events::catalog,
    crate::routes::events::api_operations,
    crate::routes::events::api_operation,
    crate::routes::events::create,
    crate::routes::events::update,
    crate::routes::events::runtime,
))]
struct Paths76;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::user_api_keys::list_user_api_keys,
    crate::routes::user_api_keys::list_user_api_key_role_options,
    crate::routes::user_api_keys::create_user_api_key,
    crate::routes::user_api_keys::revoke_user_api_key,
))]
struct Paths84;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::auth::list_providers,
    crate::routes::auth::list_login_entries,
    crate::routes::client_collectors::download,
    crate::routes::auth::sign_in,
    crate::routes::auth::sign_up,
    crate::routes::external_auth::start,
    crate::routes::external_auth::callback,
    crate::routes::external_auth::authorize,
    crate::routes::me_authentication_methods::get_methods,
    crate::routes::me_authentication_methods::unlink,
    crate::routes::me_authentication_methods::authorize_password,
    crate::routes::me_authentication_methods::initialize_password,
    crate::routes::session::get_session,
    crate::routes::session::delete_session,
    crate::routes::session::revoke_all_sessions,
    crate::routes::session::switch_workspace,
    crate::routes::session::switch_active_role,
))]
struct Paths91;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::me::get_me,
    crate::routes::me::patch_me,
    crate::routes::me::patch_me_meta,
    crate::routes::me::change_password,
    crate::routes::communications::options,
    crate::routes::communications::binding,
    crate::routes::communications::save_binding,
    crate::routes::communications::conversations,
    crate::routes::communications::management_conversations,
    crate::routes::communications::management_messages,
    crate::routes::communications::management_message,
    crate::routes::communications::management_history,
    crate::routes::communications::messages,
    crate::routes::communications::files,
    crate::routes::communications::file,
    crate::routes::communications::file_info,
    crate::routes::communications::file_image,
    crate::routes::communications::retry,
    crate::routes::assistant::get_settings,
    crate::routes::assistant::update_settings,
    crate::routes::assistant::create_conversation,
    crate::routes::assistant::list_conversations,
    crate::routes::assistant::get_conversation_messages,
    crate::routes::assistant::get_legacy_snapshot_messages,
    crate::routes::assistant::get_run_activity,
    crate::routes::assistant::start_run,
    crate::routes::assistant::start_run_stream,
    crate::routes::assistant::websocket::create_ticket,
    crate::routes::assistant::websocket::upgrade,
))]
struct Paths136;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::frontstage::list_frontstage_pages,
    crate::routes::frontstage::create_frontstage_group,
    crate::routes::frontstage::create_frontstage_page,
    crate::routes::frontstage::get_frontstage_page_detail,
    crate::routes::frontstage::update_frontstage_page_title,
    crate::routes::frontstage::move_frontstage_page,
    crate::routes::frontstage::delete_frontstage_page,
    crate::routes::frontstage::list_frontstage_page_tabs,
    crate::routes::frontstage::create_frontstage_page_tab,
    crate::routes::frontstage::update_frontstage_page_tab,
    crate::routes::frontstage::delete_frontstage_page_tab,
    crate::routes::frontstage::save_frontstage_tab_document,
    crate::routes::frontstage::list_frontstage_ui_templates,
    crate::routes::frontstage::block_tree::list_frontstage_block_roots,
    crate::routes::frontstage::block_tree::create_frontstage_block_node,
    crate::routes::frontstage::block_tree::search_frontstage_blocks,
    crate::routes::frontstage::block_tree::get_frontstage_block_node,
    crate::routes::frontstage::block_tree::update_frontstage_block_node,
    crate::routes::frontstage::block_tree::update_frontstage_block_descriptors,
    crate::routes::frontstage::block_tree::delete_frontstage_block_leaf,
    crate::routes::frontstage::block_tree::list_frontstage_block_children,
    crate::routes::frontstage::block_tree::list_frontstage_block_ancestors,
    crate::routes::frontstage::block_tree::list_frontstage_block_descendants,
    crate::routes::frontstage::block_tree::get_frontstage_block_delete_impact,
    crate::routes::frontstage::block_tree::move_frontstage_block_node,
    crate::routes::frontstage::block_tree::delete_frontstage_block_subtree,
    crate::routes::frontstage::block_tree::open_frontstage_block,
    crate::routes::frontstage::block_tree::get_frontstage_block_node_code,
    crate::routes::frontstage::block_tree::get_frontstage_block_code_fragment,
    crate::routes::frontstage::block_tree::search_frontstage_block_code,
    crate::routes::frontstage::block_tree::replace_frontstage_block_code,
    crate::routes::frontstage::block_tree::save_frontstage_block_node_code,
    crate::routes::frontstage::block_tree::patch_frontstage_block_node_code,
    crate::routes::frontstage::block_tree::get_frontstage_block_runtime_assembly,
    crate::routes::frontstage::data_capabilities::list_frontstage_data_capabilities,
    crate::routes::frontstage::block_context_contract::get_frontstage_block_context_contract,
    crate::routes::frontstage::callable_interfaces::list_frontstage_interface_capabilities,
    crate::routes::frontstage::callable_interfaces::get_frontstage_interface_capability,
    crate::routes::frontstage::components::list_frontstage_components,
    crate::routes::frontstage::components::get_frontstage_component,
    crate::routes::frontstage::components::get_frontstage_component_module_asset,
    crate::routes::frontstage::callable_interface_dispatch::dispatch_frontstage_callable_interface,
))]
struct Paths196;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::network_center::list_network_egress_provider_types,
    crate::routes::network_center::pools::add_static_http_proxy_to_pool,
    crate::routes::network_center::pools::add_provider_egresses_to_pool,
))]
struct Paths273;

#[derive(OpenApi)]
#[openapi(components(schemas(
    domain::event_center::EventCatalogSortBy,
    domain::event_center::EventCatalogSortOrder,
)))]
struct Schemas0;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::error_response::ErrorBody,
    crate::routes::user_api_keys::CreateUserApiKeyRequest,
    crate::routes::user_api_keys::RevokeUserApiKeyResponse,
    crate::routes::user_api_keys::UserApiKeyListResponse,
    crate::routes::user_api_keys::UserApiKeyRoleOptionResponse,
    crate::routes::user_api_keys::UserApiKeyRoleOptionsResponse,
    crate::routes::user_api_keys::UserApiKeyResponse,
    crate::routes::auth::AuthProviderResponse,
    crate::routes::auth::LoginBody,
    crate::routes::auth::LoginResponse,
    crate::routes::auth::SignUpBody,
    crate::routes::auth::PublicLoginEntryResponse,
    crate::routes::auth::PublicLoginEntriesResponse,
)))]
struct Schemas3;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::me::ChangePasswordBody,
    crate::routes::me::MeResponse,
    crate::routes::me::PatchMeBody,
    crate::routes::me::PatchMeMetaBody,
    crate::routes::assistant::AssistantPreferenceBody,
    crate::routes::assistant::AssistantPublishedFlowOption,
    crate::routes::assistant::AssistantMcpInstanceOption,
    crate::routes::assistant::AssistantSettingsResponse,
    crate::routes::assistant::CreateAssistantConversationBody,
    crate::routes::assistant::AssistantConversationResponse,
    crate::routes::assistant::AssistantConversationSummaryResponse,
    crate::routes::assistant::AssistantConversationPageResponse,
    crate::routes::assistant::AssistantConversationMessageResponse,
    crate::routes::assistant::AssistantRunActivityPageResponse,
    crate::routes::assistant::StartAssistantRunBody,
    crate::routes::assistant::AssistantRunResponse,
    crate::routes::assistant::websocket::CreateAssistantWebSocketTicketBody,
    crate::routes::assistant::websocket::AssistantWebSocketTicketResponse,
)))]
struct Schemas24;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::network_center::NetworkEgressProviderTypeResponse,
    crate::routes::network_center::pools::AddStaticHttpProxyToPoolBody,
    crate::routes::network_center::pools::AddProviderEgressesToPoolBody,
    crate::routes::network_center::pools::NetworkEgressPoolMemberResponse,
)))]
struct Schemas84;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::session::SwitchWorkspaceBody,
    crate::routes::session::SwitchActiveRoleBody,
    crate::routes::session::AvailableRoleResponse,
    crate::routes::session::SessionResponse,
    crate::routes::frontstage::CreateFrontstageGroupBody,
    crate::routes::frontstage::CreateFrontstagePageBody,
    crate::routes::frontstage::FrontstageUiTemplateResponse,
    crate::routes::frontstage::FrontstagePageDetailResponse,
    crate::routes::frontstage::FrontstagePageCreationResponse,
    crate::routes::frontstage::FrontstagePageTabResponse,
    crate::routes::frontstage::FrontstagePageContentPresentationResponse,
    crate::routes::frontstage::FrontstageNavigationPlacementResponse,
    crate::routes::frontstage::FrontstageTabDocumentResponse,
    crate::routes::frontstage::FrontstagePageTreeNodeKind,
    crate::routes::frontstage::FrontstagePageTreeNodeResponse,
    crate::routes::frontstage::FrontstagePageResponse,
    crate::routes::frontstage::MoveFrontstagePageBody,
    crate::routes::frontstage::block_tree::FrontstageBlockPresentationDto,
    crate::routes::frontstage::block_tree::CreateFrontstageBlockNodeBody,
    crate::routes::frontstage::block_tree::UpdateFrontstageBlockNodeBody,
    crate::routes::frontstage::block_tree::FrontstageBlockDescriptorUpdateBody,
    crate::routes::frontstage::block_tree::UpdateFrontstageBlockDescriptorsBody,
    crate::routes::frontstage::block_tree::MoveFrontstageBlockNodeBody,
    crate::routes::frontstage::block_tree::DeleteFrontstageBlockSubtreeBody,
    crate::routes::frontstage::block_tree::SaveFrontstageBlockNodeCodeBody,
    crate::routes::frontstage::block_tree::PatchFrontstageBlockNodeCodeBody,
    crate::routes::frontstage::block_tree::FrontstageSourceEditBody,
    crate::routes::frontstage::block_tree::FrontstageBlockNodeSummaryResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockNodeResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockDescendantResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockSearchResultResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockDeleteImpactResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockSubtreeDeleteResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockNodeCodeResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockCodeFragmentResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockOpenResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockRuntimeLayerResponse,
    crate::routes::frontstage::block_tree::FrontstageBlockRuntimeAssemblyResponse,
    crate::routes::frontstage::data_capabilities::FrontstageDataCapabilitiesResponse,
    crate::routes::frontstage::data_capabilities::FrontstageDataCapabilityDescriptorResponse,
    crate::routes::frontstage::data_capabilities::FrontstageDataCapabilityModelResponse,
    crate::routes::frontstage::data_capabilities::FrontstageDataCapabilityFieldResponse,
    crate::routes::frontstage::block_context_contract::BlockContextEntryKind,
    crate::routes::frontstage::block_context_contract::BlockContextMemberKind,
    crate::routes::frontstage::block_context_contract::BlockContextMemberResponse,
    crate::routes::frontstage::block_context_contract::BlockContextEntryResponse,
    crate::routes::frontstage::block_context_contract::BlockContextContractResponse,
    crate::routes::frontstage::CreateFrontstagePageTabBody,
    crate::routes::frontstage::UpdateFrontstagePageTabBody,
    crate::routes::frontstage::SaveFrontstageTabDocumentBody,
    crate::routes::frontstage::UpdateFrontstagePageMetadataBody,
)))]
struct Schemas123;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![
        (36, Paths36::openapi()),
        (72, Paths72::openapi()),
        (76, Paths76::openapi()),
        (84, Paths84::openapi()),
        (91, Paths91::openapi()),
        (136, Paths136::openapi()),
        (196, Paths196::openapi()),
        (273, Paths273::openapi()),
    ]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![
        (0, Schemas0::openapi()),
        (3, Schemas3::openapi()),
        (24, Schemas24::openapi()),
        (84, Schemas84::openapi()),
        (123, Schemas123::openapi()),
    ]
}

// Each fragment owns complete canonical operation metadata before composition.
struct CanonicalExternalAuthenticationMetadata;

impl utoipa::Modify for CanonicalExternalAuthenticationMetadata {
    fn modify(&self, document: &mut utoipa::openapi::OpenApi) {
        for binding in crate::routes::external_auth::interface::bindings_metadata() {
            let path = document
                .paths
                .paths
                .get_mut(binding.path)
                .expect("external authentication API documentation is mounted");
            let operation = match binding.method {
                "GET" => path.get.as_mut(),
                "POST" => path.post.as_mut(),
                _ => None,
            }
            .expect("external authentication method is documented");
            operation.summary = Some(binding.summary.into());
            operation.description = Some(binding.description.into());
        }
    }
}

#[doc(hidden)]
pub fn apply_modifier(document: &mut utoipa::openapi::OpenApi) {
    utoipa::Modify::modify(&CanonicalExternalAuthenticationMetadata, document);
}
