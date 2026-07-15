use std::{
    collections::{HashMap, HashSet},
    sync::Arc,
};

use anyhow::Result;
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::ports::{
    AuthRepository, AuthenticatorSettingsRepository, RoleConsolePolicyReader, SessionStore,
    UpdateProfileInput, UpdateUserMetaInput, WorkspaceRepository,
};
use domain::{
    ActorContext, AuditLogRecord, AuthenticatorRecord, BoundRole, PermissionDefinition,
    RoleScopeKind, ScopeContext, SessionRecord, UserRecord, UserStatus, WorkspaceRecord,
};

pub fn password_hash(password: &str) -> String {
    let salt = SaltString::encode_b64(b"session-security-tests")
        .expect("static salt should be valid base64");
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .expect("password hashing should succeed in tests")
        .to_string()
}

#[derive(Default, Clone)]
pub struct MemoryWorkspaceRepository {
    workspaces: Arc<RwLock<HashMap<Uuid, WorkspaceRecord>>>,
    accessible_workspaces: Arc<RwLock<HashMap<Uuid, Vec<Uuid>>>>,
    root_user_ids: Arc<RwLock<HashSet<Uuid>>>,
    console_policies: Arc<RwLock<Vec<domain::RoleConsolePolicy>>>,
}

impl MemoryWorkspaceRepository {
    #[allow(dead_code)]
    pub async fn upsert_workspace(&self, workspace: WorkspaceRecord) {
        self.workspaces
            .write()
            .await
            .insert(workspace.id, workspace);
    }

    pub async fn set_accessible_workspaces(&self, user_id: Uuid, workspaces: Vec<WorkspaceRecord>) {
        let workspace_ids: Vec<Uuid> = workspaces.iter().map(|workspace| workspace.id).collect();
        let mut stored_workspaces = self.workspaces.write().await;
        for workspace in workspaces {
            stored_workspaces.insert(workspace.id, workspace);
        }
        drop(stored_workspaces);

        self.accessible_workspaces
            .write()
            .await
            .insert(user_id, workspace_ids);
    }

    #[allow(dead_code)]
    pub async fn mark_root_user(&self, user_id: Uuid) {
        self.root_user_ids.write().await.insert(user_id);
    }

    pub async fn set_console_policies(&self, policies: Vec<domain::RoleConsolePolicy>) {
        *self.console_policies.write().await = policies;
    }
}

#[async_trait]
impl WorkspaceRepository for MemoryWorkspaceRepository {
    async fn get_workspace(&self, workspace_id: Uuid) -> Result<Option<WorkspaceRecord>> {
        Ok(self.workspaces.read().await.get(&workspace_id).cloned())
    }

    async fn list_accessible_workspaces(&self, user_id: Uuid) -> Result<Vec<WorkspaceRecord>> {
        let stored_workspaces = self.workspaces.read().await;
        let mut workspaces = if self.root_user_ids.read().await.contains(&user_id) {
            stored_workspaces.values().cloned().collect::<Vec<_>>()
        } else {
            self.accessible_workspaces
                .read()
                .await
                .get(&user_id)
                .cloned()
                .unwrap_or_default()
                .into_iter()
                .filter_map(|workspace_id| stored_workspaces.get(&workspace_id).cloned())
                .collect::<Vec<_>>()
        };

        workspaces.sort_by(|left, right| {
            left.name
                .to_lowercase()
                .cmp(&right.name.to_lowercase())
                .then_with(|| left.id.cmp(&right.id))
        });
        Ok(workspaces)
    }

