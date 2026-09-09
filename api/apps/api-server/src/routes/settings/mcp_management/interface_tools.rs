use std::{collections::HashMap, sync::Arc};

use interface_runtime::{InterfaceContract, UserPrincipal};

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum McpToolsInput {
    List,
    Create(CreateMcpToolBody),
    Get(String),
    Update(String, UpdateMcpToolBody),
    Delete(String),
    RefreshDescription(String),
    CheckDescription(String, McpDescriptionCheckBody),
}

impl InterfaceContract for McpToolsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[
                        ("tool_id", mp::text_schema()),
                        (
                            "des_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "short_description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "full_description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "execution_target",
                            mp::union_schema(vec![
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("InterfaceWrapper")),
                                    ("interface_id", mp::text_schema()),
                                ]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("McpProxy")),
                                    ("upstream_connection_id", mp::text_schema()),
                                    (
                                        "remote_tool_name",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "source_schema_hash",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                ]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("AssistantClient")),
                                    ("capability_code", mp::text_schema()),
                                ]),
                            ]),
                        ),
                        ("parameter_schema", mp::json_summary_schema()),
                        ("result_schema", mp::json_summary_schema()),
                        ("input_mapping", mp::json_summary_schema()),
                        ("output_mapping", mp::json_summary_schema()),
                        (
                            "permission_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "risk_level",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("status", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "des_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "short_description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "full_description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "execution_target",
                            mp::union_schema(vec![
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("InterfaceWrapper")),
                                    ("interface_id", mp::text_schema()),
                                ]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("McpProxy")),
                                    ("upstream_connection_id", mp::text_schema()),
                                    (
                                        "remote_tool_name",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "source_schema_hash",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                ]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("AssistantClient")),
                                    ("capability_code", mp::text_schema()),
                                ]),
                            ]),
                        ),
                        ("parameter_schema", mp::json_summary_schema()),
                        ("result_schema", mp::json_summary_schema()),
                        ("input_mapping", mp::json_summary_schema()),
                        ("output_mapping", mp::json_summary_schema()),
                        (
                            "permission_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "risk_level",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("status", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RefreshDescription")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CheckDescription")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[(
                        "des_id",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    )]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::List => mp::object_value(&[("variant",serde_json::Value::String("List".to_owned()))]), Self::Create(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned())), ("0",mp::object_value(&[("tool_id",mp::text(&(_field_0).tool_id)?), ("des_id",match (&(_field_0).des_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))])), ("short_description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).short_description).len()))])), ("full_description",match (&(_field_0).full_description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("execution_target",match &(_field_0).execution_target {crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::InterfaceWrapper {interface_id: _field_interface_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("InterfaceWrapper".to_owned())), ("interface_id",mp::text(_field_interface_id)?)]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::McpProxy {upstream_connection_id: _field_upstream_connection_id, remote_tool_name: _field_remote_tool_name, source_schema_hash: _field_source_schema_hash, .. } => mp::object_value(&[("variant",serde_json::Value::String("McpProxy".to_owned())), ("upstream_connection_id",mp::text(_field_upstream_connection_id)?), ("remote_tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_remote_tool_name).len()))])), ("source_schema_hash",mp::object_value(&[("byte_count",serde_json::json!((_field_source_schema_hash).len()))]))]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::AssistantClient {capability_code: _field_capability_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("AssistantClient".to_owned())), ("capability_code",mp::text(_field_capability_code)?)])}), ("parameter_schema",mp::json_summary(&(_field_0).parameter_schema)), ("result_schema",mp::json_summary(&(_field_0).result_schema)), ("input_mapping",mp::json_summary(&(_field_0).input_mapping)), ("output_mapping",mp::json_summary(&(_field_0).output_mapping)), ("permission_code",match (&(_field_0).permission_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).risk_level).len()))])), ("status",mp::text(&(_field_0).status)?)]))]), Self::Get(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::Update(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_1).name).len()))])), ("des_id",match (&(_field_1).des_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("short_description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_1).short_description).len()))])), ("full_description",match (&(_field_1).full_description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("execution_target",match &(_field_1).execution_target {crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::InterfaceWrapper {interface_id: _field_interface_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("InterfaceWrapper".to_owned())), ("interface_id",mp::text(_field_interface_id)?)]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::McpProxy {upstream_connection_id: _field_upstream_connection_id, remote_tool_name: _field_remote_tool_name, source_schema_hash: _field_source_schema_hash, .. } => mp::object_value(&[("variant",serde_json::Value::String("McpProxy".to_owned())), ("upstream_connection_id",mp::text(_field_upstream_connection_id)?), ("remote_tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_remote_tool_name).len()))])), ("source_schema_hash",mp::object_value(&[("byte_count",serde_json::json!((_field_source_schema_hash).len()))]))]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::AssistantClient {capability_code: _field_capability_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("AssistantClient".to_owned())), ("capability_code",mp::text(_field_capability_code)?)])}), ("parameter_schema",mp::json_summary(&(_field_1).parameter_schema)), ("result_schema",mp::json_summary(&(_field_1).result_schema)), ("input_mapping",mp::json_summary(&(_field_1).input_mapping)), ("output_mapping",mp::json_summary(&(_field_1).output_mapping)), ("permission_code",match (&(_field_1).permission_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(_field_1).risk_level).len()))])), ("status",mp::text(&(_field_1).status)?)]))]), Self::Delete(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::RefreshDescription(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("RefreshDescription".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::CheckDescription(_field_0, _field_1) => mp::object_value(&[("variant",serde_json::Value::String("CheckDescription".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))])), ("1",mp::object_value(&[("des_id",match (&(_field_1).des_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))])})
    }

    const CONTRACT_ID: &'static str = "console-mcp-tools-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed MCP tool output is projected immediately into the console response"
)]
pub(crate) enum McpToolsOutput {
    Tools(Vec<McpToolResponse>),
    Tool(McpToolResponse),
    Check(McpDescriptionCheckResponse),
    NoContent,
}

