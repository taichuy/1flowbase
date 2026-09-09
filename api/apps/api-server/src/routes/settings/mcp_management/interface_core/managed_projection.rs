use super::*;

impl InterfaceContract for McpCoreInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetCredential")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SaveCredential")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteCredential")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListInstances"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateInstance")),
                (
                    "0",
                    mp::object_schema(&[
                        ("instance_id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description_short",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "default_entry_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "webmcp_exposure",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CopyInstance")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        ("instance_id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateInstance")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        ("instance_id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description_short",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "default_entry_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "webmcp_exposure",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteInstance")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpsertGroup")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "display_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "description_short",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("MoveGroup")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "source_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "target_parent_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteGroup")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[(
                        "path",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateBinding")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "group_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("tool_id", mp::text_schema()),
                        (
                            "display_alias",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("visible", serde_json::json!({"type":"boolean"})),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateBinding")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        (
                            "group_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "display_alias",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("visible", serde_json::json!({"type":"boolean"})),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteBinding")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetDiscoveryPolicy")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateDiscoveryPolicy")),
                (
                    "0",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "1",
                    mp::object_schema(&[
                        ("list_default_limit", serde_json::json!({"type":"integer"})),
                        ("list_max_depth", serde_json::json!({"type":"integer"})),
                        ("list_regex_enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "list_regex_max_length",
                            serde_json::json!({"type":"integer"}),
                        ),
                        ("list_return_fields", mp::json_summary_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::GetCredential(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetCredential".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
            ]),
            Self::SaveCredential(_field_0, _) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("SaveCredential".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
            ]),
            Self::DeleteCredential(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteCredential".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
            ]),
            Self::ListInstances => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListInstances".to_owned()),
            )]),
            Self::CreateInstance(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateInstance".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("instance_id", mp::text(&(_field_0).instance_id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "description_short",
                            match (&(_field_0).description_short).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "default_entry_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).default_entry_path).len()),
                            )]),
                        ),
                        (
                            "webmcp_exposure",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).webmcp_exposure).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::CopyInstance(_field_0, _field_1) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CopyInstance".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[
                        ("instance_id", mp::text(&(_field_1).instance_id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).name).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::UpdateInstance(_field_0, _field_1) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateInstance".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[
                        ("instance_id", mp::text(&(_field_1).instance_id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).name).len()),
                            )]),
                        ),
                        (
                            "description_short",
                            match (&(_field_1).description_short).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("status", mp::text(&(_field_1).status)?),
                        (
                            "default_entry_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).default_entry_path).len()),
                            )]),
                        ),
                        (
                            "webmcp_exposure",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).webmcp_exposure).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::DeleteInstance(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteInstance".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
            ]),
            Self::UpsertGroup(_field_0, _field_1) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpsertGroup".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[
                        (
                            "path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).path).len()),
                            )]),
                        ),
                        (
                            "display_name",
                            match (&(_field_1).display_name).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "description_short",
                            match (&(_field_1).description_short).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("enabled", serde_json::Value::Bool(*(&(_field_1).enabled))),
                        ("sort_order", serde_json::json!(*(&(_field_1).sort_order))),
                    ]),
                ),
            ]),
            Self::MoveGroup(_field_0, _field_1) => mp::object_value(&[
                ("variant", serde_json::Value::String("MoveGroup".to_owned())),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[
                        (
                            "source_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).source_path).len()),
                            )]),
                        ),
                        (
                            "target_parent_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).target_parent_path).len()),
                            )]),
                        ),
                        ("sort_order", serde_json::json!(*(&(_field_1).sort_order))),
                    ]),
                ),
            ]),
            Self::DeleteGroup(_field_0, _field_1) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteGroup".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[(
                        "path",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((&(_field_1).path).len()),
                        )]),
                    )]),
                ),
            ]),
            Self::CreateBinding(_field_0, _field_1) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateBinding".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[
                        (
                            "group_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).group_path).len()),
                            )]),
                        ),
                        ("tool_id", mp::text(&(_field_1).tool_id)?),
                        (
                            "display_alias",
                            match (&(_field_1).display_alias).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("visible", serde_json::Value::Bool(*(&(_field_1).visible))),
                        ("sort_order", serde_json::json!(*(&(_field_1).sort_order))),
                    ]),
                ),
            ]),
            Self::UpdateBinding(_field_0, _field_1) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateBinding".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[
                        (
                            "group_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_1).group_path).len()),
                            )]),
                        ),
                        (
                            "display_alias",
                            match (&(_field_1).display_alias).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("visible", serde_json::Value::Bool(*(&(_field_1).visible))),
                        ("sort_order", serde_json::json!(*(&(_field_1).sort_order))),
                    ]),
                ),
            ]),
            Self::DeleteBinding(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteBinding".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
            ]),
            Self::GetDiscoveryPolicy(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetDiscoveryPolicy".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
            ]),
            Self::UpdateDiscoveryPolicy(_field_0, _field_1) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateDiscoveryPolicy".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_0).len()))]),
                ),
                (
                    "1",
                    mp::object_value(&[
                        (
                            "list_default_limit",
                            serde_json::json!(*(&(_field_1).list_default_limit)),
                        ),
                        (
                            "list_max_depth",
                            serde_json::json!(*(&(_field_1).list_max_depth)),
                        ),
                        (
                            "list_regex_enabled",
                            serde_json::Value::Bool(*(&(_field_1).list_regex_enabled)),
                        ),
                        (
                            "list_regex_max_length",
                            serde_json::json!(*(&(_field_1).list_regex_max_length)),
                        ),
                        (
                            "list_return_fields",
                            mp::json_summary(&(_field_1).list_return_fields),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-mcp-core-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for McpCoreOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Credential")),
                (
                    "0",
                    mp::object_schema(&[("saved", serde_json::json!({"type":"boolean"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Instances")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("instance_id",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_short",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("default_entry_path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("webmcp_exposure",mp::object_schema(&[("byte_count",mp::count_schema())])), ("managed_by",serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]})), ("created_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("updated_by",mp::object_schema(&[("byte_count",mp::count_schema())])), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema()), ("llm_tool_registration",mp::object_schema(&[("prefix",mp::object_schema(&[("byte_count",mp::count_schema())])), ("tools",mp::object_schema(&[("item_count",mp::count_schema())]))]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Instance")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("instance_id", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description_short",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "default_entry_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "webmcp_exposure",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "managed_by",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("bundle_version",mp::text_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "created_by",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "updated_by",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                        (
                            "llm_tool_registration",
                            mp::object_schema(&[
                                (
                                    "prefix",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "tools",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Group")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("instance_record_id", mp::text_schema()),
                        (
                            "path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description_short",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Binding")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("instance_record_id", mp::text_schema()),
                        ("tool_record_id", mp::text_schema()),
                        (
                            "group_path",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("tool_id", mp::text_schema()),
                        (
                            "display_alias",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("visible", serde_json::json!({"type":"boolean"})),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DiscoveryPolicy")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("instance_record_id", mp::text_schema()),
                        ("instance_id", mp::text_schema()),
                        ("list_default_limit", serde_json::json!({"type":"integer"})),
                        ("list_max_depth", serde_json::json!({"type":"integer"})),
                        ("list_regex_enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "list_regex_max_length",
                            serde_json::json!({"type":"integer"}),
                        ),
                        ("list_return_fields", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Credential(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Credential".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("saved", serde_json::Value::Bool(*(&(_field_0).saved)))]),
                ),
            ]),
            Self::Instances(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Instances".to_owned())),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("id", mp::text(&(item).id)?),
                                    ("workspace_id", mp::text(&(item).workspace_id)?),
                                    ("instance_id", mp::text(&(item).instance_id)?),
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                    (
                                        "description_short",
                                        match (&(item).description_short).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    (
                                        "default_entry_path",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).default_entry_path).len()),
                                        )]),
                                    ),
                                    (
                                        "webmcp_exposure",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).webmcp_exposure).len()),
                                        )]),
                                    ),
                                    (
                                        "managed_by",
                                        match (&(item).managed_by).as_ref() {
                                            Some(item) => mp::object_value(&[
                                                (
                                                    "organization",
                                                    mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!(
                                                            (&(item).organization).len()
                                                        ),
                                                    )]),
                                                ),
                                                ("bundle_id", mp::text(&(item).bundle_id)?),
                                                (
                                                    "bundle_version",
                                                    mp::text(&(item).bundle_version)?,
                                                ),
                                            ]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "created_by",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).created_by).len()),
                                        )]),
                                    ),
                                    (
                                        "updated_by",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).updated_by).len()),
                                        )]),
                                    ),
                                    ("created_at", mp::text(&(item).created_at)?),
                                    ("updated_at", mp::text(&(item).updated_at)?),
                                    (
                                        "llm_tool_registration",
                                        mp::object_value(&[
                                            (
                                                "prefix",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(&(item).llm_tool_registration).prefix)
                                                            .len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "tools",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!(
                                                        (&(&(item).llm_tool_registration).tools)
                                                            .len()
                                                    ),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Instance(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Instance".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        ("workspace_id", mp::text(&(_field_0).workspace_id)?),
                        ("instance_id", mp::text(&(_field_0).instance_id)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "description_short",
                            match (&(_field_0).description_short).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "default_entry_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).default_entry_path).len()),
                            )]),
                        ),
                        (
                            "webmcp_exposure",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).webmcp_exposure).len()),
                            )]),
                        ),
                        (
                            "managed_by",
                            match (&(_field_0).managed_by).as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "organization",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).organization).len()),
                                        )]),
                                    ),
                                    ("bundle_id", mp::text(&(item).bundle_id)?),
                                    ("bundle_version", mp::text(&(item).bundle_version)?),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "created_by",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).created_by).len()),
                            )]),
                        ),
                        (
                            "updated_by",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).updated_by).len()),
                            )]),
                        ),
                        ("created_at", mp::text(&(_field_0).created_at)?),
                        ("updated_at", mp::text(&(_field_0).updated_at)?),
                        (
                            "llm_tool_registration",
                            mp::object_value(&[
                                (
                                    "prefix",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).llm_tool_registration).prefix).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "tools",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).llm_tool_registration).tools).len()
                                        ),
                                    )]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            Self::Group(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Group".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "instance_record_id",
                            mp::text(&(_field_0).instance_record_id)?,
                        ),
                        (
                            "path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).path).len()),
                            )]),
                        ),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        (
                            "description_short",
                            match (&(_field_0).description_short).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        ("sort_order", serde_json::json!(*(&(_field_0).sort_order))),
                    ]),
                ),
            ]),
            Self::Binding(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Binding".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "instance_record_id",
                            mp::text(&(_field_0).instance_record_id)?,
                        ),
                        ("tool_record_id", mp::text(&(_field_0).tool_record_id)?),
                        (
                            "group_path",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).group_path).len()),
                            )]),
                        ),
                        ("tool_id", mp::text(&(_field_0).tool_id)?),
                        (
                            "display_alias",
                            match (&(_field_0).display_alias).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("visible", serde_json::Value::Bool(*(&(_field_0).visible))),
                        ("sort_order", serde_json::json!(*(&(_field_0).sort_order))),
                    ]),
                ),
            ]),
            Self::DiscoveryPolicy(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DiscoveryPolicy".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        ("workspace_id", mp::text(&(_field_0).workspace_id)?),
                        (
                            "instance_record_id",
                            mp::text(&(_field_0).instance_record_id)?,
                        ),
                        ("instance_id", mp::text(&(_field_0).instance_id)?),
                        (
                            "list_default_limit",
                            serde_json::json!(*(&(_field_0).list_default_limit)),
                        ),
                        (
                            "list_max_depth",
                            serde_json::json!(*(&(_field_0).list_max_depth)),
                        ),
                        (
                            "list_regex_enabled",
                            serde_json::Value::Bool(*(&(_field_0).list_regex_enabled)),
                        ),
                        (
                            "list_regex_max_length",
                            serde_json::json!(*(&(_field_0).list_regex_max_length)),
                        ),
                        (
                            "list_return_fields",
                            mp::json_summary(&(_field_0).list_return_fields),
                        ),
                    ]),
                ),
            ]),
            Self::NoContent => {
                mp::object_value(&[("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-mcp-core-output";
    const CONTRACT_VERSION: &'static str = "1";
}
