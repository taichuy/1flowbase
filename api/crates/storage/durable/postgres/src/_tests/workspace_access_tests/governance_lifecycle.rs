use super::*;
use control_plane_contracts::{ports::*, ControlPlaneContractError as Error};
use domain::{RoleScopeKind, RoleTemplate, TenantStatus, UserStatus};

async fn store() -> PgControlPlaneStore {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}

fn provision(code: &str) -> ProvisionWorkspaceInput {
    ProvisionWorkspaceInput {
        tenant: ProvisionWorkspaceTenant::New {
            code: code.into(),
            name: code.into(),
        },
        workspace_name: code.into(),
        logo_url: None,
        introduction: String::new(),
        actor_user_id: None,
        role_templates: ["admin", "member"]
            .into_iter()
            .map(|code| RoleTemplate {
                code: code.into(),
                name: code.into(),
                introduction: String::new(),
                scope_kind: RoleScopeKind::Workspace,
                is_builtin: false,
                is_editable: true,
                auto_grant_new_permissions: code == "admin",
                is_default_member_role: code == "member",
                permissions: Vec::new(),
            })
            .collect(),
        initial_member: None,
    }
}

fn new_user(account: &str) -> NewGovernanceUser {
    NewGovernanceUser {
        account: account.into(),
        email: format!("{account}@example.com"),
        name: account.into(),
        nickname: account.into(),
        avatar_url: None,
        credential: GovernanceUserCredential::Password {
            password_hash: "test-hash".into(),
        },
    }
}

fn assert_denied(error: anyhow::Error, reason: &'static str) {
    assert!(
        matches!(error.downcast_ref::<Error>(), Some(Error::PermissionDenied(code)) if *code == reason),
        "{error:?}"
    );
}

#[tokio::test]
async fn orphan_user_cannot_fall_back_to_root_workspace() {
    let store = store().await;
    let root_workspace =
        insert_workspace(&store, root_tenant_id(&store).await, "Root fallback").await;
    let orphan = Uuid::now_v7();
    insert_user(&store, orphan, "orphan", "member").await;
    assert_denied(
        store.default_scope_for_user(orphan).await.unwrap_err(),
        "no_accessible_workspace",
    );
    assert_denied(
        crate::repositories::workspace_id_for_user(store.pool(), orphan)
            .await
            .unwrap_err(),
        "no_accessible_workspace",
    );
    bind_role(&store, orphan, insert_root_role(&store).await).await;
    assert_eq!(
        store
            .default_scope_for_user(orphan)
            .await
            .unwrap()
            .workspace_id,
        root_workspace
    );
}

