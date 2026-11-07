//! Documentation projections of the existing notification wire contracts. These types never
//! deserialize HTTP requests or replace the service/domain DTOs. The contract crates intentionally
//! have no utoipa dependency; keep the concrete fields here aligned with the owners named below.
use serde_json::{json, Value};
use utoipa::{IntoParams, PartialSchema, ToSchema};
use uuid::Uuid;

// Sources: control_plane::notifications::NotificationInstanceListInput and
// control_plane_contracts::ports::{NotificationLogListInput, NotificationChannelListInput}.
#[derive(IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NotificationInstanceQuery {
    pub display_name: Option<String>,
    pub type_code: Option<String>,
    #[param(inline)]
    pub status: Option<domain::NotificationInstanceStatus>,
}
#[derive(IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NotificationLogQuery {
    pub page: Option<u32>,
    pub page_size: Option<u32>,
    pub instance_id: Option<Uuid>,
    pub type_code: Option<String>,
    pub status: Option<String>,
    #[param(inline)]
    pub source: Option<domain::NotificationSource>,
    pub created_from: Option<String>,
    pub created_to: Option<String>,
}
#[derive(IntoParams)]
#[into_params(parameter_in = Query)]
pub struct NotificationChannelQuery {
    pub cursor: Option<String>,
    pub page_size: Option<u32>,
    #[param(inline)]
    pub read_status: Option<domain::NotificationReadStatus>,
}

