use super::*;

impl PgControlPlaneStore {
    pub(super) async fn get_frontstage_block_node_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<Option<domain::FrontstageBlockNodeRecord>> {
        let row = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            select {DETAIL_COLUMNS}, node.input_mapping, node.output_mapping, node.runtime_descriptor
            from frontstage_block_nodes node
            left join frontstage_block_nodes parent
              on parent.scope_id = node.scope_id
             and parent.tree_partition_id = node.tree_partition_id
             and parent.id = node.parent_id
            where node.scope_id = $1 and node.tree_partition_id = $2 and node.block_id = $3
            "#,
        )))
        .bind(workspace_id)
        .bind(page_id)
        .bind(block_id)
        .fetch_optional(self.pool())
        .await?;
        row.as_ref().map(map_record).transpose()
    }

    pub(super) async fn get_frontstage_block_runtime_assembly_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<Vec<domain::frontstage::FrontstageBlockRuntimeLayer>> {
        // A single bounded recursive read owns ancestor ordering and source resolution so
        // runtime callers never fan out into one descriptor/code query per tree layer.
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            with recursive ancestry as (
                select target.id, target.parent_id, 0::integer as depth, array[target.id] as path
                from frontstage_block_nodes target
                where target.scope_id = $1
                  and target.tree_partition_id = $2
                  and target.block_id = $3

                union all

                select parent_node.id,
                       parent_node.parent_id,
                       ancestry.depth + 1,
                       ancestry.path || parent_node.id
                from ancestry
                join frontstage_block_nodes parent_node
                  on parent_node.scope_id = $1
                 and parent_node.tree_partition_id = $2
                 and parent_node.id = ancestry.parent_id
                where ancestry.depth < 63
                  and not parent_node.id = any(ancestry.path)
            )
            select {DETAIL_COLUMNS}, node.input_mapping, node.output_mapping,
                   node.runtime_descriptor, source.source_sha256 as source_revision
            from ancestry
            join frontstage_block_nodes node
              on node.scope_id = $1
             and node.tree_partition_id = $2
             and node.id = ancestry.id
            left join frontstage_block_nodes parent
              on parent.scope_id = node.scope_id
             and parent.tree_partition_id = node.tree_partition_id
             and parent.id = node.parent_id
            join frontstage_block_codes source
              on source.workspace_id = node.scope_id
             and source.page_id = node.tree_partition_id
             and source.code_ref = node.code_ref
            order by ancestry.depth desc
            "#,
        )))
        .bind(workspace_id)
        .bind(page_id)
        .bind(block_id)
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_runtime_layer).collect()
    }

    pub(super) async fn list_frontstage_block_roots_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_id: Uuid,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockNodeRecord>> {
        let rows = sqlx::query(sqlx::AssertSqlSafe(format!(
            r#"
            select {DETAIL_COLUMNS}, node.input_mapping, node.output_mapping, node.runtime_descriptor
            from frontstage_block_nodes node
            left join frontstage_block_nodes parent
              on parent.scope_id = node.scope_id
             and parent.tree_partition_id = node.tree_partition_id
             and parent.id = node.parent_id
            where node.scope_id = $1
              and node.tree_partition_id = $2
              and node.tab_id = $3
              and node.parent_id is null
            order by node.sibling_rank collate "C", node.id
            limit $4
            "#,
        )))
        .bind(workspace_id)
        .bind(page_id)
        .bind(tab_id)
        .bind(i64::from(limit))
        .fetch_all(self.pool())
        .await?;
        rows.iter().map(map_record).collect()
    }

    pub(super) async fn list_frontstage_block_children_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        parent_block_id: &str,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let parent_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            parent_block_id,
            OrderedTreeQueryError::ParentNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::list_ordered_tree_children(
            self,
            &metadata,
            OrderedTreeChildrenInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                parent_id,
                result_limit: limit,
                cursor: None,
            },
        )
        .await?
        .items;
        map_node_records(
            self,
            workspace_id,
            page_id,
            projections.into_iter().map(|projection| projection.record),
        )
        .await
    }

    pub(super) async fn list_frontstage_block_ancestors_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<Vec<domain::FrontstageBlockNodeSummary>> {
        let node_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            block_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::list_ordered_tree_ancestors(
            self,
            &metadata,
            OrderedTreeNodeInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                node_id,
            },
        )
        .await?;
        map_node_records(
            self,
            workspace_id,
            page_id,
            projections.into_iter().map(|projection| projection.record),
        )
        .await
    }

    pub(super) async fn list_frontstage_block_descendants_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
        max_depth: u32,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockDescendantProjection>> {
        let node_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            block_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::list_ordered_tree_descendants(
            self,
            &metadata,
            OrderedTreeDescendantsInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                node_id,
                max_depth: Some(max_depth),
                result_limit: limit,
                include_path: true,
                cursor: None,
            },
        )
        .await?
        .items;
        let summaries = projections
            .iter()
            .map(|projection| decode_summary(&projection.record))
            .collect::<Result<Vec<_>>>()?;
        let public_ids = map_public_ids(
            self,
            workspace_id,
            page_id,
            summaries
                .iter()
                .flat_map(|summary| [Some(summary.internal_id), summary.parent_internal_id])
                .flatten()
                .chain(
                    projections
                        .iter()
                        .flat_map(|projection| projection.path.iter().flatten().copied()),
                )
                .collect(),
        )
        .await?;
        projections
            .into_iter()
            .zip(summaries)
            .map(|(projection, summary)| {
                let path = projection
                    .path
                    .ok_or_else(|| anyhow!("ordered-tree descendant path was not projected"))?
                    .into_iter()
                    .map(|internal_id| required_public_id(&public_ids, internal_id).cloned())
                    .collect::<Result<Vec<_>>>()?;
                Ok(domain::FrontstageBlockDescendantProjection {
                    node: summary.into_node(&public_ids)?,
                    depth: projection.depth,
                    has_children: projection.has_children,
                    path,
                })
            })
            .collect()
    }

    pub(super) async fn search_frontstage_blocks_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        tab_id: Uuid,
        query: &str,
        limit: u32,
    ) -> Result<Vec<domain::FrontstageBlockSearchResult>> {
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let projections = OrderedTreeQueryRepository::search_ordered_tree_prefix(
            self,
            &metadata,
            OrderedTreeSearchInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                prefix: query.to_owned(),
                match_limit: limit,
                cursor: None,
            },
        )
        .await?
        .items;
        let summaries = projections
            .iter()
            .map(|projection| decode_summary(&projection.record))
            .collect::<Result<Vec<_>>>()?;
        let public_ids = map_public_ids(
            self,
            workspace_id,
            page_id,
            summaries
                .iter()
                .flat_map(|summary| [Some(summary.internal_id), summary.parent_internal_id])
                .flatten()
                .collect(),
        )
        .await?;
        let by_internal_id = summaries
            .iter()
            .cloned()
            .map(|summary| (summary.internal_id, summary))
            .collect::<HashMap<_, _>>();
        projections
            .into_iter()
            .zip(summaries)
            .filter(|(projection, summary)| projection.is_match && summary.tab_id == tab_id)
            .map(|(_, summary)| {
                let mut ancestors = Vec::new();
                let mut current = summary.parent_internal_id;
                let mut visited = HashSet::new();
                while let Some(internal_id) = current {
                    if !visited.insert(internal_id) {
                        return Err(anyhow!(
                            "frontstage block search ancestor context contains a cycle"
                        ));
                    }
                    let ancestor = by_internal_id.get(&internal_id).ok_or_else(|| {
                        anyhow!(
                            "frontstage block search is missing projected ancestor `{internal_id}`"
                        )
                    })?;
                    current = ancestor.parent_internal_id;
                    ancestors.push(ancestor.clone().into_node(&public_ids)?);
                }
                ancestors.reverse();
                Ok(domain::FrontstageBlockSearchResult {
                    node: summary.into_node(&public_ids)?,
                    ancestors,
                })
            })
            .collect()
    }

    pub(super) async fn get_frontstage_block_subtree_impact_impl(
        &self,
        workspace_id: Uuid,
        page_id: Uuid,
        block_id: &str,
    ) -> Result<domain::FrontstageBlockSubtreeImpact> {
        let node_id = resolve_query_node_id(
            self,
            workspace_id,
            page_id,
            block_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        let metadata = frontstage_block_summary_metadata(workspace_id);
        let result = OrderedTreeQueryRepository::get_ordered_tree_subtree_impact(
            self,
            &metadata,
            OrderedTreeSubtreeImpactInput {
                scope_id: workspace_id,
                tree_partition_id: page_id,
                node_id,
            },
        )
        .await?;
        Ok(domain::FrontstageBlockSubtreeImpact {
            affected_count: result.affected_count,
        })
    }
}
