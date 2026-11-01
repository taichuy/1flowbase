use control_plane_contracts::ports::{
    CreateUiCodeTemplateInput, CreateUiComponentRecordInput, FrontendBlockCatalogDefaultTemplate,
    FrontendBlockCatalogRepository, FrontendBlockCatalogTemplateRevision,
    ListUiComponentRecordsQuery, OfficialUiComponentCatalogRecord, ReviseUiCodeTemplateInput,
    UiComponentCatalogRepository, UiComponentRecordPatch, UiManagementRepository,
};
use domain::{UiCodeTemplateLanguage, UiComponentRecordOrigin, UiComponentRecordUpstream};
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn store() -> PgControlPlaneStore {
    let database = postgres_test_support::PostgresTestSchema::create(&database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    PgControlPlaneStore::new(pool)
}

#[tokio::test]
async fn ac_002_template_revisions_publish_and_default_are_transactional() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let created = store
        .create_ui_code_template(&CreateUiCodeTemplateInput {
            provider_code: "1flowbase".into(),
            contribution_code: "frontstage.js-ui-block".into(),
            name: "Dashboard".into(),
            source: "export default function Block() { return null }".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(created.latest_revision.revision, 1);
    assert!(store
        .set_ui_code_template_default(created.id, actor)
        .await
        .is_err());

    let revised = store
        .revise_ui_code_template(&ReviseUiCodeTemplateInput {
            template_id: created.id,
            name: "Dashboard".into(),
            source: "export default function Block() { return <main /> }".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(revised.latest_revision.revision, 2);
    let published = store
        .publish_ui_code_template_revision(created.id, 2, actor)
        .await
        .unwrap();
    assert_eq!(published.published_revision.unwrap().revision, 2);
    store
        .set_ui_code_template_default(created.id, actor)
        .await
        .unwrap();
    assert!(
        store
            .get_ui_code_template(created.id)
            .await
            .unwrap()
            .unwrap()
            .is_default
    );
}

#[tokio::test]
async fn wp_d2_component_records_are_system_scoped_crud_with_official_write_protection() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let created = store
        .create_ui_component_record(&CreateUiComponentRecordInput {
            component_code: "local.status-panel".into(),
            name: "Status panel".into(),
            description: "Shows a status".into(),
            import_code: "import StatusPanel from './StatusPanel';".into(),
            source_code: "<StatusPanel />".into(),
            source: "local".into(),
            group: "operations".into(),
            upstream: UiComponentRecordUpstream {
                identity: "@local/status-panel".into(),
                version: "0.1.0".into(),
            },
            version: "1.0.0".into(),
            keywords: vec!["status".into()],
            actor_user_id: actor,
        })
        .await
        .unwrap();
    assert_eq!(created.origin, UiComponentRecordOrigin::Custom);
    assert_eq!(created.scope_id, domain::SYSTEM_SCOPE_ID);

    let updated = store
        .update_ui_component_record(
            created.id,
            &UiComponentRecordPatch {
                name: "System status panel".into(),
                description: created.description.clone(),
                import_code: created.import_code.clone(),
                source_code: created.source_code.clone(),
                source: created.source.clone(),
                group: created.group.clone(),
                upstream: created.upstream.clone(),
                version: "1.1.0".into(),
                keywords: created.keywords.clone(),
                actor_user_id: actor,
            },
        )
        .await
        .unwrap();
    assert_eq!(updated.component_code, created.component_code);
    assert_eq!(updated.name, "System status panel");
    assert!(store.delete_ui_component_record(created.id).await.unwrap());
    assert!(store
        .get_ui_component_record(created.id)
        .await
        .unwrap()
        .is_none());

    let official_id = Uuid::now_v7();
    sqlx::query("insert into ui_component_records (id, scope_id, component_code, name, description, import_code, source_code, origin, source, \"group\", upstream_identity, upstream_version, version, keywords, created_by, updated_by) values ($1,$2,'official.button','Button','Official button','opaque import','opaque source','official','taichuy','ant-design-x','@ant-design/x','2.9.0','1.0.0',array['action'],$3,$3)")
        .bind(official_id)
        .bind(domain::SYSTEM_SCOPE_ID)
        .bind(actor)
        .execute(store.pool())
        .await
        .unwrap();
    assert!(store
        .update_ui_component_record(
            official_id,
            &UiComponentRecordPatch {
                name: "Changed".into(),
                description: "Changed".into(),
                import_code: "opaque".into(),
                source_code: "opaque".into(),
                source: "taichuy".into(),
                group: "ant-design-x".into(),
                upstream: UiComponentRecordUpstream {
                    identity: "@ant-design/x".into(),
                    version: "2.9.0".into(),
                },
                version: "1.0.1".into(),
                keywords: vec!["action".into()],
                actor_user_id: actor,
            },
        )
        .await
        .is_err());
    assert!(!store.delete_ui_component_record(official_id).await.unwrap());
}

fn official_record(code: &str, group: &str, import_code: &str) -> OfficialUiComponentCatalogRecord {
    OfficialUiComponentCatalogRecord {
        component_code: code.into(),
        name: code.into(),
        description: "Official fixture".into(),
        import_code: import_code.into(),
        source_code: "opaque source {{{".into(),
        source: "taichuy".into(),
        group: group.into(),
        upstream: UiComponentRecordUpstream {
            identity: "@not-installed/package".into(),
            version: "9.9.9".into(),
        },
        version: "1.0.0".into(),
        keywords: vec!["fixture".into()],
        catalog_updated_at: time::OffsetDateTime::parse(
            "2026-08-23T00:00:00Z",
            &time::format_description::well_known::Rfc3339,
        )
        .unwrap(),
        source_locator: format!("ui_components/@taichuy/{group}/{code}.json"),
        source_checksum: "sha256:aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa"
            .into(),
    }
}

#[tokio::test]
async fn wp_d3_authoritative_group_replace_is_atomic_and_group_scoped() {
    let store = store().await;
    let actor = Uuid::now_v7();
    store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-a",
            &[
                official_record("taichuy.group-a.keep", "group-a", "opaque import one"),
                official_record("taichuy.group-a.remove", "group-a", "opaque import two"),
            ],
            actor,
        )
        .await
        .unwrap();
    store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-b",
            &[official_record(
                "taichuy.group-b.untouched",
                "group-b",
                "opaque import",
            )],
            actor,
        )
        .await
        .unwrap();
    let custom = store
        .create_ui_component_record(&CreateUiComponentRecordInput {
            component_code: "local.group-a.custom".into(),
            name: "Custom".into(),
            description: "Custom fixture".into(),
            import_code: "custom opaque import".into(),
            source_code: "custom opaque source".into(),
            source: "taichuy".into(),
            group: "group-a".into(),
            upstream: UiComponentRecordUpstream {
                identity: "@custom/package".into(),
                version: "1.0.0".into(),
            },
            version: "1.0.0".into(),
            keywords: vec![],
            actor_user_id: actor,
        })
        .await
        .unwrap();

    store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-a",
            &[official_record(
                "taichuy.group-a.keep",
                "group-a",
                "import Missing from '@never-installed/new-version';",
            )],
            actor,
        )
        .await
        .unwrap();

    let records = store.list_ui_component_records().await.unwrap();
    assert!(records.iter().any(|record| record.id == custom.id));
    assert!(records
        .iter()
        .any(|record| record.component_code == "taichuy.group-b.untouched"));
    assert!(!records
        .iter()
        .any(|record| record.component_code == "taichuy.group-a.remove"));
    let kept = records
        .iter()
        .find(|record| record.component_code == "taichuy.group-a.keep")
        .unwrap();
    assert_eq!(
        kept.import_code,
        "import Missing from '@never-installed/new-version';"
    );
}

