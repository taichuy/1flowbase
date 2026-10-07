use super::{
    queries::{
        add_search_field, create_model, create_workspace, insert_node, isolated_database,
        record_id, typed_error, TestNode,
    },
    runtime_metadata,
};
use crate::{run_migrations, PgControlPlaneStore};
use storage_durable::{
    model_metadata::ModelMetadata,
    runtime_record_repository::{
        OrderedTreeBoundedListInput, OrderedTreeChildrenInput, OrderedTreeDescendantsInput,
        OrderedTreePage, OrderedTreeQueryError, OrderedTreeQueryRepository, OrderedTreeSearchInput,
    },
};
use uuid::Uuid;

#[derive(Clone, Copy)]
enum Kind {
    Roots,
    Children,
    Descendants,
    Search,
}

async fn page(
    store: &PgControlPlaneStore,
    metadata: &ModelMetadata,
    scope: Uuid,
    root: Uuid,
    kind: Kind,
    limit: u32,
    cursor: Option<String>,
) -> anyhow::Result<OrderedTreePage<Uuid>> {
    Ok(match kind {
        Kind::Roots => store
            .list_ordered_tree_roots(
                metadata,
                OrderedTreeBoundedListInput {
                    scope_id: scope,
                    tree_partition_id: scope,
                    result_limit: limit,
                    cursor,
                },
            )
            .await?
            .map(|node| record_id(&node.record)),
        Kind::Children => store
            .list_ordered_tree_children(
                metadata,
                OrderedTreeChildrenInput {
                    scope_id: scope,
                    tree_partition_id: scope,
                    parent_id: root,
                    result_limit: limit,
                    cursor,
                },
            )
            .await?
            .map(|node| record_id(&node.record)),
        Kind::Descendants => store
            .list_ordered_tree_descendants(
                metadata,
                OrderedTreeDescendantsInput {
                    scope_id: scope,
                    tree_partition_id: scope,
                    node_id: root,
                    max_depth: None,
                    include_path: true,
                    result_limit: limit,
                    cursor,
                },
            )
            .await?
            .map(|node| record_id(&node.record)),
        Kind::Search => {
            let result = store
                .search_ordered_tree_prefix(
                    metadata,
                    OrderedTreeSearchInput {
                        scope_id: scope,
                        tree_partition_id: scope,
                        prefix: "Match".into(),
                        match_limit: limit,
                        cursor,
                    },
                )
                .await?;
            OrderedTreePage {
                items: result
                    .items
                    .into_iter()
                    .filter(|node| node.is_match)
                    .map(|node| record_id(&node.record))
                    .collect(),
                has_more: result.has_more,
                next_cursor: result.next_cursor,
            }
        }
    })
}

