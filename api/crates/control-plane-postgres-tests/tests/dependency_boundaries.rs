use std::{collections::BTreeMap, fs, path::Path, process::Command};

const FORBIDDEN_EDGES: &[(&str, &str)] = &[
    ("interface-runtime", "api-server"),
    ("interface-runtime", "access-control"),
    ("interface-runtime", "control-plane"),
    ("interface-runtime", "control-plane-contracts"),
    ("interface-runtime", "plugin-framework"),
    ("interface-runtime", "runtime-core"),
    ("interface-runtime", "runtime-extension-host"),
    ("interface-runtime", "storage-durable"),
    ("interface-runtime", "storage-durable-postgres"),
    ("interface-runtime", "storage-ephemeral"),
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
    ("runtime-extension-sdk", "orchestration-runtime"),
    ("runtime-extension-sdk", "runtime-extension-host"),
    ("runtime-extension-sdk", "runtime-core"),
    ("runtime-extension-sdk", "plugin-framework"),
    ("runtime-extension-sdk", "control-plane"),
    ("runtime-extension-sdk", "api-server"),
    ("sync-js-runtime", "orchestration-runtime"),
    ("sync-js-runtime", "runtime-extension-host"),
    ("sync-js-runtime", "runtime-core"),
    ("sync-js-runtime", "runtime-extension-sdk"),
    ("sync-js-runtime", "extension-contracts"),
];

#[derive(Clone, Debug, Eq, PartialEq)]
struct ResolvedDependency {
    package: String,
    crate_name: String,
}

fn cargo_metadata(api: &Path) -> serde_json::Value {
    cargo_metadata_with_javascript(api, false)
}

