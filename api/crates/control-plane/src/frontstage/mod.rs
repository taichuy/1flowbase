use std::{
    cmp::Ordering,
    collections::{HashMap, HashSet},
};

use access_control::ensure_permission;
use anyhow::Result;
use uuid::Uuid;

use crate::{
    audit::audit_log,
    errors::ControlPlaneError,
    ports::{
        CreateFrontstagePageInput, CreateFrontstagePageTabInput, FrontstagePageRepository,
        MoveFrontstagePageInput, SaveFrontstageBlockCodeInput, SaveFrontstageTabDocumentInput,
        UpdateFrontstagePageMetadataInput, UpdateFrontstagePageTabInput,
    },
};

pub struct CreateFrontstageGroupCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub parent_id: Option<Uuid>,
    pub rank: Option<String>,
    pub placement: domain::frontstage::FrontstageNavigationPlacement,
    pub slug: Option<String>,
}

pub struct CreateFrontstagePageCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub parent_id: Option<Uuid>,
    pub rank: Option<String>,
    pub placement: domain::frontstage::FrontstageNavigationPlacement,
    pub slug: Option<String>,
}

pub struct UpdateFrontstagePageMetadataCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub title: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub tooltip: Option<Option<String>>,
    pub is_hidden: Option<bool>,
    pub placement: Option<domain::frontstage::FrontstageNavigationPlacement>,
    pub content_presentation: Option<domain::frontstage::FrontstagePageContentPresentation>,
    pub slug: Option<Option<String>>,
}

pub struct MoveFrontstagePageCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub parent_id: Option<Uuid>,
    pub rank: Option<String>,
}

pub struct DeleteFrontstagePageCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
}

pub struct GetFrontstagePageDetailCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_reference: String,
}

pub struct CreateFrontstagePageTabCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub title: Option<String>,
    pub route_segment: Option<String>,
    pub rank: Option<String>,
}

pub struct UpdateFrontstagePageTabCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub title: Option<Option<String>>,
    pub rank: Option<String>,
}

pub struct DeleteFrontstagePageTabCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
}

pub struct GetFrontstageBlockCodeCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub code_ref: String,
}

pub struct SaveFrontstageTabDocumentCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub tab_id: Uuid,
    pub document_payload: serde_json::Value,
}

pub struct SaveFrontstageBlockCodeCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub code_ref: String,
    pub code: String,
}

const RESERVED_FRONTSTAGE_SLUGS: &[&str] = &[
    "api",
    "applications",
    "assets",
    "auth",
    "embedded-apps",
    "frontstage",
    "health",
    "login",
    "me",
    "settings",
    "sign-in",
    "templates",
];

fn normalize_frontstage_slug(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.len() < 4
        || normalized.len() > 48
        || normalized.starts_with('-')
        || normalized.ends_with('-')
        || normalized.contains("--")
        || !normalized
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
    {
        return Err(ControlPlaneError::InvalidInput("frontstage_page_slug").into());
    }
    if RESERVED_FRONTSTAGE_SLUGS.contains(&normalized.as_str()) {
        return Err(ControlPlaneError::InvalidInput("frontstage_page_slug_reserved").into());
    }
    Ok(Some(normalized))
}

fn normalize_tab_route_segment(value: Option<String>) -> Result<Option<String>> {
    let Some(value) = value else {
        return Ok(None);
    };
    let normalized = value.trim().to_ascii_lowercase();
    if normalized.is_empty()
        || normalized.len() > 48
        || normalized.starts_with('-')
        || normalized.ends_with('-')
        || normalized.contains("--")
        || !normalized
            .bytes()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || byte == b'-')
        || Uuid::parse_str(&normalized).is_ok()
    {
        return Err(ControlPlaneError::InvalidInput("frontstage_page_tab_route_segment").into());
    }
    Ok(Some(normalized))
}

fn required_tab_title(value: Option<String>) -> Result<String> {
    let title = value.unwrap_or_default().trim().to_owned();
    if title.is_empty() {
        return Err(ControlPlaneError::InvalidInput("frontstage_page_tab_title").into());
    }
    Ok(title)
}

fn root_slug_for(
    parent_id: Option<Uuid>,
    placement: domain::frontstage::FrontstageNavigationPlacement,
    slug: Option<String>,
) -> Result<Option<String>> {
    if parent_id.is_none() && placement == domain::frontstage::FrontstageNavigationPlacement::Topbar
    {
        return normalize_frontstage_slug(slug).and_then(|slug| {
            slug.ok_or_else(|| {
                ControlPlaneError::InvalidInput("frontstage_page_slug_required").into()
            })
            .map(Some)
        });
    }
    if slug.is_some() {
        return Err(ControlPlaneError::InvalidInput("frontstage_page_slug_not_allowed").into());
    }
    Ok(None)
}