    async fn get_accessible_workspace(
        &self,
        user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Option<WorkspaceRecord>> {
        let workspaces = self.workspaces.read().await;
        if self.root_user_ids.read().await.contains(&user_id) {
            return Ok(workspaces.get(&workspace_id).cloned());
        }

        let is_accessible = self
            .accessible_workspaces
            .read()
            .await
            .get(&user_id)
            .map(|workspace_ids| workspace_ids.contains(&workspace_id))
            .unwrap_or(false);

        Ok(is_accessible
            .then(|| workspaces.get(&workspace_id).cloned())
            .flatten())
    }

    async fn update_workspace(
        &self,
        _actor_user_id: Uuid,
        workspace_id: Uuid,
        name: &str,
        logo_url: Option<&str>,
        introduction: &str,
    ) -> Result<WorkspaceRecord> {
        let mut workspaces = self.workspaces.write().await;
        let workspace = workspaces
            .entry(workspace_id)
            .or_insert_with(|| WorkspaceRecord {
                id: workspace_id,
                tenant_id: Uuid::nil(),
                name: String::new(),
                logo_url: None,
                introduction: String::new(),
            });
        workspace.name = name.to_string();
        workspace.logo_url = logo_url.map(str::to_string);
        workspace.introduction = introduction.to_string();
        Ok(workspace.clone())
    }
}

#[async_trait]
impl RoleConsolePolicyReader for MemoryWorkspaceRepository {
    async fn load_role_console_policies_for_user(
        &self,
        _user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self.console_policies.read().await.clone())
    }
}

#[derive(Clone)]
pub struct MemoryAuthRepository {
    user: Arc<RwLock<UserRecord>>,
    permissions: Arc<RwLock<HashSet<String>>>,
    authenticators: Arc<RwLock<Vec<AuthenticatorRecord>>>,
    console_policies: Arc<RwLock<Vec<domain::RoleConsolePolicy>>>,
    audit_events: Arc<RwLock<Vec<String>>>,
    audit_logs: Arc<RwLock<Vec<AuditLogRecord>>>,
    bump_session_version_calls: Arc<RwLock<Vec<(Uuid, Uuid)>>>,
}

