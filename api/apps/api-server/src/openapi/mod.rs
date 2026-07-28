use std::sync::Arc;

use anyhow::anyhow;
use axum::{extract::State, Json};
use control_plane::{
    application_public_api::{
        mapping::WorkflowExtensionResponseMode,
        published_workflow_operation::{
            build_published_workflow_operations, PublishedWorkflowOperation,
        },
    },
    ports::ApplicationPublicationRepository,
};
use serde_json::{json, Map, Value};
use utoipa::OpenApi;

#[derive(utoipa::ToSchema)]
#[schema(value_type = String, format = Binary)]
#[allow(dead_code)]
pub(crate) struct OpenApiBinaryBody(pub Vec<u8>);

use crate::{app_state::ApiState, error_response::ApiError};

#[derive(OpenApi)]
#[openapi(
    paths(
        crate::health,
        crate::console_health,
        crate::routes::applications::list_applications,
        crate::routes::applications::get_application_catalog,
        crate::routes::applications::create_application,
        crate::routes::applications::create_application_tag,
        crate::routes::applications::get_application,
        crate::routes::applications::list_application_environment_variables,
        crate::routes::applications::replace_application_environment_variables,
        crate::routes::applications::list_application_js_dependency_selections,
        crate::routes::applications::replace_application_js_dependency_selection,
        crate::routes::applications::patch_application,
        crate::routes::applications::delete_application,
        crate::routes::application_management::list_application_management,
        crate::routes::application_api::list_application_api_keys,
        crate::routes::application_api::create_application_api_key,
        crate::routes::application_api::revoke_application_api_key,
        crate::routes::application_api::get_application_api_mapping,
        crate::routes::application_api::replace_application_api_mapping,
        crate::routes::application_api::operation_bindings::get_application_operation_bindings,
        crate::routes::application_api::get_application_api_publication,
        crate::routes::application_api::publish_application_api,
        crate::routes::application_api::unpublish_application_api,
        crate::routes::application_api::patch_application_api_status,
        crate::routes::application_api::get_workflow_schedule_trigger,
        crate::routes::application_api::replace_workflow_schedule_trigger,
        crate::routes::application_api::get_application_api_docs_catalog,
        crate::routes::application_api::get_application_api_docs_category_operations,
        crate::routes::application_api::get_application_api_docs_category_openapi,
        crate::routes::application_api::get_application_api_docs_operation_openapi,
        crate::routes::application_public_api::native::create_native_run,
        crate::routes::application_public_api::native::list_native_models,
        crate::routes::application_public_api::native::get_native_run,
        crate::routes::application_public_api::native::cancel_native_run,
        crate::routes::application_public_api::native::resume_native_run,
        crate::routes::application_public_api::native::upload_native_file,
        crate::routes::application_public_api::openai::list_models,
        crate::routes::application_public_api::openai::create_chat_completion,
        crate::routes::application_public_api::openai::create_response,
        crate::routes::application_public_api::openai::create_response_compact,
        crate::routes::application_public_api::anthropic::create_message,
        crate::routes::application_public_api::anthropic::count_message_tokens,
        crate::routes::application_orchestration::get_orchestration,
        crate::routes::application_orchestration::save_draft,
        crate::routes::application_orchestration::export_agent_flow_template,
        crate::routes::application_orchestration::preview_agent_flow_template,
        crate::routes::application_orchestration::import_agent_flow_template,
        crate::routes::application_orchestration::list_official_agent_flow_template_catalog,
        crate::routes::application_orchestration::download_official_agent_flow_template,
        crate::routes::application_orchestration::restore_version,
        crate::routes::application_orchestration::update_version,
        crate::routes::application_runtime::start_flow_debug_run,
        crate::routes::application_runtime::start_flow_debug_run_stream,
        crate::routes::application_runtime::get_flow_debug_run_snapshot,
        crate::routes::application_runtime::subscribe_flow_debug_run_stream,
        crate::routes::application_runtime::cancel_flow_run,
        crate::routes::application_runtime::resume_flow_run,
        crate::routes::application_runtime::complete_callback_task,
        crate::routes::application_runtime::start_node_debug_preview,
        crate::routes::application_runtime::list_application_runs,
        crate::routes::application_runtime::export_application_run_trace_dump,
        crate::routes::application_runtime::export_application_runs_zip,
        crate::routes::application_runtime::export_application_run_archive,
        crate::routes::application_runtime::export_application_runs_archive,
        crate::routes::application_runtime::create_run_archive_upload_session,
        crate::routes::application_runtime::upload_run_archive_chunk,
        crate::routes::application_runtime::complete_run_archive_upload_session,
        crate::routes::application_runtime::get_run_archive_import_job,
        crate::routes::application_runtime::application_monitoring::get_application_run_monitoring_report,
        crate::routes::application_runtime::application_monitoring::get_application_runtime_activity,
        crate::routes::application_runtime::get_application_run_trace_tree,
        crate::routes::application_runtime::get_application_run_overview,
        crate::routes::application_runtime::get_application_run_trace_node_children,
        crate::routes::application_runtime::get_application_run_trace_node_content,
        crate::routes::application_runtime::get_application_run_trace_node_detail,
        crate::routes::application_runtime::get_application_run_trace_tool_callback_content,
        crate::routes::application_runtime::get_application_run_resume_timeline,
        crate::routes::application_runtime::get_application_run_node_last_run,
        crate::routes::application_runtime::get_runtime_debug_stream,
        crate::routes::application_runtime::debug_variable_snapshot::get_debug_variable_snapshot,
        crate::routes::application_runtime::debug_variable_cache::upsert_debug_variable_cache_entry,
        crate::routes::application_runtime::debug_variable_cache::delete_debug_variable_cache_entries,
        crate::routes::application_runtime::get_runtime_debug_artifact,
        crate::routes::application_runtime::resolve_runtime_debug_artifacts,
        crate::routes::application_runtime::get_node_last_run,
        crate::routes::user_api_keys::list_user_api_keys,
        crate::routes::user_api_keys::list_user_api_key_role_options,
        crate::routes::user_api_keys::create_user_api_key,
        crate::routes::user_api_keys::revoke_user_api_key,
        crate::routes::auth::list_providers,
        crate::routes::auth::list_login_instances,
        crate::routes::auth::sign_in,
        crate::routes::auth::sign_up,
        crate::routes::session::get_session,
        crate::routes::session::delete_session,
        crate::routes::session::revoke_all_sessions,
        crate::routes::session::switch_workspace,
        crate::routes::auth_center::get_auth_center_overview,
        crate::routes::auth_center::create_auth_center_authenticator,
        crate::routes::auth_center::copy_auth_center_authenticator,
        crate::routes::auth_center::delete_auth_center_authenticator,
        crate::routes::auth_center::reorder_auth_center_authenticators,
        crate::routes::auth_center::enable_auth_center_authenticator,
        crate::routes::auth_center::update_auth_center_authenticator_config,
        crate::routes::auth_center::update_auth_center_authenticator_public_ui_block,
        crate::routes::system::get_release_status,
        crate::routes::system::get_runtime_profile,
        crate::routes::me::get_me,
        crate::routes::me::patch_me,
        crate::routes::me::patch_me_meta,
        crate::routes::me::change_password,
        crate::routes::workspace::get_workspace,
        crate::routes::workspace::patch_workspace,
        crate::routes::i18n_catalog::get_i18n_catalog_state,
        crate::routes::i18n_catalog::get_resolved_i18n_catalog_bundle,
        crate::routes::i18n_catalog::get_i18n_catalog_update_status,
        crate::routes::i18n_catalog::activate_i18n_catalog_update,
        crate::routes::i18n_catalog::list_catalog_entries,
        crate::routes::i18n_catalog::get_catalog_entry,
        crate::routes::i18n_catalog::upsert_catalog_override,
        crate::routes::i18n_catalog::restore_catalog_override,
        crate::routes::i18n_catalog::upsert_custom_catalog_translation,
        crate::routes::i18n_catalog::delete_custom_catalog_key,
        crate::routes::i18n_catalog::restore_all_catalog_overrides,
        crate::routes::workspaces::list_workspaces,
        crate::routes::members::list_members,
        crate::routes::members::create_member,
        crate::routes::members::update_member,
        crate::routes::members::delete_member,
        crate::routes::members::disable_member,
        crate::routes::members::enable_member,
        crate::routes::members::reset_member,
        crate::routes::members::replace_member_roles,
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
        crate::routes::frontstage::create_frontstage_block,
        crate::routes::frontstage::get_frontstage_block_code,
        crate::routes::frontstage::save_frontstage_block_code,
        crate::routes::frontstage::data_capabilities::list_frontstage_data_capabilities,
        crate::routes::frontstage::callable_interfaces::list_frontstage_interface_capabilities,
        crate::routes::frontstage::callable_interfaces::get_frontstage_interface_capability,
        crate::routes::frontstage::component_capabilities::list_frontstage_component_capabilities,
        crate::routes::frontstage::component_capabilities::get_frontstage_component_capability,
        crate::routes::frontstage::component_capabilities::get_frontstage_component_module_asset,
        crate::routes::frontstage::callable_interfaces::issue_frontstage_callable_write_grant,
        crate::routes::frontstage::callable_interfaces::dispatch_frontstage_callable_interface,
        crate::routes::roles::list_roles,
        crate::routes::roles::create_role,
        crate::routes::roles::update_role,
        crate::routes::roles::delete_role,
        crate::routes::roles::get_role_permissions,
        crate::routes::roles::replace_role_permissions,
        crate::routes::roles::get_role_data_policy,
        crate::routes::roles::replace_role_data_policy,
        crate::routes::roles::list_data_model_options,
        crate::routes::navigation::get_console_navigation,
        crate::routes::permissions::list_permissions,
        crate::routes::model_definitions::list_models,
        crate::routes::model_definitions::list_agent_flow_options,
        crate::routes::model_definitions::create_model,
        crate::routes::model_definitions::get_advisor_findings,
        crate::routes::model_definitions::list_scope_grants,
        crate::routes::model_definitions::update_model,
        crate::routes::model_definitions::delete_model,
        crate::routes::model_definitions::batch_delete_models,
        crate::routes::model_definitions::create_field,
        crate::routes::model_definitions::update_field,
        crate::routes::model_definitions::delete_field,
        crate::routes::model_definitions::create_scope_grant,
        crate::routes::model_definitions::update_scope_grant,
        crate::routes::plugins::list_catalog,
        crate::routes::plugins::list_families,
        crate::routes::plugins::list_official_catalog,
        crate::routes::plugins::install_uploaded_plugin,
        crate::routes::plugins::install_plugin,
        crate::routes::plugins::install_official_plugin,
        crate::routes::plugins::refresh_catalog_projection,
        crate::routes::plugins::upgrade_latest,
        crate::routes::plugins::switch_version,
        crate::routes::plugins::delete_family,
        crate::routes::plugins::enable_plugin,
        crate::routes::plugins::assign_plugin,
        crate::routes::plugins::list_tasks,
        crate::routes::plugins::get_task,
        crate::routes::plugins::settings_routes::list_families,
        crate::routes::plugins::settings_routes::list_official_catalog,
        crate::routes::plugins::settings_routes::install_uploaded_plugin,
        crate::routes::plugins::settings_routes::install_official_plugin,
        crate::routes::plugins::settings_routes::refresh_current_node_artifact,
        crate::routes::plugins::settings_routes::install_current_node_artifact,
        crate::routes::plugins::settings_routes::upgrade_latest,
        crate::routes::plugins::settings_routes::switch_version,
        crate::routes::plugins::settings_routes::delete_family,
        crate::routes::plugins::settings_routes::get_task,
        crate::routes::frontend_block_catalog::list_frontend_blocks,
        crate::routes::js_dependencies::list_js_dependencies,
        crate::routes::node_contributions::list_node_contributions,
        crate::routes::data_sources::list_catalog,
        crate::routes::data_sources::list_data_sources,
        crate::routes::data_sources::create_data_source,
        crate::routes::data_sources::update_defaults,
        crate::routes::data_sources::validate_data_source,
        crate::routes::data_sources::rotate_secret,
        crate::routes::data_sources::list_resources,
        crate::routes::data_sources::discover_resources,
        crate::routes::data_sources::preview_read,
        crate::routes::data_sources::map_resource_to_model,
        crate::routes::file_storages::list_file_storages,
        crate::routes::file_storages::create_file_storage,
        crate::routes::file_tables::list_file_tables,
        crate::routes::file_tables::create_file_table,
        crate::routes::file_tables::bind_file_table_storage,
        crate::routes::mcp_management::get_mcp_catalog,
        crate::routes::mcp_management::list_mcp_interface_capabilities,
        crate::routes::mcp_management::list_mcp_items,
        crate::routes::mcp_management::export_mcp_catalog,
        crate::routes::mcp_management::list_mcp_instances,
        crate::routes::mcp_management::create_mcp_instance,
        crate::routes::mcp_management::copy_mcp_instance,
        crate::routes::mcp_management::update_mcp_instance,
        crate::routes::mcp_management::delete_mcp_instance,
        crate::routes::mcp_management::upsert_mcp_group,
        crate::routes::mcp_management::delete_mcp_group,
        crate::routes::mcp_management::list_mcp_tools,
        crate::routes::mcp_management::create_mcp_tool,
        crate::routes::mcp_management::get_mcp_tool,
        crate::routes::mcp_management::update_mcp_tool,
        crate::routes::mcp_management::delete_mcp_tool,
        crate::routes::mcp_management::refresh_mcp_tool_description,
        crate::routes::mcp_management::check_mcp_tool_description,
        crate::routes::mcp_management::execute_mcp_debug,
        crate::routes::mcp_management::create_mcp_tool_binding,
        crate::routes::mcp_management::update_mcp_tool_binding,
        crate::routes::mcp_management::delete_mcp_tool_binding,
        crate::routes::mcp_management::get_mcp_instance_discovery_policy,
        crate::routes::mcp_management::update_mcp_instance_discovery_policy,
        crate::routes::mcp_management::upstream::list_connections,
        crate::routes::mcp_management::upstream::create_connection,
        crate::routes::mcp_management::upstream::update_connection,
        crate::routes::mcp_management::upstream::delete_connection,
        crate::routes::mcp_management::upstream::save_credentials,
        crate::routes::mcp_management::upstream::delete_credentials,
        crate::routes::mcp_management::upstream::test_draft_connection,
        crate::routes::mcp_management::upstream::test_connection,
        crate::routes::mcp_management::upstream::discover_tools,
        crate::routes::mcp_management::upstream::import_tools,
        crate::routes::mcp_management::upstream::debug_proxy_tool,
        crate::routes::host_infrastructure::list_host_infrastructure_providers,
        crate::routes::host_infrastructure::save_host_infrastructure_provider_config,
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
        crate::routes::files::upload_file,
        crate::routes::files::read_file_content,
        crate::routes::model_providers::list_catalog,
        crate::routes::model_providers::list_instances,
        crate::routes::model_providers::list_request_logs,
        crate::routes::model_providers::delete_selected_request_logs,
        crate::routes::model_providers::clear_request_logs_batch,
        crate::routes::model_providers::create_instance,
        crate::routes::model_providers::get_main_instance,
        crate::routes::model_providers::update_main_instance,
        crate::routes::model_providers::update_instance,
        crate::routes::model_providers::validate_instance,
        crate::routes::model_providers::preview_models,
        crate::routes::model_providers::get_balance,
        crate::routes::model_providers::reveal_secret,
        crate::routes::model_providers::list_models,
        crate::routes::model_providers::refresh_models,
        crate::routes::model_providers::delete_instance,
        crate::routes::model_providers::list_options,
        crate::routes::model_providers::settings_routes::list_settings_options,
        crate::routes::runtime_models::list_records,
        crate::routes::runtime_models::get_record,
        crate::routes::runtime_models::create_record,
        crate::routes::runtime_models::update_record,
        crate::routes::runtime_models::delete_record,
        crate::routes::docs::get_data_model_openapi,
    ),
    components(schemas(
        crate::HealthResponse,
        crate::error_response::ErrorBody,
        crate::routes::applications::ApplicationApiSectionResponse,
        crate::routes::applications::ApplicationCatalogResponse,
        crate::routes::applications::ApplicationDetailResponse,
        crate::routes::applications::ApplicationEnvironmentVariableBody,
        crate::routes::applications::ApplicationEnvironmentVariableResponse,
        crate::routes::applications::ApplicationJsDependencyPermissionsResponse,
        crate::routes::applications::ApplicationJsDependencySelectionResponse,
        crate::routes::applications::ApplicationLogsSectionResponse,
        crate::routes::applications::ApplicationMonitoringSectionResponse,
        crate::routes::applications::ApplicationOrchestrationSectionResponse,
        crate::routes::applications::ApplicationSectionsResponse,
        crate::routes::applications::ApplicationSummaryResponse,
        crate::routes::applications::ApplicationTagCatalogResponse,
        crate::routes::applications::ApplicationTagResponse,
        crate::routes::applications::ApplicationTypeOptionResponse,
        crate::routes::applications::CreateApplicationBody,
        crate::routes::applications::CreateApplicationTagBody,
        crate::routes::applications::PatchApplicationBody,
        crate::routes::applications::ReplaceApplicationEnvironmentVariablesBody,
        crate::routes::applications::ReplaceApplicationJsDependencySelectionBody,
        crate::routes::application_management::ApplicationManagementItemResponse,
        crate::routes::application_management::ApplicationManagementPageResponse,
        crate::routes::application_management::ApplicationManagementTagResponse,
        crate::routes::application_api::ApplicationApiKeyResponse,
        crate::routes::application_api::ApplicationApiMappingBody,
        crate::routes::application_api::ApplicationApiMappingInputBody,
        crate::routes::application_api::ApplicationApiMappingOutputBody,
        crate::routes::application_api::ApplicationApiStatusResponse,
        crate::routes::application_api::ApplicationDraftOperationBindingProjectionResponse,
        crate::routes::application_api::ApplicationOperationBindingOperationResponse,
        crate::routes::application_api::ApplicationOperationBindingOptionsResponse,
        crate::routes::application_api::ApplicationOperationBindingProjectionResponse,
        crate::routes::application_api::ApplicationOperationBindingTargetOptionResponse,
        crate::routes::application_api::ApplicationOperationBindingUnsupportedReasonResponse,
        crate::routes::application_api::ApplicationPublicationJsDependencyPermissionsResponse,
        crate::routes::application_api::ApplicationPublicationJsDependencySnapshotResponse,
        crate::routes::application_api::ApplicationPublicationResponse,
        crate::routes::application_api::ApplicationPublishedOperationBindingProjectionResponse,
        crate::routes::application_api::ApplicationPublishedOperationBindingStatusResponse,
        crate::routes::application_api::ApplicationPublishedOperationBindingsProjectionResponse,
        crate::routes::application_api::CreateApplicationApiKeyBody,
        crate::routes::application_api::CreatedApplicationApiKeyResponse,
        crate::routes::application_api::PatchApplicationApiStatusBody,
        crate::routes::application_api::PublishApplicationApiBody,
        crate::routes::application_api::WorkflowScheduleTriggerBody,
        crate::routes::application_api::WorkflowScheduleTriggerResponse,
        crate::routes::user_api_keys::CreateUserApiKeyRequest,
        crate::routes::user_api_keys::RevokeUserApiKeyResponse,
        crate::routes::user_api_keys::UserApiKeyListResponse,
        crate::routes::user_api_keys::UserApiKeyRoleOptionResponse,
        crate::routes::user_api_keys::UserApiKeyRoleOptionsResponse,
        crate::routes::user_api_keys::UserApiKeyResponse,
        crate::routes::application_public_api::native::NativeErrorBody,
        crate::routes::application_public_api::native::NativeModelCapabilities,
        crate::routes::application_public_api::native::NativeModelListResponse,
        crate::routes::application_public_api::native::NativeModelObject,
        crate::routes::application_public_api::native::NativeModelReasoning,
        crate::routes::application_public_api::native::NativeRunResponse,
        crate::routes::application_public_api::native::ResumeNativeRunBody,
        crate::routes::application_public_api::openai::OpenAiChatCompletionResponse,
        crate::routes::application_public_api::openai::OpenAiChatCompletionChoice,
        crate::routes::application_public_api::openai::OpenAiChatMessage,
        crate::routes::application_public_api::openai::OpenAiErrorBody,
        crate::routes::application_public_api::openai::OpenAiErrorObject,
        crate::routes::application_public_api::openai::OpenAiModelListResponse,
        crate::routes::application_public_api::openai::OpenAiModelObject,
        crate::routes::application_public_api::openai::OpenAiResponsesObject,
        crate::routes::application_public_api::openai::OpenAiResponsesUsage,
        crate::routes::application_public_api::openai::OpenAiToolCall,
        crate::routes::application_public_api::openai::OpenAiToolCallFunction,
        crate::routes::application_public_api::openai::OpenAiUsage,
        crate::routes::application_public_api::anthropic::AnthropicMessageResponse,
        crate::routes::application_public_api::anthropic::AnthropicCountTokensResponse,
        crate::routes::application_public_api::anthropic::AnthropicErrorBody,
        crate::routes::application_public_api::anthropic::AnthropicErrorObject,
        crate::routes::application_public_api::anthropic::AnthropicUsage,
        crate::routes::application_orchestration::FlowDraftResponse,
        crate::routes::application_orchestration::FlowVersionResponse,
        crate::routes::application_orchestration::OrchestrationStateResponse,
        crate::routes::application_orchestration::AgentFlowTemplateApplicationResponse,
        crate::routes::application_orchestration::AgentFlowTemplateDependencyResponse,
        crate::routes::application_orchestration::AgentFlowTemplateDependencyStatusResponse,
        crate::routes::application_orchestration::AgentFlowTemplateImportedApplicationResponse,
        crate::routes::application_orchestration::AgentFlowTemplatePackageResponse,
        crate::routes::application_orchestration::AgentFlowTemplatePreviewBody,
        crate::routes::application_orchestration::AgentFlowTemplatePreviewResponse,
        crate::routes::application_orchestration::AgentFlowTemplateUnresolvedNodeResponse,
        crate::routes::application_orchestration::ImportAgentFlowTemplateBody,
        crate::routes::application_orchestration::ImportAgentFlowTemplateResponse,
        crate::routes::application_orchestration::SaveDraftBody,
        crate::routes::application_runtime::ApplicationRunDetailResponse,
        crate::routes::application_runtime::ApplicationRunOverviewResponse,
        crate::routes::application_runtime::ApplicationRunSelectedExportBody,
        crate::routes::application_runtime::ApplicationRunSelectedExportManifestResponse,
        crate::routes::application_runtime::ApplicationRunSelectedExportManifestRunResponse,
        crate::routes::application_runtime::ApplicationRunArchiveBody,
        crate::routes::application_runtime::ApplicationRunTraceExportNodeResponse,
        crate::routes::application_runtime::ApplicationRunTraceExportResponse,
        crate::routes::application_runtime::ApplicationRunTraceExportTreeResponse,
        crate::routes::application_runtime::ApplicationRunTraceExportWarningResponse,
        crate::routes::application_runtime::RunArchiveChunkUploadResponse,
        crate::routes::application_runtime::RunArchiveImportJobResponse,
        crate::routes::application_runtime::RunArchiveImportRunMappingResponse,
        crate::routes::application_runtime::RunArchiveUploadSessionCreateBody,
        crate::routes::application_runtime::RunArchiveUploadSessionResponse,
        crate::routes::application_runtime::RunArchiveV1EntryResponse,
        crate::routes::application_runtime::RunArchiveV1ManifestEntryResponse,
        crate::routes::application_runtime::RunArchiveV1ManifestResponse,
        crate::routes::application_runtime::RunArchiveV1Response,
        crate::routes::application_runtime::RunArchiveV1SourceResponse,
        crate::routes::application_runtime::ApplicationRunTraceToolCallbackContentResponse,
        crate::routes::application_runtime::application_monitoring::ApplicationRunMonitoringReportResponse,
        crate::runtime_activity::ApplicationRuntimeActivitySnapshot,
        crate::routes::application_runtime::ApplicationConversationMessageResponse,
        crate::routes::application_runtime::ApplicationConversationMessagesPageInfoResponse,
        crate::routes::application_runtime::ApplicationConversationMessagesPageResponse,
        crate::routes::application_runtime::CallbackTaskResponse,
        crate::routes::application_runtime::CheckpointResponse,
        crate::routes::application_runtime::CompleteCallbackTaskBody,
        crate::routes::application_runtime::DebugVariableSnapshotResponse,
        crate::routes::application_runtime::FlowRunResponse,
        crate::routes::application_runtime::FlowRunSummaryPageResponse,
        crate::routes::application_runtime::FlowRunSummaryResponse,
        crate::routes::application_runtime::NodeLastRunResponse,
        crate::routes::application_runtime::NodeRunResponse,
        crate::routes::application_runtime::ResumeFlowRunBody,
        crate::routes::application_runtime::RuntimeDebugStreamPartResponse,
        crate::routes::application_runtime::RuntimeDebugStreamResponse,
        crate::routes::application_runtime::RunEventResponse,
        crate::routes::application_runtime::StartFlowDebugRunBody,
        crate::routes::application_runtime::StartNodeDebugPreviewBody,
        crate::routes::auth::AuthProviderResponse,
        crate::routes::auth::LoginBody,
        crate::routes::auth::LoginResponse,
        crate::routes::auth::SignUpBody,
        crate::routes::auth::PublicLoginInstanceResponse,
        crate::routes::auth::PublicLoginInstancesResponse,
        crate::routes::auth_center::AuthCenterAuthenticatorResponse,
        crate::routes::auth_center::AuthCenterConfigFieldResponse,
        crate::routes::auth_center::AuthCenterOverviewResponse,
        crate::routes::auth_center::CreateAuthCenterAuthenticatorBody,
        crate::routes::auth_center::CopyAuthCenterAuthenticatorBody,
        crate::routes::auth_center::ReorderAuthCenterAuthenticatorsBody,
        crate::routes::auth_center::UpdateAuthCenterAuthenticatorConfigBody,
        crate::routes::auth_center::UpdateAuthCenterAuthenticatorPublicUiBlockBody,
        crate::routes::me::ChangePasswordBody,
        crate::routes::me::MeResponse,
        crate::routes::me::PatchMeBody,
        crate::routes::me::PatchMeMetaBody,
        crate::routes::i18n_catalog::CatalogManagementOriginDto,
        crate::routes::i18n_catalog::ListCatalogEntriesQuery,
        crate::routes::i18n_catalog::GetCatalogEntryQuery,
        crate::routes::i18n_catalog::UpsertCatalogTranslationBody,
        crate::routes::i18n_catalog::RestoreCatalogOverrideBody,
        crate::routes::i18n_catalog::DeleteCustomCatalogKeyBody,
        crate::routes::i18n_catalog::RestoreCatalogOverridesBody,
        crate::routes::i18n_catalog::CatalogManagementEntryResponse,
        crate::routes::i18n_catalog::CatalogManagementPageResponse,
        crate::routes::i18n_catalog::CatalogEntryMutationResponse,
        crate::routes::i18n_catalog::CatalogRevisionResponse,
        crate::routes::members::CreateMemberBody,
        crate::routes::members::MemberResponse,
        crate::routes::members::ReplaceMemberRolesBody,
        crate::routes::members::ResetMemberPasswordBody,
        crate::routes::model_definitions::CreateModelDefinitionBody,
        crate::routes::model_definitions::BatchDeleteModelDefinitionsBody,
        crate::routes::model_definitions::UpdateModelDefinitionBody,
        crate::routes::model_definitions::CreateModelFieldBody,
        crate::routes::model_definitions::UpdateModelFieldBody,
        crate::routes::model_definitions::CreateScopeGrantBody,
        crate::routes::model_definitions::UpdateScopeGrantBody,
        crate::routes::model_definitions::ModelDefinitionResponse,
        crate::routes::model_definitions::AgentFlowDataModelFieldOptionResponse,
        crate::routes::model_definitions::AgentFlowDataModelOptionResponse,
        crate::routes::model_definitions::ModelFieldResponse,
        crate::routes::model_definitions::DataModelAdvisorFindingResponse,
        crate::routes::model_definitions::ScopeGrantResponse,
        crate::routes::model_definitions::DeletedResponse,
        crate::routes::model_definitions::BatchDeletedResponse,
        crate::routes::plugins::InstallPluginBody,
        crate::routes::plugins::InstallOfficialPluginBody,
        crate::routes::plugins::InstallPluginResponse,
        crate::routes::plugins::OfficialPluginCatalogResponse,
        crate::routes::plugins::OfficialPluginCatalogEntryResponse,
        crate::routes::plugins::OfficialPluginCatalogPageResponse,
        crate::routes::plugins::PluginCatalogEntryResponse,
        crate::routes::plugins::PluginCatalogProjectionResponse,
        crate::routes::plugins::PluginFamilyResponse,
        crate::routes::plugins::PluginInstallationResponse,
        crate::routes::plugins::PluginInstalledVersionResponse,
        crate::routes::plugins::PluginTaskResponse,
        crate::routes::frontend_block_catalog::FrontendBlockCatalogResponse,
        crate::routes::frontend_block_catalog::FrontendBlockModuleAssetResponse,
        crate::routes::frontend_block_catalog::FrontendBlockCodeModuleResponse,
        crate::routes::frontend_block_catalog::FrontendBlockContextContractResponse,
        crate::routes::frontend_block_catalog::FrontendBlockPermissionsResponse,
        crate::routes::plugins::SwitchPluginVersionBody,
        crate::routes::js_dependencies::JsDependencyCatalogEntryResponse,
        crate::routes::js_dependencies::JsDependencyPermissionsResponse,
        crate::routes::node_contributions::NodeContributionQuery,
        crate::routes::node_contributions::NodeContributionResponse,
        crate::routes::data_sources::CreateDataSourceBody,
        crate::routes::data_sources::UpdateDataSourceDefaultsBody,
        crate::routes::data_sources::PreviewDataSourceReadBody,
        crate::routes::data_sources::RotateDataSourceSecretBody,
        crate::routes::data_sources::MapDataSourceResourceToModelBody,
        crate::routes::data_sources::DataSourceCatalogResponse,
        crate::routes::data_sources::DataSourceCatalogEntryResponse,
        crate::routes::data_sources::DataSourceConfigFieldOptionResponse,
        crate::routes::data_sources::DataSourceConfigFieldResponse,
        crate::routes::data_sources::DataSourceCapabilitiesResponse,
        crate::routes::data_sources::DataSourceBackendResponse,
        crate::routes::data_sources::DataSourceResponse,
        crate::routes::data_sources::DataSourceResourceCapabilitiesResponse,
        crate::routes::data_sources::DataSourceRemoteResourceResponse,
        crate::routes::data_sources::DataSourceResourcesResponse,
        crate::routes::data_sources::ValidateDataSourceResponse,
        crate::routes::data_sources::DataSourcePreviewOutputResponse,
        crate::routes::data_sources::PreviewDataSourceReadResponse,
        crate::routes::file_storages::CreateFileStorageBody,
        crate::routes::file_storages::FileStorageResponse,
        crate::routes::file_tables::BindFileTableStorageBody,
        crate::routes::file_tables::CreateFileTableBody,
        crate::routes::file_tables::FileTableResponse,
        crate::routes::mcp_management::CreateMcpInstanceBody,
        crate::routes::mcp_management::CreateMcpToolBindingBody,
        crate::routes::mcp_management::CreateMcpToolBody,
        crate::routes::mcp_management::McpCatalogResponse,
        crate::routes::mcp_management::McpDescriptionCheckBody,
        crate::routes::mcp_management::McpDescriptionCheckResponse,
        crate::routes::mcp_management::McpDebugExecuteBody,
        crate::routes::mcp_management::McpDebugExecuteDetailsResponse,
        crate::routes::mcp_management::McpDebugResponseMode,
        crate::routes::mcp_management::McpExportPackageResponse,
        crate::routes::mcp_management::McpGroupResponse,
        crate::routes::mcp_management::McpInstanceResponse,
        crate::routes::mcp_management::McpInterfaceCatalogEntryResponse,
        crate::routes::mcp_management::McpListItemSummaryResponse,
        crate::routes::mcp_management::McpInstanceDiscoveryPolicyResponse,
        crate::routes::mcp_management::McpToolBindingResponse,
        crate::routes::mcp_management::McpToolResponse,
        crate::routes::mcp_management::McpToolAvailabilityStatusDto,
        crate::routes::mcp_management::UpdateMcpInstanceDiscoveryPolicyBody,
        crate::routes::mcp_management::UpdateMcpToolBindingBody,
        crate::routes::mcp_management::UpdateMcpToolBody,
        crate::routes::mcp_management::UpsertMcpGroupBody,
        crate::routes::mcp_management::McpToolExecutionTargetDto,
        crate::routes::mcp_management::upstream::SaveMcpUpstreamConnectionBody,
        crate::routes::mcp_management::upstream::McpUpstreamConnectionResponse,
        crate::routes::mcp_management::upstream::SaveMcpUpstreamCredentialBody,
        crate::routes::mcp_management::upstream::TestMcpUpstreamConnectionDraftBody,
        crate::routes::mcp_management::upstream::McpUpstreamDraftTestResponse,
        crate::routes::mcp_management::upstream::McpUpstreamTestResponse,
        crate::routes::mcp_management::upstream::McpUpstreamToolResponse,
        crate::routes::mcp_management::upstream::McpUpstreamDiscoverResponse,
        crate::routes::mcp_management::upstream::ImportMcpUpstreamToolsBody,
        crate::routes::mcp_management::upstream::DebugMcpProxyToolBody,
        crate::routes::mcp_management::upstream::DebugMcpProxyToolResponse,
        crate::routes::host_infrastructure::HostInfrastructureProviderConfigResponse,
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
        crate::routes::host_infrastructure::PluginFormConditionResponse,
        crate::routes::host_infrastructure::PluginFormFieldSchemaResponse,
        crate::routes::host_infrastructure::PluginFormOptionResponse,
        crate::routes::host_infrastructure::SaveHostInfrastructureProviderConfigBody,
        crate::routes::host_infrastructure::SaveHostInfrastructureProviderConfigResponse,
        crate::routes::files::UploadedFileResponse,
        crate::routes::model_providers::CreateModelProviderBody,
        crate::routes::model_providers::UpdateModelProviderBody,
        crate::routes::model_providers::ModelProviderCatalogResponse,
        crate::routes::model_providers::ModelProviderCatalogEntryResponse,
        crate::routes::model_providers::ModelProviderConfigFieldResponse,
        crate::routes::model_providers::ModelProviderInstanceResponse,
        crate::routes::model_providers::ModelProviderMainInstanceResponse,
        crate::routes::model_providers::ModelProviderMainInstanceSummaryResponse,
        crate::routes::model_providers::ModelProviderBalanceInfoResponse,
        crate::routes::model_providers::ModelProviderBalanceResponse,
        crate::routes::model_providers::ModelProviderOptionGroupResponse,
        crate::routes::model_providers::ValidateModelProviderResponse,
        crate::routes::model_providers::ProviderModelDescriptorResponse,
        crate::routes::model_providers::ModelProviderModelCatalogResponse,
        crate::routes::model_providers::UpdateModelProviderMainInstanceBody,
        crate::routes::model_providers::ModelProviderOptionResponse,
        crate::routes::model_providers::ModelProviderOptionsResponse,
        crate::routes::model_providers::DeletedResponse,
        crate::routes::navigation::ConsoleNavigationItemResponse,
        crate::routes::navigation::ConsoleNavigationResponse,
        crate::routes::navigation::ConsolePermissionBindingResponse,
        crate::routes::navigation::ConsoleRouteDefinitionResponse,
        crate::routes::permissions::PermissionResponse,
        crate::routes::docs::DataModelOpenApiDocumentResponse,
        crate::routes::roles::CreateRoleBody,
        crate::routes::roles::ReplaceRoleDataPolicyBody,
        crate::routes::roles::ReplaceRolePermissionsBody,
        crate::routes::roles::RoleDataModelPolicyBody,
        crate::routes::roles::RoleDataPolicyBody,
        crate::routes::roles::RoleDataPolicyResponse,
        crate::routes::roles::RoleDataModelOptionResponse,
        crate::routes::roles::RolePermissionsResponse,
        crate::routes::roles::RoleResponse,
        crate::routes::roles::UpdateRoleBody,
        crate::routes::runtime_models::RuntimeListResponse,
        crate::routes::runtime_models::RuntimeRecordEnvelope,
        crate::routes::session::SwitchWorkspaceBody,
        crate::routes::session::SessionResponse,
        crate::routes::frontstage::CreateFrontstageGroupBody,
        crate::routes::frontstage::CreateFrontstagePageBody,
        crate::routes::frontstage::FrontstageBlockCodeResponse,
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
        crate::routes::frontstage::SaveFrontstageBlockCodeBody,
        crate::routes::frontstage::data_capabilities::FrontstageDataCapabilitiesResponse,
        crate::routes::frontstage::data_capabilities::FrontstageDataCapabilityDescriptorResponse,
        crate::routes::frontstage::data_capabilities::FrontstageDataCapabilityModelResponse,
        crate::routes::frontstage::data_capabilities::FrontstageDataCapabilityFieldResponse,
        crate::routes::frontstage::CreateFrontstagePageTabBody,
        crate::routes::frontstage::UpdateFrontstagePageTabBody,
        crate::routes::frontstage::SaveFrontstageTabDocumentBody,
        crate::routes::frontstage::UpdateFrontstagePageMetadataBody,
        crate::routes::system::LocaleMetaResponse,
        crate::routes::system::LocaleSourceResponse,
        crate::routes::system::ConsoleReleaseInfoResponse,
        crate::routes::system::ConsoleReleaseStatusResponse,
        crate::routes::system::ConsoleReleaseUpgradeCommandsResponse,
        crate::routes::system::SystemRuntimeCpuResponse,
        crate::routes::system::SystemRuntimeHostResponse,
        crate::routes::system::SystemRuntimeMemoryResponse,
        crate::routes::system::SystemRuntimePlatformResponse,
        crate::routes::system::SystemRuntimeProfileResponse,
        crate::routes::system::SystemRuntimeRelationship,
        crate::routes::system::SystemRuntimeServiceResponse,
        crate::routes::system::SystemRuntimeServicesResponse,
        crate::routes::system::SystemRuntimeTopologyResponse,
        crate::routes::workspace::PatchWorkspaceBody,
        crate::routes::workspace::WorkspaceResponse,
        crate::routes::workspaces::WorkspaceSummaryResponse,
    )),
    info(title = "1flowbase API", version = "0.1.0")
)]
pub struct ApiDoc;

