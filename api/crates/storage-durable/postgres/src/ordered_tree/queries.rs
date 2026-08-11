use std::collections::HashMap;

use anyhow::Result;
use async_trait::async_trait;
use runtime_core::{
    model_metadata::ModelMetadata,
    runtime_record_repository::{
        OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreeDescendantProjection,
        OrderedTreeDescendantsInput, OrderedTreeNodeInput, OrderedTreeNodeProjection,
        OrderedTreeQueryError, OrderedTreeQueryRepository, OrderedTreeSearchInput,
        OrderedTreeSearchProjection,
    },
};
use serde_json::Value;
use sqlx::{PgPool, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::{
    repositories::PgControlPlaneStore,
    runtime_record_repository::{normalize_record, projected_select_list, quote_identifier},
};

const TEMPLATE_PROVIDER: &str = "core";
const TEMPLATE_CODE: &str = "ordered_tree";
const TEMPLATE_VERSION: &str = "v1";
const MAX_RESULT_LIMIT: u32 = 1_000;
const MAX_DESCENDANT_DEPTH: u32 = 256;
const MAX_SEARCH_MATCHES: u32 = 100;

#[async_trait]
impl OrderedTreeQueryRepository for PgControlPlaneStore {
    async fn list_ordered_tree_roots(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeBoundedListInput,
    ) -> Result<Vec<OrderedTreeNodeProjection>> {
        ensure_ordered_tree(metadata)?;
        ensure_limit(input.result_limit, MAX_RESULT_LIMIT)?;
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let records: Vec<Value> = sqlx::query_scalar(&format!(
            "select row_to_json(node) from (select {} from {table_name} where scope_id = $1 and tree_partition_id = $2 and parent_id is null order by sibling_rank collate \"C\", id limit $3) node",
            projected_select_list(metadata)?
        ))
        .bind(input.scope_id)
        .bind(input.tree_partition_id)
        .bind(i64::from(input.result_limit))
        .fetch_all(self.pool())
        .await?;
        Ok(project_nodes(metadata, records))
    }

    async fn list_ordered_tree_children(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeChildrenInput,
    ) -> Result<Vec<OrderedTreeNodeProjection>> {
        ensure_ordered_tree(metadata)?;
        ensure_limit(input.result_limit, MAX_RESULT_LIMIT)?;
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        ensure_node_exists(
            self.pool(),
            &table_name,
            input.scope_id,
            input.tree_partition_id,
            input.parent_id,
            OrderedTreeQueryError::ParentNotFound,
        )
        .await?;
        let records: Vec<Value> = sqlx::query_scalar(&format!(
            "select row_to_json(node) from (select {} from {table_name} where scope_id = $1 and tree_partition_id = $2 and parent_id = $3 order by sibling_rank collate \"C\", id limit $4) node",
            projected_select_list(metadata)?
        ))
        .bind(input.scope_id)
        .bind(input.tree_partition_id)
        .bind(input.parent_id)
        .bind(i64::from(input.result_limit))
        .fetch_all(self.pool())
        .await?;
        Ok(project_nodes(metadata, records))
    }

    async fn list_ordered_tree_ancestors(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeNodeInput,
    ) -> Result<Vec<OrderedTreeNodeProjection>> {
        ensure_ordered_tree(metadata)?;
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        ensure_node_exists(
            self.pool(),
            &table_name,
            input.scope_id,
            input.tree_partition_id,
            input.node_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        ancestor_records(
            self.pool(),
            metadata,
            &table_name,
            input.scope_id,
            input.tree_partition_id,
            input.node_id,
        )
        .await
        .map(|records| project_nodes(metadata, records))
    }

    async fn list_ordered_tree_descendants(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeDescendantsInput,
    ) -> Result<Vec<OrderedTreeDescendantProjection>> {
        ensure_ordered_tree(metadata)?;
        ensure_limit(input.result_limit, MAX_RESULT_LIMIT)?;
        if input.max_depth == 0 || input.max_depth > MAX_DESCENDANT_DEPTH {
            return Err(OrderedTreeQueryError::InvalidMaxDepth {
                max: MAX_DESCENDANT_DEPTH,
            }
            .into());
        }
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        ensure_node_exists(
            self.pool(),
            &table_name,
            input.scope_id,
            input.tree_partition_id,
            input.node_id,
            OrderedTreeQueryError::NodeNotFound,
        )
        .await?;
        let rows: Vec<(Value, i32, bool, Vec<Uuid>)> = sqlx::query_as(&format!(
            r#"
            with recursive descendants(id, depth, id_path, rank_path) as (
                select child.id, 1, array[$3, child.id], array[child.sibling_rank]
                from {table_name} child
                where child.scope_id = $1 and child.tree_partition_id = $2
                  and child.parent_id = $3
                union all
                select child.id, descendants.depth + 1,
                       descendants.id_path || child.id,
                       descendants.rank_path || child.sibling_rank
                from {table_name} child
                join descendants on child.parent_id = descendants.id
                where child.scope_id = $1 and child.tree_partition_id = $2
                  and descendants.depth < $4
                  and not child.id = any(descendants.id_path)
            )
            select row_to_json(node), descendants.depth,
                   exists(select 1 from {table_name} child where child.scope_id = $1 and child.tree_partition_id = $2 and child.parent_id = descendants.id),
                   descendants.id_path
            from descendants
            join lateral (select {projection} from {table_name} source where source.scope_id = $1 and source.tree_partition_id = $2 and source.id = descendants.id) node on true
            order by descendants.rank_path collate "C", descendants.id_path
            limit $5
            "#,
            projection = qualified_projection(metadata, "source")?,
        ))
        .bind(input.scope_id)
        .bind(input.tree_partition_id)
        .bind(input.node_id)
        .bind(i32::try_from(input.max_depth)?)
        .bind(i64::from(input.result_limit))
        .fetch_all(self.pool())
        .await?;
        Ok(rows
            .into_iter()
            .map(
                |(record, depth, has_children, path)| OrderedTreeDescendantProjection {
                    record: normalize_record(metadata, record),
                    depth: depth as u32,
                    has_children,
                    path: input.include_path.then_some(path),
                },
            )
            .collect())
    }

    async fn search_ordered_tree_prefix(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeSearchInput,
    ) -> Result<Vec<OrderedTreeSearchProjection>> {
        ensure_ordered_tree(metadata)?;
        ensure_limit(input.match_limit, MAX_SEARCH_MATCHES)?;
        let prefix = input.prefix.trim();
        if prefix.is_empty() {
            return Err(OrderedTreeQueryError::EmptySearchPrefix.into());
        }
        let searchable_fields = metadata
            .fields
            .iter()
            .filter(|field| {
                !field.is_system
                    && matches!(
                        field.field_kind,
                        domain::ModelFieldKind::String
                            | domain::ModelFieldKind::Enum
                            | domain::ModelFieldKind::Text
                    )
            })
            .collect::<Vec<_>>();
        if searchable_fields.is_empty() {
            return Err(OrderedTreeQueryError::NoSearchableFields.into());
        }

        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let mut builder = QueryBuilder::<Postgres>::new(format!(
            "select id, row_to_json(node) from (select {} from {table_name} where scope_id = ",
            projected_select_list(metadata)?
        ));
        builder.push_bind(input.scope_id);
        builder.push(" and tree_partition_id = ");
        builder.push_bind(input.tree_partition_id);
        builder.push(" and (");
        let pattern = format!("{}%", escape_like_prefix(prefix));
        for (index, field) in searchable_fields.iter().enumerate() {
            if index > 0 {
                builder.push(" or ");
            }
            builder.push("lower(");
            builder.push(quote_identifier(&field.physical_column_name)?);
            builder.push(") collate \"C\" like lower(");
            builder.push_bind(pattern.clone());
            builder.push(") escape E'\\\\'");
        }
        builder.push(" ) order by sibling_rank collate \"C\", id limit ");
        builder.push_bind(i64::from(input.match_limit));
        builder.push(") node");
        let matches: Vec<(Uuid, Value)> = builder.build_query_as().fetch_all(self.pool()).await?;

        let mut output = Vec::<OrderedTreeSearchProjection>::new();
        let mut indexes = HashMap::<Uuid, usize>::new();
        for (match_id, record) in matches {
            let ancestors = ancestor_records(
                self.pool(),
                metadata,
                &table_name,
                input.scope_id,
                input.tree_partition_id,
                match_id,
            )
            .await?;
            for ancestor in ancestors {
                let ancestor = normalize_record(metadata, ancestor);
                let ancestor_id = record_id(&ancestor)?;
                if let std::collections::hash_map::Entry::Vacant(entry) = indexes.entry(ancestor_id)
                {
                    entry.insert(output.len());
                    output.push(OrderedTreeSearchProjection {
                        record: ancestor,
                        is_match: false,
                    });
                }
            }

            let record = normalize_record(metadata, record);
            if let Some(index) = indexes.get(&match_id).copied() {
                output[index].record = record;
                output[index].is_match = true;
            } else {
                indexes.insert(match_id, output.len());
                output.push(OrderedTreeSearchProjection {
                    record,
                    is_match: true,
                });
            }
        }
        Ok(output)
    }
}

fn ensure_ordered_tree(metadata: &ModelMetadata) -> Result<()> {
    if metadata.template_provider == TEMPLATE_PROVIDER
        && metadata.template_code == TEMPLATE_CODE
        && metadata.template_version == TEMPLATE_VERSION
    {
        Ok(())
    } else {
        Err(OrderedTreeQueryError::WrongTemplate.into())
    }
}

fn ensure_limit(limit: u32, max: u32) -> Result<()> {
    if limit == 0 || limit > max {
        Err(OrderedTreeQueryError::InvalidResultLimit { max }.into())
    } else {
        Ok(())
    }
}

async fn ensure_node_exists(
    pool: &PgPool,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
    error: OrderedTreeQueryError,
) -> Result<()> {
    let exists: bool = sqlx::query_scalar(&format!(
        "select exists(select 1 from {table_name} where scope_id = $1 and tree_partition_id = $2 and id = $3)"
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(node_id)
    .fetch_one(pool)
    .await?;
    if exists {
        Ok(())
    } else {
        Err(error.into())
    }
}

async fn ancestor_records(
    pool: &PgPool,
    metadata: &ModelMetadata,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
) -> Result<Vec<Value>> {
    let records: Vec<Value> = sqlx::query_scalar(&format!(
        r#"
        with recursive ancestors(id, parent_id, depth, path) as (
            select parent.id, parent.parent_id, 0, array[parent.id]
            from {table_name} node
            join {table_name} parent on parent.scope_id = node.scope_id
                and parent.tree_partition_id = node.tree_partition_id
                and parent.id = node.parent_id
            where node.scope_id = $1 and node.tree_partition_id = $2 and node.id = $3
            union all
            select parent.id, parent.parent_id, ancestors.depth + 1, ancestors.path || parent.id
            from {table_name} parent
            join ancestors on parent.id = ancestors.parent_id
            where parent.scope_id = $1 and parent.tree_partition_id = $2
              and not parent.id = any(ancestors.path)
              and cardinality(ancestors.path) <= $4
        )
        select row_to_json(node)
        from ancestors
        join lateral (select {projection} from {table_name} source where source.scope_id = $1 and source.tree_partition_id = $2 and source.id = ancestors.id) node on true
        order by ancestors.depth desc
        limit ($4 + 1)
        "#,
        projection = qualified_projection(metadata, "source")?,
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(node_id)
    .bind(i32::try_from(MAX_DESCENDANT_DEPTH)?)
    .fetch_all(pool)
    .await?;
    if records.len() > MAX_DESCENDANT_DEPTH as usize {
        Err(OrderedTreeQueryError::AncestorDepthLimitExceeded {
            max: MAX_DESCENDANT_DEPTH,
        }
        .into())
    } else {
        Ok(records)
    }
}

fn qualified_projection(metadata: &ModelMetadata, alias: &str) -> Result<String> {
    let projection = projected_select_list(metadata)?;
    Ok(projection
        .split(", ")
        .map(|column| format!("{alias}.{column}"))
        .collect::<Vec<_>>()
        .join(", "))
}

fn project_nodes(metadata: &ModelMetadata, records: Vec<Value>) -> Vec<OrderedTreeNodeProjection> {
    records
        .into_iter()
        .map(|record| OrderedTreeNodeProjection {
            record: normalize_record(metadata, record),
        })
        .collect()
}

fn record_id(record: &Value) -> Result<Uuid> {
    let id = record
        .get("id")
        .and_then(Value::as_str)
        .ok_or_else(|| anyhow::anyhow!("ordered-tree projection is missing id"))?;
    Uuid::parse_str(id).map_err(Into::into)
}

fn escape_like_prefix(prefix: &str) -> String {
    let mut escaped = String::with_capacity(prefix.len());
    for character in prefix.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}
