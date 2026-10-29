use super::*;

struct Fixture {
    directory: PathBuf,
    old: PathBuf,
    new: PathBuf,
}
impl Fixture {
    fn new() -> Self {
        let directory =
            std::env::temp_dir().join(format!("native-upgrade-{}", uuid::Uuid::now_v7()));
        fs::create_dir(&directory).unwrap();
        let old = directory.join("old installation");
        let new = directory.join("new package");
        for (root, version, build) in [(&old, "1.0.0", "old"), (&new, "1.1.0", "new")] {
            fs::create_dir(root).unwrap();
            fs::create_dir(root.join("web")).unwrap();
            fs::write(root.join(Manifest::executable()), build).unwrap();
            fs::write(root.join("web/index.html"), format!("<html>{build}</html>")).unwrap();
            fs::write(root.join(".env.example"), "example").unwrap();
            let mut entries = BTreeMap::new();
            for name in [Manifest::executable(), "web/index.html", ".env.example"] {
                entries.insert(name.into(), files::hash(&root.join(name)).unwrap());
            }
            let manifest = Manifest {
                format_version: 1,
                config_schema: 1,
                version: version.into(),
                build_id: build.into(),
                platform: if cfg!(windows) {
                    "windows"
                } else if cfg!(target_os = "macos") {
                    "macos"
                } else {
                    "linux"
                }
                .into(),
                arch: if cfg!(target_arch = "aarch64") {
                    "arm64"
                } else {
                    "x64"
                }
                .into(),
                postgres_major: 18,
                files: entries,
            };
            files::json(&root.join(NAME), &manifest).unwrap();
        }
        fs::create_dir_all(old.join("data/postgres")).unwrap();
        fs::write(old.join("data/postgres/PG_VERSION"), "18").unwrap();
        fs::write(old.join("data/business-file"), "original user content").unwrap();
        let http = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let pg = TcpListener::bind(("127.0.0.1", 0)).unwrap();
        let environment = format!("# preserve exact bytes\nAPI_ENV=production\nAPI_SERVER_ADDR=127.0.0.1:{}\nAPI_DATABASE_URL=postgres://postgres:fixture@127.0.0.1:{}/1flowbase\nAPI_PROVIDER_SECRET_MASTER_KEY=abcdefghijklmnopqrstuvwxyz012345\nBOOTSTRAP_ROOT_ACCOUNT=custom\nBOOTSTRAP_ROOT_EMAIL=custom@example.com\nBOOTSTRAP_ROOT_PASSWORD=custom-password\nBOOTSTRAP_WORKSPACE_NAME=custom workspace\nFLOWBASE_DATA_DIR=data\nAPI_BUSINESS_FILE_LOCAL_ROOT=data/api/storage\nAPI_PROVIDER_INSTALL_ROOT=data/api/plugins\nAPI_HOST_EXTENSION_DROPIN_ROOT=data/api/plugins/dropins\nAPI_MCP_TEMPLATE_LIBRARY_ROOT=data/api/mcp\nAPI_SYSTEM_BACKUP_REPOSITORY_ROOT=data/backups\nAPI_WORKSPACE_ROOT=api\nAPI_MODEL_PRICING_BOOTSTRAP_ROOT=api/resources/model-pricing\nAPI_APPLICATION_TEMPLATE_ROOT=api/resources/application-templates\nAPI_POSTGRES_PG_DUMP_PATH=postgres/bin/pg_dump\nAPI_POSTGRES_PG_RESTORE_PATH=postgres/bin/pg_restore\n",http.local_addr().unwrap().port(),pg.local_addr().unwrap().port());
        fs::write(old.join(".env"), environment).unwrap();
        Self {
            directory,
            old,
            new,
        }
    }
    fn backup(&self) -> PathBuf {
        fs::read_dir(self.directory.join(".old installation-upgrade-backups"))
            .unwrap()
            .next()
            .unwrap()
            .unwrap()
            .path()
    }
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn upgrade_preserves_configuration_and_user_data_and_restores_matching_snapshot() {
    let f = Fixture::new();
    let original = fs::read(f.old.join(".env")).unwrap();
    let backup = upgrade(
        &f.new,
        &f.old,
        |_| Ok(()),
        |target, _| {
            assert_eq!(
                fs::read_to_string(target.join(Manifest::executable()))?,
                "new"
            );
            assert_eq!(fs::read(target.join(".env"))?, original);
            assert_eq!(
                fs::read_to_string(target.join("data/business-file"))?,
                "original user content"
            );
            assert!(target.join(STATE).exists());
            fs::write(target.join("data/migration-result"), "new schema")?;
            Ok(())
        },
    )
    .unwrap();
    assert!(!f.old.join(STATE).exists());
    assert!(backup.join("completed.json").exists());
    restore(&f.old, &backup).unwrap();
    assert_eq!(
        fs::read_to_string(f.old.join(Manifest::executable())).unwrap(),
        "old"
    );
    assert_eq!(fs::read(f.old.join(".env")).unwrap(), original);
    assert!(!f.old.join("data/migration-result").exists());
    assert_eq!(
        fs::read_to_string(f.old.join("data/business-file")).unwrap(),
        "original user content"
    );
}

#[test]
fn failed_migration_retains_blocking_state_and_restore_resumes_partial_replacement() {
    let f = Fixture::new();
    assert!(upgrade(
        &f.new,
        &f.old,
        |_| Ok(()),
        |target, _| {
            fs::write(target.join("data/business-file"), "migration changed data")?;
            bail!("simulated migration failure")
        }
    )
    .is_err());
    assert!(f.old.join(STATE).exists());
    assert!(upgrade(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    // Simulate an interruption after removing some restore targets, including .env.
    fs::remove_file(f.old.join(".env")).unwrap();
    fs::remove_dir_all(f.old.join("web")).unwrap();
    fs::remove_file(f.old.join("data/postgres/PG_VERSION")).unwrap();
    restore(&f.old, &f.backup()).unwrap();
    assert!(!f.old.join(STATE).exists());
    assert_eq!(
        fs::read_to_string(f.old.join("data/business-file")).unwrap(),
        "original user content"
    );
}

#[test]
fn preflight_rejects_corrupt_wrong_platform_running_and_overlapping_targets() {
    let f = Fixture::new();
    let original = fs::read(f.old.join(".env")).unwrap();
    let guard = lock(&f.old.join("data/instance.lock")).unwrap();
    assert!(upgrade(
        &f.new,
        &f.old,
        |_| Ok(()),
        |_, _| panic!("validation must not run")
    )
    .is_err());
    drop(guard);
    fs::write(f.old.join("data/postgres/postmaster.pid"), "123").unwrap();
    assert!(upgrade(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    fs::remove_file(f.old.join("data/postgres/postmaster.pid")).unwrap();
    assert!(upgrade(&f.old, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    let mut manifest = Manifest::read(&f.new).unwrap();
    manifest.postgres_major = 19;
    files::json(&f.new.join(NAME), &manifest).unwrap();
    assert!(upgrade(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    manifest.postgres_major = 18;
    manifest.arch = "wrong".into();
    files::json(&f.new.join(NAME), &manifest).unwrap();
    assert!(upgrade(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    manifest.arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    }
    .into();
    files::json(&f.new.join(NAME), &manifest).unwrap();
    fs::write(f.new.join(Manifest::executable()), "corrupted").unwrap();
    assert!(upgrade(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    assert_eq!(fs::read(f.old.join(".env")).unwrap(), original);
    assert!(!f.old.join(STATE).exists());
}

#[test]
fn corrupted_backup_and_manifest_escape_are_rejected_before_replacement() {
    let f = Fixture::new();
    let backup = upgrade(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).unwrap();
    fs::write(backup.join("snapshot/data/business-file"), "corrupt").unwrap();
    assert!(restore(&f.old, &backup).is_err());
    assert_eq!(
        fs::read_to_string(f.old.join(Manifest::executable())).unwrap(),
        "new"
    );
    let mut manifest = Manifest::read(&f.new).unwrap();
    manifest.files.insert("../escape".into(), "0".repeat(64));
    files::json(&f.new.join(NAME), &manifest).unwrap();
    assert!(Manifest::read(&f.new).is_err());
}

#[test]
fn missing_configuration_and_space_fail_before_replacement() {
    let f = Fixture::new();
    assert!(files::require_space(&f.directory, u64::MAX).is_err());
    let content = fs::read_to_string(f.old.join(".env")).unwrap();
    fs::write(
        f.old.join(".env"),
        content.replace("API_DATABASE_URL=", "IGNORED_DATABASE_URL="),
    )
    .unwrap();
    assert!(upgrade(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    assert_eq!(
        fs::read_to_string(f.old.join(Manifest::executable())).unwrap(),
        "old"
    );
    assert!(!f.old.join(STATE).exists());
}
