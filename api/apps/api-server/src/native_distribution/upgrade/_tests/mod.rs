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
    files::require_space(&f.directory, 0).unwrap();
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

#[test]
fn unchanged_managed_files_skip_rewrite_clean_stale_temps_and_respect_mode_changes() {
    let f = Fixture::new();

    // Unchanged `.env.example` keeps its inode; stale temp file must still be deleted.
    fs::write(f.old.join(".update-.env.example.tmp"), "stale temp").unwrap();

    // Stale managed file present in `old` must be deleted by the new release.
    fs::write(f.old.join("web/obsolete.js"), "legacy bundle").unwrap();
    let mut old_manifest = Manifest::read(&f.old).unwrap();
    old_manifest.files.insert(
        "web/obsolete.js".into(),
        files::hash(&f.old.join("web/obsolete.js")).unwrap(),
    );

    // Identical content with different Unix mode (`0o644` -> `0o755`) must trigger replacement.
    fs::write(f.old.join("start.sh"), "#!/bin/sh\necho ok\n").unwrap();
    fs::write(f.new.join("start.sh"), "#!/bin/sh\necho ok\n").unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(f.old.join("start.sh"), fs::Permissions::from_mode(0o644)).unwrap();
        fs::set_permissions(f.new.join("start.sh"), fs::Permissions::from_mode(0o755)).unwrap();
    }
    old_manifest
        .files
        .insert("start.sh".into(), files::hash(&f.old.join("start.sh")).unwrap());
    files::json(&f.old.join(manifest::NAME), &old_manifest).unwrap();

    // New nested directory asset must be created, synced, and verified.
    fs::create_dir_all(f.new.join("web/assets/nested")).unwrap();
    fs::write(f.new.join("web/assets/nested/app.js"), "console.log('ok');").unwrap();
    let mut new_manifest = Manifest::read(&f.new).unwrap();
    new_manifest
        .files
        .insert("start.sh".into(), files::hash(&f.new.join("start.sh")).unwrap());
    new_manifest.files.insert(
        "web/assets/nested/app.js".into(),
        files::hash(&f.new.join("web/assets/nested/app.js")).unwrap(),
    );
    files::json(&f.new.join(manifest::NAME), &new_manifest).unwrap();

    #[cfg(unix)]
    let env_example_ino = {
        use std::os::unix::fs::MetadataExt;
        fs::metadata(f.old.join(".env.example")).unwrap().ino()
    };

    apply_from(&f.new, &f.old, |_| Ok(()), |_, _| Ok(())).unwrap();

    assert!(!f.old.join(".update-.env.example.tmp").exists());
    assert!(!f.old.join("web/obsolete.js").exists());
    assert_eq!(
        fs::read_to_string(f.old.join("web/assets/nested/app.js")).unwrap(),
        "console.log('ok');"
    );
    #[cfg(unix)]
    {
        use std::os::unix::fs::{MetadataExt, PermissionsExt};
        assert_eq!(
            fs::metadata(f.old.join(".env.example")).unwrap().ino(),
            env_example_ino
        );
        assert_eq!(
            fs::metadata(f.old.join("start.sh"))
                .unwrap()
                .permissions()
                .mode()
                & 0o777,
            0o755
        );
    }
    Manifest::read(&f.old).unwrap().verify(&f.old).unwrap();
}

#[cfg(unix)]
#[test]
fn staged_copy_and_replace_refuse_symlinks_and_sync_unique_directories() {
    use std::os::unix::fs::symlink;

    struct Scratch(PathBuf);
    impl Drop for Scratch {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    let root = Scratch(
        std::env::temp_dir().join(format!("native-files-{}", uuid::Uuid::now_v7())),
    );
    fs::create_dir_all(root.0.join("src/sub/deep")).unwrap();
    fs::write(root.0.join("src/sub/deep/file.txt"), "payload").unwrap();

    let mut dirty = BTreeSet::new();
    files::copy_staged(&root.0.join("src"), &root.0.join("dest"), &mut dirty).unwrap();
    assert!(dirty.contains(&root.0.join("dest")));
    assert!(dirty.contains(&root.0.join("dest/sub")));
    assert!(dirty.contains(&root.0.join("dest/sub/deep")));
    files::sync_directories(&dirty, &root.0.join("dest")).unwrap();
    assert_eq!(
        fs::read_to_string(root.0.join("dest/sub/deep/file.txt")).unwrap(),
        "payload"
    );

    let file_hash = files::hash(&root.0.join("src/sub/deep/file.txt")).unwrap();
    symlink(
        root.0.join("src/sub/deep/file.txt"),
        root.0.join("dest-link"),
    )
    .unwrap();
    assert!(!files::matches_managed_file(
        &root.0.join("src/sub/deep/file.txt"),
        &root.0.join("dest-link"),
        &file_hash
    )
    .unwrap());

    symlink(
        root.0.join("src/sub/deep/file.txt"),
        root.0.join("src/sub/link.txt"),
    )
    .unwrap();
    let mut rejected_dirty = BTreeSet::new();
    assert!(files::copy_staged(
        &root.0.join("src"),
        &root.0.join("dest-symlink"),
        &mut rejected_dirty
    )
    .is_err());
}
