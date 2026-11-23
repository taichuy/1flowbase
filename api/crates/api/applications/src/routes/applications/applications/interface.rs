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
    ApplicationCatalogResponse, ApplicationCollectorResponse, ApplicationDetailResponse,
    ApplicationEnvironmentVariableResponse, ApplicationJsDependencySelectionResponse,
    ApplicationSummaryResponse, ApplicationTagCatalogResponse, ApplicationTypeDto,
    ApplicationTypeOptionResponse, CreateApplicationBody, CreateApplicationTagBody,
    CreateWorkflowTriggerConfigBody, PatchApplicationBody,
    ReplaceApplicationEnvironmentVariablesBody, ReplaceApplicationJsDependencySelectionBody,
    WorkflowTriggerTypeDto, WorkflowTriggerTypeOptionResponse,
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

pub enum ApplicationsInput {
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

mod mp_helpers {
    use serde_json::{json, Value};
    use uuid::Uuid;

    use super::{
        collector_projection, collector_projection_schema, ApplicationCatalogResponse,
        ApplicationDetailResponse, ApplicationJsDependencySelectionResponse,
        ApplicationSummaryResponse, ApplicationTypeDto, ApplicationsInput, ApplicationsOutput,
        CreateApplicationBody, WorkflowExtensionHttpMethodBody, WorkflowExtensionResponseModeBody,
        WorkflowTriggerTypeDto,
    };
    use crate::extension_bus::managed_projection as mp;
    use crate::routes::applications_group::applications::{
        ApplicationApiCapabilityStatusResponse as ApiCap,
        ApplicationApiInvokeRoutingModeResponse as RouteMode,
        ApplicationApiSectionStatusResponse as ApiStatus, ApplicationTagResponse,
    };

    const MAX_ITEMS: usize = 32;
    const APP_TYPES: &[&str] = &["AgentFlow", "AgentLogs", "Workflow"];
    const TRIGGER_TYPES: &[&str] = &["Extension", "Schedule", "Event"];
    const HTTP_METHODS: &[&str] = &["Get", "Post", "Put", "Patch", "Delete", "Head", "Options"];

    macro_rules! obj_s {
        ($($k:literal : $v:expr),* $(,)?) => { mp::object_schema([$(($k, $v)),*]) };
    }
    macro_rules! obj_v {
        ($($k:literal : $v:expr),* $(,)?) => { mp::object_value([$(($k, $v)),*]) };
    }

    fn tag_s(t: &str) -> Value {
        obj_s! { "variant": mp::tag_schema(t) }
    }
    fn tag_v(t: &str) -> Value {
        obj_v! { "variant": Value::String(t.to_owned()) }
    }
    fn enum_s(tags: &[&str]) -> Value {
        mp::union_schema(tags.iter().copied().map(tag_s).collect())
    }
    fn bytes_s() -> Value {
        obj_s! { "byte_count": mp::count_schema() }
    }
    fn bytes_v(v: &str) -> Value {
        obj_v! { "byte_count": json!(v.len()) }
    }
    fn opt_s(s: Value) -> Value {
        json!({"anyOf": [s, {"type": "null"}]})
    }
    fn opt_bytes_v(v: Option<&str>) -> Value {
        v.map_or(Value::Null, bytes_v)
    }
    fn opt_text_v(v: Option<&str>) -> Option<Value> {
        v.map_or(Some(Value::Null), mp::text)
    }
    fn arr_s(items: Value) -> Value {
        json!({"type": "array", "maxItems": MAX_ITEMS, "items": items})
    }
    fn arr_v<T>(items: &[T], f: impl Fn(&T) -> Option<Value>) -> Option<Value> {
        const MAX_ARRAY_JSON_BYTES: usize = 60 * 1024;
        let mut out = Vec::with_capacity(items.len().min(MAX_ITEMS));
        let mut approx_bytes = 2usize;
        for item in items.iter().take(MAX_ITEMS) {
            let projected = f(item)?;
            let item_bytes = serde_json::to_vec(&projected).ok()?.len() + 1;
            if !out.is_empty() && approx_bytes + item_bytes > MAX_ARRAY_JSON_BYTES {
                break;
            }
            approx_bytes += item_bytes;
            out.push(projected);
        }
        Some(Value::Array(out))
    }
    fn var_s(t: &str, v: Value) -> Value {
        obj_s! { "variant": mp::tag_schema(t), "0": v }
    }
    fn var_v(t: &str, v: Value) -> Value {
        obj_v! { "variant": Value::String(t.to_owned()), "0": v }
    }
    fn id_s(t: &str) -> Value {
        obj_s! { "variant": mp::tag_schema(t), "application_id": mp::text_schema() }
    }
    fn id_v(t: &str, id: &Uuid) -> Value {
        obj_v! { "variant": Value::String(t.to_owned()), "application_id": Value::String(id.to_string()) }
    }
    fn body_s(t: &str, b: Value) -> Value {
        obj_s! { "variant": mp::tag_schema(t), "application_id": mp::text_schema(), "body": b }
    }
    fn body_v(t: &str, id: &Uuid, b: Value) -> Value {
        obj_v! { "variant": Value::String(t.to_owned()), "application_id": Value::String(id.to_string()), "body": b }
    }