// Every page kind traverses beyond historical ceilings with no duplicates or skipped static rows.
#[tokio::test]
async fn keyset_pages_cover_wide_trees_matches_and_empty_final_pages() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = create_workspace(&store).await;
    let model = create_model(&store, scope, "pages").await;
    let (model, field) = add_search_field(&store, &model).await;
    let metadata = runtime_metadata(&model);
    let root = Uuid::now_v7();
    insert_node(
        &store,
        &model,
        &field,
        scope,
        TestNode::new(root, None, "zU", "Context"),
    )
    .await;
    for parent in [None, Some(root)] {
        sqlx::query(&format!(r#"insert into "{}" (id, scope_id, tree_partition_id, parent_id, sibling_rank, "{}")
            select gen_random_uuid(), $1, $1, $2, lpad(series::text, 8, '0') || 'U', 'Match-' || series::text
            from generate_series(1, 1007) series"#, model.physical_table_name, field.physical_column_name))
            .bind(scope).bind(parent).execute(store.pool()).await.unwrap();
    }
    for kind in [Kind::Roots, Kind::Children, Kind::Descendants, Kind::Search] {
        let first = page(&store, &metadata, scope, root, kind, 3000, None)
            .await
            .unwrap();
        let mut expected = first.items;
        assert!(!first.has_more);
        assert!(first.next_cursor.is_none());
        assert_eq!(
            expected.len(),
            match kind {
                Kind::Roots => 1008,
                Kind::Search => 2014,
                _ => 1007,
            }
        );
        let mut actual = Vec::new();
        let mut cursor = None;
        // Finite iteration bound detects non-progressing cursors instead of hanging CI.
        for _ in 0..20 {
            let result = page(&store, &metadata, scope, root, kind, 113, cursor.take())
                .await
                .unwrap();
            assert!(result.items.len() <= 113);
            actual.extend(result.items);
            if !result.has_more {
                assert!(result.next_cursor.is_none());
                break;
            }
            assert!(result.next_cursor.is_some());
            cursor = result.next_cursor;
        }
        if matches!(kind, Kind::Search) {
            actual.sort();
            expected.sort();
        }
        assert_eq!(actual, expected);
        assert_eq!(
            actual
                .iter()
                .copied()
                .collect::<std::collections::HashSet<_>>()
                .len(),
            actual.len()
        );
    }
    let empty_scope = Uuid::now_v7();
    let empty = page(&store, &metadata, empty_scope, root, Kind::Roots, 100, None)
        .await
        .unwrap();
    assert!(empty.items.is_empty());
    assert!(!empty.has_more);
    assert!(empty.next_cursor.is_none());
    // Delete all later rows: a valid surviving anchor may lead to an empty terminal page.
    let first = page(&store, &metadata, scope, root, Kind::Children, 1, None)
        .await
        .unwrap();
    sqlx::query(&format!("delete from \"{}\" where scope_id = $1 and tree_partition_id = $1 and parent_id = $2 and id <> $3", model.physical_table_name))
        .bind(scope).bind(root).bind(first.items[0]).execute(store.pool()).await.unwrap();
    let last = page(
        &store,
        &metadata,
        scope,
        root,
        Kind::Children,
        1,
        first.next_cursor,
    )
    .await
    .unwrap();
    assert!(last.items.is_empty());
    assert!(!last.has_more);
    assert!(last.next_cursor.is_none());
}

#[tokio::test]
async fn keyset_cursor_rejects_changed_context_and_moved_deleted_anchors() {
    let database = isolated_database().await;
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let scope = create_workspace(&store).await;
    let model = create_model(&store, scope, "cursor").await;
    let (model, field) = add_search_field(&store, &model).await;
    let metadata = runtime_metadata(&model);
    let root = Uuid::now_v7();
    let other_root = Uuid::now_v7();
    insert_node(
        &store,
        &model,
        &field,
        scope,
        TestNode::new(root, None, "F", "MatchRoot"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope,
        TestNode::new(other_root, None, "k", "MatchRoot"),
    )
    .await;
    let child = Uuid::now_v7();
    let other_child = Uuid::now_v7();
    insert_node(
        &store,
        &model,
        &field,
        scope,
        TestNode::new(child, Some(root), "F", "MatchChild"),
    )
    .await;
    insert_node(
        &store,
        &model,
        &field,
        scope,
        TestNode::new(other_child, Some(root), "k", "MatchChild"),
    )
    .await;
    for kind in [Kind::Roots, Kind::Children, Kind::Descendants, Kind::Search] {
        assert_eq!(
            typed_error(
                page(
                    &store,
                    &metadata,
                    scope,
                    root,
                    kind,
                    1,
                    Some("malformed".into())
                )
                .await
                .unwrap_err()
            ),
            OrderedTreeQueryError::InvalidCursor
        );
        let first = page(&store, &metadata, scope, root, kind, 1, None)
            .await
            .unwrap();
        let cursor = first.next_cursor.unwrap();
        let mut wrong_model = metadata.clone();
        wrong_model.model_id = Uuid::now_v7();
        assert_eq!(
            typed_error(
                page(
                    &store,
                    &wrong_model,
                    scope,
                    root,
                    kind,
                    1,
                    Some(cursor.clone())
                )
                .await
                .unwrap_err()
            ),
            OrderedTreeQueryError::InvalidCursor
        );
        assert_eq!(
            typed_error(
                page(
                    &store,
                    &metadata,
                    Uuid::now_v7(),
                    root,
                    kind,
                    1,
                    Some(cursor.clone())
                )
                .await
                .unwrap_err()
            ),
            OrderedTreeQueryError::InvalidCursor
        );
        let wrong_kind = if matches!(kind, Kind::Roots) {
            Kind::Children
        } else {
            Kind::Roots
        };
        assert_eq!(
            typed_error(
                page(
                    &store,
                    &metadata,
                    scope,
                    root,
                    wrong_kind,
                    1,
                    Some(cursor.clone())
                )
                .await
                .unwrap_err()
            ),
            OrderedTreeQueryError::InvalidCursor
        );
        if matches!(kind, Kind::Children | Kind::Descendants) {
            assert_eq!(
                typed_error(
                    page(
                        &store,
                        &metadata,
                        scope,
                        other_root,
                        kind,
                        1,
                        Some(cursor.clone())
                    )
                    .await
                    .unwrap_err()
                ),
                OrderedTreeQueryError::InvalidCursor
            );
        }
        if matches!(kind, Kind::Descendants) {
            let err = store
                .list_ordered_tree_descendants(
                    &metadata,
                    OrderedTreeDescendantsInput {
                        scope_id: scope,
                        tree_partition_id: scope,
                        node_id: root,
                        max_depth: Some(99),
                        include_path: true,
                        result_limit: 1,
                        cursor: Some(cursor.clone()),
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(typed_error(err), OrderedTreeQueryError::InvalidCursor);
        }
        if matches!(kind, Kind::Search) {
            let err = store
                .search_ordered_tree_prefix(
                    &metadata,
                    OrderedTreeSearchInput {
                        scope_id: scope,
                        tree_partition_id: scope,
                        prefix: "Other".into(),
                        match_limit: 1,
                        cursor: Some(cursor.clone()),
                    },
                )
                .await
                .unwrap_err();
            assert_eq!(typed_error(err), OrderedTreeQueryError::InvalidCursor);
        }
        // Same scope, different partition is independently bound.
        let err = store
            .list_ordered_tree_roots(
                &metadata,
                OrderedTreeBoundedListInput {
                    scope_id: scope,
                    tree_partition_id: Uuid::now_v7(),
                    result_limit: 1,
                    cursor: Some(cursor.clone()),
                },
            )
            .await
            .unwrap_err();
        assert_eq!(typed_error(err), OrderedTreeQueryError::InvalidCursor);
        let anchor = first.items[0];
        // Changing an anchor's rank invalidates every kind; restore for the next kind.
        sqlx::query(&format!("update \"{}\" set sibling_rank = 'FU' where scope_id = $1 and tree_partition_id = $1 and id = $2", model.physical_table_name))
            .bind(scope).bind(anchor).execute(store.pool()).await.unwrap();
        assert_eq!(
            typed_error(
                page(&store, &metadata, scope, root, kind, 1, Some(cursor))
                    .await
                    .unwrap_err()
            ),
            OrderedTreeQueryError::StaleCursor
        );
        sqlx::query(&format!("update \"{}\" set sibling_rank = 'F' where scope_id = $1 and tree_partition_id = $1 and id = $2", model.physical_table_name))
            .bind(scope).bind(anchor).execute(store.pool()).await.unwrap();
    }
    // Reparent without changing rank: tree-path fingerprints detect this too.
    let first = page(&store, &metadata, scope, root, Kind::Children, 1, None)
        .await
        .unwrap();
    sqlx::query(&format!("update \"{}\" set parent_id = $2 where scope_id = $1 and tree_partition_id = $1 and id = $3", model.physical_table_name))
        .bind(scope).bind(other_root).bind(child).execute(store.pool()).await.unwrap();
    assert_eq!(
        typed_error(
            page(
                &store,
                &metadata,
                scope,
                root,
                Kind::Children,
                1,
                first.next_cursor
            )
            .await
            .unwrap_err()
        ),
        OrderedTreeQueryError::StaleCursor
    );
    let first = page(&store, &metadata, scope, root, Kind::Search, 1, None)
        .await
        .unwrap();
    // Root is the first match (rank F, UUID allocated before child); remove its subtree.
    sqlx::query(&format!("delete from \"{table}\" node using \"{table}\" anchor where node.scope_id = $1 and node.tree_partition_id = $1 and anchor.scope_id = $1 and anchor.tree_partition_id = $1 and anchor.id = $2 and ARRAY[node.tree_path] OPERATOR(public.<@) anchor.tree_path", table = model.physical_table_name))
        .bind(scope).bind(first.items[0]).execute(store.pool()).await.unwrap();
    assert_eq!(
        typed_error(
            page(
                &store,
                &metadata,
                scope,
                root,
                Kind::Search,
                1,
                first.next_cursor
            )
            .await
            .unwrap_err()
        ),
        OrderedTreeQueryError::StaleCursor
    );
}
