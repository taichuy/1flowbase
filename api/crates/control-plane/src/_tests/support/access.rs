use std::sync::Arc;

use anyhow::Result;
use async_trait::async_trait;
use tokio::sync::RwLock;
use uuid::Uuid;

use crate::ports::{
    CreateMemberInput, CreateWorkspaceRoleInput, MemberRepository, RoleConsolePolicyReader,
    RoleRepository, UpdateMemberInput, UpdateWorkspaceRoleInput,
};
use domain::{
    ActorContext, AuditLogRecord, BoundRole, RoleScopeKind, RoleTemplate, UserRecord, UserStatus,
};

#[derive(Debug, Clone)]
pub struct CreatedMember {
    pub role_codes: Vec<String>,
}

#[derive(Clone)]
pub struct MemoryMemberRepository {
    root_user_id: Uuid,
    actor_context: Arc<RwLock<Option<ActorContext>>>,
    console_policies: Arc<RwLock<Vec<domain::RoleConsolePolicy>>>,
    default_role_code: Arc<RwLock<String>>,
    created_members: Arc<RwLock<Vec<CreatedMember>>>,
    audit_events: Arc<RwLock<Vec<String>>>,
}

impl Default for MemoryMemberRepository {
    fn default() -> Self {
        Self::with_default_role("member")
    }
}

impl MemoryMemberRepository {
    pub fn with_default_role(role_code: &str) -> Self {
        Self {
            root_user_id: Uuid::now_v7(),
            actor_context: Arc::new(RwLock::new(None)),
            console_policies: Arc::new(RwLock::new(Vec::new())),
            default_role_code: Arc::new(RwLock::new(role_code.to_string())),
            created_members: Arc::new(RwLock::new(Vec::new())),
            audit_events: Arc::new(RwLock::new(Vec::new())),
        }
    }

    pub fn root_user_id(&self) -> Uuid {
        self.root_user_id
    }

    pub fn created_members(&self) -> Vec<CreatedMember> {
        self.created_members
            .try_read()
            .expect("created_members lock should be free in assertions")
            .clone()
    }

    pub fn audit_events(&self) -> Vec<String> {
        self.audit_events
            .try_read()
            .expect("audit_events lock should be free in assertions")
            .clone()
    }

    pub async fn set_actor_context(&self, actor: ActorContext) {
        *self.actor_context.write().await = Some(actor);
    }

    pub async fn set_console_policies(&self, policies: Vec<domain::RoleConsolePolicy>) {
        *self.console_policies.write().await = policies;
    }
}

