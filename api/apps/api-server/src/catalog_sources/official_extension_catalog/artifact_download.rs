use super::*;

const GITHUB_RAW_CONTENT_BASE_URL: &str = "https://raw.githubusercontent.com/";

pub(super) async fn download_with_client_budget(
    client: &Client,
    url: &str,
    max_bytes: usize,
    resource: &'static str,
) -> Result<Vec<u8>> {
    let response = client.get(url).send().await.map_err(|source| {
        anyhow::Error::new(ExtensionDownloadFailure::Request {
            resource,
            url: url.to_string(),
            source,
        })
    })?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow::Error::new(ExtensionDownloadFailure::Status {
            resource,
            url: url.to_string(),
            status,
        }));
    }
    let mut stream = response.bytes_stream();
    let mut bytes = Vec::new();
    while let Some(chunk) = stream.next().await {
        let chunk = chunk.map_err(|source| {
            anyhow::Error::new(ExtensionDownloadFailure::Body {
                resource,
                url: url.to_string(),
                source,
            })
        })?;
        if bytes.len().saturating_add(chunk.len()) > max_bytes {
            return Err(anyhow::Error::new(ExtensionDownloadFailure::Budget {
                resource,
            }));
        }
        bytes.extend_from_slice(&chunk);
    }
    if bytes.is_empty() {
        return Err(anyhow::Error::new(ExtensionDownloadFailure::Empty {
            resource,
        }));
    }
    Ok(bytes)
}

