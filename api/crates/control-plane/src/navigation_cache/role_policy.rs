use std::sync::Arc;

use crate::ports::RoleConsolePolicyReader;
use domain::{ActorContext, RoleConsolePolicy};

use super::{NavigationCache, NavigationCacheDomain};

pub struct CachedRoleConsolePolicyReader {
    source: Arc<dyn RoleConsolePolicyReader>,
    cache: NavigationCache,
}

impl CachedRoleConsolePolicyReader {
    pub fn new(source: Arc<dyn RoleConsolePolicyReader>, cache: NavigationCache) -> Self {
        Self { source, cache }
    }
}

/// Active role is not enough: department grants and direct bindings differ by user.
pub fn role_policy_variant(actor: &ActorContext) -> String {
    format!(
        "{}:{}:{}",
        actor.tenant_id, actor.user_id, actor.effective_display_role
    )
}

#[async_trait::async_trait]
impl RoleConsolePolicyReader for CachedRoleConsolePolicyReader {
    async fn load_role_console_policies_for_user(
        &self,
        actor: &ActorContext,
    ) -> anyhow::Result<Vec<RoleConsolePolicy>> {
        self.cache
            .load(
                NavigationCacheDomain::ConsoleRoutes,
                actor.current_workspace_id,
                &format!("role-policy:{}", role_policy_variant(actor)),
                self.source.load_role_console_policies_for_user(actor),
            )
            .await
    }
}
