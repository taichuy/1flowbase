mod coordination;
mod settings;
mod temporary;

use postgres_test_support::PostgresTestSchema;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::{future::Future, time::Duration};
use storage_durable_postgres::{
    build_main_durable_postgres_with_connection_settings, PgAdvisoryLockKey, PgConnectionSettings,
    PgControlPlaneStore, PgPoolSettings,
};
use tokio::time::{timeout, Instant};
use uuid::Uuid;

fn deadline() -> Instant {
    Instant::now() + Duration::from_secs(10)
}

fn settings(business: u32) -> PgConnectionSettings {
    let mut pool = PgPoolSettings::with_max_connections(business);
    pool.idle_timeout = None;
    pool.max_lifetime = None;
    let mut settings = PgConnectionSettings::compatible(pool);
    settings.health_check_interval = Duration::from_millis(20);
    settings
}

fn managed_key() -> (Uuid, Uuid, PgAdvisoryLockKey) {
    let installation = Uuid::now_v7();
    let scope = Uuid::now_v7();
    (
        installation,
        scope,
        PgAdvisoryLockKey::managed_mutation(installation, scope),
    )
}

async fn eventually(mut condition: impl FnMut() -> bool) {
    timeout(Duration::from_secs(10), async {
        while !condition() {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("observable connection state did not arrive");
}

async fn eventually_async<F, Fut>(mut condition: F)
where
    F: FnMut() -> Fut,
    Fut: Future<Output = bool>,
{
    timeout(Duration::from_secs(10), async {
        while !condition().await {
            tokio::task::yield_now().await;
        }
    })
    .await
    .expect("observable PostgreSQL state did not arrive");
}

struct Fixture {
    store: PgControlPlaneStore,
    observer: PgPool,
    schema: PostgresTestSchema,
}

impl Fixture {
    async fn new(settings: PgConnectionSettings) -> Self {
        // The batch supplies the base URL. All migrations, locks and backend termination
        // below run only inside this PostgresTestSchema-owned temporary database.
        let base = std::env::var("DATABASE_URL")
            .expect("Test Batch must supply DATABASE_URL for owned PostgreSQL fixtures");
        let schema = PostgresTestSchema::create(&base).await.unwrap();
        let store =
            build_main_durable_postgres_with_connection_settings(schema.database_url(), settings)
                .await
                .unwrap()
                .store;
        let observer_options = schema
            .database_url()
            .parse::<sqlx::postgres::PgConnectOptions>()
            .unwrap()
            .application_name("w1-fixture:observer");
        let observer = PgPoolOptions::new()
            .max_connections(1)
            .connect_with(observer_options)
            .await
            .unwrap();
        Self {
            store,
            observer,
            schema,
        }
    }

    async fn another_store(&self, settings: PgConnectionSettings) -> PgControlPlaneStore {
        build_main_durable_postgres_with_connection_settings(self.schema.database_url(), settings)
            .await
            .unwrap()
            .store
    }

    async fn managed_pid(&self) -> i32 {
        sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:managed-lock'")
            .fetch_one(&self.observer).await.unwrap()
    }

    async fn session_exists(&self, pid: i32) -> bool {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname = current_database() AND pid = $1)")
            .bind(pid).fetch_one(&self.observer).await.unwrap()
    }

    async fn lock_waiter_pid(&self) -> Option<i32> {
        sqlx::query_scalar("SELECT a.pid FROM pg_stat_activity a WHERE a.datname = current_database() AND a.application_name = '1flowbase:managed-lock' AND EXISTS(SELECT 1 FROM pg_locks l WHERE l.pid = a.pid AND l.locktype = 'advisory' AND NOT l.granted)")
            .fetch_optional(&self.observer).await.unwrap()
    }

    async fn close(self) {
        self.store.connections().close().await;
        self.observer.close().await;
    }
}