#[async_trait]
impl MemberRepository for MemoryMemberRepository {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        if let Some(actor) = self.actor_context.read().await.clone() {
            return Ok(actor);
        }
        Ok(ActorContext::root(actor_user_id, Uuid::nil(), "root"))
    }

    async fn create_member_with_default_role(
        &self,
        _input: &CreateMemberInput,
    ) -> Result<UserRecord> {
        let default_role_code = self.default_role_code.read().await.clone();
        self.created_members.write().await.push(CreatedMember {
            role_codes: vec![default_role_code.clone()],
        });
        Ok(UserRecord {
            id: Uuid::now_v7(),
            account: format!("{default_role_code}-1"),
            email: format!("{default_role_code}-1@example.com"),
            phone: Some("13800000000".to_string()),
            password_hash: "hash".to_string(),
            name: format!("{} 1", default_role_code.to_uppercase()),
            nickname: format!("{} 1", default_role_code.to_uppercase()),
            avatar_url: None,
            introduction: String::new(),
            preferred_locale: None,
            meta: serde_json::json!({}),
            default_display_role: Some(default_role_code.clone()),
            email_login_enabled: true,
            phone_login_enabled: false,
            status: UserStatus::Active,
            session_version: 1,
            roles: vec![BoundRole {
                code: default_role_code,
                scope_kind: RoleScopeKind::Workspace,
                workspace_id: Some(Uuid::nil()),
            }],
        })
    }

    async fn update_member_profile(&self, input: &UpdateMemberInput) -> Result<UserRecord> {
        Ok(UserRecord {
            id: input.user_id,
            account: "member-1".to_string(),
            email: input.email.clone(),
            phone: input.phone.clone(),
            password_hash: "hash".to_string(),
            name: input.name.clone(),
            nickname: input.nickname.clone(),
            avatar_url: None,
            introduction: input.introduction.clone(),
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
        })
    }

    async fn disable_member(&self, _actor_user_id: Uuid, _target_user_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn enable_member(&self, _actor_user_id: Uuid, _target_user_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn delete_member(&self, _actor_user_id: Uuid, _target_user_id: Uuid) -> Result<()> {
        Ok(())
    }

    async fn reset_member_password(
        &self,
        _actor_user_id: Uuid,
        _target_user_id: Uuid,
        _password_hash: &str,
    ) -> Result<()> {
        Ok(())
    }

    async fn replace_member_roles(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
        _target_user_id: Uuid,
        _role_codes: &[String],
    ) -> Result<()> {
        Ok(())
    }

    async fn list_members(&self, _workspace_id: Uuid) -> Result<Vec<UserRecord>> {
        Ok(Vec::new())
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        self.audit_events
            .write()
            .await
            .push(event.event_code.clone());
        Ok(())
    }
}

#[async_trait]
impl RoleConsolePolicyReader for MemoryMemberRepository {
    async fn load_role_console_policies_for_user(
        &self,
        _user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self.console_policies.read().await.clone())
    }
}

#[derive(Clone)]
pub struct MemoryRoleRepository {
    root_user_id: Uuid,
    actor_context: Arc<RwLock<Option<ActorContext>>>,
    roles: Arc<RwLock<Vec<RoleTemplate>>>,
    audit_events: Arc<RwLock<Vec<String>>>,
    touched_workspaces: Arc<RwLock<Vec<Uuid>>>,
    console_policies: Arc<RwLock<std::collections::BTreeMap<String, domain::RoleConsolePolicy>>>,
    actor_console_policies: Arc<RwLock<Vec<domain::RoleConsolePolicy>>>,
}

impl Default for MemoryRoleRepository {
    fn default() -> Self {
        Self {
            root_user_id: Uuid::now_v7(),
            actor_context: Arc::new(RwLock::new(None)),
            roles: Arc::new(RwLock::new(Vec::new())),
            audit_events: Arc::new(RwLock::new(Vec::new())),
            touched_workspaces: Arc::new(RwLock::new(Vec::new())),
            console_policies: Arc::new(RwLock::new(std::collections::BTreeMap::new())),
            actor_console_policies: Arc::new(RwLock::new(Vec::new())),
        }
    }
}

impl MemoryRoleRepository {
    pub fn root_user_id(&self) -> Uuid {
        self.root_user_id
    }

    pub fn audit_events(&self) -> Vec<String> {
        self.audit_events
            .try_read()
            .expect("audit_events lock should be free in assertions")
            .clone()
    }

    pub async fn set_actor_context(&self, actor: ActorContext) {
        *self.actor_context.write().await = Some(actor);
    }

    pub async fn set_actor_console_policies(&self, policies: Vec<domain::RoleConsolePolicy>) {
        *self.actor_console_policies.write().await = policies;
    }
}

#[async_trait]
impl RoleRepository for MemoryRoleRepository {
    async fn load_actor_context_for_user(&self, actor_user_id: Uuid) -> Result<ActorContext> {
        if let Some(actor) = self.actor_context.read().await.clone() {
            return Ok(actor);
        }
        Ok(ActorContext::root(actor_user_id, Uuid::nil(), "root"))
    }

    async fn list_roles(&self, _workspace_id: Uuid) -> Result<Vec<RoleTemplate>> {
        Ok(self.roles.read().await.clone())
    }

