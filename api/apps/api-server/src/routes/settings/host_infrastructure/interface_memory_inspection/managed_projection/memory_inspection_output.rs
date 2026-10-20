use super::*;

impl InterfaceContract for MemoryInspectionOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        Self::managed_projection_schema_impl()
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Overview(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Overview".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("can_manage", serde_json::Value::Bool((_field_0).can_manage)),
                        ("contracts", {
                            if (_field_0).contracts.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .contracts
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("contract_code", mp::text(&(item).contract_code)?),
                                            (
                                                "label",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).label.len()),
                                                )]),
                                            ),
                                            (
                                                "provider_code",
                                                match (item).provider_code.as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "capabilities",
                                                mp::object_value(&[
                                                    (
                                                        "list_entries",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.list_entries,
                                                        ),
                                                    ),
                                                    (
                                                        "list_tree",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.list_tree,
                                                        ),
                                                    ),
                                                    (
                                                        "search_entries",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.search_entries,
                                                        ),
                                                    ),
                                                    (
                                                        "reveal_value",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.reveal_value,
                                                        ),
                                                    ),
                                                    (
                                                        "default_page_size",
                                                        serde_json::json!(
                                                            (item).capabilities.default_page_size
                                                        ),
                                                    ),
                                                    (
                                                        "max_page_size",
                                                        serde_json::json!(
                                                            (item).capabilities.max_page_size
                                                        ),
                                                    ),
                                                    (
                                                        "default_byte_limit",
                                                        serde_json::json!(
                                                            (item).capabilities.default_byte_limit
                                                        ),
                                                    ),
                                                    (
                                                        "max_byte_limit",
                                                        serde_json::json!(
                                                            (item).capabilities.max_byte_limit
                                                        ),
                                                    ),
                                                    (
                                                        "default_preview_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .default_preview_size_bytes
                                                        ),
                                                    ),
                                                    (
                                                        "max_full_value_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .max_full_value_size_bytes
                                                        ),
                                                    ),
                                                    (
                                                        "max_value_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .max_value_size_bytes
                                                        ),
                                                    ),
                                                    (
                                                        "max_payload_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .max_payload_size_bytes
                                                        ),
                                                    ),
                                                ]),
                                            ),
                                            (
                                                "supported",
                                                serde_json::Value::Bool((item).supported),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::StatsOverview(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("StatsOverview".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "inspection_path",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((_field_0).inspection_path.len()),
                            )]),
                        ),
                        ("contracts", {
                            if (_field_0).contracts.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .contracts
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("contract_code", mp::text(&(item).contract_code)?),
                                            (
                                                "label",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).label.len()),
                                                )]),
                                            ),
                                            (
                                                "provider_code",
                                                match (item).provider_code.as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "capabilities",
                                                mp::object_value(&[
                                                    (
                                                        "list_entries",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.list_entries,
                                                        ),
                                                    ),
                                                    (
                                                        "list_tree",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.list_tree,
                                                        ),
                                                    ),
                                                    (
                                                        "search_entries",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.search_entries,
                                                        ),
                                                    ),
                                                    (
                                                        "reveal_value",
                                                        serde_json::Value::Bool(
                                                            (item).capabilities.reveal_value,
                                                        ),
                                                    ),
                                                    (
                                                        "default_page_size",
                                                        serde_json::json!(
                                                            (item).capabilities.default_page_size
                                                        ),
                                                    ),
                                                    (
                                                        "max_page_size",
                                                        serde_json::json!(
                                                            (item).capabilities.max_page_size
                                                        ),
                                                    ),
                                                    (
                                                        "default_byte_limit",
                                                        serde_json::json!(
                                                            (item).capabilities.default_byte_limit
                                                        ),
                                                    ),
                                                    (
                                                        "max_byte_limit",
                                                        serde_json::json!(
                                                            (item).capabilities.max_byte_limit
                                                        ),
                                                    ),
                                                    (
                                                        "default_preview_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .default_preview_size_bytes
                                                        ),
                                                    ),
                                                    (
                                                        "max_full_value_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .max_full_value_size_bytes
                                                        ),
                                                    ),
                                                    (
                                                        "max_value_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .max_value_size_bytes
                                                        ),
                                                    ),
                                                    (
                                                        "max_payload_size_bytes",
                                                        serde_json::json!(
                                                            (item)
                                                                .capabilities
                                                                .max_payload_size_bytes
                                                        ),
                                                    ),
                                                ]),
                                            ),
                                            (
                                                "supported",
                                                serde_json::Value::Bool((item).supported),
                                            ),
                                            (
                                                "inspection_path",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((item).inspection_path.len()),
                                                )]),
                                            ),
                                            ("entry_count", serde_json::json!((item).entry_count)),
                                            (
                                                "sensitive_entry_count",
                                                serde_json::json!((item).sensitive_entry_count),
                                            ),
                                            (
                                                "total_value_size_bytes",
                                                serde_json::json!((item).total_value_size_bytes),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("entry_count", serde_json::json!((_field_0).entry_count)),
                        (
                            "sensitive_entry_count",
                            serde_json::json!((_field_0).sensitive_entry_count),
                        ),
                        (
                            "total_value_size_bytes",
                            serde_json::json!((_field_0).total_value_size_bytes),
                        ),
                    ]),
                ),
            ]),
            Self::Entries(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Entries".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("contract_code", mp::text(&(_field_0).contract_code)?),
                        (
                            "label",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_0).label.len()),
                            )]),
                        ),
                        (
                            "provider_code",
                            match (_field_0).provider_code.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "capabilities",
                            mp::object_value(&[
                                (
                                    "list_entries",
                                    serde_json::Value::Bool((_field_0).capabilities.list_entries),
                                ),
                                (
                                    "list_tree",
                                    serde_json::Value::Bool((_field_0).capabilities.list_tree),
                                ),
                                (
                                    "search_entries",
                                    serde_json::Value::Bool((_field_0).capabilities.search_entries),
                                ),
                                (
                                    "reveal_value",
                                    serde_json::Value::Bool((_field_0).capabilities.reveal_value),
                                ),
                                (
                                    "default_page_size",
                                    serde_json::json!((_field_0).capabilities.default_page_size),
                                ),
                                (
                                    "max_page_size",
                                    serde_json::json!((_field_0).capabilities.max_page_size),
                                ),
                                (
                                    "default_byte_limit",
                                    serde_json::json!((_field_0).capabilities.default_byte_limit),
                                ),
                                (
                                    "max_byte_limit",
                                    serde_json::json!((_field_0).capabilities.max_byte_limit),
                                ),
                                (
                                    "default_preview_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.default_preview_size_bytes
                                    ),
                                ),
                                (
                                    "max_full_value_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.max_full_value_size_bytes
                                    ),
                                ),
                                (
                                    "max_value_size_bytes",
                                    serde_json::json!((_field_0).capabilities.max_value_size_bytes),
                                ),
                                (
                                    "max_payload_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.max_payload_size_bytes
                                    ),
                                ),
                            ]),
                        ),
                        ("supported", serde_json::Value::Bool((_field_0).supported)),
                        (
                            "inspection_path",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((_field_0).inspection_path.len()),
                            )]),
                        ),
                        ("entries", {
                            if (_field_0).entries.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .entries
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("contract_code", mp::text(&(item).contract_code)?),
                                            (
                                                "group_code",
                                                match (item).group_code.as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "entry_ref",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).entry_ref.len()),
                                                )]),
                                            ),
                                            (
                                                "key",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).key.len()),
                                                )]),
                                            ),
                                            (
                                                "inspection_path",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((item).inspection_path.len()),
                                                )]),
                                            ),
                                            (
                                                "entry_kind",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).entry_kind.len()),
                                                )]),
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "owner",
                                                match (item).owner.as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "value_size_bytes",
                                                serde_json::json!((item).value_size_bytes),
                                            ),
                                            (
                                                "metadata_size_bytes",
                                                serde_json::json!((item).metadata_size_bytes),
                                            ),
                                            (
                                                "ttl_seconds",
                                                match (item).ttl_seconds.as_ref() {
                                                    Some(item) => serde_json::json!(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "created_at_unix",
                                                match (item).created_at_unix.as_ref() {
                                                    Some(item) => serde_json::json!(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "expires_at_unix",
                                                match (item).expires_at_unix.as_ref() {
                                                    Some(item) => serde_json::json!(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "sensitive",
                                                serde_json::Value::Bool((item).sensitive),
                                            ),
                                            ("metadata", mp::json_summary(&(item).metadata)),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "next_cursor",
                            match (_field_0).next_cursor.as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("limit", serde_json::json!((_field_0).limit)),
                        ("byte_limit", serde_json::json!((_field_0).byte_limit)),
                        ("emitted_bytes", serde_json::json!((_field_0).emitted_bytes)),
                        (
                            "truncated_by_byte_limit",
                            serde_json::Value::Bool((_field_0).truncated_by_byte_limit),
                        ),
                    ]),
                ),
            ]),
            Self::Stats(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Stats".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("contract_code", mp::text(&(_field_0).contract_code)?),
                        (
                            "label",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_0).label.len()),
                            )]),
                        ),
                        (
                            "provider_code",
                            match (_field_0).provider_code.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "capabilities",
                            mp::object_value(&[
                                (
                                    "list_entries",
                                    serde_json::Value::Bool((_field_0).capabilities.list_entries),
                                ),
                                (
                                    "list_tree",
                                    serde_json::Value::Bool((_field_0).capabilities.list_tree),
                                ),
                                (
                                    "search_entries",
                                    serde_json::Value::Bool((_field_0).capabilities.search_entries),
                                ),
                                (
                                    "reveal_value",
                                    serde_json::Value::Bool((_field_0).capabilities.reveal_value),
                                ),
                                (
                                    "default_page_size",
                                    serde_json::json!((_field_0).capabilities.default_page_size),
                                ),
                                (
                                    "max_page_size",
                                    serde_json::json!((_field_0).capabilities.max_page_size),
                                ),
                                (
                                    "default_byte_limit",
                                    serde_json::json!((_field_0).capabilities.default_byte_limit),
                                ),
                                (
                                    "max_byte_limit",
                                    serde_json::json!((_field_0).capabilities.max_byte_limit),
                                ),
                                (
                                    "default_preview_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.default_preview_size_bytes
                                    ),
                                ),
                                (
                                    "max_full_value_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.max_full_value_size_bytes
                                    ),
                                ),
                                (
                                    "max_value_size_bytes",
                                    serde_json::json!((_field_0).capabilities.max_value_size_bytes),
                                ),
                                (
                                    "max_payload_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.max_payload_size_bytes
                                    ),
                                ),
                            ]),
                        ),
                        ("supported", serde_json::Value::Bool((_field_0).supported)),
                        (
                            "inspection_path",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((_field_0).inspection_path.len()),
                            )]),
                        ),
                        ("entry_count", serde_json::json!((_field_0).entry_count)),
                        (
                            "sensitive_entry_count",
                            serde_json::json!((_field_0).sensitive_entry_count),
                        ),
                        (
                            "total_value_size_bytes",
                            serde_json::json!((_field_0).total_value_size_bytes),
                        ),
                    ]),
                ),
            ]),
            Self::Tree(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Tree".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("contract_code", mp::text(&(_field_0).contract_code)?),
                        (
                            "label",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_0).label.len()),
                            )]),
                        ),
                        (
                            "provider_code",
                            match (_field_0).provider_code.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "capabilities",
                            mp::object_value(&[
                                (
                                    "list_entries",
                                    serde_json::Value::Bool((_field_0).capabilities.list_entries),
                                ),
                                (
                                    "list_tree",
                                    serde_json::Value::Bool((_field_0).capabilities.list_tree),
                                ),
                                (
                                    "search_entries",
                                    serde_json::Value::Bool((_field_0).capabilities.search_entries),
                                ),
                                (
                                    "reveal_value",
                                    serde_json::Value::Bool((_field_0).capabilities.reveal_value),
                                ),
                                (
                                    "default_page_size",
                                    serde_json::json!((_field_0).capabilities.default_page_size),
                                ),
                                (
                                    "max_page_size",
                                    serde_json::json!((_field_0).capabilities.max_page_size),
                                ),
                                (
                                    "default_byte_limit",
                                    serde_json::json!((_field_0).capabilities.default_byte_limit),
                                ),
                                (
                                    "max_byte_limit",
                                    serde_json::json!((_field_0).capabilities.max_byte_limit),
                                ),
                                (
                                    "default_preview_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.default_preview_size_bytes
                                    ),
                                ),
                                (
                                    "max_full_value_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.max_full_value_size_bytes
                                    ),
                                ),
                                (
                                    "max_value_size_bytes",
                                    serde_json::json!((_field_0).capabilities.max_value_size_bytes),
                                ),
                                (
                                    "max_payload_size_bytes",
                                    serde_json::json!(
                                        (_field_0).capabilities.max_payload_size_bytes
                                    ),
                                ),
                            ]),
                        ),
                        ("supported", serde_json::Value::Bool((_field_0).supported)),
                        (
                            "inspection_path",
                            mp::object_value(&[(
                                "item_count",
                                serde_json::json!((_field_0).inspection_path.len()),
                            )]),
                        ),
                        ("nodes", {
                            if (_field_0).nodes.len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .nodes
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "node_ref",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).node_ref.len()),
                                                )]),
                                            ),
                                            (
                                                "label",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((item).label.len()),
                                                )]),
                                            ),
                                            (
                                                "inspection_path",
                                                mp::object_value(&[(
                                                    "item_count",
                                                    serde_json::json!((item).inspection_path.len()),
                                                )]),
                                            ),
                                            ("depth", serde_json::json!((item).depth)),
                                            (
                                                "has_children",
                                                serde_json::Value::Bool((item).has_children),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "next_cursor",
                            match (_field_0).next_cursor.as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("limit", serde_json::json!((_field_0).limit)),
                        ("byte_limit", serde_json::json!((_field_0).byte_limit)),
                        ("emitted_bytes", serde_json::json!((_field_0).emitted_bytes)),
                        (
                            "truncated_by_byte_limit",
                            serde_json::Value::Bool((_field_0).truncated_by_byte_limit),
                        ),
                    ]),
                ),
            ]),
            Self::Revealed(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Revealed".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "metadata",
                            mp::object_value(&[
                                (
                                    "contract_code",
                                    mp::text(&(_field_0).metadata.contract_code)?,
                                ),
                                (
                                    "group_code",
                                    match (_field_0).metadata.group_code.as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "entry_ref",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).metadata.entry_ref.len()),
                                    )]),
                                ),
                                (
                                    "key",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).metadata.key.len()),
                                    )]),
                                ),
                                (
                                    "inspection_path",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!((_field_0)
                                            .metadata
                                            .inspection_path
                                            .len()),
                                    )]),
                                ),
                                (
                                    "entry_kind",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((_field_0).metadata.entry_kind.len()),
                                    )]),
                                ),
                                ("status", mp::text(&(_field_0).metadata.status)?),
                                (
                                    "owner",
                                    match (_field_0).metadata.owner.as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "value_size_bytes",
                                    serde_json::json!((_field_0).metadata.value_size_bytes),
                                ),
                                (
                                    "metadata_size_bytes",
                                    serde_json::json!((_field_0).metadata.metadata_size_bytes),
                                ),
                                (
                                    "ttl_seconds",
                                    match (_field_0).metadata.ttl_seconds.as_ref() {
                                        Some(item) => serde_json::json!(*(item)),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "created_at_unix",
                                    match (_field_0).metadata.created_at_unix.as_ref() {
                                        Some(item) => serde_json::json!(*(item)),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "expires_at_unix",
                                    match (_field_0).metadata.expires_at_unix.as_ref() {
                                        Some(item) => serde_json::json!(*(item)),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "sensitive",
                                    serde_json::Value::Bool((_field_0).metadata.sensitive),
                                ),
                                ("metadata", mp::json_summary(&(_field_0).metadata.metadata)),
                            ]),
                        ),
                        (
                            "reveal_mode",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_0).reveal_mode.len()),
                            )]),
                        ),
                        (
                            "value_state",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((_field_0).value_state.len()),
                            )]),
                        ),
                        (
                            "value",
                            match (_field_0).value.as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "value_preview",
                            match (_field_0).value_preview.as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "preview_size_bytes",
                            serde_json::json!((_field_0).preview_size_bytes),
                        ),
                        (
                            "full_value_size_bytes",
                            serde_json::json!((_field_0).full_value_size_bytes),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-host-infrastructure-memory-inspection-output";
    const CONTRACT_VERSION: &'static str = "1";
}

#[path = "memory_inspection_output/projection_schema.rs"]
mod projection_schema;
