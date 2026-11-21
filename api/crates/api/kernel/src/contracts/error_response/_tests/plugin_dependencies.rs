use super::{ApiError, ControlPlaneError};
use axum::{body::to_bytes, http::StatusCode, response::IntoResponse};

#[tokio::test]
async fn plugin_dependency_failures_expose_provider_resource_and_dependents_without_internal_500() {
    for (code, resource_code, dependents) in [
        ("plugin_dependency_not_installed", None, vec![]),
        (
            "plugin_dependency_not_enabled",
            Some("hosts-v1".to_owned()),
            vec![],
        ),
        (
            "plugin_dependency_resource_missing",
            Some("hosts-v1".to_owned()),
            vec![],
        ),
        (
            "plugin_dependency_in_use",
            None,
            vec!["monitor/compute-monitor".to_owned()],
        ),
    ] {
        let response = ApiError(
            ControlPlaneError::PluginDependency {
                code: code.into(),
                provider_publisher_namespace: "provider".into(),
                provider_plugin_code: "ssh".into(),
                resource_code: resource_code.clone(),
                dependents: dependents.clone(),
            }
            .into(),
        )
        .into_response();
        assert_eq!(response.status(), StatusCode::CONFLICT);
        let bytes = to_bytes(response.into_body(), 16384).await.unwrap();
        let payload: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        assert_eq!(payload["code"], code);
        assert_eq!(
            payload["details"]["provider"],
            serde_json::json!({"publisher_namespace":"provider","plugin_code":"ssh"})
        );
        assert_eq!(
            payload["details"]["resource_code"],
            serde_json::json!(resource_code)
        );
        assert_eq!(
            payload["details"]["dependents"],
            serde_json::json!(dependents)
        );
        assert!(
            payload["message"]
                .as_str()
                .unwrap()
                .contains("provider/ssh")
        );
    }
}
