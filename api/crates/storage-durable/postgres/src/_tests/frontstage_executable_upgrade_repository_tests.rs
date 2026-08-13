use control_plane::ports::FrontstageExecutableUpgradeRepository;
use serde_json::json;
use sha2::{Digest, Sha256};
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn fixture(legacy_count: usize) -> (PgControlPlaneStore, sqlx::PgPool, Vec<Uuid>, Uuid) {
    let database = postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let actor_id = Uuid::now_v7();
    sqlx::query("insert into users (id, account, email, password_hash, name, nickname, status) values ($1, $2, $3, 'x', 'Upgrade', 'Upgrade', 'active')")
        .bind(actor_id)
        .bind(format!("upgrade-{actor_id}"))
        .bind(format!("upgrade-{actor_id}@example.com"))
        .execute(&pool)
        .await
        .unwrap();
    let workspace_id = Uuid::now_v7();
    sqlx::query("insert into workspaces (id, tenant_id, name, created_by, updated_by) values ($1, '00000000-0000-0000-0000-000000000001', 'Upgrade', $2, $2)")
        .bind(workspace_id)
        .bind(actor_id)
        .execute(&pool)
        .await
        .unwrap();
    let installation_id = Uuid::now_v7();
    sqlx::query(
        r#"
        insert into extension_installations (
            id, category, organization, artifact_id, artifact_version, plugin_id,
            contract_version, protocol, display_name, source_kind, trust_level,
            verification_status, desired_state, signature_status, created_by, updated_by
        ) values ($1, 'capability-plugins', '1flowbase', '1flowbase', '1.0.0',
            '1flowbase@1.0.0', '1flowbase.capability/v1', 'stdio_json', 'Blocks',
            'builtin', 'verified_official', 'valid', 'active_requested', 'verified', $2, $2)
        "#,
    )
    .bind(installation_id)
    .bind(actor_id)
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into frontend_block_catalog (
            id, installation_id, provider_code, plugin_id, plugin_version,
            contribution_code, title, runtime, entry, context_contract,
            permission_network, permission_storage, permission_secrets,
            ui_capabilities, code_modules
        ) values ($1, $2, '1flowbase', '1flowbase@1.0.0', '1.0.0',
            'frontstage.js-ui-block', 'Block', 'native_react', 'index.js',
            '{"primitives":[],"input_schema":{}}', 'none', 'none', 'none', '[]', $3)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(installation_id)
    .bind(json!([
        {
            "source": "react", "version": "19.2.5", "exports": ["default"],
            "binding": "host", "assets": [], "type_declarations": "declare module 'react' {}"
        },
        {
            "source": "tailwindcss", "version": "4.3.3", "exports": ["default"],
            "binding": "fetched",
            "assets": [{
                "path": "tailwind.js", "role": "browser_module",
                "media_type": "text/javascript", "sha256": "a".repeat(64)
            }],
            "type_declarations": "declare module 'tailwindcss' {}"
        }
    ]))
    .execute(&pool)
    .await
    .unwrap();
    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    sqlx::query("insert into frontstage_pages (id, workspace_id, kind, title, placement, rank) values ($1, $2, 'page', 'Upgrade', 'topbar', 'a')")
        .bind(page_id)
        .bind(workspace_id)
        .execute(&pool)
        .await
        .unwrap();
    sqlx::query("insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Default', 'a', true, $4)")
        .bind(tab_id)
        .bind(workspace_id)
        .bind(page_id)
        .bind(format!("frontstage.tab.{tab_id}.root"))
        .execute(&pool)
        .await
        .unwrap();

    let mut legacy_ids = Vec::new();
    for index in 0..legacy_count {
        let row_id = Uuid::now_v7();
        legacy_ids.push(row_id);
        insert_code_and_node(
            &pool,
            row_id,
            workspace_id,
            page_id,
            tab_id,
            installation_id,
            &format!("legacy-{index}"),
            None,
        )
        .await;
    }
    let ready_id = Uuid::now_v7();
    insert_code_and_node(
        &pool,
        ready_id,
        workspace_id,
        page_id,
        tab_id,
        installation_id,
        "old-ready",
        Some(".old-ready{}"),
    )
    .await;
    (
        PgControlPlaneStore::new(pool.clone()),
        pool,
        legacy_ids,
        ready_id,
    )
}

