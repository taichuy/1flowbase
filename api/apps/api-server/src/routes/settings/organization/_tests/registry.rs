use super::*;
struct Unavailable;
impl ConsoleInterfacePort<OrganizationInput, OrganizationOutput> for Unavailable {
    fn execute<'a>(
        &'a self,
        _principal: &'a UserPrincipal,
        _input: OrganizationInput,
    ) -> ConsoleInterfaceFuture<'a, OrganizationOutput> {
        Box::pin(async {
            Err(ConsoleInterfaceTargetError(
                anyhow::anyhow!("fixture unavailable").into(),
            ))
        })
    }
}
#[test]
fn organization_registry_freezes_all_route_contracts_and_projections() {
    let registry = compile_registry(Arc::new(Unavailable)).unwrap();
    assert_eq!(registry.bindings().count(), DECLARATIONS.len());
    for declaration in DECLARATIONS {
        let binding = registry
            .binding(&interface_runtime::BindingId::new(declaration.binding_id).unwrap())
            .unwrap();
        let route = binding.projection().http_route().unwrap();
        assert_eq!(route.method(), declaration.method);
        assert_eq!(route.path(), declaration.path);
    }
    assert!(OrganizationInput::managed_projection_schema().is_some());
    assert!(OrganizationOutput::managed_projection_schema().is_some());
    assert_eq!(
        OrganizationInput::List.project_for_managed_hook().unwrap(),
        serde_json::json!({"variant":"List"})
    );
}
