use storage_durable::DurableBackendKind;

pub type MainDurableStore = crate::PgControlPlaneStore;

#[derive(Clone)]
pub struct MainDurableRuntime {
    pub kind: DurableBackendKind,
    pub store: MainDurableStore,
}

pub async fn build_main_durable_postgres(database_url: &str) -> anyhow::Result<MainDurableRuntime> {
    build_main_durable_postgres_with_max_connections(database_url, 5).await
}

pub async fn build_main_durable_postgres_with_max_connections(
    database_url: &str,
    max_connections: u32,
) -> anyhow::Result<MainDurableRuntime> {
    build_main_durable_postgres_with_pool_settings(
        database_url,
        crate::PgPoolSettings::with_max_connections(max_connections),
    )
    .await
}

pub async fn build_main_durable_postgres_with_pool_settings(
    database_url: &str,
    settings: crate::PgPoolSettings,
) -> anyhow::Result<MainDurableRuntime> {
    let pool = crate::connect_with_pool_settings(database_url, settings).await?;
    crate::run_migrations(&pool).await?;

    Ok(MainDurableRuntime {
        kind: DurableBackendKind::Postgres,
        store: crate::PgControlPlaneStore::new(pool),
    })
}