#[allow(clippy::too_many_arguments)]
async fn insert_code_and_node(
    pool: &sqlx::PgPool,
    row_id: Uuid,
    workspace_id: Uuid,
    page_id: Uuid,
    tab_id: Uuid,
    installation_id: Uuid,
    code_ref: &str,
    ready_css: Option<&str>,
) {
    let source = format!("import 'tailwindcss'; export default () => <div className=\"p-4\" data-code=\"{code_ref}\" />;");
    let source_sha = format!("{:x}", Sha256::digest(source.as_bytes()));
    let css_sha = ready_css.map(|css| format!("{:x}", Sha256::digest(css.as_bytes())));
    sqlx::query(
        r#"
        insert into frontstage_block_codes (
            id, workspace_id, page_id, code_ref, code, source_sha256,
            dependency_lock, tailwind_toolchain_lock, generated_css,
            generated_css_sha256, compiler_identity
        ) values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10, $11)
        "#,
    )
    .bind(row_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(code_ref)
    .bind(source)
    .bind(ready_css.map(|_| source_sha))
    .bind(ready_css.map(|_| json!([])))
    .bind(ready_css.map(|_| json!({ "package": "old", "version": "1" })))
    .bind(ready_css)
    .bind(css_sha)
    .bind(ready_css.map(|_| json!({ "name": "old", "version": "1" })))
    .execute(pool)
    .await
    .unwrap();
    sqlx::query(
        r#"
        insert into frontstage_block_nodes (
            id, scope_id, tree_partition_id, sibling_rank, block_id, tab_id,
            presentation, code_ref, runtime_descriptor
        ) values ($1, $2, $3, $4, $5, $6, 'page', $5, $7)
        "#,
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("{code_ref}U"))
    .bind(code_ref)
    .bind(tab_id)
    .bind(json!({
        "id": code_ref,
        "codeRef": code_ref,
        "rendererVersion": "v1",
        "catalog": { "providerCode": "1flowbase", "installationId": installation_id },
        "contribution": {
            "pluginId": "1flowbase@1.0.0", "pluginVersion": "1.0.0",
            "code": "frontstage.js-ui-block"
        }
    }))
    .execute(pool)
    .await
    .unwrap();
}

fn target() -> domain::FrontstageExecutableUpgradeTarget {
    domain::FrontstageExecutableUpgradeTarget {
        marker: "tailwind-4.3.3-v1".into(),
        contract_identity: json!({ "artifact": "compiler-4.3.3" }),
        compiler_identity: json!({ "name": "tailwind", "version": "4.3.3" }),
        toolchain_lock: json!({ "package": "tailwindcss", "version": "4.3.3" }),
    }
}

fn compiled(
    target: &domain::FrontstageExecutableUpgradeTarget,
    snapshot: &domain::LegacyFrontstageExecutableSnapshot,
) -> Vec<domain::CompiledFrontstageExecutable> {
    snapshot
        .rows
        .iter()
        .map(|row| {
            let css = format!(".upgraded-{} {{}}", row.row_id);
            domain::CompiledFrontstageExecutable {
                row_id: row.row_id,
                source_sha256: row.source_sha256.clone(),
                dependency_lock: row.dependency_lock.clone(),
                generated_css_sha256: format!("{:x}", Sha256::digest(css.as_bytes())),
                generated_css: css,
                compiler_identity: target.compiler_identity.clone(),
                toolchain_lock: target.toolchain_lock.clone(),
                contract_identity: target.contract_identity.clone(),
            }
        })
        .collect()
}

async fn started_snapshot(
    store: &PgControlPlaneStore,
    target: &domain::FrontstageExecutableUpgradeTarget,
) -> domain::LegacyFrontstageExecutableSnapshot {
    let start = store
        .begin_frontstage_executable_upgrade(target)
        .await
        .unwrap();
    let domain::FrontstageExecutableUpgradeStart::Run { run_id, .. } = start else {
        panic!("fixture must start a run");
    };
    store
        .capture_frontstage_executable_upgrade_snapshot(target, run_id)
        .await
        .unwrap()
}