pub(crate) async fn dynamic_openapi_document(state: &ApiState) -> Result<Value, ApiError> {
    let mut document = serde_json::to_value(ApiDoc::openapi())?;
    let publications = state.store.list_enabled_extension_publications().await?;
    let operations = build_published_workflow_operations(publications)
        .map_err(|_| control_plane::errors::ControlPlaneError::Conflict("workflow_route"))?;
    document["components"]["securitySchemes"]["UserApiKey"] = json!({
        "type": "http",
        "scheme": "bearer",
        "bearerFormat": "User API Key"
    });
    let document_map = document
        .as_object_mut()
        .ok_or_else(|| anyhow!("dynamic OpenAPI document must be an object"))?;
    crate::openapi_docs::ensure_session_security_schemes(document_map, &state.cookie_name)?;
    append_workflow_extension_paths(&mut document, &operations);
    Ok(document)
}

pub(crate) async fn workflow_extension_openapi_document(
    state: &ApiState,
) -> Result<Value, ApiError> {
    let mut document = dynamic_openapi_document(state).await?;
    let paths = document
        .get_mut("paths")
        .and_then(Value::as_object_mut)
        .ok_or_else(|| anyhow!("dynamic OpenAPI document must contain paths"))?;
    paths.retain(|path, _| path.starts_with("/api/ex/"));
    Ok(document)
}

