use super::*;

#[tokio::test]
async fn update_scope_grant_records_audit_event() {
    let repository = InMemoryModelDefinitionRepository::default();
    let service = ModelDefinitionService::new(repository.clone());
    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "scope_grant_audit_orders".into(),
            title: "Scope Grant Audit Orders".into(),
            status: None,
        })
        .await
        .unwrap();
    let grant = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id: Uuid::nil(),
                scope_kind: DataModelScopeKind::System,
                scope_id: SYSTEM_SCOPE_ID,
                data_model_id: created.id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap();

    service
        .update_scope_grant(UpdateScopeDataModelGrantCommand {
            actor_user_id: Uuid::nil(),
            data_model_id: created.id,
            grant_id: grant.id,
            enabled: Some(false),
            permission_profile: Some("owner".into()),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();

    assert!(repository
        .audit_events()
        .contains(&"state_model.scope_grant_updated".to_string()));
}

#[tokio::test]
async fn runtime_scope_grant_loader_denies_when_role_action_is_disabled() {
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let policy = role_data_policy(
        false,
        false,
        false,
        false,
        domain::RoleDataPolicyScope::ScopeAll,
    );
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        workspace_id,
    ))
    .with_model(system_model(model_id))
    .with_grant(scope_grant(
        Uuid::now_v7(),
        model_id,
        DataModelScopeKind::Workspace,
        workspace_id,
    ))
    .with_role_data_policy(policy, None);
    let service = ModelDefinitionService::new(repository);

    let grant = service
        .load_runtime_scope_grant(
            &ActorContext::scoped(actor_user_id, workspace_id, "member", Vec::<String>::new()),
            model_id,
            runtime_core::runtime_acl::RuntimeDataAction::View,
        )
        .await
        .unwrap();

    assert!(grant.is_none());
}

#[tokio::test]
async fn runtime_scope_grant_loader_clamps_role_scope_and_honors_model_override() {
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let policy = role_data_policy(true, true, true, true, domain::RoleDataPolicyScope::Own);
    let model_policy = role_data_model_policy(
        policy.role_id,
        model_id,
        Some(domain::RoleDataPolicyScope::ScopeAll),
        None,
        Some(domain::RoleDataPolicyScope::SystemAll),
    );
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        workspace_id,
    ))
    .with_model(system_model(model_id))
    .with_grant(scope_grant(
        Uuid::now_v7(),
        model_id,
        DataModelScopeKind::Workspace,
        workspace_id,
    ))
    .with_role_data_policy(policy.clone(), Some(model_policy));
    let service = ModelDefinitionService::new(repository);
    let actor = ActorContext::scoped(actor_user_id, workspace_id, "member", Vec::<String>::new());

    let view_grant = service
        .load_runtime_scope_grant(
            &actor,
            model_id,
            runtime_core::runtime_acl::RuntimeDataAction::View,
        )
        .await
        .unwrap()
        .expect("view override should allow scope_all");
    assert_eq!(
        view_grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::ScopeAll
    );

    let update_grant = service
        .load_runtime_scope_grant(
            &actor,
            model_id,
            runtime_core::runtime_acl::RuntimeDataAction::Update,
        )
        .await
        .unwrap()
        .expect("null update override should inherit own");
    assert_eq!(
        update_grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::Owner
    );

    let create_grant = service
        .load_runtime_scope_grant(
            &actor,
            model_id,
            runtime_core::runtime_acl::RuntimeDataAction::Create,
        )
        .await
        .unwrap()
        .expect("create should only require can_create");
    assert_eq!(
        create_grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::ScopeAll
    );
}

#[tokio::test]
async fn runtime_scope_grant_loader_honors_model_create_override() {
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let policy = role_data_policy(true, true, true, true, domain::RoleDataPolicyScope::Own);
    let mut model_policy = role_data_model_policy(policy.role_id, model_id, None, None, None);
    model_policy.can_create_override = Some(false);
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        workspace_id,
    ))
    .with_model(system_model(model_id))
    .with_grant(scope_grant(
        Uuid::now_v7(),
        model_id,
        DataModelScopeKind::Workspace,
        workspace_id,
    ))
    .with_role_data_policy(policy, Some(model_policy));
    let service = ModelDefinitionService::new(repository);
    let actor = ActorContext::scoped(actor_user_id, workspace_id, "member", Vec::<String>::new());

    let create_grant = service
        .load_runtime_scope_grant(
            &actor,
            model_id,
            runtime_core::runtime_acl::RuntimeDataAction::Create,
        )
        .await
        .unwrap();

    assert!(create_grant.is_none());
}