impl MemoryAuthRepository {
    pub fn new(user: UserRecord) -> Self {
        Self {
            user: Arc::new(RwLock::new(user)),
            permissions: Arc::new(RwLock::new(HashSet::new())),
            authenticators: Arc::new(RwLock::new(Vec::new())),
            console_policies: Arc::new(RwLock::new(Vec::new())),
            audit_events: Arc::new(RwLock::new(Vec::new())),
            audit_logs: Arc::new(RwLock::new(Vec::new())),
            bump_session_version_calls: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn root_user(preferred_locale: Option<&str>) -> Self {
        Self::new(UserRecord {
            id: Uuid::now_v7(),
            account: "root".to_string(),
            email: "root@example.com".to_string(),
            phone: None,
            password_hash: "password-hash".to_string(),
            name: "Root".to_string(),
            nickname: "Root".to_string(),
            avatar_url: None,
            introduction: String::new(),
            preferred_locale: preferred_locale.map(str::to_string),
            meta: serde_json::json!({}),
            default_display_role: Some("root".to_string()),
            email_login_enabled: true,
            phone_login_enabled: false,
            status: UserStatus::Active,
            session_version: 1,
            roles: vec![BoundRole {
                code: "root".to_string(),
                scope_kind: RoleScopeKind::System,
                workspace_id: None,
            }],
        })
    }

    pub fn scoped_user(permissions: &[&str]) -> Self {
        let repository = Self::new(UserRecord {
            id: Uuid::now_v7(),
            account: "member".to_string(),
            email: "manager@example.com".to_string(),
            phone: None,
            password_hash: "password-hash".to_string(),
            name: "Manager".to_string(),
            nickname: "Manager".to_string(),
            avatar_url: None,
            introduction: String::new(),
            preferred_locale: None,
            meta: serde_json::json!({}),
            default_display_role: Some("member".to_string()),
            email_login_enabled: true,
            phone_login_enabled: false,
            status: UserStatus::Active,
            session_version: 1,
            roles: vec![BoundRole {
                code: "member".to_string(),
                scope_kind: RoleScopeKind::Workspace,
                workspace_id: Some(Uuid::nil()),
            }],
        });
        repository
            .permissions
            .try_write()
            .expect("permissions lock should be free while constructing repository")
            .extend(permissions.iter().map(|value| value.to_string()));
        repository
    }

    pub fn user(&self) -> UserRecord {
        self.user
            .try_read()
            .expect("user lock should be free in assertions")
            .clone()
    }

    pub fn audit_events(&self) -> Vec<String> {
        self.audit_events
            .try_read()
            .expect("audit_events lock should be free in assertions")
            .clone()
    }

    pub fn audit_logs(&self) -> Vec<AuditLogRecord> {
        self.audit_logs
            .try_read()
            .expect("audit_logs lock should be free in assertions")
            .clone()
    }

    pub fn bump_session_version_calls(&self) -> Vec<(Uuid, Uuid)> {
        self.bump_session_version_calls
            .try_read()
            .expect("bump_session_version_calls lock should be free in assertions")
            .clone()
    }

    pub async fn set_console_policies(&self, policies: Vec<domain::RoleConsolePolicy>) {
        *self.console_policies.write().await = policies;
    }
}

#[async_trait]
impl AuthRepository for MemoryAuthRepository {
    async fn find_authenticator(&self, id: Uuid) -> Result<Option<AuthenticatorRecord>> {
        Ok(self
            .authenticators
            .read()
            .await
            .iter()
            .find(|authenticator| authenticator.id == id)
            .cloned())
    }

    async fn list_authenticators(&self) -> Result<Vec<AuthenticatorRecord>> {
        Ok(self.authenticators.read().await.clone())
    }

    async fn find_user_for_password_login(
        &self,
        _authenticator_id: Uuid,
        _identifier: &str,
    ) -> Result<Option<UserRecord>> {
        Ok(None)
    }

    async fn find_user_by_id(&self, user_id: Uuid) -> Result<Option<UserRecord>> {
        let user = self.user.read().await.clone();
        Ok((user.id == user_id).then_some(user))
    }

    async fn default_scope_for_user(&self, _user_id: Uuid) -> Result<ScopeContext> {
        Ok(ScopeContext {
            tenant_id: Uuid::nil(),
            workspace_id: Uuid::nil(),
        })
    }

    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        let scope = self.default_scope_for_user(actor_user_id).await?;
        self.load_actor_context(actor_user_id, scope.tenant_id, scope.workspace_id, None)
            .await
    }

    async fn load_actor_context(
        &self,
        user_id: Uuid,
        tenant_id: Uuid,
        workspace_id: Uuid,
        display_role: Option<&str>,
    ) -> Result<ActorContext> {
        let user = self.user.read().await.clone();
        let codes: Vec<String> = user
            .roles
            .iter()
            .filter(|role| {
                matches!(role.scope_kind, RoleScopeKind::System)
                    || role.workspace_id == Some(workspace_id)
            })
            .map(|role| role.code.clone())
            .collect();
        let effective_display_role = display_role
            .filter(|candidate| codes.iter().any(|code| code == *candidate))
            .map(str::to_string)
            .or_else(|| codes.first().cloned())
            .unwrap_or_else(|| "member".to_string());

        Ok(ActorContext {
            user_id,
            tenant_id,
            current_workspace_id: workspace_id,
            effective_display_role,
            is_root: codes.iter().any(|code| code == "root"),
            permissions: self.permissions.read().await.clone(),
        })
    }

    async fn update_password_hash(
        &self,
        user_id: Uuid,
        password_hash: &str,
        _actor_id: Uuid,
    ) -> Result<i64> {
        let mut user = self.user.write().await;
        anyhow::ensure!(user.id == user_id, "unknown user");
        user.password_hash = password_hash.to_string();
        user.session_version += 1;
        Ok(user.session_version)
    }

    async fn update_profile(&self, input: &UpdateProfileInput) -> Result<UserRecord> {
        let mut user = self.user.write().await;
        anyhow::ensure!(user.id == input.user_id, "unknown user");
        user.name = input.name.clone();
        user.nickname = input.nickname.clone();
        user.email = input.email.clone();
        user.phone = input.phone.clone();
        user.avatar_url = input.avatar_url.clone();
        user.introduction = input.introduction.clone();
        user.preferred_locale = input.preferred_locale.clone();
        Ok(user.clone())
    }

    async fn update_user_meta(&self, input: &UpdateUserMetaInput) -> Result<UserRecord> {
        let mut user = self.user.write().await;
        anyhow::ensure!(user.id == input.user_id, "unknown user");
        user.meta = input.meta.clone();
        Ok(user.clone())
    }

    async fn bump_session_version(&self, user_id: Uuid, actor_id: Uuid) -> Result<i64> {
        let mut user = self.user.write().await;
        anyhow::ensure!(user.id == user_id, "unknown user");
        user.session_version += 1;
        self.bump_session_version_calls
            .write()
            .await
            .push((user_id, actor_id));
        Ok(user.session_version)
    }

    async fn list_permissions(&self) -> Result<Vec<PermissionDefinition>> {
        Ok(Vec::new())
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        self.audit_logs.write().await.push(event.clone());
        self.audit_events
            .write()
            .await
            .push(event.event_code.clone());
        Ok(())
    }
}

#[async_trait]
impl AuthenticatorSettingsRepository for MemoryAuthRepository {
    async fn create_authenticator(&self, authenticator: &AuthenticatorRecord) -> Result<()> {
        self.authenticators
            .write()
            .await
            .push(authenticator.clone());
        Ok(())
    }