#[tokio::test]
async fn commit_is_all_or_nothing_audited_and_leaves_old_ready_untouched() {
    let (store, pool, legacy_ids, ready_id) = fixture(2).await;
    let target = target();
    let snapshot = started_snapshot(&store, &target).await;
    store
        .commit_frontstage_executable_upgrade(&target, &snapshot, &compiled(&target, &snapshot))
        .await
        .unwrap();
    store
        .require_frontstage_executable_cutover(&target)
        .await
        .unwrap();
    let upgraded: i64 = sqlx::query_scalar(
        "select count(*) from frontstage_block_codes where id = any($1) and source_sha256 is not null",
    )
    .bind(&legacy_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(upgraded, 2);
    let ready_css: String =
        sqlx::query_scalar("select generated_css from frontstage_block_codes where id = $1")
            .bind(ready_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(ready_css, ".old-ready{}");
    let audit_actor: Option<Uuid> = sqlx::query_scalar(
        "select actor_user_id from audit_logs where event_code = 'frontstage.executable_system_upgraded'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(audit_actor, None);
    assert_eq!(
        store
            .begin_frontstage_executable_upgrade(&target)
            .await
            .unwrap(),
        domain::FrontstageExecutableUpgradeStart::Completed
    );
}

#[tokio::test]
async fn snapshot_drift_blocks_every_update() {
    let (store, pool, legacy_ids, _) = fixture(2).await;
    let target = target();
    let snapshot = started_snapshot(&store, &target).await;
    sqlx::query("update frontstage_block_codes set code = code || ' ' where id = $1")
        .bind(legacy_ids[1])
        .execute(&pool)
        .await
        .unwrap();
    assert!(store
        .commit_frontstage_executable_upgrade(&target, &snapshot, &compiled(&target, &snapshot))
        .await
        .is_err());
    let upgraded: i64 = sqlx::query_scalar(
        "select count(*) from frontstage_block_codes where id = any($1) and source_sha256 is not null",
    )
    .bind(&legacy_ids)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(upgraded, 0);
}

#[tokio::test]
async fn running_attempt_reuses_immutable_snapshot_evidence() {
    let (store, pool, legacy_ids, _) = fixture(1).await;
    let target = target();
    let first = started_snapshot(&store, &target).await;
    sqlx::query("update frontstage_block_codes set code = code || ' ' where id = $1")
        .bind(legacy_ids[0])
        .execute(&pool)
        .await
        .unwrap();

    let resumed = store
        .capture_frontstage_executable_upgrade_snapshot(&target, first.run_id)
        .await
        .unwrap();

    assert_eq!(resumed, first);
    assert!(store
        .commit_frontstage_executable_upgrade(&target, &resumed, &compiled(&target, &resumed))
        .await
        .is_err());
}

#[tokio::test]
async fn row_audit_or_marker_failure_rolls_back_rows_and_evidence() {
    for target_table in [
        "frontstage_block_codes",
        "audit_logs",
        "frontstage_executable_upgrade_markers",
    ] {
        let (store, pool, legacy_ids, _) = fixture(2).await;
        let target = target();
        let snapshot = started_snapshot(&store, &target).await;
        sqlx::raw_sql(&format!(
            r#"
            create function reject_upgrade_fixture() returns trigger language plpgsql as $$
            begin raise exception 'fixture {target_table} failure'; end $$;
            create trigger reject_upgrade_fixture before insert or update on {target_table}
            for each row execute function reject_upgrade_fixture();
            "#
        ))
        .execute(&pool)
        .await
        .unwrap();
        assert!(store
            .commit_frontstage_executable_upgrade(&target, &snapshot, &compiled(&target, &snapshot))
            .await
            .is_err());
        let upgraded: i64 = sqlx::query_scalar(
            "select count(*) from frontstage_block_codes where id = any($1) and source_sha256 is not null",
        )
        .bind(&legacy_ids)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(upgraded, 0);
        let run_status: String = sqlx::query_scalar(
            "select status from frontstage_executable_upgrade_runs where run_id = $1",
        )
        .bind(snapshot.run_id)
        .fetch_one(&pool)
        .await
        .unwrap();
        let marker_status: String = sqlx::query_scalar(
            "select status from frontstage_executable_upgrade_markers where marker = $1",
        )
        .bind(&target.marker)
        .fetch_one(&pool)
        .await
        .unwrap();
        assert_eq!(run_status, "running");
        assert_eq!(marker_status, "running");
    }
}

#[tokio::test]
async fn failed_run_retries_with_new_attempt_and_failure_record_is_idempotent() {
    let (store, _, _, _) = fixture(1).await;
    let target = target();
    let snapshot = started_snapshot(&store, &target).await;
    let failure = domain::FrontstageExecutableUpgradeFailure {
        run_id: snapshot.run_id,
        marker: target.marker.clone(),
        error_code: "tsx_transform_failed".into(),
        target_identity: json!({ "row_id": snapshot.rows[0].row_id }),
        compiler_identity: target.compiler_identity.clone(),
    };
    store
        .record_frontstage_executable_upgrade_failure(&target, &failure)
        .await
        .unwrap();
    store
        .record_frontstage_executable_upgrade_failure(&target, &failure)
        .await
        .unwrap();
    let start = store
        .begin_frontstage_executable_upgrade(&target)
        .await
        .unwrap();
    let domain::FrontstageExecutableUpgradeStart::Run { run_id, attempt } = start else {
        panic!("failed run must retry");
    };
    assert_ne!(run_id, snapshot.run_id);
    assert_eq!(attempt, 2);
}

#[tokio::test]
async fn fresh_database_completes_and_post_cutover_legacy_fails_closed() {
    let (store, pool, _, _) = fixture(0).await;
    let target = target();
    let snapshot = started_snapshot(&store, &target).await;
    assert!(snapshot.rows.is_empty());
    store
        .commit_frontstage_executable_upgrade(&target, &snapshot, &[])
        .await
        .unwrap();
    store
        .require_frontstage_executable_cutover(&target)
        .await
        .unwrap();
    sqlx::query(
        "update frontstage_block_codes set source_sha256 = null, dependency_lock = null, tailwind_toolchain_lock = null, generated_css = null, generated_css_sha256 = null, compiler_identity = null where code_ref = 'old-ready'",
    )
    .execute(&pool)
    .await
    .unwrap();
    assert!(store
        .require_frontstage_executable_cutover(&target)
        .await
        .is_err());
}
