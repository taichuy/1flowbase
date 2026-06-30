use anyhow::Result;
use control_plane::errors::ControlPlaneError;
use control_plane::ports::{
    ApplicationRepository, AuthRepository, BootstrapRepository, CreateApplicationInput,
    CreateMemberInput, FlowRepository, MemberRepository, UpdateProfileInput, WorkspaceRepository,
};
use domain::{
    ActorContext, ApplicationRecord, AuditLogRecord, AuthenticatorRecord, FlowChangeKind,
    FlowEditorState, PermissionDefinition, RoleScopeKind, TenantRecord, UserRecord,
    WorkspaceRecord,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use crate::mappers::role_mapper::StoredRoleRow;

#[derive(Clone)]
pub struct PgControlPlaneStore {
    pool: PgPool,
}

impl PgControlPlaneStore {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }

    pub async fn upsert_authenticator(&self, authenticator: &AuthenticatorRecord) -> Result<()> {
        BootstrapRepository::upsert_authenticator(self, authenticator).await
    }

    pub async fn update_authenticator_config(
        &self,
        authenticator: &AuthenticatorRecord,
    ) -> Result<()> {
        sqlx::query(
            r#"
            update authenticators
            set title = $2,
                enabled = $3,
                options = $4,
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(authenticator.id)
        .bind(&authenticator.title)
        .bind(authenticator.enabled)
        .bind(&authenticator.options)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn create_authenticator(&self, authenticator: &AuthenticatorRecord) -> Result<()> {
        sqlx::query(
            r#"
            insert into authenticators (id, auth_type, title, enabled, is_builtin, sort_order, options)
            values ($1, $2, $3, $4, $5, $6, $7)
            "#,
        )
        .bind(authenticator.id)
        .bind(&authenticator.auth_type)
        .bind(&authenticator.title)
        .bind(authenticator.enabled)
        .bind(authenticator.is_builtin)
        .bind(authenticator.sort_order)
        .bind(&authenticator.options)
        .execute(&self.pool)
        .await?;

        Ok(())
    }

    pub async fn delete_authenticator_if_unbound(&self, id: Uuid) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        let authenticator = sqlx::query(
            r#"
            select is_builtin
            from authenticators
            where id = $1
            for update
            "#,
        )
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?;
        let Some(authenticator) = authenticator else {
            return Err(ControlPlaneError::NotFound("authenticator").into());
        };
        if authenticator.get::<bool, _>("is_builtin") {
            return Err(ControlPlaneError::InvalidInput("builtin_authenticator").into());
        }

        let identity_count: i64 = sqlx::query_scalar(
            "select count(*) from user_auth_identities where authenticator_id = $1",
        )
        .bind(id)
        .fetch_one(&mut *tx)
        .await?;
        if identity_count > 0 {
            return Err(ControlPlaneError::Conflict("authenticator_identity_bindings").into());
        }

        sqlx::query("delete from authenticators where id = $1")
            .bind(id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        Ok(())
    }

    pub async fn update_authenticator_order(&self, ids: &[Uuid]) -> Result<()> {
        let mut tx = self.pool.begin().await?;
        for (index, id) in ids.iter().enumerate() {
            sqlx::query(
                r#"
                update authenticators
                set sort_order = $2,
                    updated_at = now()
                where id = $1
                "#,
            )
            .bind(id)
            .bind((index as i32) * 10)
            .execute(&mut *tx)
            .await?;
        }
        tx.commit().await?;

        Ok(())
    }

    pub async fn upsert_permission_catalog(
        &self,
        permissions: &[PermissionDefinition],
    ) -> Result<()> {
        BootstrapRepository::upsert_permission_catalog(self, permissions).await
    }

    pub async fn upsert_root_tenant(&self) -> Result<TenantRecord> {
        BootstrapRepository::upsert_root_tenant(self).await
    }

    pub async fn upsert_workspace(
        &self,
        tenant_id: Uuid,
        workspace_name: &str,
    ) -> Result<WorkspaceRecord> {
        BootstrapRepository::upsert_workspace(self, tenant_id, workspace_name).await
    }

    pub async fn upsert_builtin_roles(&self, workspace_id: Uuid) -> Result<()> {
        BootstrapRepository::upsert_builtin_roles(self, workspace_id).await
    }

    pub async fn upsert_root_user(
        &self,
        workspace_id: Uuid,
        account: &str,
        email: &str,
        password_hash: &str,
        name: &str,
        nickname: &str,
    ) -> Result<UserRecord> {
        BootstrapRepository::upsert_root_user(
            self,
            workspace_id,
            account,
            email,
            password_hash,
            name,
            nickname,
        )
        .await
    }

    pub async fn find_authenticator(&self, id: Uuid) -> Result<Option<AuthenticatorRecord>> {
        AuthRepository::find_authenticator(self, id).await
    }

    pub async fn list_authenticators(&self) -> Result<Vec<AuthenticatorRecord>> {
        AuthRepository::list_authenticators(self).await
    }

    pub async fn find_user_for_password_login(
        &self,
        authenticator_id: Uuid,
        identifier: &str,
    ) -> Result<Option<UserRecord>> {
        AuthRepository::find_user_for_password_login(self, authenticator_id, identifier).await
    }

    pub async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<UserRecord>> {
        AuthRepository::find_user_by_id(self, user_id).await
    }

    pub async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        display_role: Option<&str>,
    ) -> Result<ActorContext> {
        AuthRepository::load_actor_context(self, user_id, tenant_id, workspace_id, display_role)
            .await
    }

    pub async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
        actor_id: Uuid,
    ) -> Result<i64> {
        AuthRepository::update_password_hash(self, user_id, password_hash, actor_id).await
    }

    pub async fn update_profile(&self, input: &UpdateProfileInput) -> Result<UserRecord> {
        AuthRepository::update_profile(self, input).await
    }

    pub async fn update_user_meta(
        &self,
        input: &control_plane::ports::UpdateUserMetaInput,
    ) -> Result<UserRecord> {
        AuthRepository::update_user_meta(self, input).await
    }

    pub async fn bump_session_version(&self, user_id: Uuid, actor_id: Uuid) -> Result<i64> {
        AuthRepository::bump_session_version(self, user_id, actor_id).await
    }

    pub async fn list_permissions(&self) -> Result<Vec<PermissionDefinition>> {
        AuthRepository::list_permissions(self).await
    }

    pub async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        AuthRepository::append_audit_log(self, event).await
    }

    pub async fn get_workspace(&self, workspace_id: Uuid) -> Result<Option<WorkspaceRecord>> {
        WorkspaceRepository::get_workspace(self, workspace_id).await
    }

    pub async fn update_workspace(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        name: &str,
        logo_url: Option<&str>,
        introduction: &str,
    ) -> Result<WorkspaceRecord> {
        WorkspaceRepository::update_workspace(
            self,
            actor_user_id,
            workspace_id,
            name,
            logo_url,
            introduction,
        )
        .await
    }

    pub async fn create_member_with_default_role(
        &self,
        input: &CreateMemberInput,
    ) -> Result<UserRecord> {
        MemberRepository::create_member_with_default_role(self, input).await
    }

    pub async fn create_application(
        &self,
        input: &CreateApplicationInput,
    ) -> Result<ApplicationRecord> {
        ApplicationRepository::create_application(self, input).await
    }

    pub async fn get_or_create_editor_state(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
    ) -> Result<FlowEditorState> {
        FlowRepository::get_or_create_editor_state(
            self,
            workspace_id,
            application_id,
            actor_user_id,
        )
        .await
    }

    pub async fn save_flow_draft(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        document: serde_json::Value,
        change_kind: FlowChangeKind,
        summary: &str,
    ) -> Result<FlowEditorState> {
        FlowRepository::save_draft(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            document,
            change_kind,
            summary,
        )
        .await
    }

    pub async fn restore_flow_version(
        &self,
        workspace_id: Uuid,
        application_id: Uuid,
        actor_user_id: Uuid,
        version_id: Uuid,
    ) -> Result<FlowEditorState> {
        FlowRepository::restore_version(
            self,
            workspace_id,
            application_id,
            actor_user_id,
            version_id,
        )
        .await
    }
}

