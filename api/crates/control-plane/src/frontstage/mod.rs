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
        CreateFrontstagePageInput, FrontstagePageRepository, MoveFrontstagePageInput,
        SaveFrontstageBlockCodeInput, SaveFrontstagePageContentInput,
        UpdateFrontstagePageMetadataInput,
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
}

pub struct CreateFrontstagePageCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub title: Option<String>,
    pub icon: Option<String>,
    pub tooltip: Option<String>,
    pub parent_id: Option<Uuid>,
    pub rank: Option<String>,
}

pub struct UpdateFrontstagePageMetadataCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub title: Option<Option<String>>,
    pub icon: Option<Option<String>>,
    pub tooltip: Option<Option<String>>,
    pub is_hidden: Option<bool>,
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
}

pub struct GetFrontstageBlockCodeCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub code_ref: String,
}

pub struct SaveFrontstagePageContentCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub schema_payload: serde_json::Value,
    pub root_payload: serde_json::Value,
}

pub struct SaveFrontstageBlockCodeCommand {
    pub actor_user_id: Uuid,
    pub workspace_id: Uuid,
    pub page_id: Uuid,
    pub code_ref: String,
    pub code: String,
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

        if command.parent_id.is_some() {
            return Err(ControlPlaneError::InvalidInput("parent_id").into());
        }

        let created = self
            .repository
            .create_frontstage_page(&CreateFrontstagePageInput {
                id: Uuid::now_v7(),
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                parent_id: None,
                kind: domain::FrontstagePageKind::Group,
                title: command.title,
                icon: command.icon,
                tooltip: command.tooltip,
                rank: normalize_rank(command.rank),
                schema_root_uid: None,
            })
            .await?;
        self.audit(&actor, &created, "frontstage.page_group_created")
            .await?;

        Ok(created)
    }

    pub async fn create_page(
        &self,
        command: CreateFrontstagePageCommand,
    ) -> Result<domain::FrontstagePageRecord> {
        let actor = self
            .repository
            .load_actor_context_for_workspace(command.actor_user_id, command.workspace_id)
            .await?;
        ensure_design_permission(&actor)?;
        self.ensure_page_parent(command.workspace_id, command.parent_id)
            .await?;

        let page_id = Uuid::now_v7();
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
                rank: normalize_rank(command.rank),
                schema_root_uid: Some(reserved_schema_root_uid(page_id)),
            })
            .await?;
        self.audit(&actor, &created, "frontstage.page_created")
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
            .get_frontstage_page_detail(command.workspace_id, command.page_id)
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
                return Err(ControlPlaneError::InvalidInput("parent_id").into());
            }
            domain::FrontstagePageKind::Page => {
                self.ensure_page_parent(command.workspace_id, command.parent_id)
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

    pub async fn save_page_content(
        &self,
        command: SaveFrontstagePageContentCommand,
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
            .save_frontstage_page_content(&SaveFrontstagePageContentInput {
                workspace_id: command.workspace_id,
                actor_user_id: command.actor_user_id,
                page_id: command.page_id,
                schema_payload: command.schema_payload,
                root_payload: command.root_payload,
            })
            .await?;
        self.audit(&actor, &detail.page, "frontstage.page_content_saved")
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

    async fn ensure_page_parent(&self, workspace_id: Uuid, parent_id: Option<Uuid>) -> Result<()> {
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

        Err(ControlPlaneError::PermissionDenied("frontstage_page_visibility").into())
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

fn reserved_schema_root_uid(page_id: Uuid) -> String {
    format!("frontstage_page_schema_root:{page_id}")
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
            .map(|rule| ((rule.page_id, rule.role_id), rule.visibility))
            .collect::<HashMap<_, _>>();

        Self {
            parent_by_id,
            role_ids,
            visibility_by_page_and_role,
        }
    }

    fn parent_id(&self, page_id: Uuid) -> Option<Uuid> {
        self.parent_by_id.get(&page_id).copied().flatten()
    }

    fn is_visible(&self, page_id: Uuid) -> bool {
        if self.role_ids.is_empty() {
            return false;
        }

        self.role_ids.iter().any(|role_id| {
            self.nearest_visibility(page_id, *role_id)
                == Some(domain::frontstage::FrontstagePageVisibility::Visible)
        })
    }

    fn nearest_visibility(
        &self,
        page_id: Uuid,
        role_id: Uuid,
    ) -> Option<domain::frontstage::FrontstagePageVisibility> {
        let mut current_id = Some(page_id);
        let mut visited = HashSet::new();

        while let Some(page_id) = current_id {
            if !visited.insert(page_id) {
                break;
            }

            if let Some(visibility) = self
                .visibility_by_page_and_role
                .get(&(Some(page_id), role_id))
            {
                return Some(*visibility);
            }

            current_id = self.parent_id(page_id);
        }

        self.visibility_by_page_and_role
            .get(&(None, role_id))
            .copied()
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
            let children = if record.kind == domain::FrontstagePageKind::Group {
                flatten_group_children(record.id, &nodes_by_parent, &mut HashSet::new())
            } else {
                vec![]
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
            slug: None,
            schema_root_uid: (kind == FrontstagePageKind::Page)
                .then(|| format!("schema-root:{id}")),
            rank: rank.to_owned(),
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
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
    fn build_visible_frontstage_page_tree_applies_root_rule_to_pages() {
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

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].page.id, page_id);
    }

    #[test]
    fn build_visible_frontstage_page_tree_uses_nearest_ancestor_rule() {
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

        assert_eq!(tree.len(), 1);
        assert_eq!(tree[0].page.id, hidden_group_id);
        assert_eq!(
            tree[0]
                .children
                .iter()
                .map(|node| node.page.id)
                .collect::<Vec<_>>(),
            vec![visible_page_id]
        );
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
            role_id,
            visibility,
            created_at: OffsetDateTime::UNIX_EPOCH,
            updated_at: OffsetDateTime::UNIX_EPOCH,
        }
    }
}
