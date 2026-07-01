use anyhow::Result;
use api_server::config::ApiConfig;
use argon2::{
    password_hash::{PasswordHasher, SaltString},
    Argon2,
};
use control_plane::bootstrap::{BootstrapConfig, BootstrapService};
use rand_core::OsRng;
use storage_durable::build_main_durable_postgres;

#[tokio::main]
async fn main() -> Result<()> {
    let config = ApiConfig::from_env()?;
    let durable = build_main_durable_postgres(&config.database_url).await?;
    let store = durable.store;
    let salt = SaltString::generate(&mut OsRng);
    let password_hash = Argon2::default()
        .hash_password(config.bootstrap_root_password.as_bytes(), &salt)
        .map_err(|err| anyhow::anyhow!("failed to hash root password: {err}"))?
        .to_string();

    let bootstrap = BootstrapService::new(store.clone())
        .run(&BootstrapConfig {
            workspace_name: config.bootstrap_workspace_name.clone(),
            root_account: config.bootstrap_root_account.clone(),
            root_email: config.bootstrap_root_email.clone(),
            root_password_hash: password_hash.clone(),
            root_name: config.bootstrap_root_name.clone(),
            root_nickname: config.bootstrap_root_nickname.clone(),
        })
        .await?;
    store
        .update_password_hash(
            bootstrap.root_user_id,
            &password_hash,
            bootstrap.root_user_id,
        )
        .await?;

    println!("reset root password for {}", config.bootstrap_root_account);
    Ok(())
}
