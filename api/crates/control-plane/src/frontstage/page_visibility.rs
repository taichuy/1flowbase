use super::*;

pub(super) fn build_visible_frontstage_page_tree(
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

pub(super) struct FrontstagePageVisibilityContext {
    pub(super) parent_by_id: HashMap<Uuid, Option<Uuid>>,
    pub(super) group_ids: HashSet<Uuid>,
    pub(super) role_ids: HashSet<Uuid>,
    pub(super) visibility_by_page_and_role:
        HashMap<(Option<Uuid>, Uuid), domain::frontstage::FrontstagePageVisibility>,
    pub(super) visibility_by_tab_and_role:
        HashMap<(Uuid, Uuid), domain::frontstage::FrontstagePageVisibility>,
}

impl FrontstagePageVisibilityContext {
    pub(super) fn new(
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
            group_ids: records
                .iter()
                .filter(|record| record.kind == domain::FrontstagePageKind::Group)
                .map(|record| record.id)
                .collect(),
            role_ids,
            visibility_by_page_and_role,
            visibility_by_tab_and_role,
        }
    }

    pub(super) fn parent_id(&self, page_id: Uuid) -> Option<Uuid> {
        self.parent_by_id.get(&page_id).copied().flatten()
    }

    pub(super) fn is_visible(&self, page_id: Uuid) -> bool {
        self.role_ids
            .iter()
            .any(|role_id| self.has_visible_ancestor_chain(page_id, *role_id))
    }

    pub(super) fn is_tab_visible(&self, page_id: Uuid, tab_id: Uuid) -> bool {
        self.role_ids.iter().any(|role_id| {
            self.has_visible_ancestor_chain(page_id, *role_id)
                && self.visibility_by_tab_and_role.get(&(tab_id, *role_id))
                    == Some(&domain::frontstage::FrontstagePageVisibility::Visible)
        })
    }

    pub(super) fn has_visible_ancestor_chain(&self, target_id: Uuid, role_id: Uuid) -> bool {
        let mut current_id = Some(target_id);
        let mut visited = HashSet::new();

        while let Some(page_id) = current_id {
            if !visited.insert(page_id) {
                return false;
            }

            let visibility = self
                .visibility_by_page_and_role
                .get(&(Some(page_id), role_id));
            // Directory reachability follows authorized descendants. An explicit denial still
            // blocks traversal, while pages always require their own grant.
            if visibility != Some(&domain::frontstage::FrontstagePageVisibility::Visible)
                && !(page_id != target_id
                    && visibility.is_none()
                    && self.group_ids.contains(&page_id))
            {
                return false;
            }

            current_id = self.parent_id(page_id);
        }

        true
    }
}

pub(super) fn has_visible_frontstage_descendant(
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

pub(super) fn build_frontstage_page_tree(
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

    fn build_children(
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
                if visiting_groups.contains(&child.id) {
                    continue;
                }
                let descendants = if child.kind == domain::FrontstagePageKind::Group {
                    build_children(child.id, nodes_by_parent, visiting_groups)
                } else {
                    vec![]
                };
                output.push(domain::FrontstagePageTreeNode {
                    page: child.clone(),
                    children: descendants,
                });
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
                build_children(record.id, &nodes_by_parent, &mut HashSet::new())
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

pub(super) fn compare_frontstage_pages(
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
