use super::*;

impl InterfaceContract for McpBundlesInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[("variant", mp::tag_schema("ListOfficial"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewOfficial")),
                (
                    "0",
                    mp::union_schema(vec![
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("OfficialCatalog")),
                            (
                                "0",
                                mp::object_schema(&[
                                    (
                                        "organization",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    ("bundle_id", mp::text_schema()),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("InstalledExtension")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("extension_installation_id", mp::text_schema()),
                                    (
                                        "instance_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "integrity_override",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("BuiltinTemplate")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("builtin_template_id", mp::text_schema()),
                                    ("instance_id", mp::text_schema()),
                                ]),
                            ),
                        ]),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportOfficial")),
                (
                    "0",
                    mp::union_schema(vec![
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("OfficialCatalog")),
                            (
                                "0",
                                mp::object_schema(&[
                                    (
                                        "organization",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    ("bundle_id", mp::text_schema()),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("InstalledExtension")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("extension_installation_id", mp::text_schema()),
                                    (
                                        "instance_id",
                                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                    ),
                                    (
                                        "integrity_override",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("BuiltinTemplate")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("builtin_template_id", mp::text_schema()),
                                    ("instance_id", mp::text_schema()),
                                ]),
                            ),
                        ]),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Export")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "organization",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("bundle_id", mp::text_schema()),
                        ("bundle_version", mp::text_schema()),
                        ("locale", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("ExportDefaults"))]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ExportInstance")),
                ("instance_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "organization",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("bundle_id", mp::text_schema()),
                        ("bundle_version", mp::text_schema()),
                        ("locale", mp::text_schema()),
                        (
                            "export_profile",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Portable"))]), mp::object_schema(&[("variant",mp::tag_schema("OfficialBuiltin"))])]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewUploaded")),
                (
                    "bytes",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportUploaded")),
                (
                    "bytes",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ListLibrary")),
                ("refresh_remote", serde_json::json!({"type":"boolean"})),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SyncLibrary")),
                (
                    "organization",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("bundle_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "bundle_version",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewLibrary")),
                (
                    "organization",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("bundle_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "bundle_version",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportLibrary")),
                (
                    "organization",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("bundle_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "bundle_version",
                        serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SwitchLibrary")),
                (
                    "organization",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("bundle_id", mp::text_schema()),
                ("bundle_version", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("DeleteLibraryRelease")),
                (
                    "organization",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("bundle_id", mp::text_schema()),
                ("bundle_version", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RepairLibraryRelease")),
                (
                    "organization",
                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                ),
                ("bundle_id", mp::text_schema()),
                ("bundle_version", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::ListOfficial { .. } => mp::object_value(&[("variant",serde_json::Value::String("ListOfficial".to_owned()))]), Self::PreviewOfficial(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("PreviewOfficial".to_owned())), ("0",match _field_0 {crate::routes::settings_group::mcp_management::bundles::McpBundleSourceBody::OfficialCatalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("OfficialCatalog".to_owned())), ("0",mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).organization).len()))])), ("bundle_id",mp::text(&(_field_0).bundle_id)?)]))]), crate::routes::settings_group::mcp_management::bundles::McpBundleSourceBody::InstalledExtension(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("InstalledExtension".to_owned())), ("0",mp::object_value(&[("extension_installation_id",mp::text(&(_field_0).extension_installation_id)?), ("instance_id",match (&(_field_0).instance_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("integrity_override",match (&(_field_0).integrity_override).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("acknowledged_warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).acknowledged_warnings).len()))]))]), None => serde_json::Value::Null })]))]), crate::routes::settings_group::mcp_management::bundles::McpBundleSourceBody::BuiltinTemplate(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("BuiltinTemplate".to_owned())), ("0",mp::object_value(&[("builtin_template_id",mp::text(&(_field_0).builtin_template_id)?), ("instance_id",mp::text(&(_field_0).instance_id)?)]))])})]), Self::ImportOfficial(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ImportOfficial".to_owned())), ("0",match _field_0 {crate::routes::settings_group::mcp_management::bundles::McpBundleSourceBody::OfficialCatalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("OfficialCatalog".to_owned())), ("0",mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).organization).len()))])), ("bundle_id",mp::text(&(_field_0).bundle_id)?)]))]), crate::routes::settings_group::mcp_management::bundles::McpBundleSourceBody::InstalledExtension(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("InstalledExtension".to_owned())), ("0",mp::object_value(&[("extension_installation_id",mp::text(&(_field_0).extension_installation_id)?), ("instance_id",match (&(_field_0).instance_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("integrity_override",match (&(_field_0).integrity_override).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("acknowledged_warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).acknowledged_warnings).len()))]))]), None => serde_json::Value::Null })]))]), crate::routes::settings_group::mcp_management::bundles::McpBundleSourceBody::BuiltinTemplate(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("BuiltinTemplate".to_owned())), ("0",mp::object_value(&[("builtin_template_id",mp::text(&(_field_0).builtin_template_id)?), ("instance_id",mp::text(&(_field_0).instance_id)?)]))])})]), Self::Export(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Export".to_owned())), ("0",mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).organization).len()))])), ("bundle_id",mp::text(&(_field_0).bundle_id)?), ("bundle_version",mp::text(&(_field_0).bundle_version)?), ("locale",mp::text(&(_field_0).locale)?)]))]), Self::ExportDefaults => mp::object_value(&[("variant",serde_json::Value::String("ExportDefaults".to_owned()))]), Self::ExportInstance {instance_id: _field_instance_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("ExportInstance".to_owned())), ("instance_id",mp::text(_field_instance_id)?), ("body",mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).organization).len()))])), ("bundle_id",mp::text(&(_field_body).bundle_id)?), ("bundle_version",mp::text(&(_field_body).bundle_version)?), ("locale",mp::text(&(_field_body).locale)?), ("export_profile",match (&(_field_body).export_profile).as_ref() { Some(item) => match item {crate::routes::settings_group::mcp_management::bundles::McpInstanceBundleExportProfile::Portable => mp::object_value(&[("variant",serde_json::Value::String("Portable".to_owned()))]), crate::routes::settings_group::mcp_management::bundles::McpInstanceBundleExportProfile::OfficialBuiltin => mp::object_value(&[("variant",serde_json::Value::String("OfficialBuiltin".to_owned()))])}, None => serde_json::Value::Null })]))]), Self::PreviewUploaded {bytes: _field_bytes, .. } => mp::object_value(&[("variant",serde_json::Value::String("PreviewUploaded".to_owned())), ("bytes",mp::object_value(&[("byte_count",serde_json::json!((_field_bytes).len()))]))]), Self::ImportUploaded {bytes: _field_bytes, .. } => mp::object_value(&[("variant",serde_json::Value::String("ImportUploaded".to_owned())), ("bytes",mp::object_value(&[("byte_count",serde_json::json!((_field_bytes).len()))]))]), Self::ListLibrary {refresh_remote: _field_refresh_remote, .. } => mp::object_value(&[("variant",serde_json::Value::String("ListLibrary".to_owned())), ("refresh_remote",serde_json::Value::Bool(*(_field_refresh_remote)))]), Self::SyncLibrary {organization: _field_organization, bundle_id: _field_bundle_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("SyncLibrary".to_owned())), ("organization",mp::object_value(&[("byte_count",serde_json::json!((_field_organization).len()))])), ("bundle_id",mp::text(_field_bundle_id)?), ("body",mp::object_value(&[("bundle_version",match (&(_field_body).bundle_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))]), Self::PreviewLibrary {organization: _field_organization, bundle_id: _field_bundle_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("PreviewLibrary".to_owned())), ("organization",mp::object_value(&[("byte_count",serde_json::json!((_field_organization).len()))])), ("bundle_id",mp::text(_field_bundle_id)?), ("body",mp::object_value(&[("bundle_version",match (&(_field_body).bundle_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))]), Self::ImportLibrary {organization: _field_organization, bundle_id: _field_bundle_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("ImportLibrary".to_owned())), ("organization",mp::object_value(&[("byte_count",serde_json::json!((_field_organization).len()))])), ("bundle_id",mp::text(_field_bundle_id)?), ("body",mp::object_value(&[("bundle_version",match (&(_field_body).bundle_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))]), Self::SwitchLibrary {organization: _field_organization, bundle_id: _field_bundle_id, bundle_version: _field_bundle_version, .. } => mp::object_value(&[("variant",serde_json::Value::String("SwitchLibrary".to_owned())), ("organization",mp::object_value(&[("byte_count",serde_json::json!((_field_organization).len()))])), ("bundle_id",mp::text(_field_bundle_id)?), ("bundle_version",mp::text(_field_bundle_version)?)]), Self::DeleteLibraryRelease {organization: _field_organization, bundle_id: _field_bundle_id, bundle_version: _field_bundle_version, .. } => mp::object_value(&[("variant",serde_json::Value::String("DeleteLibraryRelease".to_owned())), ("organization",mp::object_value(&[("byte_count",serde_json::json!((_field_organization).len()))])), ("bundle_id",mp::text(_field_bundle_id)?), ("bundle_version",mp::text(_field_bundle_version)?)]), Self::RepairLibraryRelease {organization: _field_organization, bundle_id: _field_bundle_id, bundle_version: _field_bundle_version, .. } => mp::object_value(&[("variant",serde_json::Value::String("RepairLibraryRelease".to_owned())), ("organization",mp::object_value(&[("byte_count",serde_json::json!((_field_organization).len()))])), ("bundle_id",mp::text(_field_bundle_id)?), ("bundle_version",mp::text(_field_bundle_version)?)])})
    }

    const CONTRACT_ID: &'static str = "console-mcp-bundles-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for McpBundlesOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("OfficialCatalog")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "source",
                            mp::object_schema(&[
                                (
                                    "source_kind",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "source_label",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        (
                            "entries",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("latest_version",mp::text_schema()), ("locale",mp::text_schema()), ("minimum_host_version",mp::text_schema()), ("exported_from_system_version",mp::text_schema()), ("release_tag",mp::object_schema(&[("byte_count",mp::count_schema())])), ("artifact_sha256",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewOfficial")),
                (
                    "0",
                    mp::union_schema(vec![
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("OfficialCatalog")),
                            (
                                "0",
                                mp::object_schema(&[
                                    (
                                        "manifest",
                                        mp::object_schema(&[
                                            ("schema_version", mp::text_schema()),
                                            (
                                                "organization",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            ("bundle_id", mp::text_schema()),
                                            ("bundle_version", mp::text_schema()),
                                            ("locale", mp::text_schema()),
                                            ("minimum_host_version", mp::text_schema()),
                                            ("exported_from_system_version", mp::text_schema()),
                                            (
                                                "exported_at",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "files",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                    ("current_system_version", mp::text_schema()),
                                    (
                                        "version_status",
                                        mp::union_schema(vec![
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("SameSystemVersion"),
                                            )]),
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("ExportedFromOlderSystem"),
                                            )]),
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("ExportedFromNewerSystem"),
                                            )]),
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("UnknownSystemVersion"),
                                            )]),
                                        ]),
                                    ),
                                    (
                                        "effect_summary",
                                        mp::object_schema(&[
                                            ("changes", serde_json::json!({"type":"integer"})),
                                            (
                                                "already_present",
                                                serde_json::json!({"type":"integer"}),
                                            ),
                                            ("conflicts", serde_json::json!({"type":"integer"})),
                                            ("unavailable", serde_json::json!({"type":"integer"})),
                                            ("failed", serde_json::json!({"type":"integer"})),
                                        ]),
                                    ),
                                    (
                                        "tools",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "instances",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "connections",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "shared_tool_impacts",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("InstalledExtension")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("extension_installation_id", mp::text_schema()),
                                    (
                                        "artifact_installation_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "workspace_application_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "integrity_warnings",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "required_integrity_override",
                                        serde_json::json!({"anyOf": [mp::object_schema(&[("warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                                    ),
                                    (
                                        "preview",
                                        mp::object_schema(&[
                                            ("current_system_version", mp::text_schema()),
                                            (
                                                "version_status",
                                                mp::union_schema(vec![
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("SameSystemVersion"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromOlderSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromNewerSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("UnknownSystemVersion"),
                                                    )]),
                                                ]),
                                            ),
                                            (
                                                "tools",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "instances",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "connections",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "shared_tool_impacts",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("BuiltinTemplate")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("builtin_template_id", mp::text_schema()),
                                    (
                                        "workspace_application_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "preview",
                                        mp::object_schema(&[
                                            ("current_system_version", mp::text_schema()),
                                            (
                                                "version_status",
                                                mp::union_schema(vec![
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("SameSystemVersion"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromOlderSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromNewerSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("UnknownSystemVersion"),
                                                    )]),
                                                ]),
                                            ),
                                            (
                                                "tools",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "instances",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "connections",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "shared_tool_impacts",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                ]),
                            ),
                        ]),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportOfficial")),
                (
                    "0",
                    mp::union_schema(vec![
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("OfficialCatalog")),
                            (
                                "0",
                                mp::object_schema(&[
                                    (
                                        "manifest",
                                        mp::object_schema(&[
                                            ("schema_version", mp::text_schema()),
                                            (
                                                "organization",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            ("bundle_id", mp::text_schema()),
                                            ("bundle_version", mp::text_schema()),
                                            ("locale", mp::text_schema()),
                                            ("minimum_host_version", mp::text_schema()),
                                            ("exported_from_system_version", mp::text_schema()),
                                            (
                                                "exported_at",
                                                mp::object_schema(&[(
                                                    "byte_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "files",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                    ("current_system_version", mp::text_schema()),
                                    (
                                        "version_status",
                                        mp::union_schema(vec![
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("SameSystemVersion"),
                                            )]),
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("ExportedFromOlderSystem"),
                                            )]),
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("ExportedFromNewerSystem"),
                                            )]),
                                            mp::object_schema(&[(
                                                "variant",
                                                mp::tag_schema("UnknownSystemVersion"),
                                            )]),
                                        ]),
                                    ),
                                    ("status", mp::text_schema()),
                                    (
                                        "effect_summary",
                                        mp::object_schema(&[
                                            ("changes", serde_json::json!({"type":"integer"})),
                                            (
                                                "already_present",
                                                serde_json::json!({"type":"integer"}),
                                            ),
                                            ("conflicts", serde_json::json!({"type":"integer"})),
                                            ("unavailable", serde_json::json!({"type":"integer"})),
                                            ("failed", serde_json::json!({"type":"integer"})),
                                        ]),
                                    ),
                                    (
                                        "tools",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "instances",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "connections",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("InstalledExtension")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("extension_installation_id", mp::text_schema()),
                                    (
                                        "artifact_installation_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "workspace_application_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "integrity_warnings",
                                        mp::object_schema(&[("item_count", mp::count_schema())]),
                                    ),
                                    (
                                        "import_report",
                                        mp::object_schema(&[
                                            ("current_system_version", mp::text_schema()),
                                            (
                                                "version_status",
                                                mp::union_schema(vec![
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("SameSystemVersion"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromOlderSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromNewerSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("UnknownSystemVersion"),
                                                    )]),
                                                ]),
                                            ),
                                            ("status", mp::text_schema()),
                                            (
                                                "tools",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "instances",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "connections",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                ]),
                            ),
                        ]),
                        mp::object_schema(&[
                            ("variant", mp::tag_schema("BuiltinTemplate")),
                            (
                                "0",
                                mp::object_schema(&[
                                    ("builtin_template_id", mp::text_schema()),
                                    (
                                        "workspace_application_status",
                                        mp::object_schema(&[("byte_count", mp::count_schema())]),
                                    ),
                                    (
                                        "import_report",
                                        mp::object_schema(&[
                                            ("current_system_version", mp::text_schema()),
                                            (
                                                "version_status",
                                                mp::union_schema(vec![
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("SameSystemVersion"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromOlderSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("ExportedFromNewerSystem"),
                                                    )]),
                                                    mp::object_schema(&[(
                                                        "variant",
                                                        mp::tag_schema("UnknownSystemVersion"),
                                                    )]),
                                                ]),
                                            ),
                                            ("status", mp::text_schema()),
                                            (
                                                "tools",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "instances",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                            (
                                                "connections",
                                                mp::object_schema(&[(
                                                    "item_count",
                                                    mp::count_schema(),
                                                )]),
                                            ),
                                        ]),
                                    ),
                                ]),
                            ),
                        ]),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("IntegrityChallenge")),
                (
                    "0",
                    mp::object_schema(&[
                        ("status", serde_json::json!({"type":"integer"})),
                        ("code", mp::text_schema()),
                        (
                            "message",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("extension_installation_id", mp::text_schema()),
                        (
                            "artifact_installation_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "workspace_application_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "integrity_warnings",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("overridable",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "required_integrity_override",
                            mp::object_schema(&[
                                (
                                    "warnings",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "compatibility",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_host_version",mp::text_schema()), ("minimum_host_version",mp::text_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "preview",
                            mp::object_schema(&[
                                (
                                    "manifest",
                                    mp::object_schema(&[
                                        ("schema_version", mp::text_schema()),
                                        (
                                            "organization",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        ("bundle_id", mp::text_schema()),
                                        ("bundle_version", mp::text_schema()),
                                        ("locale", mp::text_schema()),
                                        ("minimum_host_version", mp::text_schema()),
                                        ("exported_from_system_version", mp::text_schema()),
                                        (
                                            "exported_at",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "files",
                                            mp::object_schema(&[(
                                                "item_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                    ]),
                                ),
                                ("current_system_version", mp::text_schema()),
                                (
                                    "version_status",
                                    mp::union_schema(vec![
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("SameSystemVersion"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("ExportedFromOlderSystem"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("ExportedFromNewerSystem"),
                                        )]),
                                        mp::object_schema(&[(
                                            "variant",
                                            mp::tag_schema("UnknownSystemVersion"),
                                        )]),
                                    ]),
                                ),
                                (
                                    "effect_summary",
                                    mp::object_schema(&[
                                        ("changes", serde_json::json!({"type":"integer"})),
                                        ("already_present", serde_json::json!({"type":"integer"})),
                                        ("conflicts", serde_json::json!({"type":"integer"})),
                                        ("unavailable", serde_json::json!({"type":"integer"})),
                                        ("failed", serde_json::json!({"type":"integer"})),
                                    ]),
                                ),
                                (
                                    "tools",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "instances",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "connections",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "shared_tool_impacts",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Archive")),
                (
                    "0",
                    mp::object_schema(&[
                        ("status", serde_json::json!({"type":"integer"})),
                        ("content_type", mp::text_schema()),
                        (
                            "bytes",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ExportDefaults")),
                (
                    "0",
                    mp::object_schema(&[
                        ("minimum_host_version", mp::text_schema()),
                        ("current_system_version", mp::text_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Preview")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "manifest",
                            mp::object_schema(&[
                                ("schema_version", mp::text_schema()),
                                (
                                    "organization",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("bundle_id", mp::text_schema()),
                                ("bundle_version", mp::text_schema()),
                                ("locale", mp::text_schema()),
                                ("minimum_host_version", mp::text_schema()),
                                ("exported_from_system_version", mp::text_schema()),
                                (
                                    "exported_at",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "files",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        ("current_system_version", mp::text_schema()),
                        (
                            "version_status",
                            mp::union_schema(vec![
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("SameSystemVersion"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("ExportedFromOlderSystem"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("ExportedFromNewerSystem"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("UnknownSystemVersion"),
                                )]),
                            ]),
                        ),
                        (
                            "effect_summary",
                            mp::object_schema(&[
                                ("changes", serde_json::json!({"type":"integer"})),
                                ("already_present", serde_json::json!({"type":"integer"})),
                                ("conflicts", serde_json::json!({"type":"integer"})),
                                ("unavailable", serde_json::json!({"type":"integer"})),
                                ("failed", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "tools",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("effect",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Create"))]), mp::object_schema(&[("variant",mp::tag_schema("Update"))]), mp::object_schema(&[("variant",mp::tag_schema("AlreadyPresent"))]), mp::object_schema(&[("variant",mp::tag_schema("Conflict"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])])), ("result",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "instances",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("effect",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Create"))]), mp::object_schema(&[("variant",mp::tag_schema("Update"))]), mp::object_schema(&[("variant",mp::tag_schema("AlreadyPresent"))]), mp::object_schema(&[("variant",mp::tag_schema("Conflict"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])])), ("result",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "connections",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("effect",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Create"))]), mp::object_schema(&[("variant",mp::tag_schema("Update"))]), mp::object_schema(&[("variant",mp::tag_schema("AlreadyPresent"))]), mp::object_schema(&[("variant",mp::tag_schema("Conflict"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])])), ("result",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "shared_tool_impacts",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("tool_id",mp::text_schema()), ("instance_ids",serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Import")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "manifest",
                            mp::object_schema(&[
                                ("schema_version", mp::text_schema()),
                                (
                                    "organization",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("bundle_id", mp::text_schema()),
                                ("bundle_version", mp::text_schema()),
                                ("locale", mp::text_schema()),
                                ("minimum_host_version", mp::text_schema()),
                                ("exported_from_system_version", mp::text_schema()),
                                (
                                    "exported_at",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "files",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        ("current_system_version", mp::text_schema()),
                        (
                            "version_status",
                            mp::union_schema(vec![
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("SameSystemVersion"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("ExportedFromOlderSystem"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("ExportedFromNewerSystem"),
                                )]),
                                mp::object_schema(&[(
                                    "variant",
                                    mp::tag_schema("UnknownSystemVersion"),
                                )]),
                            ]),
                        ),
                        ("status", mp::text_schema()),
                        (
                            "effect_summary",
                            mp::object_schema(&[
                                ("changes", serde_json::json!({"type":"integer"})),
                                ("already_present", serde_json::json!({"type":"integer"})),
                                ("conflicts", serde_json::json!({"type":"integer"})),
                                ("unavailable", serde_json::json!({"type":"integer"})),
                                ("failed", serde_json::json!({"type":"integer"})),
                            ]),
                        ),
                        (
                            "tools",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("effect",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Create"))]), mp::object_schema(&[("variant",mp::tag_schema("Update"))]), mp::object_schema(&[("variant",mp::tag_schema("AlreadyPresent"))]), mp::object_schema(&[("variant",mp::tag_schema("Conflict"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])])), ("result",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "instances",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("effect",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Create"))]), mp::object_schema(&[("variant",mp::tag_schema("Update"))]), mp::object_schema(&[("variant",mp::tag_schema("AlreadyPresent"))]), mp::object_schema(&[("variant",mp::tag_schema("Conflict"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])])), ("result",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "connections",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("effect",mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Create"))]), mp::object_schema(&[("variant",mp::tag_schema("Update"))]), mp::object_schema(&[("variant",mp::tag_schema("AlreadyPresent"))]), mp::object_schema(&[("variant",mp::tag_schema("Conflict"))]), mp::object_schema(&[("variant",mp::tag_schema("Failed"))])])), ("result",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Library")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "source",
                            mp::object_schema(&[
                                (
                                    "source_kind",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "source_label",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                            ]),
                        ),
                        ("remote_available", serde_json::json!({"type":"boolean"})),
                        (
                            "remote_error",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "bundles",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("organization",mp::object_schema(&[("byte_count",mp::count_schema())])), ("bundle_id",mp::text_schema()), ("source_path",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("remote_versions",mp::object_schema(&[("item_count",mp::count_schema())])), ("local_versions",mp::object_schema(&[("item_count",mp::count_schema())])), ("current_bundle_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}))])}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("LibraryReceipt")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "organization",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("bundle_id", mp::text_schema()),
                        ("bundle_version", mp::text_schema()),
                        ("locale", mp::text_schema()),
                        ("minimum_host_version", mp::text_schema()),
                        ("exported_from_system_version", mp::text_schema()),
                        (
                            "release_tag",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "checksum",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "algorithm",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        ("key_id", mp::text_schema()),
                        (
                            "signature",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[("variant", mp::tag_schema("Deleted"))]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::OfficialCatalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("OfficialCatalog".to_owned())), ("0",mp::object_value(&[("source",mp::object_value(&[("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).source).source_kind).len()))])), ("source_label",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).source).source_label).len()))]))])), ("entries",{ if (&(_field_0).entries).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).entries).iter().map(|item| Some(mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("bundle_id",mp::text(&(item).bundle_id)?), ("latest_version",mp::text(&(item).latest_version)?), ("locale",mp::text(&(item).locale)?), ("minimum_host_version",mp::text(&(item).minimum_host_version)?), ("exported_from_system_version",mp::text(&(item).exported_from_system_version)?), ("release_tag",mp::object_value(&[("byte_count",serde_json::json!((&(item).release_tag).len()))])), ("artifact_sha256",match (&(item).artifact_sha256).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::PreviewOfficial(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("PreviewOfficial".to_owned())), ("0",match _field_0 {crate::routes::settings_group::mcp_management::bundles::McpBundlePreviewSourceResponse::OfficialCatalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("OfficialCatalog".to_owned())), ("0",mp::object_value(&[("manifest",mp::object_value(&[("schema_version",mp::text(&(&(_field_0).manifest).schema_version)?), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).organization).len()))])), ("bundle_id",mp::text(&(&(_field_0).manifest).bundle_id)?), ("bundle_version",mp::text(&(&(_field_0).manifest).bundle_version)?), ("locale",mp::text(&(&(_field_0).manifest).locale)?), ("minimum_host_version",mp::text(&(&(_field_0).manifest).minimum_host_version)?), ("exported_from_system_version",mp::text(&(&(_field_0).manifest).exported_from_system_version)?), ("exported_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).exported_at).len()))])), ("files",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).manifest).files).len()))]))])), ("current_system_version",mp::text(&(_field_0).current_system_version)?), ("version_status",match &(_field_0).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("effect_summary",mp::object_value(&[("changes",serde_json::json!(*(&(&(_field_0).effect_summary).changes))), ("already_present",serde_json::json!(*(&(&(_field_0).effect_summary).already_present))), ("conflicts",serde_json::json!(*(&(&(_field_0).effect_summary).conflicts))), ("unavailable",serde_json::json!(*(&(&(_field_0).effect_summary).unavailable))), ("failed",serde_json::json!(*(&(&(_field_0).effect_summary).failed)))])), ("tools",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).tools).len()))])), ("instances",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).instances).len()))])), ("connections",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).connections).len()))])), ("shared_tool_impacts",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).shared_tool_impacts).len()))]))]))]), crate::routes::settings_group::mcp_management::bundles::McpBundlePreviewSourceResponse::InstalledExtension(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("InstalledExtension".to_owned())), ("0",mp::object_value(&[("extension_installation_id",mp::text(&(_field_0).extension_installation_id)?), ("artifact_installation_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).artifact_installation_status).len()))])), ("workspace_application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).workspace_application_status).len()))])), ("integrity_warnings",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).integrity_warnings).len()))])), ("required_integrity_override",match (&(_field_0).required_integrity_override).as_ref() { Some(item) => mp::object_value(&[("warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).warnings).len()))]))]), None => serde_json::Value::Null }), ("preview",mp::object_value(&[("current_system_version",mp::text(&(&(_field_0).preview).current_system_version)?), ("version_status",match &(&(_field_0).preview).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("tools",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).tools).len()))])), ("instances",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).instances).len()))])), ("connections",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).connections).len()))])), ("shared_tool_impacts",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).shared_tool_impacts).len()))]))]))]))]), crate::routes::settings_group::mcp_management::bundles::McpBundlePreviewSourceResponse::BuiltinTemplate(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("BuiltinTemplate".to_owned())), ("0",mp::object_value(&[("builtin_template_id",mp::text(&(_field_0).builtin_template_id)?), ("workspace_application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).workspace_application_status).len()))])), ("preview",mp::object_value(&[("current_system_version",mp::text(&(&(_field_0).preview).current_system_version)?), ("version_status",match &(&(_field_0).preview).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("tools",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).tools).len()))])), ("instances",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).instances).len()))])), ("connections",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).connections).len()))])), ("shared_tool_impacts",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).shared_tool_impacts).len()))]))]))]))])})]), Self::ImportOfficial(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ImportOfficial".to_owned())), ("0",match _field_0 {crate::routes::settings_group::mcp_management::bundles::McpBundleImportSourceResponse::OfficialCatalog(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("OfficialCatalog".to_owned())), ("0",mp::object_value(&[("manifest",mp::object_value(&[("schema_version",mp::text(&(&(_field_0).manifest).schema_version)?), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).organization).len()))])), ("bundle_id",mp::text(&(&(_field_0).manifest).bundle_id)?), ("bundle_version",mp::text(&(&(_field_0).manifest).bundle_version)?), ("locale",mp::text(&(&(_field_0).manifest).locale)?), ("minimum_host_version",mp::text(&(&(_field_0).manifest).minimum_host_version)?), ("exported_from_system_version",mp::text(&(&(_field_0).manifest).exported_from_system_version)?), ("exported_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).exported_at).len()))])), ("files",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).manifest).files).len()))]))])), ("current_system_version",mp::text(&(_field_0).current_system_version)?), ("version_status",match &(_field_0).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("status",mp::text(&(_field_0).status)?), ("effect_summary",mp::object_value(&[("changes",serde_json::json!(*(&(&(_field_0).effect_summary).changes))), ("already_present",serde_json::json!(*(&(&(_field_0).effect_summary).already_present))), ("conflicts",serde_json::json!(*(&(&(_field_0).effect_summary).conflicts))), ("unavailable",serde_json::json!(*(&(&(_field_0).effect_summary).unavailable))), ("failed",serde_json::json!(*(&(&(_field_0).effect_summary).failed)))])), ("tools",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).tools).len()))])), ("instances",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).instances).len()))])), ("connections",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).connections).len()))]))]))]), crate::routes::settings_group::mcp_management::bundles::McpBundleImportSourceResponse::InstalledExtension(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("InstalledExtension".to_owned())), ("0",mp::object_value(&[("extension_installation_id",mp::text(&(_field_0).extension_installation_id)?), ("artifact_installation_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).artifact_installation_status).len()))])), ("workspace_application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).workspace_application_status).len()))])), ("integrity_warnings",mp::object_value(&[("item_count",serde_json::json!((&(_field_0).integrity_warnings).len()))])), ("import_report",mp::object_value(&[("current_system_version",mp::text(&(&(_field_0).import_report).current_system_version)?), ("version_status",match &(&(_field_0).import_report).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("status",mp::text(&(&(_field_0).import_report).status)?), ("tools",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).import_report).tools).len()))])), ("instances",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).import_report).instances).len()))])), ("connections",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).import_report).connections).len()))]))]))]))]), crate::routes::settings_group::mcp_management::bundles::McpBundleImportSourceResponse::BuiltinTemplate(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("BuiltinTemplate".to_owned())), ("0",mp::object_value(&[("builtin_template_id",mp::text(&(_field_0).builtin_template_id)?), ("workspace_application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).workspace_application_status).len()))])), ("import_report",mp::object_value(&[("current_system_version",mp::text(&(&(_field_0).import_report).current_system_version)?), ("version_status",match &(&(_field_0).import_report).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("status",mp::text(&(&(_field_0).import_report).status)?), ("tools",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).import_report).tools).len()))])), ("instances",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).import_report).instances).len()))])), ("connections",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).import_report).connections).len()))]))]))]))])})]), Self::IntegrityChallenge(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("IntegrityChallenge".to_owned())), ("0",mp::object_value(&[("status",serde_json::json!(*(&(_field_0).status))), ("code",mp::text(&(_field_0).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).message).len()))])), ("extension_installation_id",mp::text(&(_field_0).extension_installation_id)?), ("artifact_installation_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).artifact_installation_status).len()))])), ("workspace_application_status",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).workspace_application_status).len()))])), ("integrity_warnings",{ if (&(_field_0).integrity_warnings).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).integrity_warnings).iter().map(|item| Some(mp::object_value(&[("code",mp::text(&(item).code)?), ("message",mp::object_value(&[("byte_count",serde_json::json!((&(item).message).len()))])), ("overridable",serde_json::Value::Bool(*(&(item).overridable)))]))).collect::<Option<Vec<_>>>()?) }), ("required_integrity_override",mp::object_value(&[("warnings",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).required_integrity_override).warnings).len()))])), ("compatibility",match (&(&(_field_0).required_integrity_override).compatibility).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("current_host_version",mp::text(&(item).current_host_version)?), ("minimum_host_version",mp::text(&(item).minimum_host_version)?)]), None => serde_json::Value::Null })])), ("preview",mp::object_value(&[("manifest",mp::object_value(&[("schema_version",mp::text(&(&(&(_field_0).preview).manifest).schema_version)?), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).preview).manifest).organization).len()))])), ("bundle_id",mp::text(&(&(&(_field_0).preview).manifest).bundle_id)?), ("bundle_version",mp::text(&(&(&(_field_0).preview).manifest).bundle_version)?), ("locale",mp::text(&(&(&(_field_0).preview).manifest).locale)?), ("minimum_host_version",mp::text(&(&(&(_field_0).preview).manifest).minimum_host_version)?), ("exported_from_system_version",mp::text(&(&(&(_field_0).preview).manifest).exported_from_system_version)?), ("exported_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(&(_field_0).preview).manifest).exported_at).len()))])), ("files",mp::object_value(&[("item_count",serde_json::json!((&(&(&(_field_0).preview).manifest).files).len()))]))])), ("current_system_version",mp::text(&(&(_field_0).preview).current_system_version)?), ("version_status",match &(&(_field_0).preview).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("effect_summary",mp::object_value(&[("changes",serde_json::json!(*(&(&(&(_field_0).preview).effect_summary).changes))), ("already_present",serde_json::json!(*(&(&(&(_field_0).preview).effect_summary).already_present))), ("conflicts",serde_json::json!(*(&(&(&(_field_0).preview).effect_summary).conflicts))), ("unavailable",serde_json::json!(*(&(&(&(_field_0).preview).effect_summary).unavailable))), ("failed",serde_json::json!(*(&(&(&(_field_0).preview).effect_summary).failed)))])), ("tools",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).tools).len()))])), ("instances",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).instances).len()))])), ("connections",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).connections).len()))])), ("shared_tool_impacts",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).preview).shared_tool_impacts).len()))]))]))]))]), Self::Archive(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Archive".to_owned())), ("0",mp::object_value(&[("status",serde_json::json!(*(&(_field_0).status))), ("content_type",mp::text(&(_field_0).content_type)?), ("bytes",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).bytes).len()))]))]))]), Self::ExportDefaults(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ExportDefaults".to_owned())), ("0",mp::object_value(&[("minimum_host_version",mp::text(&(_field_0).minimum_host_version)?), ("current_system_version",mp::text(&(_field_0).current_system_version)?)]))]), Self::Preview(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Preview".to_owned())), ("0",mp::object_value(&[("manifest",mp::object_value(&[("schema_version",mp::text(&(&(_field_0).manifest).schema_version)?), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).organization).len()))])), ("bundle_id",mp::text(&(&(_field_0).manifest).bundle_id)?), ("bundle_version",mp::text(&(&(_field_0).manifest).bundle_version)?), ("locale",mp::text(&(&(_field_0).manifest).locale)?), ("minimum_host_version",mp::text(&(&(_field_0).manifest).minimum_host_version)?), ("exported_from_system_version",mp::text(&(&(_field_0).manifest).exported_from_system_version)?), ("exported_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).exported_at).len()))])), ("files",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).manifest).files).len()))]))])), ("current_system_version",mp::text(&(_field_0).current_system_version)?), ("version_status",match &(_field_0).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("effect_summary",mp::object_value(&[("changes",serde_json::json!(*(&(&(_field_0).effect_summary).changes))), ("already_present",serde_json::json!(*(&(&(_field_0).effect_summary).already_present))), ("conflicts",serde_json::json!(*(&(&(_field_0).effect_summary).conflicts))), ("unavailable",serde_json::json!(*(&(&(_field_0).effect_summary).unavailable))), ("failed",serde_json::json!(*(&(&(_field_0).effect_summary).failed)))])), ("tools",{ if (&(_field_0).tools).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).tools).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("effect",match &(item).effect {domain::mcp_bundle::McpBundleItemEffect::Create => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Update => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::AlreadyPresent => mp::object_value(&[("variant",serde_json::Value::String("AlreadyPresent".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Conflict => mp::object_value(&[("variant",serde_json::Value::String("Conflict".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}), ("result",mp::object_value(&[("byte_count",serde_json::json!((&(item).result).len()))])), ("reason",match (&(item).reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("instances",{ if (&(_field_0).instances).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).instances).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("effect",match &(item).effect {domain::mcp_bundle::McpBundleItemEffect::Create => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Update => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::AlreadyPresent => mp::object_value(&[("variant",serde_json::Value::String("AlreadyPresent".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Conflict => mp::object_value(&[("variant",serde_json::Value::String("Conflict".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}), ("result",mp::object_value(&[("byte_count",serde_json::json!((&(item).result).len()))])), ("reason",match (&(item).reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("connections",{ if (&(_field_0).connections).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).connections).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("effect",match &(item).effect {domain::mcp_bundle::McpBundleItemEffect::Create => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Update => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::AlreadyPresent => mp::object_value(&[("variant",serde_json::Value::String("AlreadyPresent".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Conflict => mp::object_value(&[("variant",serde_json::Value::String("Conflict".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}), ("result",mp::object_value(&[("byte_count",serde_json::json!((&(item).result).len()))])), ("reason",match (&(item).reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("shared_tool_impacts",{ if (&(_field_0).shared_tool_impacts).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).shared_tool_impacts).iter().map(|item| Some(mp::object_value(&[("tool_id",mp::text(&(item).tool_id)?), ("instance_ids",{ if (&(item).instance_ids).len() > 32 { return None; } serde_json::Value::Array((&(item).instance_ids).iter().map(|item| Some(mp::text(item)?)).collect::<Option<Vec<_>>>()?) })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Import(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Import".to_owned())), ("0",mp::object_value(&[("manifest",mp::object_value(&[("schema_version",mp::text(&(&(_field_0).manifest).schema_version)?), ("organization",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).organization).len()))])), ("bundle_id",mp::text(&(&(_field_0).manifest).bundle_id)?), ("bundle_version",mp::text(&(&(_field_0).manifest).bundle_version)?), ("locale",mp::text(&(&(_field_0).manifest).locale)?), ("minimum_host_version",mp::text(&(&(_field_0).manifest).minimum_host_version)?), ("exported_from_system_version",mp::text(&(&(_field_0).manifest).exported_from_system_version)?), ("exported_at",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).manifest).exported_at).len()))])), ("files",mp::object_value(&[("item_count",serde_json::json!((&(&(_field_0).manifest).files).len()))]))])), ("current_system_version",mp::text(&(_field_0).current_system_version)?), ("version_status",match &(_field_0).version_status {domain::mcp_bundle::McpBundleVersionStatus::SameSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("SameSystemVersion".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromOlderSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromOlderSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::ExportedFromNewerSystem => mp::object_value(&[("variant",serde_json::Value::String("ExportedFromNewerSystem".to_owned()))]), domain::mcp_bundle::McpBundleVersionStatus::UnknownSystemVersion => mp::object_value(&[("variant",serde_json::Value::String("UnknownSystemVersion".to_owned()))])}), ("status",mp::text(&(_field_0).status)?), ("effect_summary",mp::object_value(&[("changes",serde_json::json!(*(&(&(_field_0).effect_summary).changes))), ("already_present",serde_json::json!(*(&(&(_field_0).effect_summary).already_present))), ("conflicts",serde_json::json!(*(&(&(_field_0).effect_summary).conflicts))), ("unavailable",serde_json::json!(*(&(&(_field_0).effect_summary).unavailable))), ("failed",serde_json::json!(*(&(&(_field_0).effect_summary).failed)))])), ("tools",{ if (&(_field_0).tools).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).tools).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("effect",match &(item).effect {domain::mcp_bundle::McpBundleItemEffect::Create => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Update => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::AlreadyPresent => mp::object_value(&[("variant",serde_json::Value::String("AlreadyPresent".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Conflict => mp::object_value(&[("variant",serde_json::Value::String("Conflict".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}), ("result",mp::object_value(&[("byte_count",serde_json::json!((&(item).result).len()))])), ("reason",match (&(item).reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("instances",{ if (&(_field_0).instances).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).instances).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("effect",match &(item).effect {domain::mcp_bundle::McpBundleItemEffect::Create => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Update => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::AlreadyPresent => mp::object_value(&[("variant",serde_json::Value::String("AlreadyPresent".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Conflict => mp::object_value(&[("variant",serde_json::Value::String("Conflict".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}), ("result",mp::object_value(&[("byte_count",serde_json::json!((&(item).result).len()))])), ("reason",match (&(item).reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("connections",{ if (&(_field_0).connections).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).connections).iter().map(|item| Some(mp::object_value(&[("id",mp::text(&(item).id)?), ("effect",match &(item).effect {domain::mcp_bundle::McpBundleItemEffect::Create => mp::object_value(&[("variant",serde_json::Value::String("Create".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Update => mp::object_value(&[("variant",serde_json::Value::String("Update".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::AlreadyPresent => mp::object_value(&[("variant",serde_json::Value::String("AlreadyPresent".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Conflict => mp::object_value(&[("variant",serde_json::Value::String("Conflict".to_owned()))]), domain::mcp_bundle::McpBundleItemEffect::Failed => mp::object_value(&[("variant",serde_json::Value::String("Failed".to_owned()))])}), ("result",mp::object_value(&[("byte_count",serde_json::json!((&(item).result).len()))])), ("reason",match (&(item).reason).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::Library(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("Library".to_owned())), ("0",mp::object_value(&[("source",mp::object_value(&[("source_kind",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).source).source_kind).len()))])), ("source_label",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).source).source_label).len()))]))])), ("remote_available",serde_json::Value::Bool(*(&(_field_0).remote_available))), ("remote_error",match (&(_field_0).remote_error).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("bundles",{ if (&(_field_0).bundles).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).bundles).iter().map(|item| Some(mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(item).organization).len()))])), ("bundle_id",mp::text(&(item).bundle_id)?), ("source_path",match (&(item).source_path).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("remote_versions",mp::object_value(&[("item_count",serde_json::json!((&(item).remote_versions).len()))])), ("local_versions",mp::object_value(&[("item_count",serde_json::json!((&(item).local_versions).len()))])), ("current_bundle_version",match (&(item).current_bundle_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) })]))]), Self::LibraryReceipt(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("LibraryReceipt".to_owned())), ("0",mp::object_value(&[("organization",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).organization).len()))])), ("bundle_id",mp::text(&(_field_0).bundle_id)?), ("bundle_version",mp::text(&(_field_0).bundle_version)?), ("locale",mp::text(&(_field_0).locale)?), ("minimum_host_version",mp::text(&(_field_0).minimum_host_version)?), ("exported_from_system_version",mp::text(&(_field_0).exported_from_system_version)?), ("release_tag",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).release_tag).len()))])), ("checksum",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).checksum).len()))])), ("algorithm",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).algorithm).len()))])), ("key_id",mp::text(&(_field_0).key_id)?), ("signature",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).signature).len()))]))]))]), Self::Deleted => mp::object_value(&[("variant",serde_json::Value::String("Deleted".to_owned()))])})
    }

    const CONTRACT_ID: &'static str = "console-mcp-bundles-output";
    const CONTRACT_VERSION: &'static str = "1";
}
