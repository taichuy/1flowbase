use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane_contracts::{ports::*, ControlPlaneContractError as Error};
use domain::{
    AuditLogRecord, RoleScopeKind, TenantRecord, TenantStatus, UserStatus,
    WorkspaceMembershipStatus, WorkspaceRecord,
};
use sqlx::{postgres::PgRow, types::Json, Postgres, Row, Transaction};
use time::OffsetDateTime;
use uuid::Uuid;

use crate::{
    auth_repository::{
        identity_binding::insert_password_local_identities,
        lifecycle::{identity_audit_payload, lock_canonical_identity, write_audit},
        lock_console_permission_catalog, seed_role_templates,
    },
    repositories::PgControlPlaneStore,
    workspace_repository::bootstrap_workspace_id,
};

type Tx<'a> = Transaction<'a, Postgres>;
type WorkspaceRole = (Uuid, String);

pub(crate) fn map_tenant(row: &PgRow) -> Result<TenantRecord> {
    let status: &str = row.try_get("status")?;
    Ok(TenantRecord {
        id: row.try_get("id")?,
        code: row.try_get("code")?,
        name: row.try_get("name")?,
        is_root: row.try_get("is_root")?,
        is_hidden: row.try_get("is_hidden")?,
        status: TenantStatus::from_db(status)
            .ok_or_else(|| anyhow!("invalid stored tenant status"))?,
        cell_id: row.try_get("cell_id")?,
    })
}

pub(crate) fn map_unique_conflict(error: anyhow::Error, reason: &'static str) -> anyhow::Error {
    if error
        .downcast_ref::<sqlx::Error>()
        .and_then(sqlx::Error::as_database_error)
        .and_then(|error| error.code())
        .as_deref()
        == Some("23505")
    {
        Error::Conflict(reason).into()
    } else {
        error
    }
}

fn ensure_tenant_active(status: &str) -> Result<()> {
    if TenantStatus::from_db(status) != Some(TenantStatus::Active) {
        return Err(Error::PermissionDenied("tenant_suspended").into());
    }
    Ok(())
}

async fn lock_active_workspace(tx: &mut Tx<'_>, workspace_id: Uuid) -> Result<Uuid> {
    // Serialize a grant with tenant suspension and workspace deletion.
    let (tenant_id, status, workspace_status): (Uuid, String, String) = sqlx::query_as(
        "select t.id, t.status, w.status from workspaces w join tenants t on t.id = w.tenant_id
         where w.id = $1 for share of t, w",
    )
    .bind(workspace_id)
    .fetch_optional(&mut **tx)
    .await?
    .ok_or(Error::NotFound("workspace"))?;
    ensure_tenant_active(&status)?;
    if workspace_status != "active" {
        return Err(Error::PermissionDenied("workspace_suspended").into());
    }
    Ok(tenant_id)
}

async fn workspace_roles(
    tx: &mut Tx<'_>,
    workspace_id: Uuid,
    role_codes: &[String],
) -> Result<Vec<WorkspaceRole>> {
    let roles: Vec<WorkspaceRole> = sqlx::query_as(
        "select id, code from roles
         where workspace_id = $1 and scope_id = $1 and scope_kind = 'workspace'
           and system_kind is null and code not in ('root', 'system')
           and (case when cardinality($2::text[]) = 0 then is_default_member_role else code = any($2) end)
         order by code, id for share",
    ).bind(workspace_id).bind(role_codes).fetch_all(&mut **tx).await?;
    if role_codes.is_empty() && roles.is_empty() {
        return Err(Error::NotFound("default_member_role").into());
    }
    if role_codes
        .iter()
        .any(|code| !roles.iter().any(|(_, stored)| code == stored))
    {
        return Err(Error::NotFound("role").into());
    }
    Ok(roles)
}