pub async fn dynamic_openapi(State(state): State<Arc<ApiState>>) -> Result<Json<Value>, ApiError> {
    Ok(Json(dynamic_openapi_document(&state).await?))
}

fn append_workflow_extension_paths(
    document: &mut Value,
    operations: &[PublishedWorkflowOperation],
) {
    let Some(paths) = document.get_mut("paths").and_then(Value::as_object_mut) else {
        return;
    };

    for published_operation in operations {
        let operation = workflow_extension_operation(published_operation);
        let path = published_operation.public_path();
        let method = published_operation.method.as_str().to_ascii_lowercase();
        let entry = paths
            .entry(path)
            .or_insert_with(|| Value::Object(Map::new()));
        if let Some(path_item) = entry.as_object_mut() {
            path_item.insert(method, operation);
        }
    }
}

pub(crate) fn workflow_extension_operation(operation: &PublishedWorkflowOperation) -> Value {
    let mut projected = json!({
        "tags": ["Workflow Extensions"],
        "operationId": operation.interface_id,
        "summary": format!("Invoke published workflow {}", operation.application_id),
        "parameters": openapi_parameters(&operation.parameter_schema),
        "security": [
            { "sessionCookie": [], "csrfHeader": [] },
            { "UserApiKey": [] }
        ],
        "responses": {
            "202": {
                "description": "Workflow run accepted",
                "content": {
                    "application/json": {
                        "schema": accepted_run_schema()
                    }
                }
            },
            "400": native_error_response(),
            "401": native_error_response(),
            "403": native_error_response(),
            "404": native_error_response(),
            "405": native_error_response(),
            "409": native_error_response()
        }
    });
    if operation.response_mode == WorkflowExtensionResponseMode::Sync {
        projected["responses"]["200"] = json!({
            "description": "Workflow end output",
            "content": {
                "application/json": {
                    "schema": operation.result_schema
                }
            }
        });
    }
    if let Some(request_body) = openapi_request_body(&operation.parameter_schema) {
        projected["requestBody"] = request_body;
    }
    projected
}

