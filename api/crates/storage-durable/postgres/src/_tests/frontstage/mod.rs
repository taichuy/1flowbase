use std::borrow::Cow;

use serde_json::{json, Value};
use sqlx::{migrate::Migrator, PgPool};
use storage_postgres::run_migrations;
use uuid::Uuid;

fn base_database_url() -> String {
    std::env::var("DATABASE_URL")
        .unwrap_or_else(|_| "postgres://postgres:1flowbase@127.0.0.1:35432/1flowbase".into())
}

mod atomic_operations;
mod page_tab_ownership;
mod page_tabs_migration;
mod placement_integrity;
mod schema_integrity;
mod support;
mod visibility_rules;

use support::*;
