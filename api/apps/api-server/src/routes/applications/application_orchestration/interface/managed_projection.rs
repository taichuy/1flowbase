use super::*;

impl InterfaceContract for ApplicationOrchestrationInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
                ("application_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("SaveDraft")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        ("document", mp::json_summary_schema()),
                        (
                            "change_kind",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "summary",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("RestoreVersion")),
                ("application_id", mp::text_schema()),
                ("version_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("UpdateVersion")),
                ("application_id", mp::text_schema()),
                ("version_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "summary",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "summary_is_custom",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                        (
                            "is_user_protected",
                            serde_json::json!({"anyOf": [serde_json::json!({"type":"boolean"}), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewUploadedArchive")),
                (
                    "0",
                    mp::object_schema(&[
                        ("template_id", mp::text_schema()),
                        ("release_version", serde_json::json!({"type":"integer"})),
                        ("exported_from_system_version", mp::text_schema()),
                        (
                            "exported_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "application",
                            mp::object_schema(&[
                                ("application_type", mp::text_schema()),
                                (
                                    "workflow_trigger_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "description",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "icon",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "icon_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "icon_background",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        ("flow_document", mp::json_summary_schema()),
                        (
                            "dependencies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::text_schema()), ("node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("config_version",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("provider_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("model_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("plugin_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("plugin_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("contribution_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_shell",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("package_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("contribution_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("compiled_contribution_hash",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "workflow_trigger_config",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Schedule")), ("cron",mp::object_schema(&[("byte_count",mp::count_schema())])), ("timezone",mp::object_schema(&[("byte_count",mp::count_schema())])), ("input_payload",mp::json_summary_schema())]), mp::object_schema(&[("variant",mp::tag_schema("Extension"))])]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportUploadedArchive")),
                (
                    "entry",
                    mp::object_schema(&[
                        ("template_id", mp::text_schema()),
                        ("release_version", serde_json::json!({"type":"integer"})),
                        ("exported_from_system_version", mp::text_schema()),
                        (
                            "exported_at",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "application",
                            mp::object_schema(&[
                                ("application_type", mp::text_schema()),
                                (
                                    "workflow_trigger_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "description",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "icon",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "icon_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "icon_background",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        ("flow_document", mp::json_summary_schema()),
                        (
                            "dependencies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("kind",mp::text_schema()), ("node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("config_version",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("provider_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("model_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("plugin_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("plugin_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("contribution_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_shell",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("package_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("contribution_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("compiled_contribution_hash",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "workflow_trigger_config",
                            serde_json::json!({"anyOf": [mp::union_schema(vec![mp::object_schema(&[("variant",mp::tag_schema("Schedule")), ("cron",mp::object_schema(&[("byte_count",mp::count_schema())])), ("timezone",mp::object_schema(&[("byte_count",mp::count_schema())])), ("input_payload",mp::json_summary_schema())]), mp::object_schema(&[("variant",mp::tag_schema("Extension"))])]), {"type":"null"}]}),
                        ),
                    ]),
                ),
                (
                    "name",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
                (
                    "description",
                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ExportArchive")),
                (
                    "0",
                    mp::object_schema(&[(
                        "application_ids",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("PreviewInstalledArchive")),
                ("0", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ImportInstalledArchive")),
                ("installation_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[
                        (
                            "name",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "description",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                        ),
                        (
                            "integrity_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("acknowledged_warnings",mp::object_schema(&[("item_count",mp::count_schema())]))]), {"type":"null"}]}),
                        ),
                    ]),
                ),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {Self::Get {application_id: _field_application_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("Get".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string()))]), Self::SaveDraft {application_id: _field_application_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("SaveDraft".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("body",mp::object_value(&[("document",mp::json_summary(&(_field_body).document)), ("change_kind",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).change_kind).len()))])), ("summary",mp::object_value(&[("byte_count",serde_json::json!((&(_field_body).summary).len()))]))]))]), Self::RestoreVersion {application_id: _field_application_id, version_id: _field_version_id, .. } => mp::object_value(&[("variant",serde_json::Value::String("RestoreVersion".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("version_id",serde_json::Value::String((_field_version_id).to_string()))]), Self::UpdateVersion {application_id: _field_application_id, version_id: _field_version_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("UpdateVersion".to_owned())), ("application_id",serde_json::Value::String((_field_application_id).to_string())), ("version_id",serde_json::Value::String((_field_version_id).to_string())), ("body",mp::object_value(&[("summary",match (&(_field_body).summary).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("summary_is_custom",match (&(_field_body).summary_is_custom).as_ref() { Some(item) => serde_json::Value::Bool(*(item)), None => serde_json::Value::Null }), ("is_user_protected",match (&(_field_body).is_user_protected).as_ref() { Some(item) => serde_json::Value::Bool(*(item)), None => serde_json::Value::Null })]))]), Self::PreviewUploadedArchive(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("PreviewUploadedArchive".to_owned())), ("0",mp::object_value(&[("template_id",mp::text(&(_field_0).template_id)?), ("release_version",serde_json::json!(*(&(_field_0).release_version))), ("exported_from_system_version",mp::text(&(_field_0).exported_from_system_version)?), ("exported_at",mp::object_value(&[("byte_count",serde_json::json!((&(_field_0).exported_at).len()))])), ("application",mp::object_value(&[("application_type",mp::text(&(&(_field_0).application).application_type)?), ("workflow_trigger_type",match (&(&(_field_0).application).workflow_trigger_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).application).name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_0).application).description).len()))])), ("icon",match (&(&(_field_0).application).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon_type",match (&(&(_field_0).application).icon_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("icon_background",match (&(&(_field_0).application).icon_background).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("flow_document",mp::json_summary(&(_field_0).flow_document)), ("dependencies",{ if (&(_field_0).dependencies).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).dependencies).iter().map(|item| Some(mp::object_value(&[("kind",mp::text(&(item).kind)?), ("node_id",match (&(item).node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_type",match (&(item).node_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("config_version",match (&(item).config_version).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("provider_code",match (&(item).provider_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("model_id",match (&(item).model_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("plugin_id",match (&(item).plugin_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("plugin_version",match (&(item).plugin_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("contribution_code",match (&(item).contribution_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_shell",match (&(item).node_shell).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",match (&(item).schema_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("package_id",match (&(item).package_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("contribution_checksum",match (&(item).contribution_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("compiled_contribution_hash",match (&(item).compiled_contribution_hash).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("workflow_trigger_config",match (&(_field_0).workflow_trigger_config).as_ref() { Some(item) => match item {control_plane::application::WorkflowTriggerTemplateConfig::Schedule {cron: _field_cron, timezone: _field_timezone, input_payload: _field_input_payload, .. } => mp::object_value(&[("variant",serde_json::Value::String("Schedule".to_owned())), ("cron",mp::object_value(&[("byte_count",serde_json::json!((_field_cron).len()))])), ("timezone",mp::object_value(&[("byte_count",serde_json::json!((_field_timezone).len()))])), ("input_payload",mp::json_summary(_field_input_payload))]), control_plane::application::WorkflowTriggerTemplateConfig::Extension { .. } => mp::object_value(&[("variant",serde_json::Value::String("Extension".to_owned()))])}, None => serde_json::Value::Null })]))]), Self::ImportUploadedArchive {entry: _field_entry, name: _field_name, description: _field_description, .. } => mp::object_value(&[("variant",serde_json::Value::String("ImportUploadedArchive".to_owned())), ("entry",mp::object_value(&[("template_id",mp::text(&(_field_entry).template_id)?), ("release_version",serde_json::json!(*(&(_field_entry).release_version))), ("exported_from_system_version",mp::text(&(_field_entry).exported_from_system_version)?), ("exported_at",mp::object_value(&[("byte_count",serde_json::json!((&(_field_entry).exported_at).len()))])), ("application",mp::object_value(&[("application_type",mp::text(&(&(_field_entry).application).application_type)?), ("workflow_trigger_type",match (&(&(_field_entry).application).workflow_trigger_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("name",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_entry).application).name).len()))])), ("description",mp::object_value(&[("byte_count",serde_json::json!((&(&(_field_entry).application).description).len()))])), ("icon",match (&(&(_field_entry).application).icon).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("icon_type",match (&(&(_field_entry).application).icon_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("icon_background",match (&(&(_field_entry).application).icon_background).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })])), ("flow_document",mp::json_summary(&(_field_entry).flow_document)), ("dependencies",{ if (&(_field_entry).dependencies).len() > 32 { return None; } serde_json::Value::Array((&(_field_entry).dependencies).iter().map(|item| Some(mp::object_value(&[("kind",mp::text(&(item).kind)?), ("node_id",match (&(item).node_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_type",match (&(item).node_type).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("config_version",match (&(item).config_version).as_ref() { Some(item) => serde_json::json!(*(item)), None => serde_json::Value::Null }), ("provider_code",match (&(item).provider_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("model_id",match (&(item).model_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("plugin_id",match (&(item).plugin_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("plugin_version",match (&(item).plugin_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("contribution_code",match (&(item).contribution_code).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("node_shell",match (&(item).node_shell).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("schema_version",match (&(item).schema_version).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("package_id",match (&(item).package_id).as_ref() { Some(item) => mp::text(item)?, None => serde_json::Value::Null }), ("contribution_checksum",match (&(item).contribution_checksum).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("compiled_contribution_hash",match (&(item).compiled_contribution_hash).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]))).collect::<Option<Vec<_>>>()?) }), ("workflow_trigger_config",match (&(_field_entry).workflow_trigger_config).as_ref() { Some(item) => match item {control_plane::application::WorkflowTriggerTemplateConfig::Schedule {cron: _field_cron, timezone: _field_timezone, input_payload: _field_input_payload, .. } => mp::object_value(&[("variant",serde_json::Value::String("Schedule".to_owned())), ("cron",mp::object_value(&[("byte_count",serde_json::json!((_field_cron).len()))])), ("timezone",mp::object_value(&[("byte_count",serde_json::json!((_field_timezone).len()))])), ("input_payload",mp::json_summary(_field_input_payload))]), control_plane::application::WorkflowTriggerTemplateConfig::Extension { .. } => mp::object_value(&[("variant",serde_json::Value::String("Extension".to_owned()))])}, None => serde_json::Value::Null })])), ("name",match (_field_name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (_field_description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null })]), Self::ExportArchive(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("ExportArchive".to_owned())), ("0",mp::object_value(&[("application_ids",{ if (&(_field_0).application_ids).len() > 32 { return None; } serde_json::Value::Array((&(_field_0).application_ids).iter().map(|item| Some(serde_json::Value::String((item).to_string()))).collect::<Option<Vec<_>>>()?) })]))]), Self::PreviewInstalledArchive(_field_0) => mp::object_value(&[("variant",serde_json::Value::String("PreviewInstalledArchive".to_owned())), ("0",serde_json::Value::String((_field_0).to_string()))]), Self::ImportInstalledArchive {installation_id: _field_installation_id, body: _field_body, .. } => mp::object_value(&[("variant",serde_json::Value::String("ImportInstalledArchive".to_owned())), ("installation_id",serde_json::Value::String((_field_installation_id).to_string())), ("body",mp::object_value(&[("name",match (&(_field_body).name).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("description",match (&(_field_body).description).as_ref() { Some(item) => mp::object_value(&[("byte_count",serde_json::json!((item).len()))]), None => serde_json::Value::Null }), ("integrity_override",match (&(_field_body).integrity_override).as_ref() { Some(item) => mp::object_value(&[("reason",mp::object_value(&[("byte_count",serde_json::json!((&(item).reason).len()))])), ("acknowledged_warnings",mp::object_value(&[("item_count",serde_json::json!((&(item).acknowledged_warnings).len()))]))]), None => serde_json::Value::Null })]))])})
    }

    const CONTRACT_ID: &'static str = "console-application-orchestration-input";
    const CONTRACT_VERSION: &'static str = "1";
}

impl InterfaceContract for ApplicationOrchestrationOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("State")),
                (
                    "0",
                    mp::object_schema(&[
                        ("flow_id", mp::text_schema()),
                        (
                            "draft",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                ("flow_id", mp::text_schema()),
                                ("document", mp::json_summary_schema()),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                        (
                            "messages",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("key",mp::object_schema(&[("byte_count",mp::count_schema())])), ("text",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        (
                            "versions",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("id",mp::text_schema()), ("sequence",serde_json::json!({"type":"integer"})), ("trigger",mp::object_schema(&[("byte_count",mp::count_schema())])), ("change_kind",mp::object_schema(&[("byte_count",mp::count_schema())])), ("summary",mp::object_schema(&[("byte_count",mp::count_schema())])), ("summary_is_custom",serde_json::json!({"type":"boolean"})), ("is_user_protected",serde_json::json!({"type":"boolean"})), ("is_current_publication",serde_json::json!({"type":"boolean"})), ("created_at",mp::text_schema())])}),
                        ),
                        (
                            "autosave_interval_seconds",
                            serde_json::json!({"type":"integer"}),
                        ),
                        (
                            "user_protection_limit",
                            serde_json::json!({"type":"integer"}),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ArchivePreview")),
                (
                    "0",
                    mp::object_schema(&[
                        ("schema_version", mp::text_schema()),
                        (
                            "application",
                            mp::object_schema(&[
                                ("application_type", mp::text_schema()),
                                (
                                    "name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "description",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "icon",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "icon_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "icon_background",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                            ]),
                        ),
                        (
                            "dependencies",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("dependency",mp::object_schema(&[("kind",mp::text_schema()), ("node_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_type",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("config_version",serde_json::json!({"anyOf": [serde_json::json!({"type":"integer"}), {"type":"null"}]})), ("provider_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("model_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("plugin_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("plugin_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("contribution_code",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("node_shell",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("schema_version",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("package_id",serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]})), ("contribution_checksum",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]})), ("compiled_contribution_hash",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])), ("status",mp::text_schema()), ("reason",serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}))])}),
                        ),
                        (
                            "unresolved_nodes",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("node_id",mp::text_schema()), ("alias",mp::object_schema(&[("byte_count",mp::count_schema())])), ("dependency_status",mp::object_schema(&[("byte_count",mp::count_schema())])), ("reason",mp::object_schema(&[("byte_count",mp::count_schema())]))])}),
                        ),
                        ("document", mp::json_summary_schema()),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ArchiveImport")),
                (
                    "0",
                    mp::object_schema(&[
                        (
                            "application",
                            mp::object_schema(&[
                                ("id", mp::text_schema()),
                                ("application_type", mp::text_schema()),
                                (
                                    "name",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "description",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                (
                                    "icon",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "icon_type",
                                    serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                ),
                                (
                                    "icon_background",
                                    serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                ),
                                (
                                    "created_by",
                                    mp::object_schema(&[("byte_count", mp::count_schema())]),
                                ),
                                ("updated_at", mp::text_schema()),
                            ]),
                        ),
                        (
                            "orchestration",
                            mp::object_schema(&[
                                ("flow_id", mp::text_schema()),
                                (
                                    "draft",
                                    mp::object_schema(&[
                                        ("id", mp::text_schema()),
                                        ("flow_id", mp::text_schema()),
                                        ("document", mp::json_summary_schema()),
                                        ("updated_at", mp::text_schema()),
                                    ]),
                                ),
                                (
                                    "messages",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "versions",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "autosave_interval_seconds",
                                    serde_json::json!({"type":"integer"}),
                                ),
                                (
                                    "user_protection_limit",
                                    serde_json::json!({"type":"integer"}),
                                ),
                            ]),
                        ),
                        (
                            "preview",
                            mp::object_schema(&[
                                ("schema_version", mp::text_schema()),
                                (
                                    "application",
                                    mp::object_schema(&[
                                        ("application_type", mp::text_schema()),
                                        (
                                            "name",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "description",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "icon",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "icon_type",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "icon_background",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "dependencies",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "unresolved_nodes",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                ("document", mp::json_summary_schema()),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("InstalledArchivePreview")),
                (
                    "0",
                    mp::object_schema(&[
                        ("extension_installation_id", mp::text_schema()),
                        (
                            "application_status",
                            mp::object_schema(&[("byte_count", mp::count_schema())]),
                        ),
                        (
                            "integrity_warnings",
                            serde_json::json!({"type":"array","maxItems":32,"items":mp::object_schema(&[("code",mp::text_schema()), ("message",mp::object_schema(&[("byte_count",mp::count_schema())])), ("overridable",serde_json::json!({"type":"boolean"}))])}),
                        ),
                        (
                            "required_integrity_override",
                            serde_json::json!({"anyOf": [mp::object_schema(&[("warnings",mp::object_schema(&[("item_count",mp::count_schema())])), ("compatibility",serde_json::json!({"anyOf": [mp::object_schema(&[("reason",mp::object_schema(&[("byte_count",mp::count_schema())])), ("current_host_version",mp::text_schema()), ("minimum_host_version",mp::text_schema())]), {"type":"null"}]}))]), {"type":"null"}]}),
                        ),
                        (
                            "preview",
                            mp::object_schema(&[
                                ("schema_version", mp::text_schema()),
                                (
                                    "application",
                                    mp::object_schema(&[
                                        ("application_type", mp::text_schema()),
                                        (
                                            "name",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "description",
                                            mp::object_schema(&[(
                                                "byte_count",
                                                mp::count_schema(),
                                            )]),
                                        ),
                                        (
                                            "icon",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                        (
                                            "icon_type",
                                            serde_json::json!({"anyOf": [mp::text_schema(), {"type":"null"}]}),
                                        ),
                                        (
                                            "icon_background",
                                            serde_json::json!({"anyOf": [mp::object_schema(&[("byte_count",mp::count_schema())]), {"type":"null"}]}),
                                        ),
                                    ]),
                                ),
                                (
                                    "dependencies",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                (
                                    "unresolved_nodes",
                                    mp::object_schema(&[("item_count", mp::count_schema())]),
                                ),
                                ("document", mp::json_summary_schema()),
                            ]),
                        ),
                    ]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("ExportedArchive")),
                (
                    "0",
                    mp::object_schema(&[
                        ("content_type", mp::text_schema()),
                        (
                            "document",
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
            Self::State(_field_0) => mp::object_value(&[
                ("variant", serde_json::Value::String("State".to_owned())),
                (
                    "0",
                    mp::object_value(&[
                        ("flow_id", mp::text(&(_field_0).flow_id)?),
                        (
                            "draft",
                            mp::object_value(&[
                                ("id", mp::text(&(&(_field_0).draft).id)?),
                                ("flow_id", mp::text(&(&(_field_0).draft).flow_id)?),
                                ("document", mp::json_summary(&(&(_field_0).draft).document)),
                                ("updated_at", mp::text(&(&(_field_0).draft).updated_at)?),
                            ]),
                        ),
                        ("messages", {
                            if (&(_field_0).messages).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).messages)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "key",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).key).len()),
                                                )]),
                                            ),
                                            (
                                                "text",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).text).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("versions", {
                            if (&(_field_0).versions).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).versions)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("id", mp::text(&(item).id)?),
                                            ("sequence", serde_json::json!(*(&(item).sequence))),
                                            (
                                                "trigger",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).trigger).len()),
                                                )]),
                                            ),
                                            (
                                                "change_kind",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).change_kind).len()),
                                                )]),
                                            ),
                                            (
                                                "summary",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).summary).len()),
                                                )]),
                                            ),
                                            (
                                                "summary_is_custom",
                                                serde_json::Value::Bool(
                                                    *(&(item).summary_is_custom),
                                                ),
                                            ),
                                            (
                                                "is_user_protected",
                                                serde_json::Value::Bool(
                                                    *(&(item).is_user_protected),
                                                ),
                                            ),
                                            (
                                                "is_current_publication",
                                                serde_json::Value::Bool(
                                                    *(&(item).is_current_publication),
                                                ),
                                            ),
                                            ("created_at", mp::text(&(item).created_at)?),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "autosave_interval_seconds",
                            serde_json::json!(*(&(_field_0).autosave_interval_seconds)),
                        ),
                        (
                            "user_protection_limit",
                            serde_json::json!(*(&(_field_0).user_protection_limit)),
                        ),
                    ]),
                ),
            ]),
            Self::ArchivePreview(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ArchivePreview".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("schema_version", mp::text(&(_field_0).schema_version)?),
                        (
                            "application",
                            mp::object_value(&[
                                (
                                    "application_type",
                                    mp::text(&(&(_field_0).application).application_type)?,
                                ),
                                (
                                    "name",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).application).name).len()),
                                    )]),
                                ),
                                (
                                    "description",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).application).description).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "icon",
                                    match (&(&(_field_0).application).icon).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "icon_type",
                                    match (&(&(_field_0).application).icon_type).as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "icon_background",
                                    match (&(&(_field_0).application).icon_background).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                            ]),
                        ),
                        ("dependencies", {
                            if (&(_field_0).dependencies).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).dependencies)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            (
                                                "dependency",
                                                mp::object_value(&[
                                                    ("kind", mp::text(&(&(item).dependency).kind)?),
                                                    (
                                                        "node_id",
                                                        match (&(&(item).dependency).node_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "node_type",
                                                        match (&(&(item).dependency).node_type)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "config_version",
                                                        match (&(&(item).dependency).config_version)
                                                            .as_ref()
                                                        {
                                                            Some(item) => {
                                                                serde_json::json!(*(item))
                                                            }
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "provider_code",
                                                        match (&(&(item).dependency).provider_code)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "model_id",
                                                        match (&(&(item).dependency).model_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "plugin_id",
                                                        match (&(&(item).dependency).plugin_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "plugin_version",
                                                        match (&(&(item).dependency).plugin_version)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "contribution_code",
                                                        match (&(&(item).dependency)
                                                            .contribution_code)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "node_shell",
                                                        match (&(&(item).dependency).node_shell)
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
                                                        "schema_version",
                                                        match (&(&(item).dependency).schema_version)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "package_id",
                                                        match (&(&(item).dependency).package_id)
                                                            .as_ref()
                                                        {
                                                            Some(item) => mp::text(item)?,
                                                            None => serde_json::Value::Null,
                                                        },
                                                    ),
                                                    (
                                                        "contribution_checksum",
                                                        match (&(&(item).dependency)
                                                            .contribution_checksum)
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
                                                        "compiled_contribution_hash",
                                                        match (&(&(item).dependency)
                                                            .compiled_contribution_hash)
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
                                            ("status", mp::text(&(item).status)?),
                                            (
                                                "reason",
                                                match (&(item).reason).as_ref() {
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
                        ("unresolved_nodes", {
                            if (&(_field_0).unresolved_nodes).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).unresolved_nodes)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("node_id", mp::text(&(item).node_id)?),
                                            (
                                                "alias",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).alias).len()),
                                                )]),
                                            ),
                                            (
                                                "dependency_status",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!(
                                                        (&(item).dependency_status).len()
                                                    ),
                                                )]),
                                            ),
                                            (
                                                "reason",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).reason).len()),
                                                )]),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        ("document", mp::json_summary(&(_field_0).document)),
                    ]),
                ),
            ]),
            Self::ArchiveImport(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ArchiveImport".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "application",
                            mp::object_value(&[
                                ("id", mp::text(&(&(_field_0).application).id)?),
                                (
                                    "application_type",
                                    mp::text(&(&(_field_0).application).application_type)?,
                                ),
                                (
                                    "name",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!((&(&(_field_0).application).name).len()),
                                    )]),
                                ),
                                (
                                    "description",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).application).description).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "icon",
                                    match (&(&(_field_0).application).icon).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "icon_type",
                                    match (&(&(_field_0).application).icon_type).as_ref() {
                                        Some(item) => mp::text(item)?,
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "icon_background",
                                    match (&(&(_field_0).application).icon_background).as_ref() {
                                        Some(item) => mp::object_value(&[(
                                            "byte_count",
                                            serde_json::json!((item).len()),
                                        )]),
                                        None => serde_json::Value::Null,
                                    },
                                ),
                                (
                                    "created_by",
                                    mp::object_value(&[(
                                        "byte_count",
                                        serde_json::json!(
                                            (&(&(_field_0).application).created_by).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "updated_at",
                                    mp::text(&(&(_field_0).application).updated_at)?,
                                ),
                            ]),
                        ),
                        (
                            "orchestration",
                            mp::object_value(&[
                                ("flow_id", mp::text(&(&(_field_0).orchestration).flow_id)?),
                                (
                                    "draft",
                                    mp::object_value(&[
                                        ("id", mp::text(&(&(&(_field_0).orchestration).draft).id)?),
                                        (
                                            "flow_id",
                                            mp::text(
                                                &(&(&(_field_0).orchestration).draft).flow_id,
                                            )?,
                                        ),
                                        (
                                            "document",
                                            mp::json_summary(
                                                &(&(&(_field_0).orchestration).draft).document,
                                            ),
                                        ),
                                        (
                                            "updated_at",
                                            mp::text(
                                                &(&(&(_field_0).orchestration).draft).updated_at,
                                            )?,
                                        ),
                                    ]),
                                ),
                                (
                                    "messages",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).orchestration).messages).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "versions",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).orchestration).versions).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "autosave_interval_seconds",
                                    serde_json::json!(
                                        *(&(&(_field_0).orchestration).autosave_interval_seconds)
                                    ),
                                ),
                                (
                                    "user_protection_limit",
                                    serde_json::json!(
                                        *(&(&(_field_0).orchestration).user_protection_limit)
                                    ),
                                ),
                            ]),
                        ),
                        (
                            "preview",
                            mp::object_value(&[
                                (
                                    "schema_version",
                                    mp::text(&(&(_field_0).preview).schema_version)?,
                                ),
                                (
                                    "application",
                                    mp::object_value(&[
                                        (
                                            "application_type",
                                            mp::text(
                                                &(&(&(_field_0).preview).application)
                                                    .application_type,
                                            )?,
                                        ),
                                        (
                                            "name",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!(
                                                    (&(&(&(_field_0).preview).application).name)
                                                        .len()
                                                ),
                                            )]),
                                        ),
                                        (
                                            "description",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!(
                                                    (&(&(&(_field_0).preview).application)
                                                        .description)
                                                        .len()
                                                ),
                                            )]),
                                        ),
                                        (
                                            "icon",
                                            match (&(&(&(_field_0).preview).application).icon)
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
                                            "icon_type",
                                            match (&(&(&(_field_0).preview).application).icon_type)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "icon_background",
                                            match (&(&(&(_field_0).preview).application)
                                                .icon_background)
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
                                    "dependencies",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).preview).dependencies).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "unresolved_nodes",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).preview).unresolved_nodes).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "document",
                                    mp::json_summary(&(&(_field_0).preview).document),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            Self::InstalledArchivePreview(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("InstalledArchivePreview".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        (
                            "extension_installation_id",
                            mp::text(&(_field_0).extension_installation_id)?,
                        ),
                        (
                            "application_status",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).application_status).len()),
                            )]),
                        ),
                        ("integrity_warnings", {
                            if (&(_field_0).integrity_warnings).len() > 32 {
                                return None;
                            }
                            serde_json::Value::Array(
                                (&(_field_0).integrity_warnings)
                                    .iter()
                                    .map(|item| {
                                        Some(mp::object_value(&[
                                            ("code", mp::text(&(item).code)?),
                                            (
                                                "message",
                                                mp::object_value(&[(
                                                    "byte_count",
                                                    serde_json::json!((&(item).message).len()),
                                                )]),
                                            ),
                                            (
                                                "overridable",
                                                serde_json::Value::Bool(*(&(item).overridable)),
                                            ),
                                        ]))
                                    })
                                    .collect::<Option<Vec<_>>>()?,
                            )
                        }),
                        (
                            "required_integrity_override",
                            match (&(_field_0).required_integrity_override).as_ref() {
                                Some(item) => mp::object_value(&[
                                    (
                                        "warnings",
                                        mp::object_value(&[(
                                            "item_count",
                                            serde_json::json!((&(item).warnings).len()),
                                        )]),
                                    ),
                                    (
                                        "compatibility",
                                        match (&(item).compatibility).as_ref() {
                                            Some(item) => mp::object_value(&[
                                                (
                                                    "reason",
                                                    mp::object_value(&[(
                                                        "byte_count",
                                                        serde_json::json!((&(item).reason).len()),
                                                    )]),
                                                ),
                                                (
                                                    "current_host_version",
                                                    mp::text(&(item).current_host_version)?,
                                                ),
                                                (
                                                    "minimum_host_version",
                                                    mp::text(&(item).minimum_host_version)?,
                                                ),
                                            ]),
                                            None => serde_json::Value::Null,
                                        },
                                    ),
                                ]),
                                None => serde_json::Value::Null,
                            },
                        ),
                        (
                            "preview",
                            mp::object_value(&[
                                (
                                    "schema_version",
                                    mp::text(&(&(_field_0).preview).schema_version)?,
                                ),
                                (
                                    "application",
                                    mp::object_value(&[
                                        (
                                            "application_type",
                                            mp::text(
                                                &(&(&(_field_0).preview).application)
                                                    .application_type,
                                            )?,
                                        ),
                                        (
                                            "name",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!(
                                                    (&(&(&(_field_0).preview).application).name)
                                                        .len()
                                                ),
                                            )]),
                                        ),
                                        (
                                            "description",
                                            mp::object_value(&[(
                                                "byte_count",
                                                serde_json::json!(
                                                    (&(&(&(_field_0).preview).application)
                                                        .description)
                                                        .len()
                                                ),
                                            )]),
                                        ),
                                        (
                                            "icon",
                                            match (&(&(&(_field_0).preview).application).icon)
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
                                            "icon_type",
                                            match (&(&(&(_field_0).preview).application).icon_type)
                                                .as_ref()
                                            {
                                                Some(item) => mp::text(item)?,
                                                None => serde_json::Value::Null,
                                            },
                                        ),
                                        (
                                            "icon_background",
                                            match (&(&(&(_field_0).preview).application)
                                                .icon_background)
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
                                    "dependencies",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).preview).dependencies).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "unresolved_nodes",
                                    mp::object_value(&[(
                                        "item_count",
                                        serde_json::json!(
                                            (&(&(_field_0).preview).unresolved_nodes).len()
                                        ),
                                    )]),
                                ),
                                (
                                    "document",
                                    mp::json_summary(&(&(_field_0).preview).document),
                                ),
                            ]),
                        ),
                    ]),
                ),
            ]),
            Self::ExportedArchive(_field_0) => mp::object_value(&[
                (
                    "variant",
                    serde_json::Value::String("ExportedArchive".to_owned()),
                ),
                (
                    "0",
                    mp::object_value(&[
                        ("content_type", mp::text(&(_field_0).content_type)?),
                        (
                            "document",
                            mp::object_value(&[(
                                "byte_count",
                                serde_json::json!((&(_field_0).document).len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-orchestration-output";
    const CONTRACT_VERSION: &'static str = "1";
}
