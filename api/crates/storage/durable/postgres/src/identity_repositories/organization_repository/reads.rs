use super::{metadata, PgControlPlaneStore};
use crate::ordered_tree::queries::{
    list_ordered_tree_children_in_snapshot, list_ordered_tree_roots_in_snapshot,
    search_ordered_tree_prefix_in_snapshot,
};
use anyhow::Result;
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use control_plane_contracts::{ports::DepartmentListInput, ControlPlaneContractError as Error};
use domain::{Department, DepartmentPage, DepartmentTreeItem};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use sqlx::{Executor, PgConnection, Postgres, Row};
use std::collections::HashMap;
use storage_durable::runtime_record_repository::{
    OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreePage, OrderedTreeQueryError,
    OrderedTreeSearchInput,
};
use uuid::Uuid;

pub(super) async fn enrich<'e, E: Executor<'e, Database = Postgres>>(
    executor: E,
    scope_id: Uuid,
    ids: Option<&[Uuid]>,
) -> Result<Vec<DepartmentTreeItem>> {
    let rows = sqlx::query(
        r#"select d.id,d.name,d.parent_id,
          array(select r.code from department_role_bindings b join roles r on r.id=b.role_id
                where b.scope_id=$1 and b.department_id=d.id order by r.code) role_codes,
          (select count(distinct b.user_id) from departments s join user_department_bindings b
             on b.department_id=s.id and b.scope_id=$1
             where s.scope_id=$1 and s.tree_partition_id=d.tree_partition_id
               and ARRAY[s.tree_path] OPERATOR(public.<@) d.tree_path) member_count,
          exists(select 1 from departments child where child.scope_id=$1
                 and child.tree_partition_id=d.tree_partition_id and child.parent_id=d.id) has_children
        from departments d where d.scope_id=$1 and ($2::uuid[] is null or d.id=any($2))
        order by d.sibling_rank collate "C",d.id"#,
    )
    .bind(scope_id)
    .bind(ids)
    .fetch_all(executor)
    .await?;
    rows.into_iter()
        .map(|r| {
            Ok(DepartmentTreeItem {
                department: Department {
                    id: r.try_get("id")?,
                    name: r.try_get("name")?,
                    parent_id: r.try_get("parent_id")?,
                    role_codes: r.try_get("role_codes")?,
                    member_count: r.try_get("member_count")?,
                },
                has_children: r.try_get("has_children")?,
                is_match: true,
            })
        })
        .collect()
}

fn query_error(error: anyhow::Error) -> anyhow::Error {
    if let Some(error) = error.downcast_ref::<OrderedTreeQueryError>() {
        return match error {
            OrderedTreeQueryError::InvalidCursor => Error::InvalidInput("tree_invalid_cursor"),
            OrderedTreeQueryError::StaleCursor => Error::Conflict("tree_stale_cursor"),
            OrderedTreeQueryError::NodeNotFound | OrderedTreeQueryError::ParentNotFound => {
                Error::NotFound("department")
            }
            OrderedTreeQueryError::InvalidResultLimit => Error::InvalidInput("limit"),
            OrderedTreeQueryError::EmptySearchPrefix => Error::InvalidInput("prefix"),
            _ => return anyhow::anyhow!(error.clone()),
        }
        .into();
    }
    error
}

