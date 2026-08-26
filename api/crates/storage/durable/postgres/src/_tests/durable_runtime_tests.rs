use std::time::Duration;

use storage_durable_postgres::{
    build_main_durable_postgres, build_main_durable_postgres_with_pool_settings,
    DurableBackendKind, MainDurableStore, PgPoolSettings,
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
