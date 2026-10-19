use super::*;

#[async_trait]
impl BootstrapRepository for PgControlPlaneStore {
    async fn replace_login_entry_public_ui_block_if_matches(
        &self,
        login_entry_id: Uuid,
        expected: &str,
        replacement: &str,
    ) -> Result<bool> {
        let result = sqlx::query(
            r#"
            update login_entries
            set public_ui_block = $3,
                updated_at = now()
            where id = $1
              and public_ui_block = $2
            "#,
        )
        .bind(login_entry_id)
        .bind(expected)
        .bind(replacement)
        .execute(self.pool())
        .await?;
        Ok(result.rows_affected() == 1)
    }

    async fn upsert_login_entry(&self, login_entry: &LoginEntryRecord) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let connection_auth_type: String = sqlx::query_scalar(
            r#"
            insert into authentication_connections (id, auth_type, is_builtin, config)
            values ($1, $2, $3, '{}'::jsonb)
            on conflict (id) do update set id = authentication_connections.id
            returning auth_type
            "#,
        )
        .bind(login_entry.connection_id)
        .bind(&login_entry.auth_type)
        .bind(login_entry.connection_id == domain::PASSWORD_LOCAL_CONNECTION_ID)
        .fetch_one(&mut *tx)
        .await?;
        if connection_auth_type != login_entry.auth_type {
            return Err(ControlPlaneError::Conflict("authentication_connection_type").into());
        }

        sqlx::query(
            r#"
            insert into login_entries (id, connection_id, title, enabled, is_builtin, sort_order, public_ui_block, options)
            values ($1, $2, $3, $4, $5, $6, $7, $8)
            on conflict (id) do update
              set connection_id = excluded.connection_id,
                  title = case
                    when login_entries.id = '00000000-0000-0000-0000-000000000001' then login_entries.title
                    else excluded.title
                  end,
                  enabled = case
                    when login_entries.id = '00000000-0000-0000-0000-000000000001' then login_entries.enabled
                    else excluded.enabled
                  end,
                  is_builtin = excluded.is_builtin,
                  public_ui_block = case
                    when login_entries.public_ui_block = '' then excluded.public_ui_block
                    else login_entries.public_ui_block
                  end,
                  options = case
                    when login_entries.id <> '00000000-0000-0000-0000-000000000001' then excluded.options
                    else
                    case
                      when jsonb_typeof(login_entries.options) = 'object' then login_entries.options
                      else '{}'::jsonb
                    end
                    || jsonb_strip_nulls(jsonb_build_object(
                      'description',
                        case
                          when login_entries.options ? 'description' then null
                          else excluded.options -> 'description'
                        end,
                      'config_form_schema',
                        case
                          when login_entries.options ? 'config_form_schema' then null
                          else excluded.options -> 'config_form_schema'
                        end,
                      'extension_config',
                        case
                          when login_entries.options ? 'extension_config' then null
                          else excluded.options -> 'extension_config'
                        end
                    ))
                  end,
                  updated_at = now()
            "#,
        )
        .bind(login_entry.id)
        .bind(login_entry.connection_id)
        .bind(&login_entry.title)
        .bind(login_entry.enabled)
        .bind(login_entry.is_builtin)
        .bind(login_entry.sort_order)
        .bind(&login_entry.public_ui_block)
        .bind(&login_entry.options)
        .execute(&mut *tx)
        .await?;
        tx.commit().await?;