    fn app_type_v(v: &ApplicationTypeDto) -> Value {
        tag_v(match v {
            ApplicationTypeDto::AgentLogs => "AgentLogs",
            ApplicationTypeDto::AgentFlow => "AgentFlow",
            ApplicationTypeDto::Workflow => "Workflow",
        })
    }
    fn trigger_v(v: &WorkflowTriggerTypeDto) -> Value {
        tag_v(match v {
            WorkflowTriggerTypeDto::Extension => "Extension",
            WorkflowTriggerTypeDto::Schedule => "Schedule",
            WorkflowTriggerTypeDto::Event => "Event",
        })
    }
    fn tags_v(tags: &[ApplicationTagResponse]) -> Option<Value> {
        arr_v(tags, |t| {
            Some(obj_v! { "id": mp::text(&t.id)?, "name": bytes_v(&t.name) })
        })
    }
    fn js_dep_s() -> Value {
        obj_s! {
            "application_id": mp::text_schema(), "installation_id": mp::text_schema(),
            "provider_code": mp::text_schema(), "plugin_id": mp::text_schema(),
            "plugin_version": mp::text_schema(), "alias": bytes_s(), "package": bytes_s(),
            "version": mp::text_schema(), "target": bytes_s(), "artifact_path": bytes_s(),
            "artifact_hash": bytes_s(), "integrity": bytes_s(),
            "permissions": obj_s! { "network": bytes_s(), "filesystem": bytes_s(), "env": bytes_s() },
        }
    }
    fn js_dep_v(i: &ApplicationJsDependencySelectionResponse) -> Option<Value> {
        let p = &i.permissions;
        Some(obj_v! {
            "application_id": mp::text(&i.application_id)?, "installation_id": mp::text(&i.installation_id)?,
            "provider_code": mp::text(&i.provider_code)?, "plugin_id": mp::text(&i.plugin_id)?,
            "plugin_version": mp::text(&i.plugin_version)?, "alias": bytes_v(&i.alias), "package": bytes_v(&i.package),
            "version": mp::text(&i.version)?, "target": bytes_v(&i.target), "artifact_path": bytes_v(&i.artifact_path),
            "artifact_hash": bytes_v(&i.artifact_hash), "integrity": bytes_v(&i.integrity),
            "permissions": obj_v! { "network": bytes_v(&p.network), "filesystem": bytes_v(&p.filesystem), "env": bytes_v(&p.env) },
        })
    }

    pub(super) fn input_schema() -> Value {
        let trig_cfg = obj_s! {
            "cron": opt_s(bytes_s()), "timezone": opt_s(bytes_s()),
            "input_payload": opt_s(mp::json_summary_schema()), "subpath": opt_s(bytes_s()),
            "http_method": opt_s(enum_s(HTTP_METHODS)), "response_mode": opt_s(enum_s(&["Sync", "Async"])),
        };
        let create = obj_s! {
            "application_type": enum_s(APP_TYPES), "workflow_trigger_type": opt_s(enum_s(TRIGGER_TYPES)),
            "workflow_trigger_config": opt_s(trig_cfg), "name": bytes_s(), "description": bytes_s(),
            "icon": opt_s(bytes_s()), "icon_type": opt_s(mp::text_schema()), "icon_background": opt_s(bytes_s()),
        };
        let patch = obj_s! {
            "name": bytes_s(), "description": bytes_s(), "tag_ids": arr_s(mp::text_schema()),
            "icon": opt_s(bytes_s()), "icon_type": opt_s(mp::text_schema()), "icon_background": opt_s(bytes_s()),
        };
        let env_var = obj_s! { "name": bytes_s(), "value_type": mp::text_schema(), "value": mp::json_summary_schema(), "description": bytes_s() };
        let js_body = obj_s! { "installation_id": mp::text_schema(), "alias": bytes_s(), "target": bytes_s() };
        mp::union_schema(vec![
            tag_s("List"),
            tag_s("Catalog"),
            var_s("Create", create),
            var_s("CreateTag", obj_s! { "name": bytes_s() }),
            id_s("Get"),
            body_s("Patch", patch),
            id_s("Delete"),
            id_s("ListEnvironmentVariables"),
            body_s(
                "ReplaceEnvironmentVariables",
                obj_s! { "variables": arr_s(env_var) },
            ),
            id_s("ListJsDependencies"),
            body_s("ReplaceJsDependency", js_body),
        ])
    }

    fn project_create(b: &CreateApplicationBody) -> Option<Value> {
        let trig_cfg = match b.workflow_trigger_config.as_ref() {
            Some(cfg) => {
                let method = cfg.http_method.as_ref().map_or(Value::Null, |m| {
                    tag_v(match m {
                        WorkflowExtensionHttpMethodBody::Get => "Get",
                        WorkflowExtensionHttpMethodBody::Post => "Post",
                        WorkflowExtensionHttpMethodBody::Put => "Put",
                        WorkflowExtensionHttpMethodBody::Patch => "Patch",
                        WorkflowExtensionHttpMethodBody::Delete => "Delete",
                        WorkflowExtensionHttpMethodBody::Head => "Head",
                        WorkflowExtensionHttpMethodBody::Options => "Options",
                    })
                });
                let mode = cfg.response_mode.as_ref().map_or(Value::Null, |m| {
                    tag_v(match m {
                        WorkflowExtensionResponseModeBody::Sync => "Sync",
                        WorkflowExtensionResponseModeBody::Async => "Async",
                    })
                });
                obj_v! {
                    "cron": opt_bytes_v(cfg.cron.as_deref()), "timezone": opt_bytes_v(cfg.timezone.as_deref()),
                    "input_payload": cfg.input_payload.as_ref().map_or(Value::Null, mp::json_summary),
                    "subpath": opt_bytes_v(cfg.subpath.as_deref()), "http_method": method, "response_mode": mode,
                }
            }
            None => Value::Null,
        };
        Some(var_v(
            "Create",
            obj_v! {
                "application_type": app_type_v(&b.application_type),
                "workflow_trigger_type": b.workflow_trigger_type.as_ref().map_or(Value::Null, trigger_v),
                "workflow_trigger_config": trig_cfg,
                "name": bytes_v(&b.name), "description": bytes_v(&b.description),
                "icon": opt_bytes_v(b.icon.as_deref()), "icon_type": opt_text_v(b.icon_type.as_deref())?,
                "icon_background": opt_bytes_v(b.icon_background.as_deref()),
            },
        ))
    }

