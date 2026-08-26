use std::collections::HashSet;

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use serde_json::Value;
use sqlx::{Postgres, QueryBuilder, Transaction};
use storage_durable::{
    model_metadata::ModelMetadata,
    runtime_record_repository::{
        OrderedTreeCommandError, OrderedTreeCreateInput, OrderedTreeCreatePosition,
        OrderedTreeCreateResult, OrderedTreeLeafDeleteInput, OrderedTreeMoveInput,
        OrderedTreeMovePosition, OrderedTreeStructureRepository, OrderedTreeSubtreeDeleteInput,
        OrderedTreeSubtreeDeleteResult,
    },
};
use uuid::Uuid;

use super::rank::{between, rebalance, FractionalRank};
use crate::{
    repositories::PgControlPlaneStore,
    runtime_record_repository::{push_field_value, quote_identifier},
};

const TEMPLATE_PROVIDER: &str = "core";
const TEMPLATE_CODE: &str = "ordered_tree";
const TEMPLATE_VERSION: &str = "v1";

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct OrderedTreeSubtreeSnapshot {
    node_ids_child_before_parent: Vec<Uuid>,
}

impl OrderedTreeSubtreeSnapshot {
    pub(crate) fn node_ids_child_before_parent(&self) -> &[Uuid] {
        &self.node_ids_child_before_parent
    }

    pub(crate) fn affected_count(&self) -> u64 {
        self.node_ids_child_before_parent.len() as u64
    }
}

#[derive(Debug)]
struct Sibling {
    id: Uuid,
    rank: FractionalRank,
}

struct StructurePosition {
    parent_id: Option<Uuid>,
    before_id: Option<Uuid>,
    after_id: Option<Uuid>,
}

impl From<&OrderedTreeCreatePosition> for StructurePosition {
    fn from(position: &OrderedTreeCreatePosition) -> Self {
        Self {
            parent_id: position.parent_id,
            before_id: position.before_id,
            after_id: position.after_id,
        }
    }
}

impl From<&OrderedTreeMovePosition> for StructurePosition {
    fn from(position: &OrderedTreeMovePosition) -> Self {
        Self {
            parent_id: position.new_parent_id,
            before_id: position.before_id,
            after_id: position.after_id,
        }
    }
}

#[async_trait]
impl OrderedTreeStructureRepository for PgControlPlaneStore {
    async fn create_ordered_tree_node(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeCreateInput,
    ) -> Result<OrderedTreeCreateResult> {
        let mut tx = self.pool().begin().await?;
        let result = create_ordered_tree_node_in_transaction(&mut tx, metadata, input).await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn move_ordered_tree_node(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeMoveInput,
    ) -> Result<()> {
        let mut tx = self.pool().begin().await?;
        move_ordered_tree_node_in_transaction(&mut tx, metadata, input).await?;
        tx.commit().await?;
        Ok(())
    }

    async fn delete_ordered_tree_leaf(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeLeafDeleteInput,
    ) -> Result<bool> {
        let mut tx = self.pool().begin().await?;
        let result = delete_ordered_tree_leaf_in_transaction(&mut tx, metadata, input).await?;
        tx.commit().await?;
        Ok(result)
    }

    async fn delete_ordered_tree_subtree(
        &self,
        metadata: &ModelMetadata,
        input: OrderedTreeSubtreeDeleteInput,
    ) -> Result<OrderedTreeSubtreeDeleteResult> {
        let mut tx = self.pool().begin().await?;
        let result = delete_ordered_tree_subtree_in_transaction(&mut tx, metadata, input).await?;
        tx.commit().await?;
        Ok(result)
    }
}

pub(crate) async fn create_ordered_tree_node_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    metadata: &ModelMetadata,
    input: OrderedTreeCreateInput,
) -> Result<OrderedTreeCreateResult> {
    ensure_ordered_tree(metadata)?;
    let position = StructurePosition::from(&input.position);
    ensure_position_shape(&position)?;
    let table_name = quote_identifier(&metadata.physical_table_name)?;
    lock_structure(
        tx,
        metadata.model_id,
        input.scope_id,
        input.tree_partition_id,
    )
    .await?;
    ensure_parent(
        tx,
        &table_name,
        input.scope_id,
        input.tree_partition_id,
        position.parent_id,
    )
    .await?;
    let sibling_rank = allocate_position(
        tx,
        &table_name,
        input.scope_id,
        input.tree_partition_id,
        &position,
        None,
    )
    .await?;
    let node_id = Uuid::now_v7();
    insert_node(
        tx,
        metadata,
        &table_name,
        node_id,
        input.actor_user_id,
        input.scope_id,
        input.tree_partition_id,
        position.parent_id,
        &sibling_rank,
        input.payload,
    )
    .await?;
    Ok(OrderedTreeCreateResult { node_id })
}