#[tokio::test]
async fn provision_seeds_workspace_and_rolls_back_conflicts_and_suspension_blocks_access() {
    let store = store().await;
    control_plane_test_support::upsert_permission_catalog(&store)
        .await
        .unwrap();
    sqlx::query("insert into console_permission_catalog_groups (group_kind, group_id) values ('settings_feature', 'test-provision')")
        .execute(store.pool()).await.unwrap();
    let mut input = provision("provisioned");
    input.role_templates[0].permissions = vec!["application.view.own".into()];
    input.initial_member = Some(ProvisionWorkspaceMember {
        user: ProvisionWorkspaceUser::New(Box::new(new_user("founder"))),
        role_codes: vec!["admin".into()],
    });
    let result = store.provision_workspace(&input).await.unwrap();
    let founder = result.initial_member_user_id.unwrap();
    let tenant = result.tenant.id;
    let workspace = result.workspace.id;
    assert_eq!(result.tenant.status, TenantStatus::Active);
    assert_eq!(result.tenant.cell_id, "default");
    assert!(!result.tenant.is_root && !result.tenant.is_hidden);
    for query in [
        "select count(*) from notification_instances where workspace_id = $1 and type_code = 'system' and is_builtin",
        "select count(*) from workspace_i18n_catalog_states where workspace_id = $1",
        "select count(*) from workspace_billing_settings where workspace_id = $1",
        "select count(*) from roles where workspace_id = $1 and is_default_member_role",
        "select count(*) from role_permissions rp join roles r on r.id = rp.role_id where r.workspace_id = $1 and r.code = 'admin'",
        "select count(*) from role_data_policies dp join roles r on r.id = dp.role_id where r.workspace_id = $1 and r.code = 'admin' and dp.default_view_scope = 'scope_all'",
        "select count(*) from role_console_group_policies p join roles r on r.id = p.role_id where r.workspace_id = $1 and r.code = 'admin' and p.group_id = 'test-provision' and p.enabled and p.strategy = 'full'",
        "select count(*) from audit_logs where workspace_id = $1 and event_code = 'workspace.member.onboarded'",
    ] {
        assert_eq!(sqlx::query_scalar::<_, i64>(query).bind(workspace).fetch_one(store.pool()).await.unwrap(), 1, "{query}");
    }
    let summaries = store.list_tenants_and_workspaces().await.unwrap();
    let summary = &summaries
        .iter()
        .find(|row| row.tenant.id == tenant)
        .unwrap()
        .workspaces[0];
    assert_eq!(summary.active_member_count, 1);
    assert_eq!(summary.admins[0].user_id, founder);
    assert_eq!(
        store
            .inspect_workspace_quota_counts(tenant, workspace)
            .await
            .unwrap(),
        WorkspaceQuotaCounts {
            active_member_count: 1,
            tenant_workspace_count: 1,
            workspace_application_count: 0,
        }
    );
    assert_eq!(
        store
            .lookup_governance_user(&GovernanceUserLookup::Email("founder@example.com".into()))
            .await
            .unwrap()
            .unwrap()
            .user_id,
        founder
    );
    assert!(store
        .find_user_for_password_login(domain::PASSWORD_LOCAL_CONNECTION_ID, "founder")
        .await
        .unwrap()
        .is_some());
    store
        .verify_workspace_access(founder, tenant, workspace, false)
        .await
        .unwrap();

    input.tenant = ProvisionWorkspaceTenant::New {
        code: "must-roll-back".into(),
        name: "Rollback".into(),
    };
    input.workspace_name = "must-roll-back".into();
    let error = store.provision_workspace(&input).await.unwrap_err();
    assert!(
        matches!(error.downcast_ref::<Error>(), Some(Error::Conflict(_))),
        "{error:?}"
    );
    assert!(!sqlx::query_scalar::<_, bool>("select exists(select 1 from tenants where code = 'must-roll-back') or exists(select 1 from workspaces where name = 'must-roll-back')").fetch_one(store.pool()).await.unwrap());

    assert_denied(
        store
            .set_tenant_status(root_tenant_id(&store).await, TenantStatus::Suspended, None)
            .await
            .unwrap_err(),
        "root_tenant_immutable",
    );
    let application = store
        .create_application(&CreateApplicationInput {
            actor_user_id: founder,
            workspace_id: workspace,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Public API".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    assert_eq!(
        store
            .get_application_for_api_key(tenant, workspace, application.id)
            .await
            .unwrap()
            .unwrap()
            .id,
        application.id
    );
    assert!(store
        .get_application_for_api_key(root_tenant_id(&store).await, workspace, application.id)
        .await
        .unwrap()
        .is_none());
    input.tenant = ProvisionWorkspaceTenant::Existing { tenant_id: tenant };
    input.workspace_name = "Second workspace".into();
    input.initial_member = Some(ProvisionWorkspaceMember {
        user: ProvisionWorkspaceUser::Existing { user_id: founder },
        role_codes: vec![],
    });
    let second = store.provision_workspace(&input).await.unwrap();
    assert_eq!(second.tenant.id, tenant);
    assert_eq!(second.initial_member_user_id, Some(founder));
    assert_eq!(
        store
            .inspect_workspace_quota_counts(tenant, workspace)
            .await
            .unwrap(),
        WorkspaceQuotaCounts {
            active_member_count: 1,
            tenant_workspace_count: 2,
            workspace_application_count: 1,
        }
    );
    store
        .set_tenant_status(tenant, TenantStatus::Suspended, None)
        .await
        .unwrap();
    assert_denied(
        store
            .get_application_for_api_key(tenant, workspace, application.id)
            .await
            .unwrap_err(),
        "tenant_suspended",
    );
    input.workspace_name = "Blocked workspace".into();
    assert_denied(
        store.provision_workspace(&input).await.unwrap_err(),
        "tenant_suspended",
    );
    assert_denied(
        store
            .verify_workspace_access(founder, tenant, workspace, false)
            .await
            .unwrap_err(),
        "tenant_suspended",
    );
    assert_denied(
        store
            .load_actor_context(founder, tenant, workspace, None)
            .await
            .unwrap_err(),
        "tenant_suspended",
    );
    assert!(store
        .get_accessible_workspace(founder, workspace)
        .await
        .unwrap()
        .is_none());
    assert_denied(
        store
            .grant_or_activate_workspace_membership(&GrantWorkspaceMembershipInput {
                workspace_id: workspace,
                user_id: founder,
                role_codes: vec![],
                role_templates: vec![],
                actor_user_id: None,
            })
            .await
            .unwrap_err(),
        "tenant_suspended",
    );
}

async fn onboard(store: &PgControlPlaneStore, workspace_id: Uuid, account: &str) -> Uuid {
    store
        .onboard_new_user_to_workspace(&OnboardNewUserInput {
            workspace_id,
            user: new_user(account),
            role_codes: vec![],
            role_templates: vec![],
            actor_user_id: None,
        })
        .await
        .unwrap()
        .user_id
}

async fn user_snapshot(store: &PgControlPlaneStore, user: Uuid) -> serde_json::Value {
    sqlx::query_scalar("select to_jsonb(u) from users u where id = $1")
        .bind(user)
        .fetch_one(store.pool())
        .await
        .unwrap()
}

async fn workspace_member_snapshot(
    store: &PgControlPlaneStore,
    user: Uuid,
    workspace: Uuid,
) -> serde_json::Value {
    sqlx::query_scalar("select jsonb_build_object(
        'membership', (select to_jsonb(wm) from workspace_memberships wm where wm.user_id = $1 and wm.workspace_id = $2),
        'roles', (select jsonb_agg(to_jsonb(b) order by b.id) from user_role_bindings b where b.user_id = $1 and b.scope_id = $2),
        'keys', (select jsonb_agg(to_jsonb(k) order by k.id) from api_keys k where k.creator_user_id = $1 and k.scope_id = $2)
    )").bind(user).bind(workspace).fetch_one(store.pool()).await.unwrap()
}

#[tokio::test]
async fn workspace_member_mutations_do_not_change_other_tenant_identity_or_access() {
    let store = store().await;
    let mut a_input = provision("tenant-a");
    a_input.initial_member = Some(ProvisionWorkspaceMember {
        user: ProvisionWorkspaceUser::New(Box::new(new_user("admin-a"))),
        role_codes: vec!["admin".into()],
    });
    let a = store.provision_workspace(&a_input).await.unwrap();
    let b = store
        .provision_workspace(&provision("tenant-b"))
        .await
        .unwrap();
    let actor = store
        .load_actor_context(
            a.initial_member_user_id.unwrap(),
            a.tenant.id,
            a.workspace.id,
            None,
        )
        .await
        .unwrap();
    assert!(!actor.is_root);
    let shared = onboard(&store, a.workspace.id, "dual-member").await;
    let unrelated = onboard(&store, b.workspace.id, "b-member").await;
    let grant = GrantWorkspaceMembershipInput {
        workspace_id: b.workspace.id,
        user_id: shared,
        role_codes: vec![],
        role_templates: vec![],
        actor_user_id: None,
    };
    assert!(
        !store
            .grant_or_activate_workspace_membership(&grant)
            .await
            .unwrap()
            .already_active_member
    );
    assert!(
        store
            .grant_or_activate_workspace_membership(&grant)
            .await
            .unwrap()
            .already_active_member
    );
    assert_eq!(
        store
            .default_scope_for_user(shared)
            .await
            .unwrap()
            .workspace_id,
        a.workspace.id
    );

    let a_key = Uuid::now_v7();
    let b_key = Uuid::now_v7();
    for (workspace, tenant, id) in [
        (a.workspace.id, a.tenant.id, a_key),
        (b.workspace.id, b.tenant.id, b_key),
    ] {
        store
            .create_api_key(&CreateApiKeyInput {
                id,
                name: "test".into(),
                token_hash: id.to_string(),
                token_prefix: "test".into(),
                key_kind: domain::ApiKeyKind::UserApiKey,
                application_id: None,
                role_code: Some("member".into()),
                creator_user_id: shared,
                tenant_id: tenant,
                scope_kind: domain::DataModelScopeKind::Workspace,
                scope_id: workspace,
                enabled: true,
                expires_at: None,
            })
            .await
            .unwrap();
    }
    let global_before = user_snapshot(&store, shared).await;
    let b_before = workspace_member_snapshot(&store, shared, b.workspace.id).await;
    let profile = |user_id| UpdateMemberInput {
        actor_user_id: actor.user_id,
        user_id,
        name: "overwrite".into(),
        nickname: "overwrite".into(),
        email: "overwrite@example.com".into(),
        phone: None,
        introduction: String::new(),
    };
    assert_denied(
        store
            .update_member_profile_in_scope(&actor, &profile(shared))
            .await
            .unwrap_err(),
        "member_identity_externally_owned",
    );
    assert_denied(
        store
            .reset_member_password_in_scope(&actor, shared, "overwrite")
            .await
            .unwrap_err(),
        "member_identity_externally_owned",
    );
    for error in [
        store
            .disable_member_in_scope(&actor, unrelated)
            .await
            .unwrap_err(),
        store
            .enable_member_in_scope(&actor, unrelated)
            .await
            .unwrap_err(),
        store
            .delete_member_in_scope(&actor, unrelated)
            .await
            .unwrap_err(),
        store
            .update_member_profile_in_scope(&actor, &profile(unrelated))
            .await
            .unwrap_err(),
        store
            .reset_member_password_in_scope(&actor, unrelated, "overwrite")
            .await
            .unwrap_err(),
    ] {
        assert!(
            matches!(error.downcast_ref::<Error>(), Some(Error::NotFound("user"))),
            "{error:?}"
        );
    }
    let conflict = store
        .create_member_with_default_role_in_scope(
            &actor,
            &CreateMemberInput {
                actor_user_id: actor.user_id,
                workspace_id: a.workspace.id,
                account: "new-account".into(),
                email: "b-member@example.com".into(),
                phone: None,
                password_hash: "test-hash".into(),
                name: "new-account".into(),
                nickname: "new-account".into(),
                introduction: String::new(),
                email_login_enabled: true,
                phone_login_enabled: false,
            },
        )
        .await
        .unwrap_err();
    assert!(matches!(
        conflict.downcast_ref::<Error>(),
        Some(Error::Conflict("member_account_exists_use_invitation"))
    ));

    store.disable_member_in_scope(&actor, shared).await.unwrap();
    assert_eq!(
        store
            .list_members(a.workspace.id, None)
            .await
            .unwrap()
            .into_iter()
            .find(|member| member.id == shared)
            .unwrap()
            .status,
        UserStatus::Disabled
    );
    assert!(store
        .get_accessible_workspace(shared, a.workspace.id)
        .await
        .unwrap()
        .is_none());
    assert_denied(
        store
            .load_actor_context_for_bound_role(shared, a.tenant.id, a.workspace.id, "member")
            .await
            .unwrap_err(),
        "workspace_access_denied",
    );
    assert_eq!(
        store
            .default_scope_for_user(shared)
            .await
            .unwrap()
            .workspace_id,
        b.workspace.id
    );
    store
        .load_actor_context(shared, b.tenant.id, b.workspace.id, None)
        .await
        .unwrap();
    assert_eq!(user_snapshot(&store, shared).await, global_before);
    assert_eq!(
        workspace_member_snapshot(&store, shared, b.workspace.id).await,
        b_before
    );

    let a_grant = GrantWorkspaceMembershipInput {
        workspace_id: a.workspace.id,
        ..grant.clone()
    };
    assert!(
        !store
            .grant_or_activate_workspace_membership(&a_grant)
            .await
            .unwrap()
            .already_active_member
    );
    store
        .get_accessible_workspace(shared, a.workspace.id)
        .await
        .unwrap()
        .unwrap();
    store.delete_member_in_scope(&actor, shared).await.unwrap();
    assert_eq!(
        store
            .default_scope_for_user(shared)
            .await
            .unwrap()
            .workspace_id,
        b.workspace.id
    );
    assert_eq!(user_snapshot(&store, shared).await, global_before);
    assert_eq!(
        workspace_member_snapshot(&store, shared, b.workspace.id).await,
        b_before
    );
    let a_after = workspace_member_snapshot(&store, shared, a.workspace.id).await;
    assert!(a_after["membership"].is_null() && a_after["roles"].is_null());
    assert_eq!(a_after["keys"][0]["enabled"], false);
}

#[tokio::test]
async fn simultaneous_onboarding_of_one_external_subject_has_one_winner() {
    let store = store().await;
    let result = store
        .provision_workspace(&provision("external-onboarding"))
        .await
        .unwrap();
    let workspace_id = result.workspace.id;
    let connection_id = Uuid::now_v7();
    sqlx::query("insert into authentication_connections (id, auth_type) values ($1, 'test-oidc')")
        .bind(connection_id)
        .execute(store.pool())
        .await
        .unwrap();
    let identity = domain::VerifiedExternalIdentity::try_from(domain::ExternalIdentityClaim {
        connection_id,
        subject_type: "oidc-sub".into(),
        subject_value: "subject-1".into(),
        issuer: Some("https://issuer.invalid".into()),
        realm: None,
        verified: true,
        profile: serde_json::json!({}),
        metadata: serde_json::json!({}),
    })
    .unwrap();
    let request = |account| {
        let mut user = new_user(account);
        user.credential = GovernanceUserCredential::External(Box::new(identity.clone()));
        OnboardNewUserInput {
            workspace_id,
            user,
            role_codes: vec![],
            role_templates: vec![],
            actor_user_id: None,
        }
    };
    let first = request("external-a");
    let second = request("external-b");
    let (one, two) = tokio::join!(
        store.onboard_new_user_to_workspace(&first),
        store.onboard_new_user_to_workspace(&second)
    );
    let (winner, error, loser) = match (one, two) {
        (Ok(winner), Err(error)) => (winner, error, second.user.account),
        (Err(error), Ok(winner)) => (winner, error, first.user.account),
        results => panic!("expected one winner and one conflict: {results:?}"),
    };
    assert!(
        matches!(
            error.downcast_ref::<Error>(),
            Some(Error::Conflict("workspace_onboarding_identity_exists"))
        ),
        "{error:?}"
    );
    assert!(store
        .lookup_governance_user(&GovernanceUserLookup::Account(loser))
        .await
        .unwrap()
        .is_none());
    assert_eq!(
        store
            .find_user_for_verified_external_identity(&identity)
            .await
            .unwrap()
            .unwrap()
            .id,
        winner.user_id
    );
    assert_eq!(
        store
            .inspect_workspace_quota_counts(result.tenant.id, workspace_id)
            .await
            .unwrap()
            .active_member_count,
        1
    );

    let index_names: Vec<String> = sqlx::query_scalar(
        "select indexname from pg_indexes where schemaname = current_schema() and indexname in ('tenants_status_cell_id', 'workspace_memberships_status', 'workspace_memberships_workspace_status', 'users_managed_by_tenant_id') order by indexname",
    )
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        index_names,
        vec![
            "tenants_status_cell_id",
            "users_managed_by_tenant_id",
            "workspace_memberships_status",
            "workspace_memberships_workspace_status",
        ]
    );
}

