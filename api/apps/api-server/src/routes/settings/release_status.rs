use std::sync::OnceLock;

use anyhow::{anyhow, Context};
use reqwest::header::{ACCEPT, USER_AGENT};
use serde::{Deserialize, Serialize};
use time::{Duration, OffsetDateTime};
use tokio::sync::Mutex;
use utoipa::ToSchema;

const GITHUB_REPOSITORY: &str = "taichuy/1flowbase";
const GITHUB_LATEST_RELEASE_URL: &str =
    "https://api.github.com/repos/taichuy/1flowbase/releases/latest";
const GITHUB_CONTRIBUTORS_URL: &str = "https://github.com/taichuy/1flowbase/graphs/contributors";
const RELEASE_STATUS_CACHE_SECONDS: i64 = 60;
const DOCKER_SHELL_UPGRADE_COMMAND: &str = "curl -fsSL https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/shell/docker-deploy.sh | sh";
const DOCKER_POWERSHELL_UPGRADE_COMMAND: &str = "irm https://raw.githubusercontent.com/taichuy/1flowbase/main/scripts/powershell/docker-deploy.ps1 | iex";

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ConsoleReleaseInfoResponse {
    pub name: String,
    pub body: String,
    pub published_at: String,
    pub html_url: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ConsoleReleaseUpgradeCommandsResponse {
    pub shell: String,
    pub powershell: String,
}

#[derive(Clone, Debug, Serialize, ToSchema)]
pub struct ConsoleReleaseStatusResponse {
    pub current_version: String,
    pub latest_version: String,
    pub has_update: bool,
    pub release_info: Option<ConsoleReleaseInfoResponse>,
    pub contributors_url: String,
    pub upgrade_commands: ConsoleReleaseUpgradeCommandsResponse,
    pub cached: bool,
    pub warning: Option<String>,
}

#[derive(Clone)]
struct CachedReleaseStatus {
    release_status: ConsoleReleaseStatusResponse,
    stored_at: OffsetDateTime,
}

#[derive(Debug, Deserialize)]
struct GitHubLatestReleaseResponse {
    tag_name: String,
    name: Option<String>,
    body: Option<String>,
    published_at: String,
    html_url: String,
}

static RELEASE_STATUS_CACHE: OnceLock<Mutex<Option<CachedReleaseStatus>>> = OnceLock::new();

pub async fn fetch_console_release_status() -> ConsoleReleaseStatusResponse {
    if let Some(cached_release_status) = fresh_cached_release_status().await {
        return cached_release_status;
    }

    match fetch_github_latest_release().await {
        Ok(github_release) => {
            let release_status =
                release_status_from_github_release(current_service_version(), github_release);
            cache_release_status(release_status.clone()).await;
            release_status
        }
        Err(error) => stale_or_degraded_release_status(error.to_string()).await,
    }
}

fn current_service_version() -> &'static str {
    option_env!("FLOWBASE_API_SERVER_VERSION")
        .map(str::trim)
        .filter(|version| !version.is_empty())
        .unwrap_or(env!("CARGO_PKG_VERSION"))
}

async fn fresh_cached_release_status() -> Option<ConsoleReleaseStatusResponse> {
    let cache = release_status_cache();
    let guard = cache.lock().await;
    let cached_release_status = guard.as_ref()?;
    if OffsetDateTime::now_utc() - cached_release_status.stored_at
        > Duration::seconds(RELEASE_STATUS_CACHE_SECONDS)
    {
        return None;
    }

    let mut release_status = cached_release_status.release_status.clone();
    release_status.cached = true;
    Some(release_status)
}

async fn stale_or_degraded_release_status(warning: String) -> ConsoleReleaseStatusResponse {
    let cache = release_status_cache();
    let guard = cache.lock().await;

    if let Some(cached_release_status) = guard.as_ref() {
        let mut release_status = cached_release_status.release_status.clone();
        release_status.cached = true;
        release_status.warning = Some(warning);
        return release_status;
    }

    degraded_release_status(current_service_version(), &warning)
}

async fn cache_release_status(release_status: ConsoleReleaseStatusResponse) {
    let cache = release_status_cache();
    let mut guard = cache.lock().await;
    *guard = Some(CachedReleaseStatus {
        release_status,
        stored_at: OffsetDateTime::now_utc(),
    });
}

fn release_status_cache() -> &'static Mutex<Option<CachedReleaseStatus>> {
    RELEASE_STATUS_CACHE.get_or_init(|| Mutex::new(None))
}

async fn fetch_github_latest_release() -> anyhow::Result<GitHubLatestReleaseResponse> {
    let response = reqwest::Client::new()
        .get(GITHUB_LATEST_RELEASE_URL)
        .header(ACCEPT, "application/vnd.github+json")
        .header(USER_AGENT, "1flowbase-release-status")
        .send()
        .await
        .with_context(|| {
            format!("failed to request GitHub latest release for {GITHUB_REPOSITORY}")
        })?;
    let status = response.status();
    if !status.is_success() {
        return Err(anyhow!("GitHub API returned {}", status.as_u16()));
    }

    response
        .json::<GitHubLatestReleaseResponse>()
        .await
        .context("failed to parse GitHub latest release")
}