    async fn create_team_role(&self, input: &CreateWorkspaceRoleInput) -> Result<()> {
        self.touched_workspaces
            .write()
            .await
            .push(input.workspace_id);
        let mut roles = self.roles.write().await;
        if input.is_default_member_role {
            for role in roles.iter_mut() {
                if matches!(role.scope_kind, RoleScopeKind::Workspace) {
                    role.is_default_member_role = false;
                }
            }
        }
        roles.push(RoleTemplate {
            code: input.code.clone(),
            name: input.name.clone(),
            introduction: input.introduction.clone(),
            scope_kind: RoleScopeKind::Workspace,
            is_builtin: false,
            is_editable: true,
            auto_grant_new_permissions: input.auto_grant_new_permissions,
            is_default_member_role: input.is_default_member_role,
            permissions: Vec::new(),
        });
        Ok(())
    }

    async fn update_team_role(&self, input: &UpdateWorkspaceRoleInput) -> Result<()> {
        self.touched_workspaces
            .write()
            .await
            .push(input.workspace_id);
        let mut roles = self.roles.write().await;
        let role_index = roles.iter().position(|role| role.code == input.role_code);

        if matches!(input.is_default_member_role, Some(false))
            && role_index
                .and_then(|index| roles.get(index))
                .map(|role| role.is_default_member_role)
                .unwrap_or(false)
        {
            anyhow::bail!(crate::errors::ControlPlaneError::InvalidInput(
                "default_member_role_required"
            ));
        }

        if matches!(input.is_default_member_role, Some(true)) {
            for role in roles.iter_mut() {
                if matches!(role.scope_kind, RoleScopeKind::Workspace)
                    && role.code != input.role_code
                {
                    role.is_default_member_role = false;
                }
            }
        }

        if let Some(role) = role_index.and_then(|index| roles.get_mut(index)) {
            role.name = input.name.clone();
            role.introduction = input.introduction.clone();
            if let Some(value) = input.auto_grant_new_permissions {
                role.auto_grant_new_permissions = value;
            }
            if let Some(value) = input.is_default_member_role {
                role.is_default_member_role = value;
            }
        }
        Ok(())
    }

    async fn delete_team_role(
        &self,
        _actor_user_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
    ) -> Result<()> {
        self.touched_workspaces.write().await.push(workspace_id);
        self.roles
            .write()
            .await
            .retain(|role| role.code != role_code);
        Ok(())
    }

    async fn replace_role_permissions(
        &self,
        _actor_user_id: Uuid,
        workspace_id: Uuid,
        role_code: &str,
        permission_codes: &[String],
    ) -> Result<()> {
        self.touched_workspaces.write().await.push(workspace_id);
        if let Some(role) = self
            .roles
            .write()
            .await
            .iter_mut()
            .find(|role| role.code == role_code)
        {
            role.permissions = permission_codes.to_vec();
        }
        Ok(())
    }

    async fn list_role_permissions(
        &self,
        _workspace_id: Uuid,
        role_code: &str,
    ) -> Result<Vec<String>> {
        Ok(self
            .roles
            .read()
            .await
            .iter()
            .find(|role| role.code == role_code)
            .map(|role| role.permissions.clone())
            .unwrap_or_default())
    }

