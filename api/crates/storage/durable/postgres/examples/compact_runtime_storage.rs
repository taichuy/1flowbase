//! Explicit operator command. Credentials are inherited from the environment,
//! and only counts are emitted; retained bodies are never printed.
use anyhow::{anyhow, Result};
use storage_durable_postgres::{connect, run_migrations, PgControlPlaneStore};
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let mut runs = Vec::new();
    let mut migrate = false;
    let mut batch = 256;
    let mut arguments = std::env::args().skip(1);
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--flow-run"=>runs.push(arguments.next().ok_or_else(||anyhow!("missing run ID"))?.parse::<Uuid>()?),
            "--batch-size"=>batch=arguments.next().ok_or_else(||anyhow!("missing batch size"))?.parse::<i64>()?,
            "--migrate"=>migrate=true,
            _=>return Err(anyhow!("usage: compact_runtime_storage [--migrate] [--batch-size N] --flow-run UUID [--flow-run UUID ...]")),
        }
    }
    anyhow::ensure!(
        !runs.is_empty(),
        "an explicit flow-run allowlist is required"
    );
    let url = std::env::var("API_DATABASE_URL")
        .or_else(|_| std::env::var("DATABASE_URL"))
        .map_err(|_| anyhow!("database environment is required"))?;
    let pool = connect(&url)
        .await
        .map_err(|_| anyhow!("database connection failed"))?;
    if migrate {
        run_migrations(&pool)
            .await
            .map_err(|_| anyhow!("formal migrations failed"))?;
    }
    let store = PgControlPlaneStore::new(pool);
    let receipt = store
        .compact_retained_runtime_storage(&runs, batch)
        .await
        .map_err(|_| {
            anyhow!("verified compaction failed; unfinished rows retain their committed layout")
        })?;
    println!("{}", serde_json::to_string(&receipt)?);
    Ok(())
}