fn release_status_from_github_release(
    current_version: &str,
    github_release: GitHubLatestReleaseResponse,
) -> ConsoleReleaseStatusResponse {
    let latest_version = github_release.tag_name.trim_start_matches('v').to_string();
    ConsoleReleaseStatusResponse {
        current_version: current_version.to_string(),
        latest_version: latest_version.clone(),
        has_update: release_version_is_newer(current_version, &latest_version),
        release_info: Some(ConsoleReleaseInfoResponse {
            name: github_release
                .name
                .unwrap_or_else(|| github_release.tag_name.clone()),
            body: github_release.body.unwrap_or_default(),
            published_at: github_release.published_at,
            html_url: github_release.html_url,
        }),
        contributors_url: GITHUB_CONTRIBUTORS_URL.to_string(),
        upgrade_commands: release_upgrade_commands(),
        cached: false,
        warning: None,
    }
}

fn degraded_release_status(current_version: &str, warning: &str) -> ConsoleReleaseStatusResponse {
    ConsoleReleaseStatusResponse {
        current_version: current_version.to_string(),
        latest_version: current_version.to_string(),
        has_update: false,
        release_info: None,
        contributors_url: GITHUB_CONTRIBUTORS_URL.to_string(),
        upgrade_commands: release_upgrade_commands(),
        cached: false,
        warning: Some(warning.to_string()),
    }
}

fn release_upgrade_commands() -> ConsoleReleaseUpgradeCommandsResponse {
    ConsoleReleaseUpgradeCommandsResponse {
        shell: DOCKER_SHELL_UPGRADE_COMMAND.to_string(),
        powershell: DOCKER_POWERSHELL_UPGRADE_COMMAND.to_string(),
    }
}

fn release_version_is_newer(current_version: &str, latest_version: &str) -> bool {
    version_parts(latest_version) > version_parts(current_version)
}

fn version_parts(version: &str) -> [u64; 3] {
    let normalized_version = version.trim().trim_start_matches('v');
    let mut parts = [0_u64; 3];
    for (index, segment) in normalized_version.split('.').take(3).enumerate() {
        let numeric_prefix: String = segment
            .chars()
            .take_while(|character| character.is_ascii_digit())
            .collect();
        if let Ok(value) = numeric_prefix.parse::<u64>() {
            parts[index] = value;
        }
    }
    parts
}

#[cfg(test)]
mod tests {
    use super::*;

    fn github_release_fixture(tag_name: &str) -> GitHubLatestReleaseResponse {
        GitHubLatestReleaseResponse {
            tag_name: tag_name.to_string(),
            name: Some(tag_name.to_string()),
            body: Some("Release notes".to_string()),
            published_at: "2026-06-05T00:00:00Z".to_string(),
            html_url: format!("https://github.com/taichuy/1flowbase/releases/tag/{tag_name}"),
        }
    }

    #[test]
    fn release_status_marks_newer_latest_tag_as_update() {
        let release_status =
            release_status_from_github_release("0.1.5", github_release_fixture("v0.1.6"));

        assert_eq!(release_status.current_version, "0.1.5");
        assert_eq!(release_status.latest_version, "0.1.6");
        assert!(release_status.has_update);
        assert_eq!(
            release_status
                .release_info
                .as_ref()
                .map(|release| release.html_url.as_str()),
            Some("https://github.com/taichuy/1flowbase/releases/tag/v0.1.6")
        );
        assert_eq!(
            release_status.contributors_url,
            "https://github.com/taichuy/1flowbase/graphs/contributors"
        );
        assert!(release_status
            .upgrade_commands
            .shell
            .contains("scripts/shell/docker-deploy.sh"));
    }

    #[test]
    fn release_status_falls_back_to_current_version_when_github_fails() {
        let release_status = degraded_release_status("0.1.5", "GitHub API returned 500");

        assert_eq!(release_status.current_version, "0.1.5");
        assert_eq!(release_status.latest_version, "0.1.5");
        assert!(!release_status.has_update);
        assert!(release_status.release_info.is_none());
        assert_eq!(
            release_status.warning.as_deref(),
            Some("GitHub API returned 500")
        );
    }

    #[test]
    fn release_version_compare_handles_v_prefix() {
        assert!(release_version_is_newer("0.1.5", "v0.1.6"));
        assert!(!release_version_is_newer("0.1.6", "v0.1.6"));
        assert!(!release_version_is_newer("0.2.0", "v0.1.6"));
    }

    #[test]
    fn release_status_cache_is_short_enough_for_manual_release_checks() {
        assert_eq!(RELEASE_STATUS_CACHE_SECONDS, 60);
    }
}