    async fn get_role_console_policy(
        &self,
        _workspace_id: Uuid,
        role_code: &str,
    ) -> Result<domain::RoleConsolePolicy> {
        self.console_policies
            .read()
            .await
            .get(role_code)
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("role console policy not found"))
    }

    async fn replace_role_console_policy(
        &self,
        input: &crate::ports::ReplaceRoleConsolePolicyInput,
    ) -> Result<domain::RoleConsolePolicy> {
        self.touched_workspaces
            .write()
            .await
            .push(input.workspace_id);
        let policy = domain::RoleConsolePolicy::new(Uuid::now_v7(), input.groups.clone());
        self.console_policies
            .write()
            .await
            .insert(input.role_code.clone(), policy.clone());
        Ok(policy)
    }

    async fn get_role_data_policy(
        &self,
        _workspace_id: Uuid,
        role_code: &str,
    ) -> Result<crate::ports::RoleDataPolicyView> {
        let role_id = Uuid::now_v7();
        let now = time::OffsetDateTime::now_utc();
        Ok(crate::ports::RoleDataPolicyView {
            role_code: role_code.to_string(),
            default_policy: domain::RoleDataPolicyRecord {
                id: Uuid::now_v7(),
                role_id,
                role_code: role_code.to_string(),
                can_view: false,
                can_create: false,
                can_update: false,
                can_delete: false,
                default_view_scope: domain::RoleDataPolicyScope::Own,
                default_update_scope: domain::RoleDataPolicyScope::Own,
                default_delete_scope: domain::RoleDataPolicyScope::Own,
                created_at: now,
                updated_at: now,
            },
            model_policies: Vec::new(),
        })
    }

    async fn replace_role_data_policy(
        &self,
        input: &crate::ports::ReplaceRoleDataPolicyInput,
    ) -> Result<crate::ports::RoleDataPolicyView> {
        self.touched_workspaces
            .write()
            .await
            .push(input.workspace_id);
        let role_id = Uuid::now_v7();
        let now = time::OffsetDateTime::now_utc();
        Ok(crate::ports::RoleDataPolicyView {
            role_code: input.role_code.clone(),
            default_policy: domain::RoleDataPolicyRecord {
                id: Uuid::now_v7(),
                role_id,
                role_code: input.role_code.clone(),
                can_view: input.default_policy.can_view,
                can_create: input.default_policy.can_create,
                can_update: input.default_policy.can_update,
                can_delete: input.default_policy.can_delete,
                default_view_scope: input.default_policy.default_view_scope,
                default_update_scope: input.default_policy.default_update_scope,
                default_delete_scope: input.default_policy.default_delete_scope,
                created_at: now,
                updated_at: now,
            },
            model_policies: input
                .model_policies
                .iter()
                .map(|policy| domain::RoleDataModelPolicyRecord {
                    id: Uuid::now_v7(),
                    role_id,
                    data_model_id: policy.data_model_id,
                    can_create_override: policy.can_create_override,
                    view_scope_override: policy.view_scope_override,
                    update_scope_override: policy.update_scope_override,
                    delete_scope_override: policy.delete_scope_override,
                    created_at: now,
                    updated_at: now,
                })
                .collect(),
        })
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        self.audit_events
            .write()
            .await
            .push(event.event_code.clone());
        Ok(())
    }
}

