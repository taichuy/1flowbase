use control_plane::resource_action::{
    ActionDefinition, ResourceActionKernel, ResourceActionRegistry, ResourceDefinition,
    ResourceScopeKind,
};

fn test_kernel_with_plugins_install_handler(
    handler: impl Fn(serde_json::Value) -> serde_json::Value + Send + Sync + 'static,
) -> ResourceActionKernel {
    let mut registry = ResourceActionRegistry::default();
    registry
        .register_resource(ResourceDefinition::core(
            "plugins",
            ResourceScopeKind::System,
        ))
        .unwrap();
    registry
        .register_action(ActionDefinition::core("plugins", "install"))
        .unwrap();

    let mut kernel = ResourceActionKernel::new(registry);
    kernel
        .register_json_handler("plugins", "install", move |input| {
            let output = handler(input);
            async move { Ok(output) }
        })
        .unwrap();
    kernel
}

#[test]
fn registry_rejects_duplicate_action() {
    let mut registry = ResourceActionRegistry::default();
    registry
        .register_resource(ResourceDefinition::core(
            "plugins",
            ResourceScopeKind::System,
        ))
        .unwrap();
    registry
        .register_action(ActionDefinition::core("plugins", "install"))
        .unwrap();

    let err = registry
        .register_action(ActionDefinition::core("plugins", "install"))
        .unwrap_err();
    assert!(err.to_string().contains("duplicate action"));
}

#[test]
fn registry_requires_existing_resource() {
    let mut registry = ResourceActionRegistry::default();
    let err = registry
        .register_action(ActionDefinition::core("files", "upload"))
        .unwrap_err();
    assert!(err.to_string().contains("resource not registered"));
}

#[tokio::test]
async fn dispatch_calls_registered_core_handler() {
    let kernel = test_kernel_with_plugins_install_handler(|input| {
        assert_eq!(input["plugin_id"], "openai_compatible@0.3.18");
        serde_json::json!({"status": "installed"})
    });

    let output = kernel
        .dispatch_json(
            "plugins",
            "install",
            serde_json::json!({"plugin_id": "openai_compatible@0.3.18"}),
        )
        .await
        .unwrap();

    assert_eq!(output["status"], "installed");
}
