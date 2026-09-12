use std::sync::Arc;

use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) enum ProviderInstanceLifecycleInput {
    List,
    Create(CreateModelProviderBody),
    Update {
        id: String,
        body: UpdateModelProviderBody,
    },
    Validate {
        id: String,
    },
    Delete {
        id: String,
    },
}

impl InterfaceContract for ProviderInstanceLifecycleInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("List"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Create")),
                (
                    "0",
                    mp::object_schema(&[
                        ("installation_id", mp::text_schema()),
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "configured_models",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("supports_multimodal",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("pricing_provider_code",mp::text_schema()), ("pricing_model_id",mp::text_schema())])}),
                        ),
                        (
                            "enabled_model_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        (
                            "included_in_main",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                        ("config", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Update")),
                ("id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "display_name",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "configured_models",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("supports_multimodal",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("pricing_provider_code",mp::text_schema()), ("pricing_model_id",mp::text_schema())])}),
                        ),
                        (
                            "enabled_model_ids",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                        ),
                        ("included_in_main", serde_json::json!({"type":"boolean"})),
                        ("config", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Validate")),
                ("id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Delete")),
                ("id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::List => {
                mp::object_value(&[("variant", serde_json::Value::String("List".to_owned()))])
            }
            Self::Create(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Create".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("installation_id", mp::text(&(_field_0).installation_id)?),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        ("configured_models", {
                            if (&(_field_0).configured_models).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).configured_models)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("model_id", mp::text(&(item).model_id)?),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "supports_multimodal",
                                                match (&(item).supports_multimodal).as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "pricing_provider_code",
                                                mp::text(&(item).pricing_provider_code)?,
                                            ),
                                            (
                                                "pricing_model_id",
                                                mp::text(&(item).pricing_model_id)?,
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("enabled_model_ids", {
                            if (&(_field_0).enabled_model_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).enabled_model_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "included_in_main",
                            match (&(_field_0).included_in_main).as_ref() {
                                Some(item) => serde_json::Value::Bool(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("config", mp::json_summary(&(_field_0).config)),
                    ]),
                ),
            ]),
            Self::Update {
                id: _field_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Update".to_owned())),
                ("id", mp::text(_field_id)?),
                (
                    "body",
                    mp::object_value(&[
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_body).display_name).len()),
                            )]),
                        ),
                        ("configured_models", {
                            if (&(_field_body).configured_models).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).configured_models)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("model_id", mp::text(&(item).model_id)?),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "supports_multimodal",
                                                match (&(item).supports_multimodal).as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "pricing_provider_code",
                                                mp::text(&(item).pricing_provider_code)?,
                                            ),
                                            (
                                                "pricing_model_id",
                                                mp::text(&(item).pricing_model_id)?,
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("enabled_model_ids", {
                            if (&(_field_body).enabled_model_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_body).enabled_model_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "included_in_main",
                            serde_json::Value::Bool(*(&(_field_body).included_in_main)),
                        ),
                        ("config", mp::json_summary(&(_field_body).config)),
                    ]),
                ),
            ]),
            Self::Validate { id: _field_id, .. } => mp::object_value(&[
                ("variant", serde_json::Value::String("Validate".to_owned())),
                ("id", mp::text(_field_id)?),
            ]),
            Self::Delete { id: _field_id, .. } => mp::object_value(&[
                ("variant", serde_json::Value::String("Delete".to_owned())),
                ("id", mp::text(_field_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-provider-instance-lifecycle-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ProviderInstanceLifecycleOutput {
    Instances(Vec<ModelProviderInstanceResponse>),
    Instance(ModelProviderInstanceResponse),
    Validation(ValidateModelProviderResponse),
    Deleted(DeletedResponse),
}

impl InterfaceContract for ProviderInstanceLifecycleOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Instances")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("installation_id",mp::text_schema()), ("provider_code",mp::text_schema()), ("protocol",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("included_in_main",serde_json::json!({"type":"boolean"})), ("config_json",mp::json_summary_schema()), ("configured_models",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("supports_multimodal",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("pricing_provider_code",mp::text_schema()), ("pricing_model_id",mp::text_schema())])})), ("enabled_model_ids",serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()})), ("catalog_refresh_status",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_last_error_message",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_refreshed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("model_count",serde_json::json!({"type":"integer"}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Instance")),
                (
                    "0",
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
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("model_id",mp::text_schema()), ("enabled",serde_json::json!({"type":"boolean"})), ("supports_multimodal",serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]})), ("pricing_provider_code",mp::text_schema()), ("pricing_model_id",mp::text_schema())])}),
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
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Validation")),
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
                        ("output", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Deleted")),
                (
                    "0",
                    mp::object_schema(&[("deleted", serde_json::json!({"type":"boolean"}))]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Instances(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Instances".to_owned())),
                ("0", {
                    if (_field_0).len() > 32 {
                        return None;
                    }
                    serde_json::Value::Array(
                        (_field_0)
                            .iter()
                            .map(|item| {
                                Some(mp::object_value(&[
                                    ("id", mp::text(&(item).id)?),
                                    ("installation_id", mp::text(&(item).installation_id)?),
                                    ("provider_code", mp::text(&(item).provider_code)?),
                                    ("protocol", mp::text(&(item).protocol)?),
                                    (
                                        "display_name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).display_name).len()),
                                        )]),
                                    ),
                                    ("status", mp::text(&(item).status)?),
                                    (
                                        "included_in_main",
                                        serde_json::Value::Bool(*(&(item).included_in_main)),
                                    ),
                                    ("config_json", mp::json_summary(&(item).config_json)),
                                    ("configured_models", {
                                        if (&(item).configured_models).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).configured_models)
                                                .iter()
                                                .map(|item| {
                                                    Some(mp::object_value(&[
                                                        ("model_id", mp::text(&(item).model_id)?),
                                                        (
                                                            "enabled",
                                                            serde_json::Value::Bool(
                                                                *(&(item).enabled),
                                                            ),
                                                        ),
                                                        (
                                                            "supports_multimodal",
                                                            match (&(item).supports_multimodal)
                                                                .as_ref()
                                                            {
                                                                Some(item) => {
                                                                    serde_json::Value::Bool(*(item))
                                                                }
                                                                None => serde_json::Value::Null,
                                                            },
                                                        ),
                                                        (
                                                            "pricing_provider_code",
                                                            mp::text(
                                                                &(item).pricing_provider_code,
                                                            )?,
                                                        ),
                                                        (
                                                            "pricing_model_id",
                                                            mp::text(&(item).pricing_model_id)?,
                                                        ),
                                                    ]))
                                                })
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                    ("enabled_model_ids", {
                                        if (&(item).enabled_model_ids).len() > 32 {
                                            return None;
                                        }
                                        serde_json::Value::Array(
                                            (&(item).enabled_model_ids)
                                                .iter()
                                                .map(|item| Some(mp::text(item)?))
                                                .collect::<Option<Vec<_>>>()?,
                                        )
                                    }),
                                    (
                                        "catalog_refresh_status",
                                        match (&(item).catalog_refresh_status).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "catalog_last_error_message",
                                        match (&(item).catalog_last_error_message).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "catalog_refreshed_at",
                                        match (&(item).catalog_refreshed_at).as_ref() {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    ("model_count", serde_json::json!(*(&(item).model_count))),
                                ]))
                            })
                            .collect::<Option<Vec<_>>>()?,
                    )
                }),
            ]),
            Self::Instance(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Instance".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("id", mp::text(&(_field_0).id)?),
                        ("installation_id", mp::text(&(_field_0).installation_id)?),
                        ("provider_code", mp::text(&(_field_0).provider_code)?),
                        ("protocol", mp::text(&(_field_0).protocol)?),
                        (
                            "display_name",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).display_name).len()),
                            )]),
                        ),
                        ("status", mp::text(&(_field_0).status)?),
                        (
                            "included_in_main",
                            serde_json::Value::Bool(*(&(_field_0).included_in_main)),
                        ),
                        ("config_json", mp::json_summary(&(_field_0).config_json)),
                        ("configured_models", {
                            if (&(_field_0).configured_models).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).configured_models)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("model_id", mp::text(&(item).model_id)?),
                                            (
                                                "enabled",
                                                serde_json::Value::Bool(*(&(item).enabled)),
                                            ),
                                            (
                                                "supports_multimodal",
                                                match (&(item).supports_multimodal).as_ref() {
                                                    Some(item) => serde_json::Value::Bool(*(item)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "pricing_provider_code",
                                                mp::text(&(item).pricing_provider_code)?,
                                            ),
                                            (
                                                "pricing_model_id",
                                                mp::text(&(item).pricing_model_id)?,
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("enabled_model_ids", {
                            if (&(_field_0).enabled_model_ids).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).enabled_model_ids)
                                    .iter()
                                    .map(|item| Some(mp::text(item)?))
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "catalog_refresh_status",
                            match (&(_field_0).catalog_refresh_status).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "catalog_last_error_message",
                            match (&(_field_0).catalog_last_error_message).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "catalog_refreshed_at",
                            match (&(_field_0).catalog_refreshed_at).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        ("model_count", serde_json::json!(*(&(_field_0).model_count))),
                    ]),
                ),
            ]),
            Self::Validation(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("Validation".to_owned()),
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
                        ("output", mp::json_summary(&(_field_0).output)),
                    ]),
                ),
            ]),
            Self::Deleted(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("Deleted".to_owned())),
                (
                    "0",
                    mp::object_value(&[(
                        "deleted",
                        serde_json::Value::Bool(*(&(_field_0).deleted)),
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-provider-instance-lifecycle-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct ProviderInstanceLifecycleDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}

struct ProviderInstanceLifecycleAdapter(ProviderInstanceLifecycleDependencies);

impl ProviderInstanceLifecycleAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation_id: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        ModelProviderService::for_console_operation(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.secret_key.clone(),
            domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                .expect("compiled model-provider settings group must be valid"),
            operation_id,
        )
        .with_node_artifact_context(self.0.api_node_id.clone(), self.0.install_root.clone())
        .with_routing_cache_store(self.0.cache_store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderInstanceLifecycleInput,
    ) -> Result<ProviderInstanceLifecycleOutput, ApiError> {
        let actor = principal.actor().clone();
        let user_id = actor.user_id;
        match input {
            ProviderInstanceLifecycleInput::List => {
                let instances = self
                    .service(&actor, "model_providers.instances.view")
                    .list_instances(user_id)
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Instances(
                    instances.into_iter().map(to_instance_response).collect(),
                ))
            }
            ProviderInstanceLifecycleInput::Create(body) => {
                let created = self
                    .service(&actor, "model_providers.instances.create")
                    .create_instance(CreateModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        installation_id: parse_uuid(&body.installation_id, "installation_id")?,
                        display_name: body.display_name,
                        config_json: body.config,
                        configured_models: configured_models(body.configured_models),
                        enabled_model_ids: body.enabled_model_ids,
                        included_in_main: body.included_in_main,
                        preview_token: body
                            .preview_token
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "preview_token"))
                            .transpose()?,
                    })
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Instance(
                    to_instance_response(created),
                ))
            }
            ProviderInstanceLifecycleInput::Update { id, body } => {
                let updated = self
                    .service(&actor, "model_providers.instances.update")
                    .update_instance(UpdateModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                        display_name: body.display_name,
                        config_json: body.config,
                        configured_models: configured_models(body.configured_models),
                        enabled_model_ids: body.enabled_model_ids,
                        included_in_main: body.included_in_main,
                        preview_token: body
                            .preview_token
                            .as_deref()
                            .map(|raw| parse_uuid(raw, "preview_token"))
                            .transpose()?,
                    })
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Instance(
                    to_instance_response(updated),
                ))
            }
            ProviderInstanceLifecycleInput::Validate { id } => {
                let result = self
                    .service(&actor, "model_providers.instances.validate")
                    .validate_instance(user_id, parse_uuid(&id, "id")?)
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Validation(
                    to_validate_response(result),
                ))
            }
            ProviderInstanceLifecycleInput::Delete { id } => {
                self.service(&actor, "model_providers.instances.delete")
                    .delete_instance(DeleteModelProviderInstanceCommand {
                        actor_user_id: user_id,
                        instance_id: parse_uuid(&id, "id")?,
                    })
                    .await?;
                Ok(ProviderInstanceLifecycleOutput::Deleted(DeletedResponse {
                    deleted: true,
                }))
            }
        }
    }
}