#[async_trait]
impl RoleConsolePolicyReader for MemoryRoleRepository {
    async fn load_role_console_policies_for_user(
        &self,
        _user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::RoleConsolePolicy>> {
        Ok(self.actor_console_policies.read().await.clone())
    }
}

#[async_trait]
impl crate::ports::FrontstagePageRepository for MemoryRoleRepository {
    async fn load_actor_context_for_workspace(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<ActorContext> {
        let mut actor = RoleRepository::load_actor_context_for_user(self, actor_user_id).await?;
        actor.current_workspace_id = workspace_id;
        Ok(actor)
    }

    async fn list_frontstage_pages(
        &self,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontstagePageRecord>> {
        Ok(Vec::new())
    }

    async fn list_frontstage_page_visibility_rules_for_actor_roles(
        &self,
        _actor_user_id: Uuid,
        _workspace_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>> {
        Ok(Vec::new())
    }

    async fn list_frontstage_page_visibility_rules_for_role(
        &self,
        _workspace_id: Uuid,
        _role_code: &str,
    ) -> Result<Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>> {
        Ok(Vec::new())
    }

    async fn replace_frontstage_page_visibility_rules_for_role(
        &self,
        _workspace_id: Uuid,
        _role_code: &str,
        _page_ids: &[Uuid],
        _tab_ids: &[Uuid],
        _actor_user_id: Uuid,
    ) -> Result<()> {
        Ok(())
    }

    async fn get_frontstage_page(
        &self,
        _workspace_id: Uuid,
        _page_id: Uuid,
    ) -> Result<Option<domain::FrontstagePageRecord>> {
        Ok(None)
    }

    async fn list_frontstage_page_tabs(
        &self,
        _workspace_id: Uuid,
        _page_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageTabRecord>> {
        Ok(Vec::new())
    }

    async fn get_frontstage_page_tab_detail(
        &self,
        _workspace_id: Uuid,
        _page_id: Uuid,
        _tab_reference: &str,
    ) -> Result<Option<domain::frontstage::FrontstagePageDetail>> {
        Ok(None)
    }

    async fn create_frontstage_page(
        &self,
        _input: &crate::ports::CreateFrontstagePageInput,
    ) -> Result<domain::frontstage::FrontstagePageCreation> {
        anyhow::bail!("frontstage page creation is not used by role tests")
    }

    async fn create_frontstage_page_tab(
        &self,
        _input: &crate::ports::CreateFrontstagePageTabInput,
    ) -> Result<domain::frontstage::FrontstagePageTabRecord> {
        anyhow::bail!("frontstage tab creation is not used by role tests")
    }

    async fn update_frontstage_page_metadata(
        &self,
        _input: &crate::ports::UpdateFrontstagePageMetadataInput,
    ) -> Result<domain::FrontstagePageRecord> {
        anyhow::bail!("frontstage metadata update is not used by role tests")
    }

    async fn update_frontstage_page_tab(
        &self,
        _input: &crate::ports::UpdateFrontstagePageTabInput,
    ) -> Result<domain::frontstage::FrontstagePageTabRecord> {
        anyhow::bail!("frontstage tab update is not used by role tests")
    }

    async fn move_frontstage_page(
        &self,
        _input: &crate::ports::MoveFrontstagePageInput,
    ) -> Result<domain::FrontstagePageRecord> {
        anyhow::bail!("frontstage move is not used by role tests")
    }

    async fn delete_frontstage_page(&self, _workspace_id: Uuid, _page_id: Uuid) -> Result<()> {
        anyhow::bail!("frontstage page deletion is not used by role tests")
    }

    async fn delete_frontstage_page_tab(
        &self,
        _workspace_id: Uuid,
        _page_id: Uuid,
        _tab_id: Uuid,
        _actor_user_id: Uuid,
    ) -> Result<()> {
        anyhow::bail!("frontstage tab deletion is not used by role tests")
    }

    async fn save_frontstage_tab_document(
        &self,
        _input: &crate::ports::SaveFrontstageTabDocumentInput,
    ) -> Result<domain::frontstage::FrontstagePageDetail> {
        anyhow::bail!("frontstage document save is not used by role tests")
    }

    async fn create_frontstage_block(
        &self,
        _input: &crate::ports::CreateFrontstageBlockInput,
    ) -> Result<domain::frontstage::FrontstagePageDetail> {
        anyhow::bail!("frontstage block creation is not used by role tests")
    }

    async fn get_frontstage_block_code(
        &self,
        _workspace_id: Uuid,
        _page_id: Uuid,
        _code_ref: &str,
    ) -> Result<Option<domain::frontstage::FrontstageBlockCodeRecord>> {
        Ok(None)
    }

    async fn save_frontstage_block_code(
        &self,
        _input: &crate::ports::SaveFrontstageBlockCodeInput,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        anyhow::bail!("frontstage code save is not used by role tests")
    }

    async fn append_audit_log(&self, event: &AuditLogRecord) -> Result<()> {
        self.audit_events
            .write()
            .await
            .push(event.event_code.clone());
        Ok(())
    }
}