#[tokio::test]
async fn runtime_scope_grant_loader_keeps_grant_owner_as_lower_boundary() {
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let mut grant = scope_grant(
        Uuid::now_v7(),
        model_id,
        DataModelScopeKind::Workspace,
        workspace_id,
    );
    grant.permission_profile = domain::ScopeDataModelPermissionProfile::Owner;
    let policy = role_data_policy(
        true,
        true,
        true,
        true,
        domain::RoleDataPolicyScope::ScopeAll,
    );
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        workspace_id,
    ))
    .with_model(system_model(model_id))
    .with_grant(grant)
    .with_role_data_policy(policy, None);
    let service = ModelDefinitionService::new(repository);

    let grant = service
        .load_runtime_scope_grant(
            &ActorContext::scoped(actor_user_id, workspace_id, "member", Vec::<String>::new()),
            model_id,
            runtime_core::runtime_acl::RuntimeDataAction::View,
        )
        .await
        .unwrap()
        .expect("role scope_all should still be bounded by owner grant");

    assert_eq!(
        grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::Owner
    );
}

#[tokio::test]
async fn root_runtime_scope_grant_loader_prefers_system_grant_over_workspace_overlay() {
    let actor_user_id = Uuid::now_v7();
    let workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let mut workspace_grant = scope_grant(
        Uuid::now_v7(),
        model_id,
        DataModelScopeKind::Workspace,
        workspace_id,
    );
    workspace_grant.permission_profile = domain::ScopeDataModelPermissionProfile::Owner;
    let mut system_grant = scope_grant(
        Uuid::now_v7(),
        model_id,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
    );
    system_grant.permission_profile = domain::ScopeDataModelPermissionProfile::SystemAll;
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, workspace_id))
            .with_model(system_model(model_id))
            .with_grant(workspace_grant)
            .with_grant(system_grant);
    let service = ModelDefinitionService::new(repository);

    let grant = service
        .load_runtime_scope_grant(
            &ActorContext::root(actor_user_id, workspace_id, "root"),
            model_id,
            runtime_core::runtime_acl::RuntimeDataAction::View,
        )
        .await
        .unwrap()
        .expect("root should use the system grant when one exists");

    assert_eq!(grant.scope_kind, DataModelScopeKind::System);
    assert_eq!(grant.scope_id, SYSTEM_SCOPE_ID);
    assert_eq!(
        grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::SystemAll
    );
}

#[tokio::test]
async fn non_root_scope_grant_create_rejects_system_and_other_workspace_scope() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let other_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        actor_workspace_id,
    ))
    .with_model(system_model(model_id));
    let service = ModelDefinitionService::new(repository);

    let system_error = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id,
                scope_kind: DataModelScopeKind::System,
                scope_id: SYSTEM_SCOPE_ID,
                data_model_id: model_id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap_err();
    assert!(system_error.to_string().contains("permission_denied"));

    let other_workspace_error = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id,
                scope_kind: DataModelScopeKind::Workspace,
                scope_id: other_workspace_id,
                data_model_id: model_id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap_err();
    assert!(other_workspace_error
        .to_string()
        .contains("permission_denied"));

    let current_workspace_grant = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id,
                scope_kind: DataModelScopeKind::Workspace,
                scope_id: actor_workspace_id,
                data_model_id: model_id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(current_workspace_grant.scope_id, actor_workspace_id);
}

#[tokio::test]
async fn non_root_scope_grant_update_delete_authorizes_existing_grant_scope() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let other_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let system_grant_id = Uuid::now_v7();
    let other_workspace_grant_id = Uuid::now_v7();
    let current_workspace_grant_id = Uuid::now_v7();
    let repository = ScopedModelDefinitionRepository::new(scoped_manager_in_workspace(
        actor_user_id,
        actor_workspace_id,
    ))
    .with_model(system_model(model_id))
    .with_grant(scope_grant(
        system_grant_id,
        model_id,
        DataModelScopeKind::System,
        SYSTEM_SCOPE_ID,
    ))
    .with_grant(scope_grant(
        other_workspace_grant_id,
        model_id,
        DataModelScopeKind::Workspace,
        other_workspace_id,
    ))
    .with_grant(scope_grant(
        current_workspace_grant_id,
        model_id,
        DataModelScopeKind::Workspace,
        actor_workspace_id,
    ));
    let service = ModelDefinitionService::new(repository);

    for grant_id in [system_grant_id, other_workspace_grant_id] {
        let update_error = service
            .update_scope_grant(UpdateScopeDataModelGrantCommand {
                actor_user_id,
                data_model_id: model_id,
                grant_id,
                enabled: Some(false),
                permission_profile: None,
                confirm_unsafe_external_source_system_all: false,
            })
            .await
            .unwrap_err();
        assert!(update_error.to_string().contains("permission_denied"));

        let delete_error = service
            .delete_scope_grant(DeleteScopeDataModelGrantCommand {
                actor_user_id,
                data_model_id: model_id,
                grant_id,
            })
            .await
            .unwrap_err();
        assert!(delete_error.to_string().contains("permission_denied"));
    }

    let updated = service
        .update_scope_grant(UpdateScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id: current_workspace_grant_id,
            enabled: Some(false),
            permission_profile: Some("owner".into()),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();
    assert_eq!(updated.scope_id, actor_workspace_id);
    assert!(!updated.enabled);

    let deleted = service
        .delete_scope_grant(DeleteScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id: current_workspace_grant_id,
        })
        .await
        .unwrap();
    assert_eq!(deleted.scope_id, actor_workspace_id);
}

