use super::{connect_with_pool_settings, PgPoolSettings};
use anyhow::Result;
use futures_util::TryStreamExt;
use sqlx::{Acquire, PgPool, Row, Value, ValueRef};
use std::time::Duration;

async fn production_pool() -> Result<PgPool> {
    // One connection makes protocol recovery and reuse observable. This is only
    // a fixture setting; production settings are passed through unchanged.
    let url = std::env::var("DATABASE_URL")?;
    connect_with_pool_settings(&url, PgPoolSettings::with_max_connections(1)).await
}

async fn backend(pool: &PgPool) -> Result<i32> {
    Ok(sqlx::query_scalar("SELECT pg_backend_pid()")
        .fetch_one(pool)
        .await?)
}

#[tokio::test]
async fn large_bind_and_retained_row_value_survive_pool_release() -> Result<()> {
    let pool = production_pool().await?;
    let initial_backend = backend(&pool).await?;
    for repetitions in [32_768, 32_768, 65_536] {
        let text = "长历史🙂工具结果\n".repeat(repetitions);
        let bytes = text.as_bytes();
        let row = sqlx::query("SELECT $1::text AS text, $2::bytea AS raw, NULL::text AS absent")
            .bind(&text)
            .bind(bytes)
            .fetch_one(&pool)
            .await?;
        let value = ValueRef::to_owned(&row.try_get_raw("text")?);
        // Acquiring the only connection waits for the real release callback.
        assert_eq!(backend(&pool).await?, initial_backend);
        assert_eq!(row.try_get::<&str, _>("text")?, text);
        assert_eq!(row.try_get::<&[u8], _>("raw")?, bytes);
        assert_eq!(row.try_get::<Option<&str>, _>("absent")?, None);
        drop(row);
        assert_eq!(backend(&pool).await?, initial_backend);
        assert_eq!(value.try_decode::<&str>()?, text);
    }
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn dropped_transaction_rolls_back_before_connection_reuse() -> Result<()> {
    let pool = production_pool().await?;
    let initial_backend = backend(&pool).await?;
    sqlx::query("CREATE TEMP TABLE release_tx (value integer)")
        .execute(&pool)
        .await?;
    let mut transaction = pool.begin().await?;
    sqlx::query("INSERT INTO release_tx VALUES (11)")
        .execute(&mut *transaction)
        .await?;
    drop(transaction); // queues ROLLBACK before PoolConnection::drop
    assert_eq!(backend(&pool).await?, initial_backend);
    let count: i64 = sqlx::query_scalar("SELECT count(*) FROM release_tx")
        .fetch_one(&pool)
        .await?;
    assert_eq!(count, 0);

    let mut transaction = pool.begin().await?;
    sqlx::query("INSERT INTO release_tx VALUES (23)")
        .execute(&mut *transaction)
        .await?;
    transaction.commit().await?;
    assert_eq!(backend(&pool).await?, initial_backend);
    let value: i32 = sqlx::query_scalar("SELECT value FROM release_tx")
        .fetch_one(&pool)
        .await?;
    assert_eq!(value, 23);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn dropped_savepoint_preserves_outer_transaction() -> Result<()> {
    let pool = production_pool().await?;
    sqlx::query("CREATE TEMP TABLE release_savepoint (value integer)")
        .execute(&pool)
        .await?;
    let initial_backend = backend(&pool).await?;
    let mut outer = pool.begin().await?;
    sqlx::query("INSERT INTO release_savepoint VALUES (31)")
        .execute(&mut *outer)
        .await?;
    let mut nested = outer.begin().await?;
    sqlx::query("INSERT INTO release_savepoint VALUES (47)")
        .execute(&mut *nested)
        .await?;
    drop(nested);
    outer.commit().await?;
    assert_eq!(backend(&pool).await?, initial_backend);
    let values: Vec<i32> = sqlx::query_scalar("SELECT value FROM release_savepoint ORDER BY value")
        .fetch_all(&pool)
        .await?;
    assert_eq!(values, [31]);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn dropped_result_stream_preserves_retained_row_and_next_query() -> Result<()> {
    let pool = production_pool().await?;
    let initial_backend = backend(&pool).await?;
    let text = "流🙂工具".repeat(65_536);
    let mut connection = pool.acquire().await?;
    let mut stream = sqlx::query("SELECT i, $1::text AS payload FROM generate_series(1, 8) AS i")
        .bind(&text)
        .fetch(&mut *connection);
    let first = stream.try_next().await?.expect("first row");
    drop(stream);
    drop(connection);
    assert_eq!(backend(&pool).await?, initial_backend);
    assert_eq!(first.try_get::<i32, _>("i")?, 1);
    assert_eq!(first.try_get::<&str, _>("payload")?, text);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn cancelled_query_drains_before_same_connection_is_reused() -> Result<()> {
    let pool = production_pool().await?;
    let initial_backend = backend(&pool).await?;
    let mut connection = pool.acquire().await?;
    let interrupted = tokio::time::timeout(
        Duration::from_millis(30),
        sqlx::query("SELECT pg_sleep(0.15), $1::text")
            .bind("取消🙂".repeat(65_536))
            .fetch_one(&mut *connection),
    )
    .await;
    assert!(interrupted.is_err(), "fixture must interrupt the query");
    drop(connection);
    let recovered = tokio::time::timeout(Duration::from_secs(5), backend(&pool)).await??;
    assert_eq!(recovered, initial_backend);
    pool.close().await;
    Ok(())
}

#[tokio::test]
async fn database_error_and_failed_transaction_do_not_poison_pool() -> Result<()> {
    let pool = production_pool().await?;
    let mut transaction = pool.begin().await?;
    let error = sqlx::query("SELECT 1 / 0")
        .execute(&mut *transaction)
        .await
        .expect_err("division by zero");
    assert_eq!(
        error.as_database_error().and_then(|e| e.code()).as_deref(),
        Some("22012")
    );
    drop(transaction);
    let result: i32 = tokio::time::timeout(
        Duration::from_secs(5),
        sqlx::query_scalar("SELECT 42").fetch_one(&pool),
    )
    .await??;
    assert_eq!(result, 42);
    pool.close().await;
    Ok(())
}
