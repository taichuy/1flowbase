use super::*;

impl InterfaceContract for ApplicationRuntimeDebugArtifactsInput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(mp::union_schema(vec![
            mp::object_schema(&[
                ("variant", mp::tag_schema("Get")),
                ("application_id", mp::text_schema()),
                ("artifact_id", mp::text_schema()),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Resolve")),
                ("application_id", mp::text_schema()),
                (
                    "body",
                    mp::object_schema(&[(
                        "artifact_refs",
                        serde_json::json!({"type":"array","maxItems":32,"items":mp::text_schema()}),
                    )]),
                ),
            ]),
            mp::object_schema(&[
                ("variant", mp::tag_schema("Snapshot")),
                ("application_id", mp::text_schema()),
                ("run_id", mp::text_schema()),
            ]),
        ]))
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Get {
                application_id: _field_application_id,
                artifact_id: _field_artifact_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Get".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "artifact_id",
                    serde_json::Value::String((_field_artifact_id).to_string()),
                ),
            ]),
            Self::Resolve {
                application_id: _field_application_id,
                body: _field_body,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Resolve".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "body",
                    mp::object_value(&[("artifact_refs", {
                        if (&(_field_body).artifact_refs).len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (&(_field_body).artifact_refs)
                                .iter()
                                .map(|item| Some(serde_json::Value::String((item).to_string())))
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::Snapshot {
                application_id: _field_application_id,
                run_id: _field_run_id,
                ..
            } => mp::object_value(&[
                ("variant", serde_json::Value::String("Snapshot".to_owned())),
                (
                    "application_id",
                    serde_json::Value::String((_field_application_id).to_string()),
                ),
                (
                    "run_id",
                    serde_json::Value::String((_field_run_id).to_string()),
                ),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-artifacts-input";
    const CONTRACT_VERSION: &'static str = "1";
}