#[tokio::test]
async fn root_scope_grant_lifecycle_can_manage_any_scope() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let other_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(system_model(model_id));
    let service = ModelDefinitionService::new(repository);

    let system_grant = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id,
                scope_kind: DataModelScopeKind::System,
                scope_id: SYSTEM_SCOPE_ID,
                data_model_id: model_id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(system_grant.scope_kind, DataModelScopeKind::System);

    let other_workspace_grant = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id,
                scope_kind: DataModelScopeKind::Workspace,
                scope_id: other_workspace_id,
                data_model_id: model_id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap();
    assert_eq!(other_workspace_grant.scope_id, other_workspace_id);

    let updated = service
        .update_scope_grant(UpdateScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id: system_grant.id,
            enabled: Some(false),
            permission_profile: Some("owner".into()),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();
    assert_eq!(updated.scope_kind, DataModelScopeKind::System);
    assert!(!updated.enabled);

    let deleted = service
        .delete_scope_grant(DeleteScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id: other_workspace_grant.id,
        })
        .await
        .unwrap();
    assert_eq!(deleted.scope_id, other_workspace_id);
}

#[tokio::test]
async fn unsafe_external_system_all_scope_grant_requires_explicit_confirmation() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(unsafe_external_system_model(model_id));
    let service = ModelDefinitionService::new(repository);

    let error = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "system_all".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("confirmation"));

    let grant = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "system_all".into(),
            confirm_unsafe_external_source_system_all: true,
        })
        .await
        .unwrap();
    assert_eq!(
        grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::SystemAll
    );
}

#[tokio::test]
async fn workspace_scope_system_all_grant_is_rejected_even_with_confirmation() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(unsafe_external_system_model(model_id));
    let service = ModelDefinitionService::new(repository);

    let error = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::Workspace,
            scope_id: actor_workspace_id,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "system_all".into(),
            confirm_unsafe_external_source_system_all: true,
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("system_all_requires_system_scope"));
}

#[tokio::test]
async fn unsafe_external_system_all_scope_grant_update_requires_explicit_confirmation() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let grant_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(unsafe_external_system_model(model_id))
            .with_grant(scope_grant(
                grant_id,
                model_id,
                DataModelScopeKind::System,
                SYSTEM_SCOPE_ID,
            ));
    let service = ModelDefinitionService::new(repository);

    let error = service
        .update_scope_grant(UpdateScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id,
            enabled: Some(true),
            permission_profile: Some("system_all".into()),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap_err();
    assert!(error.to_string().contains("confirmation"));

    let grant = service
        .update_scope_grant(UpdateScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id,
            enabled: Some(true),
            permission_profile: Some("system_all".into()),
            confirm_unsafe_external_source_system_all: true,
        })
        .await
        .unwrap();
    assert_eq!(
        grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::SystemAll
    );
}

#[tokio::test]
async fn workspace_scope_system_all_grant_update_is_rejected_even_with_confirmation() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let grant_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(unsafe_external_system_model(model_id))
            .with_grant(scope_grant(
                grant_id,
                model_id,
                DataModelScopeKind::Workspace,
                actor_workspace_id,
            ));
    let service = ModelDefinitionService::new(repository);

    let error = service
        .update_scope_grant(UpdateScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id,
            enabled: Some(true),
            permission_profile: Some("system_all".into()),
            confirm_unsafe_external_source_system_all: true,
        })
        .await
        .unwrap_err();
    assert!(error
        .to_string()
        .contains("system_all_requires_system_scope"));
}