    pub(super) fn project_input(input: &ApplicationsInput) -> Option<Value> {
        Some(match input {
            ApplicationsInput::List => tag_v("List"),
            ApplicationsInput::Catalog { .. } => tag_v("Catalog"),
            ApplicationsInput::Create(b) => project_create(b)?,
            ApplicationsInput::CreateTag(b) => {
                var_v("CreateTag", obj_v! { "name": bytes_v(&b.name) })
            }
            ApplicationsInput::Get { application_id } => id_v("Get", application_id),
            ApplicationsInput::Patch {
                application_id,
                body: b,
            } => body_v(
                "Patch",
                application_id,
                obj_v! {
                    "name": bytes_v(&b.name), "description": bytes_v(&b.description),
                    "tag_ids": arr_v(&b.tag_ids, |s| mp::text(s))?,
                    "icon": opt_bytes_v(b.icon.as_deref()), "icon_type": opt_text_v(b.icon_type.as_deref())?,
                    "icon_background": opt_bytes_v(b.icon_background.as_deref()),
                },
            ),
            ApplicationsInput::Delete { application_id } => id_v("Delete", application_id),
            ApplicationsInput::ListEnvironmentVariables { application_id } => {
                id_v("ListEnvironmentVariables", application_id)
            }
            ApplicationsInput::ReplaceEnvironmentVariables {
                application_id,
                body,
            } => {
                let vars = arr_v(&body.variables, |v| {
                    Some(obj_v! {
                        "name": bytes_v(&v.name), "value_type": mp::text(&v.value_type)?,
                        "value": mp::json_summary(&v.value), "description": bytes_v(&v.description),
                    })
                })?;
                body_v(
                    "ReplaceEnvironmentVariables",
                    application_id,
                    obj_v! { "variables": vars },
                )
            }
            ApplicationsInput::ListJsDependencies { application_id } => {
                id_v("ListJsDependencies", application_id)
            }
            ApplicationsInput::ReplaceJsDependency {
                application_id,
                body: b,
            } => body_v(
                "ReplaceJsDependency",
                application_id,
                obj_v! {
                    "installation_id": mp::text(&b.installation_id)?, "alias": bytes_v(&b.alias), "target": bytes_v(&b.target),
                },
            ),
        })
    }

    pub(super) fn output_schema() -> Value {
        let tag_item = obj_s! { "id": mp::text_schema(), "name": bytes_s() };
        let summary = obj_s! {
            "id": mp::text_schema(), "application_type": enum_s(APP_TYPES),
            "name": bytes_s(), "description": bytes_s(), "icon": opt_s(bytes_s()),
            "icon_type": opt_s(mp::text_schema()), "icon_background": opt_s(bytes_s()),
            "created_by": bytes_s(), "updated_at": mp::text_schema(), "tags": arr_s(tag_item.clone()),
        };
        let sections = obj_s! {
            "orchestration": obj_s! {
                "status": mp::text_schema(), "subject_kind": bytes_s(), "subject_status": bytes_s(),
                "current_subject_id": opt_s(mp::text_schema()), "current_draft_id": opt_s(mp::text_schema()),
            },
            "api": obj_s! {
                "status": enum_s(&["Active", "Planned", "Available", "Unavailable"]),
                "invoke_routing_mode": enum_s(&["ApiKeyBoundApplication", "PublishedWorkflowOperation", "NotAvailable"]),
                "invoke_path_template": opt_s(bytes_s()),
                "api_capability_status": enum_s(&["Enabled", "Disabled", "NotPublished", "Available", "Unavailable"]),
            },
            "logs": obj_s! {
                "status": mp::text_schema(), "runs_capability_status": bytes_s(),
                "run_object_kind": bytes_s(), "log_retention_status": bytes_s(),
            },
            "monitoring": obj_s! {
                "status": mp::text_schema(), "metrics_capability_status": bytes_s(),
                "metrics_object_kind": bytes_s(), "tracing_config_status": bytes_s(),
            },
        };
        let detail = obj_s! {
            "id": mp::text_schema(), "application_type": enum_s(APP_TYPES),
            "workflow_trigger_type": opt_s(enum_s(TRIGGER_TYPES)),
            "name": bytes_s(), "description": bytes_s(), "icon": opt_s(bytes_s()),
            "icon_type": opt_s(mp::text_schema()), "icon_background": opt_s(bytes_s()),
            "created_by": bytes_s(), "updated_at": mp::text_schema(), "tags": arr_s(tag_item), "sections": sections,
        };
        let tag_cat = obj_s! { "id": mp::text_schema(), "name": bytes_s(), "application_count": json!({"type": "integer"}) };
        let catalog = obj_s! {
            "collectors": collector_projection_schema(),
            "types": arr_s(obj_s! { "value": enum_s(APP_TYPES), "label": bytes_s() }),
            "workflow_triggers": arr_s(obj_s! { "value": enum_s(TRIGGER_TYPES), "label": bytes_s() }),
            "tags": arr_s(tag_cat.clone()),
        };
        let env_var = obj_s! {
            "name": bytes_s(), "value_type": mp::text_schema(), "value": mp::json_summary_schema(),
            "description": bytes_s(), "updated_at": mp::text_schema(),
        };
        mp::union_schema(vec![
            var_s("Applications", arr_s(summary)),
            var_s("Catalog", catalog),
            var_s("Application", detail),
            var_s("Tag", tag_cat),
            var_s("EnvironmentVariables", arr_s(env_var)),
            var_s("JsDependencies", arr_s(js_dep_s())),
            var_s("JsDependency", js_dep_s()),
            tag_s("NoContent"),
        ])
    }

