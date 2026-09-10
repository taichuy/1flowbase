use std::sync::Arc;

use axum::{
    Json, Router,
    extract::{Query, State},
    http::{HeaderMap, header::ACCEPT_LANGUAGE},
};
use control_plane::system_runtime::SystemRuntimeService;
use interface_runtime::{InterfaceContract, UserPrincipal};
use runtime_profile::{LocaleResolution, LocaleResolutionInput, LocaleSource, RuntimeProfile};
use serde::{Deserialize, Serialize};
use utoipa::{IntoParams, ToSchema};

use crate::{
    app_state::ApiState,
    error_response::ApiError,
    response::ApiSuccess,
    routes::console_interface::{
        self, ConsoleInterfaceDeclaration, ConsoleInterfaceFuture, ConsoleInterfacePort,
        ConsoleInterfaceTargetError,
    },
    routes::console_route_assembly::{ConsoleRouteAssembly, console_get},
    runtime_profile_client::RuntimeProfileSnapshotCache,
};

pub use super::release_status::{
    ConsoleReleaseInfoResponse, ConsoleReleaseStatusResponse, ConsoleReleaseUpgradeCommandsResponse,
};

#[derive(Debug, Deserialize, IntoParams)]
pub struct SystemRuntimeProfileQuery {
    pub locale: Option<String>,
}

