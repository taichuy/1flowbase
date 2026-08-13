#[tokio::main]
async fn main() -> anyhow::Result<()> {
    api_server::init_tracing();
    api_server::frontstage_executable_upgrade::verify_release_artifact()?;
    let database_url = std::env::var("API_DATABASE_URL")
        .map_err(|_| anyhow::anyhow!("API_DATABASE_URL is required"))?;
    let durable = storage_durable::build_main_durable_postgres(&database_url).await?;
    api_server::frontstage_executable_upgrade::run_upgrade(durable.store).await
}
