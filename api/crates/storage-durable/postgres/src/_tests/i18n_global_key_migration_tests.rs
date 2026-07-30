use std::borrow::Cow;

use sqlx::migrate::Migrator;
use uuid::Uuid;

const MIGRATION_VERSION: i64 = 20260730100000;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

fn before_migrator() -> Migrator {
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

async fn legacy_workspace(pool: &sqlx::PgPool) -> Uuid {
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(pool)
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name) values ($1, $2, 'I18n key migration')",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .execute(pool)
    .await
    .unwrap();
    workspace_id
}

async fn legacy_release(pool: &sqlx::PgPool, workspace_id: Uuid) -> Uuid {
    let release_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into i18n_catalog_releases (
          id, workspace_id, schema_version, catalog_version, source_locale,
          locales, modules, generated_at, semantic_sha256
        ) values (
          $1, $2, '1flowbase.i18n-catalog-seed/v1', 'legacy', 'en_US',
          array['en_US', 'zh_Hans'],
          array['@1flowbase/console/settings', '@1flowbase/web/settings'],
          now(), $3
        )
        "#,
    )
    .bind(release_id)
    .bind(workspace_id)
    .bind(format!("sha256:{}", "a".repeat(64)))
    .execute(pool)
    .await
    .unwrap();
    release_id
}

async fn insert_legacy_message(
    pool: &sqlx::PgPool,
    release_id: Uuid,
    module: &str,
    translation: &str,
) {
    sqlx::query(
        "insert into i18n_catalog_release_messages (release_id, module, msgid) values ($1, $2, 'Settings')",
    )
    .bind(release_id)
    .bind(module)
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into i18n_catalog_release_translations
          (release_id, module, msgid, locale, translation)
        values ($1, $2, 'Settings', 'zh_Hans', $3)
        "#,
    )
    .bind(release_id)
    .bind(module)
    .bind(translation)
    .execute(pool)
    .await
    .unwrap();
}

#[tokio::test]
async fn migration_merges_equal_cross_module_values_and_preserves_catalog_state() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    before_migrator().run(&pool).await.unwrap();
    let workspace_id = legacy_workspace(&pool).await;
    let release_id = legacy_release(&pool, workspace_id).await;
    for module in ["@1flowbase/console/settings", "@1flowbase/web/settings"] {
        insert_legacy_message(&pool, release_id, module, "设置").await;
        sqlx::query(
            r#"
            insert into i18n_catalog_release_files (release_id, module, locale, path, sha256)
            values ($1, $2, 'zh_Hans', 'settings/zh_Hans.json', $3)
            "#,
        )
        .bind(release_id)
        .bind(module)
        .bind(format!("sha256:{}", "b".repeat(64)))
        .execute(&pool)
        .await
        .unwrap();
    }
    sqlx::query(
        "insert into workspace_i18n_catalog_states (workspace_id, active_release_id, revision) values ($1, $2, 7)",
    )
    .bind(workspace_id)
    .bind(release_id)
    .execute(&pool)
    .await
    .unwrap();
    for (table, translation) in [
        ("workspace_i18n_catalog_overrides", "工作区设置"),
        ("workspace_i18n_catalog_custom_translations", "本地设置"),
    ] {
        for module in ["@1flowbase/console/settings", "@1flowbase/web/settings"] {
            let statement = format!(
                "insert into {table} (workspace_id, module, msgid, locale, translation) values ($1, $2, 'Settings', 'zh_Hans', $3)"
            );
            sqlx::query(&statement)
                .bind(workspace_id)
                .bind(module)
                .bind(translation)
                .execute(&pool)
                .await
                .unwrap();
        }
    }
    for module in ["@1flowbase/console/settings", "@1flowbase/web/settings"] {
        sqlx::query(
            r#"
            insert into workspace_i18n_catalog_obsolete_messages
              (workspace_id, module, msgid, obsolete_since_release_id)
            values ($1, $2, 'Legacy settings', $3)
            "#,
        )
        .bind(workspace_id)
        .bind(module)
        .bind(release_id)
        .execute(&pool)
        .await
        .unwrap();
    }

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let descriptor: (String, i64) = sqlx::query_as(
        "select schema_version, revision from i18n_catalog_releases release join workspace_i18n_catalog_states state on state.active_release_id = release.id where release.id = $1",
    )
    .bind(release_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(descriptor, ("1flowbase.i18n-catalog-seed/v2".into(), 7));
    let translations: Vec<(String, String, String)> = sqlx::query_as(
        "select key, locale, translation from i18n_catalog_release_translations where release_id = $1 order by locale",
    )
    .bind(release_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(
        translations,
        vec![
            ("Settings".into(), "en_US".into(), "Settings".into()),
            ("Settings".into(), "zh_Hans".into(), "设置".into()),
        ]
    );
    let layer_counts: (i64, i64, i64, i64) = sqlx::query_as(
        r#"
        select
          (select count(*) from i18n_catalog_release_files where release_id = $1),
          (select count(*) from workspace_i18n_catalog_overrides where workspace_id = $2),
          (select count(*) from workspace_i18n_catalog_custom_translations where workspace_id = $2),
          (select count(*) from workspace_i18n_catalog_obsolete_messages where workspace_id = $2)
        "#,
    )
    .bind(release_id)
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(layer_counts, (1, 1, 1, 1));
    let layers: (String, String, Uuid) = sqlx::query_as(
        r#"
        select override_value.translation, custom_value.translation,
               obsolete.obsolete_since_release_id
        from workspace_i18n_catalog_overrides override_value
        join workspace_i18n_catalog_custom_translations custom_value
          on custom_value.workspace_id = override_value.workspace_id
         and custom_value.key = override_value.key
         and custom_value.locale = override_value.locale
        join workspace_i18n_catalog_obsolete_messages obsolete
          on obsolete.workspace_id = override_value.workspace_id
        where override_value.workspace_id = $1
        "#,
    )
    .bind(workspace_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(layers, ("工作区设置".into(), "本地设置".into(), release_id));
    assert!(sqlx::query(
        "update i18n_catalog_releases set catalog_version = 'mutated' where id = $1"
    )
    .bind(release_id)
    .execute(&pool)
    .await
    .is_err());
}

#[tokio::test]
async fn migration_fails_before_cutover_on_different_cross_module_translations() {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    before_migrator().run(&pool).await.unwrap();
    let workspace_id = legacy_workspace(&pool).await;
    let release_id = legacy_release(&pool, workspace_id).await;
    insert_legacy_message(&pool, release_id, "@1flowbase/console/settings", "设置").await;
    insert_legacy_message(&pool, release_id, "@1flowbase/web/settings", "配置").await;

    let error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();

    assert!(error
        .to_string()
        .contains("release key and locale have different translations across modules"));
    let legacy_count: i64 = sqlx::query_scalar(
        "select count(*) from i18n_catalog_release_translations where release_id = $1",
    )
    .bind(release_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_count, 2);
}
