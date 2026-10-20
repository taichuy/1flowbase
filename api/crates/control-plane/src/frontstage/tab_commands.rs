use super::*;

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository,
{
    pub async fn list_page_tabs(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageTabRecord>> {
        let actor = self.load_actor_context(actor_user_id, workspace_id).await?;
        self.ensure_page_visible(&actor, actor_user_id, workspace_id, page_id)
            .await?;
        let tabs = self
            .repository
            .list_frontstage_page_tabs(workspace_id, page_id)
            .await?;
        if actor.is_root {
            return Ok(tabs);
        }
        let pages = self.repository.list_frontstage_pages(workspace_id).await?;
        let rules = self
            .visibility_rules_for_actor(&actor, actor_user_id, workspace_id)
            .await?;
        let visibility = FrontstagePageVisibilityContext::new(&pages, &rules);
        Ok(tabs
            .into_iter()
            .filter(|tab| visibility.is_tab_visible(page_id, tab.id))
            .collect())
    }

    pub async fn create_page_tab(
        &self,
        command: CreateFrontstagePageTabCommand,
    ) -> Result<domain::frontstage::FrontstagePageTabRecord> {
        let actor = self
            .load_actor_context(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        let page = self
            .repository
            .get_frontstage_page(command.workspace_id, command.page_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        ensure_page_record(&page)?;
        if page.content_presentation != domain::frontstage::FrontstagePageContentPresentation::Tabs
        {
            return Err(ControlPlaneError::Conflict("frontstage_page_tabs_not_enabled").into());
        }
        let title = required_tab_title(command.title)?;
        let route_segment = normalize_tab_route_segment(command.route_segment)?.ok_or(
            ControlPlaneError::InvalidInput("frontstage_page_tab_route_segment_required"),
        )?;
        let tab_id = Uuid::now_v7();
        self.repository
            .create_frontstage_page_tab(&CreateFrontstagePageTabInput {
                id: tab_id,
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                title: Some(title),
                rank: normalize_rank(command.rank),
                is_default: false,
                route_segment: Some(route_segment),
                document_root_uid: reserved_tab_document_root_uid(tab_id),
            })
            .await
    }

    pub async fn update_page_tab(
        &self,
        command: UpdateFrontstagePageTabCommand,
    ) -> Result<domain::frontstage::FrontstagePageTabRecord> {
        let actor = self
            .load_actor_context(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.repository
            .update_frontstage_page_tab(&UpdateFrontstagePageTabInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                tab_id: command.tab_id,
                title: command.title,
                rank: command.rank.map(|rank| normalize_rank(Some(rank))),
            })
            .await
    }

    pub async fn delete_page_tab(&self, command: DeleteFrontstagePageTabCommand) -> Result<()> {
        let actor = self
            .load_actor_context(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.repository
            .delete_frontstage_page_tab(
                command.workspace_id,
                command.page_id,
                command.tab_id,
                command.actor_user_id,
            )
            .await
    }

    pub async fn save_tab_document(
        &self,
        command: SaveFrontstageTabDocumentCommand,
    ) -> Result<domain::frontstage::FrontstagePageDetail> {
        let actor = self
            .load_actor_context(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.ensure_existing_page(command.workspace_id, command.page_id)
            .await?;
        if command.document_payload.get("blocks").is_some() {
            return Err(ControlPlaneError::InvalidInput("frontstage_document_blocks").into());
        }

        let detail = self
            .repository
            .save_frontstage_tab_document(&SaveFrontstageTabDocumentInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                tab_id: command.tab_id,
                document_payload: command.document_payload,
            })
            .await?;
        self.audit(&actor, &detail.page, "frontstage.tab_document_saved")
            .await?;

        Ok(detail)
    }
}
