use anyhow::Result;
use uuid::Uuid;

use super::{
    build_visible_frontstage_page_tree, ensure_page_record, map_block_repository_error,
    ControlPlaneError, FrontstagePageRepository, FrontstagePageService,
    FrontstagePageVisibilityContext,
};
use crate::ports::FrontstageBlockTreeRepository;

pub struct FrontstageBootstrapVisibilitySnapshot {
    actor: domain::ActorContext,
    pages: Vec<domain::FrontstagePageRecord>,
    visibility_rules: Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>,
}

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository,
{
    async fn load_cached_or_durable_pages(
        &self,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontstagePageRecord>> {
        match &self.navigation_cache {
            Some(cache) => cache.frontstage_pages(&self.repository, workspace_id).await,
            None => self.repository.list_frontstage_pages(workspace_id).await,
        }
    }

    pub async fn load_visible_page_tree_with_snapshot(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<(
        Vec<domain::FrontstagePageTreeNode>,
        FrontstageBootstrapVisibilitySnapshot,
    )> {
        let actor = self.load_actor_context(actor_user_id, workspace_id).await?;
        let (pages, visibility_rules) = tokio::try_join!(
            self.load_cached_or_durable_pages(workspace_id),
            self.visibility_rules_for_actor(&actor, actor_user_id, workspace_id),
        )?;

        let visible_tree =
            build_visible_frontstage_page_tree(pages.clone(), &visibility_rules, &actor);
        Ok((
            visible_tree,
            FrontstageBootstrapVisibilitySnapshot {
                actor,
                pages,
                visibility_rules,
            },
        ))
    }

    pub fn ensure_page_visible_in_snapshot(
        &self,
        snapshot: &FrontstageBootstrapVisibilitySnapshot,
        page_id: Uuid,
    ) -> Result<()> {
        let page = snapshot
            .pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        ensure_page_record(page)?;
        if snapshot.actor.is_root {
            return Ok(());
        }
        let visibility =
            FrontstagePageVisibilityContext::new(&snapshot.pages, &snapshot.visibility_rules);
        if visibility.is_visible(page_id) {
            return Ok(());
        }
        Err(ControlPlaneError::NotFound("frontstage_page").into())
    }

    pub fn is_tab_visible_in_snapshot(
        &self,
        snapshot: &FrontstageBootstrapVisibilitySnapshot,
        page_id: Uuid,
        tab_id: Uuid,
    ) -> bool {
        if snapshot.actor.is_root {
            return true;
        }
        FrontstagePageVisibilityContext::new(&snapshot.pages, &snapshot.visibility_rules)
            .is_tab_visible(page_id, tab_id)
    }

    pub async fn list_visible_tabs_in_snapshot(
        &self,
        snapshot: &FrontstageBootstrapVisibilitySnapshot,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageTabRecord>> {
        self.ensure_page_visible_in_snapshot(snapshot, page_id)?;
        let tabs = self
            .repository
            .list_frontstage_page_tabs(workspace_id, page_id)
            .await?;
        if snapshot.actor.is_root {
            return Ok(tabs);
        }
        let visibility =
            FrontstagePageVisibilityContext::new(&snapshot.pages, &snapshot.visibility_rules);
        Ok(tabs
            .into_iter()
            .filter(|tab| visibility.is_tab_visible(page_id, tab.id))
            .collect())
    }

    pub async fn list_block_codes_in_snapshot(
        &self,
        snapshot: &FrontstageBootstrapVisibilitySnapshot,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_id: Uuid,
        nodes: &[domain::FrontstageBlockNodeRecord],
        code_refs: &[String],
    ) -> Result<Vec<domain::frontstage::FrontstageBlockCodeRecord>> {
        self.ensure_page_visible_in_snapshot(snapshot, page_id)?;
        if !self.is_tab_visible_in_snapshot(snapshot, page_id, tab_id) {
            return Err(ControlPlaneError::NotFound("frontstage_page_tab").into());
        }
        if code_refs.is_empty() {
            return Ok(Vec::new());
        }
        let codes = self
            .repository
            .list_frontstage_block_codes_by_refs(workspace_id, page_id, code_refs)
            .await?;
        if let Some(cache) = &self.block_code_cache {
            for code in &codes {
                if let Some(digest) = &code.source_sha256 {
                    for node in nodes.iter().filter(|n| n.code_ref == code.code_ref) {
                        cache.store(node, digest).await;
                    }
                }
            }
        }
        Ok(codes)
    }
}

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository + FrontstageBlockTreeRepository,
{
    pub async fn load_tab_detail_and_block_roots_in_snapshot(
        &self,
        snapshot: &FrontstageBootstrapVisibilitySnapshot,
        workspace_id: Uuid,
        page_id: Uuid,
        detail_tab_reference: Option<&str>,
        block_roots_tab_id: Option<Uuid>,
        limit: u32,
    ) -> Result<(
        Option<domain::frontstage::FrontstagePageDetail>,
        Option<Vec<domain::FrontstageBlockNodeRecord>>,
    )> {
        self.ensure_page_visible_in_snapshot(snapshot, page_id)?;

        let detail_fut = async {
            let Some(tab_ref) = detail_tab_reference else {
                return Ok::<Option<domain::frontstage::FrontstagePageDetail>, anyhow::Error>(None);
            };
            let Some(detail) = self
                .repository
                .get_frontstage_page_tab_detail(workspace_id, page_id, tab_ref)
                .await?
            else {
                return Ok(None);
            };
            if ensure_page_record(&detail.page).is_err() {
                return Ok(None);
            }
            if !self.is_tab_visible_in_snapshot(snapshot, page_id, detail.tab.id) {
                return Ok(None);
            }
            Ok(Some(detail))
        };

        let roots_fut = async {
            let Some(tab_uuid) = block_roots_tab_id else {
                return Ok::<Option<Vec<domain::FrontstageBlockNodeRecord>>, anyhow::Error>(None);
            };
            if !self.is_tab_visible_in_snapshot(snapshot, page_id, tab_uuid) {
                return Ok(None);
            }
            let roots = self
                .repository
                .list_frontstage_block_roots(workspace_id, page_id, tab_uuid, limit)
                .await
                .map_err(map_block_repository_error)?;
            Ok(Some(roots))
        };

        tokio::try_join!(detail_fut, roots_fut)
    }
}