#[derive(Debug, Error)]
pub(super) enum ExtensionDownloadFailure {
    #[error("failed to request {resource} from {url}: {source}")]
    Request {
        resource: &'static str,
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{resource} returned status {status} for {url}")]
    Status {
        resource: &'static str,
        url: String,
        status: reqwest::StatusCode,
    },
    #[error("failed to read {resource} response body: {source}")]
    Body {
        resource: &'static str,
        url: String,
        #[source]
        source: reqwest::Error,
    },
    #[error("{resource} exceeds download budget")]
    Budget { resource: &'static str },
    #[error("{resource} is empty")]
    Empty { resource: &'static str },
}

pub(super) fn artifact_download_error(
    error: ExtensionDownloadFailure,
) -> OfficialExtensionArtifactError {
    match error {
        ExtensionDownloadFailure::Request { url, source, .. } => {
            OfficialExtensionArtifactError::NetworkUnavailable {
                host: artifact_host(&url),
                source,
            }
        }
        ExtensionDownloadFailure::Status { url, status, .. }
            if status == reqwest::StatusCode::NOT_FOUND =>
        {
            OfficialExtensionArtifactError::NotFound {
                host: artifact_host(&url),
                status: status.as_u16(),
            }
        }
        ExtensionDownloadFailure::Status { url, status, .. } => {
            OfficialExtensionArtifactError::UpstreamRejected {
                host: artifact_host(&url),
                status: status.as_u16(),
            }
        }
        ExtensionDownloadFailure::Body { url, source, .. } => {
            OfficialExtensionArtifactError::ResponseInterrupted {
                host: artifact_host(&url),
                source,
            }
        }
        ExtensionDownloadFailure::Budget { .. } => {
            OfficialExtensionArtifactError::DownloadBudgetExceeded
        }
        ExtensionDownloadFailure::Empty { .. } => OfficialExtensionArtifactError::EmptyArtifact,
    }
}

pub(super) fn artifact_host(url: &str) -> String {
    reqwest::Url::parse(url)
        .ok()
        .and_then(|parsed| parsed.host_str().map(str::to_string))
        .unwrap_or_else(|| "configured extension source".to_string())
}

impl ExtensionDownloadFailure {
    pub(super) fn is_retryable(&self) -> bool {
        match self {
            Self::Request { source, .. } => !source.is_builder() && !source.is_redirect(),
            Self::Status { status, .. } => {
                *status == reqwest::StatusCode::REQUEST_TIMEOUT
                    || *status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || status.is_server_error()
            }
            Self::Body { .. } => true,
            Self::Budget { .. } | Self::Empty { .. } => false,
        }
    }
}

pub(super) fn extension_source_client(
    connect_timeout: Duration,
    request_timeout: Duration,
) -> Client {
    Client::builder()
        .connect_timeout(connect_timeout)
        .timeout(request_timeout)
        .build()
        .expect("extension source HTTP client configuration must be valid")
}

#[derive(Debug, Deserialize)]
pub(super) struct DownloadLocatorDocument {
    pub(super) kind: String,
    pub(super) locator: Option<String>,
    #[serde(default)]
    pub(super) artifacts: Vec<PlatformArtifactDocument>,
}

#[derive(Debug, Clone, Deserialize)]
pub(super) struct PlatformArtifactDocument {
    pub(super) os: String,
    pub(super) arch: String,
    pub(super) libc: Option<String>,
    pub(super) rust_target: Option<String>,
    pub(super) locator: String,
    pub(super) checksum: Option<String>,
    pub(super) signature: Option<Value>,
}

pub(super) fn resolve_artifact_descriptor(
    entry: &OfficialExtensionCatalogEntry,
    github_proxy_url: Option<&str>,
) -> Result<OfficialExtensionArtifactDescriptor> {
    let locator: DownloadLocatorDocument =
        serde_json::from_value(entry.download_locator.clone())
            .context("failed to decode official extension download locator")?;
    if entry.source.client_collector()?.is_some() && locator.kind != "release_asset" {
        bail!("client collector requires a host-independent release_asset");
    }
    match locator.kind.as_str() {
        "repository_file" | "release_asset" | "https" => {
            let url = locator.locator.ok_or_else(|| {
                anyhow!("official extension download locator has no artifact URL")
            })?;
            Ok(OfficialExtensionArtifactDescriptor {
                locator_kind: locator.kind,
                locator: rewrite_github_download_url(&url, github_proxy_url),
                expected_checksum: entry.checksum.clone(),
                signature: entry.signature.clone(),
                platform: None,
            })
        }
        "platform_release_assets" => {
            let selected = select_current_platform_artifact(&locator.artifacts)
                .ok_or_else(|| anyhow!("official extension has no artifact for this platform"))?;
            Ok(OfficialExtensionArtifactDescriptor {
                locator_kind: locator.kind,
                locator: rewrite_github_download_url(&selected.locator, github_proxy_url),
                expected_checksum: selected.checksum.clone().or_else(|| entry.checksum.clone()),
                signature: selected
                    .signature
                    .clone()
                    .or_else(|| entry.signature.clone()),
                platform: Some(OfficialExtensionArtifactPlatform {
                    os: selected.os.clone(),
                    arch: selected.arch.clone(),
                    libc: selected.libc.clone(),
                    rust_target: selected
                        .rust_target
                        .clone()
                        .unwrap_or_else(|| rust_target_for_platform(selected)),
                }),
            })
        }
        _ => bail!("unsupported official extension download locator kind"),
    }
}

pub(super) fn rust_target_for_platform(artifact: &PlatformArtifactDocument) -> String {
    let architecture = match artifact.arch.as_str() {
        "amd64" => "x86_64",
        "arm64" => "aarch64",
        value => value,
    };
    match (artifact.os.as_str(), artifact.libc.as_deref()) {
        ("linux", Some("musl")) => format!("{architecture}-unknown-linux-musl"),
        ("linux", _) => format!("{architecture}-unknown-linux-gnu"),
        ("windows", _) => format!("{architecture}-pc-windows-msvc"),
        ("macos", _) => format!("{architecture}-apple-darwin"),
        (os, _) => format!("{architecture}-unknown-{os}"),
    }
}

pub(super) fn select_current_platform_artifact(
    artifacts: &[PlatformArtifactDocument],
) -> Option<&PlatformArtifactDocument> {
    let host_os = std::env::consts::OS;
    let host_arch = match std::env::consts::ARCH {
        "x86_64" => "amd64",
        "aarch64" => "arm64",
        value => value,
    };
    let host_libc = if host_os == "linux" {
        if cfg!(target_env = "musl") {
            Some("musl")
        } else {
            Some("gnu")
        }
    } else if host_os == "windows" {
        Some("msvc")
    } else {
        None
    };
    artifacts
        .iter()
        .filter(|artifact| artifact.os == host_os && artifact.arch == host_arch)
        .max_by_key(|artifact| match (host_libc, artifact.libc.as_deref()) {
            (Some(left), Some(right)) if left == right => 3_u8,
            (Some("gnu"), Some("musl")) if host_os == "linux" => 2,
            (_, None) => 1,
            _ => 0,
        })
}

pub(crate) fn rewrite_github_raw_url(url: &str, github_proxy_url: Option<&str>) -> String {
    let Some(github_proxy_url) = github_proxy_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return url.to_string();
    };
    let github_proxy_url = github_proxy_url.trim_end_matches('/');
    let proxied_raw_prefix = format!("{github_proxy_url}/{GITHUB_RAW_CONTENT_BASE_URL}");
    if url.starts_with(&proxied_raw_prefix) || !url.starts_with(GITHUB_RAW_CONTENT_BASE_URL) {
        return url.to_string();
    }

    format!("{github_proxy_url}/{url}")
}

pub(super) fn rewrite_github_download_url(url: &str, github_proxy_url: Option<&str>) -> String {
    let raw_rewritten = rewrite_github_raw_url(url, github_proxy_url);
    if raw_rewritten != url {
        return raw_rewritten;
    }
    let Some(proxy) = github_proxy_url
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return url.to_string();
    };
    let proxy = proxy.trim_end_matches('/');
    let proxied_prefix = format!("{proxy}/https://github.com/");
    if url.starts_with(&proxied_prefix) || !url.starts_with("https://github.com/") {
        return url.to_string();
    }
    format!("{proxy}/{url}")
}
