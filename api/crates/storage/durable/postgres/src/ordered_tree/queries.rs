use std::collections::HashMap;

use super::pagination::{Context, Cursor};
use crate::{
    repositories::PgControlPlaneStore,
    runtime_record_repository::{normalize_record, projected_select_list, quote_identifier},
};
use anyhow::Result;
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{PgConnection, PgPool};
use storage_durable::{
    model_metadata::ModelMetadata,
    runtime_record_repository::{
        OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreeDescendantProjection,
        OrderedTreeDescendantsInput, OrderedTreeNodeInput, OrderedTreeNodeProjection,
        OrderedTreePage, OrderedTreeQueryError, OrderedTreeQueryRepository, OrderedTreeSearchInput,
        OrderedTreeSearchProjection, OrderedTreeSubtreeImpactInput, OrderedTreeSubtreeImpactResult,
    },
};
use uuid::Uuid;

const TEMPLATE_PROVIDER: &str = "core";
const TEMPLATE_CODE: &str = "ordered_tree";
const TEMPLATE_VERSION: &str = "v1";

#[async_trait]
impl OrderedTreeQueryRepository for PgControlPlaneStore {
    async fn get_ordered_tree_subtree_impact(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeSubtreeImpactInput,
    ) -> Result<OrderedTreeSubtreeImpactResult> {
        ensure_ordered_tree(metadata)?;
        let table_name = quote_identifier(&metadata.physical_table_name)?;
        let affected_count: i64 = sqlx::query_scalar(&format!(
            r#"
            select count(*)::bigint from {table_name} node
            join {table_name} root on root.scope_id = $1 and root.tree_partition_id = $2 and root.id = $3
            where node.scope_id = $1 and node.tree_partition_id = $2
              and ARRAY[node.tree_path] OPERATOR(public.<@) root.tree_path
            "#,
        ))
        .bind(input.scope_id)
        .bind(input.tree_partition_id)
        .bind(input.node_id)
        .fetch_one(self.pool())
        .await?;
        if affected_count == 0 {
            return Err(OrderedTreeQueryError::NodeNotFound.into());
        }
        Ok(OrderedTreeSubtreeImpactResult {
            affected_count: affected_count.try_into()?,
        })
    }

    async fn list_ordered_tree_roots(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeBoundedListInput,
    ) -> Result<OrderedTreePage<OrderedTreeNodeProjection>> {
        let mut tx = read_snapshot(self.pool()).await?;
        let page = list_ordered_tree_roots_in_snapshot(&mut tx, metadata, input).await?;
        tx.commit().await?;
        Ok(page)
    }

