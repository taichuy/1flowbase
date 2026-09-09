use super::*;

impl InterfaceContract for DataSourcesInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("ListAgentFlowOptions"))]),
            mp::object_schema(&[("variant", mp::tag_schema("ListCatalog"))]),
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[
                        ("installation_id", mp::text_schema()),
                        ("source_code", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("config_json", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateDefaults")),
                ("data_source_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "default_data_model_status",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Validate")),
                ("data_source_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RotateSecret")),
                ("data_source_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListResources")),
                ("data_source_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DiscoverResources")),
                ("data_source_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewRead")),
                ("data_source_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "resource_key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("options_json", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("MapResourceToModel")),
                ("data_source_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "resource_key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "template_provider",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("template_code", mp::text_schema()),
                        ("template_version", mp::text_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListAgentFlowOptions => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListAgentFlowOptions".to_owned()),
            )]),
            Self::ListCatalog => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListCatalog".to_owned()),
            )]),
            Self::List => {
                mp::object_value(&[("variant", serde_json::Value::String("List".to_owned()))])
            }
            Self::Create(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Create".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("installation_id", mp::text(&(_field_0).installation_id)?),
                        ("source_code", mp::text(&(_field_0).source_code)?),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        ("config_json", mp::json_summary(&(_field_0).config_json)),
                    ]),
                ),
            ]),
            Self::UpdateDefaults {
                data_source_id: _field_data_source_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateDefaults".to_owned()),
                ),
                ("data_source_id", mp::text(_field_data_source_id)?),
                (
                    "body",
                    mp::object_value(&[(
                        "default_data_model_status",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((&(_field_body).default_data_model_status).len()),
                        )]),
                    )]),
                ),
            ]),
            Self::Validate {
                data_source_id: _field_data_source_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Validate".to_owned())),
                ("data_source_id", mp::text(_field_data_source_id)?),
            ]),
            Self::RotateSecret {
                data_source_id: _field_data_source_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RotateSecret".to_owned()),
                ),
                ("data_source_id", mp::text(_field_data_source_id)?),
            ]),
            Self::ListResources {
                data_source_id: _field_data_source_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListResources".to_owned()),
                ),
                ("data_source_id", mp::text(_field_data_source_id)?),
            ]),
            Self::DiscoverResources {
                data_source_id: _field_data_source_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DiscoverResources".to_owned()),
                ),
                ("data_source_id", mp::text(_field_data_source_id)?),
            ]),
            Self::PreviewRead {
                data_source_id: _field_data_source_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PreviewRead".to_owned()),
                ),
                ("data_source_id", mp::text(_field_data_source_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "resource_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).resource_key).len()),
                            )]),
                        ),
                        (
                            "limit",
                            match (&(_field_body).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cursor",
                            match (&(_field_body).cursor).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "options_json",
                            mp::json_summary(&(_field_body).options_json),
                        ),
                    ]),
                ),
            ]),
            Self::MapResourceToModel {
                data_source_id: _field_data_source_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("MapResourceToModel".to_owned()),
                ),
                ("data_source_id", mp::text(_field_data_source_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "resource_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).resource_key).len()),
                            )]),
                        ),
                        (
                            "template_provider",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).template_provider).len()),
                            )]),
                        ),
                        ("template_code", mp::text(&(_field_body).template_code)?),
                        (
                            "template_version",
                            mp::text(&(_field_body).template_version)?,
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-data-sources-input";
    const CONTRACT_VERSION: &'static str = "1";
}
