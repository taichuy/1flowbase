use super::*;

impl InterfaceContract for ModelDefinitionsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Models")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("scope_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("scope_id",mp::text_schema()), ("code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("runtime_availability",mp::object_schema(&[("byte_count",mp::count_schema())])), ("data_source_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("external_resource_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("external_table_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("template_provider",mp::object_schema(&[("byte_count",mp::count_schema())])), ("template_code",mp::text_schema()), ("template_version",mp::text_schema()), ("template_summary",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("physical_table_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acl_namespace",mp::object_schema(&[("byte_count",mp::count_schema())])), ("audit_namespace",mp::object_schema(&[("byte_count",mp::count_schema())])), ("builtin_kind",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("capabilities",mp::object_schema(&[("can_delete",serde_json::json!({"type":"boolean"})), ("can_add_user_field",serde_json::json!({"type":"boolean"})), ("can_update_lifecycle_status",serde_json::json!({"type":"boolean"})), ("record",mp::object_schema(&[("can_list",serde_json::json!({"type":"boolean"})), ("can_get",serde_json::json!({"type":"boolean"})), ("can_create",serde_json::json!({"type":"boolean"})), ("can_update",serde_json::json!({"type":"boolean"})), ("can_delete",serde_json::json!({"type":"boolean"}))]))])), ("fields",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("physical_column_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("external_field_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("field_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("is_system",serde_json::json!({"type":"boolean"})), ("is_writable",serde_json::json!({"type":"boolean"})), ("is_required",serde_json::json!({"type":"boolean"})), ("api_required",serde_json::json!({"type":"boolean"})), ("is_unique",serde_json::json!({"type":"boolean"})), ("default_value",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("display_interface",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("display_options",mp::json_summary_schema()), ("relation_target_model_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("relation_options",mp::json_summary_schema()), ("sort_order",serde_json::json!({"type":"integer"})), ("capabilities",mp::object_schema(&[("ownership",mp::object_schema(&[("byte_count",mp::count_schema())])), ("can_update_presentation_metadata",serde_json::json!({"type":"boolean"})), ("can_update_physical_metadata",serde_json::json!({"type":"boolean"})), ("can_delete",serde_json::json!({"type":"boolean"}))]))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Templates")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("template_provider",mp::object_schema(&[("byte_count",mp::count_schema())])), ("template_code",mp::text_schema()), ("template_version",mp::text_schema()), ("summary",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("system_fields",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("summary",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("field_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("required",serde_json::json!({"type":"boolean"}))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("AgentFlowOptions")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("value",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("state",mp::text_schema()), ("disabled",serde_json::json!({"type":"boolean"})), ("disabled_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model_id",mp::text_schema()), ("model_code",mp::text_schema()), ("fields",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("value_type",mp::text_schema()), ("required",serde_json::json!({"type":"boolean"})), ("writable",serde_json::json!({"type":"boolean"}))])}))])}),
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("AdvisorFindings")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("data_model_id",mp::text_schema()), ("severity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("recommended_action",mp::object_schema(&[("byte_count",mp::count_schema())])), ("can_acknowledge",serde_json::json!({"type":"boolean"}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ScopeGrants")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("scope_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("scope_id",mp::text_schema()), ("data_model_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("permission_profile",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ScopeGrant")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "scope_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("scope_id", mp::text_schema()),
                        ("data_model_id", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "permission_profile",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Field")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("code", mp::text_schema()),
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "physical_column_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "external_field_key",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "field_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("is_system", serde_json::json!({"type":"boolean"})),
                        ("is_writable", serde_json::json!({"type":"boolean"})),
                        ("is_required", serde_json::json!({"type":"boolean"})),
                        ("api_required", serde_json::json!({"type":"boolean"})),
                        ("is_unique", serde_json::json!({"type":"boolean"})),
                        (
                            "default_value",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                        (
                            "display_interface",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("display_options", mp::json_summary_schema()),
                        (
                            "relation_target_model_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("relation_options", mp::json_summary_schema()),
                        ("sort_order", serde_json::json!({"type":"integer"})),
                        (
                            "capabilities",
                            mp::object_schema(&[
                                (
                                    "ownership",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "can_update_presentation_metadata",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                                (
                                    "can_update_physical_metadata",
                                    serde_json::json!({"type":"boolean"}),
                                ),
                                ("can_delete", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Deleted"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("BatchDeleted")),
                (
                    "0",
                    mp::object_schema(&[
                        ("deleted", serde_json::json!({"type":"boolean"})),
                        ("deleted_count", serde_json::json!({"type":"integer"})),
                        (
                            "deleted_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Models(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Models".to_owned())),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("scope_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).scope_kind).len()))])), ("scope_id",mp::text(&(item).scope_id)?), ("code",mp::text(&(item).code)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?), ("runtime_availability",mp::object_value(&[("byte_count",serde_json::json!((&(item).runtime_availability).len()))])), ("data_source_id",match (&(item).data_source_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).source_kind).len()))])), ("external_resource_key",match (&(item).external_resource_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("external_table_id",match (&(item).external_table_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("template_provider",mp::object_value(&[("byte_count",serde_json::json!((&(item).template_provider).len()))])), ("template_code",mp::text(&(item).template_code)?), ("template_version",mp::text(&(item).template_version)?), ("template_summary",match (&(item).template_summary).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("physical_table_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).physical_table_name).len()))])), ("acl_namespace",mp::object_value(&[("byte_count",serde_json::json!((&(item).acl_namespace).len()))])), ("audit_namespace",mp::object_value(&[("byte_count",serde_json::json!((&(item).audit_namespace).len()))])), ("builtin_kind",match (&(item).builtin_kind).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("capabilities",mp::object_value(&[("can_delete",serde_json::Value::Bool(*(&(&(item).capabilities).can_delete))), ("can_add_user_field",serde_json::Value::Bool(*(&(&(item).capabilities).can_add_user_field))), ("can_update_lifecycle_status",serde_json::Value::Bool(*(&(&(item).capabilities).can_update_lifecycle_status))), ("record",mp::object_value(&[("can_list",serde_json::Value::Bool(*(&(&(&(item).capabilities).record).can_list))), ("can_get",serde_json::Value::Bool(*(&(&(&(item).capabilities).record).can_get))), ("can_create",serde_json::Value::Bool(*(&(&(&(item).capabilities).record).can_create))), ("can_update",serde_json::Value::Bool(*(&(&(&(item).capabilities).record).can_update))), ("can_delete",serde_json::Value::Bool(*(&(&(&(item).capabilities).record).can_delete)))]))])), ("fields",{ if (&(item).fields).len() > 32 { return None; } serde_json::Value::Array((&(item).fields).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("code",mp::text(&(item).code)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("physical_column_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).physical_column_name).len()))])), ("external_field_key",match (&(item).external_field_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("field_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).field_kind).len()))])), ("is_system",serde_json::Value::Bool(*(&(item).is_system))), ("is_writable",serde_json::Value::Bool(*(&(item).is_writable))), ("is_required",serde_json::Value::Bool(*(&(item).is_required))), ("api_required",serde_json::Value::Bool(*(&(item).api_required))), ("is_unique",serde_json::Value::Bool(*(&(item).is_unique))), ("default_value",match (&(item).default_value).as_ref() { Some(item) => mp::json_summary(item), None => serde_json::Value::Null }), ("display_interface",match (&(item).display_interface).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("display_options",mp::json_summary(&(item).display_options)), ("relation_target_model_id",match (&(item).relation_target_model_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("relation_options",mp::json_summary(&(item).relation_options)), ("sort_order",serde_json::json!(*(&(item).sort_order))), ("capabilities",mp::object_value(&[("ownership",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).capabilities).ownership).len()))])), ("can_update_presentation_metadata",serde_json::Value::Bool(*(&(&(item).capabilities).can_update_presentation_metadata))), ("can_update_physical_metadata",serde_json::Value::Bool(*(&(&(item).capabilities).can_update_physical_metadata))), ("can_delete",serde_json::Value::Bool(*(&(&(item).capabilities).can_delete)))]))]))).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?)
                }),
            ]),
            Self::Templates(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Templates".to_owned())),
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
                                        "template_provider",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).template_provider).len()),
                                        )]),
                                    ),
                                    ("template_code", mp::text(&(item).template_code)?),
                                    ("template_version", mp::text(&(item).template_version)?),
                                    (
                                        "summary",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).summary).len()),
                                        )]),
                                    ),
                                    (
                                        "description",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).description).len()),
                                        )]),
                                    ),
                                    ("system_fields", {
                                        if (&(item).system_fields).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).system_fields)
                                                .iter()
                                                .map(|item| {
                                                    Some(mp::object_value(&[
                                                        ("code", mp::text(&(item).code)?),
                                                        (
                                                            "summary",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!((&(item)
                                                                    .summary)
                                                                    .len()),
                                                            )]),
                                                        ),
                                                        (
                                                            "description",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!((&(item)
                                                                    .description)
                                                                    .len()),
                                                            )]),
                                                        ),
                                                        (
                                                            "field_kind",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!((&(item)
                                                                    .field_kind)
                                                                    .len()),
                                                            )]),
                                                        ),
                                                        (
                                                            "required",
                                                            serde_json::Value::Bool(
                                                                *(&(item).required),
                                                            ),
                                                        ),
                                                    ]))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
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
                                        "value",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).value).len()),
                                        )]),
                                    ),
                                    (
                                        "label",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).label).len()),
                                        )]),
                                    ),
                                    ("state", mp::text(&(item).state)?),
                                    ("disabled", serde_json::Value::Bool(*(&(item).disabled))),
                                    (
                                        "disabled_reason",
                                        match (&(item).disabled_reason).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("model_id", mp::text(&(item).model_id)?),
                                    ("model_code", mp::text(&(item).model_code)?),
                                    ("fields", {
                                        if (&(item).fields).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).fields)
                                                .iter()
                                                .map(|item| {
                                                    Some(mp::object_value(&[
                                                        ("code", mp::text(&(item).code)?),
                                                        (
                                                            "title",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(item).title).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "value_type",
                                                            mp::text(&(item).value_type)?,
                                                        ),
                                                        (
                                                            "required",
                                                            serde_json::Value::Bool(
                                                                *(&(item).required),
                                                            ),
                                                        ),
                                                        (
                                                            "writable",
                                                            serde_json::Value::Bool(
                                                                *(&(item).writable),
                                                            ),
                                                        ),
                                                    ]))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
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
            Self::AdvisorFindings(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("AdvisorFindings".to_owned()),
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
                                    ("id", mp::text(&(item).id)?),
                                    ("data_model_id", mp::text(&(item).data_model_id)?),
                                    (
                                        "severity",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).severity).len()),
                                        )]),
                                    ),
                                    ("code", mp::text(&(item).code)?),
                                    (
                                        "message",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).message).len()),
                                        )]),
                                    ),
                                    (
                                        "recommended_action",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).recommended_action).len()),
                                        )]),
                                    ),
                                    (
                                        "can_acknowledge",
                                        serde_json::Value::Bool(*(&(item).can_acknowledge)),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::ScopeGrants(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ScopeGrants".to_owned()),
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
                                    ("id", mp::text(&(item).id)?),
                                    (
                                        "scope_kind",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).scope_kind).len()),
                                        )]),
                                    ),
                                    ("scope_id", mp::text(&(item).scope_id)?),
                                    ("data_model_id", mp::text(&(item).data_model_id)?),
                                    ("enabled", serde_json::Value::Bool(*(&(item).enabled))),
                                    (
                                        "permission_profile",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).permission_profile).len()),
                                        )]),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::ScopeGrant(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ScopeGrant".to_owned()),
                ),
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
                        ("data_model_id", mp::text(&(_field_0).data_model_id)?),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        (
                            "permission_profile",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).permission_profile).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Field(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Field".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
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
                        (
                            "physical_column_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).physical_column_name).len()),
                            )]),
                        ),
                        (
                            "external_field_key",
                            match (&(_field_0).external_field_key).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "field_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).field_kind).len()),
                            )]),
                        ),
                        (
                            "is_system",
                            serde_json::Value::Bool(*(&(_field_0).is_system)),
                        ),
                        (
                            "is_writable",
                            serde_json::Value::Bool(*(&(_field_0).is_writable)),
                        ),
                        (
                            "is_required",
                            serde_json::Value::Bool(*(&(_field_0).is_required)),
                        ),
                        (
                            "api_required",
                            serde_json::Value::Bool(*(&(_field_0).api_required)),
                        ),
                        (
                            "is_unique",
                            serde_json::Value::Bool(*(&(_field_0).is_unique)),
                        ),
                        (
                            "default_value",
                            match (&(_field_0).default_value).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "display_interface",
                            match (&(_field_0).display_interface).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "display_options",
                            mp::json_summary(&(_field_0).display_options),
                        ),
                        (
                            "relation_target_model_id",
                            match (&(_field_0).relation_target_model_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "relation_options",
                            mp::json_summary(&(_field_0).relation_options),
                        ),
                        ("sort_order", serde_json::json!(*(&(_field_0).sort_order))),
                        (
                            "capabilities",
                            mp::object_value(&[
                                (
                                    "ownership",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).capabilities).ownership).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "can_update_presentation_metadata",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities)
                                            .can_update_presentation_metadata),
                                    ),
                                ),
                                (
                                    "can_update_physical_metadata",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_update_physical_metadata),
                                    ),
                                ),
                                (
                                    "can_delete",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).capabilities).can_delete),
                                    ),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            Self::Deleted => {
                mp::object_value(&[("variant", serde_json::Value::String("Deleted".to_owned()))])
            }
            Self::BatchDeleted(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("BatchDeleted".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("deleted", serde_json::Value::Bool(*(&(_field_0).deleted))),
                        (
                            "deleted_count",
                            serde_json::json!(*(&(_field_0).deleted_count)),
                        ),
                        ("deleted_ids", {
                            if (&(_field_0).deleted_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).deleted_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-model-definitions-output";
    const CONTRACT_VERSION: &'static str = "1";
}