#[tokio::test]
async fn flat_governance_is_atomic_and_workspace_status_is_independent() {
    let store = store().await;
    let root_tenant = root_tenant_id(&store).await;
    let mut root_input = provision("platform");
    root_input.tenant = ProvisionWorkspaceTenant::Existing {
        tenant_id: root_tenant,
    };
    let root = store
        .provision_workspace(&root_input)
        .await
        .unwrap()
        .workspace
        .id;
    let root_user = Uuid::now_v7();
    insert_user(&store, root_user, "root-admin", "root").await;
    bind_role(&store, root_user, insert_root_role(&store).await).await;
    assert_denied(
        store
            .set_workspace_status(root, TenantStatus::Suspended, None)
            .await
            .unwrap_err(),
        "root_workspace_immutable",
    );
    assert_denied(
        store
            .revoke_workspace_admin(root, root_user, &[], None)
            .await
            .unwrap_err(),
        "root_admin_immutable",
    );

    let a = store
        .provision_workspace(&provision("flat-a"))
        .await
        .unwrap();
    let mut sibling = provision("flat-b");
    sibling.tenant = ProvisionWorkspaceTenant::Existing {
        tenant_id: a.tenant.id,
    };
    let b = store.provision_workspace(&sibling).await.unwrap();
    let (a_id, b_id) = (a.workspace.id, b.workspace.id);
    let candidate = ProvisionWorkspaceUser::New(Box::new(new_user("flat-admin")));
    assert!(store
        .assign_workspace_admins(&[], &candidate, &[], None)
        .await
        .is_err());
    assert!(store
        .assign_workspace_admins(&[a_id, Uuid::now_v7()], &candidate, &[], None)
        .await
        .is_err());
    let updated = store
        .set_workspace_status(b_id, TenantStatus::Suspended, None)
        .await
        .unwrap();
    assert_eq!(updated.status, TenantStatus::Suspended);
    assert!(!updated.is_root_workspace);
    assert_denied(
        store
            .assign_workspace_admins(&[a_id, b_id], &candidate, &[], None)
            .await
            .unwrap_err(),
        "workspace_suspended",
    );
    let count: i64 = sqlx::query_scalar("select count(*) from users where account = 'flat-admin'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!(count, 0);

    store
        .set_workspace_status(b_id, TenantStatus::Active, None)
        .await
        .unwrap();
    let user = store
        .assign_workspace_admins(&[b_id, a_id, b_id], &candidate, &[], None)
        .await
        .unwrap();
    store
        .assign_workspace_admins(
            &[a_id, b_id],
            &ProvisionWorkspaceUser::Existing {
                user_id: user.user_id,
            },
            &[],
            None,
        )
        .await
        .unwrap();
    let pool = store.pool();
    let user_id = user.user_id;
    let roles = |workspace_id| async move {
        sqlx::query_scalar::<_, String>("select r.code from user_role_bindings b join roles r on r.id = b.role_id where b.user_id = $1 and b.scope_id = $2 order by r.code")
            .bind(user_id).bind(workspace_id).fetch_all(pool).await.unwrap()
    };
    assert_eq!(roles(a_id).await, ["admin"]);
    assert_eq!(roles(b_id).await, ["admin"]);
    store
        .load_actor_context_for_bound_role(user_id, a.tenant.id, b_id, "admin")
        .await
        .unwrap();
    let app = store
        .create_application(&CreateApplicationInput {
            actor_user_id: user_id,
            workspace_id: b_id,
            application_type: domain::ApplicationType::AgentFlow,
            workflow_trigger_type: None,
            workflow_trigger_config: None,
            name: "Workspace B API".into(),
            description: String::new(),
            icon: None,
            icon_type: None,
            icon_background: None,
        })
        .await
        .unwrap();
    assert!(store
        .get_application_for_api_key(a.tenant.id, b_id, app.id)
        .await
        .unwrap()
        .is_some());

    store
        .set_workspace_status(b_id, TenantStatus::Suspended, None)
        .await
        .unwrap();
    assert_denied(
        store
            .load_actor_context(user.user_id, a.tenant.id, b_id, None)
            .await
            .unwrap_err(),
        "workspace_suspended",
    );
    assert_denied(
        store
            .load_actor_context_for_bound_role(user_id, a.tenant.id, b_id, "admin")
            .await
            .unwrap_err(),
        "workspace_suspended",
    );
    assert_denied(
        store
            .get_application_for_api_key(a.tenant.id, b_id, app.id)
            .await
            .unwrap_err(),
        "workspace_suspended",
    );
    assert!(store
        .get_accessible_workspace(root_user, b_id)
        .await
        .unwrap()
        .is_none());
    assert!(store
        .load_actor_context(user.user_id, a.tenant.id, a_id, None)
        .await
        .is_ok());
    let summaries = store.list_tenants_and_workspaces().await.unwrap();
    let workspaces = summaries
        .iter()
        .flat_map(|tenant| &tenant.workspaces)
        .map(|ws| &ws.workspace)
        .collect::<Vec<_>>();
    assert!(workspaces
        .iter()
        .any(|ws| ws.record.id == root && ws.is_root_workspace));
    assert!(workspaces
        .iter()
        .any(|ws| ws.record.id == b_id && ws.status == TenantStatus::Suspended));

    let version = || async {
        sqlx::query_scalar::<_, i64>("select session_version::bigint from users where id = $1")
            .bind(user.user_id)
            .fetch_one(store.pool())
            .await
            .unwrap()
    };
    let before = version().await;
    assert!(store
        .revoke_workspace_admin(a_id, user.user_id, &[], None)
        .await
        .unwrap());
    assert!(!store
        .revoke_workspace_admin(a_id, user.user_id, &[], None)
        .await
        .unwrap());
    assert_eq!(version().await, before + 1);
    assert_eq!(roles(a_id).await, ["member"]);
    assert_eq!(roles(b_id).await, ["admin"]);

    let single = store
        .provision_workspace(&provision("single"))
        .await
        .unwrap();
    for status in [TenantStatus::Suspended, TenantStatus::Active] {
        store
            .set_workspace_status(single.workspace.id, status, None)
            .await
            .unwrap();
        let tenant_status: String = sqlx::query_scalar("select status from tenants where id = $1")
            .bind(single.tenant.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
        assert_eq!(tenant_status, status.as_str());
    }
}

#[tokio::test]
async fn bootstrap_admin_summary_includes_protected_system_root_only_where_applicable() {
    let store = store().await;
    let tenant = store.upsert_root_tenant().await.unwrap();
    let bootstrap = store
        .upsert_workspace(tenant.id, "1flowbase")
        .await
        .unwrap()
        .id;
    control_plane_test_support::upsert_permission_catalog(&store)
        .await
        .unwrap();
    control_plane_test_support::upsert_builtin_roles(&store, bootstrap)
        .await
        .unwrap();
    let root = store
        .upsert_root_user(
            bootstrap,
            "root",
            "root@example.com",
            "test-hash",
            "Root",
            "Root",
        )
        .await
        .unwrap();

    async fn admins(store: &PgControlPlaneStore, workspace_id: Uuid) -> Vec<String> {
        store
            .list_tenants_and_workspaces()
            .await
            .unwrap()
            .into_iter()
            .flat_map(|tenant| tenant.workspaces)
            .find(|workspace| workspace.workspace.record.id == workspace_id)
            .unwrap()
            .admins
            .into_iter()
            .map(|admin| admin.account)
            .collect()
    }

    assert_eq!(admins(&store, bootstrap).await, ["root"]);
    let mut sibling = provision("root-sibling");
    sibling.tenant = ProvisionWorkspaceTenant::Existing {
        tenant_id: tenant.id,
    };
    let sibling = store
        .provision_workspace(&sibling)
        .await
        .unwrap()
        .workspace
        .id;
    let acme = store
        .provision_workspace(&provision("acme"))
        .await
        .unwrap()
        .workspace
        .id;
    for workspace_id in [sibling, acme] {
        store
            .grant_or_activate_workspace_membership(&GrantWorkspaceMembershipInput {
                workspace_id,
                user_id: root.id,
                role_codes: vec!["member".into()],
                role_templates: vec![],
                actor_user_id: None,
            })
            .await
            .unwrap();
        assert!(admins(&store, workspace_id).await.is_empty());
    }
    store
        .grant_or_activate_workspace_membership(&GrantWorkspaceMembershipInput {
            workspace_id: acme,
            user_id: root.id,
            role_codes: vec!["admin".into()],
            role_templates: vec![],
            actor_user_id: None,
        })
        .await
        .unwrap();
    assert_eq!(admins(&store, acme).await, ["root"]);
    assert!(admins(&store, sibling).await.is_empty());
    let alice = store
        .onboard_new_user_to_workspace(&OnboardNewUserInput {
            workspace_id: bootstrap,
            user: new_user("alice"),
            role_codes: vec!["admin".into()],
            role_templates: vec![],
            actor_user_id: Some(root.id),
        })
        .await
        .unwrap();
    assert_denied(
        store
            .assign_workspace_admins(
                &[bootstrap],
                &ProvisionWorkspaceUser::Existing {
                    user_id: alice.user_id,
                },
                &[],
                Some(root.id),
            )
            .await
            .unwrap_err(),
        "root_admin_immutable",
    );
    assert_denied(
        store
            .revoke_workspace_admin(bootstrap, root.id, &[], Some(root.id))
            .await
            .unwrap_err(),
        "root_admin_immutable",
    );
    assert_denied(
        store
            .revoke_workspace_admin(bootstrap, alice.user_id, &[], Some(root.id))
            .await
            .unwrap_err(),
        "root_admin_immutable",
    );
    // Root workspace administrator is strictly locked to the built-in root user.
    assert_eq!(admins(&store, bootstrap).await, ["root"]);

    let searched = store
        .search_governance_users(Some("ali"), 20)
        .await
        .unwrap();
    assert_eq!(searched.len(), 1);
    assert_eq!(searched[0].account, "alice");
}

#[tokio::test]
async fn legacy_roleless_workspace_self_heals_standard_roles_on_admin_assign_and_revoke() {
    let store = store().await;
    let root_tenant = store.upsert_root_tenant().await.unwrap();
    let _bootstrap = store
        .upsert_workspace(root_tenant.id, "1flowbase")
        .await
        .unwrap();
    control_plane_test_support::upsert_permission_catalog(&store)
        .await
        .unwrap();

    // Simulate a legacy workspace inserted without provision_workspace (0 rows in `roles`).
    let legacy_ws = store
        .upsert_workspace(root_tenant.id, "legacy-gateway-diag")
        .await
        .unwrap()
        .id;
    let initial_role_count: i64 = sqlx::query_scalar(
        "select count(*) from roles where workspace_id = $1 and scope_kind = 'workspace'",
    )
    .bind(legacy_ws)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(initial_role_count, 0);

    let candidate_id = Uuid::now_v7();
    insert_user(&store, candidate_id, "legacy-admin", "member").await;

    let templates = provision("tpl-source").role_templates;
    let assigned = store
        .assign_workspace_admins(
            &[legacy_ws],
            &ProvisionWorkspaceUser::Existing {
                user_id: candidate_id,
            },
            &templates,
            None,
        )
        .await
        .unwrap();
    assert_eq!(assigned.user_id, candidate_id);

    let seeded_codes: Vec<String> = sqlx::query_scalar(
        "select code from roles where workspace_id = $1 and scope_kind = 'workspace' order by code",
    )
    .bind(legacy_ws)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(
        seeded_codes,
        vec!["admin".to_string(), "member".to_string()]
    );

    let bound_after_assign: Vec<String> = sqlx::query_scalar(
        "select r.code from user_role_bindings b join roles r on r.id = b.role_id where b.user_id = $1 and b.scope_id = $2 order by r.code",
    )
    .bind(candidate_id)
    .bind(legacy_ws)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(bound_after_assign, vec!["admin".to_string()]);

    // Revoking admin on the healed legacy workspace demotes the active member cleanly to `member`.
    let revoked = store
        .revoke_workspace_admin(legacy_ws, candidate_id, &templates, None)
        .await
        .unwrap();
    assert!(revoked);

    let bound_after_revoke: Vec<String> = sqlx::query_scalar(
        "select r.code from user_role_bindings b join roles r on r.id = b.role_id where b.user_id = $1 and b.scope_id = $2 order by r.code",
    )
    .bind(candidate_id)
    .bind(legacy_ws)
    .fetch_all(store.pool())
    .await
    .unwrap();
    assert_eq!(bound_after_revoke, vec!["member".to_string()]);
}
