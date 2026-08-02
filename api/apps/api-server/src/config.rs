use anyhow::{anyhow, Result};
use axum::http::HeaderValue;
use serde::Deserialize;
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeMap,
    path::{Path, PathBuf},
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ApiEnvironment {
    Development,
    Production,
}

impl ApiEnvironment {
    fn parse(raw: Option<&str>) -> Result<Self> {
        match raw
            .unwrap_or("development")
            .trim()
            .to_ascii_lowercase()
            .as_str()
        {
            "development" | "dev" | "local" => Ok(Self::Development),
            "production" | "prod" => Ok(Self::Production),
            value => Err(anyhow!("invalid API_ENV `{value}`")),
        }
    }
}

#[derive(Debug, Clone)]
pub struct ApiConfig {
    pub env: ApiEnvironment,
    pub database_url: String,
    pub database_pool_max_connections: u32,
    pub business_file_local_root: String,
    pub plugin_runner_internal_base_url: String,
    pub cookie_name: String,
    pub cookie_secure: bool,
    pub session_ttl_days: i64,
    pub cors_allowed_origins: Option<Vec<HeaderValue>>,
    pub api_node_id: String,
    pub provider_install_root: String,
    pub agent_flow_template_library_root: String,
    pub mcp_template_library_root: String,
    pub provider_secret_master_key: String,
    pub host_extension_dropin_root: String,
    pub allow_unverified_filesystem_dropins: bool,
    pub allow_uploaded_host_extensions: bool,
    pub official_plugin_repository: String,
    pub official_plugin_default_registry_url: String,
    pub official_plugin_mirror_registry_url: Option<String>,
    pub official_plugin_github_proxy_url: Option<String>,
    pub official_plugin_signature_required: bool,
    pub official_plugin_trusted_public_keys_json: String,
    pub official_extension_catalog_sources:
        BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig>,
    pub official_agent_flow_template_default_index_url: String,
    pub official_agent_flow_template_mirror_index_url: Option<String>,
    pub official_mcp_bundle_default_catalog_url: String,
    pub official_mcp_bundle_mirror_catalog_url: Option<String>,
    pub official_i18n_catalog_repository: String,
    pub official_i18n_catalog_default_latest_url: String,
    pub official_i18n_catalog_mirror_latest_url: Option<String>,
    pub official_i18n_catalog_default_release_base_url: String,
    pub official_i18n_catalog_mirror_release_base_url: Option<String>,
    pub official_i18n_catalog_github_proxy_url: Option<String>,
    pub bootstrap_workspace_name: String,
    pub bootstrap_root_account: String,
    pub bootstrap_root_email: String,
    pub bootstrap_root_password: String,
    pub bootstrap_root_name: String,
    pub bootstrap_root_nickname: String,
}

