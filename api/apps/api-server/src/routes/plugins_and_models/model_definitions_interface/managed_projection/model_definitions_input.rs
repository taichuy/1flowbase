use super::*;

impl InterfaceContract for ModelDefinitionsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("List")),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "data_source_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "filter",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListCompatibleTemplates")),
                (
                    "0",
                    mp::object_schema(&[
                        ("data_source_id", mp::text_schema()),
                        (
                            "resource_key",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListAgentFlowOptions"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "scope_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "template_provider",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("template_code", mp::text_schema()),
                        ("template_version", mp::text_schema()),
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
                            "status",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("AdvisorFindings")),
                ("model_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListScopeGrants")),
                ("model_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                ("model_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "title",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}), {"type":"null"}]}),
                        ),
                        (
                            "status",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "external_table_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("model_id", mp::text_schema()),
                ("confirmed", serde_json::json!({"type":"boolean"})),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("BatchDelete")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "filter_by_tk",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                        (
                            "filter",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                        ("confirmed", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateField")),
                ("model_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
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
                            "external_field_key",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "field_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("is_required", serde_json::json!({"type":"boolean"})),
                        (
                            "api_required",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
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
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateField")),
                ("model_id", mp::text_schema()),
                ("field_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("is_required", serde_json::json!({"type":"boolean"})),
                        (
                            "api_required",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
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
                        ("relation_options", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteField")),
                ("model_id", mp::text_schema()),
                ("field_id", mp::text_schema()),
                ("confirmed", serde_json::json!({"type":"boolean"})),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateScopeGrant")),
                ("model_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "scope_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("scope_id", mp::text_schema()),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "permission_profile",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "confirm_unsafe_external_source_system_all",
                            serde_json::json!({"type":"boolean"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateScopeGrant")),
                ("model_id", mp::text_schema()),
                ("grant_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "enabled",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                        (
                            "permission_profile",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "confirm_unsafe_external_source_system_all",
                            serde_json::json!({"type":"boolean"}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List {
                query: _field_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("List".to_owned())),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "data_source_id",
                            match (&(_field_query).data_source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "filter",
                            match (&(_field_query).filter).as_ref() {
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
            Self::ListCompatibleTemplates(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListCompatibleTemplates".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("data_source_id", mp::text(&(_field_0).data_source_id)?),
                        (
                            "resource_key",
                            match (&(_field_0).resource_key).as_ref() {
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
            Self::ListAgentFlowOptions => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListAgentFlowOptions".to_owned()),
            )]),
            Self::Create(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Create".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "scope_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).scope_kind).len()),
                            )]),
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
                            "status",
                            match (&(_field_0).status).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::AdvisorFindings {
                model_id: _field_model_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("AdvisorFindings".to_owned()),
                ),
                ("model_id", mp::text(_field_model_id)?),
            ]),
            Self::ListScopeGrants {
                model_id: _field_model_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListScopeGrants".to_owned()),
                ),
                ("model_id", mp::text(_field_model_id)?),
            ]),
            Self::Update {
                model_id: _field_model_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Update".to_owned())),
                ("model_id", mp::text(_field_model_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "title",
                            match (&(_field_body).title).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "description",
                            match (&(_field_body).description).as_ref() {
                                Some(item) => match (item).as_ref() {
                                    Some(item) => mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((item).len()),
                                    )]),
                                    None => serde_json::Value::Null,
                                },
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "status",
                            match (&(_field_body).status).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "external_table_id",
                            match (&(_field_body).external_table_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Delete {
                model_id: _field_model_id,
                confirmed: _field_confirmed,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Delete".to_owned())),
                ("model_id", mp::text(_field_model_id)?),
                ("confirmed", serde_json::Value::Bool(*(_field_confirmed))),
            ]),
            Self::BatchDelete(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("BatchDelete".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "filter_by_tk",
                            match (&(_field_0).filter_by_tk).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "filter",
                            match (&(_field_0).filter).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "confirmed",
                            serde_json::Value::Bool(*(&(_field_0).confirmed)),
                        ),
                    ]),
                ),
            ]),
            Self::CreateField {
                model_id: _field_model_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateField".to_owned()),
                ),
                ("model_id", mp::text(_field_model_id)?),
                (
                    "body",
                    mp::object_value(&[
                        ("code", mp::text(&(_field_body).code)?),
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).title).len()),
                            )]),
                        ),
                        (
                            "description",
                            match (&(_field_body).description).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "external_field_key",
                            match (&(_field_body).external_field_key).as_ref() {
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
                                serde_json::json!((&(_field_body).field_kind).len()),
                            )]),
                        ),
                        (
                            "is_required",
                            serde_json::Value::Bool(*(&(_field_body).is_required)),
                        ),
                        (
                            "api_required",
                            match (&(_field_body).api_required).as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_unique",
                            serde_json::Value::Bool(*(&(_field_body).is_unique)),
                        ),
                        (
                            "default_value",
                            match (&(_field_body).default_value).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "display_interface",
                            match (&(_field_body).display_interface).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "display_options",
                            mp::json_summary(&(_field_body).display_options),
                        ),
                        (
                            "relation_target_model_id",
                            match (&(_field_body).relation_target_model_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "relation_options",
                            mp::json_summary(&(_field_body).relation_options),
                        ),
                    ]),
                ),
            ]),
            Self::UpdateField {
                model_id: _field_model_id,
                field_id: _field_field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateField".to_owned()),
                ),
                ("model_id", mp::text(_field_model_id)?),
                ("field_id", mp::text(_field_field_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).title).len()),
                            )]),
                        ),
                        (
                            "description",
                            match (&(_field_body).description).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_required",
                            serde_json::Value::Bool(*(&(_field_body).is_required)),
                        ),
                        (
                            "api_required",
                            match (&(_field_body).api_required).as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_unique",
                            serde_json::Value::Bool(*(&(_field_body).is_unique)),
                        ),
                        (
                            "default_value",
                            match (&(_field_body).default_value).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "display_interface",
                            match (&(_field_body).display_interface).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "display_options",
                            mp::json_summary(&(_field_body).display_options),
                        ),
                        (
                            "relation_options",
                            mp::json_summary(&(_field_body).relation_options),
                        ),
                    ]),
                ),
            ]),
            Self::DeleteField {
                model_id: _field_model_id,
                field_id: _field_field_id,
                confirmed: _field_confirmed,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteField".to_owned()),
                ),
                ("model_id", mp::text(_field_model_id)?),
                ("field_id", mp::text(_field_field_id)?),
                ("confirmed", serde_json::Value::Bool(*(_field_confirmed))),
            ]),
            Self::CreateScopeGrant {
                model_id: _field_model_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateScopeGrant".to_owned()),
                ),
                ("model_id", mp::text(_field_model_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "scope_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).scope_kind).len()),
                            )]),
                        ),
                        (
                            "scope_id",
                            serde_json::Value::String((&(_field_body).scope_id).to_string()),
                        ),
                        (
                            "enabled",
                            serde_json::Value::Bool(*(&(_field_body).enabled)),
                        ),
                        (
                            "permission_profile",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).permission_profile).len()),
                            )]),
                        ),
                        (
                            "confirm_unsafe_external_source_system_all",
                            serde_json::Value::Bool(
                                *(&(_field_body).confirm_unsafe_external_source_system_all),
                            ),
                        ),
                    ]),
                ),
            ]),
            Self::UpdateScopeGrant {
                model_id: _field_model_id,
                grant_id: _field_grant_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateScopeGrant".to_owned()),
                ),
                ("model_id", mp::text(_field_model_id)?),
                ("grant_id", mp::text(_field_grant_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "enabled",
                            match (&(_field_body).enabled).as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "permission_profile",
                            match (&(_field_body).permission_profile).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "confirm_unsafe_external_source_system_all",
                            serde_json::Value::Bool(
                                *(&(_field_body).confirm_unsafe_external_source_system_all),
                            ),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-model-definitions-input";
    const CONTRACT_VERSION: &'static str = "1";
}