pub struct FrontstagePageService<R> {
    repository: R,
}

impl<R> FrontstagePageService<R>
where
    R: FrontstagePageRepository,
{
    pub fn new(repository: R) -> Self {
        Self { repository }
    }

    pub async fn list_page_tree(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::FrontstagePageTreeNode>> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(actor_user_id, workspace_id)
            .await?;
        let pages = self.repository.list_frontstage_pages(workspace_id).await?;
        let visibility_rules = self
            .visibility_rules_for_actor(&actor, actor_user_id, workspace_id)
            .await?;

        Ok(build_visible_frontstage_page_tree(
            pages,
            &visibility_rules,
            &actor,
        ))
    }

    pub async fn create_group(
        &self,
        command: CreateFrontstageGroupCommand,
    ) -> Result<domain::FrontstagePageRecord> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;

        if let Some(parent_id) = command.parent_id {
            if command.placement != domain::frontstage::FrontstageNavigationPlacement::Sidebar {
                return Err(ControlPlaneError::InvalidInput("parent_id").into());
            }
            self.ensure_page_parent_placement(
                command.workspace_id,
                Some(parent_id),
                command.placement,
            )
            .await?;
        }

        let slug = root_slug_for(command.parent_id, command.placement, command.slug)?;
        let created = self
            .repository
            .create_frontstage_page(&CreateFrontstagePageInput {
                id: Uuid::now_v7(),
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                parent_id: command.parent_id,
                kind: domain::FrontstagePageKind::Group,
                title: command.title,
                icon: command.icon,
                tooltip: command.tooltip,
                placement: command.placement,
                content_presentation: domain::frontstage::FrontstagePageContentPresentation::Single,
                slug,
                rank: normalize_rank(command.rank),
                default_tab: None,
            })
            .await?;
        self.audit(&actor, &created.page, "frontstage.page_group_created")
            .await?;

        Ok(created.page)
    }

    pub async fn create_page(
        &self,
        command: CreateFrontstagePageCommand,
    ) -> Result<domain::frontstage::FrontstagePageCreation> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.ensure_page_parent_placement(
            command.workspace_id,
            command.parent_id,
            command.placement,
        )
        .await?;

        let slug = root_slug_for(command.parent_id, command.placement, command.slug)?;
        let page_id = Uuid::now_v7();
        let tab_id = Uuid::now_v7();
        let created = self
            .repository
            .create_frontstage_page(&CreateFrontstagePageInput {
                id: page_id,
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                parent_id: command.parent_id,
                kind: domain::FrontstagePageKind::Page,
                title: command.title,
                icon: command.icon,
                tooltip: command.tooltip,
                placement: command.placement,
                content_presentation: domain::frontstage::FrontstagePageContentPresentation::Single,
                slug,
                rank: normalize_rank(command.rank),
                default_tab: Some(CreateFrontstagePageTabInput {
                    id: tab_id,
                    workspace_id: command.workspace_id,
                    actor_user_id: command.actor_user_id,
                    page_id,
                    title: Some("Default".to_owned()),
                    rank: "a".to_owned(),
                    is_default: true,
                    route_segment: None,
                    document_root_uid: reserved_tab_document_root_uid(tab_id),
                }),
            })
            .await?;
        self.audit(&actor, &created.page, "frontstage.page_created")
            .await?;

        Ok(created)
    }

    pub async fn get_page_detail(
        &self,
        command: GetFrontstagePageDetailCommand,
    ) -> Result<domain::frontstage::FrontstagePageDetail> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;

        let detail = self
            .repository
            .get_frontstage_page_tab_detail(
                command.workspace_id,
                command.page_id,
                &command.tab_reference,
            )
            .await?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        ensure_page_record(&detail.page)?;
        self.ensure_page_visible(
            &actor,
            command.actor_user_id,
            command.workspace_id,
            command.page_id,
        )
        .await?;
        if !actor.is_root {
            let pages = self
                .repository
                .list_frontstage_pages(command.workspace_id)
                .await?;
            let rules = self
                .visibility_rules_for_actor(&actor, command.actor_user_id, command.workspace_id)
                .await?;
            if !FrontstagePageVisibilityContext::new(&pages, &rules)
                .is_tab_visible(command.page_id, detail.tab.id)
            {
                return Err(ControlPlaneError::NotFound("frontstage_page_tab").into());
            }
        }

        Ok(detail)
    }

    pub async fn update_metadata(
        &self,
        command: UpdateFrontstagePageMetadataCommand,
    ) -> Result<domain::FrontstagePageRecord> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;

        let existing = if command.placement.is_some()
            || command.content_presentation.is_some()
            || command.slug.is_some()
        {
            Some(
                self.repository
                    .get_frontstage_page(command.workspace_id, command.page_id)
                    .await?
                    .ok_or(ControlPlaneError::NotFound("frontstage_page"))?,
            )
        } else {
            None
        };

        if let Some(placement) = command.placement {
            let existing = existing
                .as_ref()
                .expect("existing page loaded for placement update");
            if placement != existing.placement {
                match existing.kind {
                    domain::FrontstagePageKind::Group => {
                        let has_children = self
                            .repository
                            .list_frontstage_pages(command.workspace_id)
                            .await?
                            .iter()
                            .any(|page| page.parent_id == Some(existing.id));
                        if has_children {
                            return Err(ControlPlaneError::InvalidInput(
                                "frontstage_group_placement_requires_empty_group",
                            )
                            .into());
                        }
                    }
                    domain::FrontstagePageKind::Page => {
                        self.ensure_page_parent_placement(
                            command.workspace_id,
                            existing.parent_id,
                            placement,
                        )
                        .await?;
                    }
                }
            }
        }

        if let Some(content_presentation) = command.content_presentation {
            let existing = existing
                .as_ref()
                .expect("existing page loaded for content presentation update");
            ensure_page_record(existing)?;
            if content_presentation == domain::frontstage::FrontstagePageContentPresentation::Single
                && existing.content_presentation
                    != domain::frontstage::FrontstagePageContentPresentation::Single
                && self
                    .repository
                    .list_frontstage_page_tabs(command.workspace_id, command.page_id)
                    .await?
                    .len()
                    > 1
            {
                return Err(ControlPlaneError::Conflict(
                    "frontstage_page_tabs_require_single_default",
                )
                .into());
            }
        }

        let updated = self
            .repository
            .update_frontstage_page_metadata(&UpdateFrontstagePageMetadataInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                title: command.title,
                icon: command.icon,
                tooltip: command.tooltip,
                is_hidden: command.is_hidden,
                placement: command.placement,
                content_presentation: command.content_presentation,
                slug: match command.slug {
                    Some(value) => {
                        let existing = existing
                            .as_ref()
                            .expect("existing page loaded for slug update");
                        Some(root_slug_for(
                            existing.parent_id,
                            command.placement.unwrap_or(existing.placement),
                            value,
                        )?)
                    }
                    None => None,
                },
            })
            .await?;
        self.audit(&actor, &updated, "frontstage.page_metadata_updated")
            .await?;

        Ok(updated)
    }

    pub async fn move_page(
        &self,
        command: MoveFrontstagePageCommand,
    ) -> Result<domain::FrontstagePageRecord> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;

        let existing = self
            .repository
            .get_frontstage_page(command.workspace_id, command.page_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        match existing.kind {
            domain::FrontstagePageKind::Group if command.parent_id.is_some() => {
                self.ensure_page_parent_placement(
                    command.workspace_id,
                    command.parent_id,
                    existing.placement,
                )
                .await?;
            }
            domain::FrontstagePageKind::Page => {
                self.ensure_page_parent_placement(
                    command.workspace_id,
                    command.parent_id,
                    existing.placement,
                )
                .await?;
            }
            domain::FrontstagePageKind::Group => {}
        }

        let moved = self
            .repository
            .move_frontstage_page(&MoveFrontstagePageInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                parent_id: command.parent_id,
                rank: normalize_rank(command.rank),
            })
            .await?;
        self.audit(&actor, &moved, "frontstage.page_moved").await?;

        Ok(moved)
    }

    pub async fn delete_page(&self, command: DeleteFrontstagePageCommand) -> Result<()> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;

        let existing = self
            .repository
            .get_frontstage_page(command.workspace_id, command.page_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        self.repository
            .delete_frontstage_page(command.workspace_id, command.page_id)
            .await?;
        self.audit(&actor, &existing, "frontstage.page_deleted")
            .await?;

        Ok(())
    }

    pub async fn list_page_tabs(
        &self,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageTabRecord>> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(actor_user_id, workspace_id)
            .await?;
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
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        let page = self
            .repository
            .get_frontstage_page(command.workspace_id, command.page_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        ensure_page_record(&page)?;
        if page.content_presentation
            != domain::frontstage::FrontstagePageContentPresentation::Tabs
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
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
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
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
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
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.ensure_existing_page(command.workspace_id, command.page_id)
            .await?;

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

    pub async fn get_block_code(
        &self,
        command: GetFrontstageBlockCodeCommand,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        self.ensure_page_visible(
            &actor,
            command.actor_user_id,
            command.workspace_id,
            command.page_id,
        )
        .await?;
        let code_ref = normalize_code_ref(command.code_ref)?;

        self.repository
            .get_frontstage_block_code(command.workspace_id, command.page_id, &code_ref)
            .await?
            .ok_or(ControlPlaneError::NotFound("frontstage_block_code").into())
    }

    pub async fn save_block_code(
        &self,
        command: SaveFrontstageBlockCodeCommand,
    ) -> Result<domain::frontstage::FrontstageBlockCodeRecord> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.ensure_existing_page(command.workspace_id, command.page_id)
            .await?;
        let code_ref = normalize_code_ref(command.code_ref)?;

        let saved = self
            .repository
            .save_frontstage_block_code(&SaveFrontstageBlockCodeInput {
                workspace_id: command.workspace_id,
                page_id: command.page_id,
                code_ref,
                code: command.code,
            })
            .await?;

        Ok(saved)
    }

    async fn ensure_page_parent_placement(
        &self,
        workspace_id: Uuid,
        parent_id: Option<Uuid>,
        placement: domain::frontstage::FrontstageNavigationPlacement,
    ) -> Result<()> {
        let Some(parent_id) = parent_id else {
            return Ok(());
        };

        let parent = self
            .repository
            .get_frontstage_page(workspace_id, parent_id)
            .await?
            .ok_or(ControlPlaneError::InvalidInput("parent_id"))?;

        if parent.kind != domain::FrontstagePageKind::Group {
            return Err(ControlPlaneError::InvalidInput("parent_id").into());
        }
        let placement_matches = parent.placement == placement
            || (parent.placement == domain::frontstage::FrontstageNavigationPlacement::Topbar
                && parent.parent_id.is_none()
                && placement == domain::frontstage::FrontstageNavigationPlacement::Sidebar);
        if !placement_matches {
            return Err(
                ControlPlaneError::InvalidInput("frontstage_page_placement_mismatch").into(),
            );
        }

        Ok(())
    }

    async fn ensure_existing_page(&self, workspace_id: Uuid, page_id: Uuid) -> Result<()> {
        let page = self
            .repository
            .get_frontstage_page(workspace_id, page_id)
            .await?
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        ensure_page_record(&page)
    }

    async fn ensure_page_visible(
        &self,
        actor: &domain::ActorContext,
        actor_user_id: Uuid,
        workspace_id: Uuid,
        page_id: Uuid,
    ) -> Result<()> {
        if actor.is_root {
            return Ok(());
        }

        let pages = self.repository.list_frontstage_pages(workspace_id).await?;
        let page = pages
            .iter()
            .find(|page| page.id == page_id)
            .ok_or(ControlPlaneError::NotFound("frontstage_page"))?;
        ensure_page_record(page)?;

        let visibility_rules = self
            .visibility_rules_for_actor(actor, actor_user_id, workspace_id)
            .await?;
        let visibility_context = FrontstagePageVisibilityContext::new(&pages, &visibility_rules);
        if visibility_context.is_visible(page_id) {
            return Ok(());
        }

        Err(ControlPlaneError::NotFound("frontstage_page").into())
    }

    async fn visibility_rules_for_actor(
        &self,
        actor: &domain::ActorContext,
        actor_user_id: Uuid,
        workspace_id: Uuid,
    ) -> Result<Vec<domain::frontstage::FrontstagePageVisibilityRuleRecord>> {
        if actor.is_root {
            return Ok(vec![]);
        }

        self.repository
            .list_frontstage_page_visibility_rules_for_actor_roles(actor_user_id, workspace_id)
            .await
    }

    async fn audit(
        &self,
        actor: &domain::ActorContext,
        page: &domain::FrontstagePageRecord,
        event_code: &'static str,
    ) -> Result<()> {
        self.repository
            .append_audit_log(&audit_log(
                Some(actor.current_workspace_id),
                Some(actor.user_id),
                "frontstage_page",
                Some(page.id),
                event_code,
                serde_json::json!({
                    "kind": page.kind.as_str(),
                    "title": page.title,
                    "parent_id": page.parent_id,
                }),
            ))
            .await
    }
}

