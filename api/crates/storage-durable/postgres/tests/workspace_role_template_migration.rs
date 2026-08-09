use std::borrow::Cow;

use sqlx::migrate::Migrator;
use storage_postgres::PgControlPlaneStore;
use uuid::Uuid;

const MIGRATION_VERSION: i64 = 20260809110000;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

fn before_workspace_role_template_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < MIGRATION_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

#[tokio::test]
async fn ac_002_historical_workspace_role_templates_become_user_owned() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_workspace_role_template_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "historical-workspace")
        .await
        .unwrap();

    for (code, default_member) in [("admin", false), ("member", true)] {
        sqlx::query(
            r#"
            insert into roles (
                id, scope_id, scope_kind, workspace_id, code, name, introduction,
                is_builtin, is_editable, auto_grant_new_permissions,
                is_default_member_role, system_kind
            )
            values ($1, $2, 'workspace', $2, $3, $3, '', true, true, false, $4, $3)
            "#,
        )
        .bind(Uuid::now_v7())
        .bind(workspace.id)
        .bind(code)
        .bind(default_member)
        .execute(store.pool())
        .await
        .unwrap();
    }

    sqlx::migrate!("./migrations")
        .run(store.pool())
        .await
        .unwrap();

    let roles: Vec<(String, bool, bool, Option<String>)> = sqlx::query_as(
        r#"
        select code, is_builtin, is_editable, system_kind
        from roles
        where workspace_id = $1 and code in ('admin', 'member')
        order by code
        "#,
    )
    .bind(workspace.id)
    .fetch_all(store.pool())
    .await
    .unwrap();

    assert_eq!(
        roles,
        vec![
            ("admin".to_string(), false, true, None),
            ("member".to_string(), false, true, None),
        ]
    );
}
