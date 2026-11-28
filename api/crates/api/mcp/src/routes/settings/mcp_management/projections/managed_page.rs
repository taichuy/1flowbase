use crate::extension_bus::managed_projection as mp;
use serde_json::{json, Value};

pub(crate) fn variant_schema<'a>(
    tag: &str,
    fields: impl IntoIterator<Item = (&'a str, Value)>,
) -> Value {
    mp::object_schema([
        ("variant", mp::tag_schema(tag)),
        ("0", mp::object_schema(fields)),
    ])
}

pub(crate) fn count_schema<'a>(tag: &str, fields: impl IntoIterator<Item = &'a str>) -> Value {
    variant_schema(
        tag,
        fields.into_iter().map(|name| (name, mp::count_schema())),
    )
}

pub(crate) fn project<'a>(tag: &str, fields: impl IntoIterator<Item = (&'a str, Value)>) -> Value {
    mp::object_value([("variant", json!(tag)), ("0", mp::object_value(fields))])
}

pub(crate) fn query_schema(tag: &str, position: &str, size: &str) -> Value {
    let nullable_count = || json!({"anyOf":[mp::count_schema(), {"type":"null"}]});
    variant_schema(
        tag,
        [
            (position, nullable_count()),
            (size, nullable_count()),
            ("search_byte_count", nullable_count()),
        ],
    )
}

pub(crate) fn query(
    tag: &str,
    position: (&str, Option<usize>),
    size: (&str, Option<usize>),
    search: Option<&str>,
) -> Value {
    project(
        tag,
        [
            (position.0, json!(position.1)),
            (size.0, json!(size.1)),
            ("search_byte_count", json!(search.map(str::len))),
        ],
    )
}
