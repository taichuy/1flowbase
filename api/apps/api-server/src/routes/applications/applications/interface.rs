use std::sync::Arc;

use control_plane::{
    application::{
        ApplicationService, CreateApplicationCommand, CreateApplicationTagCommand,
        DeleteApplicationCommand, ReplaceApplicationEnvironmentVariablesCommand,
        UpdateApplicationCommand,
    },
    errors::ControlPlaneError,
    js_dependency::{
        ApplicationJsDependencyService, ReplaceApplicationJsDependencySelectionCommand,
    },
    ports::{ApplicationEnvironmentVariableInput, CreateWorkflowTriggerConfig},
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;
use uuid::Uuid;

use super::{
    ApplicationCatalogResponse, ApplicationDetailResponse, ApplicationEnvironmentVariableResponse,
    ApplicationJsDependencySelectionResponse, ApplicationSummaryResponse,
    ApplicationTagCatalogResponse, ApplicationTypeDto, ApplicationTypeOptionResponse,
    CreateApplicationBody, CreateApplicationTagBody, CreateWorkflowTriggerConfigBody,
    PatchApplicationBody, ReplaceApplicationEnvironmentVariablesBody,
    ReplaceApplicationJsDependencySelectionBody, WorkflowTriggerTypeDto,
    WorkflowTriggerTypeOptionResponse,
};
use crate::routes::application_api::{
    WorkflowExtensionHttpMethodBody, WorkflowExtensionResponseModeBody,
};
use crate::{
    app_state::resolve_request_text_with,
    error_response::ApiError,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum ApplicationsInput {
    List,
    Catalog {
        locale: ConsoleLocaleHints,
    },
    Create(CreateApplicationBody),
    CreateTag(CreateApplicationTagBody),
    Get {
        application_id: Uuid,
    },
    Patch {
        application_id: Uuid,
        body: PatchApplicationBody,
    },
    Delete {
        application_id: Uuid,
    },
    ListEnvironmentVariables {
        application_id: Uuid,
    },
    ReplaceEnvironmentVariables {
        application_id: Uuid,
        body: ReplaceApplicationEnvironmentVariablesBody,
    },
    ListJsDependencies {
        application_id: Uuid,
    },
    ReplaceJsDependency {
        application_id: Uuid,
        body: ReplaceApplicationJsDependencySelectionBody,
    },
}

impl InterfaceContract for ApplicationsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[("variant", mp::tag_schema("Catalog"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "application_type",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("AgentFlow"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Workflow"))]),
                            ]),
                        ),
                        (
                            "workflow_trigger_type",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Extension"))]), mp::object_schema(&[("variant",mp::tag_schema("Schedule"))])]), {"type":"null"}]}),
                        ),
                        (
                            "workflow_trigger_config",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("cron",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("timezone",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("input_payload",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("subpath",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("http_method",serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Get"))]), mp::object_schema(&[("variant",mp::tag_schema("Post"))]), mp::object_schema(&[("variant",mp::tag_schema("Put"))]), mp::object_schema(&[("variant",mp::tag_schema("Patch"))]), mp::object_schema(&[("variant",mp::tag_schema("Delete"))]), mp::object_schema(&[("variant",mp::tag_schema("Head"))]), mp::object_schema(&[("variant",mp::tag_schema("Options"))])]), {"type":"null"}]})), ("response_mode",serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Sync"))]), mp::object_schema(&[("variant",mp::tag_schema("Async"))])]), {"type":"null"}]}))]), {"type":"null"}]}),
                        ),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "icon",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "icon_type",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "icon_background",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateTag")),
                (
                    "0",
                    mp::object_schema(&[(
                        "name",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Patch")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "tag_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "icon",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "icon_type",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "icon_background",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListEnvironmentVariables")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceEnvironmentVariables")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "variables",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("value_type",mp::text_schema()), ("value",mp::json_summary_schema()), ("description",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListJsDependencies")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReplaceJsDependency")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("installation_id", mp::text_schema()),
                        (
                            "alias",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "target",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::List => mp::object_value(&[("variant",serde_json::Value::String("List".to_owned()))]), Self::Catalog { .. } => mp::object_value(&[("variant",serde_json::Value::String("Catalog".to_owned()))]), Self::Create(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned())), ("0",mp::object_value(&[("application_type",match &(_field_0).application_type {crate::routes::applications_group::applications::ApplicationTypeDto::AgentFlow => mp::object_value(&[("variant",serde_json::Value::String("AgentFlow".to_owned()))]), crate::routes::applications_group::applications::ApplicationTypeDto::Workflow => mp::object_value(&[("variant",serde_json::Value::String("Workflow".to_owned()))])}), ("workflow_trigger_type",match (&(_field_0).workflow_trigger_type).as_ref() { Some(item) => match item {crate::routes::applications_group::applications::WorkflowTriggerTypeDto::Extension => mp::object_value(&[("variant",serde_json::Value::String("Extension".to_owned()))]), crate::routes::applications_group::applications::WorkflowTriggerTypeDto::Schedule => mp::object_value(&[("variant",serde_json::Value::String("Schedule".to_owned()))])}, None => serde_json::Value::Null }), ("workflow_trigger_config",match (&(_field_0).workflow_trigger_config).as_ref() { Some(item) => mp::object_value(&[("cron",match (&(item).cron).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("timezone",match (&(item).timezone).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("input_payload",match (&(item).input_payload).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("subpath",match (&(item).subpath).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("http_method",match (&(item).http_method).as_ref() { Some(item) => match item {crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Get => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Post => mp::object_value(&[("variant",serde_json::Value::String("Post".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Put => mp::object_value(&[("variant",serde_json::Value::String("Put".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Patch => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Delete => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Head => mp::object_value(&[("variant",serde_json::Value::String("Head".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionHttpMethodBody::Options => mp::object_value(&[("variant",serde_json::Value::String("Options".to_owned()))])}, None => serde_json::Value::Null }), ("response_mode",match (&(item).response_mode).as_ref() { Some(item) => match item {crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Sync => mp::object_value(&[("variant",serde_json::Value::String("Sync".to_owned()))]), crate::routes::applications_group::application_api::WorkflowExtensionResponseModeBody::Async => mp::object_value(&[("variant",serde_json::Value::String("Async".to_owned()))])}, None => serde_json::Value::Null })]), None => serde_json::Value::Null }), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).description).len()))])), ("icon",match (&(_field_0).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon_type",match (&(_field_0).icon_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("icon_background",match (&(_field_0).icon_background).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::CreateTag(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("CreateTag".to_owned())), ("0",mp::object_value(&[("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))]))]))]), Self::Get {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::Patch {application_id: _field_application_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("Patch".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("body",mp::object_value(&[("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).description).len()))])), ("tag_ids",{ if (&(_field_body).tag_ids).len() > 32 { return None; } serde_json::Value::Array((&(_field_body).tag_ids).iter().map(|item| Some(mp::text(item)?)).collect::<Option<Vec<_>>>()?) }), ("icon",match (&(_field_body).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon_type",match (&(_field_body).icon_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("icon_background",match (&(_field_body).icon_background).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Delete {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::ListEnvironmentVariables {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("ListEnvironmentVariables".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::ReplaceEnvironmentVariables {application_id: _field_application_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("ReplaceEnvironmentVariables".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("body",mp::object_value(&[("variables",{ if (&(_field_body).variables).len() > 32 { return None; } serde_json::Value::Array((&(_field_body).variables).iter().map(|item| Some(mp::object_value(&[("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("value_type",mp::text(&(item).value_type)?), ("value",mp::json_summary(&(item).value)), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(item).description).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::ListJsDependencies {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("ListJsDependencies".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::ReplaceJsDependency {application_id: _field_application_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("ReplaceJsDependency".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("body",mp::object_value(&[("installation_id",mp::text(&(_field_body).installation_id)?), ("alias",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).alias).len()))])), ("target",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).target).len()))]))]))])})
    }

    const CONTRACT_ID: &'static str = "console-applications-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed application output is projected immediately into the console response"
)]
pub(crate) enum ApplicationsOutput {
    Applications(Vec<ApplicationSummaryResponse>),
    Catalog(ApplicationCatalogResponse),
    Application(ApplicationDetailResponse),
    Tag(ApplicationTagCatalogResponse),
    EnvironmentVariables(Vec<ApplicationEnvironmentVariableResponse>),
    JsDependencies(Vec<ApplicationJsDependencySelectionResponse>),
    JsDependency(ApplicationJsDependencySelectionResponse),
    NoContent,
}

impl InterfaceContract for ApplicationsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Applications")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("application_type",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("AgentFlow"))]), mp::object_schema(&[("variant",mp::tag_schema("Workflow"))])])), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("icon",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("icon_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("icon_background",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("updated_at",mp::text_schema()), ("tags",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())]))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "types",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("value",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("AgentFlow"))]), mp::object_schema(&[("variant",mp::tag_schema("Workflow"))])])), ("label",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "workflow_triggers",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("value",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Extension"))]), mp::object_schema(&[("variant",mp::tag_schema("Schedule"))])])), ("label",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "tags",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("application_count",serde_json::json!({"type":"integer"}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Application")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "application_type",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("AgentFlow"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Workflow"))]),
                            ]),
                        ),
                        (
                            "workflow_trigger_type",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Extension"))]), mp::object_schema(&[("variant",mp::tag_schema("Schedule"))])]), {"type":"null"}]}),
                        ),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "icon",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "icon_type",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "icon_background",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "created_by",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("updated_at", mp::text_schema()),
                        (
                            "tags",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "sections",
                            mp::object_schema(&[
                                (
                                    "orchestration",
                                    mp::object_schema(&[
                                        ("status", mp::text_schema()),
                                        (
                                            "subject_kind",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "subject_status",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "current_subject_id",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "current_draft_id",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "api",
                                    mp::object_schema(&[
                                        (
                                            "status",
                                            mp::union_schema(vec![
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Active"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Planned"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Available"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Unavailable"),
                                                )]),
                                            ]),
                                        ),
                                        (
                                            "invoke_routing_mode",
                                            mp::union_schema(vec![
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("ApiKeyBoundApplication"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("PublishedWorkflowOperation"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("NotAvailable"),
                                                )]),
                                            ]),
                                        ),
                                        (
                                            "invoke_path_template",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "api_capability_status",
                                            mp::union_schema(vec![
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Enabled"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Disabled"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("NotPublished"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Available"),
                                                )]),
                                                mp::object_schema(&[(
                                                    "variant",
                                                    mp::tag_schema("Unavailable"),
                                                )]),
                                            ]),
                                        ),
                                    ]),
                                ),
                                (
                                    "logs",
                                    mp::object_schema(&[
                                        ("status", mp::text_schema()),
                                        (
                                            "runs_capability_status",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "run_object_kind",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "log_retention_status",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                    ]),
                                ),
                                (
                                    "monitoring",
                                    mp::object_schema(&[
                                        ("status", mp::text_schema()),
                                        (
                                            "metrics_capability_status",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "metrics_object_kind",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "tracing_config_status",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                    ]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tag")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("application_count", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("EnvironmentVariables")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("value_type",mp::text_schema()), ("value",mp::json_summary_schema()), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("updated_at",mp::text_schema())])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("JsDependencies")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("application_id",mp::text_schema()), ("installation_id",mp::text_schema()), ("provider_code",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("package",mp::object_schema(&[("byte_count",mp::count_schema())])), ("version",mp::text_schema()), ("target",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_hash",mp::object_schema(&[("byte_count",mp::count_schema())])), ("integrity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("permissions",mp::object_schema(&[("network",mp::object_schema(&[("byte_count",mp::count_schema())])), ("filesystem",mp::object_schema(&[("byte_count",mp::count_schema())])), ("env",mp::object_schema(&[("byte_count",mp::count_schema())]))]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("JsDependency")),
                (
                    "0",
                    mp::object_schema(&[
                        ("application_id", mp::text_schema()),
                        ("installation_id", mp::text_schema()),
                        ("provider_code", mp::text_schema()),
                        ("plugin_id", mp::text_schema()),
                        ("plugin_version", mp::text_schema()),
                        (
                            "alias",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "package",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("version", mp::text_schema()),
                        (
                            "target",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "artifact_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "artifact_hash",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "integrity",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "permissions",
                            mp::object_schema(&[
                                (
                                    "network",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "filesystem",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "env",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Applications(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Applications".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("application_type",match &(item).application_type {crate::routes::applications_group::applications::ApplicationTypeDto::AgentFlow => mp::object_value(&[("variant",serde_json::Value::String("AgentFlow".to_owned()))]), crate::routes::applications_group::applications::ApplicationTypeDto::Workflow => mp::object_value(&[("variant",serde_json::Value::String("Workflow".to_owned()))])}), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(item).description).len()))])), ("icon",match (&(item).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon_type",match (&(item).icon_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("icon_background",match (&(item).icon_background).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(item).created_by).len()))])), ("updated_at",mp::text(&(item).updated_at)?), ("tags",{ if (&(item).tags).len() > 32 { return None; } serde_json::Value::Array((&(item).tags).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?) })]), Self::Catalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Catalog".to_owned())), ("0",mp::object_value(&[("types",{ if (&(_field_0).types).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).types).iter().map(|item| Some(mp::object_value(&[("value",match &(item).value {crate::routes::applications_group::applications::ApplicationTypeDto::AgentFlow => mp::object_value(&[("variant",serde_json::Value::String("AgentFlow".to_owned()))]), crate::routes::applications_group::applications::ApplicationTypeDto::Workflow => mp::object_value(&[("variant",serde_json::Value::String("Workflow".to_owned()))])}), ("label",mp::object_value(&[("byte_count",serde_json::json!((&(item).label).len()))]))]))).collect::<Option<Vec<_>>>()?) }), ("workflow_triggers",{ if (&(_field_0).workflow_triggers).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).workflow_triggers).iter().map(|item| Some(mp::object_value(&[("value",match &(item).value {crate::routes::applications_group::applications::WorkflowTriggerTypeDto::Extension => mp::object_value(&[("variant",serde_json::Value::String("Extension".to_owned()))]), crate::routes::applications_group::applications::WorkflowTriggerTypeDto::Schedule => mp::object_value(&[("variant",serde_json::Value::String("Schedule".to_owned()))])}), ("label",mp::object_value(&[("byte_count",serde_json::json!((&(item).label).len()))]))]))).collect::<Option<Vec<_>>>()?) }), ("tags",{ if (&(_field_0).tags).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).tags).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("application_count",serde_json::json!(*(&(item).application_count)))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Application(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Application".to_owned())), ("0",mp::object_value(&[("id",mp::text(&(_field_0).id)?), ("application_type",match &(_field_0).application_type {crate::routes::applications_group::applications::ApplicationTypeDto::AgentFlow => mp::object_value(&[("variant",serde_json::Value::String("AgentFlow".to_owned()))]), crate::routes::applications_group::applications::ApplicationTypeDto::Workflow => mp::object_value(&[("variant",serde_json::Value::String("Workflow".to_owned()))])}), ("workflow_trigger_type",match (&(_field_0).workflow_trigger_type).as_ref() { Some(item) => match item {crate::routes::applications_group::applications::WorkflowTriggerTypeDto::Extension => mp::object_value(&[("variant",serde_json::Value::String("Extension".to_owned()))]), crate::routes::applications_group::applications::WorkflowTriggerTypeDto::Schedule => mp::object_value(&[("variant",serde_json::Value::String("Schedule".to_owned()))])}, None => serde_json::Value::Null }), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).description).len()))])), ("icon",match (&(_field_0).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon_type",match (&(_field_0).icon_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("icon_background",match (&(_field_0).icon_background).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("created_by",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).created_by).len()))])), ("updated_at",mp::text(&(_field_0).updated_at)?), ("tags",{ if (&(_field_0).tags).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).tags).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))]))]))).collect::<Option<Vec<_>>>()?) }), ("sections",mp::object_value(&[("orchestration",mp::object_value(&[("status",mp::text(&(&(&(_field_0).sections).orchestration).status)?), ("subject_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).orchestration).subject_kind).len()))])), ("subject_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).orchestration).subject_status).len()))])), ("current_subject_id",match (&(&(&(_field_0).sections).orchestration).current_subject_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("current_draft_id",match (&(&(&(_field_0).sections).orchestration).current_draft_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })])), ("api",mp::object_value(&[("status",match &(&(&(_field_0).sections).api).status {crate::routes::applications_group::applications::ApplicationApiSectionStatusResponse::Active => mp::object_value(&[("variant",serde_json::Value::String("Active".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiSectionStatusResponse::Planned => mp::object_value(&[("variant",serde_json::Value::String("Planned".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiSectionStatusResponse::Available => mp::object_value(&[("variant",serde_json::Value::String("Available".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiSectionStatusResponse::Unavailable => mp::object_value(&[("variant",serde_json::Value::String("Unavailable".to_owned()))])}), ("invoke_routing_mode",match &(&(&(_field_0).sections).api).invoke_routing_mode {crate::routes::applications_group::applications::ApplicationApiInvokeRoutingModeResponse::ApiKeyBoundApplication => mp::object_value(&[("variant",serde_json::Value::String("ApiKeyBoundApplication".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiInvokeRoutingModeResponse::PublishedWorkflowOperation => mp::object_value(&[("variant",serde_json::Value::String("PublishedWorkflowOperation".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiInvokeRoutingModeResponse::NotAvailable => mp::object_value(&[("variant",serde_json::Value::String("NotAvailable".to_owned()))])}), ("invoke_path_template",match (&(&(&(_field_0).sections).api).invoke_path_template).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("api_capability_status",match &(&(&(_field_0).sections).api).api_capability_status {crate::routes::applications_group::applications::ApplicationApiCapabilityStatusResponse::Enabled => mp::object_value(&[("variant",serde_json::Value::String("Enabled".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiCapabilityStatusResponse::Disabled => mp::object_value(&[("variant",serde_json::Value::String("Disabled".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiCapabilityStatusResponse::NotPublished => mp::object_value(&[("variant",serde_json::Value::String("NotPublished".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiCapabilityStatusResponse::Available => mp::object_value(&[("variant",serde_json::Value::String("Available".to_owned()))]), crate::routes::applications_group::applications::ApplicationApiCapabilityStatusResponse::Unavailable => mp::object_value(&[("variant",serde_json::Value::String("Unavailable".to_owned()))])})])), ("logs",mp::object_value(&[("status",mp::text(&(&(&(_field_0).sections).logs).status)?), ("runs_capability_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).logs).runs_capability_status).len()))])), ("run_object_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).logs).run_object_kind).len()))])), ("log_retention_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).logs).log_retention_status).len()))]))])), ("monitoring",mp::object_value(&[("status",mp::text(&(&(&(_field_0).sections).monitoring).status)?), ("metrics_capability_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).monitoring).metrics_capability_status).len()))])), ("metrics_object_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).monitoring).metrics_object_kind).len()))])), ("tracing_config_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).sections).monitoring).tracing_config_status).len()))]))]))]))]))]), Self::Tag(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Tag".to_owned())), ("0",mp::object_value(&[("id",mp::text(&(_field_0).id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))])), ("application_count",serde_json::json!(*(&(_field_0).application_count)))]))]), Self::EnvironmentVariables(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("EnvironmentVariables".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("value_type",mp::text(&(item).value_type)?), ("value",mp::json_summary(&(item).value)), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(item).description).len()))])), ("updated_at",mp::text(&(item).updated_at)?)]))).collect::<Option<Vec<_>>>()?) })]), Self::JsDependencies(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("JsDependencies".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("application_id",mp::text(&(item).application_id)?), ("installation_id",mp::text(&(item).installation_id)?), ("provider_code",mp::text(&(item).provider_code)?), ("plugin_id",mp::text(&(item).plugin_id)?), ("plugin_version",mp::text(&(item).plugin_version)?), ("alias",mp::object_value(&[("byte_count",serde_json::json!((&(item).alias).len()))])), ("package",mp::object_value(&[("byte_count",serde_json::json!((&(item).package).len()))])), ("version",mp::text(&(item).version)?), ("target",mp::object_value(&[("byte_count",serde_json::json!((&(item).target).len()))])), ("artifact_path",mp::object_value(&[("byte_count",serde_json::json!((&(item).artifact_path).len()))])), ("artifact_hash",mp::object_value(&[("byte_count",serde_json::json!((&(item).artifact_hash).len()))])), ("integrity",mp::object_value(&[("byte_count",serde_json::json!((&(item).integrity).len()))])), ("permissions",mp::object_value(&[("network",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).permissions).network).len()))])), ("filesystem",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).permissions).filesystem).len()))])), ("env",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).permissions).env).len()))]))]))]))).collect::<Option<Vec<_>>>()?) })]), Self::JsDependency(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("JsDependency".to_owned())), ("0",mp::object_value(&[("application_id",mp::text(&(_field_0).application_id)?), ("installation_id",mp::text(&(_field_0).installation_id)?), ("provider_code",mp::text(&(_field_0).provider_code)?), ("plugin_id",mp::text(&(_field_0).plugin_id)?), ("plugin_version",mp::text(&(_field_0).plugin_version)?), ("alias",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).alias).len()))])), ("package",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).package).len()))])), ("version",mp::text(&(_field_0).version)?), ("target",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).target).len()))])), ("artifact_path",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).artifact_path).len()))])), ("artifact_hash",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).artifact_hash).len()))])), ("integrity",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).integrity).len()))])), ("permissions",mp::object_value(&[("network",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).permissions).network).len()))])), ("filesystem",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).permissions).filesystem).len()))])), ("env",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).permissions).env).len()))]))]))]))]), Self::NoContent => mp::object_value(&[("variant",serde_json::Value::String("NoContent".to_owned()))])})
    }

    const CONTRACT_ID: &'static str = "console-applications-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ApplicationsAdapter {
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
}