impl InterfaceContract for McpToolsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tools")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("tool_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("short_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("full_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("execution_target",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("InterfaceWrapper")), ("interface_id",mp::text_schema())]), mp::object_schema(&[("variant",mp::tag_schema("McpProxy")), ("upstream_connection_id",mp::text_schema()), ("remote_tool_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_schema_hash",mp::object_schema(&[("byte_count",mp::count_schema())]))]), mp::object_schema(&[("variant",mp::tag_schema("AssistantClient")), ("capability_code",mp::text_schema())])])), ("operation",mp::text_schema()), ("parameter_schema",mp::json_summary_schema()), ("result_schema",mp::json_summary_schema()), ("input_mapping",mp::json_summary_schema()), ("output_mapping",mp::json_summary_schema()), ("permission_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("risk_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("des_id",mp::text_schema()), ("des_id_required",serde_json::json!({"type":"boolean"})), ("status",mp::text_schema()), ("availability_status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Available"))]), mp::object_schema(&[("variant",mp::tag_schema("InterfaceMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamDisabled"))]), mp::object_schema(&[("variant",mp::tag_schema("CredentialsMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamToolMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("MappingInvalid"))])])), ("availability_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("revision",serde_json::json!({"type":"integer"})), ("managed_by",serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tool")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("tool_id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "short_description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "full_description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "execution_target",
                            mp::union_schema(vec![
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("InterfaceWrapper")),
                                    ("interface_id", mp::text_schema()),
                                ]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("McpProxy")),
                                    ("upstream_connection_id", mp::text_schema()),
                                    (
                                        "remote_tool_name",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "source_schema_hash",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                ]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("AssistantClient")),
                                    ("capability_code", mp::text_schema()),
                                ]),
                            ]),
                        ),
                        ("operation", mp::text_schema()),
                        ("parameter_schema", mp::json_summary_schema()),
                        ("result_schema", mp::json_summary_schema()),
                        ("input_mapping", mp::json_summary_schema()),
                        ("output_mapping", mp::json_summary_schema()),
                        (
                            "permission_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "risk_level",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("des_id", mp::text_schema()),
                        ("des_id_required", serde_json::json!({"type":"boolean"})),
                        ("status", mp::text_schema()),
                        (
                            "availability_status",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Available"))]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("InterfaceMissing"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("UpstreamDisabled"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("CredentialsMissing"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("UpstreamToolMissing"),
                                )]),
                                mp::object_schema(&[("variant", mp::tag_schema("MappingInvalid"))]),
                            ]),
                        ),
                        (
                            "availability_reason",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("revision", serde_json::json!({"type":"integer"})),
                        (
                            "managed_by",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Check")),
                (
                    "0",
                    mp::object_schema(&[
                        ("accepted", serde_json::json!({"type":"boolean"})),
                        (
                            "current_des_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Tools(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Tools".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("workspace_id",mp::text(&(item).workspace_id)?), ("tool_id",mp::text(&(item).tool_id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("short_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).short_description).len()))])), ("full_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).full_description).len()))])), ("execution_target",match &(item).execution_target {crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::InterfaceWrapper {interface_id: _field_interface_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("InterfaceWrapper".to_owned())), ("interface_id",mp::text(_field_interface_id)?)]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::McpProxy {upstream_connection_id: _field_upstream_connection_id, remote_tool_name: _field_remote_tool_name, source_schema_hash: _field_source_schema_hash, .. } => mp::object_value(&[("variant",serde_json::Value::String("McpProxy".to_owned())), ("upstream_connection_id",mp::text(_field_upstream_connection_id)?), ("remote_tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_remote_tool_name).len()))])), ("source_schema_hash",mp::object_value(&[("byte_count",serde_json::json!((_field_source_schema_hash).len()))]))]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::AssistantClient {capability_code: _field_capability_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("AssistantClient".to_owned())), ("capability_code",mp::text(_field_capability_code)?)])}), ("operation",mp::text(&(item).operation)?), ("parameter_schema",mp::json_summary(&(item).parameter_schema)), ("result_schema",mp::json_summary(&(item).result_schema)), ("input_mapping",mp::json_summary(&(item).input_mapping)), ("output_mapping",mp::json_summary(&(item).output_mapping)), ("permission_code",match (&(item).permission_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(item).risk_level).len()))])), ("des_id",mp::text(&(item).des_id)?), ("des_id_required",serde_json::Value::Bool(*(&(item).des_id_required))), ("status",mp::text(&(item).status)?), ("availability_status",match &(item).availability_status {crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::Available => mp::object_value(&[("variant",serde_json::Value::String("Available".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::InterfaceMissing => mp::object_value(&[("variant",serde_json::Value::String("InterfaceMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamDisabled => mp::object_value(&[("variant",serde_json::Value::String("UpstreamDisabled".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::CredentialsMissing => mp::object_value(&[("variant",serde_json::Value::String("CredentialsMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamToolMissing => mp::object_value(&[("variant",serde_json::Value::String("UpstreamToolMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::MappingInvalid => mp::object_value(&[("variant",serde_json::Value::String("MappingInvalid".to_owned()))])}), ("availability_reason",match (&(item).availability_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("revision",serde_json::json!(*(&(item).revision))), ("managed_by",match (&(item).managed_by).as_ref() { Some(item) => mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("bundle_id",mp::text(&(item).bundle_id)?), ("bundle_version",mp::text(&(item).bundle_version)?)]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]), Self::Tool(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Tool".to_owned())), ("0",mp::object_value(&[("id",mp::text(&(_field_0).id)?), ("workspace_id",mp::text(&(_field_0).workspace_id)?), ("tool_id",mp::text(&(_field_0).tool_id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))])), ("short_description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).short_description).len()))])), ("full_description",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).full_description).len()))])), ("execution_target",match &(_field_0).execution_target {crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::InterfaceWrapper {interface_id: _field_interface_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("InterfaceWrapper".to_owned())), ("interface_id",mp::text(_field_interface_id)?)]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::McpProxy {upstream_connection_id: _field_upstream_connection_id, remote_tool_name: _field_remote_tool_name, source_schema_hash: _field_source_schema_hash, .. } => mp::object_value(&[("variant",serde_json::Value::String("McpProxy".to_owned())), ("upstream_connection_id",mp::text(_field_upstream_connection_id)?), ("remote_tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_remote_tool_name).len()))])), ("source_schema_hash",mp::object_value(&[("byte_count",serde_json::json!((_field_source_schema_hash).len()))]))]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::AssistantClient {capability_code: _field_capability_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("AssistantClient".to_owned())), ("capability_code",mp::text(_field_capability_code)?)])}), ("operation",mp::text(&(_field_0).operation)?), ("parameter_schema",mp::json_summary(&(_field_0).parameter_schema)), ("result_schema",mp::json_summary(&(_field_0).result_schema)), ("input_mapping",mp::json_summary(&(_field_0).input_mapping)), ("output_mapping",mp::json_summary(&(_field_0).output_mapping)), ("permission_code",match (&(_field_0).permission_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).risk_level).len()))])), ("des_id",mp::text(&(_field_0).des_id)?), ("des_id_required",serde_json::Value::Bool(*(&(_field_0).des_id_required))), ("status",mp::text(&(_field_0).status)?), ("availability_status",match &(_field_0).availability_status {crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::Available => mp::object_value(&[("variant",serde_json::Value::String("Available".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::InterfaceMissing => mp::object_value(&[("variant",serde_json::Value::String("InterfaceMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamDisabled => mp::object_value(&[("variant",serde_json::Value::String("UpstreamDisabled".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::CredentialsMissing => mp::object_value(&[("variant",serde_json::Value::String("CredentialsMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamToolMissing => mp::object_value(&[("variant",serde_json::Value::String("UpstreamToolMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::MappingInvalid => mp::object_value(&[("variant",serde_json::Value::String("MappingInvalid".to_owned()))])}), ("availability_reason",match (&(_field_0).availability_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("revision",serde_json::json!(*(&(_field_0).revision))), ("managed_by",match (&(_field_0).managed_by).as_ref() { Some(item) => mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("bundle_id",mp::text(&(item).bundle_id)?), ("bundle_version",mp::text(&(item).bundle_version)?)]), None => serde_json::Value::Null })]))]), Self::Check(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Check".to_owned())), ("0",mp::object_value(&[("accepted",serde_json::Value::Bool(*(&(_field_0).accepted))), ("current_des_id",match (&(_field_0).current_des_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))]), Self::NoContent => mp::object_value(&[("variant",serde_json::Value::String("NoContent".to_owned()))])})
    }

    const CONTRACT_ID: &'static str = "console-mcp-tools-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct McpToolsAdapter(interface_catalog::McpInterfaceCatalogDependencies);

impl McpToolsAdapter {
    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: McpToolsInput,
    ) -> Result<McpToolsOutput, ApiError> {
        let actor = principal.actor();
        let service = McpManagementService::new(self.0.store.clone());
        match input {
            McpToolsInput::List => {
                let snapshot = service.read_catalog_for_actor(actor).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                let mut tools = Vec::with_capacity(snapshot.tools.len());
                for record in snapshot.tools {
                    tools.push(
                        to_tool_response_for_actor(&self.0.store, actor, record, &operations)
                            .await?,
                    );
                }
                Ok(McpToolsOutput::Tools(tools))
            }
            McpToolsInput::Create(body) => {
                let interface_id = interface_target_id(&body.execution_target)?;
                let interface_entry =
                    interface_catalog::bindable_mcp_interface_with(&self.0, actor, interface_id)
                        .await?;
                let operation = interface_operation(&interface_entry);
                let record = service
                    .create_tool_for_actor(
                        actor,
                        to_create_tool_command(actor.user_id, body, interface_entry)?,
                    )
                    .await?;
                Ok(McpToolsOutput::Tool(to_tool_response_with_operation(
                    record,
                    operation,
                    domain::McpToolAvailabilityStatus::Available,
                )))
            }
            McpToolsInput::Get(tool_id) => {
                let record = service.get_tool(actor.user_id, &tool_id).await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpToolsOutput::Tool(
                    to_tool_response_for_actor(&self.0.store, actor, record, &operations).await?,
                ))
            }
            McpToolsInput::Update(tool_id, body) => match &body.execution_target {
                McpToolExecutionTargetDto::InterfaceWrapper { interface_id } => {
                    let interface_entry = interface_catalog::bindable_mcp_interface_with(
                        &self.0,
                        actor,
                        interface_id,
                    )
                    .await?;
                    let operation = interface_operation(&interface_entry);
                    let record = service
                        .update_tool_for_actor(
                            actor,
                            to_update_tool_command(actor.user_id, tool_id, body, interface_entry)?,
                        )
                        .await?;
                    Ok(McpToolsOutput::Tool(to_tool_response_with_operation(
                        record,
                        operation,
                        domain::McpToolAvailabilityStatus::Available,
                    )))
                }
                McpToolExecutionTargetDto::McpProxy { .. } => {
                    let execution_target = to_domain_execution_target(&body.execution_target)?;
                    let record = service
                        .update_proxy_tool_for_actor(
                            actor,
                            UpdateMcpProxyToolCommand {
                                actor_user_id: actor.user_id,
                                tool_id,
                                des_id: body.des_id,
                                name: body.name,
                                short_description: body.short_description,
                                full_description: body.full_description.unwrap_or_default(),
                                execution_target,
                                parameter_schema: body.parameter_schema,
                                result_schema: body.result_schema,
                                input_mapping: body.input_mapping,
                                output_mapping: body.output_mapping,
                                risk_level: parse_risk_level(&body.risk_level)?,
                                status: parse_tool_status(&body.status)?,
                            },
                        )
                        .await?;
                    let operations =
                        interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                    Ok(McpToolsOutput::Tool(
                        to_tool_response_for_actor(&self.0.store, actor, record, &operations)
                            .await?,
                    ))
                }
                McpToolExecutionTargetDto::AssistantClient { .. } => {
                    let execution_target = to_domain_execution_target(&body.execution_target)?;
                    let record = service
                        .update_proxy_tool_for_actor(
                            actor,
                            UpdateMcpProxyToolCommand {
                                actor_user_id: actor.user_id,
                                tool_id,
                                des_id: body.des_id,
                                name: body.name,
                                short_description: body.short_description,
                                full_description: body.full_description.unwrap_or_default(),
                                execution_target,
                                parameter_schema: body.parameter_schema,
                                result_schema: body.result_schema,
                                input_mapping: body.input_mapping,
                                output_mapping: body.output_mapping,
                                risk_level: parse_risk_level(&body.risk_level)?,
                                status: parse_tool_status(&body.status)?,
                            },
                        )
                        .await?;
                    Ok(McpToolsOutput::Tool(to_tool_response(
                        record,
                        &HashMap::new(),
                    )))
                }
            },
            McpToolsInput::Delete(tool_id) => {
                service.delete_tool_for_actor(actor, &tool_id).await?;
                Ok(McpToolsOutput::NoContent)
            }
            McpToolsInput::RefreshDescription(tool_id) => {
                let record = service
                    .refresh_tool_description_for_actor(
                        actor,
                        RefreshMcpToolDescriptionCommand {
                            actor_user_id: actor.user_id,
                            tool_id,
                        },
                    )
                    .await?;
                let operations =
                    interface_catalog::mcp_interface_operation_map_with(&self.0, actor).await?;
                Ok(McpToolsOutput::Tool(to_tool_response(record, &operations)))
            }
            McpToolsInput::CheckDescription(tool_id, body) => {
                let result = service
                    .description_check(actor.user_id, &tool_id, body.des_id.as_deref())
                    .await?;
                Ok(McpToolsOutput::Check(McpDescriptionCheckResponse {
                    accepted: result.accepted,
                    current_des_id: result.current_des_id,
                }))
            }
        }
    }
}

impl ConsoleInterfacePort<McpToolsInput, McpToolsOutput> for McpToolsAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: McpToolsInput,
    ) -> ConsoleInterfaceFuture<'a, McpToolsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.view",
        binding_id: "http.console.mcp.tools.get.v1",
        method: "GET",
        path: "/api/console/mcp/tools",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.create",
        binding_id: "http.console.mcp.tools.post.v1",
        method: "POST",
        path: "/api/console/mcp/tools",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.view",
        binding_id: "http.console.mcp.tool.get.v1",
        method: "GET",
        path: "/api/console/mcp/tools/:tool_id",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.update",
        binding_id: "http.console.mcp.tool.put.v1",
        method: "PUT",
        path: "/api/console/mcp/tools/:tool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.delete",
        binding_id: "http.console.mcp.tool.delete.v1",
        method: "DELETE",
        path: "/api/console/mcp/tools/:tool_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.description.refresh",
        binding_id: "http.console.mcp.tool-description.refresh.v1",
        method: "POST",
        path: "/api/console/mcp/tools/:tool_id/description/refresh",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "mcp.tools.description.check",
        binding_id: "http.console.mcp.tool-description.check.v1",
        method: "POST",
        path: "/api/console/mcp/tools/:tool_id/description-check",
        mutating: false,
    },
];

pub(crate) fn compile_registry(
    dependencies: interface_catalog::McpInterfaceCatalogDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-mcp-tools",
        "graph:console-mcp-tools-v1",
        DECLARATIONS,
        Arc::new(McpToolsAdapter(dependencies)),
    )
}
