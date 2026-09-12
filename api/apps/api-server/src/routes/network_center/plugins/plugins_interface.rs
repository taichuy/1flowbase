use std::{collections::HashMap, sync::Arc};

use control_plane::{
    network_egress::NetworkEgressProviderService,
    network_egress_secret::ProviderRegistryNetworkEgressSecretResolver,
    plugin_management::{
        DeletePluginFamilyCommand, InstallResolvedOfficialPluginCommand,
        InstallUploadedPluginCommand, PluginCatalogFilter, PluginManagementService,
    },
    ports::OfficialPluginSourcePort,
};
use interface_runtime::{InterfaceContract, UserPrincipal};
use storage_durable_postgres::MainDurableStore;

use super::*;
use crate::{
    host_infrastructure::CacheStore,
    official_extension_catalog::OfficialExtensionCatalogSourcePort,
    provider_runtime::{ApiProviderRuntime, ApiRuntimeServices},
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError, ConsoleLocaleHints,
    },
};

pub(crate) enum NetworkPluginInput {
    ListOfficial {
        query: OfficialPluginCatalogQuery,
        locale: ConsoleLocaleHints,
    },
    ListFamilies {
        locale: ConsoleLocaleHints,
    },
    SwitchVersion {
        provider_code: String,
        body: SwitchNetworkEgressPluginVersionBody,
        locale: ConsoleLocaleHints,
    },
    UninstallVersion {
        provider_code: String,
        installation_id: String,
        locale: ConsoleLocaleHints,
    },
    UninstallFamily {
        provider_code: String,
        locale: ConsoleLocaleHints,
    },
    InstallOfficial(InstallOfficialPluginBody),
    InstallUploaded {
        file_name: String,
        package_bytes: Vec<u8>,
    },
}

