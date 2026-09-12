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
                            serde_json::json!((&(_field_0).package_root).len()),
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
                                match (&(item).compatibility_override).as_ref() {
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
                                match (&(item).risk_override).as_ref() {
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
                                                serde_json::json!(
                                                    (&(item).acknowledged_warnings).len()
                                                ),
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
                                match (&(item).compatibility_override).as_ref() {
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
                                match (&(item).risk_override).as_ref() {
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
                                                serde_json::json!(
                                                    (&(item).acknowledged_warnings).len()
                                                ),
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

impl InterfaceContract for PluginInterfaceOutput {
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
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("installation",mp::object_schema(&[("id",mp::text_schema()), ("provider_code",mp::text_schema()), ("runtime_slot",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("plugin_id",mp::text_schema()), ("plugin_version",mp::text_schema()), ("contract_version",mp::text_schema()), ("protocol",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("source_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("trust_level",mp::object_schema(&[("byte_count",mp::count_schema())])), ("verification_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("desired_state",mp::object_schema(&[("byte_count",mp::count_schema())])), ("expected_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signature_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("signature_algorithm",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signing_key_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("metadata_json",mp::json_summary_schema()), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema())])), ("local_artifact",mp::object_schema(&[("node_id",mp::text_schema()), ("installation_id",mp::text_schema()), ("local_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("local_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("package_path",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("manifest_fingerprint",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("artifact_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("runtime_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("availability_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("checked_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("last_error",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("plugin_type",mp::text_schema()), ("namespace",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("provider_label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("model_discovery_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("assigned_to_current_workspace",serde_json::json!({"type":"boolean"})), ("catalog_refresh_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("catalog_last_error_message",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("catalog_refreshed_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Families")),
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
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("provider_code",mp::text_schema()), ("plugin_type",mp::text_schema()), ("namespace",mp::object_schema(&[("byte_count",mp::count_schema())])), ("label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description_key",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("provider_label_key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("icon",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("protocol",mp::text_schema()), ("model_discovery_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_installation_id",mp::text_schema()), ("current_version",mp::text_schema()), ("installation_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_local_artifact",mp::object_schema(&[("node_id",mp::text_schema()), ("installation_id",mp::text_schema()), ("local_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("local_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("package_path",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("manifest_fingerprint",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("artifact_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("runtime_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("availability_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("checked_at",mp::object_schema(&[("byte_count",mp::count_schema())])), ("last_error",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("latest_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("has_update",serde_json::json!({"type":"boolean"})), ("installed_versions",mp::object_schema(&[("item_count",mp::count_schema())]))])}),
                        ),
                    ]),
                ),
            ]),
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
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("plugin_id",mp::text_schema()), ("plugin_type",mp::text_schema()), ("provider_code",mp::text_schema()), ("display_name",mp::object_schema(&[("byte_count",mp::count_schema())])), ("description",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("icon",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("protocol",mp::text_schema()), ("latest_version",mp::text_schema()), ("minimum_host_version",mp::text_schema()), ("current_host_version",mp::text_schema()), ("compatibility_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("compatibility_warning_reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("selected_artifact",mp::object_schema(&[("os",mp::object_schema(&[("byte_count",mp::count_schema())])), ("arch",mp::object_schema(&[("byte_count",mp::count_schema())])), ("libc",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("rust_target",mp::object_schema(&[("byte_count",mp::count_schema())])), ("checksum",mp::object_schema(&[("byte_count",mp::count_schema())])), ("signature_algorithm",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("signing_key_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])), ("model_discovery_mode",mp::object_schema(&[("byte_count",mp::count_schema())])), ("install_status",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                    ]),
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("Projection")),
                (
                    "0",
                    mp::object_schema(&[
                        ("installation_id", mp::text_schema()),
                        ("package_code", mp::text_schema()),
                        ("package_version", mp::text_schema()),
                        (
                            "projection_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "last_error_message",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "refreshed_at",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        ("updated_at", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Artifact")),
                (
                    "0",
                    mp::object_schema(&[
                        ("node_id", mp::text_schema()),
                        ("installation_id", mp::text_schema()),
                        (
                            "local_version",
                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                        ),
                        (
                            "local_checksum",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "package_path",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "manifest_fingerprint",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "artifact_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "runtime_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "availability_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "checked_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "last_error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Task")),
                (
                    "0",
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
            mp::object_schema(&[
                ("variant", mp::tag_schema("Tasks")),
                (
                    "0",
                    serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("installation_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("workspace_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("provider_code",mp::text_schema()), ("task_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("status",mp::text_schema()), ("status_message",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("detail_json",mp::json_summary_schema()), ("created_at",mp::text_schema()), ("updated_at",mp::text_schema()), ("finished_at",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Catalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Catalog".to_owned())), ("0",mp::object_value(&[("locale_meta",mp::object_value(&[("requested_locale",match (&(&(_field_0).locale_meta).requested_locale).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("resolved_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).resolved_locale).len()))])), ("source",match &(&(_field_0).locale_meta).source {crate::routes::settings_group::system::LocaleSourceResponse::Query => mp::object_value(&[("variant",serde_json::Value::String("Query".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::ExplicitHeader => mp::object_value(&[("variant",serde_json::Value::String("ExplicitHeader".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::UserPreferredLocale => mp::object_value(&[("variant",serde_json::Value::String("UserPreferredLocale".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::AcceptLanguage => mp::object_value(&[("variant",serde_json::Value::String("AcceptLanguage".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::Fallback => mp::object_value(&[("variant",serde_json::Value::String("Fallback".to_owned()))])}), ("fallback_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).fallback_locale).len()))])), ("supported_locales",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).locale_meta).supported_locales).len()))]))])), ("i18n_catalog",mp::json_summary(&(_field_0).i18n_catalog)), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("installation",mp::object_value(&[("id",mp::text(&(&(item).installation).id)?), ("provider_code",mp::text(&(&(item).installation).provider_code)?), ("runtime_slot",match (&(&(item).installation).runtime_slot).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("plugin_id",mp::text(&(&(item).installation).plugin_id)?), ("plugin_version",mp::text(&(&(item).installation).plugin_version)?), ("contract_version",mp::text(&(&(item).installation).contract_version)?), ("protocol",mp::text(&(&(item).installation).protocol)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).installation).display_name).len()))])), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).installation).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).installation).trust_level).len()))])), ("verification_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).installation).verification_status).len()))])), ("desired_state",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).installation).desired_state).len()))])), ("expected_checksum",match (&(&(item).installation).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).installation).signature_status).len()))])), ("signature_algorithm",match (&(&(item).installation).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(&(item).installation).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("metadata_json",mp::json_summary(&(&(item).installation).metadata_json)), ("created_at",mp::text(&(&(item).installation).created_at)?), ("updated_at",mp::text(&(&(item).installation).updated_at)?)])), ("local_artifact",mp::object_value(&[("node_id",mp::text(&(&(item).local_artifact).node_id)?), ("installation_id",mp::text(&(&(item).local_artifact).installation_id)?), ("local_version",match (&(&(item).local_artifact).local_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("local_checksum",match (&(&(item).local_artifact).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("package_path",match (&(&(item).local_artifact).package_path).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("manifest_fingerprint",match (&(&(item).local_artifact).manifest_fingerprint).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("artifact_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).local_artifact).artifact_status).len()))])), ("runtime_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).local_artifact).runtime_status).len()))])), ("availability_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).local_artifact).availability_status).len()))])), ("checked_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).local_artifact).checked_at).len()))])), ("last_error",match (&(&(item).local_artifact).last_error).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("plugin_type",mp::text(&(item).plugin_type)?), ("namespace",mp::object_value(&[("byte_count",serde_json::json!((&(item).namespace).len()))])), ("label_key",mp::object_value(&[("byte_count",serde_json::json!((&(item).label_key).len()))])), ("description_key",match (&(item).description_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("provider_label_key",mp::object_value(&[("byte_count",serde_json::json!((&(item).provider_label_key).len()))])), ("model_discovery_mode",mp::object_value(&[("byte_count",serde_json::json!((&(item).model_discovery_mode).len()))])), ("assigned_to_current_workspace",serde_json::Value::Bool(*(&(item).assigned_to_current_workspace))), ("catalog_refresh_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).catalog_refresh_status).len()))])), ("catalog_last_error_message",match (&(item).catalog_last_error_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("catalog_refreshed_at",match (&(item).catalog_refreshed_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Families(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Families".to_owned())), ("0",mp::object_value(&[("locale_meta",mp::object_value(&[("requested_locale",match (&(&(_field_0).locale_meta).requested_locale).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("resolved_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).resolved_locale).len()))])), ("source",match &(&(_field_0).locale_meta).source {crate::routes::settings_group::system::LocaleSourceResponse::Query => mp::object_value(&[("variant",serde_json::Value::String("Query".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::ExplicitHeader => mp::object_value(&[("variant",serde_json::Value::String("ExplicitHeader".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::UserPreferredLocale => mp::object_value(&[("variant",serde_json::Value::String("UserPreferredLocale".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::AcceptLanguage => mp::object_value(&[("variant",serde_json::Value::String("AcceptLanguage".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::Fallback => mp::object_value(&[("variant",serde_json::Value::String("Fallback".to_owned()))])}), ("fallback_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).fallback_locale).len()))])), ("supported_locales",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).locale_meta).supported_locales).len()))]))])), ("i18n_catalog",mp::json_summary(&(_field_0).i18n_catalog)), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("provider_code",mp::text(&(item).provider_code)?), ("plugin_type",mp::text(&(item).plugin_type)?), ("namespace",mp::object_value(&[("byte_count",serde_json::json!((&(item).namespace).len()))])), ("label_key",mp::object_value(&[("byte_count",serde_json::json!((&(item).label_key).len()))])), ("description_key",match (&(item).description_key).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("provider_label_key",mp::object_value(&[("byte_count",serde_json::json!((&(item).provider_label_key).len()))])), ("icon",match (&(item).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("protocol",mp::text(&(item).protocol)?), ("model_discovery_mode",mp::object_value(&[("byte_count",serde_json::json!((&(item).model_discovery_mode).len()))])), ("current_installation_id",mp::text(&(item).current_installation_id)?), ("current_version",mp::text(&(item).current_version)?), ("installation_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).installation_status).len()))])), ("current_local_artifact",mp::object_value(&[("node_id",mp::text(&(&(item).current_local_artifact).node_id)?), ("installation_id",mp::text(&(&(item).current_local_artifact).installation_id)?), ("local_version",match (&(&(item).current_local_artifact).local_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("local_checksum",match (&(&(item).current_local_artifact).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("package_path",match (&(&(item).current_local_artifact).package_path).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("manifest_fingerprint",match (&(&(item).current_local_artifact).manifest_fingerprint).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("artifact_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).current_local_artifact).artifact_status).len()))])), ("runtime_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).current_local_artifact).runtime_status).len()))])), ("availability_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).current_local_artifact).availability_status).len()))])), ("checked_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).current_local_artifact).checked_at).len()))])), ("last_error",match (&(&(item).current_local_artifact).last_error).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("latest_version",match (&(item).latest_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("has_update",serde_json::Value::Bool(*(&(item).has_update))), ("installed_versions",mp::object_value(&[("item_count",serde_json::json!((&(item).installed_versions).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Official(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Official".to_owned())), ("0",mp::object_value(&[("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_kind).len()))])), ("source_label",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_label).len()))])), ("source_freshness",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).source_freshness).len()))])), ("locale_meta",mp::object_value(&[("requested_locale",match (&(&(_field_0).locale_meta).requested_locale).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("resolved_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).resolved_locale).len()))])), ("source",match &(&(_field_0).locale_meta).source {crate::routes::settings_group::system::LocaleSourceResponse::Query => mp::object_value(&[("variant",serde_json::Value::String("Query".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::ExplicitHeader => mp::object_value(&[("variant",serde_json::Value::String("ExplicitHeader".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::UserPreferredLocale => mp::object_value(&[("variant",serde_json::Value::String("UserPreferredLocale".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::AcceptLanguage => mp::object_value(&[("variant",serde_json::Value::String("AcceptLanguage".to_owned()))]), crate::routes::settings_group::system::LocaleSourceResponse::Fallback => mp::object_value(&[("variant",serde_json::Value::String("Fallback".to_owned()))])}), ("fallback_locale",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).locale_meta).fallback_locale).len()))])), ("supported_locales",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).locale_meta).supported_locales).len()))]))])), ("page",mp::object_value(&[("limit",serde_json::json!(*(&(&(_field_0).page).limit))), ("next_cursor",match (&(&(_field_0).page).next_cursor).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("plugin_id",mp::text(&(item).plugin_id)?), ("plugin_type",mp::text(&(item).plugin_type)?), ("provider_code",mp::text(&(item).provider_code)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(item).display_name).len()))])), ("description",match (&(item).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon",match (&(item).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("protocol",mp::text(&(item).protocol)?), ("latest_version",mp::text(&(item).latest_version)?), ("minimum_host_version",mp::text(&(item).minimum_host_version)?), ("current_host_version",mp::text(&(item).current_host_version)?), ("compatibility_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).compatibility_status).len()))])), ("compatibility_warning_reason",match (&(item).compatibility_warning_reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("selected_artifact",mp::object_value(&[("os",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).os).len()))])), ("arch",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).arch).len()))])), ("libc",match (&(&(item).selected_artifact).libc).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("rust_target",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).rust_target).len()))])), ("checksum",mp::object_value(&[("byte_count",serde_json::json!((&(&(item).selected_artifact).checksum).len()))])), ("signature_algorithm",match (&(&(item).selected_artifact).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(&(item).selected_artifact).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })])), ("model_discovery_mode",mp::object_value(&[("byte_count",serde_json::json!((&(item).model_discovery_mode).len()))])), ("install_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).install_status).len()))]))]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Installed(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Installed".to_owned())), ("0",mp::object_value(&[("installation",mp::object_value(&[("id",mp::text(&(&(_field_0).installation).id)?), ("provider_code",mp::text(&(&(_field_0).installation).provider_code)?), ("runtime_slot",match (&(&(_field_0).installation).runtime_slot).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("plugin_id",mp::text(&(&(_field_0).installation).plugin_id)?), ("plugin_version",mp::text(&(&(_field_0).installation).plugin_version)?), ("contract_version",mp::text(&(&(_field_0).installation).contract_version)?), ("protocol",mp::text(&(&(_field_0).installation).protocol)?), ("display_name",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).display_name).len()))])), ("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).source_kind).len()))])), ("trust_level",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).trust_level).len()))])), ("verification_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).verification_status).len()))])), ("desired_state",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).desired_state).len()))])), ("expected_checksum",match (&(&(_field_0).installation).expected_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signature_status",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).installation).signature_status).len()))])), ("signature_algorithm",match (&(&(_field_0).installation).signature_algorithm).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("signing_key_id",match (&(&(_field_0).installation).signing_key_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("local_artifact",match (&(&(_field_0).installation).local_artifact).as_ref() { Some(item) => mp::object_value(&[("node_id",mp::text(&(item).node_id)?), ("installation_id",mp::text(&(item).installation_id)?), ("local_version",match (&(item).local_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("local_checksum",match (&(item).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("package_path",match (&(item).package_path).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("manifest_fingerprint",match (&(item).manifest_fingerprint).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("artifact_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).artifact_status).len()))])), ("runtime_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).runtime_status).len()))])), ("availability_status",mp::object_value(&[("byte_count",serde_json::json!((&(item).availability_status).len()))])), ("checked_at",mp::object_value(&[("byte_count",serde_json::json!((&(item).checked_at).len()))])), ("last_error",match (&(item).last_error).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]), None => serde_json::Value::Null }), ("metadata_json",mp::json_summary(&(&(_field_0).installation).metadata_json)), ("created_at",mp::text(&(&(_field_0).installation).created_at)?), ("updated_at",mp::text(&(&(_field_0).installation).updated_at)?)])), ("task",mp::object_value(&[("id",mp::text(&(&(_field_0).task).id)?), ("installation_id",match (&(&(_field_0).task).installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("workspace_id",match (&(&(_field_0).task).workspace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("provider_code",mp::text(&(&(_field_0).task).provider_code)?), ("task_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).task).task_kind).len()))])), ("status",mp::text(&(&(_field_0).task).status)?), ("status_message",match (&(&(_field_0).task).status_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("detail_json",mp::json_summary(&(&(_field_0).task).detail_json)), ("created_at",mp::text(&(&(_field_0).task).created_at)?), ("updated_at",mp::text(&(&(_field_0).task).updated_at)?), ("finished_at",match (&(&(_field_0).task).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]))]), Self::Projection(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Projection".to_owned())), ("0",mp::object_value(&[("installation_id",mp::text(&(_field_0).installation_id)?), ("package_code",mp::text(&(_field_0).package_code)?), ("package_version",mp::text(&(_field_0).package_version)?), ("projection_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).projection_status).len()))])), ("last_error_message",match (&(_field_0).last_error_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("refreshed_at",match (&(_field_0).refreshed_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("updated_at",mp::text(&(_field_0).updated_at)?)]))]), Self::Artifact(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Artifact".to_owned())), ("0",mp::object_value(&[("node_id",mp::text(&(_field_0).node_id)?), ("installation_id",mp::text(&(_field_0).installation_id)?), ("local_version",match (&(_field_0).local_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("local_checksum",match (&(_field_0).local_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("package_path",match (&(_field_0).package_path).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("manifest_fingerprint",match (&(_field_0).manifest_fingerprint).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("artifact_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).artifact_status).len()))])), ("runtime_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).runtime_status).len()))])), ("availability_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).availability_status).len()))])), ("checked_at",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).checked_at).len()))])), ("last_error",match (&(_field_0).last_error).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Task(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Task".to_owned())), ("0",mp::object_value(&[("id",mp::text(&(_field_0).id)?), ("installation_id",match (&(_field_0).installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("workspace_id",match (&(_field_0).workspace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("provider_code",mp::text(&(_field_0).provider_code)?), ("task_kind",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).task_kind).len()))])), ("status",mp::text(&(_field_0).status)?), ("status_message",match (&(_field_0).status_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("detail_json",mp::json_summary(&(_field_0).detail_json)), ("created_at",mp::text(&(_field_0).created_at)?), ("updated_at",mp::text(&(_field_0).updated_at)?), ("finished_at",match (&(_field_0).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))]), Self::Tasks(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Tasks".to_owned())), ("0",{ if (_field_0).len() > 32 { return None; } serde_json::Value::Array((_field_0).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("installation_id",match (&(item).installation_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("workspace_id",match (&(item).workspace_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("provider_code",mp::text(&(item).provider_code)?), ("task_kind",mp::object_value(&[("byte_count",serde_json::json!((&(item).task_kind).len()))])), ("status",mp::text(&(item).status)?), ("status_message",match (&(item).status_message).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("detail_json",mp::json_summary(&(item).detail_json)), ("created_at",mp::text(&(item).created_at)?), ("updated_at",mp::text(&(item).updated_at)?), ("finished_at",match (&(item).finished_at).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })])})
    }

    const CONTRACT_ID: &'static str = "console-plugin-management-output";
    const CONTRACT_VERSION: &'static str = "1";
}