pub(crate) fn decode_role_scope_kind(value: &str) -> RoleScopeKind {
    match value {
        "app" | "system" => RoleScopeKind::System,
        _ => RoleScopeKind::Workspace,
    }
}

pub(crate) const ROOT_TENANT_ID: &str = "00000000-0000-0000-0000-000000000001";
pub(crate) const ROOT_TENANT_CODE: &str = "root-tenant";
pub(crate) const ROOT_TENANT_NAME: &str = "Root Tenant";

pub(crate) async fn primary_workspace_id(pool: &PgPool) -> Result<Uuid> {
    sqlx::query_scalar("select id from workspaces order by created_at asc limit 1")
        .fetch_optional(pool)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound("workspace").into())
}

pub(crate) async fn tenant_id_for_workspace(pool: &PgPool, workspace_id: Uuid) -> Result<Uuid> {
    sqlx::query_scalar("select tenant_id from workspaces where id = $1")
        .bind(workspace_id)
        .fetch_optional(pool)
        .await?
        .ok_or(control_plane::errors::ControlPlaneError::NotFound("tenant").into())
}

pub(crate) async fn workspace_id_for_user(pool: &PgPool, user_id: Uuid) -> Result<Uuid> {
    if let Some(workspace_id) = sqlx::query_scalar(
        r#"
        select workspace_id
        from workspace_memberships
        where user_id = $1
        order by created_at asc
        limit 1
        "#,
    )
    .bind(user_id)
    .fetch_optional(pool)
    .await?
    {
        Ok(workspace_id)
    } else {
        primary_workspace_id(pool).await
    }
}

