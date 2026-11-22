use std::collections::BTreeSet;

#[path = "external_registration.rs"]
mod external_registration;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{
        AuthRepository, CreateMemberInput, CreateSelfRegisteredMemberInput, MemberRepository,
        SelfRegistrationRepository, UpdateMemberInput,
    },
    ControlPlaneContractError as ControlPlaneError,
};
use domain::{ActorContext, AuditLogRecord, WorkspaceMembershipStatus};
use sqlx::{Postgres, Transaction};
use uuid::Uuid;

use crate::{
    auth_repository::identity_binding::{
        insert_password_identities_for_connection, insert_password_local_identities,
        replace_password_local_contact_identities,
    },
    auth_repository::{map_user_row, map_user_rows},
    repositories::{
        is_root_user, primary_workspace_id, tenant_id_for_workspace, workspace_id_for_user,
        PgControlPlaneStore,
    },
    workspace_governance_repository::map_unique_conflict,
};

#[async_trait]
impl SelfRegistrationRepository for PgControlPlaneStore {
    async fn create_self_registered_member(
        &self,
        input: &CreateSelfRegisteredMemberInput,
    ) -> Result<domain::UserRecord> {
        let workspace_id = primary_workspace_id(self.pool()).await?;
        let default_role: (Uuid, String, Uuid) = sqlx::query_as(
            r#"
            select id, code, scope_id
            from roles
            where scope_kind = 'workspace'
              and workspace_id = $1
              and is_default_member_role = true
            limit 1
            "#,
        )
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("default_member_role"))?;
        let user_id = Uuid::now_v7();
        let mut tx = self.pool().begin().await?;

        let result = async {
            sqlx::query(
                r#"
                insert into users (
                    id, account, email, phone, password_hash, name, nickname, avatar_url,
                    introduction, default_display_role, email_login_enabled,
                    phone_login_enabled, status, session_version, created_by, updated_by
                )
                values ($1, $2, $3, null, $4, $2, $2, null, '', $5, true, false,
                        'active', 1, null, null)
                "#,
            )
            .bind(user_id)
            .bind(&input.account)
            .bind(&input.email)
            .bind(&input.password_hash)
            .bind(&default_role.1)
            .execute(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                insert into workspace_memberships (
                    id, workspace_id, user_id, introduction, created_by, updated_by
                )
                values ($1, $2, $3, '', null, null)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(workspace_id)
            .bind(user_id)
            .execute(&mut *tx)
            .await?;

            insert_password_identities_for_connection(
                &mut tx,
                input.connection_id,
                user_id,
                &input.account,
                &input.email,
                None,
                None,
            )
            .await?;

            sqlx::query(
                r#"
                insert into user_role_bindings (
                    id, user_id, role_id, scope_id, created_by, updated_by
                )
                values ($1, $2, $3, $4, null, null)
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(user_id)
            .bind(default_role.0)
            .bind(default_role.2)
            .execute(&mut *tx)
            .await?;

            Ok::<(), anyhow::Error>(())
        }
        .await;
        if let Err(error) = result {
            if error
                .downcast_ref::<sqlx::Error>()
                .and_then(|sqlx_error| sqlx_error.as_database_error())
                .and_then(|database_error| database_error.code())
                .as_deref()
                == Some("23505")
            {
                return Err(
                    ControlPlaneError::Conflict("self_registration_identity_exists").into(),
                );
            }
            return Err(error);
        }
        tx.commit().await?;

        self.find_user_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("self-registered member missing after creation"))
    }
}

