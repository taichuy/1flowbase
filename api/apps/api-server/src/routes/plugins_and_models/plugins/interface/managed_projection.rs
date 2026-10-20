use super::*;

impl InterfaceContract for PluginInterfaceInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListCatalog")),
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
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListFamilies")),
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
                    ]),
                ),
            ]),
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("InstallPath")),
                (
                    "0",
                    mp::object_schema(&[(
                        "package_root",
                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                    )]),
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
                ("variant", mp::tag_schema("RefreshCatalogProjection")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RefreshArtifact")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("InstallArtifact")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpgradeLatest")),
                ("provider_code", mp::text_schema()),
                (
                    "body",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("compatibility_override",serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_current_host_version",mp::text_schema()), ("acknowledged_minimum_host_version",mp::text_schema())]), {"type":"null"}]})), ("risk_override",serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}))]), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SwitchVersion")),
                ("provider_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("installation_id", mp::text_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteFamily")),
                ("provider_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Enable")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Assign")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ListTasks"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("GetTask")),
                ("task_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelFamilies")),
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
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelOfficial")),
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelInstallOfficial")),
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
                ("variant", mp::tag_schema("ModelInstallUploaded")),
                (
                    "file_name",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                (
                    "package_bytes",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelRefreshArtifact")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelInstallArtifact")),
                ("installation_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelUpgradeLatest")),
                ("provider_code", mp::text_schema()),
                (
                    "body",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("compatibility_override",serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_current_host_version",mp::text_schema()), ("acknowledged_minimum_host_version",mp::text_schema())]), {"type":"null"}]})), ("risk_override",serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}))]), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelSwitchVersion")),
                ("provider_code", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[("installation_id", mp::text_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelDeleteFamily")),
                ("provider_code", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ModelGetTask")),
                ("task_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::ListCatalog {
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListCatalog".to_owned()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "plugin_type",
                            match (_field_query).plugin_type.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "locale",
                            match (_field_query).locale.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ListFamilies {
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ListFamilies".to_owned()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "plugin_type",
                            match (_field_query).plugin_type.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "locale",
                            match (_field_query).locale.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
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
                            match (_field_query).plugin_type.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "locale",
                            match (_field_query).locale.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "q",
                            match (_field_query).q.as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cursor",
                            match (_field_query).cursor.as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (_field_query).limit.as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::InstallPath(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("InstallPath".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[(
                        "package_root",
                        mp::object_value(&[(
                            "byte_count",
                            serde_json::json!((_field_0).package_root.len()),
                        )]),
                    )]),
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
                            match (_field_0).compatibility_override.as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).reason.len()),
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
                            match (_field_0).risk_override.as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).reason.len()),
                                        )]),
                                    ),
                                    (
                                        "acknowledged_warnings",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((item).acknowledged_warnings.len()),
                                        )]),
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::RefreshCatalogProjection {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RefreshCatalogProjection".to_owned()),
                ),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::RefreshArtifact {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("RefreshArtifact".to_owned()),
                ),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::InstallArtifact {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("InstallArtifact".to_owned()),
                ),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::UpgradeLatest {
                provider_code: _field_provider_code,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("UpgradeLatest".to_owned()),
                ),
                ("provider_code", mp::text(_field_provider_code)?),
                (
                    "body",
                    match (_field_body).as_ref() {
                        Some(item) => mp::object_value(&[
                            (
                                "compatibility_override",
                                match (item).compatibility_override.as_ref() {
                                    Some(item) => mp::object_value(&[
                                        (
                                            "reason",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).reason.len()),
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
                                match (item).risk_override.as_ref() {
                                    Some(item) => mp::object_value(&[
                                        (
                                            "reason",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).reason.len()),
                                            )]),
                                        ),
                                        (
                                            "acknowledged_warnings",
                                            mp::object_value(&[(
                                                "item_count",
                                                serde_json::json!((item)
                                                    .acknowledged_warnings
                                                    .len()),
                                            )]),
                                        ),
                                    ]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                        ]),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
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
            Self::DeleteFamily {
                provider_code: _field_provider_code,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("DeleteFamily".to_owned()),
                ),
                ("provider_code", mp::text(_field_provider_code)?),
            ]),
            Self::Enable {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Enable".to_owned())),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::Assign {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Assign".to_owned())),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::ListTasks => {
                mp::object_value(&[("variant", serde_json::Value::String("ListTasks".to_owned()))])
            }
            Self::GetTask {
                task_id: _field_task_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("GetTask".to_owned())),
                ("task_id", mp::text(_field_task_id)?),
            ]),
            Self::ModelFamilies {
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelFamilies".to_owned()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "plugin_type",
                            match (_field_query).plugin_type.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "locale",
                            match (_field_query).locale.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ModelOfficial {
                query: _field_query,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelOfficial".to_owned()),
                ),
                (
                    "query",
                    mp::object_value(&[
                        (
                            "plugin_type",
                            match (_field_query).plugin_type.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "locale",
                            match (_field_query).locale.as_ref() {
                                Some(item) => mp::text(item)?,
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "q",
                            match (_field_query).q.as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "cursor",
                            match (_field_query).cursor.as_ref() {
                                Some(item) => mp::object_value(&[(
                                    "byte_count",
                                    serde_json::json!((item).len()),
                                )]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "limit",
                            match (_field_query).limit.as_ref() {
                                Some(item) => serde_json::json!(*(item)),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ModelInstallOfficial(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelInstallOfficial".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("plugin_id", mp::text(&(_field_0).plugin_id)?),
                        (
                            "compatibility_override",
                            match (_field_0).compatibility_override.as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).reason.len()),
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
                            match (_field_0).risk_override.as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "reason",
                                        mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).reason.len()),
                                        )]),
                                    ),
                                    (
                                        "acknowledged_warnings",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((item).acknowledged_warnings.len()),
                                        )]),
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                    ]),
                ),
            ]),
            Self::ModelInstallUploaded {
                file_name: _field_file_name,
                package_bytes: _field_package_bytes,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelInstallUploaded".to_owned()),
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
            Self::ModelRefreshArtifact {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelRefreshArtifact".to_owned()),
                ),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::ModelInstallArtifact {
                installation_id: _field_installation_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelInstallArtifact".to_owned()),
                ),
                ("installation_id", mp::text(_field_installation_id)?),
            ]),
            Self::ModelUpgradeLatest {
                provider_code: _field_provider_code,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelUpgradeLatest".to_owned()),
                ),
                ("provider_code", mp::text(_field_provider_code)?),
                (
                    "body",
                    match (_field_body).as_ref() {
                        Some(item) => mp::object_value(&[
                            (
                                "compatibility_override",
                                match (item).compatibility_override.as_ref() {
                                    Some(item) => mp::object_value(&[
                                        (
                                            "reason",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).reason.len()),
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
                                match (item).risk_override.as_ref() {
                                    Some(item) => mp::object_value(&[
                                        (
                                            "reason",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!((item).reason.len()),
                                            )]),
                                        ),
                                        (
                                            "acknowledged_warnings",
                                            mp::object_value(&[(
                                                "item_count",
                                                serde_json::json!((item)
                                                    .acknowledged_warnings
                                                    .len()),
                                            )]),
                                        ),
                                    ]),
                                    None => serde_json::Value::Null,
                                },
                            ),
                        ]),
                        None => serde_json::Value::Null,
                    },
                ),
            ]),
            Self::ModelSwitchVersion {
                provider_code: _field_provider_code,
                body: _field_body,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelSwitchVersion".to_owned()),
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
            Self::ModelDeleteFamily {
                provider_code: _field_provider_code,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelDeleteFamily".to_owned()),
                ),
                ("provider_code", mp::text(_field_provider_code)?),
            ]),
            Self::ModelGetTask {
                task_id: _field_task_id,
                ..
            } => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ModelGetTask".to_owned()),
                ),
                ("task_id", mp::text(_field_task_id)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-plugin-management-input";
    const CONTRACT_VERSION: &'static str = "1";
}

#[path = "managed_projection/output_projection.rs"]
mod output_projection;