    fn project_summary(i: &ApplicationSummaryResponse) -> Option<Value> {
        Some(obj_v! {
            "id": mp::text(&i.id)?, "application_type": app_type_v(&i.application_type),
            "name": bytes_v(&i.name), "description": bytes_v(&i.description),
            "icon": opt_bytes_v(i.icon.as_deref()), "icon_type": opt_text_v(i.icon_type.as_deref())?,
            "icon_background": opt_bytes_v(i.icon_background.as_deref()),
            "created_by": bytes_v(&i.created_by), "updated_at": mp::text(&i.updated_at)?, "tags": tags_v(&i.tags)?,
        })
    }

    fn project_catalog(cat: &ApplicationCatalogResponse) -> Option<Value> {
        Some(obj_v! {
            "collectors": collector_projection(&cat.collectors)?,
            "types": arr_v(&cat.types, |t| Some(obj_v! { "value": app_type_v(&t.value), "label": bytes_v(&t.label) }))?,
            "workflow_triggers": arr_v(&cat.workflow_triggers, |t| Some(obj_v! { "value": trigger_v(&t.value), "label": bytes_v(&t.label) }))?,
            "tags": arr_v(&cat.tags, |t| Some(obj_v! { "id": mp::text(&t.id)?, "name": bytes_v(&t.name), "application_count": json!(t.application_count) }))?,
        })
    }

    fn project_detail(d: &ApplicationDetailResponse) -> Option<Value> {
        let (orch, api, logs, mon) = (
            &d.sections.orchestration,
            &d.sections.api,
            &d.sections.logs,
            &d.sections.monitoring,
        );
        let api_status = tag_v(match &api.status {
            ApiStatus::Active => "Active",
            ApiStatus::Planned => "Planned",
            ApiStatus::Available => "Available",
            ApiStatus::Unavailable => "Unavailable",
        });
        let routing = tag_v(match &api.invoke_routing_mode {
            RouteMode::ApiKeyBoundApplication => "ApiKeyBoundApplication",
            RouteMode::PublishedWorkflowOperation => "PublishedWorkflowOperation",
            RouteMode::NotAvailable => "NotAvailable",
        });
        let cap = tag_v(match &api.api_capability_status {
            ApiCap::Enabled => "Enabled",
            ApiCap::Disabled => "Disabled",
            ApiCap::NotPublished => "NotPublished",
            ApiCap::Available => "Available",
            ApiCap::Unavailable => "Unavailable",
        });
        let sections = obj_v! {
            "orchestration": obj_v! {
                "status": mp::text(&orch.status)?, "subject_kind": bytes_v(&orch.subject_kind),
                "subject_status": bytes_v(&orch.subject_status),
                "current_subject_id": opt_text_v(orch.current_subject_id.as_deref())?,
                "current_draft_id": opt_text_v(orch.current_draft_id.as_deref())?,
            },
            "api": obj_v! {
                "status": api_status, "invoke_routing_mode": routing,
                "invoke_path_template": opt_bytes_v(api.invoke_path_template.as_deref()), "api_capability_status": cap,
            },
            "logs": obj_v! {
                "status": mp::text(&logs.status)?, "runs_capability_status": bytes_v(&logs.runs_capability_status),
                "run_object_kind": bytes_v(&logs.run_object_kind), "log_retention_status": bytes_v(&logs.log_retention_status),
            },
            "monitoring": obj_v! {
                "status": mp::text(&mon.status)?, "metrics_capability_status": bytes_v(&mon.metrics_capability_status),
                "metrics_object_kind": bytes_v(&mon.metrics_object_kind), "tracing_config_status": bytes_v(&mon.tracing_config_status),
            },
        };
        Some(obj_v! {
            "id": mp::text(&d.id)?, "application_type": app_type_v(&d.application_type),
            "workflow_trigger_type": d.workflow_trigger_type.as_ref().map_or(Value::Null, trigger_v),
            "name": bytes_v(&d.name), "description": bytes_v(&d.description),
            "icon": opt_bytes_v(d.icon.as_deref()), "icon_type": opt_text_v(d.icon_type.as_deref())?,
            "icon_background": opt_bytes_v(d.icon_background.as_deref()),
            "created_by": bytes_v(&d.created_by), "updated_at": mp::text(&d.updated_at)?,
            "tags": tags_v(&d.tags)?, "sections": sections,
        })
    }