fn configured_models(
    models: Vec<ConfiguredModelBody>,
) -> Vec<domain::ModelProviderConfiguredModel> {
    models
        .into_iter()
        .map(|model| domain::ModelProviderConfiguredModel {
            model_id: model.model_id,
            enabled: model.enabled,
            context_window_override_tokens: model.context_window_override_tokens,
            supports_multimodal: model.supports_multimodal,
            pricing_provider_code: model.pricing_provider_code,
            pricing_model_id: model.pricing_model_id,
        })
        .collect()
}

impl ConsoleInterfacePort<ProviderInstanceLifecycleInput, ProviderInstanceLifecycleOutput>
    for ProviderInstanceLifecycleAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderInstanceLifecycleInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderInstanceLifecycleOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.view",
        binding_id: "http.console.model-providers.instances.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/instances",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.create",
        binding_id: "http.console.model-providers.instances.create.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.update",
        binding_id: "http.console.model-providers.instances.update.v1",
        method: "PATCH",
        path: "/api/console/settings/model-providers/instances/:id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.validate",
        binding_id: "http.console.model-providers.instances.validate.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/instances/:id/validate",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.instances.delete",
        binding_id: "http.console.model-providers.instances.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/model-providers/instances/:id",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: ProviderInstanceLifecycleDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-instance-lifecycle",
        "graph:console-provider-instance-lifecycle-v1",
        DECLARATIONS,
        Arc::new(ProviderInstanceLifecycleAdapter(dependencies)),
    )
}
