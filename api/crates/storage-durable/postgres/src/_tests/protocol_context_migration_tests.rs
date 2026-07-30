use std::borrow::Cow;

use sqlx::migrate::Migrator;
use storage_postgres::PgControlPlaneStore;
use uuid::Uuid;

const MIGRATION_VERSION: i64 = 20260728120000;
const MIGRATION_SQL: &str =
    include_str!("../../migrations/20260728120000_restore_legacy_protocol_context_defaults.sql");
const COMPATIBILITY_BACKFILL_VERSION: i64 = 20260729090000;
const COMPATIBILITY_BACKFILL_SQL: &str =
    include_str!("../../migrations/20260729090000_backfill_responses_compatibility_mode.sql");

struct SeededFlow {
    application_id: Uuid,
    flow_id: Uuid,
    draft_id: Uuid,
    version_id: Uuid,
    compiled_plan_id: Uuid,
}

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
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

fn before_compatibility_backfill_migrator() -> Migrator {
    let migrations = sqlx::migrate!("./migrations")
        .iter()
        .filter(|migration| migration.version < COMPATIBILITY_BACKFILL_VERSION)
        .cloned()
        .collect::<Vec<_>>();
    Migrator {
        migrations: Cow::Owned(migrations),
        ..Migrator::DEFAULT
    }
}

async fn seed_owner(store: &PgControlPlaneStore) -> (Uuid, Uuid) {
    let tenant_id: Uuid = sqlx::query_scalar("select id from tenants where code = 'root-tenant'")
        .fetch_one(store.pool())
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query(
        "insert into workspaces (id, tenant_id, name) values ($1, $2, 'Protocol context migration')",
    )
    .bind(workspace_id)
    .bind(tenant_id)
    .execute(store.pool())
    .await
    .unwrap();

    let user_id = Uuid::now_v7();
    let account = format!("protocol-context-{}", user_id.simple());
    sqlx::query(
        r#"
        insert into users (
            id, account, email, password_hash, name, nickname, introduction,
            default_display_role, email_login_enabled, phone_login_enabled, status,
            session_version
        ) values ($1, $2, $3, 'hash', $2, $2, '', 'member', true, false, 'active', 1)
        "#,
    )
    .bind(user_id)
    .bind(&account)
    .bind(format!("{account}@example.com"))
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into workspace_memberships (id, workspace_id, user_id, introduction) values ($1, $2, $3, '')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    (workspace_id, user_id)
}

async fn seed_application(store: &PgControlPlaneStore, workspace_id: Uuid, user_id: Uuid) -> Uuid {
    let application_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into applications (
            id, workspace_id, application_type, name, description, created_by, updated_by
        ) values ($1, $2, 'agent_flow', 'Protocol context migration', '', $3, $3)
        "#,
    )
    .bind(application_id)
    .bind(workspace_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    application_id
}

async fn seed_flow(
    store: &PgControlPlaneStore,
    workspace_id: Uuid,
    user_id: Uuid,
    document: &serde_json::Value,
    plan: &serde_json::Value,
) -> SeededFlow {
    let application_id = seed_application(store, workspace_id, user_id).await;

    let flow_id = Uuid::now_v7();
    let draft_id = Uuid::now_v7();
    let version_id = Uuid::now_v7();
    let compiled_plan_id = Uuid::now_v7();
    sqlx::query(
        "insert into flows (id, application_id, scope_id, created_by, updated_by) values ($1, $2, (select scope_id from applications where id = $2), $3, $3)",
    )
    .bind(flow_id)
    .bind(application_id)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        "insert into flow_drafts (id, flow_id, scope_id, schema_version, document, created_by, updated_by) values ($1, $2, (select scope_id from flows where id = $2), $3, $4, $5, $5)",
    )
    .bind(draft_id)
    .bind(flow_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(document)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_versions (
            id, flow_id, scope_id, sequence, trigger, change_kind, summary,
            summary_is_custom, is_user_protected, document, created_by, updated_by
        ) values ($1, $2, (select scope_id from flows where id = $2), 1, 'autosave',
            'logical', 'fixture', false, true, $3, $4, $4)
        "#,
    )
    .bind(version_id)
    .bind(flow_id)
    .bind(document)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into flow_compiled_plans (
            id, flow_id, flow_draft_id, schema_version, document_hash,
            document_updated_at, plan, scope_id, created_by, updated_by
        ) select $1, $2, $3, $4, 'sha256:compiled', updated_at, $5,
            (select scope_id from flows where id = $2), $6, $6
          from flow_drafts where id = $3
        "#,
    )
    .bind(compiled_plan_id)
    .bind(flow_id)
    .bind(draft_id)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(plan)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();

    SeededFlow {
        application_id,
        flow_id,
        draft_id,
        version_id,
        compiled_plan_id,
    }
}