    pub(super) fn project_output(output: &ApplicationsOutput) -> Option<Value> {
        Some(match output {
            ApplicationsOutput::Applications(items) => {
                var_v("Applications", arr_v(items, project_summary)?)
            }
            ApplicationsOutput::Catalog(cat) => var_v("Catalog", project_catalog(cat)?),
            ApplicationsOutput::Application(d) => var_v("Application", project_detail(d)?),
            ApplicationsOutput::Tag(t) => var_v(
                "Tag",
                obj_v! { "id": mp::text(&t.id)?, "name": bytes_v(&t.name), "application_count": json!(t.application_count) },
            ),
            ApplicationsOutput::EnvironmentVariables(items) => var_v(
                "EnvironmentVariables",
                arr_v(items, |v| {
                    Some(obj_v! {
                        "name": bytes_v(&v.name), "value_type": mp::text(&v.value_type)?,
                        "value": mp::json_summary(&v.value), "description": bytes_v(&v.description), "updated_at": mp::text(&v.updated_at)?,
                    })
                })?,
            ),
            ApplicationsOutput::JsDependencies(items) => {
                var_v("JsDependencies", arr_v(items, js_dep_v)?)
            }
            ApplicationsOutput::JsDependency(item) => var_v("JsDependency", js_dep_v(item)?),
            ApplicationsOutput::NoContent => tag_v("NoContent"),
        })
    }
}

impl InterfaceContract for ApplicationsInput {
    fn event_projection_schema() -> Option<serde_json::Value> {
        Some(event_projection::input_schema())
    }
    fn project_for_event(&self) -> Option<serde_json::Value> {
        event_projection::input(self)
    }
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(mp_helpers::input_schema())
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        mp_helpers::project_input(self)
    }

    const CONTRACT_ID: &'static str = "console-applications-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed application output is projected immediately into the console response"
)]
pub enum ApplicationsOutput {
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
    fn event_projection_schema() -> Option<serde_json::Value> {
        Some(event_projection::output_schema())
    }
    fn project_for_event(&self) -> Option<serde_json::Value> {
        event_projection::output(self)
    }
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Some(mp_helpers::output_schema())
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        mp_helpers::project_output(self)
    }

    const CONTRACT_ID: &'static str = "console-applications-output";
    const CONTRACT_VERSION: &'static str = "1";
}

fn affects_event_subscriptions(input: &ApplicationsInput) -> bool {
    match input {
        ApplicationsInput::Create(_) | ApplicationsInput::Delete { .. } => true,
        // PatchApplicationBody edits metadata only; trigger type has no mutation field here.
        ApplicationsInput::List
        | ApplicationsInput::Catalog { .. }
        | ApplicationsInput::CreateTag(_)
        | ApplicationsInput::Get { .. }
        | ApplicationsInput::Patch { .. }
        | ApplicationsInput::ListEnvironmentVariables { .. }
        | ApplicationsInput::ReplaceEnvironmentVariables { .. }
        | ApplicationsInput::ListJsDependencies { .. }
        | ApplicationsInput::ReplaceJsDependency { .. } => false,
    }
}

struct ApplicationsAdapter {
    collector: super::collectors::CollectorCatalogDependencies,
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
}

