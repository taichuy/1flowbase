use super::*;

impl InterfaceContract for BillingOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("PricingRules")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("provider_code",mp::text_schema()), ("upstream_model_id",mp::text_schema()), ("currency_code",mp::text_schema()), ("effective_from",serde_json::json!({"type":"integer"})), ("effective_to",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("timezone",mp::object_schema(&[("byte_count",mp::count_schema())])), ("weekday_mask",serde_json::json!({"type":"integer"})), ("local_time_start",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("local_time_end",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("priority",serde_json::json!({"type":"integer"})), ("enabled",serde_json::json!({"type":"boolean"})), ("rating_policy_enabled",serde_json::json!({"type":"boolean"})), ("rating_policy",mp::json_summary_schema()), ("source_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_catalog_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("extensions",mp::json_summary_schema()), ("created_by",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("created_at",serde_json::json!({"type":"integer"})), ("updated_at",serde_json::json!({"type":"integer"}))])}),
                        ),
                        ("total_count", serde_json::json!({"type":"integer"})),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PricingRule")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("provider_code", mp::text_schema()),
                        ("upstream_model_id", mp::text_schema()),
                        ("currency_code", mp::text_schema()),
                        ("effective_from", serde_json::json!({"type":"integer"})),
                        (
                            "effective_to",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "timezone",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("weekday_mask", serde_json::json!({"type":"integer"})),
                        (
                            "local_time_start",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "local_time_end",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("priority", serde_json::json!({"type":"integer"})),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        (
                            "rating_policy_enabled",
                            serde_json::json!({"type":"boolean"}),
                        ),
                        ("rating_policy", mp::json_summary_schema()),
                        (
                            "source_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_catalog_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "source_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "source_checksum",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("extensions", mp::json_summary_schema()),
                        (
                            "created_by",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("created_at", serde_json::json!({"type":"integer"})),
                        ("updated_at", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Deleted")),
                ("0", mp::json_summary_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PricingCatalog")),
                (
                    "0",
                    mp::object_schema(&[
                        ("schema_version", mp::text_schema()),
                        ("catalog_version", mp::text_schema()),
                        ("currency_code", mp::text_schema()),
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("provider_code",mp::text_schema()), ("upstream_model_id",mp::text_schema()), ("currency_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("effective_from",serde_json::json!({"type":"integer"})), ("effective_to",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("timezone",mp::object_schema(&[("byte_count",mp::count_schema())])), ("weekday_mask",serde_json::json!({"type":"integer"})), ("local_time_start",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("local_time_end",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("priority",serde_json::json!({"type":"integer"})), ("enabled",serde_json::json!({"type":"boolean"})), ("rating_policy_enabled",serde_json::json!({"type":"boolean"})), ("rating_policy",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]})), ("source_kind",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("source_catalog_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("extensions",serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}))])}),
                        ),
                        ("total_count", serde_json::json!({"type":"integer"})),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Imported")),
                ("0", mp::json_summary_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreditAccounts")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("user_id",mp::text_schema()), ("credit_unit",mp::object_schema(&[("byte_count",mp::count_schema())])), ("charge_enabled",serde_json::json!({"type":"boolean"})), ("current_balance",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reserved_amount",mp::object_schema(&[("byte_count",mp::count_schema())])), ("available_balance",mp::object_schema(&[("byte_count",mp::count_schema())])), ("credit_insufficient",serde_json::json!({"type":"boolean"})), ("revision",serde_json::json!({"type":"integer"})), ("created_at",serde_json::json!({"type":"integer"})), ("updated_at",serde_json::json!({"type":"integer"}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreditAccount")),
                (
                    "0",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("user_id",mp::text_schema()), ("credit_unit",mp::object_schema(&[("byte_count",mp::count_schema())])), ("charge_enabled",serde_json::json!({"type":"boolean"})), ("current_balance",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reserved_amount",mp::object_schema(&[("byte_count",mp::count_schema())])), ("available_balance",mp::object_schema(&[("byte_count",mp::count_schema())])), ("credit_insufficient",serde_json::json!({"type":"boolean"})), ("revision",serde_json::json!({"type":"integer"})), ("created_at",serde_json::json!({"type":"integer"})), ("updated_at",serde_json::json!({"type":"integer"}))]), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreditLedger")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("transaction_id",mp::text_schema()), ("workspace_id",mp::text_schema()), ("user_id",mp::text_schema()), ("actor_user_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("actor_plugin_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("transaction_type",mp::text_schema()), ("amount",mp::object_schema(&[("byte_count",mp::count_schema())])), ("balance_after",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reserved_after",mp::object_schema(&[("byte_count",mp::count_schema())])), ("credit_unit",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("source_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("idempotency_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("metadata",mp::json_summary_schema()), ("created_at",serde_json::json!({"type":"integer"}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreditTransaction")),
                (
                    "0",
                    mp::object_schema(&[
                        ("id", mp::text_schema()),
                        ("transaction_id", mp::text_schema()),
                        ("workspace_id", mp::text_schema()),
                        ("user_id", mp::text_schema()),
                        (
                            "actor_user_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "actor_plugin_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("transaction_type", mp::text_schema()),
                        (
                            "amount",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "balance_after",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "reserved_after",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "credit_unit",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "reason",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_type",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "source_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "idempotency_key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("status", mp::text_schema()),
                        ("metadata", mp::json_summary_schema()),
                        ("created_at", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::PricingRules(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PricingRules".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("items", {
                            if (&(_field_0).items).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).items)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "id",
                                                serde_json::Value::String((&(item).id).to_string()),
                                            ),
                                            ("provider_code", mp::text(&(item).provider_code)?),
                                            (
                                                "upstream_model_id",
                                                mp::text(&(item).upstream_model_id)?,
                                            ),
                                            ("currency_code", mp::text(&(item).currency_code)?),
                                            (
                                                "effective_from",
                                                serde_json::json!(
                                                    (&(item).effective_from).unix_timestamp()
                                                ),
                                            ),
                                            (
                                                "effective_to",
                                                match (&(item).effective_to).as_ref() {
                                                    Some(item) => {
                                                        serde_json::json!((item).unix_timestamp())
                                                    }
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "timezone",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).timezone).len()),
                                                )]),
                                            ),
                                            (
                                                "weekday_mask",
                                                serde_json::json!(*(&(item).weekday_mask)),
                                            ),
                                            (
                                                "local_time_start",
                                                match (&(item).local_time_start).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "local_time_end",
                                                match (&(item).local_time_end).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("priority", serde_json::json!(*(&(item).priority))),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "rating_policy_enabled",
                                                serde_json::Value::Bool(
                                                    *(&(item).rating_policy_enabled),
                                                ),
                                            ),
                                            (
                                                "rating_policy",
                                                mp::json_summary(&(item).rating_policy),
                                            ),
                                            (
                                                "source_kind",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).source_kind).len()),
                                                )]),
                                            ),
                                            (
                                                "source_catalog_id",
                                                match (&(item).source_catalog_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "source_version",
                                                match (&(item).source_version).as_ref() {
                                                    Some(item) => mp::text(item)?,
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
                                            ("extensions", mp::json_summary(&(item).extensions)),
                                            (
                                                "created_by",
                                                match (&(item).created_by).as_ref() {
                                                    Some(item) => serde_json::Value::String(
                                                        (item).to_string(),
                                                    ),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "created_at",
                                                serde_json::json!(
                                                    (&(item).created_at).unix_timestamp()
                                                ),
                                            ),
                                            (
                                                "updated_at",
                                                serde_json::json!(
                                                    (&(item).updated_at).unix_timestamp()
                                                ),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("total_count", serde_json::json!(*(&(_field_0).total_count))),
                        ("page", serde_json::json!(*(&(_field_0).page))),
                        ("page_size", serde_json::json!(*(&(_field_0).page_size))),
                    ]),
                ),
            ]),
            Self::PricingRule(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PricingRule".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "id",
                            serde_json::Value::String((&(_field_0).id).to_string()),
                        ),
                        ("provider_code", mp::text(&(_field_0).provider_code)?),
                        (
                            "upstream_model_id",
                            mp::text(&(_field_0).upstream_model_id)?,
                        ),
                        ("currency_code", mp::text(&(_field_0).currency_code)?),
                        (
                            "effective_from",
                            serde_json::json!((&(_field_0).effective_from).unix_timestamp()),
                        ),
                        (
                            "effective_to",
                            match (&(_field_0).effective_to).as_ref() {
                                Some(item) => serde_json::json!((item).unix_timestamp()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "timezone",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).timezone).len()),
                            )]),
                        ),
                        (
                            "weekday_mask",
                            serde_json::json!(*(&(_field_0).weekday_mask)),
                        ),
                        (
                            "local_time_start",
                            match (&(_field_0).local_time_start).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "local_time_end",
                            match (&(_field_0).local_time_end).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("priority", serde_json::json!(*(&(_field_0).priority))),
                        ("enabled", serde_json::Value::Bool(*(&(_field_0).enabled))),
                        (
                            "rating_policy_enabled",
                            serde_json::Value::Bool(*(&(_field_0).rating_policy_enabled)),
                        ),
                        ("rating_policy", mp::json_summary(&(_field_0).rating_policy)),
                        (
                            "source_kind",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).source_kind).len()),
                            )]),
                        ),
                        (
                            "source_catalog_id",
                            match (&(_field_0).source_catalog_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_version",
                            match (&(_field_0).source_version).as_ref() {
                                Some(item) => mp::text(item)?,
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
                        ("extensions", mp::json_summary(&(_field_0).extensions)),
                        (
                            "created_by",
                            match (&(_field_0).created_by).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "created_at",
                            serde_json::json!((&(_field_0).created_at).unix_timestamp()),
                        ),
                        (
                            "updated_at",
                            serde_json::json!((&(_field_0).updated_at).unix_timestamp()),
                        ),
                    ]),
                ),
            ]),
            Self::Deleted(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Deleted".to_owned())),
                ("0", mp::json_summary(_field_0)),
            ]),
            Self::PricingCatalog(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("PricingCatalog".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("schema_version", mp::text(&(_field_0).schema_version)?),
                        ("catalog_version", mp::text(&(_field_0).catalog_version)?),
                        ("currency_code", mp::text(&(_field_0).currency_code)?),
                        ("items", {
                            if (&(_field_0).items).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).items)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "id",
                                                match (&(item).id).as_ref() {
                                                    Some(item) => serde_json::Value::String(
                                                        (item).to_string(),
                                                    ),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("provider_code", mp::text(&(item).provider_code)?),
                                            (
                                                "upstream_model_id",
                                                mp::text(&(item).upstream_model_id)?,
                                            ),
                                            (
                                                "currency_code",
                                                match (&(item).currency_code).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "effective_from",
                                                serde_json::json!(
                                                    (&(item).effective_from).unix_timestamp()
                                                ),
                                            ),
                                            (
                                                "effective_to",
                                                match (&(item).effective_to).as_ref() {
                                                    Some(item) => {
                                                        serde_json::json!((item).unix_timestamp())
                                                    }
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "timezone",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).timezone).len()),
                                                )]),
                                            ),
                                            (
                                                "weekday_mask",
                                                serde_json::json!(*(&(item).weekday_mask)),
                                            ),
                                            (
                                                "local_time_start",
                                                match (&(item).local_time_start).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "local_time_end",
                                                match (&(item).local_time_end).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("priority", serde_json::json!(*(&(item).priority))),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "rating_policy_enabled",
                                                serde_json::Value::Bool(
                                                    *(&(item).rating_policy_enabled),
                                                ),
                                            ),
                                            (
                                                "rating_policy",
                                                match (&(item).rating_policy).as_ref() {
                                                    Some(item) => mp::json_summary(item),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "source_kind",
                                                match (&(item).source_kind).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "source_catalog_id",
                                                match (&(item).source_catalog_id).as_ref() {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "source_version",
                                                match (&(item).source_version).as_ref() {
                                                    Some(item) => mp::text(item)?,
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
                                            (
                                                "extensions",
                                                match (&(item).extensions).as_ref() {
                                                    Some(item) => mp::json_summary(item),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("total_count", serde_json::json!(*(&(_field_0).total_count))),
                        ("page", serde_json::json!(*(&(_field_0).page))),
                        ("page_size", serde_json::json!(*(&(_field_0).page_size))),
                    ]),
                ),
            ]),
            Self::Imported(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Imported".to_owned())),
                ("0", mp::json_summary(_field_0)),
            ]),
            Self::CreditAccounts(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreditAccounts".to_owned()),
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
                                    ("id", serde_json::Value::String((&(item).id).to_string())),
                                    (
                                        "workspace_id",
                                        serde_json::Value::String(
                                            (&(item).workspace_id).to_string(),
                                        ),
                                    ),
                                    (
                                        "user_id",
                                        serde_json::Value::String((&(item).user_id).to_string()),
                                    ),
                                    (
                                        "credit_unit",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).credit_unit).len()),
                                        )]),
                                    ),
                                    (
                                        "charge_enabled",
                                        serde_json::Value::Bool(*(&(item).charge_enabled)),
                                    ),
                                    (
                                        "current_balance",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).current_balance).len()),
                                        )]),
                                    ),
                                    (
                                        "reserved_amount",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).reserved_amount).len()),
                                        )]),
                                    ),
                                    (
                                        "available_balance",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).available_balance).len()),
                                        )]),
                                    ),
                                    (
                                        "credit_insufficient",
                                        serde_json::Value::Bool(*(&(item).credit_insufficient)),
                                    ),
                                    ("revision", serde_json::json!(*(&(item).revision))),
                                    (
                                        "created_at",
                                        serde_json::json!((&(item).created_at).unix_timestamp()),
                                    ),
                                    (
                                        "updated_at",
                                        serde_json::json!((&(item).updated_at).unix_timestamp()),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::CreditAccount(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreditAccount".to_owned()),
                ),
                (
                    "0",
                    match (_field_0).as_ref() {
                        Some(item) => mp::object_value(&[
                            ("id", serde_json::Value::String((&(item).id).to_string())),
                            (
                                "workspace_id",
                                serde_json::Value::String((&(item).workspace_id).to_string()),
                            ),
                            (
                                "user_id",
                                serde_json::Value::String((&(item).user_id).to_string()),
                            ),
                            (
                                "credit_unit",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).credit_unit).len()),
                                )]),
                            ),
                            (
                                "charge_enabled",
                                serde_json::Value::Bool(*(&(item).charge_enabled)),
                            ),
                            (
                                "current_balance",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).current_balance).len()),
                                )]),
                            ),
                            (
                                "reserved_amount",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).reserved_amount).len()),
                                )]),
                            ),
                            (
                                "available_balance",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(item).available_balance).len()),
                                )]),
                            ),
                            (
                                "credit_insufficient",
                                serde_json::Value::Bool(*(&(item).credit_insufficient)),
                            ),
                            ("revision", serde_json::json!(*(&(item).revision))),
                            (
                                "created_at",
                                serde_json::json!((&(item).created_at).unix_timestamp()),
                            ),
                            (
                                "updated_at",
                                serde_json::json!((&(item).updated_at).unix_timestamp()),
                            ),
                        ]),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::CreditLedger(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreditLedger".to_owned()),
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
                                    ("id", serde_json::Value::String((&(item).id).to_string())),
                                    (
                                        "transaction_id",
                                        serde_json::Value::String(
                                            (&(item).transaction_id).to_string(),
                                        ),
                                    ),
                                    (
                                        "workspace_id",
                                        serde_json::Value::String(
                                            (&(item).workspace_id).to_string(),
                                        ),
                                    ),
                                    (
                                        "user_id",
                                        serde_json::Value::String((&(item).user_id).to_string()),
                                    ),
                                    (
                                        "actor_user_id",
                                        match (&(item).actor_user_id).as_ref() {
                                            Some(item) => {
                                                serde_json::Value::String((item).to_string())
                                            }
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "actor_plugin_id",
                                        match (&(item).actor_plugin_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("transaction_type", mp::text(&(item).transaction_type)?),
                                    (
                                        "amount",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).amount).len()),
                                        )]),
                                    ),
                                    (
                                        "balance_after",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).balance_after).len()),
                                        )]),
                                    ),
                                    (
                                        "reserved_after",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).reserved_after).len()),
                                        )]),
                                    ),
                                    (
                                        "credit_unit",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).credit_unit).len()),
                                        )]),
                                    ),
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).reason).len()),
                                        )]),
                                    ),
                                    (
                                        "source_type",
                                        match (&(item).source_type).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "source_id",
                                        match (&(item).source_id).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "idempotency_key",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).idempotency_key).len()),
                                        )]),
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    ("metadata", mp::json_summary(&(item).metadata)),
                                    (
                                        "created_at",
                                        serde_json::json!((&(item).created_at).unix_timestamp()),
                                    ),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::CreditTransaction(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreditTransaction".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "id",
                            serde_json::Value::String((&(_field_0).id).to_string()),
                        ),
                        (
                            "transaction_id",
                            serde_json::Value::String((&(_field_0).transaction_id).to_string()),
                        ),
                        (
                            "workspace_id",
                            serde_json::Value::String((&(_field_0).workspace_id).to_string()),
                        ),
                        (
                            "user_id",
                            serde_json::Value::String((&(_field_0).user_id).to_string()),
                        ),
                        (
                            "actor_user_id",
                            match (&(_field_0).actor_user_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "actor_plugin_id",
                            match (&(_field_0).actor_plugin_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("transaction_type", mp::text(&(_field_0).transaction_type)?),
                        (
                            "amount",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).amount).len()),
                            )]),
                        ),
                        (
                            "balance_after",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).balance_after).len()),
                            )]),
                        ),
                        (
                            "reserved_after",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).reserved_after).len()),
                            )]),
                        ),
                        (
                            "credit_unit",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).credit_unit).len()),
                            )]),
                        ),
                        (
                            "reason",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).reason).len()),
                            )]),
                        ),
                        (
                            "source_type",
                            match (&(_field_0).source_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_id",
                            match (&(_field_0).source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "idempotency_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).idempotency_key).len()),
                            )]),
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        ("metadata", mp::json_summary(&(_field_0).metadata)),
                        (
                            "created_at",
                            serde_json::json!((&(_field_0).created_at).unix_timestamp()),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-billing-output";
    const CONTRACT_VERSION: &'static str = "1";
}
