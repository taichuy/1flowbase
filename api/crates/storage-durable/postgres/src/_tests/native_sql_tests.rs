use postgres_test_support::PostgresTestSchema;
use serde_json::json;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .or_else(|_| std::env::var("API_DATABASE_URL"))
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

#[tokio::test]
async fn ac_002_ac_004_raw_sql_preserves_order_and_native_fallback() {
    let database = PostgresTestSchema::create(&base_database_url())
        .await
        .expect("create isolated PostgreSQL schema");
    let pool = database.connect().await.expect("connect isolated schema");
    let sql = r#"
-- leading whitespace and comments are part of the opaque SQL text
create table native_sql_fixture (id integer primary key, label text not null);
insert into native_sql_fixture (id, label) values (1, '你好');
select id, label from native_sql_fixture order by id;
select id, label from native_sql_fixture where false;
select 1.25::float8 as ratio, '019fb5e6-88cc-7c20-891f-39a49f03d1ff'::uuid as source_id;
select point(1, 2) as native_value;
"#;

    let output = crate::execute_native_sql(&pool, sql)
        .await
        .expect("execute opaque SQL");

    assert_eq!(
        serde_json::to_value(output).expect("serialize canonical result"),
        json!({
            "results": [
                { "kind": "completion", "affected_rows": 0 },
                { "kind": "completion", "affected_rows": 1 },
                {
                    "kind": "row_batch",
                    "columns": [
                        {
                            "name": "id",
                            "native_type": "INT4",
                            "logical_type": "integer",
                            "encoding": "json"
                        },
                        {
                            "name": "label",
                            "native_type": "TEXT",
                            "logical_type": "string",
                            "encoding": "json"
                        }
                    ],
                    "rows": [[1, "你好"]]
                },
                { "kind": "completion", "affected_rows": 1 },
                { "kind": "completion", "affected_rows": 0 },
                {
                    "kind": "row_batch",
                    "columns": [
                        {
                            "name": "ratio",
                            "native_type": "FLOAT8",
                            "logical_type": "number",
                            "encoding": "text"
                        },
                        {
                            "name": "source_id",
                            "native_type": "UUID",
                            "logical_type": "uuid",
                            "encoding": "text"
                        }
                    ],
                    "rows": [["1.25", "019fb5e6-88cc-7c20-891f-39a49f03d1ff"]]
                },
                { "kind": "completion", "affected_rows": 1 },
                {
                    "kind": "row_batch",
                    "columns": [
                        {
                            "name": "native_value",
                            "native_type": "POINT",
                            "logical_type": "native",
                            "encoding": "text"
                        }
                    ],
                    "rows": [["(1,2)"]]
                },
                { "kind": "completion", "affected_rows": 1 }
            ]
        })
    );
}

#[tokio::test]
async fn ac_005_database_error_preserves_postgresql_code_and_detail_shape() {
    let database = PostgresTestSchema::create(&base_database_url())
        .await
        .expect("create isolated PostgreSQL schema");
    let pool = database.connect().await.expect("connect isolated schema");

    let error = crate::execute_native_sql(&pool, "select * from missing_native_sql_table")
        .await
        .expect_err("missing table must remain a source error");

    assert_eq!(error.provider_summary.as_deref(), Some("42P01"));
    assert_eq!(
        error
            .provider_details
            .as_ref()
            .and_then(|details| details.get("code"))
            .and_then(serde_json::Value::as_str),
        Some("42P01")
    );
    assert!(error.message.contains("missing_native_sql_table"));
}
