use std::sync::Arc;

use control_plane::ports::CacheStore;
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::routes::console_interface::{
    self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
    ConsoleInterfaceTargetError,
};

pub(crate) struct ProviderLocaleHints {
    explicit: Option<String>,
    accept_language: Option<String>,
}

impl ProviderLocaleHints {
    pub(crate) fn from_headers(headers: &HeaderMap) -> Self {
        Self {
            explicit: headers
                .get("x-1flowbase-locale")
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
            accept_language: headers
                .get(ACCEPT_LANGUAGE)
                .and_then(|value| value.to_str().ok())
                .map(str::to_string),
        }
    }

    pub(crate) fn resolve(
        &self,
        query_locale: Option<String>,
        preferred_locale: Option<String>,
    ) -> LocaleMetaResponse {
        runtime_profile::resolve_locale(runtime_profile::LocaleResolutionInput {
            query_locale,
            explicit_header_locale: self.explicit.clone(),
            user_preferred_locale: preferred_locale,
            accept_language: self.accept_language.clone(),
            fallback_locale: runtime_profile::FALLBACK_LOCALE,
            supported_locales: runtime_profile::SUPPORTED_LOCALES
                .iter()
                .map(|value| value.to_string())
                .collect(),
        })
        .into()
    }
}

pub(crate) enum ProviderCatalogLogsInput {
    Catalog {
        query: ModelProviderCatalogQuery,
        locale: ProviderLocaleHints,
    },
    ListLogs(ModelProviderRequestLogsQuery),
    DeleteLogs(DeleteModelProviderRequestLogsBody),
    ClearLogs(serde_json::Value),
}