pub(crate) async fn move_ordered_tree_node_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    metadata: &ModelMetadata,
    input: OrderedTreeMoveInput,
) -> Result<()> {
    ensure_ordered_tree(metadata)?;
    let position = StructurePosition::from(&input.position);
    ensure_position_shape(&position)?;
    let table_name = quote_identifier(&metadata.physical_table_name)?;
    lock_structure(
        tx,
        metadata.model_id,
        input.scope_id,
        input.tree_partition_id,
    )
    .await?;
    ensure_node(
        tx,
        &table_name,
        input.scope_id,
        input.tree_partition_id,
        input.node_id,
    )
    .await?;
    ensure_parent(
        tx,
        &table_name,
        input.scope_id,
        input.tree_partition_id,
        position.parent_id,
    )
    .await?;
    ensure_acyclic_move(
        tx,
        &table_name,
        input.scope_id,
        input.tree_partition_id,
        input.node_id,
        position.parent_id,
    )
    .await?;
    if position.before_id == Some(input.node_id) || position.after_id == Some(input.node_id) {
        return Err(OrderedTreeCommandError::AnchorSiblingGroupConflict.into());
    }
    let sibling_rank = allocate_position(
        tx,
        &table_name,
        input.scope_id,
        input.tree_partition_id,
        &position,
        Some(input.node_id),
    )
    .await?;

    let result = sqlx::query(&format!(
        "update {table_name} set parent_id = $1, sibling_rank = $2, updated_by = $3, updated_at = now() where scope_id = $4 and tree_partition_id = $5 and id = $6"
    ))
    .bind(position.parent_id)
    .bind(sibling_rank.as_str())
    .bind(nullable_actor(input.actor_user_id))
    .bind(input.scope_id)
    .bind(input.tree_partition_id)
    .bind(input.node_id)
    .execute(&mut **tx)
    .await
    .map_err(map_structure_write_error)?;
    if result.rows_affected() != 1 {
        return Err(OrderedTreeCommandError::NodeNotFound.into());
    }
    Ok(())
}

pub(crate) async fn delete_ordered_tree_leaf_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    metadata: &ModelMetadata,
    input: OrderedTreeLeafDeleteInput,
) -> Result<bool> {
    ensure_ordered_tree(metadata)?;
    let table_name = quote_identifier(&metadata.physical_table_name)?;
    lock_structure(
        tx,
        metadata.model_id,
        input.scope_id,
        input.tree_partition_id,
    )
    .await?;
    if !node_exists(
        tx,
        &table_name,
        input.scope_id,
        input.tree_partition_id,
        input.node_id,
    )
    .await?
    {
        return Ok(false);
    }
    let has_children: bool = sqlx::query_scalar(&format!(
        "select exists(select 1 from {table_name} where scope_id = $1 and tree_partition_id = $2 and parent_id = $3)"
    ))
    .bind(input.scope_id)
    .bind(input.tree_partition_id)
    .bind(input.node_id)
    .fetch_one(&mut **tx)
    .await?;
    if has_children {
        return Err(OrderedTreeCommandError::TreeNodeHasChildren.into());
    }
    let result = sqlx::query(&format!(
        "delete from {table_name} where scope_id = $1 and tree_partition_id = $2 and id = $3"
    ))
    .bind(input.scope_id)
    .bind(input.tree_partition_id)
    .bind(input.node_id)
    .execute(&mut **tx)
    .await
    .map_err(map_leaf_delete_error)?;
    Ok(result.rows_affected() == 1)
}

pub(crate) async fn delete_ordered_tree_subtree_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    metadata: &ModelMetadata,
    input: OrderedTreeSubtreeDeleteInput,
) -> Result<OrderedTreeSubtreeDeleteResult> {
    let snapshot = snapshot_ordered_tree_subtree_in_transaction(
        tx,
        metadata,
        input.scope_id,
        input.tree_partition_id,
        input.node_id,
    )
    .await?;
    let actual = snapshot.affected_count();
    if actual != input.expected_affected_count {
        return Err(OrderedTreeCommandError::ExpectedAffectedCountMismatch {
            expected: input.expected_affected_count,
            actual,
        }
        .into());
    }

    let table_name = quote_identifier(&metadata.physical_table_name)?;
    let mut deleted_count = 0_u64;
    for node_id in snapshot.node_ids_child_before_parent().iter().copied() {
        let result = sqlx::query(&format!(
            "delete from {table_name} where scope_id = $1 and tree_partition_id = $2 and id = $3"
        ))
        .bind(input.scope_id)
        .bind(input.tree_partition_id)
        .bind(node_id)
        .execute(&mut **tx)
        .await?;
        deleted_count += result.rows_affected();
    }
    if deleted_count != actual {
        return Err(OrderedTreeCommandError::ExpectedAffectedCountMismatch {
            expected: actual,
            actual: deleted_count,
        }
        .into());
    }
    Ok(OrderedTreeSubtreeDeleteResult { deleted_count })
}

