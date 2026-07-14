#[tokio::main]
async fn main() -> anyhow::Result<()> {
    api_server::console_policy_migration::run_from_env().await
}
