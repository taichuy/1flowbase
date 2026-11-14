use super::*;

impl DataSourcesOutput {
    pub(super) fn managed_projection_schema_impl() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("AgentFlowOptions")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("data_source_instance_id",mp::text_schema()), ("display_name",mp::object_schema([("byte_count",mp::count_schema())])), ("capability",mp::object_schema([("byte_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema([(
                        "entries",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("installation_id",mp::text_schema()), ("source_code",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("display_name",mp::object_schema([("byte_count",mp::count_schema())])), ("protocol",mp::text_schema()), ("config_schema",mp::object_schema([("item_count",mp::count_schema())]))])}),
                    )]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("display_name",mp::object_schema([("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("fixed",serde_json::json!({"type":"boolean"})), ("default_data_model_status",mp::object_schema([("byte_count",mp::count_schema())])), ("capabilities",mp::object_schema([("can_update_defaults",serde_json::json!({"type":"boolean"})), ("can_create_data_model",serde_json::json!({"type":"boolean"})), ("can_validate",serde_json::json!({"type":"boolean"})), ("can_discover_resources",serde_json::json!({"type":"boolean"})), ("can_preview_resources",serde_json::json!({"type":"boolean"})), ("can_map_resources",serde_json::json!({"type":"boolean"}))])), ("backend",mp::union_schema(vec![mp::object_schema([("variant",mp::tag_schema("Core")), ("durable_backend",mp::object_schema([("byte_count",mp::count_schema())]))]), mp::object_schema([("variant",mp::tag_schema("RuntimeExtension")), ("installation_id",mp::text_schema()), ("source_code",mp::text_schema()), ("config_json",mp::json_summary_schema()), ("catalog_refresh_status",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_last_error_message",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_refreshed_at",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}))])]))])}),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("DataSource")),
                (
                    "0",
                    mp::object_schema([
                        ("id", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        ("status", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("fixed", serde_json::json!({"type":"boolean"})),
                        (
                            "default_data_model_status",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "capabilities",
                            mp::object_schema([
                                ("can_update_defaults", serde_json::json!({"type":"boolean"})),
                                (
                                    "can_create_data_model",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                                ("can_validate", serde_json::json!({"type":"boolean"})),
                                (
                                    "can_discover_resources",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                                (
                                    "can_preview_resources",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                                ("can_map_resources", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                        (
                            "backend",
                            mp::union_schema(vec![
                                mp::object_schema([
                                    ("variant", mp::tag_schema("Core")),
                                    (
                                        "durable_backend",
                                        mp::object_schema([("byte_count", mp::count_schema())]),
                                    ),
                                ]),
                                mp::object_schema([
                                    ("variant", mp::tag_schema("RuntimeExtension")),
                                    ("installation_id", mp::text_schema()),
                                    ("source_code", mp::text_schema()),
                                    ("config_json", mp::json_summary_schema()),
                                    (
                                        "catalog_refresh_status",
                                        serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                    (
                                        "catalog_last_error_message",
                                        serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                    (
                                        "catalog_refreshed_at",
                                        serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                ]),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Validation")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "data_source",
                            mp::object_schema([
                                ("id", mp::text_schema()),
                                (
                                    "display_name",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                ("enabled", serde_json::json!({"type":"boolean"})),
                                ("fixed", serde_json::json!({"type":"boolean"})),
                                (
                                    "default_data_model_status",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "capabilities",
                                    mp::object_schema([
                                        (
                                            "can_update_defaults",
                                            serde_json::json!({"type":"boolean"}),
                                        ),
                                        (
                                            "can_create_data_model",
                                            serde_json::json!({"type":"boolean"}),
                                        ),
                                        ("can_validate", serde_json::json!({"type":"boolean"})),
                                        (
                                            "can_discover_resources",
                                            serde_json::json!({"type":"boolean"}),
                                        ),
                                        (
                                            "can_preview_resources",
                                            serde_json::json!({"type":"boolean"}),
                                        ),
                                        (
                                            "can_map_resources",
                                            serde_json::json!({"type":"boolean"}),
                                        ),
                                    ]),
                                ),
                                (
                                    "backend",
                                    mp::union_schema(vec![
                                        mp::object_schema([
                                            ("variant", mp::tag_schema("Core")),
                                            (
                                                "durable_backend",
                                                mp::object_schema([(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                        mp::object_schema([
                                            ("variant", mp::tag_schema("RuntimeExtension")),
                                            ("installation_id", mp::text_schema()),
                                            ("source_code", mp::text_schema()),
                                            ("config_json", mp::json_summary_schema()),
                                            (
                                                "catalog_refresh_status",
                                                serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "catalog_last_error_message",
                                                serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "catalog_refreshed_at",
                                                serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                        ]),
                                    ]),
                                ),
                            ]),
                        ),
                        ("output", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Resources")),
                (
                    "0",
                    mp::object_schema([
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("resource_key",mp::object_schema([("byte_count",mp::count_schema())])), ("display_name",mp::object_schema([("byte_count",mp::count_schema())])), ("resource_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("capabilities",mp::object_schema([("supports_list",serde_json::json!({"type":"boolean"})), ("supports_get",serde_json::json!({"type":"boolean"})), ("supports_create",serde_json::json!({"type":"boolean"})), ("supports_update",serde_json::json!({"type":"boolean"})), ("supports_delete",serde_json::json!({"type":"boolean"})), ("supports_filter",serde_json::json!({"type":"boolean"})), ("supports_sort",serde_json::json!({"type":"boolean"})), ("supports_pagination",serde_json::json!({"type":"boolean"})), ("supports_owner_filter",serde_json::json!({"type":"boolean"})), ("supports_scope_filter",serde_json::json!({"type":"boolean"})), ("supports_write",serde_json::json!({"type":"boolean"})), ("supports_transactions",serde_json::json!({"type":"boolean"}))])), ("metadata",mp::json_summary_schema())])}),
                        ),
                        (
                            "refresh_status",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "last_error_message",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "refreshed_at",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Preview")),
                (
                    "0",
                    mp::object_schema([
                        ("expires_at", mp::text_schema()),
                        (
                            "output",
                            mp::object_schema([
                                (
                                    "rows",
                                    mp::object_schema([("item_count", mp::count_schema())]),
                                ),
                                (
                                    "next_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("Model")),
                (
                    "0",
                    mp::object_schema([
                        ("id", mp::text_schema()),
                        (
                            "scope_kind",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        ("scope_id", mp::text_schema()),
                        ("code", mp::text_schema()),
                        (
                            "title",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "runtime_availability",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "data_source_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "source_kind",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "external_resource_key",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "external_table_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "template_provider",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        ("template_code", mp::text_schema()),
                        ("template_version", mp::text_schema()),
                        (
                            "template_summary",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "physical_table_name",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "acl_namespace",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "audit_namespace",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "builtin_kind",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "capabilities",
                            mp::object_schema([
                                ("can_delete", serde_json::json!({"type":"boolean"})),
                                ("can_add_user_field", serde_json::json!({"type":"boolean"})),
                                (
                                    "can_update_lifecycle_status",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                                (
                                    "record",
                                    mp::object_schema([
                                        ("can_list", serde_json::json!({"type":"boolean"})),
                                        ("can_get", serde_json::json!({"type":"boolean"})),
                                        ("can_create", serde_json::json!({"type":"boolean"})),
                                        ("can_update", serde_json::json!({"type":"boolean"})),
                                        ("can_delete", serde_json::json!({"type":"boolean"})),
                                    ]),
                                ),
                            ]),
                        ),
                        (
                            "fields",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("id",mp::text_schema()), ("code",mp::text_schema()), ("title",mp::object_schema([("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("physical_column_name",mp::object_schema([("byte_count",mp::count_schema())])), ("external_field_key",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("field_kind",mp::object_schema([("byte_count",mp::count_schema())])), ("is_system",serde_json::json!({"type":"boolean"})), ("is_writable",serde_json::json!({"type":"boolean"})), ("is_required",serde_json::json!({"type":"boolean"})), ("api_required",serde_json::json!({"type":"boolean"})), ("is_unique",serde_json::json!({"type":"boolean"})), ("default_value",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("display_interface",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("display_options",mp::json_summary_schema()), ("relation_target_model_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("relation_options",mp::json_summary_schema()), ("sort_order",serde_json::json!({"type":"integer"})), ("capabilities",mp::object_schema([("ownership",mp::object_schema([("byte_count",mp::count_schema())])), ("can_update_presentation_metadata",serde_json::json!({"type":"boolean"})), ("can_update_physical_metadata",serde_json::json!({"type":"boolean"})), ("can_delete",serde_json::json!({"type":"boolean"}))]))])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
}