fn openapi_parameters(schema: &Value) -> Vec<Value> {
    ["path", "query"]
        .into_iter()
        .flat_map(|location| {
            let object = schema.pointer(&format!("/properties/{location}"));
            let required = object
                .and_then(|value| value.get("required"))
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            object
                .and_then(|value| value.get("properties"))
                .and_then(Value::as_object)
                .into_iter()
                .flatten()
                .map(move |(name, field_schema)| json!({
                    "name": name,
                    "in": location,
                    "required": location == "path" || required.contains(&Value::String(name.clone())),
                    "schema": field_schema,
                }))
        })
        .collect()
}

fn openapi_request_body(schema: &Value) -> Option<Value> {
    let body_schema = schema.pointer("/properties/body").cloned();
    let form_schema = schema.pointer("/properties/form").cloned();
    if body_schema.is_none() && form_schema.is_none() {
        return None;
    }

    let mut content = Map::new();
    if let Some(schema) = body_schema {
        content.insert("application/json".to_string(), json!({ "schema": schema }));
    }
    if let Some(schema) = form_schema {
        content.insert(
            "application/x-www-form-urlencoded".to_string(),
            json!({ "schema": schema }),
        );
    }
    Some(json!({
        "required": true,
        "content": Value::Object(content)
    }))
}