pub(crate) fn applications_port(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
) -> Arc<dyn ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput>> {
    Arc::new(ApplicationsAdapter {
        store,
        bootstrap_workspace_id,
    })
}

impl ApplicationsAdapter {
    async fn catalog(
        &self,
        principal: &UserPrincipal,
        hints: ConsoleLocaleHints,
    ) -> Result<ApplicationCatalogResponse, ApiError> {
        let preferred_locale = self
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(ControlPlaneError::NotAuthenticated)?
            .preferred_locale;
        let locale = hints.resolve(preferred_locale);
        let tags = ApplicationService::new(self.store.for_actor(principal.actor().clone()))
            .list_application_tags(principal.actor().user_id)
            .await?;
        Ok(ApplicationCatalogResponse {
            types: application_type_catalog(&self.store, self.bootstrap_workspace_id, &locale)
                .await?,
            workflow_triggers: workflow_trigger_type_catalog(
                &self.store,
                self.bootstrap_workspace_id,
                &locale,
            )
            .await?,
            tags: tags
                .into_iter()
                .map(super::to_application_tag_catalog_entry)
                .collect(),
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ApplicationsInput,
    ) -> Result<ApplicationsOutput, ApiError> {
        let actor = principal.actor();
        let service = ApplicationService::new(self.store.for_actor(actor.clone()));
        match input {
            ApplicationsInput::List => Ok(ApplicationsOutput::Applications(
                service
                    .list_applications(actor.user_id)
                    .await?
                    .into_iter()
                    .map(super::to_application_summary)
                    .collect(),
            )),
            ApplicationsInput::Catalog { locale } => Ok(ApplicationsOutput::Catalog(
                self.catalog(principal, locale).await?,
            )),
            ApplicationsInput::Create(body) => {
                let application_type = body.application_type.into_domain();
                let workflow_trigger_type =
                    parse_workflow_trigger_type(application_type, body.workflow_trigger_type)?;
                let created = service
                    .create_application(CreateApplicationCommand {
                        actor_user_id: actor.user_id,
                        application_type,
                        workflow_trigger_type,
                        workflow_trigger_config: parse_create_workflow_trigger_config(
                            workflow_trigger_type,
                            body.workflow_trigger_config,
                        )?,
                        name: body.name,
                        description: body.description,
                        icon: body.icon,
                        icon_type: body.icon_type,
                        icon_background: body.icon_background,
                    })
                    .await?;
                Ok(ApplicationsOutput::Application(
                    super::to_application_detail(created),
                ))
            }
            ApplicationsInput::CreateTag(body) => {
                let created = service
                    .create_application_tag(CreateApplicationTagCommand {
                        actor_user_id: actor.user_id,
                        name: body.name,
                    })
                    .await?;
                Ok(ApplicationsOutput::Tag(
                    super::to_application_tag_catalog_entry(created),
                ))
            }
            ApplicationsInput::Get { application_id } => {
                let application = service
                    .get_application(actor.user_id, application_id)
                    .await?;
                Ok(ApplicationsOutput::Application(
                    super::to_application_detail(application),
                ))
            }
            ApplicationsInput::Patch {
                application_id,
                body,
            } => {
                let tag_ids = body
                    .tag_ids
                    .into_iter()
                    .map(|value| {
                        value
                            .parse::<Uuid>()
                            .map_err(|_| ControlPlaneError::InvalidInput("tag_ids"))
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                let updated = service
                    .update_application(UpdateApplicationCommand {
                        actor_user_id: actor.user_id,
                        application_id,
                        name: body.name,
                        description: body.description,
                        tag_ids,
                        icon: body.icon,
                        icon_type: body.icon_type,
                        icon_background: body.icon_background,
                    })
                    .await?;
                Ok(ApplicationsOutput::Application(
                    super::to_application_detail(updated),
                ))
            }
            ApplicationsInput::Delete { application_id } => {
                service
                    .delete_application(DeleteApplicationCommand {
                        actor_user_id: actor.user_id,
                        application_id,
                    })
                    .await?;
                Ok(ApplicationsOutput::NoContent)
            }
            ApplicationsInput::ListEnvironmentVariables { application_id } => {
                Ok(ApplicationsOutput::EnvironmentVariables(
                    service
                        .list_application_environment_variables(actor.user_id, application_id)
                        .await?
                        .into_iter()
                        .map(super::to_application_environment_variable)
                        .collect(),
                ))
            }
            ApplicationsInput::ReplaceEnvironmentVariables {
                application_id,
                body,
            } => {
                let variables = body
                    .variables
                    .into_iter()
                    .map(|variable| ApplicationEnvironmentVariableInput {
                        name: variable.name,
                        value_type: variable.value_type,
                        value: variable.value,
                        description: variable.description,
                    })
                    .collect();
                let replaced = service
                    .replace_application_environment_variables(
                        ReplaceApplicationEnvironmentVariablesCommand {
                            actor_user_id: actor.user_id,
                            application_id,
                            variables,
                        },
                    )
                    .await?;
                Ok(ApplicationsOutput::EnvironmentVariables(
                    replaced
                        .into_iter()
                        .map(super::to_application_environment_variable)
                        .collect(),
                ))
            }
            ApplicationsInput::ListJsDependencies { application_id } => {
                Ok(ApplicationsOutput::JsDependencies(
                    ApplicationJsDependencyService::new(self.store.clone())
                        .list_application_js_dependency_selections(actor.user_id, application_id)
                        .await?
                        .into_iter()
                        .map(super::to_application_js_dependency_selection)
                        .collect(),
                ))
            }
            ApplicationsInput::ReplaceJsDependency {
                application_id,
                body,
            } => {
                let installation_id = body
                    .installation_id
                    .parse::<Uuid>()
                    .map_err(|_| ControlPlaneError::InvalidInput("installation_id"))?;
                let selection = ApplicationJsDependencyService::new(self.store.clone())
                    .replace_application_js_dependency_selection(
                        ReplaceApplicationJsDependencySelectionCommand {
                            actor_user_id: actor.user_id,
                            application_id,
                            installation_id,
                            alias: body.alias,
                            target: body.target,
                        },
                    )
                    .await?;
                Ok(ApplicationsOutput::JsDependency(
                    super::to_application_js_dependency_selection(selection),
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput> for ApplicationsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ApplicationsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "applications.list",
        binding_id: "http.console.applications.list.v1",
        method: "GET",
        path: "/api/console/applications",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.create",
        binding_id: "http.console.applications.create.v1",
        method: "POST",
        path: "/api/console/applications",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.get",
        binding_id: "http.console.applications.get.v1",
        method: "GET",
        path: "/api/console/applications/:id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.update",
        binding_id: "http.console.applications.update.v1",
        method: "PATCH",
        path: "/api/console/applications/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.delete",
        binding_id: "http.console.applications.delete.v1",
        method: "DELETE",
        path: "/api/console/applications/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.catalog.get",
        binding_id: "http.console.applications.catalog.get.v1",
        method: "GET",
        path: "/api/console/applications/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.tags.create",
        binding_id: "http.console.applications.tags.create.v1",
        method: "POST",
        path: "/api/console/applications/tags",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.environment-variables.list",
        binding_id: "http.console.applications.environment-variables.list.v1",
        method: "GET",
        path: "/api/console/applications/:id/environment-variables",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.environment-variables.replace",
        binding_id: "http.console.applications.environment-variables.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:id/environment-variables",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.js-dependencies.list",
        binding_id: "http.console.applications.js-dependencies.list.v1",
        method: "GET",
        path: "/api/console/applications/:id/js-dependencies",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "applications.js-dependencies.replace",
        binding_id: "http.console.applications.js-dependencies.replace.v1",
        method: "PUT",
        path: "/api/console/applications/:id/js-dependencies",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-applications",
        "graph:console-applications-v1",
        DECLARATIONS,
        port,
    )
}

async fn application_type_catalog(
    store: &MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
) -> Result<Vec<ApplicationTypeOptionResponse>, ApiError> {
    let agent_flow =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Agent Flow").await?;
    let workflow =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Workflow").await?;
    Ok(vec![
        ApplicationTypeOptionResponse {
            value: ApplicationTypeDto::AgentFlow,
            label: agent_flow,
        },
        ApplicationTypeOptionResponse {
            value: ApplicationTypeDto::Workflow,
            label: workflow,
        },
    ])
}

async fn workflow_trigger_type_catalog(
    store: &MainDurableStore,
    bootstrap_workspace_id: Uuid,
    locale: &domain::CatalogLocale,
) -> Result<Vec<WorkflowTriggerTypeOptionResponse>, ApiError> {
    let extension =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Extension").await?;
    let schedule =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Schedule").await?;
    Ok(vec![
        WorkflowTriggerTypeOptionResponse {
            value: WorkflowTriggerTypeDto::Extension,
            label: extension,
        },
        WorkflowTriggerTypeOptionResponse {
            value: WorkflowTriggerTypeDto::Schedule,
            label: schedule,
        },
    ])
}

fn parse_create_workflow_trigger_config(
    trigger_type: Option<domain::WorkflowTriggerType>,
    config: Option<CreateWorkflowTriggerConfigBody>,
) -> Result<Option<CreateWorkflowTriggerConfig>, ApiError> {
    match (trigger_type, config) {
        (None, None) => Ok(None),
        (None, Some(_)) => Err(ControlPlaneError::InvalidInput("workflow_trigger_config").into()),
        (Some(domain::WorkflowTriggerType::Schedule), Some(config)) => {
            let cron = config
                .cron
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("cron"))?;
            let timezone = config
                .timezone
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("timezone"))?;
            Ok(Some(CreateWorkflowTriggerConfig::Schedule {
                cron,
                timezone,
                input_payload: config
                    .input_payload
                    .unwrap_or_else(|| serde_json::json!({})),
            }))
        }
        (Some(domain::WorkflowTriggerType::Extension), Some(config)) => {
            let subpath = config
                .subpath
                .filter(|value| !value.trim().is_empty())
                .ok_or(ControlPlaneError::InvalidInput("subpath"))?;
            let http_method = match config
                .http_method
                .unwrap_or(WorkflowExtensionHttpMethodBody::Post)
            {
                WorkflowExtensionHttpMethodBody::Get => "GET",
                WorkflowExtensionHttpMethodBody::Post => "POST",
                WorkflowExtensionHttpMethodBody::Put => "PUT",
                WorkflowExtensionHttpMethodBody::Patch => "PATCH",
                WorkflowExtensionHttpMethodBody::Delete => "DELETE",
                WorkflowExtensionHttpMethodBody::Head => "HEAD",
                WorkflowExtensionHttpMethodBody::Options => "OPTIONS",
            }
            .to_string();
            let response_mode = match config
                .response_mode
                .unwrap_or(WorkflowExtensionResponseModeBody::Sync)
            {
                WorkflowExtensionResponseModeBody::Sync => "sync",
                WorkflowExtensionResponseModeBody::Async => "async",
            }
            .to_string();
            Ok(Some(CreateWorkflowTriggerConfig::Extension {
                subpath,
                http_method,
                response_mode,
            }))
        }
        (Some(_), None) => Ok(None),
    }
}

fn parse_workflow_trigger_type(
    application_type: domain::ApplicationType,
    value: Option<WorkflowTriggerTypeDto>,
) -> Result<Option<domain::WorkflowTriggerType>, ApiError> {
    match application_type {
        domain::ApplicationType::AgentFlow if value.is_none() => Ok(None),
        domain::ApplicationType::AgentFlow => {
            Err(ControlPlaneError::InvalidInput("workflow_trigger_type").into())
        }
        domain::ApplicationType::Workflow => Ok(Some(
            value
                .unwrap_or(WorkflowTriggerTypeDto::Extension)
                .into_domain(),
        )),
    }
}

#[cfg(test)]
struct UnavailableApplicationsPort;

#[cfg(test)]
impl ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput> for UnavailableApplicationsPort {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: ApplicationsInput,
    ) -> ConsoleInterfaceFuture<'a, ApplicationsOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("applications fixture unavailable").into(),
            ))
        })
    }
}

#[cfg(test)]
mod tests {
    use interface_runtime::BindingId;

    use super::*;

    #[test]
    fn f09a_registry_freezes_all_console_application_bindings() {
        let registry = compile_registry(Arc::new(UnavailableApplicationsPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(
                registry
                    .binding(&BindingId::new(declaration.binding_id).unwrap())
                    .is_some()
            );
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }
}
