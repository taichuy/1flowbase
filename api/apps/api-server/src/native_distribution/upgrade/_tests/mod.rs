use super::*;
use std::collections::BTreeMap;

pub(super) struct Fixture {
    pub(super) directory: PathBuf,
    pub(super) old: PathBuf,
    pub(super) new: PathBuf,
}
impl Fixture {
    pub(super) fn new() -> Self {
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
            files::json(&root.join(manifest::NAME), &manifest).unwrap();
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
}
impl Drop for Fixture {
    fn drop(&mut self) {
        let _ = fs::remove_dir_all(&self.directory);
    }
}

#[test]
fn update_preserves_configuration_and_data_without_creating_backups() {
    let f = Fixture::new();
    let original = fs::read(f.old.join(".env")).unwrap();
    apply_from(
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
            fs::write(target.join("data/migration-result"), "new schema")?;
            Ok(())
        },
    )
    .unwrap();
    assert!(!f.old.join(STATE).exists());
    assert!(!workspace(&f.old).unwrap().exists());
    assert!(!f
        .directory
        .join(".old installation-upgrade-backups")
        .exists());
    assert!(f.old.join("data/migration-result").exists());
    assert_eq!(fs::read(f.old.join(".env")).unwrap(), original);
}
#[test]
fn failed_update_can_resume_without_rolling_back_user_data() {
    let f = Fixture::new();
    assert!(apply_from(
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
    // Emulate an interrupted atomic file write and a partially replaced program tree.
    fs::write(f.old.join("web/.update-index.html.tmp"), "partial new file").unwrap();
    fs::remove_file(f.old.join("web/index.html")).unwrap();
    apply_from(
        &f.new,
        &f.old,
        |_| Ok(()),
        |target, _| {
            assert_eq!(
                fs::read_to_string(target.join("data/business-file"))?,
                "migration changed data"
            );
            Ok(())
        },
    )
    .unwrap();
    assert!(!f.old.join(STATE).exists());
    assert!(!f.old.join("web/.update-index.html.tmp").exists());
    assert!(!workspace(&f.old).unwrap().exists());
}
#[test]
fn preflight_refuses_corruption_running_processes_platform_pg_and_overlap() {
    let f = Fixture::new();
    let original = fs::read(f.old.join(Manifest::executable())).unwrap();
    let guard = lock(&f.old.join("data/instance.lock")).unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| panic!("must not run")).is_err());
    drop(guard);
    fs::write(f.old.join("data/postgres/postmaster.pid"), "123").unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    fs::remove_file(f.old.join("data/postgres/postmaster.pid")).unwrap();
    assert!(apply_from(&f.old, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    let mut manifest = Manifest::read(&f.new).unwrap();
    manifest.postgres_major = 19;
    files::json(&f.new.join(manifest::NAME), &manifest).unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    manifest.postgres_major = 18;
    manifest.arch = "wrong".into();
    files::json(&f.new.join(manifest::NAME), &manifest).unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    manifest.arch = if cfg!(target_arch = "aarch64") {
        "arm64"
    } else {
        "x64"
    }
    .into();
    files::json(&f.new.join(manifest::NAME), &manifest).unwrap();
    fs::write(f.new.join(Manifest::executable()), "corrupted").unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    assert_eq!(
        fs::read(f.old.join(Manifest::executable())).unwrap(),
        original
    );
    assert!(!f.old.join(STATE).exists());
}
#[test]
fn corrupted_pending_package_and_unsafe_manifest_are_not_applied() {
    let f = Fixture::new();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| bail!("fail startup")).is_err());
    let work = workspace(&f.old).unwrap();
    fs::write(work.join("package/web/index.html"), "corrupted").unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| panic!("must not run")).is_err());
    let mut manifest = Manifest::read(&f.new).unwrap();
    manifest.files.insert("../data".into(), "0".repeat(64));
    files::json(&f.new.join(manifest::NAME), &manifest).unwrap();
    assert!(Manifest::read(&f.new).is_err());
}
#[test]
fn missing_configuration_and_insufficient_space_do_not_replace_program() {
    let f = Fixture::new();
    assert!(files::require_space(&f.directory, u64::MAX).is_err());
    assert!(apply_from(
        &f.new,
        &f.old,
        |_| bail!("new required configuration missing"),
        |_, _| Ok(())
    )
    .is_err());
    let env = fs::read_to_string(f.old.join(".env")).unwrap();
    fs::write(
        f.old.join(".env"),
        env.replace("API_DATABASE_URL=", "IGNORED_DATABASE_URL="),
    )
    .unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).is_err());
    assert_eq!(
        fs::read_to_string(f.old.join(Manifest::executable())).unwrap(),
        "old"
    );
    assert!(!f.old.join(STATE).exists());
}

#[test]
fn corrective_release_removes_assets_and_temporary_files_from_failed_attempts() {
    let f = Fixture::new();
    fs::write(f.new.join("web/failed-release.js"), "failed release asset").unwrap();
    let mut manifest = Manifest::read(&f.new).unwrap();
    manifest.files.insert(
        "web/failed-release.js".into(),
        files::hash(&f.new.join("web/failed-release.js")).unwrap(),
    );
    files::json(&f.new.join(manifest::NAME), &manifest).unwrap();
    assert!(apply_from(&f.new, &f.old, |_| Ok(()), |_, _| bail!("failed release")).is_err());
    fs::write(
        f.old.join("web/.update-failed-release.js.tmp"),
        "partial staged file",
    )
    .unwrap();
    fs::remove_file(f.new.join("web/failed-release.js")).unwrap();
    manifest.files.remove("web/failed-release.js");
    manifest.version = "1.2.0".into();
    manifest.build_id = "corrective-release".into();
    files::json(&f.new.join(manifest::NAME), &manifest).unwrap();
    apply_from(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).unwrap();
    assert!(!f.old.join("web/failed-release.js").exists());
    assert!(!f.old.join("web/.update-failed-release.js.tmp").exists());
    Manifest::read(&f.old).unwrap().verify(&f.old).unwrap();
}