#[derive(Debug, Clone)]
pub struct ResolvedOfficialPluginSourceConfig {
    pub source_kind: String,
    pub source_label: String,
    pub registry_url: String,
    pub github_proxy_url: Option<String>,
    pub trust_mode: String,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOfficialExtensionCatalogSourceConfig {
    pub source_kind: String,
    pub index_url: String,
    pub official_index_url: String,
    pub github_proxy_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedOfficialAgentFlowTemplateSourceConfig {
    pub source_kind: String,
    pub source_label: String,
    pub index_url: String,
    pub github_proxy_url: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ResolvedOfficialMcpBundleSourceConfig {
    pub source_kind: String,
    pub source_label: String,
    pub catalog_url: String,
    pub github_proxy_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedOfficialI18nCatalogSourceConfig {
    pub latest_url: String,
    pub release_base_url: String,
    pub github_proxy_url: Option<String>,
}

#[derive(Debug, Deserialize)]
struct TrustedPublicKeyConfig {
    key_id: String,
    algorithm: String,
    public_key_pem: String,
}

impl ApiConfig {
    pub fn from_env() -> Result<Self> {
        let vars = std::env::vars().collect::<Vec<_>>();
        let refs = vars
            .iter()
            .map(|(key, value)| (key.as_str(), value.as_str()))
            .collect::<Vec<_>>();

        Self::from_env_map(&refs)
    }

    pub fn from_env_map(entries: &[(&str, &str)]) -> Result<Self> {
        let map = entries
            .iter()
            .map(|(key, value)| ((*key).to_string(), (*value).to_string()))
            .collect::<BTreeMap<_, _>>();

        let get = |key: &str| -> Result<String> {
            map.get(key)
                .cloned()
                .ok_or_else(|| anyhow!("missing env {key}"))
        };
        let env = ApiEnvironment::parse(map.get("API_ENV").map(String::as_str))?;
        let cors_allowed_origins = parse_cors_allowed_origins(map.get("API_ALLOWED_ORIGINS"))?;
        let provider_install_root = map
            .get("API_PROVIDER_INSTALL_ROOT")
            .cloned()
            .unwrap_or_else(default_provider_install_root);
        let agent_flow_template_library_root = map
            .get("API_AGENT_FLOW_TEMPLATE_LIBRARY_ROOT")
            .cloned()
            .unwrap_or_else(default_agent_flow_template_library_root);
        let mcp_template_library_root = map
            .get("API_MCP_TEMPLATE_LIBRARY_ROOT")
            .cloned()
            .unwrap_or_else(default_mcp_template_library_root);
        let api_node_id = map
            .get("API_NODE_ID")
            .cloned()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| default_api_node_id(&provider_install_root));
        let provider_secret_master_key = map
            .get("API_PROVIDER_SECRET_MASTER_KEY")
            .cloned()
            .unwrap_or_else(|| "dev-provider-secret-master-key-unsafe".to_string());
        let host_extension_dropin_root = map
            .get("API_HOST_EXTENSION_DROPIN_ROOT")
            .cloned()
            .unwrap_or_else(|| {
                PathBuf::from(&provider_install_root)
                    .join("host-extension")
                    .join("dropins")
                    .display()
                    .to_string()
            });
        let allow_unverified_filesystem_dropins = parse_bool_flag(
            "API_PLUGIN_ALLOW_UNVERIFIED_FILESYSTEM_DROPINS",
            map.get("API_PLUGIN_ALLOW_UNVERIFIED_FILESYSTEM_DROPINS"),
            true,
        )?;
        let allow_uploaded_host_extensions = parse_bool_flag(
            "API_PLUGIN_ALLOW_UPLOADED_HOST_EXTENSIONS",
            map.get("API_PLUGIN_ALLOW_UPLOADED_HOST_EXTENSIONS"),
            false,
        )?;
        let official_plugin_repository = map
            .get("API_OFFICIAL_PLUGIN_REPOSITORY")
            .cloned()
            .unwrap_or_else(|| "taichuy/1flowbase-official-plugins".to_string());
        let official_plugin_default_registry_url = map
            .get("API_OFFICIAL_PLUGIN_DEFAULT_REGISTRY_URL")
            .cloned()
            .or_else(|| map.get("API_OFFICIAL_PLUGIN_REGISTRY_URL").cloned())
            .unwrap_or_else(|| {
                format!(
                    "https://raw.githubusercontent.com/{official_plugin_repository}/main/official-registry.json"
                )
            });
        let official_plugin_mirror_registry_url = map
            .get("API_OFFICIAL_PLUGIN_MIRROR_REGISTRY_URL")
            .cloned()
            .filter(|value| !value.trim().is_empty());
        let official_plugin_github_proxy_url = map
            .get("API_OFFICIAL_PLUGIN_GITHUB_PROXY_URL")
            .cloned()
            .filter(|value| !value.trim().is_empty());
        let official_plugin_signature_required = parse_bool_flag(
            "API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED",
            map.get("API_OFFICIAL_PLUGIN_SIGNATURE_REQUIRED"),
            true,
        )?;
        let official_plugin_trusted_public_keys_json = map
            .get("API_OFFICIAL_PLUGIN_TRUSTED_PUBLIC_KEYS_JSON")
            .cloned()
            .unwrap_or_else(default_official_plugin_trusted_public_keys_json);
        let official_extension_catalog_sources = resolve_official_extension_catalog_sources(
            &map,
            &official_plugin_repository,
            official_plugin_github_proxy_url.clone(),
        );
        let official_agent_flow_template_default_index_url = map
            .get("API_OFFICIAL_AGENT_FLOW_TEMPLATE_DEFAULT_INDEX_URL")
            .cloned()
            .or_else(|| map.get("API_OFFICIAL_AGENT_FLOW_TEMPLATE_INDEX_URL").cloned())
            .unwrap_or_else(|| {
                format!(
                    "https://raw.githubusercontent.com/{official_plugin_repository}/main/agent-flow/releases/v1/catalog.json"
                )
            });
        let official_agent_flow_template_mirror_index_url = map
            .get("API_OFFICIAL_AGENT_FLOW_TEMPLATE_MIRROR_INDEX_URL")
            .cloned()
            .filter(|value| !value.trim().is_empty());
        let official_mcp_bundle_default_catalog_url = map
            .get("API_OFFICIAL_MCP_BUNDLE_DEFAULT_CATALOG_URL")
            .cloned()
            .or_else(|| map.get("API_OFFICIAL_MCP_BUNDLE_CATALOG_URL").cloned())
            .unwrap_or_else(|| {
                format!(
                    "https://raw.githubusercontent.com/{official_plugin_repository}/main/mcp/catalog.json"
                )
            });
        let official_mcp_bundle_mirror_catalog_url = map
            .get("API_OFFICIAL_MCP_BUNDLE_MIRROR_CATALOG_URL")
            .cloned()
            .filter(|value| !value.trim().is_empty());
        let official_i18n_catalog_repository = map
            .get("API_OFFICIAL_I18N_CATALOG_REPOSITORY")
            .cloned()
            .unwrap_or_else(|| "taichuy/1flowbase-official-plugins".to_owned());
        let official_i18n_catalog_default_latest_url = map
            .get("API_OFFICIAL_I18N_CATALOG_DEFAULT_LATEST_URL")
            .cloned()
            .unwrap_or_else(|| {
                format!(
                    "https://raw.githubusercontent.com/{official_i18n_catalog_repository}/main/i18n/dist/catalog-seed.json"
                )
            });
        let official_i18n_catalog_mirror_latest_url = map
            .get("API_OFFICIAL_I18N_CATALOG_MIRROR_LATEST_URL")
            .cloned()
            .filter(|value| !value.trim().is_empty());
        let official_i18n_catalog_default_release_base_url = map
            .get("API_OFFICIAL_I18N_CATALOG_DEFAULT_RELEASE_BASE_URL")
            .cloned()
            .unwrap_or_else(|| {
                format!("https://github.com/{official_i18n_catalog_repository}/releases/download")
            });
        let official_i18n_catalog_mirror_release_base_url = map
            .get("API_OFFICIAL_I18N_CATALOG_MIRROR_RELEASE_BASE_URL")
            .cloned()
            .filter(|value| !value.trim().is_empty());
        let official_i18n_catalog_github_proxy_url = map
            .get("API_OFFICIAL_I18N_CATALOG_GITHUB_PROXY_URL")
            .cloned()
            .filter(|value| !value.trim().is_empty());

        if env == ApiEnvironment::Production && cors_allowed_origins.is_none() {
            return Err(anyhow!(
                "missing env API_ALLOWED_ORIGINS when API_ENV=production"
            ));
        }
        if env == ApiEnvironment::Production && !map.contains_key("API_PROVIDER_SECRET_MASTER_KEY")
        {
            return Err(anyhow!(
                "missing env API_PROVIDER_SECRET_MASTER_KEY when API_ENV=production"
            ));
        }
        if env == ApiEnvironment::Production
            && provider_secret_master_key_is_placeholder(&provider_secret_master_key)
        {
            return Err(anyhow!(
                "invalid env API_PROVIDER_SECRET_MASTER_KEY when API_ENV=production"
            ));
        }

        Ok(Self {
            env,
            database_url: get("API_DATABASE_URL")?,
            database_pool_max_connections: parse_positive_u32(
                "API_DATABASE_POOL_MAX_CONNECTIONS",
                map.get("API_DATABASE_POOL_MAX_CONNECTIONS"),
                5,
            )?,
            business_file_local_root: default_business_file_local_root(),
            plugin_runner_internal_base_url: map
                .get("API_PLUGIN_RUNNER_INTERNAL_BASE_URL")
                .cloned()
                .unwrap_or_else(|| "http://127.0.0.1:7801".to_string()),
            cookie_name: map
                .get("API_COOKIE_NAME")
                .cloned()
                .unwrap_or_else(|| "flowbase_console_session".to_string()),
            cookie_secure: parse_bool_flag(
                "API_COOKIE_SECURE",
                map.get("API_COOKIE_SECURE"),
                env == ApiEnvironment::Production,
            )?,
            session_ttl_days: map
                .get("API_SESSION_TTL_DAYS")
                .and_then(|value| value.parse::<i64>().ok())
                .unwrap_or(7),
            cors_allowed_origins,
            api_node_id,
            provider_install_root,
            agent_flow_template_library_root,
            mcp_template_library_root,
            provider_secret_master_key,
            host_extension_dropin_root,
            allow_unverified_filesystem_dropins,
            allow_uploaded_host_extensions,
            official_plugin_repository,
            official_plugin_default_registry_url,
            official_plugin_mirror_registry_url,
            official_plugin_github_proxy_url,
            official_plugin_signature_required,
            official_plugin_trusted_public_keys_json,
            official_extension_catalog_sources,
            official_agent_flow_template_default_index_url,
            official_agent_flow_template_mirror_index_url,
            official_mcp_bundle_default_catalog_url,
            official_mcp_bundle_mirror_catalog_url,
            official_i18n_catalog_repository,
            official_i18n_catalog_default_latest_url,
            official_i18n_catalog_mirror_latest_url,
            official_i18n_catalog_default_release_base_url,
            official_i18n_catalog_mirror_release_base_url,
            official_i18n_catalog_github_proxy_url,
            bootstrap_workspace_name: get("BOOTSTRAP_WORKSPACE_NAME")?,
            bootstrap_root_account: get("BOOTSTRAP_ROOT_ACCOUNT")?,
            bootstrap_root_email: get("BOOTSTRAP_ROOT_EMAIL")?,
            bootstrap_root_password: get("BOOTSTRAP_ROOT_PASSWORD")?,
            bootstrap_root_name: map
                .get("BOOTSTRAP_ROOT_NAME")
                .cloned()
                .unwrap_or_else(|| "Root".to_string()),
            bootstrap_root_nickname: map
                .get("BOOTSTRAP_ROOT_NICKNAME")
                .cloned()
                .unwrap_or_else(|| "Root".to_string()),
        })
    }

    pub fn resolve_official_plugin_source(&self) -> ResolvedOfficialPluginSourceConfig {
        if let Some(mirror_url) = self
            .official_plugin_mirror_registry_url
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            return ResolvedOfficialPluginSourceConfig {
                source_kind: "mirror_registry".into(),
                source_label: "Mirror source".into(),
                registry_url: mirror_url,
                github_proxy_url: self.official_plugin_github_proxy_url.clone(),
                trust_mode: self.official_plugin_trust_mode(),
            };
        }

        ResolvedOfficialPluginSourceConfig {
            source_kind: "official_registry".into(),
            source_label: "Official source".into(),
            registry_url: self.official_plugin_default_registry_url.clone(),
            github_proxy_url: self.official_plugin_github_proxy_url.clone(),
            trust_mode: self.official_plugin_trust_mode(),
        }
    }

    fn official_plugin_trust_mode(&self) -> String {
        if self.official_plugin_signature_required {
            "signature_required".to_string()
        } else {
            "allow_unsigned".to_string()
        }
    }

    pub fn resolve_official_agent_flow_template_source(
        &self,
    ) -> ResolvedOfficialAgentFlowTemplateSourceConfig {
        if let Some(mirror_url) = self
            .official_agent_flow_template_mirror_index_url
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            return ResolvedOfficialAgentFlowTemplateSourceConfig {
                source_kind: "mirror_registry".into(),
                source_label: "Mirror source".into(),
                index_url: mirror_url,
                github_proxy_url: self.official_plugin_github_proxy_url.clone(),
            };
        }

        ResolvedOfficialAgentFlowTemplateSourceConfig {
            source_kind: "official_registry".into(),
            source_label: "Official source".into(),
            index_url: self.official_agent_flow_template_default_index_url.clone(),
            github_proxy_url: self.official_plugin_github_proxy_url.clone(),
        }
    }

    pub fn resolve_official_mcp_bundle_source(&self) -> ResolvedOfficialMcpBundleSourceConfig {
        if let Some(mirror_url) = self
            .official_mcp_bundle_mirror_catalog_url
            .clone()
            .filter(|value| !value.trim().is_empty())
        {
            return ResolvedOfficialMcpBundleSourceConfig {
                source_kind: "mirror_registry".into(),
                source_label: "Mirror source".into(),
                catalog_url: mirror_url,
                github_proxy_url: self.official_plugin_github_proxy_url.clone(),
            };
        }
        ResolvedOfficialMcpBundleSourceConfig {
            source_kind: "official_registry".into(),
            source_label: "Official source".into(),
            catalog_url: self.official_mcp_bundle_default_catalog_url.clone(),
            github_proxy_url: self.official_plugin_github_proxy_url.clone(),
        }
    }

    pub fn resolve_official_i18n_catalog_source(&self) -> ResolvedOfficialI18nCatalogSourceConfig {
        let mirror_latest = self
            .official_i18n_catalog_mirror_latest_url
            .clone()
            .filter(|value| !value.trim().is_empty());
        let mirror_release_base = self
            .official_i18n_catalog_mirror_release_base_url
            .clone()
            .filter(|value| !value.trim().is_empty());
        ResolvedOfficialI18nCatalogSourceConfig {
            latest_url: mirror_latest
                .unwrap_or_else(|| self.official_i18n_catalog_default_latest_url.clone()),
            release_base_url: mirror_release_base
                .unwrap_or_else(|| self.official_i18n_catalog_default_release_base_url.clone()),
            github_proxy_url: self.official_i18n_catalog_github_proxy_url.clone(),
        }
    }

    pub fn official_plugin_trusted_public_keys(
        &self,
    ) -> Result<Vec<plugin_framework::TrustedPublicKey>> {
        serde_json::from_str::<Vec<TrustedPublicKeyConfig>>(
            &self.official_plugin_trusted_public_keys_json,
        )?
        .into_iter()
        .map(|entry| {
            Ok(plugin_framework::TrustedPublicKey {
                key_id: entry.key_id,
                algorithm: entry.algorithm,
                public_key_pem: entry.public_key_pem,
            })
        })
        .collect()
    }

    pub fn resolve_official_extension_catalog_source(
        &self,
        category: &str,
    ) -> Option<ResolvedOfficialExtensionCatalogSourceConfig> {
        self.official_extension_catalog_sources
            .get(category)
            .cloned()
    }
}

const OFFICIAL_EXTENSION_CATALOG_CATEGORIES: [(&str, &str); 6] = [
    ("agent-flow", "AGENT_FLOW"),
    ("capability-plugins", "CAPABILITY_PLUGINS"),
    ("host-extensions", "HOST_EXTENSIONS"),
    ("i18n", "I18N"),
    ("mcp", "MCP"),
    ("runtime-extensions", "RUNTIME_EXTENSIONS"),
];

fn resolve_official_extension_catalog_sources(
    map: &BTreeMap<String, String>,
    repository: &str,
    github_proxy_url: Option<String>,
) -> BTreeMap<String, ResolvedOfficialExtensionCatalogSourceConfig> {
    let default_base = format!("https://raw.githubusercontent.com/{repository}/main");
    let mirror_base = map
        .get("API_OFFICIAL_EXTENSION_CATALOG_MIRROR_BASE_URL")
        .map(String::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty());

    OFFICIAL_EXTENSION_CATALOG_CATEGORIES
        .into_iter()
        .map(|(category, env_suffix)| {
            let explicit_key = format!("API_OFFICIAL_EXTENSION_CATALOG_{env_suffix}_INDEX_URL");
            let explicit = map
                .get(&explicit_key)
                .map(String::as_str)
                .map(str::trim)
                .filter(|value| !value.is_empty());
            let (source_kind, index_url) = if let Some(index_url) = explicit {
                ("configured_mirror", index_url.to_string())
            } else if let Some(base) = mirror_base {
                (
                    "configured_mirror",
                    format!(
                        "{}/{category}/catalog/v1/index.json",
                        base.trim_end_matches('/')
                    ),
                )
            } else {
                (
                    "official_repository",
                    format!("{default_base}/{category}/catalog/v1/index.json"),
                )
            };
            (
                category.to_string(),
                ResolvedOfficialExtensionCatalogSourceConfig {
                    source_kind: source_kind.to_string(),
                    index_url,
                    official_index_url: format!("{default_base}/{category}/catalog/v1/index.json"),
                    github_proxy_url: github_proxy_url.clone(),
                },
            )
        })
        .collect()
}

fn default_api_node_id(provider_install_root: &str) -> String {
    let digest = format!("{:x}", Sha256::digest(provider_install_root.as_bytes()));
    format!("api-node-{}", &digest[..12])
}

fn provider_secret_master_key_is_placeholder(value: &str) -> bool {
    matches!(
        value.trim(),
        "" | "change-me-provider-secret-master-key" | "dev-provider-secret-master-key-unsafe"
    )
}

fn parse_cors_allowed_origins(value: Option<&String>) -> Result<Option<Vec<HeaderValue>>> {
    let Some(value) = value else {
        return Ok(None);
    };

    let origins = value
        .split(',')
        .map(str::trim)
        .filter(|entry| !entry.is_empty())
        .map(|entry| {
            HeaderValue::from_str(entry)
                .map_err(|_| anyhow!("invalid origin in API_ALLOWED_ORIGINS: `{entry}`"))
        })
        .collect::<Result<Vec<_>>>()?;

    if origins.is_empty() {
        return Ok(None);
    }

    Ok(Some(origins))
}

fn parse_positive_u32(key: &str, value: Option<&String>, default: u32) -> Result<u32> {
    let Some(value) = value else {
        return Ok(default);
    };
    let parsed = value
        .parse::<u32>()
        .map_err(|_| anyhow!("invalid env {key}: expected positive integer"))?;

    if parsed == 0 {
        return Err(anyhow!("invalid env {key}: expected positive integer"));
    }

    Ok(parsed)
}

fn default_provider_install_root() -> String {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_workspace_root(&current_dir)
        .unwrap_or(current_dir)
        .join("api")
        .join("plugins")
        .display()
        .to_string()
}

fn default_agent_flow_template_library_root() -> String {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_workspace_root(&current_dir)
        .unwrap_or(current_dir)
        .join("api")
        .join("storage")
        .join("extension-center")
        .join("agent-flow")
        .display()
        .to_string()
}

fn default_mcp_template_library_root() -> String {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_workspace_root(&current_dir)
        .unwrap_or(current_dir)
        .join("api")
        .join("storage")
        .join("extension-center")
        .join("mcp")
        .display()
        .to_string()
}

fn default_official_plugin_trusted_public_keys_json() -> String {
    r#"[{"key_id":"official-key-2026-04","algorithm":"ed25519","public_key_pem":"-----BEGIN PUBLIC KEY-----\nMCowBQYDK2VwAyEAuk3oonNd85FNP8CBRKj8RVvpdbhreoJiCguEJXPSgwg=\n-----END PUBLIC KEY-----"}]"#.to_string()
}

fn default_business_file_local_root() -> String {
    let current_dir = std::env::current_dir().unwrap_or_else(|_| PathBuf::from("."));
    find_workspace_root(&current_dir)
        .unwrap_or(current_dir)
        .join("api")
        .join("storage")
        .display()
        .to_string()
}

fn find_workspace_root(start: &Path) -> Option<PathBuf> {
    start
        .ancestors()
        .find(|path| {
            path.join(".git").exists() && path.join("api").is_dir() && path.join("web").is_dir()
        })
        .map(Path::to_path_buf)
}

fn parse_bool_flag(key: &str, value: Option<&String>, default: bool) -> Result<bool> {
    let Some(value) = value else {
        return Ok(default);
    };

    match value.trim().to_ascii_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Ok(true),
        "0" | "false" | "no" | "off" => Ok(false),
        _ => Err(anyhow!(
            "invalid env {key}: expected boolean flag, got `{value}`"
        )),
    }
}
