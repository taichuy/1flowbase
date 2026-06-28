pub(crate) mod identity_binding;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane::{
    errors::ControlPlaneError,
    ports::{
        ApiKeyRepository, AuthRepository, BootstrapRepository, CreateApiKeyInput,
        UpdateProfileInput, UpdateUserMetaInput,
    },
};
use domain::{
    ActorContext, ApiKeyRecord, AuditLogRecord, AuthenticatorRecord, BoundRole,
    PermissionDefinition, RoleScopeKind, ScopeContext, TenantRecord, UserRecord,
};
use sqlx::{PgPool, Row};
use uuid::Uuid;

use self::identity_binding::{
    insert_password_local_identities, replace_password_local_contact_identities,
    upsert_password_local_identities,
};
use crate::{
    mappers::{
        auth_mapper::{PgAuthMapper, StoredAuthenticatorRow},
        member_mapper::{PgMemberMapper, StoredMemberRow},
        workspace_mapper::{PgWorkspaceMapper, StoredWorkspaceRow},
    },
    repositories::{
        decode_role_scope_kind, tenant_id_for_workspace, PgControlPlaneStore, ROOT_TENANT_CODE,
        ROOT_TENANT_ID, ROOT_TENANT_NAME,
    },
};

async fn load_bound_roles(pool: &PgPool, user_id: Uuid) -> Result<Vec<BoundRole>> {
    let rows = sqlx::query(
        r#"
        select r.code, r.scope_kind, r.workspace_id as workspace_id
        from user_role_bindings urb
        join roles r on r.id = urb.role_id
        where urb.user_id = $1
        order by r.scope_kind asc, r.code asc
        "#,
    )
    .bind(user_id)
    .fetch_all(pool)
    .await?;

    Ok(rows
        .into_iter()
        .map(|row| BoundRole {
            code: row.get("code"),
            scope_kind: decode_role_scope_kind(row.get::<String, _>("scope_kind").as_str()),
            workspace_id: row.get("workspace_id"),
        })
        .collect())
}

fn map_api_key_row(row: sqlx::postgres::PgRow) -> Result<ApiKeyRecord> {
    let key_kind = domain::ApiKeyKind::from_db(row.get::<String, _>("key_kind").as_str())
        .ok_or_else(|| anyhow!("unknown api key kind"))?;
    Ok(ApiKeyRecord {
        id: row.get("id"),
        name: row.get("name"),
        token_hash: row.get("token_hash"),
        token_prefix: row.get("token_prefix"),
        key_kind,
        application_id: row.get("application_id"),
        role_code: row.get("role_code"),
        creator_user_id: row.get("creator_user_id"),
        tenant_id: row.get("tenant_id"),
        scope_kind: domain::DataModelScopeKind::from_db(
            row.get::<String, _>("scope_kind").as_str(),
        ),
        scope_id: row.get("scope_id"),
        enabled: row.get("enabled"),
        expires_at: row.get("expires_at"),
        last_used_at: row.get("last_used_at"),
        created_at: row.get("created_at"),
        updated_at: row.get("updated_at"),
    })
}

pub(crate) async fn map_user_row(pool: &PgPool, row: sqlx::postgres::PgRow) -> Result<UserRecord> {
    let user_id = row.get("id");
    let roles = load_bound_roles(pool, user_id)
        .await?
        .into_iter()
        .map(|role| (role.code, role.scope_kind, role.workspace_id))
        .collect();

    Ok(PgMemberMapper::to_user_record(StoredMemberRow {
        id: user_id,
        account: row.get("account"),
        email: row.get("email"),
        phone: row.get("phone"),
        password_hash: row.get("password_hash"),
        name: row.get("name"),
        nickname: row.get("nickname"),
        avatar_url: row.get("avatar_url"),
        introduction: row.get("introduction"),
        preferred_locale: row.get("preferred_locale"),
        meta: row.get("meta"),
        default_display_role: row.get("default_display_role"),
        email_login_enabled: row.get("email_login_enabled"),
        phone_login_enabled: row.get("phone_login_enabled"),
        status: row.get("status"),
        session_version: row.get("session_version"),
        roles,
    }))
}

