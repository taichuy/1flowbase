use super::*;
use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

static NEXT: AtomicUsize = AtomicUsize::new(0);
struct Fixture {
    base: PathBuf,
    root: PathBuf,
    descriptor: PackagedScript,
}
impl Fixture {
    fn new() -> Self {
        let base = std::env::temp_dir().join(format!(
            "sync-js-runtime-{}-{}",
            std::process::id(),
            NEXT.fetch_add(1, Ordering::Relaxed)
        ));
        let root = base.join("1.2.3");
        fs::create_dir_all(root.join("bin")).unwrap();
        fs::create_dir(root.join("backend")).unwrap();
        let source = b"function main(){return {ok:true}}";
        fs::write(root.join("backend/main.js"), source).unwrap();
        let descriptor = PackagedScript {
            version: "1.2.3".into(),
            path: "backend/main.js".into(),
            bytes: source.len() as u64,
            sha256: format!("{:x}", base16ct::HexDisplay(&Sha256::digest(source))),
        };
        Self {
            base,
            root,
            descriptor,
        }
    }
    fn load(&self) -> Result<String, ExecutionError> {
        load_from_executable(&self.root.join("bin/worker"), "1.2.3", &self.descriptor)
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.base);
    }
}

#[test]
fn packaged_relative_loader_survives_relocation_without_cwd() {
    let mut f = Fixture::new();
    let first = f.load().unwrap();
    let moved = f.base.with_extension("relocated");
    fs::rename(&f.base, &moved).unwrap();
    f.base = moved;
    f.root = f.base.join("1.2.3");
    assert_eq!(f.load().unwrap(), first);
}
#[test]
fn version_hash_missing_and_malformed_descriptors_are_explicit() {
    let mut f = Fixture::new();
    f.descriptor.version = "1.2.4".into();
    assert_eq!(f.load().unwrap_err().code(), "artifact_version_mismatch");
    f.descriptor.version = "1.2.3".into();
    fs::write(
        f.root.join("backend/main.js"),
        "x".repeat(f.descriptor.bytes as usize),
    )
    .unwrap();
    assert_eq!(f.load().unwrap_err().code(), "artifact_integrity_mismatch");
    f.descriptor.sha256 = "z".repeat(64);
    assert_eq!(f.load().unwrap_err().code(), "artifact_descriptor_invalid");
    f.descriptor.sha256 = "0".repeat(64);
    fs::remove_file(f.root.join("backend/main.js")).unwrap();
    assert_eq!(f.load().unwrap_err().code(), "artifact_missing");
}
#[test]
fn path_escape_non_regular_and_wrong_size_artifacts_are_rejected() {
    let mut f = Fixture::new();
    for selector in [
        "../main.js",
        "/tmp/main.js",
        "backend/../main.js",
        "backend\\main.js",
        "C:/main.js",
        "backend//main.js",
        "./backend/main.js",
    ] {
        f.descriptor.path = selector.into();
        assert_eq!(
            f.load().unwrap_err().code(),
            "artifact_path_escape",
            "{selector}"
        );
    }
    f.descriptor.path = "backend".into();
    assert_eq!(f.load().unwrap_err().code(), "artifact_invalid");
    f.descriptor.path = "backend/large.js".into();
    File::create(f.root.join(&f.descriptor.path))
        .unwrap()
        .set_len(f.descriptor.bytes + 1)
        .unwrap();
    assert_eq!(f.load().unwrap_err().code(), "artifact_size_mismatch");
}
#[cfg(unix)]
#[test]
fn symlink_escape_is_rejected_even_when_hash_matches() {
    let f = Fixture::new();
    let external = f.base.join("outside.js");
    fs::rename(f.root.join("backend/main.js"), &external).unwrap();
    std::os::unix::fs::symlink(external, f.root.join("backend/main.js")).unwrap();
    assert_eq!(f.load().unwrap_err().code(), "artifact_path_escape");
}
#[cfg(unix)]
#[test]
fn version_alias_cannot_load_another_installed_version() {
    let f = Fixture::new();
    let other = f.base.join("1.2.4");
    fs::rename(&f.root, &other).unwrap();
    std::os::unix::fs::symlink(other, &f.root).unwrap();
    assert_eq!(f.load().unwrap_err().code(), "artifact_version_mismatch");
}

#[test]
fn descriptor_size_mismatch_and_read_bound_overflow_are_explicit() {
    let mut f = Fixture::new();
    f.descriptor.bytes += 1;
    assert_eq!(f.load().unwrap_err().code(), "artifact_size_mismatch");
    f.descriptor.bytes = u64::MAX;
    assert_eq!(f.load().unwrap_err().code(), "artifact_descriptor_invalid");
}
