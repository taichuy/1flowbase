use std::collections::BTreeMap;

use control_plane::i18n_catalog::management::{
    CatalogManagementAccess, DeleteCustomMessageCommand, GetCatalogEntryCommand,
    I18nCatalogManagementService, ListCatalogEntriesCommand, RestoreAllOfficialOverridesCommand,
    RestoreOfficialTranslationCommand, UpsertCustomTranslationCommand,
    UpsertOfficialOverrideCommand,
};
use control_plane::ports::{
    BootstrapRepository, CatalogManagementOrigin, CatalogResolutionRepository,
    DeleteCatalogTranslationInput, DeleteCustomCatalogMessageInput, I18nCatalogRepository,
    RuntimeI18nCatalogRepository, UpsertCatalogTranslationInput,
};
use domain::{
    ActorContext, CatalogDigest, CatalogLocale, CatalogMessageIdentity, CatalogModuleId,
    CatalogSeedFile, CatalogTranslation, CatalogVersion, OfficialCatalogMessage,
    VerifiedCatalogRelease, WorkspaceCatalogRevision,
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

async fn empty_store() -> PgControlPlaneStore {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
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
    CatalogTranslation::new(
        identity(msgid),
        CatalogLocale::new("zh_Hans").unwrap(),
        value,
    )
    .unwrap()
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
                CatalogLocale::new("zh_Hans").unwrap(),
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
            CatalogLocale::new("zh_Hans").unwrap(),
        ],
        vec![module()],
        vec![CatalogSeedFile::new(
            module(),
            CatalogLocale::new("zh_Hans").unwrap(),
            "console/settings/zh_Hans.json",
            digest('b'),
        )
        .unwrap()],
        OffsetDateTime::UNIX_EPOCH,
        digest('a'),
        official_messages,
    )
    .unwrap()
}