    async fn list_ordered_tree_children(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeChildrenInput,
    ) -> Result<OrderedTreePage<OrderedTreeNodeProjection>> {
        let mut tx = read_snapshot(self.pool()).await?;
        let page = list_ordered_tree_children_in_snapshot(&mut tx, metadata, input).await?;
        tx.commit().await?;
        Ok(page)
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
        let mut connection = self.pool().acquire().await?;
        ancestor_records(
            &mut connection,
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
    ) -> Result<OrderedTreePage<OrderedTreeDescendantProjection>> {
        if input.max_depth == Some(0) {
            return Err(OrderedTreeQueryError::InvalidMaxDepth.into());
        }
        let mut context = Context::new(
            metadata,
            input.scope_id,
            input.tree_partition_id,
            "descendants",
        );
        context.node_id = Some(input.node_id);
        context.max_depth = input.max_depth;
        context.include_path = input.include_path;
        let mut tx = read_snapshot(self.pool()).await?;
        let page = fetch_page(
            &mut tx,
            metadata,
            &context,
            input.result_limit,
            input.cursor.as_deref(),
        )
        .await?;
        tx.commit().await?;
        Ok(page.map(|row| OrderedTreeDescendantProjection {
            record: normalize_record(metadata, row.0),
            depth: row.1 as u32,
            has_children: row.2,
            path: input.include_path.then_some(row.3),
        }))
    }

    async fn search_ordered_tree_prefix(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeSearchInput,
    ) -> Result<OrderedTreePage<OrderedTreeSearchProjection>> {
        let mut tx = read_snapshot(self.pool()).await?;
        let page = search_ordered_tree_prefix_in_snapshot(&mut tx, metadata, input).await?;
        tx.commit().await?;
        Ok(page)
    }
}

/// The caller owns a read-only repeatable-read transaction spanning this query and any enrichment.
pub(crate) async fn list_ordered_tree_roots_in_snapshot(
    connection: &mut PgConnection,
    metadata: &ModelMetadata,
    input: OrderedTreeBoundedListInput,
) -> Result<OrderedTreePage<OrderedTreeNodeProjection>> {
    let context = Context::new(metadata, input.scope_id, input.tree_partition_id, "roots");
    let page = fetch_page(
        &mut *connection,
        metadata,
        &context,
        input.result_limit,
        input.cursor.as_deref(),
    )
    .await?;
    Ok(page.map(|row| OrderedTreeNodeProjection {
        record: normalize_record(metadata, row.0),
    }))
}

/// The caller owns a read-only repeatable-read transaction spanning this query and any enrichment.
pub(crate) async fn list_ordered_tree_children_in_snapshot(
    connection: &mut PgConnection,
    metadata: &ModelMetadata,
    input: OrderedTreeChildrenInput,
) -> Result<OrderedTreePage<OrderedTreeNodeProjection>> {
    let mut context = Context::new(
        metadata,
        input.scope_id,
        input.tree_partition_id,
        "children",
    );
    context.node_id = Some(input.parent_id);
    let page = fetch_page(
        &mut *connection,
        metadata,
        &context,
        input.result_limit,
        input.cursor.as_deref(),
    )
    .await?;
    Ok(page.map(|row| OrderedTreeNodeProjection {
        record: normalize_record(metadata, row.0),
    }))
}

/// The caller owns a read-only repeatable-read transaction spanning this query and any enrichment.
pub(crate) async fn search_ordered_tree_prefix_in_snapshot(
    connection: &mut PgConnection,
    metadata: &ModelMetadata,
    input: OrderedTreeSearchInput,
) -> Result<OrderedTreePage<OrderedTreeSearchProjection>> {
    let mut context = Context::new(metadata, input.scope_id, input.tree_partition_id, "search");
    context.prefix = Some(input.prefix.trim().to_owned());
    let page = fetch_page(
        &mut *connection,
        metadata,
        &context,
        input.match_limit,
        input.cursor.as_deref(),
    )
    .await?;
    let table_name = quote_identifier(&metadata.physical_table_name)?;
    let mut output = Vec::<OrderedTreeSearchProjection>::new();
    let mut indexes = HashMap::<Uuid, usize>::new();
    for row in page.items {
        let match_id = row.5;
        for ancestor in ancestor_records(
            &mut *connection,
            metadata,
            &table_name,
            input.scope_id,
            input.tree_partition_id,
            match_id,
        )
        .await?
        {
            let ancestor = normalize_record(metadata, ancestor);
            let ancestor_id = record_id(&ancestor)?;
            if let std::collections::hash_map::Entry::Vacant(entry) = indexes.entry(ancestor_id) {
                entry.insert(output.len());
                output.push(OrderedTreeSearchProjection {
                    record: ancestor,
                    is_match: false,
                });
            }
        }
        let record = normalize_record(metadata, row.0);
        if let Some(index) = indexes.get(&match_id).copied() {
            output[index] = OrderedTreeSearchProjection {
                record,
                is_match: true,
            };
        } else {
            indexes.insert(match_id, output.len());
            output.push(OrderedTreeSearchProjection {
                record,
                is_match: true,
            });
        }
    }
    Ok(OrderedTreePage {
        items: output,
        has_more: page.has_more,
        next_cursor: page.next_cursor,
    })
}

// Rows retain the complete ordering key and tree-path fingerprint even when paths are not projected.
type PageRow = (Value, i32, bool, Vec<Uuid>, Vec<String>, Uuid, String);

async fn read_snapshot(pool: &PgPool) -> Result<sqlx::Transaction<'_, sqlx::Postgres>> {
    let mut tx = pool.begin().await?;
    sqlx::query("set transaction isolation level repeatable read read only")
        .execute(&mut *tx)
        .await?;
    Ok(tx)
}

