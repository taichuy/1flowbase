use super::*;

#[test]
fn communication_command_host_descriptors_and_unknown_controls_never_become_prompts() {
    let catalog = effective_communication_commands(Vec::new(), &[]).unwrap();
    assert_eq!(catalog.len(), 3);
    assert_eq!(
        parse_communication_command("hello", &catalog),
        CommunicationCommandIntent::Ordinary
    );
    assert!(matches!(
        parse_communication_command("/missing hello", &catalog),
        CommunicationCommandIntent::Rejected { .. }
    ));
    assert_eq!(
        parse_communication_command("/model-user-user/model:opaque", &catalog),
        CommunicationCommandIntent::Builtin(BuiltinCommunicationIntent::Model {
            model: Some("user/model:opaque".into())
        })
    );
    assert!(matches!(
        parse_communication_command("/new extra", &catalog),
        CommunicationCommandIntent::Rejected { .. }
    ));
    let disabled =
        effective_communication_commands(Vec::new(), &[("host/new".into(), false)]).unwrap();
    assert!(
        matches!(parse_communication_command("/new", &disabled), CommunicationCommandIntent::Rejected { code, .. } if code == "command_disabled")
    );
}

fn plugin(id: &str, installation_id: Uuid) -> CommunicationCommandCatalogEntry {
    CommunicationCommandCatalogEntry {
        descriptor: CommunicationCommandDescriptor {
            command_id: id.into(),
            name: "status".into(),
            description: "Status".into(),
            usage: "/status".into(),
            contribution_id: ContributionId::new("acme.bot.commands").unwrap(),
            default_enabled: true,
        },
        source_name: "Bot".into(),
        installation_id: Some(installation_id),
        enabled: true,
        available: true,
        unavailable_reason: None,
        builtin: None,
    }
}

#[test]
fn communication_command_enabled_name_conflicts_fail_closed_and_overrides_resolve_them() {
    let plugins = vec![
        plugin("acme/bot/status", Uuid::now_v7()),
        plugin("acme/other/status", Uuid::now_v7()),
    ];
    let conflicting = effective_communication_commands(plugins.clone(), &[]).unwrap();
    assert_eq!(
        conflicting
            .iter()
            .filter(|c| c.descriptor.name == "status" && !c.available)
            .count(),
        2
    );
    assert!(matches!(
        parse_communication_command("/status", &conflicting),
        CommunicationCommandIntent::Rejected { .. }
    ));
    let resolved =
        effective_communication_commands(plugins, &[("acme/other/status".into(), false)]).unwrap();
    assert!(
        matches!(parse_communication_command("/status", &resolved), CommunicationCommandIntent::Plugin { command_id, .. } if command_id == "acme/bot/status")
    );
}

#[test]
fn communication_command_reserved_names_duplicate_settings_and_unknown_overrides_are_rejected() {
    let mut forged = plugin("acme/bot/new", Uuid::now_v7());
    forged.descriptor.name = "new".into();
    forged.descriptor.usage = "/new".into();
    assert!(effective_communication_commands(vec![forged], &[]).is_err());
    assert!(effective_communication_commands(
        Vec::new(),
        &[("host/new".into(), false), ("host/new".into(), true)]
    )
    .is_err());
    // Unknown saved settings remain a service concern; registry never turns them into a handler.
    assert!(matches!(
        parse_communication_command(
            "/unknown",
            &effective_communication_commands(Vec::new(), &[("old/plugin/unknown".into(), true)])
                .unwrap()
        ),
        CommunicationCommandIntent::Rejected { .. }
    ));
}

#[test]
fn communication_command_plan_selects_exact_subscriber_from_many() {
    use crate::extension_bus::*;
    let owner = ModuleId::new("1flowbase").unwrap();
    let point = communication_command_extension_point(owner.clone()).unwrap();
    let permission = PermissionCode::new("event.subscribe").unwrap();
    let contributions = ["first", "second"]
        .into_iter()
        .map(|name| ContributionDescriptor {
            contribution_id: ContributionId::new(format!("1flowbase.{name}")).unwrap(),
            contributor_module_id: owner.clone(),
            point_id: point.point_id.clone(),
            contract_version: ContractVersion::new("1").unwrap(),
            required_permissions: BTreeSet::from([permission.clone()]),
            mode: ContributionMode::Append,
            ordering: ContributionOrdering::default(),
        })
        .collect::<Vec<_>>();
    let bindings = contributions
        .iter()
        .map(|contribution| LifecycleSubscriberBinding {
            contribution_id: contribution.contribution_id.clone(),
            subscription_id: contribution.contribution_id.as_str().into(),
            point_id: point.point_id.clone(),
            fact_contract_id: extension_contracts::COMMUNICATION_COMMAND_POINT.into(),
            fact_contract_version: "1".into(),
            handler_id: contribution.contribution_id.as_str().into(),
            handler_version: "1".into(),
        })
        .collect::<Vec<_>>();
    let graph = compile_extension_graph(vec![ModuleDescriptor {
        bus_version: ExtensionBusVersion::V1,
        module_id: owner,
        module_version: ModuleVersion::new("1").unwrap(),
        module_kind: ModuleKind::BootCore,
        activation: ModuleActivationDeclaration::Active,
        dependencies: BTreeSet::new(),
        granted_permissions: BTreeSet::from([permission]),
        extension_points: vec![point],
        contributions,
    }])
    .unwrap();
    let plan = compile_lifecycle_subscriber_plan(&graph, bindings).unwrap();
    assert_eq!(plan.subscribers().len(), 2);
    assert_eq!(
        select_communication_command_subscriber(
            &plan,
            &ContributionId::new("1flowbase.second").unwrap()
        )
        .unwrap()
        .handler_id,
        "1flowbase.second"
    );
    assert!(select_communication_command_subscriber(
        &plan,
        &ContributionId::new("1flowbase.missing").unwrap()
    )
    .is_err());
}
