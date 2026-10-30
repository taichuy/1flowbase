use super::files;
use anyhow::{ensure, Context, Result};
use serde::{Deserialize, Serialize};
use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Component, Path},
};

pub const NAME: &str = "native-distribution.json";
const ROOTS: &[&str] = &[
    "1flowbase",
    "1flowbase.exe",
    "system_recovery",
    "system_recovery.exe",
    "api",
    "web",
    "postgres",
    "README.md",
    "README_CN.md",
    "LICENSE",
    "start.sh",
    "upgrade.sh",
    "upgrade.cmd",
    "upgrade.ps1",
    ".env.example",
];

#[derive(Clone, Serialize, Deserialize)]
pub struct Manifest {
    pub format_version: u32,
    pub config_schema: u32,
    pub version: String,
    pub build_id: String,
    pub platform: String,
    pub arch: String,
    pub postgres_major: u32,
    pub files: BTreeMap<String, String>,
}
impl Manifest {
    pub fn read(root: &Path) -> Result<Self> {
        let manifest: Self = serde_json::from_slice(
            &fs::read(root.join(NAME))
                .context("Not a supported native installation: manifest missing")?,
        )?;
        manifest.validate()?;
        Ok(manifest)
    }
    pub fn validate(&self) -> Result<()> {
        let manifest = self;
        ensure!(manifest.format_version == 1 && manifest.config_schema == 1, "Unsupported native package/config format; early experimental packages are not an upgrade baseline");
        semver::Version::parse(&manifest.version).context("Invalid native package version")?;
        ensure!(
            !manifest.build_id.is_empty() && !manifest.files.is_empty(),
            "Incomplete native manifest"
        );
        for (name, hash) in &manifest.files {
            ensure!(
                !name.contains(['\\', ':'])
                    && name.split('/').all(|p| !p.is_empty()
                        && p != "."
                        && p != ".."
                        && !p.starts_with(".update-")),
                "Unsafe manifest path: {name}"
            );
            ensure!(
                Path::new(name)
                    .components()
                    .all(|p| matches!(p, Component::Normal(_)))
                    && ROOTS
                        .contains(&name.split_once('/').map_or(name.as_str(), |(root, _)| root)),
                "Unmanaged manifest path: {name}"
            );
            ensure!(
                hash.len() == 64 && hash.bytes().all(|b| b.is_ascii_hexdigit()),
                "Invalid checksum for {name}"
            );
        }
        for required in [Self::executable(), ".env.example", "web/index.html"] {
            ensure!(
                manifest.files.contains_key(required),
                "Manifest missing {required}"
            );
        }
        Ok(())
    }
    pub fn allowed_root(name: &str) -> bool {
        name == NAME || ROOTS.contains(&name)
    }
    pub fn executable() -> &'static str {
        if cfg!(windows) {
            "1flowbase.exe"
        } else {
            "1flowbase"
        }
    }
    pub fn roots(&self) -> BTreeSet<String> {
        self.files
            .keys()
            .map(|name| {
                name.split_once('/')
                    .map_or(name.as_str(), |(root, _)| root)
                    .to_owned()
            })
            .chain([NAME.to_owned()])
            .collect()
    }
    pub fn verify(&self, root: &Path) -> Result<()> {
        for (name, expected) in &self.files {
            let file = root.join(name);
            ensure!(
                fs::symlink_metadata(&file)?.is_file(),
                "Not a regular package file: {name}"
            );
            // Checking ancestors also prevents an apparently regular file under a symlink.
            let mut ancestor = file.parent().context("Invalid package file path")?;
            while ancestor != root {
                ensure!(
                    !fs::symlink_metadata(ancestor)?.file_type().is_symlink(),
                    "Symlink in package: {name}"
                );
                ancestor = ancestor.parent().context("Invalid package path")?;
            }
            ensure!(
                &files::hash(&file)? == expected,
                "Package checksum mismatch: {name}"
            );
        }
        fn check(root: &Path, path: &Path, expected: &BTreeMap<String, String>) -> Result<()> {
            let info = fs::symlink_metadata(path)?;
            ensure!(
                !info.file_type().is_symlink(),
                "Symlink in managed package tree"
            );
            if info.is_dir() {
                for entry in fs::read_dir(path)? {
                    check(root, &entry?.path(), expected)?;
                }
            } else {
                let name = path
                    .strip_prefix(root)?
                    .to_str()
                    .context("Non-UTF8 package path")?
                    .replace('\\', "/");
                ensure!(
                    name == NAME || expected.contains_key(&name),
                    "Unexpected managed file: {name}"
                );
            }
            Ok(())
        }
        for name in self.roots() {
            check(root, &root.join(name), &self.files)?;
        }
        Ok(())
    }
    pub fn compatible_with(&self, old: &Self) -> Result<()> {
        let platform = if cfg!(windows) {
            "windows"
        } else if cfg!(target_os = "macos") {
            "macos"
        } else {
            "linux"
        };
        let arch = match std::env::consts::ARCH {
            "x86_64" => "x64",
            "aarch64" => "arm64",
            other => other,
        };
        ensure!(
            self.platform == old.platform
                && self.arch == old.arch
                && self.platform == platform
                && self.arch == arch,
            "Package system/architecture does not match this installation"
        );
        ensure!(
            self.postgres_major == old.postgres_major && self.postgres_major == 18,
            "PostgreSQL major upgrades require a separate migration"
        );
        ensure!(
            semver::Version::parse(&self.version)? >= semver::Version::parse(&old.version)?,
            "Program downgrades are not supported"
        );
        ensure!(
            self.build_id != old.build_id,
            "This build is already installed"
        );
        Ok(())
    }
}
