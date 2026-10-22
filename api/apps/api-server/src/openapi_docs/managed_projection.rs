//! Read-only managed-hook projection shared by both API documentation surfaces.
//! Keep the public response types and their distinct interface contract identities at each route.
use super::{DocsCatalog, DocsCatalogCategoryOperationsPage};
use serde_json::Value;

pub(crate) enum DocsOutputProjection<'a> {
    Catalog(&'a DocsCatalog),
    CategoryOperations(&'a DocsCatalogCategoryOperationsPage),
    OpenApi(&'a Value),
}

impl DocsOutputProjection<'_> {
    pub(crate) fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "title",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("version", mp::text_schema()),
                        (
                            "categories",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("operation_count",serde_json::json!({"type":"integer"}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CategoryOperations")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        (
                            "label",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "operations",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("method",mp::text_schema()), ("path",mp::object_schema(&[("byte_count",mp::count_schema())])), ("summary",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("tags",mp::object_schema(&[("item_count",mp::count_schema())])), ("group",mp::object_schema(&[("byte_count",mp::count_schema())])), ("deprecated",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        ("total", serde_json::json!({"type":"integer"})),
                        ("offset", serde_json::json!({"type":"integer"})),
                        ("limit", serde_json::json!({"type":"integer"})),
                        ("has_more", serde_json::json!({"type":"boolean"})),
                        (
                            "next_offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("OpenApi")),
                ("0", mp::json_summary_schema()),
            ]),
        ]))
    }
    pub(crate) fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Catalog".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "title",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_0).title.len()),
                            )]),
                        ),
                        ("version", mp::text(&(_field_0).version)?),
                        ("categories", {
                            if (_field_0).categories.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .categories
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            (
                                                "label",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).label.len()),
                                                )]),
                                            ),
                                            (
                                                "operation_count",
                                                serde_json::json!((item).operation_count),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::CategoryOperations(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CategoryOperations".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        (
                            "label",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_0).label.len()),
                            )]),
                        ),
                        ("operations", {
                            if (_field_0).operations.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .operations
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("method", mp::text(&(item).method)?),
                                            (
                                                "path",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).path.len()),
                                                )]),
                                            ),
                                            (
                                                "summary",
                                                match (item).summary.as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "description",
                                                match (item).description.as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "tags",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((item).tags.len()),
                                                )]),
                                            ),
                                            (
                                                "group",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).group.len()),
                                                )]),
                                            ),
                                            (
                                                "deprecated",
                                                serde_json::Value::Bool((item).deprecated),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("total", serde_json::json!((_field_0).total)),
                        ("offset", serde_json::json!((_field_0).offset)),
                        ("limit", serde_json::json!((_field_0).limit)),
                        ("has_more", serde_json::Value::Bool((_field_0).has_more)),
                        (
                            "next_offset",
                            match (_field_0).next_offset.as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::OpenApi(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("OpenApi".to_owned())),
                ("0", mp::json_summary(_field_0)),
            ]),
        })
    }
}

#[cfg(test)]
#[path = "_tests/managed_projection.rs"]
mod tests;