async fn seed_publication(
    store: &PgControlPlaneStore,
    seeded: &SeededFlow,
    application_id: Uuid,
    user_id: Uuid,
    sequence: i64,
    snapshot: &serde_json::Value,
) -> Uuid {
    let publication_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into application_publication_versions (
            id, application_id, scope_id, flow_id, flow_version_id, compiled_plan_id,
            version_sequence, active, api_enabled, flow_schema_version, document_hash,
            document_snapshot, mapping_snapshot, runtime_profile_snapshot, output_selector,
            dependency_snapshot, created_by, updated_by
        ) values (
            $1, $2, (select scope_id from applications where id = $2), $3, $4, $5,
            $6, false, true, $7, 'sha256:immutable', $8, '{}'::jsonb, '{}'::jsonb,
            '{}'::jsonb, '[]'::jsonb, $9, $9
        )
        "#,
    )
    .bind(publication_id)
    .bind(application_id)
    .bind(seeded.flow_id)
    .bind(seeded.version_id)
    .bind(seeded.compiled_plan_id)
    .bind(sequence)
    .bind(domain::FLOW_SCHEMA_VERSION)
    .bind(snapshot)
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();
    publication_id
}

fn snapshot(nodes: serde_json::Value, marker: &str) -> serde_json::Value {
    serde_json::json!({
        "graph": {"nodes": nodes, "edges": []},
        "marker": marker
    })
}

fn protocol_context_plan() -> serde_json::Value {
    serde_json::json!({
        "nodes": {
            "missing-llm": {
                "node_id": "missing-llm",
                "node_type": "llm",
                "config": {"protocol_context": null, "sibling": "preserved"}
            },
            "missing-llm-2": {
                "node_id": "missing-llm-2",
                "node_type": "llm",
                "config": {"protocol_context": null}
            },
            "missing-llm-3": {
                "node_id": "missing-llm-3",
                "node_type": "llm",
                "config": {"protocol_context": null}
            },
            "explicit-null-llm": {
                "node_id": "explicit-null-llm",
                "node_type": "llm",
                "config": {"protocol_context": null}
            },
            "selector-llm": {
                "node_id": "selector-llm",
                "node_type": "llm",
                "config": {"protocol_context": {
                    "kind": "selector",
                    "value": ["node-code", "result", "protocol_context"]
                }}
            },
            "non-llm": {
                "node_id": "non-llm",
                "node_type": "code",
                "config": {"protocol_context": null}
            },
            "nonnull-plan-llm": {
                "node_id": "nonnull-plan-llm",
                "node_type": "llm",
                "config": {"protocol_context": {
                    "kind": "selector",
                    "value": ["sys", "protocol_context"]
                }}
            }
        },
        "topological_order": [
            "missing-llm",
            "missing-llm-2",
            "missing-llm-3",
            "explicit-null-llm",
            "selector-llm",
            "non-llm",
            "nonnull-plan-llm"
        ]
    })
}

