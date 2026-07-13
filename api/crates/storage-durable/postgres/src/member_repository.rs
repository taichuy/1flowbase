use std::collections::BTreeSet;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{AuthRepository, CreateMemberInput, MemberRepository, UpdateMemberInput},
};
use domain::{ActorContext, AuditLogRecord};
use uuid::Uuid;

use crate::{
    auth_repository::identity_binding::{
        insert_password_local_identities, replace_password_local_contact_identities,
    },
    auth_repository::map_user_row,
    repositories::{
        is_root_user, tenant_id_for_workspace, workspace_id_for_user, PgControlPlaneStore,
    },
};

#[async_trait]
impl MemberRepository for PgControlPlaneStore {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        let workspace_id = workspace_id_for_user(self.pool(), actor_user_id).await?;
        let tenant_id = tenant_id_for_workspace(self.pool(), workspace_id).await?;
        AuthRepository::load_actor_context(self, actor_user_id, tenant_id, workspace_id, None).await
    }

    async fn create_member_with_default_role(
        &self,
        input: &CreateMemberInput,
    ) -> Result<domain::UserRecord> {
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
        .fetch_optional(self.pool())
        .await?
        .ok_or(ControlPlaneError::NotFound("default_member_role"))?;
        let user_id = Uuid::now_v7();
        let mut tx = self.pool().begin().await?;

        sqlx::query(
            r#"
            insert into users (
                id, account, email, phone, password_hash, name, nickname, avatar_url, introduction,
                default_display_role, email_login_enabled, phone_login_enabled, status, session_version,
                created_by, updated_by
            )
            values (
                $1, $2, $3, $4, $5, $6, $7, null, $8,
                $9, $10, $11, 'active', 1, $12, $12
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
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            insert into workspace_memberships (id, workspace_id, user_id, introduction, created_by, updated_by)
            values ($1, $2, $3, $4, $5, $5)
            on conflict (workspace_id, user_id) do nothing
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
        .await?;

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

    async fn update_member_profile(&self, input: &UpdateMemberInput) -> Result<domain::UserRecord> {
        let mut tx = self.pool().begin().await?;
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

    async fn disable_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> Result<()> {
        if is_root_user(self.pool(), target_user_id).await? {
            return Err(ControlPlaneError::PermissionDenied("root_user_immutable").into());
        }

        let result = sqlx::query(
            r#"
            update users
            set status = 'disabled',
                session_version = session_version + 1,
                updated_by = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(target_user_id)
        .bind(actor_user_id)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("user").into());
        }

        Ok(())
    }

    async fn enable_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> Result<()> {
        let result = sqlx::query(
            r#"
            update users
            set status = 'active',
                session_version = session_version + 1,
                updated_by = $2,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(target_user_id)
        .bind(actor_user_id)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("user").into());
        }

        Ok(())
    }

    async fn delete_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> Result<()> {
        if is_root_user(self.pool(), target_user_id).await? {
            return Err(ControlPlaneError::PermissionDenied("root_user_immutable").into());
        }
        if actor_user_id == target_user_id {
            return Err(ControlPlaneError::PermissionDenied("member_self_delete_forbidden").into());
        }

        let result = sqlx::query("delete from users where id = $1")
            .bind(target_user_id)
            .execute(self.pool())
            .await
            .map_err(|err| {
                if matches!(
                    err.as_database_error().and_then(|db| db.code()).as_deref(),
                    Some("23503")
                ) {
                    anyhow::Error::from(ControlPlaneError::Conflict(
                        "member_has_referenced_resources",
                    ))
                } else {
                    anyhow::Error::from(err)
                }
            })?;

        if result.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("user").into());
        }

        Ok(())
    }

    async fn reset_member_password(
        &self,
        actor_user_id: Uuid,
        target_user_id: Uuid,
        password_hash: &str,
    ) -> Result<()> {
        if is_root_user(self.pool(), target_user_id).await? {
            return Err(ControlPlaneError::PermissionDenied("root_user_immutable").into());
        }

        let result = sqlx::query(
            r#"
            update users
            set password_hash = $2,
                session_version = session_version + 1,
                updated_by = $3,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(target_user_id)
        .bind(password_hash)
        .bind(actor_user_id)
        .execute(self.pool())
        .await?;

        if result.rows_affected() == 0 {
            return Err(ControlPlaneError::NotFound("user").into());
        }

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

    async fn list_members(&self, workspace_id: Uuid) -> Result<Vec<domain::UserRecord>> {
        let rows = sqlx::query(
            r#"
            select
              u.id, u.account, u.email, u.phone, u.password_hash, u.name, u.nickname, u.avatar_url,
              u.introduction, u.preferred_locale, u.meta, u.default_display_role, u.email_login_enabled, u.phone_login_enabled,
              u.status, u.session_version
            from workspace_memberships tm
            join users u on u.id = tm.user_id
            where tm.workspace_id = $1
            order by tm.created_at asc, u.created_at asc
            "#,
        )
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        let mut members = Vec::with_capacity(rows.len());
        for row in rows {
            members.push(map_user_row(self.pool(), row).await?);
        }

        Ok(members)
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }
}