fn accepted_run_schema() -> Value {
    json!({
        "type": "object",
        "required": ["run_id", "status"],
        "properties": {
            "run_id": { "type": "string", "format": "uuid" },
            "status": { "type": "string" }
        }
    })
}

fn native_error_response() -> Value {
    json!({
        "description": "Workflow extension API error",
        "content": {
            "application/json": {
                "schema": { "$ref": "#/components/schemas/NativeErrorBody" }
            }
        }
    })
}

#[cfg(test)]
mod workflow_operation_tests {
    use super::*;
    use control_plane::application_public_api::{
        mapping::{
            ApplicationApiMappingConfig, ApplicationApiMappingInput, ApplicationApiMappingOutput,
            ApplicationOperationBindings, WorkflowExtensionApiConfig, WorkflowExtensionHttpMethod,
        },
        publications::ApplicationPublicationVersionRecord,
        published_workflow_operation::PublishedWorkflowOperation,
    };
    use time::OffsetDateTime;
    use uuid::Uuid;

    fn operation() -> PublishedWorkflowOperation {
        let application_id = Uuid::from_u128(0x11111111111111111111111111111111);
        PublishedWorkflowOperation::from_publication(ApplicationPublicationVersionRecord {
            id: Uuid::from_u128(0x22222222222222222222222222222222),
            application_id,
            workspace_id: Uuid::from_u128(0x33333333333333333333333333333333),
            flow_id: Uuid::from_u128(0x44444444444444444444444444444444),
            flow_version_id: Uuid::from_u128(0x55555555555555555555555555555555),
            mapping_snapshot: ApplicationApiMappingConfig {
                input: ApplicationApiMappingInput {
                    query_target: "node-workflow-start.query".into(),
                    model_target: None,
                    inputs_target: None,
                    history_target: None,
                    attachments_target: None,
                },
                output: ApplicationApiMappingOutput::default(),
                extension: Some(WorkflowExtensionApiConfig {
                    slug: "orders/{order_id}".into(),
                    method: WorkflowExtensionHttpMethod::Post,
                    response_mode: WorkflowExtensionResponseMode::Sync,
                }),
            },
            operation_bindings: ApplicationOperationBindings::default(),
            extension_slug: Some("orders/{order_id}".into()),
            compiled_plan_id: Uuid::from_u128(0x66666666666666666666666666666666),
            version_sequence: 1,
            active: true,
            api_enabled: true,
            flow_schema_version: "1flowbase.flow/v2".into(),
            document_hash: "hash".into(),
            document_snapshot: json!({
                "graph": { "nodes": [
                    { "id": "node-workflow-start", "type": "workflow_start", "config": { "input_fields": [
                        { "key": "order_id", "valueType": "string", "source": "path", "required": true }
                    ] } },
                    { "id": "node-workflow-end", "type": "workflow_end", "outputs": [
                        { "key": "accepted", "valueType": "boolean" }
                    ] }
                ] }
            }),
            runtime_profile_snapshot: json!({}),
            output_selector: json!({}),
            dependency_snapshot: Vec::new(),
            created_by: Uuid::from_u128(0x77777777777777777777777777777777),
            created_at: OffsetDateTime::UNIX_EPOCH,
        })
        .unwrap()
    }

    #[test]
    fn ac_006_openapi_projects_start_end_and_current_user_or_api_key_security() {
        let projected = workflow_extension_operation(&operation());
        assert_eq!(
            projected["security"],
            json!([
                { "sessionCookie": [], "csrfHeader": [] },
                { "UserApiKey": [] }
            ])
        );
        assert_eq!(projected["parameters"][0]["name"], json!("order_id"));
        assert_eq!(
            projected["responses"]["200"]["content"]["application/json"]["schema"]["properties"]
                ["accepted"]["type"],
            json!("boolean")
        );
    }
}