#[tokio::test]
async fn safe_external_system_scope_system_all_grant_does_not_require_risk_confirmation() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let grant_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(safe_external_system_model(model_id))
            .with_grant(scope_grant(
                grant_id,
                model_id,
                DataModelScopeKind::System,
                SYSTEM_SCOPE_ID,
            ));
    let service = ModelDefinitionService::new(repository);

    let created = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "system_all".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();
    assert_eq!(
        created.permission_profile,
        domain::ScopeDataModelPermissionProfile::SystemAll
    );

    let updated = service
        .update_scope_grant(UpdateScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: model_id,
            grant_id,
            enabled: Some(true),
            permission_profile: Some("system_all".into()),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();
    assert_eq!(
        updated.permission_profile,
        domain::ScopeDataModelPermissionProfile::SystemAll
    );
}

#[tokio::test]
async fn main_source_system_all_scope_grant_does_not_require_external_risk_confirmation() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let model_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(system_model(model_id));
    let service = ModelDefinitionService::new(repository);

    let grant = service
        .create_scope_grant(CreateScopeDataModelGrantCommand {
            actor_user_id,
            scope_kind: DataModelScopeKind::System,
            scope_id: SYSTEM_SCOPE_ID,
            data_model_id: model_id,
            enabled: true,
            permission_profile: "system_all".into(),
            confirm_unsafe_external_source_system_all: false,
        })
        .await
        .unwrap();

    assert_eq!(
        grant.permission_profile,
        domain::ScopeDataModelPermissionProfile::SystemAll
    );
}

#[tokio::test]
async fn delete_scope_grant_records_audit_event() {
    let repository = InMemoryModelDefinitionRepository::default();
    let service = ModelDefinitionService::new(repository.clone());
    let created = service
        .create_model(CreateModelDefinitionCommand {
            actor_user_id: Uuid::nil(),
            scope_kind: DataModelScopeKind::Workspace,
            data_source_instance_id: None,
            external_resource_key: None,
            external_table_id: None,
            code: "delete_scope_grant_audit_orders".into(),
            title: "Delete Scope Grant Audit Orders".into(),
            status: None,
        })
        .await
        .unwrap();
    let grant = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id: Uuid::nil(),
                scope_kind: DataModelScopeKind::System,
                scope_id: SYSTEM_SCOPE_ID,
                data_model_id: created.id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap();

    service
        .delete_scope_grant(DeleteScopeDataModelGrantCommand {
            actor_user_id: Uuid::nil(),
            data_model_id: created.id,
            grant_id: grant.id,
        })
        .await
        .unwrap();

    assert!(repository
        .audit_events()
        .contains(&"state_model.scope_grant_deleted".to_string()));
}

#[tokio::test]
async fn delete_scope_grant_rejects_invisible_model_and_wrong_model_grant_pair() {
    let actor_user_id = Uuid::now_v7();
    let actor_workspace_id = Uuid::now_v7();
    let foreign_workspace_id = Uuid::now_v7();
    let foreign_model_id = Uuid::now_v7();
    let grant_model_id = Uuid::now_v7();
    let wrong_model_id = Uuid::now_v7();
    let repository =
        ScopedModelDefinitionRepository::new(actor_in_workspace(actor_user_id, actor_workspace_id))
            .with_model(model_in_workspace(foreign_model_id, foreign_workspace_id))
            .with_model(model_in_workspace(grant_model_id, actor_workspace_id))
            .with_model(model_in_workspace(wrong_model_id, actor_workspace_id));
    let service = ModelDefinitionService::new(repository.clone());
    let grant = service
        .create_scope_grant(
            control_plane::model_definition::CreateScopeDataModelGrantCommand {
                actor_user_id,
                scope_kind: DataModelScopeKind::Workspace,
                scope_id: actor_workspace_id,
                data_model_id: grant_model_id,
                enabled: true,
                permission_profile: "scope_all".into(),
                confirm_unsafe_external_source_system_all: false,
            },
        )
        .await
        .unwrap();

    let invisible_error = service
        .delete_scope_grant(DeleteScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: foreign_model_id,
            grant_id: grant.id,
        })
        .await
        .unwrap_err();
    assert!(invisible_error.to_string().contains("model_definition"));

    let wrong_pair_error = service
        .delete_scope_grant(DeleteScopeDataModelGrantCommand {
            actor_user_id,
            data_model_id: wrong_model_id,
            grant_id: grant.id,
        })
        .await
        .unwrap_err();
    assert!(wrong_pair_error
        .to_string()
        .contains("scope_data_model_grant"));

    assert!(!repository
        .audit_events()
        .contains(&"state_model.scope_grant_deleted".to_string()));
}
