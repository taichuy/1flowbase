//! One explicit observation view for network provider outputs across interfaces.
use super::NetworkEgressProviderResponse;
use crate::extension_bus::managed_projection as mp;
use serde_json::Value;

pub(super) fn provider_schema() -> Value {
    mp::object_schema(&[
        ("id", mp::text_schema()),
        (
            "extension_category",
            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
        ),
        (
            "extension_organization",
            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
        ),
        (
            "extension_artifact_id",
            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
        ),
        ("provider_code", mp::text_schema()),
        (
            "display_name",
            mp::object_schema(&[("byte_count", mp::count_schema())]),
        ),
        (
            "description",
            mp::object_schema(&[("byte_count", mp::count_schema())]),
        ),
        ("lifecycle", mp::text_schema()),
        (
            "health_status",
            mp::object_schema(&[("byte_count", mp::count_schema())]),
        ),
        (
            "last_sync_error",
            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
        ),
        (
            "last_synced_at",
            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
        ),
        (
            "egresses",
            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_egress_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("region",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("tags",mp::object_schema(&[("item_count",mp::count_schema())])), ("availability",mp::object_schema(&[("byte_count",mp::count_schema())])), ("synced_at",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
        ),
    ])
}

pub(super) fn provider_projection(provider: &NetworkEgressProviderResponse) -> Option<Value> {
    Some(mp::object_value(&[
        ("id", mp::text(&(provider).id)?),
        (
            "extension_category",
            match (provider).extension_category.as_ref() {
                Some(item) => mp::object_value(&[("byte_count", serde_json::json!((item).len()))]),
                None => serde_json::Value::Null,
            },
        ),
        (
            "extension_organization",
            match (provider).extension_organization.as_ref() {
                Some(item) => mp::object_value(&[("byte_count", serde_json::json!((item).len()))]),
                None => serde_json::Value::Null,
            },
        ),
        (
            "extension_artifact_id",
            match (provider).extension_artifact_id.as_ref() {
                Some(item) => mp::text(item)?,
                None => serde_json::Value::Null,
            },
        ),
        ("provider_code", mp::text(&(provider).provider_code)?),
        (
            "display_name",
            mp::object_value(&[(
                "byte_count",
                serde_json::json!((provider).display_name.len()),
            )]),
        ),
        (
            "description",
            mp::object_value(&[(
                "byte_count",
                serde_json::json!((provider).description.len()),
            )]),
        ),
        ("lifecycle", mp::text(&(provider).lifecycle)?),
        (
            "health_status",
            mp::object_value(&[(
                "byte_count",
                serde_json::json!((provider).health_status.len()),
            )]),
        ),
        (
            "last_sync_error",
            match (provider).last_sync_error.as_ref() {
                Some(item) => mp::object_value(&[("byte_count", serde_json::json!((item).len()))]),
                None => serde_json::Value::Null,
            },
        ),
        (
            "last_synced_at",
            match (provider).last_synced_at.as_ref() {
                Some(item) => mp::object_value(&[("byte_count", serde_json::json!((item).len()))]),
                None => serde_json::Value::Null,
            },
        ),
        ("egresses", {
            if (provider).egresses.len() > 32 {
                return None;
            }
            serde_json::Value::Array(
                (provider)
                    .egresses
                    .iter()
                    .map(|item| {
                        mp::object_value(&[
                            (
                                "provider_egress_key",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).provider_egress_key.len()),
                                )]),
                            ),
                            (
                                "display_name",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).display_name.len()),
                                )]),
                            ),
                            (
                                "region",
                                match (item).region.as_ref() {
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
                                "availability",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).availability.len()),
                                )]),
                            ),
                            (
                                "synced_at",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).synced_at.len()),
                                )]),
                            ),
                        ])
                    })
                    .collect(),
            )
        }),
    ]))
}
