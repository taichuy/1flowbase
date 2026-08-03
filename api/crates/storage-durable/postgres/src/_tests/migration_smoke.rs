use sqlx::PgPool;
use storage_postgres::{run_migrations, PgControlPlaneStore};
use uuid::Uuid;

const WORKFLOW_SCHEDULE_IDEMPOTENCY_INDEX_MIGRATION: &str = include_str!(
    "../../migrations/20260720110000_add_workflow_schedule_run_idempotency_unique_index.sql"
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

mod bootstrap_auth;
mod extensions;
mod public_runs;
mod run_scheduling;
mod schema_readiness;
mod support;

use support::*;