async fn grant_membership(
    tx: &mut Tx<'_>,
    workspace_id: Uuid,
    user_id: Uuid,
    roles: &[WorkspaceRole],
    actor: Option<Uuid>,
) -> Result<GrantWorkspaceMembershipOutput> {
    // Match the member mutation lock order. The user lock also serializes grants
    // for a yet-nonexistent membership with identity edits and physical deletion.
    sqlx::query("select id from users where id = $1 for update")
        .bind(user_id)
        .fetch_optional(&mut **tx)
        .await?
        .ok_or(Error::NotFound("user"))?;
    let prior: Option<String> = sqlx::query_scalar(
        "select status from workspace_memberships where workspace_id = $1 and user_id = $2 for update",
    ).bind(workspace_id).bind(user_id).fetch_optional(&mut **tx).await?;
    let already_active_member = prior
        .as_deref()
        .and_then(WorkspaceMembershipStatus::from_db)
        == Some(WorkspaceMembershipStatus::Active);
    if !already_active_member {
        sqlx::query(
            "insert into workspace_memberships (id, workspace_id, user_id, status, created_by, updated_by)
             values ($1, $2, $3, 'active', $4, $4)
             on conflict (workspace_id, user_id) do update
             set status = excluded.status, updated_by = excluded.updated_by, updated_at = now()",
        ).bind(Uuid::now_v7()).bind(workspace_id).bind(user_id).bind(actor).execute(&mut **tx).await?;
    }
    for (role_id, _) in roles {
        sqlx::query(
            "insert into user_role_bindings (id, user_id, role_id, scope_id, created_by, updated_by)
             values ($1, $2, $3, $4, $5, $5) on conflict (user_id, role_id) do nothing",
        ).bind(Uuid::now_v7()).bind(user_id).bind(role_id).bind(workspace_id).bind(actor).execute(&mut **tx).await?;
    }
    Ok(GrantWorkspaceMembershipOutput {
        already_active_member,
    })
}

async fn onboard_user(
    tx: &mut Tx<'_>,
    tenant_id: Uuid,
    workspace_id: Uuid,
    input: &NewGovernanceUser,
    roles: &[WorkspaceRole],
    actor: Option<Uuid>,
) -> Result<Uuid> {
    let user_id = Uuid::now_v7();
    let (password_hash, password_login) = match &input.credential {
        GovernanceUserCredential::Password { password_hash } => (password_hash.as_str(), true),
        GovernanceUserCredential::External(identity) => {
            lock_canonical_identity(tx, identity).await?;
            ("!external-identity-no-password", false)
        }
    };
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, avatar_url,
            default_display_role, email_login_enabled, status, managed_by_tenant_id, created_by, updated_by)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, 'active', $10, $11, $11)",
    ).bind(user_id).bind(&input.account).bind(&input.email).bind(password_hash).bind(&input.name)
        .bind(&input.nickname).bind(&input.avatar_url).bind(&roles[0].1).bind(password_login)
        .bind(tenant_id).bind(actor).execute(&mut **tx).await?;

    let payload = match &input.credential {
        GovernanceUserCredential::Password { .. } => {
            insert_password_local_identities(
                tx,
                user_id,
                &input.account,
                &input.email,
                None,
                actor,
            )
            .await?;
            serde_json::json!({ "credential_kind": "password_local" })
        }
        GovernanceUserCredential::External(identity) => {
            let identity_id = Uuid::now_v7();
            sqlx::query(
                "insert into user_auth_identities (id, user_id, connection_id, subject_type, subject_value,
                    issuer, realm, profile, verified, metadata, authority_namespace, created_by, updated_by)
                 values ($1, $2, $3, $4, $5, $6, $7, $8, true, $9, $10, $11, $11)",
            ).bind(identity_id).bind(user_id).bind(identity.connection_id()).bind(identity.subject_type())
                .bind(identity.subject_value()).bind(identity.issuer()).bind(identity.realm()).bind(identity.profile())
                .bind(identity.metadata()).bind(identity.authority_namespace()).bind(actor).execute(&mut **tx).await?;
            let mut payload = identity_audit_payload(identity, user_id, None, "bound");
            payload["identity_id"] = serde_json::json!(identity_id);
            payload
        }
    };
    grant_membership(tx, workspace_id, user_id, roles, actor).await?;
    write_audit(
        tx,
        &AuditLogRecord {
            id: Uuid::now_v7(),
            workspace_id: Some(workspace_id),
            actor_user_id: actor,
            target_type: "user".into(),
            target_id: Some(user_id),
            event_code: "workspace.member.onboarded".into(),
            payload: payload.clone(),
            created_at: OffsetDateTime::now_utc(),
        },
        user_id,
        &payload,
    )
    .await?;
    Ok(user_id)
}