fn ensure_design_permission(actor: &domain::ActorContext) -> Result<()> {
    ensure_permission(actor, "frontstage.page.design")
        .map_err(ControlPlaneError::PermissionDenied)?;
    Ok(())
}

fn ensure_page_record(page: &domain::FrontstagePageRecord) -> Result<()> {
    if page.kind != domain::FrontstagePageKind::Page {
        return Err(ControlPlaneError::NotFound("frontstage_page").into());
    }

    Ok(())
}

fn normalize_code_ref(code_ref: String) -> Result<String> {
    let trimmed = code_ref.trim();
    if trimmed.is_empty() || trimmed.len() > 200 {
        return Err(ControlPlaneError::InvalidInput("code_ref").into());
    }

    Ok(trimmed.to_owned())
}

fn normalize_rank(rank: Option<String>) -> String {
    rank.unwrap_or_default()
}

fn reserved_tab_document_root_uid(tab_id: Uuid) -> String {
    format!("frontstage.tab.{tab_id}.root")
}

fn build_visible_frontstage_page_tree(
    records: Vec<domain::FrontstagePageRecord>,
    visibility_rules: &[domain::frontstage::FrontstagePageVisibilityRuleRecord],
    actor: &domain::ActorContext,
) -> Vec<domain::FrontstagePageTreeNode> {
    if actor.is_root {
        return build_frontstage_page_tree(records);
    }

    let visibility_context = FrontstagePageVisibilityContext::new(&records, visibility_rules);
    let mut children_by_parent: HashMap<Uuid, Vec<Uuid>> = HashMap::new();
    for record in &records {
        if let Some(parent_id) = visibility_context.parent_id(record.id) {
            children_by_parent
                .entry(parent_id)
                .or_default()
                .push(record.id);
        }
    }

    let visible_record_ids = records
        .iter()
        .filter(|record| visibility_context.is_visible(record.id))
        .map(|record| record.id)
        .collect::<HashSet<_>>();
    let visible_page_ids = records
        .iter()
        .filter(|record| {
            record.kind == domain::FrontstagePageKind::Page
                && visible_record_ids.contains(&record.id)
        })
        .map(|record| record.id)
        .collect::<HashSet<_>>();

    let mut descendant_visibility_cache = HashMap::new();
    let retained = records
        .into_iter()
        .filter(|record| {
            if record.kind == domain::FrontstagePageKind::Page {
                return visible_page_ids.contains(&record.id);
            }

            visible_record_ids.contains(&record.id)
                || has_visible_frontstage_descendant(
                    record.id,
                    &children_by_parent,
                    &visible_page_ids,
                    &mut descendant_visibility_cache,
                    &mut HashSet::new(),
                )
        })
        .collect::<Vec<_>>();

    build_frontstage_page_tree(retained)
}

