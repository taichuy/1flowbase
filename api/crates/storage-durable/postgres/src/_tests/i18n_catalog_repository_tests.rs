use std::collections::BTreeMap;

use control_plane::ports::{
    DeleteCatalogTranslationInput, DeleteCustomCatalogMessageInput, I18nCatalogRepository,
    UpsertCatalogTranslationInput,
};
use domain::{
    CatalogDigest, CatalogLocale, CatalogMessageIdentity, CatalogModuleId, CatalogSeedFile,
    CatalogTranslation, CatalogVersion, OfficialCatalogMessage, VerifiedCatalogRelease,
    WorkspaceCatalogRevision,
};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use time::OffsetDateTime;
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn seed_store() -> (PgControlPlaneStore, Uuid) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool);
    let tenant = store.upsert_root_tenant().await.unwrap();
    let workspace = store
        .upsert_workspace(tenant.id, "I18n Catalog")
        .await
        .unwrap();
    (store, workspace.id)
}

fn digest(character: char) -> CatalogDigest {
    CatalogDigest::new(format!("sha256:{}", character.to_string().repeat(64))).unwrap()
}

fn module() -> CatalogModuleId {
    CatalogModuleId::new("@1flowbase/console/settings").unwrap()
}

fn identity(msgid: &str) -> CatalogMessageIdentity {
    CatalogMessageIdentity::new(module(), msgid).unwrap()
}

fn translation(msgid: &str, value: &str) -> CatalogTranslation {
    CatalogTranslation::new(identity(msgid), CatalogLocale::new("zh_CN").unwrap(), value).unwrap()
}

fn release(
    workspace_id: Uuid,
    release_id: Uuid,
    version: &str,
    messages: &[(&str, &str)],
) -> VerifiedCatalogRelease {
    let official_messages = messages
        .iter()
        .map(|(msgid, translated)| {
            let mut translations = BTreeMap::new();
            translations.insert(
                CatalogLocale::new("zh_CN").unwrap(),
                (*translated).to_owned(),
            );
            OfficialCatalogMessage::new(identity(msgid), translations).unwrap()
        })
        .collect();
    VerifiedCatalogRelease::new(
        release_id,
        workspace_id,
        CatalogVersion::new(version).unwrap(),
        vec![
            CatalogLocale::source(),
            CatalogLocale::new("zh_CN").unwrap(),
        ],
        vec![module()],
        vec![CatalogSeedFile::new(
            module(),
            CatalogLocale::new("zh_CN").unwrap(),
            "console/settings/zh_CN.json",
            digest('b'),
        )
        .unwrap()],
        OffsetDateTime::UNIX_EPOCH,
        digest('a'),
        official_messages,
    )
    .unwrap()
}

