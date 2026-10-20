mod bootstrap;
mod effective_roles;
pub(crate) mod identity_binding;
pub(crate) mod lifecycle;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use control_plane_contracts::{
    ports::{
        ApiKeyRepository, AuthRepository, BootstrapRepository, CreateApiKeyInput,
        UpdateProfileInput, UpdateUserMetaInput,
    },
    ControlPlaneContractError as ControlPlaneError,
};
use domain::{
    ActorContext, ApiKeyRecord, AuditLogRecord, AuthenticationConnectionRecord, BoundRole,
    LoginEntryRecord, PermissionDefinition, RoleScopeKind, ScopeContext, TenantRecord,
    UserAuthIdentity, UserRecord, VerifiedExternalIdentity,
};
use sqlx::{PgPool, Postgres, Row, Transaction};
use uuid::Uuid;

use self::identity_binding::{
    insert_password_local_identities, replace_password_local_contact_identities,
    upsert_password_local_identities,
};
use crate::{
    i18n_catalog_repository::insert_verified_release,
    mappers::{
        auth_mapper::{PgAuthMapper, StoredLoginEntryRow},
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
        select r.code, r.name, r.scope_kind, r.workspace_id as workspace_id
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
            name: row.get("name"),
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
    let roles = load_bound_roles(pool, user_id).await?;
    Ok(map_user_row_with_roles(row, roles))
}

pub(crate) async fn map_user_rows(
    pool: &PgPool,
    rows: Vec<sqlx::postgres::PgRow>,
) -> Result<Vec<UserRecord>> {
    if rows.is_empty() {
        return Ok(Vec::new());
    }
    let ids: Vec<Uuid> = rows.iter().map(|row| row.get("id")).collect();
    let roles = sqlx::query(
        "select b.user_id,r.code,r.name,r.scope_kind,r.workspace_id from user_role_bindings b join roles r on r.id=b.role_id where b.user_id=any($1) order by r.scope_kind,r.code"
    ).bind(&ids).fetch_all(pool).await?;
    let mut by_user = std::collections::HashMap::<Uuid, Vec<BoundRole>>::new();
    for row in roles {
        by_user
            .entry(row.get("user_id"))
            .or_default()
            .push(BoundRole {
                code: row.get("code"),
                name: row.get("name"),
                scope_kind: decode_role_scope_kind(row.get::<String, _>("scope_kind").as_str()),
                workspace_id: row.get("workspace_id"),
            });
    }
    Ok(rows
        .into_iter()
        .map(|row| {
            let roles = by_user
                .remove(&row.get::<Uuid, _>("id"))
                .unwrap_or_default();
            map_user_row_with_roles(row, roles)
        })
        .collect())
}

fn map_user_row_with_roles(row: sqlx::postgres::PgRow, roles: Vec<BoundRole>) -> UserRecord {
    let roles = roles
        .into_iter()
        .map(|role| (role.code, role.name, role.scope_kind, role.workspace_id))
        .collect();
    PgMemberMapper::to_user_record(StoredMemberRow {
        id: row.get("id"),
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
    })
}

async fn auto_grant_role_scopes(tx: &mut Transaction<'_, Postgres>) -> Result<Vec<(Uuid, Uuid)>> {
    Ok(sqlx::query_as(
        r#"
        select id, scope_id
        from roles
        where auto_grant_new_permissions = true
        "#,
    )
    .fetch_all(&mut **tx)
    .await?)
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

async fn upsert_role_templates(
    pool: &PgPool,
    workspace_id: Uuid,
    roles: impl IntoIterator<Item = domain::RoleTemplate>,
) -> Result<()> {
    let mut tx = pool.begin().await?;
    // Share publication order with catalog synchronization, including workspaces created
    // after boot. Acquire this before writing roles to avoid a table-lock inversion.
    sqlx::query(
        "select initialized from console_permission_catalog_sync where singleton = true for update",
    )
    .fetch_one(&mut *tx)
    .await?;

    for role in roles {
        let scope_kind = match role.scope_kind {
            RoleScopeKind::System => "system",
            RoleScopeKind::Workspace => "workspace",
        };
        let scoped_workspace_id = if matches!(role.scope_kind, RoleScopeKind::Workspace) {
            Some(workspace_id)
        } else {
            None
        };
        let system_kind =
            matches!(role.scope_kind, RoleScopeKind::System).then_some(role.code.as_str());

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
        .bind(system_kind)
        .fetch_optional(&mut *tx)
        .await?;

        let role_id: Uuid = match role.scope_kind {
            RoleScopeKind::System => {
                sqlx::query_scalar("select id from roles where scope_kind = 'system' and code = $1")
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
            if role.scope_kind == RoleScopeKind::Workspace && role.code == "admin" {
                crate::role_repository::seed_admin_console_policy(&mut tx, role_id).await?;
            }
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

        let (can_all, default_scope) = match role.code.as_str() {
            "root" => (true, "system_all"),
            "admin" => (true, "scope_all"),
            "member" => (true, "own"),
            _ => (false, "own"),
        };
        sqlx::query(
            r#"
            insert into role_data_policies (
                id, role_id, can_view, can_create, can_update, can_delete,
                default_view_scope, default_update_scope, default_delete_scope
            )
            values ($1, $2, $3, $3, $3, $3, $4, $4, $4)
            on conflict (role_id) do nothing
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(role_id)
        .bind(can_all)
        .bind(default_scope)
        .execute(&mut *tx)
        .await?;
    }

    tx.commit().await?;
    Ok(())
}

#[async_trait]
impl AuthRepository for PgControlPlaneStore {
    async fn find_authentication_connection(
        &self,
        id: Uuid,
    ) -> Result<Option<AuthenticationConnectionRecord>> {
        let row = sqlx::query(
            r#"
            select id, auth_type, is_builtin, config
            from authentication_connections
            where id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;
        Ok(row.map(|row| AuthenticationConnectionRecord {
            id: row.get("id"),
            auth_type: row.get("auth_type"),
            is_builtin: row.get("is_builtin"),
            config: row.get("config"),
        }))
    }

    async fn find_login_entry(&self, id: Uuid) -> Result<Option<LoginEntryRecord>> {
        let row = sqlx::query(
            r#"
            select entry.id, entry.connection_id, connection.auth_type, entry.title,
                   entry.enabled, entry.is_builtin, entry.sort_order,
                   entry.public_ui_block, entry.options
            from login_entries entry
            join authentication_connections connection on connection.id = entry.connection_id
            where entry.id = $1
            "#,
        )
        .bind(id)
        .fetch_optional(self.pool())
        .await?;

        Ok(row.map(|row| {
            PgAuthMapper::to_login_entry_record(StoredLoginEntryRow {
                id: row.get("id"),
                connection_id: row.get("connection_id"),
                auth_type: row.get("auth_type"),
                title: row.get("title"),
                enabled: row.get("enabled"),
                is_builtin: row.get("is_builtin"),
                sort_order: row.get("sort_order"),
                public_ui_block: row.get("public_ui_block"),
                options: row.get("options"),
            })
        }))
    }

    async fn list_login_entries(&self) -> Result<Vec<LoginEntryRecord>> {
        let rows = sqlx::query(
            r#"
            select entry.id, entry.connection_id, connection.auth_type, entry.title,
                   entry.enabled, entry.is_builtin, entry.sort_order,
                   entry.public_ui_block, entry.options
            from login_entries entry
            join authentication_connections connection on connection.id = entry.connection_id
            order by entry.sort_order asc, entry.id asc
            "#,
        )
        .fetch_all(self.pool())
        .await?;

        Ok(rows
            .into_iter()
            .map(|row| {
                PgAuthMapper::to_login_entry_record(StoredLoginEntryRow {
                    id: row.get("id"),
                    connection_id: row.get("connection_id"),
                    auth_type: row.get("auth_type"),
                    title: row.get("title"),
                    enabled: row.get("enabled"),
                    is_builtin: row.get("is_builtin"),
                    sort_order: row.get("sort_order"),
                    public_ui_block: row.get("public_ui_block"),
                    options: row.get("options"),
                })
            })
            .collect())
    }

    async fn find_user_for_password_login(
        &self,
        connection_id: Uuid,
        identifier: &str,
    ) -> Result<Option<UserRecord>> {
        let lowered = identifier.trim().to_lowercase();
        let rows = sqlx::query(
            r#"
            select
              u.id, u.account, u.email, u.phone, u.password_hash, u.name, u.nickname, u.avatar_url,
              u.introduction, u.preferred_locale, u.meta, u.default_display_role, u.email_login_enabled, u.phone_login_enabled,
              u.status, u.session_version
            from user_auth_identities i
            join users u on u.id = i.user_id
            where i.connection_id = $1
              and lower(i.subject_value) = $2
              and (
                i.subject_type = $3
                or (i.subject_type = $4 and u.email_login_enabled = true)
                or (i.subject_type = $5 and u.phone_login_enabled = true)
              )
            "#,
        )
        .bind(connection_id)
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

    async fn find_user_for_verified_external_identity(
        &self,
        identity: &VerifiedExternalIdentity,
    ) -> Result<Option<UserRecord>> {
        let row = sqlx::query(
            r#"
            select u.id, u.account, u.email, u.phone, u.password_hash, u.name, u.nickname,
                   u.avatar_url, u.introduction, u.preferred_locale, u.meta,
                   u.default_display_role, u.email_login_enabled, u.phone_login_enabled,
                   u.status, u.session_version
            from user_auth_identities identity
            join users u on u.id = identity.user_id
            where identity.authority_namespace = $1
              and identity.issuer is not distinct from $2
              and identity.realm is not distinct from $3
              and identity.subject_type = $4
              and identity.subject_value = $5
              and identity.verified = true
              and identity.state = 'bound'
            "#,
        )
        .bind(identity.authority_namespace())
        .bind(identity.issuer())
        .bind(identity.realm())
        .bind(identity.subject_type())
        .bind(identity.subject_value())
        .fetch_optional(self.pool())
        .await?;

        match row {
            Some(row) => Ok(Some(map_user_row(self.pool(), row).await?)),
            None => Ok(None),
        }
    }

    async fn bind_verified_external_identity(
        &self,
        user_id: Uuid,
        identity: &VerifiedExternalIdentity,
        audit: &AuditLogRecord,
    ) -> Result<UserAuthIdentity> {
        lifecycle::bind_identity(self, user_id, identity, audit).await
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
        if let Some(actor) = self.actor_override(user_id, tenant_id, workspace_id, display_role)? {
            return Ok(actor);
        }
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

        let effective_display_role = display_role
            .filter(|candidate| codes.iter().any(|code| code == *candidate))
            .map(str::to_string)
            .or_else(|| codes.first().cloned())
            .unwrap_or_else(|| "member".to_string());

        if effective_display_role == "root" {
            return Ok(ActorContext::root_in_scope(
                user_id,
                tenant_id,
                workspace_id,
                &effective_display_role,
            ));
        }

        let role_ids = self
            .effective_role_ids(user_id, workspace_id, &effective_display_role)
            .await?;
        let permissions: Vec<String> = sqlx::query_scalar(
            r#"
            select distinct pd.code
            from role_permissions rp
            join permission_definitions pd on pd.id = rp.permission_id
            where rp.role_id = any($1)
            order by pd.code
            "#,
        )
        .bind(&role_ids)
        .fetch_all(self.pool())
        .await?;

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
        let (_role_id, bound_role_code) =
            role.ok_or(ControlPlaneError::InvalidInput("role_code"))?;

        if bound_role_code == "root" {
            return Ok(ActorContext::root_in_scope(
                user_id,
                tenant_id,
                workspace_id,
                &bound_role_code,
            ));
        }

        let role_ids = self
            .effective_role_ids(user_id, workspace_id, &bound_role_code)
            .await?;
        let permissions: Vec<String> = sqlx::query_scalar(
            r#"
            select distinct pd.code
            from role_permissions rp
            join permission_definitions pd on pd.id = rp.permission_id
            where rp.role_id = any($1)
            order by pd.code asc
            "#,
        )
        .bind(&role_ids)
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