async fn fetch_page(
    conn: &mut PgConnection,
    metadata: &ModelMetadata,
    context: &Context,
    limit: u32,
    cursor: Option<&str>,
) -> Result<OrderedTreePage<PageRow>> {
    ensure_ordered_tree(metadata)?;
    ensure_limit(limit)?;
    let cursor = Cursor::decode(cursor, context)?;
    let table = quote_identifier(&metadata.physical_table_name)?;
    let cte = candidates(metadata, &table, context)?;
    // Cursor errors take precedence over a missing parent/root after an earlier page.
    let anchor = if let Some(cursor) = &cursor {
        Some(cursor.validate(conn, &cte).await?)
    } else {
        None
    };
    if let Some(id) = context.node_id {
        let exists: bool = sqlx::query_scalar(&format!("select exists(select 1 from {table} where scope_id = $1 and tree_partition_id = $2 and id = $3)"))
            .bind(context.scope_id).bind(context.partition_id).bind(id).fetch_one(&mut *conn).await?;
        if !exists {
            return Err(if context.query == "children" {
                OrderedTreeQueryError::ParentNotFound
            } else {
                OrderedTreeQueryError::NodeNotFound
            }
            .into());
        }
    }
    let (predicate, order) = if context.query == "descendants" {
        (
            r#"$6::text[] is null or (candidates.rank_path collate "C", candidates.id_path) > ($6::text[] collate "C", $7::uuid[])"#,
            r#"candidates.rank_path collate "C", candidates.id_path"#,
        )
    } else {
        (
            r#"$6::text[] is null or (candidates.sibling_rank collate "C", candidates.id) > (($6::text[])[1] collate "C", ($7::uuid[])[1])"#,
            r#"candidates.sibling_rank collate "C", candidates.id"#,
        )
    };
    let mut rows: Vec<PageRow> = sqlx::query_as(&format!(r#"{cte}
        select row_to_json(node), candidates.depth,
            exists(select 1 from {table} child where child.scope_id = $1 and child.tree_partition_id = $2 and child.parent_id = candidates.id),
            candidates.id_path, candidates.rank_path, candidates.id, candidates.tree_path::text
        from candidates
        join lateral (select {projection} from {table} source where source.scope_id = $1 and source.tree_partition_id = $2 and source.id = candidates.id) node on true
        where {predicate}
        order by {order}
        limit $8"#,
        projection = qualified_projection(metadata, "source")?))
        .bind(context.scope_id).bind(context.partition_id).bind(context.node_id)
        .bind(context.max_depth.map(i64::from)).bind(context.prefix.as_ref().map(|prefix| format!("{}%", escape_like_prefix(prefix))))
        .bind(anchor.as_ref().map(|anchor| anchor.ranks.clone())).bind(anchor.as_ref().map(|anchor| anchor.ids.clone()))
        .bind(i64::from(limit) + 1).fetch_all(&mut *conn).await?;
    let has_more = rows.len() > limit as usize;
    if has_more {
        rows.pop();
    }
    let next_cursor = if has_more {
        let row = rows.last().expect("a positive limit guarantees an anchor");
        Some(
            Cursor::new(
                context.clone(),
                row.5,
                row.4.clone(),
                row.3.clone(),
                row.6.clone(),
            )?
            .encode()?,
        )
    } else {
        None
    };
    Ok(OrderedTreePage {
        items: rows,
        has_more,
        next_cursor,
    })
}