/// Acquires the partition structure lock before traversing; the snapshot stays structurally
/// stable until the caller commits or rolls back the supplied transaction.
pub(crate) async fn snapshot_ordered_tree_subtree_in_transaction(
    tx: &mut Transaction<'_, Postgres>,
    metadata: &ModelMetadata,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
) -> Result<OrderedTreeSubtreeSnapshot> {
    ensure_ordered_tree(metadata)?;
    let table_name = quote_identifier(&metadata.physical_table_name)?;
    lock_structure(tx, metadata.model_id, scope_id, tree_partition_id).await?;
    load_ordered_tree_subtree_snapshot(tx, &table_name, scope_id, tree_partition_id, node_id).await
}

async fn load_ordered_tree_subtree_snapshot(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
) -> Result<OrderedTreeSubtreeSnapshot> {
    let node_ids_child_before_parent: Vec<Uuid> = sqlx::query_scalar(&format!(
        r#"
        with recursive subtree(id, depth, path) as (
            select id, 0, array[id]
            from {table_name}
            where scope_id = $1 and tree_partition_id = $2 and id = $3
            union all
            select child.id, subtree.depth + 1, subtree.path || child.id
            from {table_name} child
            join subtree on child.parent_id = subtree.id
            where child.scope_id = $1 and child.tree_partition_id = $2
              and not child.id = any(subtree.path)
        )
        select id from subtree order by depth desc, id
        "#
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(node_id)
    .fetch_all(&mut **tx)
    .await?;
    if node_ids_child_before_parent.is_empty() {
        return Err(OrderedTreeCommandError::NodeNotFound.into());
    }
    Ok(OrderedTreeSubtreeSnapshot {
        node_ids_child_before_parent,
    })
}

fn ensure_ordered_tree(metadata: &ModelMetadata) -> Result<()> {
    if metadata.template_provider == TEMPLATE_PROVIDER
        && metadata.template_code == TEMPLATE_CODE
        && metadata.template_version == TEMPLATE_VERSION
    {
        Ok(())
    } else {
        Err(OrderedTreeCommandError::WrongTemplate.into())
    }
}

fn ensure_position_shape(position: &StructurePosition) -> Result<()> {
    if position.before_id.is_some() && position.after_id.is_some() {
        Err(OrderedTreeCommandError::ConflictingAnchors.into())
    } else {
        Ok(())
    }
}

async fn lock_structure(
    tx: &mut Transaction<'_, Postgres>,
    model_id: Uuid,
    scope_id: Uuid,
    tree_partition_id: Uuid,
) -> Result<()> {
    sqlx::query("select pg_advisory_xact_lock(hashtextextended($1::text || ':' || $2::text || ':' || $3::text, 0))")
        .bind(model_id)
        .bind(scope_id)
        .bind(tree_partition_id)
        .execute(&mut **tx)
        .await?;
    Ok(())
}

async fn ensure_node(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
) -> Result<()> {
    if node_exists(tx, table_name, scope_id, tree_partition_id, node_id).await? {
        Ok(())
    } else {
        Err(OrderedTreeCommandError::NodeNotFound.into())
    }
}

async fn node_exists(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
) -> Result<bool> {
    let row: Option<Uuid> = sqlx::query_scalar(&format!(
        "select id from {table_name} where scope_id = $1 and tree_partition_id = $2 and id = $3 for update"
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(node_id)
    .fetch_optional(&mut **tx)
    .await?;
    Ok(row.is_some())
}

async fn ensure_parent(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    parent_id: Option<Uuid>,
) -> Result<()> {
    let Some(parent_id) = parent_id else {
        return Ok(());
    };
    if node_exists(tx, table_name, scope_id, tree_partition_id, parent_id).await? {
        Ok(())
    } else {
        Err(OrderedTreeCommandError::ParentNotFound.into())
    }
}

async fn ensure_acyclic_move(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    node_id: Uuid,
    new_parent_id: Option<Uuid>,
) -> Result<()> {
    let Some(new_parent_id) = new_parent_id else {
        return Ok(());
    };
    let creates_cycle: bool = sqlx::query_scalar(&format!(
        r#"
        with recursive ancestors(id, parent_id, path) as (
            select id, parent_id, array[id]
            from {table_name}
            where scope_id = $1 and tree_partition_id = $2 and id = $3
            union all
            select parent.id, parent.parent_id, ancestors.path || parent.id
            from {table_name} parent
            join ancestors on parent.id = ancestors.parent_id
            where parent.scope_id = $1 and parent.tree_partition_id = $2
              and not parent.id = any(ancestors.path)
        )
        select exists(select 1 from ancestors where id = $4)
        "#
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(new_parent_id)
    .bind(node_id)
    .fetch_one(&mut **tx)
    .await?;
    if creates_cycle {
        Err(OrderedTreeCommandError::Cycle.into())
    } else {
        Ok(())
    }
}

async fn allocate_position(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    position: &StructurePosition,
    excluded_node_id: Option<Uuid>,
) -> Result<FractionalRank> {
    ensure_anchor_group(tx, table_name, scope_id, tree_partition_id, position).await?;
    let mut siblings = load_siblings(
        tx,
        table_name,
        scope_id,
        tree_partition_id,
        position.parent_id,
        excluded_node_id,
    )
    .await?;
    let mut allocation = allocate_from_siblings(&siblings, position)?;
    if allocation.rebalance_recommended {
        rebalance_siblings(tx, table_name, scope_id, tree_partition_id, &mut siblings).await?;
        siblings = load_siblings(
            tx,
            table_name,
            scope_id,
            tree_partition_id,
            position.parent_id,
            excluded_node_id,
        )
        .await?;
        allocation = allocate_from_siblings(&siblings, position)?;
    }
    Ok(allocation.rank)
}

async fn ensure_anchor_group(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    position: &StructurePosition,
) -> Result<()> {
    let Some(anchor_id) = position.before_id.or(position.after_id) else {
        return Ok(());
    };
    let anchor_parent: Option<Option<Uuid>> = sqlx::query_scalar(&format!(
        "select parent_id from {table_name} where scope_id = $1 and tree_partition_id = $2 and id = $3 for update"
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(anchor_id)
    .fetch_optional(&mut **tx)
    .await?;
    match anchor_parent {
        None => Err(OrderedTreeCommandError::AnchorNotFound.into()),
        Some(parent_id) if parent_id != position.parent_id => {
            Err(OrderedTreeCommandError::AnchorSiblingGroupConflict.into())
        }
        Some(_) => Ok(()),
    }
}

fn allocate_from_siblings(
    siblings: &[Sibling],
    position: &StructurePosition,
) -> Result<super::rank::RankAllocation> {
    let (left, right) = if let Some(before_id) = position.before_id {
        let index = sibling_index(siblings, before_id)?;
        (
            index.checked_sub(1).and_then(|index| siblings.get(index)),
            siblings.get(index),
        )
    } else if let Some(after_id) = position.after_id {
        let index = sibling_index(siblings, after_id)?;
        (siblings.get(index), siblings.get(index + 1))
    } else {
        (siblings.last(), None)
    };
    between(left.map(|node| &node.rank), right.map(|node| &node.rank)).map_err(Into::into)
}

fn sibling_index(siblings: &[Sibling], anchor_id: Uuid) -> Result<usize> {
    siblings
        .iter()
        .position(|node| node.id == anchor_id)
        .ok_or_else(|| OrderedTreeCommandError::AnchorNotFound.into())
}

async fn load_siblings(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    parent_id: Option<Uuid>,
    excluded_node_id: Option<Uuid>,
) -> Result<Vec<Sibling>> {
    let rows: Vec<(Uuid, String)> = sqlx::query_as(&format!(
        "select id, sibling_rank from {table_name} where scope_id = $1 and tree_partition_id = $2 and parent_id is not distinct from $3 and ($4::uuid is null or id <> $4) order by sibling_rank collate \"C\", id for update"
    ))
    .bind(scope_id)
    .bind(tree_partition_id)
    .bind(parent_id)
    .bind(excluded_node_id)
    .fetch_all(&mut **tx)
    .await?;
    rows.into_iter()
        .map(|(id, rank)| {
            Ok(Sibling {
                id,
                rank: FractionalRank::parse(rank)?,
            })
        })
        .collect()
}

async fn rebalance_siblings(
    tx: &mut Transaction<'_, Postgres>,
    table_name: &str,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    siblings: &mut [Sibling],
) -> Result<()> {
    let replacements = rebalance(siblings.len())?;
    for sibling in siblings.iter() {
        sqlx::query(&format!(
            "update {table_name} set sibling_rank = $1 where scope_id = $2 and tree_partition_id = $3 and id = $4"
        ))
        .bind(format!("~{}", sibling.id.simple()))
        .bind(scope_id)
        .bind(tree_partition_id)
        .bind(sibling.id)
        .execute(&mut **tx)
        .await?;
    }
    for (sibling, replacement) in siblings.iter_mut().zip(replacements) {
        sqlx::query(&format!(
            "update {table_name} set sibling_rank = $1 where scope_id = $2 and tree_partition_id = $3 and id = $4"
        ))
        .bind(replacement.as_str())
        .bind(scope_id)
        .bind(tree_partition_id)
        .bind(sibling.id)
        .execute(&mut **tx)
        .await?;
        sibling.rank = replacement;
    }
    Ok(())
}

#[allow(clippy::too_many_arguments)]
async fn insert_node(
    tx: &mut Transaction<'_, Postgres>,
    metadata: &ModelMetadata,
    table_name: &str,
    node_id: Uuid,
    actor_user_id: Uuid,
    scope_id: Uuid,
    tree_partition_id: Uuid,
    parent_id: Option<Uuid>,
    sibling_rank: &FractionalRank,
    payload: Value,
) -> Result<()> {
    let payload = payload
        .as_object()
        .ok_or_else(|| anyhow!("runtime payload must be object"))?;
    let mut columns = HashSet::from([
        "id".to_owned(),
        "scope_id".to_owned(),
        "tree_partition_id".to_owned(),
        "created_by".to_owned(),
        "updated_by".to_owned(),
        "parent_id".to_owned(),
        "sibling_rank".to_owned(),
    ]);
    let mut fields = Vec::with_capacity(payload.len());
    for (field_code, value) in payload {
        let field = metadata
            .field_by_code(field_code)
            .ok_or_else(|| anyhow!("undeclared field code: {field_code}"))?;
        if field.is_system || !field.is_writable {
            return Err(OrderedTreeCommandError::FieldNotWritable(field_code.clone()).into());
        }
        if !columns.insert(field.physical_column_name.clone()) {
            return Err(anyhow!(
                "duplicate physical field column: {}",
                field.physical_column_name
            ));
        }
        fields.push((field, value));
    }

    let mut builder = QueryBuilder::<Postgres>::new(format!(
        "insert into {table_name} (id, scope_id, tree_partition_id, created_by, updated_by, parent_id, sibling_rank"
    ));
    for (field, _) in &fields {
        builder.push(", ");
        builder.push(quote_identifier(&field.physical_column_name)?);
    }
    builder.push(") values (");
    builder.push_bind(node_id);
    builder.push(", ");
    builder.push_bind(scope_id);
    builder.push(", ");
    builder.push_bind(tree_partition_id);
    builder.push(", ");
    builder.push_bind(nullable_actor(actor_user_id));
    builder.push(", ");
    builder.push_bind(nullable_actor(actor_user_id));
    builder.push(", ");
    builder.push_bind(parent_id);
    builder.push(", ");
    builder.push_bind(sibling_rank.as_str().to_owned());
    for (field, value) in fields {
        builder.push(", ");
        push_field_value(&mut builder, field, value)?;
    }
    builder.push(")");
    builder
        .build()
        .execute(&mut **tx)
        .await
        .map_err(map_structure_write_error)?;
    Ok(())
}

fn nullable_actor(actor_user_id: Uuid) -> Option<Uuid> {
    (!actor_user_id.is_nil()).then_some(actor_user_id)
}

fn map_structure_write_error(error: sqlx::Error) -> anyhow::Error {
    let sqlx::Error::Database(database_error) = &error else {
        return error.into();
    };
    match database_error.code().as_deref() {
        Some("23503") => OrderedTreeCommandError::ParentNotFound.into(),
        Some("23505") => OrderedTreeCommandError::PositionConflict.into(),
        Some("23514") => OrderedTreeCommandError::Cycle.into(),
        _ => error.into(),
    }
}

fn map_leaf_delete_error(error: sqlx::Error) -> anyhow::Error {
    if matches!(
        &error,
        sqlx::Error::Database(database_error)
            if database_error.code().as_deref() == Some("23503")
    ) {
        OrderedTreeCommandError::TreeNodeHasChildren.into()
    } else {
        error.into()
    }
}
