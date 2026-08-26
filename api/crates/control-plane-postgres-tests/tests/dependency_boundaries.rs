use std::{collections::BTreeMap, fs, path::Path, process::Command};

const FORBIDDEN_EDGES: &[(&str, &str)] = &[
    ("storage-durable", "storage-durable-postgres"),
    ("storage-durable-postgres", "control-plane"),
    ("storage-durable-postgres", "plugin-framework"),
    ("storage-durable-postgres", "runtime-core"),
    ("storage-durable-postgres", "access-control"),
    ("storage-ephemeral", "control-plane"),
    ("orchestration-runtime", "plugin-framework"),
    ("orchestration-runtime", "runtime-extension-host"),
    ("control-plane", "runtime-extension-host"),
    ("runtime-core", "plugin-framework"),
    ("runtime-core", "runtime-extension-host"),
    ("runtime-profile", "plugin-framework"),
    ("runtime-extension-host", "plugin-framework"),
    ("control-plane-contracts", "control-plane"),
    ("api-server", "publish-gateway"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedDependency {
    package: String,
    crate_name: String,
}

fn cargo_metadata(api: &Path) -> serde_json::Value {
    let output = Command::new("cargo")
        .args([
            "metadata",
            "--locked",
            "--offline",
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

fn package_names_by_id(metadata: &serde_json::Value) -> BTreeMap<String, String> {
    metadata["packages"]
        .as_array()
        .expect("metadata packages should be an array")
        .iter()
        .map(|package| {
            (
                package["id"].as_str().expect("package id").to_owned(),
                package["name"].as_str().expect("package name").to_owned(),
            )
        })
        .collect()
}

fn resolved_dependencies(metadata: &serde_json::Value, source: &str) -> Vec<ResolvedDependency> {
    let names = package_names_by_id(metadata);
    let Some(source_id) = names
        .iter()
        .find_map(|(id, name)| (name == source).then_some(id))
    else {
        return Vec::new();
    };
    let node = metadata["resolve"]["nodes"]
        .as_array()
        .expect("resolved nodes should be an array")
        .iter()
        .find(|node| node["id"].as_str() == Some(source_id))
        .unwrap_or_else(|| panic!("resolved graph should contain {source}"));

    node["deps"]
        .as_array()
        .expect("resolved dependencies should be an array")
        .iter()
        .map(|dependency| {
            let package_id = dependency["pkg"].as_str().expect("dependency package id");
            ResolvedDependency {
                package: names
                    .get(package_id)
                    .unwrap_or_else(|| panic!("missing package name for {package_id}"))
                    .clone(),
                crate_name: dependency["name"]
                    .as_str()
                    .expect("dependency crate name")
                    .to_owned(),
            }
        })
        .collect()
}

fn collect_production_rs_files(directory: &Path, files: &mut Vec<std::path::PathBuf>) {
    for entry in fs::read_dir(directory).expect("boundary directory should be readable") {
        let path = entry.expect("boundary entry should be readable").path();
        if path.is_dir() {
            let name = path.file_name().and_then(|name| name.to_str());
            if matches!(name, Some("tests" | "_tests")) {
                continue;
            }
            collect_production_rs_files(&path, files);
        } else if path.extension().is_some_and(|extension| extension == "rs") {
            files.push(path);
        }
    }
}

fn source_pattern_violations(directory: &Path, patterns: &[(&str, &str)]) -> Vec<String> {
    let mut files = Vec::new();
    collect_production_rs_files(directory, &mut files);
    let mut violations = Vec::new();
    for file in files {
        let source = fs::read_to_string(&file).expect("Rust source should be readable");
        for (label, pattern) in patterns {
            if source.contains(pattern) {
                violations.push(format!("{} contains {label}", file.display()));
            }
        }
    }
    violations.sort();
    violations
}

fn dependency_policy_violations(metadata: &serde_json::Value) -> Vec<String> {
    let mut violations = Vec::new();
    for (source, forbidden_target) in FORBIDDEN_EDGES {
        for dependency in resolved_dependencies(metadata, source) {
            if dependency.package == *forbidden_target {
                violations.push(format!(
                    "{source} -> {forbidden_target} (crate alias {})",
                    dependency.crate_name
                ));
            }
        }
    }
    violations.sort();
    violations
}

fn source_dependency_violations(
    metadata: &serde_json::Value,
    source: &str,
    source_text: &str,
) -> Vec<String> {
    let forbidden_targets: Vec<&str> = FORBIDDEN_EDGES
        .iter()
        .filter_map(|(candidate, target)| (*candidate == source).then_some(*target))
        .collect();
    let mut violations = resolved_dependencies(metadata, source)
        .into_iter()
        .filter(|dependency| forbidden_targets.contains(&dependency.package.as_str()))
        .filter(|dependency| source_text.contains(&format!("{}::", dependency.crate_name)))
        .map(|dependency| {
            format!(
                "{source} source references {} as {}::",
                dependency.package, dependency.crate_name
            )
        })
        .collect::<Vec<_>>();
    violations.sort();
    violations
}

#[test]
fn controlled_negative_runs_complete_policy_and_detects_dependency_rename() {
    let fixture = serde_json::json!({
        "packages": [
            {"id": "storage", "name": "storage-durable-postgres"},
            {"id": "control", "name": "control-plane"},
            {"id": "plugin", "name": "plugin-framework"},
            {"id": "runtime", "name": "runtime-core"},
            {"id": "acl", "name": "access-control"}
        ],
        "resolve": {"nodes": [
            {"id": "storage", "deps": [
                {"pkg": "control", "name": "cp_alias"},
                {"pkg": "plugin", "name": "plugin_alias"}
            ]},
            {"id": "control", "deps": []},
            {"id": "plugin", "deps": []},
            {"id": "runtime", "deps": []},
            {"id": "acl", "deps": []}
        ]}
    });

    assert_eq!(
        dependency_policy_violations(&fixture),
        vec![
            "storage-durable-postgres -> control-plane (crate alias cp_alias)",
            "storage-durable-postgres -> plugin-framework (crate alias plugin_alias)",
        ]
    );
    assert_eq!(
        source_dependency_violations(
            &fixture,
            "storage-durable-postgres",
            "fn invalid() { cp_alias::ports::AuthRepository; plugin_alias::Manifest; }",
        ),
        vec![
            "storage-durable-postgres source references control-plane as cp_alias::",
            "storage-durable-postgres source references plugin-framework as plugin_alias::",
        ]
    );
}

#[test]
fn workspace_dependency_graph_respects_layer_boundaries() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let metadata = cargo_metadata(&api);
    let package_names = package_names_by_id(&metadata)
        .into_values()
        .collect::<Vec<_>>();
    for (source, _) in FORBIDDEN_EDGES {
        assert!(
            package_names.iter().any(|name| name == source),
            "dependency policy source package {source} must exist"
        );
    }
    let mut violations = dependency_policy_violations(&metadata);

    for source in ["storage-durable-postgres", "runtime-extension-host"] {
        let source_directory = match source {
            "storage-durable-postgres" => api.join("crates/storage/durable/postgres/src"),
            "runtime-extension-host" => api.join("crates/runtime-extension-host/src"),
            _ => unreachable!(),
        };
        let mut files = Vec::new();
        collect_production_rs_files(&source_directory, &mut files);
        let joined_source = files
            .iter()
            .map(|file| fs::read_to_string(file).expect("Rust source should be readable"))
            .collect::<Vec<_>>()
            .join("\n");
        violations.extend(source_dependency_violations(
            &metadata,
            source,
            &joined_source,
        ));
    }

    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn protocol_layers_use_services_without_concrete_storage_or_sql() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let metadata = cargo_metadata(&api);
    let api_dependencies = resolved_dependencies(&metadata, "api-server");
    let mut patterns = vec![
        ("ApiDurableStore alias", "ApiDurableStore".to_owned()),
        ("direct sqlx use", "sqlx::".to_owned()),
        ("raw pool access", ".pool()".to_owned()),
    ];
    for dependency in api_dependencies.iter().filter(|dependency| {
        matches!(
            dependency.package.as_str(),
            "storage-durable-postgres" | "runtime-extension-host"
        )
    }) {
        patterns.push((
            "concrete dependency reference",
            format!("{}::", dependency.crate_name),
        ));
    }
    let borrowed_patterns = patterns
        .iter()
        .map(|(label, pattern)| (*label, pattern.as_str()))
        .collect::<Vec<_>>();
    let mut violations = Vec::new();
    for directory in [
        api.join("apps/api-server/src/routes"),
        api.join("apps/api-server/src/controllers"),
    ] {
        if directory.exists() {
            violations.extend(source_pattern_violations(&directory, &borrowed_patterns));
        }
    }
    assert_eq!(violations, Vec::<String>::new());
}

#[test]
fn d_004_plugin_runner_is_absent_from_the_workspace_and_production_dependencies() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let metadata = cargo_metadata(&api);
    let package_names = package_names_by_id(&metadata)
        .into_values()
        .collect::<Vec<_>>();
    assert!(!package_names.iter().any(|name| name == "plugin-runner"));
    assert!(!api.join("apps/plugin-runner").exists());
}