pub(super) async fn list_page(
    store: &PgControlPlaneStore,
    scope_id: Uuid,
    input: DepartmentListInput,
) -> Result<DepartmentPage> {
    if input.limit == 0 {
        return Err(Error::InvalidInput("limit").into());
    }
    if (input.ids.is_some() && (input.parent_id.is_some() || input.prefix.is_some()))
        || (input.prefix.is_some() && input.parent_id.is_some())
    {
        return Err(Error::InvalidInput("department_query").into());
    }
    let model = metadata(scope_id);
    let mut tx = store.pool().begin().await?;
    sqlx::query("set transaction isolation level repeatable read read only")
        .execute(&mut *tx)
        .await?;
    let page: OrderedTreePage<(Uuid, bool)> = if let Some(ids) = input.ids {
        lookup_page(&mut tx, scope_id, ids, input.limit, input.cursor).await?
    } else if let Some(prefix) = input.prefix {
        let page = search_ordered_tree_prefix_in_snapshot(
            &mut tx,
            &model,
            OrderedTreeSearchInput {
                scope_id,
                tree_partition_id: scope_id,
                prefix,
                match_limit: input.limit,
                cursor: input.cursor,
            },
        )
        .await
        .map_err(query_error)?;
        OrderedTreePage {
            items: page
                .items
                .into_iter()
                .map(|node| Ok((record_id(&node.record)?, node.is_match)))
                .collect::<Result<_>>()?,
            has_more: page.has_more,
            next_cursor: page.next_cursor,
        }
    } else {
        let page = if let Some(parent_id) = input.parent_id {
            list_ordered_tree_children_in_snapshot(
                &mut tx,
                &model,
                OrderedTreeChildrenInput {
                    scope_id,
                    tree_partition_id: scope_id,
                    parent_id,
                    result_limit: input.limit,
                    cursor: input.cursor,
                },
            )
            .await
        } else {
            list_ordered_tree_roots_in_snapshot(
                &mut tx,
                &model,
                OrderedTreeBoundedListInput {
                    scope_id,
                    tree_partition_id: scope_id,
                    result_limit: input.limit,
                    cursor: input.cursor,
                },
            )
            .await
        }
        .map_err(query_error)?;
        OrderedTreePage {
            items: page
                .items
                .into_iter()
                .map(|node| Ok((record_id(&node.record)?, true)))
                .collect::<Result<_>>()?,
            has_more: page.has_more,
            next_cursor: page.next_cursor,
        }
    };
    let ids: Vec<_> = page.items.iter().map(|(id, _)| *id).collect();
    let mut enriched: HashMap<_, _> = enrich(&mut *tx, scope_id, Some(&ids))
        .await?
        .into_iter()
        .map(|item| (item.department.id, item))
        .collect();
    let items = page
        .items
        .into_iter()
        .map(|(id, is_match)| {
            let mut item = enriched
                .remove(&id)
                .ok_or(Error::Conflict("tree_stale_cursor"))?;
            item.is_match = is_match;
            Ok(item)
        })
        .collect::<Result<_>>()?;
    tx.commit().await?;
    Ok(DepartmentPage {
        items,
        has_more: page.has_more,
        next_cursor: page.next_cursor,
    })
}

fn record_id(record: &serde_json::Value) -> Result<Uuid> {
    Ok(serde_json::from_value(
        record
            .get("id")
            .cloned()
            .ok_or_else(|| anyhow::anyhow!("department projection missing id"))?,
    )?)
}

// Selected-value lookup is ordered by immutable IDs, independent of tree moves.
// Its cursor is bound to the complete explicit selection and workspace.
#[derive(Serialize, Deserialize)]
struct LookupCursor {
    version: u8,
    scope_id: Uuid,
    ids_digest: String,
    after: Uuid,
}

async fn lookup_page(
    conn: &mut PgConnection,
    scope_id: Uuid,
    mut ids: Vec<Uuid>,
    limit: u32,
    cursor: Option<String>,
) -> Result<OrderedTreePage<(Uuid, bool)>> {
    ids.sort_unstable();
    ids.dedup();
    let ids_digest = URL_SAFE_NO_PAD.encode(Sha256::digest(serde_json::to_vec(&ids)?));
    let after = cursor
        .map(|cursor| -> Result<Uuid> {
            let decoded = URL_SAFE_NO_PAD
                .decode(cursor)
                .map_err(|_| Error::InvalidInput("tree_invalid_cursor"))?;
            let cursor: LookupCursor = serde_json::from_slice(&decoded)
                .map_err(|_| Error::InvalidInput("tree_invalid_cursor"))?;
            if cursor.version != 1
                || cursor.scope_id != scope_id
                || cursor.ids_digest != ids_digest
                || !ids.contains(&cursor.after)
            {
                return Err(Error::InvalidInput("tree_invalid_cursor").into());
            }
            Ok(cursor.after)
        })
        .transpose()?;
    if let Some(after) = after {
        let exists: bool = sqlx::query_scalar(
            "select exists(select 1 from departments where scope_id=$1 and id=$2)",
        )
        .bind(scope_id)
        .bind(after)
        .fetch_one(&mut *conn)
        .await?;
        if !exists {
            return Err(Error::Conflict("tree_stale_cursor").into());
        }
    }
    let mut matches: Vec<Uuid> = sqlx::query_scalar(
        "select id from departments where scope_id=$1 and id=any($2) and ($3::uuid is null or id>$3) order by id limit $4"
    ).bind(scope_id).bind(&ids).bind(after).bind(i64::from(limit)+1).fetch_all(&mut *conn).await?;
    let has_more = matches.len() as u64 > u64::from(limit);
    if has_more {
        matches.pop();
    }
    let next_cursor = if has_more {
        Some(URL_SAFE_NO_PAD.encode(serde_json::to_vec(&LookupCursor {
            version: 1,
            scope_id,
            ids_digest,
            after: *matches.last().expect("positive page limit"),
        })?))
    } else {
        None
    };
    Ok(OrderedTreePage {
        items: matches.into_iter().map(|id| (id, true)).collect(),
        has_more,
        next_cursor,
    })
}
