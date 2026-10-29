use super::env_file::{self, Values};
use anyhow::{ensure, Context, Result};
use std::{
    env, fs,
    net::SocketAddr,
    path::{Path, PathBuf},
};

pub(super) const GENERATE: &str = "__GENERATE__";
const PATH_KEYS: &[&str] = &[
    "API_BUSINESS_FILE_LOCAL_ROOT",
    "API_WORKSPACE_ROOT",
    "API_PROVIDER_INSTALL_ROOT",
    "API_HOST_EXTENSION_DROPIN_ROOT",
    "API_MCP_TEMPLATE_LIBRARY_ROOT",
    "API_SYSTEM_BACKUP_REPOSITORY_ROOT",
    "API_MODEL_PRICING_BOOTSTRAP_ROOT",
    "API_APPLICATION_TEMPLATE_ROOT",
    "API_POSTGRES_PG_DUMP_PATH",
    "API_POSTGRES_PG_RESTORE_PATH",
];

pub(super) struct Settings {
    pub values: Values,
    pub data: PathBuf,
    pub address: SocketAddr,
    pub postgres_port: u16,
    pub postgres_password: String,
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

pub(super) fn resolve(root: &Path, path: &str) -> PathBuf {
    let path = Path::new(path);
    if path.is_absolute() {
        path.to_owned()
    } else {
        root.join(path)
    }
}

impl Settings {
    /// .env is the sole persisted configuration. Inspection never writes or generates secrets.
    pub fn inspect(root: &Path) -> Result<Self> {
        Self::from_values(root, env_file::read(root)?)
    }

    pub fn from_values(root: &Path, values: Values) -> Result<Self> {
        let address: SocketAddr = env_file::required(&values, "API_SERVER_ADDR")?
            .parse()
            .context("Invalid API_SERVER_ADDR")?;
        ensure!(address.port() != 0, "API_SERVER_ADDR port must not be zero");
        let database = url::Url::parse(env_file::required(&values, "API_DATABASE_URL")?)
            .context("Invalid API_DATABASE_URL")?;
        ensure!(matches!(database.scheme(), "postgres" | "postgresql") && database.host_str() == Some("127.0.0.1") && database.username() == "postgres" && database.path() == "/1flowbase" && database.query().is_none() && database.fragment().is_none(), "Native packages manage postgres@127.0.0.1/1flowbase only; configure its port in API_DATABASE_URL");
        let postgres_port = database
            .port()
            .context("API_DATABASE_URL must include the private PostgreSQL port")?;
        ensure!(
            postgres_port != 0 && postgres_port != address.port(),
            "HTTP and PostgreSQL ports must differ and be nonzero"
        );
        let postgres_password = database
            .password()
            .context("API_DATABASE_URL must include a password or __GENERATE__")?
            .to_owned();
        // PostgreSQL URL credentials are percent-encoded, but generated passwords are ASCII hex.
        let postgres_password = percent_encoding::percent_decode_str(&postgres_password)
            .decode_utf8()?
            .into_owned();
        ensure!(!postgres_password.is_empty(), "Empty PostgreSQL password");
        let master_key = env_file::required(&values, "API_PROVIDER_SECRET_MASTER_KEY")?;
        ensure!(
            master_key == GENERATE || master_key.len() >= 32,
            "API_PROVIDER_SECRET_MASTER_KEY must have at least 32 characters or be __GENERATE__"
        );
        for key in [
            "API_ENV",
            "BOOTSTRAP_ROOT_ACCOUNT",
            "BOOTSTRAP_ROOT_EMAIL",
            "BOOTSTRAP_WORKSPACE_NAME",
        ]
        .into_iter()
        .chain(PATH_KEYS.iter().copied())
        {
            env_file::required(&values, key)?;
        }
        env_file::required(&values, "BOOTSTRAP_ROOT_PASSWORD")?;
        let data = resolve(root, env_file::required(&values, "FLOWBASE_DATA_DIR")?);
        ensure!(
            data != root && !root.starts_with(&data),
            "FLOWBASE_DATA_DIR cannot contain the program directory"
        );
        if data.join("postgres").exists() {
            ensure!(postgres_password != GENERATE && master_key != GENERATE, "Existing PostgreSQL data requires the original .env credentials; secrets will not be regenerated");
        }
        Ok(Self {
            values,
            data,
            address,
            postgres_port,
            postgres_password,
        })
    }

    pub fn load(root: &Path) -> Result<Self> {
        let mut settings = Self::inspect(root)?;
        let secret = || {
            rand::random::<[u8; 32]>()
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>()
        };
        let mut generated = Values::new();
        if settings.postgres_password == GENERATE {
            let mut database =
                url::Url::parse(env_file::required(&settings.values, "API_DATABASE_URL")?)?;
            settings.postgres_password = secret();
            database
                .set_password(Some(&settings.postgres_password))
                .map_err(|_| anyhow::anyhow!("Invalid database URL"))?;
            generated.insert("API_DATABASE_URL".into(), database.to_string());
        }
        if settings.values["API_PROVIDER_SECRET_MASTER_KEY"] == GENERATE {
            generated.insert("API_PROVIDER_SECRET_MASTER_KEY".into(), secret());
        }
        env_file::persist_generated(root, &generated)?;
        settings.values.extend(generated);
        Ok(settings)
    }

    pub fn apply(&self, root: &Path) -> Result<()> {
        // Called before Tokio starts. File entries intentionally override inherited values.
        for (key, value) in &self.values {
            env::set_var(key, value);
        }
        for &key in PATH_KEYS {
            let value = env_file::required(&self.values, key)?;
            env::set_var(key, resolve(root, value));
        }
        private_dir(&self.data.join("api/storage"))?;
        private_dir(&self.data.join("api/plugins"))?;
        Ok(())
    }
}
