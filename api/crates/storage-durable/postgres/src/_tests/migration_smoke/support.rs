use super::*;

pub(super) async fn assert_scoped_readiness_columns_and_index(
    pool: &PgPool,
    schema: &str,
    table: &str,
) {
    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select column_name
        from information_schema.columns
        where table_schema = $1
          and table_name = $2
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap();
    for expected_column in [
        "id",
        "scope_id",
        "created_at",
        "created_by",
        "updated_at",
        "updated_by",
    ] {
        assert!(
            columns.contains(&expected_column.to_string()),
            "missing {table}.{expected_column}"
        );
    }

    let scope_nullable: String = sqlx::query_scalar(
        r#"
        select is_nullable
        from information_schema.columns
        where table_schema = $1
          and table_name = $2
          and column_name = 'scope_id'
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(scope_nullable, "NO", "{table}.scope_id must be not null");

    let scope_index_count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_indexes
        where schemaname = $1
          and tablename = $2
          and indexdef ilike '%(scope_id, created_at, id)%'
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        scope_index_count, 1,
        "missing {table} scope readiness index"
    );
}

pub(super) async fn assert_primary_key_columns(
    pool: &PgPool,
    schema: &str,
    table: &str,
    expected_columns: &[&str],
) {
    let columns: Vec<String> = sqlx::query_scalar(
        r#"
        select a.attname
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        join unnest(c.conkey) with ordinality as cols(attnum, ord) on true
        join pg_attribute a on a.attrelid = r.oid and a.attnum = cols.attnum
        where n.nspname = $1
          and r.relname = $2
          and c.contype = 'p'
        order by cols.ord
        "#,
    )
    .bind(schema)
    .bind(table)
    .fetch_all(pool)
    .await
    .unwrap();
    assert_eq!(
        columns,
        expected_columns
            .iter()
            .map(|column| (*column).to_string())
            .collect::<Vec<_>>(),
        "{table} primary key should keep its business identity"
    );
}

pub(super) async fn assert_unique_constraint_columns(
    pool: &PgPool,
    schema: &str,
    table: &str,
    expected_columns: &[&str],
) {
    let count: i64 = sqlx::query_scalar(
        r#"
        select count(*)
        from pg_constraint c
        join pg_class r on r.oid = c.conrelid
        join pg_namespace n on n.oid = r.relnamespace
        where n.nspname = $1
          and r.relname = $2
          and c.contype in ('u', 'p')
          and (
            select array_agg(a.attname::text order by cols.ord)
            from unnest(c.conkey) with ordinality as cols(attnum, ord)
            join pg_attribute a on a.attrelid = r.oid and a.attnum = cols.attnum
          ) = $3::text[]
        "#,
    )
    .bind(schema)
    .bind(table)
    .bind(expected_columns)
    .fetch_one(pool)
    .await
    .unwrap();
    assert_eq!(
        count, 1,
        "{table} should keep unique identity on {expected_columns:?}"
    );
}
