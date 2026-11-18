use crate::routes::application_runtime::managed_detail_projection;

pub(super) fn schema() -> Option<serde_json::Value> {
    use crate::extension_bus::managed_projection as mp;
    Some(mp::union_schema(vec![
        mp::object_schema([
            ("variant", mp::tag_schema("Content")),
            (
                "0",
                mp::object_schema([
                    ("content_type", mp::text_schema()),
                    (
                        "bytes",
                        mp::object_schema([("byte_count", mp::count_schema())]),
                    ),
                ]),
            ),
        ]),
        mp::object_schema([
            ("variant", mp::tag_schema("Resolved")),
            (
                "0",
                mp::object_schema([(
                    "artifacts",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("artifact_ref",mp::object_schema([("byte_count",mp::count_schema())])), ("content_type",mp::text_schema()), ("value",mp::json_summary_schema())])}),
                )]),
            ),
        ]),
        mp::object_schema([
            ("variant", mp::tag_schema("Snapshot")),
            ("0", managed_detail_projection::schema()),
        ]),
    ]))
}
