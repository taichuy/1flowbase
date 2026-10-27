use std::time::Duration;

use storage_durable::DurableBackendKind;
use storage_durable_postgres::{
    build_main_durable_postgres, build_main_durable_postgres_with_connection_settings,
    build_main_durable_postgres_with_pool_settings, MainDurableStore, PgConnectionSettings,
    PgPoolSettings,
};

#[test]
fn durable_backend_kind_parses_postgres() {
    assert_eq!(
        DurableBackendKind::from_env_value("postgres")
            .unwrap()
            .as_str(),
        "postgres"
    );
}

#[test]
fn main_durable_store_alias_points_at_storage_postgres() {
    let type_name = std::any::type_name::<MainDurableStore>();
    assert!(type_name.contains("storage_durable_postgres"));
}

#[test]
fn postgres_builder_is_part_of_public_surface() {
    let _ = build_main_durable_postgres;
}

#[test]
fn postgres_builder_accepts_pool_lifecycle_settings() {
    let mut settings = PgPoolSettings::with_max_connections(1);
    settings.idle_timeout = Some(Duration::from_millis(250));
    settings.max_lifetime = Some(Duration::from_secs(1));

    let _ = build_main_durable_postgres_with_pool_settings;
    assert_eq!(settings.max_connections, 1);
    assert_eq!(settings.idle_timeout, Some(Duration::from_millis(250)));
    assert_eq!(settings.max_lifetime, Some(Duration::from_secs(1)));
}

#[test]
fn durable_crate_name_and_postgres_crate_name_are_stable() {
    assert_eq!(storage_durable::crate_name(), "storage-durable");
    assert_eq!(
        storage_durable_postgres::crate_name(),
        "storage-durable-postgres"
    );
}

#[tokio::test]
async fn invalid_connection_allocation_is_rejected_before_opening_database() {
    let mut settings = PgConnectionSettings::compatible(PgPoolSettings::with_max_connections(5));
    settings.temporary_max_connections = Some(0);
    let result =
        build_main_durable_postgres_with_connection_settings("not-a-postgres-url", settings).await;
    let error = match result {
        Ok(_) => panic!("invalid connection allocation must be rejected"),
        Err(error) => error,
    };
    assert!(error.to_string().contains("lane limit must be positive"));
}

#[test]
fn pool_lifecycle_defaults_match_sqlx_and_explicit_none_disables_recycling() {
    let defaults = sqlx::postgres::PgPoolOptions::new();
    let mut settings = PgPoolSettings::with_max_connections(5);
    assert_eq!(settings.idle_timeout, defaults.get_idle_timeout());
    assert_eq!(settings.max_lifetime, defaults.get_max_lifetime());
    settings.idle_timeout = None;
    settings.max_lifetime = None;
    let options = settings.options();
    assert_eq!(options.get_idle_timeout(), None);
    assert_eq!(options.get_max_lifetime(), None);
}