fn candidates(metadata: &ModelMetadata, table: &str, context: &Context) -> Result<String> {
    // Explicitly type all shared parameters even when a query kind does not use them.
    let parameters = "with parameters as (select $1::uuid, $2::uuid, $3::uuid, $4::bigint, $5::text), candidates as (";
    if context.query == "descendants" {
        return Ok(format!(
            r#"{parameters}
            select child.id, child.tree_path,
                public.nlevel(child.tree_path) - public.nlevel(root.tree_path) as depth,
                string_to_array(public.subpath(child.tree_path, public.nlevel(root.tree_path) - 1)::text, '.')::uuid[] as id_path,
                array(select ancestor.sibling_rank from {table} ancestor
                    where ancestor.scope_id = $1 and ancestor.tree_partition_id = $2
                    and ancestor.id = any(string_to_array(child.tree_path::text, '.')::uuid[])
                    and public.nlevel(ancestor.tree_path) > public.nlevel(root.tree_path)
                    order by public.nlevel(ancestor.tree_path))::text[] as rank_path
            from {table} child
            join {table} root on root.scope_id = $1 and root.tree_partition_id = $2 and root.id = $3
            where child.scope_id = $1 and child.tree_partition_id = $2
                and ARRAY[child.tree_path] OPERATOR(public.<@) root.tree_path
                and public.nlevel(child.tree_path) > public.nlevel(root.tree_path)
                and ($4 is null or public.nlevel(child.tree_path) - public.nlevel(root.tree_path) <= $4)
            )"#
        ));
    }
    let filter = match context.query.as_str() {
        "roots" => "parent_id is null".to_owned(),
        "children" => "parent_id = $3".to_owned(),
        "search" => {
            if context.prefix.as_deref().is_none_or(str::is_empty) {
                return Err(OrderedTreeQueryError::EmptySearchPrefix.into());
            }
            let fields = metadata
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
                .map(|field| {
                    Ok(format!(
                        "lower({}) collate \"C\" like lower($5) escape E'\\\\'",
                        quote_identifier(&field.physical_column_name)?
                    ))
                })
                .collect::<Result<Vec<_>>>()?;
            if fields.is_empty() {
                return Err(OrderedTreeQueryError::NoSearchableFields.into());
            }
            format!("({})", fields.join(" or "))
        }
        _ => unreachable!("page query kind is internally selected"),
    };
    Ok(format!("{parameters} select id, tree_path, sibling_rank, 0::integer as depth, ARRAY[id] as id_path, ARRAY[sibling_rank]::text[] as rank_path from {table} where scope_id = $1 and tree_partition_id = $2 and {filter})"))
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

fn ensure_limit(limit: u32) -> Result<()> {
    if limit == 0 {
        Err(OrderedTreeQueryError::InvalidResultLimit.into())
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
    pool: &mut sqlx::PgConnection,
    metadata: &ModelMetadata,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
) -> Result<Vec<Value>> {
    let records: Vec<Value> = sqlx::query_scalar(&format!(
        r#"
        select row_to_json(node)
        from {table_name} target
        join {table_name} ancestor on ancestor.scope_id = $1 and ancestor.tree_partition_id = $2
          and ancestor.id = any(string_to_array(target.tree_path::text, '.')::uuid[]) and ancestor.id <> target.id
        join lateral (select {projection} from {table_name} source where source.scope_id = $1 and source.tree_partition_id = $2 and source.id = ancestor.id) node on true
        where target.scope_id = $1 and target.tree_partition_id = $2 and target.id = $3
        order by public.nlevel(ancestor.tree_path)
        "#,
        projection = qualified_projection(metadata, "source")?,
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(node_id)
    .fetch_all(pool)
    .await?;
    Ok(records)
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

pub(super) fn escape_like_prefix(prefix: &str) -> String {
    let mut escaped = String::with_capacity(prefix.len());
    for character in prefix.chars() {
        if matches!(character, '\\' | '%' | '_') {
            escaped.push('\\');
        }
        escaped.push(character);
    }
    escaped
}