#[tokio::test]
async fn wp_d3_rejects_mixed_identity_before_authoritative_replace() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let before = store.list_ui_component_records().await.unwrap();
    let error = store
        .replace_official_ui_component_source_group(
            "taichuy",
            "group-a",
            &[official_record(
                "taichuy.group-b.invalid",
                "group-b",
                "opaque",
            )],
            actor,
        )
        .await
        .unwrap_err();
    assert!(error.to_string().contains("source/group"));
    assert_eq!(store.list_ui_component_records().await.unwrap(), before);
}

#[tokio::test]
async fn template_archive_removal_migration_preserves_content_and_restores_visibility() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let template = store
        .create_ui_code_template(&CreateUiCodeTemplateInput {
            provider_code: "1flowbase".into(),
            contribution_code: "frontstage.js-ui-block".into(),
            name: "Previously archived".into(),
            source: "export default function Block() { return null }".into(),
            language: UiCodeTemplateLanguage::Tsx,
            actor_user_id: actor,
        })
        .await
        .unwrap();
    // Reconstruct the old archive column/index around a real persisted template.
    sqlx::raw_sql("drop index ui_code_templates_name_idx; alter table ui_code_templates add column archived_at timestamptz; create unique index ui_code_templates_active_name_idx on ui_code_templates(scope_id,provider_code,contribution_code,lower(name)) where archived_at is null;")
        .execute(store.pool()).await.unwrap();
    sqlx::query("update ui_code_templates set archived_at = now() where id = $1")
        .bind(template.id)
        .execute(store.pool())
        .await
        .unwrap();
    sqlx::raw_sql(include_str!(
        "../../migrations/20260909170000_replace_ui_template_archive_with_delete.sql"
    ))
    .execute(store.pool())
    .await
    .unwrap();
    let restored = store.list_ui_code_templates().await.unwrap();
    assert_eq!(restored.len(), 1);
    assert_eq!(restored[0].id, template.id);
    assert_eq!(restored[0].name, template.name);
    assert_eq!(
        restored[0].latest_revision.source,
        template.latest_revision.source
    );
    assert!(store.delete_ui_code_template(template.id).await.unwrap());
    assert!(store
        .get_ui_code_template(template.id)
        .await
        .unwrap()
        .is_none());
    assert!(store.list_ui_code_templates().await.unwrap().is_empty());
    assert!(!store.delete_ui_code_template(template.id).await.unwrap());
}