impl InterfaceContract for ProviderCatalogLogsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "query",
                    mp::object_schema(&[(
                        "locale",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListLogs")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "flow_run_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "user_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "application_name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "provider_instance_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "model_id",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "status",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        ("zero_output_only", serde_json::json!({"type":"boolean"})),
                        (
                            "started_after",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "started_before",
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
                ("variant", mp::tag_schema("DeleteLogs")),
                (
                    "0",
                    mp::object_schema(&[(
                        "attempt_ids",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ClearLogs")),
                ("0", mp::json_summary_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Catalog {
                query: _field_query,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Catalog".to_owned())),
                (
                    "query",
                    mp::object_value(&[(
                        "locale",
                        match (&(_field_query).locale).as_ref() {
                            Some(item) => mp::text(item)?,
                            None => serde_json::Value::Null,
                        },
                    )]),
                ),
            ]),
            Self::ListLogs(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("ListLogs".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "flow_run_id",
                            match (&(_field_0).flow_run_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
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
                            "application_name",
                            match (&(_field_0).application_name).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "provider_instance_id",
                            match (&(_field_0).provider_instance_id).as_ref() {
                                Some(item) => serde_json::Value::String((item).to_string()),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "model_id",
                            match (&(_field_0).model_id).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "status",
                            match (&(_field_0).status).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "zero_output_only",
                            serde_json::Value::Bool(*(&(_field_0).zero_output_only)),
                        ),
                        (
                            "started_after",
                            match (&(_field_0).started_after).as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "started_before",
                            match (&(_field_0).started_before).as_ref() {
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
            Self::DeleteLogs(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteLogs".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[("attempt_ids", {
                        if (&(_field_0).attempt_ids).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_0).attempt_ids)
                                .iter()
                                .map(|item| Some(mp::text(item)?))
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::ClearLogs(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("ClearLogs".to_owned())),
                ("0", mp::json_summary(_field_0)),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-provider-catalog-logs-input";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) enum ProviderCatalogLogsOutput {
    Catalog(ModelProviderCatalogResponse),
    Logs(ModelProviderRequestLogsPageResponse),
    Deleted(DeleteModelProviderRequestLogsResponse),
    Cleared(ClearModelProviderRequestLogsResponse),
}

impl InterfaceContract for ProviderCatalogLogsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Catalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "locale_meta",
                            mp::object_schema(&[
                                (
                                    "requested_locale",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "resolved_locale",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "source",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[("variant", mp::tag_schema("Query"))]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("ExplicitHeader"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("UserPreferredLocale"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("AcceptLanguage"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("Fallback"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "fallback_locale",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "supported_locales",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        ("i18n_catalog", mp::json_summary_schema()),
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("installation_id",mp::text_schema()), ("provider_code",mp::text_schema()), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("plugin_type",mp::text_schema()), ("namespace",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("protocol",mp::text_schema()), ("model_discovery_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("desired_state",mp::object_schema(&[("byte_count",mp::count_schema())])), ("availability_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("form_schema",mp::object_schema(&[("item_count",mp::count_schema())])), ("auth",serde_json::json!({"anyOf": [mp::object_schema(&[("actions",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]})), ("operational_capabilities",mp::object_schema(&[("item_count",mp::count_schema())])), ("predefined_models",mp::object_schema(&[("item_count",mp::count_schema())])), ("catalog_refresh_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("catalog_last_error_message",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_refreshed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Logs")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "items",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("attempt_id",mp::text_schema()), ("flow_run_id",mp::text_schema()), ("node_run_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("user_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("application_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("conversation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("application_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("attempt_index",serde_json::json!({"type":"integer"})), ("is_retry",serde_json::json!({"type":"boolean"})), ("retry_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("provider_instance_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("provider_instance_display_name",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("provider_code",mp::text_schema()), ("plugin_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("protocol",mp::text_schema()), ("upstream_model_id",mp::text_schema()), ("pricing_provider_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("pricing_model_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("total_cost",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("currency_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("billing_status",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("reasoning_effort",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("status",mp::text_schema()), ("error_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("input_cache_hit_rate",serde_json::json!({"anyOf": [serde_json::json!({"type":"number"}), {"type":"null"}]})), ("started_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("total_duration_ms",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}))])}),
                        ),
                        ("total_count", serde_json::json!({"type":"integer"})),
                        ("page", serde_json::json!({"type":"integer"})),
                        ("page_size", serde_json::json!({"type":"integer"})),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Deleted")),
                (
                    "0",
                    mp::object_schema(&[("deleted_count", serde_json::json!({"type":"integer"}))]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Cleared")),
                (
                    "0",
                    mp::object_schema(&[
                        ("deleted_count", serde_json::json!({"type":"integer"})),
                        ("has_more", serde_json::json!({"type":"boolean"})),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Catalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Catalog".to_owned())), ("0",mp::object_value(&[("locale_meta",mp::object_value(&[("requested_locale",match (&(&(_field_0).locale_meta).requested_locale).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("resolved_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).resolved_locale).len()))])), ("source",match &(&(_field_0).locale_meta).source {crate::routes::settings_group::system::LocaleSourceResponse::Query => mp::object_value(&[("variant",serde_json::Value::String("Query".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::ExplicitHeader => mp::object_value(&[("variant",serde_json::Value::String("ExplicitHeader".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::UserPreferredLocale => mp::object_value(&[("variant",serde_json::Value::String("UserPreferredLocale".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::AcceptLanguage => mp::object_value(&[("variant",serde_json::Value::String("AcceptLanguage".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::Fallback => mp::object_value(&[("variant",serde_json::Value::String("Fallback".to_owned()))])}), ("fallback_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).fallback_locale).len()))])), ("supported_locales",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).locale_meta).supported_locales).len()))]))])), ("i18n_catalog",mp::json_summary(&(_field_0).i18n_catalog)), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("installation_id",mp::text(&(item).installation_id)?), ("provider_code",mp::text(&(item).provider_code)?), ("plugin_id",mp::text(&(item).plugin_id)?), ("plugin_version",mp::text(&(item).plugin_version)?), ("plugin_type",mp::text(&(item).plugin_type)?), ("namespace",mp::object_value(&[("byte_count",serde_json::json!((&(item).namespace).len()))])), ("label_key",mp::object_value(&[("byte_count",serde_json::json!((&(item).label_key).len()))])), ("description_key",match (&(item).description_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).display_name).len()))])), ("protocol",mp::text(&(item).protocol)?), ("model_discovery_mode",mp::object_value(&[("byte_count",serde_json::json!((&(item).model_discovery_mode).len()))])), ("desired_state",mp::object_value(&[("byte_count",serde_json::json!((&(item).desired_state).len()))])), ("availability_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).availability_status).len()))])), ("form_schema",mp::object_value(&[("item_count",serde_json::json!((&(item).form_schema).len()))])), ("auth",match (&(item).auth).as_ref() { Some(item) => mp::object_value(&[("actions",mp::object_value(&[("item_count",serde_json::json!((&(item).actions).len()))]))]), None => serde_json::Value::Null }), ("operational_capabilities",mp::object_value(&[("item_count",serde_json::json!((&(item).operational_capabilities).len()))])), ("predefined_models",mp::object_value(&[("item_count",serde_json::json!((&(item).predefined_models).len()))])), ("catalog_refresh_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).catalog_refresh_status).len()))])), ("catalog_last_error_message",match (&(item).catalog_last_error_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("catalog_refreshed_at",match (&(item).catalog_refreshed_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Logs(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Logs".to_owned())), ("0",mp::object_value(&[("items",{ if (&(_field_0).items).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).items).iter().map(|item| Some(mp::object_value(&[("attempt_id",mp::text(&(item).attempt_id)?), ("flow_run_id",mp::text(&(item).flow_run_id)?), ("node_run_id",match (&(item).node_run_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("user_id",match (&(item).user_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("application_id",match (&(item).application_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("conversation_id",match (&(item).conversation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("application_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).application_name).len()))])), ("attempt_index",serde_json::json!(*(&(item).attempt_index))), ("is_retry",serde_json::Value::Bool(*(&(item).is_retry))), ("retry_reason",match (&(item).retry_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("provider_instance_id",match (&(item).provider_instance_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("provider_instance_display_name",match (&(item).provider_instance_display_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("provider_code",mp::text(&(item).provider_code)?), ("plugin_id",match (&(item).plugin_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("protocol",mp::text(&(item).protocol)?), ("upstream_model_id",mp::text(&(item).upstream_model_id)?), ("pricing_provider_code",match (&(item).pricing_provider_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("pricing_model_id",match (&(item).pricing_model_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("total_cost",match (&(item).total_cost).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("currency_code",match (&(item).currency_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("billing_status",match (&(item).billing_status).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("reasoning_effort",match (&(item).reasoning_effort).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("status",mp::text(&(item).status)?), ("error_code",match (&(item).error_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("input_cache_hit_rate",match (&(item).input_cache_hit_rate).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("started_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).started_at).len()))])), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("total_duration_ms",match (&(item).total_duration_ms).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("total_count",serde_json::json!(*(&(_field_0).total_count))), ("page",serde_json::json!(*(&(_field_0).page))), ("page_size",serde_json::json!(*(&(_field_0).page_size)))]))]), Self::Deleted(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Deleted".to_owned())), ("0",mp::object_value(&[("deleted_count",serde_json::json!(*(&(_field_0).deleted_count)))]))]), Self::Cleared(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Cleared".to_owned())), ("0",mp::object_value(&[("deleted_count",serde_json::json!(*(&(_field_0).deleted_count))), ("has_more",serde_json::Value::Bool(*(&(_field_0).has_more)))]))])})
    }

    const CONTRACT_ID: &'static str = "console-provider-catalog-logs-output";
    const CONTRACT_VERSION: &'static str = "1";
}

struct ProviderCatalogLogsAdapter {
    store: MainDurableStore,
    provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    secret_key: String,
    api_node_id: String,
    install_root: String,
    cache_store: Arc<dyn CacheStore>,
}

impl ProviderCatalogLogsAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation_id: &'static str,
    ) -> crate::app_state::ApiModelProviderService {
        ModelProviderService::for_console_operation(
            self.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.provider_runtime.clone()),
            self.secret_key.clone(),
            domain::ConsolePolicyGroup::settings_feature("system.model-providers")
                .expect("compiled model-provider settings group must be valid"),
            operation_id,
        )
        .with_node_artifact_context(self.api_node_id.clone(), self.install_root.clone())
        .with_routing_cache_store(self.cache_store.clone())
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: ProviderCatalogLogsInput,
    ) -> Result<ProviderCatalogLogsOutput, ApiError> {
        let actor = principal.actor().clone();
        match input {
            ProviderCatalogLogsInput::Catalog { query, locale } => {
                let preferred_locale = self
                    .store
                    .find_user_by_id(actor.user_id)
                    .await?
                    .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
                    .preferred_locale;
                let locale_meta = locale.resolve(query.locale, preferred_locale);
                let catalog = self
                    .service(&actor, "model_providers.catalog.view")
                    .list_catalog(actor.user_id, requested_locales(&locale_meta))
                    .await?;
                Ok(ProviderCatalogLogsOutput::Catalog(
                    to_catalog_view_response(locale_meta, catalog),
                ))
            }
            ProviderCatalogLogsInput::ListLogs(query) => {
                let page = self
                    .service(&actor, "model_providers.request_logs.view")
                    .list_request_logs(ListModelProviderRequestLogsCommand {
                        actor,
                        flow_run_id: query.flow_run_id,
                        user_id: query.user_id,
                        application_name: query.application_name,
                        provider_instance_id: query.provider_instance_id,
                        model_id: query.model_id,
                        status: query.status,
                        zero_output_only: query.zero_output_only,
                        started_after: query
                            .started_after
                            .as_deref()
                            .map(parse_rfc3339_time)
                            .transpose()?,
                        started_before: query
                            .started_before
                            .as_deref()
                            .map(parse_rfc3339_time)
                            .transpose()?,
                        page: query.page.unwrap_or(1),
                        page_size: query.page_size.unwrap_or(20),
                    })
                    .await?;
                Ok(ProviderCatalogLogsOutput::Logs(
                    ModelProviderRequestLogsPageResponse {
                        items: page
                            .items
                            .into_iter()
                            .map(to_request_log_response)
                            .collect(),
                        total_count: page.total_count,
                        page: page.page,
                        page_size: page.page_size,
                    },
                ))
            }
            ProviderCatalogLogsInput::DeleteLogs(body) => {
                let attempt_ids = body
                    .attempt_ids
                    .iter()
                    .map(|attempt_id| parse_uuid(attempt_id, "attempt_ids"))
                    .collect::<Result<Vec<_>, _>>()?;
                let deleted_count =
                    self.service(&actor, "model_providers.request_logs.delete")
                        .delete_selected_request_logs(
                            DeleteSelectedModelProviderRequestLogsCommand { actor, attempt_ids },
                        )
                        .await?;
                Ok(ProviderCatalogLogsOutput::Deleted(
                    DeleteModelProviderRequestLogsResponse { deleted_count },
                ))
            }
            ProviderCatalogLogsInput::ClearLogs(body) => {
                let body: ClearModelProviderRequestLogsBody = serde_json::from_value(body)
                    .map_err(|_| {
                        control_plane::errors::ControlPlaneError::InvalidInput("clear_request_logs")
                    })?;
                let workspace_id = actor.current_workspace_id;
                let continuation = match body.continuation_token.as_deref() {
                    Some(token) => ClearModelProviderRequestLogsContinuation::Continue {
                        snapshot_created_before: clear_request_log_continuation::verify(
                            &self.secret_key,
                            workspace_id,
                            token,
                        )?,
                    },
                    None => ClearModelProviderRequestLogsContinuation::Start,
                };
                let result = self
                    .service(&actor, "model_providers.request_logs.clear")
                    .clear_request_logs_batch(ClearModelProviderRequestLogsBatchCommand {
                        actor,
                        continuation,
                    })
                    .await?;
                let continuation_token = clear_request_log_continuation::issue(
                    &self.secret_key,
                    workspace_id,
                    result.snapshot_created_before,
                )?;
                Ok(ProviderCatalogLogsOutput::Cleared(
                    ClearModelProviderRequestLogsResponse {
                        deleted_count: result.deleted_count,
                        has_more: result.has_more,
                        continuation_token,
                    },
                ))
            }
        }
    }
}

impl ConsoleInterfacePort<ProviderCatalogLogsInput, ProviderCatalogLogsOutput>
    for ProviderCatalogLogsAdapter
{
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: ProviderCatalogLogsInput,
    ) -> ConsoleInterfaceFuture<'a, ProviderCatalogLogsOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.catalog.view",
        binding_id: "http.console.model-providers.catalog.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.request_logs.view",
        binding_id: "http.console.model-providers.request-logs.view.v1",
        method: "GET",
        path: "/api/console/settings/model-providers/request-logs",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.request_logs.delete",
        binding_id: "http.console.model-providers.request-logs.delete.v1",
        method: "DELETE",
        path: "/api/console/settings/model-providers/request-logs",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "model_providers.request_logs.clear",
        binding_id: "http.console.model-providers.request-logs.clear.v1",
        method: "POST",
        path: "/api/console/settings/model-providers/request-logs/clear",
        mutating: true,
    },
];

pub(crate) struct ProviderCatalogLogsDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<crate::provider_runtime::ApiRuntimeServices>,
    pub(crate) secret_key: String,
    pub(crate) api_node_id: String,
    pub(crate) install_root: String,
    pub(crate) cache_store: Arc<dyn CacheStore>,
}

pub(crate) fn compile_registry(
    dependencies: ProviderCatalogLogsDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-provider-catalog-logs",
        "graph:console-provider-catalog-logs-v1",
        DECLARATIONS,
        Arc::new(ProviderCatalogLogsAdapter {
            store: dependencies.store,
            provider_runtime: dependencies.provider_runtime,
            secret_key: dependencies.secret_key,
            api_node_id: dependencies.api_node_id,
            install_root: dependencies.install_root,
            cache_store: dependencies.cache_store,
        }),
    )
}