async fn provision(
    tx: &mut Tx<'_>,
    input: &ProvisionWorkspaceInput,
) -> Result<ProvisionedWorkspaceOutput> {
    let templates: Vec<_> = input
        .role_templates
        .iter()
        .filter(|role| role.scope_kind == RoleScopeKind::Workspace)
        .cloned()
        .collect();
    if !templates.iter().any(|role| role.code == "admin")
        || templates
            .iter()
            .filter(|role| role.is_default_member_role)
            .count()
            != 1
        || templates
            .iter()
            .any(|role| matches!(role.code.as_str(), "root" | "system"))
    {
        return Err(Error::InvalidInput("workspace_role_templates").into());
    }
    lock_console_permission_catalog(tx).await?;
    let tenant_row = match &input.tenant {
        ProvisionWorkspaceTenant::New { code, name } => sqlx::query(
            "insert into tenants (id, code, name, status, cell_id, is_root, is_hidden, created_by, updated_by)
             values ($1, $2, $3, 'active', 'default', false, false, $4, $4)
             returning id, code, name, status, cell_id, is_root, is_hidden",
        ).bind(Uuid::now_v7()).bind(code).bind(name).bind(input.actor_user_id).fetch_one(&mut **tx).await?,
        ProvisionWorkspaceTenant::Existing { tenant_id } => sqlx::query(
            "select id, code, name, status, cell_id, is_root, is_hidden from tenants where id = $1 for share",
        ).bind(tenant_id).fetch_optional(&mut **tx).await?.ok_or(Error::NotFound("tenant"))?,
    };
    let tenant = map_tenant(&tenant_row)?;
    ensure_tenant_active(tenant.status.as_str())?;
    let workspace = WorkspaceRecord {
        id: Uuid::now_v7(),
        tenant_id: tenant.id,
        name: input.workspace_name.clone(),
        logo_url: input.logo_url.clone(),
        introduction: input.introduction.clone(),
    };
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, logo_url, introduction, created_by, updated_by)
         values ($1, $2, $3, $4, $5, $6, $6)",
    ).bind(workspace.id).bind(workspace.tenant_id).bind(&workspace.name).bind(&workspace.logo_url)
        .bind(&workspace.introduction).bind(input.actor_user_id).execute(&mut **tx).await?;
    sqlx::query(
        "insert into notification_instances (id, workspace_id, type_code, display_name, is_builtin)
         values ($1, $2, 'system', 'System notification', true)",
    )
    .bind(Uuid::now_v7())
    .bind(workspace.id)
    .execute(&mut **tx)
    .await?;
    sqlx::query("insert into workspace_i18n_catalog_states (workspace_id) values ($1) on conflict do nothing")
        .bind(workspace.id).execute(&mut **tx).await?;
    seed_role_templates(tx, workspace.id, templates, true).await?;
    let initial_member_user_id = if let Some(initial) = &input.initial_member {
        let roles = workspace_roles(tx, workspace.id, &initial.role_codes).await?;
        let user_id = match &initial.user {
            ProvisionWorkspaceUser::Existing { user_id } => {
                grant_membership(tx, workspace.id, *user_id, &roles, input.actor_user_id).await?;
                *user_id
            }
            ProvisionWorkspaceUser::New(user) => {
                onboard_user(
                    tx,
                    tenant.id,
                    workspace.id,
                    user,
                    &roles,
                    input.actor_user_id,
                )
                .await?
            }
        };
        Some(user_id)
    } else {
        None
    };
    let is_root_workspace = workspace.id == bootstrap_workspace_id(&mut **tx).await?;
    Ok(ProvisionedWorkspaceOutput {
        is_root_workspace,
        tenant,
        workspace,
        initial_member_user_id,
    })
}

