use super::*;
use std::sync::Arc;

#[async_trait]
pub trait SessionStore: Send + Sync {
    async fn put(&self, session: SessionRecord) -> anyhow::Result<()>;
    async fn get(&self, session_id: &str) -> anyhow::Result<Option<SessionRecord>>;
    async fn delete(&self, session_id: &str) -> anyhow::Result<()>;
    async fn touch(&self, session_id: &str, expires_at_unix: i64) -> anyhow::Result<()>;

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        EphemeralInspectionCapabilities::unsupported()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        Ok(Vec::new())
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        Ok(summarize_ephemeral_entries(
            &self.list_ephemeral_entries().await?,
        ))
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        Ok(paginate_ephemeral_tree(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(paginate_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            request,
        ))
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        Ok(search_ephemeral_entries(
            self.list_ephemeral_entries().await?,
            query,
            request,
        ))
    }

    async fn reveal_ephemeral_entry(
        &self,
        _entry_ref: &str,
        _reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        Ok(None)
    }
}

#[async_trait]
impl<T> SessionStore for Arc<T>
where
    T: SessionStore + ?Sized,
{
    async fn put(&self, session: SessionRecord) -> anyhow::Result<()> {
        (**self).put(session).await
    }

    async fn get(&self, session_id: &str) -> anyhow::Result<Option<SessionRecord>> {
        (**self).get(session_id).await
    }

    async fn delete(&self, session_id: &str) -> anyhow::Result<()> {
        (**self).delete(session_id).await
    }

    async fn touch(&self, session_id: &str, expires_at_unix: i64) -> anyhow::Result<()> {
        (**self).touch(session_id, expires_at_unix).await
    }

    fn ephemeral_inspection_capabilities(&self) -> EphemeralInspectionCapabilities {
        (**self).ephemeral_inspection_capabilities()
    }

    async fn list_ephemeral_entries(&self) -> anyhow::Result<Vec<EphemeralEntrySnapshot>> {
        (**self).list_ephemeral_entries().await
    }

    async fn summarize_ephemeral_entries(
        &self,
    ) -> anyhow::Result<EphemeralInspectionSummarySnapshot> {
        (**self).summarize_ephemeral_entries().await
    }

    async fn list_ephemeral_tree(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionTreePage> {
        (**self).list_ephemeral_tree(request).await
    }

    async fn list_ephemeral_entry_page(
        &self,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        (**self).list_ephemeral_entry_page(request).await
    }

    async fn search_ephemeral_entry_page(
        &self,
        query: &str,
        request: EphemeralInspectionPageRequest,
    ) -> anyhow::Result<EphemeralInspectionEntryPage> {
        (**self).search_ephemeral_entry_page(query, request).await
    }

    async fn reveal_ephemeral_entry(
        &self,
        entry_ref: &str,
        reveal_mode: EphemeralValueRevealMode,
    ) -> anyhow::Result<Option<EphemeralEntryValueSnapshot>> {
        (**self)
            .reveal_ephemeral_entry(entry_ref, reveal_mode)
            .await
    }
}

#[async_trait]
pub trait BootstrapRepository: Send + Sync {
    async fn upsert_authenticator(&self, authenticator: &AuthenticatorRecord)
        -> anyhow::Result<()>;
    async fn upsert_permission_catalog(
        &self,
        permissions: &[PermissionDefinition],
    ) -> anyhow::Result<()>;
    async fn upsert_root_tenant(&self) -> anyhow::Result<TenantRecord>;
    async fn upsert_workspace(
        &self,
        tenant_id: Uuid,
        workspace_name: &str,
    ) -> anyhow::Result<WorkspaceRecord>;
    async fn upsert_builtin_roles(&self, workspace_id: Uuid) -> anyhow::Result<()>;
    async fn upsert_root_user(
        &self,
        workspace_id: Uuid,
        account: &str,
        email: &str,
        password_hash: &str,
        name: &str,
        nickname: &str,
    ) -> anyhow::Result<UserRecord>;
}

#[async_trait]
pub trait AuthRepository: Send + Sync {
    async fn find_authenticator(&self, id: Uuid) -> anyhow::Result<Option<AuthenticatorRecord>>;
    async fn list_authenticators(&self) -> anyhow::Result<Vec<AuthenticatorRecord>> {
        Ok(self
            .find_authenticator(domain::PASSWORD_LOCAL_AUTHENTICATOR_ID)
            .await?
            .into_iter()
            .collect())
    }
    async fn find_user_for_password_login(
        &self,
        authenticator_id: Uuid,
        identifier: &str,
    ) -> anyhow::Result<Option<UserRecord>>;
    async fn find_user_by_id(&self, user_id: Uuid) -> anyhow::Result<Option<UserRecord>>;
    async fn default_scope_for_user(&self, user_id: Uuid) -> anyhow::Result<ScopeContext>;
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> anyhow::Result<ActorContext>;
    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        display_role: Option<&str>,
    ) -> anyhow::Result<ActorContext>;
    async fn load_actor_context_for_bound_role(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
    ) -> anyhow::Result<ActorContext> {
        self.load_actor_context(user_id, tenant_id, workspace_id, Some(role_code))
            .await
    }
    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
        actor_id: Uuid,
    ) -> anyhow::Result<i64>;
    async fn update_profile(&self, input: &UpdateProfileInput) -> anyhow::Result<UserRecord>;
    async fn update_user_meta(&self, input: &UpdateUserMetaInput) -> anyhow::Result<UserRecord>;
    async fn bump_session_version(&self, user_id: Uuid, actor_id: Uuid) -> anyhow::Result<i64>;
    async fn list_permissions(&self) -> anyhow::Result<Vec<PermissionDefinition>>;
    async fn append_audit_log(&self, event: &AuditLogRecord) -> anyhow::Result<()>;
}

