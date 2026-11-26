use super::public_error_code;
use control_plane_contracts::ControlPlaneContractError;

#[test]
fn plugin_dependency_uses_static_observation_code_without_dynamic_owner_details() {
    for code in ["plugin_dependency_missing", "plugin_dependency_in_use"] {
        let error = ControlPlaneContractError::PluginDependency {
            code: code.to_owned(),
            provider_publisher_namespace: "private.publisher".into(),
            provider_plugin_code: "private-provider".into(),
            resource_code: Some("private-resource".into()),
            dependents: vec!["private-consumer".into()],
        };
        assert_eq!(
            public_error_code(&anyhow::Error::new(error), "fallback"),
            "plugin_dependency"
        );
    }
}

#[test]
fn plugin_error_classification_preserves_existing_codes_and_unknown_fallback() {
    assert_eq!(
        public_error_code(
            &anyhow::Error::new(ControlPlaneContractError::InvalidInput("invalid_fixture")),
            "fallback",
        ),
        "invalid_fixture"
    );
    assert_eq!(
        public_error_code(&anyhow::anyhow!("private transport detail"), "fallback"),
        "fallback"
    );
}
