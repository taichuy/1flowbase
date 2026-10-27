mod coordination;
mod settings;
mod temporary;

use postgres_test_support::PostgresTestSchema;
use sqlx::{postgres::PgPoolOptions, Connection, PgConnection, PgPool};
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

struct RemoteHolder {
    connection: PgConnection,
    pid: i32,
    application_name: String,
    key_text: String,
    coordinator_name: &'static str,
}

impl RemoteHolder {
    async fn release(self) {
        self.connection.close().await.unwrap();
    }
}

#[derive(Debug)]
struct ServerLockSnapshot {
    coordinator_pids: Vec<i32>,
    blocked_pids: Vec<i32>,
    holder_holds: bool,
}

impl ServerLockSnapshot {
    fn no_waiter_accumulation(&self, local_cap: u32) -> bool {
        self.holder_holds
            && self.blocked_pids.is_empty()
            && self.coordinator_pids.len() <= local_cap as usize
    }
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

    async fn template_pid(&self) -> i32 {
        sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:template-lock'")
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

    async fn has_managed_probe(&self) -> bool {
        sqlx::query_scalar("SELECT EXISTS(SELECT 1 FROM pg_stat_activity WHERE datname = current_database() AND application_name = '1flowbase:managed-lock' AND position('advisory' IN query) > 0)")
            .fetch_one(&self.observer).await.unwrap()
    }

    async fn remote_holder(
        &self,
        installation: Uuid,
        scope: Uuid,
        transaction: bool,
    ) -> RemoteHolder {
        self.remote_lock(
            format!("managed-service-mutation:{installation}:{scope}"),
            "1flowbase:managed-lock",
            transaction,
        )
        .await
    }

    async fn remote_template_holder(&self, workspace: Uuid, transaction: bool) -> RemoteHolder {
        self.remote_lock(
            format!("application-template:{workspace}"),
            "1flowbase:template-lock",
            transaction,
        )
        .await
    }

    async fn remote_lock(
        &self,
        key_text: String,
        coordinator_name: &'static str,
        transaction: bool,
    ) -> RemoteHolder {
        let application_name = format!("w1a-remote-holder-{}", Uuid::now_v7().simple());
        let options = (*self.observer.connect_options())
            .clone()
            .application_name(&application_name);
        let mut connection = PgConnection::connect_with(&options).await.unwrap();
        let pid = sqlx::query_scalar("SELECT pg_backend_pid()")
            .fetch_one(&mut connection)
            .await
            .unwrap();
        if transaction {
            sqlx::query("BEGIN").execute(&mut connection).await.unwrap();
        }
        let query = if transaction {
            "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))"
        } else {
            "SELECT pg_advisory_lock(hashtextextended($1, 0))"
        };
        sqlx::query(query)
            .bind(&key_text)
            .execute(&mut connection)
            .await
            .unwrap();
        RemoteHolder {
            connection,
            pid,
            application_name,
            key_text,
            coordinator_name,
        }
    }

    async fn server_lock_snapshot(&self, holder: &RemoteHolder) -> ServerLockSnapshot {
        let coordinator_pids = sqlx::query_scalar("SELECT pid FROM pg_stat_activity WHERE datname = current_database() AND application_name = $1 ORDER BY pid")
            .bind(holder.coordinator_name).fetch_all(&self.observer).await.unwrap();
        // bigint advisory keys occupy two uint32 lock-tag fields; objsubid=1 is the
        // single-bigint API. Match the exact legacy key, database, application and PID.
        let blocked_pids = sqlx::query_scalar("WITH target AS (SELECT hashtextextended($1, 0) AS key) SELECT a.pid FROM pg_stat_activity a JOIN pg_locks l ON l.pid = a.pid CROSS JOIN target t WHERE a.datname = current_database() AND a.application_name = $2 AND l.locktype = 'advisory' AND NOT l.granted AND l.objsubid = 1 AND l.classid::bigint = ((t.key >> 32) & 4294967295) AND l.objid::bigint = (t.key & 4294967295) ORDER BY a.pid")
            .bind(&holder.key_text).bind(holder.coordinator_name).fetch_all(&self.observer).await.unwrap();
        let holder_holds = sqlx::query_scalar("WITH target AS (SELECT hashtextextended($1, 0) AS key) SELECT EXISTS(SELECT 1 FROM pg_stat_activity a JOIN pg_locks l ON l.pid = a.pid CROSS JOIN target t WHERE a.datname = current_database() AND a.pid = $2 AND a.application_name = $3 AND l.locktype = 'advisory' AND l.granted AND l.objsubid = 1 AND l.classid::bigint = ((t.key >> 32) & 4294967295) AND l.objid::bigint = (t.key & 4294967295))")
            .bind(&holder.key_text).bind(holder.pid).bind(&holder.application_name).fetch_one(&self.observer).await.unwrap();
        ServerLockSnapshot {
            coordinator_pids,
            blocked_pids,
            holder_holds,
        }
    }

    async fn close(self) {
        self.store.connections().close().await;
        self.observer.close().await;
    }
}