/// FUA-MIGRATION: publication intent repairs only synthetic compiled nulls, remains fail closed
/// across conflicting snapshots, preserves immutable sources, and is idempotent.
#[tokio::test]
async fn protocol_context_migration_restores_only_unambiguous_legacy_missing_defaults() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_migrator().run(&pool).await.unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let (workspace_id, user_id) = seed_owner(&store).await;

    let eligible_snapshot = snapshot(
        serde_json::json!([
            {"id": "missing-llm", "type": "llm", "config": {"sibling": "published"}},
            {"id": "missing-llm-2", "type": "llm", "config": {}},
            {"id": "missing-llm-3", "type": "llm", "config": {}},
            {"id": "explicit-null-llm", "type": "llm", "config": {"protocol_context": null}},
            {"id": "selector-llm", "type": "llm", "config": {"protocol_context": {
                "kind": "selector",
                "value": ["node-code", "result", "protocol_context"]
            }}},
            {"id": "non-llm", "type": "code", "config": {}},
            {"id": "nonnull-plan-llm", "type": "llm", "config": {}}
        ]),
        "eligible-publication",
    );
    let eligible_plan = protocol_context_plan();
    let eligible = seed_flow(
        &store,
        workspace_id,
        user_id,
        &eligible_snapshot,
        &eligible_plan,
    )
    .await;
    let eligible_publication = seed_publication(
        &store,
        &eligible,
        eligible.application_id,
        user_id,
        1,
        &eligible_snapshot,
    )
    .await;

    let conflict_missing_snapshot = snapshot(
        serde_json::json!([
            {"id": "conflict-llm", "type": "llm", "config": {}}
        ]),
        "conflict-missing",
    );
    let conflict_explicit_snapshot = snapshot(
        serde_json::json!([
            {"id": "conflict-llm", "type": "llm", "config": {"protocol_context": null}}
        ]),
        "conflict-explicit",
    );
    let conflict_plan = serde_json::json!({
        "nodes": {
            "conflict-llm": {
                "node_id": "conflict-llm",
                "node_type": "llm",
                "config": {"protocol_context": null}
            }
        }
    });
    let conflict = seed_flow(
        &store,
        workspace_id,
        user_id,
        &conflict_missing_snapshot,
        &conflict_plan,
    )
    .await;
    let conflict_missing_publication = seed_publication(
        &store,
        &conflict,
        conflict.application_id,
        user_id,
        1,
        &conflict_missing_snapshot,
    )
    .await;
    let conflict_evidence_application = seed_application(&store, workspace_id, user_id).await;
    let conflict_explicit_publication = seed_publication(
        &store,
        &conflict,
        conflict_evidence_application,
        user_id,
        2,
        &conflict_explicit_snapshot,
    )
    .await;

    let unpublished_snapshot = snapshot(
        serde_json::json!([
            {"id": "unpublished-llm", "type": "llm", "config": {}}
        ]),
        "unpublished",
    );
    let unpublished_plan = serde_json::json!({
        "nodes": {
            "unpublished-llm": {
                "node_id": "unpublished-llm",
                "node_type": "llm",
                "config": {"protocol_context": null}
            }
        }
    });
    let unpublished = seed_flow(
        &store,
        workspace_id,
        user_id,
        &unpublished_snapshot,
        &unpublished_plan,
    )
    .await;

    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let migrated_eligible: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(eligible.compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    for node_id in ["missing-llm", "missing-llm-2", "missing-llm-3"] {
        assert!(migrated_eligible["nodes"][node_id]["config"]
            .as_object()
            .unwrap()
            .get("protocol_context")
            .is_none());
    }
    assert_eq!(
        migrated_eligible["nodes"]["missing-llm"]["config"]["sibling"],
        "preserved"
    );
    assert_eq!(
        migrated_eligible["nodes"]["explicit-null-llm"]["config"]["protocol_context"],
        serde_json::Value::Null
    );
    assert_eq!(
        migrated_eligible["nodes"]["selector-llm"]["config"]["protocol_context"],
        eligible_plan["nodes"]["selector-llm"]["config"]["protocol_context"]
    );
    assert_eq!(
        migrated_eligible["nodes"]["non-llm"]["config"]["protocol_context"],
        serde_json::Value::Null
    );
    assert_eq!(
        migrated_eligible["nodes"]["nonnull-plan-llm"]["config"]["protocol_context"],
        eligible_plan["nodes"]["nonnull-plan-llm"]["config"]["protocol_context"]
    );

    let migrated_conflict: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(conflict.compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(migrated_conflict, conflict_plan);
    let migrated_unpublished: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(unpublished.compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(migrated_unpublished, unpublished_plan);

    for (publication_id, expected_snapshot) in [
        (eligible_publication, &eligible_snapshot),
        (conflict_missing_publication, &conflict_missing_snapshot),
        (conflict_explicit_publication, &conflict_explicit_snapshot),
    ] {
        let stored_snapshot: serde_json::Value = sqlx::query_scalar(
            "select document_snapshot from application_publication_versions where id = $1",
        )
        .bind(publication_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(&stored_snapshot, expected_snapshot);
    }

    for (seeded, expected_document) in [
        (&eligible, &eligible_snapshot),
        (&conflict, &conflict_missing_snapshot),
        (&unpublished, &unpublished_snapshot),
    ] {
        let draft: serde_json::Value =
            sqlx::query_scalar("select document from flow_drafts where id = $1")
                .bind(seeded.draft_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        let version: serde_json::Value =
            sqlx::query_scalar("select document from flow_versions where id = $1")
                .bind(seeded.version_id)
                .fetch_one(&pool)
                .await
                .unwrap();
        assert_eq!(&draft, expected_document);
        assert_eq!(&version, expected_document);
    }

    sqlx::raw_sql(MIGRATION_SQL).execute(&pool).await.unwrap();
    let after_second_run: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(eligible.compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(after_second_run, migrated_eligible);
    let conflict_after_second_run: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(conflict.compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(conflict_after_second_run, migrated_conflict);
    let unpublished_after_second_run: serde_json::Value =
        sqlx::query_scalar("select plan from flow_compiled_plans where id = $1")
            .bind(unpublished.compiled_plan_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(unpublished_after_second_run, migrated_unpublished);
}

async fn seed_compatibility_backfill_run(
    store: &PgControlPlaneStore,
    seeded: &SeededFlow,
    user_id: Uuid,
    input_payload: serde_json::Value,
    compatibility_mode: Option<&str>,
) -> Uuid {
    let flow_run_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into flow_runs (
            id, application_id, flow_id, flow_draft_id, compiled_plan_id,
            run_mode, status, input_payload, created_by, compatibility_mode,
            started_at, created_at, updated_at, title, scope_id
        ) values (
            $1, $2, $3, $4, $5, 'published_api_run', 'failed', $6, $7, $8,
            now(), now(), now(), 'Compatibility backfill fixture',
            (select scope_id from applications where id = $2)
        )
        "#,
    )
    .bind(flow_run_id)
    .bind(seeded.application_id)
    .bind(seeded.flow_id)
    .bind(seeded.draft_id)
    .bind(seeded.compiled_plan_id)
    .bind(input_payload)
    .bind(user_id)
    .bind(compatibility_mode)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into application_run_log_summaries (
            flow_run_id, application_id, run_mode, status, title, input_payload,
            compatibility_mode, started_at, created_at, updated_at, scope_id
        ) select
            id, application_id, run_mode, status, title, input_payload,
            compatibility_mode, started_at, created_at, updated_at, scope_id
        from flow_runs where id = $1
        "#,
    )
    .bind(flow_run_id)
    .execute(store.pool())
    .await
    .unwrap();

    sqlx::query(
        r#"
        insert into application_run_trace_projection_statuses (
            flow_run_id, projection_version, status, source_watermark,
            id, scope_id, created_by, updated_by
        ) values (
            $1, 1, 'succeeded', 'before-backfill', $2,
            (select scope_id from flow_runs where id = $1), $3, $3
        )
        "#,
    )
    .bind(flow_run_id)
    .bind(Uuid::now_v7())
    .bind(user_id)
    .execute(store.pool())
    .await
    .unwrap();

    flow_run_id
}

#[tokio::test]
async fn compatibility_backfill_uses_only_server_owned_responses_evidence() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_compatibility_backfill_migrator()
        .run(&pool)
        .await
        .unwrap();
    let store = PgControlPlaneStore::new(pool.clone());
    let (workspace_id, user_id) = seed_owner(&store).await;
    let document = snapshot(serde_json::json!([]), "compatibility-backfill");
    let plan = serde_json::json!({"nodes": {}});
    let seeded = seed_flow(&store, workspace_id, user_id, &document, &plan).await;

    let eligible = seed_compatibility_backfill_run(
        &store,
        &seeded,
        user_id,
        serde_json::json!({
            "sys": {"public_provider_transport": {"protocol": "openai_responses"}}
        }),
        None,
    )
    .await;
    let client_claim_only = seed_compatibility_backfill_run(
        &store,
        &seeded,
        user_id,
        serde_json::json!({
            "__client_protocol_envelope": {"source_protocol": "openai_responses"}
        }),
        None,
    )
    .await;
    let existing_mode = seed_compatibility_backfill_run(
        &store,
        &seeded,
        user_id,
        serde_json::json!({
            "sys": {"public_provider_transport": {"protocol": "openai_responses"}}
        }),
        Some("anthropic-messages-v1"),
    )
    .await;

    assert!(COMPATIBILITY_BACKFILL_SQL.contains("import_job_id is null"));
    sqlx::migrate!("./migrations").run(&pool).await.unwrap();

    let modes = sqlx::query_as::<_, (Uuid, Option<String>, Option<String>, String)>(
        r#"
        select runs.id, runs.compatibility_mode, summaries.compatibility_mode, statuses.status
        from flow_runs runs
        join application_run_log_summaries summaries on summaries.flow_run_id = runs.id
        join application_run_trace_projection_statuses statuses
          on statuses.flow_run_id = runs.id and statuses.projection_version = 1
        where runs.id = any($1)
        order by runs.id
        "#,
    )
    .bind(vec![eligible, client_claim_only, existing_mode])
    .fetch_all(&pool)
    .await
    .unwrap();

    let row = |id| modes.iter().find(|row| row.0 == id).unwrap();
    assert_eq!(row(eligible).1.as_deref(), Some("openai-responses-v1"));
    assert_eq!(row(eligible).2.as_deref(), Some("openai-responses-v1"));
    assert_eq!(row(eligible).3, "stale");
    assert_eq!(row(client_claim_only).1, None);
    assert_eq!(row(client_claim_only).2, None);
    assert_eq!(row(client_claim_only).3, "succeeded");
    assert_eq!(
        row(existing_mode).1.as_deref(),
        Some("anthropic-messages-v1")
    );
    assert_eq!(
        row(existing_mode).2.as_deref(),
        Some("anthropic-messages-v1")
    );
    assert_eq!(row(existing_mode).3, "succeeded");

    sqlx::raw_sql(COMPATIBILITY_BACKFILL_SQL)
        .execute(&pool)
        .await
        .unwrap();
    let after_second_run: (Option<String>, Option<String>, String) = sqlx::query_as(
        r#"
        select runs.compatibility_mode, summaries.compatibility_mode, statuses.status
        from flow_runs runs
        join application_run_log_summaries summaries on summaries.flow_run_id = runs.id
        join application_run_trace_projection_statuses statuses
          on statuses.flow_run_id = runs.id and statuses.projection_version = 1
        where runs.id = $1
        "#,
    )
    .bind(eligible)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        after_second_run,
        (
            Some("openai-responses-v1".to_string()),
            Some("openai-responses-v1".to_string()),
            "stale".to_string()
        )
    );
}