    async fn update_authenticator_config(&self, authenticator: &AuthenticatorRecord) -> Result<()> {
        let mut authenticators = self.authenticators.write().await;
        let existing = authenticators
            .iter_mut()
            .find(|stored| stored.id == authenticator.id)
            .ok_or_else(|| anyhow::anyhow!("authenticator not found"))?;
        *existing = authenticator.clone();
        Ok(())
    }

    async fn delete_authenticator_if_unbound(&self, id: Uuid) -> Result<()> {
        self.authenticators
            .write()
            .await
            .retain(|authenticator| authenticator.id != id);
        Ok(())
    }

    async fn update_authenticator_order(&self, ids: &[Uuid]) -> Result<()> {
        let mut authenticators = self.authenticators.write().await;
        for (sort_order, id) in ids.iter().enumerate() {
            let authenticator = authenticators
                .iter_mut()
                .find(|authenticator| authenticator.id == *id)
                .ok_or_else(|| anyhow::anyhow!("authenticator not found"))?;
            authenticator.sort_order = (sort_order as i32 + 1) * 10;
        }
        authenticators.sort_by_key(|authenticator| authenticator.sort_order);
        Ok(())
    }
}

#[async_trait]
impl RoleConsolePolicyReader for MemoryAuthRepository {
    async fn load_role_console_policies_for_user(
        &self,
        _user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self.console_policies.read().await.clone())
    }
}

#[derive(Default, Clone)]
pub struct MemorySessionStore {
    sessions: Arc<RwLock<HashMap<String, SessionRecord>>>,
    deleted_session_ids: Arc<RwLock<Vec<String>>>,
}

impl MemorySessionStore {
    pub fn deleted_session_ids(&self) -> Vec<String> {
        self.deleted_session_ids
            .try_read()
            .expect("deleted_session_ids lock should be free in assertions")
            .clone()
    }
}

#[async_trait]
impl SessionStore for MemorySessionStore {
    async fn put(&self, session: SessionRecord) -> Result<()> {
        self.sessions
            .write()
            .await
            .insert(session.session_id.clone(), session);
        Ok(())
    }

    async fn get(&self, session_id: &str) -> Result<Option<SessionRecord>> {
        Ok(self.sessions.read().await.get(session_id).cloned())
    }

    async fn delete(&self, session_id: &str) -> Result<()> {
        self.sessions.write().await.remove(session_id);
        self.deleted_session_ids
            .write()
            .await
            .push(session_id.to_string());
        Ok(())
    }

    async fn touch(&self, session_id: &str, expires_at_unix: i64) -> Result<()> {
        if let Some(existing) = self.sessions.write().await.get_mut(session_id) {
            existing.expires_at_unix = expires_at_unix;
        }
        Ok(())
    }
}

pub fn memory_actor_context(is_root: bool, permissions: &[&str]) -> ActorContext {
    let user_id = Uuid::now_v7();
    let workspace_id = Uuid::nil();
    if is_root {
        ActorContext::root(user_id, workspace_id, "root")
    } else {
        ActorContext::scoped(
            user_id,
            workspace_id,
            "member",
            permissions.iter().map(|permission| permission.to_string()),
        )
    }
}
