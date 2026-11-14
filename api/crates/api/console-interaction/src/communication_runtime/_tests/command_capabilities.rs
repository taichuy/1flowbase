use super::*;

#[test]
fn communication_command_model_availability_uses_published_capabilities() {
    assert_eq!(
        project(Ok(AssistantRunCapabilities::default()))
            .unwrap()
            .1
            .as_deref(),
        Some("model_selection_unavailable")
    );
    assert_eq!(
        project(Ok(AssistantRunCapabilities {
            model_selection_enabled: true,
            ..Default::default()
        }))
        .unwrap()
        .1
        .as_deref(),
        Some("no_models_available")
    );
    let capabilities = AssistantRunCapabilities {
        model_selection_enabled: true,
        models: vec![crate::routes::assistant::AssistantModelOption {
            id: "provider/opaque-model-id".into(),
            name: None,
            context_window: None,
            reasoning_efforts: vec![],
            default_reasoning_effort: None,
        }],
        ..Default::default()
    };
    let (actual, reason) = project(Ok(capabilities)).unwrap();
    assert_eq!(actual.models[0].id, "provider/opaque-model-id");
    assert!(reason.is_none());
}

#[test]
fn communication_command_saved_channel_is_readable_but_infrastructure_errors_propagate() {
    for error in [
        ControlPlaneError::NotFound("publication"),
        ControlPlaneError::PermissionDenied("application"),
    ] {
        assert_eq!(
            project(Err(error.into())).unwrap().1.as_deref(),
            Some("application_unavailable")
        );
    }
    assert!(project(Err(anyhow::anyhow!("database unavailable").into())).is_err());
}
