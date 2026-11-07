mod schema;

use super::*;
use crate::routes::application_runtime::managed_detail_projection;

impl InterfaceContract for ApplicationRuntimeDebugArtifactsOutput {
    fn managed_projection_schema() -> Option<serde_json::Value> {
        schema::schema()
    }
    fn project_for_managed_hook(&self) -> Option<serde_json::Value> {
        use crate::extension_bus::managed_projection as mp;
        Some(match self {
            Self::Content(_field_0) => mp::object_value([
                ("variant", serde_json::Value::String("Content".to_owned())),
                (
                    "0",
                    mp::object_value([
                        ("content_type", mp::text(&(_field_0).content_type)?),
                        (
                            "bytes",
                            mp::object_value([(
                                "byte_count",
                                serde_json::json!((_field_0).bytes.len()),
                            )]),
                        ),
                    ]),
                ),
            ]),
            Self::Resolved(_field_0) => mp::object_value([
                ("variant", serde_json::Value::String("Resolved".to_owned())),
                (
                    "0",
                    mp::object_value([("artifacts", {
                        if (_field_0).artifacts.len() > 32 {
                            return None;
                        }
                        serde_json::Value::Array(
                            (_field_0)
                                .artifacts
                                .iter()
                                .map(|item| {
                                    Some(mp::object_value([
                                        (
                                            "artifact_ref",
                                            mp::object_value([(
                                                "byte_count",
                                                serde_json::json!((item).artifact_ref.len()),
                                            )]),
                                        ),
                                        ("content_type", mp::text(&(item).content_type)?),
                                        ("value", mp::json_summary(&(item).value)),
                                    ]))
                                })
                                .collect::<Option<Vec<_>>>()?,
                        )
                    })]),
                ),
            ]),
            Self::Snapshot(detail) => mp::object_value([
                ("variant", serde_json::Value::String("Snapshot".to_owned())),
                ("0", managed_detail_projection::project(detail)?),
            ]),
        })
    }

    const CONTRACT_ID: &'static str = "console-application-runtime-debug-artifacts-output";
    const CONTRACT_VERSION: &'static str = "1";
}

impl crate::event_center::api_projection::ApiEventOutput
    for ApplicationRuntimeDebugArtifactsOutput
{
    fn document_output_schema(
        operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Option<serde_json::Value> {
        match operation.interface_id.as_str() {
            "applications.runtime.debug-snapshot.get" => Some(
                crate::routes::applications_group::event_projection::runtime::schema::<
                    ApplicationRunDetailResponse,
                >(),
            ),
            "applications.runtime.debug-artifacts.resolve" => Some(
                crate::routes::applications_group::event_projection::runtime::schema::<
                    ResolveRuntimeDebugArtifactsResponse,
                >(),
            ),
            _ => None,
        }
    }
    // Content is arbitrary bytes; Resolve and Snapshot retain their actual JSON responses.
    fn document_output_unavailable(
        operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Option<domain::event_center::ApiEventUnavailableReason> {
        (operation.interface_id == "applications.runtime.debug-artifact.get")
            .then_some(domain::event_center::ApiEventUnavailableReason::BinaryResponse)
    }
    fn project_document_output(
        &self,
        _operation: &crate::event_center::api_projection::ApiEventOperationContext,
    ) -> Result<serde_json::Value, std::sync::Arc<str>> {
        match self {
            Self::Content(_) => Err(std::sync::Arc::from(
                "artifact_content_is_binary_without_document_json_output",
            )),
            Self::Resolved(value) => {
                crate::routes::applications_group::event_projection::runtime::document(value)
            }
            Self::Snapshot(value) => {
                crate::routes::applications_group::event_projection::runtime::document(value)
            }
        }
    }
}