#[tokio::test]
async fn component_pages_preserve_search_order_bodies_and_one_snapshot_without_off_page_sources() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let mut fixtures = Vec::new();
    for index in (0..25).rev() {
        let code = format!("taichuy.paging.item-{index:02}");
        let mut record = official_record(&code, "paging", &format!("import body {index}"));
        record.name = match index {
            23 => "Éclair".into(),
            24 => "İstanbul".into(),
            _ => format!("Item {:02}", index / 2),
        };
        record.source_code = format!("source-only-token {index}");
        if index == 24 {
            record.description = "Literal %_ marker".into();
            record.keywords = vec!["KEYWORD-ONLY".into()];
        }
        fixtures.push(record);
    }
    store
        .replace_official_ui_component_source_group("taichuy", "paging", &fixtures, actor)
        .await
        .unwrap();
    let mut expected = store.list_ui_component_records().await.unwrap();
    expected.sort_by(|left, right| {
        left.name
            .cmp(&right.name)
            .then_with(|| left.component_code.cmp(&right.component_code))
    });

    for (offset, limit, start, end) in [
        (0, 20, 0, 20),
        (20, 20, 20, 25),
        (24, usize::MAX, 24, 25),
        (usize::MAX, 20, 25, 25),
        (0, 0, 0, 1),
    ] {
        let page = store
            .list_ui_component_records_page(ListUiComponentRecordsQuery {
                plugin_contribution_ids: None,
                query: Some("  ".into()),
                offset,
                limit,
            })
            .await
            .unwrap();
        assert_eq!(page.items, expected[start..end]);
        assert_eq!(
            (page.total, page.offset, page.limit),
            (25, start, limit.max(1))
        );
        assert_eq!(page.has_more, end < 25);
        assert_eq!(page.next_offset, (end < 25).then_some(end));
    }
    for (search, matching_codes) in [
        ("  ÉCLAIR  ", vec!["taichuy.paging.item-23"]),
        ("İSTANBUL", vec!["taichuy.paging.item-24"]),
        ("%_", vec!["taichuy.paging.item-24"]),
        ("keyword-only", vec!["taichuy.paging.item-24"]),
        ("source-only-token", vec![]),
        ("not-present", vec![]),
    ] {
        let page = store
            .list_ui_component_records_page(ListUiComponentRecordsQuery {
                plugin_contribution_ids: None,
                query: Some(search.into()),
                offset: 0,
                limit: 20,
            })
            .await
            .unwrap();
        assert_eq!(page.total, matching_codes.len(), "search: {search}");
        assert_eq!(
            page.items
                .iter()
                .map(|item| item.component_code.as_str())
                .collect::<Vec<_>>(),
            matching_codes
        );
    }
    for search in ["taichuy", "paging", "@not-installed", "9.9.9", "1.0.0"] {
        let page = store
            .list_ui_component_records_page(ListUiComponentRecordsQuery {
                plugin_contribution_ids: None,
                query: Some(search.into()),
                offset: 0,
                limit: 20,
            })
            .await
            .unwrap();
        assert_eq!(page.total, 25, "search: {search}");
    }

    // The fixture view fails if either source field outside the requested page is evaluated.
    // A metadata lock holds the read between snapshot acquisition and page hydration.
    let lock_id = (Uuid::now_v7().as_u128() & 0x7fff_ffff) as i32;
    let fixture_sql = format!(
        r#"
        alter table ui_component_records rename to component_page_fixture;
        create function page_source(body text, code text) returns text language plpgsql immutable as $$
        begin
            if code >= 'taichuy.paging.item-20' then raise exception 'off-page source materialized'; end if;
            return body;
        end $$;
        create function page_metadata(body text, lock_id integer) returns text language plpgsql volatile as $$
        begin
            perform pg_advisory_xact_lock_shared(9271, lock_id);
            return body;
        end $$;
        create view ui_component_records as select id, scope_id, component_code, name,
            page_metadata(description, {lock_id}) as description,
            page_source(import_code, component_code) as import_code,
            page_source(source_code, component_code) as source_code,
            origin, source, "group", upstream_identity, upstream_version, version, keywords,
            catalog_updated_at, source_locator, source_checksum, created_by, updated_by, created_at, updated_at
        from component_page_fixture;
    "#
    );
    // SQL safety: fixture DDL is static except for a locally generated integer advisory lock key.
    sqlx::raw_sql(sqlx::AssertSqlSafe(fixture_sql.as_str()))
        .execute(store.pool())
        .await
        .unwrap();
    let mut writer = store.pool().begin().await.unwrap();
    sqlx::query("select pg_advisory_xact_lock(9271, $1)")
        .bind(lock_id)
        .execute(&mut *writer)
        .await
        .unwrap();
    let reader = PgControlPlaneStore::new(store.pool().clone());
    let page_task = tokio::spawn(async move {
        reader
            .list_ui_component_records_page(ListUiComponentRecordsQuery {
                plugin_contribution_ids: None,
                query: None,
                offset: 0,
                limit: 20,
            })
            .await
    });
    tokio::time::timeout(std::time::Duration::from_secs(10), async {
        loop {
            let waiting: bool = sqlx::query_scalar("select exists(select 1 from pg_locks where locktype = 'advisory' and classid = 9271 and objid = $1::int::oid and not granted)")
                .bind(lock_id).fetch_one(&mut *writer).await.unwrap();
            if waiting { break; }
            tokio::time::sleep(std::time::Duration::from_millis(10)).await;
        }
    }).await.expect("reader must reach the metadata snapshot");
    sqlx::query("update component_page_fixture set name = 'ZZ changed', source_code = 'new source' where component_code = 'taichuy.paging.item-00'")
        .execute(&mut *writer).await.unwrap();
    sqlx::query(
        "delete from component_page_fixture where component_code = 'taichuy.paging.item-24'",
    )
    .execute(&mut *writer)
    .await
    .unwrap();
    writer.commit().await.unwrap();
    let page = page_task.await.unwrap().unwrap();
    assert_eq!(page.total, 25);
    assert_eq!(page.items, expected[..20]);
}

