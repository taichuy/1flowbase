use super::*;

impl<R: FrontstagePageRepository> FrontstagePageService<R> {
    pub fn with_navigation_cache(
        mut self,
        cache: crate::navigation_cache::NavigationCache,
    ) -> Self {
        self.navigation_cache = Some(cache);
        self
    }

    pub(super) async fn invalidate_navigation(&self, workspace_id: Uuid) {
        if let Some(cache) = &self.navigation_cache {
            cache
                .invalidate(
                    crate::navigation_cache::NavigationCacheDomain::FrontstagePages,
                    workspace_id,
                )
                .await;
        }
    }

    pub async fn list_page_tree(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontstagePageTreeNode>> {
        let actor = self.load_actor_context(actor_user_id, workspace_id).await?;
        let pages = match &self.navigation_cache {
            Some(cache) => {
                cache
                    .frontstage_pages(&self.repository, workspace_id)
                    .await?
            }
            None => self.repository.list_frontstage_pages(workspace_id).await?,
        };
        let visibility_rules = self
            .visibility_rules_for_actor(&actor, actor_user_id, workspace_id)
            .await?;

        Ok(build_visible_frontstage_page_tree(
            pages,
            &visibility_rules,
            &actor,
        ))
    }
}
