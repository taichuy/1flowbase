use std::sync::Arc;

use control_plane::ports::CacheStore;
use control_plane::{
    application::ApplicationService,
    application_public_api::{
        mapping::{
            ApplicationApiMappingService, GetApplicationApiMappingCommand,
            ReplaceApplicationApiMappingCommand,
        },
        publications::{
            ApplicationPublicationService, LoadActiveApplicationPublicationCommand,
            PublishApplicationCommand, SetApplicationApiEnabledCommand,
            UnpublishApplicationCommand,
        },
    },
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    map_publication_not_found, to_mapping_body, to_mapping_config, to_publication_response,
    ApplicationApiMappingBody, ApplicationApiStatusResponse, ApplicationPublicationResponse,
    PatchApplicationApiStatusBody, PublishApplicationApiBody, PUBLIC_RUNS_PATH,
};
use crate::{
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
};

pub(crate) enum ApplicationPublicationInput {
    GetMapping {
        application_id: Uuid,
    },
    ReplaceMapping {
        application_id: Uuid,
        body: ApplicationApiMappingBody,
    },
    GetPublication {
        application_id: Uuid,
    },
    Publish {
        application_id: Uuid,
        body: PublishApplicationApiBody,
    },
    Unpublish {
        application_id: Uuid,
    },
    SetEnabled {
        application_id: Uuid,
        body: PatchApplicationApiStatusBody,
    },
}

