use super::*;
use std::{
    fs,
    sync::atomic::{AtomicU64, Ordering},
};

struct Directory(PathBuf);
impl Directory {
    fn new() -> Self {
        static SEQUENCE: AtomicU64 = AtomicU64::new(0);
        let path = env::temp_dir().join(format!(
            "1flowbase-paths-{}-{}",
            std::process::id(),
            SEQUENCE.fetch_add(1, Ordering::Relaxed)
        ));
        fs::create_dir(&path).unwrap();
        Self(path)
    }
}
impl Drop for Directory {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.0);
    }
}

#[test]
fn default_is_beside_executable_and_does_not_create_files() {
    let directory = Directory::new();
    let package = directory.0.join("package with spaces");
    assert_eq!(
        resolve_data_root(&package, None, None).unwrap(),
        package.join("data")
    );
    assert!(!package.exists());
    assert_ne!(package, env::current_dir().unwrap());
}

#[test]
fn legacy_state_requires_explicit_migration_without_modifying_it() {
    for entry in ["settings.json", "postgres"] {
        let directory = Directory::new();
        let package = directory.0.join("package");
        let legacy = directory.0.join("legacy");
        fs::create_dir(&legacy).unwrap();
        fs::write(legacy.join(entry), b"preserve-me").unwrap();
        // An empty destination from a failed first launch must not bypass detection.
        fs::create_dir_all(package.join("data")).unwrap();
        let error = resolve_data_root(&package, None, Some(&legacy)).unwrap_err();
        assert_eq!(error.kind(), io::ErrorKind::AlreadyExists);
        assert!(error.to_string().contains(&legacy.display().to_string()));
        assert!(error
            .to_string()
            .contains(&package.join("data").display().to_string()));
        assert_eq!(fs::read(legacy.join(entry)).unwrap(), b"preserve-me");
        assert_eq!(fs::read_dir(package.join("data")).unwrap().count(), 0);
    }
}

#[test]
fn migrated_data_wins_over_retained_legacy_backup() {
    let directory = Directory::new();
    let package = directory.0.join("package");
    let data = package.join("data");
    let legacy = directory.0.join("legacy");
    for path in [&data, &legacy] {
        fs::create_dir_all(path).unwrap();
        fs::write(path.join("settings.json"), b"existing-secrets").unwrap();
    }
    assert_eq!(
        resolve_data_root(&package, None, Some(&legacy)).unwrap(),
        data
    );
}

#[test]
fn explicit_directory_keeps_existing_override_semantics() {
    let directory = Directory::new();
    let legacy = directory.0.join("legacy");
    fs::create_dir(&legacy).unwrap();
    fs::write(legacy.join("settings.json"), b"existing-secrets").unwrap();
    assert_eq!(
        resolve_data_root(&directory.0, Some(legacy.as_os_str()), Some(&legacy)).unwrap(),
        legacy
    );
    let relative = OsStr::new("custom-data");
    assert_eq!(
        resolve_data_root(&directory.0, Some(relative), None).unwrap(),
        std::path::absolute(relative).unwrap()
    );
    assert_eq!(
        resolve_data_root(&directory.0, Some(OsStr::new("")), None)
            .unwrap_err()
            .kind(),
        io::ErrorKind::InvalidInput
    );
}
