use control_plane::ports::RoleRepository;
use sqlx::PgPool;
use storage_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database_url() -> String {
    let admin_pool = PgPool::connect(&base_database_url()).await.unwrap();
    let schema = format!("test_{}", Uuid::now_v7().to_string().replace('-', ""));
    sqlx::query(&format!("create schema if not exists {schema}"))
        .execute(&admin_pool)
        .await
        .unwrap();

    format!("{}?options=-csearch_path%3D{schema}", base_database_url())
}

#[tokio::test]
async fn role_queries_respect_requested_workspace_instead_of_first_workspace() {
    let pool = connect(&isolated_database_url().await).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let first_workspace_id = Uuid::now_v7();
    let second_workspace_id = Uuid::now_v7();
    let first_workspace_name = format!("Workspace A {}", first_workspace_id.simple());
    let second_workspace_name = format!("Workspace B {}", second_workspace_id.simple());

    sqlx::query(
        r#"
        insert into workspaces (id, tenant_id, name, created_by, updated_by)
        values ($1, $2, $3, null, null),
               ($4, $2, $5, null, null)
        "#,
    )
    .bind(first_workspace_id)
    .bind(tenant_id)
    .bind(&first_workspace_name)
    .bind(second_workspace_id)
    .bind(&second_workspace_name)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into roles (
            id, scope_id, scope_kind, workspace_id, code, name, introduction, is_builtin, is_editable
        )
        values ($1, $2, 'workspace', $2, 'reviewer', 'Reviewer', '', false, true)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(second_workspace_id)
    .execute(store.pool())
    .await
    .unwrap();

    let roles = RoleRepository::list_roles(&store, second_workspace_id)
        .await
        .unwrap();

    assert_eq!(roles.len(), 1);
    assert_eq!(roles[0].code, "reviewer");
}
