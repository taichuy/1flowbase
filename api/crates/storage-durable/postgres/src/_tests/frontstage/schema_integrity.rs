use super::*;

#[tokio::test]
async fn full_migrations_reject_group_owned_tabs_and_block_codes_at_commit() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    insert_frontstage_group(&pool, group_id, workspace_id, None, "Owner Group")
        .await
        .unwrap();
    let (page_id, _, secondary_tab_id, block_code_id) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;

    let mut insert_tab = pool.begin().await.unwrap();
    let inserted_tab_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid, route_segment) values ($1, $2, $3, 'Invalid', 'z', false, $4, 'invalid')",
    )
    .bind(inserted_tab_id)
    .bind(workspace_id)
    .bind(group_id)
    .bind(format!("frontstage.tab.{inserted_tab_id}.root"))
    .execute(&mut *insert_tab)
    .await
    .unwrap();
    commit_error_contains(insert_tab, "frontstage_page_tab_owner_must_be_page").await;

    let mut update_tab = pool.begin().await.unwrap();
    sqlx::query("update frontstage_page_tabs set page_id = $1 where id = $2")
        .bind(group_id)
        .bind(secondary_tab_id)
        .execute(&mut *update_tab)
        .await
        .unwrap();
    commit_error_contains(update_tab, "frontstage_page_tab_owner_must_be_page").await;

    let mut insert_code = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'invalid.insert', 'export default 1;')",
    )
    .bind(Uuid::now_v7())
    .bind(workspace_id)
    .bind(group_id)
    .execute(&mut *insert_code)
    .await
    .unwrap();
    commit_error_contains(insert_code, "frontstage_block_code_owner_must_be_page").await;

    let mut update_code = pool.begin().await.unwrap();
    sqlx::query("update frontstage_block_codes set page_id = $1 where id = $2")
        .bind(group_id)
        .bind(block_code_id)
        .execute(&mut *update_code)
        .await
        .unwrap();
    commit_error_contains(update_code, "frontstage_block_code_owner_must_be_page").await;

    let preserved_owner_rows: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_page_tabs where page_id = $1), (select count(*) from frontstage_block_codes where page_id = $1)",
    )
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(preserved_owner_rows, (2, 1));
}

#[tokio::test]
async fn full_migrations_reject_page_to_group_with_owner_rows_and_allow_cascade_delete() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    insert_frontstage_group(&pool, group_id, workspace_id, None, "Empty Group")
        .await
        .unwrap();
    let group_kind: String = sqlx::query_scalar("select kind from frontstage_pages where id = $1")
        .bind(group_id)
        .fetch_one(&pool)
        .await
        .unwrap();
    assert_eq!(group_kind, "group");

    let (guarded_page_id, _, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    let mut kind_update = pool.begin().await.unwrap();
    sqlx::query(
        "update frontstage_pages set kind = 'group', content_presentation = 'single' where id = $1",
    )
    .bind(guarded_page_id)
    .execute(&mut *kind_update)
    .await
    .unwrap();
    commit_error_contains(kind_update, "frontstage_page_owner_rows_require_page_kind").await;

    let (deleted_page_id, _, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    sqlx::query("delete from frontstage_pages where id = $1")
        .bind(deleted_page_id)
        .execute(&pool)
        .await
        .unwrap();
    let deleted_owner_rows: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_page_tabs where page_id = $1), (select count(*) from frontstage_block_codes where page_id = $1)",
    )
    .bind(deleted_page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(deleted_owner_rows, (0, 0));
}

