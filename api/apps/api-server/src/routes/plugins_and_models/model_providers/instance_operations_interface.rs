use std::sync::Arc;

use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ProviderInstanceOperationsInput {
    Authenticate {
        id: String,
        body: AuthenticateModelProviderInstanceBody,
    },
    Usage {
        id: String,
    },
    ResetCredits {
        id: String,
    },
    ConsumeResetCredit {
        id: String,
        body: ConsumeModelProviderResetCreditBody,
    },
    Balance {
        id: String,
    },
    Preview(PreviewModelProviderModelsBody),
    Reveal {
        id: String,
        body: RevealModelProviderSecretBody,
    },
}

impl InterfaceContract for ProviderInstanceOperationsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Authenticate")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("operation", mp::json_summary_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Usage")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ResetCredits")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ConsumeResetCredit")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "idempotency_key",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Balance")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Preview")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "installation_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "instance_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("config", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Reveal")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "key",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Authenticate {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Authenticate".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[("operation", mp::json_summary(&(_field_body).operation))]),
                ),
            ]),
            Self::Usage { id: _field_id, .. } => mp::object_value(&[
                ("variant", serde_json::Value::String("Usage".to_owned())),
                ("id", mp::text(_field_id)?),
            ]),
            Self::ResetCredits { id: _field_id, .. } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ResetCredits".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
            ]),
            Self::ConsumeResetCredit {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ConsumeResetCredit".to_owned()),
                ),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[(
                        "idempotency_key",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((&(_field_body).idempotency_key).len()),
                        )]),
                    )]),
                ),
            ]),
            Self::Balance { id: _field_id, .. } => mp::object_value(&[
                ("variant", serde_json::Value::String("Balance".to_owned())),
                ("id", mp::text(_field_id)?),
            ]),
            Self::Preview(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Preview".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "installation_id",
                            match (&(_field_0).installation_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "instance_id",
                            match (&(_field_0).instance_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("config", mp::json_summary(&(_field_0).config)),
                    ]),
                ),
            ]),
            Self::Reveal {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Reveal".to_owned())),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[(
                        "key",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((&(_field_body).key).len()),
                        )]),
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-provider-instance-operations-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed provider output is projected immediately into the console response"
)]
pub(crate) enum ProviderInstanceOperationsOutput {
    Authentication(AuthenticateModelProviderInstanceResponse),
    Usage(ModelProviderUsageWindowsResponse),
    ResetCredits(ModelProviderResetCreditCountResponse),
    Consumed(ConsumeModelProviderResetCreditResponse),
    Balance(ModelProviderBalanceResponse),
    Preview(PreviewModelProviderModelsResponse),
    Secret(RevealModelProviderSecretResponse),
}

