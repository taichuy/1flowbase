use super::*;

impl InterfaceContract for SystemInterfaceOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema([
                ("variant", mp::tag_schema("ReleaseStatus")),
                (
                    "0",
                    mp::object_schema([
                        ("current_version", mp::text_schema()),
                        ("latest_version", mp::text_schema()),
                        ("has_update", serde_json::json!({"type":"boolean"})),
                        (
                            "release_info",
                            serde_json::json!({"anyOf": [mp::object_schema([("name",mp::object_schema([("byte_count",mp::count_schema())])), ("body",mp::object_schema([("byte_count",mp::count_schema())])), ("published_at",mp::text_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "upgrade_commands",
                            mp::object_schema([
                                (
                                    "shell",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "powershell",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        ("cached", serde_json::json!({"type":"boolean"})),
                        (
                            "warning",
                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("RuntimeProfile")),
                (
                    "0",
                    mp::object_schema([
                        ("api_node_id", mp::text_schema()),
                        (
                            "provider_install_root",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "host_extension_dropin_root",
                            mp::object_schema([("byte_count", mp::count_schema())]),
                        ),
                        (
                            "related_process_memory_complete",
                            serde_json::json!({"type":"boolean"}),
                        ),
                        (
                            "locale_meta",
                            mp::object_schema([
                                (
                                    "requested_locale",
                                    serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "resolved_locale",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "source",
                                    mp::union_schema(vec![
                                        mp::object_schema([("variant", mp::tag_schema("Query"))]),
                                        mp::object_schema([(
                                            "variant",
                                            mp::tag_schema("ExplicitHeader"),
                                        )]),
                                        mp::object_schema([(
                                            "variant",
                                            mp::tag_schema("UserPreferredLocale"),
                                        )]),
                                        mp::object_schema([(
                                            "variant",
                                            mp::tag_schema("AcceptLanguage"),
                                        )]),
                                        mp::object_schema([(
                                            "variant",
                                            mp::tag_schema("Fallback"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "fallback_locale",
                                    mp::object_schema([("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "supported_locales",
                                    mp::object_schema([("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "topology",
                            mp::object_schema([(
                                "relationship",
                                mp::union_schema(vec![
                                    mp::object_schema([("variant", mp::tag_schema("SameHost"))]),
                                    mp::object_schema([("variant", mp::tag_schema("SplitHost"))]),
                                    mp::object_schema([(
                                        "variant",
                                        mp::tag_schema("RunnerUnreachable"),
                                    )]),
                                ]),
                            )]),
                        ),
                        (
                            "services",
                            mp::object_schema([
                                (
                                    "api_server",
                                    mp::object_schema([
                                        ("reachable", serde_json::json!({"type":"boolean"})),
                                        (
                                            "service",
                                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "plugin_runner",
                                    mp::object_schema([
                                        ("reachable", serde_json::json!({"type":"boolean"})),
                                        (
                                            "service",
                                            mp::object_schema([("byte_count", mp::count_schema())]),
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
                                            serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                            ]),
                        ),
                        (
                            "hosts",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("host_fingerprint",mp::object_schema([("byte_count",mp::count_schema())])), ("platform",mp::object_schema([("os",mp::object_schema([("byte_count",mp::count_schema())])), ("arch",mp::object_schema([("byte_count",mp::count_schema())])), ("libc",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("rust_target_triple",mp::object_schema([("byte_count",mp::count_schema())]))])), ("cpu",mp::object_schema([("logical_count",serde_json::json!({"type":"integer"}))])), ("memory",mp::object_schema([("total_bytes",serde_json::json!({"type":"integer"})), ("total_gb",serde_json::json!({"type":"number"})), ("available_bytes",serde_json::json!({"type":"integer"})), ("available_gb",serde_json::json!({"type":"number"})), ("process_bytes",serde_json::json!({"type":"integer"})), ("process_gb",serde_json::json!({"type":"number"}))])), ("related_process_bytes",serde_json::json!({"type":"integer"})), ("related_process_count",serde_json::json!({"type":"integer"})), ("services",mp::object_schema([("item_count",mp::count_schema())]))])}),
                        ),
                        (
                            "runtime_targets",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema([("target_id",mp::text_schema()), ("reachable",serde_json::json!({"type":"boolean"})), ("host_fingerprint",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("metrics",serde_json::json!({"anyOf": [mp::object_schema([("captured_at_unix_milliseconds",serde_json::json!({"type":"integer"})), ("sample_interval_milliseconds",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]}))]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ProcessList")),
                (
                    "0",
                    mp::object_schema([
                        ("process_total", serde_json::json!({"type":"integer"})),
                        (
                            "processes",
                            serde_json::json!({"type":"array","maxItems":256,"items":mp::object_schema([("pid",serde_json::json!({"type":"integer"})), ("parent_pid",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("name",mp::text_schema()), ("command",serde_json::json!({"anyOf": [mp::object_schema([("byte_count",mp::count_schema())]), {"type":"null"}]})), ("user",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("status",mp::text_schema()), ("cpu_usage_percent",serde_json::json!({"type":"number"})), ("cpu_usage_single_core_percent",serde_json::json!({"type":"number"})), ("memory_bytes",serde_json::json!({"type":"integer"})), ("memory_usage_percent",serde_json::json!({"type":"number"})), ("start_time_unix_seconds",serde_json::json!({"type":"integer"})), ("terminable",serde_json::json!({"type":"boolean"})), ("backend_process",serde_json::json!({"type":"boolean"}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema([
                ("variant", mp::tag_schema("ProcessTermination")),
                (
                    "0",
                    mp::object_schema([
                        ("pid", serde_json::json!({"type":"integer"})),
                        ("outcome", mp::text_schema()),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ReleaseStatus(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ReleaseStatus".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([
                        ("current_version", mp::text(&(_field_0).current_version)?),
                        ("latest_version", mp::text(&(_field_0).latest_version)?),
                        ("has_update", serde_json::Value::Bool((_field_0).has_update)),
                        (
                            "release_info",
                            match (_field_0).release_info.as_ref() {
                                Some(item) => mp::object_value([
                                    (
                                        "name",
                                        mp::object_value([(
                                            "byte_count",
                                            serde_json::json!((item).name.len()),
                                        )]),
                                    ),
                                    (
                                        "body",
                                        mp::object_value([(
                                            "byte_count",
                                            serde_json::json!((item).body.len()),
                                        )]),
                                    ),
                                    ("published_at", mp::text(&(item).published_at)?),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "upgrade_commands",
                            mp::object_value([
                                (
                                    "shell",
                                    mp::object_value([(
                                        "byte_count",
                                        serde_json::json!((_field_0).upgrade_commands.shell.len()),
                                    )]),
                                ),
                                (
                                    "powershell",
                                    mp::object_value([(
                                        "byte_count",
                                        serde_json::json!((_field_0)
                                            .upgrade_commands
                                            .powershell
                                            .len()),
                                    )]),
                                ),
                            ]),
                        ),
                        ("cached", serde_json::Value::Bool((_field_0).cached)),
                        (
                            "warning",
                            match (_field_0).warning.as_ref() {
                                Some(item) => mp::object_value([(
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
                mp::object_value([
                    (
                        "variant",
                        serde_json::Value::String("RuntimeProfile".to_owned()),
                    ),
                    (
                        "0",
                        mp::object_value([
                            ("api_node_id", mp::text(&(_field_0).api_node_id)?),
                            (
                                "provider_install_root",
                                mp::object_value([(
                                    "byte_count",
                                    serde_json::json!((_field_0).provider_install_root.len()),
                                )]),
                            ),
                            (
                                "host_extension_dropin_root",
                                mp::object_value([(
                                    "byte_count",
                                    serde_json::json!((_field_0).host_extension_dropin_root.len()),
                                )]),
                            ),
                            (
                                "related_process_memory_complete",
                                serde_json::Value::Bool((_field_0).related_process_memory_complete),
                            ),
                            (
                                "locale_meta",
                                mp::object_value([
                                    (
                                        "requested_locale",
                                        match (_field_0).locale_meta.requested_locale.as_ref() {
                                            Some(item) => mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).len()),
                                            )]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                    (
                                        "resolved_locale",
                                        mp::object_value([(
                                            "byte_count",
                                            serde_json::json!((_field_0)
                                                .locale_meta
                                                .resolved_locale
                                                .len()),
                                        )]),
                                    ),
                                    (
                                        "source",
                                        match &(_field_0).locale_meta.source {
                                            LocaleSourceResponse::Query => mp::object_value([(
                                                "variant",
                                                serde_json::Value::String("Query".to_owned()),
                                            )]),
                                            LocaleSourceResponse::ExplicitHeader => {
                                                mp::object_value([(
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "ExplicitHeader".to_owned(),
                                                    ),
                                                )])
                                            }
                                            LocaleSourceResponse::UserPreferredLocale => {
                                                mp::object_value([(
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "UserPreferredLocale".to_owned(),
                                                    ),
                                                )])
                                            }
                                            LocaleSourceResponse::AcceptLanguage => {
                                                mp::object_value([(
                                                    "variant",
                                                    serde_json::Value::String(
                                                        "AcceptLanguage".to_owned(),
                                                    ),
                                                )])
                                            }
                                            LocaleSourceResponse::Fallback => mp::object_value([(
                                                "variant",
                                                serde_json::Value::String("Fallback".to_owned()),
                                            )]),
                                        },
                                    ),
                                    (
                                        "fallback_locale",
                                        mp::object_value([(
                                            "byte_count",
                                            serde_json::json!((_field_0)
                                                .locale_meta
                                                .fallback_locale
                                                .len()),
                                        )]),
                                    ),
                                    (
                                        "supported_locales",
                                        mp::object_value([(
                                            "item_count",
                                            serde_json::json!((_field_0)
                                                .locale_meta
                                                .supported_locales
                                                .len()),
                                        )]),
                                    ),
                                ]),
                            ),
                            (
                                "topology",
                                mp::object_value([(
                                    "relationship",
                                    match &(_field_0).topology.relationship {
                                        SystemRuntimeRelationship::SameHost => {
                                            mp::object_value([(
                                                "variant",
                                                serde_json::Value::String("SameHost".to_owned()),
                                            )])
                                        }
                                        SystemRuntimeRelationship::SplitHost => {
                                            mp::object_value([(
                                                "variant",
                                                serde_json::Value::String("SplitHost".to_owned()),
                                            )])
                                        }
                                        SystemRuntimeRelationship::RunnerUnreachable => {
                                            mp::object_value([(
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
                                mp::object_value([
                                    (
                                        "api_server",
                                        mp::object_value([
                                            (
                                                "reachable",
                                                serde_json::Value::Bool(
                                                    (_field_0).services.api_server.reachable,
                                                ),
                                            ),
                                            (
                                                "service",
                                                mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((_field_0)
                                                        .services
                                                        .api_server
                                                        .service
                                                        .len()),
                                                )]),
                                            ),
                                            (
                                                "status",
                                                match (_field_0).services.api_server.status.as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "version",
                                                match (_field_0)
                                                    .services
                                                    .api_server
                                                    .version
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "host_fingerprint",
                                                match (_field_0)
                                                    .services
                                                    .api_server
                                                    .host_fingerprint
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::object_value([(
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
                                        mp::object_value([
                                            (
                                                "reachable",
                                                serde_json::Value::Bool(
                                                    (_field_0).services.plugin_runner.reachable,
                                                ),
                                            ),
                                            (
                                                "service",
                                                mp::object_value([(
                                                    "byte_count",
                                                    serde_json::json!((_field_0)
                                                        .services
                                                        .plugin_runner
                                                        .service
                                                        .len()),
                                                )]),
                                            ),
                                            (
                                                "status",
                                                match (_field_0)
                                                    .services
                                                    .plugin_runner
                                                    .status
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "version",
                                                match (_field_0)
                                                    .services
                                                    .plugin_runner
                                                    .version
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::text(item)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "host_fingerprint",
                                                match (_field_0)
                                                    .services
                                                    .plugin_runner
                                                    .host_fingerprint
                                                    .as_ref()
                                                {
                                                    Some(item) => mp::object_value([(
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
                                if (_field_0).hosts.len() > 32 {
                                    return None;
                                }
                                serde_json::Value::Array(
                                    (_field_0)
                                        .hosts
                                        .iter()
                                        .map(|item| {
                                            Some(mp::object_value([
                                                (
                                                    "host_fingerprint",
                                                    mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((item)
                                                            .host_fingerprint
                                                            .len()),
                                                    )]),
                                                ),
                                                (
                                                    "platform",
                                                    mp::object_value([
                                                        (
                                                            "os",
                                                            mp::object_value([(
                                                                "byte_count",
                                                                serde_json::json!((item)
                                                                    .platform
                                                                    .os
                                                                    .len()),
                                                            )]),
                                                        ),
                                                        (
                                                            "arch",
                                                            mp::object_value([(
                                                                "byte_count",
                                                                serde_json::json!((item)
                                                                    .platform
                                                                    .arch
                                                                    .len()),
                                                            )]),
                                                        ),
                                                        (
                                                            "libc",
                                                            match (item).platform.libc.as_ref() {
                                                                Some(item) => mp::object_value([(
                                                                    "byte_count",
                                                                    serde_json::json!((item).len()),
                                                                )]),
                                                                None => serde_json::Value::Null,
                                                            },
                                                        ),
                                                        (
                                                            "rust_target_triple",
                                                            mp::object_value([(
                                                                "byte_count",
                                                                serde_json::json!((item)
                                                                    .platform
                                                                    .rust_target_triple
                                                                    .len()),
                                                            )]),
                                                        ),
                                                    ]),
                                                ),
                                                (
                                                    "cpu",
                                                    mp::object_value([(
                                                        "logical_count",
                                                        serde_json::json!((item).cpu.logical_count),
                                                    )]),
                                                ),
                                                (
                                                    "memory",
                                                    mp::object_value([
                                                        (
                                                            "total_bytes",
                                                            serde_json::json!(
                                                                (item).memory.total_bytes
                                                            ),
                                                        ),
                                                        (
                                                            "total_gb",
                                                            serde_json::json!(
                                                                (item).memory.total_gb
                                                            ),
                                                        ),
                                                        (
                                                            "available_bytes",
                                                            serde_json::json!(
                                                                (item).memory.available_bytes
                                                            ),
                                                        ),
                                                        (
                                                            "available_gb",
                                                            serde_json::json!(
                                                                (item).memory.available_gb
                                                            ),
                                                        ),
                                                        (
                                                            "process_bytes",
                                                            serde_json::json!(
                                                                (item).memory.process_bytes
                                                            ),
                                                        ),
                                                        (
                                                            "process_gb",
                                                            serde_json::json!(
                                                                (item).memory.process_gb
                                                            ),
                                                        ),
                                                    ]),
                                                ),
                                                (
                                                    "related_process_bytes",
                                                    serde_json::json!((item).related_process_bytes),
                                                ),
                                                (
                                                    "related_process_count",
                                                    serde_json::json!((item).related_process_count),
                                                ),
                                                (
                                                    "services",
                                                    mp::object_value([(
                                                        "item_count",
                                                        serde_json::json!((item).services.len()),
                                                    )]),
                                                ),
                                            ]))
                                        })
                                        .collect::<Option<Vec<_>>>()?,
                                )
                            }),
                            ("runtime_targets", {
                                if (_field_0).runtime_targets.len() > 32 {
                                    return None;
                                }
                                serde_json::Value::Array((_field_0).runtime_targets.iter().map(|item| Some(mp::object_value([("target_id",mp::text(&(item).target_id)?), ("reachable",serde_json::Value::Bool((item).reachable)), ("host_fingerprint",match (item).host_fingerprint.as_ref() { Some(item) => mp::object_value([("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("metrics",match (item).metrics.as_ref() { Some(item) => mp::object_value([("captured_at_unix_milliseconds",serde_json::json!((item).captured_at_unix_milliseconds)), ("sample_interval_milliseconds",match (item).sample_interval_milliseconds.as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null })]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?)
                            }),
                        ]),
                    ),
                ])
            }
            Self::ProcessList(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ProcessList".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([
                        ("process_total", serde_json::json!((_field_0).process_total)),
                        ("processes", {
                            if (_field_0).processes.len() > 256 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (_field_0)
                                    .processes
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value([
                                            ("pid", serde_json::json!((item).pid)),
                                            (
                                                "parent_pid",
                                                match (item).parent_pid.as_ref() {
                                                    Some(value) => serde_json::json!(*(value)),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("name", mp::text(&(item).name)?),
                                            (
                                                "command",
                                                match (item).command.as_ref() {
                                                    Some(value) => mp::object_value([(
                                                        "byte_count",
                                                        serde_json::json!((value).len()),
                                                    )]),
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            (
                                                "user",
                                                match (item).user.as_ref() {
                                                    Some(value) => mp::text(value)?,
                                                    None => serde_json::Value::Null,
                                                },
                                            ),
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "cpu_usage_percent",
                                                serde_json::json!((item).cpu_usage_percent),
                                            ),
                                            (
                                                "cpu_usage_single_core_percent",
                                                serde_json::json!(
                                                    (item).cpu_usage_single_core_percent
                                                ),
                                            ),
                                            (
                                                "memory_bytes",
                                                serde_json::json!((item).memory_bytes),
                                            ),
                                            (
                                                "memory_usage_percent",
                                                serde_json::json!((item).memory_usage_percent),
                                            ),
                                            (
                                                "start_time_unix_seconds",
                                                serde_json::json!((item).start_time_unix_seconds),
                                            ),
                                            (
                                                "terminable",
                                                serde_json::Value::Bool((item).terminable),
                                            ),
                                            (
                                                "backend_process",
                                                serde_json::Value::Bool((item).backend_process),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                    ]),
                ),
            ]),
            Self::ProcessTermination(_field_0) => mp::object_value([
                (
                    "variant",
                    serde_json::Value::String("ProcessTermination".to_owned()),
                ),
                (
                    "0",
                    mp::object_value([
                        ("pid", serde_json::json!((_field_0).pid)),
                        ("outcome", mp::text(&(_field_0).outcome)?),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-system-output";
    const CONTRACT_VERSION: &'static str = "1";
}
