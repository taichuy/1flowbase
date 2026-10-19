use crate::extension_bus::*;

#[test]
fn authentication_point_is_system_sealed_and_rejects_package_defined_ceiling() {
    let point = authentication_extension_point(ModuleId::new("core").unwrap()).unwrap();
    assert_eq!(point.scope, ScopeSemantics::System);
    assert_eq!(point.failure, FailureSemantics::FailClosed);
    assert_eq!(point.override_policy, OverridePolicy::Sealed);
    assert_eq!(point.allowed_permissions.len(), 2);
    let core = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("core").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: Default::default(),
        granted_permissions: Default::default(),
        extension_points: vec![point.clone()],
        contributions: vec![],
    };
    let mut provider = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("fixture").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::Runtime,
        activation: ModuleActivationDeclaration::Active,
        dependencies: Default::default(),
        granted_permissions: [
            PermissionCode::new(crate::AUTHENTICATION_PERMISSION).unwrap(),
            PermissionCode::new("network.http.request").unwrap(),
        ]
        .into_iter()
        .collect(),
        extension_points: vec![],
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new("fixture.auth").unwrap(),
            contributor_module_id: ModuleId::new("fixture").unwrap(),
            point_id: point.point_id.clone(),
            contract_version: ContractVersion::new(crate::AUTHENTICATION_CONTRACT_V1).unwrap(),
            required_permissions: point.allowed_permissions.clone(),
            mode: ContributionMode::Append,
            ordering: Default::default(),
        }],
    };
    let graph = compile_extension_graph(vec![core, provider.clone()]).unwrap();
    assert_eq!(graph.points()[0].contributions().len(), 1);
    assert!(compile_extension_graph(vec![provider.clone()]).is_err());
    let mut forged = point;
    forged.owner_module_id = provider.module_id.clone();
    provider.extension_points.push(forged);
    assert!(compile_extension_graph(vec![provider]).is_err());
}