impl InterfaceContract for NetworkPluginInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListOfficial")),
                (
                    "query",
                    mp::object_schema(&[
                        (
                            "plugin_type",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "locale",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "q",
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
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListFamilies"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SwitchVersion")),
                ("provider_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("installation_id", mp::text_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UninstallVersion")),
                ("provider_code", mp::text_schema()),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UninstallFamily")),
                ("provider_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("InstallOfficial")),
                (
                    "0",
                    mp::object_schema(&[
                        ("plugin_id", mp::text_schema()),
                        (
                            "compatibility_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_current_host_version",mp::text_schema()), ("acknowledged_minimum_host_version",mp::text_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "risk_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("InstallUploaded")),
                (
                    "file_name",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "package_bytes",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListOfficial {
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListOfficial".to_owned()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "plugin_type",
                            match (&(_field_query).plugin_type).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "locale",
                            match (&(_field_query).locale).as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "q",
                            match (&(_field_query).q).as_ref() {
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
                    ]),
                ),
            ]),
            Self::ListFamilies { .. } => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ListFamilies".to_owned()),
            )]),
            Self::SwitchVersion {
                provider_code: _field_provider_code,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("SwitchVersion".to_owned()),
                ),
                ("provider_code", mp::text(_field_provider_code)?),
                (
                    "body",
                    mp::object_value(&[(
                        "installation_id",
                        mp::text(&(_field_body).installation_id)?,
                    )]),
                ),
            ]),
            Self::UninstallVersion {
                provider_code: _field_provider_code,
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UninstallVersion".to_owned()),
                ),
                ("provider_code", mp::text(_field_provider_code)?),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::UninstallFamily {
                provider_code: _field_provider_code,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UninstallFamily".to_owned()),
                ),
                ("provider_code", mp::text(_field_provider_code)?),
            ]),
            Self::InstallOfficial(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("InstallOfficial".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("plugin_id", mp::text(&(_field_0).plugin_id)?),
                        (
                            "compatibility_override",
                            match (&(_field_0).compatibility_override).as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).reason).len()),
                                        )]),
                                    ),
                                    (
                                        "acknowledged_current_host_version",
                                        mp::text(&(item).acknowledged_current_host_version)?,
                                    ),
                                    (
                                        "acknowledged_minimum_host_version",
                                        mp::text(&(item).acknowledged_minimum_host_version)?,
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "risk_override",
                            match (&(_field_0).risk_override).as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).reason).len()),
                                        )]),
                                    ),
                                    (
                                        "acknowledged_warnings",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).acknowledged_warnings).len()),
                                        )]),
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::InstallUploaded {
                file_name: _field_file_name,
                package_bytes: _field_package_bytes,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("InstallUploaded".to_owned()),
                ),
                (
                    "file_name",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((_field_file_name).len()),
                    )]),
                ),
                (
                    "package_bytes",
                    mp::object_value(&[(
                        "byte_count",
                        serde_json::json!((_field_package_bytes).len()),
                    )]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-network-plugin-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[expect(
    clippy::large_enum_variant,
    reason = "the typed plugin output is projected immediately into the console response"
)]
pub(crate) enum NetworkPluginOutput {
    Official(NetworkEgressOfficialPluginCatalogResponse),
    Families(Vec<NetworkEgressPluginFamilyResponse>),
    Installed(InstallPluginResponse),
    Empty,
}

impl InterfaceContract for NetworkPluginOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Official")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "source_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_label",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "source_freshness",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
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
                        (
                            "page",
                            mp::object_schema(&[
                                ("limit", serde_json::json!({"type":"integer"})),
                                (
                                    "next_cursor",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("plugin_id",mp::text_schema()), ("plugin_type",mp::text_schema()), ("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("icon",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("protocol",mp::text_schema()), ("current_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("latest_version",mp::text_schema()), ("has_update",serde_json::json!({"type":"boolean"})), ("minimum_host_version",mp::text_schema()), ("current_host_version",mp::text_schema()), ("compatibility_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("compatibility_warning_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("selected_artifact",mp::object_schema(&[("os",mp::object_schema(&[("byte_count",mp::count_schema())])), ("arch",mp::object_schema(&[("byte_count",mp::count_schema())])), ("libc",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("rust_target",mp::object_schema(&[("byte_count",mp::count_schema())])), ("checksum",mp::object_schema(&[("byte_count",mp::count_schema())])), ("signature_algorithm",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signing_key_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])), ("model_discovery_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("install_status",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Families")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_installation_id",mp::text_schema()), ("current_version",mp::text_schema()), ("can_uninstall",serde_json::json!({"type":"boolean"})), ("installed_versions",serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("installation_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("is_current",serde_json::json!({"type":"boolean"})), ("can_uninstall",serde_json::json!({"type":"boolean"}))])}))])}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Installed")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "installation",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                ("provider_code", mp::text_schema()),
                                (
                                    "runtime_slot",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("plugin_id", mp::text_schema()),
                                ("plugin_version", mp::text_schema()),
                                ("contract_version", mp::text_schema()),
                                ("protocol", mp::text_schema()),
                                (
                                    "display_name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "source_kind",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "trust_level",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "verification_status",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "desired_state",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "expected_checksum",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "signature_status",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "signature_algorithm",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "signing_key_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "local_artifact",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("node_id",mp::text_schema()), ("installation_id",mp::text_schema()), ("local_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("local_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("package_path",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("manifest_fingerprint",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("artifact_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("runtime_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("availability_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("checked_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("last_error",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))]), {"type":"null"}]}),
                                ),
                                ("metadata_json", mp::json_summary_schema()),
                                ("created_at", mp::text_schema()),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                        (
                            "task",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                (
                                    "installation_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "workspace_id",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                ("provider_code", mp::text_schema()),
                                (
                                    "task_kind",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("status", mp::text_schema()),
                                (
                                    "status_message",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                ("detail_json", mp::json_summary_schema()),
                                ("created_at", mp::text_schema()),
                                ("updated_at", mp::text_schema()),
                                (
                                    "finished_at",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Empty"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Official(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Official".to_owned())), ("0",mp::object_value(&[("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_kind).len()))])), ("source_label",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_label).len()))])), ("source_freshness",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_freshness).len()))])), ("locale_meta",mp::object_value(&[("requested_locale",match (&(&(_field_0).locale_meta).requested_locale).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("resolved_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).resolved_locale).len()))])), ("source",match &(&(_field_0).locale_meta).source {crate::routes::settings_group::system::LocaleSourceResponse::Query => mp::object_value(&[("variant",serde_json::Value::String("Query".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::ExplicitHeader => mp::object_value(&[("variant",serde_json::Value::String("ExplicitHeader".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::UserPreferredLocale => mp::object_value(&[("variant",serde_json::Value::String("UserPreferredLocale".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::AcceptLanguage => mp::object_value(&[("variant",serde_json::Value::String("AcceptLanguage".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::Fallback => mp::object_value(&[("variant",serde_json::Value::String("Fallback".to_owned()))])}), ("fallback_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).fallback_locale).len()))])), ("supported_locales",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).locale_meta).supported_locales).len()))]))])), ("page",mp::object_value(&[("limit",serde_json::json!(*(&(&(_field_0).page).limit))), ("next_cursor",match (&(&(_field_0).page).next_cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("plugin_id",mp::text(&(item).plugin_id)?), ("plugin_type",mp::text(&(item).plugin_type)?), ("provider_code",mp::text(&(item).provider_code)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).display_name).len()))])), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon",match (&(item).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("protocol",mp::text(&(item).protocol)?), ("current_version",match (&(item).current_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("latest_version",mp::text(&(item).latest_version)?), ("has_update",serde_json::Value::Bool(*(&(item).has_update))), ("minimum_host_version",mp::text(&(item).minimum_host_version)?), ("current_host_version",mp::text(&(item).current_host_version)?), ("compatibility_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).compatibility_status).len()))])), ("compatibility_warning_reason",match (&(item).compatibility_warning_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("selected_artifact",mp::object_value(&[("os",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).os).len()))])), ("arch",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).arch).len()))])), ("libc",match (&(&(item).selected_artifact).libc).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rust_target",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).rust_target).len()))])), ("checksum",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).checksum).len()))])), ("signature_algorithm",match (&(&(item).selected_artifact).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(&(item).selected_artifact).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })])), ("model_discovery_mode",mp::object_value(&[("byte_count",serde_json::json!((&(item).model_discovery_mode).len()))])), ("install_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).install_status).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Families(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Families".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("provider_code",mp::text(&(item).provider_code)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).display_name).len()))])), ("current_installation_id",mp::text(&(item).current_installation_id)?), ("current_version",mp::text(&(item).current_version)?), ("can_uninstall",serde_json::Value::Bool(*(&(item).can_uninstall))), ("installed_versions",{ if (&(item).installed_versions).len() > 32 { return None; } serde_json::Value::Array((&(item).installed_versions).iter().map(|item| Some(mp::object_value(&[("installation_id",mp::text(&(item).installation_id)?), ("plugin_version",mp::text(&(item).plugin_version)?), ("is_current",serde_json::Value::Bool(*(&(item).is_current))), ("can_uninstall",serde_json::Value::Bool(*(&(item).can_uninstall)))]))).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?) })]), Self::Installed(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Installed".to_owned())), ("0",mp::object_value(&[("installation",mp::object_value(&[("id",mp::text(&(&(_field_0).installation).id)?), ("provider_code",mp::text(&(&(_field_0).installation).provider_code)?), ("runtime_slot",match (&(&(_field_0).installation).runtime_slot).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("plugin_id",mp::text(&(&(_field_0).installation).plugin_id)?), ("plugin_version",mp::text(&(&(_field_0).installation).plugin_version)?), ("contract_version",mp::text(&(&(_field_0).installation).contract_version)?), ("protocol",mp::text(&(&(_field_0).installation).protocol)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).display_name).len()))])), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).trust_level).len()))])), ("verification_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).verification_status).len()))])), ("desired_state",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).desired_state).len()))])), ("expected_checksum",match (&(&(_field_0).installation).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).signature_status).len()))])), ("signature_algorithm",match (&(&(_field_0).installation).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(&(_field_0).installation).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("local_artifact",match (&(&(_field_0).installation).local_artifact).as_ref() { Some(item) => mp::object_value(&[("node_id",mp::text(&(item).node_id)?), ("installation_id",mp::text(&(item).installation_id)?), ("local_version",match (&(item).local_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("local_checksum",match (&(item).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("package_path",match (&(item).package_path).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("manifest_fingerprint",match (&(item).manifest_fingerprint).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("artifact_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).artifact_status).len()))])), ("runtime_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).runtime_status).len()))])), ("availability_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).availability_status).len()))])), ("checked_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).checked_at).len()))])), ("last_error",match (&(item).last_error).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]), None => serde_json::Value::Null }), ("metadata_json",mp::json_summary(&(&(_field_0).installation).metadata_json)), ("created_at",mp::text(&(&(_field_0).installation).created_at)?), ("updated_at",mp::text(&(&(_field_0).installation).updated_at)?)])), ("task",mp::object_value(&[("id",mp::text(&(&(_field_0).task).id)?), ("installation_id",match (&(&(_field_0).task).installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("workspace_id",match (&(&(_field_0).task).workspace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("provider_code",mp::text(&(&(_field_0).task).provider_code)?), ("task_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).task).task_kind).len()))])), ("status",mp::text(&(&(_field_0).task).status)?), ("status_message",match (&(&(_field_0).task).status_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("detail_json",mp::json_summary(&(&(_field_0).task).detail_json)), ("created_at",mp::text(&(&(_field_0).task).created_at)?), ("updated_at",mp::text(&(&(_field_0).task).updated_at)?), ("finished_at",match (&(&(_field_0).task).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]))]), Self::Empty => mp::object_value(&[("variant",serde_json::Value::String("Empty".to_owned()))])})
    }

    const CONTRACT_ID: &'static str = "console-network-plugin-output";
    const CONTRACT_VERSION: &'static str = "1";
}

pub(crate) struct NetworkPluginDependencies {
    pub(crate) store: MainDurableStore,
    pub(crate) provider_runtime: Arc<ApiRuntimeServices>,
    pub(crate) official_plugin_source: Arc<dyn OfficialPluginSourcePort>,
    pub(crate) official_catalog_source: Arc<dyn OfficialExtensionCatalogSourcePort>,
    pub(crate) cache_store: Arc<dyn CacheStore>,
    pub(crate) provider_install_root: String,
    pub(crate) provider_secret_master_key: String,
    pub(crate) api_node_id: String,
    pub(crate) bootstrap_workspace_id: uuid::Uuid,
    pub(crate) allow_uploaded_host_extensions: bool,
}

struct NetworkPluginAdapter(NetworkPluginDependencies);

impl NetworkPluginAdapter {
    fn service(
        &self,
        actor: &domain::ActorContext,
        operation: &'static str,
    ) -> crate::app_state::ApiPluginManagementService {
        PluginManagementService::new(
            self.0.store.for_actor(actor.clone()),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            self.0.official_plugin_source.clone(),
            self.0.provider_install_root.clone(),
        )
        .with_node_id(self.0.api_node_id.clone())
        .with_allow_uploaded_host_extensions(self.0.allow_uploaded_host_extensions)
        .with_model_routing_cache_store(self.0.cache_store.clone())
        .for_network_egress_provider_console_operation(operation)
    }

    fn provider_service(&self) -> crate::app_state::ApiNetworkEgressProviderService {
        NetworkEgressProviderService::new(
            self.0.store.clone(),
            ApiProviderRuntime::new(self.0.provider_runtime.clone()),
            ProviderRegistryNetworkEgressSecretResolver::new(
                self.0.store.clone(),
                self.0.provider_secret_master_key.clone(),
            ),
            self.0.provider_secret_master_key.clone(),
            self.0.api_node_id.clone(),
        )
    }

    async fn preferred_locale(
        &self,
        principal: &UserPrincipal,
    ) -> Result<Option<String>, ApiError> {
        Ok(self
            .0
            .store
            .find_user_by_id(principal.actor().user_id)
            .await?
            .ok_or(control_plane::errors::ControlPlaneError::NotAuthenticated)?
            .preferred_locale)
    }

    async fn families(
        &self,
        principal: &UserPrincipal,
        locale: ConsoleLocaleHints,
        operation: &'static str,
    ) -> Result<HashMap<String, NetworkEgressPluginFamilyResponse>, ApiError> {
        let locale_meta = locale.resolve_meta(None, self.preferred_locale(principal).await?);
        let catalog = self
            .service(principal.actor(), operation)
            .list_catalog(
                principal.actor().user_id,
                PluginCatalogFilter {
                    plugin_type: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
                },
                requested_locales(&locale_meta),
            )
            .await?;
        Ok(project_plugin_families(catalog.entries)?)
    }

    async fn resolved_official_command(
        &self,
        actor_user_id: uuid::Uuid,
        workspace_id: uuid::Uuid,
        body: InstallOfficialPluginBody,
    ) -> Result<InstallResolvedOfficialPluginCommand, ApiError> {
        let mut cursor = None;
        let (entry, source_kind) = loop {
            let page = self
                .0
                .official_catalog_source
                .list_page_for_workspace(workspace_id, "runtime-extensions", cursor.as_deref())
                .await?;
            if let Some(entry) = page.entries.into_iter().find(|entry| {
                entry
                    .source
                    .metadata
                    .get("plugin_id")
                    .and_then(serde_json::Value::as_str)
                    == Some(body.plugin_id.as_str())
            }) {
                break (entry, page.source_kind);
            }
            let Some(next) = page.metadata.next_cursor else {
                return Err(
                    control_plane::errors::ControlPlaneError::NotFound("official_plugin").into(),
                );
            };
            cursor = Some(next);
        };
        let plugin_type = entry
            .source
            .metadata
            .get("plugin_type")
            .and_then(serde_json::Value::as_str)
            .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                "official_plugin_type",
            ))?
            .to_string();
        let downloaded = self
            .0
            .official_catalog_source
            .download_artifact_for_workspace(workspace_id, &entry)
            .await?;
        let expected_checksum = downloaded.descriptor.expected_checksum.clone().ok_or(
            control_plane::errors::ControlPlaneError::InvalidInput("official_plugin_checksum"),
        )?;
        Ok(InstallResolvedOfficialPluginCommand {
            actor_user_id,
            plugin_id: body.plugin_id,
            plugin_type,
            minimum_host_version: entry.host_version_requirement,
            source_kind,
            file_name: downloaded.file_name,
            package_bytes: downloaded.artifact_bytes,
            expected_checksum,
            compatibility_override: crate::routes::plugins::to_compatibility_override(
                body.compatibility_override,
            ),
            risk_override: crate::routes::plugins::to_risk_override(body.risk_override),
        })
    }

    async fn execute_inner(
        &self,
        principal: &UserPrincipal,
        input: NetworkPluginInput,
    ) -> Result<NetworkPluginOutput, ApiError> {
        let actor = principal.actor();
        match input {
            NetworkPluginInput::ListOfficial { query, locale } => {
                let locale_meta = locale.resolve_meta(
                    query.locale.clone(),
                    self.preferred_locale(principal).await?,
                );
                let local_catalog = self
                    .service(actor, "network_egress_plugins.official_catalog.view")
                    .list_catalog(
                        actor.user_id,
                        PluginCatalogFilter {
                            plugin_type: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
                        },
                        requested_locales(&locale_meta),
                    )
                    .await?;
                let installed = project_plugin_families(local_catalog.entries)?;
                let filter = official_filter(&query);
                let page = self
                    .0
                    .official_catalog_source
                    .search_for_workspace(
                        actor.current_workspace_id,
                        "runtime-extensions",
                        crate::official_extension_catalog::OfficialExtensionCatalogSearchQuery {
                            slot_code: Some(NETWORK_EGRESS_PROVIDER_PLUGIN_TYPE.to_string()),
                            q: filter.search_query,
                            limit: filter.limit,
                            cursor: query.cursor,
                        },
                    )
                    .await?;
                let entries = page
                    .entries
                    .into_iter()
                    .filter_map(|entry| {
                        match project_catalog_entry(
                            self.0.official_catalog_source.as_ref(),
                            entry,
                            &installed,
                        ) {
                            Ok(Some(entry)) => Some(Ok(entry)),
                            Ok(None) => None,
                            Err(error) => Some(Err(error)),
                        }
                    })
                    .collect::<anyhow::Result<Vec<_>>>()?;
                let locale = domain::CatalogLocale::new(locale_meta.resolved_locale.clone())
                    .expect("runtime profile resolves a supported locale");
                let source_label = crate::app_state::resolve_official_source_label_with(
                    &self.0.store,
                    self.0.bootstrap_workspace_id,
                    &locale,
                    &page.source_kind,
                    page.source_kind.clone(),
                )
                .await?;
                Ok(NetworkPluginOutput::Official(
                    NetworkEgressOfficialPluginCatalogResponse {
                        source_kind: page.source_kind,
                        source_label,
                        registry_url: page.snapshot_locator,
                        source_freshness: "fresh".to_string(),
                        locale_meta,
                        page: OfficialPluginCatalogPageResponse {
                            limit: filter.limit,
                            next_cursor: page.next_cursor,
                        },
                        entries,
                    },
                ))
            }
            NetworkPluginInput::ListFamilies { locale } => {
                let mut families = self
                    .families(principal, locale, "network_egress_plugins.families.view")
                    .await?;
                mark_referenced_versions_not_uninstallable(&self.0.store, &mut families).await?;
                Ok(NetworkPluginOutput::Families(
                    families.into_values().collect(),
                ))
            }
            NetworkPluginInput::SwitchVersion {
                provider_code,
                body,
                locale,
            } => {
                let installation_id: uuid::Uuid = body.installation_id.parse().map_err(|_| {
                    control_plane::errors::ControlPlaneError::InvalidInput("installation_id")
                })?;
                let families = self
                    .families(principal, locale, "network_egress_plugins.families.switch")
                    .await?;
                let family = families.get(&provider_code).ok_or(
                    control_plane::errors::ControlPlaneError::NotFound(
                        "network_egress_plugin_family",
                    ),
                )?;
                if !family
                    .installed_versions
                    .iter()
                    .any(|version| version.installation_id == installation_id.to_string())
                {
                    return Err(control_plane::errors::ControlPlaneError::InvalidInput(
                        "installation_id",
                    )
                    .into());
                }
                self.provider_service()
                    .activate_version(installation_id)
                    .await?;
                Ok(NetworkPluginOutput::Empty)
            }
            NetworkPluginInput::UninstallVersion {
                provider_code,
                installation_id,
                locale,
            } => {
                let installation_id: uuid::Uuid = installation_id.parse().map_err(|_| {
                    control_plane::errors::ControlPlaneError::InvalidInput("installation_id")
                })?;
                let families = self
                    .families(
                        principal,
                        locale,
                        "network_egress_plugins.families.uninstall",
                    )
                    .await?;
                let family = families.get(&provider_code).ok_or(
                    control_plane::errors::ControlPlaneError::NotFound(
                        "network_egress_plugin_family",
                    ),
                )?;
                let version = family
                    .installed_versions
                    .iter()
                    .find(|version| version.installation_id == installation_id.to_string())
                    .ok_or(control_plane::errors::ControlPlaneError::InvalidInput(
                        "installation_id",
                    ))?;
                if !version.can_uninstall {
                    return Err(control_plane::errors::ControlPlaneError::Conflict(
                        "network_egress_plugin_version_uninstall_blocked",
                    )
                    .into());
                }
                control_plane::plugin_management::ExtensionInstallationService::new(
                    self.0.store.clone(),
                    &self.0.provider_install_root,
                )
                .delete_local_installation(&self.0.api_node_id, installation_id)
                .await?;
                Ok(NetworkPluginOutput::Empty)
            }
            NetworkPluginInput::UninstallFamily {
                provider_code,
                locale,
            } => {
                let families = self
                    .families(
                        principal,
                        locale,
                        "network_egress_plugins.families.uninstall",
                    )
                    .await?;
                let family = families.get(&provider_code).ok_or(
                    control_plane::errors::ControlPlaneError::NotFound(
                        "network_egress_plugin_family",
                    ),
                )?;
                if self
                    .0
                    .store
                    .list_network_egress_providers()
                    .await?
                    .into_iter()
                    .filter_map(|provider| provider.extension_family)
                    .any(|provider_family| provider_family.artifact_id() == family.provider_code)
                {
                    return Err(control_plane::errors::ControlPlaneError::Conflict(
                        "network_egress_plugin_family_uninstall_blocked",
                    )
                    .into());
                }
                self.service(actor, "network_egress_plugins.families.uninstall")
                    .delete_family(DeletePluginFamilyCommand {
                        actor_user_id: actor.user_id,
                        provider_code,
                    })
                    .await?;
                Ok(NetworkPluginOutput::Empty)
            }
            NetworkPluginInput::InstallOfficial(body) => {
                let command = self
                    .resolved_official_command(actor.user_id, actor.current_workspace_id, body)
                    .await?;
                let result = self
                    .service(actor, "network_egress_plugins.install.official")
                    .install_resolved_official_plugin(command)
                    .await?;
                Ok(NetworkPluginOutput::Installed(to_install_response(result)))
            }
            NetworkPluginInput::InstallUploaded {
                file_name,
                package_bytes,
            } => {
                let result = self
                    .service(actor, "network_egress_plugins.install.upload")
                    .install_uploaded_network_egress_provider(InstallUploadedPluginCommand {
                        actor_user_id: actor.user_id,
                        file_name,
                        package_bytes,
                    })
                    .await?;
                Ok(NetworkPluginOutput::Installed(to_install_response(result)))
            }
        }
    }
}

impl ConsoleInterfacePort<NetworkPluginInput, NetworkPluginOutput> for NetworkPluginAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: NetworkPluginInput,
    ) -> ConsoleInterfaceFuture<'a, NetworkPluginOutput> {
        Box::pin(async move {
            self.execute_inner(principal, input)
                .await
                .map_err(ConsoleInterfaceTargetError)
        })
    }
}

const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_plugins.official_catalog.view",
        binding_id: "http.console.network-egress-plugins.official-catalog.v1",
        method: "GET",
        path: "/api/console/settings/network-center/proxy-plugins/official-catalog",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_plugins.families.view",
        binding_id: "http.console.network-egress-plugins.families.v1",
        method: "GET",
        path: "/api/console/settings/network-center/proxy-plugins/families",
        mutating: false,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_plugins.families.switch",
        binding_id: "http.console.network-egress-plugins.switch-version.v1",
        method: "POST",
        path: "/api/console/settings/network-center/proxy-plugins/families/:provider_code/switch-version",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_plugins.families.uninstall",
        binding_id: "http.console.network-egress-plugins.uninstall-version.v1",
        method: "DELETE",
        path: "/api/console/settings/network-center/proxy-plugins/families/:provider_code/versions/:installation_id",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_plugins.families.uninstall",
        binding_id: "http.console.network-egress-plugins.uninstall-family.v1",
        method: "DELETE",
        path: "/api/console/settings/network-center/proxy-plugins/families/:provider_code",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_plugins.install.official",
        binding_id: "http.console.network-egress-plugins.install-official.v1",
        method: "POST",
        path: "/api/console/settings/network-center/proxy-plugins/install-official",
        mutating: true,
    },
    ConsoleInterfaceDeclaration {
        interface_id: "network_egress_plugins.install.upload",
        binding_id: "http.console.network-egress-plugins.install-upload.v1",
        method: "POST",
        path: "/api/console/settings/network-center/proxy-plugins/install-upload",
        mutating: true,
    },
];

pub(crate) fn compile_registry(
    dependencies: NetworkPluginDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    console_interface::compile_registry(
        "api-server.console-network-plugins",
        "graph:console-network-plugins-v1",
        DECLARATIONS,
        Arc::new(NetworkPluginAdapter(dependencies)),
    )
}