pub(crate) async fn is_root_user(pool: &PgPool, user_id: Uuid) -> Result<bool> {
    sqlx::query_scalar(
        r#"
        select exists(
          select 1
          from user_role_bindings urb
          join roles r on r.id = urb.role_id
          where urb.user_id = $1
            and r.scope_kind = 'system'
            and r.code = 'root'
        )
        "#,
    )
    .bind(user_id)
    .fetch_one(pool)
    .await
    .map_err(Into::into)
}

pub(crate) fn stored_role_from_row(row: sqlx::postgres::PgRow) -> StoredRoleRow {
    let scope_kind: String = row.get("scope_kind");

    StoredRoleRow {
        id: row.get("id"),
        code: row.get("code"),
        name: row.get("name"),
        introduction: row.get("introduction"),
        scope_kind: decode_role_scope_kind(&scope_kind),
        is_builtin: row.get("is_builtin"),
        is_editable: row.get("is_editable"),
        auto_grant_new_permissions: row.get("auto_grant_new_permissions"),
        is_default_member_role: row.get("is_default_member_role"),
    }
}

pub(crate) async fn find_role_by_code(
    pool: &PgPool,
    workspace_id: Uuid,
    role_code: &str,
) -> Result<Option<StoredRoleRow>> {
    let row = sqlx::query(
        r#"
        select
          id,
          code,
          name,
          introduction,
          scope_kind,
          is_builtin,
          is_editable,
          auto_grant_new_permissions,
          is_default_member_role
        from roles
        where (scope_kind = 'system' and code = $1)
           or (scope_kind = 'workspace' and workspace_id = $2 and code = $1)
        order by scope_kind asc
        limit 1
        "#,
    )
    .bind(role_code)
    .bind(workspace_id)
    .fetch_optional(pool)
    .await?;

    Ok(row.map(stored_role_from_row))
}

pub(crate) async fn permission_codes_for_role(pool: &PgPool, role_id: Uuid) -> Result<Vec<String>> {
    sqlx::query_scalar(
        r#"
        select pd.code
        from role_permissions rp
        join permission_definitions pd on pd.id = rp.permission_id
        where rp.role_id = $1
        order by pd.code asc
        "#,
    )
    .bind(role_id)
    .fetch_all(pool)
    .await
    .map_err(Into::into)
}