impl InterfaceContract for ProviderInstanceOperationsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Authentication")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "instance",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                ("installation_id", mp::text_schema()),
                                ("provider_code", mp::text_schema()),
                                ("protocol", mp::text_schema()),
                                (
                                    "display_name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                ("included_in_main", serde_json::json!({"type":"boolean"})),
                                ("config_json", mp::json_summary_schema()),
                                (
                                    "configured_models",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "enabled_model_ids",
                                    serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                                ),
                                (
                                    "catalog_refresh_status",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "catalog_last_error_message",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "catalog_refreshed_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("model_count", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "message",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "user_action",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("kind",mp::text_schema()), ("user_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("expires_at",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("poll_interval_seconds",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("prompt",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Usage")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "windows",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("limit_window_seconds",serde_json::json!({"type":"integer"})), ("used_percent",serde_json::json!({"type":"number"})), ("reset_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "queried_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ResetCredits")),
                (
                    "0",
                    mp::object_schema(&[(
                        "available_count",
                        serde_json::json!({"type":"integer"}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Consumed")),
                (
                    "0",
                    mp::object_schema(&[("consumed", serde_json::json!({"type":"boolean"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Balance")),
                (
                    "0",
                    mp::object_schema(&[
                        ("is_available", serde_json::json!({"type":"boolean"})),
                        (
                            "balance_infos",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("currency",mp::object_schema(&[("byte_count",mp::count_schema())])), ("total_balance",mp::object_schema(&[("byte_count",mp::count_schema())])), ("granted_balance",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("topped_up_balance",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        ("provider_metadata", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Preview")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "models",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("namespace",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("label_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("description_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("display_name_fallback",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("source",mp::object_schema(&[("byte_count",mp::count_schema())])), ("supports_streaming",serde_json::json!({"type":"boolean"})), ("supports_tool_call",serde_json::json!({"type":"boolean"})), ("supports_multimodal",serde_json::json!({"type":"boolean"})), ("context_window",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("provider_metadata",mp::json_summary_schema())])}),
                        ),
                        ("expires_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Secret")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "key",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "value",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Authentication(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Authentication".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "instance",
                            mp::object_value(&[
                                ("id", mp::text(&(&(_field_0).instance).id)?),
                                (
                                    "installation_id",
                                    mp::text(&(&(_field_0).instance).installation_id)?,
                                ),
                                (
                                    "provider_code",
                                    mp::text(&(&(_field_0).instance).provider_code)?,
                                ),
                                ("protocol", mp::text(&(&(_field_0).instance).protocol)?),
                                (
                                    "display_name",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).instance).display_name).len()
                                        ),
                                    )]),
                                ),
                                ("status", mp::text(&(&(_field_0).instance).status)?),
                                (
                                    "included_in_main",
                                    serde_json::Value::Bool(
                                        *(&(&(_field_0).instance).included_in_main),
                                    ),
                                ),
                                (
                                    "config_json",
                                    mp::json_summary(&(&(_field_0).instance).config_json),
                                ),
                                (
                                    "configured_models",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!((&(&(_field_0).instance)
                                            .configured_models)
                                            .len()),
                                    )]),
                                ),
                                ("enabled_model_ids", {
                                    if (&(&(_field_0).instance).enabled_model_ids).len() > 32 {
                                        return None;
                                    }
                                    serde_json::Value::Array(
                                        (&(&(_field_0).instance).enabled_model_ids)
                                            .iter()
                                            .map(|item| Some(mp::text(item)?))
                                            .collect::<Option<Vec<_>>>()?,
                                    )
                                }),
                                (
                                    "catalog_refresh_status",
                                    match (&(&(_field_0).instance).catalog_refresh_status).as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "catalog_last_error_message",
                                    match (&(&(_field_0).instance).catalog_last_error_message)
                                        .as_ref()
                                    {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "catalog_refreshed_at",
                                    match (&(&(_field_0).instance).catalog_refreshed_at).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "model_count",
                                    serde_json::json!(*(&(&(_field_0).instance).model_count)),
                                ),
                            ]),
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "message",
                            match (&(_field_0).message).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "user_action",
                            match (&(_field_0).user_action).as_ref() {
                                Some(item) => mp::object_value(&[
                                    ("kind", mp::text(&(item).kind)?),
                                    (
                                        "user_code",
                                        match (&(item).user_code).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "expires_at",
                                        match (&(item).expires_at).as_ref() {
                                            Some(item) => mp::text(item)?,
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "poll_interval_seconds",
                                        match (&(item).poll_interval_seconds).as_ref() {
                                            Some(item) => serde_json::json!(*(item)),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "prompt",
                                        match (&(item).prompt).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::Usage(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Usage".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("windows", {
                            if (&(_field_0).windows).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).windows)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "limit_window_seconds",
                                                serde_json::json!(*(&(item).limit_window_seconds)),
                                            ),
                                            (
                                                "used_percent",
                                                serde_json::json!(*(&(item).used_percent)),
                                            ),
                                            (
                                                "reset_at",
                                                match (&(item).reset_at).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "queried_at",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).queried_at).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::ResetCredits(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ResetCredits".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "available_count",
                        serde_json::json!(*(&(_field_0).available_count)),
                    )]),
                ),
            ]),
            Self::Consumed(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Consumed".to_owned())),
                (
                    "0",
                    mp::object_value(&[(
                        "consumed",
                        serde_json::Value::Bool(*(&(_field_0).consumed)),
                    )]),
                ),
            ]),
            Self::Balance(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Balance".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "is_available",
                            serde_json::Value::Bool(*(&(_field_0).is_available)),
                        ),
                        ("balance_infos", {
                            if (&(_field_0).balance_infos).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).balance_infos)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "currency",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).currency).len()),
                                                )]),
                                            ),
                                            (
                                                "total_balance",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).total_balance).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "granted_balance",
                                                match (&(item).granted_balance).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "topped_up_balance",
                                                match (&(item).topped_up_balance).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "provider_metadata",
                            mp::json_summary(&(_field_0).provider_metadata),
                        ),
                    ]),
                ),
            ]),
            Self::Preview(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Preview".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("models", {
                            if (&(_field_0).models).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).models)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("model_id", mp::text(&(item).model_id)?),
                                            (
                                                "display_name",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).display_name).len()),
                                                )]),
                                            ),
                                            (
                                                "namespace",
                                                match (&(item).namespace).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "label_key",
                                                match (&(item).label_key).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "description_key",
                                                match (&(item).description_key).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "display_name_fallback",
                                                match (&(item).display_name_fallback).as_ref() {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "source",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).source).len()),
                                                )]),
                                            ),
                                            (
                                                "supports_streaming",
                                                serde_json::Value::Bool(
                                                    *(&(item).supports_streaming),
                                                ),
                                            ),
                                            (
                                                "supports_tool_call",
                                                serde_json::Value::Bool(
                                                    *(&(item).supports_tool_call),
                                                ),
                                            ),
                                            (
                                                "supports_multimodal",
                                                serde_json::Value::Bool(
                                                    *(&(item).supports_multimodal),
                                                ),
                                            ),
                                            (
                                                "context_window",
                                                match (&(item).context_window).as_ref() {
                                                    Some(item) => serde_json::json!(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "provider_metadata",
                                                mp::json_summary(&(item).provider_metadata),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("expires_at", mp::text(&(_field_0).expires_at)?),
                    ]),
                ),
            ]),
            Self::Secret(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Secret".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "key",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).key).len()),
                            )]),
                        ),
                        (
                            "value",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).value).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-provider-instance-operations-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderInstanceOperationsDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}

