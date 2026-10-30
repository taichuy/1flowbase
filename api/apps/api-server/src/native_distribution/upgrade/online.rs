use super::{archive, files, fresh_workspace, manifest::Manifest};
use anyhow::{ensure, Context, Result};
use serde::Deserialize;
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
};

pub const RELEASES: &str = "https://api.github.com/repos/taichuy/1flowbase/releases?per_page=100";
const MAX_ARCHIVE: u64 = 1024 * 1024 * 1024;
#[derive(Deserialize)]
struct Release {
    tag_name: String,
    draft: bool,
    prerelease: bool,
    assets: Vec<Asset>,
}
#[derive(Deserialize)]
struct Asset {
    name: String,
    browser_download_url: String,
    size: u64,
    digest: Option<String>,
}
struct Candidate {
    version: semver::Version,
    asset: Asset,
}
fn select(releases: Vec<Release>, old: &Manifest) -> Result<Option<Candidate>> {
    let current = semver::Version::parse(&old.version)?;
    let name = format!(
        "1flowbase-{}-{}.{}",
        old.platform,
        old.arch,
        if old.platform == "windows" {
            "zip"
        } else {
            "tar.gz"
        }
    );
    let mut candidates = Vec::new();
    for release in releases {
        if release.draft || (current.pre.is_empty() && release.prerelease) {
            continue;
        }
        let Ok(version) = semver::Version::parse(release.tag_name.trim_start_matches('v')) else {
            continue;
        };
        if version <= current || (current.pre.is_empty() && !version.pre.is_empty()) {
            continue;
        }
        if let Some(asset) = release.assets.into_iter().find(|asset| asset.name == name) {
            candidates.push(Candidate { version, asset });
        }
    }
    Ok(candidates
        .into_iter()
        .max_by(|a, b| a.version.cmp(&b.version)))
}
async fn metadata(client: &reqwest::Client, url: &str) -> Result<Vec<Release>> {
    let mut response = client.get(url).send().await?.error_for_status()?;
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await? {
        ensure!(
            bytes.len() + chunk.len() <= 8 * 1024 * 1024,
            "Release metadata is too large"
        );
        bytes.extend_from_slice(&chunk);
    }
    Ok(serde_json::from_slice(&bytes)?)
}
fn valid_download_url(url: &str, feed: &str) -> bool {
    if url.starts_with("https://github.com/taichuy/1flowbase/releases/download/") {
        return true;
    }
    // Only compiled unit tests can provide a local HTTP fixture. Production has no feed override.
    #[cfg(test)]
    if let (Ok(url), Ok(feed)) = (url::Url::parse(url), url::Url::parse(feed)) {
        return feed.scheme() == "http"
            && feed.host_str() == Some("127.0.0.1")
            && url.origin() == feed.origin();
    }
    let _ = feed;
    false
}
pub(super) async fn download(
    client: &reqwest::Client,
    old: &Manifest,
    work: &Path,
    feed: &str,
) -> Result<Option<PathBuf>> {
    let Some(candidate) = select(metadata(client, feed).await?, old)? else {
        return Ok(None);
    };
    let asset = candidate.asset;
    ensure!(
        valid_download_url(&asset.browser_download_url, feed),
        "Release asset is not hosted by the official repository"
    );
    ensure!(
        asset.size > 0 && asset.size <= MAX_ARCHIVE,
        "Invalid update archive size"
    );
    let digest = asset
        .digest
        .as_deref()
        .and_then(|v| v.strip_prefix("sha256:"))
        .context("Release asset has no SHA256 digest")?;
    ensure!(
        digest.len() == 64 && digest.bytes().all(|b| b.is_ascii_hexdigit()),
        "Invalid release SHA256 digest"
    );
    files::require_space(
        work.parent().context("Invalid update directory")?,
        asset.size.saturating_mul(4),
    )?;
    fresh_workspace(work)?;
    let archive = work.join(&asset.name);
    println!(
        "Downloading {} / 正在下载 {}",
        candidate.version, asset.name
    );
    let mut response = client
        .get(&asset.browser_download_url)
        .send()
        .await?
        .error_for_status()?;
    let mut file = fs::OpenOptions::new()
        .create_new(true)
        .write(true)
        .open(&archive)?;
    let mut received = 0_u64;
    while let Some(chunk) = response.chunk().await? {
        received += chunk.len() as u64;
        ensure!(
            received <= asset.size,
            "Update download exceeds declared size"
        );
        file.write_all(&chunk)?;
    }
    file.sync_all()?;
    drop(file);
    ensure!(
        received == asset.size && files::hash(&archive)?.eq_ignore_ascii_case(digest),
        "Update download checksum/size mismatch; installation unchanged"
    );
    let package_name = format!("1flowbase-{}-{}", old.platform, old.arch);
    let extracted = archive::extract(
        &archive,
        &work.join("unpacked"),
        &package_name,
        old.platform == "windows",
    )?;
    let new = Manifest::read(&extracted)?;
    ensure!(
        semver::Version::parse(&new.version)? == candidate.version,
        "Release tag and package version differ"
    );
    new.compatible_with(old)?;
    let package = work.join("package");
    fs::rename(extracted, &package)?;
    fs::remove_file(archive)?;
    files::remove(&work.join("unpacked"))?;
    Ok(Some(package))
}
#[cfg(test)]
#[path = "_tests/online.rs"]
mod tests;