fn official_seed(release_id: Uuid) -> control_plane::i18n_catalog::VerifiedOfficialCatalogSeed {
    let release = release(Uuid::nil(), release_id, "1.0.0", &[("Settings", "设置")]);
    control_plane::i18n_catalog::VerifiedOfficialCatalogSeed::new(
        release.id(),
        release.catalog_version().clone(),
        release.locales().to_vec(),
        release.modules().to_vec(),
        release.files().to_vec(),
        release.generated_at(),
        release.semantic_sha256().clone(),
        release.messages().to_vec(),
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
    assert_eq!(
        official[0]
            .message()
            .translations()
            .get(&CatalogLocale::new("zh_Hans").unwrap())
            .map(String::as_str),
        Some("工作区设置")
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

    for locale in ["en_US", "zh_Hans", "fil_Latn", "en"] {
        sqlx::query(
            r#"
            insert into i18n_catalog_release_files (release_id, module, locale, path, sha256)
            values ($1, '@1flowbase/console/settings', $2, $3, $4)
            "#,
        )
        .bind(release_id)
        .bind(locale)
        .bind(format!("locale-grammar/{locale}.json"))
        .bind(format!("sha256:{}", "c".repeat(64)))
        .execute(store.pool())
        .await
        .unwrap();
    }
    for locale in ["zh_hans", "zh_", "zh_Hans_CN", "zh_H4ns", "zh__Hans"] {
        assert!(sqlx::query(
            r#"
            insert into i18n_catalog_release_files (release_id, module, locale, path, sha256)
            values ($1, '@1flowbase/console/settings', $2, $3, $4)
            "#,
        )
        .bind(release_id)
        .bind(locale)
        .bind(format!("invalid-locale/{locale}.json"))
        .bind(format!("sha256:{}", "d".repeat(64)))
        .execute(store.pool())
        .await
        .is_err());
    }

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

#[tokio::test]
async fn ac_003_combined_root_bootstrap_is_atomic_and_restart_idempotent() {
    let store = empty_store().await;
    let tenant = BootstrapRepository::upsert_root_tenant(&store)
        .await
        .unwrap();
    let seed = official_seed(Uuid::now_v7());

    let workspace = BootstrapRepository::upsert_root_workspace_with_official_catalog(
        &store,
        tenant.id,
        "Root catalog workspace",
        &seed,
    )
    .await
    .unwrap();
    let initial_state = I18nCatalogRepository::get_workspace_catalog_state(&store, workspace.id)
        .await
        .unwrap()
        .unwrap();
    let active_release_id = initial_state.active_release_id().unwrap();

    let override_state = I18nCatalogRepository::upsert_catalog_override(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id: workspace.id,
            value: translation("Settings", "根覆盖"),
            expected_revision: initial_state.revision(),
        },
    )
    .await
    .unwrap();
    let custom_state = I18nCatalogRepository::upsert_custom_catalog_translation(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id: workspace.id,
            value: translation("custom.key", "自定义"),
            expected_revision: override_state.revision(),
        },
    )
    .await
    .unwrap();

    let restarted = BootstrapRepository::upsert_root_workspace_with_official_catalog(
        &store,
        tenant.id,
        "Root catalog workspace",
        &seed,
    )
    .await
    .unwrap();
    let restarted_state = I18nCatalogRepository::get_workspace_catalog_state(&store, workspace.id)
        .await
        .unwrap()
        .unwrap();

    assert_eq!(restarted.id, workspace.id);
    assert_eq!(restarted_state.active_release_id(), Some(active_release_id));
    assert_eq!(restarted_state.revision(), custom_state.revision());
    let release_count: i64 =
        sqlx::query_scalar("select count(*) from i18n_catalog_releases where workspace_id = $1")
            .bind(workspace.id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    let override_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_overrides where workspace_id = $1",
    )
    .bind(workspace.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let custom_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_custom_translations where workspace_id = $1",
    )
    .bind(workspace.id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!((release_count, override_count, custom_count), (1, 1, 1));
}

#[tokio::test]
async fn ac_003_combined_failure_leaves_neither_workspace_nor_catalog_state() {
    let store = empty_store().await;
    let tenant = BootstrapRepository::upsert_root_tenant(&store)
        .await
        .unwrap();
    sqlx::query(
        r#"
        create function reject_bootstrap_catalog_release() returns trigger language plpgsql as $$
        begin raise exception 'controlled combined bootstrap failure'; end;
        $$
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        create trigger reject_bootstrap_catalog_release
        before insert on i18n_catalog_releases
        for each row execute function reject_bootstrap_catalog_release()
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();

    assert!(
        BootstrapRepository::upsert_root_workspace_with_official_catalog(
            &store,
            tenant.id,
            "Rollback catalog workspace",
            &official_seed(Uuid::now_v7()),
        )
        .await
        .is_err()
    );

    let workspace_count: i64 =
        sqlx::query_scalar("select count(*) from workspaces where name = $1")
            .bind("Rollback catalog workspace")
            .fetch_one(store.pool())
            .await
            .unwrap();
    let state_count: i64 = sqlx::query_scalar("select count(*) from workspace_i18n_catalog_states")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let release_count: i64 = sqlx::query_scalar("select count(*) from i18n_catalog_releases")
        .fetch_one(store.pool())
        .await
        .unwrap();
    assert_eq!((workspace_count, state_count, release_count), (0, 0, 0));
}

#[tokio::test]
async fn ac_004_resolution_projection_is_exact_locale_and_supports_custom_identity() {
    let store = empty_store().await;
    let tenant = BootstrapRepository::upsert_root_tenant(&store)
        .await
        .unwrap();
    let workspace = BootstrapRepository::upsert_root_workspace_with_official_catalog(
        &store,
        tenant.id,
        "Resolver catalog workspace",
        &official_seed(Uuid::now_v7()),
    )
    .await
    .unwrap();
    let state = I18nCatalogRepository::get_workspace_catalog_state(&store, workspace.id)
        .await
        .unwrap()
        .unwrap();
    let official = CatalogResolutionRepository::find_catalog_resolution_candidate(
        &store,
        workspace.id,
        &identity("Settings"),
        &CatalogLocale::new("zh_Hans").unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(official.active_official.as_deref(), Some("设置"));

    let overridden_state = I18nCatalogRepository::upsert_catalog_override(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id: workspace.id,
            value: translation("Settings", "根覆盖"),
            expected_revision: state.revision(),
        },
    )
    .await
    .unwrap();
    I18nCatalogRepository::upsert_custom_catalog_translation(
        &store,
        &UpsertCatalogTranslationInput {
            workspace_id: workspace.id,
            value: translation("custom.key", "自定义"),
            expected_revision: overridden_state.revision(),
        },
    )
    .await
    .unwrap();

    let custom = CatalogResolutionRepository::find_catalog_resolution_candidate(
        &store,
        workspace.id,
        &identity("custom.key"),
        &CatalogLocale::new("zh_Hans").unwrap(),
    )
    .await
    .unwrap();
    let overridden = CatalogResolutionRepository::find_catalog_resolution_candidate(
        &store,
        workspace.id,
        &identity("Settings"),
        &CatalogLocale::new("zh_Hans").unwrap(),
    )
    .await
    .unwrap();
    let alternate = CatalogResolutionRepository::find_catalog_resolution_candidate(
        &store,
        workspace.id,
        &identity("custom.key"),
        &CatalogLocale::new("fr_FR").unwrap(),
    )
    .await
    .unwrap();

    assert_eq!(custom.root_override.as_deref(), Some("自定义"));
    assert_eq!(custom.active_official, None);
    assert_eq!(overridden.root_override.as_deref(), Some("根覆盖"));
    assert_eq!(overridden.active_official.as_deref(), Some("设置"));
    assert_eq!(alternate.root_override, None);
    assert_eq!(alternate.active_official, None);
}

#[tokio::test]
async fn runtime_projection_rejects_a_corrupt_stored_module_identity() {
    let store = empty_store().await;
    let tenant = BootstrapRepository::upsert_root_tenant(&store)
        .await
        .unwrap();
    let workspace = BootstrapRepository::upsert_root_workspace_with_official_catalog(
        &store,
        tenant.id,
        "Corrupt runtime catalog workspace",
        &official_seed(Uuid::now_v7()),
    )
    .await
    .unwrap();
    sqlx::query(
        r#"
        alter table workspace_i18n_catalog_custom_translations
        drop constraint workspace_i18n_catalog_custom_translations_module_check
        "#,
    )
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into workspace_i18n_catalog_custom_translations (
          workspace_id, module, msgid, locale, translation
        ) values ($1, 'invalid/module', 'corrupt.key', 'zh_Hans', '损坏')
        "#,
    )
    .bind(workspace.id)
    .execute(store.pool())
    .await
    .unwrap();

    let error = RuntimeI18nCatalogRepository::project_runtime_catalog(
        &store,
        workspace.id,
        &CatalogLocale::new("zh_Hans").unwrap(),
    )
    .await
    .unwrap_err();

    assert_eq!(
        error.downcast_ref::<domain::I18nCatalogInvariantError>(),
        Some(&domain::I18nCatalogInvariantError::InvalidModuleId)
    );
}

fn root_access(workspace_id: Uuid) -> CatalogManagementAccess {
    CatalogManagementAccess {
        actor: ActorContext::root(Uuid::now_v7(), workspace_id, "root"),
        current_workspace_id: workspace_id,
    }
}

async fn management_store() -> (PgControlPlaneStore, Uuid, WorkspaceCatalogRevision) {
    let store = empty_store().await;
    let tenant = BootstrapRepository::upsert_root_tenant(&store)
        .await
        .unwrap();
    let workspace = BootstrapRepository::upsert_root_workspace_with_official_catalog(
        &store,
        tenant.id,
        "Management catalog workspace",
        &official_seed(Uuid::now_v7()),
    )
    .await
    .unwrap();
    let state = I18nCatalogRepository::get_workspace_catalog_state(&store, workspace.id)
        .await
        .unwrap()
        .unwrap();
    (store, workspace.id, state.revision())
}

#[tokio::test]
async fn ac_007_management_is_root_bootstrap_only_and_projects_searchable_entries() {
    let (store, workspace_id, revision) = management_store().await;
    let service = I18nCatalogManagementService::new(store, workspace_id);
    let page = service
        .list(ListCatalogEntriesCommand {
            access: root_access(workspace_id),
            module: Some(module()),
            locale: Some(CatalogLocale::new("zh_Hans").unwrap()),
            search: Some("settings".into()),
            origin: Some(CatalogManagementOrigin::Official),
            offset: 0,
            limit: 20,
        })
        .await
        .unwrap();
    assert_eq!(page.revision, revision);
    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].msgid, "Settings");
    assert_eq!(
        page.entries[0].official_translation.as_deref(),
        Some("设置")
    );
    assert_eq!(page.entries[0].effective_value, "设置");
    assert!(!page.entries[0].missing);
    assert!(!page.entries[0].obsolete);
    let detail = service
        .detail(GetCatalogEntryCommand {
            access: root_access(workspace_id),
            identity: identity("Settings"),
            locale: CatalogLocale::new("zh_Hans").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(detail.msgid, "Settings");

    let non_root = CatalogManagementAccess {
        actor: ActorContext::scoped(Uuid::now_v7(), workspace_id, "admin", std::iter::empty()),
        current_workspace_id: workspace_id,
    };
    assert!(service
        .list(ListCatalogEntriesCommand {
            access: non_root,
            module: None,
            locale: None,
            search: None,
            origin: None,
            offset: 0,
            limit: 20,
        })
        .await
        .is_err());
    let foreign = Uuid::now_v7();
    assert!(service
        .list(ListCatalogEntriesCommand {
            access: CatalogManagementAccess {
                actor: ActorContext::root(Uuid::now_v7(), foreign, "root"),
                current_workspace_id: foreign,
            },
            module: None,
            locale: None,
            search: None,
            origin: None,
            offset: 0,
            limit: 20,
        })
        .await
        .is_err());
}

#[tokio::test]
async fn ac_008_expected_revision_serializes_atomic_mutation_and_audit() {
    let (store, workspace_id, revision) = management_store().await;
    let service = I18nCatalogManagementService::new(store.clone(), workspace_id);
    let actor = root_access(workspace_id);
    let state = service
        .upsert_official_override(UpsertOfficialOverrideCommand {
            access: actor.clone(),
            value: translation("Settings", "覆盖"),
            expected_revision: revision,
        })
        .await
        .unwrap();
    assert_eq!(state.revision().value(), revision.value() + 1);

    let audit_row: (i64, Option<Uuid>, String) = sqlx::query_as(
        r#"select count(*) over(), actor_user_id, payload ->> 'locale'
           from audit_logs where workspace_id = $1 and event_code = $2"#,
    )
    .bind(workspace_id)
    .bind("i18n_catalog.official_override.upserted")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(audit_row.0, 1);
    assert_eq!(audit_row.1, Some(actor.actor.user_id));
    assert_eq!(audit_row.2, "zh_Hans");

    assert!(service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: actor,
            value: translation("custom.stale", "不会写入"),
            expected_revision: revision,
        })
        .await
        .is_err());
    let custom_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_custom_translations where workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let all_audits: i64 =
        sqlx::query_scalar("select count(*) from audit_logs where workspace_id = $1")
            .bind(workspace_id)
            .fetch_one(store.pool())
            .await
            .unwrap();
    assert_eq!((custom_count, all_audits), (0, 1));
}