impl InterfaceContract for ApplicationPublicationInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetMapping")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceMapping")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "input",
                            mp::object_schema(&[
                                (
                                    "query_target",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "model_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "inputs_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "history_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "attachments_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "output",
                            mp::object_schema(&[
                                (
                                    "answer_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "usage_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "files_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "error_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "extension",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("slug",mp::object_schema(&[("byte_count",mp::count_schema())])), ("method",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Get"))]), mp::object_schema(&[("variant",mp::tag_schema("Post"))]), mp::object_schema(&[("variant",mp::tag_schema("Put"))]), mp::object_schema(&[("variant",mp::tag_schema("Patch"))]), mp::object_schema(&[("variant",mp::tag_schema("Delete"))]), mp::object_schema(&[("variant",mp::tag_schema("Head"))]), mp::object_schema(&[("variant",mp::tag_schema("Options"))])])), ("response_mode",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Sync"))]), mp::object_schema(&[("variant",mp::tag_schema("Async"))])]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetPublication")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Publish")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "mapping",
                            mp::object_schema(&[
                                (
                                    "input",
                                    mp::object_schema(&[
                                        (
                                            "query_target",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "model_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "inputs_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "history_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "attachments_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "output",
                                    mp::object_schema(&[
                                        (
                                            "answer_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "usage_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "files_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "error_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "extension",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("slug",mp::object_schema(&[("byte_count",mp::count_schema())])), ("method",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Get"))]), mp::object_schema(&[("variant",mp::tag_schema("Post"))]), mp::object_schema(&[("variant",mp::tag_schema("Put"))]), mp::object_schema(&[("variant",mp::tag_schema("Patch"))]), mp::object_schema(&[("variant",mp::tag_schema("Delete"))]), mp::object_schema(&[("variant",mp::tag_schema("Head"))]), mp::object_schema(&[("variant",mp::tag_schema("Options"))])])), ("response_mode",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Sync"))]), mp::object_schema(&[("variant",mp::tag_schema("Async"))])]))]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        ("api_enabled", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Unpublish")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SetEnabled")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("api_enabled", serde_json::json!({"type":"boolean"}))]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::GetMapping {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("GetMapping".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::ReplaceMapping {application_id: _field_application_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("ReplaceMapping".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("body",mp::object_value(&[("input",mp::object_value(&[("query_target",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_body).input).query_target).len()))])), ("model_target",match (&(&(_field_body).input).model_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("inputs_target",match (&(&(_field_body).input).inputs_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("history_target",match (&(&(_field_body).input).history_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("attachments_target",match (&(&(_field_body).input).attachments_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("output",mp::object_value(&[("answer_selector",match (&(&(_field_body).output).answer_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("usage_selector",match (&(&(_field_body).output).usage_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("files_selector",match (&(&(_field_body).output).files_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("error_selector",match (&(&(_field_body).output).error_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("extension",match (&(_field_body).extension).as_ref() { Some(item) => mp::object_value(&[("slug",mp::object_value(&[("byte_count",serde_json::json!((&(item).slug).len()))])), ("method",match &(item).method {crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Get => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Post => mp::object_value(&[("variant",serde_json::Value::String("Post".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Put => mp::object_value(&[("variant",serde_json::Value::String("Put".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Patch => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Delete => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Head => mp::object_value(&[("variant",serde_json::Value::String("Head".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Options => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned()))])}), ("response_mode",match &(item).response_mode {crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Sync => mp::object_value(&[("variant",serde_json::Value::String("Sync".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Async => mp::object_value(&[("variant",serde_json::Value::String("Async".to_owned()))])})]), None => serde_json::Value::Null })]))]), Self::GetPublication {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("GetPublication".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::Publish {application_id: _field_application_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("Publish".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("body",mp::object_value(&[("mapping",mp::object_value(&[("input",mp::object_value(&[("query_target",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_body).mapping).input).query_target).len()))])), ("model_target",match (&(&(&(_field_body).mapping).input).model_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("inputs_target",match (&(&(&(_field_body).mapping).input).inputs_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("history_target",match (&(&(&(_field_body).mapping).input).history_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("attachments_target",match (&(&(&(_field_body).mapping).input).attachments_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("output",mp::object_value(&[("answer_selector",match (&(&(&(_field_body).mapping).output).answer_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("usage_selector",match (&(&(&(_field_body).mapping).output).usage_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("files_selector",match (&(&(&(_field_body).mapping).output).files_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("error_selector",match (&(&(&(_field_body).mapping).output).error_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("extension",match (&(&(_field_body).mapping).extension).as_ref() { Some(item) => mp::object_value(&[("slug",mp::object_value(&[("byte_count",serde_json::json!((&(item).slug).len()))])), ("method",match &(item).method {crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Get => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Post => mp::object_value(&[("variant",serde_json::Value::String("Post".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Put => mp::object_value(&[("variant",serde_json::Value::String("Put".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Patch => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Delete => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Head => mp::object_value(&[("variant",serde_json::Value::String("Head".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Options => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned()))])}), ("response_mode",match &(item).response_mode {crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Sync => mp::object_value(&[("variant",serde_json::Value::String("Sync".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Async => mp::object_value(&[("variant",serde_json::Value::String("Async".to_owned()))])})]), None => serde_json::Value::Null })])), ("api_enabled",serde_json::Value::Bool(*(&(_field_body).api_enabled)))]))]), Self::Unpublish {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("Unpublish".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::SetEnabled {application_id: _field_application_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("SetEnabled".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("body",mp::object_value(&[("api_enabled",serde_json::Value::Bool(*(&(_field_body).api_enabled)))]))])})
    }

    const CONTRACT_ID: &'static str = "console-application-publication-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed output is projected immediately into the console response"
)]
pub(crate) enum ApplicationPublicationOutput {
    Mapping(ApplicationApiMappingBody),
    Publication(ApplicationPublicationResponse),
    Status(ApplicationApiStatusResponse),
    NoContent,
}

impl ApplicationPublicationOutput {
    pub(super) fn into_mapping(self) -> Result<ApplicationApiMappingBody, ApiError> {
        match self {
            Self::Mapping(value) => Ok(value),
            _ => Err(output_error()),
        }
    }

    pub(super) fn into_publication(self) -> Result<ApplicationPublicationResponse, ApiError> {
        match self {
            Self::Publication(value) => Ok(value),
            _ => Err(output_error()),
        }
    }

    pub(super) fn into_status(self) -> Result<ApplicationApiStatusResponse, ApiError> {
        match self {
            Self::Status(value) => Ok(value),
            _ => Err(output_error()),
        }
    }
}

fn output_error() -> ApiError {
    control_plane::errors::ControlPlaneError::InvalidInput("application_publication_output").into()
}

impl InterfaceContract for ApplicationPublicationOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Mapping")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "input",
                            mp::object_schema(&[
                                (
                                    "query_target",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "model_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "inputs_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "history_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "attachments_target",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "output",
                            mp::object_schema(&[
                                (
                                    "answer_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "usage_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "files_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "error_selector",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "extension",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("slug",mp::object_schema(&[("byte_count",mp::count_schema())])), ("method",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Get"))]), mp::object_schema(&[("variant",mp::tag_schema("Post"))]), mp::object_schema(&[("variant",mp::tag_schema("Put"))]), mp::object_schema(&[("variant",mp::tag_schema("Patch"))]), mp::object_schema(&[("variant",mp::tag_schema("Delete"))]), mp::object_schema(&[("variant",mp::tag_schema("Head"))]), mp::object_schema(&[("variant",mp::tag_schema("Options"))])])), ("response_mode",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Sync"))]), mp::object_schema(&[("variant",mp::tag_schema("Async"))])]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Publication")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("application_id", mp::text_schema()),
                        ("flow_id", mp::text_schema()),
                        ("flow_version_id", mp::text_schema()),
                        ("compiled_plan_id", mp::text_schema()),
                        ("version_sequence", serde_json::json!({"type":"integer"})),
                        ("active", serde_json::json!({"type":"boolean"})),
                        ("api_enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "mapping_snapshot",
                            mp::object_schema(&[
                                (
                                    "input",
                                    mp::object_schema(&[
                                        (
                                            "query_target",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "model_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "inputs_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "history_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "attachments_target",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "output",
                                    mp::object_schema(&[
                                        (
                                            "answer_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "usage_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "files_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "error_selector",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "extension",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("slug",mp::object_schema(&[("byte_count",mp::count_schema())])), ("method",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Get"))]), mp::object_schema(&[("variant",mp::tag_schema("Post"))]), mp::object_schema(&[("variant",mp::tag_schema("Put"))]), mp::object_schema(&[("variant",mp::tag_schema("Patch"))]), mp::object_schema(&[("variant",mp::tag_schema("Delete"))]), mp::object_schema(&[("variant",mp::tag_schema("Head"))]), mp::object_schema(&[("variant",mp::tag_schema("Options"))])])), ("response_mode",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Sync"))]), mp::object_schema(&[("variant",mp::tag_schema("Async"))])]))]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "operation",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("interface_id",mp::text_schema()), ("method",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Get"))]), mp::object_schema(&[("variant",mp::tag_schema("Post"))]), mp::object_schema(&[("variant",mp::tag_schema("Put"))]), mp::object_schema(&[("variant",mp::tag_schema("Patch"))]), mp::object_schema(&[("variant",mp::tag_schema("Delete"))]), mp::object_schema(&[("variant",mp::tag_schema("Head"))]), mp::object_schema(&[("variant",mp::tag_schema("Options"))])])), ("route_template",mp::object_schema(&[("byte_count",mp::count_schema())])), ("response_mode",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Sync"))]), mp::object_schema(&[("variant",mp::tag_schema("Async"))])])), ("parameter_schema",mp::json_summary_schema()), ("result_schema",mp::json_summary_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "dependency_snapshot",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("installation_id",mp::text_schema()), ("provider_code",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("package",mp::object_schema(&[("byte_count",mp::count_schema())])), ("version",mp::text_schema()), ("target",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_hash",mp::object_schema(&[("byte_count",mp::count_schema())])), ("integrity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("permissions",mp::object_schema(&[("network",mp::object_schema(&[("byte_count",mp::count_schema())])), ("filesystem",mp::object_schema(&[("byte_count",mp::count_schema())])), ("env",mp::object_schema(&[("byte_count",mp::count_schema())]))]))])}),
                        ),
                        ("created_by", mp::text_schema()),
                        ("created_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Status")),
                (
                    "0",
                    mp::object_schema(&[
                        ("application_id", mp::text_schema()),
                        ("api_enabled", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Mapping(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Mapping".to_owned())), ("0",mp::object_value(&[("input",mp::object_value(&[("query_target",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).input).query_target).len()))])), ("model_target",match (&(&(_field_0).input).model_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("inputs_target",match (&(&(_field_0).input).inputs_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("history_target",match (&(&(_field_0).input).history_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("attachments_target",match (&(&(_field_0).input).attachments_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("output",mp::object_value(&[("answer_selector",match (&(&(_field_0).output).answer_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("usage_selector",match (&(&(_field_0).output).usage_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("files_selector",match (&(&(_field_0).output).files_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("error_selector",match (&(&(_field_0).output).error_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("extension",match (&(_field_0).extension).as_ref() { Some(item) => mp::object_value(&[("slug",mp::object_value(&[("byte_count",serde_json::json!((&(item).slug).len()))])), ("method",match &(item).method {crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Get => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Post => mp::object_value(&[("variant",serde_json::Value::String("Post".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Put => mp::object_value(&[("variant",serde_json::Value::String("Put".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Patch => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Delete => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Head => mp::object_value(&[("variant",serde_json::Value::String("Head".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Options => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned()))])}), ("response_mode",match &(item).response_mode {crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Sync => mp::object_value(&[("variant",serde_json::Value::String("Sync".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Async => mp::object_value(&[("variant",serde_json::Value::String("Async".to_owned()))])})]), None => serde_json::Value::Null })]))]), Self::Publication(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Publication".to_owned())), ("0",mp::object_value(&[("id",serde_json::Value::String((&(_field_0).id).to_string())), ("application_id",serde_json::Value::String((&(_field_0).application_id).to_string())), ("flow_id",serde_json::Value::String((&(_field_0).flow_id).to_string())), ("flow_version_id",serde_json::Value::String((&(_field_0).flow_version_id).to_string())), ("compiled_plan_id",serde_json::Value::String((&(_field_0).compiled_plan_id).to_string())), ("version_sequence",serde_json::json!(*(&(_field_0).version_sequence))), ("active",serde_json::Value::Bool(*(&(_field_0).active))), ("api_enabled",serde_json::Value::Bool(*(&(_field_0).api_enabled))), ("mapping_snapshot",mp::object_value(&[("input",mp::object_value(&[("query_target",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).mapping_snapshot).input).query_target).len()))])), ("model_target",match (&(&(&(_field_0).mapping_snapshot).input).model_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("inputs_target",match (&(&(&(_field_0).mapping_snapshot).input).inputs_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("history_target",match (&(&(&(_field_0).mapping_snapshot).input).history_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("attachments_target",match (&(&(&(_field_0).mapping_snapshot).input).attachments_target).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("output",mp::object_value(&[("answer_selector",match (&(&(&(_field_0).mapping_snapshot).output).answer_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("usage_selector",match (&(&(&(_field_0).mapping_snapshot).output).usage_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("files_selector",match (&(&(&(_field_0).mapping_snapshot).output).files_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("error_selector",match (&(&(&(_field_0).mapping_snapshot).output).error_selector).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("extension",match (&(&(_field_0).mapping_snapshot).extension).as_ref() { Some(item) => mp::object_value(&[("slug",mp::object_value(&[("byte_count",serde_json::json!((&(item).slug).len()))])), ("method",match &(item).method {crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Get => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Post => mp::object_value(&[("variant",serde_json::Value::String("Post".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Put => mp::object_value(&[("variant",serde_json::Value::String("Put".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Patch => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Delete => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Head => mp::object_value(&[("variant",serde_json::Value::String("Head".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Options => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned()))])}), ("response_mode",match &(item).response_mode {crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Sync => mp::object_value(&[("variant",serde_json::Value::String("Sync".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Async => mp::object_value(&[("variant",serde_json::Value::String("Async".to_owned()))])})]), None => serde_json::Value::Null })])), ("operation",match (&(_field_0).operation).as_ref() { Some(item) => mp::object_value(&[("interface_id",mp::text(&(item).interface_id)?), ("method",match &(item).method {crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Get => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Post => mp::object_value(&[("variant",serde_json::Value::String("Post".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Put => mp::object_value(&[("variant",serde_json::Value::String("Put".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Patch => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Delete => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Head => mp::object_value(&[("variant",serde_json::Value::String("Head".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Options => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned()))])}), ("route_template",mp::object_value(&[("byte_count",serde_json::json!((&(item).route_template).len()))])), ("response_mode",match &(item).response_mode {crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Sync => mp::object_value(&[("variant",serde_json::Value::String("Sync".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Async => mp::object_value(&[("variant",serde_json::Value::String("Async".to_owned()))])}), ("parameter_schema",mp::json_summary(&(item).parameter_schema)), ("result_schema",mp::json_summary(&(item).result_schema))]), None => serde_json::Value::Null }), ("dependency_snapshot",{ if (&(_field_0).dependency_snapshot).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).dependency_snapshot).iter().map(|item| Some(mp::object_value(&[("installation_id",serde_json::Value::String((&(item).installation_id).to_string())), ("provider_code",mp::text(&(item).provider_code)?), ("plugin_id",mp::text(&(item).plugin_id)?), ("plugin_version",mp::text(&(item).plugin_version)?), ("alias",mp::object_value(&[("byte_count",serde_json::json!((&(item).alias).len()))])), ("package",mp::object_value(&[("byte_count",serde_json::json!((&(item).package).len()))])), ("version",mp::text(&(item).version)?), ("target",mp::object_value(&[("byte_count",serde_json::json!((&(item).target).len()))])), ("artifact_path",mp::object_value(&[("byte_count",serde_json::json!((&(item).artifact_path).len()))])), ("artifact_hash",mp::object_value(&[("byte_count",serde_json::json!((&(item).artifact_hash).len()))])), ("integrity",mp::object_value(&[("byte_count",serde_json::json!((&(item).integrity).len()))])), ("permissions",mp::object_value(&[("network",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).permissions).network).len()))])), ("filesystem",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).permissions).filesystem).len()))])), ("env",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).permissions).env).len()))]))]))]))).collect::<Option<Vec<_>>>()?) }), ("created_by",serde_json::Value::String((&(_field_0).created_by).to_string())), ("created_at",mp::text(&(_field_0).created_at)?)]))]), Self::Status(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Status".to_owned())), ("0",mp::object_value(&[("application_id",serde_json::Value::String((&(_field_0).application_id).to_string())), ("api_enabled",serde_json::Value::Bool(*(&(_field_0).api_enabled)))]))]), Self::NoContent => mp::object_value(&[("variant",serde_json::Value::String("NoContent".to_owned()))])})
    }

    const CONTRACT_ID: &'static str = "console-application-publication-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationPublicationAdapter {
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
}

pub(crate) fn port(
    store: MainDurableStore,
    cache_store: Arc<dyn CacheStore>,
) -> Arc<dyn ConsoleInterfacePort<ApplicationPublicationInput, ApplicationPublicationOutput>> {
    Arc::new(ApplicationPublicationAdapter { store, cache_store })
}

impl ConsoleInterfacePort<ApplicationPublicationInput, ApplicationPublicationOutput>
    for ApplicationPublicationAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationPublicationInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationPublicationOutput> {
        Box::pin(async move {
            let result: Result<ApplicationPublicationOutput, ApiError> = async {
                let actor = principal.actor();
                let actor_store = self.store.for_actor(actor.clone());
                let output = match input {
                    ApplicationPublicationInput::GetMapping { application_id } => {
                        let draft = ApplicationApiMappingService::new(actor_store)
                            .get_mapping_draft(GetApplicationApiMappingCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                            })
                            .await?;
                        ApplicationPublicationOutput::Mapping(to_mapping_body(draft.mapping))
                    }
                    ApplicationPublicationInput::ReplaceMapping {
                        application_id,
                        body,
                    } => {
                        let draft = ApplicationApiMappingService::new(actor_store)
                            .replace_mapping_draft(ReplaceApplicationApiMappingCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                mapping: to_mapping_config(body),
                            })
                            .await?;
                        ApplicationPublicationOutput::Mapping(to_mapping_body(draft.mapping))
                    }
                    ApplicationPublicationInput::GetPublication { application_id } => {
                        ApplicationService::new(actor_store.clone())
                            .get_application(actor.user_id, application_id)
                            .await?;
                        let publication = ApplicationPublicationService::new(actor_store)
                            .load_active_publication(LoadActiveApplicationPublicationCommand {
                                application_id,
                            })
                            .await
                            .map_err(map_publication_not_found)?;
                        ApplicationPublicationOutput::Publication(to_publication_response(
                            publication,
                        ))
                    }
                    ApplicationPublicationInput::Publish {
                        application_id,
                        body,
                    } => {
                        let publication = ApplicationPublicationService::new(actor_store)
                            .with_model_routing_cache_store(Arc::clone(&self.cache_store))
                            .publish_active_version(PublishApplicationCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                mapping: to_mapping_config(body.mapping),
                                api_enabled: body.api_enabled,
                            })
                            .await?;
                        ApplicationPublicationOutput::Publication(to_publication_response(
                            publication,
                        ))
                    }
                    ApplicationPublicationInput::Unpublish { application_id } => {
                        ApplicationPublicationService::new(actor_store)
                            .unpublish(UnpublishApplicationCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                            })
                            .await
                            .map_err(map_publication_not_found)?;
                        ApplicationPublicationOutput::NoContent
                    }
                    ApplicationPublicationInput::SetEnabled {
                        application_id,
                        body,
                    } => {
                        ApplicationPublicationService::new(actor_store)
                            .set_api_enabled(SetApplicationApiEnabledCommand {
                                actor_user_id: actor.user_id,
                                application_id,
                                api_enabled: body.api_enabled,
                            })
                            .await?;
                        ApplicationPublicationOutput::Status(ApplicationApiStatusResponse {
                            application_id,
                            api_enabled: body.api_enabled,
                            public_url: PUBLIC_RUNS_PATH.to_string(),
                        })
                    }
                };
                Ok(output)
            }
            .await;
            result.map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-mapping.get",
        binding_id: "http.console.applications.api-mapping.get.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-mapping",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-mapping.replace",
        binding_id: "http.console.applications.api-mapping.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:application_id/api-mapping",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-publication.get",
        binding_id: "http.console.applications.api-publication.get.v1",
        method: "GET",
        path: "/api/console/applications/:application_id/api-publication",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-publication.publish",
        binding_id: "http.console.applications.api-publication.publish.v1",
        method: "POST",
        path: "/api/console/applications/:application_id/api-publications",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-publication.unpublish",
        binding_id: "http.console.applications.api-publication.unpublish.v1",
        method: "DELETE",
        path: "/api/console/applications/:application_id/api-publication",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.api-status.update",
        binding_id: "http.console.applications.api-status.update.v1",
        method: "PATCH",
        path: "/api/console/applications/:application_id/api-status",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationPublicationInput, ApplicationPublicationOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-application-publication",
        "api-server.console-application-publication.graph.v1",
        DECLARATIONS,
        port,
    )
}
