use anyhow::Result;
use sqlx::{postgres::PgPoolOptions, PgPool};
use std::time::Duration;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PgPoolSettings {
    pub max_connections: u32,
    pub acquire_timeout: Duration,
    pub idle_timeout: Option<Duration>,
    pub max_lifetime: Option<Duration>,
}

impl PgPoolSettings {
    pub fn with_max_connections(max_connections: u32) -> Self {
        Self {
            max_connections,
            acquire_timeout: Duration::from_secs(5),
            idle_timeout: None,
            max_lifetime: None,
        }
    }
}

pub async fn connect(database_url: &str) -> Result<PgPool> {
    connect_with_max_connections(database_url, 5).await
}

pub async fn connect_with_max_connections(
    database_url: &str,
    max_connections: u32,
) -> Result<PgPool> {
    connect_with_pool_settings(
        database_url,
        PgPoolSettings::with_max_connections(max_connections),
    )
    .await
}

pub async fn connect_with_pool_settings(
    database_url: &str,
    settings: PgPoolSettings,
) -> Result<PgPool> {
    let mut options = PgPoolOptions::new()
        .min_connections(0)
        .max_connections(settings.max_connections)
        .acquire_timeout(settings.acquire_timeout);

    if let Some(idle_timeout) = settings.idle_timeout {
        options = options.idle_timeout(idle_timeout);
    }

    if let Some(max_lifetime) = settings.max_lifetime {
        options = options.max_lifetime(max_lifetime);
    }

    let pool = options.connect(database_url).await?;

    Ok(pool)
}