struct FrontstagePageVisibilityContext {
    parent_by_id: HashMap<Uuid, Option<Uuid>>,
    role_ids: HashSet<Uuid>,
    visibility_by_page_and_role:
        HashMap<(Option<Uuid>, Uuid), domain::frontstage::FrontstagePageVisibility>,
    visibility_by_tab_and_role: HashMap<(Uuid, Uuid), domain::frontstage::FrontstagePageVisibility>,
}

impl FrontstagePageVisibilityContext {
    fn new(
        records: &[domain::FrontstagePageRecord],
        visibility_rules: &[domain::frontstage::FrontstagePageVisibilityRuleRecord],
    ) -> Self {
        let existing_ids = records
            .iter()
            .map(|record| record.id)
            .collect::<HashSet<_>>();
        let parent_by_id = records
            .iter()
            .map(|record| {
                let parent_id = record
                    .parent_id
                    .filter(|parent_id| existing_ids.contains(parent_id));
                (record.id, parent_id)
            })
            .collect::<HashMap<_, _>>();
        let role_ids = visibility_rules
            .iter()
            .map(|rule| rule.role_id)
            .collect::<HashSet<_>>();
        let visibility_by_page_and_role = visibility_rules
            .iter()
            .filter(|rule| rule.tab_id.is_none())
            .map(|rule| ((rule.page_id, rule.role_id), rule.visibility))
            .collect::<HashMap<_, _>>();
        let visibility_by_tab_and_role = visibility_rules
            .iter()
            .filter_map(|rule| {
                rule.tab_id
                    .map(|tab_id| ((tab_id, rule.role_id), rule.visibility))
            })
            .collect::<HashMap<_, _>>();

        Self {
            parent_by_id,
            role_ids,
            visibility_by_page_and_role,
            visibility_by_tab_and_role,
        }
    }