pub(crate) enum SystemInterfaceInput {
    ReleaseStatus,
    RuntimeProfile {
        query_locale: Option<String>,
        explicit_header_locale: Option<String>,
        accept_language: Option<String>,
    },
}
impl InterfaceContract for SystemInterfaceInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("ReleaseStatus"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RuntimeProfile")),
                (
                    "query_locale",
                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                ),
                (
                    "explicit_header_locale",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                (
                    "accept_language",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ReleaseStatus => mp::object_value(&[(
                "variant",
                serde_json::Value::String("ReleaseStatus".to_owned()),
            )]),
            Self::RuntimeProfile {
                query_locale: _field_query_locale,
                explicit_header_locale: _field_explicit_header_locale,
                accept_language: _field_accept_language,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RuntimeProfile".to_owned()),
                ),
                (
                    "query_locale",
                    match (_field_query_locale).as_ref() {
                        Some(item) => mp::text(item)?,
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "explicit_header_locale",
                    match (_field_explicit_header_locale).as_ref() {
                        Some(item) => {
                            mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                        }
                        None => serde_json::Value::Null,
                    },
                ),
                (
                    "accept_language",
                    match (_field_accept_language).as_ref() {
                        Some(item) => {
                            mp::object_value(&[("byte_count", serde_json::json!((item).len()))])
                        }
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-system-input";
    const CONTRACT_VERSION: &'static str = "1";
}
pub(crate) enum SystemInterfaceOutput {
    ReleaseStatus(ConsoleReleaseStatusResponse),
    RuntimeProfile(SystemRuntimeProfileResponse),
}
impl InterfaceContract for SystemInterfaceOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ReleaseStatus")),
                (
                    "0",
                    mp::object_schema(&[
                        ("current_version", mp::text_schema()),
                        ("latest_version", mp::text_schema()),
                        ("has_update", serde_json::json!({"type":"boolean"})),
                        (
                            "release_info",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("body",mp::object_schema(&[("byte_count",mp::count_schema())])), ("published_at",mp::text_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "upgrade_commands",
                            mp::object_schema(&[
                                (
                                    "shell",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "powershell",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        ("cached", serde_json::json!({"type":"boolean"})),
                        (
                            "warning",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RuntimeProfile")),
                (
                    "0",
                    mp::object_schema(&[
                        ("api_node_id", mp::text_schema()),
                        (
                            "provider_install_root",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "host_extension_dropin_root",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "related_process_memory_complete",
                            serde_json::json!({"type":"boolean"}),
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
                            "topology",
                            mp::object_schema(&[(
                                "relationship",
                                mp::union_schema(vec![
                                    mp::object_schema(&[("variant", mp::tag_schema("SameHost"))]),
                                    mp::object_schema(&[("variant", mp::tag_schema("SplitHost"))]),
                                    mp::object_schema(&[(
                                        "variant",
                                        mp::tag_schema("RunnerUnreachable"),
                                    )]),
                                ]),
                            )]),
                        ),
                        (
                            "services",
                            mp::object_schema(&[
                                (
                                    "api_server",
                                    mp::object_schema(&[
                                        ("reachable", serde_json::json!({"type":"boolean"})),
                                        (
                                            "service",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "status",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "version",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "host_fingerprint",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "plugin_runner",
                                    mp::object_schema(&[
                                        ("reachable", serde_json::json!({"type":"boolean"})),
                                        (
                                            "service",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "status",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "version",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "host_fingerprint",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                            ]),
                        ),
                        (
                            "hosts",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("host_fingerprint",mp::object_schema(&[("byte_count",mp::count_schema())])), ("platform",mp::object_schema(&[("os",mp::object_schema(&[("byte_count",mp::count_schema())])), ("arch",mp::object_schema(&[("byte_count",mp::count_schema())])), ("libc",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("rust_target_triple",mp::object_schema(&[("byte_count",mp::count_schema())]))])), ("cpu",mp::object_schema(&[("logical_count",serde_json::json!({"type":"integer"}))])), ("memory",mp::object_schema(&[("total_bytes",serde_json::json!({"type":"integer"})), ("total_gb",serde_json::json!({"type":"number"})), ("available_bytes",serde_json::json!({"type":"integer"})), ("available_gb",serde_json::json!({"type":"number"})), ("process_bytes",serde_json::json!({"type":"integer"})), ("process_gb",serde_json::json!({"type":"number"}))])), ("related_process_bytes",serde_json::json!({"type":"integer"})), ("related_process_count",serde_json::json!({"type":"integer"})), ("services",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                        (
                            "runtime_targets",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("target_id",mp::text_schema()), ("reachable",serde_json::json!({"type":"boolean"})), ("host_fingerprint",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("metrics",serde_json::json!({"anyOf": [mp::object_schema(&[("captured_at_unix_milliseconds",serde_json::json!({"type":"integer"})), ("sample_interval_milliseconds",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}))]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ReleaseStatus(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ReleaseStatus".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("current_version", mp::text(&(_field_0).current_version)?),
                        ("latest_version", mp::text(&(_field_0).latest_version)?),
                        (
                            "has_update",
                            serde_json::Value::Bool(*(&(_field_0).has_update)),
                        ),
                        (
                            "release_info",
                            match (&(_field_0).release_info).as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "name",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).name).len()),
                                        )]),
                                    ),
                                    (
                                        "body",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((&(item).body).len()),
                                        )]),
                                    ),
                                    ("published_at", mp::text(&(item).published_at)?),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "upgrade_commands",
                            mp::object_value(&[
                                (
                                    "shell",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).upgrade_commands).shell).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "powershell",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).upgrade_commands).powershell).len()
                                        ),
                                    )]),
                                ),
                            ]),
                        ),
                        ("cached", serde_json::Value::Bool(*(&(_field_0).cached))),
                        (
                            "warning",
                            match (&(_field_0).warning).as_ref() {
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
            Self::RuntimeProfile(_field_0) => {
                mp::object_value(&[
                    (
                        "variant",
                        serde_json::Value::String("RuntimeProfile".to_owned()),
                    ),
                    (
                        "0",
                        mp::object_value(&[
                            ("api_node_id", mp::text(&(_field_0).api_node_id)?),
                            (
                                "provider_install_root",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((&(_field_0).provider_install_root).len()),
                                )]),
                            ),
                            (
                                "host_extension_dropin_root",
                                mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!(
                                        (&(_field_0).host_extension_dropin_root).len()
                                    ),
                                )]),
                            ),
                            (
                                "related_process_memory_complete",
                                serde_json::Value::Bool(
                                    *(&(_field_0).related_process_memory_complete),
                                ),
                            ),
                            (
                                "locale_meta",
                                mp::object_value(&[
                                    (
                                        "requested_locale",
                                        match (&(&(_field_0).locale_meta).requested_locale).as_ref()
                                        {
                                            Some(item) => mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "resolved_locale",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!(
                                                (&(&(_field_0).locale_meta).resolved_locale).len()
                                            ),
                                        )]),
                                    ),
                                    (
                                        "source",
                                        match &(&(_field_0).locale_meta).source {
                                            LocaleSourceResponse::Query => mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("Query".to_owned()),
                                            )]),
                                            LocaleSourceResponse::ExplicitHeader => {
                                                mp::object_value(&[(
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "ExplicitHeader".to_owned(),
                                                    ),
                                                )])
                                            }
                                            LocaleSourceResponse::UserPreferredLocale => {
                                                mp::object_value(&[(
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "UserPreferredLocale".to_owned(),
                                                    ),
                                                )])
                                            }
                                            LocaleSourceResponse::AcceptLanguage => {
                                                mp::object_value(&[(
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "AcceptLanguage".to_owned(),
                                                    ),
                                                )])
                                            }
                                            LocaleSourceResponse::Fallback => {
                                                mp::object_value(&[(
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "Fallback".to_owned(),
                                                    ),
                                                )])
                                            }
                                        },
                                    ),
                                    (
                                        "fallback_locale",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!(
                                                (&(&(_field_0).locale_meta).fallback_locale).len()
                                            ),
                                        )]),
                                    ),
                                    (
                                        "supported_locales",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!(
                                                (&(&(_field_0).locale_meta).supported_locales)
                                                    .len()
                                            ),
                                        )]),
                                    ),
                                ]),
                            ),
                            (
                                "topology",
                                mp::object_value(&[(
                                    "relationship",
                                    match &(&(_field_0).topology).relationship {
                                        SystemRuntimeRelationship::SameHost => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("SameHost".to_owned()),
                                            )])
                                        }
                                        SystemRuntimeRelationship::SplitHost => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String("SplitHost".to_owned()),
                                            )])
                                        }
                                        SystemRuntimeRelationship::RunnerUnreachable => {
                                            mp::object_value(&[(
                                                "variant",
                                                serde_json::Value::String(
                                                    "RunnerUnreachable".to_owned(),
                                                ),
                                            )])
                                        }
                                    },
                                )]),
                            ),
                            (
                                "services",
                                mp::object_value(&[
                                    (
                                        "api_server",
                                        mp::object_value(&[
                                            (
                                                "reachable",
                                                serde_json::Value::Bool(
                                                    *(&(&(&(_field_0).services).api_server)
                                                        .reachable),
                                                ),
                                            ),
                                            (
                                                "service",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(&(&(_field_0).services).api_server)
                                                            .service)
                                                            .len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "status",
                                                match (&(&(&(_field_0).services).api_server).status)
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "version",
                                                match (&(&(&(_field_0).services).api_server)
                                                    .version)
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "host_fingerprint",
                                                match (&(&(&(_field_0).services).api_server)
                                                    .host_fingerprint)
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((item).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                        ]),
                                    ),
                                    (
                                        "plugin_runner",
                                        mp::object_value(&[
                                            (
                                                "reachable",
                                                serde_json::Value::Bool(
                                                    *(&(&(&(_field_0).services).plugin_runner)
                                                        .reachable),
                                                ),
                                            ),
                                            (
                                                "service",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(&(&(_field_0).services).plugin_runner)
                                                            .service)
                                                            .len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "status",
                                                match (&(&(&(_field_0).services).plugin_runner)
                                                    .status)
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "version",
                                                match (&(&(&(_field_0).services).plugin_runner)
                                                    .version)
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "host_fingerprint",
                                                match (&(&(&(_field_0).services).plugin_runner)
                                                    .host_fingerprint)
                                                    .as_ref()
                                                {
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
                            ),
                            ("hosts", {
                                if (&(_field_0).hosts).len() > 32 {
                                    return None;
                                }
                                serde_json::Value::Array(
                                    (&(_field_0).hosts)
                                        .iter()
                                        .map(|item| {
                                            Some(mp::object_value(&[
                                                (
                                                    "host_fingerprint",
                                                    mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!(
                                                            (&(item).host_fingerprint).len()
                                                        ),
                                                    )]),
                                                ),
                                                (
                                                    "platform",
                                                    mp::object_value(&[
                                                        (
                                                            "os",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(&(item).platform).os).len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "arch",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(&(item).platform).arch)
                                                                        .len()
                                                                ),
                                                            )]),
                                                        ),
                                                        (
                                                            "libc",
                                                            match (&(&(item).platform).libc)
                                                                .as_ref()
                                                            {
                                                                Some(item) => {
                                                                    mp::object_value(&[(
                                                                        "byte_count",
                                                                        serde_json::json!(
                                                                            (item).len()
                                                                        ),
                                                                    )])
                                                                }
                                                                None => serde_json::Value::Null,
                                                            },
                                                        ),
                                                        (
                                                            "rust_target_triple",
                                                            mp::object_value(&[(
                                                                "byte_count",
                                                                serde_json::json!(
                                                                    (&(&(item).platform)
                                                                        .rust_target_triple)
                                                                        .len()
                                                                ),
                                                            )]),
                                                        ),
                                                    ]),
                                                ),
                                                (
                                                    "cpu",
                                                    mp::object_value(&[(
                                                        "logical_count",
                                                        serde_json::json!(
                                                            *(&(&(item).cpu).logical_count)
                                                        ),
                                                    )]),
                                                ),
                                                (
                                                    "memory",
                                                    mp::object_value(&[
                                                        (
                                                            "total_bytes",
                                                            serde_json::json!(
                                                                *(&(&(item).memory).total_bytes)
                                                            ),
                                                        ),
                                                        (
                                                            "total_gb",
                                                            serde_json::json!(
                                                                *(&(&(item).memory).total_gb)
                                                            ),
                                                        ),
                                                        (
                                                            "available_bytes",
                                                            serde_json::json!(
                                                                *(&(&(item).memory)
                                                                    .available_bytes)
                                                            ),
                                                        ),
                                                        (
                                                            "available_gb",
                                                            serde_json::json!(
                                                                *(&(&(item).memory).available_gb)
                                                            ),
                                                        ),
                                                        (
                                                            "process_bytes",
                                                            serde_json::json!(
                                                                *(&(&(item).memory).process_bytes)
                                                            ),
                                                        ),
                                                        (
                                                            "process_gb",
                                                            serde_json::json!(
                                                                *(&(&(item).memory).process_gb)
                                                            ),
                                                        ),
                                                    ]),
                                                ),
                                                (
                                                    "related_process_bytes",
                                                    serde_json::json!(
                                                        *(&(item).related_process_bytes)
                                                    ),
                                                ),
                                                (
                                                    "related_process_count",
                                                    serde_json::json!(
                                                        *(&(item).related_process_count)
                                                    ),
                                                ),
                                                (
                                                    "services",
                                                    mp::object_value(&[(
                                                        "item_count",
                                                        serde_json::json!((&(item).services).len()),
                                                    )]),
                                                ),
                                            ]))
                                        })
                                        .collect::<Option<Vec<_>>>()?,
                                )
                            }),
                            ("runtime_targets", {
                                if (&(_field_0).runtime_targets).len() > 32 {
                                    return None;
                                }
                                serde_json::Value::Array((&(_field_0).runtime_targets).iter().map(|item| Some(mp::object_value(&[("target_id",mp::text(&(item).target_id)?), ("reachable",serde_json::Value::Bool(*(&(item).reachable))), ("host_fingerprint",match (&(item).host_fingerprint).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("metrics",match (&(item).metrics).as_ref() { Some(item) => mp::object_value(&[("captured_at_unix_milliseconds",serde_json::json!(*(&(item).captured_at_unix_milliseconds))), ("sample_interval_milliseconds",match (&(item).sample_interval_milliseconds).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
                            }),
                        ]),
                    ),
                ])
            }
        })
    }

    const CONTRACT_ID: &'static str = "console-system-output";
    const CONTRACT_VERSION: &'static str = "1";
}
pub(crate) struct SystemInterfaceDependencies {
    pub(crate) store: storage_durable_postgres::MainDurableStore,
    pub(crate) profiles: RuntimeProfileSnapshotCache,
    pub(crate) api_node_id: String,
    pub(crate) provider_install_root: String,
    pub(crate) host_extension_dropin_root: String,
}
struct SystemInterfaceAdapter(SystemInterfaceDependencies);
impl ConsoleInterfacePort<SystemInterfaceInput, SystemInterfaceOutput> for SystemInterfaceAdapter {
    fn execute<'a>(
        &'a self,
        principal: &'a UserPrincipal,
        input: SystemInterfaceInput,
    ) -> ConsoleInterfaceFuture<'a, SystemInterfaceOutput> {
        Box::pin(async move {
            match input {
                SystemInterfaceInput::ReleaseStatus => Ok(SystemInterfaceOutput::ReleaseStatus(
                    super::release_status::fetch_console_release_status().await,
                )),
                SystemInterfaceInput::RuntimeProfile {
                    query_locale,
                    explicit_header_locale,
                    accept_language,
                } => {
                    let access = SystemRuntimeService::new(
                        self.0.store.for_actor(principal.actor().clone()),
                    )
                    .authorize_view(principal.actor().user_id)
                    .await
                    .map_err(ApiError::from)
                    .map_err(ConsoleInterfaceTargetError)?;
                    let locale = runtime_profile::resolve_locale(LocaleResolutionInput {
                        query_locale,
                        explicit_header_locale,
                        user_preferred_locale: access.preferred_locale,
                        accept_language,
                        fallback_locale: runtime_profile::FALLBACK_LOCALE,
                        supported_locales: runtime_profile::SUPPORTED_LOCALES
                            .iter()
                            .map(|value| value.to_string())
                            .collect(),
                    });
                    let profiles = self
                        .0
                        .profiles
                        .get_or_refresh()
                        .await
                        .map_err(ApiError::from)
                        .map_err(ConsoleInterfaceTargetError)?;
                    Ok(SystemInterfaceOutput::RuntimeProfile(
                        merge_runtime_profiles(
                            locale,
                            profiles.api_profile,
                            profiles.host_profile,
                            self.0.api_node_id.clone(),
                            self.0.provider_install_root.clone(),
                            self.0.host_extension_dropin_root.clone(),
                        ),
                    ))
                }
            }
        })
    }
}

