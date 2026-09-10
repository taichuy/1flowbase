use super::*;

impl InterfaceContract for BillingInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListPricingRules")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "provider_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "upstream_model_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "enabled",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                        (
                            "source_kind",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "page",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "page_size",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("CreatePricingRule")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("provider_code", mp::text_schema()),
                        ("upstream_model_id", mp::text_schema()),
                        (
                            "currency_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
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
                        ("rules", mp::json_summary_schema()),
                        (
                            "source_kind",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "extensions",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdatePricingRule")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("provider_code", mp::text_schema()),
                        ("upstream_model_id", mp::text_schema()),
                        (
                            "currency_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
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
                        ("rules", mp::json_summary_schema()),
                        (
                            "source_kind",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "extensions",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeletePricingRule")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetPricingCatalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "provider_code",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "upstream_model_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "page",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "page_size",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportPricingCatalog")),
                (
                    "0",
                    mp::object_schema(&[(
                        "catalog_ids",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListCreditAccounts")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "user_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "before_created_at",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "before_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetCreditAccount")),
                ("user_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListCreditLedger")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "limit",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "offset",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "user_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "before_created_at",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}),
                        ),
                        (
                            "before_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GrantCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ChargeCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("AdjustCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("EnableCharge")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DisableCharge")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RefundCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListPricingRules(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListPricingRules".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "provider_code",
                            match (&(_field_0).provider_code).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "upstream_model_id",
                            match (&(_field_0).upstream_model_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "enabled",
                            match (&(_field_0).enabled).as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_kind",
                            match (&(_field_0).source_kind).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "page",
                            match (&(_field_0).page).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "page_size",
                            match (&(_field_0).page_size).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::CreatePricingRule(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("CreatePricingRule".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "id",
                            match (&(_field_0).id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("provider_code", mp::text(&(_field_0).provider_code)?),
                        (
                            "upstream_model_id",
                            mp::text(&(_field_0).upstream_model_id)?,
                        ),
                        (
                            "currency_code",
                            match (&(_field_0).currency_code).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
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
                        ("rules", mp::json_summary(&(_field_0).rules)),
                        (
                            "source_kind",
                            match (&(_field_0).source_kind).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
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
                        (
                            "extensions",
                            match (&(_field_0).extensions).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::UpdatePricingRule {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpdatePricingRule".to_owned()),
                ),
                ("id", serde_json::Value::String((_field_id).to_string())),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "id",
                            match (&(_field_body).id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("provider_code", mp::text(&(_field_body).provider_code)?),
                        (
                            "upstream_model_id",
                            mp::text(&(_field_body).upstream_model_id)?,
                        ),
                        (
                            "currency_code",
                            match (&(_field_body).currency_code).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "effective_from",
                            serde_json::json!((&(_field_body).effective_from).unix_timestamp()),
                        ),
                        (
                            "effective_to",
                            match (&(_field_body).effective_to).as_ref() {
                                Some(item) => serde_json::json!((item).unix_timestamp()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "timezone",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).timezone).len()),
                            )]),
                        ),
                        (
                            "weekday_mask",
                            serde_json::json!(*(&(_field_body).weekday_mask)),
                        ),
                        (
                            "local_time_start",
                            match (&(_field_body).local_time_start).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "local_time_end",
                            match (&(_field_body).local_time_end).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("priority", serde_json::json!(*(&(_field_body).priority))),
                        (
                            "enabled",
                            serde_json::Value::Bool(*(&(_field_body).enabled)),
                        ),
                        ("rules", mp::json_summary(&(_field_body).rules)),
                        (
                            "source_kind",
                            match (&(_field_body).source_kind).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_catalog_id",
                            match (&(_field_body).source_catalog_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_version",
                            match (&(_field_body).source_version).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_checksum",
                            match (&(_field_body).source_checksum).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "extensions",
                            match (&(_field_body).extensions).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::DeletePricingRule { id: _field_id, .. } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeletePricingRule".to_owned()),
                ),
                ("id", serde_json::Value::String((_field_id).to_string())),
            ]),
            Self::GetPricingCatalog(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetPricingCatalog".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "provider_code",
                            match (&(_field_0).provider_code).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "upstream_model_id",
                            match (&(_field_0).upstream_model_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "page",
                            match (&(_field_0).page).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "page_size",
                            match (&(_field_0).page_size).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ImportPricingCatalog(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ImportPricingCatalog".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("catalog_ids", {
                        if (&(_field_0).catalog_ids).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_0).catalog_ids)
                                .iter()
                                .map(|item| Some(serde_json::Value::String((item).to_string())))
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::ListCreditAccounts(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListCreditAccounts".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "limit",
                            match (&(_field_0).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "offset",
                            match (&(_field_0).offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "user_id",
                            match (&(_field_0).user_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "before_created_at",
                            match (&(_field_0).before_created_at).as_ref() {
                                Some(item) => serde_json::json!((item).unix_timestamp()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "before_id",
                            match (&(_field_0).before_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::GetCreditAccount {
                user_id: _field_user_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GetCreditAccount".to_owned()),
                ),
                (
                    "user_id",
                    serde_json::Value::String((_field_user_id).to_string()),
                ),
            ]),
            Self::ListCreditLedger(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListCreditLedger".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "limit",
                            match (&(_field_0).limit).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "offset",
                            match (&(_field_0).offset).as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "user_id",
                            match (&(_field_0).user_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "before_created_at",
                            match (&(_field_0).before_created_at).as_ref() {
                                Some(item) => serde_json::json!((item).unix_timestamp()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "before_id",
                            match (&(_field_0).before_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::GrantCredit {
                user_id: _field_user_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("GrantCredit".to_owned()),
                ),
                (
                    "user_id",
                    serde_json::Value::String((_field_user_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "amount",
                            match (&(_field_body).amount).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "reason",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).reason).len()),
                            )]),
                        ),
                        (
                            "source_type",
                            match (&(_field_body).source_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_id",
                            match (&(_field_body).source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "idempotency_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).idempotency_key).len()),
                            )]),
                        ),
                        (
                            "metadata",
                            match (&(_field_body).metadata).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ChargeCredit {
                user_id: _field_user_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ChargeCredit".to_owned()),
                ),
                (
                    "user_id",
                    serde_json::Value::String((_field_user_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "amount",
                            match (&(_field_body).amount).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "reason",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).reason).len()),
                            )]),
                        ),
                        (
                            "source_type",
                            match (&(_field_body).source_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_id",
                            match (&(_field_body).source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "idempotency_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).idempotency_key).len()),
                            )]),
                        ),
                        (
                            "metadata",
                            match (&(_field_body).metadata).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::AdjustCredit {
                user_id: _field_user_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("AdjustCredit".to_owned()),
                ),
                (
                    "user_id",
                    serde_json::Value::String((_field_user_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "amount",
                            match (&(_field_body).amount).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "reason",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).reason).len()),
                            )]),
                        ),
                        (
                            "source_type",
                            match (&(_field_body).source_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_id",
                            match (&(_field_body).source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "idempotency_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).idempotency_key).len()),
                            )]),
                        ),
                        (
                            "metadata",
                            match (&(_field_body).metadata).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::EnableCharge {
                user_id: _field_user_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("EnableCharge".to_owned()),
                ),
                (
                    "user_id",
                    serde_json::Value::String((_field_user_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "amount",
                            match (&(_field_body).amount).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "reason",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).reason).len()),
                            )]),
                        ),
                        (
                            "source_type",
                            match (&(_field_body).source_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_id",
                            match (&(_field_body).source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "idempotency_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).idempotency_key).len()),
                            )]),
                        ),
                        (
                            "metadata",
                            match (&(_field_body).metadata).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::DisableCharge {
                user_id: _field_user_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DisableCharge".to_owned()),
                ),
                (
                    "user_id",
                    serde_json::Value::String((_field_user_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "amount",
                            match (&(_field_body).amount).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "reason",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).reason).len()),
                            )]),
                        ),
                        (
                            "source_type",
                            match (&(_field_body).source_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_id",
                            match (&(_field_body).source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "idempotency_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).idempotency_key).len()),
                            )]),
                        ),
                        (
                            "metadata",
                            match (&(_field_body).metadata).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::RefundCredit {
                user_id: _field_user_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RefundCredit".to_owned()),
                ),
                (
                    "user_id",
                    serde_json::Value::String((_field_user_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "amount",
                            match (&(_field_body).amount).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "reason",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).reason).len()),
                            )]),
                        ),
                        (
                            "source_type",
                            match (&(_field_body).source_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "source_id",
                            match (&(_field_body).source_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "idempotency_key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).idempotency_key).len()),
                            )]),
                        ),
                        (
                            "metadata",
                            match (&(_field_body).metadata).as_ref() {
                                Some(item) => mp::json_summary(item),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-billing-input";
    const CONTRACT_VERSION: &'static str = "1";
}
