use sqlx::PgPool;
use storage_durable_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const WORKFLOW_SCHEDULE_IDEMPOTENCY_INDEX_MIGRATION: &str = include_str!(
    "../../../migrations/20260720110000_add_workflow_schedule_run_idempotency_unique_index.sql"
);

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

async fn isolated_database() -> postgres_test_support::PostgresTestSchema {
    postgres_test_support::PostgresTestSchema::create(&base_database_url())
        .await
        .unwrap()
}

#[path = "migration_smoke/bootstrap_auth.rs"]
mod bootstrap_auth;
#[path = "migration_smoke/extensions.rs"]
mod extensions;
#[path = "migration_smoke/public_runs.rs"]
mod public_runs;
#[path = "migration_smoke/run_scheduling.rs"]
mod run_scheduling;
#[path = "migration_smoke/schema_readiness.rs"]
mod schema_readiness;
#[path = "migration_smoke/support.rs"]
mod support;

use support::*;
