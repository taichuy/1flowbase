use super::*;

impl InterfaceContract for UiManagementInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListTemplates")),
                (
                    "0",
                    mp::object_schema(&[(
                        "include_archived",
                        serde_json::json!({"type":"boolean"}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateTemplate")),
                (
                    "0",
                    mp::object_schema(&[
                        ("provider_code", mp::text_schema()),
                        ("contribution_code", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "language",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Jsx"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Tsx"))]),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateTemplate")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "language",
                            mp::union_schema(vec![
                                mp::object_schema(&[("variant", mp::tag_schema("Jsx"))]),
                                mp::object_schema(&[("variant", mp::tag_schema("Tsx"))]),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PublishTemplate")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("revision", serde_json::json!({"type":"integer"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SetDefaultTemplate")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ResetDefaultTemplate")),
                (
                    "0",
                    mp::object_schema(&[
                        ("provider_code", mp::text_schema()),
                        ("contribution_code", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ArchiveTemplate")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("archived", serde_json::json!({"type":"boolean"}))]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListComponents"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetComponent")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreateComponent")),
                (
                    "0",
                    mp::object_schema(&[
                        ("component_code", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("import_code", mp::text_schema()),
                        ("source_code", mp::text_schema()),
                        (
                            "source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "group",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "upstream",
                            mp::object_schema(&[
                                (
                                    "identity",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("version", mp::text_schema()),
                            ]),
                        ),
                        ("version", mp::text_schema()),
                        (
                            "keywords",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateComponent")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "description",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("import_code", mp::text_schema()),
                        ("source_code", mp::text_schema()),
                        (
                            "source",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "group",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "upstream",
                            mp::object_schema(&[
                                (
                                    "identity",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("version", mp::text_schema()),
                            ]),
                        ),
                        ("version", mp::text_schema()),
                        (
                            "keywords",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteComponent")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("CatalogIndex"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogPage")),
                ("page", serde_json::json!({"type":"integer"})),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogSearch")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "q",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("CatalogUpdateStatus"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogDownload")),
                ("component_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogSyncGroup")),
                (
                    "source",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "group",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListTemplates(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListTemplates".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "include_archived",
                        serde_json::Value::Bool(*(&(_field_0).include_archived)),
                    )]),
                ),
            ]),
            Self::CreateTemplate(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateTemplate".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("provider_code", mp::text(&(_field_0).provider_code)?),
                        (
                            "contribution_code",
                            mp::text(&(_field_0).contribution_code)?,
                        ),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "source",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source).len()),
                            )]),
                        ),
                        (
                            "language",
                            match &(_field_0).language {
                                domain::ui_management::UiCodeTemplateLanguage::Jsx => {
                                    mp::object_value(&[(
                                        "variant",
                                        serde_json::Value::String("Jsx".to_owned()),
                                    )])
                                }
                                domain::ui_management::UiCodeTemplateLanguage::Tsx => {
                                    mp::object_value(&[(
                                        "variant",
                                        serde_json::Value::String("Tsx".to_owned()),
                                    )])
                                }
                            },
                        ),
                    ]),
                ),
            ]),
            Self::UpdateTemplate {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateTemplate".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).name).len()),
                            )]),
                        ),
                        (
                            "source",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).source).len()),
                            )]),
                        ),
                        (
                            "language",
                            match &(_field_body).language {
                                domain::ui_management::UiCodeTemplateLanguage::Jsx => {
                                    mp::object_value(&[(
                                        "variant",
                                        serde_json::Value::String("Jsx".to_owned()),
                                    )])
                                }
                                domain::ui_management::UiCodeTemplateLanguage::Tsx => {
                                    mp::object_value(&[(
                                        "variant",
                                        serde_json::Value::String("Tsx".to_owned()),
                                    )])
                                }
                            },
                        ),
                    ]),
                ),
            ]),
            Self::PublishTemplate {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PublishTemplate".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[(
                        "revision",
                        serde_json::json!(*(&(_field_body).revision)),
                    )]),
                ),
            ]),
            Self::SetDefaultTemplate { id: _field_id, .. } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("SetDefaultTemplate".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
            ]),
            Self::ResetDefaultTemplate(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ResetDefaultTemplate".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("provider_code", mp::text(&(_field_0).provider_code)?),
                        (
                            "contribution_code",
                            mp::text(&(_field_0).contribution_code)?,
                        ),
                    ]),
                ),
            ]),
            Self::ArchiveTemplate {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ArchiveTemplate".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[(
                        "archived",
                        serde_json::Value::Bool(*(&(_field_body).archived)),
                    )]),
                ),
            ]),
            Self::ListComponents => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListComponents".to_owned()),
            )]),
            Self::GetComponent { id: _field_id, .. } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetComponent".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
            ]),
            Self::CreateComponent(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreateComponent".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("component_code", mp::text(&(_field_0).component_code)?),
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).name).len()),
                            )]),
                        ),
                        (
                            "description",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).description).len()),
                            )]),
                        ),
                        ("import_code", mp::text(&(_field_0).import_code)?),
                        ("source_code", mp::text(&(_field_0).source_code)?),
                        (
                            "source",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source).len()),
                            )]),
                        ),
                        (
                            "group",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).group).len()),
                            )]),
                        ),
                        (
                            "upstream",
                            mp::object_value(&[
                                (
                                    "identity",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).upstream).identity).len()),
                                    )]),
                                ),
                                ("version", mp::text(&(&(_field_0).upstream).version)?),
                            ]),
                        ),
                        ("version", mp::text(&(_field_0).version)?),
                        (
                            "keywords",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_0).keywords).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::UpdateComponent {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdateComponent".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).name).len()),
                            )]),
                        ),
                        (
                            "description",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).description).len()),
                            )]),
                        ),
                        ("import_code", mp::text(&(_field_body).import_code)?),
                        ("source_code", mp::text(&(_field_body).source_code)?),
                        (
                            "source",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).source).len()),
                            )]),
                        ),
                        (
                            "group",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).group).len()),
                            )]),
                        ),
                        (
                            "upstream",
                            mp::object_value(&[
                                (
                                    "identity",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_body).upstream).identity).len()
                                        ),
                                    )]),
                                ),
                                ("version", mp::text(&(&(_field_body).upstream).version)?),
                            ]),
                        ),
                        ("version", mp::text(&(_field_body).version)?),
                        (
                            "keywords",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((&(_field_body).keywords).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::DeleteComponent { id: _field_id, .. } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteComponent".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
            ]),
            Self::CatalogIndex => mp::object_value(&[(
                "variant",
                serde_json::Value::String("CatalogIndex".to_owned()),
            )]),
            Self::CatalogPage {
                page: _field_page, ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogPage".to_owned()),
                ),
                ("page", serde_json::json!(*(_field_page))),
            ]),
            Self::CatalogSearch(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogSearch".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "q",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).q).len()),
                            )]),
                        ),
                        ("page", serde_json::json!(*(&(_field_0).page))),
                        ("page_size", serde_json::json!(*(&(_field_0).page_size))),
                    ]),
                ),
            ]),
            Self::CatalogUpdateStatus => mp::object_value(&[(
                "variant",
                serde_json::Value::String("CatalogUpdateStatus".to_owned()),
            )]),
            Self::CatalogDownload {
                component_code: _field_component_code,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogDownload".to_owned()),
                ),
                ("component_code", mp::text(_field_component_code)?),
            ]),
            Self::CatalogSyncGroup {
                source: _field_source,
                group: _field_group,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogSyncGroup".to_owned()),
                ),
                (
                    "source",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_source).len()))]),
                ),
                (
                    "group",
                    mp::object_value(&[("byte_count", serde_json::json!((_field_group).len()))]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-ui-management-input";
    const CONTRACT_VERSION: &'static str = "1";
}