#[async_trait]
impl MemberRepository for PgControlPlaneStore {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        if let Some(actor) = self.request_actor_for_user(actor_user_id)? {
            return Ok(actor);
        }
        let workspace_id = workspace_id_for_user(self.pool(), actor_user_id).await?;
        let tenant_id = tenant_id_for_workspace(self.pool(), workspace_id).await?;
        AuthRepository::load_actor_context(self, actor_user_id, tenant_id, workspace_id, None).await
    }

    async fn create_member_with_default_role(
        &self,
        input: &CreateMemberInput,
    ) -> Result<domain::UserRecord> {
        // This legacy entry point already carries the requested workspace explicitly.
        let tenant_id = tenant_id_for_workspace(self.pool(), input.workspace_id).await?;
        let actor = self
            .load_actor_context(input.actor_user_id, tenant_id, input.workspace_id, None)
            .await?;
        self.create_member_with_default_role_in_scope(&actor, input)
            .await
    }

    async fn update_member_profile(&self, input: &UpdateMemberInput) -> Result<domain::UserRecord> {
        let actor =
            MemberRepository::load_actor_context_for_user(self, input.actor_user_id).await?;
        self.update_member_profile_in_scope(&actor, input).await
    }

    async fn disable_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> Result<()> {
        let actor = MemberRepository::load_actor_context_for_user(self, actor_user_id).await?;
        self.disable_member_in_scope(&actor, target_user_id).await
    }

    async fn enable_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> Result<()> {
        let actor = MemberRepository::load_actor_context_for_user(self, actor_user_id).await?;
        self.enable_member_in_scope(&actor, target_user_id).await
    }

    async fn delete_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> Result<()> {
        let actor = MemberRepository::load_actor_context_for_user(self, actor_user_id).await?;
        self.delete_member_in_scope(&actor, target_user_id).await
    }

    async fn reset_member_password(
        &self,
        actor_user_id: Uuid,
        target_user_id: Uuid,
        password_hash: &str,
    ) -> Result<()> {
        let actor = MemberRepository::load_actor_context_for_user(self, actor_user_id).await?;
        self.reset_member_password_in_scope(&actor, target_user_id, password_hash)
            .await
    }

    async fn create_member_with_default_role_in_scope(
        &self,
        actor: &ActorContext,
        input: &CreateMemberInput,
    ) -> Result<domain::UserRecord> {
        if actor.user_id != input.actor_user_id || actor.current_workspace_id != input.workspace_id
        {
            return Err(ControlPlaneError::PermissionDenied("workspace_access_denied").into());
        }
        self.verify_workspace_access(
            actor.user_id,
            actor.tenant_id,
            actor.current_workspace_id,
            actor.is_root,
        )
        .await?;
        let mut tx = self.pool().begin().await?;
        let identity_exists: bool = sqlx::query_scalar(
            "select exists(select 1 from users where account = $1 or email = $2 or phone = $3)",
        )
        .bind(&input.account)
        .bind(&input.email)
        .bind(&input.phone)
        .fetch_one(&mut *tx)
        .await?;
        if identity_exists {
            return Err(ControlPlaneError::Conflict("member_account_exists_use_invitation").into());
        }
        let default_role: (Uuid, String, Uuid) = sqlx::query_as(
            r#"
            select id, code, scope_id
            from roles
            where scope_kind = 'workspace'
              and workspace_id = $1
              and is_default_member_role = true
            limit 1
            "#,
        )
        .bind(input.workspace_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::NotFound("default_member_role"))?;
        let user_id = Uuid::now_v7();
        sqlx::query(
            r#"
            insert into users (
                id, account, email, phone, password_hash, name, nickname, avatar_url, introduction,
                default_display_role, email_login_enabled, phone_login_enabled, status, session_version,
                created_by, updated_by, managed_by_tenant_id
            )
            values (
                $1, $2, $3, $4, $5, $6, $7, null, $8,
                $9, $10, $11, 'active', 1, $12, $12, $13
            )
            "#,
        )
        .bind(user_id)
        .bind(&input.account)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(&input.password_hash)
        .bind(&input.name)
        .bind(&input.nickname)
        .bind(&input.introduction)
        .bind(&default_role.1)
        .bind(input.email_login_enabled)
        .bind(input.phone_login_enabled)
        .bind(input.actor_user_id)
        .bind(actor.tenant_id)
        .execute(&mut *tx)
        .await
        .map_err(|error| map_unique_conflict(error.into(), "member_account_exists_use_invitation"))?;

        sqlx::query(
            r#"
            insert into workspace_memberships (id, workspace_id, user_id, introduction, created_by, updated_by, status)
            values ($1, $2, $3, $4, $5, $5, 'active')
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(input.workspace_id)
        .bind(user_id)
        .bind(&input.introduction)
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;

        insert_password_local_identities(
            &mut tx,
            user_id,
            &input.account,
            &input.email,
            input.phone.as_deref(),
            Some(input.actor_user_id),
        )
        .await
        .map_err(|error| map_unique_conflict(error, "member_account_exists_use_invitation"))?;

        sqlx::query(
            r#"
            insert into user_role_bindings (id, user_id, role_id, scope_id, created_by, updated_by)
            values ($1, $2, $3, $4, $5, $5)
            on conflict (user_id, role_id) do nothing
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .bind(default_role.0)
        .bind(default_role.2)
        .bind(input.actor_user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.find_user_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("member missing after creation"))
    }

    async fn update_member_profile_in_scope(
        &self,
        actor: &ActorContext,
        input: &UpdateMemberInput,
    ) -> Result<domain::UserRecord> {
        if actor.user_id != input.actor_user_id {
            return Err(ControlPlaneError::PermissionDenied("workspace_access_denied").into());
        }
        let mut tx = self.pool().begin().await?;
        let target = lock_member(&mut tx, actor, input.user_id).await?;
        target.ensure_identity_editable(actor)?;
        let row = sqlx::query(
            r#"
            update users
            set name = $2,
                nickname = $3,
                email = $4,
                phone = $5,
                introduction = $6,
                updated_by = $7,
                updated_at = now()
            where id = $1
            returning id, account, email, phone, password_hash, name, nickname, avatar_url,
                      introduction, preferred_locale, meta, default_display_role, email_login_enabled, phone_login_enabled,
                      status, session_version
            "#,
        )
        .bind(input.user_id)
        .bind(&input.name)
        .bind(&input.nickname)
        .bind(&input.email)
        .bind(&input.phone)
        .bind(&input.introduction)
        .bind(input.actor_user_id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(ControlPlaneError::NotFound("user"))?;

        replace_password_local_contact_identities(
            &mut tx,
            input.user_id,
            &input.email,
            input.phone.as_deref(),
            input.actor_user_id,
        )
        .await?;
        tx.commit().await?;

        map_user_row(self.pool(), row).await
    }

    async fn disable_member_in_scope(
        &self,
        actor: &ActorContext,
        target_user_id: Uuid,
    ) -> Result<()> {
        set_membership_status(
            self,
            actor,
            target_user_id,
            WorkspaceMembershipStatus::Disabled,
        )
        .await
    }

    async fn enable_member_in_scope(
        &self,
        actor: &ActorContext,
        target_user_id: Uuid,
    ) -> Result<()> {
        set_membership_status(
            self,
            actor,
            target_user_id,
            WorkspaceMembershipStatus::Active,
        )
        .await
    }

    async fn delete_member_in_scope(
        &self,
        actor: &ActorContext,
        target_user_id: Uuid,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let target = lock_member(&mut tx, actor, target_user_id).await?;
        target.ensure_not_root()?;
        if actor.user_id == target_user_id {
            return Err(ControlPlaneError::PermissionDenied("member_self_delete_forbidden").into());
        }

        if target.has_other_workspace
            || (!actor.is_root && target.is_managed_by_other_tenant(actor))
        {
            let workspace_id = actor.current_workspace_id;
            for query in [
                "delete from user_department_bindings where user_id = $1 and scope_id = $2",
                "delete from user_role_bindings where user_id = $1 and scope_id = $2",
                "update api_keys set enabled = false, updated_at = now() where creator_user_id = $1
                   and scope_id = $2 and scope_kind = 'workspace' and key_kind = 'user_api_key' and enabled",
                "delete from workspace_memberships where user_id = $1 and workspace_id = $2",
            ] {
                sqlx::query(query).bind(target_user_id).bind(workspace_id).execute(&mut *tx).await?;
            }
        } else {
            sqlx::query("delete from users where id = $1")
                .bind(target_user_id)
                .execute(&mut *tx)
                .await
                .map_err(|err| {
                    if err.as_database_error().and_then(|db| db.code()).as_deref() == Some("23503")
                    {
                        anyhow::Error::from(ControlPlaneError::Conflict(
                            "member_has_referenced_resources",
                        ))
                    } else {
                        anyhow::Error::from(err)
                    }
                })?;
        }
        tx.commit().await?;
        Ok(())
    }

    async fn reset_member_password_in_scope(
        &self,
        actor: &ActorContext,
        target_user_id: Uuid,
        password_hash: &str,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let target = lock_member(&mut tx, actor, target_user_id).await?;
        target.ensure_not_root()?;
        target.ensure_identity_editable(actor)?;
        sqlx::query(
            "update users set password_hash = $2, session_version = session_version + 1,
                updated_by = $3, updated_at = now() where id = $1",
        )
        .bind(target_user_id)
        .bind(password_hash)
        .bind(actor.user_id)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;
        Ok(())
    }

    async fn replace_member_roles(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        target_user_id: Uuid,
        role_codes: &[String],
    ) -> Result<()> {
        let target_is_in_workspace: bool = sqlx::query_scalar(
            "select exists(select 1 from workspace_memberships where workspace_id = $1 and user_id = $2)",
        )
        .bind(workspace_id)
        .bind(target_user_id)
        .fetch_one(self.pool())
        .await?;
        if !target_is_in_workspace {
            return Err(ControlPlaneError::InvalidInput("member_scope").into());
        }

        let is_root_target = is_root_user(self.pool(), target_user_id).await?;
        let mut normalized_codes = role_codes
            .iter()
            .map(|code| code.trim())
            .filter(|code| !code.is_empty())
            .map(str::to_string)
            .collect::<BTreeSet<_>>()
            .into_iter()
            .collect::<Vec<_>>();
        if is_root_target {
            if !normalized_codes.iter().any(|code| code == "root") {
                return Err(ControlPlaneError::PermissionDenied("root_user_immutable").into());
            }
            normalized_codes.retain(|code| code != "root");
        }

        let mut role_ids = Vec::new();
        for role_code in &normalized_codes {
            let role_id: Uuid = sqlx::query_scalar(
                "select id from roles where scope_kind = 'workspace' and workspace_id = $1 and code = $2",
            )
            .bind(workspace_id)
            .bind(role_code)
            .fetch_optional(self.pool())
            .await?
            .ok_or(ControlPlaneError::InvalidInput("role_code"))?;
            role_ids.push(role_id);
        }

        let mut tx = self.pool().begin().await?;
        sqlx::query(
            r#"
            delete from user_role_bindings urb
            using roles r
            where urb.role_id = r.id
              and urb.user_id = $1
              and r.scope_kind = 'workspace'
              and r.workspace_id = $2
            "#,
        )
        .bind(target_user_id)
        .bind(workspace_id)
        .execute(&mut *tx)
        .await?;

        for role_id in role_ids {
            sqlx::query(
                r#"
                insert into user_role_bindings (id, user_id, role_id, scope_id, created_by, updated_by)
                select $1, $2, roles.id, roles.scope_id, $4, $4
                from roles
                where roles.id = $3
                on conflict (user_id, role_id) do nothing
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(target_user_id)
            .bind(role_id)
            .bind(actor_user_id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    async fn list_members(
        &self,
        workspace_id: Uuid,
        department_id: Option<Uuid>,
    ) -> Result<Vec<domain::UserRecord>> {
        if let Some(id) = department_id {
            let exists: bool = sqlx::query_scalar(
                "select exists(select 1 from departments where scope_id=$1 and id=$2)",
            )
            .bind(workspace_id)
            .bind(id)
            .fetch_one(self.pool())
            .await?;
            if !exists {
                return Err(ControlPlaneError::NotFound("department").into());
            }
        }
        let rows = sqlx::query(
            r#"
            select
              u.id, u.account, u.email, u.phone, u.password_hash, u.name, u.nickname, u.avatar_url,
              u.introduction, u.preferred_locale, u.meta, u.default_display_role, u.email_login_enabled, u.phone_login_enabled,
              case when u.status = 'disabled' or tm.status = 'disabled' then 'disabled' else 'active' end as status,
              u.session_version
            from workspace_memberships tm
            join users u on u.id = tm.user_id
            where tm.workspace_id = $1
              and ($2::uuid is null or exists (
                select 1 from departments root
                join departments child on child.scope_id=root.scope_id
                  and child.tree_partition_id=root.tree_partition_id
                  and ARRAY[child.tree_path] <@ root.tree_path
                join user_department_bindings b on b.scope_id=child.scope_id and b.department_id=child.id
                where root.scope_id=$1 and root.id=$2 and b.user_id=u.id
              ))
            order by tm.created_at asc, u.created_at asc
            "#,
        )
        .bind(workspace_id)
        .bind(department_id)
        .fetch_all(self.pool())
        .await?;
        map_user_rows(self.pool(), rows).await
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }
}

struct LockedMember {
    is_root: bool,
    managed_by_tenant_id: Option<Uuid>,
    has_other_workspace: bool,
    has_other_tenant: bool,
}

impl LockedMember {
    fn ensure_not_root(&self) -> Result<()> {
        if self.is_root {
            return Err(ControlPlaneError::PermissionDenied("root_user_immutable").into());
        }
        Ok(())
    }

    fn is_managed_by_other_tenant(&self, actor: &ActorContext) -> bool {
        self.managed_by_tenant_id
            .is_some_and(|tenant_id| tenant_id != actor.tenant_id)
    }

    fn ensure_identity_editable(&self, actor: &ActorContext) -> Result<()> {
        if !actor.is_root && (self.has_other_tenant || self.is_managed_by_other_tenant(actor)) {
            return Err(
                ControlPlaneError::PermissionDenied("member_identity_externally_owned").into(),
            );
        }
        Ok(())
    }
}

async fn lock_member(
    tx: &mut Transaction<'_, Postgres>,
    actor: &ActorContext,
    target_user_id: Uuid,
) -> Result<LockedMember> {
    // Lock the global identity before reading memberships, matching the grant
    // path so ownership checks and single-space deletion cannot race a grant.
    let managed_by_tenant_id: Option<Uuid> = sqlx::query_scalar(
        "select u.managed_by_tenant_id from users u
         where u.id = $1 and exists (select 1 from workspace_memberships wm where wm.user_id = u.id and wm.workspace_id = $2)
         for update of u",
    ).bind(target_user_id).bind(actor.current_workspace_id).fetch_optional(&mut **tx).await?
        .ok_or(ControlPlaneError::NotFound("user"))?;
    let (is_root, has_membership, has_other_workspace, has_other_tenant): (bool, bool, bool, bool) = sqlx::query_as(
        "select exists(select 1 from user_role_bindings b join roles r on r.id = b.role_id
                        where b.user_id = $1 and r.scope_kind = 'system' and r.code = 'root'),
                count(*) filter (where w.id = $2 and w.tenant_id = $3) > 0,
                count(*) filter (where w.id <> $2) > 0,
                count(*) filter (where w.tenant_id <> $3) > 0
         from workspace_memberships wm join workspaces w on w.id = wm.workspace_id where wm.user_id = $1",
    ).bind(target_user_id).bind(actor.current_workspace_id).bind(actor.tenant_id).fetch_one(&mut **tx).await?;
    if !has_membership {
        return Err(ControlPlaneError::NotFound("user").into());
    }
    Ok(LockedMember {
        is_root,
        managed_by_tenant_id,
        has_other_workspace,
        has_other_tenant,
    })
}

async fn set_membership_status(
    store: &PgControlPlaneStore,
    actor: &ActorContext,
    target_user_id: Uuid,
    status: WorkspaceMembershipStatus,
) -> Result<()> {
    let mut tx = store.pool().begin().await?;
    let target = lock_member(&mut tx, actor, target_user_id).await?;
    target.ensure_not_root()?;
    if status == WorkspaceMembershipStatus::Disabled && actor.user_id == target_user_id {
        return Err(ControlPlaneError::PermissionDenied("member_self_disable_forbidden").into());
    }
    sqlx::query(
        "update workspace_memberships set status = $3, updated_by = $4, updated_at = now()
         where workspace_id = $1 and user_id = $2 and status <> $3",
    )
    .bind(actor.current_workspace_id)
    .bind(target_user_id)
    .bind(status.as_str())
    .bind(actor.user_id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}
