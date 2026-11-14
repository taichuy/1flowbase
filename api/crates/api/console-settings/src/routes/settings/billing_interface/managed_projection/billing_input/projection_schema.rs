use super::*;

impl BillingInput {
    pub(super) fn managed_projection_schema_impl() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([("variant", mp::tag_schema("SyncPricingCatalog"))]),
            mp::object_schema([
                ("variant", mp::tag_schema("ListPricingRules")),
                (
                    "0",
                    mp::object_schema([
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
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
            mp::object_schema([
                ("variant", mp::tag_schema("CreatePricingRule")),
                (
                    "0",
                    mp::object_schema([
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        ("weekday_mask", serde_json::json!({"type":"integer"})),
                        (
                            "local_time_start",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "local_time_end",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("priority", serde_json::json!({"type":"integer"})),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("rules", mp::json_summary_schema()),
                        (
                            "source_kind",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "extensions",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("UpdatePricingRule")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        ("weekday_mask", serde_json::json!({"type":"integer"})),
                        (
                            "local_time_start",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "local_time_end",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("priority", serde_json::json!({"type":"integer"})),
                        ("enabled", serde_json::json!({"type":"boolean"})),
                        ("rules", mp::json_summary_schema()),
                        (
                            "source_kind",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
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
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "extensions",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("DeletePricingRule")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("GetPricingCatalog")),
                (
                    "0",
                    mp::object_schema([
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
            mp::object_schema([
                ("variant", mp::tag_schema("ImportPricingCatalog")),
                (
                    "0",
                    mp::object_schema([(
                        "catalog_ids",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                    )]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ListCreditAccounts")),
                (
                    "0",
                    mp::object_schema([
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
            mp::object_schema([
                ("variant", mp::tag_schema("GetCreditAccount")),
                ("user_id", mp::text_schema()),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ListCreditLedger")),
                (
                    "0",
                    mp::object_schema([
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
            mp::object_schema([
                ("variant", mp::tag_schema("GrantCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "reason",
                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ChargeCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "reason",
                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("AdjustCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "reason",
                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("EnableCharge")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "reason",
                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("DisableCharge")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "reason",
                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "metadata",
                            serde_json::json!({"anyOf": [mp::json_summary_schema(), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RefundCredit")),
                ("user_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema([
                        (
                            "amount",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "reason",
                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                            mp::object_schema([("byte_count", mp::count_schema())]),
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
}
