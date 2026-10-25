use crate::extension_bus::*;
use crate::{NOTIFICATION_CONTRACT_V1, NOTIFICATION_POINT, NOTIFICATION_SEND_PERMISSION};

#[test]
fn managed_notification_point_compiles_workspace_provider_contribution_with_authority() {
    let point = notification_extension_point(ModuleId::new("core").unwrap()).unwrap();
    assert_eq!(point.scope, ScopeSemantics::Workspace);
    assert_eq!(point.lifecycle, LifecycleSemantics::Invocation);
    assert_eq!(point.override_policy, OverridePolicy::Sealed);
    let mut core = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("core").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: Default::default(),
        granted_permissions: Default::default(),
        extension_points: vec![point],
        contributions: vec![],
    };
    let mut provider = ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: ModuleId::new("fixture").unwrap(),
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::Runtime,
        activation: ModuleActivationDeclaration::Active,
        dependencies: Default::default(),
        granted_permissions: [PermissionCode::new(NOTIFICATION_SEND_PERMISSION).unwrap()]
            .into_iter()
            .collect(),
        extension_points: vec![],
        contributions: vec![ContributionDescriptor {
            contribution_id: ContributionId::new("fixture.send").unwrap(),
            contributor_module_id: ModuleId::new("fixture").unwrap(),
            point_id: ExtensionPointId::new(NOTIFICATION_POINT).unwrap(),
            contract_version: ContractVersion::new(NOTIFICATION_CONTRACT_V1).unwrap(),
            required_permissions: [PermissionCode::new(NOTIFICATION_SEND_PERMISSION).unwrap()]
                .into_iter()
                .collect(),
            mode: ContributionMode::Append,
            ordering: Default::default(),
        }],
    };
    let graph = compile_extension_graph(vec![core.clone(), provider.clone()]).unwrap();
    assert_eq!(graph.points()[0].contributions().len(), 1);
    provider.contributions[0].contract_version = ContractVersion::new("invalid").unwrap();
    assert!(compile_extension_graph(vec![core.clone(), provider]).is_err());
    core.extension_points.clear();
    assert!(compile_extension_graph(vec![core])
        .unwrap()
        .points()
        .is_empty());
}

#[test]
fn receive_point_has_an_independent_sealed_workspace_permission() {
    let send = notification_extension_point(ModuleId::new("core").unwrap()).unwrap();
    let receive = notification_receive_extension_point(ModuleId::new("core").unwrap()).unwrap();
    assert_eq!(
        receive.point_id.as_str(),
        extension_contracts::NOTIFICATION_RECEIVE_POINT
    );
    assert_eq!(receive.contract, send.contract);
    assert_eq!(receive.scope, ScopeSemantics::Workspace);
    assert_eq!(receive.override_policy, OverridePolicy::Sealed);
    assert!(receive.allowed_permissions.contains(
        &PermissionCode::new(extension_contracts::NOTIFICATION_RECEIVE_PERMISSION).unwrap()
    ));
    assert!(!receive
        .allowed_permissions
        .contains(&PermissionCode::new(NOTIFICATION_SEND_PERMISSION).unwrap()));
}
