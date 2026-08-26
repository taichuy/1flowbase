use std::{collections::BTreeSet, fs, path::PathBuf};

use extension_package_runtime::{
    parse_plugin_manifest, PluginConsumptionKind, PluginExecutionMode,
};

#[test]
#[ignore = "requires ONEFLOWBASE_OFFICIAL_PLUGIN_ROOT or the adjacent official plugin repository"]
fn d_008_eight_official_runtime_extensions_keep_the_published_contract_matrix() {
    let official_root = std::env::var_os("ONEFLOWBASE_OFFICIAL_PLUGIN_ROOT")
        .map(PathBuf::from)
        .unwrap_or_else(|| {
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../../..")
                .join("1flowbase-official-plugins")
        });
    let extension_root = official_root.join("runtime-extensions/@taichuy");
    let expected = BTreeSet::from([
        "aliyun_bailian".to_string(),
        "anthropic".to_string(),
        "chatgpt-codex".to_string(),
        "clash-proxy".to_string(),
        "deepseek".to_string(),
        "gemini".to_string(),
        "openai".to_string(),
        "openai_compatible".to_string(),
    ]);
    let mut actual = BTreeSet::new();
    let mut modes = BTreeSet::new();
    let mut protocols = BTreeSet::new();

    for entry in fs::read_dir(&extension_root).expect("official runtime-extension directory") {
        let entry = entry.expect("official runtime-extension entry");
        if !entry.path().is_dir() {
            continue;
        }
        let manifest_path = entry.path().join("manifest.yaml");
        if !manifest_path.is_file() {
            continue;
        }
        let raw = fs::read_to_string(&manifest_path).expect("official manifest");
        let manifest = parse_plugin_manifest(&raw).unwrap_or_else(|error| {
            panic!(
                "{} must match extension-contracts: {error}",
                manifest_path.display()
            )
        });
        assert_eq!(
            manifest.consumption_kind,
            PluginConsumptionKind::RuntimeExtension
        );
        assert!(matches!(
            manifest.execution_mode,
            PluginExecutionMode::ProcessPerCall
                | PluginExecutionMode::StatefulProviderWorker
                | PluginExecutionMode::StatefulRuntimeWorker
        ));
        assert!(matches!(
            manifest.contract_version.as_str(),
            "1flowbase.provider/v2" | "1flowbase.network_egress_provider/v1"
        ));
        assert!(matches!(
            manifest.runtime.protocol.as_str(),
            "stdio_json" | "stdio_json_worker"
        ));
        actual.insert(manifest.plugin_id.clone());
        modes.insert(manifest.execution_mode.as_str());
        protocols.insert(manifest.runtime.protocol.clone());
    }

    assert_eq!(actual, expected);
    assert_eq!(
        modes,
        BTreeSet::from([
            "process_per_call",
            "stateful_provider_worker",
            "stateful_runtime_worker",
        ])
    );
    assert_eq!(
        protocols,
        BTreeSet::from(["stdio_json".to_string(), "stdio_json_worker".to_string()])
    );
}
