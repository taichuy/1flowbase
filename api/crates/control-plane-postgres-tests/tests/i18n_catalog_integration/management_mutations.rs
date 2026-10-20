use super::*;

#[tokio::test]
async fn ac_007_management_is_root_bootstrap_only_and_projects_searchable_entries() {
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let page = service
        .list(ListCatalogEntriesCommand {
            access: fixture.root_access(),
            key: Some("Settings".to_owned()),
            locale: Some(CatalogLocale::new("zh_Hans").unwrap()),
            search: Some("settings".into()),
            origin: Some(CatalogManagementOrigin::Official),
            offset: 0,
            limit: 20,
        })
        .await
        .unwrap();
    assert_eq!(page.revision, fixture.revision);
    assert_eq!(page.total, 1);
    assert_eq!(page.entries[0].key, "Settings");
    assert_eq!(
        page.entries[0].official_translation.as_deref(),
        Some("设置")
    );
    assert_eq!(page.entries[0].effective_value, "设置");
    assert!(!page.entries[0].missing);
    assert!(!page.entries[0].obsolete);
    let detail = service
        .detail(GetCatalogEntryCommand {
            access: fixture.root_access(),
            identity: identity("Settings"),
            locale: CatalogLocale::new("zh_Hans").unwrap(),
        })
        .await
        .unwrap();
    assert_eq!(detail.key, "Settings");

    let non_root = CatalogManagementAccess {
        actor: ActorContext::scoped(
            Uuid::now_v7(),
            fixture.workspace_id,
            "admin",
            std::iter::empty(),
        ),
        current_workspace_id: fixture.workspace_id,
    };
    assert!(service
        .list(ListCatalogEntriesCommand {
            access: non_root,
            key: None,
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
            key: None,
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
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let actor = fixture.root_access();
    let state = service
        .upsert_official_override(UpsertOfficialOverrideCommand {
            access: actor.clone(),
            value: translation("Settings", "覆盖"),
            expected_revision: fixture.revision,
        })
        .await
        .unwrap();
    assert_eq!(state.revision().value(), fixture.revision.value() + 1);

    let audit_row: (i64, Option<Uuid>, String, String) = sqlx::query_as(
        r#"select count(*) over(), actor_user_id, payload ->> 'key', payload ->> 'locale'
           from audit_logs where workspace_id = $1 and event_code = $2"#,
    )
    .bind(fixture.workspace_id)
    .bind("i18n_catalog.official_override.upserted")
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(audit_row.0, 1);
    assert_eq!(audit_row.1, Some(actor.actor.user_id));
    assert_eq!(audit_row.2, "Settings");
    assert_eq!(audit_row.3, "zh_Hans");

    assert!(service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: actor,
            value: translation("Stale custom", "不会写入"),
            expected_revision: fixture.revision,
        })
        .await
        .is_err());
    let state_after_stale =
        I18nCatalogRepository::get_workspace_catalog_state(&fixture.store, fixture.workspace_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(state_after_stale.revision(), state.revision());
    let custom_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_custom_translations where workspace_id = $1",
    )
    .bind(fixture.workspace_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    let all_audits: i64 =
        sqlx::query_scalar("select count(*) from audit_logs where workspace_id = $1")
            .bind(fixture.workspace_id)
            .fetch_one(fixture.store.pool())
            .await
            .unwrap();
    assert_eq!((custom_count, all_audits), (0, 1));
}

#[tokio::test]
async fn ac_008_custom_key_initializes_english_once_in_the_audited_revision_transaction() {
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let created = service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: fixture.root_access(),
            value: translation("Custom greeting", "问候"),
            expected_revision: fixture.revision,
        })
        .await
        .unwrap();
    assert_eq!(created.revision().value(), fixture.revision.value() + 1);

    let first_rows: Vec<(String, String)> = sqlx::query_as(
        r#"select locale, translation
           from workspace_i18n_catalog_custom_translations
           where workspace_id = $1 and key = 'Custom greeting'
           order by locale"#,
    )
    .bind(fixture.workspace_id)
    .fetch_all(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(
        first_rows,
        vec![
            ("en_US".to_owned(), "Custom greeting".to_owned()),
            ("zh_Hans".to_owned(), "问候".to_owned()),
        ]
    );
    let first_audits: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where workspace_id = $1 and event_code = $2",
    )
    .bind(fixture.workspace_id)
    .bind("i18n_catalog.custom_translation.upserted")
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(first_audits, 1);

    let english_updated = service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: fixture.root_access(),
            value: CatalogTranslation::new(
                identity("Custom greeting"),
                CatalogLocale::source(),
                "Edited custom greeting",
            )
            .unwrap(),
            expected_revision: created.revision(),
        })
        .await
        .unwrap();
    let chinese_updated = service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: fixture.root_access(),
            value: translation("Custom greeting", "问候语"),
            expected_revision: english_updated.revision(),
        })
        .await
        .unwrap();
    let updated_rows: Vec<(String, String)> = sqlx::query_as(
        r#"select locale, translation
           from workspace_i18n_catalog_custom_translations
           where workspace_id = $1 and key = 'Custom greeting'
           order by locale"#,
    )
    .bind(fixture.workspace_id)
    .fetch_all(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(
        updated_rows,
        vec![
            ("en_US".to_owned(), "Edited custom greeting".to_owned()),
            ("zh_Hans".to_owned(), "问候语".to_owned()),
        ]
    );

    sqlx::query(
        r#"insert into workspace_i18n_catalog_overrides
           (workspace_id, key, locale, translation)
           values ($1, 'Protected custom', 'en_US', 'Protected English')"#,
    )
    .bind(fixture.workspace_id)
    .execute(fixture.store.pool())
    .await
    .unwrap();
    service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: fixture.root_access(),
            value: translation("Protected custom", "受保护"),
            expected_revision: chinese_updated.revision(),
        })
        .await
        .unwrap();
    let protected_custom_english: i64 = sqlx::query_scalar(
        r#"select count(*) from workspace_i18n_catalog_custom_translations
           where workspace_id = $1 and key = 'Protected custom' and locale = 'en_US'"#,
    )
    .bind(fixture.workspace_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    let protected_override: String = sqlx::query_scalar(
        r#"select translation from workspace_i18n_catalog_overrides
           where workspace_id = $1 and key = 'Protected custom' and locale = 'en_US'"#,
    )
    .bind(fixture.workspace_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(protected_custom_english, 0);
    assert_eq!(protected_override, "Protected English");
}

#[tokio::test]
async fn ac_009_restore_and_explicit_custom_delete_preserve_distinct_lifecycles() {
    let fixture = management_store().await;
    let service = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let overridden = service
        .upsert_official_override(UpsertOfficialOverrideCommand {
            access: fixture.root_access(),
            value: translation("Settings", "覆盖"),
            expected_revision: fixture.revision,
        })
        .await
        .unwrap();
    let custom = service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: fixture.root_access(),
            value: translation("Custom message", "自定义"),
            expected_revision: overridden.revision(),
        })
        .await
        .unwrap();
    assert!(service
        .upsert_custom_translation(UpsertCustomTranslationCommand {
            access: fixture.root_access(),
            value: translation("Settings", "冲突"),
            expected_revision: custom.revision(),
        })
        .await
        .is_err());

    let restored = service
        .restore_official_translation(RestoreOfficialTranslationCommand {
            access: fixture.root_access(),
            identity: identity("Settings"),
            locale: CatalogLocale::new("zh_Hans").unwrap(),
            expected_revision: custom.revision(),
        })
        .await
        .unwrap();
    let official = CatalogResolutionRepository::find_catalog_resolution_candidate(
        &fixture.store,
        fixture.workspace_id,
        &identity("Settings"),
        &CatalogLocale::new("zh_Hans").unwrap(),
    )
    .await
    .unwrap();
    assert_eq!(official.root_override, None);
    assert_eq!(official.active_official.as_deref(), Some("设置"));

    let overridden_again = service
        .upsert_official_override(UpsertOfficialOverrideCommand {
            access: fixture.root_access(),
            value: translation("Settings", "再次覆盖"),
            expected_revision: restored.revision(),
        })
        .await
        .unwrap();
    let globally_restored = service
        .restore_all_official_overrides(RestoreAllOfficialOverridesCommand {
            access: fixture.root_access(),
            expected_revision: overridden_again.revision(),
        })
        .await
        .unwrap();
    let custom_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_custom_translations where workspace_id = $1",
    )
    .bind(fixture.workspace_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    let override_count: i64 = sqlx::query_scalar(
        "select count(*) from workspace_i18n_catalog_overrides where workspace_id = $1",
    )
    .bind(fixture.workspace_id)
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!((override_count, custom_count), (0, 2));

    service
        .delete_custom_message(DeleteCustomMessageCommand {
            access: fixture.root_access(),
            identity: identity("Custom message"),
            expected_revision: globally_restored.revision(),
        })
        .await
        .unwrap();
    let delete_audits: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where workspace_id = $1 and event_code = $2",
    )
    .bind(fixture.workspace_id)
    .bind("i18n_catalog.custom_message.deleted")
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(delete_audits, 1);
}