    fn parent_id(&self, page_id: Uuid) -> Option<Uuid> {
        self.parent_by_id.get(&page_id).copied().flatten()
    }

    fn is_visible(&self, page_id: Uuid) -> bool {
        self.role_ids
            .iter()
            .any(|role_id| self.has_visible_ancestor_chain(page_id, *role_id))
    }

    fn is_tab_visible(&self, page_id: Uuid, tab_id: Uuid) -> bool {
        self.role_ids.iter().any(|role_id| {
            self.has_visible_ancestor_chain(page_id, *role_id)
                && self.visibility_by_tab_and_role.get(&(tab_id, *role_id))
                    == Some(&domain::frontstage::FrontstagePageVisibility::Visible)
        })
    }

    fn has_visible_ancestor_chain(&self, page_id: Uuid, role_id: Uuid) -> bool {
        let mut current_id = Some(page_id);
        let mut visited = HashSet::new();

        while let Some(page_id) = current_id {
            if !visited.insert(page_id) {
                return false;
            }

            if self
                .visibility_by_page_and_role
                .get(&(Some(page_id), role_id))
                != Some(&domain::frontstage::FrontstagePageVisibility::Visible)
            {
                return false;
            }

            current_id = self.parent_id(page_id);
        }

        true
    }
}

fn has_visible_frontstage_descendant(
    group_id: Uuid,
    children_by_parent: &HashMap<Uuid, Vec<Uuid>>,
    visible_page_ids: &HashSet<Uuid>,
    descendant_visibility_cache: &mut HashMap<Uuid, bool>,
    visiting_groups: &mut HashSet<Uuid>,
) -> bool {
    if let Some(has_visible_descendant) = descendant_visibility_cache.get(&group_id) {
        return *has_visible_descendant;
    }

    if !visiting_groups.insert(group_id) {
        return false;
    }

    let has_visible_descendant = children_by_parent
        .get(&group_id)
        .map(|children| {
            children.iter().any(|child_id| {
                visible_page_ids.contains(child_id)
                    || has_visible_frontstage_descendant(
                        *child_id,
                        children_by_parent,
                        visible_page_ids,
                        descendant_visibility_cache,
                        visiting_groups,
                    )
            })
        })
        .unwrap_or(false);

    visiting_groups.remove(&group_id);
    descendant_visibility_cache.insert(group_id, has_visible_descendant);
    has_visible_descendant
}