pub(crate) fn compile_registry(
    dependencies: SystemInterfaceDependencies,
) -> Result<
    Arc<interface_runtime::CompiledInterfaceRegistry>,
    interface_runtime::RegistryCompilationError,
> {
    const DECLARATIONS: &[ConsoleInterfaceDeclaration] = &[
        ConsoleInterfaceDeclaration {
            interface_id: "system.runtime_profile.view",
            binding_id: "http.console.system.runtime-profile.get.v1",
            method: "GET",
            path: "/api/console/system/runtime-profile",
            mutating: false,
        },
        ConsoleInterfaceDeclaration {
            interface_id: "system.release_status.view",
            binding_id: "http.console.system.release-status.get.v1",
            method: "GET",
            path: "/api/console/system/release-status",
            mutating: false,
        },
    ];
    console_interface::compile_registry(
        "api-server.console-system",
        "graph:console-system-v1",
        DECLARATIONS,
        Arc::new(SystemInterfaceAdapter(dependencies)),
    )
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum LocaleSourceResponse {
    Query,
    ExplicitHeader,
    UserPreferredLocale,
    AcceptLanguage,
    Fallback,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct LocaleMetaResponse {
    pub requested_locale: Option<String>,
    pub resolved_locale: String,
    pub source: LocaleSourceResponse,
    pub fallback_locale: String,
    pub supported_locales: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SystemRuntimeRelationship {
    SameHost,
    SplitHost,
    RunnerUnreachable,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeTopologyResponse {
    pub relationship: SystemRuntimeRelationship,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeServiceResponse {
    pub reachable: bool,
    pub service: String,
    pub status: Option<String>,
    pub version: Option<String>,
    pub host_fingerprint: Option<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeServicesResponse {
    pub api_server: SystemRuntimeServiceResponse,
    pub plugin_runner: SystemRuntimeServiceResponse,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimePlatformResponse {
    pub os: String,
    pub arch: String,
    pub libc: Option<String>,
    pub rust_target_triple: String,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeCpuResponse {
    pub logical_count: u64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeMemoryResponse {
    pub total_bytes: u64,
    pub total_gb: f64,
    pub available_bytes: u64,
    pub available_gb: f64,
    pub process_bytes: u64,
    pub process_gb: f64,
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SystemRuntimeMetricAvailabilityResponse {
    Available,
    WarmingUp,
    Stale,
    Unavailable,
}

impl From<runtime_profile::RuntimeMetricAvailability> for SystemRuntimeMetricAvailabilityResponse {
    fn from(value: runtime_profile::RuntimeMetricAvailability) -> Self {
        match value {
            runtime_profile::RuntimeMetricAvailability::Available => Self::Available,
            runtime_profile::RuntimeMetricAvailability::WarmingUp => Self::WarmingUp,
            runtime_profile::RuntimeMetricAvailability::Stale => Self::Stale,
            runtime_profile::RuntimeMetricAvailability::Unavailable => Self::Unavailable,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
#[serde(rename_all = "snake_case")]
pub enum SystemRuntimeMetricScopeKindResponse {
    Cgroup,
    Host,
    RuntimeVisible,
}

impl From<runtime_profile::RuntimeMetricScopeKind> for SystemRuntimeMetricScopeKindResponse {
    fn from(value: runtime_profile::RuntimeMetricScopeKind) -> Self {
        match value {
            runtime_profile::RuntimeMetricScopeKind::Cgroup => Self::Cgroup,
            runtime_profile::RuntimeMetricScopeKind::Host => Self::Host,
            runtime_profile::RuntimeMetricScopeKind::RuntimeVisible => Self::RuntimeVisible,
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeCpuMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub usage_percent: Option<f64>,
    pub logical_count: u64,
    pub limit_cores: f64,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeMemoryMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub total_bytes: u64,
    pub available_bytes: u64,
    pub used_bytes: u64,
    pub process_bytes: u64,
    pub related_process_bytes: u64,
    pub related_process_count: u64,
    pub cgroup_composition: Option<SystemRuntimeCgroupMemoryCompositionResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeCgroupMemoryCompositionResponse {
    pub anonymous_bytes: Option<u64>,
    pub file_bytes: Option<u64>,
    pub kernel_bytes: Option<u64>,
    pub shared_memory_bytes: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeStorageMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub mount_point: Option<String>,
    pub file_system: Option<String>,
    pub total_bytes: Option<u64>,
    pub available_bytes: Option<u64>,
    pub used_bytes: Option<u64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeNetworkMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub received_bytes_per_second: Option<f64>,
    pub transmitted_bytes_per_second: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeDiskIoMetricsResponse {
    pub availability: SystemRuntimeMetricAvailabilityResponse,
    pub scope_kind: SystemRuntimeMetricScopeKindResponse,
    pub read_bytes_per_second: Option<f64>,
    pub written_bytes_per_second: Option<f64>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeMetricsResponse {
    pub captured_at_unix_milliseconds: i64,
    pub sample_interval_milliseconds: Option<u64>,
    pub cpu: SystemRuntimeCpuMetricsResponse,
    pub memory: SystemRuntimeMemoryMetricsResponse,
    pub storage: SystemRuntimeStorageMetricsResponse,
    pub network: SystemRuntimeNetworkMetricsResponse,
    pub disk_io: SystemRuntimeDiskIoMetricsResponse,
}

impl From<&runtime_profile::RuntimeMetricsSnapshot> for SystemRuntimeMetricsResponse {
    fn from(value: &runtime_profile::RuntimeMetricsSnapshot) -> Self {
        Self {
            captured_at_unix_milliseconds: value
                .captured_at
                .unix_timestamp()
                .saturating_mul(1_000)
                .saturating_add(i64::from(value.captured_at.millisecond())),
            sample_interval_milliseconds: value.sample_interval_milliseconds,
            cpu: SystemRuntimeCpuMetricsResponse {
                availability: value.cpu.availability.into(),
                scope_kind: value.cpu.scope_kind.into(),
                usage_percent: value.cpu.usage_percent,
                logical_count: value.cpu.logical_count,
                limit_cores: value.cpu.limit_cores,
            },
            memory: SystemRuntimeMemoryMetricsResponse {
                availability: value.memory.availability.into(),
                scope_kind: value.memory.scope_kind.into(),
                total_bytes: value.memory.total_bytes,
                available_bytes: value.memory.available_bytes,
                used_bytes: value.memory.used_bytes,
                process_bytes: value.memory.process_bytes,
                related_process_bytes: value.memory.related_process_bytes,
                related_process_count: value.memory.related_process_count,
                cgroup_composition: value.memory.cgroup_composition.as_ref().map(|composition| {
                    SystemRuntimeCgroupMemoryCompositionResponse {
                        anonymous_bytes: composition.anonymous_bytes,
                        file_bytes: composition.file_bytes,
                        kernel_bytes: composition.kernel_bytes,
                        shared_memory_bytes: composition.shared_memory_bytes,
                    }
                }),
            },
            storage: SystemRuntimeStorageMetricsResponse {
                availability: value.storage.availability.into(),
                scope_kind: value.storage.scope_kind.into(),
                mount_point: value.storage.mount_point.clone(),
                file_system: value.storage.file_system.clone(),
                total_bytes: value.storage.total_bytes,
                available_bytes: value.storage.available_bytes,
                used_bytes: value.storage.used_bytes,
            },
            network: SystemRuntimeNetworkMetricsResponse {
                availability: value.network.availability.into(),
                scope_kind: value.network.scope_kind.into(),
                received_bytes_per_second: value.network.received_bytes_per_second,
                transmitted_bytes_per_second: value.network.transmitted_bytes_per_second,
            },
            disk_io: SystemRuntimeDiskIoMetricsResponse {
                availability: value.disk_io.availability.into(),
                scope_kind: value.disk_io.scope_kind.into(),
                read_bytes_per_second: value.disk_io.read_bytes_per_second,
                written_bytes_per_second: value.disk_io.written_bytes_per_second,
            },
        }
    }
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeTargetResponse {
    pub target_id: String,
    pub reachable: bool,
    pub host_fingerprint: Option<String>,
    pub metrics: Option<SystemRuntimeMetricsResponse>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeHostResponse {
    pub host_fingerprint: String,
    pub platform: SystemRuntimePlatformResponse,
    pub cpu: SystemRuntimeCpuResponse,
    pub memory: SystemRuntimeMemoryResponse,
    pub related_process_bytes: u64,
    pub related_process_count: u64,
    pub services: Vec<String>,
}

#[derive(Debug, Serialize, ToSchema)]
pub struct SystemRuntimeProfileResponse {
    pub api_node_id: String,
    pub provider_install_root: String,
    pub host_extension_dropin_root: String,
    pub related_process_memory_complete: bool,
    pub locale_meta: LocaleMetaResponse,
    pub topology: SystemRuntimeTopologyResponse,
    pub services: SystemRuntimeServicesResponse,
    pub hosts: Vec<SystemRuntimeHostResponse>,
    pub runtime_targets: Vec<SystemRuntimeTargetResponse>,
}

pub fn router() -> Router<Arc<ApiState>> {
    route_assembly().into_router()
}

pub fn route_assembly() -> ConsoleRouteAssembly<Arc<ApiState>> {
    use access_control::ConsoleRouteOwnership::ConsoleOperation;

    ConsoleRouteAssembly::new()
        .route(
            "/system/runtime-profile",
            console_get(
                get_runtime_profile,
                ConsoleOperation("system.runtime_profile.view".to_string()),
            ),
        )
        .route(
            "/system/release-status",
            console_get(
                get_release_status,
                ConsoleOperation("system.release_status.view".to_string()),
            ),
        )
}

#[utoipa::path(
    get,
    path = "/api/console/system/release-status",
    responses(
        (status = 200, body = ConsoleReleaseStatusResponse),
        (status = 401, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_release_status(
    State(state): State<Arc<ApiState>>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<ConsoleReleaseStatusResponse>>, ApiError> {
    let snapshot_state = Arc::clone(&state);
    let SystemInterfaceOutput::ReleaseStatus(value) = console_interface::invoke(
        snapshot_state,
        "http.console.system.release-status.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        SystemInterfaceInput::ReleaseStatus,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

#[utoipa::path(
    get,
    path = "/api/console/system/runtime-profile",
    params(SystemRuntimeProfileQuery),
    responses(
        (status = 200, body = SystemRuntimeProfileResponse),
        (status = 401, body = crate::error_response::ErrorBody),
        (status = 403, body = crate::error_response::ErrorBody)
    )
)]
pub async fn get_runtime_profile(
    State(state): State<Arc<ApiState>>,
    Query(query): Query<SystemRuntimeProfileQuery>,
    headers: HeaderMap,
) -> Result<Json<ApiSuccess<SystemRuntimeProfileResponse>>, ApiError> {
    let input = SystemInterfaceInput::RuntimeProfile {
        query_locale: query.locale,
        explicit_header_locale: header_locale(&headers),
        accept_language: header_accept_language(&headers),
    };
    let snapshot_state = Arc::clone(&state);
    let SystemInterfaceOutput::RuntimeProfile(value) = console_interface::invoke(
        snapshot_state,
        "http.console.system.runtime-profile.get.v1",
        crate::extension_bus::ConsoleAuthenticationCredential::Protocol { state, headers },
        input,
    )
    .await?
    else {
        unreachable!()
    };
    Ok(Json(ApiSuccess::new(value)))
}

fn header_locale(headers: &HeaderMap) -> Option<String> {
    headers
        .get("x-1flowbase-locale")
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn header_accept_language(headers: &HeaderMap) -> Option<String> {
    headers
        .get(ACCEPT_LANGUAGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_string)
}

fn merge_runtime_profiles(
    locale_meta: LocaleResolution,
    api_profile: RuntimeProfile,
    host_profile: RuntimeProfile,
    api_node_id: String,
    provider_install_root: String,
    host_extension_dropin_root: String,
) -> SystemRuntimeProfileResponse {
    let hosts = vec![host_from_profiles(
        &api_profile,
        &[&api_profile],
        vec!["api-server", "runtime-extension-host"],
    )];
    let runtime_targets = vec![
        runtime_target_from_profile(&api_profile),
        runtime_target_from_profile(&host_profile),
    ];

    SystemRuntimeProfileResponse {
        api_node_id,
        provider_install_root,
        host_extension_dropin_root,
        related_process_memory_complete: true,
        locale_meta: locale_meta.into(),
        topology: SystemRuntimeTopologyResponse {
            relationship: SystemRuntimeRelationship::SameHost,
        },
        services: SystemRuntimeServicesResponse {
            api_server: service_from_profile(&api_profile),
            // Keep the established response key while projecting the real in-process service.
            plugin_runner: service_from_profile(&host_profile),
        },
        hosts,
        runtime_targets,
    }
}

fn runtime_target_from_profile(profile: &RuntimeProfile) -> SystemRuntimeTargetResponse {
    SystemRuntimeTargetResponse {
        target_id: profile.service.clone(),
        reachable: true,
        host_fingerprint: Some(profile.host_fingerprint.clone()),
        metrics: Some(SystemRuntimeMetricsResponse::from(&profile.metrics)),
    }
}

fn host_from_profiles(
    profile: &RuntimeProfile,
    related_profiles: &[&RuntimeProfile],
    services: Vec<&str>,
) -> SystemRuntimeHostResponse {
    let (related_process_bytes, related_process_count) =
        related_profiles
            .iter()
            .fold((0_u64, 0_u64), |(bytes, count), related_profile| {
                (
                    bytes.saturating_add(related_profile.metrics.memory.related_process_bytes),
                    count.saturating_add(related_profile.metrics.memory.related_process_count),
                )
            });
    SystemRuntimeHostResponse {
        host_fingerprint: profile.host_fingerprint.clone(),
        platform: SystemRuntimePlatformResponse {
            os: profile.platform.os.clone(),
            arch: profile.platform.arch.clone(),
            libc: profile.platform.libc.clone(),
            rust_target_triple: profile.platform.rust_target.clone(),
        },
        cpu: SystemRuntimeCpuResponse {
            logical_count: profile.cpu.logical_count,
        },
        memory: SystemRuntimeMemoryResponse {
            total_bytes: profile.memory.total_bytes,
            total_gb: profile.memory.total_gb,
            available_bytes: profile.memory.available_bytes,
            available_gb: profile.memory.available_gb,
            process_bytes: profile.memory.process_bytes,
            process_gb: profile.memory.process_gb,
        },
        related_process_bytes,
        related_process_count,
        services: services.into_iter().map(str::to_string).collect(),
    }
}

fn service_from_profile(profile: &RuntimeProfile) -> SystemRuntimeServiceResponse {
    SystemRuntimeServiceResponse {
        reachable: true,
        service: profile.service.clone(),
        status: Some(profile.service_status.clone()),
        version: Some(profile.service_version.clone()),
        host_fingerprint: Some(profile.host_fingerprint.clone()),
    }
}

impl From<LocaleResolution> for LocaleMetaResponse {
    fn from(value: LocaleResolution) -> Self {
        Self {
            requested_locale: value.requested_locale,
            resolved_locale: value.resolved_locale,
            source: value.source.into(),
            fallback_locale: value.fallback_locale,
            supported_locales: value.supported_locales,
        }
    }
}

impl From<LocaleSource> for LocaleSourceResponse {
    fn from(value: LocaleSource) -> Self {
        match value {
            LocaleSource::Query => Self::Query,
            LocaleSource::ExplicitHeader => Self::ExplicitHeader,
            LocaleSource::UserPreferredLocale => Self::UserPreferredLocale,
            LocaleSource::AcceptLanguage => Self::AcceptLanguage,
            LocaleSource::Fallback => Self::Fallback,
        }
    }
}
