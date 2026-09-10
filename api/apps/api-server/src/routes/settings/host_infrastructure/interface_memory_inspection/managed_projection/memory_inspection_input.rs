use super::*;

impl InterfaceContract for MemoryInspectionInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("Overview"))]),
            mp::object_schema(&[("variant", mp::tag_schema("StatsOverview"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Entries")),
                ("contract_code", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "path",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "byte_limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Stats")),
                ("contract_code", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[(
                        "path",
                        serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tree")),
                ("contract_code", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "path",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "byte_limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Search")),
                ("contract_code", mp::text_schema()),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "q",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "path",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "byte_limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Reveal")),
                ("contract_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "entry_ref",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "reveal_mode",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Overview => {
                mp::object_value(&[("variant", serde_json::Value::String("Overview".to_owned()))])
            }
            Self::StatsOverview => mp::object_value(&[(
                "variant",
                serde_json::Value::String("StatsOverview".to_owned()),
            )]),
            Self::Entries {
                contract_code: _field_contract_code,
                query: _field_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Entries".to_owned())),
                ("contract_code", mp::text(_field_contract_code)?),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "path",
                            match (&(_field_query).path).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cursor",
                            match (&(_field_query).cursor).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_query).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "byte_limit",
                            match (&(_field_query).byte_limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Stats {
                contract_code: _field_contract_code,
                query: _field_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Stats".to_owned())),
                ("contract_code", mp::text(_field_contract_code)?),
                (
                    "query",
                    mp::object_value(&[(
                        "path",
                        match (&(_field_query).path).as_ref() {
                            Some(item) => {
                                mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                            }
                            None => serde_json::Value::Null,
                        },
                    )]),
                ),
            ]),
            Self::Tree {
                contract_code: _field_contract_code,
                query: _field_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Tree".to_owned())),
                ("contract_code", mp::text(_field_contract_code)?),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "path",
                            match (&(_field_query).path).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cursor",
                            match (&(_field_query).cursor).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_query).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "byte_limit",
                            match (&(_field_query).byte_limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Search {
                contract_code: _field_contract_code,
                query: _field_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Search".to_owned())),
                ("contract_code", mp::text(_field_contract_code)?),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "q",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_query).q).len()),
                            )]),
                        ),
                        (
                            "path",
                            match (&(_field_query).path).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cursor",
                            match (&(_field_query).cursor).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (&(_field_query).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "byte_limit",
                            match (&(_field_query).byte_limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Reveal {
                contract_code: _field_contract_code,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Reveal".to_owned())),
                ("contract_code", mp::text(_field_contract_code)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "entry_ref",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).entry_ref).len()),
                            )]),
                        ),
                        (
                            "reveal_mode",
                            match (&(_field_body).reveal_mode).as_ref() {
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
        })
    }

    const CONTRACT_ID: &'static str = "console-host-infrastructure-memory-inspection-input";
    const CONTRACT_VERSION: &'static str = "1";
}
