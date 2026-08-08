use std::sync::{
    atomic::{AtomicBool, AtomicUsize, Ordering},
    Arc,
};

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::ports::BootstrapRepository;
use domain::{
    AuthenticatorRecord, BoundRole, PermissionDefinition, RoleScopeKind, TenantRecord, UserRecord,
    UserStatus, WorkspaceRecord,
};

#[derive(Default, Clone)]
pub struct MemoryBootstrapRepository {
    inner: Arc<MemoryBootstrapRepositoryInner>,
}

#[derive(Default)]
struct MemoryBootstrapRepositoryInner {
    authenticator_upserts: AtomicUsize,
    root_tenant_upserts: AtomicUsize,
    workspace_upserts: AtomicUsize,
    official_catalog_initialized: AtomicBool,
    official_catalog_bootstraps: AtomicUsize,
    root_user_creates: AtomicUsize,
    authenticators: RwLock<Vec<AuthenticatorRecord>>,
    root_tenant: RwLock<Option<TenantRecord>>,
    workspace: RwLock<Option<WorkspaceRecord>>,
    root_user: RwLock<Option<UserRecord>>,
}

impl MemoryBootstrapRepository {
    pub fn authenticator_upserts(&self) -> usize {
        self.inner.authenticator_upserts.load(Ordering::SeqCst)
    }

    pub fn root_user_creates(&self) -> usize {
        self.inner.root_user_creates.load(Ordering::SeqCst)
    }

    pub fn root_tenant_upserts(&self) -> usize {
        self.inner.root_tenant_upserts.load(Ordering::SeqCst)
    }

    pub fn workspace_upserts(&self) -> usize {
        self.inner.workspace_upserts.load(Ordering::SeqCst)
    }

    pub fn mark_official_catalog_initialized(&self) {
        self.inner
            .official_catalog_initialized
            .store(true, Ordering::SeqCst);
    }

    pub fn official_catalog_bootstraps(&self) -> usize {
        self.inner
            .official_catalog_bootstraps
            .load(Ordering::SeqCst)
    }

    pub async fn authenticator(&self, id: Uuid) -> Option<AuthenticatorRecord> {
        self.inner
            .authenticators
            .read()
            .await
            .iter()
            .find(|authenticator| authenticator.id == id)
            .cloned()
    }

    pub async fn seed_authenticator(&self, authenticator: AuthenticatorRecord) {
        self.inner.authenticators.write().await.push(authenticator);
    }
}

#[async_trait]
impl BootstrapRepository for MemoryBootstrapRepository {
    async fn replace_authenticator_public_ui_block_if_matches(
        &self,
        authenticator_id: Uuid,
        expected: &str,
        replacement: &str,
    ) -> Result<bool> {
        let mut authenticators = self.inner.authenticators.write().await;
        let Some(authenticator) = authenticators
            .iter_mut()
            .find(|authenticator| authenticator.id == authenticator_id)
        else {
            return Ok(false);
        };
        if authenticator.public_ui_block != expected {
            return Ok(false);
        }
        authenticator.public_ui_block = replacement.to_string();
        Ok(true)
    }

    async fn upsert_authenticator(&self, authenticator: &AuthenticatorRecord) -> Result<()> {
        self.inner
            .authenticator_upserts
            .fetch_add(1, Ordering::SeqCst);
        let mut authenticators = self.inner.authenticators.write().await;
        match authenticators
            .iter_mut()
            .find(|stored| stored.id == authenticator.id)
        {
            Some(stored) => {
                let saved_public_ui_block = stored.public_ui_block.clone();
                *stored = authenticator.clone();
                if !saved_public_ui_block.is_empty() {
                    stored.public_ui_block = saved_public_ui_block;
                }
            }
            None => authenticators.push(authenticator.clone()),
        }
        Ok(())
    }

    async fn upsert_permission_catalog(&self, _permissions: &[PermissionDefinition]) -> Result<()> {
        Ok(())
    }

    async fn upsert_root_tenant(&self) -> Result<TenantRecord> {
        self.inner
            .root_tenant_upserts
            .fetch_add(1, Ordering::SeqCst);
        if let Some(tenant) = self.inner.root_tenant.read().await.clone() {
            return Ok(tenant);
        }

        let tenant = TenantRecord {
            id: Uuid::now_v7(),
            code: "root-tenant".to_string(),
            name: "Root Tenant".to_string(),
            is_root: true,
            is_hidden: true,
        };
        *self.inner.root_tenant.write().await = Some(tenant.clone());
        Ok(tenant)
    }

    async fn root_workspace_requires_official_catalog_seed(
        &self,
        _workspace_name: &str,
    ) -> Result<bool> {
        Ok(!self
            .inner
            .official_catalog_initialized
            .load(Ordering::SeqCst))
    }

    async fn upsert_workspace(
        &self,
        tenant_id: Uuid,
        workspace_name: &str,
    ) -> Result<WorkspaceRecord> {
        self.inner.workspace_upserts.fetch_add(1, Ordering::SeqCst);
        if let Some(workspace) = self.inner.workspace.read().await.clone() {
            return Ok(workspace);
        }

        let workspace = WorkspaceRecord {
            id: Uuid::now_v7(),
            tenant_id,
            name: workspace_name.to_string(),
            logo_url: None,
            introduction: String::new(),
        };
        *self.inner.workspace.write().await = Some(workspace.clone());
        Ok(workspace)
    }

    async fn upsert_root_workspace_with_official_catalog(
        &self,
        tenant_id: Uuid,
        workspace_name: &str,
        _seed: &crate::i18n_catalog::VerifiedOfficialCatalogSeed,
    ) -> Result<WorkspaceRecord> {
        self.inner
            .official_catalog_bootstraps
            .fetch_add(1, Ordering::SeqCst);
        self.inner
            .official_catalog_initialized
            .store(true, Ordering::SeqCst);
        BootstrapRepository::upsert_workspace(self, tenant_id, workspace_name).await
    }

    async fn upsert_builtin_roles(&self, _workspace_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn upsert_root_user(
        &self,
        _workspace_id: Uuid,
        account: &str,
        email: &str,
        password_hash: &str,
        name: &str,
        nickname: &str,
    ) -> Result<UserRecord> {
        if let Some(user) = self.inner.root_user.read().await.clone() {
            return Ok(user);
        }

        self.inner.root_user_creates.fetch_add(1, Ordering::SeqCst);
        let user = UserRecord {
            id: Uuid::now_v7(),
            account: account.to_string(),
            email: email.to_string(),
            phone: None,
            password_hash: password_hash.to_string(),
            name: name.to_string(),
            nickname: nickname.to_string(),
            avatar_url: None,
            introduction: String::new(),
            preferred_locale: None,
            meta: serde_json::json!({}),
            default_display_role: Some("root".to_string()),
            email_login_enabled: true,
            phone_login_enabled: false,
            status: UserStatus::Active,
            session_version: 1,
            roles: vec![BoundRole {
                code: "root".to_string(),
                name: "Root".to_string(),
                scope_kind: RoleScopeKind::System,
                workspace_id: None,
            }],
        };
        *self.inner.root_user.write().await = Some(user.clone());
        Ok(user)
    }
}
