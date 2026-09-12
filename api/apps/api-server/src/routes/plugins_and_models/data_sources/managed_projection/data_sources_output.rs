use super::*;

impl InterfaceContract for DataSourcesOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("AgentFlowOptions")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("data_source_instance_id",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("capability",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema(&[(
                        "entries",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("installation_id",mp::text_schema()), ("source_code",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("protocol",mp::text_schema()), ("config_schema",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("List")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("fixed",serde_json::json!({"type":"boolean"})), ("default_data_model_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("capabilities",mp::object_schema(&[("can_update_defaults",serde_json::json!({"type":"boolean"})), ("can_create_data_model",serde_json::json!({"type":"boolean"})), ("can_validate",serde_json::json!({"type":"boolean"})), ("can_discover_resources",serde_json::json!({"type":"boolean"})), ("can_preview_resources",serde_json::json!({"type":"boolean"})), ("can_map_resources",serde_json::json!({"type":"boolean"}))])), ("backend",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Core")), ("durable_backend",mp::object_schema(&[("byte_count",mp::count_schema())]))]), mp::object_schema(&[("variant",mp::tag_schema("RuntimeExtension")), ("installation_id",mp::text_schema()), ("source_code",mp::text_schema()), ("config_json",mp::json_summary_schema()), ("catalog_refresh_status",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_last_error_message",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_refreshed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DataSource")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("status", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("fixed", serde_json::json!({"type":"boolean"})),
                        (
                            "default_data_model_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "capabilities",
                            mp::object_schema(&[
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
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("Core")),
                                    (
                                        "durable_backend",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                ]),
                                mp::object_schema(&[
                                    ("variant", mp::tag_schema("RuntimeExtension")),
                                    ("installation_id", mp::text_schema()),
                                    ("source_code", mp::text_schema()),
                                    ("config_json", mp::json_summary_schema()),
                                    (
                                        "catalog_refresh_status",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                    (
                                        "catalog_last_error_message",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                    (
                                        "catalog_refreshed_at",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                    ),
                                ]),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Validation")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "data_source",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                (
                                    "display_name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                ("enabled", serde_json::json!({"type":"boolean"})),
                                ("fixed", serde_json::json!({"type":"boolean"})),
                                (
                                    "default_data_model_status",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "capabilities",
                                    mp::object_schema(&[
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
                                        mp::object_schema(&[
                                            ("variant", mp::tag_schema("Core")),
                                            (
                                                "durable_backend",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                        mp::object_schema(&[
                                            ("variant", mp::tag_schema("RuntimeExtension")),
                                            ("installation_id", mp::text_schema()),
                                            ("source_code", mp::text_schema()),
                                            ("config_json", mp::json_summary_schema()),
                                            (
                                                "catalog_refresh_status",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "catalog_last_error_message",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                            ),
                                            (
                                                "catalog_refreshed_at",
                                                serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("Resources")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("resource_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("resource_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("capabilities",mp::object_schema(&[("supports_list",serde_json::json!({"type":"boolean"})), ("supports_get",serde_json::json!({"type":"boolean"})), ("supports_create",serde_json::json!({"type":"boolean"})), ("supports_update",serde_json::json!({"type":"boolean"})), ("supports_delete",serde_json::json!({"type":"boolean"})), ("supports_filter",serde_json::json!({"type":"boolean"})), ("supports_sort",serde_json::json!({"type":"boolean"})), ("supports_pagination",serde_json::json!({"type":"boolean"})), ("supports_owner_filter",serde_json::json!({"type":"boolean"})), ("supports_scope_filter",serde_json::json!({"type":"boolean"})), ("supports_write",serde_json::json!({"type":"boolean"})), ("supports_transactions",serde_json::json!({"type":"boolean"}))])), ("metadata",mp::json_summary_schema())])}),
                        ),
                        (
                            "refresh_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "last_error_message",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "refreshed_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Preview")),
                (
                    "0",
                    mp::object_schema(&[
                        ("expires_at", mp::text_schema()),
                        (
                            "output",
                            mp::object_schema(&[
                                (
                                    "rows",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "next_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Model")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "scope_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("scope_id", mp::text_schema()),
                        ("code", mp::text_schema()),
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "runtime_availability",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "data_source_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "source_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "external_resource_key",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "external_table_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "template_provider",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("template_code", mp::text_schema()),
                        ("template_version", mp::text_schema()),
                        (
                            "template_summary",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "physical_table_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "acl_namespace",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "audit_namespace",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "builtin_kind",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "capabilities",
                            mp::object_schema(&[
                                ("can_delete", serde_json::json!({"type":"boolean"})),
                                ("can_add_user_field", serde_json::json!({"type":"boolean"})),
                                (
                                    "can_update_lifecycle_status",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                                (
                                    "record",
                                    mp::object_schema(&[
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
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("physical_column_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("external_field_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("field_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("is_system",serde_json::json!({"type":"boolean"})), ("is_writable",serde_json::json!({"type":"boolean"})), ("is_required",serde_json::json!({"type":"boolean"})), ("api_required",serde_json::json!({"type":"boolean"})), ("is_unique",serde_json::json!({"type":"boolean"})), ("default_value",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("display_interface",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("display_options",mp::json_summary_schema()), ("relation_target_model_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("relation_options",mp::json_summary_schema()), ("sort_order",serde_json::json!({"type":"integer"})), ("capabilities",mp::object_schema(&[("ownership",mp::object_schema(&[("byte_count",mp::count_schema())])), ("can_update_presentation_metadata",serde_json::json!({"type":"boolean"})), ("can_update_physical_metadata",serde_json::json!({"type":"boolean"})), ("can_delete",serde_json::json!({"type":"boolean"}))]))])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::AgentFlowOptions(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("AgentFlowOptions".to_owned()),
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
                                    (
                                        "data_source_instance_id",
                                        mp::text(&(item).data_source_instance_id)?,
                                    ),
                                    (
                                        "display_name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).display_name).len()),
                                        )]),
                                    ),
                                    (
                                        "capability",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).capability).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Catalog(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Catalog".to_owned())),
                (
                    "0",
                    mp::object_value(&[("entries", {
                        if (&(_field_0).entries).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_0).entries)
                                .iter()
                                .map(|item| {
                                    Some(mp::object_value(&[
                                        ("installation_id", mp::text(&(item).installation_id)?),
                                        ("source_code", mp::text(&(item).source_code)?),
                                        ("plugin_id", mp::text(&(item).plugin_id)?),
                                        ("plugin_version", mp::text(&(item).plugin_version)?),
                                        (
                                            "display_name",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((&(item).display_name).len()),
                                            )]),
                                        ),
                                        ("protocol", mp::text(&(item).protocol)?),
                                        (
                                            "config_schema",
                                            mp::object_value(&[(
                                                "item_count",
                                                serde_json::json!((&(item).config_schema).len()),
                                            )]),
                                        ),
                                    ]))
                                })
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::List(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("List".to_owned())),
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
                                    (
                                        "display_name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).display_name).len()),
                                        )]),
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    ("enabled", serde_json::Value::Bool(*(&(item).enabled))),
                                    ("fixed", serde_json::Value::Bool(*(&(item).fixed))),
                                    (
                                        "default_data_model_status",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!(
                                                (&(item).default_data_model_status).len()
                                            ),
                                        )]),
                                    ),
                                    (
                                        "capabilities",
                                        mp::object_value(&[
                                            (
                                                "can_update_defaults",
                                                serde_json::Value::Bool(
                                                    *(&(&(item).capabilities).can_update_defaults),
                                                ),
                                            ),
                                            (
                                                "can_create_data_model",
                                                serde_json::Value::Bool(
                                                    *(&(&(item).capabilities)
                                                        .can_create_data_model),
                                                ),
                                            ),
                                            (
                                                "can_validate",
                                                serde_json::Value::Bool(
                                                    *(&(&(item).capabilities).can_validate),
                                                ),
                                            ),
                                            (
                                                "can_discover_resources",
                                                serde_json::Value::Bool(
                                                    *(&(&(item).capabilities)
                                                        .can_discover_resources),
                                                ),
                                            ),
                                            (
                                                "can_preview_resources",
                                                serde_json::Value::Bool(
                                                    *(&(&(item).capabilities)
                                                        .can_preview_resources),
                                                ),
                                            ),
                                            (
                                                "can_map_resources",
                                                serde_json::Value::Bool(
                                                    *(&(&(item).capabilities).can_map_resources),
                                                ),
                                            ),
                                        ]),
                                    ),
                                    (
                                        "backend",
                                        match &(item).backend {
                                            DataSourceBackendResponse::Core {
                                                durable_backend: _field_durable_backend,
                                                ..
                                            } => mp::object_value(&[
                                                (
                                                    "variant",
                                                    serde_json::Value::String("Core".to_owned()),
                                                ),
                                                (
                                                    "durable_backend",
                                                    mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!(
                                                            (_field_durable_backend).len()
                                                        ),
                                                    )]),
                                                ),
                                            ]),
                                            DataSourceBackendResponse::RuntimeExtension {
                                                installation_id: _field_installation_id,
                                                source_code: _field_source_code,
                                                config_json: _field_config_json,
                                                catalog_refresh_status:
                                                    _field_catalog_refresh_status,
                                                catalog_last_error_message:
                                                    _field_catalog_last_error_message,
                                                catalog_refreshed_at: _field_catalog_refreshed_at,
                                                ..
                                            } => mp::object_value(&[
                                                (
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "RuntimeExtension".to_owned(),
                                                    ),
                                                ),
                                                (
                                                    "installation_id",
                                                    mp::text(_field_installation_id)?,
                                                ),
                                                ("source_code", mp::text(_field_source_code)?),
                                                (
                                                    "config_json",
                                                    mp::json_summary(_field_config_json),
                                                ),
                                                (
                                                    "catalog_refresh_status",
                                                    match (_field_catalog_refresh_status).as_ref() {
                                                        Some(item) => mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((item).len()),
                                                        )]),
                                                        None => serde_json::Value::Null,
                                                    },
                                                ),
                                                (
                                                    "catalog_last_error_message",
                                                    match (_field_catalog_last_error_message)
                                                        .as_ref()
                                                    {
                                                        Some(item) => mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((item).len()),
                                                        )]),
                                                        None => serde_json::Value::Null,
                                                    },
                                                ),
                                                (
                                                    "catalog_refreshed_at",
                                                    match (_field_catalog_refreshed_at).as_ref() {
                                                        Some(item) => mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((item).len()),
                                                        )]),
                                                        None => serde_json::Value::Null,
                                                    },
                                                ),
                                            ]),
                                        },
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::DataSource(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DataSource".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        ("fixed", serde_json::Value::Bool(*(&(_field_0).fixed))),
                        (
                            "default_data_model_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).default_data_model_status).len()),
                            )]),
                        ),
                        (
                            "capabilities",
                            mp::object_value(&[
                                (
                                    "can_update_defaults",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_update_defaults),
                                    ),
                                ),
                                (
                                    "can_create_data_model",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_create_data_model),
                                    ),
                                ),
                                (
                                    "can_validate",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_validate),
                                    ),
                                ),
                                (
                                    "can_discover_resources",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_discover_resources),
                                    ),
                                ),
                                (
                                    "can_preview_resources",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_preview_resources),
                                    ),
                                ),
                                (
                                    "can_map_resources",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_map_resources),
                                    ),
                                ),
                            ]),
                        ),
                        (
                            "backend",
                            match &(_field_0).backend {
                                DataSourceBackendResponse::Core {
                                    durable_backend: _field_durable_backend,
                                    ..
                                } => mp::object_value(&[
                                    ("variant", serde_json::Value::String("Core".to_owned())),
                                    (
                                        "durable_backend",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((_field_durable_backend).len()),
                                        )]),
                                    ),
                                ]),
                                DataSourceBackendResponse::RuntimeExtension {
                                    installation_id: _field_installation_id,
                                    source_code: _field_source_code,
                                    config_json: _field_config_json,
                                    catalog_refresh_status: _field_catalog_refresh_status,
                                    catalog_last_error_message: _field_catalog_last_error_message,
                                    catalog_refreshed_at: _field_catalog_refreshed_at,
                                    ..
                                } => mp::object_value(&[
                                    (
                                        "variant",
                                        serde_json::Value::String("RuntimeExtension".to_owned()),
                                    ),
                                    ("installation_id", mp::text(_field_installation_id)?),
                                    ("source_code", mp::text(_field_source_code)?),
                                    ("config_json", mp::json_summary(_field_config_json)),
                                    (
                                        "catalog_refresh_status",
                                        match (_field_catalog_refresh_status).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "catalog_last_error_message",
                                        match (_field_catalog_last_error_message).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "catalog_refreshed_at",
                                        match (_field_catalog_refreshed_at).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                ]),
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Validation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Validation".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "data_source",
                            mp::object_value(&[
                                ("id", mp::text(&(&(_field_0).data_source).id)?),
                                (
                                    "display_name",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).data_source)
                                            .display_name)
                                            .len()),
                                    )]),
                                ),
                                ("status", mp::text(&(&(_field_0).data_source).status)?),
                                (
                                    "enabled",
                                    serde_json::Value::Bool(*(&(&(_field_0).data_source).enabled)),
                                ),
                                (
                                    "fixed",
                                    serde_json::Value::Bool(*(&(&(_field_0).data_source).fixed)),
                                ),
                                (
                                    "default_data_model_status",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).data_source)
                                            .default_data_model_status)
                                            .len()),
                                    )]),
                                ),
                                (
                                    "capabilities",
                                    mp::object_value(&[
                                        (
                                            "can_update_defaults",
                                            serde_json::Value::Bool(
                                                *(&(&(&(_field_0).data_source).capabilities)
                                                    .can_update_defaults),
                                            ),
                                        ),
                                        (
                                            "can_create_data_model",
                                            serde_json::Value::Bool(
                                                *(&(&(&(_field_0).data_source).capabilities)
                                                    .can_create_data_model),
                                            ),
                                        ),
                                        (
                                            "can_validate",
                                            serde_json::Value::Bool(
                                                *(&(&(&(_field_0).data_source).capabilities)
                                                    .can_validate),
                                            ),
                                        ),
                                        (
                                            "can_discover_resources",
                                            serde_json::Value::Bool(
                                                *(&(&(&(_field_0).data_source).capabilities)
                                                    .can_discover_resources),
                                            ),
                                        ),
                                        (
                                            "can_preview_resources",
                                            serde_json::Value::Bool(
                                                *(&(&(&(_field_0).data_source).capabilities)
                                                    .can_preview_resources),
                                            ),
                                        ),
                                        (
                                            "can_map_resources",
                                            serde_json::Value::Bool(
                                                *(&(&(&(_field_0).data_source).capabilities)
                                                    .can_map_resources),
                                            ),
                                        ),
                                    ]),
                                ),
                                (
                                    "backend",
                                    match &(&(_field_0).data_source).backend {
                                        DataSourceBackendResponse::Core {
                                            durable_backend: _field_durable_backend,
                                            ..
                                        } => mp::object_value(&[
                                            (
                                                "variant",
                                                serde_json::Value::String("Core".to_owned()),
                                            ),
                                            (
                                                "durable_backend",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (_field_durable_backend).len()
                                                    ),
                                                )]),
                                            ),
                                        ]),
                                        DataSourceBackendResponse::RuntimeExtension {
                                            installation_id: _field_installation_id,
                                            source_code: _field_source_code,
                                            config_json: _field_config_json,
                                            catalog_refresh_status: _field_catalog_refresh_status,
                                            catalog_last_error_message:
                                                _field_catalog_last_error_message,
                                            catalog_refreshed_at: _field_catalog_refreshed_at,
                                            ..
                                        } => mp::object_value(&[
                                            (
                                                "variant",
                                                serde_json::Value::String(
                                                    "RuntimeExtension".to_owned(),
                                                ),
                                            ),
                                            ("installation_id", mp::text(_field_installation_id)?),
                                            ("source_code", mp::text(_field_source_code)?),
                                            ("config_json", mp::json_summary(_field_config_json)),
                                            (
                                                "catalog_refresh_status",
                                                match (_field_catalog_refresh_status).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "catalog_last_error_message",
                                                match (_field_catalog_last_error_message).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "catalog_refreshed_at",
                                                match (_field_catalog_refreshed_at).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]),
                                    },
                                ),
                            ]),
                        ),
                        ("output", mp::json_summary(&(_field_0).output)),
                    ]),
                ),
            ]),
            Self::Resources(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Resources".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("entries", {
                            if (&(_field_0).entries).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).entries)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "resource_key",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).resource_key).len()),
                                                )]),
                                            ),
                                            (
                                                "display_name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).display_name).len()),
                                                )]),
                                            ),
                                            (
                                                "resource_kind",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).resource_kind).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "capabilities",
                                                mp::object_value(&[
                                                    (
                                                        "supports_list",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_list),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_get",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities).supports_get),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_create",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_create),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_update",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_update),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_delete",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_delete),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_filter",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_filter),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_sort",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_sort),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_pagination",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_pagination),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_owner_filter",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_owner_filter),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_scope_filter",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_scope_filter),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_write",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_write),
                                                        ),
                                                    ),
                                                    (
                                                        "supports_transactions",
                                                        serde_json::Value::Bool(
                                                            *(&(&(item).capabilities)
                                                                .supports_transactions),
                                                        ),
                                                    ),
                                                ]),
                                            ),
                                            ("metadata", mp::json_summary(&(item).metadata)),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "refresh_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).refresh_status).len()),
                            )]),
                        ),
                        (
                            "last_error_message",
                            match (&(_field_0).last_error_message).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "refreshed_at",
                            match (&(_field_0).refreshed_at).as_ref() {
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
            Self::Preview(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Preview".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("expires_at", mp::text(&(_field_0).expires_at)?),
                        (
                            "output",
                            mp::object_value(&[
                                (
                                    "rows",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!((&(&(_field_0).output).rows).len()),
                                    )]),
                                ),
                                (
                                    "next_cursor",
                                    match (&(&(_field_0).output).next_cursor).as_ref() {
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
                ),
            ]),
            Self::Model(_field_0) => {
                mp::object_value(&[
                    ("variant", serde_json::Value::String("Model".to_owned())),
                    (
                        "0",
                        mp::object_value(&[
                            ("id", mp::text(&(_field_0).id)?),
                            (
                                "scope_kind",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).scope_kind).len()),
                                )]),
                            ),
                            ("scope_id", mp::text(&(_field_0).scope_id)?),
                            ("code", mp::text(&(_field_0).code)?),
                            (
                                "title",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).title).len()),
                                )]),
                            ),
                            (
                                "description",
                                match (&(_field_0).description).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                            ("status", mp::text(&(_field_0).status)?),
                            (
                                "runtime_availability",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).runtime_availability).len()),
                                )]),
                            ),
                            (
                                "data_source_id",
                                match (&(_field_0).data_source_id).as_ref() {
                                    Some(item) => mp::text(item)?,
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "source_kind",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).source_kind).len()),
                                )]),
                            ),
                            (
                                "external_resource_key",
                                match (&(_field_0).external_resource_key).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "external_table_id",
                                match (&(_field_0).external_table_id).as_ref() {
                                    Some(item) => mp::text(item)?,
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "template_provider",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).template_provider).len()),
                                )]),
                            ),
                            ("template_code", mp::text(&(_field_0).template_code)?),
                            ("template_version", mp::text(&(_field_0).template_version)?),
                            (
                                "template_summary",
                                match (&(_field_0).template_summary).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "physical_table_name",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).physical_table_name).len()),
                                )]),
                            ),
                            (
                                "acl_namespace",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).acl_namespace).len()),
                                )]),
                            ),
                            (
                                "audit_namespace",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).audit_namespace).len()),
                                )]),
                            ),
                            (
                                "builtin_kind",
                                match (&(_field_0).builtin_kind).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                            (
                                "capabilities",
                                mp::object_value(&[
                                    (
                                        "can_delete",
                                        serde_json::Value::Bool(
                                            *(&(&(_field_0).capabilities).can_delete),
                                        ),
                                    ),
                                    (
                                        "can_add_user_field",
                                        serde_json::Value::Bool(
                                            *(&(&(_field_0).capabilities).can_add_user_field),
                                        ),
                                    ),
                                    (
                                        "can_update_lifecycle_status",
                                        serde_json::Value::Bool(
                                            *(&(&(_field_0).capabilities)
                                                .can_update_lifecycle_status),
                                        ),
                                    ),
                                    (
                                        "record",
                                        mp::object_value(&[
                                            (
                                                "can_list",
                                                serde_json::Value::Bool(
                                                    *(&(&(&(_field_0).capabilities).record)
                                                        .can_list),
                                                ),
                                            ),
                                            (
                                                "can_get",
                                                serde_json::Value::Bool(
                                                    *(&(&(&(_field_0).capabilities).record)
                                                        .can_get),
                                                ),
                                            ),
                                            (
                                                "can_create",
                                                serde_json::Value::Bool(
                                                    *(&(&(&(_field_0).capabilities).record)
                                                        .can_create),
                                                ),
                                            ),
                                            (
                                                "can_update",
                                                serde_json::Value::Bool(
                                                    *(&(&(&(_field_0).capabilities).record)
                                                        .can_update),
                                                ),
                                            ),
                                            (
                                                "can_delete",
                                                serde_json::Value::Bool(
                                                    *(&(&(&(_field_0).capabilities).record)
                                                        .can_delete),
                                                ),
                                            ),
                                        ]),
                                    ),
                                ]),
                            ),
                            ("fields", {
                                if (&(_field_0).fields).len() > 32 {
                                    return None;
                                }
                                serde_json::Value::Array((&(_field_0).fields).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("code",mp::text(&(item).code)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("physical_column_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).physical_column_name).len()))])), ("external_field_key",match (&(item).external_field_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("field_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).field_kind).len()))])), ("is_system",serde_json::Value::Bool(*(&(item).is_system))), ("is_writable",serde_json::Value::Bool(*(&(item).is_writable))), ("is_required",serde_json::Value::Bool(*(&(item).is_required))), ("api_required",serde_json::Value::Bool(*(&(item).api_required))), ("is_unique",serde_json::Value::Bool(*(&(item).is_unique))), ("default_value",match (&(item).default_value).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("display_interface",match (&(item).display_interface).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("display_options",mp::json_summary(&(item).display_options)), ("relation_target_model_id",match (&(item).relation_target_model_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("relation_options",mp::json_summary(&(item).relation_options)), ("sort_order",serde_json::json!(*(&(item).sort_order))), ("capabilities",mp::object_value(&[("ownership",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).capabilities).ownership).len()))])), ("can_update_presentation_metadata",serde_json::Value::Bool(*(&(&(item).capabilities).can_update_presentation_metadata))), ("can_update_physical_metadata",serde_json::Value::Bool(*(&(&(item).capabilities).can_update_physical_metadata))), ("can_delete",serde_json::Value::Bool(*(&(&(item).capabilities).can_delete)))]))]))).collect::<Option<Vec<_>>>()?)
                            }),
                        ]),
                    ),
                ])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-data-sources-output";
    const CONTRACT_VERSION: &'static str = "1";
}
