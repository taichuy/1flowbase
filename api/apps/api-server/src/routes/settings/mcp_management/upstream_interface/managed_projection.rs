use super::*;

impl InterfaceContract for McpUpstreamInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("transport", mp::text_schema()),
                        ("auth_type", mp::text_schema()),
                        (
                            "custom_header_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                ("connection_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("transport", mp::text_schema()),
                        ("auth_type", mp::text_schema()),
                        (
                            "custom_header_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                ("variant", mp::tag_schema("SaveCredentials")),
                ("connection_id", mp::text_schema()),
                (
                    "body",
                    mp::union_schema(vec![
                        mp::object_schema(&[("variant", mp::tag_schema("Bearer"))]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("CustomHeader")),
                            (
                                "header_name",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                            (
                                "header_value",
                                mp::object_schema(&[("byte_count", mp::count_schema())]),
                            ),
                        ]),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteCredentials")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("TestDraft")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "connection_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("transport", mp::text_schema()),
                        ("auth_type", mp::text_schema()),
                        (
                            "custom_header_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Test")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Discover")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Import")),
                ("connection_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "remote_tool_names",
                        mp::object_schema(&[("item_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Debug")),
                ("tool_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("arguments", mp::json_summary_schema())]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::List => mp::object_value(&[("variant",serde_json::Value::String("List".to_owned()))]), Self::Create(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned())), ("0",mp::object_value(&[("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).name).len()))])), ("transport",mp::text(&(_field_0).transport)?), ("auth_type",mp::text(&(_field_0).auth_type)?), ("custom_header_name",match (&(_field_0).custom_header_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("status",mp::text(&(_field_0).status)?)]))]), Self::Update {connection_id: _field_connection_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned())), ("connection_id",mp::text(_field_connection_id)?), ("body",mp::object_value(&[("name",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).name).len()))])), ("transport",mp::text(&(_field_body).transport)?), ("auth_type",mp::text(&(_field_body).auth_type)?), ("custom_header_name",match (&(_field_body).custom_header_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("status",mp::text(&(_field_body).status)?)]))]), Self::Delete(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Delete".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::SaveCredentials {connection_id: _field_connection_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("SaveCredentials".to_owned())), ("connection_id",mp::text(_field_connection_id)?), ("body",match _field_body {crate::routes::settings_group::mcp_management::upstream::SaveMcpUpstreamCredentialBody::Bearer { .. } => mp::object_value(&[("variant",serde_json::Value::String("Bearer".to_owned()))]), crate::routes::settings_group::mcp_management::upstream::SaveMcpUpstreamCredentialBody::CustomHeader {header_name: _field_header_name, header_value: _field_header_value, .. } => mp::object_value(&[("variant",serde_json::Value::String("CustomHeader".to_owned())), ("header_name",mp::object_value(&[("byte_count",serde_json::json!((_field_header_name).len()))])), ("header_value",mp::object_value(&[("byte_count",serde_json::json!((_field_header_value).len()))]))])})]), Self::DeleteCredentials(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("DeleteCredentials".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::TestDraft(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("TestDraft".to_owned())), ("0",mp::object_value(&[("connection_id",match (&(_field_0).connection_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("transport",mp::text(&(_field_0).transport)?), ("auth_type",mp::text(&(_field_0).auth_type)?), ("custom_header_name",match (&(_field_0).custom_header_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Test(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Test".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::Discover(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Discover".to_owned())), ("0",mp::object_value(&[("byte_count",serde_json::json!((_field_0).len()))]))]), Self::Import {connection_id: _field_connection_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("Import".to_owned())), ("connection_id",mp::text(_field_connection_id)?), ("body",mp::object_value(&[("remote_tool_names",mp::object_value(&[("item_count",serde_json::json!((&(_field_body).remote_tool_names).len()))]))]))]), Self::Debug {tool_id: _field_tool_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("Debug".to_owned())), ("tool_id",mp::text(_field_tool_id)?), ("body",mp::object_value(&[("arguments",mp::json_summary(&(_field_body).arguments))]))])})
    }

    const CONTRACT_ID: &'static str = "console-mcp-upstream-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for McpUpstreamOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Connections")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("connection_id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("transport",mp::text_schema()), ("auth_type",mp::text_schema()), ("custom_header_name",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("last_connected_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("last_discovered_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("last_error",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Created")),
                (
                    "0",
                    mp::object_schema(&[
                        ("connection_id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("transport", mp::text_schema()),
                        ("auth_type", mp::text_schema()),
                        (
                            "custom_header_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "last_connected_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "last_discovered_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "last_error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Connection")),
                (
                    "0",
                    mp::object_schema(&[
                        ("connection_id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("transport", mp::text_schema()),
                        ("auth_type", mp::text_schema()),
                        (
                            "custom_header_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "last_connected_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "last_discovered_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "last_error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DraftTest")),
                (
                    "0",
                    mp::object_schema(&[
                        ("ok", serde_json::json!({"type":"boolean"})),
                        (
                            "server_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "server_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "protocol_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "tested_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Test")),
                (
                    "0",
                    mp::object_schema(&[
                        ("connection_id", mp::text_schema()),
                        ("ok", serde_json::json!({"type":"boolean"})),
                        (
                            "server_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "server_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "protocol_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "tested_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Discovery")),
                (
                    "0",
                    mp::object_schema(&[
                        ("connection_id", mp::text_schema()),
                        (
                            "server_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "server_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("protocol_version", mp::text_schema()),
                        (
                            "discovered_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("remote_tool_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("input_schema",mp::json_summary_schema()), ("output_schema",mp::json_summary_schema()), ("source_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("imported_tool_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("schema_hash",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Imported")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("tool_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("short_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("full_description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("execution_target",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("InterfaceWrapper")), ("interface_id",mp::text_schema())]), mp::object_schema(&[("variant",mp::tag_schema("McpProxy")), ("upstream_connection_id",mp::text_schema()), ("remote_tool_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_schema_hash",mp::object_schema(&[("byte_count",mp::count_schema())]))]), mp::object_schema(&[("variant",mp::tag_schema("AssistantClient")), ("capability_code",mp::text_schema())])])), ("operation",mp::text_schema()), ("parameter_schema",mp::json_summary_schema()), ("result_schema",mp::json_summary_schema()), ("input_mapping",mp::json_summary_schema()), ("output_mapping",mp::json_summary_schema()), ("permission_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("risk_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("des_id",mp::text_schema()), ("des_id_required",serde_json::json!({"type":"boolean"})), ("status",mp::text_schema()), ("availability_status",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Available"))]), mp::object_schema(&[("variant",mp::tag_schema("InterfaceMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamDisabled"))]), mp::object_schema(&[("variant",mp::tag_schema("CredentialsMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("UpstreamToolMissing"))]), mp::object_schema(&[("variant",mp::tag_schema("MappingInvalid"))])])), ("availability_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("revision",serde_json::json!({"type":"integer"})), ("managed_by",serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Debug")),
                (
                    "0",
                    mp::object_schema(&[
                        ("local_arguments", mp::json_summary_schema()),
                        ("remote_arguments", mp::json_summary_schema()),
                        ("upstream_result", mp::json_summary_schema()),
                        ("mapped_result", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Connections(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Connections".to_owned()),
                ),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("connection_id", mp::text(&(item).connection_id)?),
                                    ("workspace_id", mp::text(&(item).workspace_id)?),
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                    ("transport", mp::text(&(item).transport)?),
                                    ("auth_type", mp::text(&(item).auth_type)?),
                                    (
                                        "custom_header_name",
                                        match (&(item).custom_header_name).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    (
                                        "last_connected_at",
                                        match (&(item).last_connected_at).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "last_discovered_at",
                                        match (&(item).last_discovered_at).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "last_error",
                                        match (&(item).last_error).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("created_at", mp::text(&(item).created_at)?),
                                    ("updated_at", mp::text(&(item).updated_at)?),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Created(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Created".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("connection_id", mp::text(&(_field_0).connection_id)?),
                        ("workspace_id", mp::text(&(_field_0).workspace_id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        ("transport", mp::text(&(_field_0).transport)?),
                        ("auth_type", mp::text(&(_field_0).auth_type)?),
                        (
                            "custom_header_name",
                            match (&(_field_0).custom_header_name).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "last_connected_at",
                            match (&(_field_0).last_connected_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_discovered_at",
                            match (&(_field_0).last_discovered_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_error",
                            match (&(_field_0).last_error).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
            Self::Connection(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Connection".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("connection_id", mp::text(&(_field_0).connection_id)?),
                        ("workspace_id", mp::text(&(_field_0).workspace_id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        ("transport", mp::text(&(_field_0).transport)?),
                        ("auth_type", mp::text(&(_field_0).auth_type)?),
                        (
                            "custom_header_name",
                            match (&(_field_0).custom_header_name).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "last_connected_at",
                            match (&(_field_0).last_connected_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_discovered_at",
                            match (&(_field_0).last_discovered_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "last_error",
                            match (&(_field_0).last_error).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                    ]),
                ),
            ]),
            Self::DraftTest(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("DraftTest".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("ok", serde_json::Value::Bool(*(&(_field_0).ok))),
                        (
                            "server_name",
                            match (&(_field_0).server_name).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "server_version",
                            match (&(_field_0).server_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "protocol_version",
                            match (&(_field_0).protocol_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "tested_at",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).tested_at).len()),
                            )]),
                        ),
                        (
                            "error",
                            match (&(_field_0).error).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Test(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Test".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("connection_id", mp::text(&(_field_0).connection_id)?),
                        ("ok", serde_json::Value::Bool(*(&(_field_0).ok))),
                        (
                            "server_name",
                            match (&(_field_0).server_name).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "server_version",
                            match (&(_field_0).server_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "protocol_version",
                            match (&(_field_0).protocol_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "tested_at",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).tested_at).len()),
                            )]),
                        ),
                        (
                            "error",
                            match (&(_field_0).error).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Discovery(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Discovery".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("connection_id", mp::text(&(_field_0).connection_id)?),
                        (
                            "server_name",
                            match (&(_field_0).server_name).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "server_version",
                            match (&(_field_0).server_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("protocol_version", mp::text(&(_field_0).protocol_version)?),
                        (
                            "discovered_at",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).discovered_at).len()),
                            )]),
                        ),
                        ("items", {
                            if (&(_field_0).items).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).items)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "remote_tool_name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).remote_tool_name).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "description",
                                                match (&(item).description).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "input_schema",
                                                mp::json_summary(&(item).input_schema),
                                            ),
                                            (
                                                "output_schema",
                                                mp::json_summary(&(item).output_schema),
                                            ),
                                            (
                                                "source_status",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).source_status).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "imported_tool_id",
                                                match (&(item).imported_tool_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "schema_hash",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).schema_hash).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::Imported(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Imported".to_owned())),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("workspace_id",mp::text(&(item).workspace_id)?), ("tool_id",mp::text(&(item).tool_id)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("short_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).short_description).len()))])), ("full_description",mp::object_value(&[("byte_count",serde_json::json!((&(item).full_description).len()))])), ("execution_target",match &(item).execution_target {crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::InterfaceWrapper {interface_id: _field_interface_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("InterfaceWrapper".to_owned())), ("interface_id",mp::text(_field_interface_id)?)]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::McpProxy {upstream_connection_id: _field_upstream_connection_id, remote_tool_name: _field_remote_tool_name, source_schema_hash: _field_source_schema_hash, .. } => mp::object_value(&[("variant",serde_json::Value::String("McpProxy".to_owned())), ("upstream_connection_id",mp::text(_field_upstream_connection_id)?), ("remote_tool_name",mp::object_value(&[("byte_count",serde_json::json!((_field_remote_tool_name).len()))])), ("source_schema_hash",mp::object_value(&[("byte_count",serde_json::json!((_field_source_schema_hash).len()))]))]), crate::routes::settings_group::mcp_management::dto::McpToolExecutionTargetDto::AssistantClient {capability_code: _field_capability_code, .. } => mp::object_value(&[("variant",serde_json::Value::String("AssistantClient".to_owned())), ("capability_code",mp::text(_field_capability_code)?)])}), ("operation",mp::text(&(item).operation)?), ("parameter_schema",mp::json_summary(&(item).parameter_schema)), ("result_schema",mp::json_summary(&(item).result_schema)), ("input_mapping",mp::json_summary(&(item).input_mapping)), ("output_mapping",mp::json_summary(&(item).output_mapping)), ("permission_code",match (&(item).permission_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("risk_level",mp::object_value(&[("byte_count",serde_json::json!((&(item).risk_level).len()))])), ("des_id",mp::text(&(item).des_id)?), ("des_id_required",serde_json::Value::Bool(*(&(item).des_id_required))), ("status",mp::text(&(item).status)?), ("availability_status",match &(item).availability_status {crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::Available => mp::object_value(&[("variant",serde_json::Value::String("Available".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::InterfaceMissing => mp::object_value(&[("variant",serde_json::Value::String("InterfaceMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamDisabled => mp::object_value(&[("variant",serde_json::Value::String("UpstreamDisabled".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::CredentialsMissing => mp::object_value(&[("variant",serde_json::Value::String("CredentialsMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::UpstreamToolMissing => mp::object_value(&[("variant",serde_json::Value::String("UpstreamToolMissing".to_owned()))]), crate::routes::settings_group::mcp_management::dto::McpToolAvailabilityStatusDto::MappingInvalid => mp::object_value(&[("variant",serde_json::Value::String("MappingInvalid".to_owned()))])}), ("availability_reason",match (&(item).availability_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("revision",serde_json::json!(*(&(item).revision))), ("managed_by",match (&(item).managed_by).as_ref() { Some(item) => mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("bundle_id",mp::text(&(item).bundle_id)?), ("bundle_version",mp::text(&(item).bundle_version)?)]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
                }),
            ]),
            Self::Debug(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Debug".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "local_arguments",
                            mp::json_summary(&(_field_0).local_arguments),
                        ),
                        (
                            "remote_arguments",
                            mp::json_summary(&(_field_0).remote_arguments),
                        ),
                        (
                            "upstream_result",
                            mp::json_summary(&(_field_0).upstream_result),
                        ),
                        ("mapped_result", mp::json_summary(&(_field_0).mapped_result)),
                    ]),
                ),
            ]),
            Self::NoContent => {
                mp::object_value(&[("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-mcp-upstream-output";
    const CONTRACT_VERSION: &'static str = "1";
}
