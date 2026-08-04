use super::*;

#[tokio::test]
async fn page_creation_keeps_one_default_tab_and_last_tab_is_guarded() {
    use control_plane::ports::{
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
    let store = storage_postgres::PgControlPlaneStore::new(pool.clone());
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

#[tokio::test]
async fn block_creation_commits_document_code_and_audit_atomically() {
    use control_plane::{
        audit::audit_log,
        ports::{
            CreateFrontstageBlockInput, CreateFrontstagePageInput, CreateFrontstagePageTabInput,
            FrontstagePageRepository,
        },
    };

    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let actor_user_id = Uuid::now_v7();
    sqlx::query(
        "insert into users (id, account, email, password_hash, name, nickname, status) values ($1, $2, $3, 'x', 'Atomic Block', 'Atomic Block', 'active')",
    )
    .bind(actor_user_id)
    .bind(format!("atomic-block-{actor_user_id}"))
    .bind(format!("atomic-block-{actor_user_id}@example.com"))
    .execute(&pool)
    .await
    .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, '00000000-0000-0000-0000-000000000001', 'Atomic Block', $2, $2)",
    )
    .bind(workspace_id)
    .bind(actor_user_id)
    .execute(&pool)
    .await
    .unwrap();

    let store = storage_postgres::PgControlPlaneStore::new(pool.clone());
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    store
        .create_frontstage_page(&CreateFrontstagePageInput {
            id: page_id,
            workspace_id,
            actor_user_id,
            parent_id: None,
            kind: domain::FrontstagePageKind::Page,
            title: Some("Atomic Page".into()),
            icon: None,
            tooltip: None,
            placement: domain::frontstage::FrontstageNavigationPlacement::Topbar,
            content_presentation: domain::frontstage::FrontstagePageContentPresentation::Single,
            slug: Some("atomic-page".into()),
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

    let first_document = json!({
        "version": 1,
        "blocks": [{
            "id": "first",
            "codeRef": "first-code",
            "renderer_version": "v1"
        }]
    });
    let audit_id = Uuid::now_v7();
    let mut first_audit = audit_log(
        Some(workspace_id),
        Some(actor_user_id),
        "frontstage_page",
        Some(page_id),
        "frontstage.block_created",
        json!({ "code_ref": "first-code" }),
    );
    first_audit.id = audit_id;
    let detail = store
        .create_frontstage_block(&CreateFrontstageBlockInput {
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            document_payload: first_document.clone(),
            code_ref: "first-code".into(),
            code: "export default function First() { return null; }".into(),
            audit_log: first_audit,
        })
        .await
        .unwrap();
    assert_eq!(detail.document.payload, first_document);
    let saved_code: String = sqlx::query_scalar(
        "select code from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = 'first-code'",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(saved_code.contains("function First"));

    let mut duplicate_audit = audit_log(
        Some(workspace_id),
        Some(actor_user_id),
        "frontstage_page",
        Some(page_id),
        "frontstage.block_created",
        json!({ "code_ref": "rolled-back-code" }),
    );
    duplicate_audit.id = audit_id;
    let failed_document = json!({
        "version": 1,
        "blocks": [{
            "id": "rolled-back",
            "codeRef": "rolled-back-code",
            "renderer_version": "v1"
        }]
    });
    store
        .create_frontstage_block(&CreateFrontstageBlockInput {
            workspace_id,
            actor_user_id,
            page_id,
            tab_id,
            document_payload: failed_document,
            code_ref: "rolled-back-code".into(),
            code: "export default function RolledBack() { return null; }".into(),
            audit_log: duplicate_audit,
        })
        .await
        .unwrap_err();

    let persisted_document: Value = sqlx::query_scalar(
        "select document_payload from frontstage_page_schemas where workspace_id = $1 and tab_id = $2",
    )
    .bind(workspace_id)
    .bind(tab_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    let rolled_back_code_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from frontstage_block_codes where workspace_id = $1 and page_id = $2 and code_ref = 'rolled-back-code')",
    )
    .bind(workspace_id)
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(persisted_document, first_document);
    assert!(!rolled_back_code_exists);
}
