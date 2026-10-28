use anyhow::{Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    env, fs,
    io::Write,
    path::Path,
};

#[derive(Serialize, Deserialize)]
pub(super) struct Settings {
    pub postgres_password: String,
    pub provider_secret_master_key: String,
    pub root_password: String,
}

pub(super) fn private_dir(path: &Path) -> Result<()> {
    fs::create_dir_all(path)?;
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(path, fs::Permissions::from_mode(0o700))?;
    }
    Ok(())
}

pub(super) fn private_file(path: &Path) -> Result<fs::File> {
    let mut options = fs::OpenOptions::new();
    options.write(true).create_new(true);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    Ok(options.open(path)?)
}

pub(super) fn port(key: &str, default: u16) -> Result<u16> {
    let value = env::var(key)
        .ok()
        .map(|value| value.parse::<u16>())
        .transpose()
        .with_context(|| format!("{key} must be a valid port"))?
        .unwrap_or(default);
    anyhow::ensure!(value != 0, "{key} cannot be zero");
    Ok(value)
}

impl Settings {
    pub fn load_or_create(data: &Path) -> Result<Self> {
        let path = data.join("settings.json");
        if path.exists() {
            let value: Self = serde_json::from_slice(&fs::read(&path)?)
                .context("Invalid settings.json; restore the original configuration")?;
            anyhow::ensure!(
                !value.postgres_password.is_empty()
                    && !value.root_password.is_empty()
                    && value.provider_secret_master_key.len() >= 32,
                "Incomplete settings.json; restore original secrets"
            );
            return Ok(value);
        }
        anyhow::ensure!(
            !data.join("postgres").exists(),
            "Existing database without settings.json; restore original configuration"
        );
        let secret = || {
            rand::random::<[u8; 32]>()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let value = Self {
            postgres_password: secret(),
            provider_secret_master_key: secret(),
            root_password: secret(),
        };
        let temporary = data.join(format!("settings-{}.tmp", std::process::id()));
        let mut file = private_file(&temporary)?;
        file.write_all(&serde_json::to_vec_pretty(&value)?)?;
        file.sync_all()?;
        fs::rename(temporary, path)?;
        Ok(value)
    }

    pub fn apply(&self, root: &Path, data: &Path, http_port: u16, pg_port: u16) -> Result<()> {
        // Called before the Tokio runtime exists; no process-global environment mutation races.
        let mut database = url::Url::parse(&format!(
            "postgres://postgres@127.0.0.1:{pg_port}/1flowbase"
        ))?;
        database
            .set_password(Some(&self.postgres_password))
            .map_err(|_| anyhow::anyhow!("Invalid database URL"))?;
        for (key, value) in [
            ("API_ENV", "production".to_owned()),
            ("API_SERVER_ADDR", format!("127.0.0.1:{http_port}")),
            ("API_DATABASE_URL", database.to_string()),
            (
                "API_ALLOWED_ORIGINS",
                format!("http://127.0.0.1:{http_port},http://localhost:{http_port}"),
            ),
            (
                "API_AUTH_PUBLIC_ORIGIN",
                format!("http://127.0.0.1:{http_port}"),
            ),
            ("API_COOKIE_SECURE", "false".to_owned()),
            (
                "API_PROVIDER_SECRET_MASTER_KEY",
                self.provider_secret_master_key.clone(),
            ),
            ("BOOTSTRAP_ROOT_PASSWORD", self.root_password.clone()),
            ("BOOTSTRAP_ROOT_ACCOUNT", "root".to_owned()),
            ("BOOTSTRAP_ROOT_EMAIL", "root@localhost".to_owned()),
            ("BOOTSTRAP_WORKSPACE_NAME", "1flowbase".to_owned()),
        ] {
            env::set_var(key, value);
        }
        for (key, path) in [
            ("API_BUSINESS_FILE_LOCAL_ROOT", data.join("api/storage")),
            ("API_WORKSPACE_ROOT", root.join("api")),
            ("API_PROVIDER_INSTALL_ROOT", data.join("api/plugins")),
            (
                "API_HOST_EXTENSION_DROPIN_ROOT",
                data.join("api/plugins/host-extension/dropins"),
            ),
            (
                "API_MCP_TEMPLATE_LIBRARY_ROOT",
                data.join("api/storage/extension-center/mcp"),
            ),
            ("API_SYSTEM_BACKUP_REPOSITORY_ROOT", data.join("backups")),
            (
                "API_MODEL_PRICING_BOOTSTRAP_ROOT",
                root.join("api/resources/model-pricing"),
            ),
            (
                "API_APPLICATION_TEMPLATE_ROOT",
                root.join("api/resources/application-templates"),
            ),
            (
                "API_POSTGRES_PG_DUMP_PATH",
                root.join(format!("postgres/bin/pg_dump{}", env::consts::EXE_SUFFIX)),
            ),
            (
                "API_POSTGRES_PG_RESTORE_PATH",
                root.join(format!(
                    "postgres/bin/pg_restore{}",
                    env::consts::EXE_SUFFIX
                )),
            ),
        ] {
            env::set_var(key, path);
        }
        private_dir(&data.join("api/storage"))?;
        private_dir(&data.join("api/plugins"))?;
        Ok(())
    }
}
