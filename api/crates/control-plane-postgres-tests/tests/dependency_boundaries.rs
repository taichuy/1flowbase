use std::{fs, path::Path};

fn collect_rs_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).expect("boundary directory should be readable") {
        let path = entry.expect("boundary entry should be readable").path();
        if path.is_dir() {
            collect_rs_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn assert_source_excludes(directory: &Path, forbidden: &[&str]) {
    let mut files = Vec::new();
    collect_rs_files(directory, &mut files);
    for file in files {
        let source = fs::read_to_string(&file).expect("Rust source should be readable");
        for pattern in forbidden {
            assert!(
                !source.contains(pattern),
                "{} contains forbidden dependency reference {pattern}",
                file.display()
            );
        }
    }
}

#[test]
fn postgres_adapter_depends_only_on_stable_owners() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let adapter = api.join("crates/storage/durable/postgres");
    let manifest = fs::read_to_string(adapter.join("Cargo.toml"))
        .expect("PostgreSQL adapter manifest should be readable");
    for package in [
        "control-plane",
        "plugin-framework",
        "runtime-core",
        "access-control",
    ] {
        assert!(
            !manifest.lines().any(|line| {
                line.trim_start().starts_with(package)
                    && line[package.len()..].trim_start().starts_with('=')
            }),
            "storage-durable-postgres must not depend on {package}"
        );
    }
    assert_source_excludes(
        &adapter.join("src"),
        &[
            "control_plane::",
            "plugin_framework::",
            "runtime_core::",
            "access_control::",
        ],
    );
}

#[test]
fn protocol_layers_do_not_import_concrete_storage_or_runtime_host_internals() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    for directory in [
        api.join("apps/api-server/src/routes"),
        api.join("apps/api-server/src/controllers"),
    ] {
        if directory.exists() {
            assert_source_excludes(
                &directory,
                &["storage_durable_postgres::", "runtime_extension_host::"],
            );
        }
    }
}
