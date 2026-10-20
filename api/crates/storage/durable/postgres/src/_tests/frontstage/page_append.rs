use super::*;
use control_plane_contracts::ports::{
    CreateFrontstagePageInput, CreateFrontstagePageTabInput, FrontstagePageRepository,
    MoveFrontstagePageInput,
};
use domain::frontstage::{
    FrontstageNavigationPlacement::Topbar, FrontstagePageContentPresentation,
};
use domain::FrontstagePageKind::{Group, Page};

fn input(workspace_id: Uuid, kind: domain::FrontstagePageKind) -> CreateFrontstagePageInput {
    let id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    CreateFrontstagePageInput {
        id,
        workspace_id,
        actor_user_id: Uuid::nil(),
        parent_id: None,
        kind,
        title: Some("Appended".into()),
        icon: None,
        tooltip: None,
        placement: Topbar,
        content_presentation: FrontstagePageContentPresentation::Single,
        slug: Some(format!("append-{id}")),
        rank: String::new(),
        default_tab: (kind == Page).then(|| CreateFrontstagePageTabInput {
            id: tab_id,
            workspace_id,
            actor_user_id: Uuid::nil(),
            page_id: id,
            title: Some("Default".into()),
            rank: "U".into(),
            is_default: true,
            route_segment: None,
            document_root_uid: format!("frontstage.tab.{tab_id}.root"),
        }),
    }
}

async fn topbar_ids(store: &crate::PgControlPlaneStore, workspace_id: Uuid) -> Vec<Uuid> {
    let mut records = store.list_frontstage_pages(workspace_id).await.unwrap();
    records.retain(|record| record.parent_id.is_none() && record.placement == Topbar);
    records.sort_by(|left, right| left.rank.cmp(&right.rank).then(left.id.cmp(&right.id)));
    assert!(records.windows(2).all(|pair| pair[0].rank < pair[1].rank));
    records.into_iter().map(|record| record.id).collect()
}

#[tokio::test]
async fn page_append_preserves_order_after_moves_deletes_and_legacy_ranks() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, other_workspace_id) = insert_frontstage_test_workspaces(&pool).await;
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let mut expected = Vec::new();
    // Numeric legacy ranks, including ties, must retain their existing id order.
    for (index, rank) in ["001000", "002500", "002500"].iter().enumerate() {
        let mut node = input(workspace_id, Group);
        node.id = Uuid::from_u128(100 + index as u128);
        node.rank = (*rank).into();
        store.create_frontstage_page(&node).await.unwrap();
        expected.push(node.id);
    }
    // Neither another workspace, a sidebar root nor a child determines the topbar tail.
    let mut other = input(other_workspace_id, Group);
    other.rank = "z".into();
    store.create_frontstage_page(&other).await.unwrap();
    for parent_id in [None, Some(expected[0])] {
        let mut sidebar = input(workspace_id, Group);
        sidebar.placement = domain::frontstage::FrontstageNavigationPlacement::Sidebar;
        sidebar.parent_id = parent_id;
        sidebar.slug = None;
        sidebar.rank = "z".into();
        store.create_frontstage_page(&sidebar).await.unwrap();
    }
    for kind in [Group, Page] {
        let node = input(workspace_id, kind);
        let created = store.create_frontstage_page(&node).await.unwrap();
        assert!(created.page.rank.as_str() < "z");
        expected.push(node.id);
        assert_eq!(topbar_ids(&store, workspace_id).await, expected);
    }
    let source = expected.remove(0);
    let target = *expected.last().unwrap();
    let source_page = store
        .get_frontstage_page(workspace_id, source)
        .await
        .unwrap()
        .unwrap();
    store
        .move_frontstage_page(&MoveFrontstagePageInput {
            workspace_id,
            actor_user_id: Uuid::nil(),
            page_id: source,
            parent_id: None,
            rank: String::new(),
            before_id: None,
            after_id: Some(target),
            placement: Topbar,
            slug: source_page.slug,
            descendant_placement: None,
        })
        .await
        .unwrap();
    expected.push(source);
    let deleted = expected.remove(1);
    store
        .delete_frontstage_page(workspace_id, deleted)
        .await
        .unwrap();
    for kind in [Group, Page] {
        let node = input(workspace_id, kind);
        store.create_frontstage_page(&node).await.unwrap();
        expected.push(node.id);
        // A fresh repository read proves the persisted order, independent of UI state.
        assert_eq!(topbar_ids(&store, workspace_id).await, expected);
    }
    assert_eq!(
        store
            .get_frontstage_page(other_workspace_id, other.id)
            .await
            .unwrap()
            .unwrap()
            .rank,
        "z"
    );
}

#[tokio::test]
async fn page_append_serializes_concurrent_creates_in_an_empty_topbar() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let group = input(workspace_id, Group);
    let page = input(workspace_id, Page);
    let (first, second) = tokio::join!(
        store.create_frontstage_page(&group),
        store.create_frontstage_page(&page)
    );
    first.unwrap();
    second.unwrap();
    let ids = topbar_ids(&store, workspace_id).await;
    assert_eq!(ids.len(), 2);
    assert!(ids.contains(&group.id) && ids.contains(&page.id));
}

#[tokio::test]
async fn page_append_bounds_rank_growth_and_rolls_back_failed_creation() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let store = crate::PgControlPlaneStore::new(pool.clone());
    let mut existing = input(workspace_id, Group);
    existing.rank = "z".repeat(32);
    store.create_frontstage_page(&existing).await.unwrap();
    let mut conflict = input(workspace_id, Group);
    conflict.slug = existing.slug.clone();
    assert!(store.create_frontstage_page(&conflict).await.is_err());
    assert_eq!(
        store
            .get_frontstage_page(workspace_id, existing.id)
            .await
            .unwrap()
            .unwrap()
            .rank,
        existing.rank
    );
    let appended = input(workspace_id, Page);
    let created = store.create_frontstage_page(&appended).await.unwrap();
    assert!(created.page.rank.len() <= 32);
    assert_eq!(
        topbar_ids(&store, workspace_id).await,
        vec![existing.id, appended.id]
    );
}