fn cargo_metadata_with_javascript(api: &Path, javascript: bool) -> serde_json::Value {
    let mut command = Command::new("cargo");
    command.arg("metadata");
    if javascript {
        command.args(["--features", "runtime-extension-sdk/javascript"]);
    }
    let output = command
        .args([
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
        } else if path.extension().is_some_and(|extension| extension == "rs")
            && !matches!(
                path.file_name().and_then(|name| name.to_str()),
                Some("tests.rs" | "_tests.rs")
            )
        {
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
fn controlled_negative_rejects_interface_runtime_implementation_dependency_alias() {
    let fixture = serde_json::json!({
        "packages": [
            {"id": "interface", "name": "interface-runtime"},
            {"id": "api", "name": "api-server"}
        ],
        "resolve": {"nodes": [
            {"id": "interface", "deps": [{"pkg": "api", "name": "application_alias"}]},
            {"id": "api", "deps": []}
        ]}
    });

    assert_eq!(
        dependency_policy_violations(&fixture),
        vec!["interface-runtime -> api-server (crate alias application_alias)"]
    );
    assert_eq!(
        source_dependency_violations(
            &fixture,
            "interface-runtime",
            "fn invalid() { application_alias::ApiState; }",
        ),
        vec!["interface-runtime source references api-server as application_alias::"]
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
fn protocol_layers_use_explicit_adapters_without_sql_or_runtime_host() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let patterns = protocol_layer_forbidden_patterns();
    let mut violations = Vec::new();
    for directory in [
        api.join("apps/api-server/src/routes"),
        api.join("apps/api-server/src/controllers"),
    ] {
        if directory.exists() {
            violations.extend(source_pattern_violations(&directory, &patterns));
        }
    }
    assert_eq!(violations, Vec::<String>::new());
}

fn protocol_layer_forbidden_patterns() -> [(&'static str, &'static str); 4] {
    [
        ("ApiDurableStore alias", "ApiDurableStore"),
        ("direct sqlx use", "sqlx::"),
        ("raw pool access", ".pool()"),
        (
            "Runtime Host implementation dependency",
            "runtime_extension_host::",
        ),
    ]
}

#[test]
fn protocol_dependency_rule_allows_explicit_durable_adapter_but_rejects_implementation_leaks() {
    let patterns = protocol_layer_forbidden_patterns();
    let allowed = "struct Adapter { store: storage_durable_postgres::MainDurableStore }";
    assert!(patterns
        .iter()
        .all(|(_, pattern)| !allowed.contains(pattern)));

    for denied in [
        "sqlx::query(\"select 1\")",
        "state.store.pool()",
        "runtime_extension_host::RuntimeExtensionHost",
        "type Store = ApiDurableStore",
    ] {
        assert!(
            patterns.iter().any(|(_, pattern)| denied.contains(pattern)),
            "boundary fixture must reject `{denied}`"
        );
    }
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

// The selector is compiler-only tooling, never a runtime/internal service edge.
fn api_test_selection_violations(metadata: &serde_json::Value) -> Vec<String> {
    let packages = metadata["packages"].as_array().expect("metadata packages");
    let mut violations = Vec::new();
    let selector = packages
        .iter()
        .find(|package| package["name"] == "api-test-selection");
    match selector {
        None => violations.push("api-test-selection package must exist".to_string()),
        Some(package) => {
            if !package["dependencies"]
                .as_array()
                .expect("selector dependencies")
                .is_empty()
            {
                violations.push("api-test-selection must have no Cargo dependencies".to_string());
            }
            let proc_macro = package["targets"]
                .as_array()
                .expect("selector targets")
                .iter()
                .any(|target| {
                    target["kind"]
                        .as_array()
                        .expect("target kinds")
                        .iter()
                        .any(|kind| kind == "proc-macro")
                });
            if !proc_macro {
                violations.push("api-test-selection must be a proc-macro crate".to_string());
            }
        }
    }
    let mut api_dev_edges = 0;
    for package in packages {
        let source = package["name"].as_str().expect("package name");
        for dependency in package["dependencies"]
            .as_array()
            .expect("package dependencies")
        {
            // Cargo metadata's name is the actual package, even with a rename.
            if dependency["name"] != "api-test-selection" {
                continue;
            }
            if source == "api-server" && dependency["kind"] == "dev" {
                api_dev_edges += 1;
            } else {
                let alias = dependency["rename"]
                    .as_str()
                    .unwrap_or("api_test_selection");
                violations.push(format!(
                    "{source} -> api-test-selection must be api-server dev-only (crate alias {alias})"
                ));
            }
        }
    }
    if api_dev_edges != 1 {
        violations.push("api-server must declare exactly one selector dev-dependency".to_string());
    }
    violations.sort();
    violations
}

#[test]
fn api_test_selection_is_dependency_free_and_consumed_only_by_api_tests() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    assert_eq!(
        api_test_selection_violations(&cargo_metadata(&api)),
        Vec::<String>::new()
    );
}

#[test]
fn controlled_negative_rejects_selector_runtime_edges_and_dependency_rename() {
    let mut fixture = serde_json::json!({
        "packages": [
            {
                "name": "api-test-selection",
                "dependencies": [],
                "targets": [{"kind": ["proc-macro"]}]
            },
            {
                "name": "api-server",
                "dependencies": [{"name": "api-test-selection", "kind": "dev", "rename": "selector_dev"}]
            },
            {"name": "domain", "dependencies": []}
        ]
    });
    assert!(api_test_selection_violations(&fixture).is_empty());
    fixture["packages"][0]["dependencies"] = serde_json::json!([{"name": "syn"}]);
    fixture["packages"][1]["dependencies"][0]["kind"] = serde_json::Value::Null;
    fixture["packages"][2]["dependencies"] = serde_json::json!([{"name": "api-test-selection", "kind": "dev", "rename": "hidden_selector"}]);
    assert_eq!(
        api_test_selection_violations(&fixture),
        vec![
            "api-server -> api-test-selection must be api-server dev-only (crate alias selector_dev)",
            "api-server must declare exactly one selector dev-dependency",
            "api-test-selection must have no Cargo dependencies",
            "domain -> api-test-selection must be api-server dev-only (crate alias hidden_selector)",
        ]
    );
    fixture["packages"][0]["targets"] = serde_json::json!([{"kind": ["lib"]}]);
    assert!(api_test_selection_violations(&fixture)
        .iter()
        .any(|violation| violation == "api-test-selection must be a proc-macro crate"));
}

#[test]
fn runtime_consumer_fixture_uses_runtime_crates_only_as_dev_dependencies() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let metadata = cargo_metadata(&api);
    let package = metadata["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|package| package["name"] == "control-plane-postgres-tests")
        .unwrap();
    for name in ["runtime-core", "orchestration-runtime"] {
        let dependencies: Vec<_> = package["dependencies"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|dependency| dependency["name"] == name)
            .collect();
        assert_eq!(dependencies.len(), 1, "one direct dependency on {name}");
        assert_eq!(dependencies[0]["kind"], "dev", "{name} is test-only");
    }
}

fn dependency_closure(metadata: &serde_json::Value, source: &str) -> Vec<String> {
    let mut pending = vec![source.to_string()];
    let mut closure = std::collections::BTreeSet::new();
    while let Some(package) = pending.pop() {
        for dependency in resolved_dependencies(metadata, &package) {
            if closure.insert(dependency.package.clone()) {
                pending.push(dependency.package);
            }
        }
    }
    closure.into_iter().collect()
}

#[test]
fn sdk_javascript_is_optional_and_shared_engine_has_no_internal_dependencies() {
    let api = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let off = cargo_metadata(&api);
    let closure = dependency_closure(&off, "runtime-extension-sdk");
    for forbidden in [
        "sync-js-runtime",
        "rquickjs",
        "rquickjs-core",
        "rquickjs-sys",
        "orchestration-runtime",
        "runtime-extension-host",
    ] {
        assert!(
            !closure.iter().any(|name| name == forbidden),
            "default SDK must not pull {forbidden}"
        );
    }
    let on = cargo_metadata_with_javascript(&api, true);
    let closure = dependency_closure(&on, "runtime-extension-sdk");
    for required in ["sync-js-runtime", "rquickjs"] {
        assert!(closure.iter().any(|name| name == required));
    }
    for forbidden in [
        "orchestration-runtime",
        "runtime-extension-host",
        "plugin-framework",
        "runtime-core",
        "control-plane",
        "api-server",
    ] {
        assert!(!closure.iter().any(|name| name == forbidden));
    }
    let local = on["packages"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|p| p["source"].is_null())
        .map(|p| p["name"].as_str().unwrap())
        .collect::<Vec<_>>();
    for dependency in resolved_dependencies(&on, "sync-js-runtime") {
        assert!(
            !local.contains(&dependency.package.as_str()),
            "engine must not depend on internal crate {}",
            dependency.package
        );
    }
    let sdk = on["packages"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "runtime-extension-sdk")
        .unwrap();
    let engine = sdk["dependencies"]
        .as_array()
        .unwrap()
        .iter()
        .find(|p| p["name"] == "sync-js-runtime")
        .unwrap();
    assert_eq!(engine["optional"], true);
    assert_eq!(sdk["features"]["default"], serde_json::json!([]));
    assert_eq!(
        sdk["features"]["javascript"],
        serde_json::json!(["dep:sync-js-runtime"])
    );
}

#[test]
fn dependency_fixture_rejects_sdk_to_orchestration_even_when_renamed() {
    let fixture = serde_json::json!({
        "packages": [{"id":"sdk","name":"runtime-extension-sdk"},
                     {"id":"orchestration","name":"orchestration-runtime"}],
        "resolve":{"nodes":[
            {"id":"sdk","deps":[{"pkg":"orchestration","name":"renamed_engine"}]},
            {"id":"orchestration","deps":[]}
        ]}
    });
    assert_eq!(
        dependency_policy_violations(&fixture),
        vec!["runtime-extension-sdk -> orchestration-runtime (crate alias renamed_engine)"]
    );
}
