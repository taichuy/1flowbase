use std::{fs, path::Path, process::Command};

fn cargo_metadata(api: &Path) -> serde_json::Value {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--offline",
            "--no-deps",
            "--format-version",
            "1",
            "--manifest-path",
        ])
        .arg(api.join("Cargo.toml"))
        .output()
        .expect("cargo metadata should start");
    assert!(
        output.status.success(),
        "cargo metadata failed: {}",
        String::from_utf8_lossy(&output.stderr)
    );
    serde_json::from_slice(&output.stdout).expect("cargo metadata should emit JSON")
}

fn package<'a>(metadata: &'a serde_json::Value, name: &str) -> &'a serde_json::Value {
    metadata["packages"]
        .as_array()
        .expect("metadata packages should be an array")
        .iter()
        .find(|package| package["name"] == name)
        .unwrap_or_else(|| panic!("metadata should contain {name}"))
}

fn dependency_crate_names(package: &serde_json::Value, dependency_name: &str) -> Vec<String> {
    package["dependencies"]
        .as_array()
        .expect("package dependencies should be an array")
        .iter()
        .filter(|dependency| dependency["name"] == dependency_name)
        .map(|dependency| {
            dependency["rename"]
                .as_str()
                .unwrap_or(dependency_name)
                .replace('-', "_")
        })
        .collect()
}

fn forbidden_dependency_edges(package: &serde_json::Value, forbidden: &[&str]) -> Vec<String> {
    forbidden
        .iter()
        .filter(|name| !dependency_crate_names(package, name).is_empty())
        .map(|name| (*name).to_string())
        .collect()
}

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

fn source_contains_crate_reference(source: &str, crate_name: &str) -> bool {
    source.contains(&format!("{crate_name}::"))
}

fn source_violations(directory: &Path, forbidden: &[String]) -> Vec<String> {
    let mut files = Vec::new();
    collect_rs_files(directory, &mut files);
    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("Rust source should be readable");
        for crate_name in forbidden {
            if source_contains_crate_reference(&source, crate_name) {
                violations.push(format!("{} contains {crate_name}::", file.display()));
            }
        }
    }
    violations.sort();
    violations
}

#[test]
fn controlled_negative_detects_renamed_dependency_and_source_reference() {
    let fixture = serde_json::json!({
        "name": "storage-durable-postgres",
        "dependencies": [{"name": "control-plane", "rename": "cp"}]
    });
    assert_eq!(
        forbidden_dependency_edges(&fixture, &["control-plane"]),
        vec!["control-plane"]
    );
    assert_eq!(
        dependency_crate_names(&fixture, "control-plane"),
        vec!["cp"]
    );
    assert!(source_contains_crate_reference(
        "fn invalid() { cp::ports::AuthRepository; }",
        "cp"
    ));
}

#[test]
fn postgres_adapter_depends_only_on_stable_owners() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let metadata = cargo_metadata(&api);
    let adapter = package(&metadata, "storage-durable-postgres");
    let forbidden = [
        "control-plane",
        "plugin-framework",
        "runtime-core",
        "access-control",
    ];
    assert_eq!(
        forbidden_dependency_edges(adapter, &forbidden),
        Vec::<String>::new()
    );

    let forbidden_crates: Vec<String> = forbidden
        .iter()
        .map(|name| name.replace('-', "_"))
        .collect();
    assert_eq!(
        source_violations(
            &api.join("crates/storage/durable/postgres/src"),
            &forbidden_crates
        ),
        Vec::<String>::new()
    );
}

#[test]
fn protocol_layers_do_not_import_concrete_storage_or_runtime_host_internals() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let metadata = cargo_metadata(&api);
    let api_server = package(&metadata, "api-server");
    let mut concrete_crates = dependency_crate_names(api_server, "storage-durable-postgres");
    concrete_crates.extend(dependency_crate_names(api_server, "runtime-extension-host"));

    let mut violations = Vec::new();
    for directory in [
        api.join("apps/api-server/src/routes"),
        api.join("apps/api-server/src/controllers"),
    ] {
        if directory.exists() {
            violations.extend(source_violations(&directory, &concrete_crates));
        }
    }
    assert_eq!(violations, Vec::<String>::new());
}
