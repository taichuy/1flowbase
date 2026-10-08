//! Explicit database-owner maintenance; never runs automatically on startup or upload.
use anyhow::{anyhow, bail, Context, Result};
use control_plane::agent_logs::AgentLogsService;
use sqlx::postgres::PgPoolOptions;
use storage_durable_postgres::PgControlPlaneStore;
use uuid::Uuid;

#[tokio::main]
async fn main() -> Result<()> {
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 4 || args[0] != "--application-id" || args[2] != "--scope-id" {
        bail!("usage: agent_logs_reprice --application-id UUID --scope-id UUID (API_DATABASE_URL required)");
    }
    let application_id = args[1].parse::<Uuid>().context("invalid application ID")?;
    let scope_id = args[3].parse::<Uuid>().context("invalid scope ID")?;
    let url = std::env::var("API_DATABASE_URL").context("API_DATABASE_URL is required")?;
    let pool = PgPoolOptions::new()
        .connect(&url)
        .await
        .map_err(|_| anyhow!("database connection failed"))?;
    let service = AgentLogsService::new(PgControlPlaneStore::new(pool.clone()));
    let mut cursor = None;
    let mut records = 0;
    let mut changed_events = 0;
    while let Some(receipt) = service
        .reprice_next_record(application_id, scope_id, cursor)
        .await?
    {
        cursor = Some(receipt.record_id);
        records += 1;
        changed_events += receipt.changed_events;
        println!("{}", serde_json::to_string(&receipt)?);
    }
    pool.close().await;
    println!(
        "{}",
        serde_json::json!({"application_id":application_id,"records":records,"changed_events":changed_events,"status":"complete"})
    );
    Ok(())
}
