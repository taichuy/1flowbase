#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurableBackendKind {
    Postgres,
}

impl DurableBackendKind {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Postgres => "postgres",
        }
    }

    pub fn from_env_value(value: &str) -> anyhow::Result<Self> {
        match value.trim().to_ascii_lowercase().as_str() {
            "postgres" => Ok(Self::Postgres),
            other => Err(anyhow::anyhow!("unsupported durable backend: {other}")),
        }
    }
}
