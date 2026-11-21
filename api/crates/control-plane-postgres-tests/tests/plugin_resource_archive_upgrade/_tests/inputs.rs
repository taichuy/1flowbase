use anyhow::{Context, Result, ensure};
use std::{
    fs,
    io::Write,
    os::unix::fs::PermissionsExt,
    path::{Path, PathBuf},
    process::{Command, Stdio},
};

pub const OWNER: &str = "taichuy/ssh";
pub const OLD_VERSION: &str = "0.1.21";
pub const NEW_VERSION: &str = "0.1.25";
pub const OLD_SHA256: &str = "7f91e97b29ccdff563ead49d46f41864061424bf43e164785047868840b9ebc2";
const BUSINESS_COMMIT: &str = "8e0ada94257c256809d47bf2684130f2c5cc7c51";

pub struct Archive {
    pub bytes: Vec<u8>,
    pub filename: String,
    pub sha256: String,
}

pub struct Inputs {
    pub old: Archive,
    pub new: Archive,
    pub source_manifest: Vec<u8>,
    pub runtime_sha256: String,
}

fn env(name: &str) -> Result<String> {
    std::env::var(name).with_context(|| format!("required fixture input {name} is missing"))
}

fn absolute_path(name: &str) -> Result<PathBuf> {
    let path = PathBuf::from(env(name)?);
    ensure!(path.is_absolute(), "{name} must be an absolute local path");
    path.canonicalize()
        .with_context(|| format!("cannot resolve {name}"))
}

fn expected_hash(name: &str) -> Result<String> {
    let hash = env(name)?;
    ensure!(
        hash.len() == 64
            && hash
                .bytes()
                .all(|c| c.is_ascii_digit() || (b'a'..=b'f').contains(&c)),
        "{name} must contain exactly 64 lowercase SHA-256 hex digits"
    );
    Ok(hash)
}

/// Use the installed coreutils tool rather than add a test-only Cargo dependency/edge.
/// Both the archive and the separately built binary are hashed from the bytes actually read.
pub fn sha256(bytes: &[u8]) -> Result<String> {
    let mut child = Command::new("sha256sum")
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::null())
        .spawn()
        .context("sha256sum is required for archive/binary provenance checks")?;
    child
        .stdin
        .take()
        .context("sha256sum stdin missing")?
        .write_all(bytes)?;
    let output = child.wait_with_output()?;
    ensure!(output.status.success(), "sha256sum failed");
    let text = String::from_utf8(output.stdout)?;
    let hash = text
        .split_whitespace()
        .next()
        .context("sha256sum returned no hash")?;
    ensure!(hash.len() == 64, "sha256sum returned an invalid hash");
    Ok(hash.into())
}

fn archive(path: &Path, expected: String) -> Result<Archive> {
    ensure!(path.is_file(), "archive input must be a regular local file");
    let bytes = fs::read(path)?;
    ensure!(
        sha256(&bytes)? == expected,
        "archive SHA-256 differs from the explicit expected input"
    );
    let filename = path
        .file_name()
        .context("archive filename missing")?
        .to_str()
        .context("archive filename must be UTF-8")?
        .to_owned();
    ensure!(
        filename.ends_with(".1flowbasepkg"),
        "expected an actual .1flowbasepkg archive"
    );
    Ok(Archive {
        bytes,
        filename,
        sha256: expected,
    })
}

// Test-only descendant commits are permitted without relabelling the binary BUILD receipt.
// Both plugins' production build inputs must still be byte-identical to that receipt's commit.
fn verify_business_build_source(business: &Path) -> Result<()> {
    ensure!(
        Command::new("git")
            .arg("-C")
            .arg(business)
            .args(["merge-base", "--is-ancestor", BUSINESS_COMMIT, "HEAD"])
            .status()?
            .success(),
        "BUSINESS candidate is not descended from the approved BUILD commit"
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
            .arg(business)
            .args(["diff", "--quiet", BUSINESS_COMMIT, "--"])
            .args(&paths)
            .status()?
            .success(),
        "BUSINESS production inputs differ from the approved BUILD commit"
    );
    let untracked = Command::new("git")
        .arg("-C")
        .arg(business)
        .args(["ls-files", "--others", "--exclude-standard", "--"])
        .args(&paths)
        .output()?;
    ensure!(
        untracked.status.success() && untracked.stdout.is_empty(),
        "untracked BUSINESS production inputs are absent from the BUILD receipt"
    );
    Ok(())
}

