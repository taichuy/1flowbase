use serde_json::{json, Value};
use uuid::Uuid;

use super::{
    ApplicationCatalogResponse, ApplicationCollectorResponse, ApplicationDetailResponse,
    ApplicationJsDependencySelectionResponse, ApplicationSummaryResponse, ApplicationTypeDto,
    ApplicationsInput, ApplicationsOutput, CreateApplicationBody, WorkflowExtensionHttpMethodBody,
    WorkflowExtensionResponseModeBody, WorkflowTriggerTypeDto,
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
    let js_body =
        obj_s! { "installation_id": mp::text_schema(), "alias": bytes_s(), "target": bytes_s() };
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
        ApplicationsInput::CreateTag(b) => var_v("CreateTag", obj_v! { "name": bytes_v(&b.name) }),
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
