//! Current typed notification delivery acceptance against real services and PostgreSQL.

async fn pool() -> sqlx::PgPool {
    let url = std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into());
    let database = postgres_test_support::PostgresTestSchema::create(&url)
        .await
        .unwrap();
    let pool = database.connect().await.unwrap();
    storage_durable_postgres::run_migrations(&pool)
        .await
        .unwrap();
    pool
}

mod credit_notification_delivery;
