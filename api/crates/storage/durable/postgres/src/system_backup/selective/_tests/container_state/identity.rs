use super::super::*;

async fn allocator(db: &PgPool) -> (i64, bool, i64, String) {
    let row = sqlx::query("select s.last_value,s.is_called,p.seqcache,a.attidentity::text as mode from identity_restore_fixture_sequence_seq s cross join pg_sequence p cross join pg_attribute a where p.seqrelid='identity_restore_fixture_sequence_seq'::regclass and a.attrelid='identity_restore_fixture'::regclass and a.attname='sequence'")
        .fetch_one(db).await.unwrap();
    (
        row.get("last_value"),
        row.get("is_called"),
        row.get("seqcache"),
        row.get("mode"),
    )
}

#[tokio::test]
async fn identity_restore_replays_and_rolls_back_modes_cache_and_exhausted_bounds() {
    let (db, _) = fixture().await;
    sqlx::query("create table identity_restore_parent(id integer primary key)")
        .execute(&db)
        .await
        .unwrap();
    sqlx::query("insert into identity_restore_parent values(1)")
        .execute(&db)
        .await
        .unwrap();
    for (increment, cache, cycle, mode) in [
        (1, 1, "no cycle", "always"),
        (1, 3, "cycle", "by default"),
        (2, 3, "no cycle", "always"),
        (-1, 3, "no cycle", "always"),
        (-1, 1, "cycle", "by default"),
        (-2, 3, "cycle", "always"),
    ] {
        let start = if increment > 0 { 1 } else { -1 };
        let ddl = format!("create table identity_restore_fixture(id integer primary key,sequence bigint generated {mode} as identity (increment by {increment} minvalue -5 maxvalue 5 start with {start} cache {cache} {cycle}) unique,parent_id integer references identity_restore_parent(id) deferrable initially deferred)");
        sqlx::query(sqlx::AssertSqlSafe(ddl.as_str()))
            .execute(&db)
            .await
            .unwrap();
        sqlx::query("insert into identity_restore_fixture(id,parent_id) values(1,1)")
            .execute(&db)
            .await
            .unwrap();
        let original = allocator(&db).await;
        let table = Table {
            name: "identity_restore_fixture".into(),
            feature_id: "system.data-models".into(),
            data: true,
            prerequisite: false,
            columns: schema::columns(
                &mut *db.acquire().await.unwrap(),
                "identity_restore_fixture",
            )
            .await
            .unwrap(),
            primary_key: vec!["id".into()],
            plugin_owners: vec![],
            prerequisites: vec![],
        };
        let tables = vec![table];
        let upsert = schema::upsert_sql(&tables[0]).unwrap();
        // Explicit identities outside generator bounds remain valid PostgreSQL row values.
        let value = if increment > 0 { 7_i64 } else { -7_i64 };
        let row = serde_json::json!({"id":1,"sequence":value,"parent_id":1});
        let mut committed = None;
        for commit in [false, true, true] {
            let mut tx = db.begin().await.unwrap();
            let columns = identity::prepare(&mut tx, &tables).await.unwrap();
            sqlx::query(sqlx::AssertSqlSafe(upsert.as_str()))
                .bind(&row)
                .execute(&mut *tx)
                .await
                .unwrap();
            // Drain deferred FK events before restoring ALWAYS with ALTER TABLE.
            sqlx::query("set constraints all immediate")
                .execute(&mut *tx)
                .await
                .unwrap();
            identity::finish(&mut tx, columns).await.unwrap();
            if commit {
                tx.commit().await.unwrap();
                let current = allocator(&db).await;
                assert_eq!(current.0, if increment > 0 { 5 } else { -5 });
                assert!(current.1);
                assert_eq!(current.2, cache);
                assert_eq!(current.3, if mode == "always" { "a" } else { "d" });
                if let Some(previous) = &committed {
                    assert_eq!(&current, previous);
                }
                committed = Some(current);
                assert_eq!(
                    sqlx::query_scalar::<_, i64>(
                        "select sequence from identity_restore_fixture where id=1"
                    )
                    .fetch_one(&db)
                    .await
                    .unwrap(),
                    value
                );
            } else {
                tx.rollback().await.unwrap();
                assert_eq!(allocator(&db).await, original);
                assert_eq!(
                    sqlx::query_scalar::<_, i64>(
                        "select sequence from identity_restore_fixture where id=1"
                    )
                    .fetch_one(&db)
                    .await
                    .unwrap(),
                    start
                );
            }
        }
        sqlx::query("drop table identity_restore_fixture")
            .execute(&db)
            .await
            .unwrap();
    }
}