fn text() -> Value {
    json!({"type":"string"})
}
fn boolean() -> Value {
    json!({"type":"boolean"})
}
fn unsigned() -> Value {
    json!({"type":"integer","minimum":0})
}
fn uuid() -> Value {
    json!({"type":"string","format":"uuid"})
}
fn nullable(schema: Value) -> Value {
    json!({"anyOf":[schema,{"type":"null"}]})
}
fn array(items: Value) -> Value {
    json!({"type":"array","items":items})
}
fn map(values: Value) -> Value {
    json!({"type":"object","additionalProperties":values})
}
fn object(properties: Value, required: &[&str]) -> Value {
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn fields(properties: Value) -> Value {
    let required = properties
        .as_object()
        .expect("schema properties")
        .keys()
        .cloned()
        .collect::<Vec<_>>();
    json!({"type":"object","properties":properties,"required":required,"additionalProperties":false})
}
fn choice(values: &[&str]) -> Value {
    json!({"type":"string","enum":values})
}
fn dynamic(description: &str) -> Value {
    json!({"type":["null","boolean","number","string","array","object"],"description":description})
}
fn instance_status() -> Value {
    choice(&["enabled", "disabled"])
}

// Sources: control_plane::notifications::{CreateNotificationInstance, PatchNotificationInstance}.
fn create_instance_schema() -> Value {
    object(
        json!({"type_code":text(),"installation_id":nullable(uuid()),"display_name":text(),
        "config_json":dynamic("Configuration validated against the selected notification type config_schema; password fields are accepted here and stored encrypted."),
        "status":nullable(instance_status())}),
        &["type_code", "display_name", "config_json"],
    )
}
fn patch_instance_schema() -> Value {
    object(
        json!({"display_name":nullable(text()),"status":nullable(instance_status()),
        "config_json":nullable(dynamic("Configuration patch validated against the selected notification type config_schema; omitted password fields retain their stored values."))}),
        &[],
    )
}

// Sources: extension_contracts::notification_contract::managed::{NotificationAuthorizationOperation,
// NotificationAuthorizationView, NotificationAuthorizationUserAction, NotificationDeliveryWindowView}.
fn authorization_operation_schema() -> Value {
    let mut operations = vec![fields(json!({"type":choice(&["begin"]),"action":text()}))];
    for operation in ["poll", "cancel", "maintain"] {
        operations.push(fields(json!({"type":choice(&[operation])})));
    }
    json!({"oneOf":operations})
}
fn authorization_view_schema() -> Value {
    object(
        json!({
            "delivery_window":object(json!({"status":choice(&["unknown","waiting_for_message","active","expired"]),
                "last_received_at_ms":unsigned(),"expires_at_ms":unsigned()}), &["status"]),
            "status":choice(&["unauthorized","pending","authorized","invalid"]),
            "user_action":object(json!({"kind":choice(&["qr_code"]),"qr_code_url":text(),
                "poll_interval_seconds":unsigned(),"expires_at":text()}), &["kind","qr_code_url","poll_interval_seconds"])
        }),
        &["status"],
    )
}

// Sources: control_plane::notifications::NotificationInstance flattens
// domain::NotificationInstanceRecord. These timestamps use explicit RFC3339 serde modules in the
// domain owner. They are strings, unlike time's default nine-element tuple representation.
fn instance_schema() -> Value {
    fields(
        json!({"id":uuid(),"workspace_id":uuid(),"type_code":text(),"installation_id":nullable(uuid()),
        "display_name":text(),"status":instance_status(),
        "config_json":dynamic("Public configuration projected from the selected type config_schema with stored credentials removed."),
        "configured_secret_fields":array(text()),
        "created_at":{"type":"string","format":"date-time"},"updated_at":{"type":"string","format":"date-time"},
        "available":boolean(),"unavailable_reason":nullable(text()),"authorization":nullable(authorization_view_schema())}),
    )
}

// Source: extension_contracts::provider_contract::{PluginFormSchema, PluginFormFieldSchema,
// PluginFormOption, PluginFormCondition}. Option fields without skip_serializing_if are present
// with null; serde(default) does not omit fields on serialization.
fn form_option_schema() -> Value {
    fields(
        json!({"label":text(),"value":dynamic("Option value owned by the notification form descriptor."),
        "description":nullable(text()),"disabled":nullable(boolean())}),
    )
}
fn form_condition_schema() -> Value {
    fields(
        json!({"field":text(),"operator":text(),"value":dynamic("Optional descriptor condition JSON value, serialized as null when absent."),
        "values":array(dynamic("Descriptor-owned condition JSON value."))}),
    )
}
fn form_field_schema() -> Value {
    let mut properties = json!({"key":text(),"label":text(),"type":text(),
        "control":nullable(text()),"group":nullable(text()),"order":nullable(json!({"type":"integer","format":"int32"})),
        "advanced":nullable(boolean()),"required":nullable(boolean()),"send_mode":nullable(text()),
        "enabled_by_default":nullable(boolean()),"description":nullable(text()),"placeholder":nullable(text()),
        "default_value":dynamic("Optional default JSON value governed by the form field's canonical schema."),
        "precision":nullable(json!({"type":"integer","minimum":0,"maximum":u32::MAX})),"unit":nullable(text()),
        "options":array(form_option_schema()),"visible_when":array(form_condition_schema()),"disabled_when":array(form_condition_schema())});
    for name in ["min", "max", "step"] {
        properties[name] = nullable(json!({"type":"number","format":"double"}));
    }
    fields(properties)
}
fn form_schema() -> Value {
    fields(
        json!({"schema_version":text(),"title":nullable(text()),"description":nullable(text()),"fields":array(form_field_schema())}),
    )
}

// Source: extension_contracts::notification_contract::i18n. These dictionary/option fields are
// omitted when empty/None; they are not the always-present nullable form fields above.
fn form_locale_schema() -> Value {
    object(
        json!({"title":text(),"description":text(),"fields":map(object(json!({"label":text(),
        "description":text(),"placeholder":text()}), &[]))}),
        &[],
    )
}
fn type_locale_schema() -> Value {
    object(
        json!({"display_name":text(),"description":text(),"config_form":form_locale_schema(),
        "input_form":form_locale_schema(),"authorization":object(json!({"actions":map(fields(json!({"label":text()})))}), &[])}),
        &[],
    )
}
fn authorization_descriptor_schema() -> Value {
    fields(
        json!({"actions":array(fields(json!({"code":text(),"label":text(),"kind":choice(&["qr_code"])}))),
        "managed_secret_keys":array(text()),"supports_delivery_window":boolean()}),
    )
}
fn log_descriptor_schema() -> Value {
    object(
        json!({"title":text(),"content":text(),"extra":map(fields(json!({"source":choice(&["input","result"]),"pointer":text()})))}),
        &["content"],
    )
}
fn receive_descriptor_schema() -> Value {
    fields(
        json!({"contribution_id":text(),"default_system_prompt":text(),"mcp_tools":array(fields(json!({"name":text(),
        "description":text(),"input_schema":dynamic("Input JSON Schema owned by this declared notification MCP tool."),
        "operation":choice(&["history_messages","history_files","read_file"])})))}),
    )
}

// Sources: control_plane::notifications::NotificationTypeCatalogEntry flattens
// extension_contracts::NotificationTypeDescriptor, including the form/i18n/managed/log/receive
// descriptor leaves. JSON Schema values remain dynamic; no strongly typed descriptor is erased.
fn type_catalog_entry_schema() -> Value {
    object(
        json!({"i18n":map(type_locale_schema()),"type_code":text(),"display_name":text(),"description":text(),
        "config_schema":dynamic("Package-owned JSON Schema governing configuration."),"config_form":form_schema(),
        "input_schema":dynamic("Package-owned JSON Schema governing business invocation input."),"input_form":form_schema(),
        "output_schema":dynamic("Package-owned JSON Schema governing the validated business result."),"contribution_id":text(),
        "log":log_descriptor_schema(),"authorization":authorization_descriptor_schema(),"receive":receive_descriptor_schema(),
        "installation_id":nullable(uuid()),"available":boolean(),"unavailable_reason":nullable(text())}),
        &[
            "type_code",
            "display_name",
            "description",
            "config_schema",
            "config_form",
            "input_schema",
            "input_form",
            "output_schema",
            "contribution_id",
            "installation_id",
            "available",
            "unavailable_reason",
        ],
    )
}

// Source: notifications::interface::Adapter maps successful Delete to Value::Null.
fn deleted_schema() -> Value {
    json!({"type":"null"})
}

macro_rules! projection {
    ($name:ident, $schema:ident) => {
        pub struct $name;
        impl ToSchema for $name {}
        impl PartialSchema for $name {
            fn schema() -> utoipa::openapi::RefOr<utoipa::openapi::schema::Schema> {
                serde_json::from_value($schema()).expect("static notification wire schema")
            }
        }
    };
}
projection!(NotificationDeletedSchema, deleted_schema);
projection!(CreateNotificationInstanceSchema, create_instance_schema);
projection!(PatchNotificationInstanceSchema, patch_instance_schema);
projection!(NotificationInstanceSchema, instance_schema);
projection!(
    NotificationAuthorizationOperationSchema,
    authorization_operation_schema
);
projection!(
    NotificationAuthorizationViewSchema,
    authorization_view_schema
);
projection!(
    NotificationTypeCatalogEntrySchema,
    type_catalog_entry_schema
);

#[cfg_attr(test, api_test_selection::group(routes))]
#[cfg(test)]
#[path = "_tests/openapi.rs"]
mod tests;