#[tokio::test]
async fn full_migrations_defer_page_owner_kind_checks_until_transaction_end() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let trigger_timing: Vec<(String, bool, bool)> = sqlx::query_as(
        r#"
        select trigger_definition.tgname,
               trigger_definition.tgdeferrable,
               trigger_definition.tginitdeferred
        from pg_trigger trigger_definition
        join pg_class table_definition
          on table_definition.oid = trigger_definition.tgrelid
        join pg_namespace table_schema
          on table_schema.oid = table_definition.relnamespace
        where table_schema.nspname = current_schema()
          and trigger_definition.tgname in (
          'frontstage_page_tabs_require_page_owner',
          'frontstage_block_codes_require_page_owner',
          'frontstage_pages_owner_rows_require_page_kind'
        )
        order by tgname
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(trigger_timing.len(), 3);
    assert!(trigger_timing
        .iter()
        .all(|(_, deferrable, initially_deferred)| *deferrable && *initially_deferred));

    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let immediate_group_id = Uuid::now_v7();
    insert_frontstage_group(
        &pool,
        immediate_group_id,
        workspace_id,
        None,
        "Immediate Group",
    )
    .await
    .unwrap();
    let immediate_tab_id = Uuid::now_v7();
    let mut immediate_check = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid, route_segment) values ($1, $2, $3, 'Immediate', 'a', false, $4, 'immediate')",
    )
    .bind(immediate_tab_id)
    .bind(workspace_id)
    .bind(immediate_group_id)
    .bind(format!("frontstage.tab.{immediate_tab_id}.root"))
    .execute(&mut *immediate_check)
    .await
    .unwrap();
    let immediate_error =
        sqlx::query("set constraints frontstage_page_tabs_require_page_owner immediate")
            .execute(&mut *immediate_check)
            .await
            .unwrap_err();
    assert!(
        immediate_error
            .to_string()
            .contains("frontstage_page_tab_owner_must_be_page"),
        "unexpected immediate constraint error: {immediate_error}"
    );
    immediate_check.rollback().await.unwrap();

    let page_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let block_code_id = Uuid::now_v7();
    let mut transaction = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, kind, title, rank) values ($1, $2, 'group', 'Ordered Page', 'a')",
    )
    .bind(page_id)
    .bind(workspace_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'ordered.code', 'export default 1;')",
    )
    .bind(block_code_id)
    .bind(workspace_id)
    .bind(page_id)
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Default', 'a', true, $4)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(page_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&mut *transaction)
    .await
    .unwrap();
    sqlx::query("update frontstage_pages set kind = 'page' where id = $1")
        .bind(page_id)
        .execute(&mut *transaction)
        .await
        .unwrap();
    transaction.commit().await.unwrap();

    let final_state: (String, i64, i64) = sqlx::query_as(
        "select kind, (select count(*) from frontstage_page_tabs where page_id = $1), (select count(*) from frontstage_block_codes where page_id = $1) from frontstage_pages where id = $1",
    )
    .bind(page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(final_state, ("page".into(), 1, 1));

    let (converted_page_id, _, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    let mut conversion = pool.begin().await.unwrap();
    sqlx::query("delete from frontstage_block_codes where page_id = $1")
        .bind(converted_page_id)
        .execute(&mut *conversion)
        .await
        .unwrap();
    sqlx::query("delete from frontstage_page_tabs where page_id = $1")
        .bind(converted_page_id)
        .execute(&mut *conversion)
        .await
        .unwrap();
    sqlx::query(
        "update frontstage_pages set kind = 'group', content_presentation = 'single' where id = $1",
    )
    .bind(converted_page_id)
    .execute(&mut *conversion)
    .await
    .unwrap();
    conversion.commit().await.unwrap();

    let converted_kind: String =
        sqlx::query_scalar("select kind from frontstage_pages where id = $1")
            .bind(converted_page_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(converted_kind, "group");
}

#[tokio::test]
async fn full_migrations_validate_old_and_new_tab_owners_after_reparent() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    let (workspace_id, second_workspace_id) = insert_frontstage_test_workspaces(&pool).await;
    let (source_page_id, source_default_tab_id, source_secondary_tab_id, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;
    let (target_page_id, target_default_tab_id, _, _) =
        insert_frontstage_page_with_owner_rows(&pool, workspace_id).await;

    let trigger_function_definition: String = sqlx::query_scalar(
        "select pg_get_functiondef(trigger_definition.tgfoid) from pg_trigger trigger_definition join pg_class table_definition on table_definition.oid = trigger_definition.tgrelid join pg_namespace table_schema on table_schema.oid = table_definition.relnamespace where table_schema.nspname = current_schema() and table_definition.relname = 'frontstage_page_tabs' and trigger_definition.tgname = 'frontstage_page_tabs_preserve_invariant'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    let normalized_trigger_function = trigger_function_definition.to_ascii_lowercase();
    assert!(normalized_trigger_function.contains("tg_op = 'update'"));
    assert!(normalized_trigger_function.contains("is distinct from"));
    assert!(normalized_trigger_function.contains("old.workspace_id"));
    assert!(normalized_trigger_function.contains("old.page_id"));
    assert!(normalized_trigger_function.contains("new.workspace_id"));
    assert!(normalized_trigger_function.contains("new.page_id"));
    assert!(normalized_trigger_function.contains(
        "else\n    perform enforce_frontstage_page_tab_invariant(new.workspace_id, new.page_id);"
    ));

    sqlx::query("delete from frontstage_page_tabs where id = $1")
        .bind(source_secondary_tab_id)
        .execute(&pool)
        .await
        .unwrap();

    let mut invalid_reparent = pool.begin().await.unwrap();
    sqlx::query(
        "update frontstage_page_tabs set page_id = $1, is_default = false, route_segment = 'source-default' where id = $2",
    )
        .bind(target_page_id)
        .bind(source_default_tab_id)
        .execute(&mut *invalid_reparent)
        .await
        .unwrap();
    commit_error_contains(
        invalid_reparent,
        "frontstage page must keep at least one tab",
    )
    .await;

    let replacement_tab_id = Uuid::now_v7();
    let mut valid_reparent = pool.begin().await.unwrap();
    sqlx::query(
        "update frontstage_page_tabs set is_default = false, route_segment = 'source-default' where id = $1",
    )
        .bind(source_default_tab_id)
        .execute(&mut *valid_reparent)
        .await
        .unwrap();
    sqlx::query(
        "update frontstage_page_tabs set is_default = false, route_segment = 'target-default' where id = $1",
    )
        .bind(target_default_tab_id)
        .execute(&mut *valid_reparent)
        .await
        .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Replacement', 'c', true, $4)",
    )
    .bind(replacement_tab_id)
    .bind(workspace_id)
    .bind(source_page_id)
    .bind(format!("frontstage.tab.{replacement_tab_id}.root"))
    .execute(&mut *valid_reparent)
    .await
    .unwrap();
    sqlx::query(
        "update frontstage_page_tabs set page_id = $1, is_default = true, route_segment = null where id = $2",
    )
        .bind(target_page_id)
        .bind(source_default_tab_id)
        .execute(&mut *valid_reparent)
        .await
        .unwrap();
    valid_reparent.commit().await.unwrap();

    let owner_state: Vec<(Uuid, i64, i64)> = sqlx::query_as(
        "select page_id, count(*), count(*) filter (where is_default) from frontstage_page_tabs where workspace_id = $1 and page_id in ($2, $3) group by page_id order by page_id",
    )
    .bind(workspace_id)
    .bind(source_page_id)
    .bind(target_page_id)
    .fetch_all(&pool)
    .await
    .unwrap();
    assert!(owner_state.contains(&(source_page_id, 1, 1)));
    assert!(owner_state.contains(&(target_page_id, 3, 1)));

    let cross_workspace_error =
        sqlx::query("update frontstage_page_tabs set workspace_id = $1 where id = $2")
            .bind(second_workspace_id)
            .bind(source_default_tab_id)
            .execute(&pool)
            .await
            .unwrap_err();
    assert!(
        cross_workspace_error
            .to_string()
            .contains("frontstage_page_tabs_workspace_id_page_id_fkey"),
        "unexpected cross-workspace error: {cross_workspace_error}"
    );
}

#[tokio::test]
async fn page_owner_kind_migration_preflight_rejects_dirty_data_without_changes() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_page_owner_kind_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (workspace_id, _) = insert_frontstage_test_workspaces(&pool).await;
    let group_id = Uuid::now_v7();
    let tab_id = Uuid::now_v7();
    let block_code_id = Uuid::now_v7();
    insert_frontstage_group(&pool, group_id, workspace_id, None, "Dirty Group")
        .await
        .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Dirty', 'a', false, $4)",
    )
    .bind(tab_id)
    .bind(workspace_id)
    .bind(group_id)
    .bind(format!("frontstage.tab.{tab_id}.root"))
    .execute(&pool)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'dirty.code', 'export default 1;')",
    )
    .bind(block_code_id)
    .bind(workspace_id)
    .bind(group_id)
    .execute(&pool)
    .await
    .unwrap();

    let migration_error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    assert!(
        migration_error
            .to_string()
            .contains("frontstage page owner kind migration rejected dirty data: tab rows 1, block code rows 1"),
        "unexpected migration error: {migration_error}"
    );

    let dirty_rows: (i64, i64) = sqlx::query_as(
        "select (select count(*) from frontstage_page_tabs where id = $1), (select count(*) from frontstage_block_codes where id = $2)",
    )
    .bind(tab_id)
    .bind(block_code_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dirty_rows, (1, 1));
    let migration_applied: bool = sqlx::query_scalar(
        "select exists(select 1 from _sqlx_migrations where version = $1 and success)",
    )
    .bind(FRONTSTAGE_PAGE_OWNER_KIND_MIGRATION_VERSION)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!migration_applied);
    let trigger_created: bool = sqlx::query_scalar(
        "select exists(select 1 from pg_trigger trigger_definition join pg_class table_definition on table_definition.oid = trigger_definition.tgrelid join pg_namespace table_schema on table_schema.oid = table_definition.relnamespace where table_schema.nspname = current_schema() and trigger_definition.tgname = 'frontstage_page_tabs_require_page_owner')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(!trigger_created);
}