#[tokio::test]
async fn immutable_release_rejects_duplicate_catalog_version() {
    let (store, workspace_id) = seed_store().await;
    let first = release(
        workspace_id,
        Uuid::now_v7(),
        "2026.07.28",
        &[("Settings", "设置")],
    );
    I18nCatalogRepository::import_verified_release(&store, &first)
        .await
        .unwrap();
    let duplicate = release(
        workspace_id,
        Uuid::now_v7(),
        "2026.07.28",
        &[("Settings", "设置二")],
    );

    assert!(
        I18nCatalogRepository::import_verified_release(&store, &duplicate)
            .await
            .is_err()
    );
    let count: i64 =
        sqlx::query_scalar("select count(*) from i18n_catalog_releases where workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!(count, 1);
}

#[tokio::test]
async fn activation_uses_one_state_row_and_rejects_stale_revision_without_partial_change() {
    let (store, workspace_id) = seed_store().await;
    let first_id = Uuid::now_v7();
    let second_id = Uuid::now_v7();
    for release in [
        release(workspace_id, first_id, "v1", &[("Settings", "设置")]),
        release(workspace_id, second_id, "v2", &[("Preferences", "偏好")]),
    ] {
        I18nCatalogRepository::import_verified_release(&store, &release)
            .await
            .unwrap();
    }
    let initial = I18nCatalogRepository::bootstrap_workspace_catalog_state(&store, workspace_id)
        .await
        .unwrap();
    I18nCatalogRepository::bootstrap_workspace_catalog_state(&store, workspace_id)
        .await
        .unwrap();
    let activated = I18nCatalogRepository::activate_verified_release(
        &store,
        workspace_id,
        first_id,
        initial.revision(),
    )
    .await
    .unwrap();

    assert!(I18nCatalogRepository::activate_verified_release(
        &store,
        workspace_id,
        second_id,
        WorkspaceCatalogRevision::initial(),
    )
    .await
    .is_err());
    let after_failure = I18nCatalogRepository::get_workspace_catalog_state(&store, workspace_id)
        .await
        .unwrap()
        .unwrap();
    assert_eq!(after_failure, activated);
    assert_eq!(after_failure.active_release_id(), Some(first_id));
    sqlx::query(
        r#"
        create function test_reject_i18n_obsolete_insert()
        returns trigger language plpgsql as $$
        begin
          raise exception 'controlled activation failure';
        end;
        $$;
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        create trigger test_reject_i18n_obsolete_insert
        before insert on workspace_i18n_catalog_obsolete_messages
        for each row execute function test_reject_i18n_obsolete_insert()
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    assert!(I18nCatalogRepository::activate_verified_release(
        &store,
        workspace_id,
        second_id,
        activated.revision(),
    )
    .await
    .is_err());
    let after_post_update_failure =
        I18nCatalogRepository::get_workspace_catalog_state(&store, workspace_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(after_post_update_failure, activated);
    let state_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_states where workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(state_count, 1);
}

#[tokio::test]
async fn activation_preserves_overrides_and_custom_rows_and_marks_english_rename_obsolete() {
    let (store, workspace_id) = seed_store().await;
    let first_id = Uuid::now_v7();
    let second_id = Uuid::now_v7();
    let first = release(workspace_id, first_id, "v1", &[("Settings", "设置")]);
    let second = release(
        workspace_id,
        second_id,
        "v2",
        &[("Workspace settings", "工作区设置")],
    );
    I18nCatalogRepository::import_verified_release(&store, &first)
        .await
        .unwrap();
    I18nCatalogRepository::import_verified_release(&store, &second)
        .await
        .unwrap();
    let initial = I18nCatalogRepository::bootstrap_workspace_catalog_state(&store, workspace_id)
        .await
        .unwrap();
    let active_v1 = I18nCatalogRepository::activate_verified_release(
        &store,
        workspace_id,
        first_id,
        initial.revision(),
    )
    .await
    .unwrap();
    let override_state = I18nCatalogRepository::upsert_catalog_override(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: translation("Settings", "配置"),
            expected_revision: active_v1.revision(),
        },
    )
    .await
    .unwrap();
    let custom_state = I18nCatalogRepository::upsert_custom_catalog_translation(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: translation("Local extension", "本地扩展"),
            expected_revision: override_state.revision(),
        },
    )
    .await
    .unwrap();

    I18nCatalogRepository::activate_verified_release(
        &store,
        workspace_id,
        second_id,
        custom_state.revision(),
    )
    .await
    .unwrap();

    let overrides = I18nCatalogRepository::list_catalog_overrides(&store, workspace_id)
        .await
        .unwrap();
    let custom = I18nCatalogRepository::list_custom_catalog_translations(&store, workspace_id)
        .await
        .unwrap();
    let official = I18nCatalogRepository::list_active_official_messages(&store, workspace_id)
        .await
        .unwrap();
    let obsolete = I18nCatalogRepository::list_obsolete_catalog_messages(&store, workspace_id)
        .await
        .unwrap();
    assert_eq!(overrides.len(), 1);
    assert_eq!(custom.len(), 1);
    assert_eq!(
        official[0].message().identity().msgid(),
        "Workspace settings"
    );
    assert_eq!(obsolete.len(), 1);
    assert_eq!(obsolete[0].identity().msgid(), "Settings");
}

#[tokio::test]
async fn override_and_custom_writes_share_the_database_protected_revision_token() {
    let (store, workspace_id) = seed_store().await;
    let initial = I18nCatalogRepository::bootstrap_workspace_catalog_state(&store, workspace_id)
        .await
        .unwrap();
    let override_value = translation("Settings", "配置");
    let override_state = I18nCatalogRepository::upsert_catalog_override(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: override_value.clone(),
            expected_revision: initial.revision(),
        },
    )
    .await
    .unwrap();

    assert!(I18nCatalogRepository::upsert_custom_catalog_translation(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: translation("Local extension", "本地扩展"),
            expected_revision: initial.revision(),
        },
    )
    .await
    .is_err());
    let deleted_override = I18nCatalogRepository::delete_catalog_override(
        &store,
        &DeleteCatalogTranslationInput {
            workspace_id,
            identity: override_value.identity().clone(),
            locale: override_value.locale().clone(),
            expected_revision: override_state.revision(),
        },
    )
    .await
    .unwrap();
    let custom_value = translation("Local extension", "本地扩展");
    let custom_state = I18nCatalogRepository::upsert_custom_catalog_translation(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id,
            value: custom_value.clone(),
            expected_revision: deleted_override.revision(),
        },
    )
    .await
    .unwrap();
    let deleted_custom = I18nCatalogRepository::delete_custom_catalog_message(
        &store,
        &DeleteCustomCatalogMessageInput {
            workspace_id,
            identity: custom_value.identity().clone(),
            expected_revision: custom_state.revision(),
        },
    )
    .await
    .unwrap();

    assert_eq!(deleted_custom.revision().value(), 4);
    assert!(
        I18nCatalogRepository::list_catalog_overrides(&store, workspace_id)
            .await
            .unwrap()
            .is_empty()
    );
    assert!(
        I18nCatalogRepository::list_custom_catalog_translations(&store, workspace_id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn migration_constraints_reject_invalid_revision_digest_and_cross_workspace_activation() {
    let (store, first_workspace_id) = seed_store().await;
    let tenant_id: Uuid = sqlx::query_scalar("select tenant_id from workspaces where id = $1")
        .bind(first_workspace_id)
        .fetch_one(store.pool())
        .await
        .unwrap();
    let second_workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name) values ($1, $2, 'Second catalog workspace')",
    )
    .bind(second_workspace_id)
    .bind(tenant_id)
    .execute(store.pool())
    .await
    .unwrap();
    let release_id = Uuid::now_v7();
    let valid = release(
        first_workspace_id,
        release_id,
        "valid",
        &[("Settings", "设置")],
    );
    I18nCatalogRepository::import_verified_release(&store, &valid)
        .await
        .unwrap();

    assert!(sqlx::query(
        "insert into workspace_i18n_catalog_states (workspace_id, revision) values ($1, -1)",
    )
    .bind(first_workspace_id)
    .execute(store.pool())
    .await
    .is_err());
    let initial =
        I18nCatalogRepository::bootstrap_workspace_catalog_state(&store, first_workspace_id)
            .await
            .unwrap();
    assert_eq!(initial.revision(), WorkspaceCatalogRevision::initial());
    assert!(sqlx::query(
        "update workspace_i18n_catalog_states set revision = 2 where workspace_id = $1",
    )
    .bind(first_workspace_id)
    .execute(store.pool())
    .await
    .is_err());
    assert!(sqlx::query(
        "update i18n_catalog_releases set catalog_version = 'mutated' where id = $1",
    )
    .bind(release_id)
    .execute(store.pool())
    .await
    .is_err());
    assert!(sqlx::query(
        r#"
        insert into i18n_catalog_releases (
          id, workspace_id, schema_version, catalog_version, source_locale,
          locales, modules, generated_at, semantic_sha256
        ) values ($1, $2, '1flowbase.i18n-catalog-seed/v1', 'bad', 'en_US',
                  array['en_US'], array['@1flowbase/console/settings'], now(), 'sha256:BAD')
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(first_workspace_id)
    .execute(store.pool())
    .await
    .is_err());
    assert!(sqlx::query(
        "insert into workspace_i18n_catalog_states (workspace_id, active_release_id) values ($1, $2)",
    )
    .bind(second_workspace_id)
    .bind(release_id)
    .execute(store.pool())
    .await
    .is_err());
}