impl Inputs {
    pub fn load() -> Result<Self> {
        ensure!(
            std::env::var_os("PLUGIN_SHARED_RESOURCE_CONTRACTS_JSON").is_none(),
            "run with the retired shared-resource environment variable unset"
        );
        // Production intake also uses temp_dir(). Its extraction and this fixture's files
        // must stay below an explicit, owner-private workspace, never a tracked public tree.
        let temp = absolute_path("TMPDIR")?;
        ensure!(temp.is_dir(), "TMPDIR must be a directory");
        ensure!(
            fs::metadata(&temp)?.permissions().mode() & 0o077 == 0,
            "TMPDIR must be owner-private (0700)"
        );
        let host = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../..")
            .canonicalize()?;
        ensure!(
            !temp.starts_with(&host),
            "TMPDIR must be outside the HOST checkout"
        );

        let business = absolute_path("PLUGIN_RESOURCE_BUSINESS_ROOT")?;
        ensure!(
            !temp.starts_with(&business),
            "TMPDIR must be outside the BUSINESS checkout"
        );
        verify_business_build_source(&business)?;
        let source_manifest =
            fs::read(business.join("runtime-extensions/@taichuy/ssh/manifest.yaml"))?;
        let runtime = absolute_path("PLUGIN_RESOURCE_FRESH_RUNTIME")?;
        let runtime_bytes = fs::read(runtime)?;
        ensure!(
            runtime_bytes.starts_with(b"\x7fELF"),
            "fresh runtime input must be the compiled Linux executable, not a script"
        );
        let runtime_sha256 = expected_hash("PLUGIN_RESOURCE_FRESH_RUNTIME_SHA256")?;
        ensure!(
            sha256(&runtime_bytes)? == runtime_sha256,
            "fresh runtime SHA-256 mismatch"
        );
        let old = archive(
            &absolute_path("PLUGIN_RESOURCE_OLD_PACKAGE")?,
            OLD_SHA256.into(),
        )?;
        let new = archive(
            &absolute_path("PLUGIN_RESOURCE_NEW_PACKAGE")?,
            expected_hash("PLUGIN_RESOURCE_NEW_PACKAGE_SHA256")?,
        )?;
        ensure!(
            old.sha256 != new.sha256,
            "new archive must not be the old archive"
        );
        Ok(Self {
            old,
            new,
            source_manifest,
            runtime_sha256,
        })
    }
}

/// Required only by the separate ignored consumer-negative test; absence is an error,
/// never a silently skipped branch of the provider-upgrade test.
pub struct ConsumerInputs {
    pub archive: Archive,
    pub source_manifest: Vec<u8>,
    pub runtime_sha256: String,
}
impl ConsumerInputs {
    pub fn load() -> Result<Self> {
        let business = absolute_path("PLUGIN_RESOURCE_BUSINESS_ROOT")?;
        verify_business_build_source(&business)?;
        let source_manifest =
            fs::read(business.join("runtime-extensions/@IQLater/compute-monitor/manifest.yaml"))?;
        let runtime_bytes = fs::read(absolute_path("PLUGIN_RESOURCE_FRESH_MONITOR_RUNTIME")?)?;
        ensure!(
            runtime_bytes.starts_with(b"\x7fELF"),
            "monitor input must be a compiled Linux executable"
        );
        let runtime_sha256 = expected_hash("PLUGIN_RESOURCE_FRESH_MONITOR_RUNTIME_SHA256")?;
        ensure!(
            sha256(&runtime_bytes)? == runtime_sha256,
            "fresh monitor runtime SHA-256 mismatch"
        );
        let archive = archive(
            &absolute_path("PLUGIN_RESOURCE_CONSUMER_PACKAGE")?,
            expected_hash("PLUGIN_RESOURCE_CONSUMER_PACKAGE_SHA256")?,
        )?;
        Ok(Self {
            archive,
            source_manifest,
            runtime_sha256,
        })
    }
}
