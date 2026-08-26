use super::*;

#[tokio::test]
async fn page_creation_keeps_one_default_tab_and_last_tab_is_guarded() {
    use control_plane_contracts::ports::{
        CreateFrontstagePageInput, CreateFrontstagePageTabInput, FrontstagePageRepository,
    };
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let actor_user_id = Uuid::now_v7();
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status) values ($1, $2, $3, 'x', 'Issue 1232', 'Issue 1232', 'active')",
    )
    .bind(actor_user_id)
    .bind(format!("issue1232-{actor_user_id}"))
    .bind(format!("issue1232-{actor_user_id}@example.com"))
    .execute(&pool)
    .await
    .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, '00000000-0000-0000-0000-000000000001', $2, $3, $3)",
    )
    .bind(workspace_id)
    .bind(format!("Issue 1232 {workspace_id}"))
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();
    let store = storage_durable_postgres::PgControlPlaneStore::new(pool.clone());
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let creation = store
        .create_frontstage_page(&CreateFrontstagePageInput {
            id: page_id,
            workspace_id,
            actor_user_id,
            parent_id: None,
            kind: domain::FrontstagePageKind::Page,
            title: Some("Page".into()),
            icon: None,
            tooltip: None,
            placement: domain::frontstage::FrontstageNavigationPlacement::Topbar,
            content_presentation: domain::frontstage::FrontstagePageContentPresentation::Single,
            slug: Some("page-root".into()),
            rank: "a".into(),
            default_tab: Some(CreateFrontstagePageTabInput {
                id: tab_id,
                workspace_id,
                actor_user_id,
                page_id,
                title: Some("Default".into()),
                rank: "a".into(),
                is_default: true,
                route_segment: None,
                document_root_uid: format!("frontstage.tab.{tab_id}.root"),
            }),
        })
        .await
        .unwrap();
    assert_eq!(creation.default_tab.unwrap().id, tab_id);
    let tabs = store
        .list_frontstage_page_tabs(workspace_id, page_id)
        .await
        .unwrap();
    assert_eq!(tabs.len(), 1);
    assert_eq!(tabs.iter().filter(|tab| tab.is_default).count(), 1);
    let error = store
        .delete_frontstage_page_tab(workspace_id, page_id, tab_id, actor_user_id)
        .await
        .unwrap_err();
    assert!(error.to_string().contains("frontstage_page_requires_tab"));
}