fn build_frontstage_page_tree(
    mut records: Vec<domain::FrontstagePageRecord>,
) -> Vec<domain::FrontstagePageTreeNode> {
    let existing_ids = records
        .iter()
        .map(|record| record.id)
        .collect::<HashSet<_>>();

    for record in &mut records {
        if !matches!(record.parent_id, Some(parent_id) if existing_ids.contains(&parent_id)) {
            record.parent_id = None;
        }
    }

    records.sort_by(compare_frontstage_pages);

    let mut nodes_by_parent: HashMap<Option<Uuid>, Vec<domain::FrontstagePageRecord>> =
        HashMap::new();
    for record in records {
        nodes_by_parent
            .entry(record.parent_id)
            .or_default()
            .push(record);
    }

    fn flatten_group_children(
        group_id: Uuid,
        nodes_by_parent: &HashMap<Option<Uuid>, Vec<domain::FrontstagePageRecord>>,
        visiting_groups: &mut HashSet<Uuid>,
    ) -> Vec<domain::FrontstagePageTreeNode> {
        if !visiting_groups.insert(group_id) {
            return vec![];
        }

        let mut output = vec![];
        if let Some(children) = nodes_by_parent.get(&Some(group_id)) {
            output.reserve(children.len());
            for child in children {
                if child.kind == domain::FrontstagePageKind::Page {
                    output.push(domain::FrontstagePageTreeNode {
                        page: child.clone(),
                        children: vec![],
                    });
                    continue;
                }

                output.extend(flatten_group_children(
                    child.id,
                    nodes_by_parent,
                    visiting_groups,
                ));
            }
        }

        visiting_groups.remove(&group_id);
        output
    }

    nodes_by_parent
        .remove(&None)
        .unwrap_or_default()
        .into_iter()
        .map(|record| {
            let children = if record.kind != domain::FrontstagePageKind::Group {
                vec![]
            } else if record.placement == domain::frontstage::FrontstageNavigationPlacement::Topbar
            {
                nodes_by_parent
                    .get(&Some(record.id))
                    .cloned()
                    .unwrap_or_default()
                    .into_iter()
                    .map(|child| {
                        let children = if child.kind == domain::FrontstagePageKind::Group {
                            flatten_group_children(child.id, &nodes_by_parent, &mut HashSet::new())
                        } else {
                            vec![]
                        };
                        domain::FrontstagePageTreeNode {
                            page: child,
                            children,
                        }
                    })
                    .collect()
            } else {
                flatten_group_children(record.id, &nodes_by_parent, &mut HashSet::new())
            };

            domain::FrontstagePageTreeNode {
                page: record,
                children,
            }
        })
        .collect()
}