#[async_trait]
impl WorkspaceGovernanceRepository for PgControlPlaneStore {
    async fn provision_workspace(
        &self,
        input: &ProvisionWorkspaceInput,
    ) -> Result<ProvisionedWorkspaceOutput> {
        let mut tx = self.pool().begin().await?;
        let result = provision(&mut tx, input)
            .await
            .map_err(|error| map_unique_conflict(error, "workspace_provision_conflict"))?;
        tx.commit()
            .await
            .map_err(|error| map_unique_conflict(error.into(), "workspace_provision_conflict"))?;
        Ok(result)
    }

    async fn grant_or_activate_workspace_membership(
        &self,
        input: &GrantWorkspaceMembershipInput,
    ) -> Result<GrantWorkspaceMembershipOutput> {
        let mut tx = self.pool().begin().await?;
        lock_active_workspace(&mut tx, input.workspace_id).await?;
        let roles = workspace_roles(&mut tx, input.workspace_id, &input.role_codes).await?;
        let result = grant_membership(
            &mut tx,
            input.workspace_id,
            input.user_id,
            &roles,
            input.actor_user_id,
        )
        .await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn onboard_new_user_to_workspace(
        &self,
        input: &OnboardNewUserInput,
    ) -> Result<OnboardedUserOutput> {
        let mut tx = self.pool().begin().await?;
        let tenant_id = lock_active_workspace(&mut tx, input.workspace_id).await?;
        let roles = workspace_roles(&mut tx, input.workspace_id, &input.role_codes).await?;
        let user_id = onboard_user(
            &mut tx,
            tenant_id,
            input.workspace_id,
            &input.user,
            &roles,
            input.actor_user_id,
        )
        .await
        .map_err(|error| map_unique_conflict(error, "workspace_onboarding_identity_exists"))?;
        tx.commit().await.map_err(|error| {
            map_unique_conflict(error.into(), "workspace_onboarding_identity_exists")
        })?;
        Ok(OnboardedUserOutput { user_id })
    }

    async fn set_workspace_status(
        &self,
        workspace_id: Uuid,
        status: TenantStatus,
        actor_user_id: Option<Uuid>,
    ) -> Result<GovernanceWorkspace> {
        let mut tx = self.pool().begin().await?;
        if workspace_id == bootstrap_workspace_id(&mut *tx).await? {
            return Err(Error::PermissionDenied("root_workspace_immutable").into());
        }
        let (tenant_id, is_root): (Uuid, bool) = sqlx::query_as(
            "select t.id, t.is_root from workspaces w join tenants t on t.id = w.tenant_id
             where w.id = $1 for update of t",
        )
        .bind(workspace_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound("workspace"))?;
        let row = sqlx::query(
            "update workspaces set status = $2, updated_by = $3, updated_at = now()
             where id = $1 and tenant_id = $4 returning name, logo_url, introduction",
        )
        .bind(workspace_id)
        .bind(status.as_str())
        .bind(actor_user_id)
        .bind(tenant_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(Error::NotFound("workspace"))?;
        if !is_root {
            // Holding the tenant lock also serializes provisioning a sibling workspace.
            sqlx::query("update tenants set status = $2, updated_by = $3, updated_at = now()
                where id = $1 and not exists (select 1 from workspaces where tenant_id = $1 and id <> $4)")
                .bind(tenant_id).bind(status.as_str()).bind(actor_user_id).bind(workspace_id)
                .execute(&mut *tx).await?;
        }
        let record = WorkspaceRecord {
            id: workspace_id,
            tenant_id,
            name: row.try_get("name")?,
            logo_url: row.try_get("logo_url")?,
            introduction: row.try_get("introduction")?,
        };
        tx.commit().await?;
        Ok(GovernanceWorkspace {
            record,
            status,
            is_root_workspace: false,
        })
    }

    async fn assign_workspace_admins(
        &self,
        workspace_ids: &[Uuid],
        user: &ProvisionWorkspaceUser,
        actor_user_id: Option<Uuid>,
    ) -> Result<GovernanceUserSummary> {
        if workspace_ids.is_empty() {
            return Err(Error::InvalidInput("workspace_ids").into());
        }
        let mut tx = self.pool().begin().await?;
        let bootstrap = bootstrap_workspace_id(&mut *tx).await?;
        if workspace_ids.contains(&bootstrap) {
            return Err(Error::PermissionDenied("root_admin_immutable").into());
        }
        let mut targets = Vec::new();
        for workspace_id in workspace_ids
            .iter()
            .copied()
            .collect::<std::collections::BTreeSet<_>>()
        {
            let tenant_id = lock_active_workspace(&mut tx, workspace_id).await?;
            let roles = workspace_roles(&mut tx, workspace_id, &["admin".into()]).await?;
            targets.push((workspace_id, tenant_id, roles));
        }
        let user_id = match user {
            ProvisionWorkspaceUser::Existing { user_id } => *user_id,
            ProvisionWorkspaceUser::New(user) => {
                let (workspace_id, tenant_id, roles) = &targets[0];
                onboard_user(
                    &mut tx,
                    *tenant_id,
                    *workspace_id,
                    user,
                    roles,
                    actor_user_id,
                )
                .await
                .map_err(|error| map_unique_conflict(error, "workspace_onboarding_conflict"))?
            }
        };
        for (workspace_id, _, roles) in targets {
            grant_membership(&mut tx, workspace_id, user_id, &roles, actor_user_id).await?;
        }
        let row = sqlx::query("select id, account, email, name, status, managed_by_tenant_id from users where id = $1")
            .bind(user_id).fetch_one(&mut *tx).await?;
        let user = governance_user_from_row(row)?;
        tx.commit().await?;
        Ok(user)
    }

    async fn revoke_workspace_admin(
        &self,
        workspace_id: Uuid,
        user_id: Uuid,
        actor_user_id: Option<Uuid>,
    ) -> Result<bool> {
        let mut tx = self.pool().begin().await?;
        sqlx::query("select id from workspaces where id = $1 for share")
            .bind(workspace_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound("workspace"))?;
        let admin_roles = workspace_roles(&mut tx, workspace_id, &["admin".into()]).await?;
        sqlx::query("select id from users where id = $1 for update")
            .bind(user_id)
            .fetch_optional(&mut *tx)
            .await?
            .ok_or(Error::NotFound("user"))?;
        if workspace_id == bootstrap_workspace_id(&mut *tx).await? {
            return Err(Error::PermissionDenied("root_admin_immutable").into());
        }
        let membership: Option<String> = sqlx::query_scalar(
            "select status from workspace_memberships where workspace_id = $1 and user_id = $2 for update",
        ).bind(workspace_id).bind(user_id).fetch_optional(&mut *tx).await?;
        let revoked = sqlx::query(
            "delete from user_role_bindings where user_id = $1 and role_id = $2 and scope_id = $3",
        )
        .bind(user_id)
        .bind(admin_roles[0].0)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?
        .rows_affected()
            > 0;
        if membership.as_deref() == Some("active") {
            let has_roles: bool = sqlx::query_scalar(
                "select exists (select 1 from user_role_bindings b join roles r on r.id = b.role_id
                 where b.user_id = $1 and b.scope_id = $2 and r.workspace_id = $2 and r.scope_kind = 'workspace')",
            ).bind(user_id).bind(workspace_id).fetch_one(&mut *tx).await?;
            if !has_roles {
                let default_roles = workspace_roles(&mut tx, workspace_id, &[]).await?;
                grant_membership(
                    &mut tx,
                    workspace_id,
                    user_id,
                    &default_roles,
                    actor_user_id,
                )
                .await?;
            }
        }
        if revoked {
            sqlx::query("update users set session_version = session_version + 1 where id = $1")
                .bind(user_id)
                .execute(&mut *tx)
                .await?;
        }
        tx.commit().await?;
        Ok(revoked)
    }

    async fn set_tenant_status(
        &self,
        tenant_id: Uuid,
        status: TenantStatus,
        actor_user_id: Option<Uuid>,
    ) -> Result<TenantRecord> {
        let mut tx = self.pool().begin().await?;
        let is_root: bool =
            sqlx::query_scalar("select is_root from tenants where id = $1 for update")
                .bind(tenant_id)
                .fetch_optional(&mut *tx)
                .await?
                .ok_or(Error::NotFound("tenant"))?;
        if is_root {
            return Err(Error::PermissionDenied("root_tenant_immutable").into());
        }
        let row = sqlx::query(
            "update tenants set status = $2, updated_by = $3, updated_at = now() where id = $1
             returning id, code, name, status, cell_id, is_root, is_hidden",
        )
        .bind(tenant_id)
        .bind(status.as_str())
        .bind(actor_user_id)
        .fetch_one(&mut *tx)
        .await?;
        let tenant = map_tenant(&row)?;
        tx.commit().await?;
        Ok(tenant)
    }

    async fn list_tenants_and_workspaces(&self) -> Result<Vec<TenantWorkspaceGovernanceSummary>> {
        let bootstrap = bootstrap_workspace_id(self.pool()).await?;
        let rows = sqlx::query(
            r#"
            select t.id, t.code, t.name, t.is_root, t.is_hidden, t.status, t.cell_id,
                   w.id as workspace_id, w.name as workspace_name, w.logo_url, w.introduction, w.status as workspace_status,
                   (select count(*) from workspace_memberships wm join users u on u.id = wm.user_id
                     where wm.workspace_id = w.id and wm.status = 'active' and u.status = 'active') as active_member_count,
                   (select count(*) from applications a where a.workspace_id = w.id) as workspace_application_count,
                   coalesce((select jsonb_agg(jsonb_build_object(
                       'user_id', u.id, 'account', u.account, 'email', u.email, 'name', u.name,
                       'status', u.status, 'managed_by_tenant_id', u.managed_by_tenant_id
                   ) order by (w.id = $1 and lower(u.account) = 'root') desc, u.account, u.id)
                   from workspace_memberships wm join users u on u.id = wm.user_id
                   where wm.workspace_id = w.id and wm.status = 'active' and u.status = 'active'
                     and exists (select 1 from user_role_bindings b join roles r on r.id = b.role_id
                                 where b.user_id = u.id and (
                                     case
                                         when w.id = $1 then (r.scope_kind = 'system' and r.code = 'root')
                                         else (r.workspace_id = w.id and r.scope_kind = 'workspace' and r.code = 'admin')
                                     end
                                 ))
                   ), '[]'::jsonb) as admins
            from tenants t left join workspaces w on w.tenant_id = t.id
            order by t.code, t.id, lower(w.name), w.id
            "#,
        ).bind(bootstrap).fetch_all(self.pool()).await?;
        let mut summaries: Vec<TenantWorkspaceGovernanceSummary> = Vec::new();
        for row in rows {
            let tenant_id = row.try_get("id")?;
            if summaries
                .last()
                .is_none_or(|last| last.tenant.id != tenant_id)
            {
                summaries.push(TenantWorkspaceGovernanceSummary {
                    tenant: map_tenant(&row)?,
                    workspaces: Vec::new(),
                });
            }
            if let Some(workspace_id) = row.try_get::<Option<Uuid>, _>("workspace_id")? {
                summaries
                    .last_mut()
                    .expect("tenant summary exists")
                    .workspaces
                    .push(WorkspaceGovernanceSummary {
                        workspace: GovernanceWorkspace {
                            is_root_workspace: workspace_id == bootstrap,
                            status: TenantStatus::from_db(row.try_get("workspace_status")?)
                                .ok_or_else(|| anyhow!("invalid stored workspace status"))?,
                            record: WorkspaceRecord {
                            id: workspace_id,
                            tenant_id,
                            name: row.try_get("workspace_name")?,
                            logo_url: row.try_get("logo_url")?,
                            introduction: row.try_get("introduction")?,
                            },
                        },
                        active_member_count: row.try_get("active_member_count")?,
                        workspace_application_count: row.try_get("workspace_application_count")?,
                        admins: row
                            .try_get::<Json<Vec<GovernanceUserSummary>>, _>("admins")?
                            .0,
                    });
            }
        }
        Ok(summaries)
    }

    async fn inspect_workspace_quota_counts(
        &self,
        tenant_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<WorkspaceQuotaCounts> {
        let (active_member_count, tenant_workspace_count, workspace_application_count) = sqlx::query_as(
            "select (select count(*) from workspace_memberships wm join users u on u.id = wm.user_id
                      where wm.workspace_id = w.id and wm.status = 'active' and u.status = 'active'),
                    (select count(*) from workspaces sibling where sibling.tenant_id = w.tenant_id),
                    (select count(*) from applications a where a.workspace_id = w.id)
             from workspaces w where w.tenant_id = $1 and w.id = $2",
        ).bind(tenant_id).bind(workspace_id).fetch_optional(self.pool()).await?.ok_or(Error::NotFound("workspace"))?;
        Ok(WorkspaceQuotaCounts {
            active_member_count,
            tenant_workspace_count,
            workspace_application_count,
        })
    }

    async fn lookup_governance_user(
        &self,
        query: &GovernanceUserLookup,
    ) -> Result<Option<GovernanceUserSummary>> {
        let (id, account, email) = match query {
            GovernanceUserLookup::UserId(id) => (Some(*id), None, None),
            GovernanceUserLookup::Account(account) => (None, Some(account.as_str()), None),
            GovernanceUserLookup::Email(email) => (None, None, Some(email.as_str())),
        };
        let row = sqlx::query(
            "select id, account, email, name, status, managed_by_tenant_id from users
             where id = $1 or account = $2 or email = $3",
        )
        .bind(id)
        .bind(account)
        .bind(email)
        .fetch_optional(self.pool())
        .await?;
        row.map(governance_user_from_row).transpose()
    }

    async fn search_governance_users(
        &self,
        query: Option<&str>,
        limit: u32,
    ) -> Result<Vec<GovernanceUserSummary>> {
        let capped_limit = i64::from(limit.clamp(1, 50));
        let pattern = query
            .map(str::trim)
            .filter(|q| !q.is_empty())
            .map(|q| {
                let escaped = q
                    .replace('\\', r"\\")
                    .replace('%', r"\%")
                    .replace('_', r"\_");
                format!("%{escaped}%")
            });
        let rows = sqlx::query(
            r#"
            select id, account, email, name, status, managed_by_tenant_id
            from users
            where ($1::text is null
                or account ilike $1 escape '\'
                or email ilike $1 escape '\'
                or name ilike $1 escape '\'
                or id::text ilike $1 escape '\')
            order by (status = 'active') desc, lower(account) asc, id asc
            limit $2
            "#,
        )
        .bind(pattern)
        .bind(capped_limit)
        .fetch_all(self.pool())
        .await?;
        rows.into_iter().map(governance_user_from_row).collect()
    }
}

fn governance_user_from_row(row: PgRow) -> Result<GovernanceUserSummary> {
            let status = match row.try_get::<&str, _>("status")? {
                "active" => UserStatus::Active,
                "disabled" => UserStatus::Disabled,
                _ => return Err(anyhow!("invalid stored user status")),
            };
            Ok(GovernanceUserSummary {
                user_id: row.try_get("id")?,
                account: row.try_get("account")?,
                email: row.try_get("email")?,
                name: row.try_get("name")?,
                status,
                managed_by_tenant_id: row.try_get("managed_by_tenant_id")?,
            })
}
