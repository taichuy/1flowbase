use anyhow::{ensure, Context, Result};
use sha2::{Digest, Sha256};
use std::{fs, os::unix::fs::PermissionsExt, path::PathBuf, process::Command, str::FromStr};

const OLD_SHA: &str = "7f91e97b29ccdff563ead49d46f41864061424bf43e164785047868840b9ebc2";
const BUSINESS_BUILD_COMMIT: &str = "8e0ada94257c256809d47bf2684130f2c5cc7c51";
pub struct Archive {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub sha: String,
    pub version: &'static str,
    pub publisher: &'static str,
    pub code: &'static str,
    pub source: Option<Vec<u8>>,
    pub runtime_sha: Option<String>,
}
pub struct Inputs {
    pub old: Archive,
    pub new: Archive,
    pub monitor: Archive,
}
pub fn sha(bytes: &[u8]) -> String {
    Sha256::digest(bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn required(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("required private local input {name} missing"))
}
fn path(name: &str) -> Result<PathBuf> {
    let value = PathBuf::from(required(name)?);
    ensure!(value.is_absolute(), "{name} must be absolute");
    value
        .canonicalize()
        .with_context(|| format!("cannot resolve {name}"))
}
fn hash(name: &str) -> Result<String> {
    let value = required(name)?;
    ensure!(
        value.len() == 64
            && value
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)),
        "{name} must be bare lowercase SHA256"
    );
    Ok(value)
}
fn archive(
    variable: &str,
    expected: String,
    version: &'static str,
    publisher: &'static str,
    code: &'static str,
) -> Result<Archive> {
    let file = path(variable)?;
    ensure!(file.is_file(), "{variable} must be a regular file");
    let bytes = fs::read(&file)?;
    ensure!(sha(&bytes) == expected, "{variable} checksum mismatch");
    let filename = file
        .file_name()
        .context("archive filename")?
        .to_str()
        .context("UTF8 filename")?
        .to_owned();
    ensure!(
        filename.ends_with(".1flowbasepkg"),
        "actual plugin archive required"
    );
    Ok(Archive {
        bytes,
        filename,
        sha: expected,
        version,
        publisher,
        code,
        source: None,
        runtime_sha: None,
    })
}
impl Inputs {
    pub fn load() -> Result<Self> {
        ensure!(
            std::env::var_os("PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON").is_none(),
            "retired whitelist must be absent"
        );
        // Validate before the API helper can use its credential-bearing default URL.
        let url = required("API_DATABASE_URL")?;
        let options = sqlx::postgres::PgConnectOptions::from_str(&url)
            .map_err(|_| anyhow::anyhow!("invalid explicit local API_DATABASE_URL"))?;
        ensure!(
            matches!(options.get_host(), "localhost" | "127.0.0.1" | "::1"),
            "only loopback disposable-test PostgreSQL allowed"
        );
        let temp = path("TMPDIR")?;
        ensure!(
            temp.is_dir() && fs::metadata(&temp)?.permissions().mode() & 0o077 == 0,
            "TMPDIR must be an owner-private directory"
        );
        let host = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()?;
        let business = path("PLUGIN_RESOURCE_BUSINESS_ROOT")?;
        ensure!(
            !temp.starts_with(host) && !temp.starts_with(&business),
            "private TMPDIR must be outside HOST/BUSINESS"
        );
        // Keep the original BUILD receipt anchored to 8e0ada942. Test-only descendants
        // (including aa0a3ef) are not new build receipts; production inputs must match.
        ensure!(
            Command::new("git")
                .arg("-C")
                .arg(&business)
                .args(["merge-base", "--is-ancestor", BUSINESS_BUILD_COMMIT, "HEAD"])
                .status()?
                .success(),
            "BUSINESS is not descended from the approved BUILD commit"
        );
        let paths: Vec<String> = [
            "runtime-extensions/@taichuy/ssh",
            "runtime-extensions/@IQLater/compute-monitor",
        ]
        .into_iter()
        .flat_map(|plugin| {
            ["src", "Cargo.toml", "Cargo.lock", "manifest.yaml", "ui"]
                .map(|name| format!("{plugin}/{name}"))
        })
        .collect();
        ensure!(
            Command::new("git")
                .arg("-C")
                .arg(&business)
                .args(["diff", "--quiet", BUSINESS_BUILD_COMMIT, "--"])
                .args(&paths)
                .status()?
                .success(),
            "BUSINESS production inputs differ from the approved BUILD commit"
        );
        let untracked = Command::new("git")
            .arg("-C")
            .arg(&business)
            .args(["ls-files", "--others", "--exclude-standard", "--"])
            .args(&paths)
            .output()?;
        ensure!(
            untracked.status.success() && untracked.stdout.is_empty(),
            "untracked BUSINESS production inputs are absent from the BUILD receipt"
        );
        let old = archive(
            "PLUGIN_RESOURCE_OLD_PACKAGE",
            OLD_SHA.into(),
            "0.1.21",
            "taichuy",
            "ssh",
        )?;
        let mut new = archive(
            "PLUGIN_RESOURCE_NEW_PACKAGE",
            hash("PLUGIN_RESOURCE_NEW_PACKAGE_SHA256")?,
            "0.1.25",
            "taichuy",
            "ssh",
        )?;
        let mut monitor = archive(
            "PLUGIN_RESOURCE_CONSUMER_PACKAGE",
            hash("PLUGIN_RESOURCE_CONSUMER_PACKAGE_SHA256")?,
            "0.2.7",
            "iqlater",
            "compute-monitor",
        )?;
        for (item, relative, executable, checksum) in [
            (
                &mut new,
                "runtime-extensions/@taichuy/ssh",
                "PLUGIN_RESOURCE_FRESH_RUNTIME",
                "PLUGIN_RESOURCE_FRESH_RUNTIME_SHA256",
            ),
            (
                &mut monitor,
                "runtime-extensions/@IQLater/compute-monitor",
                "PLUGIN_RESOURCE_FRESH_MONITOR_RUNTIME",
                "PLUGIN_RESOURCE_FRESH_MONITOR_RUNTIME_SHA256",
            ),
        ] {
            ensure!(
                Command::new("git")
                    .arg("-C")
                    .arg(&business)
                    .args(["diff", "--quiet", "HEAD", "--", relative])
                    .status()?
                    .success(),
                "BUSINESS candidate has tracked changes"
            );
            let binary = fs::read(path(executable)?)?;
            let expected = hash(checksum)?;
            ensure!(
                binary.starts_with(b"\x7fELF") && sha(&binary) == expected,
                "fresh Linux executable/hash required for {executable}"
            );
            item.runtime_sha = Some(expected);
            item.source = Some(fs::read(business.join(relative).join("manifest.yaml"))?);
        }
        ensure!(
            old.sha != new.sha,
            "old archive cannot serve as fresh candidate"
        );
        Ok(Self { old, new, monitor })
    }
}