fn compare_frontstage_pages(
    left: &domain::FrontstagePageRecord,
    right: &domain::FrontstagePageRecord,
) -> Ordering {
    let parent_cmp = left.parent_id.cmp(&right.parent_id);
    if parent_cmp != Ordering::Equal {
        return parent_cmp;
    }

    let rank_cmp = left.rank.cmp(&right.rank);
    if rank_cmp != Ordering::Equal {
        return rank_cmp;
    }

    left.id.cmp(&right.id)
}

#[cfg(test)]
mod tests {
    use super::*;
    use domain::FrontstagePageKind;
    use time::OffsetDateTime;

    fn test_uuid(value: u128) -> Uuid {
        Uuid::from_u128(value)
    }

    fn page_record(
        id: u128,
        kind: FrontstagePageKind,
        parent_id: Option<Uuid>,
        rank: &str,
    ) -> domain::FrontstagePageRecord {
        domain::FrontstagePageRecord {
            id: test_uuid(id),
            workspace_id: test_uuid(0x100),
            parent_id,
            kind,
            title: None,
            icon: None,
            tooltip: None,
            is_hidden: false,
            placement: domain::frontstage::FrontstageNavigationPlacement::Sidebar,
            content_presentation: domain::frontstage::FrontstagePageContentPresentation::Single,
            slug: None,
            rank: rank.to_owned(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }

    #[test]
    fn ac_003_normalizes_and_rejects_invalid_frontstage_slugs() {
        assert_eq!(
            normalize_frontstage_slug(Some("  Marketing-01  ".to_owned())).unwrap(),
            Some("marketing-01".to_owned())
        );
        assert!(normalize_frontstage_slug(Some("api".to_owned())).is_err());
        assert!(normalize_frontstage_slug(Some("bad--slug".to_owned())).is_err());
        assert!(normalize_frontstage_slug(Some("abc".to_owned())).is_err());
    }

    #[test]
    fn ac_003_requires_slug_only_for_topbar_roots() {
        assert!(root_slug_for(
            None,
            domain::frontstage::FrontstageNavigationPlacement::Topbar,
            None
        )
        .is_err());
        assert_eq!(
            root_slug_for(
                None,
                domain::frontstage::FrontstageNavigationPlacement::Topbar,
                Some("space-01".to_owned())
            )
            .unwrap(),
            Some("space-01".to_owned())
        );
        assert_eq!(
            root_slug_for(
                Some(Uuid::from_u128(1)),
                domain::frontstage::FrontstageNavigationPlacement::Sidebar,
                None
            )
            .unwrap(),
            None
        );
    }

    #[test]
    fn build_frontstage_page_tree_promotes_missing_parent_records_to_root() {
        let orphan_group_id = test_uuid(0x10);
        let orphan_page_id = test_uuid(0x20);
        let root_group_id = test_uuid(0x30);
        let child_page_id = test_uuid(0x40);
        let missing_parent_id = test_uuid(0x999);

        let tree = build_frontstage_page_tree(vec![
            page_record(0x20, FrontstagePageKind::Page, Some(missing_parent_id), "b"),
            page_record(0x30, FrontstagePageKind::Group, None, "c"),
            page_record(0x40, FrontstagePageKind::Page, Some(root_group_id), "a"),
            page_record(
                0x10,
                FrontstagePageKind::Group,
                Some(missing_parent_id),
                "a",
            ),
        ]);

        let root_ids = tree.iter().map(|node| node.page.id).collect::<Vec<_>>();
        assert_eq!(
            root_ids,
            vec![orphan_group_id, orphan_page_id, root_group_id]
        );
        assert_eq!(tree[0].page.parent_id, None);
        assert_eq!(tree[1].page.parent_id, None);
        assert!(tree[0].children.is_empty());
        assert!(tree[1].children.is_empty());
        assert_eq!(
            tree[2]
                .children
                .iter()
                .map(|node| node.page.id)
                .collect::<Vec<_>>(),
            vec![child_page_id]
        );
    }

    #[test]
    fn ac_007_topbar_space_preserves_direct_sidebar_groups() {
        let space_id = test_uuid(0x10);
        let group_id = test_uuid(0x20);
        let page_id = test_uuid(0x30);
        let mut space = page_record(0x10, FrontstagePageKind::Group, None, "a");
        space.placement = domain::frontstage::FrontstageNavigationPlacement::Topbar;
        space.slug = Some("space-01".to_owned());
        let group = page_record(0x20, FrontstagePageKind::Group, Some(space_id), "a");
        let page = page_record(0x30, FrontstagePageKind::Page, Some(group_id), "a");

        let tree = build_frontstage_page_tree(vec![space, group, page]);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].page.id, space_id);
        assert_eq!(tree[0].children.len(), 1);
        assert_eq!(tree[0].children[0].page.id, group_id);
        assert_eq!(tree[0].children[0].children[0].page.id, page_id);
    }