#[tokio::test]
async fn frontend_catalog_template_projection_reads_only_published_default_sources() {
    let store = store().await;
    let actor = Uuid::now_v7();
    let mut chosen = None;
    for name in ["Default", "Other published", "Unpublished"] {
        let template = store
            .create_ui_code_template(&CreateUiCodeTemplateInput {
                provider_code: "fixture.frontend".into(),
                contribution_code: "hero".into(),
                name: name.into(),
                source: format!("published {name}"),
                language: UiCodeTemplateLanguage::Tsx,
                actor_user_id: actor,
            })
            .await
            .unwrap();
        if name != "Unpublished" {
            store
                .publish_ui_code_template_revision(template.id, 1, actor)
                .await
                .unwrap();
        }
        if name == "Default" {
            store
                .set_ui_code_template_default(template.id, actor)
                .await
                .unwrap();
            store
                .revise_ui_code_template(&ReviseUiCodeTemplateInput {
                    template_id: template.id,
                    name: name.into(),
                    source: "unpublished draft".into(),
                    language: UiCodeTemplateLanguage::Jsx,
                    actor_user_id: actor,
                })
                .await
                .unwrap();
            chosen = Some(template.id);
        }
    }
    // The migrated schema remains authoritative; this read-only view instruments source
    // evaluation so a full template/draft read fails instead of silently passing parity.
    let fixture_sql = format!(
        r#"
        alter table ui_code_template_revisions rename to template_revision_fixture;
        create function catalog_source(body text, template uuid, published boolean) returns text language plpgsql immutable as $$
        begin
            if template <> '{}'::uuid or not published then raise exception 'unused template source materialized'; end if;
            return body;
        end $$;
        create view ui_code_template_revisions as select id, template_id, revision,
            catalog_source(source, template_id, is_published) as source,
            language, is_latest, is_published, created_by, created_at
        from template_revision_fixture;
    "#,
        chosen.unwrap()
    );
    // SQL safety: static fixture DDL interpolates only the UUID created by this test.
    sqlx::raw_sql(sqlx::AssertSqlSafe(fixture_sql.as_str()))
        .execute(store.pool())
        .await
        .unwrap();
    assert_eq!(
        store
            .list_active_ui_code_templates_for_catalog()
            .await
            .unwrap(),
        vec![FrontendBlockCatalogDefaultTemplate {
            provider_code: "fixture.frontend".into(),
            contribution_code: "hero".into(),
            published_revision: Some(FrontendBlockCatalogTemplateRevision {
                revision: 1,
                source: "published Default".into(),
                language: UiCodeTemplateLanguage::Tsx
            }),
        }]
    );
}