#[tokio::test]
async fn ac_009_restore_and_explicit_custom_delete_preserve_distinct_lifecycles() {
    let (store, workspace_id, revision) = management_store().await;
    let service = I18nCatalogManagementService::new(store.clone(), workspace_id);
    let overridden = service
        .upsert_official_override(UpsertOfficialOverrideCommand {
            access: root_access(workspace_id),
            value: translation("Settings", "覆盖"),
            expected_revision: revision,
        })
        .await
        .unwrap();
    let custom = service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: root_access(workspace_id),
            value: translation("custom.message", "自定义"),
            expected_revision: overridden.revision(),
        })
        .await
        .unwrap();
    assert!(service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: root_access(workspace_id),
            value: translation("Settings", "冲突"),
            expected_revision: custom.revision(),
        })
        .await
        .is_err());

    let restored = service
        .restore_official_translation(RestoreOfficialTranslationCommand {
            access: root_access(workspace_id),
            identity: identity("Settings"),
            locale: CatalogLocale::new("zh_Hans").unwrap(),
            expected_revision: custom.revision(),
        })
        .await
        .unwrap();
    let official = CatalogResolutionRepository::find_catalog_resolution_candidate(
        &store,
        workspace_id,
        &identity("Settings"),
        &CatalogLocale::new("zh_Hans").unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(official.root_override, None);
    assert_eq!(official.active_official.as_deref(), Some("设置"));

    let overridden_again = service
        .upsert_official_override(UpsertOfficialOverrideCommand {
            access: root_access(workspace_id),
            value: translation("Settings", "再次覆盖"),
            expected_revision: restored.revision(),
        })
        .await
        .unwrap();
    let globally_restored = service
        .restore_all_official_overrides(RestoreAllOfficialOverridesCommand {
            access: root_access(workspace_id),
            expected_revision: overridden_again.revision(),
        })
        .await
        .unwrap();
    let custom_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_custom_translations where workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    let override_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_overrides where workspace_id = $1",
    )
    .bind(workspace_id)
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!((override_count, custom_count), (0, 1));

    service
        .delete_custom_message(DeleteCustomMessageCommand {
            access: root_access(workspace_id),
            identity: identity("custom.message"),
            expected_revision: globally_restored.revision(),
        })
        .await
        .unwrap();
    let delete_audits: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where workspace_id = $1 and event_code = $2",
    )
    .bind(workspace_id)
    .bind("i18n_catalog.custom_message.deleted")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(delete_audits, 1);
}

#[tokio::test]
async fn ac_008_concurrent_expected_revision_has_exactly_one_winner() {
    let (store, workspace_id, revision) = management_store().await;
    let first = I18nCatalogManagementService::new(store.clone(), workspace_id);
    let second = I18nCatalogManagementService::new(store.clone(), workspace_id);
    let left = first.upsert_official_override(UpsertOfficialOverrideCommand {
        access: root_access(workspace_id),
        value: translation("Settings", "一"),
        expected_revision: revision,
    });
    let right = second.upsert_official_override(UpsertOfficialOverrideCommand {
        access: root_access(workspace_id),
        value: translation("Settings", "二"),
        expected_revision: revision,
    });
    let (left, right) = tokio::join!(left, right);
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let audits: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where workspace_id = $1 and event_code = $2",
    )
    .bind(workspace_id)
    .bind("i18n_catalog.official_override.upserted")
    .fetch_one(store.pool())
    .await
    .unwrap();
    assert_eq!(audits, 1);
}