async fn find_user_by_account(pool: &PgPool, account: &str) -> Result<Option<UserRecord>> {
    let row = sqlx::query(
        r#"
        select id, account, email, phone, password_hash, name, nickname, avatar_url,
               introduction, preferred_locale, meta, default_display_role, email_login_enabled, phone_login_enabled,
               status, session_version
        from users
        where lower(account) = $1
        "#,
    )
    .bind(account.trim().to_lowercase())
    .fetch_optional(pool)
    .await?;

    match row {
        Some(row) => Ok(Some(map_user_row(pool, row).await?)),
        None => Ok(None),
    }
}

#[async_trait]
impl BootstrapRepository for PgControlPlaneStore {
    async fn upsert_authenticator(&self, authenticator: &AuthenticatorRecord) -> Result<()> {
        sqlx::query(
            r#"
            insert into authenticators (id, name, auth_type, title, enabled, is_builtin, sort_order, options)
            values ($1, $2, $3, $4, $5, $6, 0, $7)
            on conflict (name) do update
              set auth_type = excluded.auth_type,
                  title = excluded.title,
                  enabled = excluded.enabled,
                  is_builtin = excluded.is_builtin,
                  options = excluded.options,
                  updated_at = now()
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(&authenticator.name)
        .bind(&authenticator.auth_type)
        .bind(&authenticator.title)
        .bind(authenticator.enabled)
        .bind(authenticator.is_builtin)
        .bind(&authenticator.options)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    async fn upsert_permission_catalog(&self, permissions: &[PermissionDefinition]) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let mut inserted_permission_ids = Vec::new();

        for permission in permissions {
            let inserted_permission_id: Option<Uuid> = sqlx::query_scalar(
                r#"
                insert into permission_definitions (id, scope_id, resource, action, scope, code, name, introduction)
                values ($1, $2, $3, $4, $5, $6, $7, '')
                on conflict (code) do nothing
                returning id
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(domain::SYSTEM_SCOPE_ID)
            .bind(&permission.resource)
            .bind(&permission.action)
            .bind(&permission.scope)
            .bind(&permission.code)
            .bind(&permission.name)
            .fetch_optional(&mut *tx)
            .await?;

            sqlx::query(
                r#"
                update permission_definitions
                set resource = $2,
                    action = $3,
                    scope = $4,
                    name = $5,
                    updated_at = now()
                where code = $1
                "#,
            )
            .bind(&permission.code)
            .bind(&permission.resource)
            .bind(&permission.action)
            .bind(&permission.scope)
            .bind(&permission.name)
            .execute(&mut *tx)
            .await?;

            if let Some(permission_id) = inserted_permission_id {
                inserted_permission_ids.push(permission_id);
            }
        }

        for permission_id in inserted_permission_ids {
            let role_scopes: Vec<(Uuid, Uuid)> = sqlx::query_as(
                r#"
                select id, scope_id
                from roles
                where auto_grant_new_permissions = true
                "#,
            )
            .fetch_all(&mut *tx)
            .await?;

            for (role_id, scope_id) in role_scopes {
                sqlx::query(
                    r#"
                    insert into role_permissions (id, role_id, permission_id, scope_id)
                    values ($1, $2, $3, $4)
                    on conflict (role_id, permission_id) do nothing
                    "#,
                )
                .bind(Uuid::now_v7())
                .bind(role_id)
                .bind(permission_id)
                .bind(scope_id)
                .execute(&mut *tx)
                .await?;
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn upsert_root_tenant(&self) -> Result<TenantRecord> {
        let tenant_id = Uuid::parse_str(ROOT_TENANT_ID).expect("root tenant id should be valid");
        let row = sqlx::query(
            r#"
            insert into tenants (id, code, name, is_root, is_hidden)
            values ($1, $2, $3, true, true)
            on conflict (code) do update
              set name = excluded.name,
                  is_root = true,
                  is_hidden = true,
                  updated_at = now()
            returning id, code, name, is_root, is_hidden
            "#,
        )
        .bind(tenant_id)
        .bind(ROOT_TENANT_CODE)
        .bind(ROOT_TENANT_NAME)
        .fetch_one(self.pool())
        .await?;

        Ok(TenantRecord {
            id: row.get("id"),
            code: row.get("code"),
            name: row.get("name"),
            is_root: row.get("is_root"),
            is_hidden: row.get("is_hidden"),
        })
    }

    async fn upsert_workspace(
        &self,
        tenant_id: Uuid,
        workspace_name: &str,
    ) -> Result<domain::WorkspaceRecord> {
        let existing = sqlx::query(
            r#"
            select id, tenant_id, name, logo_url, introduction
            from workspaces
            where tenant_id = $1 and lower(name) = lower($2)
            order by created_at asc
            limit 1
            "#,
        )
        .bind(tenant_id)
        .bind(workspace_name)
        .fetch_optional(self.pool())
        .await?;

        if let Some(row) = existing {
            return Ok(PgWorkspaceMapper::to_workspace_record(StoredWorkspaceRow {
                id: row.get("id"),
                tenant_id: row.get("tenant_id"),
                name: row.get("name"),
                logo_url: row.get("logo_url"),
                introduction: row.get("introduction"),
            }));
        }

        let has_workspace = sqlx::query_scalar::<_, bool>(
            "select exists(select 1 from workspaces where tenant_id = $1)",
        )
        .bind(tenant_id)
        .fetch_one(self.pool())
        .await?;
        let root_tenant_id =
            Uuid::parse_str(ROOT_TENANT_ID).expect("root tenant id should be valid");
        let id = if tenant_id == root_tenant_id && !has_workspace {
            domain::DEFAULT_SCOPE_ID
        } else {
            Uuid::now_v7()
        };
        sqlx::query(
            "insert into workspaces (id, tenant_id, name, logo_url, introduction) values ($1, $2, $3, null, '')",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(workspace_name)
        .execute(self.pool())
        .await?;

        Ok(PgWorkspaceMapper::to_workspace_record(StoredWorkspaceRow {
            id,
            tenant_id,
            name: workspace_name.to_string(),
            logo_url: None,
            introduction: String::new(),
        }))
    }

    async fn upsert_builtin_roles(&self, workspace_id: Uuid) -> Result<()> {
        let mut tx = self.pool().begin().await?;

        for role in access_control::builtin_role_templates() {
            let scope_kind = match role.scope_kind {
                RoleScopeKind::System => "system",
                RoleScopeKind::Workspace => "workspace",
            };
            let scoped_workspace_id = if matches!(role.scope_kind, RoleScopeKind::Workspace) {
                Some(workspace_id)
            } else {
                None
            };

            let inserted_role_id: Option<Uuid> = sqlx::query_scalar(
                r#"
                insert into roles (
                    id,
                    scope_id,
                    scope_kind,
                    workspace_id,
                    code,
                    name,
                    introduction,
                    is_builtin,
                    is_editable,
                    auto_grant_new_permissions,
                    is_default_member_role,
                    system_kind
                )
                values ($1, $2, $3, $4, $5, $6, '', $7, $8, $9, $10, $11)
                on conflict do nothing
                returning id
                "#,
            )
            .bind(Uuid::now_v7())
            .bind(match role.scope_kind {
                RoleScopeKind::System => domain::SYSTEM_SCOPE_ID,
                RoleScopeKind::Workspace => workspace_id,
            })
            .bind(scope_kind)
            .bind(scoped_workspace_id)
            .bind(&role.code)
            .bind(&role.name)
            .bind(role.is_builtin)
            .bind(role.is_editable)
            .bind(role.auto_grant_new_permissions)
            .bind(role.is_default_member_role)
            .bind(&role.code)
            .fetch_optional(&mut *tx)
            .await?;

            let role_id: Uuid = match role.scope_kind {
                RoleScopeKind::System => {
                    sqlx::query_scalar(
                        "select id from roles where scope_kind = 'system' and code = $1",
                    )
                    .bind(&role.code)
                    .fetch_one(&mut *tx)
                    .await?
                }
                RoleScopeKind::Workspace => sqlx::query_scalar(
                    "select id from roles where scope_kind = 'workspace' and workspace_id = $1 and code = $2",
                )
                .bind(workspace_id)
                .bind(&role.code)
                .fetch_one(&mut *tx)
                .await?,
            };

            if inserted_role_id.is_some() {
                for permission_code in role.permissions {
                    sqlx::query(
                        r#"
                        insert into role_permissions (id, role_id, permission_id, scope_id)
                        select $1, roles.id, permission_definitions.id, roles.scope_id
                        from roles
                        join permission_definitions on permission_definitions.code = $3
                        where roles.id = $2
                        on conflict (role_id, permission_id) do nothing
                        "#,
                    )
                    .bind(Uuid::now_v7())
                    .bind(role_id)
                    .bind(permission_code)
                    .execute(&mut *tx)
                    .await?;
                }
            }
        }

        tx.commit().await?;
        Ok(())
    }

    async fn upsert_root_user(
        &self,
        workspace_id: Uuid,
        account: &str,
        email: &str,
        password_hash: &str,
        name: &str,
        nickname: &str,
    ) -> Result<UserRecord> {
        if let Some(user) = find_user_by_account(self.pool(), account).await? {
            upsert_password_local_identities(self.pool(), &user).await?;
            return self
                .find_user_by_id(user.id)
                .await?
                .ok_or_else(|| anyhow!("root user missing after identity reconciliation"));
        }

        let user_id = Uuid::now_v7();
        let mut tx = self.pool().begin().await?;

        sqlx::query(
            r#"
            insert into users (
                id, account, email, phone, password_hash, name, nickname, avatar_url, introduction,
                default_display_role, email_login_enabled, phone_login_enabled, status, session_version
            )
            values ($1, $2, $3, null, $4, $5, $6, null, '', 'root', true, false, 'active', 1)
            "#,
        )
        .bind(user_id)
        .bind(account)
        .bind(email)
        .bind(password_hash)
        .bind(name)
        .bind(nickname)
        .execute(&mut *tx)
        .await?;

        insert_password_local_identities(&mut tx, user_id, account, email, None, None).await?;

        sqlx::query(
            "insert into workspace_memberships (id, workspace_id, user_id, introduction) values ($1, $2, $3, '') on conflict (workspace_id, user_id) do nothing",
        )
        .bind(Uuid::now_v7())
        .bind(workspace_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        sqlx::query(
            r#"
            insert into user_role_bindings (id, user_id, role_id, scope_id)
            select $1, $2, id, scope_id from roles where code = 'root' and scope_kind = 'system'
            on conflict (user_id, role_id) do nothing
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.find_user_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("root user missing after bootstrap"))
    }
}

#[async_trait]
impl AuthRepository for PgControlPlaneStore {
    async fn find_authenticator(&self, name: &str) -> Result<Option<AuthenticatorRecord>> {
        let row = sqlx::query(
            "select name, auth_type, title, enabled, is_builtin, options from authenticators where name = $1",
        )
        .bind(name)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| {
            PgAuthMapper::to_authenticator_record(StoredAuthenticatorRow {
                name: row.get("name"),
                auth_type: row.get("auth_type"),
                title: row.get("title"),
                enabled: row.get("enabled"),
                is_builtin: row.get("is_builtin"),
                options: row.get("options"),
            })
        }))
    }

    async fn find_user_for_password_login(&self, identifier: &str) -> Result<Option<UserRecord>> {
        let lowered = identifier.trim().to_lowercase();
        let rows = sqlx::query(
            r#"
            select
              u.id, u.account, u.email, u.phone, u.password_hash, u.name, u.nickname, u.avatar_url,
              u.introduction, u.preferred_locale, u.meta, u.default_display_role, u.email_login_enabled, u.phone_login_enabled,
              u.status, u.session_version
            from user_auth_identities i
            join users u on u.id = i.user_id
            where i.authenticator_name = $1
              and lower(i.subject_value) = $2
              and (
                i.subject_type = $3
                or (i.subject_type = $4 and u.email_login_enabled = true)
                or (i.subject_type = $5 and u.phone_login_enabled = true)
              )
            "#,
        )
        .bind(domain::PASSWORD_LOCAL_AUTHENTICATOR_NAME)
        .bind(lowered)
        .bind(domain::AUTH_SUBJECT_TYPE_ACCOUNT)
        .bind(domain::AUTH_SUBJECT_TYPE_EMAIL)
        .bind(domain::AUTH_SUBJECT_TYPE_PHONE)
        .fetch_all(self.pool())
        .await?;

        let Some(first_user_id) = rows.first().map(|row| row.get::<Uuid, _>("id")) else {
            return Ok(None);
        };
        if rows
            .iter()
            .any(|row| row.get::<Uuid, _>("id") != first_user_id)
        {
            return Ok(None);
        }

        let Some(row) = rows.into_iter().next() else {
            return Ok(None);
        };
        Ok(Some(map_user_row(self.pool(), row).await?))
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<UserRecord>> {
        let row = sqlx::query(
            r#"
            select id, account, email, phone, password_hash, name, nickname, avatar_url,
                   introduction, preferred_locale, meta, default_display_role, email_login_enabled, phone_login_enabled,
                   status, session_version
            from users where id = $1
            "#,
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await?;

        match row {
            Some(row) => Ok(Some(map_user_row(self.pool(), row).await?)),
            None => Ok(None),
        }
    }

    async fn default_scope_for_user(&self, user_id: Uuid) -> Result<ScopeContext> {
        if let Some(row) = sqlx::query(
            r#"
            select t.tenant_id, tm.workspace_id as workspace_id
            from workspace_memberships tm
            join workspaces t on t.id = tm.workspace_id
            where tm.user_id = $1
            order by tm.created_at asc
            limit 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(self.pool())
        .await?
        {
            return Ok(ScopeContext {
                tenant_id: row.get("tenant_id"),
                workspace_id: row.get("workspace_id"),
            });
        }

        let workspace_id = crate::repositories::primary_workspace_id(self.pool()).await?;
        Ok(ScopeContext {
            tenant_id: tenant_id_for_workspace(self.pool(), workspace_id).await?,
            workspace_id,
        })
    }

    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        display_role: Option<&str>,
    ) -> Result<ActorContext> {
        let codes: Vec<String> = sqlx::query_scalar(
            r#"
            select r.code
            from user_role_bindings urb
            join roles r on r.id = urb.role_id
            where urb.user_id = $1 and (r.scope_kind = 'system' or r.workspace_id = $2)
            order by r.scope_kind asc, r.code asc
            "#,
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        let permissions: Vec<String> = sqlx::query_scalar(
            r#"
            select distinct pd.code
            from user_role_bindings urb
            join roles r on r.id = urb.role_id
            join role_permissions rp on rp.role_id = r.id
            join permission_definitions pd on pd.id = rp.permission_id
            where urb.user_id = $1 and (r.scope_kind = 'system' or r.workspace_id = $2)
            order by pd.code asc
            "#,
        )
        .bind(user_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        let effective_display_role = display_role
            .filter(|candidate| codes.iter().any(|code| code == *candidate))
            .map(str::to_string)
            .or_else(|| codes.first().cloned())
            .unwrap_or_else(|| "manager".to_string());

        if codes.iter().any(|code| code == "root") {
            return Ok(ActorContext::root_in_scope(
                user_id,
                tenant_id,
                workspace_id,
                &effective_display_role,
            ));
        }

        Ok(ActorContext::scoped_in_scope(
            user_id,
            tenant_id,
            workspace_id,
            &effective_display_role,
            permissions,
        ))
    }

    async fn load_actor_context_for_bound_role(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
    ) -> Result<ActorContext> {
        let role: Option<(Uuid, String)> = sqlx::query_as(
            r#"
            select r.id, r.code
            from user_role_bindings urb
            join roles r on r.id = urb.role_id
            where urb.user_id = $1
              and r.code = $2
              and (r.scope_kind = 'system' or r.workspace_id = $3)
            limit 1
            "#,
        )
        .bind(user_id)
        .bind(role_code)
        .bind(workspace_id)
        .fetch_optional(self.pool())
        .await?;
        let (role_id, bound_role_code) =
            role.ok_or(ControlPlaneError::InvalidInput("role_code"))?;

        if bound_role_code == "root" {
            return Ok(ActorContext::root_in_scope(
                user_id,
                tenant_id,
                workspace_id,
                &bound_role_code,
            ));
        }

        let permissions: Vec<String> = sqlx::query_scalar(
            r#"
            select distinct pd.code
            from role_permissions rp
            join permission_definitions pd on pd.id = rp.permission_id
            where rp.role_id = $1
            order by pd.code asc
            "#,
        )
        .bind(role_id)
        .fetch_all(self.pool())
        .await?;

        Ok(ActorContext::scoped_in_scope(
            user_id,
            tenant_id,
            workspace_id,
            &bound_role_code,
            permissions,
        ))
    }

    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        let scope = self.default_scope_for_user(actor_user_id).await?;
        self.load_actor_context(actor_user_id, scope.tenant_id, scope.workspace_id, None)
            .await
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
        actor_id: Uuid,
    ) -> Result<i64> {
        let row = sqlx::query(
            r#"
            update users
            set password_hash = $2,
                session_version = session_version + 1,
                updated_by = $3,
                updated_at = now()
            where id = $1
            returning session_version
            "#,
        )
        .bind(user_id)
        .bind(password_hash)
        .bind(actor_id)
        .fetch_one(self.pool())
        .await?;

        Ok(row.get("session_version"))
    }

    async fn update_profile(&self, input: &UpdateProfileInput) -> Result<UserRecord> {
        let mut tx = self.pool().begin().await?;
        let row = sqlx::query(
            r#"
            update users
            set name = $2,
                nickname = $3,
                email = $4,
                phone = $5,
                avatar_url = $6,
                introduction = $7,
                preferred_locale = $8,
                updated_by = $9,
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
        .bind(&input.avatar_url)
        .bind(&input.introduction)
        .bind(&input.preferred_locale)
        .bind(input.actor_user_id)
        .fetch_one(&mut *tx)
        .await?;

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

    async fn update_user_meta(&self, input: &UpdateUserMetaInput) -> Result<UserRecord> {
        let row = sqlx::query(
            r#"
            update users
            set meta = $2,
                updated_by = $3,
                updated_at = now()
            where id = $1
            returning id, account, email, phone, password_hash, name, nickname, avatar_url,
                      introduction, preferred_locale, meta, default_display_role, email_login_enabled, phone_login_enabled,
                      status, session_version
            "#,
        )
        .bind(input.user_id)
        .bind(&input.meta)
        .bind(input.actor_user_id)
        .fetch_one(self.pool())
        .await?;

        map_user_row(self.pool(), row).await
    }

    async fn bump_session_version(&self, user_id: Uuid, actor_id: Uuid) -> Result<i64> {
        let row = sqlx::query(
            r#"
            update users
            set session_version = session_version + 1,
                updated_by = $2,
                updated_at = now()
            where id = $1
            returning session_version
            "#,
        )
        .bind(user_id)
        .bind(actor_id)
        .fetch_one(self.pool())
        .await?;

        Ok(row.get("session_version"))
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionDefinition>> {
        let rows = sqlx::query(
            "select code, resource, action, scope, name from permission_definitions order by code asc",
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| PermissionDefinition {
                code: row.get("code"),
                resource: row.get("resource"),
                action: row.get("action"),
                scope: row.get("scope"),
                name: row.get("name"),
            })
            .collect())
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        let scope_id = event.workspace_id.unwrap_or(domain::SYSTEM_SCOPE_ID);
        sqlx::query(
            r#"
            insert into audit_logs (
                id,
                workspace_id,
                scope_id,
                actor_user_id,
                target_type,
                target_id,
                event_code,
                payload,
                created_by,
                updated_by,
                created_at,
                updated_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $4, $4, $9, $9)
            "#,
        )
        .bind(event.id)
        .bind(event.workspace_id)
        .bind(scope_id)
        .bind(event.actor_user_id)
        .bind(&event.target_type)
        .bind(event.target_id)
        .bind(&event.event_code)
        .bind(&event.payload)
        .bind(event.created_at)
        .execute(self.pool())
        .await?;

        Ok(())
    }
}

#[async_trait]
impl ApiKeyRepository for PgControlPlaneStore {
    async fn create_api_key(&self, input: &CreateApiKeyInput) -> Result<ApiKeyRecord> {
        let row = sqlx::query(
            r#"
            insert into api_keys (
                id, name, token_hash, token_prefix, creator_user_id, tenant_id,
                scope_kind, scope_id, key_kind, application_id, role_code, enabled, expires_at
            )
            values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11, $12, $13)
            returning id, name, token_hash, token_prefix, creator_user_id, tenant_id,
                      scope_kind, scope_id, key_kind, application_id, role_code, enabled, expires_at,
                      last_used_at, created_at, updated_at
            "#,
        )
        .bind(input.id)
        .bind(&input.name)
        .bind(&input.token_hash)
        .bind(&input.token_prefix)
        .bind(input.creator_user_id)
        .bind(input.tenant_id)
        .bind(input.scope_kind.as_str())
        .bind(input.scope_id)
        .bind(input.key_kind.as_str())
        .bind(input.application_id)
        .bind(&input.role_code)
        .bind(input.enabled)
        .bind(input.expires_at)
        .fetch_one(self.pool())
        .await?;

        map_api_key_row(row)
    }

    async fn find_api_key_by_token_hash(&self, token_hash: &str) -> Result<Option<ApiKeyRecord>> {
        let row = sqlx::query(
            r#"
            select id, name, token_hash, token_prefix, creator_user_id, tenant_id,
                   scope_kind, scope_id, key_kind, application_id, role_code, enabled, expires_at,
                   last_used_at, created_at, updated_at
            from api_keys
            where token_hash = $1
            "#,
        )
        .bind(token_hash)
        .fetch_optional(self.pool())
        .await?;

        row.map(map_api_key_row).transpose()
    }

    async fn mark_api_key_used(&self, api_key_id: Uuid) -> Result<()> {
        sqlx::query(
            r#"
            update api_keys
            set last_used_at = now(),
                updated_at = now()
            where id = $1
            "#,
        )
        .bind(api_key_id)
        .execute(self.pool())
        .await?;

        Ok(())
    }

    async fn list_user_api_keys(
        &self,
        creator_user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<ApiKeyRecord>> {
        let rows = sqlx::query(
            r#"
            select id, name, token_hash, token_prefix, creator_user_id, tenant_id,
                   scope_kind, scope_id, key_kind, application_id, role_code, enabled, expires_at,
                   last_used_at, created_at, updated_at
            from api_keys
            where key_kind = 'user_api_key'
              and creator_user_id = $1
              and tenant_id = $2
              and scope_id = $3
            order by created_at desc, id desc
            "#,
        )
        .bind(creator_user_id)
        .bind(tenant_id)
        .bind(workspace_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_api_key_row).collect()
    }

    async fn revoke_user_api_key(
        &self,
        api_key_id: Uuid,
        creator_user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            update api_keys
            set enabled = false,
                updated_at = now()
            where id = $1
              and key_kind = 'user_api_key'
              and creator_user_id = $2
              and tenant_id = $3
              and scope_id = $4
            "#,
        )
        .bind(api_key_id)
        .bind(creator_user_id)
        .bind(tenant_id)
        .bind(workspace_id)
        .execute(self.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow!("user_api_key not found"));
        }

        Ok(())
    }

    async fn list_application_api_keys(
        &self,
        application_id: Uuid,
        creator_user_id: Uuid,
    ) -> Result<Vec<ApiKeyRecord>> {
        let rows = sqlx::query(
            r#"
            select id, name, token_hash, token_prefix, creator_user_id, tenant_id,
                   scope_kind, scope_id, key_kind, application_id, role_code, enabled, expires_at,
                   last_used_at, created_at, updated_at
            from api_keys
            where key_kind = 'application_api_key'
              and application_id = $1
              and creator_user_id = $2
              and enabled = true
            order by created_at desc, id desc
            "#,
        )
        .bind(application_id)
        .bind(creator_user_id)
        .fetch_all(self.pool())
        .await?;

        rows.into_iter().map(map_api_key_row).collect()
    }

    async fn revoke_application_api_key(
        &self,
        api_key_id: Uuid,
        application_id: Uuid,
        creator_user_id: Uuid,
    ) -> Result<()> {
        let result = sqlx::query(
            r#"
            delete from api_keys
            where id = $1
              and key_kind = 'application_api_key'
              and application_id = $2
              and creator_user_id = $3
            "#,
        )
        .bind(api_key_id)
        .bind(application_id)
        .bind(creator_user_id)
        .execute(self.pool())
        .await?;
        if result.rows_affected() == 0 {
            return Err(anyhow!("application_api_key not found"));
        }

        Ok(())
    }
}