#[tokio::test]
async fn ac_008_concurrent_expected_revision_has_exactly_one_winner() {
    let fixture = management_store().await;
    let first = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let second = I18nCatalogManagementService::new(fixture.store.clone(), fixture.workspace_id);
    let left = first.upsert_official_override(UpsertOfficialOverrideCommand {
        access: fixture.root_access(),
        value: translation("Settings", "一"),
        expected_revision: fixture.revision,
    });
    let right = second.upsert_official_override(UpsertOfficialOverrideCommand {
        access: fixture.root_access(),
        value: translation("Settings", "二"),
        expected_revision: fixture.revision,
    });
    let (left, right) = tokio::join!(left, right);
    assert_eq!(usize::from(left.is_ok()) + usize::from(right.is_ok()), 1);
    let state =
        I18nCatalogRepository::get_workspace_catalog_state(&fixture.store, fixture.workspace_id)
            .await
            .unwrap()
            .unwrap();
    assert_eq!(state.revision().value(), fixture.revision.value() + 1);
    let audits: i64 = sqlx::query_scalar(
        "select count(*) from audit_logs where workspace_id = $1 and event_code = $2",
    )
    .bind(fixture.workspace_id)
    .bind("i18n_catalog.official_override.upserted")
    .fetch_one(fixture.store.pool())
    .await
    .unwrap();
    assert_eq!(audits, 1);
}