    #[test]
    fn build_frontstage_page_tree_flattens_nested_groups_and_ignores_reentrant_group_edges() {
        let root_group_id = test_uuid(0x10);
        let nested_page_id = test_uuid(0x30);

        let tree = build_frontstage_page_tree(vec![
            page_record(0x10, FrontstagePageKind::Group, None, "a"),
            page_record(0x10, FrontstagePageKind::Group, Some(root_group_id), "a"),
            page_record(0x20, FrontstagePageKind::Group, Some(root_group_id), "b"),
            page_record(0x30, FrontstagePageKind::Page, Some(test_uuid(0x20)), "a"),
        ]);

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].page.id, root_group_id);
        assert_eq!(
            tree[0]
                .children
                .iter()
                .map(|node| (node.page.id, node.page.kind))
                .collect::<Vec<_>>(),
            vec![(nested_page_id, FrontstagePageKind::Page)]
        );
        assert!(tree[0].children.iter().all(|node| node.children.is_empty()));
    }

    #[test]
    fn build_visible_frontstage_page_tree_allows_root_without_rules() {
        let page_id = test_uuid(0x10);
        let pages = vec![page_record(0x10, FrontstagePageKind::Page, None, "a")];

        let tree = build_visible_frontstage_page_tree(
            pages,
            &[],
            &domain::ActorContext::root(test_uuid(0x01), test_uuid(0x100), "root"),
        );

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].page.id, page_id);
    }

    #[test]
    fn build_visible_frontstage_page_tree_ignores_legacy_root_rule() {
        let role_id = test_uuid(0x200);
        let page_id = test_uuid(0x10);
        let pages = vec![page_record(0x10, FrontstagePageKind::Page, None, "a")];
        let rules = vec![visibility_rule(
            None,
            role_id,
            domain::frontstage::FrontstagePageVisibility::Visible,
        )];
        let actor = domain::ActorContext::scoped(
            test_uuid(0x01),
            test_uuid(0x100),
            "viewer",
            Vec::<String>::new(),
        );

        let tree = build_visible_frontstage_page_tree(pages, &rules, &actor);

        assert!(tree.is_empty());
        assert!(!format!("{tree:?}").contains(&page_id.to_string()));
    }

    #[test]
    fn build_visible_frontstage_page_tree_requires_visible_ancestor_chain() {
        let role_id = test_uuid(0x200);
        let hidden_group_id = test_uuid(0x10);
        let visible_page_id = test_uuid(0x20);
        let hidden_page_id = test_uuid(0x30);
        let pages = vec![
            page_record(0x10, FrontstagePageKind::Group, None, "a"),
            page_record(0x20, FrontstagePageKind::Page, Some(hidden_group_id), "a"),
            page_record(0x30, FrontstagePageKind::Page, Some(hidden_group_id), "b"),
        ];
        let rules = vec![
            visibility_rule(
                None,
                role_id,
                domain::frontstage::FrontstagePageVisibility::Visible,
            ),
            visibility_rule(
                Some(hidden_group_id),
                role_id,
                domain::frontstage::FrontstagePageVisibility::Hidden,
            ),
            visibility_rule(
                Some(visible_page_id),
                role_id,
                domain::frontstage::FrontstagePageVisibility::Visible,
            ),
        ];
        let actor = domain::ActorContext::scoped(
            test_uuid(0x01),
            test_uuid(0x100),
            "viewer",
            Vec::<String>::new(),
        );

        let tree = build_visible_frontstage_page_tree(pages, &rules, &actor);

        assert!(tree.is_empty());
        assert!(!format!("{tree:?}").contains(&visible_page_id.to_string()));
        assert!(!format!("{tree:?}").contains(&hidden_page_id.to_string()));
    }

    fn visibility_rule(
        page_id: Option<Uuid>,
        role_id: Uuid,
        visibility: domain::frontstage::FrontstagePageVisibility,
    ) -> domain::frontstage::FrontstagePageVisibilityRuleRecord {
        domain::frontstage::FrontstagePageVisibilityRuleRecord {
            id: Uuid::now_v7(),
            workspace_id: test_uuid(0x100),
            page_id,
            tab_id: None,
            role_id,
            visibility,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}
