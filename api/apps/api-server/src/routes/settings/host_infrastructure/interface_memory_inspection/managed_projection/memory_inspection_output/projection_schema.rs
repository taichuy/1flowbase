use super::*;

impl MemoryInspectionOutput {
    pub(super) fn managed_projection_schema_impl() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Overview")),
                (
                    "0",
                    mp::object_schema(&[
                        ("can_manage", serde_json::json!({"type":"boolean"})),
                        (
                            "contracts",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("contract_code",mp::text_schema()), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("provider_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("capabilities",mp::object_schema(&[("list_entries",serde_json::json!({"type":"boolean"})), ("list_tree",serde_json::json!({"type":"boolean"})), ("search_entries",serde_json::json!({"type":"boolean"})), ("reveal_value",serde_json::json!({"type":"boolean"})), ("default_page_size",serde_json::json!({"type":"integer"})), ("max_page_size",serde_json::json!({"type":"integer"})), ("default_byte_limit",serde_json::json!({"type":"integer"})), ("max_byte_limit",serde_json::json!({"type":"integer"})), ("default_preview_size_bytes",serde_json::json!({"type":"integer"})), ("max_full_value_size_bytes",serde_json::json!({"type":"integer"})), ("max_value_size_bytes",serde_json::json!({"type":"integer"})), ("max_payload_size_bytes",serde_json::json!({"type":"integer"}))])), ("supported",serde_json::json!({"type":"boolean"}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("StatsOverview")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "inspection_path",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "contracts",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("contract_code",mp::text_schema()), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("provider_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("capabilities",mp::object_schema(&[("list_entries",serde_json::json!({"type":"boolean"})), ("list_tree",serde_json::json!({"type":"boolean"})), ("search_entries",serde_json::json!({"type":"boolean"})), ("reveal_value",serde_json::json!({"type":"boolean"})), ("default_page_size",serde_json::json!({"type":"integer"})), ("max_page_size",serde_json::json!({"type":"integer"})), ("default_byte_limit",serde_json::json!({"type":"integer"})), ("max_byte_limit",serde_json::json!({"type":"integer"})), ("default_preview_size_bytes",serde_json::json!({"type":"integer"})), ("max_full_value_size_bytes",serde_json::json!({"type":"integer"})), ("max_value_size_bytes",serde_json::json!({"type":"integer"})), ("max_payload_size_bytes",serde_json::json!({"type":"integer"}))])), ("supported",serde_json::json!({"type":"boolean"})), ("inspection_path",mp::object_schema(&[("item_count",mp::count_schema())])), ("entry_count",serde_json::json!({"type":"integer"})), ("sensitive_entry_count",serde_json::json!({"type":"integer"})), ("total_value_size_bytes",serde_json::json!({"type":"integer"}))])}),
                        ),
                        ("entry_count", serde_json::json!({"type":"integer"})),
                        (
                            "sensitive_entry_count",
                            serde_json::json!({"type":"integer"}),
                        ),
                        (
                            "total_value_size_bytes",
                            serde_json::json!({"type":"integer"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Entries")),
                (
                    "0",
                    mp::object_schema(&[
                        ("contract_code", mp::text_schema()),
                        (
                            "label",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "provider_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "capabilities",
                            mp::object_schema(&[
                                ("list_entries", serde_json::json!({"type":"boolean"})),
                                ("list_tree", serde_json::json!({"type":"boolean"})),
                                ("search_entries", serde_json::json!({"type":"boolean"})),
                                ("reveal_value", serde_json::json!({"type":"boolean"})),
                                ("default_page_size", serde_json::json!({"type":"integer"})),
                                ("max_page_size", serde_json::json!({"type":"integer"})),
                                ("default_byte_limit", serde_json::json!({"type":"integer"})),
                                ("max_byte_limit", serde_json::json!({"type":"integer"})),
                                (
                                    "default_preview_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_full_value_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_value_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_payload_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        ("supported", serde_json::json!({"type":"boolean"})),
                        (
                            "inspection_path",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("contract_code",mp::text_schema()), ("group_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("entry_ref",mp::object_schema(&[("byte_count",mp::count_schema())])), ("key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("inspection_path",mp::object_schema(&[("item_count",mp::count_schema())])), ("entry_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("owner",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("value_size_bytes",serde_json::json!({"type":"integer"})), ("metadata_size_bytes",serde_json::json!({"type":"integer"})), ("ttl_seconds",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("created_at_unix",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("expires_at_unix",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("sensitive",serde_json::json!({"type":"boolean"})), ("metadata",mp::json_summary_schema())])}),
                        ),
                        (
                            "next_cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("limit", serde_json::json!({"type":"integer"})),
                        ("byte_limit", serde_json::json!({"type":"integer"})),
                        ("emitted_bytes", serde_json::json!({"type":"integer"})),
                        (
                            "truncated_by_byte_limit",
                            serde_json::json!({"type":"boolean"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Stats")),
                (
                    "0",
                    mp::object_schema(&[
                        ("contract_code", mp::text_schema()),
                        (
                            "label",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "provider_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "capabilities",
                            mp::object_schema(&[
                                ("list_entries", serde_json::json!({"type":"boolean"})),
                                ("list_tree", serde_json::json!({"type":"boolean"})),
                                ("search_entries", serde_json::json!({"type":"boolean"})),
                                ("reveal_value", serde_json::json!({"type":"boolean"})),
                                ("default_page_size", serde_json::json!({"type":"integer"})),
                                ("max_page_size", serde_json::json!({"type":"integer"})),
                                ("default_byte_limit", serde_json::json!({"type":"integer"})),
                                ("max_byte_limit", serde_json::json!({"type":"integer"})),
                                (
                                    "default_preview_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_full_value_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_value_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_payload_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        ("supported", serde_json::json!({"type":"boolean"})),
                        (
                            "inspection_path",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        ("entry_count", serde_json::json!({"type":"integer"})),
                        (
                            "sensitive_entry_count",
                            serde_json::json!({"type":"integer"}),
                        ),
                        (
                            "total_value_size_bytes",
                            serde_json::json!({"type":"integer"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tree")),
                (
                    "0",
                    mp::object_schema(&[
                        ("contract_code", mp::text_schema()),
                        (
                            "label",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "provider_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "capabilities",
                            mp::object_schema(&[
                                ("list_entries", serde_json::json!({"type":"boolean"})),
                                ("list_tree", serde_json::json!({"type":"boolean"})),
                                ("search_entries", serde_json::json!({"type":"boolean"})),
                                ("reveal_value", serde_json::json!({"type":"boolean"})),
                                ("default_page_size", serde_json::json!({"type":"integer"})),
                                ("max_page_size", serde_json::json!({"type":"integer"})),
                                ("default_byte_limit", serde_json::json!({"type":"integer"})),
                                ("max_byte_limit", serde_json::json!({"type":"integer"})),
                                (
                                    "default_preview_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_full_value_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_value_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "max_payload_size_bytes",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        ("supported", serde_json::json!({"type":"boolean"})),
                        (
                            "inspection_path",
                            mp::object_schema(&[("item_count", mp::count_schema())]),
                        ),
                        (
                            "nodes",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("node_ref",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label",mp::object_schema(&[("byte_count",mp::count_schema())])), ("inspection_path",mp::object_schema(&[("item_count",mp::count_schema())])), ("depth",serde_json::json!({"type":"integer"})), ("has_children",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "next_cursor",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("limit", serde_json::json!({"type":"integer"})),
                        ("byte_limit", serde_json::json!({"type":"integer"})),
                        ("emitted_bytes", serde_json::json!({"type":"integer"})),
                        (
                            "truncated_by_byte_limit",
                            serde_json::json!({"type":"boolean"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Revealed")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "metadata",
                            mp::object_schema(&[
                                ("contract_code", mp::text_schema()),
                                (
                                    "group_code",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "entry_ref",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "key",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "inspection_path",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "entry_kind",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                (
                                    "owner",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("value_size_bytes", serde_json::json!({"type":"integer"})),
                                ("metadata_size_bytes", serde_json::json!({"type":"integer"})),
                                (
                                    "ttl_seconds",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                                ),
                                (
                                    "created_at_unix",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                                ),
                                (
                                    "expires_at_unix",
                                    serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                                ),
                                ("sensitive", serde_json::json!({"type":"boolean"})),
                                ("metadata", mp::json_summary_schema()),
                            ]),
                        ),
                        (
                            "reveal_mode",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "value_state",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "value",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                        (
                            "value_preview",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("preview_size_bytes", serde_json::json!({"type":"integer"})),
                        (
                            "full_value_size_bytes",
                            serde_json::json!({"type":"integer"}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
}