#[tokio::test]
async fn full_migrations_enforce_frontstage_page_and_block_code_workspace_ownership() {
    let pool = isolated_database().await.connect().await.unwrap();
    run_migrations(&pool).await.unwrap();
    run_migrations(&pool).await.unwrap();
    let workspace_foreign_keys: Vec<(String, String)> = sqlx::query_as(
        r#"
        select conname, pg_get_constraintdef(oid)
        from pg_constraint
        where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass)
          and conname in (
            'frontstage_pages_workspace_parent_fkey',
            'frontstage_block_codes_workspace_page_fkey'
          )
        order by conname
        "#,
    )
    .fetch_all(&pool)
    .await
    .unwrap();
    assert_eq!(workspace_foreign_keys.len(), 2);
    let legacy_foreign_keys: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass) and conname in ('frontstage_pages_parent_id_fkey', 'frontstage_block_codes_page_id_fkey')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(legacy_foreign_keys, 0);
    assert!(workspace_foreign_keys.iter().any(|(name, definition)| {
        name == "frontstage_pages_workspace_parent_fkey"
            && definition.contains("FOREIGN KEY (workspace_id, parent_id)")
            && definition.contains("REFERENCES frontstage_pages(workspace_id, id)")
            && definition.contains("ON DELETE CASCADE")
    }));
    assert!(workspace_foreign_keys.iter().any(|(name, definition)| {
        name == "frontstage_block_codes_workspace_page_fkey"
            && definition.contains("FOREIGN KEY (workspace_id, page_id)")
            && definition.contains("REFERENCES frontstage_pages(workspace_id, id)")
            && definition.contains("ON DELETE CASCADE")
    }));
    let parent_is_nullable: bool = sqlx::query_scalar(
        "select is_nullable = 'YES' from information_schema.columns where table_schema = current_schema() and table_name = 'frontstage_pages' and column_name = 'parent_id'",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(parent_is_nullable);
    let workspace_page_unique_indexes: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_index index_definition
        join pg_class table_definition on table_definition.oid = index_definition.indrelid
        join pg_class index_relation on index_relation.oid = index_definition.indexrelid
        where table_definition.oid = 'frontstage_pages'::regclass
          and index_definition.indisunique
          and index_relation.relname = 'frontstage_pages_workspace_id_id_uidx'
        "#,
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(workspace_page_unique_indexes, 1);
    let (first_workspace_id, second_workspace_id) = insert_frontstage_test_workspaces(&pool).await;
    let first_parent_id = Uuid::now_v7();
    let first_child_id = Uuid::now_v7();
    let second_parent_id = Uuid::now_v7();
    let cross_workspace_child_id = Uuid::now_v7();
    let first_code_page_id = Uuid::now_v7();
    let first_code_page_tab_id = Uuid::now_v7();
    let second_code_page_id = Uuid::now_v7();
    let second_code_page_tab_id = Uuid::now_v7();

    insert_frontstage_group(
        &pool,
        first_parent_id,
        first_workspace_id,
        None,
        "First Parent",
    )
    .await
    .unwrap();

    let mut owner_pages = pool.begin().await.unwrap();
    sqlx::query(
        "insert into frontstage_pages (id, workspace_id, parent_id, kind, title, rank) values ($1, $2, $3, 'page', 'First Code Page', 'b'), ($4, $5, null, 'page', 'Second Code Page', 'a')",
    )
    .bind(first_code_page_id)
    .bind(first_workspace_id)
    .bind(first_parent_id)
    .bind(second_code_page_id)
    .bind(second_workspace_id)
    .execute(&mut *owner_pages)
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_page_tabs (id, workspace_id, page_id, title, rank, is_default, document_root_uid) values ($1, $2, $3, 'Default', 'a', true, $4), ($5, $6, $7, 'Default', 'a', true, $8)",
    )
    .bind(first_code_page_tab_id)
    .bind(first_workspace_id)
    .bind(first_code_page_id)
    .bind(format!("frontstage.tab.{first_code_page_tab_id}.root"))
    .bind(second_code_page_tab_id)
    .bind(second_workspace_id)
    .bind(second_code_page_id)
    .bind(format!("frontstage.tab.{second_code_page_tab_id}.root"))
    .execute(&mut *owner_pages)
    .await
    .unwrap();
    owner_pages.commit().await.unwrap();
    insert_frontstage_group(
        &pool,
        first_child_id,
        first_workspace_id,
        Some(first_parent_id),
        "First Child",
    )
    .await
    .unwrap();
    insert_frontstage_group(
        &pool,
        second_parent_id,
        second_workspace_id,
        None,
        "Second Parent",
    )
    .await
    .unwrap();

    let cross_workspace_parent_error = insert_frontstage_group(
        &pool,
        cross_workspace_child_id,
        second_workspace_id,
        Some(first_parent_id),
        "Invalid Child",
    )
    .await
    .unwrap_err();
    assert_eq!(
        cross_workspace_parent_error
            .as_database_error()
            .and_then(|error| error.code().map(|code| code.into_owned())),
        Some("23503".into())
    );

    let first_block_code_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'same-workspace', 'export default 1;')",
    )
    .bind(first_block_code_id)
    .bind(first_workspace_id)
    .bind(first_code_page_id)
    .execute(&pool)
    .await
    .unwrap();
    let cross_workspace_block_error = sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'cross-workspace', 'export default 2;')",
    )
    .bind(Uuid::now_v7())
    .bind(second_workspace_id)
    .bind(first_code_page_id)
    .execute(&pool)
    .await
    .unwrap_err();
    assert_eq!(
        cross_workspace_block_error
            .as_database_error()
            .and_then(|error| error.code().map(|code| code.into_owned())),
        Some("23503".into())
    );

    let second_block_code_id = Uuid::now_v7();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'same-workspace', 'export default 3;')",
    )
    .bind(second_block_code_id)
    .bind(second_workspace_id)
    .bind(second_code_page_id)
    .execute(&pool)
    .await
    .unwrap();

    sqlx::query("delete from frontstage_pages where workspace_id = $1 and id = $2")
        .bind(first_workspace_id)
        .bind(first_parent_id)
        .execute(&pool)
        .await
        .unwrap();

    let first_workspace_page_count: i64 = sqlx::query_scalar(
        "select count(*) from frontstage_pages where workspace_id = $1 and id in ($2, $3, $4)",
    )
    .bind(first_workspace_id)
    .bind(first_parent_id)
    .bind(first_child_id)
    .bind(first_code_page_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(first_workspace_page_count, 0);
    let first_block_code_exists: bool =
        sqlx::query_scalar("select exists(select 1 from frontstage_block_codes where id = $1)")
            .bind(first_block_code_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!first_block_code_exists);
    let second_workspace_page_exists: bool = sqlx::query_scalar(
        "select exists(select 1 from frontstage_pages where workspace_id = $1 and id = $2)",
    )
    .bind(second_workspace_id)
    .bind(second_parent_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(second_workspace_page_exists);
    let second_block_code_exists: bool =
        sqlx::query_scalar("select exists(select 1 from frontstage_block_codes where id = $1)")
            .bind(second_block_code_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(second_block_code_exists);
}

#[tokio::test]
async fn workspace_integrity_migration_rejects_dirty_history_without_schema_or_data_changes() {
    let pool = isolated_database().await.connect().await.unwrap();
    before_frontstage_workspace_integrity_migrator()
        .run(&pool)
        .await
        .unwrap();
    let (first_workspace_id, second_workspace_id) = insert_frontstage_test_workspaces(&pool).await;
    let first_parent_id = Uuid::now_v7();
    let dirty_child_id = Uuid::now_v7();
    let dirty_block_code_id = Uuid::now_v7();

    insert_frontstage_group(
        &pool,
        first_parent_id,
        first_workspace_id,
        None,
        "Dirty Parent",
    )
    .await
    .unwrap();
    insert_frontstage_group(
        &pool,
        dirty_child_id,
        second_workspace_id,
        Some(first_parent_id),
        "Dirty Child",
    )
    .await
    .unwrap();
    sqlx::query(
        "insert into frontstage_block_codes (id, workspace_id, page_id, code_ref, code) values ($1, $2, $3, 'dirty-owner', 'export default 4;')",
    )
    .bind(dirty_block_code_id)
    .bind(second_workspace_id)
    .bind(first_parent_id)
    .execute(&pool)
    .await
    .unwrap();

    let migration_error = sqlx::migrate!("./migrations").run(&pool).await.unwrap_err();
    let migration_message = migration_error.to_string();
    assert!(
        migration_message.contains("frontstage workspace integrity migration rejected dirty data"),
        "unexpected migration error: {migration_message}"
    );
    assert!(migration_message.contains("parent rows 1"));
    assert!(migration_message.contains("block code rows 1"));

    let dirty_rows: i64 = sqlx::query_scalar(
        "select (select count(*) from frontstage_pages where id = $1) + (select count(*) from frontstage_block_codes where id = $2)",
    )
    .bind(dirty_child_id)
    .bind(dirty_block_code_id)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(dirty_rows, 2);
    let old_constraints: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass) and conname in ('frontstage_pages_parent_id_fkey', 'frontstage_block_codes_page_id_fkey')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(old_constraints, 2);
    let new_constraints: i64 = sqlx::query_scalar(
        "select count(*) from pg_constraint where conrelid in ('frontstage_pages'::regclass, 'frontstage_block_codes'::regclass) and conname in ('frontstage_pages_workspace_parent_fkey', 'frontstage_block_codes_workspace_page_fkey')",
    )
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(new_constraints, 0);
    let migration_recorded: bool =
        sqlx::query_scalar("select exists(select 1 from _sqlx_migrations where version = $1)")
            .bind(FRONTSTAGE_WORKSPACE_INTEGRITY_MIGRATION_VERSION)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert!(!migration_recorded);
}
