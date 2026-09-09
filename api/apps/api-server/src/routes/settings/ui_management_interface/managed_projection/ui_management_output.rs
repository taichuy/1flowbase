use super::*;

impl InterfaceContract for UiManagementOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("PluginSettingsPage")),
                ("route_id", mp::text_schema()),
                ("feature_id", mp::text_schema()),
                ("template_id", mp::text_schema()),
                ("provider_code", mp::text_schema()),
                ("contribution_code", mp::text_schema()),
                (
                    "source",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("language", mp::text_schema()),
                ("revision", serde_json::json!({"type":"integer"})),
                ("applied_plugin_version", mp::text_schema()),
                (
                    "overwrite_on_plugin_upgrade",
                    serde_json::json!({"type":"boolean"}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Templates")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "default_template",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("provider_code",mp::text_schema()), ("contribution_code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("language",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Jsx"))]), mp::object_schema(&[("variant",mp::tag_schema("Tsx"))])])), ("version",mp::text_schema()), ("is_default",serde_json::json!({"type":"boolean"}))]), {"type":"null"}]}),
                        ),
                        (
                            "official",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_code",mp::text_schema()), ("contribution_code",mp::text_schema()), ("title",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("language",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Jsx"))]), mp::object_schema(&[("variant",mp::tag_schema("Tsx"))])])), ("version",mp::text_schema()), ("is_default",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "managed",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("provider_code",mp::text_schema()), ("contribution_code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("latest_revision",mp::object_schema(&[("revision",serde_json::json!({"type":"integer"})), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("language",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Jsx"))]), mp::object_schema(&[("variant",mp::tag_schema("Tsx"))])])), ("is_published",serde_json::json!({"type":"boolean"}))])), ("published_revision",serde_json::json!({"anyOf": [mp::object_schema(&[("revision",serde_json::json!({"type":"integer"})), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("language",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Jsx"))]), mp::object_schema(&[("variant",mp::tag_schema("Tsx"))])])), ("is_published",serde_json::json!({"type":"boolean"}))]), {"type":"null"}]})), ("is_default",serde_json::json!({"type":"boolean"})), ("is_archived",serde_json::json!({"type":"boolean"})), ("owner_plugin_code",serde_json::json!({"anyOf":[mp::text_schema(),{"type":"null"}]})), ("owner_feature_id",serde_json::json!({"anyOf":[mp::text_schema(),{"type":"null"}]})), ("applied_plugin_version",serde_json::json!({"anyOf":[mp::text_schema(),{"type":"null"}]})), ("overwrite_on_plugin_upgrade",serde_json::json!({"type":"boolean"}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Template")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("provider_code", mp::text_schema()),
                        ("contribution_code", mp::text_schema()),
                        (
                            "name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "latest_revision",
                            mp::object_schema(&[
                                ("revision", serde_json::json!({"type":"integer"})),
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
                                ("is_published", serde_json::json!({"type":"boolean"})),
                            ]),
                        ),
                        (
                            "published_revision",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("revision",serde_json::json!({"type":"integer"})), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("language",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Jsx"))]), mp::object_schema(&[("variant",mp::tag_schema("Tsx"))])])), ("is_published",serde_json::json!({"type":"boolean"}))]), {"type":"null"}]}),
                        ),
                        ("is_default", serde_json::json!({"type":"boolean"})),
                        ("is_archived", serde_json::json!({"type":"boolean"})),
                        (
                            "owner_plugin_code",
                            serde_json::json!({"anyOf":[mp::text_schema(),{"type":"null"}]}),
                        ),
                        (
                            "owner_feature_id",
                            serde_json::json!({"anyOf":[mp::text_schema(),{"type":"null"}]}),
                        ),
                        (
                            "applied_plugin_version",
                            serde_json::json!({"anyOf":[mp::text_schema(),{"type":"null"}]}),
                        ),
                        (
                            "overwrite_on_plugin_upgrade",
                            serde_json::json!({"type":"boolean"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Components")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("scope_id",mp::text_schema()), ("component_code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("import_code",mp::text_schema()), ("source_code",mp::text_schema()), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("group",mp::object_schema(&[("byte_count",mp::count_schema())])), ("upstream",mp::object_schema(&[("identity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("version",mp::text_schema())])), ("version",mp::text_schema()), ("keywords",mp::object_schema(&[("item_count",mp::count_schema())])), ("catalog_updated_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("source_locator",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("source_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Component")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("scope_id", mp::text_schema()),
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
                        (
                            "catalog_updated_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "source_locator",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "source_checksum",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("created_at", mp::text_schema()),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogIndex")),
                (
                    "0",
                    mp::object_schema(&[
                        ("catalog_version", mp::text_schema()),
                        (
                            "generated_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("page_size", serde_json::json!({"type":"integer"})),
                        ("total_components", serde_json::json!({"type":"integer"})),
                        (
                            "source_fingerprint",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogPage")),
                (
                    "0",
                    mp::object_schema(&[
                        ("catalog_version", mp::text_schema()),
                        ("total_components", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                        ("page", serde_json::json!({"type":"integer"})),
                        (
                            "cursor",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "next_cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "records",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("component_code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("import_code",mp::text_schema()), ("source_code",mp::text_schema()), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("group",mp::object_schema(&[("byte_count",mp::count_schema())])), ("upstream",mp::object_schema(&[("identity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("version",mp::text_schema())])), ("version",mp::text_schema()), ("keywords",mp::object_schema(&[("item_count",mp::count_schema())])), ("catalog_updated_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_locator",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_checksum",mp::object_schema(&[("byte_count",mp::count_schema())])), ("local_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogSearch")),
                (
                    "0",
                    mp::object_schema(&[
                        ("catalog_version", mp::text_schema()),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                        ("total_entries", serde_json::json!({"type":"integer"})),
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("component_code",mp::text_schema()), ("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("group",mp::object_schema(&[("byte_count",mp::count_schema())])), ("upstream",mp::object_schema(&[("identity",mp::object_schema(&[("byte_count",mp::count_schema())])), ("version",mp::text_schema())])), ("version",mp::text_schema()), ("keywords",mp::object_schema(&[("item_count",mp::count_schema())])), ("catalog_page",serde_json::json!({"type":"integer"})), ("local_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogUpdateStatus")),
                (
                    "0",
                    mp::object_schema(&[
                        ("catalog_version", mp::text_schema()),
                        (
                            "source_fingerprint",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("update_available", serde_json::json!({"type":"boolean"})),
                        (
                            "groups",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("group",mp::object_schema(&[("byte_count",mp::count_schema())])), ("remote_records",serde_json::json!({"type":"integer"})), ("new_or_updated_records",serde_json::json!({"type":"integer"})), ("removed_records",serde_json::json!({"type":"integer"})), ("update_available",serde_json::json!({"type":"boolean"}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogComponent")),
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
                        (
                            "catalog_updated_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_locator",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_checksum",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "local_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CatalogSync")),
                (
                    "0",
                    mp::object_schema(&[(
                        "synchronized_records",
                        serde_json::json!({"type":"integer"}),
                    )]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("NoContent"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::PluginSettingsPage(page) => mp::object_value(&[
                ("variant", serde_json::json!("PluginSettingsPage")),
                ("route_id", mp::text(&page.route_id)?),
                ("feature_id", mp::text(&page.feature_id)?),
                ("template_id", mp::text(&page.template_id)?),
                ("provider_code", mp::text(&page.provider_code)?),
                ("contribution_code", mp::text(&page.contribution_code)?),
                (
                    "source",
                    mp::object_value(&[("byte_count", serde_json::json!(page.source.len()))]),
                ),
                ("language", mp::text(page.language.as_str())?),
                ("revision", serde_json::json!(page.revision)),
                (
                    "applied_plugin_version",
                    mp::text(&page.applied_plugin_version)?,
                ),
                (
                    "overwrite_on_plugin_upgrade",
                    serde_json::json!(page.overwrite_on_plugin_upgrade),
                ),
            ]),
            Self::Templates(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Templates".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "default_template",
                            match (&(_field_0).default_template).as_ref() {
                                Some(item) => mp::object_value(&[
                                    ("provider_code", mp::text(&(item).provider_code)?),
                                    ("contribution_code", mp::text(&(item).contribution_code)?),
                                    (
                                        "title",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).title).len()),
                                        )]),
                                    ),
                                    (
                                        "source",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).source).len()),
                                        )]),
                                    ),
                                    (
                                        "language",
                                        match &(item).language {
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
                                    ("version", mp::text(&(item).version)?),
                                    ("is_default", serde_json::Value::Bool(*(&(item).is_default))),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("official", {
                            if (&(_field_0).official).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).official).iter().map(|item| Some(mp::object_value(&[("provider_code",mp::text(&(item).provider_code)?), ("contribution_code",mp::text(&(item).contribution_code)?), ("title",mp::object_value(&[("byte_count",serde_json::json!((&(item).title).len()))])), ("source",mp::object_value(&[("byte_count",serde_json::json!((&(item).source).len()))])), ("language",match &(item).language {domain::ui_management::UiCodeTemplateLanguage::Jsx => mp::object_value(&[("variant",serde_json::Value::String("Jsx".to_owned()))]), domain::ui_management::UiCodeTemplateLanguage::Tsx => mp::object_value(&[("variant",serde_json::Value::String("Tsx".to_owned()))])}), ("version",mp::text(&(item).version)?), ("is_default",serde_json::Value::Bool(*(&(item).is_default)))]))).collect::<Option<Vec<_>>>()?)
                        }),
                        ("managed", {
                            if (&(_field_0).managed).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array((&(_field_0).managed).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("provider_code",mp::text(&(item).provider_code)?), ("contribution_code",mp::text(&(item).contribution_code)?), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(item).name).len()))])), ("latest_revision",mp::object_value(&[("revision",serde_json::json!(*(&(&(item).latest_revision).revision))), ("source",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).latest_revision).source).len()))])), ("language",match &(&(item).latest_revision).language {domain::ui_management::UiCodeTemplateLanguage::Jsx => mp::object_value(&[("variant",serde_json::Value::String("Jsx".to_owned()))]), domain::ui_management::UiCodeTemplateLanguage::Tsx => mp::object_value(&[("variant",serde_json::Value::String("Tsx".to_owned()))])}), ("is_published",serde_json::Value::Bool(*(&(&(item).latest_revision).is_published)))])), ("published_revision",match (&(item).published_revision).as_ref() { Some(item) => mp::object_value(&[("revision",serde_json::json!(*(&(item).revision))), ("source",mp::object_value(&[("byte_count",serde_json::json!((&(item).source).len()))])), ("language",match &(item).language {domain::ui_management::UiCodeTemplateLanguage::Jsx => mp::object_value(&[("variant",serde_json::Value::String("Jsx".to_owned()))]), domain::ui_management::UiCodeTemplateLanguage::Tsx => mp::object_value(&[("variant",serde_json::Value::String("Tsx".to_owned()))])}), ("is_published",serde_json::Value::Bool(*(&(item).is_published)))]), None => serde_json::Value::Null }), ("is_default",serde_json::Value::Bool(*(&(item).is_default))), ("is_archived",serde_json::Value::Bool(*(&(item).is_archived))), ("owner_plugin_code",match (&(item).owner_plugin_code).as_ref() { Some(value) => mp::text(value)?, None => serde_json::Value::Null }), ("owner_feature_id",match (&(item).owner_feature_id).as_ref() { Some(value) => mp::text(value)?, None => serde_json::Value::Null }), ("applied_plugin_version",match (&(item).applied_plugin_version).as_ref() { Some(value) => mp::text(value)?, None => serde_json::Value::Null }), ("overwrite_on_plugin_upgrade",serde_json::Value::Bool((item).overwrite_on_plugin_upgrade))]))).collect::<Option<Vec<_>>>()?)
                        }),
                    ]),
                ),
            ]),
            Self::Template(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Template".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
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
                            "latest_revision",
                            mp::object_value(&[
                                (
                                    "revision",
                                    serde_json::json!(*(&(&(_field_0).latest_revision).revision)),
                                ),
                                (
                                    "source",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).latest_revision).source).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "language",
                                    match &(&(_field_0).latest_revision).language {
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
                                (
                                    "is_published",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).latest_revision).is_published),
                                    ),
                                ),
                            ]),
                        ),
                        (
                            "published_revision",
                            match (&(_field_0).published_revision).as_ref() {
                                Some(item) => mp::object_value(&[
                                    ("revision", serde_json::json!(*(&(item).revision))),
                                    (
                                        "source",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).source).len()),
                                        )]),
                                    ),
                                    (
                                        "language",
                                        match &(item).language {
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
                                    (
                                        "is_published",
                                        serde_json::Value::Bool(*(&(item).is_published)),
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "is_default",
                            serde_json::Value::Bool(*(&(_field_0).is_default)),
                        ),
                        (
                            "is_archived",
                            serde_json::Value::Bool(*(&(_field_0).is_archived)),
                        ),
                        (
                            "owner_plugin_code",
                            match (&(_field_0).owner_plugin_code).as_ref() {
                                Some(value) => mp::text(value)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "owner_feature_id",
                            match (&(_field_0).owner_feature_id).as_ref() {
                                Some(value) => mp::text(value)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "applied_plugin_version",
                            match (&(_field_0).applied_plugin_version).as_ref() {
                                Some(value) => mp::text(value)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "overwrite_on_plugin_upgrade",
                            serde_json::Value::Bool((_field_0).overwrite_on_plugin_upgrade),
                        ),
                    ]),
                ),
            ]),
            Self::Components(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Components".to_owned()),
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
                                    ("scope_id", mp::text(&(item).scope_id)?),
                                    ("component_code", mp::text(&(item).component_code)?),
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                    (
                                        "description",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).description).len()),
                                        )]),
                                    ),
                                    ("import_code", mp::text(&(item).import_code)?),
                                    ("source_code", mp::text(&(item).source_code)?),
                                    (
                                        "source",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).source).len()),
                                        )]),
                                    ),
                                    (
                                        "group",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).group).len()),
                                        )]),
                                    ),
                                    (
                                        "upstream",
                                        mp::object_value(&[
                                            (
                                                "identity",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(&(item).upstream)
                                                        .identity)
                                                        .len()),
                                                )]),
                                            ),
                                            ("version", mp::text(&(&(item).upstream).version)?),
                                        ]),
                                    ),
                                    ("version", mp::text(&(item).version)?),
                                    (
                                        "keywords",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).keywords).len()),
                                        )]),
                                    ),
                                    (
                                        "catalog_updated_at",
                                        match (&(item).catalog_updated_at).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "source_locator",
                                        match (&(item).source_locator).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "source_checksum",
                                        match (&(item).source_checksum).as_ref() {
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
            Self::Component(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Component".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        ("scope_id", mp::text(&(_field_0).scope_id)?),
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
                        (
                            "catalog_updated_at",
                            match (&(_field_0).catalog_updated_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_locator",
                            match (&(_field_0).source_locator).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_checksum",
                            match (&(_field_0).source_checksum).as_ref() {
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
            Self::CatalogIndex(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogIndex".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("catalog_version", mp::text(&(_field_0).catalog_version)?),
                        (
                            "generated_at",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).generated_at).len()),
                            )]),
                        ),
                        ("page_size", serde_json::json!(*(&(_field_0).page_size))),
                        (
                            "total_components",
                            serde_json::json!(*(&(_field_0).total_components)),
                        ),
                        (
                            "source_fingerprint",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source_fingerprint).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::CatalogPage(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogPage".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("catalog_version", mp::text(&(_field_0).catalog_version)?),
                        (
                            "total_components",
                            serde_json::json!(*(&(_field_0).total_components)),
                        ),
                        ("page_size", serde_json::json!(*(&(_field_0).page_size))),
                        ("page", serde_json::json!(*(&(_field_0).page))),
                        (
                            "cursor",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).cursor).len()),
                            )]),
                        ),
                        (
                            "next_cursor",
                            match (&(_field_0).next_cursor).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("records", {
                            if (&(_field_0).records).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).records)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("component_code", mp::text(&(item).component_code)?),
                                            (
                                                "name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).name).len()),
                                                )]),
                                            ),
                                            (
                                                "description",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).description).len()),
                                                )]),
                                            ),
                                            ("import_code", mp::text(&(item).import_code)?),
                                            ("source_code", mp::text(&(item).source_code)?),
                                            (
                                                "source",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).source).len()),
                                                )]),
                                            ),
                                            (
                                                "group",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).group).len()),
                                                )]),
                                            ),
                                            (
                                                "upstream",
                                                mp::object_value(&[
                                                    (
                                                        "identity",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((&(&(item)
                                                                .upstream)
                                                                .identity)
                                                                .len()),
                                                        )]),
                                                    ),
                                                    (
                                                        "version",
                                                        mp::text(&(&(item).upstream).version)?,
                                                    ),
                                                ]),
                                            ),
                                            ("version", mp::text(&(item).version)?),
                                            (
                                                "keywords",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).keywords).len()),
                                                )]),
                                            ),
                                            (
                                                "catalog_updated_at",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).catalog_updated_at).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "source_locator",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).source_locator).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "source_checksum",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).source_checksum).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "local_version",
                                                match (&(item).local_version).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::CatalogSearch(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogSearch".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("catalog_version", mp::text(&(_field_0).catalog_version)?),
                        ("page", serde_json::json!(*(&(_field_0).page))),
                        ("page_size", serde_json::json!(*(&(_field_0).page_size))),
                        (
                            "total_entries",
                            serde_json::json!(*(&(_field_0).total_entries)),
                        ),
                        ("entries", {
                            if (&(_field_0).entries).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).entries)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("component_code", mp::text(&(item).component_code)?),
                                            (
                                                "name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).name).len()),
                                                )]),
                                            ),
                                            (
                                                "description",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).description).len()),
                                                )]),
                                            ),
                                            (
                                                "source",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).source).len()),
                                                )]),
                                            ),
                                            (
                                                "group",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).group).len()),
                                                )]),
                                            ),
                                            (
                                                "upstream",
                                                mp::object_value(&[
                                                    (
                                                        "identity",
                                                        mp::object_value(&[(
                                                            "byte_count",
                                                            serde_json::json!((&(&(item)
                                                                .upstream)
                                                                .identity)
                                                                .len()),
                                                        )]),
                                                    ),
                                                    (
                                                        "version",
                                                        mp::text(&(&(item).upstream).version)?,
                                                    ),
                                                ]),
                                            ),
                                            ("version", mp::text(&(item).version)?),
                                            (
                                                "keywords",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((&(item).keywords).len()),
                                                )]),
                                            ),
                                            (
                                                "catalog_page",
                                                serde_json::json!(*(&(item).catalog_page)),
                                            ),
                                            (
                                                "local_version",
                                                match (&(item).local_version).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::CatalogUpdateStatus(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogUpdateStatus".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("catalog_version", mp::text(&(_field_0).catalog_version)?),
                        (
                            "source_fingerprint",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source_fingerprint).len()),
                            )]),
                        ),
                        (
                            "update_available",
                            serde_json::Value::Bool(*(&(_field_0).update_available)),
                        ),
                        ("groups", {
                            if (&(_field_0).groups).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).groups)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "source",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).source).len()),
                                                )]),
                                            ),
                                            (
                                                "group",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).group).len()),
                                                )]),
                                            ),
                                            (
                                                "remote_records",
                                                serde_json::json!(*(&(item).remote_records)),
                                            ),
                                            (
                                                "new_or_updated_records",
                                                serde_json::json!(
                                                    *(&(item).new_or_updated_records)
                                                ),
                                            ),
                                            (
                                                "removed_records",
                                                serde_json::json!(*(&(item).removed_records)),
                                            ),
                                            (
                                                "update_available",
                                                serde_json::Value::Bool(
                                                    *(&(item).update_available),
                                                ),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::CatalogComponent(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogComponent".to_owned()),
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
                        (
                            "catalog_updated_at",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).catalog_updated_at).len()),
                            )]),
                        ),
                        (
                            "source_locator",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source_locator).len()),
                            )]),
                        ),
                        (
                            "source_checksum",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source_checksum).len()),
                            )]),
                        ),
                        (
                            "local_version",
                            match (&(_field_0).local_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::CatalogSync(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CatalogSync".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "synchronized_records",
                        serde_json::json!(*(&(_field_0).synchronized_records)),
                    )]),
                ),
            ]),
            Self::NoContent => {
                mp::object_value(&[("variant", serde_json::Value::String("NoContent".to_owned()))])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-ui-management-output";
    const CONTRACT_VERSION: &'static str = "1";
}