struct ProviderInstanceOperationsAdapter(ProviderInstanceOperationsDependencies);

impl ProviderInstanceOperationsAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        group: &'static str,
        operation: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        ModelProviderService::for_console_operation(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.secret_key.clone(),
            if group == "settings" {
                domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                    .expect("compiled model-provider settings group must be valid")
            } else {
                domain::ConsolePolicyGroup::other("other.model-providers")
                    .expect("compiled model-provider other group must be valid")
            },
            operation,
        )
        .with_node_artifact_context(self.0.api_node_id.clone(), self.0.install_root.clone())
        .with_routing_cache_store(self.0.cache_store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderInstanceOperationsInput,
    ) -> Result<ProviderInstanceOperationsOutput, ApiError> {
        let actor = principal.actor().clone();
        let user_id = actor.user_id;
        match input {
            ProviderInstanceOperationsInput::Authenticate { id, body } => {
                let operation = serde_json::from_value::<ProviderAuthOperation>(body.operation)
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput(
                            "provider_auth_operation",
                        )
                    })?;
                let result = self
                    .service(&actor, "settings", "model_providers.instances.authenticate")
                    .authenticate_instance(AuthenticateModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                        operation,
                    })
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Authentication(
                    to_authenticate_response(result),
                ))
            }
            ProviderInstanceOperationsInput::Usage { id } => {
                let result = self
                    .service(&actor, "settings", "model_providers.instances.usage.view")
                    .get_usage_windows(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Usage(
                    to_usage_windows_response(result),
                ))
            }
            ProviderInstanceOperationsInput::ResetCredits { id } => {
                let result = self
                    .service(
                        &actor,
                        "settings",
                        "model_providers.instances.reset_credits.view",
                    )
                    .count_reset_credits(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::ResetCredits(
                    to_reset_credit_count_response(result),
                ))
            }
            ProviderInstanceOperationsInput::ConsumeResetCredit { id, body } => {
                let result = self
                    .service(
                        &actor,
                        "settings",
                        "model_providers.instances.reset_credits.consume",
                    )
                    .consume_reset_credit(ConsumeModelProviderResetCreditCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                        idempotency_key: body.idempotency_key,
                    })
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Consumed(
                    to_consume_reset_credit_response(result),
                ))
            }
            ProviderInstanceOperationsInput::Balance { id } => {
                let result = self
                    .service(&actor, "other", "model_providers.balance.view")
                    .get_balance(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Balance(
                    to_balance_response(result),
                ))
            }
            ProviderInstanceOperationsInput::Preview(body) => {
                let preview = self
                    .service(&actor, "settings", "model_providers.preview.view")
                    .preview_models(PreviewModelProviderModelsCommand {
                        actor_user_id: user_id,
                        installation_id: body
                            .installation_id
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "installation_id"))
                            .transpose()?,
                        instance_id: body
                            .instance_id
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "instance_id"))
                            .transpose()?,
                        config_json: body.config,
                    })
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Preview(
                    PreviewModelProviderModelsResponse {
                        models: preview
                            .models
                            .into_iter()
                            .map(to_runtime_model_descriptor_response)
                            .collect(),
                        preview_token: preview.preview_token.to_string(),
                        expires_at: format_optional_time(Some(preview.expires_at))
                            .unwrap_or_default(),
                    },
                ))
            }
            ProviderInstanceOperationsInput::Reveal { id, body } => {
                let value = self
                    .service(
                        &actor,
                        "settings",
                        "model_providers.instances.secrets.reveal",
                    )
                    .reveal_secret(user_id, parse_uuid(&id, "id")?, &body.key)
                    .await?;
                Ok(ProviderInstanceOperationsOutput::Secret(
                    RevealModelProviderSecretResponse {
                        key: body.key,
                        value,
                    },
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ProviderInstanceOperationsInput, ProviderInstanceOperationsOutput>
    for ProviderInstanceOperationsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderInstanceOperationsInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderInstanceOperationsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.authenticate",
        binding_id: "http.console.model-providers.instances.authenticate.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/authenticate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.usage.view",
        binding_id: "http.console.model-providers.instances.usage.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances/:id/usage",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.reset_credits.view",
        binding_id: "http.console.model-providers.instances.reset-credits.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances/:id/reset-credits",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.reset_credits.consume",
        binding_id: "http.console.model-providers.instances.reset-credits.consume.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/reset-credits/consume",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.balance.view",
        binding_id: "http.console.model-providers.balance.view.v1",
        method: "GET",
        path: "/api/console/model-providers/:id/balance",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.preview.view",
        binding_id: "http.console.model-providers.preview.view.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/preview-models",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.secrets.reveal",
        binding_id: "http.console.model-providers.instances.secrets.reveal.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/secrets/reveal",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: ProviderInstanceOperationsDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-instance-operations",
        "graph:console-provider-instance-operations-v1",
        DECLARATIONS,
        Arc::new(ProviderInstanceOperationsAdapter(dependencies)),
    )
}