        Ok(())
    }

    async fn upsert_permission_catalog(&self, permissions: &[PermissionDefinition]) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        let mut inserted_permissions = Vec::new();

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
                inserted_permissions.push((permission_id, permission.code.clone()));
            }
        }

        for (permission_id, _permission_code) in inserted_permissions {
            for (role_id, scope_id) in auto_grant_role_scopes(&mut tx).await? {
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

    async fn root_workspace_requires_official_catalog_seed(
        &self,
        workspace_name: &str,
    ) -> Result<bool> {
        let root_tenant_id =
            Uuid::parse_str(ROOT_TENANT_ID).expect("root tenant id should be valid");
        let initialized = sqlx::query_scalar::<_, bool>(
            r#"
            select exists(
              select 1
              from workspaces w
              join workspace_i18n_catalog_states state on state.workspace_id = w.id
              where w.tenant_id = $1
                and lower(w.name) = lower($2)
                and state.active_release_id is not null
            )
            "#,
        )
        .bind(root_tenant_id)
        .bind(workspace_name)
        .fetch_one(self.pool())
        .await?;

        Ok(!initialized)
    }

    async fn upsert_workspace_for_bootstrap(
        &self,
        tenant_id: Uuid,
        workspace_name: &str,
    ) -> Result<control_plane_contracts::ports::WorkspaceBootstrapResult> {
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
            return Ok(control_plane_contracts::ports::WorkspaceBootstrapResult {
                workspace: PgWorkspaceMapper::to_workspace_record(StoredWorkspaceRow {
                    id: row.get("id"),
                    tenant_id: row.get("tenant_id"),
                    name: row.get("name"),
                    logo_url: row.get("logo_url"),
                    introduction: row.get("introduction"),
                }),
                created: false,
            });
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
        let mut tx = self.pool().begin().await?;
        sqlx::query(
            "insert into workspaces (id, tenant_id, name, logo_url, introduction) values ($1, $2, $3, null, '')",
        )
        .bind(id)
        .bind(tenant_id)
        .bind(workspace_name)
        .execute(&mut *tx)
        .await?;
        sqlx::query("INSERT INTO notification_instances (id,workspace_id,type_code,display_name,is_builtin) VALUES ($1,$2,'system','System notification',true)")
            .bind(Uuid::now_v7()).bind(id).execute(&mut *tx).await?;
        tx.commit().await?;

        Ok(control_plane_contracts::ports::WorkspaceBootstrapResult {
            workspace: PgWorkspaceMapper::to_workspace_record(StoredWorkspaceRow {
                id,
                tenant_id,
                name: workspace_name.to_string(),
                logo_url: None,
                introduction: String::new(),
            }),
            created: true,
        })
    }

    async fn upsert_root_workspace_with_official_catalog_for_bootstrap(
        &self,
        tenant_id: Uuid,
        workspace_name: &str,
        seed: &control_plane_contracts::i18n_catalog::VerifiedOfficialCatalogSeed,
    ) -> Result<control_plane_contracts::ports::WorkspaceBootstrapResult> {
        let mut transaction = self.pool().begin().await?;
        let existing = sqlx::query(
            r#"
            select id, tenant_id, name, logo_url, introduction
            from workspaces
            where tenant_id = $1 and lower(name) = lower($2)
            order by created_at asc
            limit 1
            for update
            "#,
        )
        .bind(tenant_id)
        .bind(workspace_name)
        .fetch_optional(&mut *transaction)
        .await?;
        let (workspace, created) = if let Some(row) = existing {
            (
                PgWorkspaceMapper::to_workspace_record(StoredWorkspaceRow {
                    id: row.get("id"),
                    tenant_id: row.get("tenant_id"),
                    name: row.get("name"),
                    logo_url: row.get("logo_url"),
                    introduction: row.get("introduction"),
                }),
                false,
            )
        } else {
            let has_workspace: bool =
                sqlx::query_scalar("select exists(select 1 from workspaces where tenant_id = $1)")
                    .bind(tenant_id)
                    .fetch_one(&mut *transaction)
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
            .execute(&mut *transaction)
            .await?;
            sqlx::query("INSERT INTO notification_instances (id,workspace_id,type_code,display_name,is_builtin) VALUES ($1,$2,'system','System notification',true)")
                .bind(Uuid::now_v7()).bind(id).execute(&mut *transaction).await?;
            (
                PgWorkspaceMapper::to_workspace_record(StoredWorkspaceRow {
                    id,
                    tenant_id,
                    name: workspace_name.to_owned(),
                    logo_url: None,
                    introduction: String::new(),
                }),
                true,
            )
        };

        let existing_release = sqlx::query(
            r#"
            select id, semantic_sha256
            from i18n_catalog_releases
            where workspace_id = $1 and catalog_version = $2
            "#,
        )
        .bind(workspace.id)
        .bind(seed.catalog_version().as_str())
        .fetch_optional(&mut *transaction)
        .await?;
        let release_id = if let Some(row) = existing_release {
            if row.get::<String, _>("semantic_sha256") != seed.semantic_sha256().as_str() {
                return Err(ControlPlaneError::Conflict("official_i18n_catalog_seed").into());
            }
            row.get("id")
        } else {
            let release = seed.bind_to_workspace(workspace.id)?;
            insert_verified_release(&mut transaction, &release).await?;
            release.id()
        };

        sqlx::query(
            r#"
            insert into workspace_i18n_catalog_states (workspace_id)
            values ($1)
            on conflict (workspace_id) do nothing
            "#,
        )
        .bind(workspace.id)
        .execute(&mut *transaction)
        .await?;
        sqlx::query(
            r#"
            update workspace_i18n_catalog_states
            set active_release_id = $2, revision = revision + 1, updated_at = now()
            where workspace_id = $1 and active_release_id is null
            "#,
        )
        .bind(workspace.id)
        .bind(release_id)
        .execute(&mut *transaction)
        .await?;

        transaction.commit().await?;
        Ok(control_plane_contracts::ports::WorkspaceBootstrapResult { workspace, created })
    }

    async fn upsert_root_role(
        &self,
        workspace_id: Uuid,
        templates: &[domain::RoleTemplate],
    ) -> Result<()> {
        upsert_role_templates(self.pool(), workspace_id, templates.iter().cloned()).await
    }

    async fn seed_workspace_role_templates(
        &self,
        workspace_id: Uuid,
        templates: &[domain::RoleTemplate],
    ) -> Result<()> {
        upsert_role_templates(self.pool(), workspace_id, templates.iter().cloned()).await
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

        // Membership creation precedes the root role binding, so its credit-account trigger sees
        // a root user only after this transaction has established the binding.
        sqlx::query(
            "update user_credit_accounts set charge_enabled = false where workspace_id = $1 and user_id = $2 and credit_unit = 'USD'",
        )
        .bind(workspace_id)
        .bind(user_id)
        .execute(&mut *tx)
        .await?;

        tx.commit().await?;

        self.find_user_by_id(user_id)
            .await?
            .ok_or_else(|| anyhow!("root user missing after bootstrap"))
    }
}
