// Original category ordinals retain utoipa path and schema collision precedence.
use utoipa::OpenApi;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::plugins::extension_center::contribution_authority::grant_contribution_authorizations,
    crate::routes::plugins::extension_center::contribution_authority::revoke_contribution_authorizations,
    crate::routes::plugins::extension_center::contribution_authority::view_contribution_authorizations,
    crate::routes::plugins::extension_center::disable_installed_plugin,
    crate::routes::plugins::extension_center::enable_installed_plugin,
    crate::routes::plugins::extension_center::managed_execution::resume_managed_delivery,
    crate::routes::plugins::extension_center::managed_execution::view_managed_execution,
    crate::routes::plugins::extension_center::managed_execution::retire_managed_execution,
))]
struct Paths0;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::model_providers::authenticate_instance,
    crate::routes::model_providers::consume_reset_credit,
    crate::routes::model_providers::count_reset_credits,
    crate::routes::model_providers::get_usage_windows,
    crate::routes::plugins::install_current_node_artifact,
    crate::routes::plugins::refresh_current_node_artifact,
))]
struct Paths12;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::model_providers::icons::read_provider_icon,
    crate::routes::runtime_models::metadata_interface::get_runtime_model_metadata,
))]
struct Paths74;

#[derive(OpenApi)]
#[openapi(paths(
    crate::managed_services::record_permissions::get,
    crate::managed_services::record_permissions::replace,
    crate::managed_services::record_permissions::capabilities,
))]
struct Paths238;

#[derive(OpenApi)]
#[openapi(paths(
    crate::routes::model_definitions::list_models,
    crate::routes::model_definitions::list_compatible_templates,
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
))]
struct Paths258;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::model_definitions::CreateModelDefinitionBody,
    crate::routes::model_definitions::BatchDeleteModelDefinitionsBody,
    crate::routes::model_definitions::UpdateModelDefinitionBody,
    crate::routes::model_definitions::CreateModelFieldBody,
    crate::routes::model_definitions::UpdateModelFieldBody,
    crate::routes::model_definitions::CreateScopeGrantBody,
    crate::routes::model_definitions::UpdateScopeGrantBody,
    crate::routes::model_definitions::ModelDefinitionResponse,
    crate::routes::model_definitions::CompatibleTemplateCatalogEntryResponse,
    crate::routes::model_definitions::CompatibleTemplateSystemFieldResponse,
    crate::routes::model_definitions::AgentFlowDataModelFieldOptionResponse,
    crate::routes::model_definitions::AgentFlowDataModelOptionResponse,
    crate::routes::model_definitions::ModelFieldResponse,
    crate::routes::model_definitions::DataModelAdvisorFindingResponse,
    crate::routes::model_definitions::ScopeGrantResponse,
    crate::routes::model_definitions::DeletedResponse,
    crate::routes::model_definitions::BatchDeletedResponse,
)))]
struct Schemas67;

#[derive(OpenApi)]
#[openapi(components(schemas(
    crate::routes::runtime_models::metadata::RuntimeDataModelMetadata,
    crate::routes::runtime_models::metadata::RuntimeDataModelField,
    crate::routes::runtime_models::metadata::RuntimeDataModelRecordCapabilities,
    crate::routes::runtime_models::RuntimeListResponse,
    crate::routes::runtime_models::RuntimeRecordEnvelope,
)))]
struct Schemas118;

#[doc(hidden)]
pub fn paths_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![
        (0, Paths0::openapi()),
        (12, Paths12::openapi()),
        (74, Paths74::openapi()),
        (238, Paths238::openapi()),
        (258, Paths258::openapi()),
    ]
}

#[doc(hidden)]
pub fn schemas_fragments() -> Vec<(usize, utoipa::openapi::OpenApi)> {
    vec![(67, Schemas67::openapi()), (118, Schemas118::openapi())]
}