pub fn applications_port(
    store: MainDurableStore,
    bootstrap_workspace_id: Uuid,
    collector: super::collectors::CollectorCatalogDependencies,
    changes: Arc<tokio::sync::Notify>,
) -> Arc<dyn ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput>> {
    crate::workers::event_workflow::refresh::after_success(
        Arc::new(ApplicationsAdapter {
            collector,
            store,
            bootstrap_workspace_id,
        }),
        changes,
        affects_event_subscriptions,
    )
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
            collectors: super::collectors::catalog(
                &self.store,
                &self.collector,
                principal.actor(),
                &locale,
            )
            .await?,
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
                    .collect::<Result<Vec<_>, _>>()?,
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
                    super::to_application_detail(created)?,
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
                    super::to_application_detail(application)?,
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
                    super::to_application_detail(updated)?,
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
                        .collect::<Result<Vec<_>, _>>()?,
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
                        .collect::<Result<Vec<_>, _>>()?,
                ))
            }
            ApplicationsInput::ListJsDependencies { application_id } => {
                Ok(ApplicationsOutput::JsDependencies(
                    ApplicationJsDependencyService::new(self.store.for_actor(actor.clone()))
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
                let selection =
                    ApplicationJsDependencyService::new(self.store.for_actor(actor.clone()))
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

const fn decl(
    interface_id: &'static str,
    binding_id: &'static str,
    method: &'static str,
    path: &'static str,
    mutating: bool,
) -> ConsoleInterfaceDeclaration<'static> {
    ConsoleInterfaceDeclaration {
        interface_id,
        binding_id,
        method,
        path,
        mutating,
    }
}

pub(crate) const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    decl(
        "applications.list",
        "http.console.applications.list.v1",
        "GET",
        "/api/console/applications",
        false,
    ),
    decl(
        "applications.create",
        "http.console.applications.create.v1",
        "POST",
        "/api/console/applications",
        true,
    ),
    decl(
        "applications.get",
        "http.console.applications.get.v1",
        "GET",
        "/api/console/applications/:id",
        false,
    ),
    decl(
        "applications.update",
        "http.console.applications.update.v1",
        "PATCH",
        "/api/console/applications/:id",
        true,
    ),
    decl(
        "applications.delete",
        "http.console.applications.delete.v1",
        "DELETE",
        "/api/console/applications/:id",
        true,
    ),
    decl(
        "applications.catalog.get",
        "http.console.applications.catalog.get.v1",
        "GET",
        "/api/console/applications/catalog",
        false,
    ),
    decl(
        "applications.tags.create",
        "http.console.applications.tags.create.v1",
        "POST",
        "/api/console/applications/tags",
        true,
    ),
    decl(
        "applications.environment-variables.list",
        "http.console.applications.environment-variables.list.v1",
        "GET",
        "/api/console/applications/:id/environment-variables",
        false,
    ),
    decl(
        "applications.environment-variables.replace",
        "http.console.applications.environment-variables.replace.v1",
        "PUT",
        "/api/console/applications/:id/environment-variables",
        true,
    ),
    decl(
        "applications.js-dependencies.list",
        "http.console.applications.js-dependencies.list.v1",
        "GET",
        "/api/console/applications/:id/js-dependencies",
        false,
    ),
    decl(
        "applications.js-dependencies.replace",
        "http.console.applications.js-dependencies.replace.v1",
        "PUT",
        "/api/console/applications/:id/js-dependencies",
        true,
    ),
];

pub fn compile_registry(
    port: Arc<dyn ConsoleInterfacePort<ApplicationsInput, ApplicationsOutput>>,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry_with_event_observation(
        "api-server.console-applications",
        "graph:console-applications-v1",
        DECLARATIONS,
        port,
        event_projection::OPERATIONS,
    )
}

// The hook inventory uses metadata; the HTTP DTO retains the complete collector catalog.
fn collector_projection_schema() -> serde_json::Value {
    use crate::extension_bus::managed_projection as mp;
    mp::object_schema([("item_count", mp::count_schema())])
}

fn collector_projection(collectors: &[ApplicationCollectorResponse]) -> Option<serde_json::Value> {
    use crate::extension_bus::managed_projection as mp;
    Some(mp::object_value([(
        "item_count",
        serde_json::json!(collectors.len()),
    )]))
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
    let agent_logs =
        resolve_request_text_with(store, bootstrap_workspace_id, locale, "Agent Logs").await?;
    Ok(vec![
        ApplicationTypeOptionResponse {
            value: ApplicationTypeDto::AgentFlow,
            label: agent_flow,
        },
        ApplicationTypeOptionResponse {
            value: ApplicationTypeDto::Workflow,
            label: workflow,
        },
        ApplicationTypeOptionResponse {
            value: ApplicationTypeDto::AgentLogs,
            label: agent_logs,
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
    let event = resolve_request_text_with(store, bootstrap_workspace_id, locale, "Event").await?;
    Ok(vec![
        WorkflowTriggerTypeOptionResponse {
            value: WorkflowTriggerTypeDto::Event,
            label: event,
        },
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
            let timezone = config.timezone.unwrap_or_default().trim().to_owned();
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
        (Some(domain::WorkflowTriggerType::Event), Some(_)) => {
            Err(ControlPlaneError::InvalidInput("workflow_trigger_config").into())
        }
        (Some(_), None) => Ok(None),
    }
}

fn parse_workflow_trigger_type(
    application_type: domain::ApplicationType,
    value: Option<WorkflowTriggerTypeDto>,
) -> Result<Option<domain::WorkflowTriggerType>, ApiError> {
    match application_type {
        domain::ApplicationType::AgentFlow | domain::ApplicationType::AgentLogs
            if value.is_none() =>
        {
            Ok(None)
        }
        domain::ApplicationType::AgentFlow | domain::ApplicationType::AgentLogs => {
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
    fn schedule_creation_accepts_omitted_or_empty_timezone() {
        for input in [
            serde_json::json!({"cron": "0 9 * * *"}),
            serde_json::json!({"cron": "0 9 * * *", "timezone": ""}),
        ] {
            let config = serde_json::from_value(input).unwrap();
            let parsed = parse_create_workflow_trigger_config(
                Some(domain::WorkflowTriggerType::Schedule),
                Some(config),
            )
            .unwrap();
            assert!(matches!(
                parsed,
                Some(CreateWorkflowTriggerConfig::Schedule { timezone, .. }) if timezone.is_empty()
            ));
        }
    }

    #[test]
    fn f09a_registry_freezes_all_console_application_bindings() {
        let registry = compile_registry(Arc::new(UnavailableApplicationsPort)).unwrap();
        for declaration in DECLARATIONS {
            assert!(registry
                .binding(&BindingId::new(declaration.binding_id).unwrap())
                .is_some());
        }
        assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    }

    #[test]
    fn managed_hook_projection_validates_forty_agent_logs_and_agent_flow_applications() {
        use crate::routes::applications_group::applications::{
            ApplicationEnvironmentVariableBody, ApplicationTagResponse,
        };
        use extension_contracts::ManagedProjectionContract;

        let output_contract = ManagedProjectionContract {
            contract_id: ApplicationsOutput::CONTRACT_ID.into(),
            contract_version: ApplicationsOutput::CONTRACT_VERSION.into(),
            schema: ApplicationsOutput::managed_projection_schema().unwrap(),
        }
        .compile_for_registration()
        .unwrap();

        let applications = (0..40)
            .map(|index| ApplicationSummaryResponse {
                id: Uuid::from_u128((index + 1) as u128).to_string(),
                application_type: match index % 3 {
                    0 => ApplicationTypeDto::AgentLogs,
                    1 => ApplicationTypeDto::AgentFlow,
                    _ => ApplicationTypeDto::Workflow,
                },
                name: format!("App {index}"),
                description: format!("Description {index}"),
                icon: None,
                icon_type: None,
                icon_background: None,
                created_by: "owner".into(),
                updated_at: "2026-08-02T00:00:00Z".into(),
                tags: (0..if index == 0 { 36 } else { 2 })
                    .map(|tag_idx| ApplicationTagResponse {
                        id: Uuid::from_u128((1_000 + tag_idx) as u128).to_string(),
                        name: format!("tag-{tag_idx}"),
                    })
                    .collect(),
            })
            .collect();

        let projected_apps = ApplicationsOutput::Applications(applications)
            .project_for_managed_hook()
            .expect("40 applications must project bounded hook preview instead of failing");
        assert_eq!(projected_apps["0"].as_array().unwrap().len(), 32);
        assert_eq!(
            projected_apps["0"][0]["application_type"],
            serde_json::json!({"variant": "AgentLogs"})
        );
        assert_eq!(
            projected_apps["0"][1]["application_type"],
            serde_json::json!({"variant": "AgentFlow"})
        );
        assert_eq!(projected_apps["0"][0]["tags"].as_array().unwrap().len(), 32);
        output_contract.validate(&projected_apps).unwrap();

        let projected_catalog = ApplicationsOutput::Catalog(ApplicationCatalogResponse {
            collectors: Vec::new(),
            types: vec![
                ApplicationTypeOptionResponse {
                    value: ApplicationTypeDto::AgentFlow,
                    label: "Agent Flow".into(),
                },
                ApplicationTypeOptionResponse {
                    value: ApplicationTypeDto::Workflow,
                    label: "Workflow".into(),
                },
                ApplicationTypeOptionResponse {
                    value: ApplicationTypeDto::AgentLogs,
                    label: "Agent Logs".into(),
                },
            ],
            workflow_triggers: Vec::new(),
            tags: Vec::new(),
        })
        .project_for_managed_hook()
        .unwrap();
        output_contract.validate(&projected_catalog).unwrap();

        let input_contract = ManagedProjectionContract {
            contract_id: ApplicationsInput::CONTRACT_ID.into(),
            contract_version: ApplicationsInput::CONTRACT_VERSION.into(),
            schema: ApplicationsInput::managed_projection_schema().unwrap(),
        }
        .compile_for_registration()
        .unwrap();

        let projected_env_input = ApplicationsInput::ReplaceEnvironmentVariables {
            application_id: Uuid::from_u128(1),
            body: ReplaceApplicationEnvironmentVariablesBody {
                variables: (0..40)
                    .map(|idx| ApplicationEnvironmentVariableBody {
                        name: format!("VAR_{idx}"),
                        value_type: "string".into(),
                        value: serde_json::json!("v"),
                        description: String::new(),
                    })
                    .collect(),
            },
        }
        .project_for_managed_hook()
        .expect("40 environment variables must project bounded hook preview");
        assert_eq!(
            projected_env_input["body"]["variables"]
                .as_array()
                .unwrap()
                .len(),
            32
        );
        input_contract.validate(&projected_env_input).unwrap();
    }
}

#[cfg(test)]
#[path = "_tests/collectors.rs"]
mod collector_tests;

mod event_projection {
    use super::*;
    use crate::extension_bus::managed_projection::object_schema as object;
    use serde_json::{json, Value};

    pub(super) const OPERATIONS: &[&str] = &[
        "applications.list",
        "applications.create",
        "applications.get",
        "applications.update",
        "applications.delete",
    ];
    fn text() -> Value {
        json!({"type":"string"})
    }
    fn nullable(schema: Value) -> Value {
        json!({"anyOf":[schema,{"type":"null"}]})
    }
    fn application_type() -> Value {
        json!({"type":"string","enum":["agent_logs","agent_flow","workflow"]})
    }
    fn body_schema(create: bool) -> Value {
        let mut schema = object([
            ("name", text()),
            ("description", text()),
            ("icon", nullable(text())),
            ("icon_type", nullable(text())),
            ("icon_background", nullable(text())),
        ]);
        let properties = schema["properties"].as_object_mut().expect("object schema");
        if create {
            properties.insert("application_type".into(), application_type());
            properties.insert(
                "workflow_trigger_type".into(),
                nullable(json!({"type":"string","enum":["extension","schedule","event"]})),
            );
            properties.insert(
                "workflow_trigger_config".into(),
                nullable(object([
                    ("cron", nullable(text())),
                    ("timezone", nullable(text())),
                    ("input_payload", Value::Bool(true)),
                    ("subpath", nullable(text())),
                    ("http_method", nullable(text())),
                    ("response_mode", nullable(text())),
                ])),
            );
        } else {
            properties.insert("tag_ids".into(), json!({"type":"array","items":text()}));
        }
        let required = properties.keys().cloned().map(Value::String).collect();
        schema["required"] = Value::Array(required);
        schema
    }
    pub(super) fn input_schema() -> Value {
        json!({"oneOf":[
            object([("variant",json!({"const":"List"}))]),
            object([("variant",json!({"const":"Create"})),("0",body_schema(true))]),
            object([("variant",json!({"const":"Get"})),("application_id",text())]),
            object([("variant",json!({"const":"Patch"})),("application_id",text()),("body",body_schema(false))]),
            object([("variant",json!({"const":"Delete"})),("application_id",text())]),
        ]})
    }
    pub(super) fn input(input: &ApplicationsInput) -> Option<Value> {
        Some(match input {
            ApplicationsInput::List => json!({"variant":"List"}),
            ApplicationsInput::Create(body) => json!({"variant":"Create","0":{
                "application_type":body.application_type,"workflow_trigger_type":body.workflow_trigger_type,
                "workflow_trigger_config":body.workflow_trigger_config.as_ref().map(|config| json!({
                    "cron":config.cron,"timezone":config.timezone,"input_payload":config.input_payload,
                    "subpath":config.subpath,"http_method":config.http_method,"response_mode":config.response_mode,
                })),
                "name":body.name,"description":body.description,"icon":body.icon,"icon_type":body.icon_type,"icon_background":body.icon_background,
            }}),
            ApplicationsInput::Get { application_id } => {
                json!({"variant":"Get","application_id":application_id})
            }
            ApplicationsInput::Patch {
                application_id,
                body,
            } => json!({"variant":"Patch","application_id":application_id,"body":{
                "name":body.name,"description":body.description,"tag_ids":body.tag_ids,
                "icon":body.icon,"icon_type":body.icon_type,"icon_background":body.icon_background,
            }}),
            ApplicationsInput::Delete { application_id } => {
                json!({"variant":"Delete","application_id":application_id})
            }
            _ => return None,
        })
    }
    fn application_schema(detail: bool) -> Value {
        let mut schema = object([
            ("id", text()),
            ("application_type", application_type()),
            ("name", text()),
            ("description", text()),
            ("icon", nullable(text())),
            ("icon_type", nullable(text())),
            ("icon_background", nullable(text())),
            ("created_by", text()),
            ("updated_at", text()),
            (
                "tags",
                json!({"type":"array","items":object([("id",text()),("name",text())])}),
            ),
        ]);
        if detail {
            schema["properties"]["workflow_trigger_type"] =
                nullable(json!({"type":"string","enum":["extension","schedule","event"]}));
            schema["required"]
                .as_array_mut()
                .expect("object required fields")
                .push(json!("workflow_trigger_type"));
        }
        schema
    }
    pub(super) fn output_schema() -> Value {
        json!({"oneOf":[
            object([("variant",json!({"const":"Applications"})),("0",json!({"type":"array","items":application_schema(false)}))]),
            object([("variant",json!({"const":"Application"})),("0",application_schema(true))]),
            object([("variant",json!({"const":"NoContent"}))]),
        ]})
    }
    fn summary(value: &ApplicationSummaryResponse) -> Value {
        json!({"id":value.id,"application_type":value.application_type,"name":value.name,"description":value.description,
            "icon":value.icon,"icon_type":value.icon_type,"icon_background":value.icon_background,
            "created_by":value.created_by,"updated_at":value.updated_at,"tags":value.tags})
    }
    pub(super) fn output(output: &ApplicationsOutput) -> Option<Value> {
        Some(match output {
            ApplicationsOutput::Applications(values) => {
                json!({"variant":"Applications","0":values.iter().map(summary).collect::<Vec<_>>()})
            }
            ApplicationsOutput::Application(value) => json!({"variant":"Application","0":{
                "workflow_trigger_type":value.workflow_trigger_type,
                "id":value.id,"application_type":value.application_type,"name":value.name,"description":value.description,
                "icon":value.icon,"icon_type":value.icon_type,"icon_background":value.icon_background,
                "created_by":value.created_by,"updated_at":value.updated_at,"tags":value.tags,
            }}),
            ApplicationsOutput::NoContent => json!({"variant":"NoContent"}),
            _ => return None,
        })
    }
}

#[cfg(test)]
#[path = "interface/_tests/event_projection.rs"]
mod event_projection_tests;

impl crate::openapi_interface::contract::ApiInput for ApplicationsInput {
    fn document_input_schema(
        operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<serde_json::Value> {
        (operation.interface_id == "applications.environment-variables.replace").then(|| {
            crate::routes::applications_group::event_projection::environment_input_schema(operation)
        })
    }
    fn project_document_input(
        &self,
        operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        use crate::openapi_interface::contract::to_value;
        use crate::routes::applications_group::event_projection::{environment_body, request};
        let empty = || serde_json::json!({});
        let id_path = |id: &Uuid| serde_json::json!({"application_id": id});
        let null = serde_json::Value::Null;
        let (path_params, body) = match self {
            Self::List | Self::Catalog { .. } => (empty(), null),
            Self::Create(body) => (empty(), to_value(body)?),
            Self::CreateTag(body) => (empty(), to_value(body)?),
            Self::Get { application_id }
            | Self::Delete { application_id }
            | Self::ListEnvironmentVariables { application_id }
            | Self::ListJsDependencies { application_id } => (id_path(application_id), null),
            Self::Patch {
                application_id,
                body,
            } => (id_path(application_id), to_value(body)?),
            Self::ReplaceEnvironmentVariables {
                application_id,
                body,
            } => (id_path(application_id), to_value(&environment_body(body))?),
            Self::ReplaceJsDependency {
                application_id,
                body,
            } => (id_path(application_id), to_value(body)?),
        };
        Ok(request(operation, path_params, empty(), body))
    }
}

impl crate::openapi_interface::contract::ApiOutput for ApplicationsOutput {
    fn document_output_schema(
        operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Option<serde_json::Value> {
        matches!(operation.interface_id.as_str(), "applications.environment-variables.list" | "applications.environment-variables.replace").then(crate::openapi_interface::contract::schema::<Vec<crate::routes::applications_group::event_projection::EnvironmentVariableResponse>>)
    }
    fn project_document_output(
        &self,
        _operation: &crate::openapi_interface::contract::ApiOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        match self {
            Self::Applications(value) => crate::openapi_interface::contract::to_value(value),
            Self::Catalog(value) => crate::openapi_interface::contract::to_value(value),
            Self::Application(value) => crate::openapi_interface::contract::to_value(value),
            Self::Tag(value) => crate::openapi_interface::contract::to_value(value),
            Self::EnvironmentVariables(values) => crate::openapi_interface::contract::to_value(
                &crate::routes::applications_group::event_projection::environment_response(values),
            ),
            Self::JsDependencies(value) => crate::openapi_interface::contract::to_value(value),
            Self::JsDependency(value) => crate::openapi_interface::contract::to_value(value),
            Self::NoContent => Ok(serde_json::Value::Null),
        }
    }
}
