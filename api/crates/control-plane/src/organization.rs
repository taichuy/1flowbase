use crate::{
    errors::ControlPlaneError as Error,
    navigation_cache::{NavigationCache, NavigationCacheDomain},
    ports::{
        MemberRepository, OrganizationRepository, RoleConsolePolicyReader, SaveDepartmentInput,
    },
};
use anyhow::Result;
use serde::Serialize;
use uuid::Uuid;

#[derive(Debug, Clone, Serialize)]
pub struct OrganizationAccess {
    pub can_list: bool,
    pub can_create: bool,
    pub can_update: bool,
    pub can_delete: bool,
    pub can_replace_member_departments: bool,
    pub can_assign_roles: bool,
}

pub struct OrganizationService<R> {
    repository: R,
    navigation_cache: Option<NavigationCache>,
}

impl<R: OrganizationRepository + MemberRepository + RoleConsolePolicyReader>
    OrganizationService<R>
{
    pub fn new(repository: R) -> Self {
        Self {
            repository,
            navigation_cache: None,
        }
    }
    pub fn with_navigation_cache(mut self, cache: NavigationCache) -> Self {
        self.navigation_cache = Some(cache);
        self
    }

    pub async fn access(&self, actor: &domain::ActorContext) -> Result<OrganizationAccess> {
        let policies = self
            .repository
            .load_role_console_policies_for_user(actor)
            .await?;
        let allowed = |code: &str| {
            actor.is_root
                || domain::effective_console_simple_operation(
                    &policies,
                    &domain::ConsolePolicyGroup::settings_feature(
                        access_control::SYSTEM_MEMBERS_SETTINGS_FEATURE_ID,
                    )
                    .expect("members feature"),
                    &domain::ConsoleOperationId::try_from(code).expect("organization operation"),
                )
        };
        Ok(OrganizationAccess {
            can_list: allowed("departments.list"),
            can_create: allowed("departments.create"),
            can_update: allowed("departments.update"),
            can_delete: allowed("departments.delete"),
            can_replace_member_departments: allowed("members.departments.replace"),
            can_assign_roles: allowed("members.roles.replace"),
        })
    }
    fn ensure(allowed: bool) -> Result<()> {
        if allowed {
            Ok(())
        } else {
            Err(Error::PermissionDenied("permission_denied").into())
        }
    }
    async fn invalidate(&self, workspace_id: Uuid) {
        if let Some(cache) = &self.navigation_cache {
            cache
                .invalidate(NavigationCacheDomain::ConsoleRoutes, workspace_id)
                .await;
            cache
                .invalidate(NavigationCacheDomain::FrontstagePages, workspace_id)
                .await;
        }
    }
    pub async fn list(&self, actor: &domain::ActorContext) -> Result<Vec<domain::Department>> {
        Self::ensure(self.access(actor).await?.can_list)?;
        self.repository
            .list_departments(actor.current_workspace_id)
            .await
    }
    pub async fn save(
        &self,
        actor: &domain::ActorContext,
        id: Option<Uuid>,
        name: String,
        parent_id: Option<Uuid>,
        role_codes: Vec<String>,
    ) -> Result<domain::Department> {
        let access = self.access(actor).await?;
        Self::ensure(if id.is_some() {
            access.can_update
        } else {
            access.can_create
        })?;
        let department = self
            .repository
            .save_department(&SaveDepartmentInput {
                actor_user_id: actor.user_id,
                workspace_id: actor.current_workspace_id,
                department_id: id,
                name,
                parent_id,
                role_codes,
                can_assign_roles: access.can_assign_roles,
            })
            .await?;
        self.invalidate(actor.current_workspace_id).await;
        Ok(department)
    }
    pub async fn delete(&self, actor: &domain::ActorContext, id: Uuid) -> Result<()> {
        Self::ensure(self.access(actor).await?.can_delete)?;
        self.repository
            .delete_department(actor.current_workspace_id, id)
            .await?;
        self.invalidate(actor.current_workspace_id).await;
        Ok(())
    }
    pub async fn replace_member_departments(
        &self,
        actor: &domain::ActorContext,
        user_id: Uuid,
        input: domain::MemberDepartments,
    ) -> Result<()> {
        Self::ensure(self.access(actor).await?.can_replace_member_departments)?;
        if !input.is_valid() {
            return Err(Error::InvalidInput("primary_department_id").into());
        }
        self.repository
            .replace_member_departments(actor.current_workspace_id, user_id, &input)
            .await?;
        self.invalidate(actor.current_workspace_id).await;
        Ok(())
    }
}