#[async_trait]
pub trait AuthenticatorSettingsRepository: Send + Sync {
    async fn create_authenticator(&self, authenticator: &AuthenticatorRecord)
        -> anyhow::Result<()>;
    async fn update_authenticator_config(
        &self,
        authenticator: &AuthenticatorRecord,
    ) -> anyhow::Result<()>;
    async fn delete_authenticator_if_unbound(&self, id: Uuid) -> anyhow::Result<()>;
    async fn update_authenticator_order(&self, ids: &[Uuid]) -> anyhow::Result<()>;
}

#[derive(Debug, Clone)]
pub struct CreateApiKeyInput {
    pub id: Uuid,
    pub name: String,
    pub token_hash: String,
    pub token_prefix: String,
    pub key_kind: domain::ApiKeyKind,
    pub application_id: Option<Uuid>,
    pub role_code: Option<String>,
    pub creator_user_id: Uuid,
    pub tenant_id: Uuid,
    pub scope_kind: DataModelScopeKind,
    pub scope_id: Uuid,
    pub enabled: bool,
    pub expires_at: Option<OffsetDateTime>,
}

#[async_trait]
pub trait ApiKeyRepository: Send + Sync {
    async fn create_api_key(&self, input: &CreateApiKeyInput) -> anyhow::Result<ApiKeyRecord>;
    async fn find_api_key_by_token_hash(
        &self,
        token_hash: &str,
    ) -> anyhow::Result<Option<ApiKeyRecord>>;
    async fn mark_api_key_used(&self, api_key_id: Uuid) -> anyhow::Result<()>;
    async fn list_user_api_keys(
        &self,
        creator_user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<Vec<ApiKeyRecord>>;
    async fn revoke_user_api_key(
        &self,
        api_key_id: Uuid,
        creator_user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<()>;
    async fn list_application_api_keys(
        &self,
        application_id: Uuid,
        creator_user_id: Uuid,
    ) -> anyhow::Result<Vec<ApiKeyRecord>>;
    async fn revoke_application_api_key(
        &self,
        api_key_id: Uuid,
        application_id: Uuid,
        creator_user_id: Uuid,
    ) -> anyhow::Result<()>;
}

#[async_trait]
pub trait WorkspaceRepository: Send + Sync {
    async fn get_workspace(&self, workspace_id: Uuid) -> anyhow::Result<Option<WorkspaceRecord>>;
    async fn list_accessible_workspaces(
        &self,
        user_id: Uuid,
    ) -> anyhow::Result<Vec<WorkspaceRecord>>;
    async fn get_accessible_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> anyhow::Result<Option<WorkspaceRecord>>;
    async fn update_workspace(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        name: &str,
        logo_url: Option<&str>,
        introduction: &str,
    ) -> anyhow::Result<WorkspaceRecord>;
}
#[derive(Debug, Clone)]
pub struct CreateMemberInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub account: String,
    pub email: String,
    pub phone: Option<String>,
    pub password_hash: String,
    pub name: String,
    pub nickname: String,
    pub introduction: String,
    pub email_login_enabled: bool,
    pub phone_login_enabled: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateMemberInput {
    pub actor_user_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub nickname: String,
    pub email: String,
    pub phone: Option<String>,
    pub introduction: String,
}

#[derive(Debug, Clone)]
pub struct UpdateProfileInput {
    pub actor_user_id: Uuid,
    pub user_id: Uuid,
    pub name: String,
    pub nickname: String,
    pub email: String,
    pub phone: Option<String>,
    pub avatar_url: Option<String>,
    pub introduction: String,
    pub preferred_locale: Option<String>,
}

#[derive(Debug, Clone)]
pub struct UpdateUserMetaInput {
    pub actor_user_id: Uuid,
    pub user_id: Uuid,
    pub meta: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct CreateWorkspaceRoleInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub code: String,
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: bool,
    pub is_default_member_role: bool,
}

#[derive(Debug, Clone)]
pub struct UpdateWorkspaceRoleInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub role_code: String,
    pub name: String,
    pub introduction: String,
    pub auto_grant_new_permissions: Option<bool>,
    pub is_default_member_role: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct RoleDataPolicyDefaultsInput {
    pub can_view: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub default_view_scope: domain::RoleDataPolicyScope,
    pub default_update_scope: domain::RoleDataPolicyScope,
    pub default_delete_scope: domain::RoleDataPolicyScope,
}

#[derive(Debug, Clone)]
pub struct RoleDataModelPolicyInput {
    pub data_model_id: Uuid,
    pub can_create_override: Option<bool>,
    pub view_scope_override: Option<domain::RoleDataPolicyScope>,
    pub update_scope_override: Option<domain::RoleDataPolicyScope>,
    pub delete_scope_override: Option<domain::RoleDataPolicyScope>,
}

#[derive(Debug, Clone)]
pub struct ReplaceRoleDataPolicyInput {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub role_code: String,
    pub default_policy: RoleDataPolicyDefaultsInput,
    pub model_policies: Vec<RoleDataModelPolicyInput>,
}

#[derive(Debug, Clone)]
pub struct RoleDataPolicyView {
    pub role_code: String,
    pub default_policy: domain::RoleDataPolicyRecord,
    pub model_policies: Vec<domain::RoleDataModelPolicyRecord>,
}

#[async_trait]
pub trait MemberRepository: Send + Sync {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> anyhow::Result<ActorContext>;
    async fn create_member_with_default_role(
        &self,
        input: &CreateMemberInput,
    ) -> anyhow::Result<UserRecord>;
    async fn update_member_profile(&self, input: &UpdateMemberInput) -> anyhow::Result<UserRecord>;
    async fn disable_member(&self, actor_user_id: Uuid, target_user_id: Uuid)
        -> anyhow::Result<()>;
    async fn enable_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> anyhow::Result<()>;
    async fn delete_member(&self, actor_user_id: Uuid, target_user_id: Uuid) -> anyhow::Result<()>;
    async fn reset_member_password(
        &self,
        actor_user_id: Uuid,
        target_user_id: Uuid,
        password_hash: &str,
    ) -> anyhow::Result<()>;
    async fn replace_member_roles(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        target_user_id: Uuid,
        role_codes: &[String],
    ) -> anyhow::Result<()>;
    async fn list_members(&self, workspace_id: Uuid) -> anyhow::Result<Vec<UserRecord>>;
    async fn append_audit_log(&self, event: &AuditLogRecord) -> anyhow::Result<()>;
}

#[async_trait]
pub trait RoleRepository: Send + Sync {
    async fn load_actor_context_for_user(
        &self,
        actor_user_id: Uuid,
    ) -> anyhow::Result<ActorContext>;
    async fn list_roles(&self, workspace_id: Uuid) -> anyhow::Result<Vec<RoleTemplate>>;
    async fn create_team_role(&self, input: &CreateWorkspaceRoleInput) -> anyhow::Result<()>;
    async fn update_team_role(&self, input: &UpdateWorkspaceRoleInput) -> anyhow::Result<()>;
    async fn delete_team_role(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
    ) -> anyhow::Result<()>;
    async fn replace_role_permissions(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
        permission_codes: &[String],
    ) -> anyhow::Result<()>;
    async fn list_role_permissions(
        &self,
        workspace_id: Uuid,
        role_code: &str,
    ) -> anyhow::Result<Vec<String>>;
    async fn get_role_data_policy(
        &self,
        workspace_id: Uuid,
        role_code: &str,
    ) -> anyhow::Result<RoleDataPolicyView>;
    async fn replace_role_data_policy(
        &self,
        input: &ReplaceRoleDataPolicyInput,
    ) -> anyhow::Result<RoleDataPolicyView>;
    async fn append_audit_log(&self, event: &AuditLogRecord) -> anyhow::Result<()>;
}
